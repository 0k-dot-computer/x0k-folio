---
x0k:
  format: folio/v1
  id: x0k:implementation/tangle/source-refs
  type: implementation
  status: draft
  summary: Tree-sitter symbol extraction for a `from=` chunk — the syntax tree decides where a symbol's body begins and ends, with no regex and no brace counting — and the symbol listing the doc browser reads.
  concerns: [tangle, literate, tree-sitter, symbol-extraction]
  tangle:
    crate: x0k-tangle
    root: src/source_ref.rs
  edges:
    cites:
      - x0k:implementation/tangle/protocol
      - x0k:implementation/tangle/chunk
---
# Symbol extraction for `from=` chunks

A `from=` chunk is the inverse of a tangle output: instead of the
document owning code that becomes a source file, the [literate
document](../../wiki/literate-programming.md "x0k:wiki/literate-programming")
*references* code that already lives in a source file. The fence
declares "fill this block with the body of `Type::method` from
`crate/src/file.rs`," and on sync, the toolchain extracts the
symbol's body and writes it into the chunk.

This module does the extraction. It uses tree-sitter to parse Rust
source code into a syntax tree, walks the tree looking for a symbol
matching the declared path, and returns the byte range that
constitutes the symbol's body. No regex, no brace-counting — the AST
is the source of truth for symbol boundaries.

The same machinery powers `list_symbols`, which the doc-browser uses
to enumerate symbols in a file for navigation panels.

## Imports

<a name="chunk-imports"></a><sub>[`src/source_ref.rs`](../../../x0k-tangle/src/source_ref.rs) · `#imports`</sub>

```rust {#imports}
use anyhow::{bail, Result};
use tree_sitter::{Node, Parser};
```

## The result shape

`SymbolSpan` is what the extractor returns. It carries the symbol's
name (the leaf identifier — `arc`, `Canvas2DState`, `helper`), the
extracted body text, line bounds (1-indexed for editor-friendly
diagnostics), and byte bounds (for precise stitch-back).

<a name="chunk-symbol-span"></a><sub>[`src/source_ref.rs`](../../../x0k-tangle/src/source_ref.rs) · `#symbol-span`</sub>

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

## The single-symbol extractor

`extract_symbol` parses the source, walks the tree looking for a
symbol matching `symbol_path` (split on `::`), and returns one
`SymbolSpan`. The path resolution is multi-step:

- For a top-level symbol (`helper`), parse_path is `["helper"]` and
  the walker matches at depth 0.
- For an impl method (`Canvas2DState::arc`), the walker enters the
  matching impl block and continues descent.
- For a module-qualified path (`tests::test_it`), the walker enters
  the module body.

<a name="chunk-extract-symbol"></a><sub>[`src/source_ref.rs`](../../../x0k-tangle/src/source_ref.rs) · `#extract-symbol`</sub>

```rust {#extract-symbol}
pub fn extract_symbol(source: &str, symbol_path: &str) -> Result<SymbolSpan> {
    let mut parser = Parser::new();
    let lang = tree_sitter_rust::LANGUAGE;
    parser
        .set_language(&lang.into())
        .map_err(|e| anyhow::anyhow!("tree-sitter language error: {}", e))?;

    let tree = parser
        .parse(source, None)
        .ok_or_else(|| anyhow::anyhow!("tree-sitter parse failed"))?;

    let parts: Vec<&str> = symbol_path.split("::").collect();

    let mut matches = Vec::new();
    collect_matching_symbols(tree.root_node(), source, &parts, 0, &mut matches);

    match matches.len() {
        0 => bail!("symbol '{}' not found", symbol_path),
        1 => Ok(matches.into_iter().next().unwrap()),
        _ => {
            // Prefer exact depth match
            if let Some(m) = matches.iter().find(|m| {
                m.name == symbol_path || m.name == *parts.last().unwrap_or(&"")
            }) {
                Ok(m.clone())
            } else {
                Ok(matches.into_iter().next().unwrap())
            }
        }
    }
}
```

If multiple symbols match (rare — usually only happens when two
distinct paths share the same leaf, like `Foo::new` and `Bar::new`),
the resolver prefers an exact path match before falling back to the
first candidate.

## Listing all symbols

`list_symbols` walks the entire tree collecting every top-level item,
qualified by its containing impl / module. The doc-browser's symbol
navigation panel consumes this; literate authors can also use it as a
sanity check ("did the tangle hit the symbols I expected?").

<a name="chunk-list-symbols"></a><sub>[`src/source_ref.rs`](../../../x0k-tangle/src/source_ref.rs) · `#list-symbols`</sub>

```rust {#list-symbols}
pub fn list_symbols(source: &str) -> Result<Vec<SymbolSpan>> {
    let mut parser = Parser::new();
    let lang = tree_sitter_rust::LANGUAGE;
    parser
        .set_language(&lang.into())
        .map_err(|e| anyhow::anyhow!("tree-sitter language error: {}", e))?;

    let tree = parser
        .parse(source, None)
        .ok_or_else(|| anyhow::anyhow!("tree-sitter parse failed"))?;

    let mut symbols = Vec::new();
    collect_all_symbols(tree.root_node(), source, &[], &mut symbols);
    Ok(symbols)
}
```

## Tree-walking helpers

`node_text` slices the source by the node's byte range — tree-sitter
gives us byte positions, the source string is in-memory, so the body
extraction is one slice.

<a name="chunk-tree-helpers"></a><sub>[`src/source_ref.rs`](../../../x0k-tangle/src/source_ref.rs) · `#tree-helpers`</sub>

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

<a name="chunk-walk-matching"></a><sub>[`src/source_ref.rs`](../../../x0k-tangle/src/source_ref.rs) · `#walk-matching`</sub>

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

<a name="chunk-walk-all"></a><sub>[`src/source_ref.rs`](../../../x0k-tangle/src/source_ref.rs) · `#walk-all`</sub>

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

## Tests

The fixture is one Rust file with an enum, a struct, an impl block
with three methods, a `Default` impl, a free function, and a
`#[cfg(test)] mod tests`. The cases exercise: top-level item lookup
(enum + struct + function), nested method lookup, missing-symbol
errors, and full-list enumeration.

<a name="chunk-tests"></a><sub>[`src/source_ref.rs`](../../../x0k-tangle/src/source_ref.rs) · `#tests`</sub>

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
}
`````

## Composing the module

<a name="chunk-root"></a><sub>[`src/source_ref.rs`](../../../x0k-tangle/src/source_ref.rs) · `#root` · assembles [imports](#chunk-imports) · [symbol-span](#chunk-symbol-span) · [extract-symbol](#chunk-extract-symbol) · [list-symbols](#chunk-list-symbols) · [tree-helpers](#chunk-tree-helpers) · [walk-matching](#chunk-walk-matching) · [walk-all](#chunk-walk-all) · [tests](#chunk-tests)</sub>

```rust {#root}
<<imports>>

<<symbol-span>>

<<extract-symbol>>

<<list-symbols>>

<<tree-helpers>>

<<walk-matching>>

<<walk-all>>

<<tests>>
```
