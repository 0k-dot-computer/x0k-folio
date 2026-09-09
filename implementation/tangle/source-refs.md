---
x0k:
  format: folio/v1
  id: x0k:implementation/tangle/source-refs
  type: implementation
  status: draft
  summary: Tree-sitter symbol extraction for a `from=` chunk — the grammar of the language the chunk declares decides where a symbol's body begins and ends, with no regex and no brace counting — and the symbol listing the doc browser reads.
  concerns:
  - tangle
  - literate
  - tree-sitter
  - symbol-extraction
  - lang-aware
  tangle:
    crate: crates/x0k-tangle
    root: src/source_ref.rs
  edges:
    cites:
    - x0k:implementation/tangle/protocol
    - x0k:implementation/tangle/chunk
---
# Symbol extraction for `from=` chunks

A `from=` chunk is the inverse of a tangle output: instead of the
document owning code that becomes a source file, the [literate
document](../../background/literate-programming.md "x0k:wiki/literate-programming")
*references* code that already lives in a source file. The fence
declares "fill this block with the body of `Type::method` from
`crate/src/file.rs`," and on sync, the toolchain extracts the
symbol's body and writes it into the chunk.

This module does the extraction. It parses the source with the
tree-sitter grammar for the language the chunk declares — Rust,
TypeScript (which also parses plain JavaScript), TSX, Python or
Julia — walks the tree looking for a symbol matching the declared
path, and returns the byte range that constitutes the symbol's body.
No regex, no brace-counting — the AST is the source of truth for
symbol boundaries.

A chunk in a language with no walker here is refused by name before
anything is parsed, and that refusal is why the language is an
argument at all. Read through the Rust grammar, a JavaScript file
parses into a tree with none of the reader's symbols in it, and the
honest report of that is "extraction does not speak this language" —
not "symbol `createHorizonRemap` not found", which reads as *you typed
the name wrong* and sends a reader off to check their own exports.

The same honesty governs the other end of the walk. A path that
matches two definitions is not a tie to break: `Debug` and `Display`
both define `fn fmt` on the same type, four lines apart, and a chunk
that says `Pairs::fmt` has not said which one. Picking the first
attaches prose to a body its author never read, and exits zero —
the one failure this whole substrate exists to prevent. So an
ambiguous path is refused, the candidates are printed with the impl
header each sits under, and Rust's own qualified spelling
(`<Pairs<'_, R> as fmt::Display>::fmt`) selects one.

The same machinery powers `list_symbols_in`, which the doc-browser
uses to enumerate symbols in a file for navigation panels.

## Imports

<a name="chunk-imports"></a><sub>[`src/source_ref.rs`](../../crates/x0k-tangle/src/source_ref.rs) · `#imports`</sub>

```rust {#imports}
use anyhow::{bail, Result};
use tree_sitter::{Node, Parser, Tree};
use x0k_syntax::Language as FenceLanguage;
```

## The result shape

`SymbolSpan` is what the extractor returns. It carries the symbol's
name (the leaf identifier — `arc`, `Canvas2DState`, `helper`), the
extracted body text, line bounds (1-indexed for editor-friendly
diagnostics), and byte bounds (for precise stitch-back).

<a name="chunk-symbol-span"></a><sub>[`src/source_ref.rs`](../../crates/x0k-tangle/src/source_ref.rs) · `#symbol-span`</sub>

```rust {#symbol-span}
#[derive(Debug, Clone)]
pub struct SymbolSpan {
    pub name: String,
    pub body: String,
    pub start_line: usize,
    pub end_line: usize,
    pub byte_start: usize,
    pub byte_end: usize,
}
```

## Which grammar

Extraction needs two things from a language: a tree-sitter grammar,
and the vocabulary of node kinds its symbols wear. The grammar comes
from the crate that already links it; the vocabulary is the walker
further down.

The fence tags — `rs`, `ts`, `js`, `jsx`, `py` — are not ours to
redefine. `x0k-syntax` owns that mapping for the whole toolchain (it
is what the weaver highlights by), so `for_lang` resolves through it
and then narrows, because `x0k-syntax` also knows JSON, which has a
grammar but no symbol walker here. Julia is the exception, and an
honest one: `x0k-syntax` has no Julia grammar because nothing
highlights Julia yet, so the tag is recognised here, in the one
function below, rather than by widening a highlighting crate to
carry a language it does not highlight.

The grammars themselves we take from `tree-sitter-rust`,
`tree-sitter-typescript`, `tree-sitter-python` and
`tree-sitter-julia` directly, as this crate already did for Rust:
`x0k-syntax` hands out classified tokens, not `tree_sitter::Language`
handles, and widening a highlighting crate's API to serve a caller
that wants a parser would be the worse seam.

<a name="chunk-symbol-language"></a><sub>[`src/source_ref.rs`](../../crates/x0k-tangle/src/source_ref.rs) · `#symbol-language`</sub>

```rust {#symbol-language}
/// The languages symbol extraction can walk.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SymbolLanguage {
    Rust,
    /// TypeScript, whose grammar also parses plain JavaScript.
    TypeScript,
    /// TSX, which additionally parses JSX.
    Tsx,
    Python,
    Julia,
}

/// The set `for_lang` names when it refuses, spelled as fence tags.
const SUPPORTED_LANGS: &str = "rust, typescript, javascript, tsx, python, julia";
```

`for_lang` is the first thing a caller does with a `from=` chunk, and
the only place the limit is stated. It takes the fence tag as an
`Option` because a chunk may carry none, and that is refused too: a
`symbol=` with no language is a document that has not said which
grammar to read its source with.

<a name="chunk-for-lang"></a><sub>[`src/source_ref.rs`](../../crates/x0k-tangle/src/source_ref.rs) · `#for-lang`</sub>

```rust {#for-lang}
impl SymbolLanguage {
    /// Resolve a chunk's declared fence language, or refuse by name.
    pub fn for_lang(lang: Option<&str>) -> Result<Self> {
        let Some(tag) = lang else {
            bail!("symbol extraction needs a language on the fence (one of {SUPPORTED_LANGS})");
        };
        if is_julia_tag(tag) {
            return Ok(Self::Julia);
        }
        match FenceLanguage::from_str(tag) {
            Some(FenceLanguage::Rust) => Ok(Self::Rust),
            Some(FenceLanguage::Typescript) => Ok(Self::TypeScript),
            Some(FenceLanguage::Tsx) => Ok(Self::Tsx),
            Some(FenceLanguage::Python) => Ok(Self::Python),
            _ => bail!("symbol extraction supports {SUPPORTED_LANGS} (this chunk is `{tag}`)"),
        }
    }

    fn grammar(self) -> tree_sitter::Language {
        match self {
            Self::Rust => tree_sitter_rust::LANGUAGE.into(),
            Self::TypeScript => tree_sitter_typescript::LANGUAGE_TYPESCRIPT.into(),
            Self::Tsx => tree_sitter_typescript::LANGUAGE_TSX.into(),
            Self::Python => tree_sitter_python::LANGUAGE.into(),
            Self::Julia => tree_sitter_julia::LANGUAGE.into(),
        }
    }
}
```

The Julia tag is spelled once, here, and read by the chunk-reference
scanner too — the other half of the toolchain that dispatches on a
chunk's declared language.

<a name="chunk-julia-tag"></a><sub>[`src/source_ref.rs`](../../crates/x0k-tangle/src/source_ref.rs) · `#julia-tag`</sub>

```rust {#julia-tag}
/// The one place the Julia fence tag is spelled. `x0k-syntax`'s vocabulary
/// has no Julia — nothing highlights it — so the two passes that walk a
/// Julia tree recognise the tag themselves.
pub(crate) fn is_julia_tag(tag: &str) -> bool {
    matches!(tag.to_ascii_lowercase().as_str(), "julia" | "jl")
}
```

## The single-symbol extractor

`extract_symbol_in` parses the source, walks the tree looking for a
symbol matching `symbol_path`, and returns one `SymbolSpan`. The path
resolution is multi-step:

- For a top-level symbol (`helper`), the query is `["helper"]` and
  the walker matches at depth 0.
- For an impl method (`Canvas2DState::arc`), the walker enters the
  matching impl block and continues descent.
- For a module-qualified path (`tests::test_it`), the walker enters
  the module body.

Each language brings its own walker, and each walker returns
*candidates* rather than an answer, because whether the path was
unambiguous is not a question a walker can answer from inside one
scope.

<a name="chunk-extract-symbol"></a><sub>[`src/source_ref.rs`](../../crates/x0k-tangle/src/source_ref.rs) · `#extract-symbol`</sub>

```rust {#extract-symbol}
pub fn extract_symbol_in(
    source: &str,
    symbol_path: &str,
    lang: SymbolLanguage,
) -> Result<SymbolSpan> {
    let tree = parse(source, lang)?;
    let query = parse_symbol_query(symbol_path, lang);
    let root = tree.root_node();

    let mut matches = Vec::new();
    match lang {
        SymbolLanguage::Rust => {
            collect_matching_symbols(root, source, &query, 0, None, &mut matches)
        }
        SymbolLanguage::TypeScript | SymbolLanguage::Tsx => {
            collect_matching_ts_symbols(root, source, &query, 0, None, &mut matches)
        }
        SymbolLanguage::Python => {
            collect_matching_py_symbols(root, source, &query, 0, None, &mut matches)
        }
        SymbolLanguage::Julia => {
            collect_matching_jl_symbols(root, source, &query, 0, None, &mut matches)
        }
    }
    pick_match(matches, symbol_path)
}
```

## What a `symbol=` says

A query is the path segments to descend, plus — for a Rust
`<Type as Trait>::method` — the trait the impl block must implement.
The trait is not a segment: it does not name a scope to enter, it
narrows which of several impl blocks on the same type counts.

<a name="chunk-symbol-query"></a><sub>[`src/source_ref.rs`](../../crates/x0k-tangle/src/source_ref.rs) · `#symbol-query`</sub>

```rust {#symbol-query}
/// A parsed `symbol=`: the segments to descend, and the trait a Rust
/// `<Type as Trait>::method` path pins the impl block to.
struct SymbolQuery<'p> {
    parts: Vec<&'p str>,
    trait_bound: Option<&'p str>,
}

fn parse_symbol_query(symbol_path: &str, lang: SymbolLanguage) -> SymbolQuery<'_> {
    if lang == SymbolLanguage::Rust {
        if let Some(query) = parse_qualified_path(symbol_path) {
            return query;
        }
    }
    SymbolQuery {
        parts: split_symbol_path(symbol_path, lang),
        trait_bound: None,
    }
}
```

