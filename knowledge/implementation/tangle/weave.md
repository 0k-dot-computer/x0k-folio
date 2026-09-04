---
x0k:
  format: folio/v1
  id: x0k:implementation/tangle/weave
  type: implementation
  status: draft
  summary: "The other output channel from the same parsed document: self-contained HTML with chunk-headed code, language-tabbed variants, media mount points and parameter panels, consumed directly by the doc browser."
  concerns: [tangle, literate, weave, html, rendering, 0k.computer]
  tangle:
    crate: x0k-tangle
    root: src/weave.rs
  edges:
    cites:
      - x0k:implementation/tangle/protocol
      - x0k:implementation/tangle/parsing
      - x0k:implementation/tangle/chunk
---
# Weaving literate documents into HTML

If tangle is the path from `.md` to `.rs`, *weave* is the path from
`.md` to a browsable page. The two operations consume the same parsed
document — the same chunk graph, the same frontmatter, the same
`<<refs>>` — and produce different output channels. The compiler
reads tangle's output; humans (and the doc-browser, and now you)
read weave's.

Weave doesn't run any pipelines. It renders the document as
self-contained HTML: prose + chunk-headed code blocks +
language-tabbed multi-variant blocks + media embed mount points +
parameter panel data divs. The doc-browser (`ui/0k.computer`) consumes
these pages directly; the operationally-canonical `x0k-tangle weave`
CLI subcommand writes them to disk for static hosting.

## Imports + the output type

The module imports the chunk-shape primitives from `parser` (so it
can call `parse_info_string` on fence info-strings during rendering),
the markdown walker from `pulldown-cmark`, and the standard `Write`
trait for formatted appends to the output string. It also pulls the
renderer-agnostic tokenizer from `x0k-syntax` — the same crate the
native UI uses. Weave is its *web presenter*: it maps each
`TokenKind` to a CSS class (`css_class`) and wraps the matching source
span in a `<span>`, so the emitted HTML carries semantic classes the
stylesheet (and 0k.computer) colour. Native and web share one tokenizer;
only the last inch — colour vs. class — differs.

```rust {#imports}
use crate::parser::{parse_info_string, ParsedDocument};
use anyhow::Result;
use x0k_syntax::{css_class, highlight, HighlightedToken, Language, TokenKind};
use pulldown_cmark::{CodeBlockKind, Event, Options, Parser, Tag, TagEnd};
use std::collections::HashSet;
use std::fmt::Write;

pub struct WeaveOutput {
    pub html: String,
    pub title: Option<String>,
}
```

`WeaveOutput` carries the rendered HTML and an extracted title. The
title is needed because some output paths (e.g. a static-site builder
generating a sidebar listing) need the title independently of the
body.

## The main weave function

`weave_html` is the entry point. It scans the body twice: once just
to extract the title (the first text node after the first H1), then
again to render. The two-pass approach is simple and fast enough —
the document is already in memory.

The render pass walks the pulldown-cmark event stream and dispatches:
code blocks go to `render_code_block` (which knows about chunk
headers, language tabs, media embeds, and the `x0k:params` block);
everything else flows through plain HTML emission.

```rust {#weave-html-fn}
pub fn weave_html(content: &str, doc: &ParsedDocument) -> Result<WeaveOutput> {
    let (_, body) = split_body(content);
    let mut html = String::new();
    let mut title = None;

    html.push_str("<!DOCTYPE html>\n<html lang=\"en\">\n<head>\n");
    html.push_str("<meta charset=\"utf-8\">\n");
    html.push_str("<meta name=\"viewport\" content=\"width=device-width, initial-scale=1\">\n");

    // Extract title from first H1
    let opts = Options::ENABLE_TABLES
        | Options::ENABLE_STRIKETHROUGH
        | Options::ENABLE_MATH
        | Options::ENABLE_FOOTNOTES;
    let title_parser = Parser::new_ext(body, opts);
    for event in title_parser {
        if let Event::Start(Tag::Heading { level, .. }) = &event {
            if *level == pulldown_cmark::HeadingLevel::H1 {
                // Next text event is the title
            }
        }
        if let Event::Text(text) = &event {
            if title.is_none() {
                title = Some(text.to_string());
            }
            break;
        }
    }

    if let Some(ref t) = title {
        writeln!(html, "<title>{}</title>", escape_html(t))?;
    }

    html.push_str("<style>\n");
    html.push_str(STYLESHEET);
    html.push_str("</style>\n");
    html.push_str("</head>\n<body>\n");

    // Frontmatter metadata
    html.push_str("<nav class=\"doc-meta\">\n");
    if let Some(ref crate_name) = doc.tangle_crate {
        writeln!(html, "<span class=\"meta-tag\">crate: {}</span>", escape_html(crate_name))?;
    }
    html.push_str("</nav>\n");

    html.push_str("<article class=\"doc-body\">\n");

    // Render markdown body, intercepting code blocks for chunk rendering
    let parser = Parser::new_ext(body, opts);
    let mut in_code_block = false;
    let mut current_info = String::new();
    let mut current_code = String::new();
    // Track chunk names already rendered as tabbed groups so we skip
    // duplicate fenced blocks for the same multi-language chunk.
    let mut rendered_tab_chunks: HashSet<String> = HashSet::new();
    // Monotonic counter handed to each chunk render so doc-comment hover-card
    // ids (`doc-{chunk_idx}-{run_idx}`) are stable and unique across the page.
    let mut chunk_seq: usize = 0;

    // Heading-buffering state. pulldown-cmark delivers a heading's text in
    // subsequent inline events, so the slug (derived from the plain text) is
    // not known at `Start`. While `in_heading`, inline arms append their HTML
    // into `heading_html` and their plain text into `heading_text`; at the
    // matching `End` we compute the slug, dedup it, and flush
    // `<hN id="{slug}">{heading_html}</hN>`.
    let mut in_heading = false;
    let mut heading_html = String::new();
    let mut heading_text = String::new();
    let mut used_slugs: HashSet<String> = HashSet::new();

    // Footnote state. `ENABLE_FOOTNOTES` emits `FootnoteReference(name)` at the
    // citation site and a `FootnoteDefinition(name)` block at the position the
    // `[^name]: …` definition was authored. We render references inline as a
    // superscript link and divert each definition's HTML into `footnote_defs`,
    // which we flush as a `<section class="footnotes">` at the end of the page —
    // the standard markdown-footnote layout. `footnote_order` assigns each
    // distinct label a stable 1-based number on first reference.
    let mut footnote_defs = String::new();
    let mut in_footnote_def = false;
    let mut footnote_order: Vec<String> = Vec::new();
    // The slugified id of the definition currently being walked, so the
    // closing `</li>` can emit a back-reference arrow to the citation.
    let mut current_footnote_id = String::new();

    for event in parser {
        match event {
            Event::Start(Tag::CodeBlock(kind)) => {
                in_code_block = true;
                current_code.clear();
                current_info = match kind {
                    CodeBlockKind::Fenced(info) => info.to_string(),
                    CodeBlockKind::Indented => String::new(),
                };
            }
            Event::Text(text) if in_code_block => {
                current_code.push_str(&text);
            }
            Event::End(TagEnd::CodeBlock) => {
                in_code_block = false;
                render_code_block(
                    &mut html,
                    &current_info,
                    &current_code,
                    doc,
                    &mut rendered_tab_chunks,
                    &mut chunk_seq,
                )?;
            }
            Event::Start(Tag::Heading { .. }) => {
                // Begin buffering: inline events until the matching End route
                // into the heading buffers instead of `html`.
                in_heading = true;
                heading_html.clear();
                heading_text.clear();
            }
            Event::End(TagEnd::Heading(level)) => {
                in_heading = false;
                let tag = heading_tag(level);
                let slug = unique_slug(&slugify(&heading_text), &mut used_slugs);
                writeln!(html, "<{} id=\"{}\">{}</{}>", tag, slug, heading_html, tag)?;
            }
            Event::Start(Tag::Paragraph) => {
                let sink = if in_footnote_def { &mut footnote_defs } else { &mut html };
                sink.push_str("<p>");
            }
            Event::End(TagEnd::Paragraph) => {
                let sink = if in_footnote_def { &mut footnote_defs } else { &mut html };
                sink.push_str("</p>\n");
            }
            Event::Start(Tag::List(None)) => {
                html.push_str("<ul>\n");
            }
            Event::End(TagEnd::List(false)) => {
                html.push_str("</ul>\n");
            }
            Event::Start(Tag::List(Some(_))) => {
                html.push_str("<ol>\n");
            }
            Event::End(TagEnd::List(true)) => {
                html.push_str("</ol>\n");
            }
            Event::Start(Tag::Item) => {
                html.push_str("<li>");
            }
            Event::End(TagEnd::Item) => {
                html.push_str("</li>\n");
            }
            Event::Start(Tag::Strong) => {
                let sink = if in_footnote_def {
                    &mut footnote_defs
                } else if in_heading {
                    &mut heading_html
                } else {
                    &mut html
                };
                sink.push_str("<strong>");
            }
            Event::End(TagEnd::Strong) => {
                let sink = if in_footnote_def {
                    &mut footnote_defs
                } else if in_heading {
                    &mut heading_html
                } else {
                    &mut html
                };
                sink.push_str("</strong>");
            }
            Event::Start(Tag::Emphasis) => {
                let sink = if in_footnote_def {
                    &mut footnote_defs
                } else if in_heading {
                    &mut heading_html
                } else {
                    &mut html
                };
                sink.push_str("<em>");
            }
            Event::End(TagEnd::Emphasis) => {
                let sink = if in_footnote_def {
                    &mut footnote_defs
                } else if in_heading {
                    &mut heading_html
                } else {
                    &mut html
                };
                sink.push_str("</em>");
            }
            Event::Start(Tag::Link { dest_url, .. }) => {
                let sink = if in_footnote_def {
                    &mut footnote_defs
                } else if in_heading {
                    &mut heading_html
                } else {
                    &mut html
                };
                write!(sink, "<a href=\"{}\">", escape_html(&dest_url))?;
            }
            Event::End(TagEnd::Link) => {
                let sink = if in_footnote_def {
                    &mut footnote_defs
                } else if in_heading {
                    &mut heading_html
                } else {
                    &mut html
                };
                sink.push_str("</a>");
            }
            Event::Code(text) => {
                // Inline code: HTML into the active sink, plain text into the
                // slug accumulator when buffering a heading.
                if in_heading {
                    write!(heading_html, "<code>{}</code>", escape_html(&text))?;
                    heading_text.push_str(&text);
                } else if in_footnote_def {
                    write!(footnote_defs, "<code>{}</code>", escape_html(&text))?;
                } else {
                    write!(html, "<code>{}</code>", escape_html(&text))?;
                }
            }
            Event::Text(text) if !in_code_block => {
                if in_heading {
                    heading_html.push_str(&escape_html(&text));
                    heading_text.push_str(&text);
                } else if in_footnote_def {
                    footnote_defs.push_str(&escape_html(&text));
                } else {
                    html.push_str(&escape_html(&text));
                }
            }
            // `$…$` / `$$…$$` (ENABLE_MATH) → MathML. `render_math` returns
            // verbatim markup; route through the same heading-aware sink the
            // inline arms use so math in a heading is kept (the slug ignores it).
            Event::InlineMath(tex) => {
                let sink = if in_footnote_def {
                    &mut footnote_defs
                } else if in_heading {
                    &mut heading_html
                } else {
                    &mut html
                };
                sink.push_str(&render_math(&tex, false));
            }
            Event::DisplayMath(tex) => {
                let sink = if in_footnote_def {
                    &mut footnote_defs
                } else if in_heading {
                    &mut heading_html
                } else {
                    &mut html
                };
                sink.push_str(&render_math(&tex, true));
            }
            Event::SoftBreak => {
                let sink = if in_footnote_def {
                    &mut footnote_defs
                } else if in_heading {
                    &mut heading_html
                } else {
                    &mut html
                };
                sink.push('\n');
            }
            Event::HardBreak => {
                html.push_str("<br>\n");
            }
            Event::Start(Tag::Table(_)) => {
                html.push_str("<table>\n");
            }
            Event::End(TagEnd::Table) => {
                html.push_str("</table>\n");
            }
            Event::Start(Tag::TableHead) => {
                html.push_str("<thead><tr>\n");
            }
            Event::End(TagEnd::TableHead) => {
                html.push_str("</tr></thead>\n");
            }
            Event::Start(Tag::TableRow) => {
                html.push_str("<tr>\n");
            }
            Event::End(TagEnd::TableRow) => {
                html.push_str("</tr>\n");
            }
            Event::Start(Tag::TableCell) => {
                html.push_str("<td>");
            }
            Event::End(TagEnd::TableCell) => {
                html.push_str("</td>\n");
            }
            Event::Start(Tag::BlockQuote(_)) => {
                html.push_str("<blockquote>");
            }
            Event::End(TagEnd::BlockQuote(_)) => {
                html.push_str("</blockquote>\n");
            }
            Event::Rule => {
                html.push_str("<hr>\n");
            }
            // Raw HTML blocks/inline (e.g. an `<iframe>` embedding a live
            // example) pass through verbatim. The weaver's output is trusted
            // literate-doc source, so block-level and inline HTML are emitted
            // as authored rather than escaped. Routed into the heading buffer
            // when buffering a heading so inline HTML inside a heading is kept.
            Event::Html(raw) | Event::InlineHtml(raw) => {
                let sink = if in_footnote_def {
                    &mut footnote_defs
                } else if in_heading {
                    &mut heading_html
                } else {
                    &mut html
                };
                sink.push_str(&raw);
            }
            // A `[^label]` citation. Assign the label a stable 1-based number
            // (in first-reference order) and emit a superscript backlink into
            // the active sink. The matching `<li id="fn-{id}">` is produced
            // when the definition block is walked, below.
            Event::FootnoteReference(label) => {
                let n = footnote_number(&label, &mut footnote_order);
                let id = slugify(&label);
                let sink = if in_heading { &mut heading_html } else { &mut html };
                write!(
                    sink,
                    "<sup class=\"footnote-ref\" id=\"fnref-{id}\"><a href=\"#fn-{id}\">{n}</a></sup>"
                )?;
            }
            // `[^label]: …` definition body. Its inline content (paragraph,
            // text, emphasis, links) routes into `footnote_defs` while
            // `in_footnote_def` is set; here we open/close the surrounding
            // `<li>` with a back-reference arrow to the citation site.
            Event::Start(Tag::FootnoteDefinition(label)) => {
                in_footnote_def = true;
                current_footnote_id = slugify(&label);
                let n = footnote_number(&label, &mut footnote_order);
                write!(footnote_defs, "<li id=\"fn-{current_footnote_id}\" value=\"{n}\">")?;
            }
            Event::End(TagEnd::FootnoteDefinition) => {
                in_footnote_def = false;
                writeln!(
                    footnote_defs,
                    " <a class=\"footnote-backref\" href=\"#fnref-{current_footnote_id}\">\u{21a9}</a></li>"
                )?;
            }
            _ => {}
        }
    }

    html.push_str("</article>\n");

    // Collected footnote definitions render as an ordered list in a trailing
    // section — the standard markdown-footnote layout.
    if !footnote_defs.is_empty() {
        html.push_str("<section class=\"footnotes\">\n<hr>\n<ol>\n");
        html.push_str(&footnote_defs);
        html.push_str("</ol>\n</section>\n");
    }

    html.push_str("</body>\n</html>\n");

    Ok(WeaveOutput { html, title })
}
```

