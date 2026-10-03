
# An index is the document seen from outside

```turtle folio:document
implementation:tangle\/doc-index a x0k:Implementation ;
    x0k:status "draft" ;
    x0k:summary "The one-pass walk that emits a serializable index of a corpus — envelope fields, tangle target, mtime, and per-chunk coordinates — so a sidebar or a figure can render a chunk without re-parsing its document." ;
    x0k:concerns "tangle", "index", "chunks", "source-refs", "authoring-ui", "figures" ;
    x0k:cites implementation:tangle\/parsing,
        implementation:tangle\/source-refs,
        implementation:folio\/colophon ;
    x0k:implements design:literate-programming ;
    folio:tangleCrate "crates/x0k-tangle" ;
    folio:tangleRoot "src/index.rs" .
```

The authoring UI's sidebar, a worked figure that binds to a document's real
code, and any tool that wants to list the [literate
corpus](../../background/literate-programming.md "x0k:wiki/literate-programming") all ask the same
question: what documents are here, and what chunks do they carry? Parsing
every document on every ask is too slow for a sidebar and too coupled for a
figure. So `x0k-tangle index` walks a set of paths once and emits a
serializable `DocIndex`: one `DocEntry` per document whose header names it,
with its header's fields, its tangle target, its modification time, and a
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
use serde::Serialize;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use x0k_folio::colophon::{
    find_header, header_literals, host_frontmatter, parse_envelope_in, parse_turtle,
    predeclared_prefixes, strip_header, turtle_name, FolioError,
};
use x0k_ontology::concept_facts::OntologyModel;
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
when the document offers nothing to fill them.

The keys are the header's, spelled the way the header's own reader spells
them. `id` is the header's subject, compact (`x0k:design/retry-budget`);
`doc_type` is the genus its class names, in kebab-case (`design`,
`implementation`); `status`, `summary`, `concerns` and `body_format` are
`x0k:status`, `x0k:summary`, `x0k:concerns` and `x0k:bodyFormat`. `edges`
is `Colophon.edges` as it stands: every statement whose object is an IRI,
keyed by the predicate's **compact term** — `"x0k:cites"`,
`"x0k:motivatedBy"`, `"x0k:publishes"` — with compact targets, so a key in
the index is the term an author typed in the header and the term a
consumer finds in the vocabulary.

`properties` is the header's other half: every statement whose object is a
literal, keyed by the predicate **as the header spells it** under the
predeclared prefixes — `"x0k:status"`, `"x0k:summary"`, `"x0k:confidence"`,
`"x0k:confidentiality"`, `"folio:tangleCrate"`, `"folio:tangleRoots"` — each
mapped to its values in statement order, as their lexical forms. Every
value is a JSON string: a number or a boolean is the text the header wrote,
and an `rdf:JSON` literal is its JSON text, unparsed, so a consumer that
wants the structure parses that one string. Nothing is filtered or typed:
the fields above that a literal also fills (`status`, `summary`,
`concerns`, `body_format`) appear here too, as written, and a tool term
(`folio:`) is listed beside the vocabulary's. It is what a script reads a
header term from instead of matching Turtle lines, so it is filled from
the Turtle itself and not from the typed reading: a header the vocabulary
refuses still lists its literals, and only one whose Turtle does not parse
lists none. A consumer that had to
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
    /// The header's `x0k:summary` — the line a reader is offered under the
    /// title. Empty when the document declares none.
    pub summary: String,
    /// The genus the header's class names, kebab-case (`design`).
    pub doc_type: String,
    pub status: String,
    /// Body format dispatch flag: `"markdown"` (default) or `"html"`. The
    /// authoring UI gates in-place editing on this (HTML bodies are
    /// read-only today).
    pub body_format: String,
    pub concerns: Vec<String>,
    /// The header's edges, keyed by compact predicate (`x0k:cites`), each
    /// target a compact id.
    pub edges: BTreeMap<String, Vec<String>>,
    /// The header's literal statements, keyed by the predicate as the header
    /// spells it (`x0k:confidence`, `folio:tangleCrate`), each value its
    /// lexical form — an `rdf:JSON` literal as its JSON text.
    pub properties: BTreeMap<String, Vec<String>>,
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
order. Headers are read against the vocabulary this build ships, built
once per walk, so a class a shipped module declares is a genus here too.

