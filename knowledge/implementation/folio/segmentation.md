---
x0k:
  format: folio/v1
  id: x0k:implementation/folio/segmentation
  type: implementation
  status: draft
  summary: Why a block needs both an identifier that survives edits and a hash that does not, and how the pair makes an acceptance go stale rather than orphaned or silently migrated onto someone else's paragraph.
  concerns: [folio, blocks, hashing, underwriting, markdown]
  tangle:
    crate: x0k-folio
    root: src/block_segment.rs
  edges:
    implements:
      - x0k:design/in-prose-authoring
      - x0k:design/prose-provenance-and-underwriting
    cites:
      - x0k:implementation/folio/colophon
      - x0k:implementation/folio/provenance
      - x0k:implementation/folio/structural
---
# Segmentation: two identities for every block

Underwriting — "I have read this block and accept it as currently stated"
— needs to attach a judgment to a piece of a document and then *notice*
when that piece changes. That single requirement forces a design decision
this module exists to make: every top-level block of a folio body carries
**two** identifiers, and they answer opposite questions.

- The **block id** answers "which block is this?" It must *survive* edits
  to the block's own content, so that an underwriting recorded against a
  paragraph stays attached to that paragraph — and goes **stale** — when
  the prose changes, rather than getting orphaned.
- The **content hash** answers "is this still the text that was
  accepted?" It must *change* on every content edit; the acceptance locks
  to an exact hash, and staleness is simply `recorded_hash !=
  current_hash`.

The obvious move is one identifier doing both jobs, and each choice fails
one requirement. A pure content hash loses the acceptance the moment a
typo is fixed (nothing points at the new hash). A pure positional index
silently *migrates* the acceptance onto whatever paragraph now sits third
— worse than losing it, because it lies. Two identifiers, each honest
about one axis, is the minimal correct shape.

Take our carried example, the publication manifest
`x0k-folio.md`. Its body opens with a lead paragraph, then
`## What is published`, then prose under that heading. Segmenting it
yields blocks whose ids read like postal addresses —
`_root/p/0`, `_root/h/0`, `what-is-published/p/0` — each carrying a
blake3 hash of its exact text. When the operator underwrites the lead
paragraph and someone later revises it, the id still says "the lead
paragraph" and the hash says "not the text you accepted."

There is a sibling identity scheme in this crate,
[`structural.md`](structural.md)'s `BlockId`, which folds content AND
position into one hash. That is not a rival answer to the same question —
it anchors *proposals*, which should survive only while the targeted
content is unchanged. Underwriting wants the opposite stability. The two
schemes coexist because the two lifetimes genuinely differ.

One more constraint shapes everything here: the id must be recomputable
**from the file alone** — no sidecar, no CRDT. `decisions/` is
filesystem-canonical, so any reader (a human, an agent, a freshly booted
daemon) must derive the same ids for the same bytes. Loro-structural ids
are reserved for the db-canonical `knowledge/` tree.

```rust {#module-doc}
//! Body segmentation for in-place authoring + underwriting.
//!
//! Splits a folio body into an ordered list of top-level **segments**
//! (paragraph, heading, code block, list, blockquote, html block). Each
//! segment carries two distinct identifiers, and the distinction is
//! load-bearing:
//!
//! - [`BlockSegment::block_id`] — a *stability key* derived from the
//!   block's heading-path + kind + ordinal-within-section. It survives
//!   edits to the block's own content (editing a paragraph's prose does
//!   not change its id), so an underwriting recorded against a block stays
//!   attached to that block and goes **stale** rather than getting lost.
//! - [`BlockSegment::content_hash`] — a blake3 hash of the block's
//!   canonicalized text. It changes on every content edit; the underwriting
//!   gesture locks an acceptance to *this exact hash*, and staleness is
//!   `recorded_hash != current_hash`.
//!
//! This is deliberately different from [`crate::structural_block::BlockId`],
//! which folds content AND position into one hash for *proposal anchoring*
//! (an anchor that survives only while the targeted content is unchanged).
//! Underwriting needs the opposite: an id that is stable *across* content
//! edits. Hence a separate, string-shaped, recomputable-from-the-file key.
//!
//! The id is recomputable from the file alone — no sidecar, no CRDT — so any
//! reader (human, agent, a freshly-booted daemon) derives the same id for
//! the same block. `decisions/` is filesystem-canonical, so this is the
//! right authority model; Loro-structural ids are reserved for the
//! db-canonical `knowledge/` tree.

use std::ops::Range;

use pulldown_cmark::{Event, HeadingLevel, Options, Parser, Tag, TagEnd};
```

