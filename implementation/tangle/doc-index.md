---
x0k:
  format: folio/v1
  id: x0k:implementation/tangle/doc-index
  type: implementation
  status: draft
  summary: The one-pass walk that emits a serializable index of a corpus — envelope fields, tangle target, mtime, and per-chunk coordinates — so a sidebar or a figure can render a chunk without re-parsing its document.
  concerns:
  - tangle
  - index
  - chunks
  - source-refs
  - authoring-ui
  - figures
  tangle:
    crate: crates/x0k-tangle
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
code, and any tool that wants to list the [literate
corpus](../../background/literate-programming.md "x0k:wiki/literate-programming") all ask the same
question: what documents are here, and what chunks do they carry? Parsing
every document on every ask is too slow for a sidebar and too coupled for a
figure. So `x0k-tangle index` walks a set of paths once and emits a
serializable `DocIndex`: one `DocEntry` per folio/v1 document with its
envelope fields, its tangle target, its modification time, and a
`ChunkSummary` per chunk carrying enough coordinates that a consumer can
render the chunk's code without re-parsing the document.

The carried example is a document with two `from=` chunks —

<a name="chunk-verdict"></a><sub>[`src/index.rs`](../../crates/x0k-tangle/src/index.rs) · `#verdict`</sub>

    ```rust {#verdict from="machine.rs" symbol="classify_range"}
    ```
<a name="chunk-shelf"></a><sub>[`src/index.rs`](../../crates/x0k-tangle/src/index.rs) · `#shelf`</sub>

    ```rust {#shelf from="machine.rs" symbol="Shelf"}
    ```

— whose index entry reports, for `verdict`, the source file, the symbol,
the extracted text, and the 1-based source line the symbol begins on; and
for `shelf`, a **span map** naming `seal` and `merge` with line ranges
relative to the extracted chunk rather than the source file. A figure step
that says "highlight `seal`" resolves to a line band through that map, and
the band stays correct when `machine.rs` gains a preamble.

<a name="chunk-module-doc"></a><sub>[`src/index.rs`](../../crates/x0k-tangle/src/index.rs) · `#module-doc`</sub>

```rust {#module-doc}
use crate::parser::parse_document;
use crate::source_ref::{extract_symbol_in, list_symbols_in, SymbolLanguage};
use anyhow::Result;
use serde::{Deserialize, Serialize};
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

`title` and `summary` are the two strings a list row is built out of — the
name of the entry and the line under it — so both are always present, empty
when the document offers nothing to fill them. A consumer that had to
distinguish "absent" from "empty" here would be asking a question the corpus
cannot answer: a document with no summary and a document whose summary is the
empty string are the same document.

<a name="chunk-doc-index"></a><sub>[`src/index.rs`](../../crates/x0k-tangle/src/index.rs) · `#doc-index`</sub>

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
    /// The envelope's `summary` — the line a reader is offered under the
    /// title. Empty when the document declares none.
    pub summary: String,
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

<a name="chunk-chunk-summary"></a><sub>[`src/index.rs`](../../crates/x0k-tangle/src/index.rs) · `#chunk-summary`</sub>

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

<a name="chunk-build-index"></a><sub>[`src/index.rs`](../../crates/x0k-tangle/src/index.rs) · `#build-index`</sub>

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
its source file *with the grammar its fence declares*, to recover the
authoritative body, start line, and span map; when re-extraction is
unavailable the chunk's own body stands in; an owned chunk's text is its
combined body, and a media chunk has none.

The language is the chunk's, never an assumption. A `ts` chunk read with the
Rust grammar matches no symbol, so a non-Rust `from=` chunk used to fall all
the way back to its own (empty) doc body and no span map — a silent hole in
the doc browser rather than a message. A fence in a language symbol
extraction has no grammar for (`toml`, say) is the same shape and gets the
same answer: no coordinates, quietly. Every failure on this path already
falls back to the doc's own body, so an unsupported language costs a chunk
its span map and nothing else — the index still builds.

<a name="chunk-index-file"></a><sub>[`src/index.rs`](../../crates/x0k-tangle/src/index.rs) · `#index-file`</sub>

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

    // One read of the `x0k:` block; every envelope field below comes out
    // of it.
    let envelope = envelope_fields(&content);

    // The last fallback is this caller's to supply: an entry always has a
    // file, so a document that names itself nowhere is listed under its stem.
    let title = document_title(&content).unwrap_or_else(|| {
        path.file_stem()
            .map(|stem| stem.to_string_lossy().to_string())
            .unwrap_or_default()
    });

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
                        let lang = SymbolLanguage::for_lang(chunk.lang.as_deref());
                        if let (Ok(source), Ok(lang)) =
                            (std::fs::read_to_string(&source_file), lang)
                        {
                            if let Ok(span) = extract_symbol_in(&source, sym, lang) {
                                source_start_line = Some(span.start_line);
                                span_map = build_span_map(&span.body, lang);
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
        id: envelope.id,
        path: rel_path,
        title,
        summary: envelope.summary,
        doc_type: envelope.doc_type,
        status: envelope.status,
        body_format: envelope.body_format,
        concerns: envelope.concerns,
        edges: envelope.edges,
        tangle_crate: parsed.tangle_crate,
        tangle_root: parsed.tangle_root.map(|p| p.display().to_string()),
        modified_us,
        chunks,
    }))
}
```

`list_symbols_in` reports 1-based lines against whatever text it is given, so
handing it the extracted body yields chunk-relative ranges for free. It takes
the same language the extraction ran under: the body came out of that grammar,
and listing it under another one would be reading the extract with the wrong
eyes.

<a name="chunk-build-span-map"></a><sub>[`src/index.rs`](../../crates/x0k-tangle/src/index.rs) · `#build-span-map`</sub>

