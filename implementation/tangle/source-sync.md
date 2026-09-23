---
x0k:
  format: folio/v1
  id: x0k:implementation/tangle/source-sync
  type: implementation
  status: draft
  summary: 'The two paths that run against the tangle: filling a `from=` chunk''s body from the file it names, and replacing a named chunk''s body programmatically so a host can write a value back and re-tangle in lockstep.'
  concerns:
  - tangle
  - sync
  - reverse-tangle
  - source-refs
  - chunks
  tangle:
    crate: crates/x0k-tangle
    root: src/sync.rs
  edges:
    implements:
    - x0k:design/literate-programming
    cites:
    - x0k:implementation/tangle/source-refs
    - x0k:implementation/tangle/parsing
    - x0k:implementation/tangle/reverse-stitch
---

# Pulling code back into the document

Tangling flows one way: chunks in a [literate
document](../../background/literate-programming.md "x0k:wiki/literate-programming") become a source file. Two
situations need the other direction. A document that *references* a symbol
in existing hand-written source — a chunk carrying `from="…" symbol="…"` and
an empty body — wants that body filled in from the file, so the prose can
sit beside code it does not own. And a host that authors a chunk's content
programmatically — an editing UI that writes a live configuration value
back into the JSON chunk a document tangles that config file from — needs to
replace one named chunk's body and re-tangle, so document and artifact stay
in lockstep. This module is
both: `sync_document` for source references, `replace_chunk_body` for
programmatic write-back.

The carried example is a document with

<a name="chunk-my-fn"></a><sub>[`src/sync.rs`](../../crates/x0k-tangle/src/sync.rs) · `#my-fn`</sub>

    ```rust {#my-fn from="test.rs" symbol="my_fn"}
    ```

Syncing it reads `test.rs`, extracts the span of `my_fn` with the
source-refs extractor, and rewrites the document so the fence encloses that
body. Running it again replaces the body with whatever `my_fn` is now.

<a name="chunk-module-doc"></a><sub>[`src/sync.rs`](../../crates/x0k-tangle/src/sync.rs) · `#module-doc`</sub>

```rust {#module-doc}
use crate::parser::{parse_document, ParsedDocument};
use crate::source_ref::{extract_symbol_in, SymbolLanguage};
use anyhow::{Context, Result};
use std::path::{Path, PathBuf};
```

## The contract of a sync

A sync reports how many chunks it populated, how many it skipped, and every
error it met without stopping. A chunk is skipped when it has `from` but no
`symbol` — the extractor needs a name to find a span. A missing source file,
a fence whose language extraction cannot walk, or an unextractable symbol is
an error against that chunk, and the rest of the document still syncs. The
document is rewritten only when at least one patch exists.

The distinction matters to the caller: skipped chunks are a document saying
nothing was asked of them, errors are a document asking for something and not
getting it. A `sync` that reports errors has left the document out of step
with the source it names, which is the whole condition the verb exists to
remove — so the CLI turns any error into a non-zero exit
([`crate.md`](crate.md)).

<a name="chunk-sync-result"></a><sub>[`src/sync.rs`](../../crates/x0k-tangle/src/sync.rs) · `#sync-result`</sub>

```rust {#sync-result}
pub struct SyncResult {
    pub doc_path: PathBuf,
    pub chunks_populated: usize,
    pub chunks_skipped: usize,
    pub errors: Vec<String>,
}
```

<a name="chunk-sync-document"></a><sub>[`src/sync.rs`](../../crates/x0k-tangle/src/sync.rs) · `#sync-document`</sub>

