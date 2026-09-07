---
x0k:
  format: folio/v1
  id: x0k:implementation/icon/emit
  type: implementation
  status: draft
  summary: One writer for every textual form of an icon — the normalized declaration, a standalone SVG file per scheme, inline SVG over CSS variables, and a symbol sprite of many — deterministic to the byte, with a fixed attribute order and one way to write a number, so that a declaration round-trips and two builds agree.
  concerns:
  - icons
  - svg
  - emit
  - publishing
  - sprite
  tangle:
    crate: x0k-icon
    root: src/emit.rs
  edges:
    implements:
    - x0k:design/icon-profile
    cites:
    - x0k:architecture/entity-iconography
    - x0k:implementation/icon/bind
    - x0k:implementation/icon/validate
    - x0k:implementation/tangle/region-repo
---
# Writing an icon out

One declaration, one reader, five forms. The
[profile](x0k:design/icon-profile) lists what each surface consumes — a
file per scheme for a README on GitHub, inline SVG for a woven page, a
`<symbol>` sprite for the plates, paths for the shell, pixels for a
favicon — and says that none of them is something a surface draws for
itself. This chapter writes the three that are text. The path form is the
shell's adapter over the typed icon and lives with the painter in
`x0k-ui-draw`; the raster needs a rasterizer and lives behind a feature
this crate does not carry. What is here is everything a published
repository's build needs, and it is built on one observation from the
[binder](bind.md): every textual form is the same tree with a different
string in each paint. So there is one writer, and the forms differ only
in what wraps the tree and what the roles were bound to.

The writer's one discipline is determinism. A projected repository
commits its icons as files and re-projects on every publish; a woven
page is re-woven whenever its source changes; and every one of those
runs must produce the bytes the last one did, or the diff is noise and
the tree is never clean. So attributes are written in a fixed order, a
number is written one way, indentation is two spaces, and there is no
map anywhere in the path from tree to text. The same discipline is what
makes *normalization* a real thing: written under the name binding, the
carried example comes back exactly as the design wrote it, byte for
byte, which the tests below hold for every one of the profile's first
inhabitants.

<a name="chunk-module-doc"></a><sub>[`src/emit.rs`](../../../../x0k-icon/src/emit.rs) · `#module-doc`</sub>

```rust {#module-doc}
//! Writing an accepted icon as text: the normalized declaration, a
//! standalone SVG per scheme, inline SVG over CSS variables, and a
//! `<symbol>` sprite of many icons.
//!
//! One writer produces every form — a [`BoundIcon`] is written with a
//! fixed attribute order and one number format, so the output is
//! deterministic to the byte and [`normalized`] returns a declaration to
//! its canonical text. The profile-owned attributes (round caps and
//! joins, `role="img"`, `aria-label`) are written on outputs and never
//! on the normalized declaration, which stays what its author may write.
//! The path form the native shell paints and the raster a favicon needs
//! are not here: the first is `x0k-ui-draw`'s adapter, the second needs a
//! rasterizer (`x0k:architecture/entity-iconography`, amendment).
```

<a name="chunk-imports"></a><sub>[`src/emit.rs`](../../../../x0k-icon/src/emit.rs) · `#imports`</sub>

```rust {#imports}
use crate::bind::{bind, BoundElement, BoundIcon, Palette, RoleBinding, Scheme};
use crate::parse::{PathCommand, Shape};
use crate::validate::Accepted;

/// The namespace every emitted `<svg>` root declares.
pub const SVG_NAMESPACE: &str = "http://www.w3.org/2000/svg";
```

## What an output is called

An icon is declared under the heading of the thing it depicts, and that
placement gives every output two strings: a *stem* derived from the
entity's id, which names files and symbols, and the section's title,
which the emitter writes as the icon's accessible label so the label and
the heading cannot disagree. The design's own example is the rule:
`x0k:affordance/read_an_affordance_out_of_a_document` emits
`read-an-affordance-out-of-a-document-light.svg`.

<a name="chunk-label"></a><sub>[`src/emit.rs`](../../../../x0k-icon/src/emit.rs) · `#label`</sub>