```rust {#build-span-map}
/// Build a chunk-relative symbol-relative span map from an extracted chunk
/// body, read under the grammar the body was extracted with.
/// `list_symbols_in` reports 1-based line numbers against the text it is
/// given, so passing the extracted body yields ranges relative to the chunk
/// itself (stable under source-file moves). Returns `None` when the body has no
/// nameable sub-symbols, so the field is omitted rather than serialized empty.
fn build_span_map(body: &str, lang: SymbolLanguage) -> Option<Vec<SpanMapEntry>> {
    let symbols = list_symbols_in(body, lang).ok()?;
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

## Reading the envelope

The index reads the `x0k:` block with `serde_norway` — the same YAML parser
`x0k_folio::colophon::parse_envelope` reads it with, and the one `ingest`
projects from — into a struct whose every field is optional and whose
unknown keys are ignored. That is the whole difference between this reader
and the typed one: it agrees with them about what the YAML *says* and asks
nothing about what it means. A `type:` naming a class no vocabulary
declares, a `status:` outside the six — `check` refuses both and the index
carries both, because an index over a working corpus describes what is
there rather than judging it.

It did not always agree about what the YAML says. Until this reader the
index scanned the frontmatter line by line, and two ordinary spellings fell
through the scan. A flow sequence under `edges:` —
`refined_by: [x0k:design/block]` — produced `"edges": {}`, silently: the
document went into the index looking like one with no edges at all while
`check` and `ingest` both saw the edge. And a quoted scalar kept its
quotes, so `id: "x0k:design/quoted"` was indexed as `"\"x0k:design/quoted\""`
and matched nothing a consumer could type (both jj, 2026-09-23). Each is a
property of YAML a line scanner has to re-implement one spelling at a time,
and the scanner was always going to be a spelling behind.

The scanner stays, one rung down, for the envelopes the parser refuses. A
summary with a stray `: ` in it is not well-formed YAML — eleven of x0k's
own documents are in that state as this is written — and `check` says so,
which is `check`'s job. The index's job is to still show you the document,
so when the parse fails the line scan answers instead: worse, tolerant, and
reached only where the typed verbs have already refused. Nothing that
`check` accepts is read by it.

<a name="chunk-envelope-fields"></a><sub>[`src/index.rs`](../../crates/x0k-tangle/src/index.rs) · `#envelope-fields`</sub>

```rust {#envelope-fields}
/// The envelope fields an index entry is built from.
///
/// Every field is optional, which is what lets this share a parser with
/// the typed envelope without sharing its refusals: `type` here is the
/// string the author wrote, not a genus.
#[derive(Default, Deserialize)]
#[serde(default)]
struct EnvelopeFields {
    id: String,
    #[serde(rename = "type")]
    doc_type: String,
    status: String,
    summary: String,
    concerns: Vec<String>,
    edges: BTreeMap<String, Vec<String>>,
    /// Body format dispatch flag. Defaulted by [`envelope_fields`] rather
    /// than here, so an absent and an empty `body_format:` read alike.
    body_format: String,
}

#[derive(Deserialize)]
struct EnvelopeRoot {
    x0k: Option<EnvelopeFields>,
}

/// Read the `x0k:` block out of a document's frontmatter, by parser where
/// the frontmatter is YAML and by line scan where it is not.
fn envelope_fields(content: &str) -> EnvelopeFields {
    let scanned = || scan_envelope_fields(content);
    let (Some(frontmatter), _) = split_frontmatter(content) else {
        return scanned();
    };
    let parsed = serde_norway::from_str::<EnvelopeRoot>(frontmatter)
        .ok()
        .and_then(|root| root.x0k);
    match parsed {
        Some(mut fields) => {
            if fields.body_format.is_empty() {
                fields.body_format = "markdown".to_string();
            }
            fields
        }
        None => scanned(),
    }
}
```

The scanner below is what the index used to read every envelope with, and
what it now reads only the malformed ones with. It tracks two list
contexts, `concerns:` and `edges:`, leaves the edges context at the first
non-indented non-comment key, and accepts both list spellings in each.
`summary:` gets a third context, because it is the one envelope field
written as a paragraph: a corpus writes it inline most of the time, quoted
about half the time, and occasionally as a `>-` folded block whose text is
on the lines below. The scanner joins those continuation lines with spaces,
which is what folding means, and stops at the next key at the summary's own
indentation.