## The segment vocabulary

Six kinds cover the top-level constructs a body can hold. Each kind
carries two names: a short tag that goes *into* the block id (ids are
read by humans in provenance logs, so `brief/p/2` beats
`brief/paragraph/2`) and a full name for API clients rendering a block
model.

```rust {#block-kind}
/// Block-level kind. One variant per top-level markdown construct we treat
/// as an independently editable / underwritable segment. HTML bodies map
/// every top-level element to [`BlockKind::HtmlSection`] (see
/// [`segment_body`]).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BlockKind {
    Heading,
    Paragraph,
    CodeBlock,
    List,
    Quote,
    HtmlSection,
}

impl BlockKind {
    /// Short tag used in the `block_id` string (`Brief/p/2`).
    pub fn tag(self) -> &'static str {
        match self {
            BlockKind::Heading => "h",
            BlockKind::Paragraph => "p",
            BlockKind::CodeBlock => "code",
            BlockKind::List => "list",
            BlockKind::Quote => "quote",
            BlockKind::HtmlSection => "html",
        }
    }

    /// Full, API-facing name (`"paragraph"`, `"code_block"`) for clients
    /// rendering a block model.
    pub fn name(self) -> &'static str {
        match self {
            BlockKind::Heading => "heading",
            BlockKind::Paragraph => "paragraph",
            BlockKind::CodeBlock => "code_block",
            BlockKind::List => "list",
            BlockKind::Quote => "quote",
            BlockKind::HtmlSection => "html_section",
        }
    }
}
```

A segment is the two identities plus what a caller needs to *use* them:
the byte range (so an editor can splice a replacement into the original
body) and the exact text slice (the per-block markdown an editor shows).

```rust {#block-segment}
/// One segment of a document body.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BlockSegment {
    /// Stability key: `{heading_path}/{kind_tag}/{ordinal}`. Stable across
    /// edits to this block's own content; drifts only when blocks are
    /// inserted/removed above it within the same section.
    pub block_id: String,
    pub kind: BlockKind,
    /// Byte range of the segment in the original body string.
    pub byte_range: Range<usize>,
    /// blake3 hash (hex) of the canonicalized segment text. The underwriting
    /// acceptance lock.
    pub content_hash: String,
    /// The exact body slice for this segment (the editor's per-block markdown).
    pub text: String,
}
```

The doc comment on `block_id` names the honest limit of the stability
key: it drifts when blocks are inserted or removed *above* it within the
same section, because the ordinal moves. Section-scoping (below) is what
keeps that blast radius small — an edit under `## Details` never renumbers
anything under `## Brief`.

## The entry point

```rust {#segment-body}
/// Segment a document body into ordered top-level blocks.
///
/// `body_format` is `"markdown"` (default) or `"html"`. For markdown we walk
/// pulldown-cmark's offset iterator at depth 0; for HTML — read-only in the
/// editor today, but underwriting still wants segments — we fall back to a
/// single whole-body section; true per-element HTML segmentation is a
/// deferred refinement (see `html_canonical`). Unknown formats are
/// treated as markdown.
pub fn segment_body(body: &str, body_format: &str) -> Vec<BlockSegment> {
    if body_format == "html" {
        return segment_html(body);
    }
    segment_markdown(body)
}
```

## Hashing, and what the hash forgives

