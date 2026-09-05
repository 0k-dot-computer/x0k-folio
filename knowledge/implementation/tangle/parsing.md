---
x0k:
  format: folio/v1
  id: x0k:implementation/tangle/parsing
  type: implementation
  status: draft
  summary: Markdown to a `ParsedDocument` — a frontmatter splitter that needs no YAML parser, the chunk info-string grammar, and the append-and-variant rules for a chunk name that appears more than once.
  concerns: [tangle, literate, parsing, markdown, info-string]
  tangle:
    crate: x0k-tangle
    root: src/parser.rs
  edges:
    cites:
      - x0k:implementation/tangle/protocol
      - x0k:implementation/tangle/chunk
---
# Parsing a literate document

The parser turns a markdown file — a [literate
document](../../wiki/literate-programming.md "x0k:wiki/literate-programming") — into a `ParsedDocument`: tangle
metadata from the frontmatter, pipeline declarations from the
folio/v1 envelope, and a map of named code chunks keyed by
`{#name}` from each fence's info-string.

Pulldown-cmark already does the markdown parsing. What this module
adds:

- A frontmatter splitter that doesn't drag in a full YAML parser
  (we only need `tangle.crate` and `tangle.root`; the rest is
  handled by `x0k-folio::colophon::parse_envelope`).
- An info-string parser that recognizes the x0k chunk attributes
  (`{#name}`, `file=`, `symbol=`, `from=`, `proves=`, `x0k:media`)
  and the language token in front of the brace.
- A code-block iterator that buckets chunk bodies by name, with
  multi-body append semantics for repeated `#name` fences and
  multi-language variants when the same `#name` appears with
  different language tokens.

## Imports

<a name="chunk-imports"></a><sub>[`src/parser.rs`](../../../x0k-tangle/src/parser.rs) · `#imports`</sub>

```rust {#imports}
use crate::chunk::{Chunk, ChunkBody};
use anyhow::Result;
use x0k_folio::colophon::{parse_envelope, PipelineDecl};
use pulldown_cmark::{CodeBlockKind, Event, Options, Parser, Tag, TagEnd};
use std::collections::HashMap;
use std::path::PathBuf;
```

## The ParsedDocument shape

The parser returns a single `ParsedDocument` for a whole markdown
file. Shape rationale:

- `id` — the document's folio/v1 frontmatter `id:` URI (e.g.
  `x0k:implementation/tangle/parsing`), recovered from the shared
  envelope parser. `None` when the doc isn't folio/v1. The corpus
  resolver (see [`multi-doc-resolve.md`](multi-doc-resolve.md)) keys
  documents by this URI so a `<<uri::chunk>>` ref can find its target.
- `tangle_crate` / `tangle_root` — extracted from the legacy
  `tangle:` block in the frontmatter. Both are `Option` because
  pipeline-only docs don't have a `tangle:` block.
- `tangle_roots` — the per-language root map from `tangle.roots:`,
  in declaration order. A bilingual document — one prose body
  projecting a Rust body and a Gallowglass body — declares one root
  per fence language instead of a single `root:`:

  ```yaml
  tangle:
    roots:
      rust: x0k-tangle/tests/gls_fold_demo.rs
      gallowglass: gls/demo/fold.gls
  ```

  Empty for single-target docs; `root:` and `roots:` may coexist,
  with `root:` serving variants whose language has no entry.
- `chunks` — keyed by chunk name, but each value is a `Vec<Chunk>`,
  not a single `Chunk`. This holds language variants: a `#name`
  fence in Rust and a `#name` fence in TypeScript share the
  same key but are distinct entries in the vec.
- `chunk_order` — declaration order of names, used by the
  identity pipeline to walk top-level chunks deterministically.
- `pipelines` — declared pipelines from the folio/v1 envelope.
  Empty when the doc isn't folio/v1 or has no `pipelines:`.

<a name="chunk-parsed-document-type"></a><sub>[`src/parser.rs`](../../../x0k-tangle/src/parser.rs) · `#parsed-document-type`</sub>