<a name="chunk-extract-body-format"></a><sub>[`src/index.rs`](../../crates/x0k-tangle/src/index.rs) · `#extract-body-format`</sub>

```rust {#extract-body-format}
/// Scan the frontmatter for `body_format:`, defaulting to `"markdown"` when
/// absent (matches `x0k_folio::colophon::normalize_body_format`). The line
/// scanner's half of [`envelope_fields`]; the parser reads the field off
/// the envelope directly.
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

## What a document is called

A title is the *name* of an entry, and a corpus offers it in more than one
place. x0k's own documents open with a `# ` heading, so that is the first
thing asked for. But the two conventions a Markdown corpus outside this one
is most likely to be written in both fail that question: a Docusaurus or
MkDocs site puts the name in the host frontmatter's `title:` and starts the
body at `## Context`, and a reference page generated from data has no heading
at all. Asking only for `# ` answered `""` for all fifteen of a real ADR log
and for every generated table beside it.

Worse, the old scanner read the whole file, frontmatter and fenced code
together, so the first line anywhere that happened to begin `# ` won. On a
Python page that was a comment three hundred lines into an example —
`# return None if the discriminator value isn't found` — presented to a
reader as the name of the page. A code fence is a quotation, and nothing
inside a quotation names the document that quotes it.

The `##` a page leads with is its name; a `##` further down is not. The
fallback for "no `# `, no host `title:`" was the first heading of any level
anywhere in the body, and on a corpus that keeps its page names in an
MkDocs `nav:` — where nothing in the file says the name at all — that
answered with a section. A version policy indexed as *Pydantic V1*, a page
about unions as *Union Modes*: plausible, confident, and wrong in the way a
blank was not, because a reader has no reason to doubt it. A heading that
opens a body is the page starting with its own name. A heading below prose
is a division of a page that has already started, and it names the division.

So the resolution walks five sources in the order a reader would, and the
document says which one it took by which one is non-empty:

1. the body's first `# ` heading, outside every fence;
2. an opening `<h1>`, for the documents whose `body_format` is `html` and
   whose heading is therefore a tag rather than a hash — two of x0k's own
   design documents, which indexed as `""` for the same reason the ADRs did;
3. the host frontmatter's own top-level `title:`, which is where Docusaurus
   and MkDocs keep the name;
4. a heading of *any* level that the body **opens** with — the first
   non-blank line — for a page written under a `##`;
5. the envelope's own `summary`, which is the document describing itself
   and is never about a part of it;
6. the filename stem — always available, never wrong about identity even
   when it is ugly.

The resolver stops at 5 and returns `None`, because the fallback at 6 is not
the same fallback for every caller: `index` has a path to take a stem from,
and `weave` has only the document's id.

A summary is a sentence where a title wants a phrase, so 5 is a demotion,
not a discovery — it is there because the alternative it replaced was a
section name asserting itself as the page's, and a document's own sentence
about itself is at least about the whole document. A corpus that dislikes
the sentence has the fix in its own hands: give the page an `# ` heading.

<a name="chunk-document-title"></a><sub>[`src/index.rs`](../../crates/x0k-tangle/src/index.rs) · `#document-title`</sub>

```rust {#document-title}
/// A document's title, resolved the way a reader would ask for it: the body's
/// first `# ` heading, then an opening `<h1>` for an HTML body, then the host
/// frontmatter's `title:`, then a heading the body opens with, then the
/// envelope's `summary`. Fenced regions are skipped — a `#` comment inside an
/// example names nothing — and a heading below prose names its section rather
/// than the page. `None` when the document offers no name at all, leaving the
/// last fallback (a filename stem, a document id) to the caller that has one.
pub fn document_title(content: &str) -> Option<String> {
    let (frontmatter, body) = split_frontmatter(content);
    if let Some(h1) = first_heading(body, |level| level == 1) {
        return Some(h1);
    }
    if let Some(h1) = first_html_h1(body) {
        return Some(h1);
    }
    if let Some(host) = frontmatter.and_then(host_frontmatter_title) {
        return Some(host);
    }
    if let Some(opening) = opening_heading(body) {
        return Some(opening);
    }
    // The envelope parser, not a second reader of `summary:` — that key has
    // three spellings and one of them is a folded block.
    let summary = envelope_fields(content).summary;
    (!summary.is_empty()).then_some(summary)
}
```

A heading opens a body when it is the body's first non-blank line. Nothing
can have quoted it by then, so this one needs no fence tracking.

<a name="chunk-opening-heading"></a><sub>[`src/index.rs`](../../crates/x0k-tangle/src/index.rs) · `#opening-heading`</sub>

