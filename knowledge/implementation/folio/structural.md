---
x0k:
  format: folio/v1
  id: x0k:implementation/folio/structural
  type: implementation
  status: draft
  summary: "The parser-agnostic block tree both the markdown and the HTML sides parse into: syntactic, orthogonal to the editorial axis, and owned here so two renderer crates can share it without a dependency cycle."
  concerns: [folio, blocks, rendering, parsing, identity]
  tangle:
    crate: x0k-folio
    root: src/structural_block.rs
  edges:
    implements:
      - x0k:design/body-format-isomorphism
      - x0k:design/in-prose-authoring
    cites:
      - x0k:implementation/folio/segmentation
      - x0k:implementation/folio/html-canonical
    motivated_by:
      - x0k:intent/50de2d40-a0ce-4bb1-98bc-33da47a03c7f
---
# The structural block tree

Two parsers and two renderers meet in the middle of the document
pipeline: markdown and HTML are both parsed into *something*, and that
something is both painted natively and serialized back out. If each pair
invented its own intermediate shape, the body-format-isomorphism laws —
"a heading is the same heading whichever serialization you edit it in" —
would be unstatable, and the markdown-parsing UI crate and the
HTML-parsing viewer crate would need to depend on each other to share
one. This module is the neutral meeting point: a parser-agnostic
**structural block tree**, owned by `x0k-folio` precisely so
`x0k-ui-widgets` and `x0k-doc-viewer` can both consume it without
forming a cargo dependency cycle.

The tree is *syntactic* — headings, paragraphs, lists, code blocks,
quotes, tables. It is deliberately orthogonal to the *editorial* axis
(`DocumentBlock`'s Editable / Reference / Region / Marker — who owns
what, what is AI-overlaid). The two compose rather than merge: an
editorial wrapper can carry structural content as its payload, so the
one document has two independent coordinate systems, each answering its
own kind of question.

The design pressure that shaped the less obvious variants is a single
principle: **the model does not interpret layout, it only refuses to
lose it.** Parse our carried publication manifest and you get the
obvious blocks. Parse a designed HTML body — a page the operator (the
human running the system) built by hand, with `<div class="src">` grids
and `<span class="eyebrow">` accents — and the obvious models both
fail. Descending *through* a layout container hoists its children to
the top level, and the wrapper is simply gone on the next save; when
one such page was first round-tripped through an editor built on that
model, the save silently deleted 82 `<div>`s and 42 grid cells of
layout the author never asked to lose. Capturing the container
*verbatim* as opaque HTML preserves the
markup but makes every word inside it non-editable, which defeats the
point of opening the document. The `Container` and `Raw` variants are
the third way: keep the wrapper as a node, carry its tag and attributes
opaquely, keep the children individually editable.

<a name="chunk-module-doc"></a><sub>[`src/structural_block.rs`](../../../x0k-folio/src/structural_block.rs) · `#module-doc`</sub>

```rust {#module-doc}
//! Canonical structural block tree.
//!
//! A parser-agnostic structural representation of a document's block-level
//! syntax (headings, paragraphs, lists, code blocks, quotes, tagged
//! regions, embedded HTML sections) plus inline-level spans (text,
//! links, images, code, strong, emphasis, marks). Both the markdown parser
//! (`x0k_ui_widgets::util::markdown::parse_markdown_structural`) and the
//! HTML parser (`x0k_doc_viewer::parse_html_structural`) produce this
//! shape; the Native renderer (`x0k_ui_widgets::views::structural_renderer`)
//! and the Blitz serializer (`x0k_doc_viewer::parse_structural_to_html`)
//! consume it.
//!
//! The type lives in this crate (`x0k-folio`) so the markdown parser,
//! HTML parser, Native renderer, and Blitz serializer all converge on a
//! single definition without forming a `x0k-ui-widgets` ↔
//! `x0k-doc-viewer` cargo dependency cycle.
//!
//! This is orthogonal to `DocumentBlock` (the *editorial* axis:
//! Editable / Reference / Region / Marker — who owns what, what's
//! AI-overlaid). The two compose: an editorial wrapper can carry
//! structural content as its payload rather than raw markdown strings.
//!
//! ## BlockId allocation
//!
//! `BlockId` is a content-hash + position composite (`u64`). Stable
//! across re-edits as long as the surrounding content remains stable —
//! AI proposals anchored to a `BlockId` survive re-renders that don't
//! alter the targeted block's content or its position in the document.
//! If a block's text or its index drift, its id changes and proposals
//! need re-anchoring.
//!
//! The hash is the FNV-1a 64-bit hash of the block's kind tag + content
//! (heading text, paragraph spans flattened, code-block body, etc.)
//! mixed with the sibling position. `BlockId(0)` is reserved as the
//! "unset" sentinel and is never produced by the allocator.

use std::hash::Hasher;

// ============================================================================
// Public types
// ============================================================================
```