<a name="chunk-build-index"></a><sub>[`src/index.rs`](../../crates/x0k-tangle/src/index.rs) · `#build-index`</sub>

```rust {#build-index}
pub fn build_index(paths: &[PathBuf], workspace_root: &Path) -> Result<DocIndex> {
    let mut docs = Vec::new();
    let model = OntologyModel::shipped();

    for path in paths {
        if path.is_file() && path.extension().is_some_and(|e| e == "md") {
            if let Some(entry) = index_file(path, workspace_root, &model)? {
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
                    if let Some(doc_entry) = index_file(p, workspace_root, &model)? {
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

A file is a document if its header names it: a `turtle folio:document`
block whose subject is the document's id. A file with no header, or with an
untyped `<>` header that carries tool configuration and no identity, is not
listed; a document the parser rejects is skipped rather than failing the
whole index. The mtime is best-effort. The header fields come from the
shared header parser (below), and the chunk summaries carry the
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
fn index_file(path: &Path, workspace_root: &Path, model: &OntologyModel) -> Result<Option<DocEntry>> {
    let content = std::fs::read_to_string(path)?;

    let Some(header) = header_fields(&content, model) else {
        return Ok(None);
    };

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
        id: header.id,
        path: rel_path,
        title,
        summary: header.summary,
        doc_type: header.doc_type,
        status: header.status,
        body_format: header.body_format,
        concerns: header.concerns,
        edges: header.edges,
        properties: header_properties(&content, model),
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

## Reading the header

The index reads the header with `x0k_folio::colophon::parse_envelope_in` —
the parser `check` reads it with — and copies the typed values out as the
strings a JSON row carries. Sharing the parser is what stops the two verbs
describing the same file differently: an edge `check` sees is an edge the
index lists, under the same compact term.

What the index does not share is the refusal. A header whose class the
vocabulary does not declare, or whose `x0k:status` is outside the six, is
refused by `check`, and `check` says so; the index still lists the document,
with every header field empty, because an index over a working corpus
describes what is there rather than judging it, and a sidebar that drops a
page while its author is mid-edit is worse than one a verb disagrees with.
A header whose Turtle does not parse at all is a different case: the
tangler's own parser refuses it (it cannot say where the document goes),
and the index skips the document with it.

<a name="chunk-header-fields"></a><sub>[`src/index.rs`](../../crates/x0k-tangle/src/index.rs) · `#header-fields`</sub>

```rust {#header-fields}
/// The header fields an index entry is built from: the typed header's own
/// values, spelled as strings.
#[derive(Default)]
struct HeaderFields {
    id: String,
    doc_type: String,
    status: String,
    summary: String,
    concerns: Vec<String>,
    edges: BTreeMap<String, Vec<String>>,
    body_format: String,
}

/// Read a document's header against `model`. `None` when the document is
/// not listed: it has no header, or an untyped `<>` header that names
/// nothing. A typed header the vocabulary refuses still lists its document,
/// with every field empty and the body format at its default.
fn header_fields(content: &str, model: &OntologyModel) -> Option<HeaderFields> {
    match parse_envelope_in(model, content) {
        Ok((env, _)) => Some(HeaderFields {
            id: env.id,
            doc_type: env.doc_type.as_str().to_string(),
            status: env.status.map(|s| s.as_str().to_string()).unwrap_or_default(),
            summary: env.summary.unwrap_or_default(),
            concerns: env.concerns,
            edges: env.edges,
            body_format: env.body_format,
        }),
        Err(FolioError::NoHeader | FolioError::Untyped) => None,
        Err(_) => Some(HeaderFields {
            body_format: x0k_folio::colophon::BODY_FORMAT_MARKDOWN.to_string(),
            ..HeaderFields::default()
        }),
    }
}
```

The literals are read off the header's Turtle with the parser every other
reader uses, under the prefixes the typed reading predeclares, and each
predicate is written back the way an author writes it, so the key a script
asks for is the term it would have matched.

<a name="chunk-header-properties"></a><sub>[`src/index.rs`](../../crates/x0k-tangle/src/index.rs) · `#header-properties`</sub>

