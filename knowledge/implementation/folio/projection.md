---
x0k:
  format: folio/v1
  id: x0k:implementation/folio/projection
  type: implementation
  status: draft
  summary: The Loro round trip — document projected to a file, a human's file edit parsed back into ops — behind the `plugins` feature; the one chapter whose module a standalone build never compiles.
  concerns: [folio, projection, loro, plugins, materialization]
  tangle:
    crate: x0k-folio
    root: src/projection.rs
  edges:
    implements:
      - x0k:design/body-format-isomorphism
    cites:
      - x0k:architecture/filesystem-graph-materialization
      - x0k:implementation/folio/colophon
      - x0k:implementation/folio/html-canonical
      - x0k:implementation/folio/format
---
# The folio/v1 projection plugin

**This module does not build outside the monorepo.** If you are reading
it in the published `x0k-folio` repository, you are holding source that
your `cargo build` never compiles: it implements traits from
`x0k-types`, a crate that is not published, and the projector that made
the repository dropped that dependency together with the `plugins`
feature that gated this module on. The source still ships because the
literate corpus is published whole, and because the design point is
worth reading even where it cannot run — the format library is the
public artifact; this chapter is where it meets the private substrate.

That substrate is the daemon's file-materialization loop, set out in an
internal architecture decision (`filesystem-graph-materialization`).
A folio document that lives in [Loro](../../wiki/loro.md "x0k:wiki/loro") — the
[CRDT](../../wiki/event-graph-crdts.md "x0k:wiki/event-graph-crdts") document store the
daemon keeps its live state in — is *projected* to a file on disk, and
a human's file edit is *parsed back* into Loro ops; this module
implements both directions of that round trip for `format: folio/v1`
documents.

Why the module is absent from the public build, in one paragraph: the
plugin's traits come from `x0k-types` — the substrate's class registry
and projection machinery — which drags in the whole substrate tree
(serialization, entry types, vector clocks). The published, standalone
`x0k-folio` (see [`format.md`](format.md)) is a *format library*:
parse, render, canonicalize, resolve — pure functions over strings. So
the entire module sits behind the `plugins` cargo feature (on by
default in the monorepo; pruned from the published manifest), and
`x0k-types` is an optional dependency that the severance turns off.
What the public reader holds in their hands is exactly the format; what
the monorepo builds is the format wired into its substrate.

Both directions are thin by design:

- **Render** (Loro → file): the Loro doc stores the rendered file body —
  envelope plus markdown, as written by the upstream renderer — under
  the `content` text path. Render reads it and emits the bytes
  verbatim. No synthesis: an empty or absent `content` means an empty
  file.
- **Parse** (file → Loro): a file edit becomes the minimal
  delete-then-insert pair of text ops that transforms the old content
  into the new. The one wrinkle is HTML bodies, which are canonicalized
  through [`html-canonical.md`](html-canonical.md) *before* diffing —
  the single chokepoint promise, kept at the write boundary.

<a name="chunk-module-doc"></a><sub>[`src/projection.rs`](../../../x0k-folio/src/projection.rs) · `#module-doc`</sub>

