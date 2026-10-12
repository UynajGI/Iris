//! Edit only Iris-owned selection properties; preserve all other XMP source bytes.
use crate::domain::{ColorLabel, Photo};
use anyhow::{Context, Result};
use roxmltree::{Document, Node};
use std::ops::Range;
use std::path::Path;

pub(super) const MAX_BYTES: u64 = 8 * 1024 * 1024;
const RDF: &str = "http://www.w3.org/1999/02/22-rdf-syntax-ns#";
const XMP: &str = "http://ns.adobe.com/xap/1.0/";
const IRIS: &str = "urn:iris:selection:1.0";

fn owned(namespace: Option<&str>, name: &str) -> bool {
    (namespace == Some(XMP) && matches!(name, "Rating" | "Label"))
        || (namespace == Some(IRIS) && name == "Decision")
}

fn label(color: ColorLabel) -> &'static str {
    match color {
        ColorLabel::None => "",
        ColorLabel::Red => "Red",
        ColorLabel::Yellow => "Yellow",
        ColorLabel::Green => "Green",
        ColorLabel::Blue => "Blue",
        ColorLabel::Purple => "Purple",
    }
}

fn description(photo: &Photo, subject: &str) -> Result<String> {
    anyhow::ensure!(photo.rating <= 5, "invalid photo rating");
    // Decisions and stars are independent. Reject is not encoded as Rating=-1,
    // which would erase a separately assigned star rating in other applications.
    let subject = subject
        .replace('&', "&amp;")
        .replace('\'', "&apos;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;");
    Ok(format!(
        "<rdf:Description xmlns:rdf='{RDF}' rdf:about='{subject}' xmlns:xmp='{XMP}' xmlns:iris='{IRIS}' xmp:Rating='{}' xmp:Label='{}' iris:Decision='{}'/>",
        photo.rating, label(photo.color_label), photo.decision.as_str()
    ))
}

fn describes_photo(node: Node<'_, '_>, photo: &Photo, source: &Path) -> bool {
    if !node.has_tag_name((RDF, "Description")) {
        return false;
    }
    let about = node.attribute((RDF, "about")).unwrap_or("");
    if about.is_empty() || about == photo.filename || about == photo.path {
        return true;
    }
    let Ok(base) = url::Url::from_file_path(source) else {
        return false;
    };
    let Ok(resolved) = base.join(about) else {
        return false;
    };
    if resolved.scheme() != "file" || resolved.query().is_some() || resolved.fragment().is_some() {
        return false;
    }
    let (Ok(actual), Ok(expected)) = (resolved.to_file_path(), base.to_file_path()) else {
        return false;
    };
    #[cfg(windows)]
    {
        actual
            .to_string_lossy()
            .eq_ignore_ascii_case(&expected.to_string_lossy())
    }
    #[cfg(not(windows))]
    {
        actual == expected
    }
}

fn removed_properties(rdf: Node<'_, '_>, photo: &Photo, source: &Path) -> Vec<Range<usize>> {
    let mut ranges = Vec::new();
    for node in rdf
        .children()
        .filter(|n| describes_photo(*n, photo, source))
    {
        if owns_entire_description(node) {
            ranges.push(node.range());
            continue;
        }
        ranges.extend(
            node.attributes()
                .filter(|a| owned(a.namespace(), a.name()))
                .map(|a| a.range()),
        );
        ranges.extend(
            node.children()
                .filter(|n| n.is_element() && owned(n.tag_name().namespace(), n.tag_name().name()))
                .map(|n| n.range()),
        );
    }
    ranges
}

fn owns_entire_description(node: Node<'_, '_>) -> bool {
    let has_owned = node.attributes().any(|a| owned(a.namespace(), a.name()))
        || node
            .children()
            .any(|n| n.is_element() && owned(n.tag_name().namespace(), n.tag_name().name()));
    has_owned
        && node.attributes().all(|a| {
            owned(a.namespace(), a.name()) || (a.namespace() == Some(RDF) && a.name() == "about")
        })
        && node.children().all(|n| {
            (n.is_text() && n.text().is_none_or(|s| s.trim().is_empty()))
                || (n.is_element() && owned(n.tag_name().namespace(), n.tag_name().name()))
        })
}

