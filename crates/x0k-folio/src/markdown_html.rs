//! Markdown → themed HTML conversion.
//!
//! Phase 3's body-format dispatch: documents whose header states no
//! `x0k:bodyFormat` (markdown, the default) flow through this converter
//! on their way to the substrate. HTML bodies (`x0k:bodyFormat "html"`)
//! skip this and go straight to the substrate.
//!
//! # Semantic CSS classes
//!
//! Themes export a vocabulary of semantic CSS classes (see
//! `corpora/x0k/decisions/design/presentation/representation-axes.md`). The converter injects
//! those classes onto block elements so the same body markup reads as
//! that theme regardless of which one is active. Today's vocabulary:
//!
//! | Element | Class | Notes |
//! |---------|-------|-------|
//! | `<h1>` | `x0k-display` | Title-scale heading |
//! | `<h2>` | `x0k-heading` | Section heading |
//! | `<h3>`..`<h6>` | `x0k-subheading` | Lower-level headings |
//! | `<p>` | `x0k-body` | Body paragraph |
//! | `<code>` (inline) | `x0k-mono` | Inline monospace |
//! | `<pre><code>` (fenced) | `x0k-code-block` | Fenced code block; the language tag, when present, is preserved as `language-<lang>` per CommonMark convention |
//! | `<blockquote>` | `x0k-blockquote` | Pulled-out quoted prose |
//! | `<table>` | `x0k-table` | GFM tables |
//!
//! Every heading carries its slug as `id`
//! ([`crate::transclusion::heading_slug`]), so a link's `#section`
//! names it. A block whose marker gives it an id
//! (`[block-7a3f]: #block-7a3f`, `x0k:implementation/folio/block-identity`)
//! carries that id, so a link's `#block-…` names it too.
//!
//! An `x0k:transclude` fence becomes the empty element that names its
//! source, `<div class="transclusion" data-ref="…">`; the host that shows
//! the page fills it (`x0k:design/transclusion`, "The primitive").
//!
//! Lists, links, images, and emphasis ride on their semantic element
//! names; themes style them by element selector. We don't add classes
//! for things themes can reach via plain CSS — that just bloats the
//! emitted markup.
//!
//! # Implementation note
//!
//! The converter uses `pulldown_cmark`'s event stream and emits HTML
//! by hand. The simpler approach — `pulldown_cmark::html::push_html`
//! followed by string substitution — was prototyped but produced
//! mis-classification on nested blocks (e.g. a `<p>` inside a `<li>`
//! got the body class even though themes usually style list-item
//! paragraphs differently). Streaming the events lets us scope class
//! injection to top-level blocks. The implementation stays small —
//! we're not reimplementing pulldown, just rewriting the few tags
//! that need theme classes.

use pulldown_cmark::{CodeBlockKind, Event, HeadingLevel, Options, Parser, Tag, TagEnd};

