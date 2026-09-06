---
x0k:
  format: folio/v1
  id: x0k:implementation/icon/bind
  type: implementation
  status: draft
  summary: "Binding an accepted icon's four roles to strings — a publication's palette block read by serde, one colour per role per scheme; the theme's CSS variables; or the role names themselves — so the emitter writes one form of SVG whatever a surface resolves a paint to."
  concerns: [icons, svg, themes, palette, publishing]
  tangle:
    crate: x0k-icon
    root: src/bind.rs
  edges:
    implements:
      - x0k:design/icon-profile
    cites:
      - x0k:architecture/entity-iconography
      - x0k:architecture/publication-is-the-shipping-unit
      - x0k:design/theme-system
      - x0k:implementation/icon/validate
      - x0k:implementation/icon/emit
---
# Binding roles

An icon names no colour. Its paints are four words — `ink`, `line`,
`paper`, `accent` — and something outside the icon says what each word is
on the surface where the icon is shown. The
[profile](x0k:design/icon-profile) names two such things. A theme binds
roles to *roles*: `ink = "text"`, so an icon in the shell is dark on the
light scheme and light on the dark one with nothing more said. A
publication binds roles to *colours*: a projected repository has no theme
document in reach, so it carries a palette block of its own, the four
roles bound to literals once per scheme, and the projector tints every
icon it ships from that.