## The document and its fences

A parsed document is just the block sequence. Frontmatter never enters —
the loader strips the envelope (see [`colophon.md`](colophon.md)) before
the body reaches any parser, so the structural tree has exactly one job.

<a name="chunk-structural-doc"></a><sub>[`src/structural_block.rs`](../../../x0k-folio/src/structural_block.rs) · `#structural-doc`</sub>

```rust {#structural-doc}
/// A parsed document as a sequence of structural blocks.
///
/// Frontmatter is **not** carried here — it stays where it lives today
/// (the loader strips the YAML envelope before handing the body to the
/// parser).
#[derive(Debug, Clone, PartialEq)]
pub struct StructuralDoc {
    pub blocks: Vec<StructuralBlock>,
}

impl StructuralDoc {
    pub fn new(blocks: Vec<StructuralBlock>) -> Self {
        Self { blocks }
    }

    pub fn is_empty(&self) -> bool {
        self.blocks.is_empty()
    }

    pub fn len(&self) -> usize {
        self.blocks.len()
    }
}
```

`FenceInfo` is the isomorphism at its smallest grain: a markdown fence
info string (`yaml x0k:prompt audience=operator`) and an HTML code
element's data attributes must round-trip through each other without
loss or reordering. So the carrier parses *and* normalizes — canonical
token order is language, `x0k:type`, trailing info, with whitespace
collapsed — and both projections build it, one from the raw info string
and one from decomposed attribute parts.

A marker is an instruction, though, and that is a problem for any
document that wants to *teach* the syntax. `yaml x0k:affordance` in a
body is not a mention of the inline-entity grammar, it is a declaration
of an affordance ([`inline-entities.md`](inline-entities.md)), so the
chapter explaining the grammar cannot show it without making a claim it
cannot keep. The tangler hit the same wall one layer over and answered
it with a `!`: a line reading `<<!name>>` is the literal `<<name>>`,
never the reference ([`resolution.md`](../tangle/resolution.md)). The
carrier borrows that answer verbatim. `x0k:!affordance` is an
**illustrative** marker — it parses, normalizes, round-trips, and
renders exactly like the real thing, and it declares nothing.

The escape is spent on the accessor rather than on a flag beside it.
`x0k_type()` is the question nearly every consumer asks, and it answers
`None` for an illustrative fence: a caller that has never heard of the
escape cannot act on one by forgetting to check. Only the two callers
that must see the marker *as written* — the info-string reconstruction
and the HTML attribute — reach for `x0k_marker()`, which keeps the `!`.

<a name="chunk-fence-info"></a><sub>[`src/structural_block.rs`](../../../x0k-folio/src/structural_block.rs) · `#fence-info`</sub>

