---
x0k:
  format: folio/v1
  id: x0k:implementation/folio/html-canonical
  type: implementation
  status: draft
  summary: "The chokepoint every HTML body passes before it is stored: idempotent normalization, alphabetical attributes, an explicit whitespace policy, and behaviour-bearing markup removed whole."
  concerns: [folio, html, canonicalization, normalization, security]
  tangle:
    crate: x0k-folio
    root: src/html_canonical.rs
  edges:
    implements:
      - x0k:design/body-format-isomorphism
    cites:
      - x0k:implementation/folio/colophon
      - x0k:implementation/folio/structural
      - x0k:implementation/folio/canonical-patch
---
# Canonical HTML: one true serialization

A folio document may carry an HTML body (`body_format: html` in the
envelope — see [`colophon.md`](colophon.md)). The moment HTML enters a
substrate built on text CRDTs and version control, a problem appears
that markdown never had: HTML has *many* serializations of the same
document. `<br>` and `<br/>`, attribute order, whitespace runs between
tags — all render identically and all diff differently. Store HTML
verbatim and every write churns bytes that mean nothing, the Loro
round-trip drifts, and — the sharper edge — a drive-by `<script>` rides
in with the noise.

The cure is a **canonical form**: this module is the single chokepoint
every HTML body passes through before it is written to Loro or the
working tree. `normalize_html` promises four invariants:

- **Idempotence** — `normalize_html(normalize_html(x)) ==
  normalize_html(x)`. This is the CRDT-facing guarantee: a canonical
  body re-normalized is byte-identical, so writes converge instead of
  ping-ponging.
- **Stable attribute order** — alphabetical by qualified name.
- **A whitespace policy** — preserved verbatim inside `<pre>`/`<code>`,
  collapsed to single spaces elsewhere, no leading/trailing whitespace
  inside non-preformatted blocks.
- **Behavior stripping** — `<script>`, `<form>`, `<input>`, `<iframe>`,
  `<object>`, `<embed>` removed whole; `on*` handlers and inline
  `style` dropped; `javascript:` hrefs dropped. A folio body is
  presentational prose. SVG survives untouched-in-structure (it is
  presentational, not behavioral); themes style via class, which is why
  inline `style` is not sacred.

Parsing is html5ever driving an `RcDom`, fragment-mode rooted at
`<body>` so callers can hand over a bare `<p>...</p>` without a full
`<html>` shell. Serialization is hand-rolled: html5ever's own serializer
does not let us reorder attributes, choose void-element syntax, or
apply the whitespace policy, and the canonical form *is* those choices.

On top of the canonical form sits a small mutation vocabulary
(`CanonicalPatch`), because the editing surfaces need to change a
canonical body without ever handing an editor raw serialized tags. The
carried thread continues here: when a themed HTML rendering of a
decision document is edited in place, the edit arrives as "replace this
visible text span" or "set this attribute" against canonical addresses
— never as string surgery on markup.

```rust {#module-doc}
//! Canonical HTML normalizer for folio/v1 HTML bodies.
//!
//! HTML is an opt-in alternate body format on folio/v1 documents
//! (`body_format: html`). Storing HTML verbatim would let formatting
//! noise (attribute order, optional slashes on void elements, whitespace
//! runs, drive-by `<script>` insertions) drift the file across writes and
//! break the Loro round-trip. This module is the single chokepoint: every
//! HTML body is run through `normalize_html` before it is written to
//! Loro / the working tree.
//!
//! `normalize_html` returns a canonical form with these invariants:
//!
//! - **Idempotent**: `normalize_html(normalize_html(x)) == normalize_html(x)`
//!   for all valid HTML inputs.
//! - **Stable attribute order**: attributes on each element are sorted
//!   alphabetically by qualified name.
//! - **Whitespace policy**: preserved inside `<pre>` and `<code>`;
//!   collapsed to a single space between text-flow elements elsewhere; no
//!   leading or trailing whitespace inside non-pre blocks.
//! - **Void elements**: written without trailing slash
//!   (`<br>`, not `<br/>`). Standard HTML void element set.
//! - **Behavior stripping**: `<script>`, `<form>`, `<input>`, `<iframe>`,
//!   `<object>`, `<embed>` are removed (element and contents). `on*`
//!   event-handler attributes are dropped. Inline `style` attributes are
//!   dropped (themes apply CSS via class). `href` values starting with
//!   `javascript:` are dropped.
//! - **SVG preserved**: `<svg>` and its descendants are presentational,
//!   not behavioral; their content is normalized (whitespace, attribute
//!   order) but no elements are stripped.

use html5ever::driver::{parse_fragment, ParseOpts};
use html5ever::tendril::TendrilSink;
use html5ever::tree_builder::TreeBuilderOpts;
use html5ever::{ns, LocalName, QualName};
use markup5ever_rcdom::{Handle, NodeData, RcDom};
use std::collections::{BTreeMap, HashSet};

pub use crate::canonical_patch::{
    CanonicalHtmlPatch, CanonicalPatch, CanonicalPatchError, CanonicalTextPoint,
};
```

