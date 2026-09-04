---
x0k:
  format: folio/v1
  id: x0k:implementation/tangle/resolution
  type: implementation
  status: draft
  summary: Recursive `<<name>>` expansion with the call site's indentation re-applied, the language-pinned walk a bilingual document needs, and the sweep that reports undefined refs and cycles before a tangle runs.
  concerns: [tangle, literate, resolve, expansion, cycle-detection]
  tangle:
    crate: x0k-tangle
    root: src/resolve.rs
  edges:
    cites:
      - x0k:implementation/tangle/protocol
      - x0k:implementation/tangle/chunk
      - x0k:implementation/tangle/chunk-refs
      - x0k:implementation/tangle/parsing
---
# Resolving `<<chunk-ref>>` expansion

Parsing gives us a chunk map. Resolution turns one of those
chunks into its fully-expanded body — every `<<name>>` line
replaced by the recursive expansion of the named chunk, with the
call-site's indentation re-applied to every output line.

Three public functions:

- `expand_chunk(doc, name)` — recursively expand one chunk. Used
  by the identity pipeline and by the weaver's preview surface.
- `expand_chunk_lang(doc, name, lang)` — the same walk pinned to
  one fence language. A bilingual document defines the same chunk
  name once per language; when the identity pipeline emits the
  Gallowglass body it must resolve `<<refs>>` to the Gallowglass
  variants, not whichever variant was declared first.
- `check_all_refs(doc)` — sweep every non-media chunk in the doc
  looking for undefined references and expansion cycles. Used by
  the `x0k-tangle check` CLI to surface authoring errors before
  tangling.

Expansion is language-aware: we ask
`chunk_refs::find_chunk_refs_aware` for the reference set rather
than the naive line-based scanner. This lets the substrate
explain itself — a Rust chunk whose body contains `r#"<<x>>"#`
as a literal string is correctly identified as code-with-data,
not code-with-a-ref.

Expansion has one escape. A line reading `<<!name>>` — the `!`
directly inside the brackets, indentation allowed before it — is not a
reference but a request for the literal line `<<name>>` in the output,
with the indentation kept. It exists so a document can *show* the
reference syntax in what it tangles: a README that teaches the
`<<greet>>` line, a fixture that carries one. The precedent is noweb's
`@<<`. `!` is reserved for this by the scanner
([`chunk.md`](chunk.md) § "Finding chunk references"): the fence
parser's name grammar is permissive enough that `{#!x}` would parse,
so the guarantee is not "no such chunk could exist" but "no such chunk
can be reached by reference" — `<<!x>>` is always the literal, never
the chunk. Inside a Rust string literal or comment the escape is left
alone like any other `<<x>>` there (the language-aware scanner filters
both the same way), which is what lets this chapter's own tests carry
`<<!…>>` inside their fixture strings and still tangle. The escape
produces text; it takes no `::` split and is never checked against
the chunk map. The reverse path ([`reverse-stitch.md`](reverse-stitch.md))
re-escapes a lifted `<<name>>` line so the literal survives a
round-trip.

Expansion is also *corpus-aware*. A ref written `<<uri::chunk>>`
names a chunk in another document; resolving it requires a set of
parsed documents indexed by their frontmatter `id:` URI. The core
walker is generalized to carry an optional [`Corpus`] (defined in
[`multi-doc-resolve.md`](multi-doc-resolve.md)) and the URI of the
document currently being expanded. Bare `<<chunk>>` refs resolve
within that current document exactly as before; the single-doc entry
points pass `corpus = None`, so the within-doc path is untouched.

## Imports

```rust {#imports}
use crate::chunk_refs::find_chunk_refs_aware;
use crate::multi_doc_resolve::Corpus;
use crate::parser::ParsedDocument;
use anyhow::{bail, Result};
use std::collections::HashSet;

/// The cycle-detection key. For a within-doc expansion the doc-URI
/// slot is the empty string; for a corpus expansion it is the target
/// document's `id:` URI, so the same chunk name in two different docs
/// is two distinct nodes in the visited set.
pub(crate) type VisitKey = (String, String);
```

## expand_chunk