```rust {#sync-document}
pub fn sync_document(doc_path: &Path, workspace_root: &Path) -> Result<SyncResult> {
    let content = std::fs::read_to_string(doc_path)?;
    let parsed = parse_document(&content)?;

    let mut patches: Vec<FromPatch> = Vec::new();
    let mut errors: Vec<String> = Vec::new();
    let mut skipped = 0;

    for name in &parsed.chunk_order {
        let Some(chunk) = parsed.chunk(name) else {
            continue;
        };

        if chunk.is_media || !chunk.is_from_ref() {
            continue;
        }

        let Some(ref from_path) = chunk.from else {
            continue;
        };
        let Some(ref symbol) = chunk.symbol else {
            skipped += 1;
            continue;
        };

        // Which grammar reads the source is a property of the document
        // alone, so it is settled before any file is opened: a chunk in a
        // language extraction cannot walk says so, rather than reporting
        // the symbol missing from a tree it was never in.
        let lang = match SymbolLanguage::for_lang(chunk.lang.as_deref()) {
            Ok(lang) => lang,
            Err(e) => {
                errors.push(format!("chunk '{}': {}", name, e));
                continue;
            }
        };

        let source_file = workspace_root.join(from_path);
        if !source_file.exists() {
            errors.push(format!(
                "chunk '{}': source file not found: {}",
                name,
                source_file.display()
            ));
            continue;
        }

        let source_content = std::fs::read_to_string(&source_file)
            .with_context(|| format!("reading {}", source_file.display()))?;

        match extract_symbol_in(&source_content, symbol, lang) {
            Ok(span) => {
                patches.push(FromPatch {
                    chunk_name: name.clone(),
                    new_body: span.body,
                });
            }
            Err(e) => {
                errors.push(format!("chunk '{}': {}", name, e));
            }
        }
    }

    if patches.is_empty() {
        return Ok(SyncResult {
            doc_path: doc_path.to_path_buf(),
            chunks_populated: 0,
            chunks_skipped: skipped,
            errors,
        });
    }

    let new_content = apply_from_patches(&content, &parsed, &patches)?;
    std::fs::write(doc_path, &new_content)?;

    Ok(SyncResult {
        doc_path: doc_path.to_path_buf(),
        chunks_populated: patches.len(),
        chunks_skipped: skipped,
        errors,
    })
}
```

## Where a body ends

Both rewrite paths splice a new body between a chunk's fences, so both have
to answer the question the reader already answers: which line closes this
fence? The reader is pulldown-cmark ([`parsing.md`](parsing.md)), and
CommonMark's answer is that a fenced block closes at a line of at least as
many fence characters as the opener, indented no more than three columns
past it, carrying nothing else. An indented ```` ``` ```` inside the body —
the end of a fenced example in a mirrored Python docstring — is body text.

The writer used to answer differently: it stopped at the first line whose
*trimmed* text began with three backticks. On a symbol whose docstring holds
a fenced example the two rules disagreed, and `sync` spliced the new body in
ahead of the nested fence, leaving the tail of the old body standing as
document prose — then appended it again on every later run, reporting
success each time. An adopter measured a 1,949-line pydantic class tripling
its document in three syncs, and found no escape hatch: a four-backtick
outer fence grew at the same rate, because the writer never looked at how
wide the opener was (2026-09-22; regression test
`a_mirror_whose_body_holds_a_fence_syncs_once`).

`Fence` is the one rule, and both paths go through it.

<a name="chunk-fence"></a><sub>[`src/sync.rs`](../../crates/x0k-tangle/src/sync.rs) · `#fence`</sub>

```rust {#fence}
/// A chunk's opening fence, and the rule for what closes it.
///
/// Only backticks: the info-string parser reads no other fence character,
/// so a tilde fence never names a chunk in the first place.
struct Fence {
    /// Bytes of leading whitespace on the opening fence line.
    indent: usize,
    /// How many backticks the opener carries.
    width: usize,
}
```

The three-column allowance is measured from the opener rather than from
column zero. CommonMark states it absolutely, but a chunk nested inside a
list item reaches the reader with its container's indentation already
stripped, so the opener's column is the one both sides of that reader agree
on.

<a name="chunk-fence-read"></a><sub>[`src/sync.rs`](../../crates/x0k-tangle/src/sync.rs) · `#fence-read`</sub>

