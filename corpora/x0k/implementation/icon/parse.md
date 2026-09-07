---
x0k:
  format: folio/v1
  id: x0k:implementation/icon/parse
  type: implementation
  status: draft
  summary: Reading an icon-profile declaration into its typed form — the two grids and their numbers, the four roles, the six elements and their geometry — permissive enough to carry what the checker will refuse, so that every refusal can name its rule and its element.
  concerns:
  - icons
  - svg
  - parsing
  - profile
  tangle:
    crate: x0k-icon
    root: src/parse.rs
  edges:
    implements:
    - x0k:design/icon-profile
    cites:
    - x0k:architecture/entity-iconography
    - x0k:implementation/icon/validate
    - x0k:implementation/icon/crate
---
# Reading a declaration

An icon in x0k is a few lines of SVG written under the heading of the
thing it depicts — a fenced `svg x0k:icon` block in a design, on a class
page, in the section that owns a chrome slot. The
[profile](x0k:design/icon-profile) that fixes what those lines may say is
small on purpose: two grids, six elements, four paint roles, a budget.
This chapter is the first thing that happens to such a block — it is read
into a typed form — and the one decision that shapes the whole reader is
*how much it refuses*.

The tempting answer is "everything the profile forbids": a parser that
knows the vocabulary can reject a `<text>` element or a hex colour on
sight. But the profile's checker promises something a rejecting parser
cannot deliver. It says *which rule* a drawing broke and *which element*
broke it, and for the composing agent that redraws on refusal, one refusal
per round trip is a slow loop. So the parser here is permissive on
purpose. It reads any well-formed XML into the profile's shapes and keeps
what does not fit — an unknown element, an unadmitted attribute, a literal
colour, a lowercase path command — *as data*, typed as exactly the thing
it is. The [checker](validate.md) then reads that data and reports every
rule at once. The parser refuses only what is not a declaration at all:
malformed XML, or a coordinate that is not a number.

The one icon carried through this chapter and its siblings is the mark for
the tangle affordance — a document with its fenced block sliding out to
the right as a file:

```svg x0k:!icon
<svg viewBox="0 0 16 16">
  <path d="M2.5 1.5 H8.5 L11 4 V14.5 H2.5 Z" fill="none" stroke="line" stroke-width="1"/>
  <path d="M8.5 1.5 V4 H11" fill="none" stroke="line" stroke-width="1"/>
  <path d="M4.5 6.5 H9 M4.5 8.5 H7" fill="none" stroke="line" stroke-width="1"/>
  <rect x="6.5" y="9.5" width="7" height="4" rx="0.5" fill="paper" stroke="ink" stroke-width="1.5"/>
  <path d="M8 11.5 H12" fill="none" stroke="ink" stroke-width="1"/>
</svg>
```

Read, it is five elements spending sixteen of the grid's twenty-four
commands, four of them painted in `line` and one — the subject, the block
— stroked in bold `ink` over `paper`. Every type below exists so that
sentence has a literal referent.

<a name="chunk-module-doc"></a><sub>[`src/parse.rs`](../../../../x0k-icon/src/parse.rs) · `#module-doc`</sub>

```rust {#module-doc}
//! Reading an icon-profile declaration into its typed form.
//!
//! [`parse`] turns the SVG text of an `svg x0k:icon` block into an
//! [`Icon`]: the root's view box, and a tree of [`Element`]s each carrying
//! its [`Shape`], its paints as [`Paint`], and its stroke attributes.
//! The parser is permissive by design — an element outside the profile,
//! an attribute the vocabulary does not admit, a literal colour or a
//! relative path command are all *kept*, typed as what they are, so that
//! the checker in `validate` can name every rule a drawing breaks in one
//! pass. Only text that is not a declaration at all (malformed XML, a
//! coordinate that is not a number) is refused here, as [`ParseError`].
//!
//! The profile's numbers — the two grids, their safe areas, stroke
//! registers and budgets — are [`Grid`]'s, read from
//! `x0k:design/icon-profile`.
```

<a name="chunk-imports"></a><sub>[`src/parse.rs`](../../../../x0k-icon/src/parse.rs) · `#imports`</sub>

```rust {#imports}
use std::fmt;

use roxmltree::{Document, Node, NodeType};
```

## The profile's numbers

Everything numeric in the profile hangs off the grid. There are two —
sixteen units for nearly everything, twenty-four for a chrome verb and a
raster master — and no third; each fixes a safe area, a stroke register
and a detail budget. The table in the design is this type's method set,
and nothing else in the crate holds a number the design states.

<a name="chunk-grid"></a><sub>[`src/parse.rs`](../../../../x0k-icon/src/parse.rs) · `#grid`</sub>

```rust {#grid}
/// The two grids an icon may be drawn on. Every profile number — safe
/// area, stroke register, budget — is a function of the grid
/// (`x0k:design/icon-profile` § "The grid").
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Grid {
    /// `viewBox="0 0 16 16"`: the working size for nearly every mark.
    Sixteen,
    /// `viewBox="0 0 24 24"`: chrome verbs and the master for rasters.
    TwentyFour,
}

impl Grid {
    /// The grid's side in units.
    pub fn size(self) -> f64 {
        match self {
            Grid::Sixteen => 16.0,
            Grid::TwentyFour => 24.0,
        }
    }

    /// The `viewBox` attribute value this grid is declared with.
    pub fn view_box(self) -> &'static str {
        match self {
            Grid::Sixteen => "0 0 16 16",
            Grid::TwentyFour => "0 0 24 24",
        }
    }
}
```