A thin wrapper that allocates the `visited` set used by the
cycle-detection guard in `expand_recursive`. Each call gets a
fresh set so multiple expansions on the same doc don't
cross-contaminate.

This is the single-doc entry point: it passes `corpus = None` and an
empty current-doc URI, so `<<uri::chunk>>` refs (which need a corpus)
error as undefined-cross-doc and bare `<<chunk>>` refs resolve
within `doc`. The corpus entry point lives in
[`multi-doc-resolve.md`](multi-doc-resolve.md) and shares the same
engine below.

```rust {#expand-chunk-fn}
pub fn expand_chunk(doc: &ParsedDocument, name: &str) -> Result<String> {
    let mut visited = HashSet::new();
    expand_recursive(None, "", doc, name, None, &mut visited)
}

/// Expand one chunk with reference resolution pinned to `lang`:
/// every `<<name>>` resolves to the named chunk's `lang` variant,
/// falling back to the first variant when no lang-specific one
/// exists (the `chunk_for_lang` contract).
pub fn expand_chunk_lang(doc: &ParsedDocument, name: &str, lang: &str) -> Result<String> {
    let mut visited = HashSet::new();
    expand_recursive(None, "", doc, name, Some(lang), &mut visited)
}
```

## expand_recursive

The heart of the resolver. Walks the chunk's body line by line,
substituting any reference with its recursive expansion. The
key mechanics:

- **Cycle guard.** The visited set is keyed by `(doc_uri, name)`,
  not just `name`, so a cross-doc cycle (doc A → doc B → doc A) is
  caught the same way an intra-doc one is. `visited.insert(key)`
  returns false on a re-visit; that's the signal we've looped back to
  a chunk already on the expansion stack, and we bail rather than
  overflow. When we leave a frame we `visited.remove(&key)` so
  siblings can still reference the same fragment (cycles are about
  *parent* references, not siblings).
- **Missing chunk.** Returns an `undefined chunk` error with
  the `<<name>>` formatting the operator sees in source.
- **Escape.** An escaped ref (`<<!name>>`) is emitted as the literal
  `<<name>>` under the call-site indent and expands nothing; it is
  handled before the doc-URI branch so the text after the `!` is
  never treated as a target.
- **Language pinning.** With `lang = Some(l)`, chunk lookups go
  through `chunk_for_lang` so a bilingual doc's shared chunk names
  resolve to the requested language's variants throughout the walk.
  `None` keeps the first-variant lookup. The pin travels down the
  recursion unchanged — a Gallowglass expansion never silently
  splices in a Rust fragment.
- **Media chunks.** A `x0k:media` chunk is a viz embed, not
  code; trying to expand one is a programmer error and we
  bail. The identity pipeline filters media chunks out
  upstream so this branch only triggers when somebody
  bypasses the filter.
- **Cross-doc hop.** When a ref carries a `doc_uri`, we need a
  corpus to find the target document. With no corpus (the single-doc
  path), that's an error. With a corpus, we look the target doc up by
  URI and recurse into it — the recursion carries the *target's* URI
  as the new current doc, so the resolved chunk's own bare refs
  resolve against the doc that defined it, and the visited key tracks
  the right document.
- **Indent preservation.** When the ref line has leading
  whitespace, every line of the expansion gets that whitespace
  re-applied (empty lines stay empty). This holds across a cross-doc
  hop because indentation is applied at the call site, not the
  definition site.
- **Trailing-newline normalization.** We append `\n` after every
  expanded chunk and after every literal line; the final `pop()`
  strips the trailing `\n` so the returned string matches the
  input convention (`combined_body` does not include a trailing
  newline).

