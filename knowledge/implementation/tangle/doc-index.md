---
x0k:
  format: folio/v1
  id: x0k:implementation/tangle/doc-index
  type: implementation
  status: draft
  summary: The one-pass walk that emits a serializable index of a corpus — envelope fields, tangle target, mtime, and per-chunk coordinates — so a sidebar or a figure can render a chunk without re-parsing its document.
  concerns: [tangle, index, chunks, source-refs, authoring-ui, figures]
  tangle:
    crate: x0k-tangle
    root: src/index.rs
  edges:
    implements:
      - x0k:design/literate-programming
    cites:
      - x0k:implementation/tangle/parsing
      - x0k:implementation/tangle/source-refs
      - x0k:implementation/folio/colophon
---

# An index is the document seen from outside

The authoring UI's sidebar, a worked figure that binds to a document's real
code, and any tool that wants to list the literate corpus all ask the same
question: what documents are here, and what chunks do they carry? Parsing
every document on every ask is too slow for a sidebar and too coupled for a
figure. So `x0k-tangle index` walks a set of paths once and emits a
serializable `DocIndex`: one `DocEntry` per folio/v1 document with its
envelope fields, its tangle target, its modification time, and a
`ChunkSummary` per chunk carrying enough coordinates that a consumer can
render the chunk's code without re-parsing the document.

The carried example is a document with two `from=` chunks —

    ```rust {#verdict from="machine.rs" symbol="classify_range"}
    ```
    ```rust {#shelf from="machine.rs" symbol="Shelf"}
    ```

— whose index entry reports, for `verdict`, the source file, the symbol,
the extracted text, and the 1-based source line the symbol begins on; and
for `shelf`, a **span map** naming `seal` and `merge` with line ranges
relative to the extracted chunk rather than the source file. A figure step
that says "highlight `seal`" resolves to a line band through that map, and
the band stays correct when `machine.rs` gains a preamble.

```rust {#module-doc}
use crate::parser::parse_document;
use crate::source_ref::{extract_symbol, list_symbols};
use anyhow::Result;
use serde::Serialize;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
```

## The shape

Fields that are only meaningful for some chunks (`from`, `symbol`, `text`,
`source_start_line`, `span_map`) are omitted from the JSON when absent, so
a consumer distinguishes "owned chunk" from "from-chunk whose source could
not be read" by presence rather than by empty strings. `modified_us` is the
sidebar's recency key; `body_format` is the flag the authoring UI gates
in-place editing on.

```rust {#doc-index}
#[derive(Debug, Serialize)]
pub struct DocIndex {
    pub docs: Vec<DocEntry>,
}

#[derive(Debug, Serialize)]
pub struct DocEntry {
    pub id: String,
    pub path: String,
    pub title: String,
    pub doc_type: String,
    pub status: String,
    /// Body format dispatch flag: `"markdown"` (default) or `"html"`. The
    /// authoring UI gates in-place editing on this (HTML bodies are
    /// read-only today).
    pub body_format: String,
    pub concerns: Vec<String>,
    pub edges: BTreeMap<String, Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tangle_crate: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tangle_root: Option<String>,
    /// Source-file modification time in microseconds since the Unix epoch.
    /// The sidebar sorts the doc list most-recently-edited first so the docs
    /// being actively worked on float to the top. `None` when the file's mtime
    /// can't be read (the entry then sorts last in the recency order).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub modified_us: Option<u64>,
    pub chunks: Vec<ChunkSummary>,
}
```