```rust {#fence-info}
/// Parsed folio fence metadata shared by the markdown and HTML projections.
///
/// The canonical info-string order is `language`, `x0k:type`, then the
/// remaining info. ASCII whitespace is normalized to single spaces, as
/// required by `isomorphism-grammars.md` sections 1.1 and 3.2.
///
/// A marker written `x0k:!type` is **illustrative**: the fence shows the
/// syntax instead of invoking it, exactly as the tangler's `<<!name>>`
/// shows a chunk reference instead of expanding it. [`Self::x0k_type`]
/// reports `None` for such a fence, so consumers refuse it by default;
/// [`Self::x0k_marker`] keeps the `!` so the escape survives every
/// round-trip.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct FenceInfo {
    language: Option<String>,
    /// The marker token as written, `!` included: `affordance` or
    /// `!affordance`. Never carries the `x0k:` prefix.
    x0k_marker: Option<String>,
    info: Option<String>,
}

impl FenceInfo {
    /// Parse and normalize a markdown fence info string.
    pub fn parse(raw: &str) -> Self {
        let mut language = None;
        let mut x0k_marker = None;
        let mut info = Vec::new();

        for (index, token) in raw.split_ascii_whitespace().enumerate() {
            if let Some(value) = token.strip_prefix("x0k:").filter(|value| is_marker(value)) {
                if x0k_marker.is_none() {
                    x0k_marker = Some(value.to_string());
                    continue;
                }
            }
            if index == 0 && x0k_marker.is_none() {
                language = Some(token.to_string());
            } else {
                info.push(token);
            }
        }

        Self {
            language,
            x0k_marker,
            info: (!info.is_empty()).then(|| info.join(" ")),
        }
    }

    /// Build the carrier from HTML attributes or another decomposed source.
    ///
    /// `x0k_type` is the marker as written — `data-x0k-type="!affordance"`
    /// is the HTML spelling of an illustrative fence, and arrives here
    /// with its `!` intact.
    pub fn from_parts(language: Option<&str>, x0k_type: Option<&str>, info: Option<&str>) -> Self {
        let normalize = |value: &str| value.split_ascii_whitespace().collect::<Vec<_>>().join(" ");
        let language = language
            .map(normalize)
            .filter(|value| !value.is_empty());
        let x0k_marker = x0k_type
            .map(|value| value.trim().strip_prefix("x0k:").unwrap_or(value.trim()))
            .and_then(|value| value.split_ascii_whitespace().next())
            .filter(|value| is_marker(value))
            .map(str::to_string);
        let info = info.map(normalize).filter(|value| !value.is_empty());
        Self {
            language,
            x0k_marker,
            info,
        }
    }

    /// The fence's syntax-highlighting language, when present.
    pub fn language(&self) -> Option<&str> {
        self.language.as_deref()
    }

    /// The type this fence **declares**, without the `x0k:` prefix.
    ///
    /// `None` for a fence with no marker *and* for an illustrative one:
    /// `x0k:!affordance` declares nothing, so every consumer that asks
    /// "what does this fence declare?" is answered correctly without
    /// knowing the escape exists.
    pub fn x0k_type(&self) -> Option<&str> {
        self.x0k_marker
            .as_deref()
            .filter(|marker| !marker.starts_with('!'))
    }

    /// The marker token as written, `!` included — the form that goes
    /// back out as `x0k:<marker>` or `data-x0k-type`, and the honest
    /// test for "does this fence carry a marker at all?".
    pub fn x0k_marker(&self) -> Option<&str> {
        self.x0k_marker.as_deref()
    }

    /// Whether the marker is escaped: the fence shows the syntax rather
    /// than invoking it.
    pub fn is_illustrative(&self) -> bool {
        self.x0k_marker
            .as_deref()
            .is_some_and(|marker| marker.starts_with('!'))
    }

    /// Trailing fence metadata carried as `data-x0k-info`.
    pub fn info(&self) -> Option<&str> {
        self.info.as_deref()
    }

    /// Reconstruct the normalized markdown info string.
    pub fn canonical(&self) -> String {
        let mut parts = Vec::new();
        if let Some(language) = self.language.as_deref().filter(|value| !value.is_empty()) {
            parts.push(language.to_string());
        }
        if let Some(marker) = self.x0k_marker() {
            parts.push(format!("x0k:{marker}"));
        }
        if let Some(info) = self.info.as_deref().filter(|value| !value.is_empty()) {
            parts.push(info.to_string());
        }
        parts.join(" ")
    }
}

/// A marker token must name something: neither `x0k:` nor a bare
/// `x0k:!` is a marker, so both fall through to the info string.
fn is_marker(value: &str) -> bool {
    !value.is_empty() && value != "!"
}
```

## Block variants

The first six variants map one-to-one onto pulldown-cmark's block tags;
they need little argument. The interesting ones are the last five, each
the residue of a real failure mode:

