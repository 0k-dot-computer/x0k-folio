---
x0k:
  format: folio/v1
  id: x0k:implementation/tangle/multi-doc-resolve
  type: implementation
  status: draft
  summary: Lifting expansion from one document to a corpus indexed by envelope id, so `<<uri::chunk>>` transcludes a chunk defined elsewhere without a second implementation of the expansion walk.
  concerns: [tangle, literate, resolve, transclusion, cross-document, cycle-detection]
  tangle:
    crate: x0k-tangle
    root: src/multi_doc_resolve.rs
  edges:
    cites:
      - x0k:implementation/tangle/protocol
      - x0k:implementation/tangle/chunk
      - x0k:implementation/tangle/chunk-refs
      - x0k:implementation/tangle/parsing
      - x0k:implementation/tangle/resolution
---
# Cross-document chunk transclusion

Within-document resolution ([`resolution.md`](resolution.md)) expands
`<<chunk>>` refs against one parsed document. This module lifts that to
a **corpus**: a set of parsed documents indexed by their frontmatter
`id:` URI, so a chunk in one doc can transclude a chunk defined in
another via `<<uri::chunk>>`.

The motivating use is a future literate agent-profile system — a
profile document assembles itself from shared prompt-fragment chunks
that live in other documents, the same way a literate program assembles
a function from chunks defined elsewhere in the file. Cross-doc
transclusion is the substrate primitive that makes "shared fragment,
referenced by URI" expressible.

The design is deliberately thin. We do *not* re-implement expansion;
we reuse [`resolution.md`](resolution.md)'s `expand_recursive`, which is
already generalized to carry an optional corpus and a current-document
URI. This module owns only two things: the `Corpus` index type and the
public entry point that seeds expansion with the right starting
document.

## Imports

```rust {#imports}
use crate::parser::ParsedDocument;
use crate::resolve::{expand_recursive, VisitKey};
use anyhow::Result;
use std::collections::HashMap;
use std::collections::HashSet;
```

## The Corpus

A `Corpus` borrows a set of parsed documents, indexed by URI. We hold
references, not owned docs: callers parse documents once (the CLI walks
a directory; a test builds a handful inline) and lend them to the
resolver for the duration of an expansion. The lifetime `'a` ties the
borrowed map to the documents it points at.

```rust {#corpus-type}
/// A set of parsed documents indexed by their frontmatter `id:` URI.
/// `<<uri::chunk>>` refs resolve the document by URI, then the chunk
/// within it.
pub struct Corpus<'a> {
    docs: HashMap<String, &'a ParsedDocument>,
}
```

## Building and querying

`from_docs` indexes an iterator of parsed documents by their `id`,
skipping any document with no `id:` URI (it can't be a cross-doc
target). `doc` is the single lookup the resolver needs.

```rust {#corpus-impl}
impl<'a> Corpus<'a> {
    /// Index the given documents by their `id:` URI. Documents without
    /// an `id` are skipped — they can never be a cross-doc target.
    pub fn from_docs(docs: impl IntoIterator<Item = &'a ParsedDocument>) -> Self {
        let mut map = HashMap::new();
        for doc in docs {
            if let Some(id) = &doc.id {
                map.insert(id.clone(), doc);
            }
        }
        Corpus { docs: map }
    }

    /// Look up a document by its `id:` URI.
    pub fn doc(&self, uri: &str) -> Option<&'a ParsedDocument> {
        self.docs.get(uri).copied()
    }

    /// Number of indexed documents.
    pub fn len(&self) -> usize {
        self.docs.len()
    }

    pub fn is_empty(&self) -> bool {
        self.docs.is_empty()
    }
}
```

## The entry point

`expand_chunk_in_corpus` expands one chunk in one starting document,
resolving any `<<uri::chunk>>` refs (recursively) against the corpus.
It allocates a fresh visited set keyed by `(doc_uri, chunk_name)` — the
cross-doc cycle guard — and seeds it with the starting document's own
URI so the first frame's bare refs key correctly.

The starting document must itself be in the corpus (it's the natural
shape: index everything, then expand a chunk in one of them). We take
the URI explicitly rather than reading `start.id` so a caller can
expand the same parsed doc under a chosen identity if it ever needs to.

```rust {#expand-in-corpus-fn}
pub fn expand_chunk_in_corpus(
    corpus: &Corpus,
    start_uri: &str,
    chunk_name: &str,
) -> Result<String> {
    let start = corpus.doc(start_uri).ok_or_else(|| {
        anyhow::anyhow!("starting document '{}' not found in corpus", start_uri)
    })?;
    let mut visited: HashSet<VisitKey> = HashSet::new();
    expand_recursive(Some(corpus), start_uri, start, chunk_name, None, &mut visited)
}
```

## Tests

The tests pin the four behaviors that make cross-doc transclusion
correct: a plain two-doc transclusion, a recursive case (a transcluded
chunk itself reaches into a third doc), a cross-doc cycle that errors
cleanly instead of looping, and indentation preservation across the
hop.