The three policy sets, as data. Keeping them as `const` slices (rather
than inline matches) makes the security surface reviewable at a glance:

```rust {#policy-sets}
/// Elements removed entirely (open tag, content, close tag) on normalize.
/// These carry behavior or external resource semantics inappropriate for a
/// folio body.
const STRIPPED_ELEMENTS: &[&str] = &["script", "form", "input", "iframe", "object", "embed"];

/// HTML void elements (per the WHATWG spec). Emitted as `<tag ...>` with
/// no trailing slash and no closing tag.
const VOID_ELEMENTS: &[&str] = &[
    "area", "base", "br", "col", "embed", "hr", "img", "input", "keygen", "link", "meta", "param",
    "source", "track", "wbr",
];

/// Tags inside which whitespace is preserved verbatim (no collapse, no
/// trim). Standard HTML pre-formatted contexts.
const PRESERVE_WHITESPACE_ELEMENTS: &[&str] = &["pre", "code"];

/// Normalize an HTML body string into the canonical form documented at the
/// module level. Idempotent and self-contained — does not consult the
/// envelope or filesystem.
pub fn normalize_html(input: &str) -> String {
    let dom = parse_body_fragment(input);

    serialize_body_fragment(&dom, input.len())
}
```

## Addressing into a canonical body

Editing needs coordinates, and the coordinates are not this module's to
define. The patch vocabulary — `CanonicalPatch`, its `CanonicalTextPoint`
addresses, and `CanonicalPatchError` — lives in
[`canonical-patch.md`](canonical-patch.md), because the same grammar
spans markdown bodies: an editor that says "replace the visible text of
`/p[1]/strong[1]`" must not care which dialect the body is stored in. A
point names an element by a rooted path of `tag[index]` segments
(`/article[1]/p[2]`), a run by ordinal among that element's direct text
children, and a position by byte offset into the decoded run. The
addresses are only meaningful against the canonical serialization —
which is exactly why every public function below re-normalizes its
input before resolving them.

This module re-exports the vocabulary under its own name so HTML-side
callers (the renderer-lockstep overlay) see one coherent surface;
`CanonicalHtmlPatch` is the alias those callers spell.

## The patch API

`apply_canonical_patches` normalizes, parses, applies attribute patches
first (they cannot invalidate text addresses), then text patches, and
re-serializes canonically — so the output of an edit is by construction
in canonical form, and the attribute filter runs on patched values too
(you cannot patch `onclick` back in through the API).