The safe area is the margin: one unit in on the 16 grid, two on the 24,
and every coordinate an author writes lies inside it. The stroke register
is two weights and no more, and the budget is the legibility floor stated
as a ceiling.

<a name="chunk-grid-numbers"></a><sub>[`src/parse.rs`](../../../../x0k-icon/src/parse.rs) · `#grid-numbers`</sub>

```rust {#grid-numbers}
impl Grid {
    /// The inclusive range every coordinate and control point must lie in.
    pub fn safe_area(self) -> (f64, f64) {
        match self {
            Grid::Sixteen => (1.0, 15.0),
            Grid::TwentyFour => (2.0, 22.0),
        }
    }

    /// The `regular` stroke: detail and contour.
    pub fn regular(self) -> f64 {
        match self {
            Grid::Sixteen => 1.0,
            Grid::TwentyFour => 1.5,
        }
    }

    /// The `bold` stroke: the subject's primary contour.
    pub fn bold(self) -> f64 {
        match self {
            Grid::Sixteen => 1.5,
            Grid::TwentyFour => 2.0,
        }
    }

    /// The most drawing commands an icon may spend.
    pub fn command_budget(self) -> usize {
        match self {
            Grid::Sixteen => 24,
            Grid::TwentyFour => 40,
        }
    }

    /// The most elements an icon may hold, the root excluded.
    pub fn element_budget(self) -> usize {
        match self {
            Grid::Sixteen => 8,
            Grid::TwentyFour => 12,
        }
    }
}
```

A view box names a grid only when it is exactly `0 0 16 16` or
`0 0 24 24`; anything else is off-grid, and the checker says so.

<a name="chunk-grid-from-view-box"></a><sub>[`src/parse.rs`](../../../../x0k-icon/src/parse.rs) · `#grid-from-view-box`</sub>

```rust {#grid-from-view-box}
impl Grid {
    /// The grid a `viewBox` declares, if it is one of the two.
    pub fn from_view_box(view_box: [f64; 4]) -> Option<Grid> {
        if view_box == [0.0, 0.0, 16.0, 16.0] {
            Some(Grid::Sixteen)
        } else if view_box == [0.0, 0.0, 24.0, 24.0] {
            Some(Grid::TwentyFour)
        } else {
            None
        }
    }
}
```

## The paints

An icon never names a colour; it names what a stroke *is*. Four roles,
and `none`. A `fill` or `stroke` that says anything else — a hex value, a
named colour, `currentColor`, a `url()` — is kept verbatim as a
`Literal`, because the checker's refusal of it wants to quote it back.

<a name="chunk-role"></a><sub>[`src/parse.rs`](../../../../x0k-icon/src/parse.rs) · `#role`</sub>

```rust {#role}
/// The four paint roles. A theme binds each to one of its colour roles;
/// a publication binds each to a literal per scheme
/// (`x0k:design/icon-profile` § "The paints").
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Role {
    /// The subject's own marks: its bold contour, a filled disc, a tick.
    Ink,
    /// Supporting strokes and absence: detail lines, pins, the dotted ring.
    Line,
    /// The ground, where a mark must knock something out. A fill only.
    Paper,
    /// One emphasis at most.
    Accent,
}

impl Role {
    /// Every role, in the order the palette block writes them.
    pub const ALL: [Role; 4] = [Role::Ink, Role::Line, Role::Paper, Role::Accent];

    /// The role's name as an author writes it in `fill=` or `stroke=`.
    pub fn name(self) -> &'static str {
        match self {
            Role::Ink => "ink",
            Role::Line => "line",
            Role::Paper => "paper",
            Role::Accent => "accent",
        }
    }

    /// The role a paint value names, if it names one.
    pub fn from_name(name: &str) -> Option<Role> {
        Role::ALL.into_iter().find(|role| role.name() == name)
    }
}
```

<a name="chunk-paint"></a><sub>[`src/parse.rs`](../../../../x0k-icon/src/parse.rs) · `#paint`</sub>

```rust {#paint}
/// A `fill` or `stroke` value as declared. `Literal` is anything that is
/// neither a role nor `none`; it is carried so the checker can refuse it
/// by rule, never painted.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Paint {
    /// `none`: nothing painted.
    None,
    /// One of the four roles.
    Role(Role),
    /// A value outside the profile, kept verbatim.
    Literal(String),
}

impl Paint {
    fn parse(value: &str) -> Paint {
        let value = value.trim();
        if value == "none" {
            return Paint::None;
        }
        match Role::from_name(value) {
            Some(role) => Paint::Role(role),
            None => Paint::Literal(value.to_string()),
        }
    }
}
```

## The geometry