```rust {#label}
/// What an icon's outputs are named and labelled by: a stem from the
/// depicted entity's id, and the title of the section it is declared in.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Label {
    pub stem: String,
    pub title: String,
}

impl Label {
    /// The label for an icon declared under `title` on the entity `id`.
    pub fn for_entity(id: &str, title: &str) -> Label {
        Label { stem: stem_of(id), title: title.to_string() }
    }
}

/// The file stem an entity id derives: its last path segment, lowercased,
/// every run of characters outside `[a-z0-9]` written as one `-`.
pub fn stem_of(entity_id: &str) -> String {
    let last = entity_id.rsplit('/').next().unwrap_or(entity_id);
    let mut stem = String::new();
    for c in last.chars() {
        if c.is_ascii_alphanumeric() {
            stem.push(c.to_ascii_lowercase());
        } else if !stem.ends_with('-') {
            stem.push('-');
        }
    }
    stem.trim_matches('-').to_string()
}
```

## The forms

The normalized declaration is the tree under the name binding with only
the author's root attribute, `viewBox`. It is what an author may write
back into their block, and it is the form a content hash is taken over:
two declarations that differ only in attribute order or `13.50` against
`13.5` normalize to the same bytes.

<a name="chunk-normalized"></a><sub>[`src/emit.rs`](../../../../x0k-icon/src/emit.rs) · `#normalized`</sub>

```rust {#normalized}
/// The declaration in canonical text: attribute order and number format
/// fixed, roles written as their names, nothing the author may not
/// write. Re-parsing it yields the same icon.
pub fn normalized(icon: &Accepted) -> String {
    let bound = bind(icon, &RoleBinding::names());
    let mut w = Writer::default();
    w.open("svg", &[("viewBox", icon.grid().view_box().to_string())], false);
    w.children(&bound.elements);
    w.close("svg");
    w.out
}
```

A standalone SVG is the same tree wrapped for a renderer that has
nothing else: the namespace, the profile's round caps and joins on the
root where every stroke inherits them, and the accessibility attributes
the label supplies. Under a literal binding it is a file; under the
CSS-variable binding it is what a woven page inlines.

<a name="chunk-svg"></a><sub>[`src/emit.rs`](../../../../x0k-icon/src/emit.rs) · `#svg`</sub>

```rust {#svg}
/// One icon as a complete `<svg>` document: namespace, `role="img"`,
/// the label as `aria-label`, and the profile's round caps and joins on
/// the root. Under a palette binding this is a file; under
/// [`RoleBinding::css_variables`] it is inline SVG.
pub fn svg(bound: &BoundIcon, label: &Label) -> String {
    let mut w = Writer::default();
    w.open(
        "svg",
        &[
            ("xmlns", SVG_NAMESPACE.to_string()),
            ("viewBox", bound.grid.view_box().to_string()),
            ("role", "img".to_string()),
            ("aria-label", label.title.clone()),
            ("stroke-linecap", "round".to_string()),
            ("stroke-linejoin", "round".to_string()),
        ],
        false,
    );
    w.children(&bound.elements);
    w.close("svg");
    w.out
}

/// The icon as inline SVG for a themed page: roles as `var(--icon-<role>)`.
pub fn inline_svg(icon: &Accepted, label: &Label) -> String {
    svg(&bind(icon, &RoleBinding::css_variables()), label)
}
```

Where a surface cannot resolve a variable, the emitter writes one file
per scheme, and the page shows them through a `<picture>` whose
`prefers-color-scheme` source picks the dark one — the convention the
[repository projector](../tangle/region-repo.md "x0k:implementation/tangle/region-repo") already
uses for its plates. The file names are the stem and the scheme; the
order is light then dark, always.

<a name="chunk-files"></a><sub>[`src/emit.rs`](../../../../x0k-icon/src/emit.rs) · `#files`</sub>

```rust {#files}
/// The per-scheme files a publication ships for one icon: `(name, text)`
/// for `<stem>-light.svg` then `<stem>-dark.svg`, bound from the
/// publication's palette.
pub fn files(icon: &Accepted, palette: &Palette, label: &Label) -> Vec<(String, String)> {
    Scheme::BOTH
        .into_iter()
        .map(|scheme| {
            let name = format!("{}-{}.svg", label.stem, scheme.name());
            (name, svg(&bind(icon, palette.scheme(scheme)), label))
        })
        .collect()
}
```