```rust {#fence-read}
impl Fence {
    /// Read `line` as an opening fence, if it is one.
    fn open(line: &str) -> Option<Fence> {
        let rest = line.trim_start();
        let width = rest.bytes().take_while(|&b| b == b'`').count();
        (width >= 3).then(|| Fence {
            indent: line.len() - rest.len(),
            width,
        })
    }

    /// Does `line` close this fence? The reader's rule, and now the
    /// writer's: wide enough, not indented past the allowance, nothing
    /// else on the line.
    fn closes(&self, line: &str) -> bool {
        let rest = line.trim_start();
        if line.len() - rest.len() > self.indent + 3 {
            return false;
        }
        let width = rest.bytes().take_while(|&b| b == b'`').count();
        width >= self.width && rest[width..].trim().is_empty()
    }
}
```

Agreeing about where the old body ends is half of it. The other half is that
the body we splice in has to be one the reader reads back whole: a body
carrying a line that would close the fence needs a wider fence. That is the
answer an author would otherwise have to guess at — and, before this, guess
wrong, since widening by hand did nothing — so the writer computes it.

<a name="chunk-fence-write"></a><sub>[`src/sync.rs`](../../crates/x0k-tangle/src/sync.rs) · `#fence-write`</sub>

```rust {#fence-write}
impl Fence {
    /// How wide this fence has to be to hold `body` without the reader
    /// finding an end inside it.
    fn width_for(&self, body: &str) -> usize {
        body.lines()
            .filter(|line| self.closes(line))
            .map(|line| line.trim_start().bytes().take_while(|&b| b == b'`').count() + 1)
            .fold(self.width, usize::max)
    }

    /// The opening line re-emitted at `width`, info string intact.
    fn open_line(&self, line: &str, width: usize) -> String {
        format!(
            "{}{}{}",
            &line[..self.indent],
            "`".repeat(width),
            &line.trim_start()[self.width..]
        )
    }

    /// A closing fence at `width`, under the opener's indentation.
    fn close_line(&self, width: usize) -> String {
        format!("{}{}", " ".repeat(self.indent), "`".repeat(width))
    }
}
```

## Applying patches

A patch is a chunk name and its new body. Application walks the document's
lines, and at a fence whose info string names a patched chunk it emits the
fence, skips the old body through to that fence's closing line, emits the
new body, and emits the closing fence. The parsed document is threaded
through but unused — the walk locates fences by re-parsing info strings, so
it is self-sufficient.

A widened fence replaces the author's closing line, since the one they wrote
no longer closes the one we opened. An unwidened fence keeps that line byte
for byte, so a sync with nothing to change changes nothing.

Walking by lines loses one byte, and the walk has to put it back.
`lines()` drops a trailing newline and `join("\n")` does not restore it,
so a document that ended with one came back without it, and every sync
produced a `\ No newline at end of file` diff — one that ping-pongs
forever against any end-of-file-newline hook, since the hook restores the
byte and the next sync strips it again. The final newline is reattached
exactly when the input carried one, so a document that genuinely ends
without one does not gain one either. `replace_chunk_body` below already
worked this way; the two rewrite paths now agree.

<a name="chunk-from-patch"></a><sub>[`src/sync.rs`](../../crates/x0k-tangle/src/sync.rs) · `#from-patch`</sub>

```rust {#from-patch}
struct FromPatch {
    chunk_name: String,
    new_body: String,
}
```

<a name="chunk-apply-from-patches"></a><sub>[`src/sync.rs`](../../crates/x0k-tangle/src/sync.rs) · `#apply-from-patches`</sub>