```rust {#opening-heading}
/// The heading a body opens with, at any level, or `None` when the body opens
/// with anything else. The first non-blank line is the only line that can
/// name the page rather than a part of it.
fn opening_heading(body: &str) -> Option<String> {
    let first = body.lines().map(str::trim).find(|line| !line.is_empty())?;
    let hashes = first.chars().take_while(|&c| c == '#').count();
    if !(1..=6).contains(&hashes) {
        return None;
    }
    let heading = first[hashes..].strip_prefix(' ')?.trim().trim_end_matches('#').trim();
    (!heading.is_empty()).then(|| heading.to_string())
}
```

An HTML body's heading is a tag, and the text between the tags is all that is
wanted — no entity decoding, no nested markup, because a title that needs
either is not a title.

<a name="chunk-first-html-h1"></a><sub>[`src/index.rs`](../../crates/x0k-tangle/src/index.rs) · `#first-html-h1`</sub>

```rust {#first-html-h1}
/// The text of the first `<h1>` in an HTML body. Bodies with
/// `body_format: html` carry their heading as a tag, so the hash scanner
/// finds nothing in them at all.
fn first_html_h1(body: &str) -> Option<String> {
    let open = body.find("<h1")?;
    let after_tag = body[open..].find('>')? + open + 1;
    let close = body[after_tag..].find("</h1>")? + after_tag;
    let text = body[after_tag..close].trim();
    (!text.is_empty() && !text.contains('<')).then(|| text.to_string())
}
```

A fence opens with three or more backticks or tildes and closes on a run of
the same character at least as long, which is how a Markdown example quotes
another Markdown example. Tracking the opening run's character and length is
the whole of it.

<a name="chunk-first-heading"></a><sub>[`src/index.rs`](../../crates/x0k-tangle/src/index.rs) · `#first-heading`</sub>

```rust {#first-heading}
/// The first ATX heading in a body whose level satisfies `accept`, skipping
/// fenced regions. A fence closes on a run of its own character at least as
/// long as the one that opened it, so an example containing a shorter fence
/// stays quoted.
fn first_heading(body: &str, accept: impl Fn(usize) -> bool) -> Option<String> {
    let mut fence: Option<(char, usize)> = None;
    for line in body.lines() {
        let trimmed = line.trim_start();
        if let Some((marker, opened)) = fence {
            let run = trimmed.chars().take_while(|&c| c == marker).count();
            if run >= opened && trimmed[run..].trim().is_empty() {
                fence = None;
            }
            continue;
        }
        if let Some(marker) = trimmed.chars().next().filter(|c| *c == '`' || *c == '~') {
            let run = trimmed.chars().take_while(|&c| c == marker).count();
            if run >= 3 {
                fence = Some((marker, run));
                continue;
            }
        }
        let hashes = trimmed.chars().take_while(|&c| c == '#').count();
        if accept(hashes) {
            if let Some(heading) = trimmed[hashes..].strip_prefix(' ') {
                let heading = heading.trim().trim_end_matches('#').trim();
                if !heading.is_empty() {
                    return Some(heading.to_string());
                }
            }
        }
    }
    None
}
```

The host's `title:` is the one at column zero. Everything the envelope owns
lives indented under `x0k:`, so indentation is what separates "the site's
name for this page" from any key nested inside a block — and the envelope has
no `title` of its own to be confused with.

<a name="chunk-host-frontmatter-title"></a><sub>[`src/index.rs`](../../crates/x0k-tangle/src/index.rs) · `#host-frontmatter-title`</sub>

```rust {#host-frontmatter-title}
/// The host frontmatter's own `title:` — the top-level key, at column zero,
/// which is where Docusaurus and MkDocs keep a page's name. Anything indented
/// belongs to a nested block (the `x0k:` envelope among them) and is not the
/// host's.
fn host_frontmatter_title(frontmatter: &str) -> Option<String> {
    frontmatter
        .lines()
        .filter(|line| !line.starts_with([' ', '\t']))
        .find_map(|line| line.strip_prefix("title:"))
        .map(unquote_scalar)
        .filter(|title| !title.is_empty())
}
```

<a name="chunk-split-frontmatter"></a><sub>[`src/index.rs`](../../crates/x0k-tangle/src/index.rs) · `#split-frontmatter`</sub>

```rust {#split-frontmatter}
/// Split a document into its frontmatter (without the `---` fences) and its
/// body. A file that does not open with `---`, or never closes it, is all
/// body.
fn split_frontmatter(content: &str) -> (Option<&str>, &str) {
    let Some(rest) = content.strip_prefix("---") else {
        return (None, content);
    };
    let Some(end) = rest.find("\n---") else {
        return (None, content);
    };
    let body = rest[end + 4..].trim_start_matches(['\r', '\n']);
    (Some(&rest[..end]), body)
}
```

<a name="chunk-unquote-scalar"></a><sub>[`src/index.rs`](../../crates/x0k-tangle/src/index.rs) · `#unquote-scalar`</sub>