Six elements, each with the geometry its row in the vocabulary admits, and
a seventh case for what is not in the vocabulary at all. `Foreign` is the
permissive parser's honesty: a `<text>`, a `<defs>`, a comment or a run of
character data is read as an element of that name and nothing beneath it
is read, so the checker can point at it.

<a name="chunk-shape"></a><sub>[`src/parse.rs`](../../../../x0k-icon/src/parse.rs) · `#shape`</sub>

```rust {#shape}
/// An element's geometry — one variant per element in the vocabulary
/// (`x0k:design/icon-profile` § "The vocabulary"), plus `Foreign` for
/// anything outside it.
#[derive(Debug, Clone, PartialEq)]
pub enum Shape {
    /// `<g>`: places its children; carries only inherited paints and a
    /// translation.
    Group,
    /// `<path d=…>`, `evenodd` when `fill-rule="evenodd"` is written.
    Path { d: Vec<PathCommand>, evenodd: bool },
    /// `<line x1 y1 x2 y2>`.
    Line { x1: f64, y1: f64, x2: f64, y2: f64 },
    /// `<circle cx cy r>`.
    Circle { cx: f64, cy: f64, r: f64 },
    /// `<rect x y width height [rx]>`.
    Rect { x: f64, y: f64, width: f64, height: f64, rx: Option<f64> },
    /// `<polyline points=…>`.
    Polyline { points: Vec<(f64, f64)> },
    /// An element, text run, comment or processing instruction outside
    /// the vocabulary, named; nothing beneath it is read.
    Foreign(String),
}
```

A path's `d` is the one attribute with a grammar of its own. The profile
admits eight absolute commands, and a command letter outside them — or
any lowercase, relative one — is kept as `Unknown` so the refusal can name
the letter. Parsing stops at the first unknown command: what its
parameters would have meant is not knowable, and the drawing is refused
regardless.

<a name="chunk-path-command"></a><sub>[`src/parse.rs`](../../../../x0k-icon/src/parse.rs) · `#path-command`</sub>

```rust {#path-command}
/// One absolute path command. `Unknown` is a letter outside `M L H V C Q
/// A Z`, kept for the checker; the commands after it are not read.
#[derive(Debug, Clone, PartialEq)]
pub enum PathCommand {
    MoveTo(f64, f64),
    LineTo(f64, f64),
    Horizontal(f64),
    Vertical(f64),
    /// `C x1 y1 x2 y2 x y`.
    Cubic([f64; 6]),
    /// `Q x1 y1 x y`.
    Quadratic([f64; 4]),
    /// `A rx ry rotation large-arc sweep x y`.
    Arc { rx: f64, ry: f64, rotation: f64, large_arc: bool, sweep: bool, x: f64, y: f64 },
    Close,
    Unknown(char),
}
```

A `transform` exists so a `g` can place a sub-mark, and it may translate
and nothing else. Whatever else an author writes there — a `scale`, a
`rotate`, a matrix — is kept as `Other`, verbatim.

<a name="chunk-transform"></a><sub>[`src/parse.rs`](../../../../x0k-icon/src/parse.rs) · `#transform`</sub>

```rust {#transform}
/// A `transform` value: the one form the profile admits, or the text of
/// one it does not.
#[derive(Debug, Clone, PartialEq)]
pub enum Transform {
    /// `translate(x y)`.
    Translate(f64, f64),
    /// Anything else, kept verbatim for the refusal.
    Other(String),
}

impl Transform {
    fn parse(value: &str) -> Transform {
        let value = value.trim();
        let inner = value
            .strip_prefix("translate(")
            .and_then(|rest| rest.strip_suffix(')'));
        let numbers: Option<Vec<f64>> = inner.and_then(|inner| {
            inner
                .split(|c: char| c.is_whitespace() || c == ',')
                .filter(|s| !s.is_empty())
                .map(|s| s.parse::<f64>().ok())
                .collect()
        });
        match numbers.as_deref() {
            Some([x, y]) => Transform::Translate(*x, *y),
            Some([x]) => Transform::Translate(*x, 0.0),
            _ => Transform::Other(value.to_string()),
        }
    }
}
```

## The element and the icon

An element is its shape, its own paints and stroke attributes, its
transform, its children, and two things the permissive reading needs: the
names of any attributes its row does not admit, and an ordinal — its
position in document order, root excluded — which is how a refusal says
"the fourth element". Paints are `Option` because absence and `none` are
different facts: an absent `fill` on a `path` is SVG's default black,
which the checker treats as unpainted, while `fill="none"` is an author
saying so.

<a name="chunk-element"></a><sub>[`src/parse.rs`](../../../../x0k-icon/src/parse.rs) · `#element`</sub>

