---
x0k:
  format: folio/v1
  id: x0k:implementation/syntax/tokenizer
  type: implementation
  status: draft
  summary: Source text to a flat list of (byte range, kind) spans and nothing further, so a native presenter and a web presenter share one classification and disagree only about presentation.
  concerns: [syntax-highlighting, tree-sitter, tokens, presentation, rendering]
  tangle:
    crate: x0k-syntax
    root: src/lib.rs
  edges:
    implements:
      - x0k:design/literate-programming
    cites:
      - x0k:implementation/tangle/weave
      - x0k:design/representation-axes
---

# Tokens are not colors

Every surface that shows code — the native editor, the woven HTML a tangle
produces, the web app — wants syntax highlighting, and each has its own idea
of what a keyword looks like. If the tokenizer chose the color, every
consumer would either inherit one theme or re-tokenize. So this crate stops
one step short: it maps source text to a flat list of `(byte range,
TokenKind)` spans and knows nothing about colors, themes, fonts, or HTML. A
native presenter resolves a `TokenKind` to a theme color; a web presenter
resolves it to a CSS class through `css_class`. The classification is shared;
the presentation is not.

The carried example is the Rust line `fn main() { let x = 42; }`. Tokenized,
it yields `fn` and `let` as keywords, `main` as a function name (its parent
node is a `function_item`), `42` as a number, and the braces and semicolon
as punctuation. A native surface paints those five kinds five colors; the
weaver wraps each in `<span class="tok-…">`; the ranges are the same.

## Feature gating

The tree-sitter grammars for JSON, Rust, Python, TypeScript and TSX compile
in behind the `syntax-highlight` feature, on by default. With the feature
off, `highlight` returns `None` for every input and the grammar crates are
not built at all — a consumer that only needs the `TokenKind` vocabulary
(or a build that cannot afford five C grammars) pays nothing.

<a name="chunk-module-doc"></a><sub>[`src/lib.rs`](../../../x0k-syntax/src/lib.rs) · `#module-doc`</sub>

```rust {#module-doc}
//! Pure tree-sitter syntax tokenizer.
//!
//! Maps source code to a flat list of semantic [`HighlightedToken`] spans
//! (`byte range + TokenKind`). This crate has **no rendering dependencies** —
//! it knows nothing about colors, themes, fonts, or HTML. Consumers map
//! [`TokenKind`] to their own presentation:
//!
//! - A **native** presenter resolves `TokenKind` to a theme color.
//! - A **web** presenter (such as the HTML the `x0k-tangle` weave emits)
//!   resolves it to a CSS class via [`css_class`].
//!
//! Tree-sitter grammars for JSON, Rust, Python, TypeScript and TSX are
//! compiled in behind the `syntax-highlight` feature (default on). With the
//! feature off, [`highlight`] always returns `None` and the grammar crates
//! are not built.
//!
//! The grammars are pinned to tree-sitter 0.24 (language ABI 14). A
//! consumer that links its own tree-sitter grammars must bump in lockstep
//! with this crate: mixing ABI versions fails at `set_language`.

use std::ops::Range;
```

## The shared vocabulary

A `Language` is parsed from a code-fence info string, accepting the common
aliases. TypeScript's grammar is a superset of JavaScript and TSX's of JSX,
so the JS aliases route to the TS grammars rather than needing grammars of
their own.

<a name="chunk-language"></a><sub>[`src/lib.rs`](../../../x0k-syntax/src/lib.rs) · `#language`</sub>