The renderer is intentionally CommonMark-strict — no auto-linking,
no smartypants. The page is a faithful projection of the source
markdown, plus the chunk-specific extensions.

## Rendering one code block

`render_code_block` is the dispatch for fenced blocks. The info-string
tells us what kind of block this is:

- `yaml x0k:params` / `x0k:params` → data div for the parameter panel
- contains `x0k:media` → an embed mount point (the doc-browser
  resolves the ref to a compiled WASM artifact at render time)
- has `#name` and matches a multi-language chunk → tabbed group
- has `#name`, single-language → chunk-headed `<pre><code>` block
- anonymous → plain `<pre><code>` (no header, no chunk affordances)

```rust {#render-code-block-fn}
fn render_code_block(
    html: &mut String,
    info: &str,
    code: &str,
    doc: &ParsedDocument,
    rendered_tab_chunks: &mut HashSet<String>,
    chunk_seq: &mut usize,
) -> Result<()> {
    let attrs = parse_info_string(info);

    // x0k:params block — render as a param panel data element
    if info.trim_start().starts_with("yaml x0k:params") || info.trim_start().starts_with("x0k:params") {
        render_param_panel(html, code)?;
        return Ok(());
    }

    // x0k:media reference — render as embed mount point
    if info.contains("x0k:media") {
        if let Some(ref name) = attrs.name {
            writeln!(
                html,
                "<div class=\"media-embed\" data-chunk=\"{}\" data-ref=\"{}\">\
                 <span class=\"media-label\">interactive: {}</span></div>",
                escape_html(name),
                attrs.from.as_ref().map(|p| p.display().to_string()).unwrap_or_default(),
                escape_html(name),
            )?;
            return Ok(());
        }
        // media ref= style
        let ref_attr = extract_ref_from_info(info);
        if let Some(ref_uri) = ref_attr {
            writeln!(
                html,
                "<div class=\"media-embed\" data-media-ref=\"{}\">\
                 <span class=\"media-label\">visualization: {}</span></div>",
                escape_html(&ref_uri),
                escape_html(&ref_uri),
            )?;
            return Ok(());
        }
    }

    // Named chunk — render with header
    if let Some(ref name) = attrs.name {
        // Check for multi-language variants
        let variants = doc.chunk_variants(name);
        let is_multi_lang = variants.is_some_and(|v| v.len() > 1);

        if is_multi_lang {
            // If we already rendered this chunk as a tabbed group, skip.
            if rendered_tab_chunks.contains(name.as_str()) {
                return Ok(());
            }
            rendered_tab_chunks.insert(name.clone());

            let variants = variants.unwrap();
            render_tabbed_chunk(html, name, variants, doc, chunk_seq)?;
        } else {
            // Single-language chunk — render as before
            let kind = if attrs.from.is_some() {
                "from"
            } else {
                "owned"
            };
            let lang = attrs.lang.as_deref().unwrap_or("");

            // Render the body first so a module-level (`//!`) docstring can hand
            // back a `data-doc` id to stamp on the chunk-name element in the
            // header. Doc-comment hover-cards are appended after `</pre>`.
            let language = Language::from_str(lang);
            let chunk_idx = *chunk_seq;
            *chunk_seq += 1;
            let rendered = render_code_with_refs(code, doc, language, chunk_idx)?;

            html.push_str("<div class=\"chunk\">\n");
            write!(
                html,
                "<div class=\"chunk-header\" id=\"chunk-{}\">",
                escape_html(name)
            )?;
            // A header-anchored (`//!`) docstring stamps `data-doc` on the
            // chunk-name so the reader can surface the module card from the name.
            match &rendered.header_doc_id {
                Some(id) => write!(
                    html,
                    "<span class=\"chunk-name doc-symbol\" data-doc=\"{}\">{}</span>",
                    escape_html(id),
                    escape_html(name),
                )?,
                None => {
                    write!(html, "<span class=\"chunk-name\">{}</span>", escape_html(name))?
                }
            }
            if !lang.is_empty() {
                write!(html, " <span class=\"chunk-lang\">{}</span>", escape_html(lang))?;
            }
            write!(html, " <span class=\"chunk-kind\">{}</span>", kind)?;

            if let Some(ref from) = attrs.from {
                write!(
                    html,
                    " <span class=\"chunk-from\">{}</span>",
                    escape_html(&from.display().to_string())
                )?;
            }
            if let Some(ref sym) = attrs.symbol {
                write!(html, " <span class=\"chunk-symbol\">{}</span>", escape_html(sym))?;
            }
            html.push_str("</div>\n");

            write!(html, "<pre><code class=\"language-{}\">", escape_html(lang))?;
            html.push_str(&rendered.body);
            html.push_str("</code></pre>\n");
            // Hover-card payloads live right after `</pre>`, never inside it.
            html.push_str(&rendered.cards);
            html.push_str("</div>\n");
        }
    } else {
        // Anonymous code block — highlight, but no chunk-ref linking
        let lang = attrs.lang.as_deref().unwrap_or("");
        let tokens = Language::from_str(lang)
            .and_then(|l| highlight(code, l))
            .unwrap_or_default();
        write!(html, "<pre><code class=\"language-{}\">", escape_html(lang))?;
        if tokens.is_empty() {
            html.push_str(&escape_html(code));
        } else {
            emit_highlighted_range(html, code, 0, code.len(), &tokens)?;
        }
        html.push_str("</code></pre>\n");
    }

    Ok(())
}
```

The `rendered_tab_chunks` deduplication matters: when a chunk has two
language variants, pulldown-cmark gives us two `End(CodeBlock)`
events — one per fence. We render the tabbed group once and skip the
second event.

## Tabbed multi-language chunks

When a chunk has variants in TypeScript and Rust (or any other pair),
weave renders one tabbed widget: chunk header, tab strip, one panel
per language. The first language tab is active on page load.

The onclick handler is inline JavaScript rather than a separately
loaded script because the weave output is meant to be self-contained
HTML — a single file you can drop on any host. The closure is mildly
ugly but it works in every browser without any framework.

```rust {#render-tabbed-chunk-fn}
fn render_tabbed_chunk(
    html: &mut String,
    name: &str,
    variants: &[crate::chunk::Chunk],
    doc: &ParsedDocument,
    chunk_seq: &mut usize,
) -> Result<()> {
    // Render the panels first (into a buffer) so a module-level (`//!`)
    // docstring in any variant can hand back a `data-doc` id for the header.
    let mut panels = String::new();
    let mut header_doc_id: Option<String> = None;
    for (i, variant) in variants.iter().enumerate() {
        let lang = variant.lang.as_deref().unwrap_or("text");
        let active = if i == 0 { " active" } else { "" };
        let body = variant.combined_body();

        write!(
            panels,
            "<div class=\"chunk-tab-panel{}\" data-lang=\"{}\">",
            active,
            escape_html(lang),
        )?;
        let language = Language::from_str(lang);
        let chunk_idx = *chunk_seq;
        *chunk_seq += 1;
        let rendered = render_code_with_refs(&body, doc, language, chunk_idx)?;
        if header_doc_id.is_none() {
            header_doc_id = rendered.header_doc_id;
        }
        write!(panels, "<pre><code class=\"language-{}\">", escape_html(lang))?;
        panels.push_str(&rendered.body);
        panels.push_str("</code></pre>\n");
        // Hover-card payloads live right after `</pre>`, never inside it.
        panels.push_str(&rendered.cards);
        panels.push_str("</div>\n");
    }

    html.push_str("<div class=\"chunk\">\n");

    // Chunk header with name (no per-lang badge — tabs replace it)
    write!(
        html,
        "<div class=\"chunk-header\" id=\"chunk-{}\">",
        escape_html(name)
    )?;
    match &header_doc_id {
        Some(id) => write!(
            html,
            "<span class=\"chunk-name doc-symbol\" data-doc=\"{}\">{}</span>",
            escape_html(id),
            escape_html(name),
        )?,
        None => write!(
            html,
            "<span class=\"chunk-name\">{}</span>",
            escape_html(name)
        )?,
    }

    // Show kind from first variant
    let kind = if variants.first().is_some_and(|v| v.from.is_some()) {
        "from"
    } else {
        "owned"
    };
    write!(html, " <span class=\"chunk-kind\">{}</span>", kind)?;
    html.push_str("</div>\n");

    // Tab strip
    html.push_str("<div class=\"chunk-tabs\">\n");
    for (i, variant) in variants.iter().enumerate() {
        let lang = variant.lang.as_deref().unwrap_or("text");
        let active = if i == 0 { " active" } else { "" };
        let display_lang = capitalize_lang(lang);
        writeln!(
            html,
            "<button class=\"chunk-tab{}\" data-lang=\"{}\" \
             onclick=\"(function(btn){{ \
               var chunk=btn.closest('.chunk'); \
               chunk.querySelectorAll('.chunk-tab').forEach(function(t){{t.classList.remove('active')}}); \
               chunk.querySelectorAll('.chunk-tab-panel').forEach(function(p){{p.classList.remove('active')}}); \
               btn.classList.add('active'); \
               var panel=chunk.querySelector('.chunk-tab-panel[data-lang=\\x27'+btn.dataset.lang+'\\x27]'); \
               if(panel)panel.classList.add('active'); \
             }})(this)\">{}</button>",
            active,
            escape_html(lang),
            escape_html(&display_lang),
        )?;
    }
    html.push_str("</div>\n");

    // Tab panels (rendered above).
    html.push_str(&panels);

    html.push_str("</div>\n");
    Ok(())
}
```

```rust {#capitalize-lang-fn}
/// Capitalize a language identifier for display in tabs.
fn capitalize_lang(lang: &str) -> String {
    match lang {
        "rust" => "Rust".to_string(),
        "typescript" | "ts" => "TypeScript".to_string(),
        "javascript" | "js" => "JavaScript".to_string(),
        "python" | "py" => "Python".to_string(),
        "html" => "HTML".to_string(),
        "css" => "CSS".to_string(),
        "sql" => "SQL".to_string(),
        "toml" => "TOML".to_string(),
        "yaml" | "yml" => "YAML".to_string(),
        "json" => "JSON".to_string(),
        "glsl" | "wgsl" => lang.to_uppercase(),
        other => {
            let mut c = other.chars();
            match c.next() {
                None => String::new(),
                Some(first) => {
                    let mut s = first.to_uppercase().to_string();
                    s.extend(c);
                    s
                }
            }
        }
    }
}
```

## Rendering code with chunk-ref links

Inside a chunk body, `<<ref>>` lines render as clickable anchors that
navigate to the corresponding `#chunk-<ref>` ID elsewhere on the
page. Every other line is syntax-highlighted: the chunk is tokenized
once via `x0k-syntax`, and each line's source span is emitted with
per-token `<span class="tok-...">` wrappers (falling back to plain
escaped text when the language is unknown or unsupported). Ref lines
are never highlighted — they are composition directives, not source.
An escaped ref line, `<<!ref>>` ([`resolution.md`](resolution.md)),
renders as the same literal text the tangler emits — `<<ref>>` under
its indent — with no anchor: the page shows what the output will
hold, and there is nothing to navigate to.

```rust {#render-code-with-refs-fn}
/// A maximal sequence of consecutive `Comment` tokens separated only by
/// whitespace, treated as one *logical comment run* for presentation. A
/// `// foo` line and a contiguous `// bar` line below it form one run even
/// though tree-sitter emits one `line_comment` token per line; a `/* */`
/// block is already one token but still a one-token run. The reader wraps
/// each run in a single element so per-kind styling and folding act on the
/// whole comment, not on individual lines.
struct CommentRun {
    /// Byte offset of the first comment token (the `//` or `/*`).
    start: usize,
    /// Byte offset of the end of the last comment token.
    end: usize,
    /// Classification of the run's combined text: doc / banner / tag / aside.
    kind: &'static str,
    /// Number of source lines the run covers (`\n` count within + 1).
    lines: usize,
    /// For `tag` runs, the canonical attention word (TODO, SAFETY, …) emitted
    /// as `data-tag` so the reader can colour-code the chip by severity.
    tag: Option<&'static str>,
}

impl CommentRun {
    /// Foldable runs are long prose-ish *aside* comments — three lines or more.
    /// Doc runs are no longer folded inline at all: they are lifted out of the
    /// `<pre>` entirely and reattached to the symbol they describe as a
    /// hover-card, so there is nothing left in the code to fold. Banners and
    /// tags are short by nature and never fold.
    fn foldable(&self) -> bool {
        self.lines >= 3 && matches!(self.kind, "aside")
    }
}

/// Classify a logical comment run from its combined source text. The text
/// still carries its comment markers (`//`, `///`, `/* */`, …); we inspect
/// the first non-whitespace content to decide.
///
/// - `doc`: the first non-whitespace begins a doc-comment marker
///   (`///`, `//!`, `/**`, `/*!`).
/// - `banner`: after stripping the line marker, the content is dominated by a
///   divider run — three or more consecutive box-drawing / rule characters.
/// - `tag`: the first word after the marker (case-insensitive, optional
///   trailing `:` or `(`) is a recognised attention tag (TODO, FIXME, …).
/// - `aside`: everything else — the common inline-explanation case.
fn classify_comment(text: &str) -> &'static str {
    let trimmed = text.trim_start();

    // Doc comments win regardless of content.
    if trimmed.starts_with("///")
        || trimmed.starts_with("//!")
        || trimmed.starts_with("/**")
        || trimmed.starts_with("/*!")
    {
        return "doc";
    }

    // Strip the leading comment marker so banner/tag detection sees content.
    let content = strip_comment_marker(trimmed);
    let content = content.trim_start();

    // Banner: a divider run anywhere in the content (after the marker), e.g.
    // `── State ──`, `===== Foo =====`, `-------`. Dedicated rule glyphs
    // (box-drawing / bullet) read as dividers at 2+ since they essentially
    // never appear in code; the ASCII overloads (`- = * # ~`) need 3+ to avoid
    // matching `--` (decrement), `==`, `*`, etc. in real source.
    const RULE_GLYPHS: &[char] = &['─', '—', '═', '━', '•'];
    const ASCII_DIVIDERS: &[char] = &['-', '=', '*', '#', '~'];
    let mut glyph_run = 0usize;
    let mut ascii_run = 0usize;
    for ch in content.chars() {
        if RULE_GLYPHS.contains(&ch) {
            glyph_run += 1;
            ascii_run = 0;
            if glyph_run >= 2 {
                return "banner";
            }
        } else if ASCII_DIVIDERS.contains(&ch) {
            ascii_run += 1;
            glyph_run = 0;
            if ascii_run >= 3 {
                return "banner";
            }
        } else {
            glyph_run = 0;
            ascii_run = 0;
        }
    }

    // Tag: first word (up to a trailing `:` / `(` / whitespace) is a known
    // attention tag, case-insensitive.
    let first_word: String = content
        .chars()
        .take_while(|c| !c.is_whitespace() && *c != ':' && *c != '(')
        .collect();
    const TAGS: &[&str] = &[
        "TODO", "FIXME", "HACK", "XXX", "BUG", "NOTE", "SAFETY", "PERF", "WARNING",
        "OPTIMIZE",
    ];
    if !first_word.is_empty()
        && TAGS.iter().any(|t| t.eq_ignore_ascii_case(&first_word))
    {
        return "tag";
    }

    "aside"
}

/// The canonical attention tag a comment names (TODO, SAFETY, …), or `None`.
/// Emitted as `data-tag` on a `tag` run so the reader can colour the chip by
/// severity. Mirrors the first-word matching in [`classify_comment`].
fn comment_tag_word(text: &str) -> Option<&'static str> {
    let content = strip_comment_marker(text.trim_start());
    let content = content.trim_start();
    let first_word: String = content
        .chars()
        .take_while(|c| !c.is_whitespace() && *c != ':' && *c != '(')
        .collect();
    if first_word.is_empty() {
        return None;
    }
    const TAGS: &[&str] = &[
        "TODO", "FIXME", "HACK", "XXX", "BUG", "NOTE", "SAFETY", "PERF", "WARNING",
        "OPTIMIZE",
    ];
    TAGS.iter().copied().find(|t| t.eq_ignore_ascii_case(&first_word))
}