This chapter is the second of those, and the small type underneath both.
The observation that makes it small is that the emitter does not care
what a role becomes. A file on GitHub wants `#111111`; a woven page wants
`var(--icon-ink)` for the theme's stylesheet to resolve live; the
normalized form of a declaration wants the word `ink` back. Those are
three bindings of the same four roles to four strings, and one emitter
writes all three. So binding is: replace every role in an accepted icon
with the string a [`RoleBinding`](#role-binding) gives it, and hand the
result — a [`BoundIcon`](#bound-icon), paints now plain text — to the
[emitter](emit.md).

The carried example on `x0k-folio`'s own palette: the page's `line`
strokes become `#b88e44` on the light scheme and `#96b4dc` on the dark;
the block's `ink` becomes `#111111` and `#e2e8f0`; its `paper` fill,
`#fffff8` and `#1e293b`. Same drawing, two files.

<a name="chunk-module-doc"></a><sub>[`src/bind.rs`](../../../x0k-icon/src/bind.rs) · `#module-doc`</sub>

```rust {#module-doc}
//! Binding an accepted icon's paint roles to the strings a surface
//! resolves them to.
//!
//! [`Palette`] is a publication's palette block as `x0k:design/icon-profile`
//! § "The paints" writes it — one [`RoleBinding`] per [`Scheme`] — read by
//! serde. [`RoleBinding::css_variables`] is the binding a woven page or a
//! sprite uses, and [`RoleBinding::names`] is the identity binding the
//! normalized declaration is written with. [`bind`] applies one to an
//! [`Accepted`] icon and yields a [`BoundIcon`], whose paints are strings
//! the emitter writes verbatim.
```

<a name="chunk-imports"></a><sub>[`src/bind.rs`](../../../x0k-icon/src/bind.rs) · `#imports`</sub>

```rust {#imports}
use serde::{Deserialize, Serialize};

use crate::parse::{Element, Grid, Paint, Role, Shape, Transform};
use crate::validate::Accepted;
```

## A binding of the four roles

Four strings, one per role, in the order the design's palette block
writes them. The struct's field names are the block's keys, which is what
lets a publication's YAML deserialize into it without a mapping layer;
the derive refuses a fifth key, so a palette that binds a role the
profile does not have — the `signal` the design leaves open — fails to
read rather than silently binding nothing.

<a name="chunk-role-binding"></a><sub>[`src/bind.rs`](../../../x0k-icon/src/bind.rs) · `#role-binding`</sub>

```rust {#role-binding}
/// The four roles bound to four strings — colours from a palette, CSS
/// variables, or the role names themselves.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RoleBinding {
    pub ink: String,
    pub line: String,
    pub paper: String,
    pub accent: String,
}

impl RoleBinding {
    /// What the binding makes of a role.
    pub fn get(&self, role: Role) -> &str {
        match role {
            Role::Ink => &self.ink,
            Role::Line => &self.line,
            Role::Paper => &self.paper,
            Role::Accent => &self.accent,
        }
    }
}
```

Two bindings are the crate's own. The CSS-variable binding writes each
role as `var(--icon-<role>)`, which the theme's codegen aliases onto the
theme's declared role bindings; a woven page, the site and the sprite all
use it, and the scheme resolves live. The name binding writes each role
as itself — it is what makes the normalized declaration one more output
of the same emitter rather than a second writer.

<a name="chunk-crate-bindings"></a><sub>[`src/bind.rs`](../../../x0k-icon/src/bind.rs) · `#crate-bindings`</sub>

```rust {#crate-bindings}
impl RoleBinding {
    /// Each role as the CSS custom property a themed page resolves:
    /// `var(--icon-ink)` and so on.
    pub fn css_variables() -> RoleBinding {
        RoleBinding::from_fn(|role| format!("var(--icon-{role})"))
    }

    /// Each role as its own name — the binding a declaration is
    /// normalized under.
    pub fn names() -> RoleBinding {
        RoleBinding::from_fn(|role| role.name().to_string())
    }

    fn from_fn(f: impl Fn(Role) -> String) -> RoleBinding {
        RoleBinding { ink: f(Role::Ink), line: f(Role::Line), paper: f(Role::Paper), accent: f(Role::Accent) }
    }
}
```

## The publication's palette

A palette is a binding per scheme, and the schemes are two. The type is
the shape of the design's block exactly:

```yaml
palette:
  light: { ink: "#111111", line: "#b88e44", paper: "#fffff8", accent: "#b88e44" }
  dark:  { ink: "#e2e8f0", line: "#96b4dc", paper: "#1e293b", accent: "#96b4dc" }
```

The projector reads the block out of the publication's envelope and
hands the `palette:` value here; what a colour string *is* — hex, a
named colour — the crate does not check, because the emitter writes it
into an attribute and the surface's renderer is the authority on colour
syntax. What it does guarantee is that both schemes are bound: a palette
with only `light` does not read.

<a name="chunk-scheme"></a><sub>[`src/bind.rs`](../../../x0k-icon/src/bind.rs) · `#scheme`</sub>

```rust {#scheme}
/// The two colour schemes a surface resolves. Where a surface cannot
/// resolve a variable, the emitter writes one artifact per scheme.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Scheme {
    Light,
    Dark,
}

impl Scheme {
    /// Both schemes, light first — the order per-scheme files are emitted in.
    pub const BOTH: [Scheme; 2] = [Scheme::Light, Scheme::Dark];

    /// The scheme's name, as a file suffix and a palette key.
    pub fn name(self) -> &'static str {
        match self {
            Scheme::Light => "light",
            Scheme::Dark => "dark",
        }
    }
}
```

<a name="chunk-palette"></a><sub>[`src/bind.rs`](../../../x0k-icon/src/bind.rs) · `#palette`</sub>

```rust {#palette}
/// A publication's palette block: the four roles bound to colours, once
/// per scheme (`x0k:design/icon-profile` § "The paints").
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Palette {
    pub light: RoleBinding,
    pub dark: RoleBinding,
}

impl Palette {
    /// The binding for one scheme.
    pub fn scheme(&self, scheme: Scheme) -> &RoleBinding {
        match scheme {
            Scheme::Light => &self.light,
            Scheme::Dark => &self.dark,
        }
    }
}
```

## The bound icon

A bound icon is the accepted icon's tree with every paint a string, and
with the two facts validation established made structural: a transform
is a translation on a `g`, so it is a pair of numbers; a paint is a role
or `none`, so it is a string or absent. What the emitter needs and
nothing it does not.

<a name="chunk-bound-icon"></a><sub>[`src/bind.rs`](../../../x0k-icon/src/bind.rs) · `#bound-icon`</sub>

```rust {#bound-icon}
/// An accepted icon with its roles bound: what the emitter writes.
#[derive(Debug, Clone, PartialEq)]
pub struct BoundIcon {
    pub grid: Grid,
    pub elements: Vec<BoundElement>,
}

/// One element of a bound icon. `fill` and `stroke` are attribute values
/// as they will be written — a colour, a variable, a role name, or
/// `none` — or absent where the author wrote nothing.
#[derive(Debug, Clone, PartialEq)]
pub struct BoundElement {
    pub shape: Shape,
    pub fill: Option<String>,
    pub stroke: Option<String>,
    pub stroke_width: Option<f64>,
    pub dasharray: Option<Vec<f64>>,
    pub translate: Option<(f64, f64)>,
    pub children: Vec<BoundElement>,
}
```

Binding is a walk that replaces. Nothing is computed and nothing is
decided: the checker decided, and the binding says what each word means
here.

<a name="chunk-bind"></a><sub>[`src/bind.rs`](../../../x0k-icon/src/bind.rs) · `#bind`</sub>

```rust {#bind}
/// Bind an accepted icon's roles under one binding.
pub fn bind(icon: &Accepted, binding: &RoleBinding) -> BoundIcon {
    BoundIcon {
        grid: icon.grid(),
        elements: icon.children.iter().map(|el| bind_element(el, binding)).collect(),
    }
}

fn bind_element(el: &Element, binding: &RoleBinding) -> BoundElement {
    let paint = |paint: &Option<Paint>| {
        paint.as_ref().map(|p| match p {
            Paint::None => "none".to_string(),
            Paint::Role(role) => binding.get(*role).to_string(),
            Paint::Literal(text) => text.clone(),
        })
    };
    BoundElement {
        shape: el.shape.clone(),
        fill: paint(&el.fill),
        stroke: paint(&el.stroke),
        stroke_width: el.stroke_width,
        dasharray: el.dasharray.clone(),
        translate: match el.transform {
            Some(Transform::Translate(x, y)) => Some((x, y)),
            _ => None,
        },
        children: el.children.iter().map(|child| bind_element(child, binding)).collect(),
    }
}
```

The `Literal` arm is unreachable on an `Accepted` icon — the checker
refuses every literal — and it passes the text through rather than
panicking, because a binder that can bring a paint pass down is a binder
in the wrong place to raise an alarm. The type is the guard; the arm is
what the compiler asks for.

## What the tests pin

The design's own palette block reads into the type as written, flow
mappings and all; a block with a fifth role does not. Binding the carried
example under it yields the colours the chapter opened with, and under
the CSS binding yields variables. Binding is how an icon is [shown on a
surface](../../../decisions/design/presentation/icon-profile/show-an-icon-on-any-surface.md "x0k:affordance/show_an_icon_on_a_surface") in that surface's
colours without the drawing changing.

<a name="chunk-tests"></a><sub>[`src/bind.rs`](../../../x0k-icon/src/bind.rs) · `#tests` · proves [Show an icon on any surface](../../../decisions/design/presentation/icon-profile/show-an-icon-on-any-surface.md)</sub>

```rust {#tests proves="x0k:affordance/show_an_icon_on_a_surface"}
#[cfg(test)]
mod tests {
    use super::*;
    use crate::fixtures;
    use crate::parse::parse;
    use crate::validate::validate;

    const PALETTE: &str = r##"
light: { ink: "#111111", line: "#b88e44", paper: "#fffff8", accent: "#b88e44" }
dark:  { ink: "#e2e8f0", line: "#96b4dc", paper: "#1e293b", accent: "#96b4dc" }
"##;

    #[test]
    fn the_publications_palette_block_reads_as_written() {
        let palette: Palette = serde_norway::from_str(PALETTE).unwrap();
        assert_eq!(palette.light.ink, "#111111");
        assert_eq!(palette.scheme(Scheme::Dark).get(Role::Paper), "#1e293b");
    }

    #[test]
    fn a_fifth_role_or_a_missing_scheme_does_not_read() {
        let fifth = PALETTE.replace(r##"accent: "#b88e44" }"##, r##"accent: "#b88e44", signal: "#c00" }"##);
        assert!(serde_norway::from_str::<Palette>(&fifth).is_err());
        let light_only = PALETTE.lines().take(2).collect::<Vec<_>>().join("\n");
        assert!(serde_norway::from_str::<Palette>(&light_only).is_err());
    }

    #[test]
    fn the_tangle_icon_binds_to_the_folio_palette() {
        let palette: Palette = serde_norway::from_str(PALETTE).unwrap();
        let icon = validate(parse(fixtures::TANGLE).unwrap()).unwrap();
        let light = bind(&icon, &palette.light);
        let dark = bind(&icon, &palette.dark);
        assert_eq!(light.elements[0].stroke.as_deref(), Some("#b88e44"));
        assert_eq!(dark.elements[0].stroke.as_deref(), Some("#96b4dc"));
        assert_eq!(light.elements[3].fill.as_deref(), Some("#fffff8"));
        assert_eq!(light.elements[3].stroke.as_deref(), Some("#111111"));
        assert_eq!(light.elements[0].fill.as_deref(), Some("none"));
    }

    #[test]
    fn the_css_binding_writes_variables_and_the_name_binding_writes_roles() {
        let icon = validate(parse(fixtures::TANGLE).unwrap()).unwrap();
        let css = bind(&icon, &RoleBinding::css_variables());
        assert_eq!(css.elements[3].stroke.as_deref(), Some("var(--icon-ink)"));
        let names = bind(&icon, &RoleBinding::names());
        assert_eq!(names.elements[3].fill.as_deref(), Some("paper"));
    }
}
```

## Composing the module

<a name="chunk-root"></a><sub>[`src/bind.rs`](../../../x0k-icon/src/bind.rs) · `#root` · assembles [module-doc](#chunk-module-doc) · [imports](#chunk-imports) · [role-binding](#chunk-role-binding) · [crate-bindings](#chunk-crate-bindings) · [scheme](#chunk-scheme) · [palette](#chunk-palette) · [bound-icon](#chunk-bound-icon) · [bind](#chunk-bind) · [tests](#chunk-tests)</sub>

```rust {#root}
<<module-doc>>

<<imports>>

<<role-binding>>

<<crate-bindings>>

<<scheme>>

<<palette>>

<<bound-icon>>

<<bind>>

<<tests>>
```

What binding deliberately does not know is whether the colours it was
handed are legible on their ground. The profile adds no second contrast
rule because a theme binds to a pair
[`semantic-color-roles`](x0k:design/semantic-color-roles) already
checks, and a publication's block is a handful of literals its author
chose against its own plates. That leaves one place a bad pair can enter
— a palette block written carelessly — and this crate lets it through on
purpose: the check belongs with the colour system, and an icon binder
that graded palettes would be a third opinion on a question two designs
already own.
