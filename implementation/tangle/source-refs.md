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
tree-sitter grammar for the language the chunk declares — Rust, or
TypeScript, which also parses plain JavaScript — walks the tree
looking for a symbol matching the declared path, and returns the byte
range that constitutes the symbol's body. No regex, no brace-counting
— the AST is the source of truth for symbol boundaries.

A chunk in a language with no walker here is refused by name before
anything is parsed, and that refusal is why the language is an
argument at all. Read through the Rust grammar, a JavaScript file
parses into a tree with none of the reader's symbols in it, and the
honest report of that is "extraction does not speak this language" —
not "symbol `createHorizonRemap` not found", which reads as *you typed
the name wrong* and sends a reader off to check their own exports.

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

The fence tags — `rs`, `ts`, `js`, `jsx` — are not ours to redefine.
`x0k-syntax` owns that mapping for the whole toolchain (it is what the
weaver highlights by), so `for_lang` resolves through it and then
narrows, because `x0k-syntax` also knows JSON and Python, which have
grammars but no symbol walker here. The grammars themselves we take
from `tree-sitter-rust` and `tree-sitter-typescript` directly, as this
crate already did for Rust: `x0k-syntax` hands out classified tokens,
not `tree_sitter::Language` handles, and widening a highlighting
crate's API to serve a caller that wants a parser would be the worse
seam.

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
}

/// The set `for_lang` names when it refuses, spelled as fence tags.
const SUPPORTED_LANGS: &str = "rust, typescript, javascript, tsx";
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
        match FenceLanguage::from_str(tag) {
            Some(FenceLanguage::Rust) => Ok(Self::Rust),
            Some(FenceLanguage::Typescript) => Ok(Self::TypeScript),
            Some(FenceLanguage::Tsx) => Ok(Self::Tsx),
            _ => bail!("symbol extraction supports {SUPPORTED_LANGS} (this chunk is `{tag}`)"),
        }
    }

    fn grammar(self) -> tree_sitter::Language {
        match self {
            Self::Rust => tree_sitter_rust::LANGUAGE.into(),
            Self::TypeScript => tree_sitter_typescript::LANGUAGE_TYPESCRIPT.into(),
            Self::Tsx => tree_sitter_typescript::LANGUAGE_TSX.into(),
        }
    }
}
```

## The single-symbol extractor

`extract_symbol_in` parses the source, walks the tree looking for a
symbol matching `symbol_path`, and returns one `SymbolSpan`. The path
resolution is multi-step:

- For a top-level symbol (`helper`), parse_path is `["helper"]` and
  the walker matches at depth 0.
- For an impl method (`Canvas2DState::arc`), the walker enters the
  matching impl block and continues descent.
- For a module-qualified path (`tests::test_it`), the walker enters
  the module body.

<a name="chunk-extract-symbol"></a><sub>[`src/source_ref.rs`](../../crates/x0k-tangle/src/source_ref.rs) · `#extract-symbol`</sub>

```rust {#extract-symbol}
pub fn extract_symbol_in(
    source: &str,
    symbol_path: &str,
    lang: SymbolLanguage,
) -> Result<SymbolSpan> {
    let tree = parse(source, lang)?;
    let parts = split_symbol_path(symbol_path, lang);
    let root = tree.root_node();

    let mut matches = Vec::new();
    match lang {
        SymbolLanguage::Rust => collect_matching_symbols(root, source, &parts, 0, &mut matches),
        SymbolLanguage::TypeScript | SymbolLanguage::Tsx => {
            collect_matching_ts_symbols(root, source, &parts, 0, &mut matches)
        }
    }
    pick_match(matches, symbol_path, &parts)
}
```

The path separator is the one a reader of that language already
writes. Rust splits on `::` alone, because a `.` never separates Rust
items. TypeScript takes either, so a document can spell every
`symbol=` the same way whatever the chunk's language.

<a name="chunk-split-symbol-path"></a><sub>[`src/source_ref.rs`](../../crates/x0k-tangle/src/source_ref.rs) · `#split-symbol-path`</sub>

