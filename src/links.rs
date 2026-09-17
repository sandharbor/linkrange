use serde::{Deserialize, Serialize}; // Added for AnchorType serialization
use std::fmt;
use std::path::Path; // Added for AnchorType Display

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum AnchorType {
    Section,
    Block,
}

impl fmt::Display for AnchorType {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            AnchorType::Section => write!(f, "section"),
            AnchorType::Block => write!(f, "block"),
        }
    }
}

#[derive(Debug, PartialEq, Eq, Clone)]
pub struct LinkSemantics {
    pub title: String,
    pub target_path_prefix: String,
    pub alias: Option<String>,
    pub media_size: Option<u32>,
    pub anchor: Option<String>,
    pub anchor_type: Option<AnchorType>,
    pub file_type: String,
}

const IMAGE_EXTENSIONS: &[&str] = &[
    ".jpg",
    ".jpeg",
    ".png",
    ".gif",
    ".svg",
    ".webp",
    ".excalidraw",
];

/// All file type extensions recognized by the link parser.
/// Used for stripping extensions from titles and categorizing file types.
pub const KNOWN_FILE_TYPE_EXTENSIONS: &[&str] = &[
    "md",
    "html",
    "css",
    "js",
    "jpg",
    "jpeg",
    "png",
    "gif",
    "svg",
    "webp",
    "pdf",
    "txt",
    "excalidraw",
];

// Helper to strip anchor markers for robust extension checking
fn strip_anchor_markers(name: &str) -> &str {
    if let Some(pos) = name.rfind("#^") {
        &name[..pos]
    } else if let Some(pos) = name.rfind('^') {
        // For block anchors like `file^block`
        &name[..pos]
    } else if let Some(pos) = name.rfind('#') {
        // For section anchors like `file#section`
        &name[..pos]
    } else {
        name
    }
}

fn is_image_extension(filename_candidate: &str) -> bool {
    // filename_candidate is the part of the link before the *last* main pipe (if any).
    // It could be "image.png", "path/to/image.png#section", or even "image.png|internal_text_ignored_for_ext_check".
    // We need to get the actual filename part, which means taking content before any internal pipe, then stripping anchors.
    let base_for_extension_check = filename_candidate
        .split('|')
        .next()
        .unwrap_or(filename_candidate);
    let name_after_anchor_stripping = strip_anchor_markers(base_for_extension_check);

    if let Some(ext_osstr) = Path::new(name_after_anchor_stripping).extension() {
        if let Some(ext_str) = ext_osstr.to_str() {
            let lower_ext = format!(".{}", ext_str.to_lowercase());
            return IMAGE_EXTENSIONS
                .iter()
                .any(|&known_ext| lower_ext == *known_ext);
        }
    }
    false
}

