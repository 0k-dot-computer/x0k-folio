---
x0k:
  format: folio/v1
  id: x0k:design/literate-programming#read-a-document-as-the-woven-artifact
  type: design
  status: proposed
  edges:
    transcludes:
      - x0k:design/literate-programming
---

### Read a document as the woven artifact

I read a literate document as a rendered whole — its prose and its code in one
continuous argument, code spans highlighted, chunks resolved where they are
referenced rather than where they happen to be defined. The reading order is
the one the author chose, not the one the compiler needs.

```yaml x0k:affordance
id: x0k:affordance/weave_a_document
actors: [human]
edges:
  enabledBy:
    - x0k:software-module/x0k-tangle
    - x0k:software-module/x0k-syntax
    - x0k:software-module/x0k-folio
  requires:
    - x0k:affordance/tangle_source_from_a_document
```

<picture><source media="(prefers-color-scheme: dark)" srcset="../../../../affordances/for-a-person-dark.svg"><img alt="for a person" src="../../../../affordances/for-a-person-light.svg" height="20"></picture> <picture><source media="(prefers-color-scheme: dark)" srcset="../../../../affordances/status-proven-dark.svg"><img alt="proven" src="../../../../affordances/status-proven-light.svg" height="16"></picture> *proven* · for a person · reachable through `cli` `x0k-tangle weave`, `sdk` `weave_chapter`, `sdk` `weave_html`

*realized in* [x0k-tangle: the crate and its CLI](../../../../knowledge/implementation/tangle/crate.md) · [Weaving literate documents into HTML](../../../../knowledge/implementation/tangle/weave.md) · [Weaving a chapter for a forge](../../../../knowledge/implementation/tangle/region-gfm.md)

*proven by* each test below, as its chapter tangles it and as it ran at projection.

<details><summary><code>the_weave_inverts_line_for_line</code> · passed · <a href="../../../../knowledge/implementation/tangle/region-gfm.md#chunk-tests">#tests</a> in Weaving a chapter for a forge</summary>

```rust
#[test]
fn the_weave_inverts_line_for_line() {
    assert_eq!(unweave_chapter(&woven()), CHAPTER);
}
```

</details>

<details><summary><code>the_woven_chapter_tangles_to_the_same_chunks</code> · passed · <a href="../../../../knowledge/implementation/tangle/region-gfm.md#chunk-tests">#tests</a> in Weaving a chapter for a forge</summary>

```rust
#[test]
fn the_woven_chapter_tangles_to_the_same_chunks() {
    let a = parse_document(CHAPTER).unwrap();
    let b = parse_document(&woven()).unwrap();
    assert_eq!(a.chunk_order, b.chunk_order);
    for name in &a.chunk_order {
        let (ca, cb) = (a.chunk_variants(name).unwrap(), b.chunk_variants(name).unwrap());
        assert_eq!(ca.len(), cb.len(), "{name}");
        for (x, y) in ca.iter().zip(cb.iter()) {
            assert_eq!(x.file_target, y.file_target, "{name}");
            assert_eq!(x.proves, y.proves, "{name}");
            let (bx, by): (Vec<&str>, Vec<&str>) =
                (x.bodies.iter().map(|b| b.text.as_str()).collect(), y.bodies.iter().map(|b| b.text.as_str()).collect());
            assert_eq!(bx, by, "{name}");
        }
    }
}
```

</details>

<details><summary><code>captions_say_file_name_proof_and_assembly_and_links_land</code> · passed · <a href="../../../../knowledge/implementation/tangle/region-gfm.md#chunk-tests">#tests</a> in Weaving a chapter for a forge</summary>

