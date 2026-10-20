// SPDX-License-Identifier: GPL-3.0-or-later
use super::*;
use serde_json::Value;
use std::collections::BTreeMap;

pub(super) fn changes(
    conn: &Connection,
    id: &str,
    revision: i64,
) -> Result<BTreeMap<String, String>> {
    let mut result = BTreeMap::new();
    if revision == 0 {
        return Ok(result);
    }
    let before = record(conn, id, Some(revision - 1))?.document;
    let after = record(conn, id, Some(revision))?.document;
    for node in &after.nodes {
        let old = before.nodes.iter().find(|old| old.id == node.id);
        if let Some(old) = old {
            changed_node(old, node, &mut result)?;
        } else {
            result.insert(format!("{}/added", node.name), String::new());
        }
    }
    for node in &before.nodes {
        if !after.nodes.iter().any(|new| new.id == node.id) {
            result.insert(format!("{}/removed", node.name), String::new());
        }
    }
    diff(
        &serde_json::to_value(&before.output)?,
        &serde_json::to_value(&after.output)?,
        "output",
        &mut result,
    );
    if before.output_node != after.output_node {
        result.insert("output_node".into(), String::new());
    }
    if before.source.raw_white_balance != after.source.raw_white_balance {
        result.insert("raw_white_balance".into(), String::new());
    }
    Ok(result)
}

fn changed_node(
    old: &iris_render::document::Node,
    node: &iris_render::document::Node,
    result: &mut BTreeMap<String, String>,
) -> Result<()> {
    if old.name != node.name {
        result.insert(format!("{}/name", node.name), String::new());
    }
    if old.inputs != node.inputs {
        result.insert(format!("{}/inputs", node.name), String::new());
    }
    diff(
        &serde_json::to_value(&old.operation)?,
        &serde_json::to_value(&node.operation)?,
        &node.name,
        result,
    );
    if old.enabled != node.enabled {
        result.insert(format!("{}/enabled", node.name), node.enabled.to_string());
    }
    Ok(())
}

fn diff(before: &Value, after: &Value, path: &str, result: &mut BTreeMap<String, String>) {
    if before == after {
        return;
    }
    if let (Some(a), Some(b)) = (before.as_object(), after.as_object()) {
        for (key, value) in b {
            diff(
                a.get(key).unwrap_or(&Value::Null),
                value,
                &format!("{path}/{key}"),
                result,
            );
        }
        for key in a.keys().filter(|key| !b.contains_key(*key)) {
            result.insert(format!("{path}/{key}"), String::new());
        }
    } else {
        // Arrays (curves, brush strokes) are summarized, never copied into the history list.
        let value = match after {
            Value::Number(value) => value.to_string(),
            Value::Bool(value) => value.to_string(),
            _ => String::new(),
        };
        result.insert(path.into(), value);
    }
}