```rust {#parsed-document-type}
#[derive(Debug, Clone)]
pub struct ParsedDocument {
    /// The document's folio/v1 frontmatter `id:` URI, when present.
    /// Used by the corpus resolver to key documents for cross-doc refs.
    pub id: Option<String>,
    pub tangle_crate: Option<String>,
    pub tangle_root: Option<PathBuf>,
    /// Per-language tangle roots from `tangle.roots:` (fence language →
    /// output path), in declaration order. Empty when the doc declares
    /// only the single `root:`.
    pub tangle_roots: Vec<(String, PathBuf)>,
    /// Chunks grouped by name. Multiple entries per name when the same
    /// chunk has implementations in different languages.
    pub chunks: HashMap<String, Vec<Chunk>>,
    pub chunk_order: Vec<String>,
    /// Pipeline declarations from the document's folio/v1
    /// frontmatter `pipelines:` block. Empty when the document is not
    /// a folio/v1 file or has no pipelines.
    pub pipelines: Vec<PipelineDecl>,
}
```

## Lookup affordances

The three lookup methods cover the common access patterns:

- `chunk(name)` — first variant; fine for single-language docs.
- `chunk_for_lang(name, lang)` — exact lang match, falling back to
  the first variant so callers don't have to special-case
  missing-variant.
- `chunk_variants(name)` — full slice for callers that walk every
  variant (the weaver does this when rendering tabs).

<a name="chunk-parsed-document-impl"></a><sub>[`src/parser.rs`](../../../x0k-tangle/src/parser.rs) · `#parsed-document-impl`</sub>

```rust {#parsed-document-impl}
impl ParsedDocument {
    /// Get the first (or only) chunk with this name. For single-language
    /// docs this is the common path.
    pub fn chunk(&self, name: &str) -> Option<&Chunk> {
        self.chunks.get(name).and_then(|v| v.first())
    }

    /// Get the chunk variant matching a target language. Falls back to
    /// the first variant if no lang-specific match exists.
    pub fn chunk_for_lang(&self, name: &str, lang: &str) -> Option<&Chunk> {
        let variants = self.chunks.get(name)?;
        variants
            .iter()
            .find(|c| c.lang.as_deref() == Some(lang))
            .or_else(|| variants.first())
    }

    /// Get all language variants for a chunk name.
    pub fn chunk_variants(&self, name: &str) -> Option<&[Chunk]> {
        self.chunks.get(name).map(|v| v.as_slice())
    }
}
```

## Info-string attributes

The fence info-string is the substrate's keyhole into chunk
metadata. The shape:

```
<lang>? x0k:media? {#name file="…" symbol="…" from="…" proves="…"}
```

`proves=` is the one attribute that says nothing about *where* the
chunk goes. It names the affordance — or, comma-separated, the
affordances — that the tests in this chunk are evidence for:
`proves="x0k:affordance/read_a_line"`. The edge is authored on the
block that declares the test because that is the only place it cannot
go stale silently: rename the test and the chunk still says what it
proves; delete the chunk and the edge is gone with it, which a checker
can see. The projector reads it back to derive an affordance's status,
and the tangler itself never looks at it.

`ChunkAttrs` is the intermediate parsed shape; the parser drains
this into the `Chunk` fields once a fence's body is read.

<a name="chunk-chunk-attrs-type"></a><sub>[`src/parser.rs`](../../../x0k-tangle/src/parser.rs) · `#chunk-attrs-type`</sub>

```rust {#chunk-attrs-type}
#[derive(Debug, Clone, Default)]
pub struct ChunkAttrs {
    pub name: Option<String>,
    pub file: Option<PathBuf>,
    pub symbol: Option<String>,
    pub from: Option<PathBuf>,
    /// Affordance ids from `proves=`, in the order written.
    pub proves: Vec<String>,
    pub is_media: bool,
    pub lang: Option<String>,
}
```

## parse_info_string

