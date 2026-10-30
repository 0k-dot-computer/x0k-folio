# The folio projection plugin

```turtle folio:document
implementation:folio\/projection a x0k:Implementation ;
    x0k:status "draft" ;
    x0k:summary "The Loro round trip — document projected to a file, a human's file edit parsed back into ops — behind the `plugins` feature; the one chapter whose module a standalone build never compiles." ;
    x0k:concerns "folio", "projection", "loro", "plugins", "materialization" ;
    x0k:cites architecture:filesystem-graph-materialization,
        implementation:folio\/colophon,
        implementation:folio\/html-canonical,
        implementation:folio\/format ;
    x0k:implements design:body-format-isomorphism ;
    folio:tangleCrate "crates/x0k-folio" ;
    folio:tangleRoot "src/projection.rs" .
```

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
A folio document that lives in [Loro](../../background/loro.md "x0k:wiki/loro") — the
[CRDT](../../background/event-graph-crdts.md "x0k:wiki/event-graph-crdts") document store the
daemon keeps its live state in — is *projected* to a file on disk, and
a human's file edit is *parsed back* into Loro ops; this module
implements both directions of that round trip for folio documents.

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
  header plus markdown, as written by the upstream renderer — under
  the `content` text path. Render reads it and emits the bytes
  verbatim. No synthesis: an empty or absent `content` means an empty
  file.
- **Parse** (file → Loro): a file edit becomes the minimal
  delete-then-insert pair of text ops that transforms the old content
  into the new. Every body format is imported as the author's bytes,
  HTML included: the canonical HTML form of
  [`html-canonical.md`](html-canonical.md) is a normal form documents
  are compared under, not what an import stores.

Together they make a round trip that holds byte for byte: a file parsed
into a document and materialized back is the file. That is what lets the
document be the authority while the file stays an ordinary file an
editor, `git` or `jj` can hold (see "The round trip holds" below).

<a name="chunk-module-doc"></a><sub>[`src/projection.rs`](../../crates/x0k-folio/src/projection.rs) · `#module-doc`</sub>

```rust {#module-doc}
//! The folio projection plugin.
//!
//! Reads the canonical body text out of the Loro doc (`content`
//! LoroText path) and emits it verbatim. The body already includes
//! the rendered header plus markdown — the wiki
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
//! `folio/v2` but each carries its own `PathTemplate` +
//! `LiveEditPolicy`.

use anyhow::{Context, Result};
use x0k_types::operations::DocumentOp;
use x0k_types::projection_plugin::{
    DocumentBodyHandle, LiveEditPolicy, PathTemplate, ProjectionParser, ProjectionPlugin,
};

/// Canonical plugin name. Matches the value used in
/// `config/projection-classes.toml` for the `plugin = "..."` entries.
pub const FOLIO_PLUGIN_NAME: &str = "folio/v2";
```

## The plugin

One instance per class entry: many document classes (`x0k:wiki/*`,
`x0k:design/*`, …) all name plugin `folio/v2`, each parameterized with
its own path template and live-edit policy. The plugin itself holds
nothing else — it is a stateless pair of functions plus configuration.

<a name="chunk-plugin-type"></a><sub>[`src/projection.rs`](../../crates/x0k-folio/src/projection.rs) · `#plugin-type`</sub>

```rust {#plugin-type}
/// The folio projection plugin instance. Parameterized per class
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
        FOLIO_PLUGIN_NAME
    }

    fn render(&self, doc_handle: &dyn DocumentBodyHandle) -> Result<Vec<u8>> {
        // The Loro doc stores the rendered file body (header +
        // markdown) under `content`. Empty or absent means "the file
        // would be empty"; we emit zero bytes rather than synthesize.
        let body = doc_handle
            .read_text("content")
            .context("folio render: read `content` from doc handle")?
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
ops. Every body passes through verbatim, whatever its format, and a file
that doesn't parse as a folio document (no header yet, a half-written
save) is diffed the same way: the materializer must never refuse a
half-written file, and must never rewrite one either.

An importer preserves the authored source
(`filesystem-graph-materialization`, "Renderers and importers preserve
authored source details"). Canonicalizing an HTML body on the way in
broke that: the canonical form collapses the whitespace between block
elements, so the first projection back to disk rewrote a hand-laid HTML
manuscript as one line. Comparing two HTML bodies under the normal form
is the caller's business (`N_h` in `design:body-format-isomorphism`);
storing the normal form in place of the file is not.

<a name="chunk-parse-diff"></a><sub>[`src/projection.rs`](../../crates/x0k-folio/src/projection.rs) · `#parse-diff`</sub>

