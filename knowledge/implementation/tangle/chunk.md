---
x0k:
  format: folio/v1
  id: x0k:implementation/tangle/chunk
  type: implementation
  status: draft
  summary: The in-memory types the parser produces, the resolver consumes and the weaver renders, kept free of traversal logic so every other module can share them without acquiring dependencies.
  concerns: [tangle, literate, chunks, data-shape]
  tangle:
    crate: x0k-tangle
    root: src/chunk.rs
  edges:
    cites:
      - x0k:implementation/tangle/protocol
      - x0k:implementation/tangle/resolution
      - x0k:implementation/tangle/chunk-refs
      - x0k:implementation/tangle/pipeline
---
# The chunk shape

`chunk.rs` defines what a "chunk" is in the literate substrate — the
in-memory shape that the parser produces, the resolver consumes, and
the weaver renders. Keeping this module tight (one type per concept,
no traversal logic) means resolve / weave / identity-pipeline can all
import the same types without acquiring extra deps.

## Imports

The module needs only `PathBuf` for the `file_target` / `from`
attributes; everything else is plain types.

```rust {#imports}
use std::path::PathBuf;
```

## The Chunk type

A `Chunk` carries:

- `name` — what `#name` the author put in the fence info-string.
- `lang` — the language token (e.g. `"rust"`, `"typescript"`); `None`
  when the fence had no language.
- `bodies` — a vec rather than a single string, because multiple
  fences with the same `#name` accumulate (the classic literate
  "presented in pieces, assembled in compile order" pattern).
- `file_target` — when the fence had `file=`, where this chunk's
  expansion lands.
- `symbol`, `from` — set when the chunk pulls its body from a source
  file via `from=` (read-only reference, not an owned chunk).
- `is_media` — set when the info-string carried `x0k:media`, marking
  the block as a viz embed rather than code.

```rust {#chunk-types}
#[derive(Debug, Clone)]
pub struct Chunk {
    pub name: String,
    pub lang: Option<String>,
    pub bodies: Vec<ChunkBody>,
    pub file_target: Option<PathBuf>,
    pub symbol: Option<String>,
    pub from: Option<PathBuf>,
    pub is_media: bool,
}

pub fn lang_matches_extension(lang: Option<&str>, ext: &str) -> bool {
    match (lang, ext) {
        (None, _) => true,
        (Some("rust"), "rs") => true,
        (Some("typescript"), "ts") | (Some("ts"), "ts") => true,
        (Some("javascript"), "js") | (Some("js"), "js") => true,
        (Some("python"), "py") => true,
        (Some(l), e) => l == e,
    }
}
```

The `lang_matches_extension` helper handles the small case where a
multi-language chunk needs to pick a variant by output file extension
— the identity pipeline uses it to decide which language variant
goes into a `.rs` vs a `.ts` output target.

## ChunkBody

A single fence's body, plus the line number in the source `.md` for
diagnostics and stitch-back round-trips.

```rust {#chunk-body}
#[derive(Debug, Clone)]
pub struct ChunkBody {
    pub text: String,
    pub source_line: usize,
}
```

## ChunkRef

A located `<<name>>` occurrence inside a chunk body. The indent string
is captured because the resolver applies it as a prefix to every line
of the expanded content (so nested chunks stay indented at the call
site).

A ref can also name a chunk in *another* document:
`<<x0k:doc/uri::chunk-name>>` carries the `::`-separated target doc
URI in `doc_uri`. A bare `<<chunk-name>>` leaves `doc_uri` `None` —
it resolves within the current document, exactly as before. The
single-doc resolver ignores `doc_uri`; the corpus resolver (see
[`multi-doc-resolve.md`](multi-doc-resolve.md)) uses it to hop docs.

A ref can also be *escaped*: a line reading `<<!name>>` is a request
for the literal text `<<name>>` in the output, not for an expansion.
The finder still reports it — as a `ChunkRef` with `escaped` set and
`name` holding the text after the `!` — so that every consumer sees
the line through the same scanner: the resolver emits it verbatim
(indent kept), the checker and the fragment-set pass skip it, and the
weaver renders it without an anchor. Nothing after the `!` is
interpreted; `doc_uri` is always `None` on an escaped ref.

