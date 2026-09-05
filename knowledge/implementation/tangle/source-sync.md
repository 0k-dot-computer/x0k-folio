---
x0k:
  format: folio/v1
  id: x0k:implementation/tangle/source-sync
  type: implementation
  status: draft
  summary: "The two paths that run against the tangle: filling a `from=` chunk's body from the file it names, and replacing a named chunk's body programmatically so a host can write a value back and re-tangle in lockstep."
  concerns: [tangle, sync, reverse-tangle, source-refs, chunks]
  tangle:
    crate: x0k-tangle
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
document](../../wiki/literate-programming.md "x0k:wiki/literate-programming") become a source file. Two
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

<a name="chunk-my-fn"></a><sub>[`src/sync.rs`](../../../x0k-tangle/src/sync.rs) · `#my-fn`</sub>

    ```rust {#my-fn from="test.rs" symbol="my_fn"}
    ```

Syncing it reads `test.rs`, extracts the span of `my_fn` with the
source-refs extractor, and rewrites the document so the fence encloses that
body. Running it again replaces the body with whatever `my_fn` is now.

<a name="chunk-module-doc"></a><sub>[`src/sync.rs`](../../../x0k-tangle/src/sync.rs) · `#module-doc`</sub>

```rust {#module-doc}
use crate::parser::{parse_document, ParsedDocument};
use crate::source_ref::extract_symbol;
use anyhow::{Context, Result};
use std::path::{Path, PathBuf};
```

## The contract of a sync

A sync reports how many chunks it populated, how many it skipped, and every
error it met without stopping. A chunk is skipped when it has `from` but no
`symbol` — the extractor needs a name to find a span. A missing source file
or an unextractable symbol is an error against that chunk, and the rest of
the document still syncs. The document is rewritten only when at least one
patch exists.

<a name="chunk-sync-result"></a><sub>[`src/sync.rs`](../../../x0k-tangle/src/sync.rs) · `#sync-result`</sub>

```rust {#sync-result}
pub struct SyncResult {
    pub doc_path: PathBuf,
    pub chunks_populated: usize,
    pub chunks_skipped: usize,
    pub errors: Vec<String>,
}
```

<a name="chunk-sync-document"></a><sub>[`src/sync.rs`](../../../x0k-tangle/src/sync.rs) · `#sync-document`</sub>

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

        match extract_symbol(&source_content, symbol) {
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

## Applying patches

A patch is a chunk name and its new body. Application walks the document's
lines, and at a fence whose info string names a patched chunk it emits the
fence, skips the old body through to the closing fence, emits the new body,
and emits the closing fence. The parsed document is threaded through but
unused — the walk locates fences by re-parsing info strings, so it is
self-sufficient.

<a name="chunk-from-patch"></a><sub>[`src/sync.rs`](../../../x0k-tangle/src/sync.rs) · `#from-patch`</sub>

```rust {#from-patch}
struct FromPatch {
    chunk_name: String,
    new_body: String,
}
```

<a name="chunk-apply-from-patches"></a><sub>[`src/sync.rs`](../../../x0k-tangle/src/sync.rs) · `#apply-from-patches`</sub>

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

        // Detect fenced code block opening with chunk attributes
        if line.trim_start().starts_with("```") {
            let info = line.trim_start().trim_start_matches('`');
            let attrs = crate::parser::parse_info_string(info);

            if let Some(ref chunk_name) = attrs.name {
                if let Some(patch) = patches.iter().find(|p| p.chunk_name == *chunk_name) {
                    // Emit the opening fence
                    result.push(line.to_string());
                    i += 1;

                    // Skip old body (everything until closing ```)
                    while i < lines.len() && !lines[i].trim_start().starts_with("```") {
                        i += 1;
                    }

                    // Emit new body
                    for body_line in patch.new_body.lines() {
                        result.push(body_line.to_string());
                    }

                    // Emit closing fence
                    if i < lines.len() {
                        result.push(lines[i].to_string());
                    }
                    i += 1;
                    continue;
                }
            }
        }

        result.push(line.to_string());
        i += 1;
    }

    Ok(result.join("\n"))
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

<a name="chunk-replace-chunk-body"></a><sub>[`src/sync.rs`](../../../x0k-tangle/src/sync.rs) · `#replace-chunk-body`</sub>

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
    let mut out: Vec<&str> = Vec::new();
    let mut lines = md.lines();
    let mut replaced = false;

    while let Some(line) = lines.next() {
        out.push(line);
        let trimmed = line.trim_start();
        if !replaced && trimmed.starts_with("```") && {
            // Match `{#name}` or `{#name <attrs>` — not a prefix of a longer name.
            match trimmed.find(&needle) {
                Some(pos) => {
                    let after = trimmed[pos + needle.len()..].chars().next();
                    matches!(after, Some('}') | Some(' ') | None)
                }
                None => false,
            }
        } {
            // Emit the replacement body, then skip the old body through the
            // closing fence (which we keep).
            for body_line in new_body.lines() {
                out.push(body_line);
            }
            let mut closed = false;
            for inner in lines.by_ref() {
                if inner.trim_start().starts_with("```") {
                    out.push(inner);
                    closed = true;
                    break;
                }
            }
            if !closed {
                anyhow::bail!("chunk `{chunk_name}` has no closing fence");
            }
            replaced = true;
        }
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

<a name="chunk-tests"></a><sub>[`src/sync.rs`](../../../x0k-tangle/src/sync.rs) · `#tests`</sub>

`````rust {#tests}
#[cfg(test)]
mod tests {
    use super::*;

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
}
`````

## The file

<a name="chunk-root"></a><sub>[`src/sync.rs`](../../../x0k-tangle/src/sync.rs) · `#root` · assembles [module-doc](#chunk-module-doc) · [sync-result](#chunk-sync-result) · [sync-document](#chunk-sync-document) · [from-patch](#chunk-from-patch) · [apply-from-patches](#chunk-apply-from-patches) · [replace-chunk-body](#chunk-replace-chunk-body) · [tests](#chunk-tests)</sub>

```rust {#root}
<<module-doc>>

<<sync-result>>

<<sync-document>>

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
