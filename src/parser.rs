use crate::links::{parse_link_text, parse_markdown_link_href, AnchorType as LibAnchorType};
use serde::{Deserialize, Serialize};
use std::path::Path;

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub(crate) struct FileIdentifier {
    pub directory: String,
    pub title: String,
    pub file_type: String,
    pub path: String, // no leading "/" (matches fs_search `path`)
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Link {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub link_source_name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub link_requested_target_source: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub link_resolved_target_source: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub link_source_error: Option<String>,
    pub link_original_text: String,
    pub link_source_page_path: String,

    pub link_parsed_directory: String,
    pub link_parsed_title: String,
    pub link_parsed_file_type: String,
    pub link_parsed_anchor: Option<String>,
    pub link_parsed_anchor_type: Option<LibAnchorType>,
    pub link_parsed_alias: Option<String>,
    pub link_parsed_media_size: Option<u32>,

    pub link_resolved_target_directory: String,
    pub link_resolved_target_path: String,

    /// True for standard markdown links `[text](href)` where paths are relative to the source file.
    /// False for wiki-links `[[inner]]` where paths are resolved by fuzzy search.
    pub is_relative_path_link: bool,
    pub is_embedded: bool,
    #[serde(default)]
    pub target: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub resolution: Option<crate::ResolutionExplanation>,
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) enum ExtractedLink {
    Wiki(String),
    Embedded(Box<ExtractedLink>),
    Markdown { text: String, href: String },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct ScanResult {
    pub source_file: FileIdentifier,
    pub outgoing_links: Vec<Link>,
}

const IMAGE_EXTENSIONS_MAIN: &[&str] = &["jpg", "jpeg", "png", "gif", "svg", "webp", "excalidraw"];
const TEXT_SOURCE_EXTENSIONS: &[&str] = &["md", "html", "css", "js"];

pub(crate) fn is_supported_source_extension(extension: &str) -> bool {
    TEXT_SOURCE_EXTENSIONS.contains(&extension)
        || extension == "pdf"
        || IMAGE_EXTENSIONS_MAIN.contains(&extension)
}

/// Detects whether a markdown file is an Obsidian Excalidraw drawing by content.
/// Obsidian's Excalidraw plugin writes `excalidraw-plugin: parsed` into the YAML
/// frontmatter. We only inspect the leading frontmatter region to keep this cheap.
fn is_excalidraw_markdown(content: &str) -> bool {
    if !content.starts_with("---") {
        return false;
    }
    let after_open = &content[3..];
    let close_rel = match after_open.find("\n---") {
        Some(idx) => idx,
        None => return false,
    };
    let frontmatter = &after_open[..close_rel];
    frontmatter.contains("excalidraw-plugin: parsed")
}

fn extract_page_identifier(path: &std::path::Path, base_dir: &std::path::Path) -> FileIdentifier {
    let mut directory = path
        .parent()
        .unwrap_or_else(|| std::path::Path::new(""))
        .strip_prefix(base_dir)
        .unwrap_or_else(|_| path.parent().unwrap_or_else(|| std::path::Path::new("")))
        .to_string_lossy()
        .into_owned();
    if directory.starts_with("./") {
        directory = directory[2..].to_string();
    }

    let file_name_full = path.file_name().unwrap_or_default().to_string_lossy();
    let (name_part, file_type_str) = if let Some(dot_pos) = file_name_full.rfind('.') {
        let stem_candidate = &file_name_full[..dot_pos];
        let mut ext = file_name_full[dot_pos + 1..].to_lowercase();

        const KNOWN_PARSEABLE_EXTENSIONS: &[&str] = &["md", "html", "css", "js", "txt"];
        const KNOWN_OTHER_DOCUMENT_EXTENSIONS: &[&str] = &["pdf"];

        if !(KNOWN_PARSEABLE_EXTENSIONS.contains(&ext.as_str())
            || KNOWN_OTHER_DOCUMENT_EXTENSIONS.contains(&ext.as_str())
            || IMAGE_EXTENSIONS_MAIN.contains(&ext.as_str()))
        {
            ext = "other".to_string();
        }
        (stem_candidate.to_string(), ext)
    } else {
        (file_name_full.to_string(), "md".to_string())
    };

    let title = if let Some(hash_pos) = name_part.rfind("#^") {
        name_part[..hash_pos].to_string()
    } else {
        name_part
    };

    let path = path
        .strip_prefix(base_dir)
        .unwrap_or(path)
        .to_string_lossy()
        .replace('\\', "/");

    FileIdentifier {
        directory,
        title,
        file_type: file_type_str,
        path,
    }
}

fn calculate_normalized_directory(
    source_dir_path_str: &str,
    link_path_prefix_opt: Option<&String>,
) -> String {
    let initial_path_to_normalize: std::path::PathBuf = match link_path_prefix_opt {
        None => std::path::PathBuf::from(source_dir_path_str),
        Some(prefix_str) => {
            std::path::PathBuf::from(prefix_str.strip_prefix('/').unwrap_or(prefix_str))
        }
    };

    let mut normalized_components = Vec::new();
    for component in initial_path_to_normalize.components() {
        match component {
            std::path::Component::ParentDir => {
                if normalized_components
                    .last()
                    .is_some_and(|c| matches!(c, std::path::Component::Normal(_)))
                {
                    normalized_components.pop();
                } else {
                    normalized_components.push(component);
                }
            }
            std::path::Component::CurDir => {}
            _ => normalized_components.push(component),
        }
    }

    let final_path: std::path::PathBuf = normalized_components.into_iter().collect();
    let mut path_str = final_path.to_string_lossy().into_owned();
    if path_str == "." {
        path_str = String::new();
    }
    path_str
}

fn parse_extracted_link(link: ExtractedLink, source: &FileIdentifier) -> Link {
    match link {
        ExtractedLink::Wiki(inner) => parse_out_link(&inner, source),
        ExtractedLink::Markdown { text, href } => parse_out_markdown_link(&text, &href, source),
        ExtractedLink::Embedded(inner) => {
            let mut link = parse_extracted_link(*inner, source);
            link.is_embedded = true;
            link
        }
    }
}

fn extract_links(content: &str) -> Vec<ExtractedLink> {
    use pulldown_cmark::{Event, LinkType, Options, Parser, Tag};
    let mut out = Vec::new();
    let mut markup = String::new();
    let mut blocked = Vec::new();
    let mut options = Options::empty();
    options.insert(Options::ENABLE_WIKILINKS);
    for (event, range) in Parser::new_ext(content, options).into_offset_iter() {
        match event {
            Event::Start(Tag::Link {
                link_type,
                dest_url,
                ..
            })
            | Event::Start(Tag::Image {
                link_type,
                dest_url,
                ..
            }) => {
                blocked.push(range.clone());
                let raw = &content[range];
                let embedded = raw.starts_with('!');
                let link = if matches!(link_type, LinkType::WikiLink { .. }) {
                    let raw = raw.trim_start_matches('!');
                    let inner = raw
                        .strip_prefix("[[")
                        .and_then(|s| s.strip_suffix("]]"))
                        .unwrap_or(&dest_url);
                    if !is_internal_html_target(inner)
                        && crate::sources::wiki_target(inner).1.is_none()
                    {
                        continue;
                    }
                    ExtractedLink::Wiki(inner.into())
                } else {
                    if !is_internal_html_target(&dest_url) {
                        continue;
                    }
                    let text = raw
                        .trim_start_matches('!')
                        .strip_prefix('[')
                        .and_then(|s| s.split_once(']'))
                        .map_or(dest_url.as_ref(), |(text, _)| text);
                    ExtractedLink::Markdown {
                        text: text.into(),
                        href: dest_url.into_string(),
                    }
                };
                out.push(if embedded {
                    ExtractedLink::Embedded(Box::new(link))
                } else {
                    link
                });
            }
            Event::Html(html) | Event::InlineHtml(html) => {
                blocked.push(range);
                markup.push_str(&html);
                markup.push('\n');
            }
            Event::Code(_) | Event::Start(Tag::CodeBlock(_)) => {
                blocked.push(range);
            }
            _ => {}
        }
    }
    // Obsidian accepts unescaped spaces in inline destinations. Preserve that
    // established extension while using CommonMark ranges to exclude examples,
    // comments, and links already recognized by the standard parser.
    blocked.sort_by_key(|range| range.start);
    let mut cursor = 0;
    let bytes = content.as_bytes();
    let mut excluded = 0;
    while cursor < bytes.len() {
        while excluded < blocked.len() && blocked[excluded].end <= cursor {
            excluded += 1;
        }
        if excluded < blocked.len() && blocked[excluded].start <= cursor {
            cursor = blocked[excluded].end;
            continue;
        }
        if bytes[cursor] == b'[' && (cursor == 0 || !matches!(bytes[cursor - 1], b'[' | b'\\')) {
            if let Some(extracted) = try_extract_markdown_link(content, cursor) {
                if is_internal_html_target(match &extracted.link {
                    ExtractedLink::Markdown { href, .. } => href,
                    _ => "",
                }) {
                    let embedded = cursor > 0 && bytes[cursor - 1] == b'!';
                    out.push(if embedded {
                        ExtractedLink::Embedded(Box::new(extracted.link))
                    } else {
                        extracted.link
                    });
                }
                cursor = extracted.end_pos;
                continue;
            }
        }
        cursor += 1;
    }
    if !markup.is_empty() {
        out.extend(extract_markup_links(&markup));
    }
    out
}

pub(crate) fn is_internal_html_target(target: &str) -> bool {
    let target = target.trim();
    if target.starts_with("source://") {
        return true;
    }
    if target.is_empty() || target.starts_with('#') || target.starts_with("//") {
        return false;
    }
    let first = target.split(['/', '#', '?']).next().unwrap_or("");
    !first.contains(':')
}

fn extract_markup_links(content: &str) -> Vec<ExtractedLink> {
    crate::markup::extract(content)
}

struct ExtractedMarkdownLink {
    link: ExtractedLink,
    end_pos: usize,
}

/// Tries to extract a markdown link starting at `start` which points to the opening `[`.
/// Returns the extracted link and the position after the closing `)`, or None if not a valid link.
fn try_extract_markdown_link(content: &str, start: usize) -> Option<ExtractedMarkdownLink> {
    let bytes = content.as_bytes();
    let len = bytes.len();

    // Find the matching `]` for `[text]`
    let mut depth = 0;
    let mut j = start;
    while j < len {
        if bytes[j] == b'[' {
            depth += 1;
        } else if bytes[j] == b']' {
            depth -= 1;
            if depth == 0 {
                break;
            }
        } else if bytes[j] == b'\n' {
            // Don't span across newlines for the text part
            return None;
        }
        j += 1;
    }
    if depth != 0 || j >= len {
        return None;
    }

    let text = &content[start + 1..j];
    let after_bracket = j + 1;

    // Must be immediately followed by `(`
    if after_bracket >= len || bytes[after_bracket] != b'(' {
        return None;
    }

    // Find the matching `)` for `(href)`
    let href_start = after_bracket + 1;
    let mut paren_depth = 1;
    let mut k = href_start;
    while k < len && paren_depth > 0 {
        if bytes[k] == b'(' {
            paren_depth += 1;
        } else if bytes[k] == b')' {
            paren_depth -= 1;
        } else if bytes[k] == b'\n' {
            return None;
        }
        k += 1;
    }
    if paren_depth != 0 {
        return None;
    }

    let href = content[href_start..k - 1].trim();

    // Skip external links
    if href.starts_with("http://") || href.starts_with("https://") {
        return None;
    }
    // Skip anchor-only links
    if href.starts_with('#') {
        return None;
    }
    // Skip empty hrefs
    if href.is_empty() {
        return None;
    }

    Some(ExtractedMarkdownLink {
        link: ExtractedLink::Markdown {
            text: text.to_string(),
            href: href.to_string(),
        },
        end_pos: k,
    })
}

fn make_source_page_path(source_page: &FileIdentifier) -> String {
    source_page.path.clone()
}

fn parse_out_link(inner_link_text: &str, source_page: &FileIdentifier) -> Link {
    let semantics = parse_link_text(inner_link_text);

    Link {
        link_source_name: None,
        link_requested_target_source: semantics.requested_source,
        link_resolved_target_source: None,
        link_source_error: semantics.source_error,
        link_original_text: inner_link_text.to_string(),
        link_source_page_path: make_source_page_path(source_page),
        link_parsed_directory: semantics.target_path_prefix,
        link_parsed_title: semantics.title,
        link_parsed_file_type: semantics.file_type,
        link_parsed_anchor: semantics.anchor,
        link_parsed_anchor_type: semantics.anchor_type,
        link_parsed_alias: semantics.alias,
        link_parsed_media_size: semantics.media_size,
        link_resolved_target_directory: String::new(),
        link_resolved_target_path: String::new(),
        is_relative_path_link: false,
        is_embedded: false,
        target: None,
        resolution: None,
    }
}

fn parse_out_markdown_link(display_text: &str, href: &str, source_page: &FileIdentifier) -> Link {
    let mut semantics = parse_markdown_link_href(href);

    // Resolve the relative path against the source file's directory.
    // Markdown links are relative to the source file, so we prepend the source directory
    // and let calculate_normalized_directory handle `..` and `.` segments.
    let resolved_prefix = if semantics.target_path_prefix.starts_with('/') {
        semantics
            .target_path_prefix
            .trim_start_matches('/')
            .to_string()
    } else if semantics.target_path_prefix.is_empty() {
        // Same-directory reference: use source page's directory
        if source_page.directory.is_empty() {
            String::new()
        } else {
            format!("{}/", source_page.directory)
        }
    } else {
        // Combine source dir with relative path
        if source_page.directory.is_empty() {
            semantics.target_path_prefix.clone()
        } else {
            format!("{}/{}", source_page.directory, semantics.target_path_prefix)
        }
    };

    // Normalize the combined path (resolves `..` and `.` segments)
    let normalized_dir = if resolved_prefix.is_empty() {
        String::new()
    } else {
        let trailing_slash = resolved_prefix.ends_with('/');
        let normalized = calculate_normalized_directory("", Some(&resolved_prefix));
        if trailing_slash && !normalized.is_empty() && !normalized.ends_with('/') {
            format!("{}/", normalized)
        } else if trailing_slash && normalized.is_empty() {
            String::new()
        } else {
            normalized
        }
    };

    // Override alias with the display text from [text](href)
    semantics.alias = Some(display_text.to_string());

    Link {
        link_source_name: None,
        link_requested_target_source: semantics.requested_source,
        link_resolved_target_source: None,
        link_source_error: semantics.source_error,
        link_original_text: href.to_string(),
        link_source_page_path: make_source_page_path(source_page),
        link_parsed_directory: normalized_dir,
        link_parsed_title: semantics.title,
        link_parsed_file_type: semantics.file_type,
        link_parsed_anchor: semantics.anchor,
        link_parsed_anchor_type: semantics.anchor_type,
        link_parsed_alias: semantics.alias,
        link_parsed_media_size: semantics.media_size,
        link_resolved_target_directory: String::new(),
        link_resolved_target_path: String::new(),
        is_relative_path_link: true,
        is_embedded: false,
        target: None,
        resolution: None,
    }
}

fn target_text_without_alias_or_size(link_text: &str) -> String {
    let mut last_unescaped_pipe = None;
    let mut escaped = false;
    for (idx, ch) in link_text.char_indices() {
        if escaped {
            escaped = false;
            continue;
        }
        if ch == '\\' {
            escaped = true;
            continue;
        }
        if ch == '|' {
            last_unescaped_pipe = Some(idx);
        }
    }
    let target = last_unescaped_pipe
        .map(|idx| &link_text[..idx])
        .unwrap_or(link_text);
    target.replace("\\|", "|")
}

fn strip_anchor_markers_for_extension_check(name: &str) -> &str {
    if let Some(pos) = name.rfind("#^") {
        &name[..pos]
    } else if let Some(pos) = name.rfind('^') {
        &name[..pos]
    } else if let Some(pos) = name.rfind('#') {
        &name[..pos]
    } else {
        name
    }
}

pub(crate) fn wiki_link_has_explicit_file_type(link: &Link) -> bool {
    if link.is_relative_path_link {
        return true;
    }
    if link.link_parsed_file_type != "md" {
        return true;
    }

    let (unqualified, _) = crate::sources::wiki_target(&link.link_original_text);
    let target_text = target_text_without_alias_or_size(&unqualified);
    let filename = target_text
        .rsplit('/')
        .next()
        .unwrap_or(target_text.as_str());
    let filename_without_anchor = strip_anchor_markers_for_extension_check(filename);
    Path::new(filename_without_anchor)
        .extension()
        .and_then(|ext| ext.to_str())
        .map(|ext| ext.eq_ignore_ascii_case("md"))
        .unwrap_or(false)
}

/// Parse a file already read by the index, so fingerprints and links describe the same bytes.
pub(crate) fn scan_file(path: &Path, graph_root: &Path, bytes: &[u8]) -> ScanResult {
    let mut source_file = extract_page_identifier(path, graph_root);
    let mut outgoing_links = Vec::new();
    if let Ok(content) = std::str::from_utf8(bytes) {
        if source_file.file_type == "md" {
            if is_excalidraw_markdown(content) {
                source_file.file_type = "excalidraw".to_string();
                if let Some(title) = source_file.title.strip_suffix(".excalidraw") {
                    source_file.title = title.to_string();
                }
            }
            outgoing_links = extract_links(content)
                .into_iter()
                .map(|link| parse_extracted_link(link, &source_file))
                .collect();
        } else if source_file.file_type == "html" || source_file.file_type == "svg" {
            outgoing_links = extract_markup_links(content)
                .into_iter()
                .map(|link| parse_extracted_link(link, &source_file))
                .collect();
        }
    }
    ScanResult {
        source_file,
        outgoing_links,
    }
}

#[cfg(test)]
fn resolve_links(mut scans: Vec<ScanResult>) -> Vec<ScanResult> {
    let resolver = crate::resolver::Resolver::new(
        scans.iter().map(|scan| &scan.source_file),
        &Default::default(),
    );
    for scan in &mut scans {
        for link in &mut scan.outgoing_links {
            resolver.resolve(link, &scan.source_file.directory);
        }
    }
    scans
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_links_wiki_basic() {
        let content = "Hello [[page one]] and [[page two]]";
        let links = extract_links(content);
        assert_eq!(
            links,
            vec![
                ExtractedLink::Wiki("page one".to_string()),
                ExtractedLink::Wiki("page two".to_string()),
            ]
        );
    }

    #[test]
    fn test_extract_links_wiki_skips_fenced_code_block() {
        let content = "Before\n```\n[[hidden link]]\n```\nAfter [[visible link]]";
        let links = extract_links(content);
        assert_eq!(links, vec![ExtractedLink::Wiki("visible link".to_string())]);
    }

    #[test]
    fn test_extract_links_wiki_skips_fenced_code_block_with_language() {
        let content = "Before\n```txt\n[[hidden link]]\nsome text\n```\nAfter [[visible link]]";
        let links = extract_links(content);
        assert_eq!(links, vec![ExtractedLink::Wiki("visible link".to_string())]);
    }

    #[test]
    fn test_extract_links_wiki_skips_inline_code() {
        let content = "Before `[[hidden link]]` and [[visible link]]";
        let links = extract_links(content);
        assert_eq!(links, vec![ExtractedLink::Wiki("visible link".to_string())]);
    }

    #[test]
    fn test_extract_links_wiki_skips_both_code_types() {
        let content = "Inline `[[a]]` and fenced:\n```\n[[b]]\n```\nReal [[c]]";
        let links = extract_links(content);
        assert_eq!(links, vec![ExtractedLink::Wiki("c".to_string())]);
    }

    #[test]
    fn test_extract_links_no_links() {
        let content = "No links here, just text.";
        let links = extract_links(content);
        assert!(links.is_empty());
    }

    #[test]
    fn test_extract_links_empty_string() {
        let links = extract_links("");
        assert!(links.is_empty());
    }

    #[test]
    fn test_extract_markup_links_finds_pages_and_assets() {
        let content = r#"<!doctype html>
            <link rel="stylesheet" href="./shared.css">
            <a href='../note.md'>Note</a>
            <img src="./image.svg" alt="">
            <script src="./behavior.js"></script>"#;
        assert_eq!(
            extract_markup_links(content),
            vec![
                ExtractedLink::Embedded(Box::new(ExtractedLink::Markdown {
                    text: "./shared.css".to_string(),
                    href: "./shared.css".to_string(),
                })),
                ExtractedLink::Markdown {
                    text: "../note.md".to_string(),
                    href: "../note.md".to_string(),
                },
                ExtractedLink::Embedded(Box::new(ExtractedLink::Markdown {
                    text: "./image.svg".to_string(),
                    href: "./image.svg".to_string(),
                })),
                ExtractedLink::Embedded(Box::new(ExtractedLink::Markdown {
                    text: "./behavior.js".to_string(),
                    href: "./behavior.js".to_string(),
                })),
            ]
        );
    }

    #[test]
    fn test_extract_markup_links_skips_external_and_non_url_attributes() {
        let content = r##"<a data-href="./not-a-link.md" href="#local">Local</a>
            <a href="https://example.com">External</a>
            <img src="data:image/svg+xml;base64,abc">
            <a HREF="./page.html">Page</a>"##;
        assert_eq!(
            extract_markup_links(content),
            vec![ExtractedLink::Markdown {
                text: "./page.html".to_string(),
                href: "./page.html".to_string(),
            }]
        );
    }

    #[test]
    fn test_extract_markup_links_finds_svg_shape_links() {
        let content = r##"<svg xmlns="http://www.w3.org/2000/svg">
            <a href="../page.md"><circle r="20"/></a>
            <a href="#local"><rect width="10" height="10"/></a>
            <image href="./texture.png"/>
        </svg>"##;
        assert_eq!(
            extract_markup_links(content),
            vec![
                ExtractedLink::Markdown {
                    text: "../page.md".to_string(),
                    href: "../page.md".to_string(),
                },
                ExtractedLink::Embedded(Box::new(ExtractedLink::Markdown {
                    text: "./texture.png".to_string(),
                    href: "./texture.png".to_string(),
                })),
            ]
        );
    }

    // --- Markdown link extraction tests ---

    #[test]
    fn test_extract_links_markdown_basic() {
        let content = "See [my page](./path/to/file.md) for details.";
        let links = extract_links(content);
        assert_eq!(
            links,
            vec![ExtractedLink::Markdown {
                text: "my page".to_string(),
                href: "./path/to/file.md".to_string(),
            }]
        );
    }

    #[test]
    fn wiki_embeds_are_distinct_from_links() {
        assert_eq!(
            extract_links("![[image.png]] [[image.png]]"),
            vec![
                ExtractedLink::Embedded(Box::new(ExtractedLink::Wiki("image.png".into()))),
                ExtractedLink::Wiki("image.png".into()),
            ]
        );
    }

    #[test]
    fn test_extract_links_markdown_image_embed() {
        let content = "An image: ![alt text](./images/photo.png)";
        let links = extract_links(content);
        assert_eq!(
            links,
            vec![ExtractedLink::Embedded(Box::new(ExtractedLink::Markdown {
                text: "alt text".to_string(),
                href: "./images/photo.png".to_string(),
            }))]
        );
    }

    #[test]
    fn test_extract_links_markdown_skips_external() {
        let content = "Visit [Google](https://google.com) and [local](./page.md)";
        let links = extract_links(content);
        assert_eq!(
            links,
            vec![ExtractedLink::Markdown {
                text: "local".to_string(),
                href: "./page.md".to_string(),
            }]
        );
    }

    #[test]
    fn test_extract_links_markdown_skips_anchor_only() {
        let content = "See [section](#heading) and [file](./file.md)";
        let links = extract_links(content);
        assert_eq!(
            links,
            vec![ExtractedLink::Markdown {
                text: "file".to_string(),
                href: "./file.md".to_string(),
            }]
        );
    }

    #[test]
    fn test_extract_links_markdown_skips_code_block() {
        let content = "Before\n```\n[hidden](./hidden.md)\n```\nAfter [visible](./visible.md)";
        let links = extract_links(content);
        assert_eq!(
            links,
            vec![ExtractedLink::Markdown {
                text: "visible".to_string(),
                href: "./visible.md".to_string(),
            }]
        );
    }

    #[test]
    fn test_extract_links_markdown_skips_inline_code() {
        let content = "Code `[hidden](./hidden.md)` and [visible](./visible.md)";
        let links = extract_links(content);
        assert_eq!(
            links,
            vec![ExtractedLink::Markdown {
                text: "visible".to_string(),
                href: "./visible.md".to_string(),
            }]
        );
    }

    #[test]
    fn test_extract_links_mixed_wiki_and_markdown() {
        let content = "Wiki [[page one]] and markdown [page two](./page-two.md)";
        let links = extract_links(content);
        assert_eq!(
            links,
            vec![
                ExtractedLink::Wiki("page one".to_string()),
                ExtractedLink::Markdown {
                    text: "page two".to_string(),
                    href: "./page-two.md".to_string(),
                },
            ]
        );
    }

    #[test]
    fn test_extract_links_excalidraw_element_links_section() {
        let content = [
            "## Element Links",
            "iWVOgeeI: [[page linked from a non-text element]]",
            "",
            "%%",
            "## Drawing",
            "```compressed-json",
            "[[hidden inside compressed scene text]]",
            "```",
        ]
        .join("\n");
        let links = extract_links(&content);
        assert_eq!(
            links,
            vec![ExtractedLink::Wiki(
                "page linked from a non-text element".to_string()
            ),]
        );
    }

    #[test]
    fn test_extract_links_markdown_with_anchor() {
        let content = "See [section](./file.md#heading)";
        let links = extract_links(content);
        assert_eq!(
            links,
            vec![ExtractedLink::Markdown {
                text: "section".to_string(),
                href: "./file.md#heading".to_string(),
            }]
        );
    }

    #[test]
    fn test_extract_links_markdown_relative_parent() {
        let content = "Go [up](../parent/file.md)";
        let links = extract_links(content);
        assert_eq!(
            links,
            vec![ExtractedLink::Markdown {
                text: "up".to_string(),
                href: "../parent/file.md".to_string(),
            }]
        );
    }

    // --- resolve_links: wiki link with directory prefix ---
    //
    // Wiki link prefixes (`[[sub/foo.png]]`) match Obsidian's vault-relative
    // semantics: the prefix is matched as a path *suffix* against any file in
    // the graph, not strictly rooted at graph_root. The exact-match fast path
    // covers the simple unwrapped case; the suffix fallback covers the case
    // where the user pointed `sourceDirectory` one level above the actual data.

    fn make_page(directory: &str, title: &str, file_type: &str) -> FileIdentifier {
        let path = if directory.is_empty() {
            format!("{}.{}", title, file_type)
        } else {
            format!("{}/{}.{}", directory, title, file_type)
        };
        FileIdentifier {
            directory: directory.to_string(),
            title: title.to_string(),
            file_type: file_type.to_string(),
            path,
        }
    }

    fn wiki_link(prefix: &str, title: &str, file_type: &str, source: &FileIdentifier) -> Link {
        Link {
            link_original_text: format!("{}{}.{}", prefix, title, file_type),
            link_source_page_path: source.path.clone(),
            link_parsed_directory: prefix.to_string(),
            link_parsed_title: title.to_string(),
            link_parsed_file_type: file_type.to_string(),
            link_parsed_anchor: None,
            link_parsed_anchor_type: None,
            link_parsed_alias: None,
            link_parsed_media_size: None,
            link_resolved_target_directory: String::new(),
            link_resolved_target_path: String::new(),
            is_relative_path_link: false,
            is_embedded: false,
            target: None,
            resolution: None,
            link_source_name: None,
            link_requested_target_source: None,
            link_resolved_target_source: None,
            link_source_error: None,
        }
    }

    fn extensionless_wiki_link(prefix: &str, title: &str, source: &FileIdentifier) -> Link {
        Link {
            link_original_text: format!("{}{}", prefix, title),
            link_source_page_path: source.path.clone(),
            link_parsed_directory: prefix.to_string(),
            link_parsed_title: title.to_string(),
            link_parsed_file_type: "md".to_string(),
            link_parsed_anchor: None,
            link_parsed_anchor_type: None,
            link_parsed_alias: None,
            link_parsed_media_size: None,
            link_resolved_target_directory: String::new(),
            link_resolved_target_path: String::new(),
            is_relative_path_link: false,
            is_embedded: false,
            target: None,
            resolution: None,
            link_source_name: None,
            link_requested_target_source: None,
            link_resolved_target_source: None,
            link_source_error: None,
        }
    }

    fn page_only(p: FileIdentifier) -> ScanResult {
        ScanResult {
            source_file: p,
            outgoing_links: vec![],
        }
    }

    fn page_with_link(p: FileIdentifier, link: Link) -> ScanResult {
        ScanResult {
            source_file: p,
            outgoing_links: vec![link],
        }
    }

    #[test]
    fn test_resolve_wiki_link_with_prefix_exact_match() {
        let source = make_page("", "embedded media", "md");
        let target = make_page("t006", "foo", "png");
        let link = wiki_link("t006/", "foo", "png", &source);
        let out = resolve_links(vec![page_with_link(source, link), page_only(target)]);
        let resolved = &out[0].outgoing_links[0];
        assert_eq!(resolved.link_resolved_target_directory, "t006");
        assert_eq!(resolved.link_resolved_target_path, "t006/foo.png");
    }

    #[test]
    fn test_resolve_wiki_link_with_prefix_falls_back_to_suffix_match() {
        // Wrapper case: graph_root is a directory above the actual notes, so
        // `[[t006/foo.png]]` from `data/embedded media.md` must still find
        // `data/t006/foo.png` even though no `t006/foo.png` exists at root.
        let source = make_page("data", "embedded media", "md");
        let target = make_page("data/t006", "foo", "png");
        let link = wiki_link("t006/", "foo", "png", &source);
        let out = resolve_links(vec![page_with_link(source, link), page_only(target)]);
        let resolved = &out[0].outgoing_links[0];
        assert_eq!(resolved.link_resolved_target_directory, "data/t006");
        assert_eq!(resolved.link_resolved_target_path, "data/t006/foo.png");
    }

    #[test]
    fn test_resolve_wiki_link_with_prefix_suffix_match_prefers_shallowest() {
        // Two candidates both end with `/sub`: shallowest wins.
        let source = make_page("", "src", "md");
        let shallow = make_page("shallow/sub", "foo", "png");
        let deep = make_page("deep/extra/sub", "foo", "png");
        let link = wiki_link("sub/", "foo", "png", &source);
        let out = resolve_links(vec![
            page_with_link(source, link),
            page_only(shallow),
            page_only(deep),
        ]);
        let resolved = &out[0].outgoing_links[0];
        assert_eq!(resolved.link_resolved_target_directory, "shallow/sub");
    }

    #[test]
    fn test_resolve_wiki_link_with_prefix_root_match_outranks_deeper_suffix() {
        // When both a root-rooted match and a deeper suffix match exist,
        // the exact (root) match wins via the fast path and never enters the
        // fallback.
        let source = make_page("", "src", "md");
        let at_root = make_page("t006", "foo", "png");
        let nested = make_page("data/t006", "foo", "png");
        let link = wiki_link("t006/", "foo", "png", &source);
        let out = resolve_links(vec![
            page_with_link(source, link),
            page_only(at_root),
            page_only(nested),
        ]);
        let resolved = &out[0].outgoing_links[0];
        assert_eq!(resolved.link_resolved_target_directory, "t006");
    }

    #[test]
    fn test_resolve_wiki_link_with_prefix_unresolvable_keeps_prefix() {
        // No file matches by title+file_type at all: leave the resolved
        // directory as the raw prefix so the link stays unresolvable rather
        // than collapsing to an unrelated file.
        let source = make_page("", "src", "md");
        let unrelated = make_page("other", "different", "png");
        let link = wiki_link("t006/", "foo", "png", &source);
        let out = resolve_links(vec![page_with_link(source, link), page_only(unrelated)]);
        let resolved = &out[0].outgoing_links[0];
        assert_eq!(resolved.link_resolved_target_directory, "t006");
        assert_eq!(resolved.link_resolved_target_path, "t006/foo.png");
    }

    #[test]
    fn test_resolve_extensionless_wiki_link_can_target_excalidraw() {
        let source = make_page("t006", "embedding page", "md");
        let target = make_page("t006 - second directory", "embedded drawing", "excalidraw");
        let link = extensionless_wiki_link("", "embedded drawing", &source);
        let out = resolve_links(vec![page_with_link(source, link), page_only(target)]);
        let resolved = &out[0].outgoing_links[0];
        assert_eq!(resolved.link_parsed_file_type, "excalidraw");
        assert_eq!(
            resolved.link_resolved_target_directory,
            "t006 - second directory"
        );
        assert_eq!(
            resolved.link_resolved_target_path,
            "t006 - second directory/embedded drawing.excalidraw"
        );
    }

    #[test]
    fn test_resolve_explicit_md_wiki_link_does_not_target_excalidraw() {
        let source = make_page("t006", "embedding page", "md");
        let target = make_page("t006 - second directory", "embedded drawing", "excalidraw");
        let link = wiki_link("", "embedded drawing", "md", &source);
        let out = resolve_links(vec![page_with_link(source, link), page_only(target)]);
        let resolved = &out[0].outgoing_links[0];
        assert_eq!(resolved.link_parsed_file_type, "md");
        assert_eq!(resolved.link_resolved_target_directory, "");
        assert_eq!(resolved.link_resolved_target_path, "embedded drawing.md");
    }
}