That face is how a surface that ships files [shows an
icon](../../decisions/design/presentation/icon-profile/show-an-icon-on-any-surface.md "x0k:affordance/show_an_icon_on_a_surface"): the projector asks for
the two files and writes them beside the page. Its rustdoc is the cue.

<a name="folio-instance-68747470733a2f2f306b2e636f6d70757465722f6f6e746f6c6f6779237369676e69666965722f78306b2d69636f6e2d66696c6573-1"></a><sub data-instance-iri="https://0k.computer/ontology#signifier/x0k-icon-files" data-concept-iri="https://0k.computer/ontology#Signifier" data-source-document="corpora/x0k/implementation/icon/emit.md"><strong>Signifier</strong> · The forms · <code>https://0k.computer/ontology#signifier/x0k-icon-files</code> · <a href="#folio-source-68747470733a2f2f306b2e636f6d70757465722f6f6e746f6c6f6779237369676e69666965722f78306b2d69636f6e2d66696c6573-1">source declaration</a></sub><a name="folio-source-68747470733a2f2f306b2e636f6d70757465722f6f6e746f6c6f6779237369676e69666965722f78306b2d69636f6e2d66696c6573-1"></a>

```yaml x0k:signifier
id: x0k:signifier/x0k-icon-files
cue: files
edges:
  signifies:
    - x0k:affordance/show_an_icon_on_a_surface
  presentedOn:
    - x0k:surface/sdk
```

A page that draws many icons — the ontology plate with its forty-two
class sigils — wants them once, in a `<defs>` block, and a
`<use href="#icon-<stem>">` wherever one is shown. The sprite is that
block: one hidden `<svg>` holding a `<symbol>` per icon, each carrying its
own view box and the profile's caps and joins, all bound to the CSS
variables so the page's theme colours every one together. Icons are
written in the order given; a caller that wants a stable sprite sorts
before calling, and the plate's generated view does.

<a name="chunk-sprite"></a><sub>[`src/emit.rs`](../../../../x0k-icon/src/emit.rs) · `#sprite`</sub>

```rust {#sprite}
/// Many icons as one `<symbol>` sprite over CSS variables: a hidden
/// `<svg>` whose `<defs>` holds `<symbol id="icon-<stem>">` per icon, in
/// the order given, for `<use href="#icon-<stem>">` on the page.
pub fn sprite(icons: &[(&Label, &Accepted)]) -> String {
    let mut w = Writer::default();
    w.open(
        "svg",
        &[
            ("xmlns", SVG_NAMESPACE.to_string()),
            ("width", "0".to_string()),
            ("height", "0".to_string()),
            ("aria-hidden", "true".to_string()),
        ],
        false,
    );
    w.open("defs", &[], false);
    for (label, icon) in icons {
        let bound = bind(icon, &RoleBinding::css_variables());
        w.open(
            "symbol",
            &[
                ("id", format!("icon-{}", label.stem)),
                ("viewBox", bound.grid.view_box().to_string()),
                ("stroke-linecap", "round".to_string()),
                ("stroke-linejoin", "round".to_string()),
            ],
            false,
        );
        w.children(&bound.elements);
        w.close("symbol");
    }
    w.close("defs");
    w.close("svg");
    w.out
}
```

## The writer

The writer holds the text and the depth. An element is opened with its
attributes in the order handed to it, self-closed when it has no
children, and closed on its own line otherwise. Attribute values are
escaped for the three characters XML cannot take bare; a label's title
is the one value that ever needs it.

<a name="chunk-writer"></a><sub>[`src/emit.rs`](../../../../x0k-icon/src/emit.rs) · `#writer`</sub>

```rust {#writer}
#[derive(Default)]
struct Writer {
    out: String,
    depth: usize,
}

impl Writer {
    fn open(&mut self, tag: &str, attrs: &[(&str, String)], self_closing: bool) {
        self.out.push_str(&"  ".repeat(self.depth));
        self.out.push('<');
        self.out.push_str(tag);
        for (name, value) in attrs {
            self.out.push_str(&format!(" {name}=\"{}\"", escape(value)));
        }
        if self_closing {
            self.out.push_str("/>\n");
        } else {
            self.out.push_str(">\n");
            self.depth += 1;
        }
    }

    fn close(&mut self, tag: &str) {
        self.depth -= 1;
        self.out.push_str(&"  ".repeat(self.depth));
        self.out.push_str(&format!("</{tag}>\n"));
    }

    fn children(&mut self, elements: &[BoundElement]) {
        for el in elements {
            self.element(el);
        }
    }
}

fn escape(value: &str) -> String {
    value.replace('&', "&amp;").replace('<', "&lt;").replace('"', "&quot;")
}
```