```rust {#split-symbol-path}
fn split_symbol_path(symbol_path: &str, lang: SymbolLanguage) -> Vec<&str> {
    match lang {
        SymbolLanguage::Rust => symbol_path.split("::").collect(),
        SymbolLanguage::TypeScript | SymbolLanguage::Tsx => symbol_path
            .split(['.', ':'])
            .filter(|part| !part.is_empty())
            .collect(),
    }
}
```

If multiple symbols match (rare — usually only happens when two
distinct paths share the same leaf, like `Foo::new` and `Bar::new`),
the resolver prefers an exact path match before falling back to the
first candidate.

<a name="chunk-pick-match"></a><sub>[`src/source_ref.rs`](../../crates/x0k-tangle/src/source_ref.rs) · `#pick-match`</sub>

```rust {#pick-match}
fn pick_match(matches: Vec<SymbolSpan>, symbol_path: &str, parts: &[&str]) -> Result<SymbolSpan> {
    match matches.len() {
        0 => bail!("symbol '{}' not found", symbol_path),
        1 => Ok(matches.into_iter().next().unwrap()),
        _ => {
            // Prefer exact depth match
            if let Some(m) = matches
                .iter()
                .find(|m| m.name == symbol_path || m.name == *parts.last().unwrap_or(&""))
            {
                Ok(m.clone())
            } else {
                Ok(matches.into_iter().next().unwrap())
            }
        }
    }
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
extraction is one slice.

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

fn impl_type_name(node: Node, source: &str) -> Option<String> {
    // For `impl Foo { ... }`, extract "Foo"
    // For `impl Trait for Foo { ... }`, extract "Foo"
    node.child_by_field_name("type")
        .map(|t| node_text(t, source).to_string())
}

fn make_span(name: String, node: Node, source: &str) -> SymbolSpan {
    SymbolSpan {
        name,
        body: node_text(node, source).to_string(),
        start_line: node.start_position().row + 1,
        end_line: node.end_position().row + 1,
        byte_start: node.start_byte(),
        byte_end: node.end_byte(),
    }
}
```

`named_child_text` tries the canonical `name` field first, then falls
back to any `identifier` / `type_identifier` child — different
tree-sitter node kinds expose their name differently, and this covers
both styles.