Top-level info-string parser. Handles the optional `x0k:media`
flag (it can appear anywhere in the front-matter token list; we
strip it then re-tokenise), the language token in front of the
brace, and the brace-attribute block.

<a name="chunk-parse-info-string-fn"></a><sub>[`src/parser.rs`](../../../x0k-tangle/src/parser.rs) · `#parse-info-string-fn`</sub>

```rust {#parse-info-string-fn}
pub fn parse_info_string(info: &str) -> ChunkAttrs {
    let mut attrs = ChunkAttrs::default();
    let mut remaining = info.trim();

    let owned;
    if remaining.contains("x0k:media") {
        attrs.is_media = true;
        owned = remaining.replace("x0k:media", "");
        remaining = owned.trim();
    }

    if let Some(brace_start) = remaining.find('{') {
        let lang_part = remaining[..brace_start].trim();
        if !lang_part.is_empty() {
            attrs.lang = Some(lang_part.to_string());
        }
        if let Some(brace_end) = remaining.find('}') {
            let attr_str = &remaining[brace_start + 1..brace_end];
            parse_brace_attrs(attr_str, &mut attrs);
        }
    } else {
        let lang_part = remaining.trim();
        if !lang_part.is_empty() {
            attrs.lang = Some(lang_part.to_string());
        }
    }

    attrs
}
```

The `owned` binding outlives the `remaining` reference because Rust
borrow rules need the replacement-string's lifetime to span the
later use of `remaining`. A `String` would work too; we use a
manually-managed binding to avoid an alloc on the common
non-media path.

## Brace attributes

Inside `{...}` we have a tiny attribute language: `#name` for the
chunk identifier, `file=`, `symbol=`, `from=` for the typed
attributes, `proves=` for a comma-separated list of affordance ids,
optional commas as separators. Anything unrecognized is skipped (we
err toward "tolerate unknown tokens" so future attribute additions
don't break old code paths — an older tangler reading a `proves=`
chunk tangles it exactly as before, which is what let the attribute
arrive in the corpus ahead of the code that reads it). The list form
needs the quotes: a bare value stops at the first comma, because a
comma is also the separator between attributes.

<a name="chunk-parse-brace-attrs-fn"></a><sub>[`src/parser.rs`](../../../x0k-tangle/src/parser.rs) · `#parse-brace-attrs-fn`</sub>

```rust {#parse-brace-attrs-fn}
fn parse_brace_attrs(attr_str: &str, attrs: &mut ChunkAttrs) {
    let mut rest = attr_str.trim();
    while !rest.is_empty() {
        rest = rest.trim_start();
        if rest.starts_with('#') {
            // #chunk-name
            let name_str = &rest[1..];
            let end = name_str
                .find(|c: char| c.is_whitespace() || c == ',' || c == '}')
                .unwrap_or(name_str.len());
            attrs.name = Some(name_str[..end].to_string());
            rest = &name_str[end..];
        } else if let Some(after) = rest.strip_prefix("file=") {
            let (val, r) = extract_quoted_or_bare(after);
            attrs.file = Some(PathBuf::from(val));
            rest = r;
        } else if let Some(after) = rest.strip_prefix("symbol=") {
            let (val, r) = extract_quoted_or_bare(after);
            attrs.symbol = Some(val.to_string());
            rest = r;
        } else if let Some(after) = rest.strip_prefix("from=") {
            let (val, r) = extract_quoted_or_bare(after);
            attrs.from = Some(PathBuf::from(val));
            rest = r;
        } else if let Some(after) = rest.strip_prefix("proves=") {
            let (val, r) = extract_quoted_or_bare(after);
            attrs.proves.extend(
                val.split(',')
                    .map(str::trim)
                    .filter(|id| !id.is_empty())
                    .map(str::to_string),
            );
            rest = r;
        } else if rest.starts_with(',') {
            rest = &rest[1..];
        } else {
            // Skip unknown token
            let end = rest
                .find(|c: char| c.is_whitespace() || c == ',')
                .unwrap_or(rest.len());
            rest = &rest[end..];
        }
    }
}
```