```rust {#element}
/// One element of a declaration, as written.
#[derive(Debug, Clone, PartialEq)]
pub struct Element {
    /// Position in document order among the root's descendants, from 1.
    /// A refusal names an element by it.
    pub ordinal: usize,
    pub shape: Shape,
    pub fill: Option<Paint>,
    pub stroke: Option<Paint>,
    pub stroke_width: Option<f64>,
    /// `stroke-dasharray`, as a list of lengths.
    pub dasharray: Option<Vec<f64>>,
    pub transform: Option<Transform>,
    /// Attributes the element's row does not admit, by name — `style`,
    /// `id`, `opacity`, a cap or join — kept for the checker.
    pub unknown_attrs: Vec<String>,
    pub children: Vec<Element>,
}

impl Element {
    /// The element's tag as the vocabulary names it.
    pub fn tag(&self) -> &str {
        match &self.shape {
            Shape::Group => "g",
            Shape::Path { .. } => "path",
            Shape::Line { .. } => "line",
            Shape::Circle { .. } => "circle",
            Shape::Rect { .. } => "rect",
            Shape::Polyline { .. } => "polyline",
            Shape::Foreign(name) => name,
        }
    }
}
```

The icon is the root and its children. The root's tag and any attribute
beyond `viewBox` are kept for the same reason everything else is: the
profile's first rule is "one `<svg>` root carrying nothing else", and the
checker wants to say which of those two halves failed. The namespace
declaration is not an attribute here — XML reads it as a namespace, and
the emitter supplies it on every output.

<a name="chunk-icon"></a><sub>[`src/parse.rs`](../../../../x0k-icon/src/parse.rs) · `#icon`</sub>

```rust {#icon}
/// A parsed declaration: the root and its children.
#[derive(Debug, Clone, PartialEq)]
pub struct Icon {
    /// The root element's tag; `svg` on every accepted icon.
    pub root_tag: String,
    /// `viewBox` as four numbers, when present and numeric.
    pub view_box: Option<[f64; 4]>,
    /// Attributes on the root beyond `viewBox`, by name.
    pub root_extra: Vec<String>,
    pub children: Vec<Element>,
}

impl Icon {
    /// The grid the view box declares, if it declares one.
    pub fn grid(&self) -> Option<Grid> {
        self.view_box.and_then(Grid::from_view_box)
    }

    /// Every element in document order, depth first — the order
    /// ordinals were assigned in.
    pub fn elements(&self) -> Vec<&Element> {
        fn walk<'a>(el: &'a Element, out: &mut Vec<&'a Element>) {
            out.push(el);
            for child in &el.children {
                walk(child, out);
            }
        }
        let mut out = Vec::new();
        for child in &self.children {
            walk(child, &mut out);
        }
        out
    }
}
```

The detail budget is two counts, and this is where they are defined so
the [checker](validate.md) and a curious test agree on them. A path
command letter counts one, `Z` included; a `line`, `circle` or `rect`
counts one; a `polyline` counts its points. A `g` draws nothing and
counts no commands, but it is an element and counts as one. The carried
example spends sixteen commands across five elements, as the design says
it does.

<a name="chunk-counts"></a><sub>[`src/parse.rs`](../../../../x0k-icon/src/parse.rs) · `#counts`</sub>

```rust {#counts}
impl Icon {
    /// Drawing commands spent, by the profile's counting rule.
    pub fn command_count(&self) -> usize {
        self.elements()
            .iter()
            .map(|el| match &el.shape {
                Shape::Path { d, .. } => d.len(),
                Shape::Line { .. } | Shape::Circle { .. } | Shape::Rect { .. } => 1,
                Shape::Polyline { points } => points.len(),
                Shape::Group | Shape::Foreign(_) => 0,
            })
            .sum()
    }

    /// Elements held, the root excluded.
    pub fn element_count(&self) -> usize {
        self.elements().len()
    }
}
```

## What the parser refuses

Three things, none of them a profile rule: text that is not XML (a
DOCTYPE among them — `roxmltree` refuses a DTD before reading further,
which is the profile's position too), a geometry attribute an element
needs and does not carry, and a value where a number was expected and
something else was written. Everything else reaches the checker.

<a name="chunk-parse-error"></a><sub>[`src/parse.rs`](../../../../x0k-icon/src/parse.rs) · `#parse-error`</sub>

```rust {#parse-error}
/// Why a text could not be read as a declaration at all. A profile rule
/// broken by a readable declaration is a [`Defect`](crate::validate::Defect),
/// not one of these.
///
/// The `Display` and `Error` impls are written out rather than derived:
/// this crate ships in `x0k:publication/x0k-folio`, whose projection is
/// checked by `cargo deny`, and the workspace's `thiserror` is a major
/// version behind the one the published vocabulary crate already pulls
/// through its Turtle parser. Four messages are cheaper than a duplicate
/// dependency in someone else's lockfile.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ParseError {
    Xml(String),
    MissingAttribute { element: String, attribute: &'static str },
    NotANumber { element: String, attribute: String, value: String },
    TruncatedPath { element: String, command: char },
}

impl std::fmt::Display for ParseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ParseError::Xml(e) => write!(f, "not well-formed XML: {e}"),
            ParseError::MissingAttribute { element, attribute } => {
                write!(f, "{element} carries no `{attribute}`")
            }
            ParseError::NotANumber { element, attribute, value } => {
                write!(f, "{element}: `{attribute}=\"{value}\"` is not a number")
            }
            ParseError::TruncatedPath { element, command } => {
                write!(f, "{element}: path data ends inside a `{command}` command")
            }
        }
    }
}

