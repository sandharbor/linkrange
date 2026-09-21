use crate::{index, resolver::Resolver, Diagnostic, Link, ResolutionExplanation, Source};
use anyhow::{Context, Result};
use percent_encoding::{utf8_percent_encode, AsciiSet, CONTROLS};
use std::collections::{BTreeMap, BTreeSet};

const PATH_ESCAPES: &AsciiSet = &CONTROLS
    .add(b' ')
    .add(b'"')
    .add(b'#')
    .add(b'%')
    .add(b'?')
    .add(b'[')
    .add(b']')
    .add(b'\\')
    .add(b'`')
    .add(b'{')
    .add(b'}')
    .add(b'<')
    .add(b'>');

/// Canonical names and aliases use the same case-sensitive, URL-safe grammar.
pub fn validate_source_name(name: &str) -> Result<()> {
    anyhow::ensure!(
        !name.is_empty()
            && name.len() <= 64
            && name.as_bytes()[0].is_ascii_lowercase()
            && name
                .bytes()
                .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == b'-' || c == b'_'),
        "Invalid source name {name:?}: expected [a-z][a-z0-9_-]{{0,63}}"
    );
    Ok(())
}

pub(crate) fn normalize_relative(path: &str) -> Result<String> {
    anyhow::ensure!(
        !path.starts_with('/') && !path.contains('\\') && !path.contains('\0'),
        "Expected source-root-relative path: {path}"
    );
    anyhow::ensure!(
        !path.split('/').any(|s| s == ".."),
        "Path must remain inside source root: {path}"
    );
    Ok(path
        .split('/')
        .filter(|s| !s.is_empty() && *s != ".")
        .collect::<Vec<_>>()
        .join("/"))
}

/// Encode a source-relative identity. A source root is `source://name/`.
pub fn source_locator(name: &str, path: &str) -> String {
    format!(
        "source://{name}/{}",
        utf8_percent_encode(path, PATH_ESCAPES)
    )
}

fn decode_path(path: &str) -> Result<String> {
    for (i, byte) in path.bytes().enumerate() {
        if byte == b'%' {
            anyhow::ensure!(
                path.as_bytes()
                    .get(i + 1)
                    .is_some_and(u8::is_ascii_hexdigit)
                    && path
                        .as_bytes()
                        .get(i + 2)
                        .is_some_and(u8::is_ascii_hexdigit),
                "Invalid URL escape in source path"
            );
        }
    }
    let decoded = percent_encoding::percent_decode_str(path)
        .decode_utf8()
        .context("Source path must be UTF-8")?;
    normalize_relative(&decoded)
}

/// Parse a request/result locator. Link fragments and queries are not file identities.
pub fn parse_source_locator(locator: &str) -> Result<(String, String)> {
    let rest = locator
        .strip_prefix("source://")
        .context("Expected source://<name>/<path>")?;
    let (name, path) = rest
        .split_once('/')
        .context("Source locator requires a slash after its name")?;
    validate_source_name(name)?;
    anyhow::ensure!(
        !path.contains(['#', '?']),
        "Source locators cannot contain a query or fragment; percent-encode filename characters"
    );
    Ok((name.into(), decode_path(path)?))
}

pub(crate) fn validate_sources(sources: &[Source]) -> Result<Vec<Source>> {
    anyhow::ensure!(!sources.is_empty(), "At least one source is required");
    let mut names = BTreeSet::new();
    let mut result: Vec<Source> = Vec::new();
    for source in sources {
        for name in std::iter::once(&source.name).chain(&source.aliases) {
            validate_source_name(name)?;
            anyhow::ensure!(
                names.insert(name),
                "Source name or alias is already registered: {name}"
            );
        }
        let directory = source.directory.canonicalize().with_context(|| {
            format!(
                "Source {:?} is unavailable: {}",
                source.name,
                source.directory.display()
            )
        })?;
        anyhow::ensure!(
            directory.is_dir(),
            "Source {:?} must be a directory",
            source.name
        );
        for existing in &result {
            anyhow::ensure!(
                !directory.starts_with(&existing.directory)
                    && !existing.directory.starts_with(&directory),
                "Sources {:?} and {:?} have duplicate or overlapping physical roots",
                existing.name,
                source.name
            );
        }
        result.push(Source {
            directory,
            ..source.clone()
        });
    }
    Ok(result)
}