## Quoted-or-bare extractor

A small utility: `attr="value with spaces"` or `attr=bare_value`.
Returns the value and the remaining input after the value.

<a name="chunk-extract-quoted-or-bare-fn"></a><sub>[`src/parser.rs`](../../../x0k-tangle/src/parser.rs) · `#extract-quoted-or-bare-fn`</sub>

```rust {#extract-quoted-or-bare-fn}
fn extract_quoted_or_bare(s: &str) -> (&str, &str) {
    let s = s.trim_start();
    if let Some(inner) = s.strip_prefix('"') {
        if let Some(end_quote) = inner.find('"') {
            (&inner[..end_quote], &inner[end_quote + 1..])
        } else {
            (inner, "")
        }
    } else {
        let end = s
            .find(|c: char| c.is_whitespace() || c == ',' || c == '}')
            .unwrap_or(s.len());
        (&s[..end], &s[end..])
    }
}
```

## parse_document

The public entry point. Splits frontmatter, recovers `tangle:`
fields, reuses `x0k-folio::parse_envelope` for the typed
`pipelines:` list, then iterates pulldown-cmark events to bucket
code blocks into the chunk map.

Two non-obvious mechanics worth flagging:

- **Line numbers.** Each `ChunkBody` records its source line in
  the markdown for diagnostics. We compute it from
  pulldown-cmark's byte offsets by precomputing newline positions
  and `take_while`-counting. Adding the `body_start_line` offset
  accounts for the frontmatter header lines that pulldown-cmark
  doesn't see (we feed it just the body).
- **Append vs. variant.** When a `#name` fence reappears with the
  same language token, its body is appended to the existing
  variant's `bodies` vec. When it appears with a *different*
  language token, a new variant is added. This is what makes
  multi-language chunks composable.

<a name="chunk-parse-document-fn"></a><sub>[`src/parser.rs`](../../../x0k-tangle/src/parser.rs) · `#parse-document-fn`</sub>