```rust {#header-properties}
/// Every literal statement the header makes: predicate as the header spells
/// it under `model`'s predeclared prefixes, to the lexical forms in statement
/// order. Empty when the document has no header or its Turtle does not parse.
fn header_properties(content: &str, model: &OntologyModel) -> BTreeMap<String, Vec<String>> {
    let mut out: BTreeMap<String, Vec<String>> = BTreeMap::new();
    let Some(header) = find_header(content) else {
        return out;
    };
    let prefixes = predeclared_prefixes(model);
    let Ok(triples) = parse_turtle(&header.text, &prefixes, header.line + 1) else {
        return out;
    };
    for triple in &triples {
        if let oxrdf::Term::Literal(literal) = &triple.object {
            out.entry(turtle_name(triple.predicate.as_str(), &prefixes))
                .or_default()
                .push(literal.value().to_string());
        }
    }
    out
}
```

## What a document is called

A title is the *name* of an entry, and a corpus offers it in more than one
place. x0k's own documents open with a `# ` heading, so that was the first
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

A document that carries both a host `title:` and a `# ` heading has given
itself two names, and they are not the same kind of thing. The host
frontmatter's `title:` is the record: it is what the site names the page by
everywhere *outside* the page — Docusaurus uses it for the page metadata and
the sidebar and adds it as a heading only when the body has none, and MkDocs
takes the `title` meta-data key ahead of a level-one heading. The body's
`# ` is presentation: the heading a reader meets on the page itself, free to
be a section name (`# URL Readers` under `title: Url Reader Service`,
`# Backstage Search` under `title: Search Documentation`, both from a real
Backstage docs tree). An index row is a name seen from outside, so it takes
the record. Asking for the heading first ranked the presentation above it,
which an evaluator reported in two successive rounds as surprising for
anyone whose H1 is a section heading (Backstage re-evaluations, 2026-09-23).
None of x0k's own documents carries a host `title:`, so every one of them
is named by its `# `.

So the resolution walks six sources in the order a reader would, and the
document says which one it took by which one is non-empty:

1. the host frontmatter's own top-level `title:`, which is where Docusaurus
   and MkDocs keep the name — the record, when there is one
   (`colophon::host_frontmatter` finds the block; that key is the one thing
   the index reads from host frontmatter);
2. the body's first `# ` heading, outside every fence;
3. an opening `<h1>`, for the documents whose `x0k:bodyFormat` is `"html"`
   and whose heading is therefore a tag rather than a hash — two of x0k's
   own design documents, which indexed as `""` for the same reason the ADRs
   did;
4. a heading of *any* level that the body **opens** with — the first
   non-blank line — for a page written under a `##`;
5. the header's own `x0k:summary`, which is the document describing itself
   and is never about a part of it;
6. the filename stem — always available, never wrong about identity even
   when it is ugly.

The body every heading is looked for in is the document with its host
frontmatter and its header lifted out (`colophon::strip_header`). The header
sits after the `# ` line or, when the body opens any other way, at the very
top, where it would otherwise be the first non-blank line and hide the
heading a page opens with at 4. Lifting it out leaves exactly the body the
author wrote, so where the header sits never changes what a page is called.

The resolver stops at 5 and returns `None`, because the fallback at 6 is not
the same fallback for every caller: `index` has a path to take a stem from,
and `weave` has only the document's id.

A summary is a sentence where a title wants a phrase, so 5 is a demotion,
not a discovery — it is there because the alternative it replaced was a
section name asserting itself as the page's, and a document's own sentence
about itself is at least about the whole document. It is read with
`colophon::header_literals`, so a header the vocabulary refuses still lends
its summary, as it lends the index its row. A corpus that dislikes the
sentence has the fix in its own hands: give the page an `# ` heading.

<a name="chunk-document-title"></a><sub>[`src/index.rs`](../../crates/x0k-tangle/src/index.rs) · `#document-title`</sub>