/// Convert a markdown body to a themed HTML string.
///
/// The output is a fragment — no `<html>`, `<head>`, or `<body>` —
/// because the substrate ingests it under a theme-controlled root.
/// The body wrapper is added by [`wrap_for_substrate`] when the host
/// needs a full document.
pub fn markdown_to_html(markdown: &str) -> String {
    let mut opts = Options::empty();
    opts.insert(Options::ENABLE_TABLES);
    opts.insert(Options::ENABLE_FOOTNOTES);
    opts.insert(Options::ENABLE_STRIKETHROUGH);
    opts.insert(Options::ENABLE_TASKLISTS);
    opts.insert(Options::ENABLE_SMART_PUNCTUATION);

    let parser = Parser::new_ext(markdown, opts).into_offset_iter();
    let mut out = String::with_capacity(markdown.len() * 2);
    let mut slugs = heading_slugs(markdown, opts).into_iter();
    // A loose list item's marker would also name its first paragraph.
    let mut named = std::collections::HashSet::new();
    // The blocks this body's figures bind, by name: data and panels.
    let data = crate::inline_entity::document_data(markdown);
    let panels = crate::inline_entity::document_params(markdown);
    // An `x0k:media` fence becomes a mount point and an `x0k:transclude`
    // fence the element its host fills; the body of either is dropped.
    let mut media = 0usize;
    let mut in_media = false;

    for (event, range) in parser {
        match event {
            Event::Start(Tag::CodeBlock(CodeBlockKind::Fenced(ref info))) if media_fence(info).is_some() => {
                let (media_ref, height) = media_fence(info).unwrap_or_default();
                let source = crate::inline_entity::media_source(info);
                let payload = source
                    .as_deref()
                    .and_then(|name| data.iter().find(|block| block.name == name))
                    .map(|block| block.payload.as_str());
                let params = source
                    .as_deref()
                    .and_then(|name| panels.iter().find(|panel| panel.name == name))
                    .map(|panel| panel.to_json());
                let binding = MountBinding { source: source.as_deref(), payload, params: params.as_deref() };
                write_media_mount(&mut out, media, &media_ref, height, media_child_height(info), binding);
                media += 1;
                in_media = true;
            }
            Event::Start(Tag::CodeBlock(CodeBlockKind::Fenced(ref info))) if transclude_fence(info).is_some() => {
                out.push_str("<div class=\"transclusion\" data-ref=\"");
                escape_attr(&mut out, &transclude_fence(info).unwrap_or_default());
                out.push_str("\"></div>");
                in_media = true;
            }
            Event::End(TagEnd::CodeBlock) if in_media => in_media = false,
            Event::Text(_) if in_media => {}
            Event::Start(tag) => {
                let id = match &tag {
                    Tag::Heading { id: Some(_), .. } => {
                        slugs.next();
                        None
                    }
                    Tag::Heading { .. } => block_marker(markdown, range.start).or_else(|| slugs.next()),
                    Tag::Paragraph | Tag::Item | Tag::BlockQuote(_) | Tag::Table(_) | Tag::CodeBlock(_) => {
                        block_marker(markdown, range.start)
                    }
                    _ => None,
                };
                let at = out.len();
                write_start(&mut out, tag);
                if let Some(id) = id.filter(|id| named.insert(id.clone())) {
                    give_id(&mut out, at, &id);
                }
            }
            Event::End(tag_end) => write_end(&mut out, tag_end),
            Event::Text(t) => escape_html(&mut out, &t),
            Event::Code(t) => {
                out.push_str("<code class=\"x0k-mono\">");
                escape_html(&mut out, &t);
                out.push_str("</code>");
            }
            Event::Html(html) | Event::InlineHtml(html) => {
                // Pass HTML chunks through as-is. The substrate (and
                // the eventual Phase 0 normalizer when this round-
                // trips back to disk) sanitize. For markdown bodies
                // displayed in the viewer, inline HTML is rare; for
                // converted HTML bodies we don't pass through this
                // function at all.
                out.push_str(&html);
            }
            Event::SoftBreak => out.push('\n'),
            Event::HardBreak => out.push_str("<br>"),
            Event::Rule => out.push_str("<hr>"),
            Event::FootnoteReference(name) => {
                out.push_str("<sup class=\"x0k-footnote-ref\"><a href=\"#fn-");
                escape_attr(&mut out, &name);
                out.push_str("\">");
                escape_html(&mut out, &name);
                out.push_str("</a></sup>");
            }
            Event::TaskListMarker(checked) => {
                out.push_str(if checked {
                    "<input type=\"checkbox\" checked disabled> "
                } else {
                    "<input type=\"checkbox\" disabled> "
                });
            }
            // Math extension events surface when ENABLE_MATH is set
            // (we don't enable it). Pass through their source so a
            // future opt-in doesn't silently drop content.
            Event::InlineMath(s) => {
                out.push_str("<span class=\"x0k-math\">");
                escape_html(&mut out, &s);
                out.push_str("</span>");
            }
            Event::DisplayMath(s) => {
                out.push_str("<div class=\"x0k-math x0k-math-display\">");
                escape_html(&mut out, &s);
                out.push_str("</div>");
            }
        }
    }

    out
}

/// The slug of each heading in `markdown`, in document order.
fn heading_slugs(markdown: &str, opts: Options) -> Vec<String> {
    let mut slugs = Vec::new();
    let mut text: Option<String> = None;
    for event in Parser::new_ext(markdown, opts) {
        match event {
            Event::Start(Tag::Heading { .. }) => text = Some(String::new()),
            Event::End(TagEnd::Heading(_)) => {
                if let Some(t) = text.take() {
                    slugs.push(crate::transclusion::heading_slug(t.trim()));
                }
            }
            Event::Text(t) | Event::Code(t) => {
                if let Some(acc) = text.as_mut() {
                    acc.push_str(&t);
                }
            }
            _ => {}
        }
    }
    slugs
}

/// The block id a marker gives the block starting at `start`: a marker
/// line (`[block-…]: #block-…`) just before it, or on the block's own
/// first line after a list bullet.
fn block_marker(markdown: &str, start: usize) -> Option<String> {
    let line_start = markdown[..start].rfind('\n').map_or(0, |i| i + 1);
    let own_line = markdown[start..].lines().next().unwrap_or("");
    let before = markdown[..line_start.saturating_sub(1)].rsplit('\n').next().filter(|_| line_start > 0);
    let after_bullet = |line: &str| {
        let t = line.trim_start();
        let t = t.strip_prefix(['-', '*', '+']).map(str::trim_start).unwrap_or(t);
        marker_id(t)
    };
    after_bullet(own_line).or_else(|| before.and_then(after_bullet))
}

/// The id in a marker line `[block-x]: #block-x`.
fn marker_id(line: &str) -> Option<String> {
    let (label, target) = line.trim().strip_prefix('[')?.split_once("]: #")?;
    (label == target.trim() && crate::block_identity::BlockId::parse(label).is_some()).then(|| label.to_owned())
}

/// Give the start tag written at `at` the attribute `id`.
fn give_id(out: &mut String, at: usize, id: &str) {
    let Some(close) = out[at..].find('>') else { return };
    let mut attr = String::from(" id=\"");
    escape_attr(&mut attr, id);
    attr.push('"');
    out.insert_str(at + close, &attr);
}

/// The id of the `index`th `x0k:media` mount point in a rendered body.
pub fn media_mount_id(index: usize) -> String {
    format!("x0k-media-{index}")
}

