use crate::{
    parser::{FileIdentifier, Link},
    ResolutionExplanation,
};
use std::collections::{BTreeMap, HashMap};
use unicode_normalization::UnicodeNormalization;

#[derive(Clone)]
struct Candidate {
    path: String,
    directory: String,
    title: String,
    format: String,
}

pub(crate) struct Resolver {
    candidates: Vec<Candidate>,
    by_title: HashMap<String, Vec<usize>>,
    by_path: HashMap<String, usize>,
    by_folded_title: HashMap<String, Vec<usize>>,
    by_folded_path: HashMap<String, Vec<usize>>,
}

/// Link text and file names also match regardless of letter case and of
/// Unicode normalization (`é` precomposed, or `e` plus a combining accent),
/// as in Obsidian. An exact match is always preferred: folded names are
/// only consulted when nothing matches exactly.
fn fold(text: &str) -> String {
    text.nfc().collect::<String>().to_lowercase()
}

fn rank(format: &str) -> usize {
    match format {
        "md" => 0,
        "html" => 1,
        "excalidraw" => 2,
        "css" | "js" => 3,
        "png" | "jpg" | "jpeg" | "gif" | "svg" | "webp" => 4,
        "pdf" | "txt" => 5,
        _ => 6,
    }
}

impl Resolver {
    pub fn new<'a>(
        files: impl Iterator<Item = &'a FileIdentifier>,
        aliases: &BTreeMap<String, String>,
    ) -> Self {
        let mut candidates: Vec<_> = files
            .map(|f| Candidate {
                path: f.path.clone(),
                directory: f.directory.clone(),
                title: f.title.clone(),
                format: f.file_type.clone(),
            })
            .collect();
        let mut alias_paths = Vec::new();
        for (alias, target) in aliases {
            for candidate in &candidates {
                if candidate.path == *target || candidate.path.starts_with(&format!("{target}/")) {
                    let alias_path = format!("{alias}{}", &candidate.path[target.len()..]);
                    let directory = alias_path
                        .rsplit_once('/')
                        .map_or("", |(dir, _)| dir)
                        .to_string();
                    let name = alias_path.rsplit('/').next().unwrap();
                    let title = if candidate.format == "excalidraw" {
                        name.trim_end_matches(".md").trim_end_matches(".excalidraw")
                    } else {
                        name.rsplit_once('.').map_or(name, |(name, _)| name)
                    };
                    let title = title.to_string();
                    alias_paths.push((
                        alias_path,
                        Candidate {
                            directory,
                            title,
                            ..candidate.clone()
                        },
                    ));
                }
            }
        }
        let mut by_path: HashMap<_, _> = candidates
            .iter()
            .enumerate()
            .map(|(id, c)| (c.path.clone(), id))
            .collect();
        for (path, candidate) in alias_paths {
            by_path.insert(path, candidates.len());
            candidates.push(candidate);
        }
        let mut by_title: HashMap<String, Vec<usize>> = HashMap::new();
        for (id, candidate) in candidates.iter().enumerate() {
            by_title
                .entry(candidate.title.clone())
                .or_default()
                .push(id);
        }
        let mut by_folded_title: HashMap<String, Vec<usize>> = HashMap::new();
        let mut by_folded_path: HashMap<String, Vec<usize>> = HashMap::new();
        for (path, id) in &by_path {
            by_folded_path.entry(fold(path)).or_default().push(*id);
        }
        for ids in by_folded_path.values_mut() {
            ids.sort_by(|a, b| candidates[*a].path.cmp(&candidates[*b].path));
        }
        for (id, candidate) in candidates.iter().enumerate() {
            by_folded_title
                .entry(fold(&candidate.title))
                .or_default()
                .push(id);
        }
        Self {
            candidates,
            by_path,
            by_title,
            by_folded_title,
            by_folded_path,
        }
    }

    /// The candidate at exactly `path`, or else the first one whose path
    /// differs only in letter case or Unicode normalization.
    fn by_path_or_folded(&self, path: &str) -> Option<usize> {
        self.by_path.get(path).copied().or_else(|| {
            self.by_folded_path
                .get(&fold(path))
                .and_then(|ids| ids.first().copied())
        })
    }

    fn matches(
        &self,
        link: &Link,
        source_directory: &str,
        explain: bool,
    ) -> (Vec<usize>, &'static str) {
        let directory = link
            .link_parsed_directory
            .trim_end_matches('/')
            .trim_start_matches('/');
        if directory.split('/').any(|part| part == "..") {
            return (Vec::new(), "outsideSourceRoot");
        }
        let requested = if directory.is_empty() {
            format!("{}.{}", link.link_parsed_title, link.link_parsed_file_type)
        } else {
            format!(
                "{directory}/{}.{}",
                link.link_parsed_title, link.link_parsed_file_type
            )
        };
        if link.is_relative_path_link {
            return (
                self.by_path_or_folded(&requested).into_iter().collect(),
                "relativePath",
            );
        }
        // An explicit physical filename remains valid even when its detected
        // format differs from its suffix (notably Excalidraw Markdown).
        if crate::parser::wiki_link_has_explicit_file_type(link) {
            if let Some(id) = self.by_path_or_folded(&requested) {
                if self.candidates[id].format != link.link_parsed_file_type {
                    return (vec![id], "exactPath");
                }
            }
        }
        let mut folded = false;
        let mut candidates = self.title_candidates(link, directory, folded);
        if candidates.is_empty() {
            folded = true;
            candidates = self.title_candidates(link, directory, folded);
        }
        let same_directory = |a: &str, b: &str| if folded { fold(a) == fold(b) } else { a == b };
        let key = |id: &usize| {
            let c = &self.candidates[*id];
            let priority = if !directory.is_empty() {
                if same_directory(&c.directory, directory) {
                    0
                } else {
                    1
                }
            } else if c.directory.is_empty() {
                0
            } else if same_directory(&c.directory, source_directory) {
                1
            } else {
                2
            };
            (
                priority,
                c.directory.split('/').filter(|s| !s.is_empty()).count(),
                &c.directory,
                rank(&c.format),
                &c.format,
                &c.path,
            )
        };
        if explain {
            candidates.sort_by_key(key);
        } else if let Some(best) = candidates.iter().min_by_key(|id| key(id)).copied() {
            candidates.clear();
            candidates.push(best);
        }
        let reason = candidates.first().map_or("unresolved", |id| {
            let c = &self.candidates[*id];
            if !directory.is_empty() {
                if same_directory(&c.directory, directory) {
                    "exactPath"
                } else {
                    "directorySuffix"
                }
            } else if c.directory.is_empty() {
                "sourceRoot"
            } else if same_directory(&c.directory, source_directory) {
                "sourceDirectory"
            } else {
                "shallowestThenLexical"
            }
        });
        (candidates, reason)
    }

    /// Candidates with the link's title, in the link's directory if it names
    /// one, preferring the link's file type. With `folded`, titles and
    /// directories match regardless of letter case and Unicode normalization.
    fn title_candidates(&self, link: &Link, directory: &str, folded: bool) -> Vec<usize> {
        let ids = if folded {
            self.by_folded_title.get(&fold(&link.link_parsed_title))
        } else {
            self.by_title.get(&link.link_parsed_title)
        }
        .cloned()
        .unwrap_or_default();
        let comparable = |text: &str| if folded { fold(text) } else { text.to_string() };
        let wanted = comparable(directory);
        let path_matches = |id: &usize| {
            let candidate_directory = comparable(&self.candidates[*id].directory);
            directory.is_empty()
                || candidate_directory == wanted
                || candidate_directory.ends_with(&format!("/{wanted}"))
        };
        let candidates: Vec<usize> = ids
            .iter()
            .copied()
            .filter(path_matches)
            .filter(|id| self.candidates[*id].format == link.link_parsed_file_type)
            .collect();
        if candidates.is_empty() && !crate::parser::wiki_link_has_explicit_file_type(link) {
            return ids.into_iter().filter(path_matches).collect();
        }
        candidates
    }

    pub fn resolve(&self, link: &mut Link, source_directory: &str) {
        let (matches, _) = self.matches(link, source_directory, false);
        if let Some(id) = matches.first() {
            let c = &self.candidates[*id];
            link.target = Some(c.path.clone());
            link.link_resolved_target_path = c.path.clone();
            link.link_resolved_target_directory =
                c.path.rsplit_once('/').map_or("", |(dir, _)| dir).into();
            link.link_parsed_file_type = c.format.clone();
        } else {
            let dir = link.link_parsed_directory.trim_matches('/');
            link.link_resolved_target_path = if dir.is_empty() {
                format!("{}.{}", link.link_parsed_title, link.link_parsed_file_type)
            } else {
                format!(
                    "{dir}/{}.{}",
                    link.link_parsed_title, link.link_parsed_file_type
                )
            };
            link.link_resolved_target_directory = dir.into();
            link.target = None;
        }
    }

    pub fn explain(&self, link: &Link, source_directory: &str) -> ResolutionExplanation {
        let mut original = link.clone();
        original.link_parsed_file_type = if link.is_relative_path_link {
            crate::links::parse_markdown_link_href(&link.link_original_text).file_type
        } else {
            crate::links::parse_link_text(&link.link_original_text).file_type
        };
        let (matches, reason) = self.matches(&original, source_directory, true);
        let mut paths: Vec<_> = matches
            .into_iter()
            .map(|id| self.candidates[id].path.clone())
            .collect();
        paths.dedup();
        ResolutionExplanation {
            reason: reason.into(),
            candidates: paths,
        }
    }
}