An element's attributes come in the vocabulary table's order: its
geometry, then `fill`, `stroke`, `stroke-width`, `stroke-dasharray`,
`fill-rule`, and a `g`'s `transform` last. That order is the one the
design's own drawings use, which is why they normalize to themselves.

<a name="chunk-write-element"></a><sub>[`src/emit.rs`](../../../../x0k-icon/src/emit.rs) · `#write-element`</sub>

```rust {#write-element}
impl Writer {
    fn element(&mut self, el: &BoundElement) {
        let tag = match &el.shape {
            Shape::Group => "g",
            Shape::Path { .. } => "path",
            Shape::Line { .. } => "line",
            Shape::Circle { .. } => "circle",
            Shape::Rect { .. } => "rect",
            Shape::Polyline { .. } => "polyline",
            Shape::Foreign(name) => name.as_str(),
        };
        let mut attrs = geometry_attrs(&el.shape);
        push(&mut attrs, "fill", el.fill.clone());
        push(&mut attrs, "stroke", el.stroke.clone());
        push(&mut attrs, "stroke-width", el.stroke_width.map(num));
        push(&mut attrs, "stroke-dasharray", el.dasharray.as_deref().map(nums));
        if let Shape::Path { evenodd: true, .. } = el.shape {
            attrs.push(("fill-rule", "evenodd".to_string()));
        }
        push(&mut attrs, "transform", el.translate.map(|(x, y)| format!("translate({} {})", num(x), num(y))));
        let leaf = el.children.is_empty();
        self.open(tag, &attrs, leaf);
        if !leaf {
            self.children(&el.children);
            self.close(tag);
        }
    }
}

fn push(attrs: &mut Vec<(&'static str, String)>, name: &'static str, value: Option<String>) {
    if let Some(value) = value {
        attrs.push((name, value));
    }
}
```

<a name="chunk-geometry-attrs"></a><sub>[`src/emit.rs`](../../../../x0k-icon/src/emit.rs) · `#geometry-attrs`</sub>

```rust {#geometry-attrs}
fn geometry_attrs(shape: &Shape) -> Vec<(&'static str, String)> {
    match shape {
        Shape::Group | Shape::Foreign(_) => Vec::new(),
        Shape::Path { d, .. } => vec![("d", path_data(d))],
        Shape::Line { x1, y1, x2, y2 } => {
            vec![("x1", num(*x1)), ("y1", num(*y1)), ("x2", num(*x2)), ("y2", num(*y2))]
        }
        Shape::Circle { cx, cy, r } => vec![("cx", num(*cx)), ("cy", num(*cy)), ("r", num(*r))],
        Shape::Rect { x, y, width, height, rx } => {
            let mut attrs =
                vec![("x", num(*x)), ("y", num(*y)), ("width", num(*width)), ("height", num(*height))];
            attrs.extend(rx.map(|rx| ("rx", num(rx))));
            attrs
        }
        Shape::Polyline { points } => {
            let text = points.iter().map(|(x, y)| format!("{},{}", num(*x), num(*y))).collect::<Vec<_>>();
            vec![("points", text.join(" "))]
        }
    }
}
```

Path data is written with every command letter explicit, the letter
against its first number, commands separated by one space — `M2.5 1.5
H8.5 L11 4 V14.5 H2.5 Z` — which is the form the design draws in.

<a name="chunk-path-data"></a><sub>[`src/emit.rs`](../../../../x0k-icon/src/emit.rs) · `#path-data`</sub>

```rust {#path-data}
fn path_data(d: &[PathCommand]) -> String {
    d.iter()
        .map(|command| {
            let params: Vec<f64> = match command {
                PathCommand::MoveTo(x, y) | PathCommand::LineTo(x, y) => vec![*x, *y],
                PathCommand::Horizontal(v) | PathCommand::Vertical(v) => vec![*v],
                PathCommand::Cubic(p) => p.to_vec(),
                PathCommand::Quadratic(p) => p.to_vec(),
                PathCommand::Arc { rx, ry, rotation, large_arc, sweep, x, y } => {
                    vec![*rx, *ry, *rotation, f64::from(*large_arc), f64::from(*sweep), *x, *y]
                }
                PathCommand::Close | PathCommand::Unknown(_) => Vec::new(),
            };
            format!("{}{}", command.letter(), nums(&params))
        })
        .collect::<Vec<_>>()
        .join(" ")
}
```