```rust {#chunk-summary}
#[derive(Debug, Serialize)]
pub struct ChunkSummary {
    pub name: String,
    pub kind: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub lang: Option<String>,
    pub lines: usize,
    /// Source file the chunk was pulled from (`from=` attribute), relative to
    /// the workspace root. Present only for `from` chunks; `None` for owned
    /// code and media embeds.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub from: Option<String>,
    /// The tree-sitter symbol path extracted from the source file
    /// (`symbol=` attribute), e.g. `classify_range` or `Canvas2DState::arc`.
    /// Present only for `from` chunks that name a symbol.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub symbol: Option<String>,
    /// The chunk's extracted text — for a `from` chunk, the symbol body as it
    /// lives in the source file; for owned chunks, the combined chunk body.
    /// A worked figure binds to this so it can render the document's real code.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub text: Option<String>,
    /// The 1-based line in the source file where the extracted symbol begins.
    /// Lets a consumer correlate the rendered panel back to the source. Present
    /// only for `from` chunks whose symbol could be re-extracted.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source_start_line: Option<usize>,
    /// Symbol-relative span map: each named sub-symbol within the extracted
    /// chunk paired with its **chunk-relative** (1-based, against `text`) line
    /// range. A figure step names a sub-symbol; the platform resolves it here
    /// to a line band in the rendered panel — stable under source moves because
    /// the lines are relative to the extracted chunk, not the source file.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub span_map: Option<Vec<SpanMapEntry>>,
}

/// One entry in a chunk's symbol-relative span map: a sub-symbol name and its
/// chunk-relative line range (1-based, inclusive of both ends).
#[derive(Debug, Serialize)]
pub struct SpanMapEntry {
    pub symbol: String,
    pub start_line: usize,
    pub end_line: usize,
}
```

## Walking the paths

A path is either a markdown file or a directory to walk. `AGENTS.md` and
`CLAUDE.md` are skipped by name — they are guidance, not corpus — and the
result is sorted by document id so the index is stable across filesystem
order.

```rust {#build-index}
pub fn build_index(paths: &[PathBuf], workspace_root: &Path) -> Result<DocIndex> {
    let mut docs = Vec::new();

    for path in paths {
        if path.is_file() && path.extension().is_some_and(|e| e == "md") {
            if let Some(entry) = index_file(path, workspace_root)? {
                docs.push(entry);
            }
        } else if path.is_dir() {
            for entry in walkdir::WalkDir::new(path)
                .into_iter()
                .filter_map(|e| e.ok())
            {
                let p = entry.path();
                if p.extension().is_some_and(|e| e == "md")
                    && !p
                        .file_name()
                        .is_some_and(|n| n == "AGENTS.md" || n == "CLAUDE.md")
                {
                    if let Some(doc_entry) = index_file(p, workspace_root)? {
                        docs.push(doc_entry);
                    }
                }
            }
        }
    }

    docs.sort_by(|a, b| a.id.cmp(&b.id));
    Ok(DocIndex { docs })
}
```

## Indexing one file

A file is a document if it opens with `---` and mentions `folio/v1`; a
document the parser rejects is skipped rather than failing the whole index.
The mtime is best-effort. The envelope fields come from a line scanner
rather than a YAML parser (below), and the chunk summaries carry the
coordinates the carried example shows: a `from=` symbol is re-extracted from
its source file to recover the authoritative body, start line, and span map;
when re-extraction is unavailable the chunk's own body stands in; an owned
chunk's text is its combined body, and a media chunk has none.

