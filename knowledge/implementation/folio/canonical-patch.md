---
x0k:
  format: folio/v1
  id: x0k:implementation/folio/canonical-patch
  type: implementation
  status: draft
  summary: The structural-address patch grammar editors speak instead of byte offsets, so one editing intent is true of a markdown body and an HTML body alike and cannot smuggle non-canonical markup past normalization.
  concerns: [folio, editing, patches, markdown, html, canonicalization]
  tangle:
    crate: x0k-folio
    root: src/canonical_patch.rs
  edges:
    implements:
      - x0k:design/body-format-isomorphism
    cites:
      - x0k:implementation/folio/colophon
      - x0k:implementation/folio/structural
      - x0k:implementation/folio/html-canonical
---
# Canonical patches: one grammar, two dialects

A folio body is stored as markdown or as HTML (`body_format` in the
envelope — [`colophon.md`](colophon.md)), and the design
`x0k:design/body-format-isomorphism` promises an author meets the *same*
document either way. Two editing surfaces act on that document: the
renderer-lockstep HTML overlay and the editor bridge that relays a live
editor's changes into the shared CRDT. An editing intent — "replace the
visible word in that strong span", "point that link somewhere else" —
has nothing to do with how the body happens to be serialized. So the
question this module answers is: what does an editor *say* when it
wants a change, such that the sentence is true of both dialects?

The wrong answer is byte offsets into a serialization, and it is wrong
twice over. Offsets into the markdown mean nothing against the HTML and
vice versa, so every surface would carry two edit paths. Worse, a byte
splice is a way around canonicalization: a live editor could write
non-canonical HTML into the shared document, the next file-side save
would re-normalize it into one whole-span rewrite, and concurrent
character positions inside that span would be steamrolled. The merge
engine cannot fix what the grammar lets through.

The answer is **one patch grammar over structural addresses**.
`CanonicalPatch` names an element by a rooted path of `tag[index]`
segments in the shared HTML subset `H_md` — `/p[1]/strong[1]`,
`/pre[1]/code[1]` — and a text position by run ordinal and byte offset
inside that element. Each dialect resolves those identities into its
own projection: the HTML side walks the canonical DOM
([`html-canonical.md`](html-canonical.md)); the markdown side, which
this chapter owns, builds an index over pulldown-cmark's offset events
that mints the *same* paths for the same structure and edits the source
in place. Both apply attributes before visible text, and both emit
output that is canonical for their dialect.

One example runs through the chapter. The same document, twice:

```text
A **shared**[source](https://old.example).
```

```html
<p>A <strong>shared</strong><a href="https://old.example">source</a>.</p>
```

Two patches: replace the visible text of `/p[1]/strong[1]` from byte 0
to byte 6 with `single`, and set `href` on `/p[1]/a[1]` to
`https://new.example`. Applied to the markdown body the result is
`A **single**[source](https://new.example).`; applied to the HTML body,
`<p>A <strong>single</strong><a href="https://new.example">source</a>.</p>`.
Every mechanism below exists to make those two sentences literally
true — and `tests/patch_grammar.rs` pins them, patch order deliberately
supplied text-first.

## The contract

Before any mechanism, what the module promises. It performs no IO,
spawns nothing, reads no clock; every function is a pure map from
strings and patches to a string or a typed error. Patch coordinates are
dialect-neutral identities in the structural tree, never offsets into
markdown or HTML bytes. Attributes are applied before text in every
projection, so a text address computed against the pre-patch structure
stays valid. The envelope of a folio document is preserved byte for
byte; coordinates start at the body root. HTML output is canonical by
construction — it passes through `normalize_html` on the way in and the
canonical serializer on the way out. Markdown output is
*source-stable*: bytes the patch did not address are the bytes the
author wrote.

```rust {#module-doc}
//! Canonical structural patches shared by markdown and HTML folio bodies.
//!
//! Patches name structural segments (`/p[1]/strong[1]`,
//! `/pre[1]/code[1]`, and so on), never serialized byte positions. Each body
//! dialect resolves those identities into its own projection, applies
//! attributes before visible-text replacements, and emits canonical output.

use std::collections::BTreeMap;
use std::fmt;
use std::ops::Range;

use pulldown_cmark::{Event, HeadingLevel, Options, Parser, Tag};
use serde::{Deserialize, Serialize};

use crate::colophon::{parse_envelope, BODY_FORMAT_HTML, BODY_FORMAT_MARKDOWN};
use crate::html_canonical::{apply_canonical_patches, normalize_html};
use crate::FenceInfo;
```

A **text point** is one position inside one decoded visible-text run of
one element. The run ordinal counts direct text children of the
element; the offset is into the *decoded* text (entities resolved,
escapes removed), which is what an editor sees. The type derives serde
because it rides the editor bridge's wire:

```rust {#text-point}
/// A point in one decoded visible-text run of a canonical structural body.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct CanonicalTextPoint {
    pub element: String,
    pub text_run: usize,
    pub byte_offset: usize,
}

impl CanonicalTextPoint {
    pub fn new(element: impl Into<String>, text_run: usize, byte_offset: usize) -> Self {
        Self {
            element: element.into(),
            text_run,
            byte_offset,
        }
    }
}
```

The vocabulary is two operations, and the split is the safety property.
Attribute mutation is structurally separate from visible-text
replacement, so a text edit can never rewrite serialized tags or
silently discard sibling markup — there is no "replace inner HTML" and
no "replace source bytes" on purpose. The `op` tag gives the wire form
the bridge's `Patch` message carries: `{"op": "set_attribute", ...}`
and `{"op": "replace_visible_text", ...}`.

