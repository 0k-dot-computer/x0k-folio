---
x0k:
  format: folio/v1
  id: x0k:implementation/icon/validate
  type: implementation
  status: draft
  summary: 'The checker: every refusal rule of the icon profile read against a parsed declaration in one pass, each broken rule reported as a typed defect naming the rule and the element, and an accepted icon made a type the rest of the crate can trust.'
  concerns:
  - icons
  - svg
  - validation
  - profile
  tangle:
    crate: x0k-icon
    root: src/validate.rs
  edges:
    implements:
    - x0k:design/icon-profile
    cites:
    - x0k:architecture/entity-iconography
    - x0k:implementation/icon/parse
    - x0k:implementation/icon/crate
---
# The checker

The [profile](x0k:design/icon-profile) promises an author two things
about a drawing that breaks a rule: they are told *which rule* and
*where*, and they are never handed back a silently altered drawing. This
chapter is that promise. It reads the typed form the
[parser](parse.md) produced — permissive on purpose, carrying every
unadmitted thing as data — and walks it once against the fifteen rules
the design numbers, reporting each breach as a [`Defect`](#defect) that
names the rule and the element. It repairs nothing. The composing agent's
retry loop ([`entity-iconography`](x0k:architecture/entity-iconography))
redraws on the list; a person reads it and fixes the block.

The chapter's second job is quieter and matters more to the rest of the
crate. A declaration that passes becomes an [`Accepted`](#accepted) — the
same icon, wrapped, with its grid known — and binding and emitting take
only that. So "the emitter never writes a literal colour" is not a
convention the binder observes; it is a fact about what the binder can be
handed.

Take the carried example, the tangle mark, and break it three ways: paint
the block `#111111` instead of `ink`, slide it two units right so its far
edge lands at 15.5, and give the page five more text lines. The checker's
answer to that one drawing is three defects — rule 5 on the fourth
element quoting the hex, rule 11 on the same element naming the corner at
15.5, rule 12 on the root with the count of twenty-six — and the author
fixes all three before asking again.

<a name="chunk-module-doc"></a><sub>[`src/validate.rs`](../../../../x0k-icon/src/validate.rs) · `#module-doc`</sub>

```rust {#module-doc}
//! Checking a parsed declaration against the icon profile.
//!
//! [`validate`] reads an [`Icon`] against every refusal rule of
//! `x0k:design/icon-profile` § "What the checker refuses" and returns
//! either an [`Accepted`] icon — the same icon, its grid known, the type
//! the binder and emitter take — or every [`Defect`] found, each naming
//! its [`Rule`] and the element that broke it. Nothing is repaired: a
//! declaration is what its author wrote. [`one_per_grid`] is the one
//! rule that spans declarations, for the section reader that holds
//! several.
```

<a name="chunk-imports"></a><sub>[`src/validate.rs`](../../../../x0k-icon/src/validate.rs) · `#imports`</sub>

```rust {#imports}
use std::fmt;
use std::ops::Deref;

use crate::parse::{Element, Grid, Icon, Paint, PathCommand, Role, Shape, Transform};
```

## The rules, as a type

The design numbers its rules one to fifteen, and an author who reads
"rule 5" in a refusal goes to that list. So the rule is an enum whose
order *is* the design's order, and a refusal prints the number and the
design's own title for it.

<a name="chunk-rule"></a><sub>[`src/validate.rs`](../../../../x0k-icon/src/validate.rs) · `#rule`</sub>

```rust {#rule}
/// The profile's refusal rules, in the design's numbering
/// (`x0k:design/icon-profile` § "What the checker refuses").
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Rule {
    NotOneRoot,
    OffGridViewBox,
    UnknownElement,
    UnknownAttribute,
    LiteralPaint,
    PaperAsStroke,
    OffRegisterStroke,
    OffProfileDash,
    RelativeOrUnknownPathCommand,
    OffTheHalfUnit,
    OutsideTheSafeArea,
    OverBudget,
    TransformNotATranslation,
    NothingPainted,
    TwoIconsOnOneGrid,
}

impl Rule {
    /// The rule's number in the design's list.
    pub fn number(self) -> u8 {
        self as u8 + 1
    }

    /// The design's title for the rule.
    pub fn title(self) -> &'static str {
        match self {
            Rule::NotOneRoot => "not one root",
            Rule::OffGridViewBox => "off-grid viewBox",
            Rule::UnknownElement => "unknown element",
            Rule::UnknownAttribute => "unknown attribute",
            Rule::LiteralPaint => "a literal paint",
            Rule::PaperAsStroke => "paper as a stroke",
            Rule::OffRegisterStroke => "off-register stroke",
            Rule::OffProfileDash => "off-profile dash",
            Rule::RelativeOrUnknownPathCommand => "relative or unknown path command",
            Rule::OffTheHalfUnit => "off the half-unit",
            Rule::OutsideTheSafeArea => "outside the safe area",
            Rule::OverBudget => "over budget",
            Rule::TransformNotATranslation => "a transform that is not a translation",
            Rule::NothingPainted => "nothing painted",
            Rule::TwoIconsOnOneGrid => "two icons on one grid",
        }
    }
}
```

## The defect

A defect is a rule, the element that broke it, and one line of detail
quoting what was written. The element is named by its ordinal — the
parser's document-order count — and its tag, with ordinal zero for the
root, because a rule about the view box or the budget is a rule about the
whole drawing.

<a name="chunk-defect"></a><sub>[`src/validate.rs`](../../../../x0k-icon/src/validate.rs) · `#defect`</sub>

```rust {#defect}
/// The element a defect names: its document-order ordinal (0 is the
/// root) and its tag.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ElementRef {
    pub ordinal: usize,
    pub tag: String,
}

impl ElementRef {
    fn root() -> ElementRef {
        ElementRef { ordinal: 0, tag: "svg".to_string() }
    }

    fn of(el: &Element) -> ElementRef {
        ElementRef { ordinal: el.ordinal, tag: el.tag().to_string() }
    }
}

/// One broken rule: which, where, and what was written there.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Defect {
    pub rule: Rule,
    pub element: ElementRef,
    pub detail: String,
}

impl fmt::Display for Defect {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "rule {} ({}): {} #{} — {}",
            self.rule.number(),
            self.rule.title(),
            self.element.tag,
            self.element.ordinal,
            self.detail
        )
    }
}
```

## The accepted icon

`Accepted` is an `Icon` that has been through the checker, and the grid
it was checked on. It dereferences to the icon so a consumer reads it
like one; it cannot be constructed anywhere but here, which is the whole
of its meaning.

<a name="chunk-accepted"></a><sub>[`src/validate.rs`](../../../../x0k-icon/src/validate.rs) · `#accepted`</sub>

```rust {#accepted}
/// An icon the checker accepted, with its grid. The only way to obtain
/// one is [`validate`]; the binder and emitter take nothing else.
#[derive(Debug, Clone, PartialEq)]
pub struct Accepted {
    icon: Icon,
    grid: Grid,
}

impl Accepted {
    /// The grid the icon is drawn on.
    pub fn grid(&self) -> Grid {
        self.grid
    }

    /// The icon, unwrapped.
    pub fn into_icon(self) -> Icon {
        self.icon
    }
}

impl Deref for Accepted {
    type Target = Icon;
    fn deref(&self) -> &Icon {
        &self.icon
    }
}
```

## The walk

`validate` is one pass: the root's two rules first, then every element
in document order with the context a rule needs carried down — the
paints a `g` passes to its children, the offset a `g`'s translation adds
to their coordinates, and the tag of the parent, since a `path` inside a
`path` is a known element in a place the vocabulary does not admit. The
two rules about the drawing as a whole, budget and emptiness, come last.

Rules that need a grid — the stroke register, the safe area, the budget
— are not evaluated when the view box declares none. There is no safe
area to be outside of on a grid the profile does not have, and the
off-grid refusal already says what to fix.

<a name="chunk-validate"></a><sub>[`src/validate.rs`](../../../../x0k-icon/src/validate.rs) · `#validate`</sub>

```rust {#validate}
/// Check a parsed icon against every profile rule. `Ok` is the icon,
/// accepted; `Err` is every defect found, in document order, the
/// whole-drawing rules last.
pub fn validate(icon: Icon) -> Result<Accepted, Vec<Defect>> {
    let mut defects = Vec::new();
    check_root(&icon, &mut defects);
    let grid = icon.grid();
    let mut walk = Walk { grid, defects, painted: false };
    let inherited = Inherited { fill: None, stroke: None, offset: (0.0, 0.0), parent: "svg" };
    for el in &icon.children {
        walk.element(el, &inherited);
    }
    if let Some(grid) = grid {
        walk.budget(&icon, grid);
    }
    if !walk.painted {
        walk.report(Rule::NothingPainted, ElementRef::root(), "no element carries a visible paint");
    }
    match (grid, walk.defects.is_empty()) {
        (Some(grid), true) => Ok(Accepted { icon, grid }),
        _ => Err(walk.defects),
    }
}
```

The root is one `<svg>` carrying `viewBox` and nothing else, and that
view box is one of the two grids. A root that is some other element, or
that carries a `width`, an `id`, a hand-written `aria-label`, breaks the
first rule; a view box that is missing or not a grid breaks the second.

<a name="chunk-check-root"></a><sub>[`src/validate.rs`](../../../../x0k-icon/src/validate.rs) · `#check-root`</sub>

```rust {#check-root}
fn check_root(icon: &Icon, defects: &mut Vec<Defect>) {
    let root = ElementRef { ordinal: 0, tag: icon.root_tag.clone() };
    if icon.root_tag != "svg" {
        defects.push(Defect {
            rule: Rule::NotOneRoot,
            element: root.clone(),
            detail: format!("the root is `<{}>`, not `<svg>`", icon.root_tag),
        });
    }
    for name in &icon.root_extra {
        defects.push(Defect {
            rule: Rule::NotOneRoot,
            element: root.clone(),
            detail: format!("the root carries `{name}`; only `viewBox` and `xmlns` are admitted"),
        });
    }
    if icon.grid().is_none() {
        let written = match icon.view_box {
            Some(vb) => format!("viewBox=\"{}\"", vb.map(num).join(" ")),
            None => "no viewBox".to_string(),
        };
        defects.push(Defect {
            rule: Rule::OffGridViewBox,
            element: root,
            detail: format!("{written}; the grids are `0 0 16 16` and `0 0 24 24`"),
        });
    }
}
```

The walk's state is the grid, the defects so far, and one bit: whether
any shape has been seen with a visible paint, which is what rule 14 asks.
What is inherited down the tree is the effective fill and stroke, the
accumulated translation, and the parent's tag.

<a name="chunk-walk-state"></a><sub>[`src/validate.rs`](../../../../x0k-icon/src/validate.rs) · `#walk-state`</sub>

```rust {#walk-state}
struct Walk {
    grid: Option<Grid>,
    defects: Vec<Defect>,
    painted: bool,
}

struct Inherited<'a> {
    fill: Option<&'a Paint>,
    stroke: Option<&'a Paint>,
    offset: (f64, f64),
    parent: &'a str,
}

impl Walk {
    fn report(&mut self, rule: Rule, element: ElementRef, detail: impl Into<String>) {
        self.defects.push(Defect { rule, element, detail: detail.into() });
    }
}
```

An element is checked in the order the rules are numbered, as far as one
element can be: vocabulary (3, 4), paints (5, 6), stroke (7, 8), path
commands (9), geometry (10, 11), transform (13). A foreign element is
reported and left; nothing beneath it was read. Then its children, with
what this element passes down.

<a name="chunk-walk-element"></a><sub>[`src/validate.rs`](../../../../x0k-icon/src/validate.rs) · `#walk-element`</sub>

```rust {#walk-element}
impl Walk {
    fn element(&mut self, el: &Element, inherited: &Inherited<'_>) {
        let at = ElementRef::of(el);
        if let Shape::Foreign(what) = &el.shape {
            self.report(Rule::UnknownElement, at, format!("`{what}` is not in the vocabulary"));
            return;
        }
        if inherited.parent != "svg" && inherited.parent != "g" {
            let detail = format!("`{}` inside a `{}`", el.tag(), inherited.parent);
            self.report(Rule::UnknownElement, at.clone(), detail);
        }
        self.attributes(el, &at);
        self.paints(el, &at);
        self.stroke(el, &at);
        self.path_commands(el, &at);
        let offset = self.transform(el, &at, inherited.offset);
        self.geometry(el, &at, offset);
        let below = Inherited {
            fill: el.fill.as_ref().or(inherited.fill),
            stroke: el.stroke.as_ref().or(inherited.stroke),
            offset,
            parent: el.tag(),
        };
        self.painted |= is_painted(el, &below);
        for child in &el.children {
            self.element(child, &below);
        }
    }
}
```

The parser already sorted each element's attributes against its row, so
rule 4 is a report of what it set aside.

<a name="chunk-check-attributes"></a><sub>[`src/validate.rs`](../../../../x0k-icon/src/validate.rs) · `#check-attributes`</sub>

```rust {#check-attributes}
impl Walk {
    fn attributes(&mut self, el: &Element, at: &ElementRef) {
        for name in &el.unknown_attrs {
            let detail = format!("`{name}` is not admitted on `{}`", el.tag());
            self.report(Rule::UnknownAttribute, at.clone(), detail);
        }
    }
}
```

A paint is a role or `none`; anything else was kept verbatim and is
quoted back. `paper` is the ground and is a fill only — on a stroke it
would draw the ground as a line, which no surface can mean.

<a name="chunk-check-paints"></a><sub>[`src/validate.rs`](../../../../x0k-icon/src/validate.rs) · `#check-paints`</sub>

```rust {#check-paints}
impl Walk {
    fn paints(&mut self, el: &Element, at: &ElementRef) {
        for (attr, paint) in [("fill", &el.fill), ("stroke", &el.stroke)] {
            if let Some(Paint::Literal(value)) = paint {
                let detail = format!("{attr}=\"{value}\"; a paint is ink, line, paper, accent or none");
                self.report(Rule::LiteralPaint, at.clone(), detail);
            }
        }
        if el.stroke == Some(Paint::Role(Role::Paper)) {
            self.report(Rule::PaperAsStroke, at.clone(), "stroke=\"paper\"; paper is a fill only");
        }
    }
}
```

Two stroke weights per grid and no more; one dash pattern, `1 2`, and
no other. The register is read from the grid, so the same declaration
means a different pair of widths on the 24 grid — which is why the check
waits for a grid.

<a name="chunk-check-stroke"></a><sub>[`src/validate.rs`](../../../../x0k-icon/src/validate.rs) · `#check-stroke`</sub>

```rust {#check-stroke}
impl Walk {
    fn stroke(&mut self, el: &Element, at: &ElementRef) {
        if let (Some(width), Some(grid)) = (el.stroke_width, self.grid) {
            if width != grid.regular() && width != grid.bold() {
                let detail = format!(
                    "stroke-width=\"{}\"; the register is {} (regular) and {} (bold)",
                    num(width),
                    num(grid.regular()),
                    num(grid.bold())
                );
                self.report(Rule::OffRegisterStroke, at.clone(), detail);
            }
        }
        if let Some(dash) = &el.dasharray {
            if dash != &[1.0, 2.0] {
                let written = dash.iter().map(|v| num(*v)).collect::<Vec<_>>().join(" ");
                let detail = format!("stroke-dasharray=\"{written}\"; the one dash is `1 2`");
                self.report(Rule::OffProfileDash, at.clone(), detail);
            }
        }
    }
}
```

A path command outside the eight absolute ones was kept as its letter,
and the parser stopped there; one refusal names it.

<a name="chunk-check-path-commands"></a><sub>[`src/validate.rs`](../../../../x0k-icon/src/validate.rs) · `#check-path-commands`</sub>

```rust {#check-path-commands}
impl Walk {
    fn path_commands(&mut self, el: &Element, at: &ElementRef) {
        let Shape::Path { d, .. } = &el.shape else { return };
        for command in d {
            if let PathCommand::Unknown(letter) = command {
                let detail = format!("`{letter}` in d; commands are absolute M L H V C Q A Z");
                self.report(Rule::RelativeOrUnknownPathCommand, at.clone(), detail);
            }
        }
    }
}
```

A transform is admitted on a `g` and only as a translation. The
translation is returned as the offset the element's subtree is drawn at,
because the safe area is a fact about where marks land, not about the
numbers an author wrote before a `g` moved them; its own two values must
sit on the half-unit like any coordinate.

<a name="chunk-check-transform"></a><sub>[`src/validate.rs`](../../../../x0k-icon/src/validate.rs) · `#check-transform`</sub>

```rust {#check-transform}
impl Walk {
    fn transform(&mut self, el: &Element, at: &ElementRef, offset: (f64, f64)) -> (f64, f64) {
        match &el.transform {
            None => offset,
            Some(Transform::Other(text)) => {
                let detail = format!("transform=\"{text}\"; only `translate(x y)` is admitted");
                self.report(Rule::TransformNotATranslation, at.clone(), detail);
                offset
            }
            Some(Transform::Translate(dx, dy)) => {
                if el.tag() != "g" {
                    let detail = format!("a transform on `{}`; only a `g` may carry one", el.tag());
                    self.report(Rule::TransformNotATranslation, at.clone(), detail);
                }
                for (name, v) in [("translate x", *dx), ("translate y", *dy)] {
                    self.half_unit(at, name, v);
                }
                (offset.0 + dx, offset.1 + dy)
            }
        }
    }
}
```

## Geometry: the half-unit and the safe area

Every number an author writes for geometry is checked twice. It must be
a multiple of 0.5 — a hairline centred on a whole unit greys across two
pixel rows — and every point it places must lie in the safe area. The
two checks read different things: the half-unit rule reads *values*
(a radius, a width, an arc's radii), the safe-area rule reads *points*
(where the value puts a mark). A rectangle's far corner is a point the
author wrote by writing a width, so it is checked; a circle's four
extremes are points its radius places; a curve's control points are
checked as the design says, rather than the curve's true extent, so an
author's drawing is judged on the numbers in it.

<a name="chunk-check-geometry"></a><sub>[`src/validate.rs`](../../../../x0k-icon/src/validate.rs) · `#check-geometry`</sub>

```rust {#check-geometry}
impl Walk {
    fn geometry(&mut self, el: &Element, at: &ElementRef, offset: (f64, f64)) {
        let (values, points) = geometry_of(&el.shape);
        for (name, v) in values {
            self.half_unit(at, name, v);
        }
        for (name, (x, y)) in points {
            self.safe_area(at, name, (x + offset.0, y + offset.1));
        }
    }

    fn half_unit(&mut self, at: &ElementRef, name: &str, v: f64) {
        if (v * 2.0).fract() != 0.0 {
            let detail = format!("{name} = {}; coordinates sit on multiples of 0.5", num(v));
            self.report(Rule::OffTheHalfUnit, at.clone(), detail);
        }
    }

    fn safe_area(&mut self, at: &ElementRef, name: &str, (x, y): (f64, f64)) {
        let Some(grid) = self.grid else { return };
        let (lo, hi) = grid.safe_area();
        if !(lo..=hi).contains(&x) || !(lo..=hi).contains(&y) {
            let detail = format!(
                "{name} at ({} {}) leaves the safe area {} … {}",
                num(x),
                num(y),
                num(lo),
                num(hi)
            );
            self.report(Rule::OutsideTheSafeArea, at.clone(), detail);
        }
    }
}
```

Each shape yields its values and its points. For a path the current
point is tracked so `H` and `V` — which write one number — still place a
point; an arc's rotation and flags are neither.

<a name="chunk-geometry-of"></a><sub>[`src/validate.rs`](../../../../x0k-icon/src/validate.rs) · `#geometry-of`</sub>

```rust {#geometry-of}
type Values = Vec<(&'static str, f64)>;
type Points = Vec<(&'static str, (f64, f64))>;

fn geometry_of(shape: &Shape) -> (Values, Points) {
    let mut values = Vec::new();
    let mut points = Vec::new();
    match shape {
        Shape::Group | Shape::Foreign(_) => {}
        Shape::Line { x1, y1, x2, y2 } => {
            values.extend([("x1", *x1), ("y1", *y1), ("x2", *x2), ("y2", *y2)]);
            points.extend([("x1 y1", (*x1, *y1)), ("x2 y2", (*x2, *y2))]);
        }
        Shape::Circle { cx, cy, r } => {
            values.extend([("cx", *cx), ("cy", *cy), ("r", *r)]);
            points.extend([
                ("left extreme", (cx - r, *cy)),
                ("right extreme", (cx + r, *cy)),
                ("top extreme", (*cx, cy - r)),
                ("bottom extreme", (*cx, cy + r)),
            ]);
        }
        Shape::Rect { x, y, width, height, rx } => {
            values.extend([("x", *x), ("y", *y), ("width", *width), ("height", *height)]);
            values.extend(rx.map(|rx| ("rx", rx)));
            points.extend([("corner", (*x, *y)), ("far corner", (x + width, y + height))]);
        }
        Shape::Polyline { points: pts } => {
            for (x, y) in pts {
                values.extend([("point x", *x), ("point y", *y)]);
                points.push(("point", (*x, *y)));
            }
        }
        Shape::Path { d, .. } => path_geometry(d, &mut values, &mut points),
    }
    (values, points)
}
```

<a name="chunk-path-geometry"></a><sub>[`src/validate.rs`](../../../../x0k-icon/src/validate.rs) · `#path-geometry`</sub>

```rust {#path-geometry}
fn path_geometry(d: &[PathCommand], values: &mut Values, points: &mut Points) {
    let mut current = (0.0, 0.0);
    for command in d {
        match command {
            PathCommand::MoveTo(x, y) | PathCommand::LineTo(x, y) => {
                values.extend([("x", *x), ("y", *y)]);
                current = (*x, *y);
                points.push(("point", current));
            }
            PathCommand::Horizontal(x) => {
                values.push(("x", *x));
                current.0 = *x;
                points.push(("point", current));
            }
            PathCommand::Vertical(y) => {
                values.push(("y", *y));
                current.1 = *y;
                points.push(("point", current));
            }
            PathCommand::Cubic([x1, y1, x2, y2, x, y]) => {
                values.extend([("x1", *x1), ("y1", *y1), ("x2", *x2), ("y2", *y2), ("x", *x), ("y", *y)]);
                points.extend([("control point", (*x1, *y1)), ("control point", (*x2, *y2))]);
                current = (*x, *y);
                points.push(("point", current));
            }
            PathCommand::Quadratic([x1, y1, x, y]) => {
                values.extend([("x1", *x1), ("y1", *y1), ("x", *x), ("y", *y)]);
                points.push(("control point", (*x1, *y1)));
                current = (*x, *y);
                points.push(("point", current));
            }
            PathCommand::Arc { rx, ry, x, y, .. } => {
                values.extend([("rx", *rx), ("ry", *ry), ("x", *x), ("y", *y)]);
                current = (*x, *y);
                points.push(("point", current));
            }
            PathCommand::Close | PathCommand::Unknown(_) => {}
        }
    }
}
```

## The drawing as a whole

Two rules read the whole drawing. The budget is the counts the parser
defines, against the grid's ceiling. Emptiness is the `painted` bit the
walk carries: a shape counts as painted when its effective fill or stroke
— its own, or one inherited from a `g` above it — is a role, or a literal.
A literal is refused on its own account, but it would paint, and the
rule here is about a drawing that would emit as *empty*; reporting both
would be two refusals for one mistake. A `line` or `polyline` has no
fill to inherit, and a `g` paints nothing itself.

<a name="chunk-check-budget"></a><sub>[`src/validate.rs`](../../../../x0k-icon/src/validate.rs) · `#check-budget`</sub>

```rust {#check-budget}
impl Walk {
    fn budget(&mut self, icon: &Icon, grid: Grid) {
        let commands = icon.command_count();
        if commands > grid.command_budget() {
            let detail = format!("{commands} drawing commands; the budget is {}", grid.command_budget());
            self.report(Rule::OverBudget, ElementRef::root(), detail);
        }
        let elements = icon.element_count();
        if elements > grid.element_budget() {
            let detail = format!("{elements} elements; the budget is {}", grid.element_budget());
            self.report(Rule::OverBudget, ElementRef::root(), detail);
        }
    }
}

fn is_painted(el: &Element, effective: &Inherited<'_>) -> bool {
    let visible = |paint: Option<&Paint>| matches!(paint, Some(Paint::Role(_) | Paint::Literal(_)));
    match &el.shape {
        Shape::Group | Shape::Foreign(_) => false,
        Shape::Line { .. } | Shape::Polyline { .. } => visible(effective.stroke),
        _ => visible(effective.fill) || visible(effective.stroke),
    }
}
```

## One icon per grid

The fifteenth rule is about a section, not a drawing: a section holds at
most one icon per grid. Which blocks share a section is the business of
the reader that finds them in a document — [`x0k-folio`'s inline-entity
extractor](../folio/inline-entities.md "x0k:implementation/folio/inline-entities"), which this crate
does not link — so the rule is offered as a function over icons already
accepted, for that reader to call with the icons of one section.

<a name="chunk-one-per-grid"></a><sub>[`src/validate.rs`](../../../../x0k-icon/src/validate.rs) · `#one-per-grid`</sub>

```rust {#one-per-grid}
/// Rule 15 over the icons of one section: a second icon on a grid one
/// already occupies is a defect naming the later icon by its position in
/// the slice.
pub fn one_per_grid(icons: &[&Accepted]) -> Vec<Defect> {
    let mut seen: Vec<Grid> = Vec::new();
    let mut defects = Vec::new();
    for (i, icon) in icons.iter().enumerate() {
        if seen.contains(&icon.grid()) {
            defects.push(Defect {
                rule: Rule::TwoIconsOnOneGrid,
                element: ElementRef::root(),
                detail: format!("icon {} of the section is a second on the {} grid", i + 1, num(icon.grid().size())),
            });
        }
        seen.push(icon.grid());
    }
    defects
}
```

Numbers in a refusal are printed the way an author writes them — `15.5`,
not `15.5000` — which is the same rule the emitter uses; it lives in the
[emitter](emit.md) and is borrowed here.

<a name="chunk-num"></a><sub>[`src/validate.rs`](../../../../x0k-icon/src/validate.rs) · `#num`</sub>

```rust {#num}
use crate::emit::num;
```

## What the tests pin

One test per rule, each a small drawing broken one way, so that a rule
whose check silently stops firing is a red test with the rule's name on
it. Three of them are the carried example's own breakages — a literal
colour, a corner past the safe area, a page over budget — and one is the
whole drawing broken three ways at once, pinning that the checker reports
every rule in one pass rather than the first. That is what it means to
[check an icon against the
profile](../../decisions/design/presentation/icon-profile/check-an-icon-against-the-profile.md "x0k:affordance/check_an_icon_against_the_profile"): a list, not
a verdict.

<a name="chunk-tests"></a><sub>[`src/validate.rs`](../../../../x0k-icon/src/validate.rs) · `#tests` · proves [Check an icon against the profile](../../decisions/design/presentation/icon-profile/check-an-icon-against-the-profile.md)</sub>

```rust {#tests proves="x0k:affordance/check_an_icon_against_the_profile"}
#[cfg(test)]
mod tests {
    use super::*;
    use crate::fixtures;
    use crate::parse::parse;

    fn refusals(svg: &str) -> Vec<Rule> {
        match validate(parse(svg).unwrap()) {
            Ok(_) => Vec::new(),
            Err(defects) => defects.into_iter().map(|d| d.rule).collect(),
        }
    }

    fn wrap(body: &str) -> String {
        format!("<svg viewBox=\"0 0 16 16\">{body}</svg>")
    }

    const RING: &str = r#"<circle cx="8" cy="8" r="6" fill="none" stroke="ink" stroke-width="1.5"/>"#;

    #[test]
    fn the_tangle_icon_is_accepted() {
        let accepted = validate(parse(fixtures::TANGLE).unwrap()).unwrap();
        assert_eq!(accepted.grid(), Grid::Sixteen);
        assert_eq!(accepted.command_count(), 16);
    }

    #[test]
    fn a_literal_colour_is_refused_by_rule_5() {
        let svg = fixtures::TANGLE.replace(r#"stroke="ink" stroke-width="1.5""#, r##"stroke="#111111" stroke-width="1.5""##);
        let defects = validate(parse(&svg).unwrap()).unwrap_err();
        assert_eq!(defects.len(), 1);
        assert_eq!(defects[0].rule, Rule::LiteralPaint);
        assert_eq!(defects[0].element, ElementRef { ordinal: 4, tag: "rect".to_string() });
        assert_eq!(
            defects[0].to_string(),
            "rule 5 (a literal paint): rect #4 — stroke=\"#111111\"; a paint is ink, line, paper, accent or none"
        );
    }

    #[test]
    fn a_corner_past_the_safe_area_is_refused_by_rule_11() {
        let svg = fixtures::TANGLE.replace(r#"x="6.5" y="9.5" width="7""#, r#"x="8.5" y="9.5" width="7""#);
        let defects = validate(parse(&svg).unwrap()).unwrap_err();
        assert_eq!(defects.len(), 1);
        assert_eq!(defects[0].rule, Rule::OutsideTheSafeArea);
        assert_eq!(defects[0].element.ordinal, 4);
        assert!(defects[0].detail.starts_with("far corner at (15.5 13.5)"), "{}", defects[0].detail);
    }

    #[test]
    fn a_page_over_budget_is_refused_by_rule_12() {
        let svg = fixtures::TANGLE.replace(
            r#"d="M4.5 6.5 H9 M4.5 8.5 H7""#,
            r#"d="M4.5 6.5 H9 M4.5 8.5 H7 M4.5 10.5 H7 M4.5 11 H7 M4.5 12 H7 M4.5 13 H7 M4.5 14 H7""#,
        );
        let defects = validate(parse(&svg).unwrap()).unwrap_err();
        assert_eq!(defects.len(), 1);
        assert_eq!(defects[0].rule, Rule::OverBudget);
        assert_eq!(defects[0].element.ordinal, 0);
        assert_eq!(defects[0].detail, "26 drawing commands; the budget is 24");
    }

    #[test]
    fn every_broken_rule_is_reported_in_one_pass() {
        let svg = fixtures::TANGLE
            .replace(r#"stroke="ink" stroke-width="1.5""#, r##"stroke="#111111" stroke-width="1.5""##)
            .replace(r#"x="6.5" y="9.5" width="7""#, r#"x="8.5" y="9.5" width="7""#)
            .replace(
                r#"d="M4.5 6.5 H9 M4.5 8.5 H7""#,
                r#"d="M4.5 6.5 H9 M4.5 8.5 H7 M4.5 10.5 H7 M4.5 11 H7 M4.5 12 H7 M4.5 13 H7 M4.5 14 H7""#,
            );
        assert_eq!(refusals(&svg), vec![Rule::LiteralPaint, Rule::OutsideTheSafeArea, Rule::OverBudget]);
    }

    #[test]
    fn each_rule_has_a_drawing_that_breaks_only_it() {
        let cases: Vec<(String, Rule)> = vec![
            (format!("<g viewBox=\"0 0 16 16\">{RING}</g>"), Rule::NotOneRoot),
            (format!("<svg viewBox=\"0 0 16 16\" width=\"16\">{RING}</svg>"), Rule::NotOneRoot),
            (format!("<svg viewBox=\"0 0 20 20\">{RING}</svg>"), Rule::OffGridViewBox),
            (wrap(&format!("{RING}<text x=\"2\" y=\"2\">a</text>")), Rule::UnknownElement),
            (wrap(&format!("{RING}<!-- a comment -->")), Rule::UnknownElement),
            (wrap(&RING.replace("fill=\"none\"", "fill=\"none\" id=\"ring\"")), Rule::UnknownAttribute),
            (wrap(&RING.replace("stroke=\"ink\"", "stroke=\"currentColor\"")), Rule::LiteralPaint),
            (wrap(&RING.replace("stroke=\"ink\"", "stroke=\"paper\"")), Rule::PaperAsStroke),
            (wrap(&RING.replace("stroke-width=\"1.5\"", "stroke-width=\"2\"")), Rule::OffRegisterStroke),
            (wrap(&RING.replace("stroke-width=\"1.5\"", "stroke-width=\"1\" stroke-dasharray=\"2 2\"")), Rule::OffProfileDash),
            (wrap(r#"<path d="M4 4 l8 8" fill="none" stroke="ink" stroke-width="1"/>"#), Rule::RelativeOrUnknownPathCommand),
            (wrap(&RING.replace("r=\"6\"", "r=\"5.25\"")), Rule::OffTheHalfUnit),
            (wrap(&RING.replace("cx=\"8\"", "cx=\"9.5\"")), Rule::OutsideTheSafeArea),
            (wrap(&RING.repeat(9)), Rule::OverBudget),
            (wrap(&format!("<g transform=\"rotate(45)\">{RING}</g>")), Rule::TransformNotATranslation),
            (wrap(&RING.replace("<circle", "<circle transform=\"translate(1 1)\"")), Rule::TransformNotATranslation),
            (wrap(&RING.replace("stroke=\"ink\"", "stroke=\"none\"")), Rule::NothingPainted),
        ];
        for (svg, rule) in cases {
            assert_eq!(refusals(&svg), vec![rule], "{svg}");
        }
    }

    #[test]
    fn a_translation_moves_the_safe_area_check_with_its_subtree() {
        let inside = wrap(&format!("<g transform=\"translate(1 1)\">{}</g>", RING.replace("r=\"6\"", "r=\"5\"").replace("cx=\"8\" cy=\"8\"", "cx=\"7\" cy=\"7\"")));
        assert_eq!(refusals(&inside), vec![]);
        let outside = wrap(&format!("<g transform=\"translate(2 2)\">{RING}</g>"));
        assert_eq!(refusals(&outside), vec![Rule::OutsideTheSafeArea, Rule::OutsideTheSafeArea]);
    }

    #[test]
    fn a_paint_inherited_from_a_group_counts_as_painted() {
        let svg = wrap(r#"<g stroke="ink" stroke-width="1.5"><circle cx="8" cy="8" r="6"/></g>"#);
        assert_eq!(refusals(&svg), vec![]);
    }

    #[test]
    fn a_second_icon_on_one_grid_is_refused_by_rule_15() {
        let a = validate(parse(fixtures::TANGLE).unwrap()).unwrap();
        let b = validate(parse(fixtures::WEAVE).unwrap()).unwrap();
        let defects = one_per_grid(&[&a, &b]);
        assert_eq!(defects.len(), 1);
        assert_eq!(defects[0].rule, Rule::TwoIconsOnOneGrid);
        assert_eq!(one_per_grid(&[&a]), vec![]);
    }
}
```

## Composing the module

<a name="chunk-root"></a><sub>[`src/validate.rs`](../../../../x0k-icon/src/validate.rs) · `#root` · assembles [module-doc](#chunk-module-doc) · [imports](#chunk-imports) · [num](#chunk-num) · [rule](#chunk-rule) · [defect](#chunk-defect) · [accepted](#chunk-accepted) · [validate](#chunk-validate) · [check-root](#chunk-check-root) · [walk-state](#chunk-walk-state) · [walk-element](#chunk-walk-element) · [check-attributes](#chunk-check-attributes) · [check-paints](#chunk-check-paints) · [check-stroke](#chunk-check-stroke) · [check-path-commands](#chunk-check-path-commands) · [check-transform](#chunk-check-transform) · [check-geometry](#chunk-check-geometry) · [geometry-of](#chunk-geometry-of) · [path-geometry](#chunk-path-geometry) · [check-budget](#chunk-check-budget) · [one-per-grid](#chunk-one-per-grid) · [tests](#chunk-tests)</sub>

```rust {#root}
<<module-doc>>

<<imports>>

<<num>>

<<rule>>

<<defect>>

<<accepted>>

<<validate>>

<<check-root>>

<<walk-state>>

<<walk-element>>

<<check-attributes>>

<<check-paints>>

<<check-stroke>>

<<check-path-commands>>

<<check-transform>>

<<check-geometry>>

<<geometry-of>>

<<path-geometry>>

<<check-budget>>

<<one-per-grid>>

<<tests>>
```

What the checker cannot refuse, the design says outright: whether the
mark depicts its subject. Every rule above is a rule about *form* — where
a number sits, what a paint is named, how much a drawing spends — and a
drawing can satisfy all fifteen and still be a blot. That judgment is a
reader's, and the profile leaves it there; the checker's value is that
when a reader says "this does not read", the reason is never a rule the
machine could have caught.