- `HtmlSection` — raw HTML kept as a leaf, for regions the renderer
  should not walk into.
- `Tagged` — the x0k fence carrier (`x0k:<type>`), wrapping a
  sub-document or fenced payload.
- `Container` — the refuses-to-lose-layout variant derived above.
- `Transclusion` — content inlined from another document by reference,
  painted as ordinary prose with a quiet provenance affordance so the
  read is seamless but authorship is not laundered (mechanism in
  [`transclusion.md`](transclusion.md)).
- `Table` — GFM tables, cells carrying the same inline vocabulary as a
  paragraph.

<a name="chunk-structural-block"></a><sub>[`src/structural_block.rs`](../../../x0k-folio/src/structural_block.rs) · `#structural-block`</sub>

```rust {#structural-block}
/// Block-level structure: the syntactic skeleton of a document.
///
/// One variant per pulldown-cmark `Tag::*` that produces block content,
/// plus `HtmlSection` for inline HTML and `Tagged` for x0k-specific
/// fence carriers and editorial regions.
#[derive(Debug, Clone, PartialEq)]
pub enum StructuralBlock {
    /// `# H1` .. `###### H6`.
    Heading {
        level: u8,
        content: Vec<InlineSpan>,
        id: BlockId,
    },
    /// `<p>` / a run of prose terminated by a blank line.
    Paragraph {
        content: Vec<InlineSpan>,
        id: BlockId,
    },
    /// ```` ```lang\nbody\n``` ````
    CodeBlock {
        lang: Option<String>,
        body: String,
        id: BlockId,
    },
    /// `- a\n- b\n` (`ordered = false`) or `1. a\n2. b\n` (`ordered = true`).
    /// `tight` distinguishes direct item content from paragraph-wrapped items;
    /// both markdown and HTML preserve that CommonMark distinction.
    /// Items may carry mixed inline + block content (paragraph followed
    /// by a nested list); `StructuralListItem::content` is itself a
    /// `Vec<StructuralBlock>`.
    List {
        ordered: bool,
        tight: bool,
        items: Vec<StructuralListItem>,
        id: BlockId,
    },
    /// `> ...`. May nest (a `Quote` whose content contains another `Quote`).
    Quote {
        content: Vec<StructuralBlock>,
        id: BlockId,
    },
    /// Raw HTML emitted by the source. The HTML-body parser produces this
    /// for `<section class="sketch-*">` regions where the renderer should
    /// treat the section as a leaf (not walk into its children).
    /// `class_scope` records the outer `class=` so themes can hook the
    /// scope; `html` is the inner HTML serialization.
    ///
    /// The markdown parser also emits `HtmlSection` when pulldown-cmark
    /// surfaces a block-level HTML event so HTML embedded in a markdown
    /// body has a place to live in the structural tree.
    HtmlSection {
        html: String,
        class_scope: String,
        id: BlockId,
    },
    /// x0k-specific tagged region wrapping a sub-document or a fenced-code
    /// payload. Both markdown and HTML parsers use this for `x0k:<type>`
    /// fence carriers; editor-authored tagged regions use it more generally.
    Tagged {
        tag: String,
        content: Vec<StructuralBlock>,
        id: BlockId,
    },
    /// A layout container the document author wrote — `<div class="src">`,
    /// `<section>`, `<figure>` — kept as a node so its children stay
    /// individually editable **and** its own tag and attributes survive a
    /// save. Descending into it loses the wrapper on reserialization;
    /// capturing it verbatim makes its contents non-editable.
    ///
    /// `attrs` is the serialized attribute list (`class="src" id="x"`),
    /// carried opaquely: the structural model does not interpret layout, it
    /// only refuses to lose it.
    Container {
        tag: String,
        attrs: String,
        content: Vec<StructuralBlock>,
        id: BlockId,
    },
    /// A transcluded region: the inlined content of another document (or a
    /// section of it), addressed by `source_uri` (a folio URI,
    /// optionally `#section`). `content` is the resolved sub-tree — the
    /// renderer paints it as ordinary prose with a quiet provenance
    /// affordance ("canonically from <source_uri>"), so the read is
    /// seamless but authorship is not laundered. Read-only today.
    ///
    /// The markdown parser emits this for a `x0k:transclude {ref="..."}`
    /// fence; `content` is empty when no resolver has been applied (the
    /// renderer then shows the provenance marker alone, a degraded "go
    /// there" affordance). See `x0k-folio::transclusion`.
    Transclusion {
        /// The reference as authored: `x0k:doc/x` or `x0k:doc/x#section`.
        source_uri: String,
        /// The resolved structural content. Empty when unresolved.
        content: Vec<StructuralBlock>,
        id: BlockId,
    },
    /// A thematic break (`---` in canonical Markdown, `<hr>` in H_md).
    ThematicBreak {
        id: BlockId,
    },
    /// A plain GFM/H_md table. Cells carry the same inline vocabulary as a
    /// paragraph; block content inside a cell remains outside H_md.
    Table {
        alignments: Vec<TableAlignment>,
        head: Vec<Vec<InlineSpan>>,
        rows: Vec<Vec<Vec<InlineSpan>>>,
        id: BlockId,
    },
}