```rust {#patch-vocabulary}
/// The one mutation vocabulary for canonical folio body structure.
///
/// Attribute patches are always applied before text patches. The element and
/// text-run coordinates are identities in the canonical structural tree, not
/// offsets in a markdown or HTML serialization.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "op", rename_all = "snake_case")]
pub enum CanonicalPatch {
    SetAttribute {
        element: String,
        name: String,
        value: Option<String>,
    },
    ReplaceVisibleText {
        start: CanonicalTextPoint,
        end: CanonicalTextPoint,
        replacement: String,
    },
}
```

HTML-side callers spell the same type under a dialect-flavored name;
[`html-canonical.md`](html-canonical.md) re-exports it:

```rust {#html-patch-alias}
/// The HTML-side spelling of [`CanonicalPatch`]; the same type.
pub type CanonicalHtmlPatch = CanonicalPatch;
```

Errors are typed because the bridge reports them to an editor that can
act on them. The first five are shared by both projections; the four
that follow are markdown's own — an attribute outside the dialect's
allowlist, an attribute value the dialect cannot carry, a replacement
that would break a fence, and a document whose envelope will not parse.
`ReversedRange` stays in the vocabulary although neither projection
raises it today: both normalize a reversed range by swapping.

```rust {#patch-errors}
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum CanonicalPatchError {
    ElementNotFound(String),
    TextRunNotFound {
        element: String,
        text_run: usize,
    },
    InvalidTextOffset {
        element: String,
        text_run: usize,
        byte_offset: usize,
    },
    ReversedRange,
    InvalidAttributeName(String),
    UnsupportedMarkdownAttribute {
        element: String,
        name: String,
    },
    InvalidMarkdownAttribute {
        element: String,
        name: String,
        value: Option<String>,
    },
    InvalidMarkdownReplacement(String),
    InvalidFolio(String),
}
```

```rust {#error-display}
impl fmt::Display for CanonicalPatchError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ElementNotFound(element) => write!(f, "canonical element not found: {element}"),
            Self::TextRunNotFound { element, text_run } => {
                write!(f, "canonical text run not found: {element} run {text_run}")
            }
            Self::InvalidTextOffset {
                element,
                text_run,
                byte_offset,
            } => write!(
                f,
                "invalid UTF-8 text offset {byte_offset}: {element} run {text_run}"
            ),
            Self::ReversedRange => write!(f, "visible-text patch range is reversed"),
            Self::InvalidAttributeName(name) => {
                write!(f, "invalid canonical attribute name: {name}")
            }
            Self::UnsupportedMarkdownAttribute { element, name } => write!(
                f,
                "canonical attribute `{name}` is not representable on markdown element {element}"
            ),
            Self::InvalidMarkdownAttribute {
                element,
                name,
                value,
            } => write!(
                f,
                "invalid markdown projection for attribute `{name}`={value:?} on {element}"
            ),
            Self::InvalidMarkdownReplacement(message) => {
                write!(f, "invalid markdown visible-text replacement: {message}")
            }
            Self::InvalidFolio(message) => write!(f, "invalid folio/v1 document: {message}"),
        }
    }
}

impl std::error::Error for CanonicalPatchError {}
```

## Dispatch by dialect

The body format string from the envelope selects the projection. The
HTML arm is the DOM path in [`html-canonical.md`](html-canonical.md);
the markdown arm is the rest of this chapter. Any other format is an
`InvalidFolio` error rather than a silent pass-through — a patch that
cannot be applied must not report success.

```rust {#dispatch-by-dialect}
/// Apply canonical patches to a body in either supported folio dialect.
pub fn apply_body_patches(
    body: &str,
    body_format: &str,
    patches: &[CanonicalPatch],
) -> Result<String, CanonicalPatchError> {
    match body_format {
        BODY_FORMAT_HTML => apply_canonical_patches(body, patches),
        BODY_FORMAT_MARKDOWN => apply_markdown_patches(body, patches),
        other => Err(CanonicalPatchError::InvalidFolio(format!(
            "unsupported body format `{other}`"
        ))),
    }
}
```

A whole folio document is envelope plus body, and the envelope's
formatting is not part of the edit grammar: patches start at the body
root, and the envelope bytes are copied through untouched. The one
assumption made explicit is that the body is a suffix of the file —
`parse_envelope` returns the body as such, and the length subtraction
that recovers its start is checked rather than trusted.

```rust {#patch-folio-document}
/// Apply canonical body patches to a complete folio/v1 document.
///
/// The envelope is preserved byte-for-byte. Patch coordinates start at the
/// body root, so envelope formatting never becomes part of the edit grammar.
pub fn apply_folio_patches(
    content: &str,
    patches: &[CanonicalPatch],
) -> Result<String, CanonicalPatchError> {
    let (envelope, body) = parse_envelope(content)
        .map_err(|error| CanonicalPatchError::InvalidFolio(error.to_string()))?;
    let body_start = content.len().checked_sub(body.len()).ok_or_else(|| {
        CanonicalPatchError::InvalidFolio("body is not a file suffix".to_string())
    })?;
    let patched = apply_body_patches(&body, &envelope.body_format, patches)?;
    let mut output = String::with_capacity(body_start + patched.len());
    output.push_str(&content[..body_start]);
    output.push_str(&patched);
    Ok(output)
}
```

## Canonicalization at the bridge

The editor bridge has two ways to change a document: the patch grammar
above, and a compatibility path that accepts a whole edited value from
a character-indexed editor. Both must land on canonical content, and
neither may make the merge engine — or its identity — part of the
document contract. `canonicalize_folio_content` is the chokepoint:
content that is not a folio document is the generic bridge's business
and passes through unchanged; a folio HTML body takes the same
normalizer as the HTML overlay; a folio markdown body is left alone,
because the source file *is* the projection of record and nothing here
reformats it.