impl std::error::Error for ParseError {}
```

## Reading the tree

`parse` is the chapter's face. It hands the XML to `roxmltree`, reads the
root, and walks its children assigning ordinals as it goes.

<a name="chunk-parse"></a><sub>[`src/parse.rs`](../../../../x0k-icon/src/parse.rs) · `#parse`</sub>

```rust {#parse}
/// Read the text of an `svg x0k:icon` block into an [`Icon`]. Permissive:
/// what the profile refuses is kept typed for the checker; only text
/// that is not a declaration is an error.
pub fn parse(text: &str) -> Result<Icon, ParseError> {
    let doc = Document::parse(text).map_err(|e| ParseError::Xml(e.to_string()))?;
    let root = doc.root_element();
    let mut view_box = None;
    let mut root_extra = Vec::new();
    for attr in root.attributes() {
        if attr.name() == "viewBox" {
            view_box = Some(read_view_box(attr.value())?);
        } else {
            root_extra.push(attr.name().to_string());
        }
    }
    let mut ordinal = 0;
    let children = read_children(root, &mut ordinal)?;
    Ok(Icon {
        root_tag: root.tag_name().name().to_string(),
        view_box,
        root_extra,
        children,
    })
}

fn read_view_box(value: &str) -> Result<[f64; 4], ParseError> {
    let numbers = number_list("svg", "viewBox", value)?;
    numbers.try_into().map_err(|_| ParseError::NotANumber {
        element: "svg".to_string(),
        attribute: "viewBox".to_string(),
        value: value.to_string(),
    })
}
```

Children come in four kinds under XML, and only one of them is an
element. Whitespace between elements is the author's indentation and
means nothing; any other character data, a comment, or a processing
instruction is a `Foreign` element named for what it is, so the checker
refuses it under the same rule as a `<text>`.

<a name="chunk-read-children"></a><sub>[`src/parse.rs`](../../../../x0k-icon/src/parse.rs) · `#read-children`</sub>

```rust {#read-children}
fn read_children(parent: Node, ordinal: &mut usize) -> Result<Vec<Element>, ParseError> {
    let mut out = Vec::new();
    for node in parent.children() {
        let foreign = match node.node_type() {
            NodeType::Element => None,
            NodeType::Text if node.text().is_some_and(|t| t.trim().is_empty()) => continue,
            NodeType::Text => Some("text"),
            NodeType::Comment => Some("comment"),
            NodeType::PI => Some("processing instruction"),
            NodeType::Root => continue,
        };
        *ordinal += 1;
        match foreign {
            Some(what) => out.push(foreign_element(*ordinal, what)),
            None => out.push(read_element(node, ordinal)?),
        }
    }
    Ok(out)
}

fn foreign_element(ordinal: usize, what: &str) -> Element {
    Element {
        ordinal,
        shape: Shape::Foreign(what.to_string()),
        fill: None,
        stroke: None,
        stroke_width: None,
        dasharray: None,
        transform: None,
        unknown_attrs: Vec::new(),
        children: Vec::new(),
    }
}
```

An element's attributes are sorted against its row of the vocabulary:
the paint and stroke attributes are read into their fields, the geometry
is read by the shape, and whatever the row does not admit is recorded by
name. `fill-rule` is admitted with one value only, so a `fill-rule` that
says anything but `evenodd` is recorded as unadmitted rather than read as
a fill rule. `transform` is read wherever it appears rather than sorted
against the row, because *where* a transform may appear is a rule of its
own (the thirteenth) and one refusal should name it. An element outside
the vocabulary is `Foreign` and its children are not read: the checker
refuses the element, and what it contains is not the author's question.

<a name="chunk-read-element"></a><sub>[`src/parse.rs`](../../../../x0k-icon/src/parse.rs) · `#read-element`</sub>

```rust {#read-element}
fn read_element(node: Node, ordinal: &mut usize) -> Result<Element, ParseError> {
    let tag = node.tag_name().name();
    let me = *ordinal;
    let admitted = admitted_attributes(tag);
    let attr = |name: &str| node.attribute(name);
    let mut unknown_attrs: Vec<String> = node
        .attributes()
        .filter(|a| a.name() != "transform" && !admitted.iter().any(|name| *name == a.name()))
        .map(|a| a.name().to_string())
        .collect();
    if attr("fill-rule").is_some_and(|v| v.trim() != "evenodd") {
        unknown_attrs.push("fill-rule".to_string());
    }
    let shape = read_shape(node, tag)?;
    let children = match &shape {
        Shape::Foreign(_) => Vec::new(),
        _ => read_children(node, ordinal)?,
    };
    Ok(Element {
        ordinal: me,
        shape,
        fill: attr("fill").map(Paint::parse),
        stroke: attr("stroke").map(Paint::parse),
        stroke_width: attr("stroke-width")
            .map(|v| number(tag, "stroke-width", v))
            .transpose()?,
        dasharray: attr("stroke-dasharray")
            .map(|v| number_list(tag, "stroke-dasharray", v))
            .transpose()?,
        transform: attr("transform").map(Transform::parse),
        unknown_attrs,
        children,
    })
}
```