/// One `x0k:media` mount point in rendered HTML: the element a host
/// embeds the guest on (`HtmlRenderer::embed_element`).
#[derive(Clone, Debug, PartialEq)]
pub struct MediaMount {
    /// The element's `id`.
    pub id: String,
    /// The `x0k:` doc URI (`ref="…"`) or chunk name (`{#name}`) the fence
    /// declares.
    pub media_ref: String,
    /// The declared height in CSS px; 400 when the fence declares none.
    pub height: f64,
    /// The name of the blocks of the document the fence binds its guest to
    /// (`source="…"`).
    pub source: Option<String>,
    /// The data block of that name's body, which the guest receives through
    /// `SetData`; `None` when the document has no data block of that name.
    pub payload: Option<String>,
    /// The params of the parameter panel of that name, a JSON object the
    /// guest receives through `SetParams`; `None` when the document has no
    /// panel of that name.
    pub params: Option<String>,
    /// The HTML the guest holds in its child slot (`data-media-child`), a
    /// document of its own that may hold figures in turn.
    pub child: Option<String>,
    /// The tallest the child slot may be, CSS px (`data-child-height`): a
    /// longer child scrolls inside it. `None` lets the child stand at its
    /// full height.
    pub child_height: Option<f64>,
}

/// The source an `x0k:transclude` fence names. Asks `FenceInfo`, so an
/// illustrative `x0k:!transclude` names nothing.
pub fn transclude_fence(info: &str) -> Option<String> {
    if crate::FenceInfo::parse(info).x0k_type() != Some("transclude") {
        return None;
    }
    crate::transclusion::extract_fence_ref(info)
}

/// A block's summary: what a canvas draws for it from far out, as plain text.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BlockSummary {
    /// `heading`, `paragraph`, `list`, `quote`, `table`, `code`, `figure` or
    /// `transclusion`.
    pub kind: &'static str,
    pub text: String,
}

/// The longest summary, in characters, before it is cut with an ellipsis.
const SUMMARY_CHARS: usize = 80;

/// Each top-level block of `markdown`, summarized without HTML: a heading's
/// text, the first sentence of a paragraph or quote, a list's first items, a
/// table's header, a figure's or transclusion's source, a code block's
/// language.
pub fn block_summaries(markdown: &str) -> Vec<BlockSummary> {
    let opts = Options::ENABLE_TABLES | Options::ENABLE_FOOTNOTES | Options::ENABLE_STRIKETHROUGH
        | Options::ENABLE_TASKLISTS | Options::ENABLE_SMART_PUNCTUATION;
    let mut out = Vec::new();
    let mut depth = 0usize;
    let mut kind = "";
    let mut text = String::new();
    let mut items = Vec::<String>::new();
    for event in Parser::new_ext(markdown, opts) {
        match event {
            Event::Start(tag) => {
                if depth == 0 {
                    text.clear();
                    items.clear();
                    kind = match &tag {
                        Tag::Heading { .. } => "heading",
                        Tag::List(_) => "list",
                        Tag::BlockQuote(_) => "quote",
                        Tag::Table(_) => "table",
                        Tag::CodeBlock(CodeBlockKind::Fenced(info)) if transclude_fence(info).is_some() => {
                            text = transclude_fence(info).unwrap_or_default();
                            "transclusion"
                        }
                        Tag::CodeBlock(CodeBlockKind::Fenced(info)) if media_fence(info).is_some() => {
                            text = media_fence(info).map(|(reference, _)| reference).unwrap_or_default();
                            "figure"
                        }
                        Tag::CodeBlock(CodeBlockKind::Fenced(info)) => {
                            text = info.split_whitespace().next().unwrap_or("").to_owned();
                            "code"
                        }
                        Tag::CodeBlock(_) => "code",
                        _ => "paragraph",
                    };
                }
                if depth == 1 && kind == "list" && matches!(tag, Tag::Item) { items.push(String::new()); }
                depth += 1;
            }
            Event::End(_) => {
                depth = depth.saturating_sub(1);
                if depth == 0 && !kind.is_empty() {
                    let whole = match kind {
                        "list" => items.iter().map(|item| first_sentence(item.trim())).filter(|item| !item.is_empty()).collect::<Vec<_>>().join("; "),
                        "paragraph" | "quote" => first_sentence(text.trim()),
                        _ => text.trim().to_owned(),
                    };
                    out.push(BlockSummary { kind, text: cut(&whole) });
                    kind = "";
                }
            }
            Event::Text(words) | Event::Code(words) if depth > 0 && !matches!(kind, "code" | "figure" | "transclusion") => {
                let target = if kind == "list" { items.last_mut().unwrap_or(&mut text) } else { &mut text };
                target.push_str(&words);
                if kind == "table" { target.push_str(" · "); }
            }
            Event::SoftBreak | Event::HardBreak if depth > 0 => {
                if kind == "list" { if let Some(item) = items.last_mut() { item.push(' '); } } else { text.push(' '); }
            }
            _ => {}
        }
    }
    out
}

/// Up to and including the first full stop that ends a sentence.
fn first_sentence(text: &str) -> String {
    let end = text.char_indices().find(|&(at, c)| matches!(c, '.' | '?' | '!') && text[at + c.len_utf8()..].starts_with(' ')).map(|(at, c)| at + c.len_utf8());
    end.map_or(text, |end| &text[..end]).to_owned()
}