/// Strip the leading line/block comment marker from a comment's text so the
/// remaining content can be classified. Handles `///`, `//!`, `//`, `/**`,
/// `/*!`, `/*`. Leaves the content (including any closing `*/`) intact.
fn strip_comment_marker(s: &str) -> &str {
    for marker in ["///", "//!", "//", "/**", "/*!", "/*"] {
        if let Some(rest) = s.strip_prefix(marker) {
            return rest;
        }
    }
    s
}

/// Walk the highlight tokens and coalesce consecutive `Comment` tokens that
/// are separated only by whitespace into logical comment runs. Each run
/// records its byte span (start of the first comment token to end of the
/// last), its classification, and the number of source lines it covers.
fn compute_comment_runs(code: &str, tokens: &[HighlightedToken]) -> Vec<CommentRun> {
    let mut runs: Vec<CommentRun> = Vec::new();
    let mut current: Option<(usize, usize)> = None; // (start, end) byte span

    let flush = |runs: &mut Vec<CommentRun>, span: (usize, usize)| {
        let (start, raw_end) = span;
        // Line-comment tokens include their trailing `\n`. Trim a single
        // trailing newline so `end` points at the last *visible* comment byte:
        // the wrapper then closes right after the last comment character, and
        // the separating newline stays outside it (it precedes the next code
        // line, not the comment). Inner newlines stay inside the span.
        let mut end = raw_end;
        if code[start..end].ends_with('\n') {
            end -= 1;
            if code[start..end].ends_with('\r') {
                end -= 1;
            }
        }
        let text = &code[start..end];
        let lines = text.matches('\n').count() + 1;
        let kind = classify_comment(text);
        let tag = if kind == "tag" { comment_tag_word(text) } else { None };
        runs.push(CommentRun {
            start,
            end,
            kind,
            lines,
            tag,
        });
    };

    for tok in tokens {
        // Skip tokens that fall *inside* the current run's byte span. The
        // rust grammar emits nested children — e.g. the third `/` of a `///`
        // doc comment surfaces as an Operator at an offset within the
        // enclosing `line_comment` token. Those are decoration on a comment we
        // already opened, not a break in the run.
        if let Some((start, prev_end)) = current {
            if tok.range.start < prev_end {
                // Extend the span if this nested token reaches past the current
                // end (defensive — comment tokens normally enclose their
                // children), then move on without breaking the run.
                if tok.range.end > prev_end && matches!(tok.kind, TokenKind::Comment) {
                    current = Some((start, tok.range.end));
                }
                continue;
            }
        }

        if matches!(tok.kind, TokenKind::Comment) {
            match current {
                Some((start, prev_end)) => {
                    // Continue the run only if the gap to the previous comment
                    // token is whitespace-only; otherwise close it and start
                    // fresh.
                    if code[prev_end..tok.range.start].trim().is_empty() {
                        current = Some((start, tok.range.end));
                    } else {
                        flush(&mut runs, current.take().unwrap());
                        current = Some((tok.range.start, tok.range.end));
                    }
                }
                None => current = Some((tok.range.start, tok.range.end)),
            }
        } else if let Some(span) = current {
            // A non-whitespace, non-comment token at or past the run's end
            // breaks it. (Whitespace is never tokenized, so any token reaching
            // here is real content.)
            flush(&mut runs, span);
            current = None;
        }
    }
    if let Some(span) = current {
        flush(&mut runs, span);
    }
    runs
}