```rust {#apply-patches}
/// Apply canonical HTML patches, attributes first, and return a canonical
/// serialization of the same DOM structure.
pub fn apply_canonical_patches(
    input: &str,
    patches: &[CanonicalPatch],
) -> Result<String, CanonicalPatchError> {
    let canonical = normalize_html(input);
    let dom = parse_body_fragment(&canonical);

    for patch in patches
        .iter()
        .filter(|patch| matches!(patch, CanonicalPatch::SetAttribute { .. }))
    {
        apply_attribute_patch(&dom, patch)?;
    }
    for patch in patches
        .iter()
        .filter(|patch| matches!(patch, CanonicalPatch::ReplaceVisibleText { .. }))
    {
        apply_text_patch(&dom, patch)?;
    }

    Ok(serialize_body_fragment(&dom, canonical.len()))
}

/// Return the decoded contents of one canonical direct text run.
pub fn canonical_text_run(
    input: &str,
    point: &CanonicalTextPoint,
) -> Result<String, CanonicalPatchError> {
    let dom = parse_body_fragment(&normalize_html(input));
    let runs = canonical_text_runs(&dom);
    let index = find_text_run_index(&runs, point)?;
    Ok(text_run_contents(&runs[index]))
}

/// Serialize only the canonical children of an element.
///
/// Hosts use this to remove deterministic rendering chrome without parsing
/// HTML as text. The returned fragment follows the same policy as
/// [`normalize_html`].
pub fn canonical_element_inner_html(
    input: &str,
    element: &str,
) -> Result<String, CanonicalPatchError> {
    let canonical = normalize_html(input);
    let dom = parse_body_fragment(&canonical);
    let node = find_element(&dom, element)
        .ok_or_else(|| CanonicalPatchError::ElementNotFound(element.to_string()))?;
    let stripped: HashSet<&'static str> = STRIPPED_ELEMENTS.iter().copied().collect();
    let void: HashSet<&'static str> = VOID_ELEMENTS.iter().copied().collect();
    let preserve: HashSet<&'static str> = PRESERVE_WHITESPACE_ELEMENTS.iter().copied().collect();
    let ctx = SerializeCtx {
        stripped: &stripped,
        void: &void,
        preserve: &preserve,
    };
    let mut out = String::with_capacity(canonical.len());
    for child in node.children.borrow().iter() {
        serialize_inline(&mut out, child, &ctx, false, None);
    }
    while out.ends_with(' ') || out.ends_with('\t') {
        out.pop();
    }
    Ok(out)
}

/// Put two canonical text points in document order and validate their UTF-8
/// offsets against the current canonical body.
pub fn order_canonical_text_points(
    input: &str,
    a: &CanonicalTextPoint,
    b: &CanonicalTextPoint,
) -> Result<(CanonicalTextPoint, CanonicalTextPoint), CanonicalPatchError> {
    let dom = parse_body_fragment(&normalize_html(input));
    let runs = canonical_text_runs(&dom);
    let a_index = find_text_run_index(&runs, a)?;
    let b_index = find_text_run_index(&runs, b)?;
    validate_text_offset(&runs[a_index], a.byte_offset)?;
    validate_text_offset(&runs[b_index], b.byte_offset)?;
    if (a_index, a.byte_offset) <= (b_index, b.byte_offset) {
        Ok((a.clone(), b.clone()))
    } else {
        Ok((b.clone(), a.clone()))
    }
}

/// Read decoded visible text across a canonical span without exposing tag
/// serialization to the caller.
pub fn canonical_text_in_range(
    input: &str,
    a: &CanonicalTextPoint,
    b: &CanonicalTextPoint,
) -> Result<String, CanonicalPatchError> {
    let dom = parse_body_fragment(&normalize_html(input));
    let runs = canonical_text_runs(&dom);
    let a_index = find_text_run_index(&runs, a)?;
    let b_index = find_text_run_index(&runs, b)?;
    validate_text_offset(&runs[a_index], a.byte_offset)?;
    validate_text_offset(&runs[b_index], b.byte_offset)?;
    let ((start_index, start_offset), (end_index, end_offset)) =
        if (a_index, a.byte_offset) <= (b_index, b.byte_offset) {
            ((a_index, a.byte_offset), (b_index, b.byte_offset))
        } else {
            ((b_index, b.byte_offset), (a_index, a.byte_offset))
        };
    if start_index == end_index {
        return Ok(text_run_contents(&runs[start_index])[start_offset..end_offset].to_string());
    }
    let mut text = String::new();
    text.push_str(&text_run_contents(&runs[start_index])[start_offset..]);
    for run in &runs[start_index + 1..end_index] {
        text.push_str(&text_run_contents(run));
    }
    text.push_str(&text_run_contents(&runs[end_index])[..end_offset]);
    Ok(text)
}
```

## Parsing and walking

The fragment parse and the two traversals that back the address scheme.
`canonical_text_runs` walks the element tree minting `tag[index]` paths
(1-based, per-tag sibling counting) and collecting every element's
direct text children in document order; `find_element` walks a path back
down. The two must agree on the path grammar — they are the encoder and
decoder of the same little language.