fn cut(text: &str) -> String {
    let text = text.trim_end_matches([' ', '·']).trim();
    if text.chars().count() <= SUMMARY_CHARS { return text.to_owned(); }
    let mut out: String = text.chars().take(SUMMARY_CHARS - 1).collect();
    out.push('…');
    out
}

/// What an `x0k:media` fence declares: its reference and height. Asks
/// `FenceInfo` what the fence declares, so an illustrative `x0k:!media`
/// declares nothing.
pub fn media_fence(info: &str) -> Option<(String, f64)> {
    if crate::FenceInfo::parse(info).x0k_type() != Some("media") {
        return None;
    }
    let attr = |name: &str| {
        let needle = format!("{name}=\"");
        let start = info.find(&needle)? + needle.len();
        let end = info[start..].find('"')?;
        Some(info[start..start + end].to_owned())
    };
    let chunk = || info.split_once("{#")?.1.split_once('}').map(|(c, _)| c.to_owned());
    let media_ref = attr("ref").or_else(chunk).filter(|r| !r.is_empty())?;
    let height = attr("height").and_then(|v| v.parse::<f64>().ok()).unwrap_or(400.0);
    Some((media_ref, height))
}

/// The tallest an `x0k:media` fence lets its guest's child slot be
/// (`child-height="…"`), CSS px; `None` when it sets no limit.
pub fn media_child_height(info: &str) -> Option<f64> {
    let needle = "child-height=\"";
    let start = info.find(needle)? + needle.len();
    let end = info[start..].find('"')?;
    info[start..start + end].parse::<f64>().ok().filter(|height| *height > 0.0)
}

/// The scheme an `x0k:media` reference uses to name a canvas instead of a
/// guest: `ref="canvas:<world>"`. Every reader of a media fence carries the
/// reference through unchanged (the weave's mount points, the structural
/// body's surface lines, the Blitz body's mount points), so only resolution
/// tells the two apart: a guest's reference resolves through its folio's
/// `folio:tangleCrate`, a canvas's names a world the host builds itself.
pub const CANVAS_MEDIA_SCHEME: &str = "canvas:";

/// The world a media reference names when it names a canvas
/// (`canvas:ontology` → `ontology`), or `None` for a guest's reference.
pub fn media_canvas(media_ref: &str) -> Option<&str> {
    media_ref.strip_prefix(CANVAS_MEDIA_SCHEME).filter(|world| !world.is_empty())
}

/// What a mount point binds: the blocks' name, the data block's body and the
/// panel's params.
struct MountBinding<'a> {
    source: Option<&'a str>,
    payload: Option<&'a str>,
    params: Option<&'a str>,
}

fn write_media_mount(out: &mut String, index: usize, media_ref: &str, height: f64, child_height: Option<f64>, binding: MountBinding<'_>) {
    out.push_str("<div class=\"media-embed\" id=\"");
    out.push_str(&media_mount_id(index));
    out.push_str("\" data-media-ref=\"");
    escape_attr(out, media_ref);
    out.push_str(&format!("\" data-height=\"{height}\""));
    if let Some(child_height) = child_height {
        out.push_str(&format!(" data-child-height=\"{child_height}\""));
    }
    for (name, value) in [
        ("data-media-source", binding.source),
        ("data-media-payload", binding.payload),
        ("data-media-params", binding.params),
    ] {
        if let Some(value) = value {
            out.push_str(&format!(" {name}=\""));
            escape_attr(out, value);
            out.push('"');
        }
    }
    out.push_str("><span class=\"media-label\">interactive: ");
    escape_html(out, media_ref);
    out.push_str("</span></div>\n");
}

/// The `x0k:media` mount points in `html`, in document order: the ones
/// [`markdown_to_html`] wrote, and the ones an HTML body states itself. A
/// mount point is a `<div class="media-embed">` with an `id` and a
/// `data-media-ref`. Its data is the payload its tag carries, or the text of
/// the `<pre data-block="…">` its `data-source` names.
pub fn media_mounts(html: &str) -> Vec<MediaMount> {
    let attr = |tag: &str, name: &str| {
        let needle = format!(" {name}=\"");
        let start = tag.find(&needle)? + needle.len();
        let end = tag[start..].find('"')?;
        Some(tag[start..start + end].to_owned())
    };
    html.match_indices("<div ")
        .filter_map(|(at, _)| {
            let tag = &html[at..at + html[at..].find('>')?];
            let class = attr(tag, "class")?;
            if !class.split_whitespace().any(|name| name == "media-embed") {
                return None;
            }
            let source = attr(tag, "data-media-source").or_else(|| attr(tag, "data-source")).map(|v| unescape_attr(&v));
            let payload = attr(tag, "data-media-payload").map(|v| unescape_attr(&v))
                .or_else(|| source.as_deref().and_then(|name| data_block(html, name)));
            Some(MediaMount {
                id: attr(tag, "id")?,
                media_ref: unescape_attr(&attr(tag, "data-media-ref")?),
                height: attr(tag, "data-height").and_then(|v| v.parse().ok()).unwrap_or(400.0),
                source,
                payload,
                params: attr(tag, "data-media-params").map(|v| unescape_attr(&v)),
                child: attr(tag, "data-media-child").map(|v| unescape_attr(&v)),
                child_height: attr(tag, "data-child-height").and_then(|v| v.parse().ok()).filter(|height: &f64| *height > 0.0),
            })
        })
        .collect()
}

