use crate::parser::{is_internal_html_target, ExtractedLink};
use html5ever::{
    tendril::StrTendril,
    tokenizer::{
        states::RawKind, BufferQueue, TagKind, Token, TokenSink, TokenSinkResult, Tokenizer,
    },
};
use std::cell::RefCell;

#[derive(Default)]
struct Sink(RefCell<Vec<ExtractedLink>>);
impl TokenSink for Sink {
    type Handle = ();
    fn process_token(&self, token: Token, _: u64) -> TokenSinkResult<()> {
        if let Token::TagToken(tag) = token {
            if tag.kind != TagKind::StartTag {
                return TokenSinkResult::Continue;
            }
            let element: &str = tag.name.as_ref();
            let rel = tag
                .attrs
                .iter()
                .find(|a| &*a.name.local == "rel")
                .map(|a| a.value.to_ascii_lowercase())
                .unwrap_or_default();
            for attr in &tag.attrs {
                let attribute: &str = attr.name.local.as_ref();
                let embedded = match (element, attribute) {
                    ("a" | "area", "href" | "xlink:href") => false,
                    ("image" | "use", "href" | "xlink:href") => true,
                    (
                        "img" | "script" | "audio" | "video" | "source" | "track" | "iframe"
                        | "embed" | "input",
                        "src",
                    ) => true,
                    ("video", "poster") | ("object", "data") => true,
                    ("link", "href")
                        if rel.split_whitespace().any(|r| {
                            matches!(r, "stylesheet" | "icon" | "preload" | "modulepreload")
                        }) =>
                    {
                        true
                    }
                    _ => continue,
                };
                let href = attr.value.trim();
                if is_internal_html_target(href) {
                    let link = ExtractedLink::Markdown {
                        text: href.into(),
                        href: href.into(),
                    };
                    self.0.borrow_mut().push(if embedded {
                        ExtractedLink::Embedded(Box::new(link))
                    } else {
                        link
                    });
                }
            }
            return match element {
                "script" => TokenSinkResult::RawData(RawKind::ScriptData),
                "style" | "xmp" | "iframe" | "noembed" | "noframes" => {
                    TokenSinkResult::RawData(RawKind::Rawtext)
                }
                "title" | "textarea" => TokenSinkResult::RawData(RawKind::Rcdata),
                _ => TokenSinkResult::Continue,
            };
        }
        TokenSinkResult::Continue
    }
}

pub(crate) fn extract(content: &str) -> Vec<ExtractedLink> {
    let tokenizer = Tokenizer::new(Sink::default(), Default::default());
    let input = BufferQueue::default();
    input.push_back(StrTendril::from(content));
    let _ = tokenizer.feed(&input);
    tokenizer.end();
    tokenizer.sink.0.into_inner()
}