```rust {#parse-and-walk}
fn parse_body_fragment(input: &str) -> RcDom {
    let opts = ParseOpts {
        tree_builder: TreeBuilderOpts {
            drop_doctype: true,
            ..Default::default()
        },
        ..Default::default()
    };
    // Parse as a fragment under <body>. This accepts bare body content
    // (`<p>hi</p>`) as well as full documents (`<html>...</html>`).
    let context = QualName::new(None, ns!(html), LocalName::from("body"));
    parse_fragment(RcDom::default(), opts, context, vec![], false).one(input)
}

fn serialize_body_fragment(dom: &RcDom, capacity: usize) -> String {
    let mut out = String::with_capacity(capacity);
    // The fragment-parser wraps content in `<html>`; descend into the
    // <html> root and serialize its children.
    let root = &dom.document;
    let children = root.children.borrow();
    let stripped: HashSet<&'static str> = STRIPPED_ELEMENTS.iter().copied().collect();
    let void: HashSet<&'static str> = VOID_ELEMENTS.iter().copied().collect();
    let preserve: HashSet<&'static str> = PRESERVE_WHITESPACE_ELEMENTS.iter().copied().collect();
    let ctx = SerializeCtx {
        stripped: &stripped,
        void: &void,
        preserve: &preserve,
    };
    for child in children.iter() {
        // The fragment root contains a single <html> node with one
        // <body>-or-similar child holding the actual fragment content.
        serialize_root_child(&mut out, child, &ctx);
    }
    out
}

#[derive(Clone)]
struct CanonicalTextRun {
    point: CanonicalTextPoint,
    handle: Handle,
}

fn canonical_text_runs(dom: &RcDom) -> Vec<CanonicalTextRun> {
    let mut runs = Vec::new();
    for child in dom.document.children.borrow().iter() {
        if matches!(&child.data, NodeData::Element { name, .. } if name.local.as_ref() == "html") {
            collect_element_text_runs(child, "", &mut runs);
        }
    }
    runs
}

fn collect_element_text_runs(node: &Handle, parent_path: &str, out: &mut Vec<CanonicalTextRun>) {
    let children = node.children.borrow();
    let mut same_tag_index: BTreeMap<String, usize> = BTreeMap::new();
    for child in children.iter() {
        let NodeData::Element { name, .. } = &child.data else {
            continue;
        };
        let tag = name.local.as_ref().to_string();
        let index = same_tag_index.entry(tag.clone()).or_default();
        *index += 1;
        let segment = format!("{tag}[{}]", *index);
        let path = if parent_path.is_empty() {
            format!("/{segment}")
        } else {
            format!("{parent_path}/{segment}")
        };

        let mut text_run = 0usize;
        for grandchild in child.children.borrow().iter() {
            if matches!(grandchild.data, NodeData::Text { .. }) {
                out.push(CanonicalTextRun {
                    point: CanonicalTextPoint::new(&path, text_run, 0),
                    handle: grandchild.clone(),
                });
                text_run += 1;
            }
        }
        collect_element_text_runs(child, &path, out);
    }
}

fn find_element(dom: &RcDom, wanted: &str) -> Option<Handle> {
    if !wanted.starts_with('/') {
        return None;
    }
    let mut current = dom
        .document
        .children
        .borrow()
        .iter()
        .find(|node| matches!(&node.data, NodeData::Element { name, .. } if name.local.as_ref() == "html"))?
        .clone();

    for raw in wanted.trim_start_matches('/').split('/') {
        let (tag, index) = parse_canonical_segment(raw)?;
        let next = current
            .children
            .borrow()
            .iter()
            .filter(|child| matches!(&child.data, NodeData::Element { name, .. } if name.local.as_ref() == tag))
            .nth(index - 1)?
            .clone();
        current = next;
    }
    Some(current)
}

fn parse_canonical_segment(segment: &str) -> Option<(&str, usize)> {
    let open = segment.rfind('[')?;
    if open == 0 || !segment.ends_with(']') {
        return None;
    }
    let index = segment[open + 1..segment.len() - 1].parse().ok()?;
    (index > 0).then_some((&segment[..open], index))
}
```

## Applying patches

Attribute patching validates the name (no whitespace or quote-breaking
characters), removes any existing attribute of that name, and re-adds
through the same filter the serializer uses — a `SetAttribute` of
`style` or `onclick` is silently a removal. Text patching resolves both
endpoints, normalizes their order, and splices: one run in the simple
case; for a span crossing runs, the start run keeps its head plus the
replacement, interior runs empty, the end run keeps its tail.