The vocabulary table, as data. `g` admits no dash and no geometry; `line`
and `polyline` admit no fill; a `rect`'s `rx` is the one optional
geometry attribute. An element not in the table admits nothing, which is
consistent: it is refused whole.

<a name="chunk-admitted-attributes"></a><sub>[`src/parse.rs`](../../../../x0k-icon/src/parse.rs) · `#admitted-attributes`</sub>

```rust {#admitted-attributes}
/// The attributes an element's row of the vocabulary admits.
fn admitted_attributes(tag: &str) -> &'static [&'static str] {
    match tag {
        "g" => &["fill", "stroke", "stroke-width", "transform"],
        "path" => &["d", "fill", "stroke", "stroke-width", "stroke-dasharray", "fill-rule"],
        "line" => &["x1", "y1", "x2", "y2", "stroke", "stroke-width", "stroke-dasharray"],
        "circle" => &["cx", "cy", "r", "fill", "stroke", "stroke-width", "stroke-dasharray"],
        "rect" => &[
            "x", "y", "width", "height", "rx", "fill", "stroke", "stroke-width", "stroke-dasharray",
        ],
        "polyline" => &["points", "stroke", "stroke-width", "stroke-dasharray"],
        _ => &[],
    }
}
```

Reading a shape is reading its required numbers. A missing one is a parse
error, not a rule: the profile has no default geometry, and an SVG
renderer's zero would draw something the author never wrote.

<a name="chunk-read-shape"></a><sub>[`src/parse.rs`](../../../../x0k-icon/src/parse.rs) · `#read-shape`</sub>

```rust {#read-shape}
fn read_shape(node: Node, tag: &str) -> Result<Shape, ParseError> {
    let required = |name: &'static str| -> Result<f64, ParseError> {
        let value = node.attribute(name).ok_or_else(|| ParseError::MissingAttribute {
            element: tag.to_string(),
            attribute: name,
        })?;
        number(tag, name, value)
    };
    Ok(match tag {
        "g" => Shape::Group,
        "path" => Shape::Path {
            d: parse_path(tag, node.attribute("d").unwrap_or_default())?,
            evenodd: node.attribute("fill-rule").is_some_and(|v| v.trim() == "evenodd"),
        },
        "line" => Shape::Line {
            x1: required("x1")?,
            y1: required("y1")?,
            x2: required("x2")?,
            y2: required("y2")?,
        },
        "circle" => Shape::Circle { cx: required("cx")?, cy: required("cy")?, r: required("r")? },
        "rect" => Shape::Rect {
            x: required("x")?,
            y: required("y")?,
            width: required("width")?,
            height: required("height")?,
            rx: node.attribute("rx").map(|v| number(tag, "rx", v)).transpose()?,
        },
        "polyline" => Shape::Polyline {
            points: read_points(tag, node.attribute("points").unwrap_or_default())?,
        },
        _ => Shape::Foreign(tag.to_string()),
    })
}
```

## Numbers, lists, and path data

The rest is tokenizing. SVG lets numbers in a list be separated by
whitespace, commas, or both, and lets a path run its numbers straight
into the next command letter; the tokenizer here accepts all of that and
the emitter writes back exactly one form, which is what normalization
means for a declaration.

<a name="chunk-numbers"></a><sub>[`src/parse.rs`](../../../../x0k-icon/src/parse.rs) · `#numbers`</sub>

```rust {#numbers}
fn number(element: &str, attribute: &str, value: &str) -> Result<f64, ParseError> {
    value.trim().parse::<f64>().map_err(|_| ParseError::NotANumber {
        element: element.to_string(),
        attribute: attribute.to_string(),
        value: value.to_string(),
    })
}

fn number_list(element: &str, attribute: &str, value: &str) -> Result<Vec<f64>, ParseError> {
    value
        .split(|c: char| c.is_whitespace() || c == ',')
        .filter(|s| !s.is_empty())
        .map(|s| number(element, attribute, s))
        .collect()
}

fn read_points(element: &str, value: &str) -> Result<Vec<(f64, f64)>, ParseError> {
    let numbers = number_list(element, "points", value)?;
    if numbers.len() % 2 != 0 {
        return Err(ParseError::NotANumber {
            element: element.to_string(),
            attribute: "points".to_string(),
            value: value.to_string(),
        });
    }
    Ok(numbers.chunks(2).map(|pair| (pair[0], pair[1])).collect())
}
```

Path data is a stream of letters and numbers. A letter is a token by
itself; a number runs until a character that cannot continue it, where a
sign may only open a number or follow an exponent — so `M1.5-2` is two
numbers, as SVG says it is.

<a name="chunk-path-tokens"></a><sub>[`src/parse.rs`](../../../../x0k-icon/src/parse.rs) · `#path-tokens`</sub>