```rust {#parse-document-fn}
pub fn parse_document(content: &str) -> Result<ParsedDocument> {
    let (frontmatter, body) = split_frontmatter(content);
    let (tangle_crate, tangle_root, tangle_roots) = parse_tangle_frontmatter(frontmatter);

    // The folio/v1 envelope carries both the document `id:` URI
    // and the `pipelines:` declarations. Reuse the shared envelope
    // parser when the document is a folio/v1 file; tolerate
    // non-folio docs by leaving `id` `None` and pipelines empty.
    let (id, pipelines) = match parse_envelope(content) {
        Ok((env, _)) => (Some(env.id), env.pipelines),
        Err(_) => (None, Vec::new()),
    };

    let mut chunks: HashMap<String, Vec<Chunk>> = HashMap::new();
    let mut chunk_order: Vec<String> = Vec::new();

    let body_start_line = if frontmatter.is_some() {
        content[..content.len() - body.len()]
            .lines()
            .count()
    } else {
        0
    };

    let opts = Options::empty();
    let parser = Parser::new_ext(body, opts);

    let mut in_code_block = false;
    let mut current_info = String::new();
    let mut current_code = String::new();
    let mut code_block_start_line: usize = 0;
    let mut current_line: usize;

    // Track line numbers by scanning the body
    let line_offsets: Vec<usize> = body
        .match_indices('\n')
        .map(|(i, _)| i)
        .collect();

    for (event, range) in parser.into_offset_iter() {
        // Compute line number from byte offset
        let byte_offset = range.start;
        current_line = body_start_line
            + line_offsets
                .iter()
                .take_while(|&&off| off < byte_offset)
                .count();

        match event {
            Event::Start(Tag::CodeBlock(kind)) => {
                in_code_block = true;
                current_code.clear();
                code_block_start_line = current_line;
                current_info = match kind {
                    CodeBlockKind::Fenced(info) => info.to_string(),
                    CodeBlockKind::Indented => String::new(),
                };
            }
            Event::Text(text) if in_code_block => {
                current_code.push_str(&text);
            }
            Event::End(TagEnd::CodeBlock) => {
                in_code_block = false;
                let attrs = parse_info_string(&current_info);

                if let Some(name) = attrs.name {
                    let body_text = current_code.trim_end().to_string();
                    let body = ChunkBody {
                        text: body_text,
                        source_line: code_block_start_line,
                    };

                    let variants = chunks.entry(name.clone()).or_default();

                    // Find existing variant with same lang (append semantics)
                    let same_lang = variants.iter_mut().find(|c| c.lang == attrs.lang);

                    if let Some(existing) = same_lang {
                        existing.bodies.push(body);
                        if attrs.file.is_some() && existing.file_target.is_none() {
                            existing.file_target = attrs.file;
                        }
                        if attrs.symbol.is_some() && existing.symbol.is_none() {
                            existing.symbol = attrs.symbol;
                        }
                        if attrs.from.is_some() && existing.from.is_none() {
                            existing.from = attrs.from;
                        }
                        // Every fence of a chunk may name what it proves;
                        // the chunk carries the union, once each.
                        for id in attrs.proves {
                            if !existing.proves.contains(&id) {
                                existing.proves.push(id);
                            }
                        }
                    } else {
                        // New language variant for this chunk name
                        variants.push(Chunk {
                            name: name.clone(),
                            lang: attrs.lang.clone(),
                            bodies: vec![body],
                            file_target: attrs.file,
                            symbol: attrs.symbol,
                            from: attrs.from,
                            is_media: attrs.is_media,
                            proves: attrs.proves,
                        });
                    }

                    if !chunk_order.contains(&name) {
                        chunk_order.push(name);
                    }
                }
            }
            _ => {}
        }
    }

    Ok(ParsedDocument {
        id,
        tangle_crate,
        tangle_root,
        tangle_roots,
        chunks,
        chunk_order,
        pipelines,
    })
}
```

## Frontmatter helpers

`split_frontmatter` finds the `---` envelope; `parse_tangle_frontmatter`
hand-rolls a tiny YAML walker that picks out `tangle.crate`,
`tangle.root`, and the `tangle.roots:` language map without pulling in
`serde_yaml` for a handful of strings. The `roots:` submap is
recognized by indentation: once inside it, any deeper-indented
`lang: path` line is a per-language root, and the first line at or
above the `roots:` indent ends the submap.

<a name="chunk-split-frontmatter-fn"></a><sub>[`src/parser.rs`](../../../x0k-tangle/src/parser.rs) · `#split-frontmatter-fn`</sub>

```rust {#split-frontmatter-fn}
fn split_frontmatter(content: &str) -> (Option<&str>, &str) {
    if !content.starts_with("---") {
        return (None, content);
    }
    let after_first = &content[3..];
    if let Some(end) = after_first.find("\n---") {
        let yaml = &after_first[..end];
        let body_start = 3 + end + 4; // "---" + yaml + "\n---"
        let body = if body_start < content.len() {
            &content[body_start..]
        } else {
            ""
        };
        // Skip leading newline in body
        let body = body.strip_prefix('\n').unwrap_or(body);
        (Some(yaml), body)
    } else {
        (None, content)
    }
}
```

<a name="chunk-parse-tangle-frontmatter-fn"></a><sub>[`src/parser.rs`](../../../x0k-tangle/src/parser.rs) · `#parse-tangle-frontmatter-fn`</sub>

