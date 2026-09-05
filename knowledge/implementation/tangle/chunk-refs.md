---
x0k:
  format: folio/v1
  id: x0k:implementation/tangle/chunk-refs
  type: implementation
  status: draft
  summary: Why the line-based reference scan is not enough for a substrate that quotes itself, and the tree-sitter pass that drops matches sitting inside string literals, raw strings and comments.
  concerns: [tangle, literate, chunk-refs, tree-sitter, lang-aware]
  tangle:
    crate: x0k-tangle
    root: src/chunk_refs.rs
  edges:
    cites:
      - x0k:implementation/tangle/protocol
      - x0k:implementation/tangle/chunk
      - x0k:implementation/tangle/resolution
    presupposes:
      - x0k:wiki/literate-programming
---
# Language-aware chunk reference extraction

`chunk.rs` defines a line-based `find_chunk_refs` that hunts for
lines whose trimmed content is exactly `<<name>>`. The rule is
deliberately structural: chunk refs are not meant to be inlined
inside expressions or comments, so a line like `// <<x>>` or
`let s = "<<x>>";` is correctly *not* a ref under that scan.

But the line-based rule is not enough for the literate substrate
explaining itself. Some of `x0k-tangle`'s own modules contain
test fixtures and string literals that legitimately have
`<<imports>>` as data — e.g. a Rust string `r#"<<imports>>"#`
sitting alone on its own line. Under the naive rule that line
matches the ref pattern and the resolver tries to expand
`<<imports>>` as a literate chunk.

This module adds a wrapper, `find_chunk_refs_aware`, that uses
tree-sitter to identify spans inside string literals, raw strings,
and comments, then filters out any naive match whose source line
starts inside an excluded span. When the chunk's `lang` is one we
have a grammar for (today: Rust), the aware scan applies; for
anything else we fall back to the line-based rule (better to
over-include and surface an "undefined chunk" error than silently
drop a real ref).

## Imports

We re-export the `ChunkRef` type from `chunk.rs` and call its
`find_chunk_refs` as the naive seed; everything else here is
tree-sitter wiring.

```rust {#imports}
use crate::chunk::{find_chunk_refs, ChunkRef};
use tree_sitter::{Node, Parser};
```

## The public function

The wrapper has a simple contract: pass the chunk body and its
declared language; get back the same `Vec<ChunkRef>` the naive
function returns, minus any ref whose line lies inside a
language-construct we know not to recurse into.

The fall-back semantics are deliberately conservative:

- `lang = None` → no grammar → return the naive scan verbatim.
- `lang` is unknown to us → same; return the naive scan.
- tree-sitter fails to parse → return the naive scan.
- tree-sitter succeeds but finds no exclude spans → naive scan.

Only when we have a parsed tree AND it identifies at least one
excluded span do we filter. Inside that filter, we check whether
the ref's source line starts inside any recorded byte span; if so,
drop it. Otherwise keep it.

The filter does not distinguish an escaped ref (`<<!name>>`, see
[`chunk.md`](chunk.md)) from a plain one: inside a string literal or
comment both are dropped, so both stay verbatim in the output. That
uniformity is what lets the resolver's and weaver's own tests carry
`<<!…>>` inside fixture strings — the escape is only ever interpreted
where a plain ref would have been.

```rust {#find-chunk-refs-aware-fn}
pub fn find_chunk_refs_aware(content: &str, lang: Option<&str>) -> Vec<ChunkRef> {
    let raw = find_chunk_refs(content);
    let Some(lang_name) = lang else {
        return raw;
    };
    let Some(exclude_spans) = collect_exclude_spans(content, lang_name) else {
        return raw;
    };
    if exclude_spans.is_empty() {
        return raw;
    }
    let line_offsets = line_start_offsets(content);
    raw.into_iter()
        .filter(|r| {
            let Some(line_start) = line_offsets.get(r.line_in_body) else {
                return true;
            };
            let trimmed_offset = *line_start
                + content[*line_start..]
                    .lines()
                    .next()
                    .map(|line| line.len() - line.trim_start().len())
                    .unwrap_or(0);
            !exclude_spans
                .iter()
                .any(|&(start, end)| trimmed_offset >= start && trimmed_offset < end)
        })
        .collect()
}
```