A number is written the way an author writes it on the half-unit grid:
`8`, not `8.0`; `13.5`, not `13.50`. Every coordinate the profile
accepts is exactly representable, so the shortest form is exact, and
the checker borrows the same function to quote a value back.

<a name="chunk-num"></a><sub>[`src/emit.rs`](../../../../x0k-icon/src/emit.rs) · `#num`</sub>

```rust {#num}
/// A profile number in canonical text: an integer without a point,
/// otherwise the shortest exact decimal.
pub(crate) fn num(v: f64) -> String {
    if v.fract() == 0.0 && v.abs() < 1e15 {
        format!("{}", v as i64)
    } else {
        format!("{v}")
    }
}

fn nums(values: &[f64]) -> String {
    values.iter().map(|v| num(*v)).collect::<Vec<_>>().join(" ")
}
```

## What the tests pin

Normalization returns the carried example to the design's bytes, and it
is idempotent. A standalone file carries the namespace, the label and the
profile's caps and joins on its root and nowhere else; the two files a
palette yields differ only in their colours; the inline form writes
variables; the sprite holds one symbol per icon under the stem the
entity's id derives. Each is a form a surface consumes to [show an
icon](../../decisions/design/presentation/icon-profile/show-an-icon-on-any-surface.md "x0k:affordance/show_an_icon_on_a_surface") with nothing drawn by
hand.

<a name="chunk-tests"></a><sub>[`src/emit.rs`](../../../../x0k-icon/src/emit.rs) · `#tests` · proves [Show an icon on any surface](../../decisions/design/presentation/icon-profile/show-an-icon-on-any-surface.md)</sub>