The path separator is the one a reader of that language already
writes. Rust splits on `::` alone, because a `.` never separates Rust
items. TypeScript and Python take either, so a document can spell
every `symbol=` the same way whatever the chunk's language. Julia
splits on `.` only — `::` is a type annotation there, and it appears
inside the very signatures a Julia `symbol=` may name.

<a name="chunk-split-symbol-path"></a><sub>[`src/source_ref.rs`](../../crates/x0k-tangle/src/source_ref.rs) · `#split-symbol-path`</sub>

```rust {#split-symbol-path}
fn split_symbol_path(symbol_path: &str, lang: SymbolLanguage) -> Vec<&str> {
    match lang {
        SymbolLanguage::Rust => symbol_path.split("::").collect(),
        SymbolLanguage::Julia => split_julia_path(symbol_path),
        _ => symbol_path
            .split(['.', ':'])
            .filter(|part| !part.is_empty())
            .collect(),
    }
}
```

Julia's own separator needs one more precaution. A `symbol=` there may
name a method by its signature — `quadgk(f, a, b)` — and a signature
holds dots of its own, in a default like `atol=1e-8` or a qualified
type. So the split only cuts at bracket depth zero, and
`QuadGK.quadgk(f, a, b)` comes apart into the module and the method
rather than into four pieces.

<a name="chunk-split-julia-path"></a><sub>[`src/source_ref.rs`](../../crates/x0k-tangle/src/source_ref.rs) · `#split-julia-path`</sub>

```rust {#split-julia-path}
fn split_julia_path(path: &str) -> Vec<&str> {
    let mut parts = Vec::new();
    let mut depth = 0usize;
    let mut start = 0usize;
    for (i, c) in path.char_indices() {
        match c {
            '(' | '{' | '[' => depth += 1,
            ')' | '}' | ']' => depth = depth.saturating_sub(1),
            '.' if depth == 0 => {
                parts.push(&path[start..i]);
                start = i + c.len_utf8();
            }
            _ => {}
        }
    }
    parts.push(&path[start..]);
    parts.into_iter().filter(|part| !part.is_empty()).collect()
}
```

The qualified form is Rust's own syntax for the same disambiguation,
which is why it is the one we read: an author who has hit two `fmt`s
already knows how the language spells the answer.

<a name="chunk-parse-qualified-path"></a><sub>[`src/source_ref.rs`](../../crates/x0k-tangle/src/source_ref.rs) · `#parse-qualified-path`</sub>

```rust {#parse-qualified-path}
/// `<Pairs<'_, R> as fmt::Display>::fmt` — the type, the trait that
/// selects one impl of it, and the path below.
fn parse_qualified_path(path: &str) -> Option<SymbolQuery<'_>> {
    let inner = path.strip_prefix('<')?;
    let close = closing_angle(inner)?;
    let (qualifier, rest) = inner.split_at(close);
    let (ty, bound) = split_at_as(qualifier)?;
    let mut parts = vec![ty];
    parts.extend(rest.strip_prefix(">::")?.split("::"));
    Some(SymbolQuery {
        parts,
        trait_bound: Some(bound),
    })
}
```

Both halves of that parse count angle-bracket depth, because the type
carries its own: `Pairs<'_, R>` closes a `>` that is not the
qualifier's. Every byte examined is ASCII, so byte indices into the
path are char boundaries.

<a name="chunk-angle-scanning"></a><sub>[`src/source_ref.rs`](../../crates/x0k-tangle/src/source_ref.rs) · `#angle-scanning`</sub>

```rust {#angle-scanning}
/// The `>` that closes the qualifier, skipping the ones inside it.
fn closing_angle(s: &str) -> Option<usize> {
    let mut depth = 0usize;
    for (i, c) in s.char_indices() {
        match c {
            '<' => depth += 1,
            '>' if depth == 0 => return Some(i),
            '>' => depth -= 1,
            _ => {}
        }
    }
    None
}

fn split_at_as(qualifier: &str) -> Option<(&str, &str)> {
    let bytes = qualifier.as_bytes();
    let mut depth = 0usize;
    for i in 0..bytes.len() {
        match bytes[i] {
            b'<' => depth += 1,
            b'>' => depth = depth.saturating_sub(1),
            b' ' if depth == 0 && qualifier[i..].starts_with(" as ") => {
                return Some((qualifier[..i].trim(), qualifier[i + 4..].trim()));
            }
            _ => {}
        }
    }
    None
}
```

## Choosing among candidates

A candidate is a match plus everything it takes to tell it from
another match of the same name: where it sits, and the spelling that
would select it alone. `MatchKind` records one weaker reading — a
bare type name also names its `impl` blocks, and that reading loses
to a declared item of that name rather than competing with it.

<a name="chunk-candidate"></a><sub>[`src/source_ref.rs`](../../crates/x0k-tangle/src/source_ref.rs) · `#candidate`</sub>

```rust {#candidate}
struct Candidate {
    span: SymbolSpan,
    /// The enclosing impl / class / module, spelled as the source spells it.
    scope: Option<String>,
    /// A `symbol=` naming this candidate and no other, where the language
    /// has such a spelling.
    select: Option<String>,
    kind: MatchKind,
}

#[derive(PartialEq, Eq, Clone, Copy)]
enum MatchKind {
    /// A declared item: `fn`, `struct`, `def`, `struct … end`.
    Item,
    /// The `impl` block a bare type name also names.
    ImplBlock,
}
```

`pick_match` is where the substrate's central claim is kept or lost.
Zero matches is the reader's typo; one is the answer; more than one
is a question only the author can settle, so it is asked rather than
guessed.

<a name="chunk-pick-match"></a><sub>[`src/source_ref.rs`](../../crates/x0k-tangle/src/source_ref.rs) · `#pick-match`</sub>

```rust {#pick-match}
fn pick_match(mut matches: Vec<Candidate>, symbol_path: &str) -> Result<SymbolSpan> {
    if matches.iter().any(|c| c.kind == MatchKind::Item) {
        matches.retain(|c| c.kind == MatchKind::Item);
    }
    match matches.len() {
        0 => bail!("symbol '{symbol_path}' not found"),
        1 => Ok(matches.remove(0).span),
        _ => bail!("{}", ambiguity_report(&matches, symbol_path)),
    }
}
```

The report is written for someone who cannot see the file: the line,
the scope as the source spells it, and — when the language has a
spelling that selects one — the exact `symbol=` to paste.

<a name="chunk-ambiguity-report"></a><sub>[`src/source_ref.rs`](../../crates/x0k-tangle/src/source_ref.rs) · `#ambiguity-report`</sub>

```rust {#ambiguity-report}
fn ambiguity_report(matches: &[Candidate], symbol_path: &str) -> String {
    let mut report = format!(
        "symbol '{symbol_path}' is ambiguous — {} definitions match:",
        matches.len()
    );
    for m in matches {
        report.push_str(&format!("\n  line {}", m.span.start_line));
        if let Some(scope) = &m.scope {
            report.push_str(&format!(" in `{scope}`"));
        }
        if let Some(select) = &m.select {
            report.push_str(&format!(" — select it with symbol=\"{select}\""));
        }
    }
    if matches.iter().all(|m| m.select.is_none()) {
        report.push_str("\n  no spelling names one of these alone; reference the enclosing item");
    }
    report
}
```

`matched` builds the common case: a candidate that inherits its scope
and its selector from whatever the walk descended through.

<a name="chunk-matched"></a><sub>[`src/source_ref.rs`](../../crates/x0k-tangle/src/source_ref.rs) · `#matched`</sub>

```rust {#matched}
fn matched(span: SymbolSpan, enclosing: Option<&Enclosing>, kind: MatchKind) -> Candidate {
    let select = enclosing
        .and_then(|e| e.qualifier.as_deref())
        .map(|qualifier| format!("{qualifier}::{}", span.name));
    Candidate {
        scope: enclosing.and_then(|e| e.header.clone()),
        select,
        span,
        kind,
    }
}

/// What a descent step knows about the scope it entered: how to name it in
/// a report, and how a `symbol=` would pin it.
#[derive(Default, Clone)]
struct Enclosing {
    header: Option<String>,
    qualifier: Option<String>,
}
```

## Listing all symbols