`trimmed_offset` is the byte position of the first non-whitespace
character on the ref's line. If that byte falls inside a tracked
exclude span, the `<<name>>` token is provably inside a comment or
string. We use the byte at the first non-whitespace char (not the
line start) so that an indented `<<x>>` inside a multi-line
raw string is still classified as inside-the-raw-string.

## Line-offset table

A tiny helper: precompute the byte offsets where each line starts,
so the filter above can map `line_in_body` → byte position in O(1).

```rust {#line-start-offsets-fn}
fn line_start_offsets(content: &str) -> Vec<usize> {
    let mut offsets = vec![0usize];
    for (i, c) in content.char_indices() {
        if c == '\n' {
            offsets.push(i + 1);
        }
    }
    offsets
}
```

## Tree-sitter span collection

`collect_exclude_spans` picks the language, parses, and walks the
resulting tree collecting the byte ranges of any node whose `kind()`
matches our exclude list. Returns `None` when we don't have a
grammar for the language, signalling "fall back to naive."

The `exclude_kinds` list is curated for Rust today:
- `string_literal` — `"foo"`, `b"foo"`
- `raw_string_literal` — `r"foo"`, `r#"foo"#`, `r##"foo"##`, etc.
- `line_comment` — `// ...`
- `block_comment` — `/* ... */`

Adding support for TypeScript / JavaScript means: depend on
`tree-sitter-typescript` (or javascript), add a match arm, and
extend `exclude_kinds` with that grammar's node kinds (likely
`string`, `template_string`, `comment`). Deferred — the Rust
unblocker is what the substrate needs right now.

```rust {#collect-exclude-spans-fn}
fn collect_exclude_spans(content: &str, lang: &str) -> Option<Vec<(usize, usize)>> {
    let language = match lang {
        "rust" | "rs" => tree_sitter_rust::LANGUAGE,
        _ => return None,
    };
    let mut parser = Parser::new();
    parser.set_language(&language.into()).ok()?;
    let tree = parser.parse(content, None)?;
    let mut spans = Vec::new();
    let exclude_kinds = ["string_literal", "raw_string_literal", "line_comment", "block_comment"];
    walk_collect(tree.root_node(), &exclude_kinds, &mut spans);
    Some(spans)
}
```

## Tree walk

When a node matches our exclude list we record its byte span and
stop descending — we don't care about nested string interpolations
or comment-inside-comment, and we don't want to record overlapping
spans. For non-excluded nodes we recurse.

```rust {#walk-collect-fn}
fn walk_collect(node: Node, exclude_kinds: &[&str], spans: &mut Vec<(usize, usize)>) {
    if exclude_kinds.contains(&node.kind()) {
        spans.push((node.start_byte(), node.end_byte()));
        return;
    }
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        walk_collect(child, exclude_kinds, spans);
    }
}
```

## Tests

Coverage hits the Rust edge cases:

- Raw strings (including `r#"..."#` with embedded `#`).
- Line comments and block comments.
- Multi-line raw strings — the span runs across many lines and
  every interior `<<x>>` must be filtered.
- Fall-back when `lang = None` or unknown.
- The `"rs"` alias.
- Malformed code: tree-sitter recovers with an error tree but does
  not crash; we don't make a strict assertion on count here.
- An escaped ref inside a string is dropped like a plain one; outside,
  it is reported with `escaped` set.