```rust {#chunk-ref}
#[derive(Debug, Clone)]
pub struct ChunkRef {
    pub name: String,
    /// `Some(uri)` when the ref targets a chunk in another document
    /// (`<<uri::name>>`); `None` for a within-document `<<name>>`.
    pub doc_uri: Option<String>,
    pub indent: String,
    pub line_in_body: usize,
    /// The line read `<<!name>>`: a verbatim `<<name>>` is wanted in
    /// the output, and nothing is expanded.
    pub escaped: bool,
}
```

## Methods on Chunk

Two small affordances:

- `is_from_ref()` — true when the chunk pulls its body from a source
  file via `from=`. Used by every dispatcher to skip from-chunks
  during identity tangle (they're inputs, not outputs).
- `combined_body()` — join all body sections with `\n`. Multi-body
  chunks (multiple `#name` fences in the same doc) become one
  continuous string.

```rust {#chunk-methods}
impl Chunk {
    pub fn is_from_ref(&self) -> bool {
        self.from.is_some()
    }

    pub fn combined_body(&self) -> String {
        self.bodies
            .iter()
            .map(|b| b.text.as_str())
            .collect::<Vec<_>>()
            .join("\n")
    }
}
```

## Finding chunk references

The `<<name>>` reference syntax is parsed here, called during
expansion (by `resolve.rs`) and verification (by `check_all_refs`).
The rule: a line, after trimming leading whitespace, must be
`<<name>>` exactly — no commentary, no inline mixing. The function
reports each ref's name, the indent prefix, and the zero-indexed line
position inside the body.

A ref's interior may carry a `::`-delimited document URI:
`<<x0k:doc/uri::chunk-name>>`. We split on the *last* `::` so a chunk
name itself never contains `::` ambiguity while the doc URI (which can
contain `:` from the `x0k:` scheme) stays intact — the chunk name is
the segment after the final `::`, the doc URI is everything before it.
A bare `<<name>>` has no `::`, so `doc_uri` stays `None` and resolution
is within the current doc, unchanged.

A `!` directly inside the brackets — `<<!name>>` — is the verbatim
escape (noweb's `@<<` is the precedent): the ref is reported with
`escaped` set, its `name` is the text after the `!` (trimmed, and held
to the same non-empty/no-spaces rule, so `<<! a b >>` is no ref at all
and passes through untouched), and no `::` split is attempted, because
what follows the `!` is never resolved. The choice of `!` is a
reservation, not a coincidence: the fence-attribute parser
([`parsing.md`](parsing.md), `parse_brace_attrs`) reads a chunk name
as `#` followed by whatever runs up to whitespace, `,` or `}` — it
would accept `{#!x}` as a name — so a bare grammar check cannot show
that `!` was already impossible. What makes it safe is that no chunk
in the corpus is named with a leading `!`, and from here on none can
be reached by reference: `<<!x>>` always means the literal `<<x>>`.

```rust {#find-chunk-refs}
pub fn find_chunk_refs(text: &str) -> Vec<ChunkRef> {
    let mut refs = Vec::new();
    for (i, line) in text.lines().enumerate() {
        let trimmed = line.trim_start();
        if let Some(rest) = trimmed.strip_prefix("<<") {
            if let Some(inner) = rest.strip_suffix(">>") {
                let inner = inner.trim();
                let (doc_uri, name, escaped) = match inner.strip_prefix('!') {
                    Some(literal) => (None, literal.trim(), true),
                    None => {
                        let (doc_uri, name) = split_ref_target(inner);
                        (doc_uri, name, false)
                    }
                };
                if !name.is_empty() && !name.contains(' ') {
                    let indent_len = line.len() - trimmed.len();
                    refs.push(ChunkRef {
                        name: name.to_string(),
                        doc_uri,
                        indent: line[..indent_len].to_string(),
                        line_in_body: i,
                        escaped,
                    });
                }
            }
        }
    }
    refs
}
```

`split_ref_target` separates an optional doc URI from the chunk name.
It uses `rsplit_once("::")` so the chunk name is the final segment and
the doc URI is everything before the last `::`. An empty doc URI side
(a ref written `<<::name>>`) is treated as no URI, falling back to
within-doc resolution.

```rust {#split-ref-target}
fn split_ref_target(inner: &str) -> (Option<String>, &str) {
    match inner.rsplit_once("::") {
        Some((uri, name)) if !uri.trim().is_empty() => {
            (Some(uri.trim().to_string()), name.trim())
        }
        _ => (None, inner),
    }
}
```

The strictness — "the entire trimmed line must be `<<x>>`" — is a
deliberate constraint. Chunk refs are *structural*: they say "drop
the expansion of chunk X here," and they're meant to compose cleanly
with whatever indent context the call-site provides. They are
emphatically NOT meant to be inlined inside expressions, comments, or
string literals. A line containing `// <<x>>` is not a ref because
`//` survives `trim_start`. A line containing `let s = "<<x>>"` is
not a ref because text follows the closing `>>`.

This single-mechanism design used to make the substrate
self-illustrating-but-self-limiting: when a literate doc's chunks
contain string literals that happen to embed `<<x>>` syntax (as the
substrate's own `parser.rs` and `resolve.rs` tests do, deliberately),
those lines would look like chunk refs to the resolver. The fix is
a language-aware overlay, [`chunk_refs.rs`](chunk-refs.md), that
uses tree-sitter to identify string-literal and comment spans and
filter naive matches accordingly. Callers that know the chunk's
language pass it in; callers (or chunks) without a known grammar
fall back to the naive scan.

## Tests

The chunk tests exercise the `find_chunk_refs` cases: a body with
mixed standalone-ref and indented-ref lines; a body with a comment
that mentions `<<x>>` inline, which must NOT be detected as a ref,
because the literate syntax is structural; the cross-doc split; and
the escape, which is reported as a ref with `escaped` set, keeps its
indent, and takes no `::` split even when the literal contains one.

```rust {#tests}
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn find_refs_basic() {
        let text = "use std::io;\n<<imports>>\n    <<helpers>>\nfn main() {}";
        let refs = find_chunk_refs(text);
        assert_eq!(refs.len(), 2);
        assert_eq!(refs[0].name, "imports");
        assert_eq!(refs[0].indent, "");
        assert_eq!(refs[0].line_in_body, 1);
        assert_eq!(refs[1].name, "helpers");
        assert_eq!(refs[1].indent, "    ");
        assert_eq!(refs[1].line_in_body, 2);
    }

    #[test]
    fn ignores_non_refs() {
        let text = "let x = vec![1, 2];\n// <<not-a-ref>>";
        let refs = find_chunk_refs(text);
        assert_eq!(refs.len(), 0);
    }

    #[test]
    fn cross_doc_ref_splits_uri_and_name() {
        let text = "    <<x0k:implementation/tangle/shared::greeting>>";
        let refs = find_chunk_refs(text);
        assert_eq!(refs.len(), 1);
        assert_eq!(refs[0].doc_uri.as_deref(), Some("x0k:implementation/tangle/shared"));
        assert_eq!(refs[0].name, "greeting");
        assert_eq!(refs[0].indent, "    ");
    }

    #[test]
    fn bare_ref_has_no_doc_uri() {
        let text = "<<greeting>>";
        let refs = find_chunk_refs(text);
        assert_eq!(refs.len(), 1);
        assert_eq!(refs[0].doc_uri, None);
        assert_eq!(refs[0].name, "greeting");
        assert!(!refs[0].escaped);
    }

    #[test]
    fn escaped_ref_is_reported_verbatim_with_its_indent() {
        let text = "  <<!greeting>>\n<<! not a ref >>\n<<!x0k:doc::name>>";
        let refs = find_chunk_refs(text);
        assert_eq!(refs.len(), 2);
        assert!(refs[0].escaped);
        assert_eq!(refs[0].name, "greeting");
        assert_eq!(refs[0].indent, "  ");
        assert_eq!(refs[0].line_in_body, 0);
        assert!(refs[1].escaped);
        assert_eq!(refs[1].name, "x0k:doc::name");
        assert_eq!(refs[1].doc_uri, None);
        assert_eq!(refs[1].line_in_body, 2);
    }
}
```

## Composing the module

```rust {#root}
<<imports>>

<<chunk-types>>

<<chunk-body>>

<<chunk-ref>>

<<chunk-methods>>

<<find-chunk-refs>>

<<split-ref-target>>

<<tests>>
```