```rust {#index-file}
fn index_file(path: &Path, workspace_root: &Path) -> Result<Option<DocEntry>> {
    let content = std::fs::read_to_string(path)?;

    if !content.starts_with("---") || !content.contains("folio/v1") {
        return Ok(None);
    }

    // Source-file mtime in µs since the Unix epoch, used by the sidebar to
    // sort most-recently-edited first. Best-effort: any failure leaves the
    // field `None` so the entry sorts last in the recency order.
    let modified_us = std::fs::metadata(path)
        .and_then(|m| m.modified())
        .ok()
        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
        .map(|d| d.as_micros() as u64);

    let parsed = match parse_document(&content) {
        Ok(p) => p,
        Err(_) => return Ok(None),
    };

    let rel_path = path
        .strip_prefix(workspace_root)
        .unwrap_or(path)
        .to_string_lossy()
        .to_string();

    // Extract frontmatter fields via simple line scanning
    let (id, doc_type, status, concerns, edges) = extract_frontmatter_fields(&content);

    let title = extract_title(&content);

    let mut chunks = Vec::new();
    for name in &parsed.chunk_order {
        if let Some(variants) = parsed.chunks.get(name) {
            for chunk in variants {
                let kind = if chunk.is_media {
                    "media"
                } else if chunk.is_from_ref() {
                    "from"
                } else {
                    "owned"
                };
                let lines: usize = chunk.bodies.iter().map(|b| b.text.lines().count()).sum();

                // Carry source coordinates so a worked figure can bind to the
                // real chunk. For a `from=` symbol we re-extract from the source
                // file to recover its authoritative body + source start line +
                // sub-symbol span map; for an owned chunk the text is the chunk
                // body and there is no source-file coordinate.
                let from = chunk.from.as_ref().map(|p| p.display().to_string());
                let symbol = chunk.symbol.clone();
                let mut text = None;
                let mut source_start_line = None;
                let mut span_map = None;

                if chunk.is_from_ref() {
                    if let (Some(rel), Some(sym)) = (&chunk.from, &chunk.symbol) {
                        let source_file = workspace_root.join(rel);
                        if let Ok(source) = std::fs::read_to_string(&source_file) {
                            if let Ok(span) = extract_symbol(&source, sym) {
                                source_start_line = Some(span.start_line);
                                span_map = build_span_map(&span.body);
                                text = Some(span.body);
                            }
                        }
                    }
                    if text.is_none() {
                        // Re-extraction unavailable (missing file / unnamed
                        // symbol); fall back to whatever body the doc carries.
                        let body = chunk.combined_body();
                        if !body.is_empty() {
                            text = Some(body);
                        }
                    }
                } else if !chunk.is_media {
                    let body = chunk.combined_body();
                    if !body.is_empty() {
                        text = Some(body);
                    }
                }

                chunks.push(ChunkSummary {
                    name: name.clone(),
                    kind: kind.to_string(),
                    lang: chunk.lang.clone(),
                    lines,
                    from,
                    symbol,
                    text,
                    source_start_line,
                    span_map,
                });
            }
        }
    }

    Ok(Some(DocEntry {
        id,
        path: rel_path,
        title,
        doc_type,
        status,
        body_format: extract_body_format(&content),
        concerns,
        edges,
        tangle_crate: parsed.tangle_crate,
        tangle_root: parsed.tangle_root.map(|p| p.display().to_string()),
        modified_us,
        chunks,
    }))
}
```

`list_symbols` reports 1-based lines against whatever text it is given, so
handing it the extracted body yields chunk-relative ranges for free.

```rust {#build-span-map}
/// Build a chunk-relative symbol-relative span map from an extracted chunk
/// body. `list_symbols` reports 1-based line numbers against the text it is
/// given, so passing the extracted body yields ranges relative to the chunk
/// itself (stable under source-file moves). Returns `None` when the body has no
/// nameable sub-symbols, so the field is omitted rather than serialized empty.
fn build_span_map(body: &str) -> Option<Vec<SpanMapEntry>> {
    let symbols = list_symbols(body).ok()?;
    if symbols.is_empty() {
        return None;
    }
    Some(
        symbols
            .into_iter()
            .map(|s| SpanMapEntry {
                symbol: s.name,
                start_line: s.start_line,
                end_line: s.end_line,
            })
            .collect(),
    )
}
```

## Reading the envelope without a YAML parser

The index reads five envelope fields by line scanning: `body_format`, the
first `# ` heading as title, and the `id`/`type`/`status`/`concerns`/`edges`
quintuple. This is the third frontmatter reader in the published crates —
`x0k_folio::colophon::parse_envelope` is the typed one, and
`x0k-tangle`'s presentation module has a splitter of its own — and it is
kept because it tolerates envelopes the typed parser rejects, which is what
an index over a working corpus needs. The scanner tracks two list contexts,
`concerns:` (inline `[a, b]` or dashed lines) and `edges:` (a predicate key
followed by dashed values), and leaves the edges context at the first
non-indented, non-comment key.