```rust {#document-title}
/// A document's title, resolved the way a reader would ask for it: the host
/// frontmatter's `title:` (the record), then the body's first `# ` heading,
/// then an opening `<h1>` for an HTML body, then a heading the body opens
/// with, then the header's `x0k:summary`. The body is the document with its
/// host frontmatter and header lifted out. Fenced regions are skipped — a `#`
/// comment inside an example names nothing — and a heading below prose names
/// its section rather than the page. `None` when the document offers no name
/// at all, leaving the last fallback (a filename stem, a document id) to the
/// caller that has one.
pub fn document_title(content: &str) -> Option<String> {
    if let Some(host) = host_frontmatter_title(content) {
        return Some(host);
    }
    let body = strip_header(content);
    if let Some(h1) = first_level_one_heading(&body) {
        return Some(h1);
    }
    if let Some(opening) = opening_heading(&body) {
        return Some(opening);
    }
    header_literals(content, "x0k:summary")
        .ok()?
        .into_iter()
        .map(|literal| literal.value)
        .find(|summary| !summary.is_empty())
}
```

A level-one heading is asked for in two spellings, and both questions below
ask it the same way, so it is asked in one place.

<a name="chunk-first-level-one-heading"></a><sub>[`src/index.rs`](../../crates/x0k-tangle/src/index.rs) · `#first-level-one-heading`</sub>

```rust {#first-level-one-heading}
/// The body's level-one heading: its first `# ` outside every fence, or, for
/// a body that carries its heading as a tag, the opening `<h1>`.
fn first_level_one_heading(body: &str) -> Option<String> {
    first_heading(body, |level| level == 1).or_else(|| first_html_h1(body))
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
/// The text of the first `<h1>` in an HTML body. Bodies whose
/// `x0k:bodyFormat` is `"html"` carry their heading as a tag, so the hash
/// scanner finds nothing in them at all.
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

The host's `title:` is the one at column zero. Anything indented belongs to
a nested block of the host's own, and is not the page's name.

<a name="chunk-host-frontmatter-title"></a><sub>[`src/index.rs`](../../crates/x0k-tangle/src/index.rs) · `#host-frontmatter-title`</sub>

```rust {#host-frontmatter-title}
/// The host frontmatter's own `title:` — the top-level key, at column zero,
/// which is where Docusaurus and MkDocs keep a page's name.
fn host_frontmatter_title(content: &str) -> Option<String> {
    let range = host_frontmatter(content)?;
    content[range]
        .lines()
        .filter(|line| !line.starts_with([' ', '\t']))
        .find_map(|line| line.strip_prefix("title:"))
        .map(unquote_scalar)
        .filter(|title| !title.is_empty())
}
```

<a name="chunk-unquote-scalar"></a><sub>[`src/index.rs`](../../crates/x0k-tangle/src/index.rs) · `#unquote-scalar`</sub>

```rust {#unquote-scalar}
/// A host frontmatter scalar as the title reader sees it: trimmed, and
/// stripped of one matched pair of surrounding quotes. Enough for the values
/// a site generator's `title:` is written as.
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

## When the two names disagree

Precedence settles which name `index` and `weave` use. It does not settle
whether the author meant to give two. A heading that differs from the record
only in how it is typeset is the same name — `title: 'ADR013: Proper use of
HTTP fetching libraries'` over `# ADR013: Proper use of *HTTP* fetching
libraries.` is one title written twice. A heading that differs in its words is
either a section name standing where a page name usually stands, or a rename
that reached one of the two places and not the other, and only the author
knows which. So `check` says so, once per document, as a **warning**: it
names both strings, and it never changes the run's exit code, with or
without `--closed`, because neither reading is a defect. The H1 is
presentation, and presentation is the author's to choose; the warning is
there so that the choice is made on purpose.

The comparison, stated so an author can predict it — each string is:

1. stripped of Markdown emphasis and code delimiters, `*`, `_` and `` ` ``,
   wherever they occur;
2. trimmed of surrounding whitespace, with every inner run of whitespace
   read as one space;
3. trimmed of trailing punctuation — `.`, `,`, `:`, `;`, `!`, `?`;
4. lower-cased;

and the document warns when the two results differ. Only a level-one heading
takes part. A `##` is never an H1, and a host `title:` over a body that opens
at `## Context` is the Docusaurus shape itself rather than a disagreement. A
document with only one of the two names has nothing to disagree with.

<a name="chunk-title-disagreement"></a><sub>[`src/index.rs`](../../crates/x0k-tangle/src/index.rs) · `#title-disagreement`</sub>

```rust {#title-disagreement}
/// The two names a document gives itself, when they are two different names:
/// the host frontmatter's `title:` and the body's level-one heading.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TitleDisagreement {
    /// The host frontmatter's `title:` as written — the name `index` takes.
    pub frontmatter: String,
    /// The body's first `# ` (or an HTML body's `<h1>`) as written.
    pub heading: String,
}