pub fn parse_link_text(original_link_text: &str) -> LinkSemantics {
    // Normalize escaped pipes (\|) to regular pipes (|).
    // In markdown tables, Obsidian escapes the alias pipe as \| to avoid
    // conflicting with the table cell separator.
    let normalized = original_link_text.replace("\\|", "|");
    let link_text = normalized.as_str();

    let mut text_for_path_and_title_and_anchor = link_text;
    let mut parsed_alias: Option<String> = None;
    let mut parsed_media_size: Option<u32> = None;

    if let Some(pipe_idx) = link_text.rfind('|') {
        let before_pipe = &link_text[..pipe_idx];
        let after_pipe = &link_text[pipe_idx + 1..];

        text_for_path_and_title_and_anchor = before_pipe;

        // is_image_extension is now robust to anchors in `before_pipe`
        if is_image_extension(before_pipe) {
            if let Ok(num) = after_pipe.parse::<u32>() {
                if !after_pipe.starts_with('+')
                    && !after_pipe.starts_with('-')
                    && after_pipe.chars().all(char::is_numeric)
                {
                    parsed_media_size = Some(num);
                } else {
                    parsed_alias = Some(after_pipe.to_string());
                }
            } else {
                parsed_alias = Some(after_pipe.to_string());
            }
        } else {
            parsed_alias = Some(after_pipe.to_string());
        }
    }

    let parsed_prefix: String;
    let title_part_with_potential_anchor: String;

    if text_for_path_and_title_and_anchor.is_empty() {
        parsed_prefix = "".to_string();
        title_part_with_potential_anchor = "".to_string();
    } else if let Some(slash_idx) = text_for_path_and_title_and_anchor.rfind('/') {
        parsed_prefix = text_for_path_and_title_and_anchor[..=slash_idx].to_string();
        title_part_with_potential_anchor =
            text_for_path_and_title_and_anchor[slash_idx + 1..].to_string();
    } else {
        parsed_prefix = "".to_string();
        title_part_with_potential_anchor = text_for_path_and_title_and_anchor.to_string();
    }

    let mut final_title = title_part_with_potential_anchor.clone();
    let mut final_anchor: Option<String> = None;
    let mut final_anchor_type: Option<AnchorType> = None;

    if let Some(pos) = final_title.rfind("#^") {
        final_anchor = Some(final_title[pos + 2..].to_string());
        final_anchor_type = Some(AnchorType::Block);
        final_title = final_title[..pos].to_string();
    } else if let Some(pos) = final_title.rfind('^') {
        final_anchor = Some(final_title[pos + 1..].to_string());
        final_anchor_type = Some(AnchorType::Block);
        final_title = final_title[..pos].to_string();
    } else if let Some(pos) = final_title.rfind('#') {
        final_anchor = Some(final_title[pos + 1..].to_string());
        final_anchor_type = Some(AnchorType::Section);
        final_title = final_title[..pos].to_string();
    }

    // Extract file_type and update title if an extension is present
    let mut title_to_return = final_title.clone();
    let mut file_type_from_title = "md".to_string(); // Default file type

    // Only attempt to parse extension if final_title does not contain an internal pipe
    if !final_title.contains('|') {
        if let Some(dot_pos) = final_title.rfind('.') {
            // Check if dot is not the first char and there is an extension part
            if dot_pos > 0 && dot_pos < final_title.len() - 1 {
                let potential_ext = final_title[dot_pos + 1..].to_lowercase();
                let known_file_type_extensions = KNOWN_FILE_TYPE_EXTENSIONS;

                if known_file_type_extensions.contains(&potential_ext.as_str()) {
                    file_type_from_title = potential_ext;
                    title_to_return = final_title[..dot_pos].to_string();
                } else {
                    // Check if this looks like a file extension (short, alphanumeric, no spaces)
                    // vs. just punctuation in a sentence
                    let looks_like_extension = potential_ext.len() <= 5
                        && potential_ext.chars().all(|c| c.is_alphanumeric())
                        && !potential_ext.contains(' ');

                    if looks_like_extension {
                        file_type_from_title = "other".to_string();
                        title_to_return = final_title[..dot_pos].to_string();
                    }
                    // If it doesn't look like an extension, keep the full title and default file_type
                }
            }
        }
    }

    LinkSemantics {
        title: title_to_return,
        target_path_prefix: parsed_prefix,
        alias: parsed_alias,
        media_size: parsed_media_size,
        anchor: final_anchor,
        anchor_type: final_anchor_type,
        file_type: file_type_from_title,
    }
}