```rust {#tests}
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn skips_refs_in_rust_raw_strings() {
        let content = r##"fn demo() {
    let _x = r#"
    <<imports>>
    "#;
    // <<other>> in line comment
    /* <<another>> in block */
    let _y = "<<not_a_ref>>";
    <<actually_a_ref>>
}
"##;
        let refs = find_chunk_refs_aware(content, Some("rust"));
        assert_eq!(refs.len(), 1, "got {:?}", refs);
        assert_eq!(refs[0].name, "actually_a_ref");
    }

    #[test]
    fn escaped_refs_are_filtered_like_plain_ones() {
        let content = "fn f() {\n    let _s = \"\n    <<!in_string>>\n    \";\n    <<!kept>>\n}\n";
        let refs = find_chunk_refs_aware(content, Some("rust"));
        assert_eq!(refs.len(), 1, "got {:?}", refs);
        assert_eq!(refs[0].name, "kept");
        assert!(refs[0].escaped);
    }

    #[test]
    fn skips_refs_inside_block_comment() {
        let content = "fn f() {\n    /*\n    <<inside_block>>\n    */\n    <<real>>\n}\n";
        let refs = find_chunk_refs_aware(content, Some("rust"));
        assert_eq!(refs.len(), 1, "got {:?}", refs);
        assert_eq!(refs[0].name, "real");
    }

    #[test]
    fn skips_refs_inside_line_comment() {
        let content = "// <<commented>>\n<<real>>\n";
        let refs = find_chunk_refs_aware(content, Some("rust"));
        // The current line-based parser already rejects `// <<x>>` since
        // the trimmed line starts with `//`. But a stripped variant
        // could appear inside a `/* */` span at column 0; this guards
        // that we don't double-include `<<real>>`.
        assert_eq!(refs.len(), 1);
        assert_eq!(refs[0].name, "real");
    }

    #[test]
    fn handles_nested_raw_string_delimiters() {
        let content = r##"fn f() {
    let _x = r#"hash <<X>> in #-delim raw"#;
}
"##;
        let refs = find_chunk_refs_aware(content, Some("rust"));
        assert_eq!(refs.len(), 0);
    }

    #[test]
    fn raw_string_spanning_multiple_lines() {
        let content = "fn f() {\n    let _x = r#\"\n<<imports>>\n    <<helpers>>\n\"#;\n}\n";
        let refs = find_chunk_refs_aware(content, Some("rust"));
        assert_eq!(refs.len(), 0, "got {:?}", refs);
    }

    #[test]
    fn falls_back_to_naive_for_unknown_lang() {
        let content = "let x = '<<imports>>';\n<<other>>\n";
        let refs = find_chunk_refs_aware(content, Some("python"));
        // No grammar → no filtering. The standalone <<other>> line is a
        // ref under the line-based rule; the inline `'<<imports>>'` is
        // not (text follows on the line).
        assert_eq!(refs.len(), 1);
        assert_eq!(refs[0].name, "other");
    }

    #[test]
    fn falls_back_to_naive_for_none_lang() {
        let content = "<<one>>\n  <<two>>\n";
        let refs = find_chunk_refs_aware(content, None);
        assert_eq!(refs.len(), 2);
        assert_eq!(refs[0].name, "one");
        assert_eq!(refs[1].name, "two");
    }

    #[test]
    fn rust_lang_with_no_strings_or_comments_matches_naive() {
        let content = "fn main() {\n    <<imports>>\n    <<body>>\n}\n";
        let refs = find_chunk_refs_aware(content, Some("rust"));
        assert_eq!(refs.len(), 2);
        assert_eq!(refs[0].name, "imports");
        assert_eq!(refs[1].name, "body");
    }

    #[test]
    fn rs_alias_recognized() {
        let content = "fn f() { let _x = \"<<not>>\"; }\n<<yes>>\n";
        let refs = find_chunk_refs_aware(content, Some("rs"));
        // The first << is inside a string but on the same line as `fn f()`,
        // so the line-based scan rejects it anyway. The second matches.
        assert_eq!(refs.len(), 1);
        assert_eq!(refs[0].name, "yes");
    }

    #[test]
    fn syntax_error_falls_back_conservatively() {
        // Malformed Rust: unmatched brace. tree-sitter parses with errors
        // but still produces a tree; the exclude spans we recover should
        // be enough to filter what they cover, and ambiguous matches
        // pass through (we err toward over-including).
        let content = "fn broken( {\n    <<imports>>\n";
        let refs = find_chunk_refs_aware(content, Some("rust"));
        // Whether tree-sitter recovers `<<imports>>` as a string or not,
        // we should not crash. We accept either 0 or 1 refs.
        assert!(refs.len() <= 1);
    }
}
```

## Composing the module

```rust {#root}
<<imports>>

<<find-chunk-refs-aware-fn>>

<<line-start-offsets-fn>>

<<collect-exclude-spans-fn>>

<<walk-collect-fn>>

<<tests>>
```