impl StructuralBlock {
    /// Block id getter, for consumers that need to anchor an overlay
    /// (e.g. AI proposal) to a specific structural block.
    pub fn id(&self) -> BlockId {
        match self {
            StructuralBlock::Heading { id, .. }
            | StructuralBlock::Paragraph { id, .. }
            | StructuralBlock::CodeBlock { id, .. }
            | StructuralBlock::List { id, .. }
            | StructuralBlock::Quote { id, .. }
            | StructuralBlock::HtmlSection { id, .. }
            | StructuralBlock::Tagged { id, .. }
            | StructuralBlock::Container { id, .. }
            | StructuralBlock::Transclusion { id, .. }
            | StructuralBlock::ThematicBreak { id }
            | StructuralBlock::Table { id, .. } => *id,
        }
    }
}

/// Column alignment carried by a plain table.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TableAlignment {
    None,
    Left,
    Center,
    Right,
}

/// One item in a `List`. Contains its own block sub-tree so a list
/// item can hold a paragraph followed by a nested list, a code block,
/// etc. — matching pulldown-cmark's tight/loose list semantics.
#[derive(Debug, Clone, PartialEq)]
pub struct StructuralListItem {
    pub content: Vec<StructuralBlock>,
    pub id: BlockId,
}
```

## Inline spans

The inline vocabulary commits to what pulldown-cmark surfaces directly —
text, links, images, inline code, strong, emphasis — plus two variants born
of the same refuse-to-lose principle. A link and an image look alike and
are not: the grammar's H_md allowlist admits `href` on `a` and `src`/`alt`
on `img`, so an image's alt text is an *attribute* — a string, because no
attribute value can hold markup — while a link's label is ordinary inline
*content*. The label is therefore `Vec<InlineSpan>`, exactly as `Strong`
and `Emphasis` carry theirs; flatten it and `<a><code>x</code></a>` comes
back as `<a>x</a>` on the next save. `Raw` is `Container` one level down:
flatten `<span class="note">` to its text and the loss is invisible
until the author saves, at which point the styling hook is gone from
the document. `Mark` is the anchor consumers splice in to overlay a
proposal onto a range; no parser emits it.

Some real syntax has no dedicated variant yet — footnotes and
strikethrough (parsed but rendered as plain text in the tree). The comment
names them rather than hiding them; each is an isomorphism gap a future
variant closes.

<a name="chunk-inline-span"></a><sub>[`src/structural_block.rs`](../../../x0k-folio/src/structural_block.rs) · `#inline-span`</sub>