The fixture docs embed triple-backtick markdown fences inside Rust raw
strings. Language-aware ref extraction skips `<<...>>` tokens inside
those raw strings (see [`chunk-refs.md`](chunk-refs.md)), so the doc
tangles without the resolver chasing its own fixture data.

`````rust {#tests}
#[cfg(test)]
mod tests {
    use super::*;
    use crate::parser::parse_document;

    fn doc_a() -> ParsedDocument {
        parse_document(
            r#"---
x0k:
  format: folio/v1
  id: x0k:implementation/tangle/doc-a
  type: implementation
---
```rust {#greeting}
println!("hello from A");
```
"#,
        )
        .unwrap()
    }

    #[test]
    fn transcludes_chunk_from_another_doc() {
        let a = doc_a();
        let b = parse_document(
            r#"---
x0k:
  format: folio/v1
  id: x0k:implementation/tangle/doc-b
  type: implementation
---
```rust {#main}
fn main() {
    <<x0k:implementation/tangle/doc-a::greeting>>
}
```
"#,
        )
        .unwrap();

        let corpus = Corpus::from_docs([&a, &b]);
        assert_eq!(corpus.len(), 2);
        let out =
            expand_chunk_in_corpus(&corpus, "x0k:implementation/tangle/doc-b", "main").unwrap();
        assert!(out.contains("fn main()"));
        assert!(out.contains("hello from A"));
    }

    #[test]
    fn cross_doc_preserves_indent() {
        let a = doc_a();
        let b = parse_document(
            r#"---
x0k:
  format: folio/v1
  id: x0k:implementation/tangle/doc-b
  type: implementation
---
```rust {#main}
fn main() {
    <<x0k:implementation/tangle/doc-a::greeting>>
}
```
"#,
        )
        .unwrap();

        let corpus = Corpus::from_docs([&a, &b]);
        let out =
            expand_chunk_in_corpus(&corpus, "x0k:implementation/tangle/doc-b", "main").unwrap();
        // The ref line was indented four spaces; the transcluded body
        // line should carry that indent.
        assert!(out.contains("    println!(\"hello from A\");"));
    }

    #[test]
    fn recursive_cross_doc_transclusion() {
        // c -> b -> a, all across document boundaries.
        let a = doc_a();
        let b = parse_document(
            r#"---
x0k:
  format: folio/v1
  id: x0k:implementation/tangle/doc-b
  type: implementation
---
```rust {#wrapper}
{
    <<x0k:implementation/tangle/doc-a::greeting>>
}
```
"#,
        )
        .unwrap();
        let c = parse_document(
            r#"---
x0k:
  format: folio/v1
  id: x0k:implementation/tangle/doc-c
  type: implementation
---
```rust {#main}
fn main()
    <<x0k:implementation/tangle/doc-b::wrapper>>
```
"#,
        )
        .unwrap();

        let corpus = Corpus::from_docs([&a, &b, &c]);
        let out =
            expand_chunk_in_corpus(&corpus, "x0k:implementation/tangle/doc-c", "main").unwrap();
        assert!(out.contains("fn main()"));
        assert!(out.contains("hello from A"));
        // The greeting picked up doc-c's indent (4) plus doc-b's brace
        // block indent (4) = 8 spaces at the deepest call site.
        assert!(out.contains("        println!(\"hello from A\");"), "got:\n{out}");
    }

    #[test]
    fn cross_doc_cycle_errors() {
        // doc-x::a -> doc-y::b -> doc-x::a — a cycle across the boundary.
        let x = parse_document(
            r#"---
x0k:
  format: folio/v1
  id: x0k:implementation/tangle/doc-x
  type: implementation
---
```rust {#a}
<<x0k:implementation/tangle/doc-y::b>>
```
"#,
        )
        .unwrap();
        let y = parse_document(
            r#"---
x0k:
  format: folio/v1
  id: x0k:implementation/tangle/doc-y
  type: implementation
---
```rust {#b}
<<x0k:implementation/tangle/doc-x::a>>
```
"#,
        )
        .unwrap();

        let corpus = Corpus::from_docs([&x, &y]);
        let err =
            expand_chunk_in_corpus(&corpus, "x0k:implementation/tangle/doc-x", "a").unwrap_err();
        assert!(err.to_string().contains("cycle"), "got: {err}");
    }

    #[test]
    fn unknown_target_doc_errors() {
        let b = parse_document(
            r#"---
x0k:
  format: folio/v1
  id: x0k:implementation/tangle/doc-b
  type: implementation
---
```rust {#main}
<<x0k:implementation/tangle/does-not-exist::nope>>
```
"#,
        )
        .unwrap();
        let corpus = Corpus::from_docs([&b]);
        let err =
            expand_chunk_in_corpus(&corpus, "x0k:implementation/tangle/doc-b", "main").unwrap_err();
        assert!(err.to_string().contains("unknown document"), "got: {err}");
    }
}
`````

## Composing the module

```rust {#root}
<<imports>>

<<corpus-type>>

<<corpus-impl>>

<<expand-in-corpus-fn>>

<<tests>>
```