```rust {#extract-body-format}
/// Scan the frontmatter for `body_format:`, defaulting to `"markdown"` when
/// absent (matches `x0k_folio::colophon::normalize_body_format`).
fn extract_body_format(content: &str) -> String {
    let mut in_frontmatter = false;
    for line in content.lines() {
        if line.trim() == "---" {
            if in_frontmatter {
                break;
            }
            in_frontmatter = true;
            continue;
        }
        if !in_frontmatter {
            continue;
        }
        if let Some(val) = line.trim().strip_prefix("body_format:") {
            let v = val.trim();
            if !v.is_empty() {
                return v.to_string();
            }
        }
    }
    "markdown".to_string()
}
```

```rust {#extract-title}
fn extract_title(content: &str) -> String {
    for line in content.lines() {
        let trimmed = line.trim();
        if let Some(heading) = trimmed.strip_prefix("# ") {
            return heading.to_string();
        }
    }
    String::new()
}
```

```rust {#extract-frontmatter-fields}
fn extract_frontmatter_fields(
    content: &str,
) -> (
    String,
    String,
    String,
    Vec<String>,
    BTreeMap<String, Vec<String>>,
) {
    let mut id = String::new();
    let mut doc_type = String::new();
    let mut status = String::new();
    let mut concerns = Vec::new();
    let mut edges: BTreeMap<String, Vec<String>> = BTreeMap::new();

    let mut in_frontmatter = false;
    let mut in_edges = false;
    let mut in_concerns = false;
    let mut current_predicate = String::new();

    for line in content.lines() {
        if line.trim() == "---" {
            if in_frontmatter {
                break;
            }
            in_frontmatter = true;
            continue;
        }
        if !in_frontmatter {
            continue;
        }

        let trimmed = line.trim();

        if let Some(val) = trimmed.strip_prefix("id:") {
            id = val.trim().to_string();
            in_edges = false;
            in_concerns = false;
        } else if let Some(val) = trimmed.strip_prefix("type:") {
            doc_type = val.trim().to_string();
            in_edges = false;
            in_concerns = false;
        } else if let Some(val) = trimmed.strip_prefix("status:") {
            status = val.trim().to_string();
            in_edges = false;
            in_concerns = false;
        } else if trimmed.starts_with("concerns:") {
            in_concerns = true;
            in_edges = false;
            // Inline array: concerns: [a, b, c]
            if let Some(arr) = trimmed.strip_prefix("concerns:") {
                let arr = arr.trim();
                if arr.starts_with('[') && arr.ends_with(']') {
                    concerns = arr[1..arr.len() - 1]
                        .split(',')
                        .map(|s| s.trim().to_string())
                        .filter(|s| !s.is_empty())
                        .collect();
                    in_concerns = false;
                }
            }
        } else if trimmed == "edges:" {
            in_edges = true;
            in_concerns = false;
        } else if in_concerns && trimmed.starts_with("- ") {
            concerns.push(trimmed[2..].trim().to_string());
        } else if in_edges {
            if let Some(item) = trimmed.strip_prefix("- ") {
                let val = item.trim().to_string();
                edges
                    .entry(current_predicate.clone())
                    .or_default()
                    .push(val);
            } else if trimmed.ends_with(':') && !trimmed.starts_with('-') {
                current_predicate = trimmed.trim_end_matches(':').trim().to_string();
            } else if !trimmed.is_empty()
                && !trimmed.starts_with('#')
                && !line.starts_with("    ")
                && !line.starts_with("\t\t")
            {
                in_edges = false;
            }
        }
    }

    (id, doc_type, status, concerns, edges)
}
```

## Tests

The last test writes a source file and a document into a fresh temp
directory and asserts the carried example: source coordinates on `verdict`,
a chunk-relative span map on `shelf`.