/// A `doc` comment run that has been *lifted out* of the code: instead of
/// rendering the `///` lines inline, we attach the docstring to the symbol it
/// describes as an on-demand hover-card. Each anchor records the byte span of
/// the described symbol (so we can wrap it in `<span class="doc-symbol">`), a
/// stable `data-doc` id shared by the symbol and its card, and the card's
/// already-rendered HTML (the docstring run through markdown).
struct DocAnchor {
    /// Byte span of the run in `code`, so the line loop can suppress the
    /// `///` lines instead of emitting them into the `<pre>`.
    run_start: usize,
    run_end: usize,
    /// Where the docstring attaches.
    anchor: AnchorKind,
    /// `doc-{chunk_idx}-{run_idx}` — the id shared by `.doc-symbol[data-doc]`
    /// (or the header) and its `.doc-card[data-doc]` payload.
    id: String,
    /// The docstring rendered as markdown, ready to drop into a `.doc-card`.
    card_html: String,
}

/// How a doc run's hover-card attaches to the page.
enum AnchorKind {
    /// `///` / `/**` outer docs → wrap the described symbol at this byte span in
    /// `<span class="doc-symbol" data-doc>` and lift the `///` lines out.
    Symbol(usize, usize),
    /// `//!` / `/*!` inner docs → the chunk header carries the `data-doc`; the
    /// `///`/`//!` lines are still lifted out of the `<pre>`.
    Header,
    /// No symbol found and not an inner doc → render the `///` lines inline as an
    /// ordinary doc comment run (graceful fallback, nothing lost, no card).
    Inline,
}

/// Strip the comment markers from a doc run's raw text and render the result as
/// markdown. Each line's leading `///` / `//!` / `/**` / `/*!` marker is peeled
/// (plus one optional following space), any trailing `*/` is dropped, the lines
/// are rejoined, and the body is fed through the same pulldown-cmark options the
/// prose path uses. The output is block-level HTML — fine, since the card lives
/// *outside* the `<pre>`.
fn render_doc_markdown(raw: &str) -> String {
    let mut body = String::new();
    for (i, line) in raw.lines().enumerate() {
        if i > 0 {
            body.push('\n');
        }
        let mut l = line.trim_start();
        // Peel a leading doc/comment marker.
        for marker in ["///", "//!", "/**", "/*!", "//", "/*"] {
            if let Some(rest) = l.strip_prefix(marker) {
                l = rest;
                break;
            }
        }
        // Drop a trailing block-comment close.
        let l = l.strip_suffix("*/").unwrap_or(l);
        // Drop a single leading space (the conventional `/// text` gap) without
        // eating intentional indentation beyond it.
        let l = l.strip_prefix(' ').unwrap_or(l);
        body.push_str(l);
    }
    // Render via the same event-walk the prose path uses (the crate is built
    // without pulldown-cmark's `html` feature, so we emit tags ourselves). The
    // card lives outside the `<pre>`, so block-level structure is welcome.
    let opts = Options::ENABLE_TABLES | Options::ENABLE_STRIKETHROUGH | Options::ENABLE_MATH;
    let parser = Parser::new_ext(&body, opts);
    let mut out = String::new();
    for event in parser {
        match event {
            Event::Start(Tag::Paragraph) => out.push_str("<p>"),
            Event::End(TagEnd::Paragraph) => out.push_str("</p>"),
            Event::Start(Tag::Emphasis) => out.push_str("<em>"),
            Event::End(TagEnd::Emphasis) => out.push_str("</em>"),
            Event::Start(Tag::Strong) => out.push_str("<strong>"),
            Event::End(TagEnd::Strong) => out.push_str("</strong>"),
            Event::Start(Tag::List(None)) => out.push_str("<ul>"),
            Event::End(TagEnd::List(false)) => out.push_str("</ul>"),
            Event::Start(Tag::List(Some(_))) => out.push_str("<ol>"),
            Event::End(TagEnd::List(true)) => out.push_str("</ol>"),
            Event::Start(Tag::Item) => out.push_str("<li>"),
            Event::End(TagEnd::Item) => out.push_str("</li>"),
            Event::Start(Tag::Link { dest_url, .. }) => {
                let _ = write!(out, "<a href=\"{}\">", escape_html(&dest_url));
            }
            Event::End(TagEnd::Link) => out.push_str("</a>"),
            Event::Start(Tag::BlockQuote(_)) => out.push_str("<blockquote>"),
            Event::End(TagEnd::BlockQuote(_)) => out.push_str("</blockquote>"),
            Event::Start(Tag::Heading { level, .. }) => {
                let _ = write!(out, "<{}>", heading_tag(level));
            }
            Event::End(TagEnd::Heading(level)) => {
                let _ = write!(out, "</{}>", heading_tag(level));
            }
            Event::Code(text) => {
                let _ = write!(out, "<code>{}</code>", escape_html(&text));
            }
            Event::Text(text) => out.push_str(&escape_html(&text)),
            // Math in a docstring renders in the hover-card too.
            Event::InlineMath(tex) => out.push_str(&render_math(&tex, false)),
            Event::DisplayMath(tex) => out.push_str(&render_math(&tex, true)),
            Event::SoftBreak => out.push(' '),
            Event::HardBreak => out.push_str("<br>"),
            _ => {}
        }
    }
    out
}

/// For each `doc` run, find the symbol it documents and build its hover-card.
///
/// The described symbol is the first `Identifier` / `Type` / `Function` token on
/// the next non-blank, non-comment source line after the run ends (keywords,
/// operators, and punctuation like `pub`/`fn`/`(` are skipped). `//!` / `/*!`
/// inner-doc runs document the enclosing module, not a following item, so they
/// resolve their symbol to `None` and the caller anchors them to the chunk
/// header instead. A doc run that finds no following symbol also yields `None`,
/// and the caller falls back to rendering it inline so nothing is lost.
fn plan_doc_anchors(
    code: &str,
    tokens: &[HighlightedToken],
    runs: &[CommentRun],
    chunk_idx: usize,
) -> Vec<DocAnchor> {
    let mut anchors = Vec::new();
    for (run_idx, run) in runs.iter().enumerate() {
        if run.kind != "doc" {
            continue;
        }
        let raw = &code[run.start..run.end];
        let is_inner = {
            let t = raw.trim_start();
            t.starts_with("//!") || t.starts_with("/*!")
        };
        // Inner docs (`//!`, `/*!`) describe the module/file, never a following
        // item → anchor to the header. Outer docs look for the symbol on the
        // next code line; if none is found, fall back to inline rendering.
        let anchor = if is_inner {
            AnchorKind::Header
        } else {
            match find_described_symbol(code, tokens, run.end) {
                Some((s, e)) => AnchorKind::Symbol(s, e),
                None => AnchorKind::Inline,
            }
        };
        anchors.push(DocAnchor {
            run_start: run.start,
            run_end: run.end,
            anchor,
            id: format!("doc-{}-{}", chunk_idx, run_idx),
            card_html: render_doc_markdown(raw),
        });
    }
    anchors
}

/// Find the byte span of the symbol a doc comment describes: the first
/// `Identifier` / `Type` / `Function` token sitting on the next non-blank,
/// non-comment source line that starts after `after` (the run's end offset).
fn find_described_symbol(
    code: &str,
    tokens: &[HighlightedToken],
    after: usize,
) -> Option<(usize, usize)> {
    // Locate the start of the first non-blank, non-comment line after the run.
    // We walk lines from `after`, skipping blank lines and lines whose first
    // non-whitespace begins a comment marker.
    let mut idx = after;
    // Advance past the remainder of the line the run ended on (to its newline).
    if let Some(nl) = code[idx..].find('\n') {
        idx += nl + 1;
    } else {
        return None;
    }
    let mut target_line: Option<(usize, usize)> = None;
    while idx < code.len() {
        let line_end = code[idx..].find('\n').map(|n| idx + n).unwrap_or(code.len());
        let line = &code[idx..line_end];
        let trimmed = line.trim_start();
        // Skip blank lines, comment lines, AND the decorators/attributes that
        // sit between a docstring and the item it documents — Rust
        // `#[derive(…)]` / `#![…]`, TS/Python `@decorator`. These are the most
        // common reason a doc comment fails to find its symbol: the docstring
        // describes the `fn`/`struct` two lines down, not the attribute. We
        // keep scanning to the real declaration line.
        if trimmed.is_empty()
            || trimmed.starts_with("//")
            || trimmed.starts_with("/*")
            || trimmed.starts_with("#[")
            || trimmed.starts_with("#![")
            || trimmed.starts_with('@')
        {
            idx = line_end + 1;
            continue;
        }
        target_line = Some((idx, line_end));
        break;
    }
    let (ls, le) = target_line?;
    // First Identifier/Type/Function token whose range lies within the line.
    for tok in tokens {
        if tok.range.start < ls || tok.range.end > le {
            continue;
        }
        if matches!(
            tok.kind,
            TokenKind::Identifier | TokenKind::Type | TokenKind::Function
        ) {
            return Some((tok.range.start, tok.range.end));
        }
    }
    None
}

/// The product of rendering one chunk body: the `<pre>` interior, the deferred
/// hover-card payload (flushed *after* `</pre>`), and the optional `data-doc`
/// id a module-level (`//!`) docstring wants stamped on the chunk header.
struct RenderedChunk {
    /// The HTML for the inside of `<pre><code>…</code></pre>`.
    body: String,
    /// `<div class="doc-card" …>` blocks, emitted right after the closing
    /// `</pre>` (never inside it).
    cards: String,
    /// When a `//!` inner-doc run anchored to the header, the id to put on the
    /// header's chunk-name element so it carries the `data-doc` affordance.
    header_doc_id: Option<String>,
}