```rust {#apply-patch-impls}
fn apply_attribute_patch(dom: &RcDom, patch: &CanonicalPatch) -> Result<(), CanonicalPatchError> {
    let CanonicalPatch::SetAttribute {
        element,
        name,
        value,
    } = patch
    else {
        return Ok(());
    };
    if name.is_empty()
        || name
            .chars()
            .any(|ch| ch.is_ascii_whitespace() || matches!(ch, '"' | '\'' | '<' | '>' | '='))
    {
        return Err(CanonicalPatchError::InvalidAttributeName(name.clone()));
    }
    let node = find_element(dom, element)
        .ok_or_else(|| CanonicalPatchError::ElementNotFound(element.clone()))?;
    let NodeData::Element { attrs, .. } = &node.data else {
        return Err(CanonicalPatchError::ElementNotFound(element.clone()));
    };
    let mut attrs = attrs.borrow_mut();
    attrs.retain(|attr| qualified_attr_name(attr) != *name);
    if let Some(value) = value {
        if attribute_passes_filter(&name.to_ascii_lowercase(), value) {
            attrs.push(html5ever::Attribute {
                name: QualName::new(None, ns!(), LocalName::from(name.as_str())),
                value: value.as_str().into(),
            });
        }
    }
    Ok(())
}

fn apply_text_patch(dom: &RcDom, patch: &CanonicalPatch) -> Result<(), CanonicalPatchError> {
    let CanonicalPatch::ReplaceVisibleText {
        start,
        end,
        replacement,
    } = patch
    else {
        return Ok(());
    };
    let runs = canonical_text_runs(dom);
    let mut start = start;
    let mut end = end;
    let mut start_index = find_text_run_index(&runs, start)?;
    let mut end_index = find_text_run_index(&runs, end)?;
    validate_text_offset(&runs[start_index], start.byte_offset)?;
    validate_text_offset(&runs[end_index], end.byte_offset)?;
    if (start_index, start.byte_offset) > (end_index, end.byte_offset) {
        std::mem::swap(&mut start, &mut end);
        std::mem::swap(&mut start_index, &mut end_index);
    }

    if start_index == end_index {
        replace_text_slice(
            &runs[start_index],
            start.byte_offset,
            end.byte_offset,
            replacement,
        );
        return Ok(());
    }

    let start_len = text_run_contents(&runs[start_index]).len();
    replace_text_slice(
        &runs[start_index],
        start.byte_offset,
        start_len,
        replacement,
    );
    for run in &runs[start_index + 1..end_index] {
        replace_text_slice(run, 0, text_run_contents(run).len(), "");
    }
    replace_text_slice(&runs[end_index], 0, end.byte_offset, "");
    Ok(())
}

fn find_text_run_index(
    runs: &[CanonicalTextRun],
    point: &CanonicalTextPoint,
) -> Result<usize, CanonicalPatchError> {
    runs.iter()
        .position(|run| run.point.element == point.element && run.point.text_run == point.text_run)
        .ok_or_else(|| CanonicalPatchError::TextRunNotFound {
            element: point.element.clone(),
            text_run: point.text_run,
        })
}

fn validate_text_offset(
    run: &CanonicalTextRun,
    byte_offset: usize,
) -> Result<(), CanonicalPatchError> {
    let contents = text_run_contents(run);
    if byte_offset <= contents.len() && contents.is_char_boundary(byte_offset) {
        Ok(())
    } else {
        Err(CanonicalPatchError::InvalidTextOffset {
            element: run.point.element.clone(),
            text_run: run.point.text_run,
            byte_offset,
        })
    }
}

fn text_run_contents(run: &CanonicalTextRun) -> String {
    match &run.handle.data {
        NodeData::Text { contents } => contents.borrow().to_string(),
        _ => String::new(),
    }
}

fn replace_text_slice(run: &CanonicalTextRun, start: usize, end: usize, replacement: &str) {
    let NodeData::Text { contents } = &run.handle.data else {
        return;
    };
    let mut next = contents.borrow().to_string();
    next.replace_range(start..end, replacement);
    *contents.borrow_mut() = next.into();
}
```

## The serializer

The custom serializer is where the canonical form is actually decided,
and it is a recursive walk with two pieces of inherited context: the
policy sets and the preserve-whitespace flag that flips on at
`<pre>`/`<code>`. Comments, doctypes, and processing instructions are
dropped — the body is canonical prose, not a place for embedded
annotations. For a non-preserve element, the trick for edge-trimming is
positional: capture the output length before the children, emit, then
trim the slice in place.