```rust {#language}
/// Supported languages for syntax highlighting.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Language {
    Json,
    Rust,
    Python,
    /// TypeScript (also used for plain JavaScript).
    Typescript,
    /// TSX (also used for JSX).
    Tsx,
}

impl Language {
    /// Parse a language from a code-fence info string.
    ///
    /// Accepts common variations like "rust"/"rs", "python"/"py",
    /// "typescript"/"ts", "tsx", "javascript"/"js", "jsx".
    ///
    /// `None` for anything else — an info string naming a language with no
    /// grammar here is an ordinary, expected case (a fence tagged `text`,
    /// or a language nobody has added yet), so this stays an inherent
    /// `Option` lookup rather than `FromStr`. Every caller writes
    /// `.and_then(Language::from_str)` over that `Option`; the trait would
    /// force a `Result` and an error type carrying nothing.
    #[allow(clippy::should_implement_trait)]
    pub fn from_str(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "json" => Some(Self::Json),
            "rust" | "rs" => Some(Self::Rust),
            "python" | "py" => Some(Self::Python),
            // The TypeScript grammar is a superset that also parses JavaScript.
            "typescript" | "ts" | "javascript" | "js" => Some(Self::Typescript),
            // The TSX grammar additionally parses JSX.
            "tsx" | "jsx" => Some(Self::Tsx),
            _ => None,
        }
    }
}
```

The token kinds are the contract every presenter agrees on. `Default` is the
fallback a presenter paints in its ordinary code color.

<a name="chunk-token-kind"></a><sub>[`src/lib.rs`](../../../x0k-syntax/src/lib.rs) · `#token-kind`</sub>

```rust {#token-kind}
/// Token types for syntax highlighting.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TokenKind {
    /// Keywords: let, fn, if, for, true, false, null
    Keyword,
    /// String literals: "hello", 'c'
    String,
    /// Numeric literals: 42, 3.14
    Number,
    /// Comments: // comment, /* block */
    Comment,
    /// Punctuation: {}[]():;,
    Punctuation,
    /// Operators: = + - * / < > !
    Operator,
    /// Variable names
    Identifier,
    /// JSON keys, struct fields
    Property,
    /// Type names
    Type,
    /// Function names
    Function,
    /// Fallback - uses default code color
    Default,
}

/// A highlighted token with its byte range in the source.
#[derive(Debug, Clone)]
pub struct HighlightedToken {
    /// Byte range in the source code.
    pub range: Range<usize>,
    /// Token classification for coloring.
    pub kind: TokenKind,
}

impl HighlightedToken {
    /// Create a new highlighted token.
    pub fn new(range: Range<usize>, kind: TokenKind) -> Self {
        Self { range, kind }
    }
}
```

`css_class` is the one presentation-adjacent thing here, and it is still
only a name: the matching CSS lives in whichever surface consumes it.

<a name="chunk-css-class"></a><sub>[`src/lib.rs`](../../../x0k-syntax/src/lib.rs) · `#css-class`</sub>

```rust {#css-class}
/// The stable CSS class name for a token kind, e.g. `TokenKind::Keyword =>
/// "tok-keyword"`. This is the shared class-name contract every HTML/web
/// presenter agrees on; the matching CSS lives in the consuming surface's
/// stylesheet. Native presenters ignore this and map `TokenKind` straight
/// to a color.
pub fn css_class(kind: TokenKind) -> &'static str {
    match kind {
        TokenKind::Keyword => "tok-keyword",
        TokenKind::String => "tok-string",
        TokenKind::Number => "tok-number",
        TokenKind::Comment => "tok-comment",
        TokenKind::Punctuation => "tok-punctuation",
        TokenKind::Operator => "tok-operator",
        TokenKind::Identifier => "tok-identifier",
        TokenKind::Property => "tok-property",
        TokenKind::Type => "tok-type",
        TokenKind::Function => "tok-function",
        TokenKind::Default => "tok-default",
    }
}
```

## The entry point

`highlight` dispatches on language. The two definitions are `cfg`-gated
mirror images: with the feature off, the signature survives and the body is
`None`, so callers compile either way.

<a name="chunk-highlight"></a><sub>[`src/lib.rs`](../../../x0k-syntax/src/lib.rs) · `#highlight`</sub>

