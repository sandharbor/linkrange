//! Links in a PDF: its link annotations' URIs, and the files their `GoToR`
//! and `Launch` actions open. They are relative to the PDF, as an HTML page's
//! URLs are, and web addresses are left out, as they are for HTML. A PDF that
//! cannot be read has no links.

use crate::parser::ExtractedLink;
use lopdf::{decode_text_string, Dictionary, Document, Object};
use percent_encoding::{utf8_percent_encode, AsciiSet, CONTROLS};

/// A file name as a URL path: `%`, `#`, and `?` stay part of the name.
const FILE_NAME: &AsciiSet = &CONTROLS.add(b'%').add(b'#').add(b'?');

pub(crate) fn extract_links(bytes: &[u8]) -> Vec<ExtractedLink> {
    let Ok(doc) = Document::load_mem(bytes) else {
        return Vec::new();
    };
    let mut links = Vec::new();
    for page_id in doc.get_pages().into_values() {
        let Ok(page) = doc.get_dictionary(page_id) else {
            continue;
        };
        let Ok(annotations) = page.get_deref(b"Annots", &doc).and_then(Object::as_array) else {
            continue;
        };
        for annotation in annotations {
            let Some(annotation) = doc
                .dereference(annotation)
                .ok()
                .and_then(|(_, object)| object.as_dict().ok())
            else {
                continue;
            };
            let is_link = annotation.get(b"Subtype").and_then(Object::as_name).ok()
                == Some(b"Link".as_slice());
            if let Some(target) = is_link.then(|| link_target(annotation, &doc)).flatten() {
                if crate::parser::is_internal_html_target(&target) {
                    links.push(ExtractedLink::Markup(target));
                }
            }
        }
    }
    links
}

/// Where a link annotation's action goes: its URI, or the file it opens as a
/// URL path. An absolute file path is outside any source, so it is no link.
fn link_target(annotation: &Dictionary, doc: &Document) -> Option<String> {
    let action = annotation.get_deref(b"A", doc).ok()?.as_dict().ok()?;
    match action.get(b"S").and_then(Object::as_name).ok()? {
        b"URI" => {
            let uri = action.get_deref(b"URI", doc).ok()?.as_str().ok()?;
            Some(String::from_utf8_lossy(uri).into_owned())
        }
        b"GoToR" | b"Launch" => {
            let file = file_name(action.get_deref(b"F", doc).ok()?, doc)?;
            let absolute = file.starts_with('/') || file.as_bytes().get(1) == Some(&b':');
            (!absolute).then(|| utf8_percent_encode(&file, FILE_NAME).to_string())
        }
        _ => None,
    }
}

/// The file a file specification names: a string, or a dictionary's `UF`
/// (Unicode) or `F` entry.
fn file_name(spec: &Object, doc: &Document) -> Option<String> {
    let name = match spec {
        Object::Dictionary(dictionary) => dictionary
            .get_deref(b"UF", doc)
            .or_else(|_| dictionary.get_deref(b"F", doc))
            .ok()?,
        other => other,
    };
    decode_text_string(name)
        .ok()
        .filter(|name| !name.is_empty())
}