/// `Some` when a document carries both a host `title:` and a level-one
/// heading and they still differ once both are reduced by
/// `comparable_title`; `None` when they agree or either one is absent.
pub fn title_disagreement(content: &str) -> Option<TitleDisagreement> {
    let frontmatter = host_frontmatter_title(content)?;
    let heading = first_level_one_heading(&strip_header(content))?;
    (comparable_title(&frontmatter) != comparable_title(&heading))
        .then_some(TitleDisagreement { frontmatter, heading })
}

/// A title reduced to what the disagreement warning compares: emphasis and
/// code delimiters dropped, whitespace collapsed, trailing punctuation
/// trimmed, lower-cased.
fn comparable_title(title: &str) -> String {
    let unmarked: String = title
        .chars()
        .filter(|c| !matches!(c, '*' | '_' | '`'))
        .collect();
    let collapsed = unmarked.split_whitespace().collect::<Vec<_>>().join(" ");
    collapsed
        .trim_end_matches(|c: char| {
            matches!(c, '.' | ',' | ':' | ';' | '!' | '?') || c.is_whitespace()
        })
        .to_lowercase()
}
```

## Tests

The header tests read a typed header into its row and pin the two ways a
file is left out or kept in: an untyped `<>` header is not listed, and a
header the vocabulary refuses is listed with its fields empty.

The title tests are the sources in order, each written as the corpus that
actually produced it: an x0k document with an `# ` heading, a Docusaurus ADR
whose name is in the host frontmatter and whose body opens at `## Context`,
an HTML body whose heading is a tag, a page that opens with `##` and quotes a
`#` comment inside a fence, an MkDocs page whose `##` arrives after prose and
which must therefore *not* be named by it, and a generated table with no
heading at all, named by its summary. The fenced-comment case once presented
a Python comment as the name of a page, and the prose-then-`##` case once
presented a section as one. One more test asks every source at once and
takes them away one at a time, which pins the order itself, and a header
placed at the top of a body that opens at `##` is shown not to hide that
heading. The precedence test is an evaluator's own file, a host `title:`
over a different `# `, and the three disagreement tests are the rule's three
cases: two spellings of one name are silent, two names are reported by both
strings, and one name has nothing to disagree with.