`list_symbols_in` walks the entire tree collecting every top-level
item, qualified by its containing impl / module / class. The
doc-browser's symbol navigation panel consumes this; literate authors
can also use it as a sanity check ("did the tangle hit the symbols I
expected?").

<a name="chunk-list-symbols"></a><sub>[`src/source_ref.rs`](../../crates/x0k-tangle/src/source_ref.rs) · `#list-symbols`</sub>

```rust {#list-symbols}
pub fn list_symbols_in(source: &str, lang: SymbolLanguage) -> Result<Vec<SymbolSpan>> {
    let tree = parse(source, lang)?;
    let root = tree.root_node();
    let mut symbols = Vec::new();
    match lang {
        SymbolLanguage::Rust => collect_all_symbols(root, source, &[], &mut symbols),
        SymbolLanguage::TypeScript | SymbolLanguage::Tsx => {
            collect_all_ts_symbols(root, source, &[], &mut symbols)
        }
        SymbolLanguage::Python => collect_all_py_symbols(root, source, &[], &mut symbols),
        SymbolLanguage::Julia => collect_all_jl_symbols(root, source, &[], &mut symbols),
    }
    Ok(symbols)
}
```

Both entry points keep a Rust-pinned shorthand, for a caller that
holds source and has no declared language to consult. The document
index is the one such caller: its span map decorates a doc-browser
panel, and a miss there costs a fold-out, not a sync. Anything reading
a `from=` chunk knows the language and passes it.

<a name="chunk-rust-shims"></a><sub>[`src/source_ref.rs`](../../crates/x0k-tangle/src/source_ref.rs) · `#rust-shims`</sub>

```rust {#rust-shims}
/// Rust-pinned [`extract_symbol_in`]: for a caller holding source with no
/// declared language to consult. Anything reading a `from=` chunk has the
/// chunk's language and passes it.
pub fn extract_symbol(source: &str, symbol_path: &str) -> Result<SymbolSpan> {
    extract_symbol_in(source, symbol_path, SymbolLanguage::Rust)
}

/// Rust-pinned [`list_symbols_in`], paired with [`extract_symbol`].
pub fn list_symbols(source: &str) -> Result<Vec<SymbolSpan>> {
    list_symbols_in(source, SymbolLanguage::Rust)
}
```

## Tree-walking helpers

`parse` is the one place a parser is configured, so a language reaches
tree-sitter exactly once and the two entry points cannot drift.

<a name="chunk-parse-source"></a><sub>[`src/source_ref.rs`](../../crates/x0k-tangle/src/source_ref.rs) · `#parse-source`</sub>

```rust {#parse-source}
fn parse(source: &str, lang: SymbolLanguage) -> Result<Tree> {
    let mut parser = Parser::new();
    parser
        .set_language(&lang.grammar())
        .map_err(|e| anyhow::anyhow!("tree-sitter language error: {}", e))?;
    parser
        .parse(source, None)
        .ok_or_else(|| anyhow::anyhow!("tree-sitter parse failed"))
}
```

`node_text` slices the source by the node's byte range — tree-sitter
gives us byte positions, the source string is in-memory, so the body
extraction is one slice. Because it is a byte slice at node
boundaries, a `δλ` or an `x̄` comes out exactly as it went in.

<a name="chunk-tree-helpers"></a><sub>[`src/source_ref.rs`](../../crates/x0k-tangle/src/source_ref.rs) · `#tree-helpers`</sub>

```rust {#tree-helpers}
fn node_text<'a>(node: Node, source: &'a str) -> &'a str {
    &source[node.byte_range()]
}

fn named_child_text(node: Node, source: &str) -> Option<String> {
    node.child_by_field_name("name")
        .map(|child| node_text(child, source).to_string())
        .or_else(|| {
            node.children(&mut node.walk())
                .find(|child| {
                    child.kind() == "identifier" || child.kind() == "type_identifier"
                })
                .map(|child| node_text(child, source).to_string())
        })
}

fn collapse_ws(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}
```

`named_child_text` tries the canonical `name` field first, then falls
back to any `identifier` / `type_identifier` child — different
tree-sitter node kinds expose their name differently, and this covers
both styles.

## Where a symbol really starts

A tree-sitter item node does not contain everything the reader wrote
above it. In `tree-sitter-rust`, attributes and doc comments are
*preceding siblings*: a span that begins at the `struct_item` returns
a struct with no `#[derive(Debug)]` and no `///` line. That is not
merely lossy. On the graduation path this substrate promises — drop
`from=`, add `tangle:`, and the document now owns the file — a
`#[derive(Clone, Copy)]` lost off a public type still compiles, and
silently removes trait impls from every crate downstream.

So a span starts at the earliest preceding sibling that belongs to
the item. `Leading` names what "belongs" means per language, because
the answer differs: Python's decorators are already inside the
item's own node, and TypeScript's `export` wrapper likewise.

<a name="chunk-leading"></a><sub>[`src/source_ref.rs`](../../crates/x0k-tangle/src/source_ref.rs) · `#leading`</sub>

```rust {#leading}
/// What precedes an item and belongs to it.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Leading {
    /// Nothing to attach: the item node already contains everything a
    /// reader wrote — a Python `decorated_definition`, a TS `export`.
    None,
    /// `#[derive(…)]` attributes and `///` outer doc comments.
    RustAttrs,
    /// The string literal Julia binds to the definition beneath it.
    JuliaDocstring,
}
```

The attachment rule is the one the compiler reads by: the right kind
of node, and no blank line between. The blank line is what keeps a
file-header `//!` or an unrelated `// note` out — and it has to count
the newline a `line_comment` node carries inside itself, since that
grammar ends the comment node after its own line break.

<a name="chunk-span-start"></a><sub>[`src/source_ref.rs`](../../crates/x0k-tangle/src/source_ref.rs) · `#span-start`</sub>

```rust {#span-start}
fn span_start<'t>(node: Node<'t>, source: &str, leading: Leading) -> Node<'t> {
    let mut start = node;
    while let Some(prev) = start.prev_sibling() {
        if !attaches(prev, start, source, leading) {
            break;
        }
        start = prev;
    }
    start
}

fn attaches(prev: Node, item: Node, source: &str, leading: Leading) -> bool {
    let gap = &source[prev.end_byte()..item.start_byte()];
    let breaks = gap.matches('\n').count() + usize::from(node_text(prev, source).ends_with('\n'));
    if breaks > 1 {
        return false;
    }
    match leading {
        Leading::None => false,
        // `outer` is the grammar's own field for `///` and `/** */`, which
        // is exactly the distinction wanted: `//!` documents the file.
        Leading::RustAttrs => {
            prev.kind() == "attribute_item"
                || (matches!(prev.kind(), "line_comment" | "block_comment")
                    && prev.child_by_field_name("outer").is_some())
        }
        Leading::JuliaDocstring => prev.kind() == "string_literal",
    }
}
```

`make_span` is then the slice from that start to the item's own end.

<a name="chunk-make-span"></a><sub>[`src/source_ref.rs`](../../crates/x0k-tangle/src/source_ref.rs) · `#make-span`</sub>

```rust {#make-span}
fn make_span(name: String, node: Node, source: &str, leading: Leading) -> SymbolSpan {
    let start = span_start(node, source, leading);
    SymbolSpan {
        name,
        body: source[start.start_byte()..node.end_byte()].to_string(),
        start_line: start.start_position().row + 1,
        end_line: node.end_position().row + 1,
        byte_start: start.start_byte(),
        byte_end: node.end_byte(),
    }
}
```

## Reading an impl block

`impl_type_name` extracts the implementor type from an impl block.
For `impl Trait for Foo` it returns `Foo` (the *receiver*, not the
trait being implemented). That's the right behavior for path
resolution — `Canvas2DState::arc` looks up the impl block that
provides methods *on* `Canvas2DState`, not the one that implements a
trait *for* it.

The text it returns is the header's, type parameters included:
`impl<R: RuleType> Op<R>` yields `Op<R>`. Comparing that literally
would make two thirds of a generic crate unreachable, and would make
the required spelling depend on which lifetime style that impl block
happened to use — `Pairs<'i, R>` in one file, `Pairs<'_, R>` in the
next. So the base name matches too, and the fully-spelled form keeps
working.

<a name="chunk-impl-reading"></a><sub>[`src/source_ref.rs`](../../crates/x0k-tangle/src/source_ref.rs) · `#impl-reading`</sub>

```rust {#impl-reading}
fn impl_type_name(node: Node, source: &str) -> Option<String> {
    node.child_by_field_name("type")
        .map(|t| node_text(t, source).to_string())
}

fn impl_trait_name(node: Node, source: &str) -> Option<String> {
    node.child_by_field_name("trait")
        .map(|t| node_text(t, source).to_string())
}

/// `Op<R>` → `Op`: the name the author writes in prose.
fn base_type_name(ty: &str) -> &str {
    ty.split('<').next().unwrap_or(ty).trim()
}
```

A `<Type as Trait>` qualifier matches the impl's trait by its last
path segment as well as in full, so `Display` and `fmt::Display` both
name the same impl. A trait spelled with generic arguments has to
match in full — splitting `From<a::B>` on `::` would name nothing.

<a name="chunk-trait-matching"></a><sub>[`src/source_ref.rs`](../../crates/x0k-tangle/src/source_ref.rs) · `#trait-matching`</sub>

```rust {#trait-matching}
fn trait_matches(bound: Option<&str>, actual: Option<&str>) -> bool {
    let Some(bound) = bound else {
        return true;
    };
    let Some(actual) = actual else {
        return false;
    };
    actual == bound || last_segment(actual) == last_segment(bound)
}

fn last_segment(path: &str) -> &str {
    if path.contains('<') {
        return path;
    }
    path.rsplit("::").next().unwrap_or(path)
}
```

The header as written is what a reader recognises in a report —
`impl<R: RuleType> fmt::Debug for Pairs<'_, R>`, not a reconstruction
of it — so it is sliced from the source up to the impl's body.

<a name="chunk-impl-header"></a><sub>[`src/source_ref.rs`](../../crates/x0k-tangle/src/source_ref.rs) · `#impl-header`</sub>

```rust {#impl-header}
fn impl_header(node: Node, source: &str) -> String {
    let end = node
        .child_by_field_name("body")
        .map_or(node.end_byte(), |body| body.start_byte());
    collapse_ws(&source[node.start_byte()..end])
}
```

## Depth-walking the matched symbol path

`collect_matching_symbols` is the recursive descent that implements
`extract_symbol`'s path-walking. At each depth:

- The current path part is the target name.
- For leaf items (functions, structs, enums, etc.) that match the
  target name AT the final depth, record the span.
- For impl blocks whose receiver type matches the target name and
  we're NOT at the final depth, descend into the impl body.
- For module items whose name matches the target name and we're NOT
  at the final depth, descend into the module body.

<a name="chunk-walk-matching"></a><sub>[`src/source_ref.rs`](../../crates/x0k-tangle/src/source_ref.rs) · `#walk-matching`</sub>

```rust {#walk-matching}
fn collect_matching_symbols(
    node: Node,
    source: &str,
    query: &SymbolQuery,
    depth: usize,
    enclosing: Option<&Enclosing>,
    matches: &mut Vec<Candidate>,
) {
    if depth >= query.parts.len() {
        return;
    }
    let target = query.parts[depth];
    let is_last = depth == query.parts.len() - 1;

    for child in node.children(&mut node.walk()) {
        match child.kind() {
            "function_item" | "struct_item" | "enum_item" | "type_item" | "const_item"
            | "static_item" | "trait_item" | "macro_definition" => {
                if let Some(name) = named_child_text(child, source) {
                    if name == target && is_last {
                        let span = make_span(name, child, source, Leading::RustAttrs);
                        matches.push(matched(span, enclosing, MatchKind::Item));
                    }
                }
            }
            "impl_item" => collect_matching_impl(child, source, query, depth, matches),
            "mod_item" => collect_matching_mod(child, source, query, depth, matches),
            _ => {}
        }
    }
}
```