```rust {#path-tokens}
#[derive(Clone, Copy)]
enum PathToken {
    Command(char),
    Number(f64),
}

fn path_tokens(element: &str, d: &str) -> Result<Vec<PathToken>, ParseError> {
    let mut tokens = Vec::new();
    let chars: Vec<char> = d.chars().collect();
    let mut i = 0;
    while i < chars.len() {
        let c = chars[i];
        if c.is_ascii_alphabetic() {
            tokens.push(PathToken::Command(c));
            i += 1;
        } else if c.is_whitespace() || c == ',' {
            i += 1;
        } else {
            let start = i;
            i += 1;
            while i < chars.len() {
                let x = chars[i];
                let after_exponent = matches!(chars[i - 1], 'e' | 'E');
                let continues = x.is_ascii_digit()
                    || x == '.'
                    || x == 'e'
                    || x == 'E'
                    || ((x == '-' || x == '+') && after_exponent);
                if !continues {
                    break;
                }
                i += 1;
            }
            let text: String = chars[start..i].iter().collect();
            tokens.push(PathToken::Number(number(element, "d", &text)?));
        }
    }
    Ok(tokens)
}
```

Commands take a fixed number of parameters each, and SVG lets a letter
be followed by more than one set — an implicit repeat, where a repeated
`M` means `L`. The profile's canonical form writes every letter out, and
reading the implicit form is what lets a declaration written the terse
way normalize to it without changing what any rule would say. An unknown
or relative letter ends the read; the drawing is refused for it and the
rest is unknowable.

<a name="chunk-parse-path"></a><sub>[`src/parse.rs`](../../../../x0k-icon/src/parse.rs) · `#parse-path`</sub>

```rust {#parse-path}
fn parse_path(element: &str, d: &str) -> Result<Vec<PathCommand>, ParseError> {
    let tokens = path_tokens(element, d)?;
    let mut commands = Vec::new();
    let mut i = 0;
    while i < tokens.len() {
        let PathToken::Command(mut letter) = tokens[i] else {
            i += 1;
            continue;
        };
        i += 1;
        let Some(arity) = command_arity(letter) else {
            commands.push(PathCommand::Unknown(letter));
            break;
        };
        loop {
            let params: Vec<f64> = tokens[i..]
                .iter()
                .take(arity)
                .map_while(|t| match t {
                    PathToken::Number(n) => Some(*n),
                    PathToken::Command(_) => None,
                })
                .collect();
            if params.len() < arity {
                return Err(ParseError::TruncatedPath { element: element.to_string(), command: letter });
            }
            i += arity;
            commands.push(path_command(letter, &params));
            if letter == 'M' {
                letter = 'L';
            }
            if arity == 0 || !matches!(tokens.get(i), Some(PathToken::Number(_))) {
                break;
            }
        }
    }
    Ok(commands)
}

fn command_arity(letter: char) -> Option<usize> {
    Some(match letter {
        'M' | 'L' => 2,
        'H' | 'V' => 1,
        'C' => 6,
        'Q' => 4,
        'A' => 7,
        'Z' => 0,
        _ => return None,
    })
}

fn path_command(letter: char, p: &[f64]) -> PathCommand {
    match letter {
        'M' => PathCommand::MoveTo(p[0], p[1]),
        'L' => PathCommand::LineTo(p[0], p[1]),
        'H' => PathCommand::Horizontal(p[0]),
        'V' => PathCommand::Vertical(p[0]),
        'C' => PathCommand::Cubic([p[0], p[1], p[2], p[3], p[4], p[5]]),
        'Q' => PathCommand::Quadratic([p[0], p[1], p[2], p[3]]),
        'A' => PathCommand::Arc {
            rx: p[0],
            ry: p[1],
            rotation: p[2],
            large_arc: p[3] != 0.0,
            sweep: p[4] != 0.0,
            x: p[5],
            y: p[6],
        },
        _ => PathCommand::Close,
    }
}
```

A command knows its letter, which the emitter and a refusal both need.

<a name="chunk-command-letter"></a><sub>[`src/parse.rs`](../../../../x0k-icon/src/parse.rs) · `#command-letter`</sub>

```rust {#command-letter}
impl PathCommand {
    /// The command's letter as written in `d`.
    pub fn letter(&self) -> char {
        match self {
            PathCommand::MoveTo(..) => 'M',
            PathCommand::LineTo(..) => 'L',
            PathCommand::Horizontal(_) => 'H',
            PathCommand::Vertical(_) => 'V',
            PathCommand::Cubic(_) => 'C',
            PathCommand::Quadratic(_) => 'Q',
            PathCommand::Arc { .. } => 'A',
            PathCommand::Close => 'Z',
            PathCommand::Unknown(c) => *c,
        }
    }
}

impl fmt::Display for Role {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.name())
    }
}
```

## What the tests pin

The carried example reads to the shape the design describes — five
elements, sixteen commands, the block a `rect` filled with `paper` — and
the terse forms SVG allows read to the same commands as the canonical
one. What the parser keeps rather than refuses is pinned too: a literal
colour, an unknown element, a relative command each arrive typed, which
is the contract the [checker](validate.md) rests on. Reading a
declaration is the [first half of declaring an
icon](x0k:affordance/declare_an_icon): a block under a heading is a
declaration only because something reads it as one.

<a name="chunk-tests"></a><sub>[`src/parse.rs`](../../../../x0k-icon/src/parse.rs) · `#tests` · proves `x0k:affordance/declare_an_icon`</sub>