```rust {#unquote-scalar}
/// A YAML scalar as the line scanner sees it: trimmed, and stripped of one
/// matched pair of surrounding quotes. Enough for the values a title or a
/// summary is written as; the typed parser
/// (`x0k_folio::colophon::parse_envelope`) is the one that owns YAML.
fn unquote_scalar(value: &str) -> String {
    let value = value.trim();
    for quote in ['"', '\''] {
        if value.len() >= 2 && value.starts_with(quote) && value.ends_with(quote) {
            return value[1..value.len() - 1].to_string();
        }
    }
    value.to_string()
}
```

<a name="chunk-scan-envelope-fields"></a><sub>[`src/index.rs`](../../crates/x0k-tangle/src/index.rs) · `#scan-envelope-fields`</sub>

```rust {#scan-envelope-fields}
/// The envelope as a line scan sees it. Reached only for frontmatter
/// `serde_norway` refuses: it is behind the parser on every YAML spelling
/// and always will be, and it is still better than an index that goes
/// blank on a document mid-edit.
fn scan_envelope_fields(content: &str) -> EnvelopeFields {
    /// The predicate in `predicate: [a, b]`, or None when the line is not
    /// that. Split at the last colon before the bracket, because the key may
    /// carry a module prefix and the values are ids full of colons.
    fn inline_edge_key(line: &str) -> Option<String> {
        let open = line.find('[')?;
        line.ends_with(']')
            .then(|| line[..open].rfind(':'))
            .flatten()
            .map(|colon| line[..colon].trim().to_string())
            .filter(|predicate| !predicate.is_empty())
    }

    let mut id = String::new();
    let mut doc_type = String::new();
    let mut status = String::new();
    let mut summary = String::new();
    let mut concerns = Vec::new();
    let mut edges: BTreeMap<String, Vec<String>> = BTreeMap::new();

    let mut in_frontmatter = false;
    let mut in_edges = false;
    let mut in_concerns = false;
    let mut summary_block: Option<usize> = None;
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
        let indent = line.len() - line.trim_start().len();

        // A folded `summary: >-` runs until the next key at its own
        // indentation; its continuation lines join with spaces, which is what
        // folding means.
        if let Some(opened_at) = summary_block {
            if trimmed.is_empty() || indent > opened_at {
                if !trimmed.is_empty() {
                    if !summary.is_empty() {
                        summary.push(' ');
                    }
                    summary.push_str(trimmed);
                }
                continue;
            }
            summary_block = None;
        }

        if let Some(val) = trimmed.strip_prefix("summary:") {
            in_edges = false;
            in_concerns = false;
            let val = val.trim();
            if val.starts_with('>') || val.starts_with('|') {
                summary_block = Some(indent);
                summary.clear();
            } else {
                summary = unquote_scalar(val);
            }
        } else if let Some(val) = trimmed.strip_prefix("id:") {
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
            } else if let Some(predicate) = inline_edge_key(trimmed) {
                // `cites: [a, b]`. The key ends at the last colon before the
                // bracket: a prefixed predicate has one inside the key and
                // every `x0k:` target has one inside a value.
                let open = trimmed.find('[').unwrap_or(trimmed.len());
                let values = trimmed[open..].trim_matches(['[', ']'].as_slice());
                edges.entry(predicate).or_default().extend(
                    values
                        .split(',')
                        .map(|s| s.trim().to_string())
                        .filter(|s| !s.is_empty()),
                );
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

    EnvelopeFields {
        id,
        doc_type,
        status,
        summary,
        concerns,
        edges,
        body_format: extract_body_format(content),
    }
}
```

## Tests

The title tests are the sources in order, each written as the corpus that
actually produced it: an x0k document with an `# ` heading, a Docusaurus ADR
whose name is in the host frontmatter and whose body opens at `## Context`, a
page that opens with `##` and quotes a `#` comment inside a fence three
hundred lines down, an MkDocs page whose `##` arrives after prose and which
must therefore *not* be named by it, and a generated table with no heading at
all. All but the first were wrong once — the fenced-comment case presented a
Python comment as the name of a page, and the prose-then-`##` case presented
a section as one — so each names the evaluation that found it.

The last three tests each write a source file and a document into a fresh
temp directory. The first asserts the carried example: source coordinates on
`verdict`, a chunk-relative span map on `shelf`. The second is the same
example in JavaScript, and it is the one that pins the fence-language
dispatch — under the Rust grammar its class matches nothing and both
coordinates come back empty. The third is a `toml` chunk: no grammar, no
coordinates, and an index that still builds.