```rust {#expand-recursive-fn}
pub(crate) fn expand_recursive(
    corpus: Option<&Corpus>,
    current_uri: &str,
    doc: &ParsedDocument,
    name: &str,
    lang: Option<&str>,
    visited: &mut HashSet<VisitKey>,
) -> Result<String> {
    let key = (current_uri.to_string(), name.to_string());
    if !visited.insert(key.clone()) {
        bail!("cycle detected: chunk '{}' references itself", name);
    }

    let chunk = match lang {
        Some(l) => doc.chunk_for_lang(name, l),
        None => doc.chunk(name),
    }
    .ok_or_else(|| anyhow::anyhow!("undefined chunk: '<<{}>>'", name))?;

    if chunk.is_media {
        bail!(
            "chunk '{}' is a x0k:media block and cannot be expanded",
            name
        );
    }

    let combined = chunk.combined_body();
    let refs = find_chunk_refs_aware(&combined, chunk.lang.as_deref());

    if refs.is_empty() {
        visited.remove(&key);
        return Ok(combined);
    }

    let mut result = String::new();
    for (i, line) in combined.lines().enumerate() {
        if let Some(r) = refs.iter().find(|r| r.line_in_body == i) {
            if r.escaped {
                // `<<!name>>` asks for the literal line `<<name>>`.
                result.push_str(&r.indent);
                result.push_str("<<");
                result.push_str(&r.name);
                result.push_str(">>\n");
                continue;
            }
            // Resolve the ref's target document. A bare ref stays in
            // the current doc; a `uri::` ref hops via the corpus.
            let expanded = match &r.doc_uri {
                None => expand_recursive(corpus, current_uri, doc, &r.name, lang, visited)?,
                Some(uri) => {
                    let corpus = corpus.ok_or_else(|| {
                        anyhow::anyhow!(
                            "cross-doc ref '<<{}::{}>>' requires a corpus; \
                             use a corpus resolver, not single-doc expand",
                            uri,
                            r.name
                        )
                    })?;
                    let target = corpus.doc(uri).ok_or_else(|| {
                        anyhow::anyhow!(
                            "cross-doc ref '<<{}::{}>>' targets unknown document '{}'",
                            uri,
                            r.name,
                            uri
                        )
                    })?;
                    expand_recursive(Some(corpus), uri, target, &r.name, lang, visited)?
                }
            };
            for (j, expanded_line) in expanded.lines().enumerate() {
                if j > 0 {
                    result.push('\n');
                }
                if !expanded_line.is_empty() {
                    result.push_str(&r.indent);
                    result.push_str(expanded_line);
                } else {
                    result.push_str(expanded_line);
                }
            }
            result.push('\n');
        } else {
            result.push_str(line);
            result.push('\n');
        }
    }

    // Remove trailing newline to match input convention
    if result.ends_with('\n') {
        result.pop();
    }

    visited.remove(&key);
    Ok(result)
}
```

## check_all_refs

The author-time validator. Sweeps every non-media chunk variant
and collects:

- Undefined references — any `<<name>>` pointing at a chunk that
  doesn't exist in the doc.
- Cycles — surfaced by attempting `expand_recursive` on each
  chunk and capturing any error whose message contains
  `"cycle"`.

Returns a `Vec<String>` of human-readable diagnostics; the CLI
prints them and exits non-zero when the vec is non-empty.

Three design notes:

- We walk **variants**, not just `chunk(name)`, so a doc with a
  Rust variant and a TypeScript variant of the same name has
  both bodies scanned for refs. Both must be self-consistent.
- **Cross-doc refs are out of scope here.** `check_all_refs` validates
  one document in isolation; a `<<uri::chunk>>` ref points outside it,
  so we skip undefined-chunk checking for refs carrying a `doc_uri`
  (the corpus resolver validates those when it has the whole set).
  Bare refs are still checked against the local chunk map. An escaped
  ref is text, so it is skipped too — `<<!nonexistent>>` is not an
  undefined reference.
- The cycle pass goes through `expand_recursive` rather than a
  separate graph walk because the expansion logic IS the
  authoritative cycle definition (visited-set based, scoped to
  one ancestor chain). Reimplementing that as a separate walk
  would drift over time. We pass `corpus = None`, so a cross-doc ref
  reached during this single-doc pass surfaces as an error rather
  than a phantom cycle.