The content hash is blake3 over *canonicalized* text. Two decisions live
in that sentence. blake3 rather than `DefaultHasher` because the hash is
persisted in provenance logs and must be stable across compiler versions
and rebuilds — `DefaultHasher` explicitly is not, and a hash that changes
on `cargo update` would mark every underwriting in the corpus stale at
once. And canonicalization, because an edit that cannot change how the
block renders (trailing whitespace, surrounding blank lines) should not
revoke anyone's acceptance. Interior whitespace *does* count — `hello
world` and `hello  world` render differently in a code span.

```rust {#hash-block}
/// blake3 hex hash of canonicalized text. Public so the save / underwriting
/// tools hash the same way the segmenter does.
pub fn hash_block(text: &str) -> String {
    blake3::hash(canonicalize(text).as_bytes())
        .to_hex()
        .to_string()
}

/// Canonicalize a block's text before hashing so rendering-neutral edits
/// (trailing whitespace, surrounding blank lines) don't churn the hash:
/// trim each line's trailing whitespace, then trim the block.
fn canonicalize(text: &str) -> String {
    text.lines()
        .map(|l| l.trim_end())
        .collect::<Vec<_>>()
        .join("\n")
        .trim()
        .to_string()
}
```

## Building the address

The block id's path component comes from the heading structure. Two small
helpers: a level-to-number map (pulldown-cmark's `HeadingLevel` is an
enum) and a slugifier that turns heading text into a path segment. The
slug caps at 40 characters so a long heading doesn't make every id under
it unreadable; the cap is safe because the ordinal disambiguates.

```rust {#heading-helpers}
fn heading_level_num(level: HeadingLevel) -> usize {
    match level {
        HeadingLevel::H1 => 1,
        HeadingLevel::H2 => 2,
        HeadingLevel::H3 => 3,
        HeadingLevel::H4 => 4,
        HeadingLevel::H5 => 5,
        HeadingLevel::H6 => 6,
    }
}

/// Slugify a heading's text for use in a `block_id` path component: lower,
/// alnum runs joined by `-`, capped so ids stay readable.
fn slug(text: &str) -> String {
    let mut out = String::new();
    let mut prev_dash = false;
    for ch in text.chars() {
        if ch.is_alphanumeric() {
            out.extend(ch.to_lowercase());
            prev_dash = false;
        } else if !prev_dash && !out.is_empty() {
            out.push('-');
            prev_dash = true;
        }
    }
    let trimmed = out.trim_matches('-');
    let capped: String = trimmed.chars().take(40).collect();
    // Trim AGAIN after the cap: trimming only before it lets truncation land
    // mid-word and leave a trailing dash, so a long heading slugs to
    // `...-the-vocabulary-`. The anchor is a public address — a transclusion
    // ref and a publication's section selector both spell it — so a dangling
    // dash is not cosmetic.
    let capped = capped.trim_matches('-');
    if capped.is_empty() {
        "untitled".to_string()
    } else {
        capped.to_string()
    }
}
```

This slug scheme is a cross-module contract:
[`transclusion.md`](transclusion.md)'s `heading_slug` must produce
identical output so that section anchors minted here resolve there. The
duplication (rather than one shared function) keeps `transclusion`
consumable without this module; the transclusion chapter owns the test
that pins the equivalence.

## The markdown walk

Segmenting is one pass over pulldown-cmark's offset iterator, tracking
three pieces of state: a depth counter (only blocks that open at depth 0
are segments — a paragraph inside a list item belongs to the list), a
stack of heading slugs (the current section path), and per-`(path, kind)`
ordinal counters.

The subtlety worth deriving is *when the heading stack moves*. A heading
is a block in its own right, and it must be attributed to its **parent**
section — `## Brief` is a child of the document root, not of itself. So
on a heading's close event the stack first pops everything at the same or
deeper level (a sibling `## B` after `## A` must not nest under `A`),
then the heading's segment is emitted against the *popped* path, and only
then is the new slug pushed so subsequent blocks nest under it. Getting
this order wrong produces ids like `brief/h/0` for `## Brief` itself —
an address that names the thing as its own parent.

```rust {#segment-markdown}
fn segment_markdown(body: &str) -> Vec<BlockSegment> {
    let mut opts = Options::empty();
    opts.insert(Options::ENABLE_STRIKETHROUGH);
    opts.insert(Options::ENABLE_TABLES);

    let mut segments: Vec<BlockSegment> = Vec::new();
    // Heading slugs currently in scope, indexed by depth. `heading_path`
    // is rebuilt from this stack each time a top-level block is emitted.
    let mut heading_stack: Vec<(usize, String)> = Vec::new();
    // Per-(heading_path, kind) ordinal counters, keyed by the path+tag.
    let mut ordinals: std::collections::HashMap<String, usize> = std::collections::HashMap::new();
    // Depth tracking: we only segment blocks that open at depth 0.
    let mut depth: usize = 0;
    // When a depth-0 block opens, remember (kind, start_offset).
    let mut open: Option<(BlockKind, usize)> = None;

    let path_string = |stack: &[(usize, String)]| -> String {
        if stack.is_empty() {
            "_root".to_string()
        } else {
            stack
                .iter()
                .map(|(_, s)| s.as_str())
                .collect::<Vec<_>>()
                .join("/")
        }
    };

    for (event, range) in Parser::new_ext(body, opts).into_offset_iter() {
        match event {
            Event::Start(tag) => {
                if depth == 0 {
                    let kind = match &tag {
                        Tag::Heading { .. } => Some(BlockKind::Heading),
                        Tag::Paragraph => Some(BlockKind::Paragraph),
                        Tag::CodeBlock(_) => Some(BlockKind::CodeBlock),
                        Tag::List(_) => Some(BlockKind::List),
                        Tag::BlockQuote(_) => Some(BlockKind::Quote),
                        Tag::HtmlBlock => Some(BlockKind::HtmlSection),
                        _ => None,
                    };
                    if let Some(kind) = kind {
                        open = Some((kind, range.start));
                    }
                }
                // Headings adjust the path stack on close, not open, so the
                // heading itself is attributed to its parent section.
                depth += 1;
            }
            Event::End(tag_end) => {
                depth = depth.saturating_sub(1);
                if depth == 0 {
                    if let Some((kind, start)) = open.take() {
                        let end = range.end;
                        let text = body[start..end].to_string();

                        // A heading is attributed to its *parent* section, so
                        // pop deeper-or-equal headings off the stack BEFORE
                        // computing the path — a sibling `## B` after `## A`
                        // nests under their shared ancestor, not under `A`.
                        if let TagEnd::Heading(level) = tag_end {
                            let lvl = heading_level_num(level);
                            while let Some((d, _)) = heading_stack.last() {
                                if *d >= lvl {
                                    heading_stack.pop();
                                } else {
                                    break;
                                }
                            }
                        }

                        let path = path_string(&heading_stack);
                        let okey = format!("{path}|{}", kind.tag());
                        let ord = ordinals.entry(okey).or_insert(0);
                        let ordinal = *ord;
                        *ord += 1;

                        let block_id = format!("{path}/{}/{ordinal}", kind.tag());
                        segments.push(BlockSegment {
                            block_id,
                            kind,
                            byte_range: start..end,
                            content_hash: hash_block(&text),
                            text,
                        });

                        // Now push the heading so subsequent siblings nest
                        // under it.
                        if let TagEnd::Heading(level) = tag_end {
                            let lvl = heading_level_num(level);
                            let htext = body[start..end].trim_start_matches('#').trim();
                            heading_stack.push((lvl, slug(htext)));
                        }
                    }
                }
            }
            _ => {}
        }
    }

    segments
}
```

## The HTML fallback

HTML bodies get one whole-body segment. This is a knowing simplification,
not an oversight: the editor keeps HTML bodies read-only, so a single
segment is enough to carry provenance and accept an underwriting over the
whole body. Per-element segmentation — each top-level `<section>`, `<p>`,
`<h*>` addressed through [`html-canonical.md`](html-canonical.md)'s
canonical paths — is the natural refinement if HTML bodies ever become
block-editable, and nothing in the id scheme blocks it.

```rust {#segment-html}
/// HTML segmentation: one whole-body section. Per-element HTML
/// segmentation (each top-level `<section>/<p>/<h*>/...` via
/// `html_canonical`) is a deferred refinement; the editor keeps HTML bodies
/// read-only, so a single segment is enough to carry provenance.
fn segment_html(body: &str) -> Vec<BlockSegment> {
    let trimmed = body.trim();
    if trimmed.is_empty() {
        return Vec::new();
    }
    vec![BlockSegment {
        block_id: "_root/html/0".to_string(),
        kind: BlockKind::HtmlSection,
        byte_range: 0..body.len(),
        content_hash: hash_block(body),
        text: body.to_string(),
    }]
}
```

## Tests

The fixture is a miniature of the carried example's shape — intro
paragraph, two `##` sections, a code block and a list — and the tests pin
each half of the two-identity contract in turn: section-scoped addresses,
id-stable-hash-changed under a content edit, byte ranges that slice back
to the text, and whitespace-forgiving hashing.

```rust {#tests}
#[cfg(test)]
mod tests {
    use super::*;

    const DOC: &str = "Intro paragraph.\n\n## Brief\n\nFirst point here.\n\nSecond point here.\n\n## Details\n\n```rust\nfn x() {}\n```\n\n- a\n- b\n";

    #[test]
    fn segments_top_level_blocks() {
        let segs = segment_body(DOC, "markdown");
        let kinds: Vec<BlockKind> = segs.iter().map(|s| s.kind).collect();
        assert_eq!(
            kinds,
            vec![
                BlockKind::Paragraph, // intro
                BlockKind::Heading,   // ## Brief
                BlockKind::Paragraph, // first point
                BlockKind::Paragraph, // second point
                BlockKind::Heading,   // ## Details
                BlockKind::CodeBlock,
                BlockKind::List,
            ]
        );
    }

    #[test]
    fn block_ids_are_section_scoped() {
        let segs = segment_body(DOC, "markdown");
        let ids: Vec<&str> = segs.iter().map(|s| s.block_id.as_str()).collect();
        assert_eq!(ids[0], "_root/p/0");
        assert_eq!(ids[1], "_root/h/0");
        assert_eq!(ids[2], "brief/p/0");
        assert_eq!(ids[3], "brief/p/1");
        assert_eq!(ids[4], "_root/h/1");
        assert_eq!(ids[5], "details/code/0");
        assert_eq!(ids[6], "details/list/0");
    }

    #[test]
    fn block_id_stable_but_hash_changes_on_content_edit() {
        let a = segment_body(DOC, "markdown");
        let edited = DOC.replace("First point here.", "First point, revised.");
        let b = segment_body(&edited, "markdown");
        // The edited paragraph keeps its id (stability key) ...
        assert_eq!(a[2].block_id, b[2].block_id);
        // ... but its content hash changes (staleness signal).
        assert_ne!(a[2].content_hash, b[2].content_hash);
        // Untouched neighbors keep both id and hash.
        assert_eq!(a[3].block_id, b[3].block_id);
        assert_eq!(a[3].content_hash, b[3].content_hash);
    }

    #[test]
    fn byte_ranges_slice_back_to_text() {
        let segs = segment_body(DOC, "markdown");
        for s in &segs {
            assert_eq!(&DOC[s.byte_range.clone()], s.text);
        }
    }

    #[test]
    fn idempotent_segmentation() {
        let segs1 = segment_body(DOC, "markdown");
        let segs2 = segment_body(DOC, "markdown");
        assert_eq!(segs1, segs2);
    }

    #[test]
    fn hash_is_whitespace_canonical() {
        assert_eq!(hash_block("hello world"), hash_block("hello world  \n"));
        assert_ne!(hash_block("hello world"), hash_block("hello  world"));
    }

    /// The save tool preserves frontmatter byte-for-byte by keeping the
    /// content prefix (everything before the parsed body slice) and swapping
    /// only the body. This guards against `render_envelope`'s canonicalizing
    /// field reorder ever leaking into the human-canonical decisions tree.
    #[test]
    fn body_swap_preserves_frontmatter_verbatim() {
        let file = "---\nx0k:\n  format: folio/v1\n  id: x0k:design/x\n  type: design\n  status: proposed\n  # a human comment that render_envelope would drop\n  concerns:\n    - ui\n---\nOriginal body paragraph.\n";
        let (_env, body) = crate::colophon::parse_envelope(file).expect("parses");
        // Reconstruct the verbatim prefix exactly as save_decision_body does.
        let prefix = &file[..file.len() - body.len()];
        let rebuilt = format!("{prefix}{body}");
        assert_eq!(
            rebuilt, file,
            "prefix+body must reconstruct the source byte-for-byte"
        );

        // Swapping the body keeps the frontmatter (incl. the comment) intact.
        let new_body = "Revised body paragraph.\n";
        let swapped = format!("{prefix}{new_body}");
        assert!(swapped.contains("# a human comment that render_envelope would drop"));
        assert!(swapped.contains("Revised body paragraph."));
        assert!(!swapped.contains("Original body paragraph."));
    }
}
```

The last test is really a contract for a *different* module's caller: it
demonstrates the prefix-splice technique that lets a save tool replace a
body without ever re-rendering the frontmatter — the promise
[`colophon.md`](colophon.md) ends on. It lives here because the
segmenter's byte ranges are what make per-block splicing possible at all.

## Composing the module

```rust {#root}
<<module-doc>>

<<block-kind>>

<<block-segment>>

<<segment-body>>

<<hash-block>>

<<heading-helpers>>

<<segment-markdown>>

<<segment-html>>

<<tests>>
```

What remains open is the drift case the stability key does not cover:
insert a paragraph at the top of a section and every later same-kind
sibling's ordinal shifts by one, marking their underwritings stale even
though their text is unchanged. That is the conservative failure —
acceptances are never silently migrated, a human just re-confirms — but
it is real friction, and any future scheme that wants to fix it (content
similarity matching, say) has to argue with the recompute-from-file-alone
constraint first.