An impl block is two things at once: a scope holding methods, and —
when the path ends there — an item in its own right. Both readings
are checked against the query's trait bound first, which is what
makes `<Pairs<'_, R> as fmt::Display>::fmt` walk into one of two
otherwise identical blocks.

<a name="chunk-walk-matching-impl"></a><sub>[`src/source_ref.rs`](../../crates/x0k-tangle/src/source_ref.rs) · `#walk-matching-impl`</sub>

```rust {#walk-matching-impl}
fn collect_matching_impl(
    node: Node,
    source: &str,
    query: &SymbolQuery,
    depth: usize,
    matches: &mut Vec<Candidate>,
) {
    let Some(ty) = impl_type_name(node, source) else {
        return;
    };
    let target = query.parts[depth];
    if ty != target && base_type_name(&ty) != target {
        return;
    }
    let implemented = impl_trait_name(node, source);
    if !trait_matches(query.trait_bound, implemented.as_deref()) {
        return;
    }
    let enclosing = Enclosing {
        header: Some(impl_header(node, source)),
        qualifier: implemented.map(|t| format!("<{ty} as {t}>")),
    };
    if depth == query.parts.len() - 1 {
        let span = make_span(format!("impl {ty}"), node, source, Leading::RustAttrs);
        matches.push(matched(span, Some(&enclosing), MatchKind::ImplBlock));
    } else if let Some(body) = node.child_by_field_name("body") {
        collect_matching_symbols(body, source, query, depth + 1, Some(&enclosing), matches);
    }
}
```

A module is only a scope: `mod tests` at the end of a path names a
file region no `from=` chunk asks for, so the descent is the whole
arm.

<a name="chunk-walk-matching-mod"></a><sub>[`src/source_ref.rs`](../../crates/x0k-tangle/src/source_ref.rs) · `#walk-matching-mod`</sub>

```rust {#walk-matching-mod}
fn collect_matching_mod(
    node: Node,
    source: &str,
    query: &SymbolQuery,
    depth: usize,
    matches: &mut Vec<Candidate>,
) {
    let Some(name) = named_child_text(node, source) else {
        return;
    };
    if name != query.parts[depth] || depth == query.parts.len() - 1 {
        return;
    }
    let enclosing = Enclosing {
        header: Some(format!("mod {name}")),
        qualifier: None,
    };
    for child in node.children(&mut node.walk()) {
        if child.kind() == "declaration_list" {
            collect_matching_symbols(child, source, query, depth + 1, Some(&enclosing), matches);
        }
    }
}
```

## Listing every symbol

`collect_all_symbols` is structurally similar to the matching walker,
but accumulates every leaf into a flat vec with its fully-qualified
name (path components joined by `::`). Impl blocks contribute their
receiver type to the prefix; module blocks contribute their name.

<a name="chunk-walk-all"></a><sub>[`src/source_ref.rs`](../../crates/x0k-tangle/src/source_ref.rs) · `#walk-all`</sub>

```rust {#walk-all}
fn collect_all_symbols(
    node: Node,
    source: &str,
    prefix: &[String],
    symbols: &mut Vec<SymbolSpan>,
) {
    for child in node.children(&mut node.walk()) {
        match child.kind() {
            "function_item" | "struct_item" | "enum_item" | "type_item" | "const_item"
            | "static_item" | "trait_item" | "macro_definition" => {
                if let Some(name) = named_child_text(child, source) {
                    let full_name = qualify(prefix, &name);
                    symbols.push(make_span(full_name, child, source, Leading::RustAttrs));
                }
            }
            "impl_item" => {
                if let Some(type_name) = impl_type_name(child, source) {
                    let mut impl_prefix = prefix.to_vec();
                    impl_prefix.push(type_name);
                    if let Some(body) = child.child_by_field_name("body") {
                        collect_all_symbols(body, source, &impl_prefix, symbols);
                    }
                }
            }
            "mod_item" => {
                if let Some(name) = named_child_text(child, source) {
                    let mut new_prefix = prefix.to_vec();
                    new_prefix.push(name);
                    for grandchild in child.children(&mut child.walk()) {
                        if grandchild.kind() == "declaration_list" {
                            collect_all_symbols(grandchild, source, &new_prefix, symbols);
                        }
                    }
                }
            }
            _ => {}
        }
    }
}

fn qualify(prefix: &[String], name: &str) -> String {
    if prefix.is_empty() {
        name.to_string()
    } else {
        format!("{}::{}", prefix.join("::"), name)
    }
}
```

## The TypeScript vocabulary

Rust and TypeScript disagree about what a symbol looks like, and not
only in the names of node kinds. A Rust `fn` is one node with the
whole item inside it. A TypeScript `export function f() {}` is a
`function_declaration` wrapped in an `export_statement`, and
`export const remap = (u) => u` hides its name three levels down in a
`variable_declarator` while the `const` and the `;` a reader expects
to see with it live on the statement above.

So the TypeScript side normalizes first. `ts_item` maps one child of a
scope to the three things a walk needs — the name a `symbol=` segment
must equal, the span the chunk receives, and the scope a dotted path
descends into — and the two walkers below are then the same shape as
the Rust ones with the vocabulary lifted out.

<a name="chunk-ts-item"></a><sub>[`src/source_ref.rs`](../../crates/x0k-tangle/src/source_ref.rs) · `#ts-item`</sub>

```rust {#ts-item}
struct TsItem<'t> {
    name: String,
    /// What a `symbol=` chunk is filled with — the `export` wrapper
    /// included, because the reader wrote it and expects to see it.
    span: Node<'t>,
    /// The scope `Outer.inner` descends into, for a class or namespace.
    body: Option<Node<'t>>,
}
```

The declaration forms covered are the ones a module's public surface
is written in: `function` and `class` declarations (generator and
abstract included), `interface`, `type` and `enum` declarations,
`const`/`let`/`var` bindings, and class members reached through a
class body — each with or without a leading `export`. Deliberately
outside the map, and reported as "not found" because that is the
truth: re-exports (`export { f } from "./x"` declares nothing here),
object-literal members, `prototype` and `module.exports` assignment,
anonymous `export default`, and the members of an interface or enum.

<a name="chunk-ts-item-fn"></a><sub>[`src/source_ref.rs`](../../crates/x0k-tangle/src/source_ref.rs) · `#ts-item-fn`</sub>

```rust {#ts-item-fn}
fn ts_item<'t>(node: Node<'t>, source: &str) -> Option<TsItem<'t>> {
    let decl = match node.kind() {
        "export_statement" => node
            .child_by_field_name("declaration")
            .or_else(|| node.named_child(0))?,
        _ => node,
    };
    match decl.kind() {
        "function_declaration" | "generator_function_declaration" | "class_declaration"
        | "abstract_class_declaration" | "interface_declaration" | "type_alias_declaration"
        | "enum_declaration" => Some(TsItem {
            name: named_child_text(decl, source)?,
            span: node,
            body: decl.child_by_field_name("body"),
        }),
        "lexical_declaration" | "variable_declaration" => {
            // The declarator carries the name; the span is the whole
            // statement, so `const` and the `;` come along with it.
            let declarator = decl
                .children(&mut decl.walk())
                .find(|c| c.kind() == "variable_declarator")?;
            Some(TsItem {
                name: node_text(declarator.child_by_field_name("name")?, source).to_string(),
                span: node,
                body: None,
            })
        }
        "method_definition" | "public_field_definition" => Some(TsItem {
            name: node_text(decl.child_by_field_name("name")?, source).to_string(),
            span: node,
            body: None,
        }),
        _ => None,
    }
}
```

With that normalization, the matching walk is the Rust one with the
per-kind arms gone: at each depth, take the children that name
something, keep the one whose name is the target, and either record it
(at the last part) or descend into its body.

<a name="chunk-walk-matching-ts"></a><sub>[`src/source_ref.rs`](../../crates/x0k-tangle/src/source_ref.rs) · `#walk-matching-ts`</sub>

```rust {#walk-matching-ts}
fn collect_matching_ts_symbols(
    node: Node,
    source: &str,
    query: &SymbolQuery,
    depth: usize,
    enclosing: Option<&Enclosing>,
    matches: &mut Vec<Candidate>,
) {
    if depth >= query.parts.len() {
        return;
    }
    let target = query.parts[depth];
    let is_last = depth == query.parts.len() - 1;

    for child in node.children(&mut node.walk()) {
        let Some(item) = ts_item(child, source) else {
            continue;
        };
        if item.name != target {
            continue;
        }
        if is_last {
            let span = make_span(item.name, item.span, source, Leading::None);
            matches.push(matched(span, enclosing, MatchKind::Item));
        } else if let Some(body) = item.body {
            let scope = Enclosing {
                header: Some(item.name),
                qualifier: None,
            };
            collect_matching_ts_symbols(body, source, query, depth + 1, Some(&scope), matches);
        }
    }
}
```

Listing is the same walk with the name test dropped. Unlike the Rust
lister, which descends through an impl block without recording it, a
class is itself a symbol a reader can reference — so it is recorded
*and* descended into, and `RemapPass` and `RemapPass::encode` both
appear.

<a name="chunk-walk-all-ts"></a><sub>[`src/source_ref.rs`](../../crates/x0k-tangle/src/source_ref.rs) · `#walk-all-ts`</sub>

```rust {#walk-all-ts}
fn collect_all_ts_symbols(
    node: Node,
    source: &str,
    prefix: &[String],
    symbols: &mut Vec<SymbolSpan>,
) {
    for child in node.children(&mut node.walk()) {
        let Some(item) = ts_item(child, source) else {
            continue;
        };
        let full_name = qualify(prefix, &item.name);
        symbols.push(make_span(full_name, item.span, source, Leading::None));
        if let Some(body) = item.body {
            let mut nested = prefix.to_vec();
            nested.push(item.name);
            collect_all_ts_symbols(body, source, &nested, symbols);
        }
    }
}
```

## The Python vocabulary

Python's definitions are one node each — `async def` is a
`function_definition` with the `async` inside it, not a wrapper — so
the normalization is shorter than TypeScript's. The one shape that
matters is the decorator: `@property def size` parses as a
`decorated_definition` whose span already covers the decorators. A
function without its `@property` is not that function, and here the
grammar hands that to us for free.