The properties test reads a header's literals into the index row: tool
terms beside vocabulary terms, numbers and `rdf:JSON` as the text written,
and a refused header still listing what it says.

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

    fn fields(content: &str) -> Option<HeaderFields> {
        header_fields(content, &OntologyModel::shipped())
    }

    #[test]
    fn a_typed_header_fills_the_row() {
        let content = "# Test Document\n\n```turtle folio:document\ndesign:test a x0k:Design ;\n    \
            x0k:status \"proposed\" ;\n    x0k:concerns \"ui\", \"lod\" ;\n    \
            x0k:cites wiki:foo, wiki:bar ;\n    x0k:refinedBy design:block .\n```\n\nBody here.\n";
        let fields = fields(content).expect("a typed header is listed");
        assert_eq!(fields.id, "x0k:design/test");
        assert_eq!(fields.doc_type, "design");
        assert_eq!(fields.status, "proposed");
        assert_eq!(fields.summary, "");
        assert_eq!(fields.concerns, vec!["ui", "lod"]);
        // Keyed by the compact term, the way the header spells it.
        assert_eq!(fields.edges["x0k:cites"], vec!["x0k:wiki/foo", "x0k:wiki/bar"]);
        assert_eq!(fields.edges["x0k:refinedBy"], vec!["x0k:design/block"]);
        assert_eq!(fields.body_format, "markdown");
    }

    #[test]
    fn an_untyped_header_or_none_is_not_listed() {
        assert!(fields("# Plain\n\nNo header here.\n").is_none());
        let untyped = "# Page\n\n```turtle folio:document\n<> folio:tangleCrate \"x\" ;\n    \
            folio:tangleRoot \"src/lib.rs\" .\n```\n";
        assert!(fields(untyped).is_none());
    }

    /// `index` refuses nothing a working corpus is in the middle of: a
    /// status outside the six is refused by `check` and still listed here,
    /// with nothing typed to say about it.
    #[test]
    fn a_header_the_vocabulary_refuses_is_listed_empty() {
        let content = "# ADR013\n\n```turtle folio:document\narchitecture:adr013 a x0k:Architecture ;\n    \
            x0k:status \"ratified\" .\n```\n";
        let fields = fields(content).expect("a refused header still lists its document");
        assert_eq!(fields.id, "");
        assert_eq!(fields.status, "");
        assert_eq!(fields.body_format, "markdown");
    }

    #[test]
    fn title_takes_the_body_h1_when_the_host_names_nothing() {
        let content = "# My Title\n\n```turtle folio:document\ndesign:t a x0k:Design .\n```\n\nBody.";
        assert_eq!(document_title(content).as_deref(), Some("My Title"));
    }

    #[test]
    fn the_host_title_outranks_a_body_h1() {
        // The record over the presentation: a Backstage evaluator's own file.
        let content = "---\nid: adrs-adrZ\ntitle: 'ADRZ: Frontmatter wins?'\n---\n\n# Body H1 Different\n\nText.\n";
        assert_eq!(
            document_title(content).as_deref(),
            Some("ADRZ: Frontmatter wins?")
        );
    }

    #[test]
    fn only_the_host_title_at_column_zero_is_read() {
        let content = "---\nnav:\n  title: Nested\n---\n\nProse with no heading.\n";
        assert_eq!(document_title(content), None);
    }

    #[test]
    fn a_host_title_and_an_h1_that_differ_only_in_typesetting_agree() {
        let content = "---\ntitle: 'ADR013: Proper use of HTTP fetching libraries'\n---\n\n#   adr013: Proper use of *HTTP*   fetching `libraries`.  \n\nText.\n";
        assert_eq!(title_disagreement(content), None);
    }

    #[test]
    fn a_host_title_and_an_h1_that_differ_in_words_disagree_by_name() {
        let content = "---\ntitle: Url Reader Service\n---\n\n# URL Readers\n\nText.\n";
        assert_eq!(
            title_disagreement(content),
            Some(TitleDisagreement {
                frontmatter: "Url Reader Service".to_string(),
                heading: "URL Readers".to_string(),
            })
        );
    }

    #[test]
    fn a_document_with_one_name_has_nothing_to_disagree_with() {
        // Host title over a body that opens at `##`: the Docusaurus shape.
        let host_only = "---\ntitle: Some Page\n---\n\n## Context\n";
        assert_eq!(title_disagreement(host_only), None);
        // An H1 and no host title: every x0k document.
        let h1_only = "# Some Other Page\n\n```turtle folio:document\ndesign:o a x0k:Design .\n```\n";
        assert_eq!(title_disagreement(h1_only), None);
    }

    #[test]
    fn title_falls_back_to_host_frontmatter_when_the_body_opens_at_h2() {
        // The Docusaurus/MkDocs shape: the name is a host frontmatter key and
        // the body starts at `## Context`.
        let content = "---\nid: adrs-adr013\ntitle: 'ADR013: [superseded] Proper use of HTTP fetching libraries'\n---\n\n## Context\n\nUsing multiple HTTP packages…\n";
        assert_eq!(
            document_title(content).as_deref(),
            Some("ADR013: [superseded] Proper use of HTTP fetching libraries")
        );
    }

    #[test]
    fn title_reads_an_html_body_s_opening_h1() {
        // `x0k:bodyFormat "html"` documents carry their heading as a tag; the
        // hash scanner finds nothing in them. Two of x0k's own design
        // decisions.
        let content = "```turtle folio:document\ndesign:files-surface a x0k:Design ;\n    x0k:bodyFormat \"html\" .\n```\n\n<h1>Files surface</h1>\n\n<p>A library you already know how to use.</p>\n";
        assert_eq!(document_title(content).as_deref(), Some("Files surface"));
    }

    #[test]
    fn title_takes_a_heading_the_body_opens_with_and_skips_fenced_regions() {
        // A `#` comment inside an example is a quotation, not a name. This
        // page indexed as "return None if the discriminator value isn't found"
        // — a Python comment 342 lines in (pydantic evaluation, 2026-09-22).
        // The header sits at the top, ahead of the `##`, and does not hide it.
        let content = "```turtle folio:document\nwiki:union-modes a x0k:Wiki .\n```\n## Union Modes\n\nUnions are fundamentally different.\n\n```python\n# return None if the discriminator value isn't found\nreturn None\n```\n\n### Left to Right Mode\n";
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
        let content = "```turtle folio:document\nwiki:union-modes a x0k:Wiki .\n```\nUnions are fundamentally different.\n\n## Union Modes\n\n### Left to Right Mode\n";
        assert_eq!(document_title(content), None);
    }

    #[test]
    fn a_document_that_names_itself_nowhere_falls_back_to_its_summary() {
        // A generated reference table: no headings, no host `title:`. What it
        // does have is a sentence about itself, which is at least about the
        // whole of it.
        let content = "```turtle folio:document\nwiki:conversion-table a x0k:Wiki ;\n    x0k:summary \"What Pydantic converts to what.\" .\n```\n| Field | Type |\n| --- | --- |\n| a | int |\n";
        assert_eq!(
            document_title(content).as_deref(),
            Some("What Pydantic converts to what.")
        );
        // A header the vocabulary refuses still lends its summary.
        let refused = content.replace(" ;\n    x0k:summary", " ;\n    x0k:status \"ratified\" ;\n    x0k:summary");
        assert_eq!(
            document_title(&refused).as_deref(),
            Some("What Pydantic converts to what.")
        );
    }

    #[test]
    fn a_document_with_no_heading_and_no_summary_has_no_title_of_its_own() {
        // Nothing in the file names it. The resolver declines and `index`
        // lists it under its filename stem.
        let content = "```turtle folio:document\nwiki:conversion-table a x0k:Wiki .\n```\n| Field | Type |\n| --- | --- |\n| a | int |\n";
        assert_eq!(document_title(content), None);
    }

    /// The order itself: every source present, then each taken away in turn,
    /// so the next one down answers.
    #[test]
    fn the_title_sources_answer_in_order() {
        let header = "```turtle folio:document\nwiki:order a x0k:Wiki ;\n    x0k:summary \"The summary.\" .\n```\n";
        let host = "---\ntitle: The record\n---\n";
        let opening = "## The opening heading\n\n";
        let hash = "# The hash heading\n\n";
        let tag = "<h1>The tag heading</h1>\n\n";
        let prose = "Prose.\n";

        let all = format!("{host}{header}{opening}{hash}{tag}{prose}");
        assert_eq!(document_title(&all).as_deref(), Some("The record"));
        let no_host = format!("{header}{opening}{hash}{tag}{prose}");
        assert_eq!(document_title(&no_host).as_deref(), Some("The hash heading"));
        let no_hash = format!("{header}{opening}{tag}{prose}");
        assert_eq!(document_title(&no_hash).as_deref(), Some("The tag heading"));
        let no_tag = format!("{header}{opening}{prose}");
        assert_eq!(document_title(&no_tag).as_deref(), Some("The opening heading"));
        let no_opening = format!("{header}{prose}");
        assert_eq!(document_title(&no_opening).as_deref(), Some("The summary."));
        let nothing = format!("```turtle folio:document\nwiki:order a x0k:Wiki .\n```\n{prose}");
        assert_eq!(document_title(&nothing), None);
    }

    #[test]
    fn the_index_carries_every_literal_the_header_states() {
        let model = OntologyModel::shipped();
        let content = "# Germ\n\n```turtle folio:document\nwiki:germ a x0k:Wiki ;\n    x0k:status \"draft\" ;\n    x0k:summary \"A germ.\" ;\n    x0k:concerns \"a\", \"b\" ;\n    x0k:confidence \"sketch\" ;\n    x0k:cites wiki:other ;\n    folio:tangleCrate \"substrate/cells/germ\" ;\n    folio:tangleRoots '{\"rust\":\"src/lib.rs\"}'^^rdf:JSON .\n```\n\nBody.\n";
        let properties = header_properties(content, &model);
        let strings = |values: &[&str]| values.iter().map(|v| v.to_string()).collect::<Vec<_>>();
        assert_eq!(properties["x0k:status"], strings(&["draft"]));
        assert_eq!(properties["x0k:summary"], strings(&["A germ."]));
        assert_eq!(properties["x0k:concerns"], strings(&["a", "b"]), "statement order");
        assert_eq!(properties["x0k:confidence"], strings(&["sketch"]));
        assert_eq!(properties["folio:tangleCrate"], strings(&["substrate/cells/germ"]));
        assert_eq!(properties["folio:tangleRoots"], strings(&["{\"rust\":\"src/lib.rs\"}"]), "JSON as its text");
        assert!(!properties.contains_key("x0k:cites"), "an IRI object is an edge, not a property");

        let refused = content.replace("\"draft\"", "\"ratified\"");
        assert_eq!(header_properties(&refused, &model)["x0k:status"], strings(&["ratified"]));
        assert!(header_properties("# Plain\n\nNo header.\n", &model).is_empty());
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
            "```turtle folio:document\nwiki:conversion-table a x0k:Wiki ;\n    x0k:status \"stable\" .\n```\n| Field | Type |\n",
        )
        .unwrap();

        let index = build_index(&[doc_path], &dir).unwrap();
        let entry = &index.docs[0];
        assert_eq!(entry.title, "conversion_table");
        assert_eq!(entry.summary, "");
        assert_eq!(entry.properties["x0k:status"], vec!["stable".to_string()]);

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
            "# Test Doc\n\n```turtle folio:document\nimplementation:test\\/doc a x0k:Implementation ;\n    x0k:status \"draft\" .\n```\n\n```rust {{#verdict from=\"{src_rel}\" symbol=\"classify_range\"}}\n```\n\n```rust {{#shelf from=\"{src_rel}\" symbol=\"Shelf\"}}\n```\n"
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
            "# Test Doc\n\n```turtle folio:document\nimplementation:test\\/js a x0k:Implementation ;\n    x0k:status \"draft\" .\n```\n\n```js {{#widget from=\"{src_rel}\" symbol=\"Widget\"}}\n```\n"
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

        let doc = "# Test Doc\n\n```turtle folio:document\nimplementation:test\\/toml a x0k:Implementation ;\n    x0k:status \"draft\" .\n```\n\n```toml {#pkg from=\"Cargo.toml\" symbol=\"package\"}\n```\n";
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