```rust {#apply-from-patches}
fn apply_from_patches(
    content: &str,
    _parsed: &ParsedDocument,
    patches: &[FromPatch],
) -> Result<String> {
    let lines: Vec<&str> = content.lines().collect();
    let mut result: Vec<String> = Vec::new();
    let mut i = 0;

    while i < lines.len() {
        let line = lines[i];

        // A fence opens a patched chunk when its info string names one.
        let patched = Fence::open(line).and_then(|fence| {
            let info = line.trim_start().trim_start_matches('`');
            let name = crate::parser::parse_info_string(info).name?;
            let patch = patches.iter().find(|p| p.chunk_name == name)?;
            Some((fence, patch))
        });

        if let Some((fence, patch)) = patched {
            let width = fence.width_for(&patch.new_body);
            result.push(fence.open_line(line, width));
            i += 1;

            // Skip the old body to the line that ends it — by the
            // reader's rule, not by the first indented backticks.
            while i < lines.len() && !fence.closes(lines[i]) {
                i += 1;
            }

            result.extend(patch.new_body.lines().map(str::to_string));

            if i < lines.len() {
                result.push(if width == fence.width {
                    lines[i].to_string()
                } else {
                    fence.close_line(width)
                });
            }
            i += 1;
            continue;
        }

        result.push(line.to_string());
        i += 1;
    }

    // `lines()` dropped the final newline; put it back iff it was there.
    let mut out = result.join("\n");
    if content.ends_with('\n') {
        out.push('\n');
    }
    Ok(out)
}
```

## Programmatic write-back

`replace_chunk_body` is the same operation for one chunk, exposed to hosts.
It locates the chunk by scanning fence info strings for `{#<name>` and
checking the character after — `}`, a space, or end of line — so `world`
does not match `worldspec`. Only the first occurrence is replaced:
multi-body chunks are append-composed by tangling, and programmatic
write-back targets single-body artifact chunks. A trailing newline is
preserved, because `lines()` drops it and a document that lost one on every
save would churn.

It ends the old body and sizes the new one through the same `Fence`, so a
host committing a value that happens to contain a fence gets the widening
too — the two rewrite paths share the rule rather than each carrying their
own idea of where a body stops.

<a name="chunk-replace-chunk-body"></a><sub>[`src/sync.rs`](../../crates/x0k-tangle/src/sync.rs) · `#replace-chunk-body`</sub>

```rust {#replace-chunk-body}
/// Replace the body of one named fenced chunk in a literate document,
/// returning the rewritten markdown — the reverse-tangle primitive for hosts
/// that author a chunk's content programmatically (an editing UI commits an
/// edited configuration value into the JSON chunk its document tangles the
/// config file from, then re-tangles so doc → artifact stay in lockstep).
///
/// The chunk is located by scanning fence info strings for `{#<name>` (the
/// same attribute syntax the parser reads); everything between that fence
/// line and its closing fence is replaced with `new_body`. Errors when the
/// chunk is absent. Only the FIRST occurrence is replaced (multi-body chunks
/// are append-composed by tangling; programmatic write-back targets
/// single-body artifact chunks).
pub fn replace_chunk_body(md: &str, chunk_name: &str, new_body: &str) -> Result<String> {
    let needle = format!("{{#{chunk_name}");
    let mut out: Vec<String> = Vec::new();
    let mut lines = md.lines();
    let mut replaced = false;

    while let Some(line) = lines.next() {
        let trimmed = line.trim_start();
        // Match `{#name}` or `{#name <attrs>` — not a prefix of a longer name.
        let names_it = match trimmed.find(&needle) {
            Some(pos) => {
                let after = trimmed[pos + needle.len()..].chars().next();
                matches!(after, Some('}') | Some(' ') | None)
            }
            None => false,
        };
        let Some(fence) = Fence::open(line).filter(|_| !replaced && names_it) else {
            out.push(line.to_string());
            continue;
        };

        // Emit the replacement body between fences wide enough to hold it,
        // then skip the old body through the line that ends it.
        let width = fence.width_for(new_body);
        out.push(fence.open_line(line, width));
        out.extend(new_body.lines().map(str::to_string));

        let mut closed = false;
        for inner in lines.by_ref() {
            if fence.closes(inner) {
                out.push(if width == fence.width {
                    inner.to_string()
                } else {
                    fence.close_line(width)
                });
                closed = true;
                break;
            }
        }
        if !closed {
            anyhow::bail!("chunk `{chunk_name}` has no closing fence");
        }
        replaced = true;
    }

    if !replaced {
        anyhow::bail!("chunk `{chunk_name}` not found in document");
    }
    // Preserve a trailing newline (lines() drops it).
    let mut s = out.join("\n");
    if md.ends_with('\n') {
        s.push('\n');
    }
    Ok(s)
}
```

## Tests

The patch-application cases work on strings. The two `sync_document` cases
need a document and a source file on disk in the same workspace, so they
build one in a temp directory: one syncing a JavaScript chunk end to end —
the path that used to report a live export as missing because the file was
read with the Rust grammar — and one on a language extraction does not walk,
which must leave the document untouched and say why.

The idempotence cases all run through `synced_twice_in`, which syncs, syncs
again, and demands the two documents be the same bytes — then reads the
document back and holds it to `check`'s own predicate, that the body it
shows is the body the source holds line for line. A fix that stopped the
growth but left the document saying something else would pass the first half
and fail the second.

<a name="chunk-tests"></a><sub>[`src/sync.rs`](../../crates/x0k-tangle/src/sync.rs) · `#tests`</sub>

`````rust {#tests}
#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    /// A workspace holding one source file and one document that
    /// references a symbol in it. Returns the document's path.
    fn workspace(source_name: &str, source: &str, doc: &str) -> (TempDir, PathBuf) {
        let tmp = TempDir::new().unwrap();
        std::fs::write(tmp.path().join(source_name), source).unwrap();
        let doc_path = tmp.path().join("doc.md");
        std::fs::write(&doc_path, doc).unwrap();
        (tmp, doc_path)
    }

    #[test]
    fn syncs_a_javascript_chunk_from_its_source() {
        let (tmp, doc_path) = workspace(
            "remap.js",
            "export function createHorizonRemap(scale) {\n  return (u) => u * scale;\n}\n",
            "# Remap\n\n```javascript {#remap from=\"remap.js\" symbol=\"createHorizonRemap\"}\n```\n",
        );

        let result = sync_document(&doc_path, tmp.path()).unwrap();
        assert_eq!(result.errors, Vec::<String>::new());
        assert_eq!(result.chunks_populated, 1);

        let synced = std::fs::read_to_string(&doc_path).unwrap();
        assert!(
            synced.contains("export function createHorizonRemap(scale) {"),
            "got {synced}"
        );
        assert!(synced.contains("return (u) => u * scale;"), "got {synced}");
    }

    #[test]
    fn a_chunk_in_an_unwalkable_language_errors_and_leaves_the_document_alone() {
        let doc = "# Remap\n\n```ruby {#remap from=\"remap.rb\" symbol=\"create_remap\"}\n```\n";
        let (tmp, doc_path) = workspace("remap.rb", "def create_remap\n  1\nend\n", doc);

        let result = sync_document(&doc_path, tmp.path()).unwrap();
        assert_eq!(result.chunks_populated, 0);
        assert_eq!(result.errors.len(), 1, "got {:?}", result.errors);
        let err = &result.errors[0];
        assert!(err.contains("chunk 'remap'"), "got {err}");
        assert!(err.contains("symbol extraction supports"), "got {err}");
        assert!(!err.contains("not found"), "got {err}");
        assert_eq!(std::fs::read_to_string(&doc_path).unwrap(), doc);
    }

    #[test]
    fn replace_chunk_body_swaps_only_the_named_chunk() {
        let md = "# Doc\n\n```json {#world file=out.json}\nold\nbody\n```\n\n```rust {#other}\nkeep\n```\n";
        let out = replace_chunk_body(md, "world", "new\ncontent").unwrap();
        assert!(out.contains("```json {#world file=out.json}\nnew\ncontent\n```"));
        assert!(out.contains("```rust {#other}\nkeep\n```"));
        assert!(!out.contains("old\nbody"));
        // `{#world` must not match a longer name.
        assert!(replace_chunk_body(md, "wor", "x").is_err());
    }

    #[test]
    fn apply_patches_replaces_body() {
        let content = r#"# Test

```rust {#my-fn from="test.rs" symbol="my_fn"}
```

Some prose.
"#;
        let parsed = parse_document(content).unwrap();
        let patches = vec![FromPatch {
            chunk_name: "my-fn".to_string(),
            new_body: "fn my_fn() {\n    println!(\"hello\");\n}".to_string(),
        }];

        let result = apply_from_patches(content, &parsed, &patches).unwrap();
        assert!(result.contains("fn my_fn()"));
        assert!(result.contains("println!(\"hello\");"));
        assert!(result.contains("Some prose."));
    }

    #[test]
    fn apply_patches_replaces_existing_body() {
        let content = r#"```rust {#my-fn from="test.rs" symbol="my_fn"}
fn old_version() {}
```
"#;
        let parsed = parse_document(content).unwrap();
        let patches = vec![FromPatch {
            chunk_name: "my-fn".to_string(),
            new_body: "fn new_version() {\n    // updated\n}".to_string(),
        }];

        let result = apply_from_patches(content, &parsed, &patches).unwrap();
        assert!(result.contains("fn new_version()"));
        assert!(!result.contains("fn old_version()"));
    }

    /// The document ends `` ``` `` + newline before the sync and must end
    /// the same way after it — twice, since a rewrite that strips the byte
    /// ping-pongs against any end-of-file-newline hook.
    fn synced_twice(doc: &str) -> String {
        let (tmp, doc_path) = workspace(
            "remap.js",
            "export function createHorizonRemap(scale) {\n  return (u) => u * scale;\n}\n",
            doc,
        );
        let first = sync_document(&doc_path, tmp.path()).unwrap();
        assert_eq!(first.chunks_populated, 1, "{:?}", first.errors);
        let after_one = std::fs::read_to_string(&doc_path).unwrap();
        sync_document(&doc_path, tmp.path()).unwrap();
        let after_two = std::fs::read_to_string(&doc_path).unwrap();
        assert_eq!(after_one, after_two, "sync is not idempotent");
        after_two
    }

    #[test]
    fn a_synced_document_keeps_its_trailing_newline() {
        let synced = synced_twice(
            "# Remap\n\n```javascript {#remap from=\"remap.js\" symbol=\"createHorizonRemap\"}\n```\n",
        );
        assert!(synced.ends_with("```\n"), "got {synced:?}");
    }

    #[test]
    fn a_document_without_a_trailing_newline_does_not_gain_one() {
        let synced = synced_twice(
            "# Remap\n\n```javascript {#remap from=\"remap.js\" symbol=\"createHorizonRemap\"}\n```",
        );
        assert!(synced.ends_with("```"), "got {synced:?}");
        assert!(!synced.ends_with('\n'), "got {synced:?}");
    }

    /// A class whose docstring holds a fenced example — how a Python
    /// library documents its public surface, and the shape that made
    /// `sync` grow a document on every run.
    const PY_WITH_FENCE: &str = r#"class Thing:
    """A thing.

    Example:
        ```python
        from thing import Thing
        t = Thing()
        ```
    """

    x: int = 1
"#;

    /// A body carrying a line the reader would read as the end of a
    /// three-backtick fence. Splicing it verbatim would cut the chunk in
    /// half, so the fence has to widen.
    const JS_WITH_BARE_FENCE: &str =
        "export function readme() {\n  return `\n```\nhi\n```\n`;\n}\n";

    /// Sync twice, demand the second run changed nothing, and hold the
    /// result to `check`'s predicate: the body shown is the body the
    /// source holds, line for line.
    fn synced_twice_in(source_name: &str, source: &str, doc: &str) -> String {
        let (tmp, doc_path) = workspace(source_name, source, doc);

        let first = sync_document(&doc_path, tmp.path()).unwrap();
        assert_eq!(first.errors, Vec::<String>::new());
        assert_eq!(first.chunks_populated, 1);
        let after_one = std::fs::read_to_string(&doc_path).unwrap();

        sync_document(&doc_path, tmp.path()).unwrap();
        let after_two = std::fs::read_to_string(&doc_path).unwrap();
        assert_eq!(after_one, after_two, "sync is not idempotent");

        let parsed = parse_document(&after_two).unwrap();
        let chunk = parsed.chunk("thing").unwrap();
        let lang = SymbolLanguage::for_lang(chunk.lang.as_deref()).unwrap();
        let span = extract_symbol_in(source, chunk.symbol.as_ref().unwrap(), lang).unwrap();
        assert!(
            chunk.bodies[0].text.lines().eq(span.body.lines()),
            "check would call this drifted:\n{}",
            chunk.bodies[0].text
        );
        after_two
    }

    #[test]
    fn a_mirror_whose_body_holds_a_fence_syncs_once() {
        let synced = synced_twice_in(
            "thing.py",
            PY_WITH_FENCE,
            "# doc\n\n```python {#thing from=\"thing.py\" symbol=\"Thing\"}\n```\n",
        );
        assert_eq!(
            synced.matches("from thing import Thing").count(),
            1,
            "the docstring example was duplicated: {synced}"
        );
    }

    /// Widening the outer fence by hand used to change nothing, because
    /// the writer never read how wide the opener was. It has to work now,
    /// since an author who hits this reaches for it first.
    #[test]
    fn a_four_backtick_mirror_holds_a_fence_too() {
        let synced = synced_twice_in(
            "thing.py",
            PY_WITH_FENCE,
            "# doc\n\n````python {#thing from=\"thing.py\" symbol=\"Thing\"}\n````\n",
        );
        assert_eq!(
            synced.matches("from thing import Thing").count(),
            1,
            "the docstring example was duplicated: {synced}"
        );
        assert!(synced.contains("````python {#thing"), "got {synced}");
    }

    #[test]
    fn a_body_that_would_close_the_fence_widens_it() {
        let synced = synced_twice_in(
            "readme.js",
            JS_WITH_BARE_FENCE,
            "# doc\n\n```javascript {#thing from=\"readme.js\" symbol=\"readme\"}\n```\n",
        );
        assert!(
            synced.contains("````javascript {#thing"),
            "the opener widened: {synced}"
        );
        assert!(synced.ends_with("````\n"), "and so did its closer: {synced}");
    }

    #[test]
    fn a_fence_ends_only_where_the_reader_ends_it() {
        let fence = Fence::open("```python {#thing}").unwrap();
        assert!(!fence.closes("        ```"), "an indented nested fence");
        assert!(!fence.closes("``"), "too few backticks");
        assert!(!fence.closes("``` python"), "a closer carries nothing else");
        assert!(fence.closes("```"), "the outer fence at the opener's column");
        assert!(fence.closes("  ````  "), "wider, inside the allowance");

        let wide = Fence::open("````rust {#thing}").unwrap();
        assert!(!wide.closes("```"), "narrower than its opener");
        assert_eq!(wide.width_for("```\nhi\n```"), 4, "narrower lines are safe");
        assert_eq!(wide.width_for("````\nhi"), 5, "one wider than the widest");
        assert_eq!(wide.width_for("    ````\nhi"), 4, "indented past the allowance");
    }
}
`````

## The file

<a name="chunk-root"></a><sub>[`src/sync.rs`](../../crates/x0k-tangle/src/sync.rs) · `#root` · assembles [module-doc](#chunk-module-doc) · [sync-result](#chunk-sync-result) · [sync-document](#chunk-sync-document) · [fence](#chunk-fence) · [fence-read](#chunk-fence-read) · [fence-write](#chunk-fence-write) · [from-patch](#chunk-from-patch) · [apply-from-patches](#chunk-apply-from-patches) · [replace-chunk-body](#chunk-replace-chunk-body) · [tests](#chunk-tests)</sub>

```rust {#root}
<<module-doc>>

<<sync-result>>

<<sync-document>>

<<fence>>

<<fence-read>>

<<fence-write>>

<<from-patch>>

<<apply-from-patches>>

<<replace-chunk-body>>

<<tests>>
```

Both directions here overwrite a chunk body wholesale. There is no merge:
if a document's author and the referenced source both changed, the source
wins on sync, which is the correct answer exactly when the chunk is a
reference and not an authored body — and the `from` attribute is how the
chunk says which it is.