```rust {#canonicalize-folio-content}
/// Canonicalize a bridge-produced document value without making the merge
/// engine or its identity part of the document contract.
///
/// Non-folio content remains valid for the generic editor bridge and passes
/// through unchanged. Folio HTML bodies take the same normalizer as the HTML
/// overlay; markdown source is already the canonical file projection and is
/// preserved byte-for-byte.
pub fn canonicalize_folio_content(content: &str) -> String {
    let Ok((envelope, body)) = parse_envelope(content) else {
        return content.to_string();
    };
    if envelope.body_format != BODY_FORMAT_HTML {
        return content.to_string();
    }
    let Some(body_start) = content.len().checked_sub(body.len()) else {
        return content.to_string();
    };
    let normalized = normalize_html(&body);
    let mut output = String::with_capacity(body_start + normalized.len());
    output.push_str(&content[..body_start]);
    output.push_str(&normalized);
    output
}
```

The compatibility path is where the bypass would live, so it gets the
stricter rule. If the edited value parses as folio, it is canonicalized
like any other. If it does not, what matters is what it was *before*:
a value that was a valid folio document may not escape canonicalization
by corrupting its own envelope — that is an error back to the editor —
while a generic non-folio document keeps its pass-through behavior,
including the case where an edit first turns it into a valid folio
document (that edit canonicalizes, by the first arm).

```rust {#canonicalize-edited-content}
/// Canonicalize the result of a source-level compatibility edit.
///
/// A value that was folio/v1 before the edit may not escape canonicalization
/// by corrupting its envelope. Generic non-folio documents retain the bridge's
/// existing pass-through behavior, including when an edit first turns one into
/// a valid folio document.
pub fn canonicalize_edited_folio_content(
    previous: &str,
    edited: &str,
) -> Result<String, CanonicalPatchError> {
    let previous_was_folio = parse_envelope(previous).is_ok();
    match parse_envelope(edited) {
        Ok(_) => Ok(canonicalize_folio_content(edited)),
        Err(error) if previous_was_folio => Err(CanonicalPatchError::InvalidFolio(format!(
            "source edit broke the envelope: {error}"
        ))),
        Err(_) => Ok(edited.to_string()),
    }
}
```

## The markdown projection

`apply_markdown_patches` mirrors the HTML path's shape: attributes, then
text. Each patch re-indexes the current source before it resolves,
because a patch shifts every byte range after it and re-parsing is the
simple correct answer. The cost is one parse per patch; patch batches
are small, and a stale-range bug would be worse than the parse.

```rust {#apply-markdown-patches}
/// Apply canonical patches to a markdown body and emit a source-stable
/// markdown projection.
pub fn apply_markdown_patches(
    input: &str,
    patches: &[CanonicalPatch],
) -> Result<String, CanonicalPatchError> {
    let mut output = input.to_string();
    for patch in patches
        .iter()
        .filter(|patch| matches!(patch, CanonicalPatch::SetAttribute { .. }))
    {
        output = apply_markdown_attribute_patch(&output, patch)?;
    }
    for patch in patches
        .iter()
        .filter(|patch| matches!(patch, CanonicalPatch::ReplaceVisibleText { .. }))
    {
        output = apply_markdown_text_patch(&output, patch)?;
    }
    Ok(output)
}
```

### The index

Two things are addressable in markdown: **elements** that carry a
patchable attribute — links, fenced code blocks, task markers — and
**text runs**. Each remembers its source byte range, which is what makes
in-place editing possible. A text run also keeps its decoded text and a
*kind*, because the way back from decoded text to source depends on
where the run sits: plain text is escaped, a code span is re-fenced, a
fenced block is emitted verbatim. Frames are the walk's stack; each
mints paths for its children by counting siblings per tag.

```rust {#markdown-index-types}
#[derive(Clone, Debug)]
enum MarkdownElementKind {
    Link { destination: String },
    Fence,
    TaskMarker,
}

#[derive(Clone, Debug)]
struct MarkdownElement {
    path: String,
    source: Range<usize>,
    kind: MarkdownElementKind,
}

#[derive(Clone, Copy, Debug)]
enum MarkdownTextKind {
    Text,
    CodeSpan,
    CodeBlock,
}

#[derive(Clone, Debug)]
struct MarkdownTextRun {
    point: CanonicalTextPoint,
    source: Range<usize>,
    decoded: String,
    kind: MarkdownTextKind,
}

#[derive(Debug)]
struct MarkdownFrame {
    path: String,
    tag: String,
    child_counts: BTreeMap<String, usize>,
    text_runs: usize,
}

impl MarkdownFrame {
    fn root() -> Self {
        Self {
            path: String::new(),
            tag: String::new(),
            child_counts: BTreeMap::new(),
            text_runs: 0,
        }
    }
}

#[derive(Default)]
struct MarkdownIndex {
    elements: Vec<MarkdownElement>,
    text_runs: Vec<MarkdownTextRun>,
}
```