```rust {#highlight}
/// Highlight code, returning token ranges.
///
/// Returns `None` if the language is not supported or highlighting fails.
/// When the `syntax-highlight` feature is disabled, always returns `None`.
#[cfg(feature = "syntax-highlight")]
pub fn highlight(code: &str, language: Language) -> Option<Vec<HighlightedToken>> {
    match language {
        Language::Json => highlight_json(code),
        Language::Rust => highlight_rust(code),
        Language::Python => highlight_python(code),
        Language::Typescript => highlight_typescript(code, false),
        Language::Tsx => highlight_typescript(code, true),
    }
}

/// Highlight code - no-op when feature is disabled.
#[cfg(not(feature = "syntax-highlight"))]
pub fn highlight(_code: &str, _language: Language) -> Option<Vec<HighlightedToken>> {
    None
}
```

## One walk per grammar

Each grammar gets the same shape: build a parser, set the language, parse,
then recurse over the concrete syntax tree pushing a token for every node
whose kind maps to a `TokenKind`. Node kinds are tree-sitter's names, so the
match arms are grammar-specific strings. The recursion visits every node —
a `string` node and its child `string_content` may both push — and pushes in
tree order, which for a presenter that only reads ranges is what it needs.

<a name="chunk-section-marker"></a><sub>[`src/lib.rs`](../../../x0k-syntax/src/lib.rs) · `#section-marker`</sub>

```rust {#section-marker}
// ============================================================================
// Language-specific implementations
// ============================================================================
```

JSON distinguishes a key from a string value by position: a `string` whose
parent is a `pair` and which is that pair's first child is a `Property`.

<a name="chunk-json"></a><sub>[`src/lib.rs`](../../../x0k-syntax/src/lib.rs) · `#json`</sub>

```rust {#json}
#[cfg(feature = "syntax-highlight")]
fn highlight_json(code: &str) -> Option<Vec<HighlightedToken>> {
    use tree_sitter::Parser;

    let mut parser = Parser::new();
    let language = tree_sitter_json::LANGUAGE.into();
    if let Err(e) = parser.set_language(&language) {
        tracing::warn!(?e, "highlight_json: failed to set language");
        return None;
    }

    let tree = match parser.parse(code, None) {
        Some(t) => t,
        None => {
            tracing::warn!("highlight_json: parse returned None");
            return None;
        }
    };
    let root = tree.root_node();

    let mut tokens = Vec::new();
    collect_json_tokens(&root, &mut tokens);
    tracing::debug!(token_count = tokens.len(), "highlight_json: success");
    Some(tokens)
}

#[cfg(feature = "syntax-highlight")]
fn collect_json_tokens(node: &tree_sitter::Node, tokens: &mut Vec<HighlightedToken>) {
    let kind = match node.kind() {
        // JSON-specific node types
        "string" => {
            // Check if this is a property key (parent is "pair" and we're the first child)
            if let Some(parent) = node.parent() {
                if parent.kind() == "pair" {
                    if let Some(first_child) = parent.child(0) {
                        if first_child.id() == node.id() {
                            Some(TokenKind::Property)
                        } else {
                            Some(TokenKind::String)
                        }
                    } else {
                        Some(TokenKind::String)
                    }
                } else {
                    Some(TokenKind::String)
                }
            } else {
                Some(TokenKind::String)
            }
        }
        "number" => Some(TokenKind::Number),
        "true" | "false" | "null" => Some(TokenKind::Keyword),
        "{" | "}" | "[" | "]" | ":" | "," => Some(TokenKind::Punctuation),
        _ => None,
    };

    if let Some(kind) = kind {
        let range = node.byte_range();
        tokens.push(HighlightedToken::new(range, kind));
    }

    // Recurse into children
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        collect_json_tokens(&child, tokens);
    }
}
```

Rust classifies a plain `identifier` as a `Function` when its parent is a
function item or a call; everything else falls to the grammar's named
literal, comment, and type nodes.

<a name="chunk-rust"></a><sub>[`src/lib.rs`](../../../x0k-syntax/src/lib.rs) · `#rust`</sub>