```rust
#[test]
fn captions_say_file_name_proof_and_assembly_and_links_land() {
    let w = woven();
    assert!(w.contains("<a name=\"chunk-parse\"></a><sub>[`src/lib.rs`](../../../demo-crate/src/lib.rs) · `#parse`</sub>\n\n```rust {#parse}\n"), "{w}");
    assert!(w.contains("<a name=\"chunk-root\"></a><sub>[`src/lib.rs`](../../../demo-crate/src/lib.rs) · `#root` · assembles [parse](#chunk-parse)</sub>\n\n```rust {#root}\n"), "once, and never the escaped ref: {w}");
    assert!(w.contains("<a name=\"chunk-parse-2\"></a><sub>[`src/lib.rs`](../../../demo-crate/src/lib.rs) · `#parse` · continues</sub>\n\n"), "{w}");
    assert!(w.contains("<a name=\"chunk-tests\"></a><sub>[`tests/proof.rs`](../../../demo-crate/tests/proof.rs) · `#tests` · proves [Read a line](../../../decisions/design/demo-design/read-a-line.md)</sub>\n\n"), "{w}");
    assert!(w.contains("Reads [first lines](../../wiki/first-lines.md \"x0k:wiki/first-lines\") and [elsewhere](x0k:wiki/elsewhere);\n"), "a carried link lands, an uncarried one is left: {w}");
    assert!(w.contains("see `[not a link](x0k:wiki/first-lines)` and [the design](../../../decisions/design/demo-design.md#read-a-line \"x0k:design/demo-design#read-a-line\").\n"), "{w}");
    assert!(!w.contains("chunk-shown"), "a fence inside a fence is not a chunk: {w}");
}
```

</details>

<details><summary><code>a_string_literal_naming_an_id_is_not_a_woven_line</code> · passed · <a href="../../../../knowledge/implementation/tangle/region-gfm.md#chunk-tests">#tests</a> in Weaving a chapter for a forge</summary>

```rust
#[test]
fn a_string_literal_naming_an_id_is_not_a_woven_line() {
    let (uri, aff) = links();
    let links = ChapterLinks { uri_to_rel: &uri, affordances: &aff };
    let chapter = CHAPTER.replace(
        "```rust {#parse}\n",
        "```rust {#parse}\nconst ID: &str = \"x0k:design/example\";\n",
    );
    let woven = weave_chapter(&chapter, "knowledge/implementation/demo/lines.md", Some("demo-crate"), &links)
        .expect("a literal is not a rewritten link");
    assert_eq!(unweave_chapter(&woven), chapter);
}
```

</details>

<details><summary><code>a_source_holding_a_woven_line_is_refused</code> · passed · <a href="../../../../knowledge/implementation/tangle/region-gfm.md#chunk-tests">#tests</a> in Weaving a chapter for a forge</summary>

```rust
#[test]
fn a_source_holding_a_woven_line_is_refused() {
    let (uri, aff) = links();
    let links = ChapterLinks { uri_to_rel: &uri, affordances: &aff };
    let err = weave_chapter(&woven(), "knowledge/implementation/demo/lines.md", Some("demo-crate"), &links)
        .expect_err("woven twice");
    assert!(err.to_string().contains("already holds a woven line"), "{err}");
}
```

</details>

<details><summary><code>the_affordance_section_carries_the_evidence_under_its_block</code> · passed · <a href="../../../../knowledge/implementation/tangle/region-gfm.md#chunk-tests">#tests</a> in Weaving a chapter for a forge</summary>

```rust
#[test]
fn the_affordance_section_carries_the_evidence_under_its_block() {
    let page = "---\nx0k:\n  format: folio/v1\n  id: x0k:design/demo-design#read-a-line\n  type: design\n---\n\n### Read a line\n\nI read a line.\n\n```yaml x0k:affordance\nid: x0k:affordance/read_a_line\nactors: [human]\n```\n\nAfter.\n";
    let ev = AffordanceEvidence {
        id: "x0k:affordance/read_a_line".to_string(),
        glyphs: "<img alt=\"proven\">".to_string(),
        status: "proven".to_string(),
        actors: "a person".to_string(),
        cues: vec![("cli".to_string(), "demo read".to_string())],
        chapters: vec![("Lines".to_string(), "../../../knowledge/implementation/demo/lines.md".to_string())],
        proofs: vec![ProofEvidence {
            test: "a_line_is_read".to_string(),
            outcome: Some("passed".to_string()),
            chapter: ("Lines".to_string(), "../../../knowledge/implementation/demo/lines.md".to_string()),
            chunk: "tests".to_string(),
            source: "#[test]\nfn a_line_is_read() {}\n".to_string(),
        }],
    };
    let out = weave_affordance_section(page, &[ev]);
    let expected = "```yaml x0k:affordance\nid: x0k:affordance/read_a_line\nactors: [human]\n```\n\n<img alt=\"proven\"> *proven* · for a person · reachable through `cli` `demo read`\n\n*realized in* [Lines](../../../knowledge/implementation/demo/lines.md)\n\n*proven by* each test below, as its chapter tangles it and as it ran at projection.\n\n<details><summary><code>a_line_is_read</code> · passed · <a href=\"../../../knowledge/implementation/demo/lines.md#chunk-tests\">#tests</a> in Lines</summary>\n\n```rust\n#[test]\nfn a_line_is_read() {}\n```\n\n</details>\n\n\nAfter.\n";
    assert!(out.ends_with(expected), "{out}");
    assert_eq!(weave_affordance_section(page, &[]), page, "no evidence, no change");
}
```

</details>

<details><summary><code>relative_links_climb_out_and_descend</code> · passed · <a href="../../../../knowledge/implementation/tangle/region-gfm.md#chunk-tests">#tests</a> in Weaving a chapter for a forge</summary>

```rust
#[test]
fn relative_links_climb_out_and_descend() {
    assert_eq!(relative_link("README.md", "knowledge/wiki/a.md"), "knowledge/wiki/a.md");
    assert_eq!(relative_link("knowledge/implementation/demo/lines.md", "knowledge/wiki/a.md"), "../../wiki/a.md");
    assert_eq!(relative_link("decisions/design/d/read.md", "demo-crate/src/lib.rs"), "../../../demo-crate/src/lib.rs");
    assert_eq!(relative_link("a/b.md", "a/c.md"), "c.md");
}
```

</details>

<details><summary><code>single_language_chunk_renders_without_tabs</code> · passed · <a href="../../../../knowledge/implementation/tangle/weave.md#chunk-tests">#tests</a> in Weaving literate documents into HTML</summary>

```rust
#[test]
fn single_language_chunk_renders_without_tabs() {
    let content = r#"# Test

```rust {#imports}
use std::io;
```
"#;
    let doc = parse_document(content).unwrap();
    let output = weave_html(content, &doc).unwrap();

    // Extract just the <article> body (skip <style> which contains class definitions)
    let body = output.html.split("<article").nth(1).unwrap();

    // Should have a normal chunk header with the lang badge
    assert!(body.contains("chunk-header"));
    assert!(body.contains("chunk-lang"));
    // Should NOT have tab markup in the body
    assert!(!body.contains("chunk-tabs"));
    assert!(!body.contains("chunk-tab-panel"));
}
```

</details>

<details><summary><code>multi_language_chunk_renders_tabs</code> · passed · <a href="../../../../knowledge/implementation/tangle/weave.md#chunk-tests">#tests</a> in Weaving literate documents into HTML</summary>

```rust
#[test]
fn multi_language_chunk_renders_tabs() {
    let content = r#"# Camera

```typescript {#zoom-toward}
zoomToward(sx: number, sy: number, delta: number) { }
```

```rust {#zoom-toward}
pub fn zoom_toward(&mut self, sx: f32, sy: f32, delta: f32) { }
```
"#;
    let doc = parse_document(content).unwrap();
    let output = weave_html(content, &doc).unwrap();

    // Should have tab container
    assert!(output.html.contains("chunk-tabs"));

    // Should have tab buttons for both languages
    assert!(output.html.contains("data-lang=\"typescript\""));
    assert!(output.html.contains("data-lang=\"rust\""));
    assert!(output.html.contains(">TypeScript</button>"));
    assert!(output.html.contains(">Rust</button>"));

    // Should have tab panels
    assert!(output.html.contains("chunk-tab-panel"));

    // First tab+panel should be active
    assert!(output.html.contains("chunk-tab active\" data-lang=\"typescript\""));
    assert!(output.html.contains("chunk-tab-panel active\" data-lang=\"typescript\""));

    // Both code bodies should be present
    assert!(output.html.contains("zoomToward"));
    assert!(output.html.contains("zoom_toward"));

    // Should only render ONE chunk div (not two separate chunks)
    let chunk_count = output.html.matches("<div class=\"chunk\">").count();
    assert_eq!(chunk_count, 1, "multi-lang chunk should render as a single tabbed chunk");
}
```

</details>

<details><summary><code>mixed_single_and_multi_lang_chunks</code> · passed · <a href="../../../../knowledge/implementation/tangle/weave.md#chunk-tests">#tests</a> in Weaving literate documents into HTML</summary>

```rust
#[test]
fn mixed_single_and_multi_lang_chunks() {
    let content = r#"# Mixed

```rust {#preamble}
use std::io;
```

```typescript {#render}
function render() {}
```

```rust {#render}
fn render() {}
```
"#;
    let doc = parse_document(content).unwrap();
    let output = weave_html(content, &doc).unwrap();

    // preamble is single-lang — no tabs, has chunk-lang badge
    assert!(output.html.contains("id=\"chunk-preamble\""));
    // render is multi-lang — has tabs
    assert!(output.html.contains("chunk-tabs"));

    // Two chunk divs total: one for preamble, one for tabbed render
    let chunk_count = output.html.matches("<div class=\"chunk\">").count();
    assert_eq!(chunk_count, 2);
}
```

</details>

<details><summary><code>params_block_renders_data_div</code> · passed · <a href="../../../../knowledge/implementation/tangle/weave.md#chunk-tests">#tests</a> in Weaving literate documents into HTML</summary>

```rust
#[test]
fn params_block_renders_data_div() {
    let content = r#"# Viz

```yaml x0k:params
- id: threshold_gap
display_name: Hysteresis Gap
description: Ratio between zoom_in and zoom_out thresholds
type: { kind: float, min: 0.1, max: 3.0, step: 0.1 }
default: 1.5
- id: num_levels
display_name: LOD Levels
description: Number of detail levels
type: { kind: uint, min: 2, max: 8 }
default: 4
```
"#;
    let doc = parse_document(content).unwrap();
    let output = weave_html(content, &doc).unwrap();

    // Should have param-panel-data div
    assert!(output.html.contains("param-panel-data"));
    assert!(output.html.contains("data-params="));
    // Should contain the param IDs in JSON
    assert!(output.html.contains("threshold_gap"));
    assert!(output.html.contains("num_levels"));
    // Should NOT render as a code block
    assert!(!output.html.contains("<pre><code"));
}
```

</details>

<details><summary><code>tab_buttons_have_onclick</code> · passed · <a href="../../../../knowledge/implementation/tangle/weave.md#chunk-tests">#tests</a> in Weaving literate documents into HTML</summary>

```rust
#[test]
fn tab_buttons_have_onclick() {
    let content = r#"# Tabs

```typescript {#example}
let x = 1;
```

```rust {#example}
let x = 1;
```
"#;
    let doc = parse_document(content).unwrap();
    let output = weave_html(content, &doc).unwrap();

    // onclick attributes should be present (for innerHTML compatibility)
    assert!(output.html.contains("onclick=\""));
}
```

</details>

<details><summary><code>rust_chunk_emits_highlight_spans</code> · passed · <a href="../../../../knowledge/implementation/tangle/weave.md#chunk-tests">#tests</a> in Weaving literate documents into HTML</summary>

```rust
#[test]
fn rust_chunk_emits_highlight_spans() {
    let content = r#"# Hl

```rust {#demo}
fn main() { let x = 42; }
```
"#;
    let doc = parse_document(content).unwrap();
    let output = weave_html(content, &doc).unwrap();

    // Keyword and number tokens are wrapped in tok-* spans.
    assert!(output.html.contains("<span class=\"tok-keyword\">fn</span>"));
    assert!(output.html.contains("<span class=\"tok-number\">42</span>"));
    // A <<ref>> on its own line is still an anchor, never highlighted.
}
```

</details>

<details><summary><code>classify_comment_kinds</code> · passed · <a href="../../../../knowledge/implementation/tangle/weave.md#chunk-tests">#tests</a> in Weaving literate documents into HTML</summary>

```rust
#[test]
fn classify_comment_kinds() {
    assert_eq!(classify_comment("/// a doc comment"), "doc");
    assert_eq!(classify_comment("//! inner doc"), "doc");
    assert_eq!(classify_comment("/** block doc */"), "doc");
    assert_eq!(classify_comment("// ── State ──"), "banner");
    assert_eq!(classify_comment("// ===== Foo ====="), "banner");
    assert_eq!(classify_comment("// -------"), "banner");
    assert_eq!(classify_comment("// SAFETY: invariant holds"), "tag");
    assert_eq!(classify_comment("// TODO(x): finish"), "tag");
    assert_eq!(classify_comment("// note: lowercase still a tag"), "tag");
    assert_eq!(classify_comment("// just a plain remark"), "aside");
    // A doc marker beats a divider in the same text.
    assert_eq!(classify_comment("/// ----- titled -----"), "doc");
}
```

</details>

<details><summary><code>comment_run_grouping_wraps_and_classifies</code> · passed · <a href="../../../../knowledge/implementation/tangle/weave.md#chunk-tests">#tests</a> in Weaving literate documents into HTML</summary>

```rust
#[test]
fn comment_run_grouping_wraps_and_classifies() {
    // A 4-line `///` doc run (lifted into a hover-card on the symbol it
    // describes), a banner line, a SAFETY tag line, and an aside, interleaved
    // with real code so the non-comment bytes can be checked for byte-identity.
    let content = r#"# Comments

```rust {#demo}
/// First doc line.
/// Second doc line.
/// Third doc line.
/// Fourth doc line.
fn f() {}
// ── Section ──
let a = 1;
// SAFETY: pointer is valid here
let b = 2;
// a plain aside
let c = 3;
```
"#;
    let doc = parse_document(content).unwrap();
    let output = weave_html(content, &doc).unwrap();
    let body = output.html.split("<article").nth(1).unwrap();
    let pre = body.split("<pre>").nth(1).unwrap().split("</pre>").next().unwrap();

    // The doc run is lifted OUT of the `<pre>`: the `///` text never appears
    // inside the code, and there is no inline comment-doc wrapper.
    assert!(!pre.contains("First doc line"));
    assert!(!pre.contains("comment-run comment-doc"));
    assert!(!pre.contains("comment-doc foldable"));

    // The described symbol `f` (the next code line) is wrapped in a
    // doc-symbol affordance carrying the run's data-doc id.
    assert!(pre.contains("<span class=\"doc-symbol\" data-doc=\"doc-0-0\">"));
    // Its hover-card payload is flushed right after `</pre>`, hidden, and
    // holds the rendered docstring prose.
    assert!(body.contains("</pre>\n<div class=\"doc-card\" data-doc=\"doc-0-0\" hidden>"));
    let card = body
        .split("data-doc=\"doc-0-0\" hidden>")
        .nth(1)
        .unwrap()
        .split("</div>")
        .next()
        .unwrap();
    assert!(card.contains("First doc line"));
    assert!(card.contains("Fourth doc line"));

    // Banner, tag, aside still render inline, single-line, never foldable.
    assert!(body.contains(
        "<span class=\"comment-run comment-banner\" data-comment-lines=\"1\">"
    ));
    // The tag run carries its canonical word as `data-tag` for chip colouring.
    assert!(body.contains(
        "<span class=\"comment-run comment-tag\" data-comment-lines=\"1\" data-tag=\"SAFETY\">"
    ));
    assert!(body.contains(
        "<span class=\"comment-run comment-aside\" data-comment-lines=\"1\">"
    ));
    assert!(!body.contains("comment-banner foldable"));
    assert!(!body.contains("comment-tag foldable"));

    // Inner per-token comment spans survive for the inline runs.
    assert!(body.contains("<span class=\"tok-comment\">"));

    // Non-comment code bytes are unchanged: the highlighted keyword/number
    // spans for the interleaved code are still present and untouched.
    assert!(body.contains("<span class=\"tok-keyword\">fn</span>"));
    assert!(body.contains("<span class=\"tok-keyword\">let</span>"));
    assert!(body.contains("<span class=\"tok-number\">1</span>"));
    assert!(body.contains("<span class=\"tok-number\">3</span>"));
    // The wrapper must not leak into non-comment lines: each inline run's
    // closing </span> is followed by the newline and the next code line's
    // own tokens — never another comment-run open.
    assert!(body.contains("</span>\n<span class=\"tok-keyword\">let</span>"));
}
```

</details>

<details><summary><code>render_doc_markdown_strips_markers_and_renders</code> · passed · <a href="../../../../knowledge/implementation/tangle/weave.md#chunk-tests">#tests</a> in Weaving literate documents into HTML</summary>

```rust
#[test]
fn render_doc_markdown_strips_markers_and_renders() {
    // Markers peeled, markdown rendered: inline code becomes <code>.
    let out = render_doc_markdown("/// a `code` ref");
    assert!(out.contains("<code>code</code>"), "got: {out}");
    assert!(!out.contains("///"));

    // Inner-doc and block markers peel too; **bold** renders.
    let out2 = render_doc_markdown("//! module **docs**");
    assert!(out2.contains("<strong>docs</strong>"));
    assert!(!out2.contains("//!"));
}
```

</details>

<details><summary><code>math_renders_to_mathml</code> · passed · <a href="../../../../knowledge/implementation/tangle/weave.md#chunk-tests">#tests</a> in Weaving literate documents into HTML</summary>

```rust
#[test]
fn math_renders_to_mathml() {
    // Inline `$…$` and display `$$…$$` become MathML; the `$` delimiters
    // are gone and the math is real markup (not html-escaped).
    let content = "# Math\n\nMass-energy: $E = mc^2$ and\n\n$$\\frac{1}{2}$$\n";
    let doc = parse_document(content).unwrap();
    let body = weave_html(content, &doc).unwrap().html;
    let body = body.split("<article").nth(1).unwrap();
    assert!(body.contains("<math"), "inline math → <math>: {body}");
    assert!(body.contains("display=\"block\""), "display math → block");
    assert!(!body.contains("$E = mc^2$"), "the $-delimiters are consumed");
    // <math> is emitted verbatim, not escaped into &lt;math&gt;.
    assert!(!body.contains("&lt;math"));
}
```

</details>

<details><summary><code>bad_math_falls_back_to_raw_tex</code> · passed · <a href="../../../../knowledge/implementation/tangle/weave.md#chunk-tests">#tests</a> in Weaving literate documents into HTML</summary>

```rust
#[test]
fn bad_math_falls_back_to_raw_tex() {
    // An unsupported macro must not panic or vanish — it lands in a
    // `.math-error` carrying the escaped raw TeX.
    // Unknown macro (latex2mathml returns Ok with an embedded parse-error
    // marker) → fallback, no broken MathML leaks through.
    let out = render_math("\\nonexistentmacro{x}", false);
    assert!(out.contains("class=\"math-error\""), "got: {out}");
    assert!(out.contains("nonexistentmacro"), "raw TeX preserved: {out}");
    assert!(!out.contains("PARSE ERROR"), "no broken-MathML marker leaks: {out}");
    // Hard syntax error (unbalanced braces) also falls back, no panic.
    let hard = render_math("\\frac{1}{", true);
    assert!(hard.contains("class=\"math-error\""), "got: {hard}");
    // A valid fragment renders to MathML inline.
    let ok = render_math("x^2", false);
    assert!(ok.contains("<math") && ok.contains("display=\"inline\""), "got: {ok}");
}
```

</details>

<details><summary><code>doc_comment_lifts_to_symbol_and_card</code> · passed · <a href="../../../../knowledge/implementation/tangle/weave.md#chunk-tests">#tests</a> in Weaving literate documents into HTML</summary>

```rust
#[test]
fn doc_comment_lifts_to_symbol_and_card() {
    // The canonical shape: `/// foo` over `pub fn bar()` →
    // doc-symbol on `bar`, a card payload with "foo", no `///` in the <pre>.
    let content = r#"# Doc

```rust {#demo}
/// foo
pub fn bar() {}
```
"#;
    let doc = parse_document(content).unwrap();
    let output = weave_html(content, &doc).unwrap();
    let body = output.html.split("<article").nth(1).unwrap();
    let pre = body.split("<pre>").nth(1).unwrap().split("</pre>").next().unwrap();

    // No `///` in the code.
    assert!(!pre.contains("///"));
    assert!(!pre.contains("foo"));
    // `bar` carries the doc-symbol affordance.
    assert!(pre.contains("data-doc=\"doc-0-0\">"));
    assert!(pre.contains("doc-symbol"));
    assert!(pre.contains("bar"));
    // The card holds the rendered docstring and sits after </pre>, hidden.
    assert!(body.contains("<div class=\"doc-card\" data-doc=\"doc-0-0\" hidden>"));
    assert!(body.contains(">foo</"));
}
```

</details>

<details><summary><code>doc_comment_skips_attribute_to_reach_symbol</code> · passed · <a href="../../../../knowledge/implementation/tangle/weave.md#chunk-tests">#tests</a> in Weaving literate documents into HTML</summary>

```rust
#[test]
fn doc_comment_skips_attribute_to_reach_symbol() {
    // The common Rust shape: a docstring, an attribute, then the item. The
    // symbol is the `fn`, NOT the attribute — the heuristic must skip
    // `#[wasm_bindgen]` and anchor `bar`.
    let content = r#"# Doc

```rust {#demo}
/// foo
#[wasm_bindgen]
pub fn bar() {}
```
"#;
    let doc = parse_document(content).unwrap();
    let output = weave_html(content, &doc).unwrap();
    let body = output.html.split("<article").nth(1).unwrap();
    let pre = body.split("<pre>").nth(1).unwrap().split("</pre>").next().unwrap();
    assert!(!pre.contains("///"), "docstring lifted out of the code");
    // The attribute stays in the code; the doc-symbol lands on `bar`, not on
    // the attribute name.
    assert!(pre.contains("#[wasm_bindgen]") || pre.contains("wasm_bindgen"));
    assert!(pre.contains("doc-symbol"));
    // The doc-symbol wraps `bar` (the declaration), and a card exists.
    assert!(body.contains("<div class=\"doc-card\" data-doc=\"doc-0-0\" hidden>"));
    // Confirm the symbol span is on `bar` by checking the doc-symbol span text.
    let sym = pre.split("doc-symbol").nth(1).unwrap_or("");
    assert!(sym.contains("bar"), "doc-symbol anchors the declaration, not the attribute");
}
```

</details>

<details><summary><code>inner_doc_anchors_to_chunk_header</code> · passed · <a href="../../../../knowledge/implementation/tangle/weave.md#chunk-tests">#tests</a> in Weaving literate documents into HTML</summary>

```rust
#[test]
fn inner_doc_anchors_to_chunk_header() {
    // `//!` documents the module → the chunk-name in the header carries the
    // data-doc, the card is emitted, and the `//!` text is lifted out.
    let content = r#"# Inner

```rust {#demo}
//! module overview
fn f() {}
```
"#;
    let doc = parse_document(content).unwrap();
    let output = weave_html(content, &doc).unwrap();
    let body = output.html.split("<article").nth(1).unwrap();

    // The chunk-name element carries data-doc for the module card.
    assert!(body.contains("<span class=\"chunk-name doc-symbol\" data-doc=\"doc-0-0\">demo</span>"));
    // The card exists and the `//!` text is not inside the <pre>.
    let pre = body.split("<pre>").nth(1).unwrap().split("</pre>").next().unwrap();
    assert!(!pre.contains("module overview"));
    assert!(body.contains("<div class=\"doc-card\" data-doc=\"doc-0-0\" hidden>"));
    assert!(body.contains("module overview"));
}
```

</details>

<details><summary><code>doc_comment_without_symbol_falls_back_inline</code> · passed · <a href="../../../../knowledge/implementation/tangle/weave.md#chunk-tests">#tests</a> in Weaving literate documents into HTML</summary>

```rust
#[test]
fn doc_comment_without_symbol_falls_back_inline() {
    // A doc comment with no following declaration keeps rendering inline
    // (nothing lost), and emits no card.
    let content = r#"# Fallback

```rust {#demo}
fn f() {
/// orphan doc
}
```
"#;
    let doc = parse_document(content).unwrap();
    let output = weave_html(content, &doc).unwrap();
    let body = output.html.split("<article").nth(1).unwrap();
    let pre = body.split("<pre>").nth(1).unwrap().split("</pre>").next().unwrap();

    // Rendered inline as a comment-doc run; no card emitted.
    assert!(pre.contains("comment-run comment-doc"));
    assert!(pre.contains("orphan doc"));
    assert!(!body.contains("doc-card"));
}
```

</details>

<details><summary><code>comment_run_preserves_indentation_bytes</code> · passed · <a href="../../../../knowledge/implementation/tangle/weave.md#chunk-tests">#tests</a> in Weaving literate documents into HTML</summary>

```rust
#[test]
fn comment_run_preserves_indentation_bytes() {
    // An indented comment: the leading whitespace must stay OUTSIDE the
    // wrapper, byte-for-byte, and the wrapper opens at the `//`.
    let content = r#"# Indent

```rust {#demo}
fn f() {
// an indented aside
let x = 1;
}
```
"#;
    let doc = parse_document(content).unwrap();
    let output = weave_html(content, &doc).unwrap();
    let body = output.html.split("<article").nth(1).unwrap();

    // Four spaces of indent precede the wrapper open verbatim.
    assert!(body.contains(
        "    <span class=\"comment-run comment-aside\" data-comment-lines=\"1\">"
    ));
    // The following code line keeps its four-space indent intact.
    assert!(body.contains("    <span class=\"tok-keyword\">let</span>"));
}
```

</details>

<details><summary><code>ref_line_is_not_highlighted</code> · passed · <a href="../../../../knowledge/implementation/tangle/weave.md#chunk-tests">#tests</a> in Weaving literate documents into HTML</summary>

```rust
#[test]
fn ref_line_is_not_highlighted() {
    let content = r#"# Compose

```rust {#leaf}
let y = 1;
```

```rust {#root}
<<leaf>>
```
"#;
    let doc = parse_document(content).unwrap();
    let output = weave_html(content, &doc).unwrap();

    // The composing ref renders as a chunk-ref anchor, not a code span.
    assert!(output.html.contains("class=\"chunk-ref\" href=\"#chunk-leaf\""));
}
```

</details>

<details><summary><code>escaped_ref_line_renders_as_the_literal_without_an_anchor</code> · passed · <a href="../../../../knowledge/implementation/tangle/weave.md#chunk-tests">#tests</a> in Weaving literate documents into HTML</summary>

```rust
#[test]
fn escaped_ref_line_renders_as_the_literal_without_an_anchor() {
    let content = r#"# Show the syntax

```rust {#leaf}
let y = 1;
```

```markdown {#readme}
Put this line where the listing says so:
<<!leaf>>
```
"#;
    let doc = parse_document(content).unwrap();
    let output = weave_html(content, &doc).unwrap();

    assert!(output.html.contains("    &lt;&lt;leaf&gt;&gt;\n"), "{}", output.html);
    assert!(!output.html.contains("href=\"#chunk-leaf\""));
    assert!(!output.html.contains("&lt;&lt;!leaf&gt;&gt;"));
}
```

</details>

<details><summary><code>headings_get_slug_ids</code> · passed · <a href="../../../../knowledge/implementation/tangle/weave.md#chunk-tests">#tests</a> in Weaving literate documents into HTML</summary>

```rust
#[test]
fn headings_get_slug_ids() {
    let content = r#"# Doc

## Overview

prose

### Design Details

more prose
"#;
    let doc = parse_document(content).unwrap();
    let output = weave_html(content, &doc).unwrap();

    assert!(output.html.contains("<h2 id=\"overview\">Overview</h2>"));
    assert!(output.html.contains("<h3 id=\"design-details\">Design Details</h3>"));
}
```

</details>

<details><summary><code>duplicate_headings_get_deduped_ids</code> · passed · <a href="../../../../knowledge/implementation/tangle/weave.md#chunk-tests">#tests</a> in Weaving literate documents into HTML</summary>

```rust
#[test]
fn duplicate_headings_get_deduped_ids() {
    let content = r#"# Doc

## Notes

first

## Notes

second
"#;
    let doc = parse_document(content).unwrap();
    let output = weave_html(content, &doc).unwrap();

    assert!(output.html.contains("<h2 id=\"notes\">Notes</h2>"));
    assert!(output.html.contains("<h2 id=\"notes-2\">Notes</h2>"));
}
```

</details>

<details><summary><code>heading_with_inline_markup_slugs_and_preserves_markup</code> · passed · <a href="../../../../knowledge/implementation/tangle/weave.md#chunk-tests">#tests</a> in Weaving literate documents into HTML</summary>

```rust
#[test]
fn heading_with_inline_markup_slugs_and_preserves_markup() {
    let content = "# Doc\n\n## The `weave` function\n\nbody\n";
    let doc = parse_document(content).unwrap();
    let output = weave_html(content, &doc).unwrap();

    // Slug ignores the inline-code backticks and punctuation.
    assert!(output.html.contains("<h2 id=\"the-weave-function\">"));
    // The inner markup is preserved inside the heading.
    assert!(output.html.contains("<code>weave</code>"));
}
```

</details>