```rust {#parse-diff}
impl ProjectionParser for ColophonProjection {
    fn parse_diff(&self, old: &[u8], new: &[u8]) -> Result<Vec<DocumentOp>> {
        let old =
            std::str::from_utf8(old).context("folio parse_diff: old bytes are not UTF-8")?;
        let new =
            std::str::from_utf8(new).context("folio parse_diff: new bytes are not UTF-8")?;
        Ok(build_text_diff_ops(old, new))
    }
}
```

## The minimal diff

The diff is deliberately naive: strip the longest common prefix and
suffix, and what remains is one delete plus one insert. Not a Myers
diff — for the actual editing patterns (a human or agent changes one
region between two saves) the single-window edit is minimal in
practice, and its simplicity means op indices are trivially right.
Indices are in Unicode scalar values, not bytes, because that is what
Loro's character-indexed text ops address; the `char`-vector
materialization is the price of that alignment.

<a name="chunk-text-diff"></a><sub>[`src/projection.rs`](../../crates/x0k-folio/src/projection.rs) · `#text-diff`</sub>

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

<a name="chunk-tests"></a><sub>[`src/projection.rs`](../../crates/x0k-folio/src/projection.rs) · `#tests`</sub>

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
            content: Some("```turtle folio:document\nwiki:page a x0k:Wiki .\n```\nbody\n".to_string()),
        };
        let rendered = plugin.render(&handle).unwrap();
        assert_eq!(
            String::from_utf8(rendered).unwrap(),
            "```turtle folio:document\nwiki:page a x0k:Wiki .\n```\nbody\n"
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
        assert_eq!(plugin.name(), "folio/v2");
        assert_eq!(plugin.name(), FOLIO_PLUGIN_NAME);
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

## The round trip holds

What must hold, stated once: **for every file `f` this plugin binds,
`materialize(parse(f)) = f`, byte for byte**, where `parse` is
`parse_diff` from the empty document, the operations are applied to the
character-indexed text the store keeps, and `materialize` is `render`.
And for any two saves `a` and `b` of one file, applying `parse_diff(a, b)`
to the document holding `a` leaves it holding `b`. A file is then a faithful
view of the document: editing the file and editing the document through its
operations have the same result, and a file the daemon writes back differs
from the last one only where the document changed.

The check runs over the real corpus and over generated documents. Every
manuscript under `corpora/x0k/manuscripts/` must round-trip. Generated
documents draw headings, paragraphs with multi-byte text, nested lists,
backtick and tilde fences, graph and icon fences, transclusion and
`x0k:media` fences, tables, raw HTML blocks, quotes, block-identity
markers, HTML-bodied documents, and the bytes an editor leaves behind:
CRLF endings, a byte-order mark, no final newline. The generator is a
seeded splitmix draw, so a failure names its seed and reproduces.

A check that cannot fail proves nothing, so the same check is run against
a deliberately lossy serializer (one that drops trailing blanks, as a
formatter would) and must catch it on both the corpus and the generator.
Before HTML bodies were imported verbatim, the corpus check failed on the
one HTML-bodied manuscript, `understanding-x0k/index.md`: canonicalization
put its 140-line body on one line.

Naming a document's blocks ([`block-identity.md`](block-identity.md)) is an
edit like any other, so it must keep the round trip too, and in an HTML body
it changes bytes only where an id is written: every manuscript and every
generated document, once named, round-trips byte for byte, and an HTML body
read without its ids is the body before naming.

<a name="chunk-round-trip-tests"></a><sub>[`tests/projection_round_trip.rs`](../../crates/x0k-folio/tests/projection_round_trip.rs) · `#round-trip-tests`</sub>

```rust {#round-trip-tests file="tests/projection_round_trip.rs"}
//! The projection round trip: a file parsed into document operations and
//! materialized back is the same file, byte for byte.
//!
//! "Parse" is the plugin's own `parse_diff` from the empty document;
//! "apply" is the character-indexed text model Loro's `content` text keeps;
//! "materialize" is the plugin's own `render`. The property holds for every
//! manuscript in the corpus and for generated documents; an edit applied
//! as the diff of two saves reproduces the second save.

use std::path::PathBuf;

use x0k_folio::ColophonProjection;
use x0k_types::operations::DocumentOp;
use x0k_types::projection_plugin::{
    DocumentBodyHandle, LiveEditPolicy, PathTemplate, ProjectionPlugin,
};

fn plugin() -> ColophonProjection {
    ColophonProjection::new(
        PathTemplate::parse("corpora/x0k/manuscripts/{slug}.md").expect("slug template"),
        LiveEditPolicy::AutoOn,
    )
}

/// The document's `content` text, as the store keeps it: Unicode scalar
/// values, addressed by index.
struct Body(Vec<char>);

impl DocumentBodyHandle for Body {
    fn read_text(&self, path: &str) -> anyhow::Result<Option<String>> {
        Ok((path == "content").then(|| self.0.iter().collect()))
    }
}

fn apply(body: &mut Body, ops: &[DocumentOp]) {
    for op in ops {
        match op {
            DocumentOp::TextInsert { path, index, text } if path == "content" => {
                let tail = body.0.split_off(*index);
                body.0.extend(text.chars());
                body.0.extend(tail);
            }
            DocumentOp::TextDelete { path, index, len } if path == "content" => {
                body.0.drain(*index..*index + *len);
            }
            other => panic!("the folio parser emitted a non-text op: {other:?}"),
        }
    }
}

/// Parse `file` against `before`, apply the operations to `before`.
fn import(before: &str, file: &[u8]) -> Body {
    let projection = plugin();
    let parser = projection.parser().expect("folio/v2 parses");
    let ops = parser.parse_diff(before.as_bytes(), file).expect("parse");
    let mut body = Body(before.chars().collect());
    apply(&mut body, &ops);
    body
}

fn materialize(body: &Body) -> Vec<u8> {
    plugin().render(body).expect("render")
}

/// A serializer that loses what a careless one loses: trailing blanks on
/// each line, and the final newline normalized. The checks below must
/// refuse it, or they could not refuse anything.
fn lossy_materialize(body: &Body) -> Vec<u8> {
    let text: String = body.0.iter().collect();
    let mut out: String = text.lines().map(|l| l.trim_end()).collect::<Vec<_>>().join("\n");
    out.push('\n');
    out.into_bytes()
}

/// `Ok` when `file` survives parse-then-materialize under `serializer`;
/// otherwise where the first byte differs, with a little context.
fn round_trip(file: &[u8], serializer: fn(&Body) -> Vec<u8>) -> Result<(), String> {
    let out = serializer(&import("", file));
    if out == file {
        return Ok(());
    }
    let at = out
        .iter()
        .zip(file.iter())
        .position(|(a, b)| a != b)
        .unwrap_or(out.len().min(file.len()));
    let window = |bytes: &[u8]| {
        let start = at.saturating_sub(24);
        let end = (at + 24).min(bytes.len());
        String::from_utf8_lossy(&bytes[start.min(end)..end]).into_owned()
    };
    Err(format!(
        "first difference at byte {at} (file {} bytes, materialized {} bytes): file {:?} / materialized {:?}",
        file.len(),
        out.len(),
        window(file),
        window(&out)
    ))
}

/// The checkout this crate sits in: the nearest directory above it holding
/// the projection-class registry, wherever the crate has moved.
fn workspace() -> PathBuf {
    let mut dir = PathBuf::from(std::env::var_os("CARGO_MANIFEST_DIR").expect("cargo sets CARGO_MANIFEST_DIR when it runs a test"));
    while !dir.join("config/projection-classes.toml").is_file() {
        assert!(dir.pop(), "no checkout above the crate directory");
    }
    dir
}

fn manuscripts() -> Vec<PathBuf> {
    let root = workspace().join("corpora/x0k/manuscripts");
    let mut out = Vec::new();
    let mut stack = vec![root.clone()];
    while let Some(dir) = stack.pop() {
        for entry in std::fs::read_dir(&dir).unwrap_or_else(|e| panic!("{}: {e}", dir.display())) {
            let path = entry.expect("dir entry").path();
            if path.is_dir() {
                stack.push(path);
            } else if path.extension().and_then(|e| e.to_str()) == Some("md") {
                out.push(path);
            }
        }
    }
    out.sort();
    out
}

#[test]
fn every_manuscript_round_trips_byte_for_byte() {
    let files = manuscripts();
    assert!(files.len() >= 20, "the corpus holds its manuscripts: found {}", files.len());
    let failures: Vec<String> = files
        .iter()
        .filter_map(|path| {
            let bytes = std::fs::read(path).expect("read manuscript");
            round_trip(&bytes, materialize)
                .err()
                .map(|why| format!("{}: {why}", path.display()))
        })
        .collect();
    assert!(failures.is_empty(), "{} of {} manuscripts do not round-trip:\n{}", failures.len(), files.len(), failures.join("\n"));
}

#[test]
fn a_lossy_serializer_fails_the_manuscript_check() {
    let caught = manuscripts()
        .iter()
        .filter(|path| round_trip(&std::fs::read(path).unwrap(), lossy_materialize).is_err())
        .count();
    assert!(caught > 0, "a serializer that drops trailing blanks must fail on some manuscript");
}

/// A small deterministic generator: every run draws the same documents, so
/// a failure names its seed and reproduces.
struct Draw(u64);

impl Draw {
    fn next(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }

    fn below(&mut self, n: usize) -> usize {
        (self.next() % n as u64) as usize
    }

    fn pick<'a>(&mut self, items: &[&'a str]) -> &'a str {
        items[self.below(items.len())]
    }
}

const WORDS: &[&str] = &[
    "plan", "x0k", "the", "café", "naïve", "日本語", "seed🌱", "—", "a\tb", "*em*", "**strong**",
    "`code`", "[link](x0k:wiki/loro)", "[[discovery-loop]]", "<span>x</span>", "&amp;", "\\*", "end  ",
    "$x^2$", "~~gone~~",
];

fn inline(d: &mut Draw) -> String {
    let n = 1 + d.below(10);
    (0..n).map(|_| d.pick(WORDS)).collect::<Vec<_>>().join(" ")
}

fn block(d: &mut Draw) -> String {
    match d.below(12) {
        0 => format!("{} {}\n", "#".repeat(1 + d.below(6)), inline(d)),
        1 => (0..1 + d.below(3)).map(|_| inline(d) + "\n").collect(),
        2 => (0..1 + d.below(4))
            .map(|i| {
                let indent = if i > 0 && d.below(3) == 0 { "  " } else { "" };
                format!("{indent}{} {}\n", d.pick(&["-", "*", "1.", "+"]), inline(d))
            })
            .collect(),
        3 => {
            let fence = d.pick(&["```", "~~~"]);
            let info = d.pick(&["", "rust", "turtle folio:graph", "text", "svg x0k:icon"]);
            let lines: String = (0..d.below(4)).map(|_| inline(d) + "\n").collect();
            format!("{fence}{info}\n{lines}{fence}\n")
        }
        4 => "```x0k:transclude {ref=\"x0k:design/places#brief\"}\n```\n".to_string(),
        5 => "```x0k:media {ref=\"x0k:a/b\" source=\"knobs\"}\n```\n".to_string(),
        6 => format!("| a | {} |\n|---|:-:|\n| 1 | {} |\n", inline(d), inline(d)),
        7 => d.pick(&["<div class=\"x\">\n<p>hi</p>\n</div>\n", "<!-- a comment -->\n", "<details><summary>s</summary>\nbody\n</details>\n"]).to_string(),
        8 => format!("[block-{}]: #block-{}\n", d.below(9999), d.below(9999)),
        9 => format!("> {}\n> {}\n", inline(d), inline(d)),
        10 => "---\n".to_string(),
        _ => d.pick(&["\n", "   \n", "\t\n"]).to_string(),
    }
}