/// Render a chunk body and its lifted doc-comment hover-cards.
///
/// `chunk_idx` makes the per-run `data-doc` ids stable and unique across the
/// page (`doc-{chunk_idx}-{run_idx}`). Returns the `<pre>` body, the card
/// payload to flush after `</pre>`, and an optional header `data-doc` id for a
/// module-level (`//!`) docstring.
fn render_code_with_refs(
    code: &str,
    doc: &ParsedDocument,
    lang: Option<Language>,
    chunk_idx: usize,
) -> Result<RenderedChunk> {
    let html = &mut String::new();
    // Tokenize the whole chunk once; ranges are byte offsets into `code`.
    let tokens = lang.and_then(|l| highlight(code, l)).unwrap_or_default();
    // Precompute logical comment runs so we can wrap each in one element as
    // the line loop flows through its byte span.
    let runs = compute_comment_runs(code, &tokens);
    // Plan doc-comment hover-cards: which runs lift out, the symbol each
    // describes, and the markdown payload.
    let anchors = plan_doc_anchors(code, &tokens, &runs, chunk_idx);

    // A doc run is *suppressed* (lifted out of the `<pre>`) when it anchors to a
    // symbol or the header. `Inline`-anchored runs are NOT suppressed — their
    // `///` lines stay in the code as an ordinary doc comment run, and they emit
    // no card.
    let suppressed: Vec<(usize, usize)> = anchors
        .iter()
        .filter(|a| !matches!(a.anchor, AnchorKind::Inline))
        .map(|a| (a.run_start, a.run_end))
        .collect();

    // The symbol spans to wrap inline, paired with their card id.
    let symbol_marks: Vec<(usize, usize, &str)> = anchors
        .iter()
        .filter_map(|a| match a.anchor {
            AnchorKind::Symbol(s, e) => Some((s, e, a.id.as_str())),
            _ => None,
        })
        .collect();

    let mut line_start = 0usize;
    for line in code.lines() {
        let line_end = line_start + line.len();
        let trimmed = line.trim_start();
        let first_nonws = line_start + (line.len() - trimmed.len());

        // Suppress whole comment-only lines belonging to a lifted doc run: the
        // `///` text never reaches the `<pre>`, and its trailing newline is
        // dropped with it so the code reads clean (no blank gap).
        if suppressed
            .iter()
            .any(|&(rs, re)| first_nonws >= rs && first_nonws < re)
        {
            line_start = line_end + 1;
            continue;
        }

        // A whole-line `<<ref>>` becomes a navigable anchor (owned-chunk
        // composition). Ref lines are never highlighted — not source. An
        // escaped `<<!ref>>` renders as the literal `<<ref>>` the tangler
        // emits, with no anchor.
        let mut handled = false;
        if let Some(rest) = trimmed.strip_prefix("<<") {
            if let Some(ref_name) = rest.strip_suffix(">>") {
                let ref_name = ref_name.trim();
                let indent = &line[..line.len() - trimmed.len()];
                if let Some(literal) = ref_name.strip_prefix('!') {
                    let literal = literal.trim();
                    if !literal.is_empty() && !literal.contains(' ') {
                        writeln!(
                            html,
                            "{}&lt;&lt;{}&gt;&gt;",
                            escape_html(indent),
                            escape_html(literal),
                        )?;
                        handled = true;
                    }
                } else if !ref_name.is_empty()
                    && !ref_name.contains(' ')
                    && doc.chunks.contains_key(ref_name)
                {
                    writeln!(
                        html,
                        "{}<a class=\"chunk-ref\" href=\"#chunk-{}\">&lt;&lt;{}&gt;&gt;</a>",
                        escape_html(indent),
                        escape_html(ref_name),
                        escape_html(ref_name),
                    )?;
                    handled = true;
                }
            }
        }

        if !handled {
            if tokens.is_empty() {
                writeln!(html, "{}", escape_html(line))?;
            } else {
                emit_line_with_runs(
                    html,
                    code,
                    line_start,
                    line_end,
                    &tokens,
                    &runs,
                    &symbol_marks,
                )?;
                html.push('\n');
            }
        }

        // Advance past this line and the `\n` that `lines()` stripped.
        line_start = line_end + 1;
    }

    // Build the deferred card payload, flushed by the caller after `</pre>`.
    // Inline-fallback runs already emitted their `///` lines into the `<pre>`
    // and have no card.
    let mut cards = String::new();
    let mut header_doc_id = None;
    for a in &anchors {
        if matches!(a.anchor, AnchorKind::Inline) {
            continue;
        }
        // The first header-anchored (`//!`) run wins the header affordance.
        if matches!(a.anchor, AnchorKind::Header) && header_doc_id.is_none() {
            header_doc_id = Some(a.id.clone());
        }
        writeln!(
            cards,
            "<div class=\"doc-card\" data-doc=\"{}\" hidden>{}</div>",
            escape_html(&a.id),
            a.card_html,
        )?;
    }
    Ok(RenderedChunk {
        body: std::mem::take(html),
        cards,
        header_doc_id,
    })
}

/// Emit one source line `[start, end)`, injecting comment-run wrapper open/close
/// tags at the exact byte offsets where a run begins and ends. The wrapper
/// `<span class="comment-run …">` opens precisely at a run's first comment
/// token (so leading indentation before `//` stays *outside* the wrapper) and
/// closes at the last comment token's end. Newlines between a run's lines stay
/// inside the wrapper because `<pre>` preserves them and the run's byte span
/// crosses the line breaks. Whitespace is byte-for-byte preserved: the wrapper
/// tags add no spaces or newlines of their own.
fn emit_line_with_runs(
    html: &mut String,
    code: &str,
    start: usize,
    end: usize,
    tokens: &[HighlightedToken],
    runs: &[CommentRun],
    symbol_marks: &[(usize, usize, &str)],
) -> Result<()> {
    // Collect the boundary points (open at run.start, close at run.end) that
    // fall within this line, in byte order. A run spanning multiple lines only
    // contributes its open on the first line and its close on the last.
    let mut seg_start = start;
    for run in runs {
        // Open boundary inside this line: emit code up to it, then the wrapper.
        // The pre-comment segment is code, so it may hold a doc-symbol mark.
        if run.start >= seg_start && run.start < end {
            emit_range_with_symbols(html, code, seg_start, run.start, tokens, symbol_marks)?;
            let fold = if run.foldable() { " foldable" } else { "" };
            let tag_attr = match run.tag {
                Some(t) => format!(" data-tag=\"{}\"", t),
                None => String::new(),
            };
            write!(
                html,
                "<span class=\"comment-run comment-{}{}\" data-comment-lines=\"{}\"{}>",
                run.kind, fold, run.lines, tag_attr,
            )?;
            seg_start = run.start;
        }
        // Close boundary inside this line: emit the comment bytes up to the end
        // of the run, then close the wrapper. Comment interior never holds a
        // doc-symbol mark (symbols live on the code line after the run).
        if run.end > start && run.end <= end {
            emit_highlighted_range(html, code, seg_start, run.end, tokens)?;
            html.push_str("</span>");
            seg_start = run.end;
        }
    }
    // Remainder of the line after the last boundary (or the whole line if no
    // boundary fell inside it). For an open run that continues past this line,
    // this emits the rest of the line *inside* the still-open wrapper. Code, so
    // it may carry a doc-symbol mark.
    emit_range_with_symbols(html, code, seg_start, end, tokens, symbol_marks)?;
    Ok(())
}

/// Emit `code[start..end)` like [`emit_highlighted_range`], but wrap any
/// described-symbol span that falls inside it in a `<span class="doc-symbol"
/// data-doc="ID">…</span>`. The symbol's own syntax highlighting is preserved:
/// the inner range is itself rendered through `emit_highlighted_range`, so a
/// `Function`/`Type` symbol keeps its `tok-*` colour span and an `Identifier`
/// symbol stays bare — just wrapped by the doc-symbol affordance.
fn emit_range_with_symbols(
    html: &mut String,
    code: &str,
    start: usize,
    end: usize,
    tokens: &[HighlightedToken],
    symbol_marks: &[(usize, usize, &str)],
) -> Result<()> {
    let mut cursor = start;
    // Marks are few; scan in byte order, emitting code before each, then the
    // wrapped symbol.
    let mut marks: Vec<&(usize, usize, &str)> = symbol_marks
        .iter()
        .filter(|&&(s, e, _)| s >= start && e <= end)
        .collect();
    marks.sort_by_key(|&&(s, _, _)| s);
    for m in marks {
        let (s, e, id) = *m;
        if s < cursor {
            continue;
        }
        emit_highlighted_range(html, code, cursor, s, tokens)?;
        write!(html, "<span class=\"doc-symbol\" data-doc=\"{}\">", escape_html(id))?;
        emit_highlighted_range(html, code, s, e, tokens)?;
        html.push_str("</span>");
        cursor = e;
    }
    emit_highlighted_range(html, code, cursor, end, tokens)?;
    Ok(())
}

/// Emit `code[start..end]` as HTML, wrapping each highlighted token in a
/// `<span class="tok-...">` and HTML-escaping everything else. Tokens are
/// byte ranges over the whole `code`; only the portion overlapping
/// `[start, end)` is emitted, so a token spanning several lines is split
/// cleanly per line. `Default`/`Identifier` tokens are emitted unwrapped
/// (they take the base code colour), matching the native presenter.
fn emit_highlighted_range(
    html: &mut String,
    code: &str,
    start: usize,
    end: usize,
    tokens: &[HighlightedToken],
) -> Result<()> {
    let mut cursor = start;
    for tok in tokens {
        // Clamp the token to this range; skip tokens outside or already past
        // the cursor (handles nested/overlapping nodes gracefully).
        if tok.range.end <= cursor || tok.range.start >= end {
            continue;
        }
        let ts = tok.range.start.max(cursor);
        let te = tok.range.end.min(end);
        if ts >= te {
            continue;
        }
        if ts > cursor {
            html.push_str(&escape_html(&code[cursor..ts]));
        }
        if matches!(tok.kind, TokenKind::Default | TokenKind::Identifier) {
            html.push_str(&escape_html(&code[ts..te]));
        } else {
            write!(
                html,
                "<span class=\"{}\">{}</span>",
                css_class(tok.kind),
                escape_html(&code[ts..te]),
            )?;
        }
        cursor = te;
    }
    if cursor < end {
        html.push_str(&escape_html(&code[cursor..end]));
    }
    Ok(())
}
```

The ref-detection rule matches `find_chunk_refs` in
[`chunk.rs`](chunk.md) — same trim-start, same strict bounding by
`<<` / `>>`, same name-validity check.

## The x0k:params block

A `yaml x0k:params` (or just `x0k:params`) fence is a structured
parameter list authored as YAML. Rather than render it as a code
block, weave parses the YAML minimally and emits a `<div
class="param-panel-data" data-params="...">` element with the params
serialized as JSON. The doc-browser's parameter-panel component
reads the JSON and builds the interactive control surface.

```rust {#render-param-panel-fn}
/// Render a `x0k:params` block as a hidden data div that the doc browser
/// reads to build the interactive parameter panel.
///
/// The YAML content is a list of parameter definitions. We parse it minimally
/// and re-serialize as JSON in a `data-params` attribute, so the browser-side
/// component can deserialize without a YAML parser.
fn render_param_panel(html: &mut String, yaml_content: &str) -> Result<()> {
    // Minimal YAML-to-JSON conversion for the flat param list format.
    // Each item has: id, display_name, description, type.kind, type.min/max/step, default.
    let mut params = Vec::new();
    let mut current: Option<ParamEntry> = None;

    for line in yaml_content.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with("- id:") {
            if let Some(p) = current.take() {
                params.push(p);
            }
            let id = trimmed.strip_prefix("- id:").unwrap().trim().to_string();
            current = Some(ParamEntry {
                id,
                ..Default::default()
            });
        } else if let Some(ref mut p) = current {
            if let Some(val) = trimmed.strip_prefix("display_name:") {
                p.display_name = val.trim().to_string();
            } else if let Some(val) = trimmed.strip_prefix("description:") {
                p.description = val.trim().to_string();
            } else if let Some(val) = trimmed.strip_prefix("default:") {
                p.default = val.trim().to_string();
            } else if let Some(val) = trimmed.strip_prefix("type:") {
                // Inline type: { kind: float, min: 0.1, max: 3.0, step: 0.1 }
                let type_str = val.trim();
                parse_param_type(type_str, p);
            }
        }
    }
    if let Some(p) = current.take() {
        params.push(p);
    }

    // Serialize as JSON array
    let mut json = String::from("[");
    for (i, p) in params.iter().enumerate() {
        if i > 0 {
            json.push(',');
        }
        write!(
            json,
            "{{\"id\":\"{}\",\"display_name\":\"{}\",\"description\":\"{}\",\
             \"kind\":\"{}\",\"min\":{},\"max\":{},\"step\":{},\"default\":{}}}",
            escape_json(&p.id),
            escape_json(&p.display_name),
            escape_json(&p.description),
            escape_json(&p.kind),
            p.min,
            p.max,
            p.step,
            p.default,
        )?;
    }
    json.push(']');

    writeln!(
        html,
        "<div class=\"param-panel-data\" data-params='{}'></div>",
        json,
    )?;

    Ok(())
}

#[derive(Default)]
struct ParamEntry {
    id: String,
    display_name: String,
    description: String,
    kind: String,
    min: String,
    max: String,
    step: String,
    default: String,
}

fn parse_param_type(type_str: &str, p: &mut ParamEntry) {
    // Parse { kind: float, min: 0.1, max: 3.0, step: 0.1 }
    let inner = type_str
        .trim_start_matches('{')
        .trim_end_matches('}')
        .trim();
    for part in inner.split(',') {
        let part = part.trim();
        if let Some(val) = part.strip_prefix("kind:") {
            p.kind = val.trim().to_string();
        } else if let Some(val) = part.strip_prefix("min:") {
            p.min = val.trim().to_string();
        } else if let Some(val) = part.strip_prefix("max:") {
            p.max = val.trim().to_string();
        } else if let Some(val) = part.strip_prefix("step:") {
            p.step = val.trim().to_string();
        }
    }
    // Default step to 1 for integer types
    if p.step.is_empty() {
        p.step = "1".to_string();
    }
    if p.min.is_empty() {
        p.min = "0".to_string();
    }
    if p.max.is_empty() {
        p.max = "100".to_string();
    }
}