```rust {#rust}
#[cfg(feature = "syntax-highlight")]
fn highlight_rust(code: &str) -> Option<Vec<HighlightedToken>> {
    use tree_sitter::Parser;

    let mut parser = Parser::new();
    let language = tree_sitter_rust::LANGUAGE.into();
    if let Err(e) = parser.set_language(&language) {
        tracing::warn!(?e, "highlight_rust: failed to set language");
        return None;
    }

    let tree = match parser.parse(code, None) {
        Some(t) => t,
        None => {
            tracing::warn!("highlight_rust: parse returned None");
            return None;
        }
    };
    let root = tree.root_node();

    let mut tokens = Vec::new();
    collect_rust_tokens(&root, &mut tokens);
    tracing::debug!(token_count = tokens.len(), "highlight_rust: success");
    Some(tokens)
}

#[cfg(feature = "syntax-highlight")]
fn collect_rust_tokens(node: &tree_sitter::Node, tokens: &mut Vec<HighlightedToken>) {
    let kind = match node.kind() {
        // Keywords
        "let" | "mut" | "fn" | "pub" | "struct" | "enum" | "impl" | "trait" | "use" | "mod"
        | "if" | "else" | "match" | "for" | "while" | "loop" | "return" | "break" | "continue"
        | "const" | "static" | "type" | "where" | "as" | "in" | "ref" | "self" | "Self"
        | "super" | "crate" | "async" | "await" | "dyn" | "move" | "unsafe" | "extern" => {
            Some(TokenKind::Keyword)
        }
        "true" | "false" => Some(TokenKind::Keyword),

        // Strings and characters
        "string_literal" | "raw_string_literal" | "char_literal" => Some(TokenKind::String),

        // Numbers
        "integer_literal" | "float_literal" => Some(TokenKind::Number),

        // Comments
        "line_comment" | "block_comment" => Some(TokenKind::Comment),

        // Types
        "type_identifier" | "primitive_type" => Some(TokenKind::Type),

        // Functions
        "identifier" if is_function_name(node) => Some(TokenKind::Function),

        // Field access
        "field_identifier" => Some(TokenKind::Property),

        // Punctuation
        "{" | "}" | "[" | "]" | "(" | ")" | ";" | "," | "::" | ":" | "->" | "=>" => {
            Some(TokenKind::Punctuation)
        }

        // Operators
        "=" | "+" | "-" | "*" | "/" | "%" | "&" | "|" | "^" | "!" | "<" | ">" | "==" | "!="
        | "<=" | ">=" | "&&" | "||" | "+=" | "-=" | "*=" | "/=" | ".." | "..=" | "?" => {
            Some(TokenKind::Operator)
        }

        _ => None,
    };

    if let Some(kind) = kind {
        let range = node.byte_range();
        tokens.push(HighlightedToken::new(range, kind));
    }

    // Recurse into children
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        collect_rust_tokens(&child, tokens);
    }
}

#[cfg(feature = "syntax-highlight")]
fn is_function_name(node: &tree_sitter::Node) -> bool {
    if let Some(parent) = node.parent() {
        matches!(
            parent.kind(),
            "function_item" | "call_expression" | "method_call_expression"
        )
    } else {
        false
    }
}
```

Python's walk is the same shape with the `?` operator in place of the logged
failure branches — a missing grammar or failed parse falls through to `None`
either way.

<a name="chunk-python"></a><sub>[`src/lib.rs`](../../../x0k-syntax/src/lib.rs) · `#python`</sub>