/// `html` followed by every child document its figures hold, and theirs in
/// turn: the markup to resolve every figure a page reaches. A figure is not
/// followed into a child it already holds above itself.
pub fn with_child_markup(html: &str) -> String {
    fn follow(html: &str, chain: &mut Vec<String>, out: &mut String) {
        for mount in media_mounts(html) {
            let Some(child) = mount.child else { continue };
            if chain.contains(&mount.media_ref) {
                continue;
            }
            out.push_str(&child);
            chain.push(mount.media_ref);
            follow(&child, chain, out);
            chain.pop();
        }
    }
    let mut out = html.to_owned();
    follow(html, &mut Vec::new(), &mut out);
    out
}

/// The text of the `<pre data-block="name">` an HTML body holds.
fn data_block(html: &str, name: &str) -> Option<String> {
    let mut needle = String::from("data-block=\"");
    escape_attr(&mut needle, name);
    needle.push('"');
    let at = html.find(&needle)?;
    let open = html[..at].rfind("<pre")?;
    let start = open + html[open..].find('>')? + 1;
    let end = start + html[start..].find("</pre>")?;
    Some(unescape_attr(&html[start..end]))
}

fn unescape_attr(s: &str) -> String {
    s.replace("&quot;", "\"").replace("&lt;", "<").replace("&gt;", ">").replace("&amp;", "&")
}

/// Wrap a body fragment as a full HTML document. The substrate's
/// `load(html, base_url)` call accepts either; wrapping makes the
/// emitted markup easier to inspect when debugging theme issues.
///
/// `title` is used for `<title>` so dev-tools / window-title surfaces
/// have something meaningful when they read it.
pub fn wrap_for_substrate(title: &str, body_fragment: &str) -> String {
    let mut out = String::with_capacity(body_fragment.len() + 256);
    out.push_str("<!doctype html><html lang=\"en\"><head><meta charset=\"utf-8\"><title>");
    escape_html(&mut out, title);
    out.push_str("</title></head><body><article class=\"x0k-document\">");
    out.push_str(body_fragment);
    out.push_str("</article></body></html>");
    out
}

fn write_start(out: &mut String, tag: Tag<'_>) {
    match tag {
        Tag::Paragraph => out.push_str("<p class=\"x0k-body\">"),
        Tag::Heading {
            level,
            id,
            classes,
            attrs: _,
        } => {
            // We override class to keep the semantic vocabulary
            // canonical; if the author specified additional classes
            // via `{.foo}` extensions, we append them after our class.
            let lv = heading_level(level);
            let cls = heading_class(level);
            out.push('<');
            out.push_str(lv);
            out.push_str(" class=\"");
            out.push_str(cls);
            for c in &classes {
                out.push(' ');
                escape_attr(out, c);
            }
            out.push('"');
            if let Some(id_str) = id {
                out.push_str(" id=\"");
                escape_attr(out, &id_str);
                out.push('"');
            }
            out.push('>');
        }
        Tag::BlockQuote(_) => out.push_str("<blockquote class=\"x0k-blockquote\">"),
        Tag::CodeBlock(kind) => {
            out.push_str("<pre class=\"x0k-code-block\"><code");
            if let CodeBlockKind::Fenced(lang) = kind {
                if !lang.is_empty() {
                    out.push_str(" class=\"language-");
                    escape_attr(out, &lang);
                    out.push('"');
                }
            }
            out.push('>');
        }
        Tag::List(start) => {
            if let Some(n) = start {
                out.push_str("<ol");
                if n != 1 {
                    out.push_str(&format!(" start=\"{n}\""));
                }
                out.push('>');
            } else {
                out.push_str("<ul>");
            }
        }
        Tag::Item => out.push_str("<li>"),
        Tag::Emphasis => out.push_str("<em>"),
        Tag::Strong => out.push_str("<strong>"),
        Tag::Strikethrough => out.push_str("<del>"),
        Tag::Link {
            link_type: _,
            dest_url,
            title,
            id: _,
        } => {
            out.push_str("<a href=\"");
            escape_attr(out, &dest_url);
            out.push('"');
            if !title.is_empty() {
                out.push_str(" title=\"");
                escape_attr(out, &title);
                out.push('"');
            }
            out.push('>');
        }
        Tag::Image {
            link_type: _,
            dest_url,
            title,
            id: _,
        } => {
            out.push_str("<img src=\"");
            escape_attr(out, &dest_url);
            out.push('"');
            if !title.is_empty() {
                out.push_str(" alt=\"");
                escape_attr(out, &title);
                out.push('"');
            }
            out.push('>');
        }
        Tag::Table(_) => out.push_str("<table class=\"x0k-table\">"),
        Tag::TableHead => out.push_str("<thead><tr>"),
        Tag::TableRow => out.push_str("<tr>"),
        Tag::TableCell => out.push_str("<td>"),
        Tag::FootnoteDefinition(name) => {
            out.push_str("<div class=\"x0k-footnote\" id=\"fn-");
            escape_attr(out, &name);
            out.push_str("\">");
        }
        Tag::HtmlBlock => {}
        Tag::MetadataBlock(_) => {}
        // `DefinitionList*` variants exist in newer pulldown; tolerate
        // them as plain block wrappers when present so the converter
        // doesn't panic on future markdown extensions.
        _ => {}
    }
}