fn escape_json(s: &str) -> String {
    s.replace('\\', "\\\\")
        .replace('"', "\\\"")
        .replace('\n', "\\n")
}
```

The parser is deliberately small — a real YAML parser would pull in
a dep we don't need for this single flat-list use case. The shape is
constrained enough that line-by-line scanning works.

## Small helpers

`extract_ref_from_info` finds the `ref="..."` value in a fence
info-string (used by the media-embed path). `heading_tag` maps
pulldown-cmark levels to HTML tag names. `escape_html` is the
standard four-replace HTML entity escape. `split_body` is a copy of
the frontmatter splitter — duplicated rather than shared to keep
weave's only-dep on `parser` to the chunk-shape side.

```rust {#small-helpers}
fn extract_ref_from_info(info: &str) -> Option<String> {
    if let Some(pos) = info.find("ref=\"") {
        let after = &info[pos + 5..];
        if let Some(end) = after.find('"') {
            return Some(after[..end].to_string());
        }
    }
    None
}

fn heading_tag(level: pulldown_cmark::HeadingLevel) -> &'static str {
    match level {
        pulldown_cmark::HeadingLevel::H1 => "h1",
        pulldown_cmark::HeadingLevel::H2 => "h2",
        pulldown_cmark::HeadingLevel::H3 => "h3",
        pulldown_cmark::HeadingLevel::H4 => "h4",
        pulldown_cmark::HeadingLevel::H5 => "h5",
        pulldown_cmark::HeadingLevel::H6 => "h6",
    }
}