The walk itself. pulldown-cmark's offset iterator yields every event
with its source range, which is exactly the structure-with-bytes we
need. A `Start` pushes as many frames as the element maps to (a fenced
block pushes two, `pre` then `code`, so the fence lands at
`/pre[1]/code[1]` — the design's one carrier) and remembers how many,
so the matching `End` pops the same count. Links and fences record an
element at the innermost frame; a task marker attaches to the nearest
enclosing `li`. Text inside `pre > code` is a code-block run; an inline
`Code` event gets a synthetic `code` frame for the duration of one run,
so a code span is addressable as `/p[1]/code[1]` just as it is in the
HTML. The events that mint nothing are the honest edge of the grammar:
breaks, rules, and raw HTML islands are complement matter, and the
design makes no claim to edit their internals.

```rust {#markdown-index}
fn markdown_index(input: &str) -> MarkdownIndex {
    let options =
        Options::ENABLE_STRIKETHROUGH | Options::ENABLE_TABLES | Options::ENABLE_TASKLISTS;
    let mut index = MarkdownIndex::default();
    let mut frames = vec![MarkdownFrame::root()];
    let mut event_frame_counts = Vec::new();

    for (event, source) in Parser::new_ext(input, options).into_offset_iter() {
        match event {
            Event::Start(tag) => {
                let tags = markdown_element_tags(&tag, &frames);
                let mut pushed = 0usize;
                for html_tag in tags {
                    push_markdown_frame(&mut frames, html_tag);
                    pushed += 1;
                }
                if let Tag::Link { dest_url, .. } = &tag {
                    if let Some(frame) = frames.last() {
                        index.elements.push(MarkdownElement {
                            path: frame.path.clone(),
                            source: source.clone(),
                            kind: MarkdownElementKind::Link {
                                destination: dest_url.to_string(),
                            },
                        });
                    }
                }
                if matches!(tag, Tag::CodeBlock(_)) {
                    if let Some(frame) = frames.last() {
                        index.elements.push(MarkdownElement {
                            path: frame.path.clone(),
                            source: source.clone(),
                            kind: MarkdownElementKind::Fence,
                        });
                    }
                }
                event_frame_counts.push(pushed);
            }
            Event::End(_) => {
                let count = event_frame_counts.pop().unwrap_or(0);
                for _ in 0..count {
                    if frames.len() > 1 {
                        frames.pop();
                    }
                }
            }
            Event::Text(text) => {
                let kind = if frames.last().is_some_and(|frame| frame.tag == "code")
                    && frames
                        .iter()
                        .rev()
                        .nth(1)
                        .is_some_and(|frame| frame.tag == "pre")
                {
                    MarkdownTextKind::CodeBlock
                } else {
                    MarkdownTextKind::Text
                };
                push_markdown_text_run(&mut index, &mut frames, source, text.to_string(), kind);
            }
            Event::Code(text) => {
                push_markdown_frame(&mut frames, "code".to_string());
                push_markdown_text_run(
                    &mut index,
                    &mut frames,
                    source,
                    text.to_string(),
                    MarkdownTextKind::CodeSpan,
                );
                frames.pop();
            }
            Event::TaskListMarker(_) => {
                if let Some(frame) = frames.iter().rev().find(|frame| frame.tag == "li") {
                    index.elements.push(MarkdownElement {
                        path: frame.path.clone(),
                        source,
                        kind: MarkdownElementKind::TaskMarker,
                    });
                }
            }
            Event::SoftBreak
            | Event::HardBreak
            | Event::Rule
            | Event::Html(_)
            | Event::InlineHtml(_)
            | Event::FootnoteReference(_)
            | Event::InlineMath(_)
            | Event::DisplayMath(_) => {}
        }
    }
    index
}
```

Paths are minted exactly as the HTML walk mints them — 1-based sibling
ordinal per tag under the parent, joined by `/` — which is the whole
reason the two dialects agree on `/p[1]/strong[1]`:

```rust {#push-markdown-frame}
fn push_markdown_frame(frames: &mut Vec<MarkdownFrame>, tag: String) {
    let parent = frames.last_mut().expect("markdown root frame");
    let count = parent.child_counts.entry(tag.clone()).or_default();
    *count += 1;
    let path = if parent.path.is_empty() {
        format!("/{tag}[{count}]")
    } else {
        format!("{}/{tag}[{count}]", parent.path)
    };
    frames.push(MarkdownFrame {
        path,
        tag,
        child_counts: BTreeMap::new(),
        text_runs: 0,
    });
}
```

A run's identity is (element, ordinal); the point stored on it carries
offset 0 and exists so `find_markdown_text_run` can compare identities
without a second type:

```rust {#push-markdown-text-run}
fn push_markdown_text_run(
    index: &mut MarkdownIndex,
    frames: &mut [MarkdownFrame],
    source: Range<usize>,
    decoded: String,
    kind: MarkdownTextKind,
) {
    let frame = frames.last_mut().expect("markdown root frame");
    let text_run = frame.text_runs;
    frame.text_runs += 1;
    index.text_runs.push(MarkdownTextRun {
        point: CanonicalTextPoint::new(&frame.path, text_run, 0),
        source,
        decoded,
        kind,
    });
}
```

The tag map is `H_md` from the design, spelled out. Table cells are
`th` under a `thead` and `td` otherwise; footnote definitions, HTML
blocks, and metadata blocks are transparent — no frame, so their
children (if any) address through them. One candid mismatch: definition
lists are minted here (`dl`, `dt`, `dd`) although the design places
them outside `H_md`; a markdown patch can address them, but nothing on
the HTML side promises to agree.

```rust {#markdown-element-tags}
fn markdown_element_tags(tag: &Tag<'_>, frames: &[MarkdownFrame]) -> Vec<String> {
    match tag {
        Tag::Paragraph => vec!["p".to_string()],
        Tag::Heading { level, .. } => vec![heading_tag(*level).to_string()],
        Tag::BlockQuote(_) => vec!["blockquote".to_string()],
        Tag::CodeBlock(_) => vec!["pre".to_string(), "code".to_string()],
        Tag::List(start) => vec![if start.is_some() { "ol" } else { "ul" }.to_string()],
        Tag::Item => vec!["li".to_string()],
        Tag::Strong => vec!["strong".to_string()],
        Tag::Emphasis => vec!["em".to_string()],
        Tag::Strikethrough => vec!["del".to_string()],
        Tag::Link { .. } => vec!["a".to_string()],
        Tag::Image { .. } => vec!["img".to_string()],
        Tag::Table(_) => vec!["table".to_string()],
        Tag::TableHead => vec!["thead".to_string()],
        Tag::TableRow => vec!["tr".to_string()],
        Tag::TableCell => vec![if frames.iter().any(|frame| frame.tag == "thead") {
            "th"
        } else {
            "td"
        }
        .to_string()],
        Tag::DefinitionList => vec!["dl".to_string()],
        Tag::DefinitionListTitle => vec!["dt".to_string()],
        Tag::DefinitionListDefinition => vec!["dd".to_string()],
        Tag::FootnoteDefinition(_) | Tag::HtmlBlock | Tag::MetadataBlock(_) => Vec::new(),
    }
}

fn heading_tag(level: HeadingLevel) -> &'static str {
    match level {
        HeadingLevel::H1 => "h1",
        HeadingLevel::H2 => "h2",
        HeadingLevel::H3 => "h3",
        HeadingLevel::H4 => "h4",
        HeadingLevel::H5 => "h5",
        HeadingLevel::H6 => "h6",
    }
}
```

### Attribute patches

Markdown has no general attribute syntax, so a `SetAttribute` is
admissible only where the dialect has a carrier for it — and that set
is the design's allowlist, verbatim: `href` on a link; `class`
(`language-*`), `data-x0k-type`, and `data-x0k-info` on a fenced code
block; `data-x0k-task` on a task item. Anything else is
`UnsupportedMarkdownAttribute`, which is the right answer: the HTML
overlay can set `class` on a paragraph, but that paragraph is then
designed matter, not shared structure, and a markdown body has nowhere
to put it.

```rust {#apply-markdown-attribute-patch}
fn apply_markdown_attribute_patch(
    input: &str,
    patch: &CanonicalPatch,
) -> Result<String, CanonicalPatchError> {
    let CanonicalPatch::SetAttribute {
        element,
        name,
        value,
    } = patch
    else {
        return Ok(input.to_string());
    };
    validate_attribute_name(name)?;
    let entry = markdown_index(input)
        .elements
        .into_iter()
        .find(|entry| entry.path == *element)
        .ok_or_else(|| CanonicalPatchError::ElementNotFound(element.clone()))?;
    match entry.kind {
        MarkdownElementKind::Link { destination } if name == "href" => {
            replace_markdown_link_destination(input, &entry.source, &destination, value, element)
        }
        MarkdownElementKind::Fence
            if matches!(name.as_str(), "class" | "data-x0k-type" | "data-x0k-info") =>
        {
            replace_markdown_fence_attribute(input, &entry.source, name, value, element)
        }
        MarkdownElementKind::TaskMarker if name == "data-x0k-task" => {
            replace_markdown_task_marker(input, &entry.source, value, element)
        }
        _ => Err(CanonicalPatchError::UnsupportedMarkdownAttribute {
            element: element.clone(),
            name: name.clone(),
        }),
    }
}
```

The name check is the HTML side's rule, repeated here so a markdown
patch cannot smuggle a name the HTML projection would refuse:

```rust {#validate-attribute-name}
fn validate_attribute_name(name: &str) -> Result<(), CanonicalPatchError> {
    if name.is_empty()
        || name
            .chars()
            .any(|ch| ch.is_ascii_whitespace() || matches!(ch, '"' | '\'' | '<' | '>' | '='))
    {
        Err(CanonicalPatchError::InvalidAttributeName(name.to_string()))
    } else {
        Ok(())
    }
}
```

Retargeting a link is grungier than it sounds. A link cannot lose its
destination and remain a link, so `None` is refused; `javascript:` is
refused at the source because markdown has no normalizer downstream to
strip it, and the policy must hold in both dialects. Then the
destination has to be *found* inside the link's source span, and the
honest mechanism is `rfind` of the parsed destination text — a
heuristic that relies on the destination appearing verbatim in the
source, which pulldown-cmark's unescaped `dest_url` does not guarantee
when the author escaped parentheses. A miss surfaces as
`InvalidMarkdownAttribute`, never as a silent splice into the wrong
bytes. The new value is escaped on the way in so it cannot close the
link early.

```rust {#replace-link-destination}
fn replace_markdown_link_destination(
    input: &str,
    source: &Range<usize>,
    destination: &str,
    value: &Option<String>,
    element: &str,
) -> Result<String, CanonicalPatchError> {
    let Some(value) = value else {
        return Err(CanonicalPatchError::InvalidMarkdownAttribute {
            element: element.to_string(),
            name: "href".to_string(),
            value: None,
        });
    };
    if value
        .trim_start()
        .to_ascii_lowercase()
        .starts_with("javascript:")
    {
        return Err(CanonicalPatchError::InvalidMarkdownAttribute {
            element: element.to_string(),
            name: "href".to_string(),
            value: Some(value.clone()),
        });
    }
    let fragment = &input[source.clone()];
    let relative = fragment.rfind(destination).ok_or_else(|| {
        CanonicalPatchError::InvalidMarkdownAttribute {
            element: element.to_string(),
            name: "href".to_string(),
            value: Some(value.clone()),
        }
    })?;
    let start = source.start + relative;
    let end = start + destination.len();
    let mut output = input.to_string();
    output.replace_range(start..end, &escape_markdown_destination(value));
    Ok(output)
}

fn escape_markdown_destination(value: &str) -> String {
    value
        .replace('\\', "\\\\")
        .replace('(', "\\(")
        .replace(')', "\\)")
        .replace(' ', "%20")
}
```

A fence attribute is a component of the info string, and the info
string has one canonical order (`lang`, `x0k:type`, then the rest —
`FenceInfo` in [`structural.md`](structural.md)). So the mechanism is:
re-read the opening line (indent, a marker run of at least three
backticks or tildes, the info string), swap one component, and re-emit
the canonical carrier. `class` must be `language-<lang>` with a
non-empty language, or `None` to clear it; the `unreachable!` is
justified by the caller's match arm.

```rust {#replace-fence-attribute}
fn replace_markdown_fence_attribute(
    input: &str,
    source: &Range<usize>,
    name: &str,
    value: &Option<String>,
    element: &str,
) -> Result<String, CanonicalPatchError> {
    let fragment = &input[source.clone()];
    let line_end = fragment.find('\n').unwrap_or(fragment.len());
    let opening = &fragment[..line_end];
    let indent_len = opening.len() - opening.trim_start().len();
    let rest = &opening[indent_len..];
    let marker_len = rest
        .bytes()
        .take_while(|byte| *byte == b'`' || *byte == b'~')
        .count();
    if marker_len < 3 {
        return Err(CanonicalPatchError::InvalidMarkdownAttribute {
            element: element.to_string(),
            name: name.to_string(),
            value: value.clone(),
        });
    }
    let current = FenceInfo::parse(rest[marker_len..].trim());
    let mut language = current.language().map(str::to_string);
    // The marker, not the declared type: an illustrative `x0k:!type`
    // must survive a patch to a neighbouring attribute rather than be
    // promoted into a declaration.
    let mut x0k_type = current.x0k_marker().map(str::to_string);
    let mut info = current.info().map(str::to_string);
    match name {
        "class" => {
            language = value
                .as_deref()
                .and_then(|value| value.strip_prefix("language-"))
                .filter(|value| !value.is_empty())
                .map(str::to_string);
            if value.is_some() && language.is_none() {
                return Err(CanonicalPatchError::InvalidMarkdownAttribute {
                    element: element.to_string(),
                    name: name.to_string(),
                    value: value.clone(),
                });
            }
        }
        "data-x0k-type" => x0k_type = value.clone(),
        "data-x0k-info" => info = value.clone(),
        _ => unreachable!("filtered by caller"),
    }
    let carrier = FenceInfo::from_parts(language.as_deref(), x0k_type.as_deref(), info.as_deref())
        .canonical();
    let mut new_opening = opening[..indent_len + marker_len].to_string();
    new_opening.push_str(&carrier);
    let mut output = input.to_string();
    output.replace_range(source.start..source.start + line_end, &new_opening);
    Ok(output)
}
```

A task marker is the simplest carrier: the event's source range is the
`[ ]` or `[x]` itself, and the value vocabulary is closed.

```rust {#replace-task-marker}
fn replace_markdown_task_marker(
    input: &str,
    source: &Range<usize>,
    value: &Option<String>,
    element: &str,
) -> Result<String, CanonicalPatchError> {
    let marker = match value.as_deref() {
        Some("todo") => "[ ]",
        Some("done") => "[x]",
        _ => {
            return Err(CanonicalPatchError::InvalidMarkdownAttribute {
                element: element.to_string(),
                name: "data-x0k-task".to_string(),
                value: value.clone(),
            })
        }
    };
    let mut output = input.to_string();
    output.replace_range(source.clone(), marker);
    Ok(output)
}
```

### Text patches

Text replacement resolves both endpoints to run indices, validates the
offsets against the decoded text (a byte offset must sit on a char
boundary), and normalizes order by swapping. Then, for every run from
start to end: the start run keeps its head and takes the replacement,
interior runs empty, the end run keeps its tail. Each run yields a new
source string for its own byte range, and the splices are applied
back-to-front so the earlier ranges stay valid while the later ones
change.

```rust {#apply-markdown-text-patch}
fn apply_markdown_text_patch(
    input: &str,
    patch: &CanonicalPatch,
) -> Result<String, CanonicalPatchError> {
    let CanonicalPatch::ReplaceVisibleText {
        start,
        end,
        replacement,
    } = patch
    else {
        return Ok(input.to_string());
    };
    let runs = markdown_index(input).text_runs;
    let mut start_index = find_markdown_text_run(&runs, start)?;
    let mut end_index = find_markdown_text_run(&runs, end)?;
    let mut start_offset = start.byte_offset;
    let mut end_offset = end.byte_offset;
    validate_markdown_text_offset(&runs[start_index], start.byte_offset)?;
    validate_markdown_text_offset(&runs[end_index], end.byte_offset)?;
    if (start_index, start_offset) > (end_index, end_offset) {
        std::mem::swap(&mut start_index, &mut end_index);
        std::mem::swap(&mut start_offset, &mut end_offset);
    }

    let mut replacements = Vec::new();
    for (index, run) in runs
        .iter()
        .enumerate()
        .take(end_index + 1)
        .skip(start_index)
    {
        let from = if index == start_index {
            start_offset
        } else {
            0
        };
        let to = if index == end_index {
            end_offset
        } else {
            run.decoded.len()
        };
        let inserted = if index == start_index {
            replacement
        } else {
            ""
        };
        replacements.push((
            run.source.clone(),
            replace_markdown_run(input, run, from, to, inserted)?,
        ));
    }
    replacements.sort_by_key(|entry| std::cmp::Reverse(entry.0.start));
    let mut output = input.to_string();
    for (range, replacement) in replacements {
        output.replace_range(range, &replacement);
    }
    Ok(output)
}
```

Source-stability lives in one branch. When a run's source bytes equal
its decoded text — no escapes, no entities — the replacement is spliced
into the *source* and only the inserted text is rendered, so every
untouched byte survives, punctuation included. When they differ, the
whole decoded run is edited and re-rendered, and the author's original
escaping is replaced by ours.

```rust {#replace-markdown-run}
fn replace_markdown_run(
    input: &str,
    run: &MarkdownTextRun,
    from: usize,
    to: usize,
    replacement: &str,
) -> Result<String, CanonicalPatchError> {
    let source = &input[run.source.clone()];
    if source == run.decoded {
        let mut output = source.to_string();
        output.replace_range(from..to, &render_markdown_text(replacement, run.kind)?);
        return Ok(output);
    }

    let mut decoded = run.decoded.clone();
    decoded.replace_range(from..to, replacement);
    render_markdown_text(&decoded, run.kind)
}
```

Resolution and validation are identity lookups over the index:

```rust {#resolve-markdown-run}
fn find_markdown_text_run(
    runs: &[MarkdownTextRun],
    point: &CanonicalTextPoint,
) -> Result<usize, CanonicalPatchError> {
    runs.iter()
        .position(|run| run.point.element == point.element && run.point.text_run == point.text_run)
        .ok_or_else(|| CanonicalPatchError::TextRunNotFound {
            element: point.element.clone(),
            text_run: point.text_run,
        })
}

fn validate_markdown_text_offset(
    run: &MarkdownTextRun,
    byte_offset: usize,
) -> Result<(), CanonicalPatchError> {
    if byte_offset <= run.decoded.len() && run.decoded.is_char_boundary(byte_offset) {
        Ok(())
    } else {
        Err(CanonicalPatchError::InvalidTextOffset {
            element: run.point.element.clone(),
            text_run: run.point.text_run,
            byte_offset,
        })
    }
}
```

Rendering decoded text back into source depends on the run's kind.
Plain text is escaped; a code span is re-fenced with one more backtick
than its longest internal run; a fenced block is emitted verbatim, with
one guard — a replacement line that opens with three backticks would
close the fence from inside, so it is refused rather than let the
document's structure change under a text edit.

```rust {#render-markdown-text}
fn render_markdown_text(
    decoded: &str,
    kind: MarkdownTextKind,
) -> Result<String, CanonicalPatchError> {
    match kind {
        MarkdownTextKind::Text => Ok(escape_markdown_visible_text(decoded)),
        MarkdownTextKind::CodeSpan => Ok(render_code_span(decoded)),
        MarkdownTextKind::CodeBlock => {
            if decoded
                .lines()
                .any(|line| line.trim_start().starts_with("```"))
            {
                return Err(CanonicalPatchError::InvalidMarkdownReplacement(
                    "a fenced code replacement cannot introduce a closing backtick line"
                        .to_string(),
                ));
            }
            Ok(decoded.to_string())
        }
    }
}
```

The escaping is deliberately conservative: every ASCII character
CommonMark can read as structure gets a backslash. This is more than
the design's `N_m` printer would emit ("only the escaping needed to
preserve structure on reparse"), and the result is correspondingly
noisier than a hand-written line — but it is unconditionally safe, and
the fast path above means it only touches text the patch actually
replaced.

```rust {#escape-markdown-text}
fn escape_markdown_visible_text(text: &str) -> String {
    let mut output = String::with_capacity(text.len());
    for character in text.chars() {
        if matches!(
            character,
            '\\' | '`'
                | '*'
                | '_'
                | '{'
                | '}'
                | '['
                | ']'
                | '<'
                | '>'
                | '('
                | ')'
                | '#'
                | '+'
                | '-'
                | '.'
                | '!'
                | '|'
        ) {
            output.push('\\');
        }
        output.push(character);
    }
    output
}

fn render_code_span(text: &str) -> String {
    let longest = text
        .split(|character| character != '`')
        .map(str::len)
        .max()
        .unwrap_or(0);
    let fence = "`".repeat(longest + 1);
    format!("{fence}{text}{fence}")
}
```

## Tests

Two unit tests pin the load-bearing claims of the markdown side: that
the index mints HTML-shaped paths (`/p[1]/strong[1]`, `/p[1]/a[1]`) for
the carried example, and that a text patch through the fast path leaves
neighboring punctuation untouched.

```rust {#tests}
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn markdown_paths_match_html_shaped_segments() {
        let index = markdown_index("A **shared** [link](https://example.com).\n");
        assert!(index
            .text_runs
            .iter()
            .any(|run| run.point.element == "/p[1]/strong[1]"));
        assert!(index
            .elements
            .iter()
            .any(|element| element.path == "/p[1]/a[1]"));
    }

    #[test]
    fn markdown_text_patch_preserves_untouched_punctuation() {
        let patched = apply_markdown_patches(
            "A shared sentence.\n",
            &[CanonicalPatch::ReplaceVisibleText {
                start: CanonicalTextPoint::new("/p[1]", 0, 2),
                end: CanonicalTextPoint::new("/p[1]", 0, 8),
                replacement: "single".to_string(),
            }],
        )
        .expect("paragraph patch");

        assert_eq!(patched, "A single sentence.\n");
    }
}
```

### Across the dialects

Both tests above live inside the markdown projection, and neither can
state the claim the design actually makes. `x0k:design/body-format-isomorphism`
promises that *one sentence is true of both bodies*; a test that only ever
sees one projection cannot say that. So the carried example is run twice
through the crate's public surface, in `tests/patch_grammar.rs`.

```rust {#patch-grammar-imports file="tests/patch_grammar.rs"}
use x0k_folio::{
    apply_folio_patches, canonicalize_edited_folio_content, canonicalize_folio_content,
    CanonicalPatch, CanonicalTextPoint,
};
```

The fixtures are whole documents rather than bare bodies. The dialect is a
fact about the envelope, so handing the module a body alone would test a
dispatch that never happens in the running system:

```rust {#patch-grammar-documents file="tests/patch_grammar.rs"}
const MARKDOWN_DOC: &str = r#"---
x0k:
  format: folio/v1
  id: x0k:wiki/patch-markdown
  type: wiki
---
A **shared**[source](https://old.example).
"#;

const HTML_DOC: &str = r#"---
x0k:
  format: folio/v1
  id: x0k:design/patch-html
  type: design
  body_format: html
---
<p>A <strong>shared</strong><a href="https://old.example">source</a>.</p>
"#;
```

One patch list serves both, and it is deliberately in the wrong order —
text first, attribute second. Both projections sort it back, applying
attributes before visible text, so the order a caller happens to supply
cannot make the two dialects disagree:

```rust {#patch-grammar-patches file="tests/patch_grammar.rs"}
fn equivalent_patches() -> Vec<CanonicalPatch> {
    vec![
        // Deliberately supplied text-first. The grammar applies attributes
        // first in both dialect projections.
        CanonicalPatch::ReplaceVisibleText {
            start: CanonicalTextPoint::new("/p[1]/strong[1]", 0, 0),
            end: CanonicalTextPoint::new("/p[1]/strong[1]", 0, "shared".len()),
            replacement: "single".to_string(),
        },
        CanonicalPatch::SetAttribute {
            element: "/p[1]/a[1]".to_string(),
            name: "href".to_string(),
            value: Some("https://new.example".to_string()),
        },
    ]
}
```

The first test is this chapter's opening promise, checked: the two result
sentences from the introduction, byte for byte, and the wire tags an editing
surface will actually put on the socket.

```rust {#patch-grammar-both-dialects file="tests/patch_grammar.rs"}
#[test]
fn markdown_and_html_bodies_accept_the_same_patch_grammar() {
    let patches = equivalent_patches();
    let markdown = apply_folio_patches(MARKDOWN_DOC, &patches).expect("markdown patch");
    let html = apply_folio_patches(HTML_DOC, &patches).expect("HTML patch");

    assert!(markdown.contains("A **single**[source](https://new.example)."));
    assert!(
        html.contains(
            r#"<p>A <strong>single</strong><a href="https://new.example">source</a>.</p>"#
        ),
        "patched HTML was: {html}"
    );

    let wire = serde_json::to_value(&patches).expect("serialize shared patch grammar");
    assert_eq!(wire[0]["op"], "replace_visible_text");
    assert_eq!(wire[1]["op"], "set_attribute");
}
```

A patch must not start a rewrite that never settles. Saving a patched body
and reparsing it has to return the same bytes — in both dialects, and on the
second pass as well as the first. This is the test that keeps the markdown
side's "changed nothing it was not asked to" from decaying into "changed
something small every time":

```rust {#patch-grammar-reparse-stable file="tests/patch_grammar.rs"}
#[test]
fn patched_bodies_are_save_reparse_stable_in_both_dialects() {
    for source in [MARKDOWN_DOC, HTML_DOC] {
        let saved = apply_folio_patches(source, &equivalent_patches()).expect("patch body");
        let reparsed = canonicalize_folio_content(&saved);
        let reparsed_again = canonicalize_folio_content(&reparsed);

        assert_eq!(saved, reparsed);
        assert_eq!(reparsed, reparsed_again);
    }
}
```

The last test guards the seam the grammar exists to close. A bridge that
hands back a whole edited HTML body is exactly the bypass byte splices would
have allowed: the raw result here carries a `<script>` and out-of-order
attributes. The canonicalizing entry point strips the one and sorts the
other before the edit becomes a document, and the result is a fixed point.
An edit that breaks the envelope is not repaired — it is refused, because a
document the parser cannot read is not an edit the merge engine can reason
about.

```rust {#patch-grammar-bridge-normalization file="tests/patch_grammar.rs"}
#[test]
fn bridge_style_source_edit_cannot_bypass_html_normalization() {
    let raw_bridge_result = HTML_DOC.replace(
        "<p>",
        r#"<p z-last="2" a-first="1"><script>unsafe()</script>"#,
    );
    assert!(raw_bridge_result.contains("<script>"));

    let canonical = canonicalize_edited_folio_content(HTML_DOC, &raw_bridge_result)
        .expect("bridge compatibility edit remains a valid folio document");

    assert!(!canonical.contains("<script>"));
    assert!(canonical.contains(r#"<p a-first="1" z-last="2">"#));
    assert_eq!(canonical, canonicalize_folio_content(&canonical));

    let broken_envelope = raw_bridge_result.replacen("---", "--", 1);
    assert!(canonicalize_edited_folio_content(HTML_DOC, &broken_envelope).is_err());
}
```

```rust {#patch-grammar-root file="tests/patch_grammar.rs"}
<<patch-grammar-imports>>

<<patch-grammar-documents>>

<<patch-grammar-patches>>

<<patch-grammar-both-dialects>>

<<patch-grammar-reparse-stable>>

<<patch-grammar-bridge-normalization>>
```

## Composing the module

```rust {#root}
<<module-doc>>

<<text-point>>

<<patch-vocabulary>>

<<html-patch-alias>>

<<patch-errors>>

<<error-display>>

<<dispatch-by-dialect>>

<<patch-folio-document>>

<<canonicalize-folio-content>>

<<canonicalize-edited-content>>

<<apply-markdown-patches>>

<<markdown-index-types>>

<<markdown-index>>

<<push-markdown-frame>>

<<push-markdown-text-run>>

<<markdown-element-tags>>

<<apply-markdown-attribute-patch>>

<<validate-attribute-name>>

<<replace-link-destination>>

<<replace-fence-attribute>>

<<replace-task-marker>>

<<apply-markdown-text-patch>>

<<replace-markdown-run>>

<<resolve-markdown-run>>

<<render-markdown-text>>

<<escape-markdown-text>>

<<tests>>
```

The grammar is one and the projections are two, but only one of them
has a normal form. An HTML patch lands on a normalizer, and "canonical
output" is a theorem. A markdown patch lands on source, where
"canonical" can only mean "changed nothing it was not asked to" — and
that is the genuinely hard part of this module: the markdown side is a
source editor wearing a structural editor's interface. It holds because
pulldown-cmark gives structure *with* byte ranges, and because the
seams are named rather than hidden — a destination located by `rfind`,
escaping that over-approximates `N_m`, a fence that refuses to be closed
from inside. Where the design's printer normal form and this module's
source-stability disagree, the disagreement is visible in the bytes,
not in the structure, and the reparse-stability test is what keeps it
that way.