```rust {#python}
#[cfg(feature = "syntax-highlight")]
fn highlight_python(code: &str) -> Option<Vec<HighlightedToken>> {
    use tree_sitter::Parser;

    let mut parser = Parser::new();
    let language = tree_sitter_python::LANGUAGE.into();
    parser.set_language(&language).ok()?;

    let tree = parser.parse(code, None)?;
    let root = tree.root_node();

    let mut tokens = Vec::new();
    collect_python_tokens(&root, &mut tokens);
    Some(tokens)
}

#[cfg(feature = "syntax-highlight")]
fn collect_python_tokens(node: &tree_sitter::Node, tokens: &mut Vec<HighlightedToken>) {
    let kind = match node.kind() {
        // Keywords
        "def" | "class" | "if" | "elif" | "else" | "for" | "while" | "try" | "except"
        | "finally" | "with" | "as" | "import" | "from" | "return" | "yield" | "raise"
        | "break" | "continue" | "pass" | "lambda" | "and" | "or" | "not" | "in" | "is"
        | "global" | "nonlocal" | "assert" | "del" | "async" | "await" => Some(TokenKind::Keyword),
        "true" | "false" | "none" | "True" | "False" | "None" => Some(TokenKind::Keyword),

        // Strings
        "string" | "string_start" | "string_content" | "string_end" => Some(TokenKind::String),

        // Numbers
        "integer" | "float" => Some(TokenKind::Number),

        // Comments
        "comment" => Some(TokenKind::Comment),

        // Functions
        "identifier" if is_python_function_name(node) => Some(TokenKind::Function),

        // Attributes (like field access)
        "attribute" => Some(TokenKind::Property),

        // Punctuation
        "(" | ")" | "[" | "]" | "{" | "}" | ":" | "," | "." | "->" => Some(TokenKind::Punctuation),

        // Operators
        "=" | "+" | "-" | "*" | "/" | "//" | "%" | "**" | "@" | "&" | "|" | "^" | "~" | "<"
        | ">" | "<=" | ">=" | "==" | "!=" | "+=" | "-=" | "*=" | "/=" | "//=" | "%=" | "**="
        | "&=" | "|=" | "^=" => Some(TokenKind::Operator),

        _ => None,
    };

    if let Some(kind) = kind {
        let range = node.byte_range();
        tokens.push(HighlightedToken::new(range, kind));
    }

    // Recurse into children
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        collect_python_tokens(&child, tokens);
    }
}

#[cfg(feature = "syntax-highlight")]
fn is_python_function_name(node: &tree_sitter::Node) -> bool {
    if let Some(parent) = node.parent() {
        matches!(parent.kind(), "function_definition" | "call")
    } else {
        false
    }
}
```

TypeScript and TSX share one walk; the flag only selects which grammar to
load. A JSX element name is painted as a `Type` (so `<Component/>` reads like
a type, as it is), and JSX attribute names ride the `property_identifier`
arm.

<a name="chunk-typescript"></a><sub>[`src/lib.rs`](../../../x0k-syntax/src/lib.rs) · `#typescript`</sub>