pub(super) fn merge(photo: &Photo, original: Option<&str>, path: &Path) -> Result<String> {
    let Some(original) = original else {
        let description = description(photo, "")?;
        return Ok(format!(
            "<?xpacket begin='\u{feff}' id='W5M0MpCehiHzreSzNTczkc9d'?>\n<x:xmpmeta xmlns:x='adobe:ns:meta/'><rdf:RDF xmlns:rdf='{RDF}'>{description}</rdf:RDF></x:xmpmeta>\n<?xpacket end='w'?>\n"
        ));
    };
    anyhow::ensure!(
        original.len() as u64 <= MAX_BYTES,
        "XMP sidecar exceeds size limit"
    );
    // roxmltree rejects DTDs by default; no external entity or network resolution.
    let document = Document::parse(original).context("invalid XMP; original was not changed")?;
    let roots: Vec<_> = document
        .descendants()
        .filter(|n| n.has_tag_name((RDF, "RDF")))
        .collect();
    anyhow::ensure!(roots.len() == 1, "XMP must contain exactly one RDF root");
    let rdf = roots[0];
    let subject = rdf
        .children()
        .filter(|n| describes_photo(*n, photo, path))
        .filter_map(|n| n.attribute((RDF, "about")))
        .find(|about| !about.is_empty())
        .unwrap_or("");
    let description = description(photo, subject)?;
    let range = rdf.range();
    let source = &original[range.clone()];
    let mut edits: Vec<_> = removed_properties(rdf, photo, path)
        .into_iter()
        .map(|r| (r, String::new()))
        .collect();
    if source.ends_with("/>") {
        let qname = source[1..]
            .split(|c: char| c.is_whitespace() || c == '/' || c == '>')
            .next()
            .context("invalid RDF name")?;
        edits.push((
            range.end - 2..range.end,
            format!(">{description}</{qname}>"),
        ));
    } else {
        let end = range.start + source.rfind("</").context("missing RDF closing tag")?;
        edits.push((end..end, description));
    }
    edits.sort_by_key(|(range, _)| range.start);
    let mut output = String::with_capacity(original.len() + 256);
    let mut cursor = 0;
    for (range, replacement) in edits {
        anyhow::ensure!(range.start >= cursor, "overlapping XMP properties");
        output.push_str(&original[cursor..range.start]);
        output.push_str(&replacement);
        cursor = range.end;
    }
    output.push_str(&original[cursor..]);
    Document::parse(&output).context("invalid merged XMP; original was not changed")?;
    Ok(output)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::{Action, AnalysisStatus};

    fn merge(photo: &Photo, original: Option<&str>) -> Result<String> {
        super::merge(photo, original, Path::new(&photo.path))
    }

    fn photo() -> Photo {
        Photo {
            id: 1,
            project_id: 1,
            path: "synthetic.jpg".into(),
            filename: "synthetic.jpg".into(),
            format: "jpeg".into(),
            mtime: 0,
            size_bytes: 0,
            width: 32,
            height: 32,
            taken_at: None,
            capture_variant_id: None,
            missing: false,
            quarantined: false,
            decision: Action::Reject,
            rating: 3,
            color_label: ColorLabel::Purple,
            analysis_status: AnalysisStatus::Missing,
            analysis: None,
        }
    }

    fn assert_marks(text: &str, photo: &Photo) {
        let doc = Document::parse(text).unwrap();
        let descriptions: Vec<_> = doc
            .descendants()
            .filter(|n| n.has_tag_name((RDF, "Description")))
            .collect();
        let ratings: Vec<_> = descriptions
            .iter()
            .filter_map(|n| n.attribute((XMP, "Rating")))
            .collect();
        assert_eq!(ratings, vec![photo.rating.to_string()]);
        assert!(descriptions
            .iter()
            .any(|n| n.attribute((XMP, "Label")) == Some(label(photo.color_label))));
        assert!(descriptions
            .iter()
            .any(|n| n.attribute((IRIS, "Decision")) == Some(photo.decision.as_str())));
    }

    #[test]
    fn stars_colors_and_decisions_are_independent() {
        let mut p = photo();
        for action in [Action::Keep, Action::Reject, Action::Pending, Action::Flag] {
            for rating in 0..=5 {
                for color in [
                    ColorLabel::None,
                    ColorLabel::Red,
                    ColorLabel::Yellow,
                    ColorLabel::Green,
                    ColorLabel::Blue,
                    ColorLabel::Purple,
                ] {
                    p.decision = action;
                    p.rating = rating;
                    p.color_label = color;
                    assert_marks(&merge(&p, None).unwrap(), &p);
                }
            }
        }
    }

    #[test]
    fn preserves_adjustments_copyright_unicode_comments_and_namespace_aliases() {
        let original = format!("<?xpacket begin='\u{feff}'?><x:xmpmeta xmlns:x='adobe:ns:meta/'><r:RDF xmlns:r='{RDF}' xmlns:m='{XMP}' xmlns:crs='http://ns.adobe.com/camera-raw-settings/1.0/' xmlns:dc='http://purl.org/dc/elements/1.1/'><r:Description r:about='' m:Rating='5' crs:Exposure2012='1.25'><!-- 保留注释 --><m:Label>Red</m:Label><dc:rights>摄影师 &amp; 作者</dc:rights><crs:ToneCurvePV2012><r:Seq><r:li>0, 0</r:li></r:Seq></crs:ToneCurvePV2012></r:Description></r:RDF></x:xmpmeta><?xpacket end='w'?>");
        let p = photo();
        let output = merge(&p, Some(&original)).unwrap();
        assert_marks(&output, &p);
        for value in [
            "crs:Exposure2012='1.25'",
            "<!-- 保留注释 -->",
            "<dc:rights>摄影师 &amp; 作者</dc:rights>",
            "<crs:ToneCurvePV2012><r:Seq><r:li>0, 0</r:li></r:Seq></crs:ToneCurvePV2012>",
            "<?xpacket end='w'?>",
        ] {
            assert!(output.contains(value), "{value}");
        }
        assert!(!output.contains("<m:Label>"));
        let twice = merge(&p, Some(&output)).unwrap();
        assert_marks(&twice, &p);
        assert_eq!(twice, output);
        assert_eq!(twice.matches("crs:Exposure2012").count(), 1);
    }

    #[test]
    fn handles_empty_rdf_and_element_properties_without_touching_other_subjects() {
        let p = photo();
        for original in [format!("<RDF xmlns='{RDF}'/>"), format!("<r:RDF xmlns:r='{RDF}' xmlns:m='{XMP}'><r:Description><m:Rating>1</m:Rating><m:Label>Blue</m:Label></r:Description></r:RDF>")] {
            assert_marks(&merge(&p, Some(&original)).unwrap(), &p);
        }
        let other = "<r:Description r:about='other' m:Rating='4'/>";
        let original = format!("<r:RDF xmlns:r='{RDF}' xmlns:m='{XMP}'>{other}</r:RDF>");
        assert!(merge(&p, Some(&original)).unwrap().contains(other));
    }

    #[test]
    fn malformed_missing_rdf_and_dtd_documents_are_rejected() {
        for original in [
            "not xml",
            "<x/>",
            "<!DOCTYPE x [<!ENTITY data 'text'>]><x/>",
            "<x><unclosed></x>",
        ] {
            assert!(merge(&photo(), Some(original)).is_err());
        }
    }

    #[test]
    fn file_subjects_replace_owned_marks_and_preserve_other_subjects() {
        let temp = tempfile::tempdir().unwrap();
        let p = photo();
        let path = temp.path().join(&p.filename);
        for about in [
            p.filename.clone(),
            "synthetic%2Ejpg".into(),
            url::Url::from_file_path(&path).unwrap().to_string(),
        ] {
            let other = "<r:Description r:about='other.jpg' m:Rating='4'/>";
            let original = format!("<r:RDF xmlns:r='{RDF}' xmlns:m='{XMP}' xmlns:c='urn:custom'><r:Description r:about='{about}' m:Rating='5'><m:Label>Red</m:Label><c:grading>untouched</c:grading></r:Description>{other}</r:RDF>");
            let output = super::merge(&p, Some(&original), &path).unwrap();
            let doc = Document::parse(&output).unwrap();
            let ratings: Vec<_> = doc
                .descendants()
                .filter(|n| describes_photo(*n, &p, &path))
                .filter_map(|n| n.attribute((XMP, "Rating")))
                .collect();
            assert_eq!(ratings, vec!["3"]);
            assert!(!output.contains("<m:Label>"));
            assert!(output.contains("<c:grading>untouched</c:grading>"));
            assert!(output.contains(other));
            assert_eq!(super::merge(&p, Some(&output), &path).unwrap(), output);
        }
        let mut unicode = p;
        unicode.filename = "人 像&片.jpg".into();
        unicode.path = unicode.filename.clone();
        let path = temp.path().join(&unicode.filename);
        let original = format!("<r:RDF xmlns:r='{RDF}' xmlns:m='{XMP}'><r:Description r:about='人 像&amp;片.jpg' m:Rating='5'/></r:RDF>");
        let output = super::merge(&unicode, Some(&original), &path).unwrap();
        assert_marks(&output, &unicode);
        assert!(output.contains("rdf:about='人 像&amp;片.jpg'"));
    }
}
