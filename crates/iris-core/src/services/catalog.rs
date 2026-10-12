use super::*;
use rusqlite::types::Value as SqlValue;

// The projection contains only query fields. Full analysis JSON is joined after
// filtering, stable sorting and LIMIT; offscreen faces/vectors are not decoded.
const CANDIDATES: &str = "SELECT p.id,p.path,p.format,p.size_bytes,p.taken_at,p.mtime,
    COALESCE((SELECT action FROM decisions d WHERE d.photo_id=p.id AND d.undone_at IS NULL ORDER BY d.id DESC LIMIT 1),'pending') AS decision,
    COALESCE((SELECT rating FROM photo_marks m WHERE m.photo_id=p.id AND m.undone_at IS NULL ORDER BY m.id DESC LIMIT 1),0) AS rating,
    COALESCE((SELECT color_label FROM photo_marks m WHERE m.photo_id=p.id AND m.undone_at IS NULL ORDER BY m.id DESC LIMIT 1),'none') AS color,
    CASE WHEN c.version=?2 AND c.settings_key=?3 AND c.source_invalid=0 THEN c.score ELSE -1.0 END AS score,
    CASE WHEN c.version=?2 AND c.settings_key=?3 AND c.source_invalid=0 THEN c.verdict END AS verdict
    FROM photos p LEFT JOIN analysis_catalog c ON c.photo_id=p.id WHERE p.project_id=?1";

impl Services {
    pub(super) fn catalog_page(&self, project_id: i64, filter: &PhotoFilter) -> Result<Vec<Photo>> {
        let settings = self.settings(project_id)?;
        // serde Value and typed settings serialize object fields differently;
        // match the typed normalization used by the write-time projection.
        let key = serde_json::to_string(
            &serde_json::from_value::<crate::vision::AnalysisSettings>(settings.clone())?,
        )?;
        let mut values = vec![
            SqlValue::Integer(project_id),
            SqlValue::Text(crate::vision::ANALYSIS_VERSION.into()),
            SqlValue::Text(key),
        ];
        let available = if filter.include_missing.unwrap_or(false) {
            ""
        } else {
            " AND p.missing=0 AND p.quarantined=0"
        };
        let mut conditions = filter_conditions(filter, &mut values);
        if let Some(scene_id) = &filter.scene_id {
            anyhow::ensure!(scene_id.len() <= 128, "invalid scene id");
            let ids = self
                .scenes(project_id)?
                .into_iter()
                .find(|scene| &scene.id == scene_id)
                .map(|scene| scene.member_photo_ids)
                .unwrap_or_default();
            let parameter = bind(&mut values, SqlValue::Text(serde_json::to_string(&ids)?));
            conditions.push_str(&format!(
                " AND page.id IN (SELECT value FROM json_each({parameter}))"
            ));
        }
        let order = catalog_order(filter)?;
        let limit = bind(&mut values, integer(filter.limit.unwrap_or(usize::MAX)));
        let offset = bind(&mut values, integer(filter.offset.unwrap_or(0)));
        let sql = format!(
            "WITH candidates AS ({CANDIDATES}{available}),
             page AS MATERIALIZED (SELECT * FROM candidates AS page WHERE 1=1{conditions}
                ORDER BY {order} LIMIT {limit} OFFSET {offset})
             {PHOTO_SELECT} JOIN page ON page.id=p.id ORDER BY {order}"
        );
        let mut query = self.store.conn.prepare(&sql)?;
        let mut photos = query
            .query_map(rusqlite::params_from_iter(values), photo_row)?
            .collect::<Result<Vec<_>, _>>()?;
        for photo in &mut photos {
            photo.analysis_status = analysis_status(photo.analysis.as_ref(), &settings);
        }
        Ok(photos)
    }
}

fn integer(value: usize) -> SqlValue {
    SqlValue::Integer(value.min(i64::MAX as usize) as i64)
}

fn bind(values: &mut Vec<SqlValue>, value: SqlValue) -> String {
    values.push(value);
    format!("?{}", values.len())
}

fn filter_conditions(filter: &PhotoFilter, values: &mut Vec<SqlValue>) -> String {
    let color = filter.color_label.map(|value| value.as_str().to_owned());
    let fields = [
        (
            "decision",
            filter.decision.map(|value| value.as_str().into()),
        ),
        ("color", color),
        ("format", filter.format.clone()),
        ("verdict", filter.verdict.clone()),
    ];
    let mut sql = String::new();
    for (column, value) in fields {
        if let Some(value) = value {
            let parameter = bind(values, SqlValue::Text(value));
            sql.push_str(&format!(" AND {column}={parameter}"));
        }
    }
    if let Some(rating) = filter.rating {
        let parameter = bind(values, SqlValue::Integer(rating.into()));
        sql.push_str(&format!(" AND rating={parameter}"));
    }
    if let Some(id) = filter.photo_id {
        let parameter = bind(values, SqlValue::Integer(id));
        sql.push_str(&format!(" AND page.id={parameter}"));
    }
    sql
}

fn catalog_order(filter: &PhotoFilter) -> Result<String> {
    let reverse = filter.descending.unwrap_or(false);
    let direction = if reverse { "DESC" } else { "ASC" };
    // All components reverse together, matching beta3's final Vec::reverse.
    // Explicit ids keep ties stable across query plans and page boundaries.
    Ok(match filter.sort.as_deref().unwrap_or("name") {
        "name" => format!("page.path COLLATE BINARY {direction},page.id {direction}"),
        "score" | "suggestion" => format!(
            "page.score {},page.id {direction}",
            if reverse { "ASC" } else { "DESC" }
        ),
        "size" => format!("page.size_bytes {direction},page.id {direction}"),
        "date" => format!(
            "COALESCE(page.taken_at,'') {direction},page.mtime {direction},page.id {direction}"
        ),
        _ => bail!("unsupported photo sort"),
    })
}