/// Parses a Markdown or HTML URL (e.g., `../dir/file.md#section`) into LinkSemantics.
/// Unlike `parse_link_text` (which handles Obsidian wiki-link inner text with pipes, media sizes, etc.),
/// this function handles explicit relative paths where the file extension is always present
/// and display text / alias are provided separately by the caller.
pub fn parse_markdown_link_href(href: &str) -> LinkSemantics {
    let mut path_portion = href;
    let mut final_anchor: Option<String> = None;
    let mut final_anchor_type: Option<AnchorType> = None;

    // Extract anchor (same priority as wiki links: #^ then # )
    if let Some(pos) = path_portion.find("#^") {
        final_anchor = Some(path_portion[pos + 2..].to_string());
        final_anchor_type = Some(AnchorType::Block);
        path_portion = &path_portion[..pos];
    } else if let Some(pos) = path_portion.find('#') {
        final_anchor = Some(path_portion[pos + 1..].to_string());
        final_anchor_type = Some(AnchorType::Section);
        path_portion = &path_portion[..pos];
    }

    // Separate URL syntax before decoding so escaped '#' and '?' remain part
    // of the filename. Decode once; '+' in a path is a literal plus, not a space.
    path_portion = path_portion
        .split_once('?')
        .map_or(path_portion, |(path, _)| path);
    let decoded = percent_encoding::percent_decode_str(path_portion)
        .decode_utf8()
        .unwrap_or(std::borrow::Cow::Borrowed(path_portion));
    let path_portion = decoded.as_ref();

    // Split into directory prefix and filename
    let (parsed_prefix, filename) = if let Some(slash_idx) = path_portion.rfind('/') {
        (
            path_portion[..=slash_idx].to_string(),
            &path_portion[slash_idx + 1..],
        )
    } else {
        (String::new(), path_portion)
    };

    // Extract file extension and title
    let mut title = filename.to_string();
    let mut file_type = "md".to_string();

    if let Some(dot_pos) = filename.rfind('.') {
        if dot_pos > 0 && dot_pos < filename.len() - 1 {
            let ext = filename[dot_pos + 1..].to_lowercase();
            let known_extensions = KNOWN_FILE_TYPE_EXTENSIONS;
            if known_extensions.contains(&ext.as_str()) {
                file_type = ext;
                title = filename[..dot_pos].to_string();
            } else {
                let looks_like_extension = ext.len() <= 5
                    && ext.chars().all(|c| c.is_alphanumeric())
                    && !ext.contains(' ');
                if looks_like_extension {
                    file_type = "other".to_string();
                    title = filename[..dot_pos].to_string();
                }
            }
        }
    }

    LinkSemantics {
        title,
        target_path_prefix: parsed_prefix,
        alias: None,
        media_size: None,
        anchor: final_anchor,
        anchor_type: final_anchor_type,
        file_type,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Updated check function to include anchor, anchor_type, and file_type
    #[allow(clippy::too_many_arguments)]
    fn check(
        original: &str,
        expected_title: &str,
        expected_file_type: &str,
        expected_prefix: &str,
        expected_alias: Option<&str>,
        expected_media_size: Option<u32>,
        expected_anchor: Option<&str>,
        expected_anchor_type: Option<AnchorType>,
    ) {
        assert_eq!(
            parse_link_text(original),
            LinkSemantics {
                title: expected_title.to_string(),
                target_path_prefix: expected_prefix.to_string(),
                alias: expected_alias.map(String::from),
                media_size: expected_media_size,
                anchor: expected_anchor.map(String::from),
                anchor_type: expected_anchor_type,
                file_type: expected_file_type.to_string(),
            }
        );
    }

    #[test]
    fn test_simple_page() {
        check(
            "simple_page",
            "simple_page",
            "md",
            "",
            None,
            None,
            None,
            None,
        );
    }

    #[test]
    fn test_path_to_doc() {
        check(
            "path/to/doc",
            "doc",
            "md",
            "path/to/",
            None,
            None,
            None,
            None,
        );
    }

    #[test]
    fn test_path_to_doc_with_alias() {
        check(
            "path/to/doc|My Document",
            "doc",
            "md",
            "path/to/",
            Some("My Document"),
            None,
            None,
            None,
        );
    }

    #[test]
    fn test_image_with_media_size() {
        check(
            "image.png|300",
            "image",
            "png",
            "",
            None,
            Some(300),
            None,
            None,
        );
    }

    #[test]
    fn test_image_with_uppercase_ext_media_size() {
        check(
            "image.JPG|250",
            "image",
            "jpg",
            "",
            None,
            Some(250),
            None,
            None,
        );
        check(
            "image.PNG|300",
            "image",
            "png",
            "",
            None,
            Some(300),
            None,
            None,
        );
    }

    #[test]
    fn test_path_to_image_with_media_size() {
        check(
            "path/to/image.jpeg|500",
            "image",
            "jpeg",
            "path/to/",
            None,
            Some(500),
            None,
            None,
        );
    }

    #[test]
    fn test_image_with_text_alias() {
        check(
            "image.gif|NotASize",
            "image",
            "gif",
            "",
            Some("NotASize"),
            None,
            None,
            None,
        );
    }

    #[test]
    fn test_image_with_numeric_alias_that_is_not_plain_number() {
        check(
            "image.png|+300",
            "image",
            "png",
            "",
            Some("+300"),
            None,
            None,
            None,
        );
        check(
            "image.png|300a",
            "image",
            "png",
            "",
            Some("300a"),
            None,
            None,
            None,
        );
    }

    #[test]
    fn test_document_with_numeric_alias() {
        check(
            "document.pdf|300",
            "document",
            "pdf",
            "",
            Some("300"),
            None,
            None,
            None,
        );
    }

    #[test]
    fn test_page_with_empty_alias() {
        check("page|", "page", "md", "", Some(""), None, None, None);
    }

    #[test]
    fn test_just_alias() {
        check(
            "|JustAlias",
            "",
            "md",
            "",
            Some("JustAlias"),
            None,
            None,
            None,
        );
    }

    #[test]
    fn test_path_with_empty_alias_for_image() {
        check("a/b/c.jpg|", "c", "jpg", "a/b/", Some(""), None, None, None);
    }

    #[test]
    fn test_no_extension_file_with_numeric_alias() {
        check(
            "no_extension|300",
            "no_extension",
            "md",
            "",
            Some("300"),
            None,
            None,
            None,
        );
    }

    #[test]
    fn test_empty_string() {
        check("", "", "md", "", None, None, None, None);
    }

    #[test]
    fn test_only_path_no_filename() {
        check("path/to/", "", "md", "path/to/", None, None, None, None);
    }

    #[test]
    fn test_only_path_with_alias() {
        check(
            "path/to/|Alias",
            "",
            "md",
            "path/to/",
            Some("Alias"),
            None,
            None,
            None,
        );
    }

    #[test]
    fn test_complex_image_name_with_dots_media_size() {
        check(
            "archive.tar.gz.jpg|700",
            "archive.tar.gz",
            "jpg",
            "",
            None,
            Some(700),
            None,
            None,
        );
    }

    #[test]
    fn test_link_with_multiple_pipes_alias() {
        check(
            "title|alias1|alias2",
            "title|alias1",
            "md",
            "",
            Some("alias2"),
            None,
            None,
            None,
        );
    }

    #[test]
    fn test_image_link_with_multiple_pipes_media_size() {
        check(
            "image.png|text|300",
            "image.png|text",
            "md",
            "",
            None,
            Some(300),
            None,
            None,
        );
    }

    #[test]
    fn test_image_link_with_multiple_pipes_alias_instead_of_size() {
        check(
            "image.png|text|alias",
            "image.png|text",
            "md",
            "",
            Some("alias"),
            None,
            None,
            None,
        );
    }

    // New tests for anchors
    #[test]
    fn test_page_with_section_anchor() {
        check(
            "Page#section",
            "Page",
            "md",
            "",
            None,
            None,
            Some("section"),
            Some(AnchorType::Section),
        );
    }

    #[test]
    fn test_page_with_block_anchor_hash_caret() {
        check(
            "Page#^block",
            "Page",
            "md",
            "",
            None,
            None,
            Some("block"),
            Some(AnchorType::Block),
        );
    }

    #[test]
    fn test_page_with_block_anchor_caret_only() {
        check(
            "Page^block",
            "Page",
            "md",
            "",
            None,
            None,
            Some("block"),
            Some(AnchorType::Block),
        );
    }

    #[test]
    fn test_path_to_page_with_section_anchor() {
        check(
            "path/to/Page.md#section",
            "Page",
            "md",
            "path/to/",
            None,
            None,
            Some("section"),
            Some(AnchorType::Section),
        );
    }

    #[test]
    fn test_page_with_alias_and_section_anchor() {
        check(
            "Page#section|My Alias",
            "Page",
            "md",
            "",
            Some("My Alias"),
            None,
            Some("section"),
            Some(AnchorType::Section),
        );
    }

    #[test]
    fn test_image_with_media_size_and_block_anchor() {
        check(
            "image.png#^blockid|300",
            "image",
            "png",
            "",
            None,
            Some(300),
            Some("blockid"),
            Some(AnchorType::Block),
        );
    }

    #[test]
    fn test_image_jpg_with_media_size_and_block_anchor() {
        check(
            "image.jpg#^blockid|300",
            "image",
            "jpg",
            "",
            None,
            Some(300),
            Some("blockid"),
            Some(AnchorType::Block),
        );
    }

    #[test]
    fn test_image_with_alias_and_section_anchor() {
        check(
            "image.png#section|thumbnail",
            "image",
            "png",
            "",
            Some("thumbnail"),
            None,
            Some("section"),
            Some(AnchorType::Section),
        );
    }

    #[test]
    fn test_empty_title_with_section_anchor() {
        check(
            "#section",
            "",
            "md",
            "",
            None,
            None,
            Some("section"),
            Some(AnchorType::Section),
        );
    }

    #[test]
    fn test_empty_title_with_block_anchor() {
        check(
            "#^block",
            "",
            "md",
            "",
            None,
            None,
            Some("block"),
            Some(AnchorType::Block),
        );
    }

    #[test]
    fn test_path_and_empty_title_with_block_anchor() {
        check(
            "path/to/#^block",
            "",
            "md",
            "path/to/",
            None,
            None,
            Some("block"),
            Some(AnchorType::Block),
        );
    }

    #[test]
    fn test_link_with_anchor_containing_spaces_and_dots() {
        check(
            "file.md#Header section 1.0",
            "file",
            "md",
            "",
            None,
            None,
            Some("Header section 1.0"),
            Some(AnchorType::Section),
        );
    }

    #[test]
    fn test_filename_with_caret_not_as_anchor() {
        check(
            "file^name.md",
            "file",
            "md",
            "",
            None,
            None,
            Some("name.md"),
            Some(AnchorType::Block),
        );
        check(
            "file#name.md",
            "file",
            "md",
            "",
            None,
            None,
            Some("name.md"),
            Some(AnchorType::Section),
        );
        check(
            "file#^name.md",
            "file",
            "md",
            "",
            None,
            None,
            Some("name.md"),
            Some(AnchorType::Block),
        );
    }

    // --- New tests for file type extraction ---
    #[test]
    fn test_document_with_pdf_extension() {
        check("mydoc.pdf", "mydoc", "pdf", "", None, None, None, None);
    }

    #[test]
    fn test_document_with_txt_extension() {
        check("notes.txt", "notes", "txt", "", None, None, None, None);
    }

    #[test]
    fn test_document_with_excalidraw_extension() {
        check(
            "diagram.excalidraw",
            "diagram",
            "excalidraw",
            "",
            None,
            None,
            None,
            None,
        );
    }

    #[test]
    fn test_document_with_excalidraw_extension_and_alias() {
        check(
            "diagram.excalidraw|My Diagram",
            "diagram",
            "excalidraw",
            "",
            Some("My Diagram"),
            None,
            None,
            None,
        );
    }

    #[test]
    fn test_document_with_unknown_extension() {
        check(
            "archive.zip",
            "archive",
            "other",
            "",
            None,
            None,
            None,
            None,
        );
    }

    #[test]
    fn test_title_with_dots_and_known_extension() {
        check(
            "version1.2.document.md",
            "version1.2.document",
            "md",
            "",
            None,
            None,
            None,
            None,
        );
    }

    #[test]
    fn test_title_with_dots_and_unknown_extension() {
        check(
            "backup.rev1.dat",
            "backup.rev1",
            "other",
            "",
            None,
            None,
            None,
            None,
        );
    }

    #[test]
    fn test_leading_dot_filename_no_extension_stripping() {
        check(".bashrc", ".bashrc", "md", "", None, None, None, None);
    }

    #[test]
    fn test_leading_dot_filename_with_known_extension_after_dot() {
        check(".config.md", ".config", "md", "", None, None, None, None);
    }

    #[test]
    fn test_trailing_dot_filename_no_extension_stripping() {
        check("file.", "file.", "md", "", None, None, None, None);
    }

    #[test]
    fn test_image_with_excalidraw_extension_and_size() {
        check(
            "my_drawing.excalidraw|400",
            "my_drawing",
            "excalidraw",
            "",
            None,
            Some(400),
            None,
            None,
        );
    }

    #[test]
    fn test_non_image_file_with_extension_and_numeric_alias() {
        check(
            "file.log|300",
            "file",
            "other",
            "",
            Some("300"),
            None,
            None,
            None,
        );
    }

    #[test]
    fn test_period_not_followed_by_known_extension_stays_in_title() {
        check(
            "this vs. that",
            "this vs. that",
            "md",
            "",
            None,
            None,
            None,
            None,
        );
    }

    #[test]
    fn test_period_with_unknown_extension_stays_in_title() {
        check(
            "file.unknown",
            "file.unknown",
            "md",
            "",
            None,
            None,
            None,
            None,
        );
    }

    #[test]
    fn test_multiple_periods_without_known_extension() {
        check(
            "version 1.2.3 notes",
            "version 1.2.3 notes",
            "md",
            "",
            None,
            None,
            None,
            None,
        );
    }

    #[test]
    fn test_escaped_pipe_alias_in_table() {
        check(
            "t011/t011 --- table test row 1 column 1\\|table test row 1 column 1",
            "t011 --- table test row 1 column 1",
            "md",
            "t011/",
            Some("table test row 1 column 1"),
            None,
            None,
            None,
        );
    }

    // --- Tests for parse_markdown_link_href ---

    fn check_md_href(
        href: &str,
        expected_title: &str,
        expected_file_type: &str,
        expected_prefix: &str,
        expected_anchor: Option<&str>,
        expected_anchor_type: Option<AnchorType>,
    ) {
        assert_eq!(
            parse_markdown_link_href(href),
            LinkSemantics {
                title: expected_title.to_string(),
                target_path_prefix: expected_prefix.to_string(),
                alias: None,
                media_size: None,
                anchor: expected_anchor.map(String::from),
                anchor_type: expected_anchor_type,
                file_type: expected_file_type.to_string(),
            }
        );
    }

    #[test]
    fn test_md_href_simple_file() {
        check_md_href("file.md", "file", "md", "", None, None);
    }

    #[test]
    fn test_html_and_web_asset_hrefs() {
        check_md_href("page.html", "page", "html", "", None, None);
        check_md_href("assets/site.css", "site", "css", "assets/", None, None);
        check_md_href("assets/site.js", "site", "js", "assets/", None, None);
    }

    #[test]
    fn test_md_href_relative_parent_dir() {
        check_md_href("../dir/file.md", "file", "md", "../dir/", None, None);
    }

    #[test]
    fn test_md_href_current_dir_image() {
        check_md_href(
            "./images/photo.png",
            "photo",
            "png",
            "./images/",
            None,
            None,
        );
    }

    #[test]
    fn test_md_href_section_anchor() {
        check_md_href(
            "file.md#section",
            "file",
            "md",
            "",
            Some("section"),
            Some(AnchorType::Section),
        );
    }

    #[test]
    fn test_md_href_block_anchor() {
        check_md_href(
            "file.md#^blockid",
            "file",
            "md",
            "",
            Some("blockid"),
            Some(AnchorType::Block),
        );
    }

    #[test]
    fn test_md_href_deep_path_pdf() {
        check_md_href(
            "deep/nested/path/file.pdf",
            "file",
            "pdf",
            "deep/nested/path/",
            None,
            None,
        );
    }

    #[test]
    fn test_md_href_no_extension() {
        check_md_href("file", "file", "md", "", None, None);
    }

    #[test]
    fn test_md_href_url_encoded_spaces() {
        check_md_href(
            "name%20with%20spaces.md",
            "name with spaces",
            "md",
            "",
            None,
            None,
        );
    }

    #[test]
    fn test_md_href_invalid_escapes_do_not_change_the_filename() {
        for filename in ["literal%", "literal%2", "literal%GG", "literal%FF"] {
            check_md_href(&format!("{filename}.js"), filename, "js", "", None, None);
        }
    }

    #[test]
    fn test_md_href_path_with_anchor_and_section() {
        check_md_href(
            "../dir/page.md#heading",
            "page",
            "md",
            "../dir/",
            Some("heading"),
            Some(AnchorType::Section),
        );
    }

    #[test]
    fn test_md_href_image_with_anchor() {
        check_md_href(
            "./img.png#section",
            "img",
            "png",
            "./",
            Some("section"),
            Some(AnchorType::Section),
        );
    }

    #[test]
    fn test_md_href_unknown_extension() {
        check_md_href("archive.zip", "archive", "other", "", None, None);
    }

    #[test]
    fn test_md_href_multiple_parent_dirs() {
        check_md_href(
            "../../other/file.md",
            "file",
            "md",
            "../../other/",
            None,
            None,
        );
    }
}