/// Strip qualification before the existing wiki parser interprets anchors and aliases.
pub(crate) fn wiki_target(text: &str) -> (String, Option<String>) {
    let normalized = text.replace("\\|", "|");
    let target_end = normalized.find(['#', '^', '|']).unwrap_or(normalized.len());
    let target = &normalized[..target_end];
    match target.split_once("::") {
        Some((page, source)) => (
            format!("{page}{}", &normalized[target_end..]),
            Some(source.into()),
        ),
        None => (text.into(), None),
    }
}

/// Keep malformed source URLs as unresolved occurrences, never local fallbacks.
pub(crate) fn url_target(href: &str) -> (String, Option<String>, Option<String>) {
    let Some(rest) = href.strip_prefix("source://") else {
        return (href.into(), None, None);
    };
    let Some((name, path)) = rest.split_once('/') else {
        return (
            String::new(),
            Some(rest.into()),
            Some("invalidSourceReference".into()),
        );
    };
    let file_path = path.split(['#', '?']).next().unwrap_or("");
    let invalid = validate_source_name(name)
        .and_then(|_| decode_path(file_path))
        .is_err()
        || file_path.is_empty();
    (
        format!("/{path}"),
        Some(name.into()),
        invalid.then(|| "invalidSourceReference".into()),
    )
}

pub(crate) struct RegistryResolver {
    resolvers: BTreeMap<String, Resolver>,
    names: BTreeMap<String, String>,
    pub qualified: bool,
}

impl RegistryResolver {
    pub fn new(indexes: &[(Source, index::Index)], qualified: bool) -> Self {
        let resolvers = indexes
            .iter()
            .map(|(source, index)| {
                (
                    source.name.clone(),
                    Resolver::new(
                        index.files.iter().map(|entry| &entry.scan.source_file),
                        &index.aliases,
                    ),
                )
            })
            .collect();
        let names = indexes
            .iter()
            .flat_map(|(source, _)| {
                std::iter::once(&source.name)
                    .chain(&source.aliases)
                    .map(|name| (name.clone(), source.name.clone()))
            })
            .collect();
        Self {
            resolvers,
            names,
            qualified,
        }
    }

    pub fn canonical_name(&self, name: &str) -> Result<&str> {
        self.names
            .get(name)
            .map(String::as_str)
            .with_context(|| format!("Source is not registered: {name}"))
    }

    pub fn resolve(&self, link: &mut Link, source: &str, directory: &str) -> Option<Diagnostic> {
        let requested = link
            .link_requested_target_source
            .as_deref()
            .unwrap_or(source);
        if self.qualified || link.link_requested_target_source.is_some() {
            link.link_source_name = Some(source.into());
        }
        if self.qualified {
            link.link_source_page_path = source_locator(source, &link.link_source_page_path);
        }
        if validate_source_name(requested).is_err() {
            link.link_source_error = Some("invalidSourceReference".into());
        }
        let selected = self.names.get(requested);
        if selected.is_none() && link.link_source_error.is_none() {
            link.link_source_error = Some("unregisteredSource".into());
        }
        if let Some(error) = &link.link_source_error {
            let mut diagnostic = Diagnostic::new(
                &link.link_source_page_path,
                error,
                format!("Source reference {requested:?} cannot be resolved"),
            );
            diagnostic.requested_source = Some(requested.into());
            diagnostic.link_original_text = Some(link.link_original_text.clone());
            return Some(diagnostic);
        }
        let selected = selected.unwrap();
        if self.qualified || link.link_requested_target_source.is_some() {
            link.link_resolved_target_source = Some(selected.clone());
        }
        let context = if selected == source { directory } else { "" };
        self.resolvers[selected].resolve(link, context);
        if self.qualified {
            link.target = link
                .target
                .as_ref()
                .map(|path| source_locator(selected, path));
            link.link_resolved_target_path =
                source_locator(selected, &link.link_resolved_target_path);
            link.link_resolved_target_directory =
                source_locator(selected, &link.link_resolved_target_directory);
        }
        None
    }

    pub fn explain(&self, link: &Link, directory: &str) -> ResolutionExplanation {
        if let Some(error) = &link.link_source_error {
            return ResolutionExplanation {
                reason: error.clone(),
                candidates: Vec::new(),
            };
        }
        let source = link.link_source_name.as_deref().unwrap_or("source");
        let target = link
            .link_resolved_target_source
            .as_deref()
            .unwrap_or(source);
        let mut explanation =
            self.resolvers[target].explain(link, if source == target { directory } else { "" });
        if self.qualified {
            explanation.candidates = explanation
                .candidates
                .iter()
                .map(|p| source_locator(target, p))
                .collect();
        }
        explanation
    }
}