```rust {#inline-span}
/// Inline-level span: leaves of the structural tree inside a
/// `Heading` or `Paragraph`'s `content`.
///
/// The committed variants are what pulldown-cmark surfaces directly: text,
/// links, images, inline code, strong (bold), emphasis (italic). `Mark` is a
/// placeholder for AI-proposal / annotation overlays the renderer can
/// resolve to highlighted ranges; it's not produced by the
/// markdown parser, only by consumers that splice proposals
/// onto the parsed tree.
///
/// **Not currently mapped to dedicated variants** (flagged as
/// follow-up): footnote references / definitions — pulldown-cmark
/// produces `Event::FootnoteReference` / `Tag::FootnoteDefinition`
/// only when `Options::ENABLE_FOOTNOTES` is set, which the existing
/// parser doesn't enable. Strikethrough (`Tag::Strikethrough`) is
/// enabled in the existing parser but doesn't yet have an `InlineSpan`
/// variant — currently rendered as plain text in the structural tree.
#[derive(Debug, Clone, PartialEq)]
pub enum InlineSpan {
    Text(String),
    /// The H_md anchor carrier. Its label is inline children, not flat
    /// text, so `[**bold**](u)` and `<a><code>x</code></a>` keep the markup
    /// the author wrote; `uri` is the one attribute
    /// body-format-isomorphism section 2.2 admits on `a`.
    Link {
        uri: String,
        content: Vec<InlineSpan>,
    },
    /// The H_md image carrier. Per body-format-isomorphism section 2.2,
    /// only `src` and `alt` cross the structural boundary; `title` and
    /// every other attribute remain complement matter.
    Image {
        uri: String,
        alt: String,
    },
    Code(String),
    Strong(Vec<InlineSpan>),
    Emphasis(Vec<InlineSpan>),
    /// An inline element the author wrote that carries presentation the
    /// model does not otherwise model — `<span class="eyebrow">` or an
    /// `<img>` with a disallowed attribute. Held so a save can rebuild it
    /// verbatim.
    ///
    /// Same rationale as [`StructuralBlock::Container`] one level down:
    /// flattening `<span class="note">` to its text is invisible until the
    /// operator saves, at which point the styling hook is gone from the
    /// document. `void` marks elements with no closing tag (`<br>`,
    /// `<img>`), which must not be emitted as `<br></br>`.
    Raw {
        tag: String,
        attrs: String,
        content: Vec<InlineSpan>,
        void: bool,
    },
    /// AI-proposal / annotation anchor. Not parser-emitted; consumers
    /// splice this in to mark a range as overlaid.
    Mark(MarkerId),
    /// A source-authored hard line break (`\\\n` in canonical Markdown,
    /// `<br>` in H_md).
    HardBreak,
}
```

## Block identity for proposal anchoring

This is the *other* identity scheme in the crate, and the contrast with
[`segmentation.md`](segmentation.md)'s stability key is the fastest way
to understand it. A proposal ("replace this paragraph with that") should
survive re-renders that change nothing — and should *die* the moment the
targeted content or its position changes, because a proposal against
moved-or-edited text is a proposal against text the proposer never saw.
So `BlockId` folds content AND position into one hash: maximum
sensitivity, by design, where the underwriting key — the stability key
a human's acceptance of a block locks to, minted in
[`segmentation.md`](segmentation.md) — wants maximum stability.

<a name="chunk-block-id"></a><sub>[`src/structural_block.rs`](../../../x0k-folio/src/structural_block.rs) · `#block-id`</sub>

```rust {#block-id}
// ============================================================================
// IDs
// ============================================================================

/// Stable-ish identifier for a structural block. See module-level docs.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct BlockId(pub u64);

impl BlockId {
    /// Sentinel id meaning "not yet allocated". Never produced by
    /// `BlockIdAllocator::next`.
    pub const UNSET: BlockId = BlockId(0);
}

/// Identifier for an inline `Mark` overlay. Separate type from
/// `BlockId` so the type system distinguishes block-level anchors
/// from inline-range anchors.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct MarkerId(pub u64);
```

The allocator mixes a content seed with a monotonically increasing
sibling position, position last, so two identical paragraphs at
different positions cannot collide. Zero is rewritten to one on the
astronomically unlikely hash collision, because the type promises the
`UNSET` sentinel is never allocated:

<a name="chunk-allocator"></a><sub>[`src/structural_block.rs`](../../../x0k-folio/src/structural_block.rs) · `#allocator`</sub>