```rust {#parse-tangle-frontmatter-fn}
type TangleFrontmatter = (Option<String>, Option<PathBuf>, Vec<(String, PathBuf)>);

fn parse_tangle_frontmatter(yaml: Option<&str>) -> TangleFrontmatter {
    let Some(yaml) = yaml else {
        return (None, None, Vec::new());
    };

    // Simple extraction — look for tangle.crate, tangle.root, and the
    // tangle.roots: submap without pulling in a full YAML parser.
    let mut in_tangle = false;
    let mut roots_indent: Option<usize> = None;
    let mut crate_name = None;
    let mut root = None;
    let mut roots: Vec<(String, PathBuf)> = Vec::new();

    for line in yaml.lines() {
        let trimmed = line.trim();
        if trimmed == "tangle:" {
            in_tangle = true;
            continue;
        }
        if in_tangle {
            if !line.starts_with(' ') && !line.starts_with('\t') && !trimmed.is_empty() {
                break;
            }
            let indent = line.len() - line.trim_start().len();
            if let Some(ri) = roots_indent {
                // Inside `roots:` — deeper-indented `lang: path` lines
                // are entries; the first line at or above the submap's
                // indent ends it (and is processed normally below).
                if indent > ri && !trimmed.is_empty() {
                    if let Some((lang, path)) = trimmed.split_once(':') {
                        let path = path.trim();
                        if !path.is_empty() {
                            roots.push((lang.trim().to_string(), PathBuf::from(path)));
                        }
                    }
                    continue;
                }
                roots_indent = None;
            }
            if trimmed == "roots:" {
                roots_indent = Some(indent);
            } else if let Some(val) = trimmed.strip_prefix("crate:") {
                crate_name = Some(val.trim().to_string());
            } else if let Some(val) = trimmed.strip_prefix("root:") {
                root = Some(PathBuf::from(val.trim()));
            }
        }
    }

    (crate_name, root, roots)
}
```

## Tests

The tests are the spec: they pin down what the info-string parser
accepts and how the chunk map composes under repeated `#name`
fences and multi-language variants.

The fixture docs in these tests contain triple-backtick markdown
fences inside a Rust raw string. They tangle correctly because
language-aware chunk-ref extraction skips refs that fall inside
Rust raw strings (see [`chunk-refs.md`](chunk-refs.md)); without
that wrapper, the resolver would try to expand `<<imports>>` etc.
inside the fixture text.

<a name="chunk-tests"></a><sub>[`src/parser.rs`](../../../x0k-tangle/src/parser.rs) · `#tests`</sub>

