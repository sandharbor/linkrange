use crate::{Diagnostic, FrontmatterField};
use std::collections::BTreeMap;

/// The YAML between a note's opening `---` and its closing `---` or `...`,
/// and whether that closing line was found.
fn frontmatter(content: &str) -> Option<(&str, bool)> {
    let content = content.strip_prefix('\u{feff}').unwrap_or(content);
    let mut lines = content.split_inclusive('\n');
    let open = lines.next()?;
    if open.trim_end_matches(['\r', '\n']) != "---" {
        return None;
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
    Some((&content[open.len()..open.len() + length], closed))
}

/// A note's frontmatter aliases, read as Obsidian reads them: the `aliases`
/// key in any letter case, holding one string or a list of strings, each
/// trimmed, empty ones left out. Malformed frontmatter has none.
pub(crate) fn note_aliases(content: &str) -> Vec<String> {
    let Some((frontmatter, true)) = frontmatter(content) else {
        return Vec::new();
    };
    if !frontmatter.to_lowercase().contains("aliases") {
        return Vec::new();
    }
    let Ok(serde_json::Value::Object(map)) = serde_yaml::from_str(frontmatter) else {
        return Vec::new();
    };
    let value = map
        .iter()
        .find(|(key, _)| key.eq_ignore_ascii_case("aliases"));
    let names: Vec<&str> = match value.map(|(_, value)| value) {
        Some(serde_json::Value::String(name)) => vec![name],
        Some(serde_json::Value::Array(names)) => {
            names.iter().filter_map(|name| name.as_str()).collect()
        }
        _ => Vec::new(),
    };
    names
        .into_iter()
        .map(str::trim)
        .filter(|name| !name.is_empty())
        .map(String::from)
        .collect()
}

pub(crate) fn collect(
    path: &str,
    content: &str,
    fields: &[FrontmatterField],
) -> (BTreeMap<String, serde_json::Value>, Vec<Diagnostic>, usize) {
    let mut values = BTreeMap::new();
    if fields.is_empty() {
        return (values, Vec::new(), 0);
    }
    let Some((frontmatter, closed)) = frontmatter(content) else {
        return (values, Vec::new(), 0);
    };
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