<a name="chunk-tests"></a><sub>[`src/index.rs`](../../crates/x0k-tangle/src/index.rs) · `#tests`</sub>

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
        let fields = envelope_fields(content);
        assert_eq!(fields.id, "x0k:design/test");
        assert_eq!(fields.doc_type, "design");
        assert_eq!(fields.status, "proposed");
        assert_eq!(fields.summary, "");
        assert_eq!(fields.concerns, vec!["ui", "lod"]);
        assert_eq!(fields.edges["cites"], vec!["x0k:wiki/foo", "x0k:wiki/bar"]);
        assert_eq!(fields.body_format, "markdown");
    }

    /// A quoted id is a quoted id in YAML and a bare one to every consumer.
    /// The line scanner kept the quote characters, so `index` emitted an id
    /// nothing could match while `check` accepted the document and `ingest`
    /// projected it correctly (jj, 2026-09-23).
    #[test]
    fn quoted_scalars_index_without_their_quotes() {
        let content = "---\nx0k:\n  format: folio/v1\n  id: \"x0k:design/quoted\"\n  \
            type: \"design\"\n  status: 'proposed'\n  concerns:\n    - \"x0k:concept/a\"\n  \
            edges:\n    refined_by:\n      - \"x0k:design/block\"\n---\n# Quoted\n";
        let fields = envelope_fields(content);
        assert_eq!(fields.id, "x0k:design/quoted");
        assert_eq!(fields.doc_type, "design");
        assert_eq!(fields.status, "proposed");
        assert_eq!(fields.concerns, vec!["x0k:concept/a"]);
        assert_eq!(fields.edges["refined_by"], vec!["x0k:design/block"]);
    }

    /// `index` refuses nothing: a genus and a status no vocabulary declares
    /// are carried as written, because an index describes the corpus and
    /// `check` judges it. Sharing a parser with the typed envelope must not
    /// import its refusals (Backstage, 2026-09-23).
    #[test]
    fn a_genus_and_status_outside_the_shipped_sets_are_carried_as_written() {
        let content = "---\nx0k:\n  format: folio/v1\n  id: bs:architecture/adr013\n  \
            type: adr\n  status: ratified\n---\n# ADR013\n";
        let fields = envelope_fields(content);
        assert_eq!(fields.doc_type, "adr");
        assert_eq!(fields.status, "ratified");
    }

    /// A summary carrying a bare `: ` is not well-formed YAML, and eleven of
    /// x0k's own documents are written that way. `check` refuses them; the
    /// index still lists them, off the line scanner, because a sidebar that
    /// empties itself mid-edit is worse than one a verb disagrees with.
    #[test]
    fn an_envelope_the_parser_refuses_falls_back_to_the_line_scan() {
        let content = "---\nx0k:\n  format: folio/v1\n  id: x0k:wiki/unison\n  type: wiki\n  \
            summary: Written for one purpose: the codec's third mode.\n  \
            edges:\n    cites:\n      - x0k:wiki/gallowglass\n---\n# Unison\n";
        assert!(serde_norway::from_str::<EnvelopeRoot>(
            split_frontmatter(content).0.unwrap()).is_err(),
            "the fixture has to be YAML the parser refuses, or this proves nothing");
        let fields = envelope_fields(content);
        assert_eq!(fields.id, "x0k:wiki/unison");
        assert_eq!(fields.edges["cites"], vec!["x0k:wiki/gallowglass"]);
        assert_eq!(fields.body_format, "markdown");
    }

    /// An envelope the typed parser accepts and indexes as having no edges
    /// at all is the worst kind of disagreement: nothing fails. This spelling
    /// ingested clean and produced `"edges": {}` until the index learned to
    /// read YAML with a YAML parser (2026-09-23, folio evaluation; the guide
    /// had blamed the vocabulary, but `cites` was dropped as readily as `bs:`).
    #[test]
    fn inline_edge_arrays_index_the_same_as_dashed_ones() {
        let document = |edges: &str| format!("---\nx0k:\n  format: folio/v1\n  \
            id: x0k:design/test\n  type: design\n{edges}---\n# Test\n");
        let inline = document(
            "  edges:\n    cites: [x0k:design/two, x0k:design/three]\n    \
             bs:superseded_by: [x0k:design/four]\n");
        let dashed = document(
            "  edges:\n    cites:\n      - x0k:design/two\n      - x0k:design/three\n    \
             bs:superseded_by:\n      - x0k:design/four\n");
        let edges = |content: &str| envelope_fields(content).edges;
        assert_eq!(edges(&inline), edges(&dashed));
        assert_eq!(edges(&inline)["cites"], vec!["x0k:design/two", "x0k:design/three"]);
        // The key is everything before the last colon outside the brackets.
        assert_eq!(edges(&inline)["bs:superseded_by"], vec!["x0k:design/four"]);
        assert!(edges(&document("  edges:\n    cites: []\n"))["cites"].is_empty());
    }

    #[test]
    fn summary_is_read_quoted_plain_and_folded() {
        // The three spellings a corpus writes a summary in. The folded block
        // is rare (two documents in x0k's own corpus) and the one the line
        // scanner underneath this reader returns empty.
        let quoted = "---\nx0k:\n  summary: \"ADR013: [superseded] fetching\"\n  status: superseded\n---\n";
        assert_eq!(
            envelope_fields(quoted).summary,
            "ADR013: [superseded] fetching"
        );

        let plain = "---\nx0k:\n  summary: Smart mode finds the best match.\n---\n";
        assert_eq!(
            envelope_fields(plain).summary,
            "Smart mode finds the best match."
        );

        let folded = "---\nx0k:\n  summary: >-\n    A germ is a picture\n    with provenance.\n  status: draft\n---\n";
        let fields = envelope_fields(folded);
        assert_eq!(fields.summary, "A germ is a picture with provenance.");
        assert_eq!(fields.status, "draft", "the folded block ends at the next key");
    }

    #[test]
    fn title_prefers_the_body_h1() {
        let content = "---\nx0k:\n  format: folio/v1\n---\n# My Title\n\nBody.";
        assert_eq!(document_title(content).as_deref(), Some("My Title"));
    }

    #[test]
    fn title_reads_an_html_body_s_opening_h1() {
        // `body_format: html` documents carry their heading as a tag; the hash
        // scanner finds nothing in them. Two of x0k's own design decisions.
        let content = "---\nx0k:\n  format: folio/v1\n  body_format: html\n---\n\n<h1>Files surface</h1>\n\n<p>A library you already know how to use.</p>\n";
        assert_eq!(document_title(content).as_deref(), Some("Files surface"));
    }

    #[test]
    fn title_falls_back_to_host_frontmatter_when_the_body_opens_at_h2() {
        // The Docusaurus/MkDocs shape: the name is a host frontmatter key and
        // the body starts at `## Context`. Fifteen real ADRs indexed as `""`
        // before this (Backstage evaluation, 2026-09-22).
        let content = "---\nid: adrs-adr013\ntitle: 'ADR013: [superseded] Proper use of HTTP fetching libraries'\nx0k:\n  format: folio/v1\n  type: architecture\n---\n\n## Context\n\nUsing multiple HTTP packages…\n";
        assert_eq!(
            document_title(content).as_deref(),
            Some("ADR013: [superseded] Proper use of HTTP fetching libraries")
        );
    }

    #[test]
    fn title_takes_a_heading_the_body_opens_with_and_skips_fenced_regions() {
        // A `#` comment inside an example is a quotation, not a name. This
        // page indexed as "return None if the discriminator value isn't found"
        // — a Python comment 342 lines in (pydantic evaluation, 2026-09-22).
        let content = "---\nx0k:\n  format: folio/v1\n---\n## Union Modes\n\nUnions are fundamentally different.\n\n```python\n# return None if the discriminator value isn't found\nreturn None\n```\n\n### Left to Right Mode\n";
        assert_eq!(document_title(content).as_deref(), Some("Union Modes"));
    }

    #[test]
    fn a_heading_below_prose_names_its_section_not_the_page() {
        // The same page as MkDocs actually writes it: the name is in `nav:`,
        // the body opens with prose, and `## Union Modes` is a division of a
        // page already under way. Titled "Union Modes" — plausible and wrong,
        // where the blank it replaced was merely absent (pydantic re-
        // evaluation, 2026-09-22). With no summary either, the resolver
        // declines and `index` falls back to the stem.
        let content = "---\nx0k:\n  format: folio/v1\n---\nUnions are fundamentally different.\n\n## Union Modes\n\n### Left to Right Mode\n";
        assert_eq!(document_title(content), None);
    }

    #[test]
    fn a_document_that_names_itself_nowhere_falls_back_to_its_summary() {
        // A generated reference table: no headings, no host `title:`. What it
        // does have is a sentence about itself, which is at least about the
        // whole of it.
        let content = "---\nx0k:\n  format: folio/v1\n  summary: What Pydantic converts to what.\n---\n| Field | Type |\n| --- | --- |\n| a | int |\n";
        assert_eq!(
            document_title(content).as_deref(),
            Some("What Pydantic converts to what.")
        );
    }

    #[test]
    fn a_document_with_no_heading_and_no_summary_has_no_title_of_its_own() {
        // Nothing in the file names it. The resolver declines and `index`
        // lists it under its filename stem.
        let content = "---\nx0k:\n  format: folio/v1\n---\n| Field | Type |\n| --- | --- |\n| a | int |\n";
        assert_eq!(document_title(content), None);
    }

    #[test]
    fn index_titles_a_headingless_document_with_its_filename() {
        let dir = std::env::temp_dir().join(format!(
            "x0k-tangle-index-stem-test-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        let doc_path = dir.join("conversion_table.md");
        std::fs::write(
            &doc_path,
            "---\nx0k:\n  format: folio/v1\n  id: pyd:concept/conversion-table\n  type: wiki\n  status: stable\n---\n| Field | Type |\n",
        )
        .unwrap();

        let index = build_index(&[doc_path], &dir).unwrap();
        let entry = &index.docs[0];
        assert_eq!(entry.title, "conversion_table");
        assert_eq!(entry.summary, "");

        std::fs::remove_dir_all(&dir).ok();
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
        assert_eq!(verdict.source_start_line, Some(4));

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

    #[test]
    fn from_chunk_in_javascript_carries_source_coords() {
        // The same carried example in another language. Read under the Rust
        // grammar — what this path did before it consulted the fence — the
        // class matches nothing, and the chunk comes back with no start line
        // and no span map at all.
        let dir = std::env::temp_dir().join(format!(
            "x0k-tangle-index-js-test-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(&dir).unwrap();

        let src_rel = "widget.js";
        let source = "\
// preamble line
const MODE = 1;

class Widget {
  render() {
    return MODE;
  }
  dispose() {
    return 0;
  }
}
";
        std::fs::write(dir.join(src_rel), source).unwrap();

        let doc = format!(
            "---\nx0k:\n  format: folio/v1\n  id: x0k:implementation/test/js\n  type: implementation\n  status: draft\n---\n# Test Doc\n\n```js {{#widget from=\"{src_rel}\" symbol=\"Widget\"}}\n```\n"
        );
        let doc_path = dir.join("doc.md");
        std::fs::write(&doc_path, doc).unwrap();

        let index = build_index(&[doc_path], &dir).unwrap();
        let entry = index
            .docs
            .iter()
            .find(|d| d.id == "x0k:implementation/test/js")
            .unwrap();
        let widget = entry.chunks.iter().find(|c| c.name == "widget").unwrap();

        let text = widget
            .text
            .as_deref()
            .expect("a js from chunk carries extracted text");
        assert!(text.contains("class Widget"), "text = {text:?}");
        // `class Widget` begins on line 4 of the source file (1-based).
        assert_eq!(widget.source_start_line, Some(4));
        let span_map = widget
            .span_map
            .as_ref()
            .expect("a js class chunk carries a span map");
        let render = span_map
            .iter()
            .find(|e| e.symbol.ends_with("render"))
            .expect("span map names the render method");
        assert_eq!(render.start_line, 2, "render is chunk-relative line 2");
        assert!(span_map.iter().any(|e| e.symbol.ends_with("dispose")));

        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn from_chunk_in_an_unextractable_language_degrades_quietly() {
        // `toml` has no symbol grammar. The index still builds; the chunk
        // simply carries no source coordinates, and the doc's own body (empty
        // here, as `from=` chunks are) stands in.
        let dir = std::env::temp_dir().join(format!(
            "x0k-tangle-index-toml-test-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("Cargo.toml"), "[package]\nname = \"demo\"\n").unwrap();

        let doc = "---\nx0k:\n  format: folio/v1\n  id: x0k:implementation/test/toml\n  type: implementation\n  status: draft\n---\n# Test Doc\n\n```toml {#pkg from=\"Cargo.toml\" symbol=\"package\"}\n```\n";
        let doc_path = dir.join("doc.md");
        std::fs::write(&doc_path, doc).unwrap();

        let index = build_index(&[doc_path], &dir).unwrap();
        let entry = index
            .docs
            .iter()
            .find(|d| d.id == "x0k:implementation/test/toml")
            .unwrap();
        let pkg = entry.chunks.iter().find(|c| c.name == "pkg").unwrap();
        assert_eq!(pkg.kind, "from");
        assert_eq!(pkg.source_start_line, None);
        assert!(pkg.span_map.is_none());

        std::fs::remove_dir_all(&dir).ok();
    }
}
```

## The file

<a name="chunk-root"></a><sub>[`src/index.rs`](../../crates/x0k-tangle/src/index.rs) · `#root` · assembles [module-doc](#chunk-module-doc) · [doc-index](#chunk-doc-index) · [chunk-summary](#chunk-chunk-summary) · [build-index](#chunk-build-index) · [index-file](#chunk-index-file) · [build-span-map](#chunk-build-span-map) · [extract-body-format](#chunk-extract-body-format) · [document-title](#chunk-document-title) · [first-heading](#chunk-first-heading) · [first-html-h1](#chunk-first-html-h1) · [host-frontmatter-title](#chunk-host-frontmatter-title) · [split-frontmatter](#chunk-split-frontmatter) · [unquote-scalar](#chunk-unquote-scalar) · [envelope-fields](#chunk-envelope-fields) · [scan-envelope-fields](#chunk-scan-envelope-fields) · [tests](#chunk-tests)</sub>

```rust {#root}
<<module-doc>>

<<doc-index>>

<<chunk-summary>>

<<build-index>>

<<index-file>>

<<build-span-map>>

<<extract-body-format>>

<<document-title>>

<<first-heading>>

<<first-html-h1>>

<<host-frontmatter-title>>

<<split-frontmatter>>

<<unquote-scalar>>

<<envelope-fields>>

<<scan-envelope-fields>>

<<tests>>
```

The index is a projection of the corpus and nothing more — it holds no
state a re-walk cannot rebuild — which is why the tolerant reader is
acceptable here where it would not be in the envelope validator: an index
that is wrong about one document's `status` costs a sidebar row, and the
next walk corrects it. Tolerant is about meaning, though, and never about
syntax — sharing `check`'s parser is what stops the two verbs describing
the same file differently.