What a dotted path descends into is the harder question, because
Python has no namespace but the module and the class. A class body is
a scope a reader names — `Sampler.build` is how Python itself spells
that method. A function body is not: a `def` inside a `def` is a
closure, `factory.nested` names nothing in the language, and quoting
one in a document means quoting the function that owns it. So
`body` is the class body alone, and a nested `def` is deliberately
unreachable.

<a name="chunk-py-item"></a><sub>[`src/source_ref.rs`](../../crates/x0k-tangle/src/source_ref.rs) · `#py-item`</sub>

```rust {#py-item}
struct PyItem<'t> {
    name: String,
    span: Node<'t>,
    /// The class body a dotted path descends into.
    body: Option<Node<'t>>,
}

fn py_item<'t>(node: Node<'t>, source: &str) -> Option<PyItem<'t>> {
    match node.kind() {
        "decorated_definition" => {
            let inner = py_item(node.child_by_field_name("definition")?, source)?;
            Some(PyItem {
                span: node,
                ..inner
            })
        }
        "class_definition" => Some(PyItem {
            name: node_text(node.child_by_field_name("name")?, source).to_string(),
            span: node,
            body: node.child_by_field_name("body"),
        }),
        "function_definition" => Some(PyItem {
            name: node_text(node.child_by_field_name("name")?, source).to_string(),
            span: node,
            body: None,
        }),
        "expression_statement" => py_binding(node, source),
        _ => None,
    }
}
```

One thing Python needs that no other language here does. A method
sliced out of a class starts mid-line, so its first line comes out
flush while every line under it still carries the class's
indentation. In Rust or TypeScript that is only untidy; in Python it
is a syntax error the moment a definition carries a decorator, and
the chunk a document shows would not compile. So a Python span is
dedented by the column its definition starts at, and `byte_start` /
`byte_end` keep pointing at the source range the text came from.

<a name="chunk-py-span"></a><sub>[`src/source_ref.rs`](../../crates/x0k-tangle/src/source_ref.rs) · `#py-span`</sub>

```rust {#py-span}
fn py_span(name: String, node: Node, source: &str) -> SymbolSpan {
    let mut span = make_span(name, node, source, Leading::None);
    let columns = node.start_position().column;
    if columns == 0 {
        return span;
    }
    let mut out = String::new();
    for (i, line) in span.body.split('\n').enumerate() {
        if i > 0 {
            out.push('\n');
            let stripped = line
                .chars()
                .take(columns)
                .take_while(|c| *c == ' ' || *c == '\t')
                .count();
            out.push_str(&line[stripped..]);
        } else {
            out.push_str(line);
        }
    }
    span.body = out;
    span
}
```

A module- or class-level binding is a symbol a document quotes —
`VERSION = "1.0"`, `MAX_DEPTH: int = 8`, a `dataclass` field. An
assignment to an attribute is not: `self.n = n` declares no name this
file owns, and the walker never enters the function body it sits in
anyway.

<a name="chunk-py-binding"></a><sub>[`src/source_ref.rs`](../../crates/x0k-tangle/src/source_ref.rs) · `#py-binding`</sub>

```rust {#py-binding}
fn py_binding<'t>(node: Node<'t>, source: &str) -> Option<PyItem<'t>> {
    let assignment = node.named_child(0).filter(|c| c.kind() == "assignment")?;
    let left = assignment.child_by_field_name("left")?;
    (left.kind() == "identifier").then(|| PyItem {
        name: node_text(left, source).to_string(),
        span: node,
        body: None,
    })
}
```

The two walks are then the TypeScript pair with `py_item` in place of
`ts_item`.

<a name="chunk-walk-py"></a><sub>[`src/source_ref.rs`](../../crates/x0k-tangle/src/source_ref.rs) · `#walk-py`</sub>

```rust {#walk-py}
fn collect_matching_py_symbols(
    node: Node,
    source: &str,
    query: &SymbolQuery,
    depth: usize,
    enclosing: Option<&Enclosing>,
    matches: &mut Vec<Candidate>,
) {
    if depth >= query.parts.len() {
        return;
    }
    let target = query.parts[depth];
    let is_last = depth == query.parts.len() - 1;

    for child in node.children(&mut node.walk()) {
        let Some(item) = py_item(child, source) else {
            continue;
        };
        if item.name != target {
            continue;
        }
        if is_last {
            let span = py_span(item.name, item.span, source);
            matches.push(matched(span, enclosing, MatchKind::Item));
        } else if let Some(body) = item.body {
            let scope = Enclosing {
                header: Some(format!("class {}", item.name)),
                qualifier: None,
            };
            collect_matching_py_symbols(body, source, query, depth + 1, Some(&scope), matches);
        }
    }
}

fn collect_all_py_symbols(
    node: Node,
    source: &str,
    prefix: &[String],
    symbols: &mut Vec<SymbolSpan>,
) {
    for child in node.children(&mut node.walk()) {
        let Some(item) = py_item(child, source) else {
            continue;
        };
        symbols.push(py_span(qualify(prefix, &item.name), item.span, source));
        if let Some(body) = item.body {
            let mut nested = prefix.to_vec();
            nested.push(item.name);
            collect_all_py_symbols(body, source, &nested, symbols);
        }
    }
}
```

## The Julia vocabulary

Julia asks two questions the other grammars do not. First, a
definition can be a `function … end` block or an assignment —
`δλ(x) = x^2 + 1` is a definition, and the tree says `assignment`.
Second, and harder: one name is many definitions. Multiple dispatch
means `quadgk(f, a, b)` and `quadgk(f, segments)` are two methods of
one function, deliberately, and a `symbol=` naming only `quadgk` has
genuinely not said which. So a Julia item carries its signature as
written, which is both what tells two methods apart in an ambiguity
report and the spelling that selects one.

<a name="chunk-jl-item"></a><sub>[`src/source_ref.rs`](../../crates/x0k-tangle/src/source_ref.rs) · `#jl-item`</sub>

```rust {#jl-item}
struct JlItem<'t> {
    name: String,
    /// The signature as written — `quadgk(f, a, b)`. Julia's methods share
    /// a name by design; this is how a `symbol=` means one of them.
    selector: Option<String>,
    span: Node<'t>,
    /// A module: Julia's one namespace, and all that `A.b` descends.
    body: Option<Node<'t>>,
}
```

The forms are the ones a package's surface is written in: `function`
blocks and the assignment short form, `struct` and `mutable struct`,
`abstract type` and `primitive type`, `macro`, `module`, and `const`.
A bare `x = 1` is not among them — Julia writes a module-level
binding with `const`, and a top-level assignment to a plain name is a
script's working variable. A macro call wrapping a definition
(`@inline function f …`) is kept whole, for the reason a Python
decorator is: the reader wrote it, and it changes what the definition
means.

<a name="chunk-jl-item-fn"></a><sub>[`src/source_ref.rs`](../../crates/x0k-tangle/src/source_ref.rs) · `#jl-item-fn`</sub>

```rust {#jl-item-fn}
fn jl_item<'t>(node: Node<'t>, source: &str) -> Option<JlItem<'t>> {
    match node.kind() {
        "macrocall_expression" => {
            let wrapped = node
                .children(&mut node.walk())
                .find(|c| c.kind() == "macro_argument_list")?
                .named_child(0)?;
            let inner = jl_item(wrapped, source)?;
            Some(JlItem { span: node, ..inner })
        }
        "function_definition" | "macro_definition" | "assignment" => {
            jl_signature_item(node, source)
        }
        "struct_definition" | "abstract_definition" | "primitive_definition" => Some(JlItem {
            name: jl_type_name(node, source)?,
            selector: None,
            span: node,
            body: None,
        }),
        "module_definition" => Some(JlItem {
            name: node_text(node.child_by_field_name("name")?, source).to_string(),
            selector: None,
            span: node,
            body: Some(node),
        }),
        "const_statement" => jl_const_item(node, source),
        _ => None,
    }
}
```

Everything with a signature is read the same way, whether the
signature sits under a `function` keyword or on the left of an `=`.
The call inside it may be wrapped in a return type (`f(x)::Int`) or a
`where` clause, so the search for it descends; an assignment whose
left side holds no call at all is not a definition, and drops out
here.

<a name="chunk-jl-signature-item"></a><sub>[`src/source_ref.rs`](../../crates/x0k-tangle/src/source_ref.rs) · `#jl-signature-item`</sub>

```rust {#jl-signature-item}
fn jl_signature_item<'t>(node: Node<'t>, source: &str) -> Option<JlItem<'t>> {
    let signature = node
        .children(&mut node.walk())
        .find(|c| c.kind() == "signature")
        .or_else(|| node.named_child(0))?;
    let call = jl_call(signature)?;
    Some(JlItem {
        name: node_text(call.named_child(0)?, source).to_string(),
        selector: Some(collapse_ws(node_text(signature, source))),
        span: node,
        body: None,
    })
}

fn jl_call<'t>(node: Node<'t>) -> Option<Node<'t>> {
    if node.kind() == "call_expression" {
        return Some(node);
    }
    node.named_children(&mut node.walk()).find_map(jl_call)
}
```

A type's name hangs off a `type_head`, whose parameters are not part
of it: `struct Segment{T}` is `Segment`. A `const` binds through an
assignment.

<a name="chunk-jl-type-and-const"></a><sub>[`src/source_ref.rs`](../../crates/x0k-tangle/src/source_ref.rs) · `#jl-type-and-const`</sub>

```rust {#jl-type-and-const}
fn jl_type_name(node: Node, source: &str) -> Option<String> {
    let head = node
        .children(&mut node.walk())
        .find(|c| c.kind() == "type_head")?;
    Some(node_text(first_identifier(head)?, source).to_string())
}

fn jl_const_item<'t>(node: Node<'t>, source: &str) -> Option<JlItem<'t>> {
    let assignment = node
        .children(&mut node.walk())
        .find(|c| c.kind() == "assignment")?;
    Some(JlItem {
        name: node_text(first_identifier(assignment.named_child(0)?)?, source).to_string(),
        selector: None,
        span: node,
        body: None,
    })
}

fn first_identifier<'t>(node: Node<'t>) -> Option<Node<'t>> {
    if node.kind() == "identifier" {
        return Some(node);
    }
    node.named_children(&mut node.walk()).find_map(first_identifier)
}
```