fn escape_html(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

/// Render a LaTeX math fragment to a MathML `<math>` string. `display` picks
/// block (`$$…$$`) vs inline (`$…$`) form; the browser (the Chromium-only
/// 0k.computer target renders MathML Core natively) lays it out. The returned
/// string is MARKUP — callers push it verbatim, NOT through `escape_html`.
///
/// On a LaTeX error (an unsupported macro, a typo) we never drop the math:
/// fall back to the raw TeX in a `<code class="math-error">` carrying the
/// parser message in `title`, so the author sees what failed instead of a
/// blank.
fn render_math(tex: &str, display: bool) -> String {
    let style = if display {
        latex2mathml::DisplayStyle::Block
    } else {
        latex2mathml::DisplayStyle::Inline
    };
    let fallback = |msg: &str| {
        format!(
            "<code class=\"math-error\" title=\"{}\">{}</code>",
            escape_html(msg),
            escape_html(tex),
        )
    };
    match latex2mathml::latex_to_mathml(tex, style) {
        // latex2mathml returns `Err` only for hard syntax errors; for an
        // UNKNOWN macro it returns `Ok` with an embedded `[PARSE ERROR: …]`
        // `<mtext>` marker. Treat that as a failure too so the author sees the
        // raw TeX rather than a broken-looking equation.
        Ok(mathml) if !mathml.contains("[PARSE ERROR") => mathml,
        Ok(_) => fallback("unsupported LaTeX"),
        Err(err) => fallback(&err.to_string()),
    }
}

/// Turn a heading's plain text into a URL-safe anchor slug: lowercase, each run
/// of non-`[a-z0-9]` characters collapsed to a single `-`, leading/trailing `-`
/// trimmed. An empty result falls back to `section` (so the heading still gets
/// a stable id the outline can scroll to).
fn slugify(text: &str) -> String {
    let mut slug = String::new();
    let mut prev_dash = false;
    for ch in text.chars().flat_map(|c| c.to_lowercase()) {
        if ch.is_ascii_alphanumeric() {
            slug.push(ch);
            prev_dash = false;
        } else if !prev_dash {
            slug.push('-');
            prev_dash = true;
        }
    }
    let trimmed = slug.trim_matches('-');
    if trimmed.is_empty() {
        "section".to_string()
    } else {
        trimmed.to_string()
    }
}

/// Map a footnote label to its 1-based display number, assigned in
/// first-reference order. `order` accumulates labels as they are first seen
/// (whether from a citation or its definition), so a reference and its
/// definition share the same number regardless of which the parser emits
/// first.
fn footnote_number(label: &str, order: &mut Vec<String>) -> usize {
    if let Some(idx) = order.iter().position(|l| l == label) {
        idx + 1
    } else {
        order.push(label.to_string());
        order.len()
    }
}

/// Deduplicate a slug across the document: the first occurrence is bare, the
/// second becomes `slug-2`, the third `slug-3`, and so on. The chosen id is
/// recorded in `used` so later headings collide against it too.
fn unique_slug(base: &str, used: &mut HashSet<String>) -> String {
    if used.insert(base.to_string()) {
        return base.to_string();
    }
    let mut n = 2;
    loop {
        let candidate = format!("{}-{}", base, n);
        if used.insert(candidate.clone()) {
            return candidate;
        }
        n += 1;
    }
}

fn split_body(content: &str) -> (Option<&str>, &str) {
    if !content.starts_with("---") {
        return (None, content);
    }
    let after_first = &content[3..];
    if let Some(end) = after_first.find("\n---") {
        let yaml = &after_first[..end];
        let body_start = 3 + end + 4;
        let body = if body_start < content.len() {
            &content[body_start..]
        } else {
            ""
        };
        let body = body.strip_prefix('\n').unwrap_or(body);
        (Some(yaml), body)
    } else {
        (None, content)
    }
}
```

## The stylesheet

The stylesheet is a single inline CSS block embedded into every weave
output via `<style>`. The aesthetic is intentionally muted: a dark
serif body, monospace code, accent blue for chunk names and links.
The CSS variables make it cheap to fork for a different theme; the
class names are stable so the doc-browser's per-page lensing can
target them.

```rust {#stylesheet}
const STYLESHEET: &str = r#"
:root {
  --bg: #0f0f12;
  --bg-surface: #16161c;
  --bg-chunk: #1a1a24;
  --text: #d4d4dc;
  --text-quiet: #7a7a88;
  --accent: #6090ff;
  --accent-dim: #405880;
  --border: #2a2a36;
  --code-bg: #12121a;
  --chunk-header-bg: #1e1e2a;
  --media-bg: #1a1a28;
  --link: #7ab0ff;
  --tok-keyword: #c08adf;
  --tok-string: #8fce6f;
  --tok-number: #d8a657;
  --tok-comment: #7a7a88;
  --tok-type: #6fb3d2;
  --tok-function: #7ab0ff;
  --tok-property: #82aaff;
  --tok-operator: #c8c8d4;
  --tok-punctuation: #9a9aa8;
  --font-sans: -apple-system, BlinkMacSystemFont, "Segoe UI", Roboto, sans-serif;
  --font-mono: "JetBrains Mono", "Fira Code", "Cascadia Code", monospace;
  --font-serif: "ET Book", "Palatino Linotype", "Book Antiqua", Palatino, serif;
}

* { box-sizing: border-box; margin: 0; padding: 0; }

body {
  background: var(--bg);
  color: var(--text);
  font-family: var(--font-serif);
  font-size: 17px;
  line-height: 1.65;
  max-width: 48em;
  margin: 0 auto;
  padding: 2em 1.5em;
}

h1 {
  font-size: 1.8em;
  font-weight: 600;
  margin: 0.5em 0 0.8em;
  color: #e8e8f0;
  letter-spacing: -0.01em;
}

h2 {
  font-size: 1.3em;
  font-weight: 600;
  margin: 1.8em 0 0.6em;
  color: #c8c8d4;
  border-bottom: 1px solid var(--border);
  padding-bottom: 0.3em;
}

h3 {
  font-size: 1.1em;
  font-weight: 600;
  margin: 1.4em 0 0.4em;
  color: #b8b8c8;
}

p { margin: 0.8em 0; }

a { color: var(--link); text-decoration: none; }
a:hover { text-decoration: underline; }

code {
  font-family: var(--font-mono);
  font-size: 0.88em;
  background: var(--code-bg);
  padding: 0.15em 0.35em;
  border-radius: 3px;
}

pre {
  background: var(--code-bg);
  padding: 1em;
  overflow-x: auto;
  border-radius: 4px;
  margin: 0;
}

pre code {
  background: none;
  padding: 0;
  font-size: 0.85em;
  line-height: 1.5;
}

/* Math (MathML emitted by render_math). MathML uses currentColor, so it
   tracks the text colour automatically. Display math ($$…$$) is a centred
   block that scrolls horizontally rather than overflowing the column; inline
   math ($…$) sits on the text baseline. .math-error is the raw-TeX fallback. */
math { font-size: 1.05em; }
math[display="block"] {
  display: block;
  margin: 0.9em 0;
  text-align: center;
  overflow-x: auto;
  overflow-y: hidden;
}
.math-error {
  color: #c0563b;
  background: var(--code-bg);
  border-bottom: 1px dotted #c0563b;
}

/* Syntax-highlight token spans (emitted by emit_highlighted_range).
   tok-default / tok-identifier are emitted unwrapped and inherit --text. */
.tok-keyword { color: var(--tok-keyword); }
.tok-string { color: var(--tok-string); }
.tok-number { color: var(--tok-number); }
.tok-comment { color: var(--tok-comment); }

/* Logical comment runs (emitted by emit_line_with_runs). The wrapper carries
   the run-level kind; the inner .tok-comment spans keep the base comment
   colour. data-comment-lines records the source line span. */
.comment-run { display: inline; }

/* Doc comments are normally lifted out of the code and reattached to the
   symbol they describe as a hover-card (see .doc-symbol / .doc-card below).
   This colour only applies to the inline *fallback* — a doc comment with no
   following symbol, left in place so nothing is lost. */
.comment-doc .tok-comment { color: #9aa0b4; }

/* Banner / section-divider comments — muted, a touch wider, act as rules. */
.comment-banner .tok-comment {
  color: #5f6478;
  letter-spacing: 0.04em;
}

/* Attention tags (TODO / FIXME / SAFETY / …) — warmer, weighted. */
.comment-tag .tok-comment {
  color: #d8a657;
  font-weight: 600;
}

/* Asides — the default inline explanation; inherits the base comment colour. */
.comment-aside .tok-comment { color: var(--tok-comment); }

/* Foldable runs (long asides, >=3 lines). Folding itself is the reader's job
   (CSS/JS in 0k.computer); standalone weave just marks them with a faint gutter
   rule so they read as a collapsible unit. Doc runs are never foldable — they
   are lifted out into hover-cards rather than folded in place. */
.comment-run.foldable {
  border-left: 2px solid var(--border);
  padding-left: 0.4em;
  margin-left: -0.4em;
}

/* A described symbol that carries a doc-comment hover-card. The dotted
   underline + help cursor signal there's a docstring to reveal; 0k.computer
   binds the actual popover to .doc-symbol[data-doc]. */
.doc-symbol {
  text-decoration: underline dotted var(--accent-dim);
  text-underline-offset: 3px;
  cursor: help;
}

/* The lifted docstring payload, flushed right after the chunk's </pre>. Hidden
   by default ([hidden]); the reader unhides / repositions it as a popover keyed
   by data-doc. Styled as a simple bordered card so standalone weave output is
   still legible if the card is revealed without 0k.computer's JS. */
.doc-card[hidden] { display: none; }
.doc-card {
  margin: 0.4em 0 0.8em;
  padding: 0.6em 0.9em;
  background: var(--bg-surface);
  border: 1px solid var(--border);
  border-left: 3px solid var(--accent-dim);
  border-radius: 4px;
  font-family: var(--font-serif);
  font-size: 0.92em;
  color: var(--text);
}
.doc-card > :first-child { margin-top: 0; }
.doc-card > :last-child { margin-bottom: 0; }
.doc-card code {
  font-family: var(--font-mono);
  font-size: 0.85em;
}
.tok-type { color: var(--tok-type); }
.tok-function { color: var(--tok-function); }
.tok-property { color: var(--tok-property); }
.tok-operator { color: var(--tok-operator); }
.tok-punctuation { color: var(--tok-punctuation); }

table {
  border-collapse: collapse;
  margin: 1em 0;
  font-size: 0.92em;
}

td, th {
  border: 1px solid var(--border);
  padding: 0.4em 0.8em;
}

th { background: var(--bg-surface); }

blockquote {
  border-left: 3px solid var(--accent-dim);
  padding-left: 1em;
  margin: 1em 0;
  color: var(--text-quiet);
}

ul, ol { margin: 0.5em 0; padding-left: 1.5em; }
li { margin: 0.2em 0; }

strong { color: #e0e0ec; }

hr {
  border: none;
  border-top: 1px solid var(--border);
  margin: 2em 0;
}

.doc-meta {
  margin-bottom: 1.5em;
  padding: 0.5em 0;
  border-bottom: 1px solid var(--border);
  font-family: var(--font-mono);
  font-size: 0.8em;
  color: var(--text-quiet);
}

.meta-tag { margin-right: 1em; }

.chunk {
  margin: 1.2em 0;
  border: 1px solid var(--border);
  border-radius: 6px;
  overflow: hidden;
}

.chunk-header {
  background: var(--chunk-header-bg);
  padding: 0.4em 0.8em;
  font-family: var(--font-mono);
  font-size: 0.78em;
  border-bottom: 1px solid var(--border);
  display: flex;
  gap: 0.8em;
  align-items: center;
  flex-wrap: wrap;
}

.chunk-name {
  color: var(--accent);
  font-weight: 600;
}

.chunk-lang {
  color: var(--text-quiet);
  padding: 0.1em 0.4em;
  border: 1px solid var(--border);
  border-radius: 3px;
  font-size: 0.9em;
}

.chunk-kind {
  color: var(--text-quiet);
  font-style: italic;
}

.chunk-from, .chunk-symbol {
  color: var(--text-quiet);
  font-size: 0.9em;
}

.chunk-ref {
  color: var(--accent);
  text-decoration: none;
  font-style: italic;
}

.chunk-ref:hover {
  text-decoration: underline;
}

.media-embed {
  margin: 1.2em 0;
  padding: 2em;
  background: var(--media-bg);
  border: 1px dashed var(--accent-dim);
  border-radius: 6px;
  text-align: center;
  min-height: 120px;
  display: flex;
  align-items: center;
  justify-content: center;
}

.media-label {
  font-family: var(--font-mono);
  font-size: 0.8em;
  color: var(--accent-dim);
}

.chunk-tabs {
  display: flex;
  gap: 0;
  border-bottom: 1px solid var(--border);
  background: var(--chunk-header-bg);
}

.chunk-tab {
  padding: 4px 12px;
  background: transparent;
  border: none;
  border-bottom: 2px solid transparent;
  color: var(--text-quiet);
  font-family: var(--font-mono);
  font-size: 11px;
  cursor: pointer;
}

.chunk-tab:hover { color: var(--text); }
.chunk-tab.active {
  color: var(--accent);
  border-bottom-color: var(--accent);
}

.chunk-tab-panel { display: none; }
.chunk-tab-panel.active { display: block; }

sup.footnote-ref { font-size: 0.75em; line-height: 0; }
sup.footnote-ref a { text-decoration: none; }
section.footnotes { margin-top: 3em; font-size: 0.9em; }
section.footnotes hr { margin-bottom: 1em; }
section.footnotes ol { padding-left: 1.5em; }
section.footnotes li { margin: 0.4em 0; }
.footnote-backref { text-decoration: none; margin-left: 0.3em; }
"#;
```

## Tests

The weave tests exercise the four rendering paths that matter most:
single-language chunks (no tabs, has the language badge), multi-
language chunks (tabs, no per-fence duplication), mixed docs (both
patterns side-by-side), and the `x0k:params` block (renders as a
data div instead of a code block). One final test checks the onclick
attribute is present — the doc-browser injects the rendered HTML via
innerHTML, and an event listener bound at registration time would
fail; the inline onclick is the cheapest portable solution.

`````rust {#tests}
#[cfg(test)]
mod tests {
    use super::*;
    use crate::parser::parse_document;

    #[test]
    fn single_language_chunk_renders_without_tabs() {
        let content = r#"# Test

```rust {#imports}
use std::io;
```
"#;
        let doc = parse_document(content).unwrap();
        let output = weave_html(content, &doc).unwrap();

        // Extract just the <article> body (skip <style> which contains class definitions)
        let body = output.html.split("<article").nth(1).unwrap();

        // Should have a normal chunk header with the lang badge
        assert!(body.contains("chunk-header"));
        assert!(body.contains("chunk-lang"));
        // Should NOT have tab markup in the body
        assert!(!body.contains("chunk-tabs"));
        assert!(!body.contains("chunk-tab-panel"));
    }

    #[test]
    fn multi_language_chunk_renders_tabs() {
        let content = r#"# Camera

```typescript {#zoom-toward}
zoomToward(sx: number, sy: number, delta: number) { }
```

```rust {#zoom-toward}
pub fn zoom_toward(&mut self, sx: f32, sy: f32, delta: f32) { }
```
"#;
        let doc = parse_document(content).unwrap();
        let output = weave_html(content, &doc).unwrap();

        // Should have tab container
        assert!(output.html.contains("chunk-tabs"));

        // Should have tab buttons for both languages
        assert!(output.html.contains("data-lang=\"typescript\""));
        assert!(output.html.contains("data-lang=\"rust\""));
        assert!(output.html.contains(">TypeScript</button>"));
        assert!(output.html.contains(">Rust</button>"));

        // Should have tab panels
        assert!(output.html.contains("chunk-tab-panel"));

        // First tab+panel should be active
        assert!(output.html.contains("chunk-tab active\" data-lang=\"typescript\""));
        assert!(output.html.contains("chunk-tab-panel active\" data-lang=\"typescript\""));

        // Both code bodies should be present
        assert!(output.html.contains("zoomToward"));
        assert!(output.html.contains("zoom_toward"));

        // Should only render ONE chunk div (not two separate chunks)
        let chunk_count = output.html.matches("<div class=\"chunk\">").count();
        assert_eq!(chunk_count, 1, "multi-lang chunk should render as a single tabbed chunk");
    }

    #[test]
    fn mixed_single_and_multi_lang_chunks() {
        let content = r#"# Mixed

```rust {#preamble}
use std::io;
```

```typescript {#render}
function render() {}
```

```rust {#render}
fn render() {}
```
"#;
        let doc = parse_document(content).unwrap();
        let output = weave_html(content, &doc).unwrap();

        // preamble is single-lang — no tabs, has chunk-lang badge
        assert!(output.html.contains("id=\"chunk-preamble\""));
        // render is multi-lang — has tabs
        assert!(output.html.contains("chunk-tabs"));

        // Two chunk divs total: one for preamble, one for tabbed render
        let chunk_count = output.html.matches("<div class=\"chunk\">").count();
        assert_eq!(chunk_count, 2);
    }

    #[test]
    fn params_block_renders_data_div() {
        let content = r#"# Viz

```yaml x0k:params
- id: threshold_gap
  display_name: Hysteresis Gap
  description: Ratio between zoom_in and zoom_out thresholds
  type: { kind: float, min: 0.1, max: 3.0, step: 0.1 }
  default: 1.5
- id: num_levels
  display_name: LOD Levels
  description: Number of detail levels
  type: { kind: uint, min: 2, max: 8 }
  default: 4
```
"#;
        let doc = parse_document(content).unwrap();
        let output = weave_html(content, &doc).unwrap();

        // Should have param-panel-data div
        assert!(output.html.contains("param-panel-data"));
        assert!(output.html.contains("data-params="));
        // Should contain the param IDs in JSON
        assert!(output.html.contains("threshold_gap"));
        assert!(output.html.contains("num_levels"));
        // Should NOT render as a code block
        assert!(!output.html.contains("<pre><code"));
    }

    #[test]
    fn tab_buttons_have_onclick() {
        let content = r#"# Tabs

```typescript {#example}
let x = 1;
```

```rust {#example}
let x = 1;
```
"#;
        let doc = parse_document(content).unwrap();
        let output = weave_html(content, &doc).unwrap();

        // onclick attributes should be present (for innerHTML compatibility)
        assert!(output.html.contains("onclick=\""));
    }

    #[test]
    fn rust_chunk_emits_highlight_spans() {
        let content = r#"# Hl

```rust {#demo}
fn main() { let x = 42; }
```
"#;
        let doc = parse_document(content).unwrap();
        let output = weave_html(content, &doc).unwrap();

        // Keyword and number tokens are wrapped in tok-* spans.
        assert!(output.html.contains("<span class=\"tok-keyword\">fn</span>"));
        assert!(output.html.contains("<span class=\"tok-number\">42</span>"));
        // A <<ref>> on its own line is still an anchor, never highlighted.
    }

    #[test]
    fn classify_comment_kinds() {
        assert_eq!(classify_comment("/// a doc comment"), "doc");
        assert_eq!(classify_comment("//! inner doc"), "doc");
        assert_eq!(classify_comment("/** block doc */"), "doc");
        assert_eq!(classify_comment("// ── State ──"), "banner");
        assert_eq!(classify_comment("// ===== Foo ====="), "banner");
        assert_eq!(classify_comment("// -------"), "banner");
        assert_eq!(classify_comment("// SAFETY: invariant holds"), "tag");
        assert_eq!(classify_comment("// TODO(x): finish"), "tag");
        assert_eq!(classify_comment("// note: lowercase still a tag"), "tag");
        assert_eq!(classify_comment("// just a plain remark"), "aside");
        // A doc marker beats a divider in the same text.
        assert_eq!(classify_comment("/// ----- titled -----"), "doc");
    }

    #[test]
    fn comment_run_grouping_wraps_and_classifies() {
        // A 4-line `///` doc run (lifted into a hover-card on the symbol it
        // describes), a banner line, a SAFETY tag line, and an aside, interleaved
        // with real code so the non-comment bytes can be checked for byte-identity.
        let content = r#"# Comments

```rust {#demo}
/// First doc line.
/// Second doc line.
/// Third doc line.
/// Fourth doc line.
fn f() {}
// ── Section ──
let a = 1;
// SAFETY: pointer is valid here
let b = 2;
// a plain aside
let c = 3;
```
"#;
        let doc = parse_document(content).unwrap();
        let output = weave_html(content, &doc).unwrap();
        let body = output.html.split("<article").nth(1).unwrap();
        let pre = body.split("<pre>").nth(1).unwrap().split("</pre>").next().unwrap();

        // The doc run is lifted OUT of the `<pre>`: the `///` text never appears
        // inside the code, and there is no inline comment-doc wrapper.
        assert!(!pre.contains("First doc line"));
        assert!(!pre.contains("comment-run comment-doc"));
        assert!(!pre.contains("comment-doc foldable"));

        // The described symbol `f` (the next code line) is wrapped in a
        // doc-symbol affordance carrying the run's data-doc id.
        assert!(pre.contains("<span class=\"doc-symbol\" data-doc=\"doc-0-0\">"));
        // Its hover-card payload is flushed right after `</pre>`, hidden, and
        // holds the rendered docstring prose.
        assert!(body.contains("</pre>\n<div class=\"doc-card\" data-doc=\"doc-0-0\" hidden>"));
        let card = body
            .split("data-doc=\"doc-0-0\" hidden>")
            .nth(1)
            .unwrap()
            .split("</div>")
            .next()
            .unwrap();
        assert!(card.contains("First doc line"));
        assert!(card.contains("Fourth doc line"));

        // Banner, tag, aside still render inline, single-line, never foldable.
        assert!(body.contains(
            "<span class=\"comment-run comment-banner\" data-comment-lines=\"1\">"
        ));
        // The tag run carries its canonical word as `data-tag` for chip colouring.
        assert!(body.contains(
            "<span class=\"comment-run comment-tag\" data-comment-lines=\"1\" data-tag=\"SAFETY\">"
        ));
        assert!(body.contains(
            "<span class=\"comment-run comment-aside\" data-comment-lines=\"1\">"
        ));
        assert!(!body.contains("comment-banner foldable"));
        assert!(!body.contains("comment-tag foldable"));

        // Inner per-token comment spans survive for the inline runs.
        assert!(body.contains("<span class=\"tok-comment\">"));

        // Non-comment code bytes are unchanged: the highlighted keyword/number
        // spans for the interleaved code are still present and untouched.
        assert!(body.contains("<span class=\"tok-keyword\">fn</span>"));
        assert!(body.contains("<span class=\"tok-keyword\">let</span>"));
        assert!(body.contains("<span class=\"tok-number\">1</span>"));
        assert!(body.contains("<span class=\"tok-number\">3</span>"));
        // The wrapper must not leak into non-comment lines: each inline run's
        // closing </span> is followed by the newline and the next code line's
        // own tokens — never another comment-run open.
        assert!(body.contains("</span>\n<span class=\"tok-keyword\">let</span>"));
    }

    #[test]
    fn render_doc_markdown_strips_markers_and_renders() {
        // Markers peeled, markdown rendered: inline code becomes <code>.
        let out = render_doc_markdown("/// a `code` ref");
        assert!(out.contains("<code>code</code>"), "got: {out}");
        assert!(!out.contains("///"));

        // Inner-doc and block markers peel too; **bold** renders.
        let out2 = render_doc_markdown("//! module **docs**");
        assert!(out2.contains("<strong>docs</strong>"));
        assert!(!out2.contains("//!"));
    }

    #[test]
    fn math_renders_to_mathml() {
        // Inline `$…$` and display `$$…$$` become MathML; the `$` delimiters
        // are gone and the math is real markup (not html-escaped).
        let content = "# Math\n\nMass-energy: $E = mc^2$ and\n\n$$\\frac{1}{2}$$\n";
        let doc = parse_document(content).unwrap();
        let body = weave_html(content, &doc).unwrap().html;
        let body = body.split("<article").nth(1).unwrap();
        assert!(body.contains("<math"), "inline math → <math>: {body}");
        assert!(body.contains("display=\"block\""), "display math → block");
        assert!(!body.contains("$E = mc^2$"), "the $-delimiters are consumed");
        // <math> is emitted verbatim, not escaped into &lt;math&gt;.
        assert!(!body.contains("&lt;math"));
    }

    #[test]
    fn bad_math_falls_back_to_raw_tex() {
        // An unsupported macro must not panic or vanish — it lands in a
        // `.math-error` carrying the escaped raw TeX.
        // Unknown macro (latex2mathml returns Ok with an embedded parse-error
        // marker) → fallback, no broken MathML leaks through.
        let out = render_math("\\nonexistentmacro{x}", false);
        assert!(out.contains("class=\"math-error\""), "got: {out}");
        assert!(out.contains("nonexistentmacro"), "raw TeX preserved: {out}");
        assert!(!out.contains("PARSE ERROR"), "no broken-MathML marker leaks: {out}");
        // Hard syntax error (unbalanced braces) also falls back, no panic.
        let hard = render_math("\\frac{1}{", true);
        assert!(hard.contains("class=\"math-error\""), "got: {hard}");
        // A valid fragment renders to MathML inline.
        let ok = render_math("x^2", false);
        assert!(ok.contains("<math") && ok.contains("display=\"inline\""), "got: {ok}");
    }

    #[test]
    fn doc_comment_lifts_to_symbol_and_card() {
        // The canonical shape: `/// foo` over `pub fn bar()` →
        // doc-symbol on `bar`, a card payload with "foo", no `///` in the <pre>.
        let content = r#"# Doc

```rust {#demo}
/// foo
pub fn bar() {}
```
"#;
        let doc = parse_document(content).unwrap();
        let output = weave_html(content, &doc).unwrap();
        let body = output.html.split("<article").nth(1).unwrap();
        let pre = body.split("<pre>").nth(1).unwrap().split("</pre>").next().unwrap();

        // No `///` in the code.
        assert!(!pre.contains("///"));
        assert!(!pre.contains("foo"));
        // `bar` carries the doc-symbol affordance.
        assert!(pre.contains("data-doc=\"doc-0-0\">"));
        assert!(pre.contains("doc-symbol"));
        assert!(pre.contains("bar"));
        // The card holds the rendered docstring and sits after </pre>, hidden.
        assert!(body.contains("<div class=\"doc-card\" data-doc=\"doc-0-0\" hidden>"));
        assert!(body.contains(">foo</"));
    }

    #[test]
    fn doc_comment_skips_attribute_to_reach_symbol() {
        // The common Rust shape: a docstring, an attribute, then the item. The
        // symbol is the `fn`, NOT the attribute — the heuristic must skip
        // `#[wasm_bindgen]` and anchor `bar`.
        let content = r#"# Doc

```rust {#demo}
/// foo
#[wasm_bindgen]
pub fn bar() {}
```
"#;
        let doc = parse_document(content).unwrap();
        let output = weave_html(content, &doc).unwrap();
        let body = output.html.split("<article").nth(1).unwrap();
        let pre = body.split("<pre>").nth(1).unwrap().split("</pre>").next().unwrap();
        assert!(!pre.contains("///"), "docstring lifted out of the code");
        // The attribute stays in the code; the doc-symbol lands on `bar`, not on
        // the attribute name.
        assert!(pre.contains("#[wasm_bindgen]") || pre.contains("wasm_bindgen"));
        assert!(pre.contains("doc-symbol"));
        // The doc-symbol wraps `bar` (the declaration), and a card exists.
        assert!(body.contains("<div class=\"doc-card\" data-doc=\"doc-0-0\" hidden>"));
        // Confirm the symbol span is on `bar` by checking the doc-symbol span text.
        let sym = pre.split("doc-symbol").nth(1).unwrap_or("");
        assert!(sym.contains("bar"), "doc-symbol anchors the declaration, not the attribute");
    }

    #[test]
    fn inner_doc_anchors_to_chunk_header() {
        // `//!` documents the module → the chunk-name in the header carries the
        // data-doc, the card is emitted, and the `//!` text is lifted out.
        let content = r#"# Inner

```rust {#demo}
//! module overview
fn f() {}
```
"#;
        let doc = parse_document(content).unwrap();
        let output = weave_html(content, &doc).unwrap();
        let body = output.html.split("<article").nth(1).unwrap();

        // The chunk-name element carries data-doc for the module card.
        assert!(body.contains("<span class=\"chunk-name doc-symbol\" data-doc=\"doc-0-0\">demo</span>"));
        // The card exists and the `//!` text is not inside the <pre>.
        let pre = body.split("<pre>").nth(1).unwrap().split("</pre>").next().unwrap();
        assert!(!pre.contains("module overview"));
        assert!(body.contains("<div class=\"doc-card\" data-doc=\"doc-0-0\" hidden>"));
        assert!(body.contains("module overview"));
    }

    #[test]
    fn doc_comment_without_symbol_falls_back_inline() {
        // A doc comment with no following declaration keeps rendering inline
        // (nothing lost), and emits no card.
        let content = r#"# Fallback

```rust {#demo}
fn f() {
    /// orphan doc
}
```
"#;
        let doc = parse_document(content).unwrap();
        let output = weave_html(content, &doc).unwrap();
        let body = output.html.split("<article").nth(1).unwrap();
        let pre = body.split("<pre>").nth(1).unwrap().split("</pre>").next().unwrap();

        // Rendered inline as a comment-doc run; no card emitted.
        assert!(pre.contains("comment-run comment-doc"));
        assert!(pre.contains("orphan doc"));
        assert!(!body.contains("doc-card"));
    }

    #[test]
    fn comment_run_preserves_indentation_bytes() {
        // An indented comment: the leading whitespace must stay OUTSIDE the
        // wrapper, byte-for-byte, and the wrapper opens at the `//`.
        let content = r#"# Indent

```rust {#demo}
fn f() {
    // an indented aside
    let x = 1;
}
```
"#;
        let doc = parse_document(content).unwrap();
        let output = weave_html(content, &doc).unwrap();
        let body = output.html.split("<article").nth(1).unwrap();

        // Four spaces of indent precede the wrapper open verbatim.
        assert!(body.contains(
            "    <span class=\"comment-run comment-aside\" data-comment-lines=\"1\">"
        ));
        // The following code line keeps its four-space indent intact.
        assert!(body.contains("    <span class=\"tok-keyword\">let</span>"));
    }

    #[test]
    fn ref_line_is_not_highlighted() {
        let content = r#"# Compose

```rust {#leaf}
let y = 1;
```

```rust {#root}
<<leaf>>
```
"#;
        let doc = parse_document(content).unwrap();
        let output = weave_html(content, &doc).unwrap();

        // The composing ref renders as a chunk-ref anchor, not a code span.
        assert!(output.html.contains("class=\"chunk-ref\" href=\"#chunk-leaf\""));
    }

    #[test]
    fn escaped_ref_line_renders_as_the_literal_without_an_anchor() {
        let content = r#"# Show the syntax

```rust {#leaf}
let y = 1;
```

```markdown {#readme}
Put this line where the listing says so:
    <<!leaf>>
```
"#;
        let doc = parse_document(content).unwrap();
        let output = weave_html(content, &doc).unwrap();

        assert!(output.html.contains("    &lt;&lt;leaf&gt;&gt;\n"), "{}", output.html);
        assert!(!output.html.contains("href=\"#chunk-leaf\""));
        assert!(!output.html.contains("&lt;&lt;!leaf&gt;&gt;"));
    }

    #[test]
    fn headings_get_slug_ids() {
        let content = r#"# Doc

## Overview

prose

### Design Details

more prose
"#;
        let doc = parse_document(content).unwrap();
        let output = weave_html(content, &doc).unwrap();

        assert!(output.html.contains("<h2 id=\"overview\">Overview</h2>"));
        assert!(output.html.contains("<h3 id=\"design-details\">Design Details</h3>"));
    }

    #[test]
    fn duplicate_headings_get_deduped_ids() {
        let content = r#"# Doc

## Notes

first

## Notes

second
"#;
        let doc = parse_document(content).unwrap();
        let output = weave_html(content, &doc).unwrap();

        assert!(output.html.contains("<h2 id=\"notes\">Notes</h2>"));
        assert!(output.html.contains("<h2 id=\"notes-2\">Notes</h2>"));
    }

    #[test]
    fn heading_with_inline_markup_slugs_and_preserves_markup() {
        let content = "# Doc\n\n## The `weave` function\n\nbody\n";
        let doc = parse_document(content).unwrap();
        let output = weave_html(content, &doc).unwrap();

        // Slug ignores the inline-code backticks and punctuation.
        assert!(output.html.contains("<h2 id=\"the-weave-function\">"));
        // The inner markup is preserved inside the heading.
        assert!(output.html.contains("<code>weave</code>"));
    }
}
`````

## Composing the module

```rust {#root}
<<imports>>

<<weave-html-fn>>

<<render-code-block-fn>>

<<render-tabbed-chunk-fn>>

<<capitalize-lang-fn>>

<<render-code-with-refs-fn>>

<<render-param-panel-fn>>

<<small-helpers>>

<<stylesheet>>

<<tests>>
```