```rust {#tests proves="x0k:affordance/declare_an_icon"}
#[cfg(test)]
mod tests {
    use super::*;
    use crate::fixtures;

    #[test]
    fn the_tangle_icon_reads_as_the_design_describes_it() {
        let icon = parse(fixtures::TANGLE).unwrap();
        assert_eq!(icon.root_tag, "svg");
        assert_eq!(icon.grid(), Some(Grid::Sixteen));
        assert_eq!(icon.element_count(), 5);
        assert_eq!(icon.command_count(), 16);
        let block = &icon.children[3];
        assert!(matches!(block.shape, Shape::Rect { rx: Some(0.5), .. }));
        assert_eq!(block.fill, Some(Paint::Role(Role::Paper)));
        assert_eq!(block.stroke, Some(Paint::Role(Role::Ink)));
        assert_eq!(block.stroke_width, Some(1.5));
    }

    #[test]
    fn terse_path_data_reads_to_the_canonical_commands() {
        let canonical = parse(r#"<svg viewBox="0 0 16 16"><path d="M2 2 L4 4 L6 2 Z"/></svg>"#).unwrap();
        let terse = parse(r#"<svg viewBox="0 0 16 16"><path d="M2,2 4,4 6 2Z"/></svg>"#).unwrap();
        assert_eq!(canonical.children[0].shape, terse.children[0].shape);
        assert_eq!(canonical.command_count(), 4);
    }

    #[test]
    fn what_the_profile_refuses_is_kept_typed() {
        let icon = parse(
            r##"<svg viewBox="0 0 16 16" width="16">
  <circle cx="8" cy="8" r="6" fill="#111111" style="x"/>
  <text x="1" y="1">a</text>
  <path d="m2 2 l4 4"/>
  <g transform="scale(2)"/>
</svg>"##,
        )
        .unwrap();
        assert_eq!(icon.root_extra, vec!["width"]);
        let els = icon.elements();
        assert_eq!(els[0].fill, Some(Paint::Literal("#111111".to_string())));
        assert_eq!(els[0].unknown_attrs, vec!["style"]);
        assert_eq!(els[1].shape, Shape::Foreign("text".to_string()));
        assert!(matches!(&els[2].shape, Shape::Path { d, .. } if d == &[PathCommand::Unknown('m')]));
        assert_eq!(els[3].transform, Some(Transform::Other("scale(2)".to_string())));
    }

    #[test]
    fn what_is_not_a_declaration_is_an_error() {
        assert!(matches!(parse("<svg"), Err(ParseError::Xml(_))));
        assert!(matches!(
            parse(r#"<svg viewBox="0 0 16 16"><circle cx="8" cy="8"/></svg>"#),
            Err(ParseError::MissingAttribute { attribute: "r", .. })
        ));
        assert!(matches!(
            parse(r#"<svg viewBox="0 0 16 16"><path d="M2 2 L4"/></svg>"#),
            Err(ParseError::TruncatedPath { command: 'L', .. })
        ));
    }
}
```

## Composing the module

<a name="chunk-root"></a><sub>[`src/parse.rs`](../../../../x0k-icon/src/parse.rs) · `#root` · assembles [module-doc](#chunk-module-doc) · [imports](#chunk-imports) · [grid](#chunk-grid) · [grid-numbers](#chunk-grid-numbers) · [grid-from-view-box](#chunk-grid-from-view-box) · [role](#chunk-role) · [paint](#chunk-paint) · [shape](#chunk-shape) · [path-command](#chunk-path-command) · [transform](#chunk-transform) · [element](#chunk-element) · [icon](#chunk-icon) · [counts](#chunk-counts) · [parse-error](#chunk-parse-error) · [parse](#chunk-parse) · [read-children](#chunk-read-children) · [read-element](#chunk-read-element) · [admitted-attributes](#chunk-admitted-attributes) · [read-shape](#chunk-read-shape) · [numbers](#chunk-numbers) · [path-tokens](#chunk-path-tokens) · [parse-path](#chunk-parse-path) · [command-letter](#chunk-command-letter) · [tests](#chunk-tests)</sub>

```rust {#root}
<<module-doc>>

<<imports>>

<<grid>>

<<grid-numbers>>

<<grid-from-view-box>>

<<role>>

<<paint>>

<<shape>>

<<path-command>>

<<transform>>

<<element>>

<<icon>>

<<counts>>

<<parse-error>>

<<parse>>

<<read-children>>

<<read-element>>

<<admitted-attributes>>

<<read-shape>>

<<numbers>>

<<path-tokens>>

<<parse-path>>

<<command-letter>>

<<tests>>
```

The permissiveness has a boundary worth stating. The parser keeps what a
rule will refuse, but it does not keep *everything*: a `Foreign` element's
attributes and children are dropped, and path data past an unknown
command is dropped. Both are places where reading further would produce
data no rule could act on, and the refusal that follows makes the loss
moot. What it means is that the typed form is faithful for every accepted
icon and faithful *enough* for every refused one — enough to say what was
wrong, never enough to redraw it, which is the design's line exactly.