fn write_end(out: &mut String, tag: TagEnd) {
    match tag {
        TagEnd::Paragraph => out.push_str("</p>"),
        TagEnd::Heading(level) => {
            out.push_str("</");
            out.push_str(heading_level(level));
            out.push('>');
        }
        TagEnd::BlockQuote(_) => out.push_str("</blockquote>"),
        TagEnd::CodeBlock => out.push_str("</code></pre>"),
        TagEnd::List(ordered) => {
            if ordered {
                out.push_str("</ol>");
            } else {
                out.push_str("</ul>");
            }
        }
        TagEnd::Item => out.push_str("</li>"),
        TagEnd::Emphasis => out.push_str("</em>"),
        TagEnd::Strong => out.push_str("</strong>"),
        TagEnd::Strikethrough => out.push_str("</del>"),
        TagEnd::Link => out.push_str("</a>"),
        TagEnd::Image => {}
        TagEnd::Table => out.push_str("</table>"),
        TagEnd::TableHead => out.push_str("</tr></thead><tbody>"),
        TagEnd::TableRow => out.push_str("</tr>"),
        TagEnd::TableCell => out.push_str("</td>"),
        TagEnd::FootnoteDefinition => out.push_str("</div>"),
        TagEnd::HtmlBlock => {}
        TagEnd::MetadataBlock(_) => {}
        _ => {}
    }
}

fn heading_level(level: HeadingLevel) -> &'static str {
    match level {
        HeadingLevel::H1 => "h1",
        HeadingLevel::H2 => "h2",
        HeadingLevel::H3 => "h3",
        HeadingLevel::H4 => "h4",
        HeadingLevel::H5 => "h5",
        HeadingLevel::H6 => "h6",
    }
}

fn heading_class(level: HeadingLevel) -> &'static str {
    match level {
        HeadingLevel::H1 => "x0k-display",
        HeadingLevel::H2 => "x0k-heading",
        _ => "x0k-subheading",
    }
}

fn escape_html(out: &mut String, s: &str) {
    for c in s.chars() {
        match c {
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '&' => out.push_str("&amp;"),
            other => out.push(other),
        }
    }
}