const HEADER: &str = "```turtle folio:document\nmanuscript:work\\/part a x0k:Manuscript ;\n    x0k:status \"draft\" .\n```\n";
const HTML_HEADER: &str = "```turtle folio:document\nmanuscript:work\\/part a x0k:Manuscript ;\n    x0k:status \"draft\" ;\n    x0k:bodyFormat \"html\" .\n```\n";

/// A Markdown document: an optional title and header, blocks separated by
/// blank lines (or not), and the byte-level variations an editor leaves —
/// CRLF endings, a byte-order mark, no final newline.
fn markdown_document(d: &mut Draw) -> String {
    let mut doc = String::new();
    if d.below(2) == 0 {
        doc.push_str(&format!("# {}\n\n{HEADER}\n", inline(d)));
    }
    for _ in 0..d.below(12) {
        doc.push_str(&block(d));
        doc.push_str(d.pick(&["\n", "", "\n\n"]));
    }
    if d.below(10) == 0 {
        doc = doc.replace('\n', "\r\n");
    }
    if d.below(10) == 0 {
        doc.pop();
    }
    if d.below(20) == 0 {
        doc.insert(0, '\u{feff}');
    }
    doc
}

fn html_document(d: &mut Draw) -> String {
    let mut doc = format!("# {}\n\n{HTML_HEADER}\n", inline(d));
    for _ in 0..1 + d.below(6) {
        let element = d.pick(&["p", "h2", "section", "blockquote", "li"]);
        let gap = d.pick(&["\n", "\n\n", "", "\n  "]);
        doc.push_str(&format!("<{element}>{}</{element}>{gap}", inline(d)));
    }
    doc
}