```rust {#check-all-refs-fn}
pub fn check_all_refs(doc: &ParsedDocument) -> Result<Vec<String>> {
    let mut errors = Vec::new();

    for (name, variants) in &doc.chunks {
        for chunk in variants {
            if chunk.is_media {
                continue;
            }
            let combined = chunk.combined_body();
            let refs = find_chunk_refs_aware(&combined, chunk.lang.as_deref());
            for r in refs {
                // Cross-doc refs are validated by the corpus resolver,
                // not against a single doc's chunk map; an escaped ref
                // is literal text and names nothing.
                if r.doc_uri.is_some() || r.escaped {
                    continue;
                }
                if !doc.chunks.contains_key(&r.name) {
                    errors.push(format!(
                        "chunk '{}' references undefined chunk '<<{}>>'",
                        name, r.name
                    ));
                }
            }
        }
    }

    // Check for cycles
    for name in doc.chunks.keys() {
        let Some(chunk) = doc.chunk(name) else { continue };
        if chunk.is_media {
            continue;
        }
        let mut visited = HashSet::new();
        if let Err(e) = expand_recursive(None, "", doc, name, None, &mut visited) {
            let msg = e.to_string();
            if msg.contains("cycle") {
                errors.push(msg);
            }
        }
    }

    Ok(errors)
}
```

## Tests

The tests cover the expansion shapes: simple substitution, indent
preservation across multi-line expansions, cycle detection,
undefined-ref reporting through `check_all_refs`, and the escape —
`<<!greet>>` tangles to the literal `<<greet>>` with its indent, even
when a chunk named `greet` exists, and `check_all_refs` does not
report an escaped name that names no chunk.

The fixture docs embed triple-backtick markdown fences inside a
Rust raw string. Language-aware ref extraction (see
[`chunk-refs.md`](chunk-refs.md)) skips the `<<body>>` /
`<<inner>>` / `<<b>>` etc. tokens that appear inside the
fixture's raw string, which is what makes this doc tangle
without the resolver chasing its own test data.

`````rust {#tests}
#[cfg(test)]
mod tests {
    use super::*;
    use crate::parser::parse_document;

    #[test]
    fn expand_simple() {
        let doc = r#"
```rust {#body}
println!("hello");
```

```rust {#main}
fn main() {
    <<body>>
}
```
"#;
        let parsed = parse_document(doc).unwrap();
        let expanded = expand_chunk(&parsed, "main").unwrap();
        assert!(expanded.contains("println!(\"hello\");"));
        assert!(expanded.contains("fn main()"));
    }

    #[test]
    fn expand_preserves_indent() {
        let doc = r#"
```rust {#inner}
line1();
line2();
```

```rust {#outer}
fn f() {
    <<inner>>
}
```
"#;
        let parsed = parse_document(doc).unwrap();
        let expanded = expand_chunk(&parsed, "outer").unwrap();
        assert!(expanded.contains("    line1();"));
        assert!(expanded.contains("    line2();"));
    }

    #[test]
    fn detect_cycle() {
        let doc = r#"
```rust {#a}
<<b>>
```

```rust {#b}
<<a>>
```
"#;
        let parsed = parse_document(doc).unwrap();
        assert!(expand_chunk(&parsed, "a").is_err());
    }

    #[test]
    fn undefined_ref_detected() {
        let doc = r#"
```rust {#main}
<<nonexistent>>
```
"#;
        let parsed = parse_document(doc).unwrap();
        let errors = check_all_refs(&parsed).unwrap();
        assert_eq!(errors.len(), 1);
        assert!(errors[0].contains("nonexistent"));
    }

    #[test]
    fn escaped_ref_tangles_to_the_literal_line() {
        let doc = r#"
```rust {#greet}
fn greet() {}
```

```markdown {#readme}
Where the listing says so, put this line:
    <<!greet>>
and the tangler splices `greet` in there.
<<!nonexistent>>
```
"#;
        let parsed = parse_document(doc).unwrap();
        let result = expand_chunk(&parsed, "readme").unwrap();
        assert_eq!(
            result,
            "Where the listing says so, put this line:\n    <<greet>>\nand the tangler splices `greet` in there.\n<<nonexistent>>"
        );
        assert!(check_all_refs(&parsed).unwrap().is_empty());
    }
}
`````

## Composing the module

```rust {#root}
<<imports>>

<<expand-chunk-fn>>

<<expand-recursive-fn>>

<<check-all-refs-fn>>

<<tests>>
```