The Julia walk has one more move than the others. A method attached
to another module's function carries the dot in its own name —
`Base.show(io, s)` defines `Base.show`, it does not define `show`
inside a module `Base` this file could descend into. So the target is
compared against the item's name, against the item's signature, and
against the rest of the path joined back up.

<a name="chunk-walk-matching-jl"></a><sub>[`src/source_ref.rs`](../../crates/x0k-tangle/src/source_ref.rs) · `#walk-matching-jl`</sub>

```rust {#walk-matching-jl}
fn collect_matching_jl_symbols(
    node: Node,
    source: &str,
    query: &SymbolQuery,
    depth: usize,
    enclosing: Option<&Enclosing>,
    matches: &mut Vec<Candidate>,
) {
    if depth >= query.parts.len() {
        return;
    }
    let target = collapse_ws(query.parts[depth]);
    let dotted = query.parts[depth..].join(".");
    let is_last = depth == query.parts.len() - 1;

    for child in node.children(&mut node.walk()) {
        let Some(item) = jl_item(child, source) else {
            continue;
        };
        let whole = item.name == dotted || item.selector.as_deref() == Some(target.as_str());
        if whole || (item.name == target && is_last) {
            let select = jl_select(item.selector, enclosing);
            let span = make_span(item.name, item.span, source, Leading::JuliaDocstring);
            matches.push(Candidate {
                select,
                ..matched(span, enclosing, MatchKind::Item)
            });
        } else if item.name == target {
            if let Some(body) = item.body {
                let scope = jl_module_scope(&item.name, enclosing);
                collect_matching_jl_symbols(body, source, query, depth + 1, Some(&scope), matches);
            }
        }
    }
}
```

Listing walks modules the way the TypeScript lister walks classes:
the module is a symbol, and so is everything under it.

A selector printed in an ambiguity report has to be pasteable where
the reader found the problem, so it carries the module path the walk
descended through.

<a name="chunk-jl-scope"></a><sub>[`src/source_ref.rs`](../../crates/x0k-tangle/src/source_ref.rs) · `#jl-scope`</sub>

```rust {#jl-scope}
fn jl_select(signature: Option<String>, enclosing: Option<&Enclosing>) -> Option<String> {
    let signature = signature?;
    Some(match enclosing.and_then(|e| e.qualifier.as_deref()) {
        Some(prefix) => format!("{prefix}.{signature}"),
        None => signature,
    })
}

fn jl_module_scope(name: &str, enclosing: Option<&Enclosing>) -> Enclosing {
    let qualifier = match enclosing.and_then(|e| e.qualifier.as_deref()) {
        Some(outer) => format!("{outer}.{name}"),
        None => name.to_string(),
    };
    Enclosing {
        header: Some(format!("module {name}")),
        qualifier: Some(qualifier),
    }
}
```

<a name="chunk-walk-all-jl"></a><sub>[`src/source_ref.rs`](../../crates/x0k-tangle/src/source_ref.rs) · `#walk-all-jl`</sub>

```rust {#walk-all-jl}
fn collect_all_jl_symbols(
    node: Node,
    source: &str,
    prefix: &[String],
    symbols: &mut Vec<SymbolSpan>,
) {
    for child in node.children(&mut node.walk()) {
        let Some(item) = jl_item(child, source) else {
            continue;
        };
        symbols.push(make_span(
            qualify(prefix, &item.name),
            item.span,
            source,
            Leading::JuliaDocstring,
        ));
        if let Some(body) = item.body {
            let mut nested = prefix.to_vec();
            nested.push(item.name);
            collect_all_jl_symbols(body, source, &nested, symbols);
        }
    }
}
```

## Tests

One fixture per grammar. The Rust file has an enum, a struct, an
impl block with three methods, a `Default` impl, a free function, and
a `#[cfg(test)] mod tests`; its cases exercise top-level item lookup
(enum + struct + function), nested method lookup, missing-symbol
errors, and full-list enumeration.

The TypeScript file is a shader module's public surface, written the
way the maintainer of one writes it — exported function, exported
const, exported class with a method, an interface, and one unexported
helper. Its cases pin what the language dispatch is for: a `js` chunk
resolves to the TypeScript grammar, the export wrapper survives into
the extracted body, both path spellings reach a method, and a chunk in
a language with no walker is refused with a message that names the
limit instead of blaming the reader's symbol name.

The remaining fixtures are the two languages' own hazards: a Rust
file in the shape of `pest`, where two traits define `fmt` and two
thirds of the impl blocks are generic; a Python module with stacked
decorators and a nested class; and a Julia module written the way
numerical code is, with Unicode names, docstrings, and one function
carrying two methods.

