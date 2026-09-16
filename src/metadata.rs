use crate::{Diagnostic, FrontmatterField};
use std::collections::BTreeMap;

pub(crate) fn collect(
    path: &str,
    content: &str,
    fields: &[FrontmatterField],
) -> (BTreeMap<String, serde_json::Value>, Vec<Diagnostic>, usize) {
    let mut values = BTreeMap::new();
    if fields.is_empty() {
        return (values, Vec::new(), 0);
    }
    let content = content.strip_prefix('\u{feff}').unwrap_or(content);
    let mut lines = content.split_inclusive('\n');
    let Some(open) = lines.next() else {
        return (values, Vec::new(), 0);
    };
    if open.trim_end_matches(['\r', '\n']) != "---" {
        return (values, Vec::new(), 0);
    }
    let mut length = 0;
    let mut closed = false;
    for line in lines {
        if matches!(line.trim_end_matches(['\r', '\n']), "---" | "...") {
            closed = true;
            break;
        }
        length += line.len();
    }
    let frontmatter = &content[open.len()..open.len() + length];
    if !fields
        .iter()
        .any(|field| frontmatter.contains(field.substring.as_deref().unwrap_or(&field.key)))
    {
        return (values, Vec::new(), 0);
    }
    if !closed {
        return (
            values,
            vec![Diagnostic::new(
                path,
                "malformedFrontmatter",
                "Missing closing frontmatter delimiter",
            )],
            0,
        );
    }
    let parsed = serde_yaml::from_str::<serde_json::Value>(frontmatter);
    match parsed {
        Ok(serde_json::Value::Object(map)) => {
            for field in fields {
                if let Some(value) = map.get(&field.key) {
                    values.insert(field.key.clone(), value.clone());
                }
            }
            (values, Vec::new(), 1)
        }
        Ok(_) => (
            values,
            vec![Diagnostic::new(
                path,
                "malformedFrontmatter",
                "Frontmatter must be a mapping",
            )],
            1,
        ),
        Err(error) => (
            values,
            vec![Diagnostic::new(path, "malformedFrontmatter", error)],
            1,
        ),
    }
}