```rust {#allocator}
/// Allocates `BlockId`s during a parser pass. Folds a content seed
/// (kind tag + the block's textual fingerprint) and the sibling
/// position into an FNV-1a hash. Position is mixed in last so two
/// otherwise-identical paragraphs at different positions don't
/// collide.
#[derive(Debug, Default)]
pub struct BlockIdAllocator {
    next_position: u64,
}

impl BlockIdAllocator {
    pub fn new() -> Self {
        Self::default()
    }

    /// Allocate an id for a block whose `kind_tag` (`"heading"`,
    /// `"paragraph"`, etc.) and `seed` (the heading text, the
    /// paragraph's flattened span text, the code body, etc.) feed the
    /// content hash. Position counter increments on every call.
    pub fn next(&mut self, kind_tag: &str, seed: &str) -> BlockId {
        let mut hasher = Fnv1a64::new();
        hasher.write(kind_tag.as_bytes());
        hasher.write(&[0]);
        hasher.write(seed.as_bytes());
        hasher.write(&[0]);
        hasher.write(&self.next_position.to_le_bytes());
        self.next_position = self.next_position.wrapping_add(1);
        let mut h = hasher.finish();
        if h == 0 {
            // Collision with the UNSET sentinel is vanishingly unlikely
            // but the type promises UNSET is never produced.
            h = 1;
        }
        BlockId(h)
    }
}
```

Why FNV-1a, hand-inlined, instead of a hash crate or the standard
library's `DefaultHasher`? The requirements are stable, deterministic,
fast, non-cryptographic — and `DefaultHasher` is explicitly *not* stable
across compiler versions, which disqualifies it for an id we want to
survive cargo rebuilds. Fifteen lines of arithmetic beats a dependency:

<a name="chunk-fnv"></a><sub>[`src/structural_block.rs`](../../../x0k-folio/src/structural_block.rs) · `#fnv`</sub>

```rust {#fnv}
// ============================================================================
// FNV-1a (64-bit) — no external dep
// ============================================================================

/// Tiny inline FNV-1a 64 implementation. Avoid pulling a hash crate
/// for this — the requirements are stable, deterministic, fast, and
/// non-cryptographic; `std::collections::hash_map::DefaultHasher` is
/// explicitly not stable across compiler versions so it's a poor fit
/// for an id allocator we want to survive cargo rebuilds.
struct Fnv1a64 {
    state: u64,
}

impl Fnv1a64 {
    const OFFSET: u64 = 0xcbf2_9ce4_8422_2325;
    const PRIME: u64 = 0x0000_0100_0000_01B3;

    fn new() -> Self {
        Self {
            state: Self::OFFSET,
        }
    }
}

impl Hasher for Fnv1a64 {
    fn write(&mut self, bytes: &[u8]) {
        for b in bytes {
            self.state ^= *b as u64;
            self.state = self.state.wrapping_mul(Self::PRIME);
        }
    }

    fn finish(&self) -> u64 {
        self.state
    }
}
```

## Tests

The tests pin the allocator's contract (deterministic, position-
sensitive, never `UNSET`), the id getter across variants, and the
`FenceInfo` normalization in both directions — raw info string and
decomposed HTML parts converging on one canonical form.

<a name="chunk-tests"></a><sub>[`src/structural_block.rs`](../../../x0k-folio/src/structural_block.rs) · `#tests`</sub>