```rust {#serializer}
struct SerializeCtx<'a> {
    stripped: &'a HashSet<&'static str>,
    void: &'a HashSet<&'static str>,
    preserve: &'a HashSet<&'static str>,
}

fn serialize_root_child(out: &mut String, node: &Handle, ctx: &SerializeCtx) {
    match &node.data {
        NodeData::Element { name, .. } if name.local.as_ref() == "html" => {
            // Descend into the fragment wrapper; the wrapper itself isn't
            // emitted.
            for child in node.children.borrow().iter() {
                serialize_inline(out, child, ctx, false, None);
            }
        }
        _ => {
            serialize_inline(out, node, ctx, false, None);
        }
    }
    // Trim any trailing whitespace at the top level so the canonical form
    // doesn't carry stray spaces. Inner whitespace collapse handles the
    // inside; this only matters when the input ends with a space-textnode.
    while out.ends_with(' ') || out.ends_with('\t') {
        out.pop();
    }
}

/// Serialize a node. `preserve_ws` is the inherited "are we inside a
/// `<pre>` / `<code>`" flag. `prev_char` is the last character already
/// emitted to `out` inside the current collapse window — used so a
/// space-collapse boundary doesn't insert a leading space at the start
/// of a block.
fn serialize_inline(
    out: &mut String,
    node: &Handle,
    ctx: &SerializeCtx,
    preserve_ws: bool,
    _prev_char: Option<char>,
) {
    match &node.data {
        NodeData::Document => {
            for child in node.children.borrow().iter() {
                serialize_inline(out, child, ctx, preserve_ws, None);
            }
        }
        NodeData::Doctype { .. } => {
            // Doctypes are dropped — body-fragment normalization is
            // doctype-free by design.
        }
        NodeData::Comment { .. } => {
            // Strip HTML comments. They contribute neither rendered
            // content nor structure; the body is canonical prose, not a
            // place for embedded annotations.
        }
        NodeData::Text { contents } => {
            let raw = contents.borrow();
            if preserve_ws {
                out.push_str(&raw);
            } else {
                append_collapsed_text(out, &raw);
            }
        }
        NodeData::Element { name, attrs, .. } => {
            let tag_local: &str = name.local.as_ref();

            if ctx.stripped.contains(tag_local) {
                // Drop the element and all of its content.
                return;
            }

            let next_preserve = preserve_ws || ctx.preserve.contains(tag_local);
            let is_void = ctx.void.contains(tag_local);

            // Write opening tag with filtered + alphabetized attributes.
            out.push('<');
            out.push_str(tag_local);
            let attrs_borrowed = attrs.borrow();
            let mut keep: Vec<(String, String)> = Vec::with_capacity(attrs_borrowed.len());
            for attr in attrs_borrowed.iter() {
                let key = qualified_attr_name(attr);
                let lower = key.to_ascii_lowercase();
                if !attribute_passes_filter(&lower, &attr.value) {
                    continue;
                }
                keep.push((key, attr.value.to_string()));
            }
            // Stable alphabetical order on qualified name.
            keep.sort_by(|(a, _), (b, _)| a.cmp(b));
            for (k, v) in keep {
                out.push(' ');
                out.push_str(&k);
                out.push('=');
                out.push('"');
                out.push_str(&escape_attr_value(&v));
                out.push('"');
            }
            out.push('>');

            if is_void {
                // Void elements have no children and no close tag.
                return;
            }

            // Children
            if next_preserve {
                for child in node.children.borrow().iter() {
                    serialize_inline(out, child, ctx, true, None);
                }
            } else {
                // Non-preserve: trim leading/trailing whitespace in the
                // contents of this element. We do that by capturing the
                // output position before children, emitting normally, and
                // then trimming the slice in place.
                let start = out.len();
                for child in node.children.borrow().iter() {
                    serialize_inline(out, child, ctx, false, None);
                }
                trim_block_edges(out, start);
            }

            // Closing tag
            out.push_str("</");
            out.push_str(tag_local);
            out.push('>');
        }
        NodeData::ProcessingInstruction { .. } => {
            // Processing instructions don't have a meaningful place in
            // canonical body HTML.
        }
    }
}
```

The text-level helpers carry the whitespace and escaping policies. Note
the escaping asymmetry, straight from the HTML spec: text context
escapes `<`, `>`, `&`; attribute-value context (double-quoted) needs
only `&` and `"`:

```rust {#text-helpers}
/// Append `raw` to `out`, collapsing runs of whitespace (ASCII whitespace
/// per HTML spec) into a single space. A leading space is suppressed if
/// `out` already ends with whitespace (or is empty / ends at a tag close
/// that came from a block edge).
fn append_collapsed_text(out: &mut String, raw: &str) {
    let mut prev_is_space = out.ends_with(' ') || out.is_empty() || out.ends_with('>');
    for ch in raw.chars() {
        if ch == ' ' || ch == '\n' || ch == '\r' || ch == '\t' {
            if !prev_is_space {
                out.push(' ');
                prev_is_space = true;
            }
        } else {
            // Escape text-context special characters per HTML spec.
            match ch {
                '<' => out.push_str("&lt;"),
                '>' => out.push_str("&gt;"),
                '&' => out.push_str("&amp;"),
                _ => out.push(ch),
            }
            prev_is_space = false;
        }
    }
}

/// Trim whitespace at the start and end of the slice `out[start..]`. Used to
/// remove leading/trailing whitespace inside a non-preserve element after
/// its children have been emitted.
fn trim_block_edges(out: &mut String, start: usize) {
    // Trim trailing spaces from the back.
    while out.len() > start && (out.ends_with(' ') || out.ends_with('\t')) {
        out.pop();
    }
    // Trim leading spaces from the front. We only need to consider a
    // single leading space because the collapser never emits more than
    // one consecutive whitespace.
    let inner = &out[start..];
    if inner.starts_with(' ') {
        out.remove(start);
    }
}

/// Return the qualified attribute name as it should appear in the
/// serialized output. For non-namespaced attributes this is just the local
/// name. For namespaced attributes (xml:, xlink:, xmlns:) the prefix is
/// preserved so SVG content round-trips.
fn qualified_attr_name(attr: &html5ever::Attribute) -> String {
    let local: &str = attr.name.local.as_ref();
    match attr.name.prefix.as_ref().map(|p| p.as_ref()) {
        Some(prefix) if !prefix.is_empty() => format!("{prefix}:{local}"),
        _ => local.to_string(),
    }
}

/// Decide whether an attribute survives normalization. Drops:
/// - `on*` event handlers.
/// - `style` (inline styles; themes apply CSS via class).
/// - `href` with a `javascript:` scheme value.
fn attribute_passes_filter(lower_key: &str, value: &str) -> bool {
    if lower_key.starts_with("on") {
        return false;
    }
    if lower_key == "style" {
        return false;
    }
    if lower_key == "href" {
        let trimmed = value.trim_start();
        if trimmed.to_ascii_lowercase().starts_with("javascript:") {
            return false;
        }
    }
    true
}

/// Escape a string for safe inclusion as a double-quoted attribute value.
/// HTML attribute escaping is narrower than text-context escaping; only
/// `&` and `"` need rewriting.
fn escape_attr_value(value: &str) -> String {
    let mut out = String::with_capacity(value.len());
    for ch in value.chars() {
        match ch {
            '&' => out.push_str("&amp;"),
            '"' => out.push_str("&quot;"),
            _ => out.push(ch),
        }
    }
    out
}
```

The `attribute_passes_filter` blocklist deserves its candor: `on*`
catches every event handler but also any future attribute that happens
to start with those letters, and the `javascript:` check does not chase
encodings (`&#106;avascript:` and friends are neutralized earlier
because html5ever decodes entities at parse time, but the filter's
correctness rests on that parse-time behavior, not on the string check
alone). It is a normalizer for cooperating documents with a safety
margin — not a standalone HTML sanitizer, and nothing downstream treats
it as one.