```rust {#typescript}
#[cfg(feature = "syntax-highlight")]
fn highlight_typescript(code: &str, tsx: bool) -> Option<Vec<HighlightedToken>> {
    use tree_sitter::Parser;

    let mut parser = Parser::new();
    let language = if tsx {
        tree_sitter_typescript::LANGUAGE_TSX.into()
    } else {
        tree_sitter_typescript::LANGUAGE_TYPESCRIPT.into()
    };
    if let Err(e) = parser.set_language(&language) {
        tracing::warn!(?e, tsx, "highlight_typescript: failed to set language");
        return None;
    }

    let tree = match parser.parse(code, None) {
        Some(t) => t,
        None => {
            tracing::warn!(tsx, "highlight_typescript: parse returned None");
            return None;
        }
    };
    let root = tree.root_node();

    let mut tokens = Vec::new();
    collect_ts_tokens(&root, &mut tokens);
    tracing::debug!(
        token_count = tokens.len(),
        tsx,
        "highlight_typescript: success"
    );
    Some(tokens)
}

#[cfg(feature = "syntax-highlight")]
fn collect_ts_tokens(node: &tree_sitter::Node, tokens: &mut Vec<HighlightedToken>) {
    let kind = match node.kind() {
        // Keywords (anonymous literal nodes in the grammar)
        "const" | "let" | "var" | "function" | "return" | "if" | "else" | "for" | "while"
        | "do" | "switch" | "case" | "default" | "break" | "continue" | "class" | "interface"
        | "type" | "enum" | "namespace" | "module" | "import" | "export" | "from" | "as"
        | "extends" | "implements" | "new" | "delete" | "typeof" | "instanceof" | "in" | "of"
        | "void" | "async" | "await" | "yield" | "throw" | "try" | "catch" | "finally"
        | "public" | "private" | "protected" | "readonly" | "static" | "abstract" | "declare"
        | "get" | "set" | "keyof" | "infer" | "satisfies" | "is" => Some(TokenKind::Keyword),
        "true" | "false" | "null" | "undefined" => Some(TokenKind::Keyword),

        // Strings (and template literals / regex)
        "string" | "template_string" | "string_fragment" | "regex" => Some(TokenKind::String),

        // Numbers
        "number" => Some(TokenKind::Number),

        // Comments
        "comment" => Some(TokenKind::Comment),

        // Types
        "type_identifier" | "predefined_type" => Some(TokenKind::Type),

        // Functions
        "identifier" if is_ts_function_name(node) => Some(TokenKind::Function),

        // JSX element names render as types (e.g. <Component/>, <div/>)
        "identifier" if is_jsx_tag_name(node) => Some(TokenKind::Type),

        // Object keys, member access, JSX attribute names
        "property_identifier" | "shorthand_property_identifier" => Some(TokenKind::Property),

        // Punctuation
        "{" | "}" | "[" | "]" | "(" | ")" | ";" | "," | "." | ":" | "?." | "=>" | "<" | ">"
        | "</" | "/>" => Some(TokenKind::Punctuation),

        // Operators
        "=" | "+" | "-" | "*" | "/" | "%" | "**" | "&" | "|" | "^" | "~" | "!" | "==" | "==="
        | "!=" | "!==" | "<=" | ">=" | "&&" | "||" | "??" | "+=" | "-=" | "*=" | "/=" | "%="
        | "?" | "..." => Some(TokenKind::Operator),

        _ => None,
    };

    if let Some(kind) = kind {
        let range = node.byte_range();
        tokens.push(HighlightedToken::new(range, kind));
    }

    // Recurse into children
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        collect_ts_tokens(&child, tokens);
    }
}

#[cfg(feature = "syntax-highlight")]
fn is_ts_function_name(node: &tree_sitter::Node) -> bool {
    if let Some(parent) = node.parent() {
        matches!(
            parent.kind(),
            "function_declaration"
                | "function_expression"
                | "generator_function_declaration"
                | "call_expression"
                | "method_definition"
                | "function_signature"
        )
    } else {
        false
    }
}

#[cfg(feature = "syntax-highlight")]
fn is_jsx_tag_name(node: &tree_sitter::Node) -> bool {
    if let Some(parent) = node.parent() {
        matches!(
            parent.kind(),
            "jsx_opening_element" | "jsx_closing_element" | "jsx_self_closing_element"
        )
    } else {
        false
    }
}
```

## Tests

The feature-off tests pin the language table and the class contract; the
feature-on tests pin one classification decision per grammar — the JSON key,
the Rust keywords, the TS type annotation, the JSX tag.

<a name="chunk-tests"></a><sub>[`src/lib.rs`](../../../x0k-syntax/src/lib.rs) · `#tests`</sub>