```rust {#tests}
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extract_fields_basic() {
        let content = r#"---
x0k:
  format: folio/v1
  id: x0k:design/test
  type: design
  status: proposed
  concerns: [ui, lod]
  edges:
    cites:
      - x0k:wiki/foo
      - x0k:wiki/bar
---
# Test Document

Body here.
"#;
        let (id, doc_type, status, concerns, edges) = extract_frontmatter_fields(content);
        assert_eq!(id, "x0k:design/test");
        assert_eq!(doc_type, "design");
        assert_eq!(status, "proposed");
        assert_eq!(concerns, vec!["ui", "lod"]);
        assert_eq!(edges["cites"], vec!["x0k:wiki/foo", "x0k:wiki/bar"]);
    }

    #[test]
    fn extract_title_from_h1() {
        let content = "---\nx0k:\n  format: folio/v1\n---\n# My Title\n\nBody.";
        assert_eq!(extract_title(content), "My Title");
    }

    #[test]
    fn from_chunk_summary_carries_source_coords() {
        // A `from=`/`symbol=` chunk's index summary should carry the source
        // file, the symbol, the extracted text, the source start line, and a
        // chunk-relative span map — the coordinates a worked figure binds to.
        let dir = std::env::temp_dir().join(format!(
            "x0k-tangle-index-test-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(&dir).unwrap();

        // Source file: a documented function the figure binds, plus an impl
        // block whose body holds nameable sub-symbols (methods) for the span
        // map. Two leading lines push `classify_range` off line 1, so
        // `source_start_line` is meaningfully > 1.
        let src_rel = "machine.rs";
        let source = "\
// preamble line
use std::cmp;

/// Classify a range into one of three verdicts.
fn classify_range(local: u32, remote: u32) -> bool {
    local == remote
}

impl Shelf {
    fn seal(&self) -> u32 {
        0
    }
    fn merge(&self) -> u32 {
        1
    }
}
";
        std::fs::write(dir.join(src_rel), source).unwrap();

        let doc = format!(
            "---\nx0k:\n  format: folio/v1\n  id: x0k:implementation/test/doc\n  type: implementation\n  status: draft\n---\n# Test Doc\n\n```rust {{#verdict from=\"{src_rel}\" symbol=\"classify_range\"}}\n```\n\n```rust {{#shelf from=\"{src_rel}\" symbol=\"Shelf\"}}\n```\n"
        );
        let doc_path = dir.join("doc.md");
        std::fs::write(&doc_path, doc).unwrap();

        let index = build_index(&[doc_path], &dir).unwrap();
        let entry = index
            .docs
            .iter()
            .find(|d| d.id == "x0k:implementation/test/doc")
            .unwrap();

        // The bound function: source file, symbol, extracted text, and source
        // start line all populate — the coordinates a worked figure needs.
        let verdict = entry.chunks.iter().find(|c| c.name == "verdict").unwrap();
        assert_eq!(verdict.kind, "from");
        assert_eq!(verdict.from.as_deref(), Some(src_rel));
        assert_eq!(verdict.symbol.as_deref(), Some("classify_range"));
        let text = verdict
            .text
            .as_deref()
            .expect("from chunk carries extracted text");
        assert!(text.contains("fn classify_range"), "text = {text:?}");
        // `fn classify_range` begins on line 5 of the source file (1-based).
        assert_eq!(verdict.source_start_line, Some(5));

        // The impl block: its method bodies give the span map nested
        // sub-symbols with chunk-relative line numbers. `impl Shelf` is
        // chunk-relative line 1; `seal` is line 2.
        let shelf = entry.chunks.iter().find(|c| c.name == "shelf").unwrap();
        let span_map = shelf
            .span_map
            .as_ref()
            .expect("impl chunk carries a span map");
        let seal = span_map
            .iter()
            .find(|e| e.symbol.ends_with("seal"))
            .expect("span map names the seal method");
        assert_eq!(seal.start_line, 2, "seal is chunk-relative line 2");
        assert!(span_map.iter().any(|e| e.symbol.ends_with("merge")));

        std::fs::remove_dir_all(&dir).ok();
    }
}
```

## The file

```rust {#root}
<<module-doc>>

<<doc-index>>

<<chunk-summary>>

<<build-index>>

<<index-file>>

<<build-span-map>>

<<extract-body-format>>

<<extract-title>>

<<extract-frontmatter-fields>>

<<tests>>
```

The index is a projection of the corpus and nothing more — it holds no
state a re-walk cannot rebuild — which is why the tolerant scanner is
acceptable here where it would not be in the envelope validator: an index
that is wrong about one document's `status` costs a sidebar row, and the
next walk corrects it.