```rust {#tests proves="x0k:affordance/show_an_icon_on_a_surface"}
#[cfg(test)]
mod tests {
    use super::*;
    use crate::fixtures;
    use crate::parse::parse;
    use crate::validate::validate;

    fn tangle() -> Accepted {
        validate(parse(fixtures::TANGLE).unwrap()).unwrap()
    }

    fn folio_palette() -> Palette {
        Palette {
            light: RoleBinding {
                ink: "#111111".into(),
                line: "#b88e44".into(),
                paper: "#fffff8".into(),
                accent: "#b88e44".into(),
            },
            dark: RoleBinding {
                ink: "#e2e8f0".into(),
                line: "#96b4dc".into(),
                paper: "#1e293b".into(),
                accent: "#96b4dc".into(),
            },
        }
    }

    #[test]
    fn the_tangle_icon_normalizes_to_the_designs_bytes() {
        let icon = tangle();
        let text = normalized(&icon);
        assert_eq!(text, fixtures::TANGLE);
        let again = validate(parse(&text).unwrap()).unwrap();
        assert_eq!(normalized(&again), text);
    }

    #[test]
    fn a_terse_declaration_normalizes_to_the_canonical_form() {
        let terse = r#"<svg viewBox="0 0 16 16"><path stroke-width="1.50" stroke="ink" d="M2,2 4,4 6 2Z" fill="none"/></svg>"#;
        let icon = validate(parse(terse).unwrap()).unwrap();
        assert_eq!(
            normalized(&icon),
            "<svg viewBox=\"0 0 16 16\">\n  <path d=\"M2 2 L4 4 L6 2 Z\" fill=\"none\" stroke=\"ink\" stroke-width=\"1.5\"/>\n</svg>\n"
        );
    }

    #[test]
    fn the_stem_is_the_entity_ids_last_segment_in_file_form() {
        assert_eq!(
            stem_of("x0k:affordance/read_an_affordance_out_of_a_document"),
            "read-an-affordance-out-of-a-document"
        );
        assert_eq!(stem_of("x0k:class/AIAgent"), "aiagent");
    }

    #[test]
    fn the_per_scheme_files_differ_only_in_their_colours() {
        let label = Label::for_entity("x0k:affordance/project_source_code_out_of_a_document", "Tangle — project source code out of a document");
        let out = files(&tangle(), &folio_palette(), &label);
        assert_eq!(out[0].0, "project-source-code-out-of-a-document-light.svg");
        assert_eq!(out[1].0, "project-source-code-out-of-a-document-dark.svg");
        let light = &out[0].1;
        assert!(light.starts_with("<svg xmlns=\"http://www.w3.org/2000/svg\" viewBox=\"0 0 16 16\" role=\"img\" aria-label=\"Tangle — project source code out of a document\" stroke-linecap=\"round\" stroke-linejoin=\"round\">\n"), "{light}");
        assert!(light.contains("stroke=\"#b88e44\""));
        assert!(!light.contains("stroke-linecap=\"round\"/>"), "caps and joins live on the root only");
        let recoloured = light.replace("#b88e44", "#96b4dc").replace("#111111", "#e2e8f0").replace("#fffff8", "#1e293b");
        assert_eq!(recoloured, out[1].1);
    }

    #[test]
    fn inline_svg_writes_the_roles_as_variables() {
        let label = Label::for_entity("x0k:affordance/project_source_code_out_of_a_document", "Tangle");
        let text = inline_svg(&tangle(), &label);
        assert!(text.contains("fill=\"var(--icon-paper)\" stroke=\"var(--icon-ink)\""));
        assert!(!text.contains("\"ink\""));
    }

    #[test]
    fn the_sprite_holds_one_symbol_per_icon_under_its_stem() {
        let weave = validate(parse(fixtures::WEAVE).unwrap()).unwrap();
        let a = Label::for_entity("x0k:affordance/project_source_code_out_of_a_document", "Tangle");
        let b = Label::for_entity("x0k:affordance/read_a_document_as_the_woven_artifact", "Weave");
        let text = sprite(&[(&a, &tangle()), (&b, &weave)]);
        assert!(text.starts_with("<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"0\" height=\"0\" aria-hidden=\"true\">\n  <defs>\n    <symbol id=\"icon-project-source-code-out-of-a-document\" viewBox=\"0 0 16 16\" stroke-linecap=\"round\" stroke-linejoin=\"round\">\n      <path d=\"M2.5 1.5 H8.5"), "{text}");
        assert!(text.contains("<symbol id=\"icon-read-a-document-as-the-woven-artifact\""));
        assert_eq!(text.matches("<symbol ").count(), 2);
        assert!(text.ends_with("    </symbol>\n  </defs>\n</svg>\n"));
    }

    #[test]
    fn a_label_is_escaped_where_xml_needs_it() {
        let label = Label { stem: "x".into(), title: "Both — a person & \"an agent\"".into() };
        let text = svg(&bind(&tangle(), &RoleBinding::names()), &label);
        assert!(text.contains("aria-label=\"Both — a person &amp; &quot;an agent&quot;\""));
    }
}
```

## Composing the module

<a name="chunk-root"></a><sub>[`src/emit.rs`](../../../../x0k-icon/src/emit.rs) · `#root` · assembles [module-doc](#chunk-module-doc) · [imports](#chunk-imports) · [label](#chunk-label) · [normalized](#chunk-normalized) · [svg](#chunk-svg) · [files](#chunk-files) · [sprite](#chunk-sprite) · [writer](#chunk-writer) · [write-element](#chunk-write-element) · [geometry-attrs](#chunk-geometry-attrs) · [path-data](#chunk-path-data) · [num](#chunk-num) · [tests](#chunk-tests)</sub>

```rust {#root}
<<module-doc>>

<<imports>>

<<label>>

<<normalized>>

<<svg>>

<<files>>

<<sprite>>

<<writer>>

<<write-element>>

<<geometry-attrs>>

<<path-data>>

<<num>>

<<tests>>
```

Two of the five forms are absent here and the absence is a boundary, not
a gap. The path form is not text: it is the typed tree the shell's
painter walks with the live theme's bindings in hand, and writing it as
strings only to parse it again at paint time would put a parser in the
frame loop. The raster is text's opposite — pixels — and wants a
rasterizer this crate must not carry if a published repository is to
build without one. The line between them is the line between what a
build needs and what a renderer needs, and this chapter stops on the
build's side of it.