`impl_type_name` extracts the implementor type from an impl block.
For `impl Trait for Foo` it returns `Foo` (the *receiver*, not the
trait being implemented). That's the right behavior for path
resolution — `Canvas2DState::arc` looks up the impl block that
provides methods *on* `Canvas2DState`, not the one that implements a
trait *for* it.

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
    parts: &[&str],
    depth: usize,
    matches: &mut Vec<SymbolSpan>,
) {
    if depth >= parts.len() {
        return;
    }

    let target = parts[depth];
    let is_last = depth == parts.len() - 1;

    for child in node.children(&mut node.walk()) {
        match child.kind() {
            "function_item" | "struct_item" | "enum_item" | "type_item"
            | "const_item" | "static_item" | "trait_item" | "macro_definition" => {
                if let Some(name) = named_child_text(child, source) {
                    if name == target && is_last {
                        matches.push(make_span(name, child, source));
                    }
                }
            }
            "impl_item" => {
                if let Some(type_name) = impl_type_name(child, source) {
                    if type_name == target && !is_last {
                        // Descend into impl to find methods
                        if let Some(body) = child.child_by_field_name("body") {
                            collect_matching_symbols(
                                body, source, parts, depth + 1, matches,
                            );
                        }
                    }
                    if type_name == target && is_last {
                        matches.push(make_span(
                            format!("impl {}", type_name),
                            child,
                            source,
                        ));
                    }
                }
            }
            // Recurse into module bodies
            "mod_item" => {
                if let Some(name) = named_child_text(child, source) {
                    if name == target && !is_last {
                        for grandchild in child.children(&mut child.walk()) {
                            if grandchild.kind() == "declaration_list" {
                                collect_matching_symbols(
                                    grandchild, source, parts, depth + 1, matches,
                                );
                            }
                        }
                    }
                }
            }
            _ => {}
        }
    }

    // Also try matching at current depth without consuming a part
    // (for top-level items when depth == 0 and parts.len() == 1)
    if depth == 0 && parts.len() == 1 {
        // Already handled above
    }
}
```

The trailing `if depth == 0 && parts.len() == 1` block is a no-op
left as a hook for future single-part matching variations — kept as a
comment so the structure is self-explanatory.

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
            "function_item" | "struct_item" | "enum_item" | "type_item"
            | "const_item" | "static_item" | "trait_item" | "macro_definition" => {
                if let Some(name) = named_child_text(child, source) {
                    let full_name = if prefix.is_empty() {
                        name
                    } else {
                        format!("{}::{}", prefix.join("::"), name)
                    };
                    symbols.push(make_span(full_name, child, source));
                }
            }
            "impl_item" => {
                if let Some(type_name) = impl_type_name(child, source) {
                    let impl_prefix = if prefix.is_empty() {
                        vec![type_name.clone()]
                    } else {
                        let mut p = prefix.to_vec();
                        p.push(type_name.clone());
                        p
                    };
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
    parts: &[&str],
    depth: usize,
    matches: &mut Vec<SymbolSpan>,
) {
    if depth >= parts.len() {
        return;
    }
    let target = parts[depth];
    let is_last = depth == parts.len() - 1;

    for child in node.children(&mut node.walk()) {
        let Some(item) = ts_item(child, source) else {
            continue;
        };
        if item.name != target {
            continue;
        }
        if is_last {
            matches.push(make_span(item.name, item.span, source));
        } else if let Some(body) = item.body {
            collect_matching_ts_symbols(body, source, parts, depth + 1, matches);
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
        let full_name = if prefix.is_empty() {
            item.name.clone()
        } else {
            format!("{}::{}", prefix.join("::"), item.name)
        };
        symbols.push(make_span(full_name, item.span, source));
        if let Some(body) = item.body {
            let mut nested = prefix.to_vec();
            nested.push(item.name);
            collect_all_ts_symbols(body, source, &nested, symbols);
        }
    }
}
```

## Tests

Two fixtures, one per grammar. The Rust file has an enum, a struct, an
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

<a name="chunk-tests"></a><sub>[`src/source_ref.rs`](../../crates/x0k-tangle/src/source_ref.rs) · `#tests` · assembles [ts-tests](#chunk-ts-tests)</sub>

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

    <<ts-tests>>
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
    let err = SymbolLanguage::for_lang(Some("python"))
        .unwrap_err()
        .to_string();
    assert!(err.contains("symbol extraction supports"), "got {err}");
    assert!(err.contains("python"), "got {err}");
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

## Composing the module

<a name="chunk-root"></a><sub>[`src/source_ref.rs`](../../crates/x0k-tangle/src/source_ref.rs) · `#root` · assembles [imports](#chunk-imports) · [symbol-span](#chunk-symbol-span) · [symbol-language](#chunk-symbol-language) · [for-lang](#chunk-for-lang) · [extract-symbol](#chunk-extract-symbol) · [split-symbol-path](#chunk-split-symbol-path) · [pick-match](#chunk-pick-match) · [list-symbols](#chunk-list-symbols) · [rust-shims](#chunk-rust-shims) · [parse-source](#chunk-parse-source) · [tree-helpers](#chunk-tree-helpers) · [walk-matching](#chunk-walk-matching) · [walk-all](#chunk-walk-all) · [ts-item](#chunk-ts-item) · [ts-item-fn](#chunk-ts-item-fn) · [walk-matching-ts](#chunk-walk-matching-ts) · [walk-all-ts](#chunk-walk-all-ts) · [tests](#chunk-tests)</sub>

```rust {#root}
<<imports>>

<<symbol-span>>

<<symbol-language>>

<<for-lang>>

<<extract-symbol>>

<<split-symbol-path>>

<<pick-match>>

<<list-symbols>>

<<rust-shims>>

<<parse-source>>

<<tree-helpers>>

<<walk-matching>>

<<walk-all>>

<<ts-item>>

<<ts-item-fn>>

<<walk-matching-ts>>

<<walk-all-ts>>

<<tests>>
```