`````rust {#tests}
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_info_string_basic() {
        let attrs = parse_info_string("rust {#my-chunk}");
        assert_eq!(attrs.lang.as_deref(), Some("rust"));
        assert_eq!(attrs.name.as_deref(), Some("my-chunk"));
        assert!(!attrs.is_media);
    }

    #[test]
    fn parse_info_string_with_file() {
        let attrs = parse_info_string("rust {#root file=\"src/lib.rs\"}");
        assert_eq!(attrs.name.as_deref(), Some("root"));
        assert_eq!(attrs.file.as_ref().unwrap().to_str(), Some("src/lib.rs"));
    }

    #[test]
    fn parse_info_string_from_symbol() {
        let attrs =
            parse_info_string("rust {#decode from=\"x0k-motif-api/src/protocol.rs\" symbol=\"MotifFrame::decode\"}");
        assert_eq!(attrs.name.as_deref(), Some("decode"));
        assert_eq!(
            attrs.from.as_ref().unwrap().to_str(),
            Some("x0k-motif-api/src/protocol.rs")
        );
        assert_eq!(attrs.symbol.as_deref(), Some("MotifFrame::decode"));
    }

    #[test]
    fn parse_info_string_media() {
        let attrs = parse_info_string("rust x0k:media {#viz}");
        assert!(attrs.is_media);
        assert_eq!(attrs.name.as_deref(), Some("viz"));
    }

    #[test]
    fn parse_info_string_proves_one_or_a_list() {
        let attrs = parse_info_string("rust {#pin proves=\"x0k:affordance/a\"}");
        assert_eq!(attrs.name.as_deref(), Some("pin"));
        assert_eq!(attrs.proves, vec!["x0k:affordance/a"]);

        let attrs = parse_info_string(
            "rust {#pin file=\"tests/pin.rs\" proves=\"x0k:affordance/a, x0k:affordance/b\"}",
        );
        assert_eq!(attrs.file.as_ref().unwrap().to_str(), Some("tests/pin.rs"));
        assert_eq!(attrs.proves, vec!["x0k:affordance/a", "x0k:affordance/b"]);

        // A bare value is one id; the comma after it separates attributes.
        let attrs = parse_info_string("rust {#pin proves=x0k:affordance/a, file=\"t.rs\"}");
        assert_eq!(attrs.proves, vec!["x0k:affordance/a"]);
        assert_eq!(attrs.file.as_ref().unwrap().to_str(), Some("t.rs"));

        // An attribute this parser does not know is skipped, not fatal.
        let attrs = parse_info_string("rust {#pin frobs=\"x\" proves=\"x0k:affordance/a\"}");
        assert_eq!(attrs.name.as_deref(), Some("pin"));
        assert_eq!(attrs.proves, vec!["x0k:affordance/a"]);
    }

    #[test]
    fn a_proving_chunk_carries_its_edge_and_is_otherwise_ordinary() {
        let doc = "```rust {#pin file=\"tests/pin.rs\" proves=\"x0k:affordance/a\"}\n#[test]\nfn one() {}\n```\n\n```rust {#pin proves=\"x0k:affordance/b, x0k:affordance/a\"}\n#[test]\nfn two() {}\n```\n";
        let parsed = parse_document(doc).unwrap();
        let pin = parsed.chunk("pin").unwrap();
        assert_eq!(pin.bodies.len(), 2, "append semantics are untouched");
        assert_eq!(pin.file_target.as_ref().unwrap().to_str(), Some("tests/pin.rs"));
        assert_eq!(pin.proves, vec!["x0k:affordance/a", "x0k:affordance/b"], "the union, once each");
        assert!(parsed.chunk("pin").unwrap().combined_body().contains("fn two"));
    }

    #[test]
    fn parse_document_extracts_chunks() {
        let doc = r#"---
x0k:
  format: folio/v1
  id: x0k:design/test
  type: design
  tangle:
    crate: x0k-test
    root: src/lib.rs
---
# Test

Some prose.

```rust {#imports}
use std::io;
```

More prose.

```rust {#main}
fn main() {
    <<imports>>
    println!("hello");
}
```
"#;
        let parsed = parse_document(doc).unwrap();
        assert_eq!(parsed.tangle_crate.as_deref(), Some("x0k-test"));
        assert_eq!(
            parsed.tangle_root.as_ref().unwrap().to_str(),
            Some("src/lib.rs")
        );
        assert_eq!(parsed.chunks.len(), 2);
        assert!(parsed.chunk("imports").is_some());
        assert!(parsed.chunk("main").is_some());
        assert_eq!(parsed.chunk_order, vec!["imports", "main"]);
        assert!(parsed.tangle_roots.is_empty());
    }

    #[test]
    fn parse_document_extracts_per_language_roots() {
        let doc = r#"---
x0k:
  format: folio/v1
  id: x0k:design/test
  type: design
  tangle:
    roots:
      rust: x0k-test/tests/demo.rs
      gallowglass: gls/demo/fold.gls
  edges:
    cites:
      - x0k:design/other
---
# Test
"#;
        let parsed = parse_document(doc).unwrap();
        assert!(parsed.tangle_crate.is_none());
        assert!(parsed.tangle_root.is_none());
        assert_eq!(
            parsed.tangle_roots,
            vec![
                ("rust".to_string(), PathBuf::from("x0k-test/tests/demo.rs")),
                ("gallowglass".to_string(), PathBuf::from("gls/demo/fold.gls")),
            ]
        );
    }

    #[test]
    fn media_chunks_flagged() {
        let doc = "```rust x0k:media {#viz}\nfn visualize() {}\n```\n";
        let parsed = parse_document(doc).unwrap();
        assert!(parsed.chunk("viz").unwrap().is_media);
    }

    #[test]
    fn append_semantics() {
        let doc = "```rust {#parts}\npart 1\n```\n\n```rust {#parts}\npart 2\n```\n";
        let parsed = parse_document(doc).unwrap();
        let parts = parsed.chunk("parts").unwrap();
        assert_eq!(parts.bodies.len(), 2);
        assert_eq!(parts.bodies[0].text, "part 1");
        assert_eq!(parts.bodies[1].text, "part 2");
    }

    #[test]
    fn multi_language_chunks() {
        let doc = r#"
```typescript {#zoom-toward file="src/camera.ts"}
zoomToward(sx: number, sy: number, delta: number) { }
```

```rust {#zoom-toward file="src/camera.rs"}
pub fn zoom_toward(&mut self, sx: f32, sy: f32, delta: f32) { }
```
"#;
        let parsed = parse_document(doc).unwrap();
        // One chunk name, two language variants
        let variants = parsed.chunk_variants("zoom-toward").unwrap();
        assert_eq!(variants.len(), 2);
        assert_eq!(variants[0].lang.as_deref(), Some("typescript"));
        assert_eq!(variants[1].lang.as_deref(), Some("rust"));
        assert!(variants[0].combined_body().contains("zoomToward"));
        assert!(variants[1].combined_body().contains("zoom_toward"));
        // chunk() returns first variant
        assert_eq!(parsed.chunk("zoom-toward").unwrap().lang.as_deref(), Some("typescript"));
        // chunk_for_lang selects by language
        let rust = parsed.chunk_for_lang("zoom-toward", "rust").unwrap();
        assert!(rust.combined_body().contains("zoom_toward"));
        let ts = parsed.chunk_for_lang("zoom-toward", "typescript").unwrap();
        assert!(ts.combined_body().contains("zoomToward"));
        // Only one entry in chunk_order
        assert_eq!(parsed.chunk_order.iter().filter(|n| *n == "zoom-toward").count(), 1);
    }

    #[test]
    fn same_lang_appends() {
        let doc = r#"
```rust {#parts}
part 1
```

```rust {#parts}
part 2
```

```typescript {#parts}
ts part
```
"#;
        let parsed = parse_document(doc).unwrap();
        let variants = parsed.chunk_variants("parts").unwrap();
        // Two variants: rust (2 bodies appended) and typescript (1 body)
        assert_eq!(variants.len(), 2);
        let rust = variants.iter().find(|c| c.lang.as_deref() == Some("rust")).unwrap();
        assert_eq!(rust.bodies.len(), 2);
        let ts = variants.iter().find(|c| c.lang.as_deref() == Some("typescript")).unwrap();
        assert_eq!(ts.bodies.len(), 1);
    }
}
`````

## Composing the module

<a name="chunk-root"></a><sub>[`src/parser.rs`](../../../x0k-tangle/src/parser.rs) · `#root` · assembles [imports](#chunk-imports) · [parsed-document-type](#chunk-parsed-document-type) · [parsed-document-impl](#chunk-parsed-document-impl) · [chunk-attrs-type](#chunk-chunk-attrs-type) · [parse-info-string-fn](#chunk-parse-info-string-fn) · [parse-brace-attrs-fn](#chunk-parse-brace-attrs-fn) · [extract-quoted-or-bare-fn](#chunk-extract-quoted-or-bare-fn) · [parse-document-fn](#chunk-parse-document-fn) · [split-frontmatter-fn](#chunk-split-frontmatter-fn) · [parse-tangle-frontmatter-fn](#chunk-parse-tangle-frontmatter-fn) · [tests](#chunk-tests)</sub>

```rust {#root}
<<imports>>

<<parsed-document-type>>

<<parsed-document-impl>>

<<chunk-attrs-type>>

<<parse-info-string-fn>>

<<parse-brace-attrs-fn>>

<<extract-quoted-or-bare-fn>>

<<parse-document-fn>>

<<split-frontmatter-fn>>

<<parse-tangle-frontmatter-fn>>

<<tests>>
```