```rust {#tests}
// ============================================================================
// Tests — shape sanity, allocator determinism
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn block_id_allocator_is_deterministic() {
        let mut a = BlockIdAllocator::new();
        let mut b = BlockIdAllocator::new();
        for (kind, seed) in [("heading", "Title"), ("paragraph", "body"), ("list", "")] {
            assert_eq!(a.next(kind, seed), b.next(kind, seed));
        }
    }

    #[test]
    fn block_id_allocator_distinguishes_position() {
        let mut a = BlockIdAllocator::new();
        let first = a.next("paragraph", "same body");
        let second = a.next("paragraph", "same body");
        assert_ne!(first, second, "same content at different positions");
    }

    #[test]
    fn block_id_never_unset() {
        let mut a = BlockIdAllocator::new();
        // Run a bunch — even if a hash ever lands on zero the
        // allocator must rewrite it.
        for i in 0..256 {
            let id = a.next("paragraph", &format!("seed-{i}"));
            assert_ne!(id, BlockId::UNSET);
        }
    }

    #[test]
    fn structural_block_id_getter_matches_variant() {
        let id = BlockId(42);
        let p = StructuralBlock::Paragraph {
            content: vec![InlineSpan::Text("hi".into())],
            id,
        };
        assert_eq!(p.id(), id);

        let h = StructuralBlock::Heading {
            level: 2,
            content: vec![InlineSpan::Text("Section".into())],
            id: BlockId(7),
        };
        assert_eq!(h.id(), BlockId(7));
    }

    #[test]
    fn fence_info_canonicalizes_type_before_trailing_info() {
        let carrier = FenceInfo::parse("yaml   audience=operator x0k:prompt");
        assert_eq!(carrier.language(), Some("yaml"));
        assert_eq!(carrier.x0k_type(), Some("prompt"));
        assert_eq!(carrier.info(), Some("audience=operator"));
        assert_eq!(carrier.canonical(), "yaml x0k:prompt audience=operator");
    }

    #[test]
    fn fence_info_from_html_parts_normalizes_bare_type_and_spacing() {
        let carrier = FenceInfo::from_parts(
            Some("rust"),
            Some("x0k:affordance"),
            Some("{#chunk}   tangle=src/lib.rs"),
        );
        assert_eq!(
            carrier.canonical(),
            "rust x0k:affordance {#chunk} tangle=src/lib.rs"
        );
    }

    #[test]
    fn illustrative_marker_declares_nothing_and_survives_the_round_trip() {
        let carrier = FenceInfo::parse("yaml x0k:!affordance");
        assert_eq!(carrier.language(), Some("yaml"));
        // The whole point: the question consumers ask comes back empty.
        assert_eq!(carrier.x0k_type(), None);
        assert!(carrier.is_illustrative());
        assert_eq!(carrier.x0k_marker(), Some("!affordance"));
        // A rewrite through the tree must not quietly promote the
        // example into a declaration.
        assert_eq!(carrier.canonical(), "yaml x0k:!affordance");
        assert_eq!(FenceInfo::parse(&carrier.canonical()), carrier);
    }

    #[test]
    fn illustrative_marker_round_trips_through_html_parts() {
        let carrier = FenceInfo::from_parts(Some("yaml"), Some("!affordance"), None);
        assert_eq!(carrier.x0k_type(), None);
        assert_eq!(carrier.x0k_marker(), Some("!affordance"));
        assert_eq!(carrier.canonical(), "yaml x0k:!affordance");
        // `data-x0k-type` may arrive still wearing the `x0k:` prefix.
        assert_eq!(
            FenceInfo::from_parts(Some("yaml"), Some("x0k:!affordance"), None),
            carrier
        );
    }

    #[test]
    fn a_bare_bang_is_not_a_marker() {
        let carrier = FenceInfo::parse("yaml x0k:!");
        assert_eq!(carrier.x0k_marker(), None);
        assert!(!carrier.is_illustrative());
        assert_eq!(carrier.info(), Some("x0k:!"));
    }
}
```

## Composing the module

<a name="chunk-root"></a><sub>[`src/structural_block.rs`](../../../x0k-folio/src/structural_block.rs) · `#root` · assembles [module-doc](#chunk-module-doc) · [structural-doc](#chunk-structural-doc) · [fence-info](#chunk-fence-info) · [structural-block](#chunk-structural-block) · [inline-span](#chunk-inline-span) · [block-id](#chunk-block-id) · [allocator](#chunk-allocator) · [fnv](#chunk-fnv) · [tests](#chunk-tests)</sub>

```rust {#root}
<<module-doc>>

<<structural-doc>>

<<fence-info>>

<<structural-block>>

<<inline-span>>

<<block-id>>

<<allocator>>

<<fnv>>

<<tests>>
```

The tension this module lives with: it sits between two grammars that
are not equal in power. HTML can say things markdown cannot, and every
variant added here to hold designed matter (`Container`, `Raw`,
`HtmlSection`) widens the shared model toward HTML's expressiveness
while keeping markdown bodies untouched. The named gaps — footnotes and
strikethrough — are the current edge of that widening, and
closing each one is a small, testable unit against the isomorphism laws.