<a name="chunk-root"></a><sub>[`src/index.rs`](../../crates/x0k-tangle/src/index.rs) · `#root` · assembles [module-doc](#chunk-module-doc) · [doc-index](#chunk-doc-index) · [chunk-summary](#chunk-chunk-summary) · [build-index](#chunk-build-index) · [index-file](#chunk-index-file) · [build-span-map](#chunk-build-span-map) · [header-fields](#chunk-header-fields) · [header-properties](#chunk-header-properties) · [document-title](#chunk-document-title) · [first-level-one-heading](#chunk-first-level-one-heading) · [opening-heading](#chunk-opening-heading) · [first-html-h1](#chunk-first-html-h1) · [first-heading](#chunk-first-heading) · [title-disagreement](#chunk-title-disagreement) · [host-frontmatter-title](#chunk-host-frontmatter-title) · [unquote-scalar](#chunk-unquote-scalar) · [tests](#chunk-tests)</sub>

```rust {#root}
<<module-doc>>

<<doc-index>>

<<chunk-summary>>

<<build-index>>

<<index-file>>

<<build-span-map>>

<<header-fields>>

<<header-properties>>

<<document-title>>

<<first-level-one-heading>>

<<opening-heading>>

<<first-html-h1>>

<<first-heading>>

<<title-disagreement>>

<<host-frontmatter-title>>

<<unquote-scalar>>

<<tests>>
```

The index is a projection of the corpus and nothing more — it holds no
state a re-walk cannot rebuild — which is why listing a refused header is
acceptable here where it would not be in the header check: an index that
says nothing about one document's `status` costs a sidebar row, and the
next walk corrects it. Tolerant is about meaning, though, and never about
syntax — sharing `check`'s parser is what stops the two verbs describing
the same file differently.