```rust {#tests}
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_language_from_str() {
        assert_eq!(Language::from_str("json"), Some(Language::Json));
        assert_eq!(Language::from_str("JSON"), Some(Language::Json));
        assert_eq!(Language::from_str("rust"), Some(Language::Rust));
        assert_eq!(Language::from_str("rs"), Some(Language::Rust));
        assert_eq!(Language::from_str("python"), Some(Language::Python));
        assert_eq!(Language::from_str("py"), Some(Language::Python));
        assert_eq!(Language::from_str("typescript"), Some(Language::Typescript));
        assert_eq!(Language::from_str("ts"), Some(Language::Typescript));
        assert_eq!(Language::from_str("js"), Some(Language::Typescript));
        assert_eq!(Language::from_str("tsx"), Some(Language::Tsx));
        assert_eq!(Language::from_str("jsx"), Some(Language::Tsx));
        assert_eq!(Language::from_str("unknown"), None);
    }

    #[test]
    fn test_css_class_distinct() {
        assert_eq!(css_class(TokenKind::Keyword), "tok-keyword");
        assert_ne!(css_class(TokenKind::Keyword), css_class(TokenKind::String));
    }

    #[cfg(feature = "syntax-highlight")]
    #[test]
    fn test_highlight_json() {
        let code = r#"{"key": "value", "num": 42, "flag": true}"#;
        let tokens = highlight(code, Language::Json).expect("should highlight JSON");
        assert!(!tokens.is_empty());

        let property_tokens: Vec<_> = tokens
            .iter()
            .filter(|t| t.kind == TokenKind::Property)
            .collect();
        assert!(!property_tokens.is_empty(), "should have property tokens");

        let number_tokens: Vec<_> = tokens
            .iter()
            .filter(|t| t.kind == TokenKind::Number)
            .collect();
        assert_eq!(number_tokens.len(), 1, "should have one number token");

        let keyword_tokens: Vec<_> = tokens
            .iter()
            .filter(|t| t.kind == TokenKind::Keyword)
            .collect();
        assert_eq!(
            keyword_tokens.len(),
            1,
            "should have one keyword token (true)"
        );
    }

    #[cfg(feature = "syntax-highlight")]
    #[test]
    fn test_highlight_rust() {
        let code = r#"fn main() { let x = 42; }"#;
        let tokens = highlight(code, Language::Rust).expect("should highlight Rust");
        assert!(!tokens.is_empty());

        let keyword_tokens: Vec<_> = tokens
            .iter()
            .filter(|t| t.kind == TokenKind::Keyword)
            .collect();
        assert!(
            keyword_tokens.len() >= 2,
            "should have at least fn and let keywords"
        );
    }

    #[cfg(feature = "syntax-highlight")]
    #[test]
    fn test_highlight_typescript() {
        let code = r#"const greeting: string = "hello"; function add(a: number) { return a; }"#;
        let tokens = highlight(code, Language::Typescript).expect("should highlight TS");
        assert!(!tokens.is_empty());

        let has_keyword = tokens.iter().any(|t| t.kind == TokenKind::Keyword);
        let has_string = tokens.iter().any(|t| t.kind == TokenKind::String);
        let has_type = tokens.iter().any(|t| t.kind == TokenKind::Type);
        assert!(
            has_keyword,
            "should classify const/function/return as keywords"
        );
        assert!(has_string, "should classify the string literal");
        assert!(has_type, "should classify the `string`/`number` types");
    }

    #[cfg(feature = "syntax-highlight")]
    #[test]
    fn test_highlight_tsx() {
        let code = r#"const App = () => <div className="x">{label}</div>;"#;
        let tokens = highlight(code, Language::Tsx).expect("should highlight TSX");
        assert!(!tokens.is_empty());
        // JSX tag name should be classified as a type, attribute as a property.
        let has_type = tokens.iter().any(|t| t.kind == TokenKind::Type);
        let has_property = tokens.iter().any(|t| t.kind == TokenKind::Property);
        assert!(has_type, "JSX element name should be a Type token");
        assert!(
            has_property,
            "JSX attribute name should be a Property token"
        );
    }

    #[cfg(not(feature = "syntax-highlight"))]
    #[test]
    fn test_highlight_returns_none_without_feature() {
        assert!(highlight("{}", Language::Json).is_none());
    }
}
```

## The file

<a name="chunk-root"></a><sub>[`src/lib.rs`](../../../x0k-syntax/src/lib.rs) · `#root` · assembles [module-doc](#chunk-module-doc) · [language](#chunk-language) · [token-kind](#chunk-token-kind) · [css-class](#chunk-css-class) · [highlight](#chunk-highlight) · [section-marker](#chunk-section-marker) · [json](#chunk-json) · [rust](#chunk-rust) · [python](#chunk-python) · [typescript](#chunk-typescript) · [tests](#chunk-tests)</sub>

```rust {#root}
<<module-doc>>

<<language>>

<<token-kind>>

<<css-class>>

<<highlight>>

<<section-marker>>

<<json>>

<<rust>>

<<python>>

<<typescript>>

<<tests>>
```

The classifications are deliberately shallow: no scope resolution, no
semantic types, no distinction between a local and a field beyond what the
parse tree names. That is the right depth for a presenter that paints
ranges, and it is what keeps five grammars fitting in one file without a
query language between them.