<a name="chunk-tests"></a><sub>[`src/source_ref.rs`](../../crates/x0k-tangle/src/source_ref.rs) · `#tests` · assembles [ambiguity-tests](#chunk-ambiguity-tests) · [ts-tests](#chunk-ts-tests) · [py-tests](#chunk-py-tests) · [jl-tests](#chunk-jl-tests)</sub>

`````rust {#tests}
#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = r#"
use std::io;

enum DrawCommand {
    Fill(Path),
    Stroke(Path),
}

struct Canvas2DState {
    path: Vec<u8>,
    fill_color: Color,
}

impl Canvas2DState {
    pub fn new() -> Self {
        Self { path: Vec::new(), fill_color: Color::BLACK }
    }

    pub fn arc(&mut self, x: f64, y: f64, radius: f64) {
        // arc implementation
        let sweep = x + y;
    }

    pub fn flush(&self) {
        // flush implementation
    }
}

impl Default for Canvas2DState {
    fn default() -> Self {
        Self::new()
    }
}

fn helper() -> bool {
    true
}

#[cfg(test)]
mod tests {
    fn test_it() {}
}
"#;

    #[test]
    fn extract_top_level_enum() {
        let span = extract_symbol(SAMPLE, "DrawCommand").unwrap();
        assert!(span.body.contains("enum DrawCommand"));
        assert!(span.body.contains("Stroke"));
    }

    #[test]
    fn extract_top_level_struct() {
        let span = extract_symbol(SAMPLE, "Canvas2DState").unwrap();
        assert!(span.body.contains("struct Canvas2DState"));
        assert!(span.body.contains("fill_color"));
    }

    #[test]
    fn extract_method() {
        let span = extract_symbol(SAMPLE, "Canvas2DState::arc").unwrap();
        assert!(span.body.contains("pub fn arc"));
        assert!(span.body.contains("sweep"));
        assert!(!span.body.contains("pub fn flush"));
    }

    #[test]
    fn extract_top_level_fn() {
        let span = extract_symbol(SAMPLE, "helper").unwrap();
        assert!(span.body.contains("fn helper"));
    }

    #[test]
    fn missing_symbol_errors() {
        assert!(extract_symbol(SAMPLE, "nonexistent").is_err());
    }

    #[test]
    fn list_finds_all() {
        let syms = list_symbols(SAMPLE).unwrap();
        let names: Vec<&str> = syms.iter().map(|s| s.name.as_str()).collect();
        assert!(names.contains(&"DrawCommand"));
        assert!(names.contains(&"Canvas2DState"));
        assert!(names.contains(&"Canvas2DState::new"));
        assert!(names.contains(&"Canvas2DState::arc"));
        assert!(names.contains(&"Canvas2DState::flush"));
        assert!(names.contains(&"helper"));
    }

    <<ambiguity-tests>>

    <<ts-tests>>

    <<py-tests>>

    <<jl-tests>>
}
`````

The ambiguity fixture is `pest`'s shape, down to the four lines
between the two `fmt`s. What it pins is a refusal: the tool that
promises prose cannot fall out of step with code must not answer a
question the document did not ask.

<a name="chunk-ambiguity-tests"></a><sub>[`src/source_ref.rs`](../../crates/x0k-tangle/src/source_ref.rs) · `#ambiguity-tests`</sub>

`````rust {#ambiguity-tests}
const PEST_SAMPLE: &str = r#"
use std::fmt;

/// Implementation of a `Stack` which maintains popped elements.
#[derive(Debug)]
#[repr(C)]
pub struct Stack<T: Clone> {
    /// All elements in the stack.
    cache: Vec<T>,
}

// Not a doc comment, and a blank line away.

pub struct Plain;

impl<R: RuleType> fmt::Debug for Pairs<'_, R> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "debug shape")
    }
}

impl<R: RuleType> fmt::Display for Pairs<'_, R> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "display shape")
    }
}

impl<R: RuleType> Op<R> {
    /// Prefix operator.
    #[inline]
    pub fn prefix(x: u8) -> Self {
        todo!()
    }
}
"#;

#[test]
fn two_traits_defining_fmt_refuse_to_guess() {
    let err = extract_symbol(PEST_SAMPLE, "Pairs<'_, R>::fmt")
        .unwrap_err()
        .to_string();
    assert!(err.contains("ambiguous"), "got {err}");
    assert!(err.contains("fmt::Debug for Pairs<'_, R>"), "got {err}");
    assert!(err.contains("fmt::Display for Pairs<'_, R>"), "got {err}");
    assert!(
        err.contains("symbol=\"<Pairs<'_, R> as fmt::Display>::fmt\""),
        "got {err}"
    );
}

#[test]
fn a_qualified_path_selects_one_of_two_fmts() {
    let display = extract_symbol(PEST_SAMPLE, "<Pairs<'_, R> as fmt::Display>::fmt").unwrap();
    assert!(display.body.contains("display shape"), "got {:?}", display.body);
    let debug = extract_symbol(PEST_SAMPLE, "<Pairs<'_, R> as fmt::Debug>::fmt").unwrap();
    assert!(debug.body.contains("debug shape"), "got {:?}", debug.body);
}

#[test]
fn a_qualifier_may_drop_the_generics_and_the_trait_path() {
    let span = extract_symbol(PEST_SAMPLE, "<Pairs as Display>::fmt").unwrap();
    assert!(span.body.contains("display shape"), "got {:?}", span.body);
}

#[test]
fn a_generic_impl_is_reachable_by_its_base_name() {
    let bare = extract_symbol(PEST_SAMPLE, "Op::prefix").unwrap();
    let spelled = extract_symbol(PEST_SAMPLE, "Op<R>::prefix").unwrap();
    assert!(bare.body.contains("pub fn prefix"), "got {:?}", bare.body);
    assert_eq!(bare.byte_start, spelled.byte_start);
}

#[test]
fn attributes_and_doc_comments_come_along() {
    let span = extract_symbol(PEST_SAMPLE, "Stack").unwrap();
    assert!(
        span.body.starts_with("/// Implementation of a `Stack`"),
        "got {:?}",
        span.body
    );
    assert!(span.body.contains("#[derive(Debug)]"), "got {:?}", span.body);
    assert!(span.body.contains("#[repr(C)]"), "got {:?}", span.body);
    assert!(span.body.contains("cache: Vec<T>"), "got {:?}", span.body);
}

#[test]
fn a_method_keeps_its_attribute_and_doc() {
    let span = extract_symbol(PEST_SAMPLE, "Op::prefix").unwrap();
    assert!(
        span.body.starts_with("/// Prefix operator."),
        "got {:?}",
        span.body
    );
    assert!(span.body.contains("#[inline]"), "got {:?}", span.body);
}

#[test]
fn a_plain_comment_across_a_blank_line_stays_out() {
    let span = extract_symbol(PEST_SAMPLE, "Plain").unwrap();
    assert_eq!(span.body, "pub struct Plain;", "got {:?}", span.body);
}
`````

<a name="chunk-ts-tests"></a><sub>[`src/source_ref.rs`](../../crates/x0k-tangle/src/source_ref.rs) · `#ts-tests`</sub>

```rust {#ts-tests}
const TS_SAMPLE: &str = r#"
export function createHorizonRemap(opts) {
  const scale = opts.scale ?? 1;
  return (u, v) => [u * scale, v * scale];
}

export const HORIZON_EPSILON = 1e-6;

export class RemapPass {
  encode(pass) {
    pass.draw(3);
  }
}

export interface RemapOptions {
  scale: number;
}

function internalHelper() {
  return 42;
}
"#;

#[test]
fn extracts_an_exported_js_function_with_its_export() {
    let span =
        extract_symbol_in(TS_SAMPLE, "createHorizonRemap", SymbolLanguage::TypeScript).unwrap();
    assert!(
        span.body.starts_with("export function createHorizonRemap"),
        "got {:?}",
        span.body
    );
    assert!(span.body.contains("opts.scale"));
    assert!(!span.body.contains("internalHelper"));
}

#[test]
fn extracts_an_exported_const_with_its_declaration() {
    let span =
        extract_symbol_in(TS_SAMPLE, "HORIZON_EPSILON", SymbolLanguage::TypeScript).unwrap();
    assert!(
        span.body.starts_with("export const HORIZON_EPSILON = 1e-6"),
        "got {:?}",
        span.body
    );
}

#[test]
fn extracts_a_class_method_by_either_path_spelling() {
    let dotted =
        extract_symbol_in(TS_SAMPLE, "RemapPass.encode", SymbolLanguage::TypeScript).unwrap();
    let colons =
        extract_symbol_in(TS_SAMPLE, "RemapPass::encode", SymbolLanguage::TypeScript).unwrap();
    assert!(dotted.body.starts_with("encode(pass)"), "got {:?}", dotted.body);
    assert_eq!(dotted.byte_start, colons.byte_start);
}

#[test]
fn finds_an_unexported_function_too() {
    let span =
        extract_symbol_in(TS_SAMPLE, "internalHelper", SymbolLanguage::TypeScript).unwrap();
    assert!(span.body.starts_with("function internalHelper"));
}

#[test]
fn lists_every_ts_symbol_including_class_members() {
    let syms = list_symbols_in(TS_SAMPLE, SymbolLanguage::TypeScript).unwrap();
    let names: Vec<&str> = syms.iter().map(|s| s.name.as_str()).collect();
    for expected in [
        "createHorizonRemap",
        "HORIZON_EPSILON",
        "RemapPass",
        "RemapPass::encode",
        "RemapOptions",
        "internalHelper",
    ] {
        assert!(names.contains(&expected), "{expected} missing from {names:?}");
    }
}

#[test]
fn js_and_jsx_fence_tags_resolve_to_a_grammar() {
    assert_eq!(
        SymbolLanguage::for_lang(Some("js")).unwrap(),
        SymbolLanguage::TypeScript
    );
    assert_eq!(
        SymbolLanguage::for_lang(Some("javascript")).unwrap(),
        SymbolLanguage::TypeScript
    );
    assert_eq!(
        SymbolLanguage::for_lang(Some("jsx")).unwrap(),
        SymbolLanguage::Tsx
    );
    assert_eq!(
        SymbolLanguage::for_lang(Some("rs")).unwrap(),
        SymbolLanguage::Rust
    );
}

#[test]
fn an_unsupported_language_names_the_limit_not_the_symbol() {
    let err = SymbolLanguage::for_lang(Some("ruby"))
        .unwrap_err()
        .to_string();
    assert!(err.contains("symbol extraction supports"), "got {err}");
    assert!(err.contains("ruby"), "got {err}");
    assert!(!err.contains("not found"), "got {err}");
}

#[test]
fn a_fence_with_no_language_is_refused() {
    let err = SymbolLanguage::for_lang(None).unwrap_err().to_string();
    assert!(err.contains("needs a language on the fence"), "got {err}");
}

#[test]
fn a_js_symbol_read_through_the_rust_grammar_is_invisible() {
    // The defect this dispatch exists for: the Rust-pinned shorthand
    // finds nothing in JavaScript, and "not found" is why a reader
    // spent an afternoon checking their own export names.
    assert!(extract_symbol(TS_SAMPLE, "createHorizonRemap").is_err());
}
```

The Python cases are the ones a scientific-computing module is made
of: a plain `def`, an `async def`, a class whose methods are reached
by either path spelling, a decorated definition that must keep its
decorator, stacked decorators, and a module-level constant.

<a name="chunk-py-tests"></a><sub>[`src/source_ref.rs`](../../crates/x0k-tangle/src/source_ref.rs) · `#py-tests`</sub>

```rust {#py-tests}
const PY_SAMPLE: &str = r#"
import functools

VERSION = "1.0"
MAX_DEPTH: int = 8

def logp(value, mu):
    return -0.5 * (value - mu) ** 2

async def fetch(url):
    return await get(url)

class Sampler:
    """A step method."""

    def __init__(self, n):
        self.n = n

    @property
    def size(self):
        return self.n

    @staticmethod
    @functools.cache
    def build(n):
        return Sampler(n)

    class Inner:
        def deep(self):
            return 1

@dataclass
class Config:
    lr: float = 0.1
"#;

#[test]
fn python_fence_tags_resolve_to_the_python_grammar() {
    assert_eq!(
        SymbolLanguage::for_lang(Some("python")).unwrap(),
        SymbolLanguage::Python
    );
    assert_eq!(
        SymbolLanguage::for_lang(Some("py")).unwrap(),
        SymbolLanguage::Python
    );
}

#[test]
fn extracts_a_plain_def() {
    let span = extract_symbol_in(PY_SAMPLE, "logp", SymbolLanguage::Python).unwrap();
    assert!(span.body.starts_with("def logp(value, mu):"), "got {:?}", span.body);
    assert!(span.body.contains("-0.5"));
    assert!(!span.body.contains("async def"));
}

#[test]
fn an_async_def_is_one_definition() {
    let span = extract_symbol_in(PY_SAMPLE, "fetch", SymbolLanguage::Python).unwrap();
    assert!(span.body.starts_with("async def fetch(url):"), "got {:?}", span.body);
}

#[test]
fn extracts_a_python_method_by_either_path_spelling() {
    let dotted = extract_symbol_in(PY_SAMPLE, "Sampler.__init__", SymbolLanguage::Python).unwrap();
    let colons = extract_symbol_in(PY_SAMPLE, "Sampler::__init__", SymbolLanguage::Python).unwrap();
    assert!(dotted.body.starts_with("def __init__(self, n):"), "got {:?}", dotted.body);
    assert_eq!(dotted.byte_start, colons.byte_start);
}

#[test]
fn a_decorated_def_keeps_its_decorator() {
    let span = extract_symbol_in(PY_SAMPLE, "Sampler.size", SymbolLanguage::Python).unwrap();
    assert!(span.body.starts_with("@property"), "got {:?}", span.body);
    assert!(span.body.contains("def size(self):"), "got {:?}", span.body);
}

#[test]
fn a_method_comes_out_dedented_so_it_parses_alone() {
    // Without this a decorated method is an IndentationError: the first
    // decorator flush left, the second at the class's indentation.
    let span = extract_symbol_in(PY_SAMPLE, "Sampler.build", SymbolLanguage::Python).unwrap();
    assert_eq!(
        span.body,
        "@staticmethod\n@functools.cache\ndef build(n):\n    return Sampler(n)",
        "got {:?}",
        span.body
    );
    let method = extract_symbol_in(PY_SAMPLE, "Sampler.size", SymbolLanguage::Python).unwrap();
    assert!(method.body.contains("\n    return self.n"), "got {:?}", method.body);
}

#[test]
fn stacked_decorators_all_come_along() {
    let span = extract_symbol_in(PY_SAMPLE, "Sampler.build", SymbolLanguage::Python).unwrap();
    assert!(span.body.starts_with("@staticmethod"), "got {:?}", span.body);
    assert!(span.body.contains("@functools.cache"), "got {:?}", span.body);
}

#[test]
fn a_decorated_class_keeps_its_decorator() {
    let span = extract_symbol_in(PY_SAMPLE, "Config", SymbolLanguage::Python).unwrap();
    assert!(span.body.starts_with("@dataclass"), "got {:?}", span.body);
    assert!(span.body.contains("lr: float = 0.1"), "got {:?}", span.body);
}

#[test]
fn a_module_level_binding_is_a_symbol() {
    let plain = extract_symbol_in(PY_SAMPLE, "VERSION", SymbolLanguage::Python).unwrap();
    assert_eq!(plain.body, "VERSION = \"1.0\"", "got {:?}", plain.body);
    let annotated = extract_symbol_in(PY_SAMPLE, "MAX_DEPTH", SymbolLanguage::Python).unwrap();
    assert_eq!(annotated.body, "MAX_DEPTH: int = 8", "got {:?}", annotated.body);
}

#[test]
fn lists_every_python_symbol_including_nested_classes() {
    let syms = list_symbols_in(PY_SAMPLE, SymbolLanguage::Python).unwrap();
    let names: Vec<&str> = syms.iter().map(|s| s.name.as_str()).collect();
    for expected in [
        "VERSION",
        "MAX_DEPTH",
        "logp",
        "fetch",
        "Sampler",
        "Sampler::__init__",
        "Sampler::size",
        "Sampler::build",
        "Sampler::Inner",
        "Sampler::Inner::deep",
        "Config",
    ] {
        assert!(names.contains(&expected), "{expected} missing from {names:?}");
    }
}

#[test]
fn a_local_assignment_is_not_a_python_symbol() {
    assert!(extract_symbol_in(PY_SAMPLE, "Sampler.n", SymbolLanguage::Python).is_err());
}
```

The Julia cases carry the two things that make Julia different: names
outside ASCII, which must come back byte-for-byte, and one function
with two methods, which must refuse to guess and then say how to name
each.

<a name="chunk-jl-tests"></a><sub>[`src/source_ref.rs`](../../crates/x0k-tangle/src/source_ref.rs) · `#jl-tests`</sub>

```rust {#jl-tests}
const JL_SAMPLE: &str = r#"
module QuadGK

const MAXN = 12
const δ_tol = 1e-8

"""
    gauss(f, a, b)

Two-point Gauss rule.
"""
function gauss(f, a, b)
    return (b - a) * f((a + b) / 2)
end

δλ(x) = x^2 + 1

x̄(v) = sum(v) / length(v)

function quadgk(f, a, b)
    return gauss(f, a, b)
end

function quadgk(f, segments)
    return sum(s -> gauss(f, s...), segments)
end

struct Segment{T}
    a::T
    b::T
end

mutable struct Cache
    hits::Int
end

macro checked(ex)
    return esc(ex)
end

@inline function fast(x)
    return x
end

module Inner
    function deep(x)
        return x
    end
end

end
"#;

#[test]
fn julia_fence_tags_resolve_to_the_julia_grammar() {
    assert_eq!(
        SymbolLanguage::for_lang(Some("julia")).unwrap(),
        SymbolLanguage::Julia
    );
    assert_eq!(
        SymbolLanguage::for_lang(Some("jl")).unwrap(),
        SymbolLanguage::Julia
    );
}

#[test]
fn extracts_a_function_block_with_its_docstring() {
    let span = extract_symbol_in(JL_SAMPLE, "QuadGK.gauss", SymbolLanguage::Julia).unwrap();
    assert!(span.body.starts_with("\"\"\""), "got {:?}", span.body);
    assert!(span.body.contains("gauss(f, a, b)"), "got {:?}", span.body);
    assert!(span.body.contains("function gauss(f, a, b)"), "got {:?}", span.body);
    assert!(span.body.trim_end().ends_with("end"), "got {:?}", span.body);
}

#[test]
fn a_unicode_short_form_survives_byte_for_byte() {
    let span = extract_symbol_in(JL_SAMPLE, "QuadGK.δλ", SymbolLanguage::Julia).unwrap();
    assert_eq!(span.name, "δλ");
    assert_eq!(span.body, "δλ(x) = x^2 + 1", "got {:?}", span.body);
    let combining = extract_symbol_in(JL_SAMPLE, "QuadGK.x̄", SymbolLanguage::Julia).unwrap();
    assert_eq!(combining.name, "x̄");
    assert_eq!(combining.body, "x̄(v) = sum(v) / length(v)");
    assert_eq!(&JL_SAMPLE[combining.byte_start..combining.byte_end], combining.body);
}

#[test]
fn a_unicode_const_survives_byte_for_byte() {
    let span = extract_symbol_in(JL_SAMPLE, "QuadGK.δ_tol", SymbolLanguage::Julia).unwrap();
    assert_eq!(span.body, "const δ_tol = 1e-8", "got {:?}", span.body);
}

#[test]
fn two_methods_of_one_function_refuse_to_guess() {
    let err = extract_symbol_in(JL_SAMPLE, "QuadGK.quadgk", SymbolLanguage::Julia)
        .unwrap_err()
        .to_string();
    assert!(err.contains("ambiguous"), "got {err}");
    assert!(err.contains("symbol=\"QuadGK.quadgk(f, a, b)\""), "got {err}");
    assert!(err.contains("symbol=\"QuadGK.quadgk(f, segments)\""), "got {err}");
}

#[test]
fn a_signature_selects_one_method() {
    let span =
        extract_symbol_in(JL_SAMPLE, "QuadGK.quadgk(f, segments)", SymbolLanguage::Julia).unwrap();
    assert!(span.body.contains("segments)"), "got {:?}", span.body);
    assert!(!span.body.contains("(b - a)"), "got {:?}", span.body);
}

#[test]
fn extracts_structs_macros_and_macro_wrapped_definitions() {
    let plain = extract_symbol_in(JL_SAMPLE, "QuadGK.Segment", SymbolLanguage::Julia).unwrap();
    assert!(plain.body.starts_with("struct Segment{T}"), "got {:?}", plain.body);
    let mutable = extract_symbol_in(JL_SAMPLE, "QuadGK.Cache", SymbolLanguage::Julia).unwrap();
    assert!(mutable.body.starts_with("mutable struct Cache"), "got {:?}", mutable.body);
    let mac = extract_symbol_in(JL_SAMPLE, "QuadGK.checked", SymbolLanguage::Julia).unwrap();
    assert!(mac.body.starts_with("macro checked(ex)"), "got {:?}", mac.body);
    let wrapped = extract_symbol_in(JL_SAMPLE, "QuadGK.fast", SymbolLanguage::Julia).unwrap();
    assert!(wrapped.body.starts_with("@inline function fast(x)"), "got {:?}", wrapped.body);
}

#[test]
fn descends_into_a_nested_julia_module() {
    let span = extract_symbol_in(JL_SAMPLE, "QuadGK.Inner.deep", SymbolLanguage::Julia).unwrap();
    assert!(span.body.starts_with("function deep(x)"), "got {:?}", span.body);
}

#[test]
fn lists_every_julia_symbol_under_its_module() {
    let syms = list_symbols_in(JL_SAMPLE, SymbolLanguage::Julia).unwrap();
    let names: Vec<&str> = syms.iter().map(|s| s.name.as_str()).collect();
    for expected in [
        "QuadGK",
        "QuadGK::MAXN",
        "QuadGK::δ_tol",
        "QuadGK::gauss",
        "QuadGK::δλ",
        "QuadGK::x̄",
        "QuadGK::Segment",
        "QuadGK::Cache",
        "QuadGK::checked",
        "QuadGK::fast",
        "QuadGK::Inner",
        "QuadGK::Inner::deep",
    ] {
        assert!(names.contains(&expected), "{expected} missing from {names:?}");
    }
}
```

## Composing the module

<a name="chunk-root"></a><sub>[`src/source_ref.rs`](../../crates/x0k-tangle/src/source_ref.rs) · `#root` · assembles [imports](#chunk-imports) · [symbol-span](#chunk-symbol-span) · [symbol-language](#chunk-symbol-language) · [for-lang](#chunk-for-lang) · [julia-tag](#chunk-julia-tag) · [extract-symbol](#chunk-extract-symbol) · [symbol-query](#chunk-symbol-query) · [split-symbol-path](#chunk-split-symbol-path) · [split-julia-path](#chunk-split-julia-path) · [parse-qualified-path](#chunk-parse-qualified-path) · [angle-scanning](#chunk-angle-scanning) · [candidate](#chunk-candidate) · [pick-match](#chunk-pick-match) · [ambiguity-report](#chunk-ambiguity-report) · [matched](#chunk-matched) · [list-symbols](#chunk-list-symbols) · [rust-shims](#chunk-rust-shims) · [parse-source](#chunk-parse-source) · [tree-helpers](#chunk-tree-helpers) · [leading](#chunk-leading) · [span-start](#chunk-span-start) · [make-span](#chunk-make-span) · [impl-reading](#chunk-impl-reading) · [trait-matching](#chunk-trait-matching) · [impl-header](#chunk-impl-header) · [walk-matching](#chunk-walk-matching) · [walk-matching-impl](#chunk-walk-matching-impl) · [walk-matching-mod](#chunk-walk-matching-mod) · [walk-all](#chunk-walk-all) · [ts-item](#chunk-ts-item) · [ts-item-fn](#chunk-ts-item-fn) · [walk-matching-ts](#chunk-walk-matching-ts) · [walk-all-ts](#chunk-walk-all-ts) · [py-item](#chunk-py-item) · [py-binding](#chunk-py-binding) · [py-span](#chunk-py-span) · [walk-py](#chunk-walk-py) · [jl-item](#chunk-jl-item) · [jl-item-fn](#chunk-jl-item-fn) · [jl-signature-item](#chunk-jl-signature-item) · [jl-type-and-const](#chunk-jl-type-and-const) · [jl-scope](#chunk-jl-scope) · [walk-matching-jl](#chunk-walk-matching-jl) · [walk-all-jl](#chunk-walk-all-jl) · [tests](#chunk-tests)</sub>

```rust {#root}
<<imports>>

<<symbol-span>>

<<symbol-language>>

<<for-lang>>

<<julia-tag>>

<<extract-symbol>>

<<symbol-query>>

<<split-symbol-path>>

<<split-julia-path>>

<<parse-qualified-path>>

<<angle-scanning>>

<<candidate>>

<<pick-match>>

<<ambiguity-report>>

<<matched>>

<<list-symbols>>

<<rust-shims>>

<<parse-source>>

<<tree-helpers>>

<<leading>>

<<span-start>>

<<make-span>>

<<impl-reading>>

<<trait-matching>>

<<impl-header>>

<<walk-matching>>

<<walk-matching-impl>>

<<walk-matching-mod>>

<<walk-all>>

<<ts-item>>

<<ts-item-fn>>

<<walk-matching-ts>>

<<walk-all-ts>>

<<py-item>>

<<py-binding>>

<<py-span>>

<<walk-py>>

<<jl-item>>

<<jl-item-fn>>

<<jl-signature-item>>

<<jl-type-and-const>>

<<jl-scope>>

<<walk-matching-jl>>

<<walk-all-jl>>

<<tests>>
```