## Tests

The battery leans hardest on idempotence — the invariant everything else
rides on — including a combined stress case exercising reordering, void
shapes, whitespace, and stripping in one input. The rest pin each policy
line by line, plus the inner-HTML extraction round-trip.

```rust {#tests}
#[cfg(test)]
mod tests {
    use super::*;

    fn norm(s: &str) -> String {
        normalize_html(s)
    }

    #[test]
    fn idempotent_on_simple_paragraph() {
        let once = norm("<p>hi</p>");
        let twice = norm(&once);
        assert_eq!(once, twice);
    }

    #[test]
    fn idempotent_on_attribute_reorder() {
        // First pass alphabetizes; second pass should preserve order.
        let once = norm("<a id=\"x\" class=\"foo\" href=\"/path\">link</a>");
        let twice = norm(&once);
        assert_eq!(once, twice);
        // Confirm the order is alphabetical: class < href < id
        let class_pos = once.find("class=").expect("class attr");
        let href_pos = once.find("href=").expect("href attr");
        let id_pos = once.find("id=").expect("id attr");
        assert!(class_pos < href_pos && href_pos < id_pos, "got: {once}");
    }

    #[test]
    fn void_element_no_trailing_slash() {
        let out = norm("<p>line<br/>break</p>");
        assert!(out.contains("<br>"));
        assert!(!out.contains("<br/>"));
        assert!(!out.contains("</br>"));
        // And both shapes converge.
        let alt = norm("<p>line<br>break</p>");
        assert_eq!(out, alt);
    }

    #[test]
    fn whitespace_preserved_inside_pre() {
        let raw = "<pre>line one\n  line two\n    line three</pre>";
        let out = norm(raw);
        assert!(out.contains("line one\n  line two\n    line three"));
    }

    #[test]
    fn whitespace_preserved_inside_code() {
        let raw = "<code>  spaced  </code>";
        let out = norm(raw);
        assert!(out.contains("<code>  spaced  </code>"));
    }

    #[test]
    fn whitespace_collapsed_outside_pre() {
        let out = norm("<p>a    b\n\n  c</p>");
        assert!(out.contains("<p>a b c</p>"), "got: {out}");
    }

    #[test]
    fn script_element_stripped() {
        let out = norm("<p>before</p><script>alert('xss')</script><p>after</p>");
        assert!(!out.contains("script"));
        assert!(!out.contains("alert"));
        assert!(out.contains("<p>before</p>"));
        assert!(out.contains("<p>after</p>"));
    }

    #[test]
    fn form_and_iframe_stripped() {
        let out = norm("<form action=\"x\"><input/></form><iframe src=\"y\"></iframe><p>kept</p>");
        assert!(!out.contains("form"));
        assert!(!out.contains("iframe"));
        assert!(!out.contains("input"));
        assert!(out.contains("<p>kept</p>"));
    }

    #[test]
    fn object_and_embed_stripped() {
        let out = norm("<object data=\"x\"></object><embed src=\"y\"><p>kept</p>");
        assert!(!out.contains("object"));
        assert!(!out.contains("embed"));
        assert!(out.contains("<p>kept</p>"));
    }

    #[test]
    fn on_event_attributes_stripped_visual_content_intact() {
        let out = norm("<p onclick=\"alert(1)\" onmouseover=\"f()\">hi</p>");
        assert!(!out.contains("onclick"));
        assert!(!out.contains("onmouseover"));
        assert!(!out.contains("alert"));
        assert!(out.contains("<p>hi</p>"));
    }

    #[test]
    fn style_attribute_stripped_visual_content_intact() {
        let out = norm("<p style=\"color: red\">hi</p>");
        assert!(!out.contains("style="));
        assert!(!out.contains("color: red"));
        assert!(out.contains("<p>hi</p>"));
    }

    #[test]
    fn javascript_href_stripped() {
        let out = norm("<a href=\"javascript:alert(1)\">click</a>");
        assert!(!out.contains("javascript:"));
        // The anchor element itself is kept; only the href is dropped.
        assert!(out.contains("<a"));
        assert!(out.contains(">click</a>"));
    }

    #[test]
    fn normal_href_preserved() {
        let out = norm("<a href=\"/path/to/page\">link</a>");
        assert!(out.contains("href=\"/path/to/page\""));
    }

    #[test]
    fn svg_preserved() {
        // SVG is presentational, not behavioral. The element and its
        // children survive normalization.
        let out = norm(r#"<svg viewBox="0 0 10 10"><circle cx="5" cy="5" r="3"></circle></svg>"#);
        assert!(out.contains("svg"));
        assert!(out.contains("circle"));
        assert!(out.contains("viewBox"));
    }

    #[test]
    fn nested_elements_idempotent() {
        let raw = r#"<div class="z" id="a"><p>one</p><p>two<br/>three</p></div>"#;
        let once = norm(raw);
        let twice = norm(&once);
        assert_eq!(once, twice);
    }

    #[test]
    fn full_document_idempotency_battery() {
        // Combined: reordering, void shape, whitespace, behavior stripping
        // — running normalize twice gives the same output.
        let raw = r#"
            <div id="z" class="a">
              <p style="color:red" onclick="x()">  hello   world  </p>
              <pre>  preserved
   indented </pre>
              <br/>
              <a href="javascript:hack()">bad</a>
              <a class="b" href="/ok" id="a">good</a>
              <script>alert(1)</script>
              <form action="x"><input name="y"/></form>
              <svg><circle r="1"></circle></svg>
            </div>
        "#;
        let once = norm(raw);
        let twice = norm(&once);
        assert_eq!(
            once, twice,
            "second pass differed:\nfirst: {once}\nsecond: {twice}"
        );
        // Spot-check invariants.
        assert!(!once.contains("script"));
        assert!(!once.contains("javascript:"));
        assert!(!once.contains("onclick"));
        assert!(!once.contains("style="));
        assert!(once.contains("svg"));
        assert!(once.contains("<br>"));
        assert!(!once.contains("<br/>"));
        // Pre content preserved.
        assert!(once.contains("  preserved\n   indented "));
    }

    #[test]
    fn empty_input_yields_empty_output() {
        assert_eq!(norm(""), "");
    }

    #[test]
    fn plain_text_input_passes_through_collapsed() {
        let out = norm("just   text   here");
        assert_eq!(out.trim(), "just text here");
    }

    #[test]
    fn entity_text_round_trip_escapes_ampersand() {
        // Input has a literal `&` in text — should be escaped to `&amp;`.
        let once = norm("<p>A & B</p>");
        assert!(once.contains("&amp;"));
        let twice = norm(&once);
        assert_eq!(once, twice, "entity escaping must be idempotent");
    }

    #[test]
    fn element_inner_html_preserves_canonical_children() {
        let input = r#"<article data-owner="host"><p class="lead">one <em>two</em></p><p>three</p></article>"#;
        let article = "/article[1]";

        let inner = canonical_element_inner_html(input, article)
            .expect("canonical article should have extractable children");

        assert_eq!(inner, r#"<p class="lead">one <em>two</em></p><p>three</p>"#);
        assert_eq!(norm(&inner), inner);
    }
}
```

## Composing the module

```rust {#root}
<<module-doc>>

<<policy-sets>>

<<apply-patches>>

<<parse-and-walk>>

<<apply-patch-impls>>

<<serializer>>

<<text-helpers>>

<<tests>>
```

The genuinely hard part of this module is not any one function — it is
that "canonical" is a *social* contract between four parties: this
serializer, the Loro diff path, the structural parsers, and the themes.
Idempotence makes the contract checkable from inside, and the
idempotency battery is the strongest test in the crate for that reason.
What no test here can check is the fourth party — whether a theme
depends on a serialization detail (say, attribute order) beyond the
promised invariants. The invariants list at the top of the module is
therefore the interface; anything not promised there is free to change.