#[test]
fn generated_documents_round_trip() {
    for seed in 0..2048u64 {
        let mut d = Draw(seed);
        let doc = markdown_document(&mut d);
        round_trip(doc.as_bytes(), materialize).unwrap_or_else(|why| panic!("seed {seed}: {why}\n{doc:?}"));
    }
}

/// An HTML body is imported as written: its round trip is byte for byte
/// like a Markdown body's, not up to the HTML normal form.
#[test]
fn generated_html_documents_round_trip() {
    for seed in 0..512u64 {
        let mut d = Draw(seed);
        let doc = html_document(&mut d);
        round_trip(doc.as_bytes(), materialize).unwrap_or_else(|why| panic!("seed {seed}: {why}\n{doc:?}"));
    }
}

/// The live path: a file saved twice becomes the diff of the two saves,
/// applied to the document that holds the first. The document then holds
/// the second save exactly.
#[test]
fn an_edit_between_two_saves_reproduces_the_second_save() {
    for seed in 0..2048u64 {
        let mut d = Draw(seed);
        let before = markdown_document(&mut d);
        let after = markdown_document(&mut d);
        let body = import(&before, after.as_bytes());
        assert_eq!(materialize(&body), after.as_bytes(), "seed {seed}");
    }
}

#[test]
fn a_lossy_serializer_fails_the_generated_check() {
    let caught = (0..256u64)
        .filter(|seed| {
            let doc = markdown_document(&mut Draw(*seed));
            round_trip(doc.as_bytes(), lossy_materialize).is_err()
        })
        .count();
    assert!(caught > 0, "a serializer that drops trailing blanks must fail on some generated document");
}