```rust {#module-doc}
//! `folio/v1` projection plugin.
//!
//! Reads the canonical body text out of the Loro doc (`content`
//! LoroText path) and emits it verbatim. The body already includes
//! the rendered `--- ... ---` envelope plus markdown — the wiki
//! crate's `to_folio_markdown` is the upstream renderer that
//! writes into `content`, so this plugin is a thin pass-through on
//! the read side.
//!
//! The companion `parse_diff` implementation owns the minimal-text-diff
//! algorithm; the daemon's `file_materializer` delegates here rather
//! than reimplementing it.
//!
//! One plugin instance per `ClassEntry` in the registry — multiple
//! classes (`x0k:wiki/*`, `x0k:design/*`, …) all map to plugin name
//! `folio/v1` but each carries its own `PathTemplate` +
//! `LiveEditPolicy`.

use crate::colophon::{parse_envelope, render_envelope, BODY_FORMAT_HTML};
use crate::html_canonical::normalize_html;
use anyhow::{Context, Result};
use x0k_types::operations::DocumentOp;
use x0k_types::projection_plugin::{
    DocumentBodyHandle, LiveEditPolicy, PathTemplate, ProjectionParser, ProjectionPlugin,
};

/// Canonical plugin name. Matches the value used in
/// `config/projection-classes.toml` for the `plugin = "..."` entries.
pub const FOLIO_V1_PLUGIN_NAME: &str = "folio/v1";
```

## The plugin

One instance per class entry: many document classes (`x0k:wiki/*`,
`x0k:design/*`, …) all name plugin `folio/v1`, each parameterized with
its own path template and live-edit policy. The plugin itself holds
nothing else — it is a stateless pair of functions plus configuration.

<a name="chunk-plugin-type"></a><sub>[`src/projection.rs`](../../../x0k-folio/src/projection.rs) · `#plugin-type`</sub>

```rust {#plugin-type}
/// `folio/v1` projection plugin instance. Parameterized per class
/// (via the registry) by `path_template` and `live_edit_policy`.
#[derive(Debug, Clone)]
pub struct ColophonProjection {
    path_template: PathTemplate,
    live_edit_policy: LiveEditPolicy,
}

impl ColophonProjection {
    pub fn new(path_template: PathTemplate, live_edit_policy: LiveEditPolicy) -> Self {
        Self {
            path_template,
            live_edit_policy,
        }
    }
}

impl ProjectionPlugin for ColophonProjection {
    fn name(&self) -> &str {
        FOLIO_V1_PLUGIN_NAME
    }

    fn render(&self, doc_handle: &dyn DocumentBodyHandle) -> Result<Vec<u8>> {
        // The Loro doc stores the rendered file body (envelope +
        // markdown) under `content`. Empty or absent means "the file
        // would be empty"; we emit zero bytes rather than synthesize.
        let body = doc_handle
            .read_text("content")
            .context("folio/v1 render: read `content` from doc handle")?
            .unwrap_or_default();
        Ok(body.into_bytes())
    }

    fn path_template(&self) -> &PathTemplate {
        &self.path_template
    }

    fn live_edit_policy(&self) -> LiveEditPolicy {
        self.live_edit_policy
    }

    fn parser(&self) -> Option<&dyn ProjectionParser> {
        Some(self)
    }
}
```

## Parsing a file edit into ops

The parse side receives old and new bytes and must produce Loro text
ops. Markdown bodies pass through verbatim. HTML bodies take the
canonicalization detour: if the new content parses as a folio/v1 file
with `body_format: html`, the envelope is re-rendered canonically and
the body normalized, and the *canonical* form is what gets diffed — so
concurrent edits round-trip through Loro without diff drift. A file
that doesn't parse (no envelope yet, raw body) falls back to the
verbatim diff rather than erroring; the materializer must never refuse
a half-written file.

<a name="chunk-parse-diff"></a><sub>[`src/projection.rs`](../../../x0k-folio/src/projection.rs) · `#parse-diff`</sub>

```rust {#parse-diff}
impl ProjectionParser for ColophonProjection {
    fn parse_diff(&self, old: &[u8], new: &[u8]) -> Result<Vec<DocumentOp>> {
        let old =
            std::str::from_utf8(old).context("folio/v1 parse_diff: old bytes are not UTF-8")?;
        let new =
            std::str::from_utf8(new).context("folio/v1 parse_diff: new bytes are not UTF-8")?;
        // HTML bodies are normalized on write so concurrent edits round-trip
        // through Loro without diff drift. Markdown bodies pass through
        // unchanged. If the new content isn't parseable as a folio/v1
        // file (no envelope yet, raw body), fall back to the verbatim diff.
        let new_canonicalized = canonicalize_html_body_if_needed(new);
        let new_str = new_canonicalized.as_deref().unwrap_or(new);
        Ok(build_text_diff_ops(old, new_str))
    }
}

/// If `content` parses as a folio/v1 file with `body_format: html`,
/// return the canonicalized form (envelope re-rendered + body normalized).
/// Otherwise return `None` — the caller treats the input verbatim.
///
/// This is the single chokepoint where HTML bodies pick up canonical
/// normalization before being written into the Loro `content` text path.
/// Markdown bodies are passed through unchanged (the parsed envelope is
/// re-rendered identically, and the markdown body is appended verbatim).
fn canonicalize_html_body_if_needed(content: &str) -> Option<String> {
    let (env, body) = parse_envelope(content).ok()?;
    if env.body_format != BODY_FORMAT_HTML {
        return None;
    }
    let normalized_body = normalize_html(&body);
    // The renderer emits the canonical envelope; concatenate with the
    // normalized body. The body in folio/v1 files starts on the line
    // *after* the closing `---`, which `render_envelope` already terminates
    // with a newline — so the body slots in directly.
    let mut out = String::with_capacity(content.len());
    out.push_str(&render_envelope(&env));
    out.push_str(&normalized_body);
    Some(out)
}
```

Note what `canonicalize_html_body_if_needed` does to an HTML file's
*envelope*: it re-renders it through
[`colophon.md`](colophon.md)'s canonical renderer, which normalizes
field order and drops unknown keys. For HTML-bodied documents that is
correct — they are machine-projected, nobody hand-edits their
frontmatter. It is exactly the operation the markdown path must never
perform, which is why the markdown path returns `None` and diffs the
author's bytes untouched.

## The minimal diff

The diff is deliberately naive: strip the longest common prefix and
suffix, and what remains is one delete plus one insert. Not a Myers
diff — for the actual editing patterns (a human or agent changes one
region between two saves) the single-window edit is minimal in
practice, and its simplicity means op indices are trivially right.
Indices are in Unicode scalar values, not bytes, because that is what
Loro's character-indexed text ops address; the `char`-vector
materialization is the price of that alignment.

<a name="chunk-text-diff"></a><sub>[`src/projection.rs`](../../../x0k-folio/src/projection.rs) · `#text-diff`</sub>

```rust {#text-diff}
/// Build the minimal set of Loro text ops that take `old_content` to
/// `new_content`. Operates on Unicode scalar values to match Loro's
/// character-indexed text ops.
///
/// The plugin owns the algorithm; a host's file-materializer — the
/// component that turns a document's edits into filesystem writes —
/// delegates here rather than carrying its own diff.
pub fn build_text_diff_ops(old_content: &str, new_content: &str) -> Vec<DocumentOp> {
    if old_content == new_content {
        return Vec::new();
    }
    let (prefix_chars, mid_old_chars, mid_new_text) = text_diff(old_content, new_content);
    let mut ops = Vec::new();
    if mid_old_chars > 0 {
        ops.push(DocumentOp::TextDelete {
            path: "content".to_string(),
            index: prefix_chars,
            len: mid_old_chars,
        });
    }
    if !mid_new_text.is_empty() {
        ops.push(DocumentOp::TextInsert {
            path: "content".to_string(),
            index: prefix_chars,
            text: mid_new_text,
        });
    }
    ops
}

/// Compute `(prefix_chars, deleted_old_chars, inserted_new_text)` —
/// the minimal single-edit window between `old` and `new` after
/// stripping common prefix and suffix. Operates on Unicode scalar
/// values so indices line up with Loro's text-op indices.
pub fn text_diff(old: &str, new: &str) -> (usize, usize, String) {
    let old_chars: Vec<char> = old.chars().collect();
    let new_chars: Vec<char> = new.chars().collect();

    let prefix_len = old_chars
        .iter()
        .zip(new_chars.iter())
        .take_while(|(a, b)| a == b)
        .count();

    let max_suffix = old_chars
        .len()
        .min(new_chars.len())
        .saturating_sub(prefix_len);
    let suffix_len = (0..max_suffix)
        .take_while(|i| old_chars[old_chars.len() - 1 - i] == new_chars[new_chars.len() - 1 - i])
        .count();

    let old_mid_len = old_chars.len() - prefix_len - suffix_len;
    let new_mid: String = new_chars[prefix_len..new_chars.len() - suffix_len]
        .iter()
        .collect();

    (prefix_len, old_mid_len, new_mid)
}
```

## Tests

A stub `DocumentBodyHandle` stands in for the Loro runtime, which keeps
the plugin's tests as pure as the plugin. They pin render's
verbatim/empty behavior, the three diff shapes (pure insert,
delete-then-insert, no-op), UTF-8 rejection, and that the injected
per-class configuration is visible through the trait.

<a name="chunk-tests"></a><sub>[`src/projection.rs`](../../../x0k-folio/src/projection.rs) · `#tests`</sub>

```rust {#tests}
#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;

    /// In-memory `DocumentBodyHandle` for plugin tests without a Loro
    /// runtime.
    struct StubHandle {
        content: Option<String>,
    }

    impl DocumentBodyHandle for StubHandle {
        fn read_text(&self, path: &str) -> Result<Option<String>> {
            if path == "content" {
                Ok(self.content.clone())
            } else {
                Ok(None)
            }
        }

        fn read_metadata(&self) -> Result<BTreeMap<String, String>> {
            Ok(BTreeMap::new())
        }
    }

    fn fixture() -> ColophonProjection {
        ColophonProjection::new(
            PathTemplate::parse("knowledge/wiki/{slug}.md").unwrap(),
            LiveEditPolicy::AutoOn,
        )
    }

    #[test]
    fn render_returns_content_bytes() {
        let plugin = fixture();
        let handle = StubHandle {
            content: Some("---\nx0k:\n  format: folio/v1\n---\nbody\n".to_string()),
        };
        let rendered = plugin.render(&handle).unwrap();
        assert_eq!(
            String::from_utf8(rendered).unwrap(),
            "---\nx0k:\n  format: folio/v1\n---\nbody\n"
        );
    }

    #[test]
    fn render_empty_when_content_missing() {
        let plugin = fixture();
        let handle = StubHandle { content: None };
        let rendered = plugin.render(&handle).unwrap();
        assert!(rendered.is_empty());
    }

    #[test]
    fn parse_diff_minimal_insert() {
        let plugin = fixture();
        let ops = plugin
            .parse_diff(b"hello world", b"hello brave world")
            .unwrap();
        // Pure insert — single TextInsert op at index 6, no delete.
        assert_eq!(ops.len(), 1);
        match &ops[0] {
            DocumentOp::TextInsert { path, index, text } => {
                assert_eq!(path, "content");
                assert_eq!(*index, 6);
                assert_eq!(text, "brave ");
            }
            other => panic!("expected TextInsert, got {other:?}"),
        }
    }

    #[test]
    fn parse_diff_replace_emits_delete_then_insert() {
        let plugin = fixture();
        let ops = plugin.parse_diff(b"hello world", b"hello mars").unwrap();
        assert_eq!(ops.len(), 2);
        assert!(matches!(ops[0], DocumentOp::TextDelete { .. }));
        assert!(matches!(ops[1], DocumentOp::TextInsert { .. }));
    }

    #[test]
    fn parse_diff_no_change_returns_empty() {
        let plugin = fixture();
        let ops = plugin.parse_diff(b"same", b"same").unwrap();
        assert!(ops.is_empty());
    }

    #[test]
    fn parse_diff_rejects_invalid_utf8() {
        let plugin = fixture();
        let err = plugin
            .parse_diff(&[0xFF, 0xFE], b"valid")
            .expect_err("invalid utf8 must reject");
        assert!(err.to_string().contains("UTF-8"));
    }

    #[test]
    fn name_is_canonical() {
        let plugin = fixture();
        assert_eq!(plugin.name(), "folio/v1");
        assert_eq!(plugin.name(), FOLIO_V1_PLUGIN_NAME);
    }

    #[test]
    fn injected_path_template_and_policy_visible_through_trait() {
        let plugin = ColophonProjection::new(
            PathTemplate::parse("decisions/design/{slug}.md").unwrap(),
            LiveEditPolicy::Forbidden,
        );
        assert_eq!(
            plugin.path_template().as_str(),
            "decisions/design/{slug}.md"
        );
        assert_eq!(plugin.live_edit_policy(), LiveEditPolicy::Forbidden);
    }
}
```

## Composing the module

<a name="chunk-root"></a><sub>[`src/projection.rs`](../../../x0k-folio/src/projection.rs) · `#root` · assembles [module-doc](#chunk-module-doc) · [plugin-type](#chunk-plugin-type) · [parse-diff](#chunk-parse-diff) · [text-diff](#chunk-text-diff) · [tests](#chunk-tests)</sub>

```rust {#root}
<<module-doc>>

<<plugin-type>>

<<parse-diff>>

<<text-diff>>

<<tests>>
```

The single-window diff is the module's honest trade: two edits far
apart in one save collapse into one oversized delete-insert spanning
everything between them, which is correct but coarser than necessary —
concurrent-edit merge quality through Loro degrades gracefully rather
than being optimal. If real usage ever shows that mattering, the fix is
contained: `text_diff` is the only function to sharpen, and its
contract (`prefix`, `deleted`, `inserted`) already speaks in the terms a
finer diff would produce.