fn escape_attr(out: &mut String, s: &str) {
    for c in s.chars() {
        match c {
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '&' => out.push_str("&amp;"),
            '"' => out.push_str("&quot;"),
            other => out.push(other),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_block_has_a_summary_without_html() {
        let markdown = "# Notebook\n\nToday's notes. More of them, *quoted* too.\n\n- one thing. And more\n- two\n\n> Said once. Then again.\n\n```x0k:transclude {ref=\"x0k:wiki/glossary#cell\"}\n```\n\n```x0k:media {ref=\"x0k:implementation/canvas/viz-doc-canvas\"}\n```\n\n```rust\nfn main() {}\n```\n\n| Term | Meaning |\n|---|---|\n| cell | unit |\n";
        let summaries: Vec<(&str, String)> = block_summaries(markdown).into_iter().map(|s| (s.kind, s.text)).collect();
        assert_eq!(summaries, [
            ("heading", "Notebook".to_owned()),
            ("paragraph", "Today\u{2019}s notes.".to_owned()),
            ("list", "one thing.; two".to_owned()),
            ("quote", "Said once.".to_owned()),
            ("transclusion", "x0k:wiki/glossary#cell".to_owned()),
            ("figure", "x0k:implementation/canvas/viz-doc-canvas".to_owned()),
            ("code", "rust".to_owned()),
            ("table", "Term · Meaning · cell · unit".to_owned()),
        ]);
        let long = block_summaries(&"word ".repeat(40));
        assert_eq!(long[0].text.chars().count(), 80);
        assert!(long[0].text.ends_with('…') && !long[0].text.contains('<'));
    }

    #[test]
    fn a_transclude_fence_becomes_the_element_its_host_fills() {
        let html = markdown_to_html("Before.\n\n```x0k:transclude {ref=\"x0k:wiki/quoted#the-part\"}\n```\n\nAfter.\n");
        assert!(html.contains("<div class=\"transclusion\" data-ref=\"x0k:wiki/quoted#the-part\"></div>"), "{html}");
        assert!(!html.contains("language-x0k:transclude") && !html.contains("<pre"), "{html}");
        assert!(html.contains("Before.") && html.contains("After."));
        // An illustrative fence names nothing and stays a code block.
        let shown = markdown_to_html("```x0k:!transclude {ref=\"x0k:wiki/quoted\"}\n```\n");
        assert!(!shown.contains("class=\"transclusion\""), "{shown}");
    }

    #[test]
    fn an_html_body_states_its_own_mount_points() {
        let html = concat!(
            "<p>Before.</p>\n",
            "<div class=\"media-embed\" id=\"reduction\" data-media-ref=\"x0k:implementation/a/viz\" data-source=\"rows\"></div>\n",
            "<details class=\"reveal figure-data\"><pre data-block=\"rows\">{&quot;n&quot;: 1 &lt; 2}</pre></details>\n",
            "<div class=\"media-embed\" data-media-ref=\"x0k:implementation/a/other\"></div>\n",
        );
        let mounts = media_mounts(html);
        assert_eq!(mounts.len(), 1, "a mount point without an id cannot be addressed");
        assert_eq!(mounts[0].id, "reduction");
        assert_eq!(mounts[0].media_ref, "x0k:implementation/a/viz");
        assert_eq!(mounts[0].source.as_deref(), Some("rows"));
        assert_eq!(mounts[0].payload.as_deref(), Some("{\"n\": 1 < 2}"));
    }

    #[test]
    fn child_markup_reaches_every_figure_a_page_holds_and_stops_at_a_figure_inside_itself() {
        let mount = |id: &str, media_ref: &str, child: &str| {
            let mut out = format!("<div class=\"media-embed\" id=\"{id}\" data-media-ref=\"{media_ref}\"");
            if !child.is_empty() {
                out.push_str(" data-media-child=\"");
                escape_attr(&mut out, child);
                out.push('"');
            }
            out.push_str("></div>\n");
            out
        };
        let inner = mount("i", "x0k:a/inner", "");
        let excerpt = format!("<p>Quoted.</p>\n{inner}");
        let page = mount("c", "x0k:a/canvas", &excerpt);
        let mounts = media_mounts(&page);
        assert_eq!(mounts.len(), 1, "the excerpt's figure is not the page's");
        assert_eq!(mounts[0].child.as_deref(), Some(excerpt.as_str()));
        let reached: Vec<String> = media_mounts(&with_child_markup(&page)).into_iter().map(|m| m.media_ref).collect();
        assert_eq!(reached, ["x0k:a/canvas", "x0k:a/inner"]);
        // A canvas whose excerpt holds the canvas again is followed once.
        let looped = mount("c", "x0k:a/canvas", &format!("<p>Again.</p>\n{}", mount("c2", "x0k:a/canvas", &excerpt)));
        let reached: Vec<String> = media_mounts(&with_child_markup(&looped)).into_iter().map(|m| m.media_ref).collect();
        assert_eq!(reached, ["x0k:a/canvas", "x0k:a/canvas"], "{looped}");
    }

    #[test]
    fn a_media_fence_is_a_mount_point_and_an_illustrative_one_stays_code() {
        let html = markdown_to_html(
            "```x0k:media {ref=\"x0k:a/b\" height=\"320\"}\n```\n\n```x0k:!media {ref=\"x0k:a/c\"}\n```\n\n```x0k:media {#scene}\n```\n",
        );
        let mounts = media_mounts(&html);
        assert_eq!(
            mounts,
            vec![
                MediaMount { id: "x0k-media-0".into(), media_ref: "x0k:a/b".into(), height: 320.0, source: None, payload: None, params: None, child: None, child_height: None },
                MediaMount { id: "x0k-media-1".into(), media_ref: "scene".into(), height: 400.0, source: None, payload: None, params: None, child: None, child_height: None },
            ],
            "{html}"
        );
        assert!(html.contains("<pre class=\"x0k-code-block\">"), "the illustrative fence stays a code block: {html}");
    }

    #[test]
    fn a_canvas_fence_names_its_world_and_a_guest_fence_names_none() {
        let info = "x0k:media {ref=\"canvas:ontology\" height=\"320\"}";
        let (media_ref, height) = media_fence(info).expect("a canvas fence is a media fence");
        assert_eq!((media_ref.as_str(), height), ("canvas:ontology", 320.0));
        assert_eq!(media_canvas(&media_ref), Some("ontology"));
        assert_eq!(media_canvas("x0k:implementation/canvas/viz-hysteresis"), None);
        assert_eq!(media_canvas("canvas:"), None, "a canvas names a world");
        // The mount point carries the reference as any other figure's.
        let html = markdown_to_html(&format!("```{info}\n```\n"));
        let mounts = media_mounts(&html);
        assert_eq!(mounts.len(), 1, "{html}");
        assert_eq!(media_canvas(&mounts[0].media_ref), Some("ontology"));
        assert_eq!(mounts[0].height, 320.0);
    }

    #[test]
    fn a_media_fence_carries_the_block_it_binds() {
        let html = markdown_to_html(
            "```x0k:media {ref=\"x0k:a/b\" source=\"series\"}\n```\n\n```json x0k:data {#series}\n[\"<1>\", 2]\n```\n\n```x0k:media {ref=\"x0k:a/c\" source=\"missing\"}\n```\n",
        );
        let mounts = media_mounts(&html);
        assert_eq!(
            (mounts[0].source.as_deref(), mounts[0].payload.as_deref()),
            (Some("series"), Some("[\"<1>\", 2]\n")),
            "{html}"
        );
        assert_eq!((mounts[1].source.as_deref(), mounts[1].payload.as_deref()), (Some("missing"), None), "{html}");
    }

    #[test]
    fn a_media_fence_carries_its_panel_as_params_and_never_as_data() {
        let html = markdown_to_html(
            "```x0k:media {ref=\"x0k:a/b\" source=\"knobs\"}\n```\n\n\
             ```turtle folio:graph {source=\"knobs\"}\nx0k:a\\/b\\/gap a folio:Parameter ;\n    folio:default 1.5 .\n```\n",
        );
        let mounts = media_mounts(&html);
        assert_eq!(
            (mounts[0].source.as_deref(), mounts[0].payload.as_deref(), mounts[0].params.as_deref()),
            (Some("knobs"), None, Some(r#"{"gap":1.5}"#)),
            "{html}"
        );
    }

    #[test]
    fn paragraphs_get_body_class() {
        let html = markdown_to_html("Hello world.");
        assert!(html.contains("<p class=\"x0k-body\">Hello world.</p>"));
    }

    #[test]
    fn h1_gets_display_class() {
        let html = markdown_to_html("# Title");
        assert!(html.contains("<h1 class=\"x0k-display\""));
        assert!(html.contains("Title</h1>"));
    }

    #[test]
    fn h2_gets_heading_class() {
        let html = markdown_to_html("## Section");
        assert!(html.contains("<h2 class=\"x0k-heading\""));
    }

    #[test]
    fn h3_and_below_get_subheading_class() {
        let html = markdown_to_html("### Sub\n\n#### Subsub");
        assert!(html.contains("<h3 class=\"x0k-subheading\""));
        assert!(html.contains("<h4 class=\"x0k-subheading\""));
    }

    #[test]
    fn inline_code_gets_mono_class() {
        let html = markdown_to_html("a `foo` b");
        assert!(html.contains("<code class=\"x0k-mono\">foo</code>"));
    }

    #[test]
    fn fenced_code_preserves_language_class() {
        let html = markdown_to_html("```rust\nfn main() {}\n```");
        assert!(html.contains("<pre class=\"x0k-code-block\">"));
        assert!(html.contains("<code class=\"language-rust\">"));
        assert!(html.contains("fn main() {}"));
    }

    #[test]
    fn unfenced_code_block_has_no_language() {
        let html = markdown_to_html("```\nplain\n```");
        assert!(html.contains("<pre class=\"x0k-code-block\"><code>plain"));
    }

    #[test]
    fn blockquote_gets_class() {
        let html = markdown_to_html("> a quote");
        assert!(html.contains("<blockquote class=\"x0k-blockquote\""));
    }

    #[test]
    fn lists_emit_ul_ol() {
        let html = markdown_to_html("- one\n- two");
        assert!(html.contains("<ul>"));
        assert!(html.contains("<li>"));

        let html = markdown_to_html("1. one\n2. two");
        assert!(html.contains("<ol>"));
    }

    #[test]
    fn link_text_passes_through() {
        let html = markdown_to_html("[label](x0k:wiki/oracle)");
        assert!(html.contains("href=\"x0k:wiki/oracle\""));
        assert!(html.contains(">label</a>"));
    }

    #[test]
    fn html_escapes_special_chars() {
        let html = markdown_to_html("a < b & c > d");
        // Note: smart-punct may normalize quotes but not <>&.
        assert!(html.contains("&lt;"));
        assert!(html.contains("&gt;"));
        assert!(html.contains("&amp;"));
    }

    #[test]
    fn tables_get_table_class() {
        let md = "| a | b |\n|---|---|\n| 1 | 2 |";
        let html = markdown_to_html(md);
        assert!(html.contains("<table class=\"x0k-table\""));
        assert!(html.contains("<thead>"));
        assert!(html.contains("<tbody>"));
    }

    #[test]
    fn wrap_for_substrate_includes_title() {
        let frag = "<p class=\"x0k-body\">hi</p>";
        let doc = wrap_for_substrate("My Doc", frag);
        assert!(doc.contains("<title>My Doc</title>"));
        assert!(doc.contains("class=\"x0k-document\""));
        assert!(doc.contains(frag));
    }

    #[test]
    fn every_heading_carries_its_slug() {
        let html = markdown_to_html("## The `x0k` Decision\n\n### 2. A crossing node\n\n## Notes");
        assert!(html.contains("<h2 class=\"x0k-heading\" id=\"the-x0k-decision\">"), "{html}");
        assert!(html.contains("<h3 class=\"x0k-subheading\" id=\"2-a-crossing-node\">"), "{html}");
        assert!(html.contains("<h2 class=\"x0k-heading\" id=\"notes\">"), "{html}");
    }

    #[test]
    fn a_marked_block_carries_its_id() {
        let html = markdown_to_html(
            "Intro.\n\n[block-7a3f09c2]: #block-7a3f09c2\nA marked paragraph.\n\n- [block-0c41e9d2]: #block-0c41e9d2\n  an item\n- plain item\n",
        );
        assert!(html.contains("<p class=\"x0k-body\" id=\"block-7a3f09c2\">A marked paragraph."), "{html}");
        assert!(html.contains("<li id=\"block-0c41e9d2\">"), "{html}");
        assert_eq!(html.matches(" id=").count(), 2, "nothing else is named: {html}");
    }
}