/// A document whose blocks are named round-trips byte for byte; an HTML
/// body gains bytes only in the ids written into its start tags.
#[test]
fn named_documents_round_trip() {
    use x0k_folio::block_identity::{assign_ids, blocks, BlockText};
    let named = |text: &str, label: &str| {
        let named = assign_ids(text, "x0k:manuscript/named");
        round_trip(named.as_bytes(), materialize).unwrap_or_else(|why| panic!("{label}: {why}"));
        let html = named.contains("data-block-id");
        if html {
            assert_eq!(BlockText::parse(&named).visible(), text, "{label}: naming changed more than ids");
        }
        html
    };
    let manuscripts = manuscripts();
    let html = manuscripts
        .iter()
        .filter(|path| named(&std::fs::read_to_string(path).unwrap(), &path.display().to_string()))
        .count();
    assert!(html >= 1, "the HTML-bodied manuscripts are named by attribute");
    for seed in 0..512u64 {
        let doc = html_document(&mut Draw(seed));
        assert!(named(&doc, &format!("html seed {seed}")), "html seed {seed}: no block was named");
        assert!(blocks(&assign_ids(&doc, "x0k:manuscript/named")).iter().all(|block| block.id.is_some()));
    }
    for seed in 0..2048u64 {
        named(&markdown_document(&mut Draw(seed)), &format!("seed {seed}"));
    }
}
```

## Composing the module

<a name="chunk-root"></a><sub>[`src/projection.rs`](../../crates/x0k-folio/src/projection.rs) · `#root` · assembles [module-doc](#chunk-module-doc) · [plugin-type](#chunk-plugin-type) · [parse-diff](#chunk-parse-diff) · [text-diff](#chunk-text-diff) · [tests](#chunk-tests)</sub>

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
