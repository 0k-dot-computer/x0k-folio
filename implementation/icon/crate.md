---
x0k:
  format: folio/v1
  id: x0k:implementation/icon/crate
  type: implementation
  status: draft
  summary: The crate's contract rather than a mechanism — the four chapters composed, the one face that checks a declaration end to end, the design's worked-example icons carried as fixtures every chapter tests against, and what a consumer may name.
  concerns:
  - icons
  - svg
  - crate
  - publishing
  - profile
  tangle:
    crate: crates/x0k-icon
    root: src/lib.rs
  edges:
    implements:
    - x0k:design/icon-profile
    cites:
    - x0k:architecture/entity-iconography
    - x0k:publication/x0k-folio
    - x0k:implementation/icon/parse
    - x0k:implementation/icon/validate
    - x0k:implementation/icon/bind
    - x0k:implementation/icon/emit
---
# x0k-icon: the crate

There is one icon language in x0k and one implementation of it. The
[`entity-iconography` ADR](x0k:architecture/entity-iconography) decides
both: a mark is a small, role-painted SVG in the
[profile](x0k:design/icon-profile), declared where the thing it depicts is
declared, and one crate reads, checks, tints and writes it for every
surface — the projector that builds a published repository and the shell
that paints a rail both link this crate, so a mark drawn once is the same
mark on both. That is why it is a crate and not Gallowglass on a cell: it
runs inside `cargo build` of a repository that ships no evaluator, and
inside a paint pass that waits on nothing, and it holds no state a cell's
fold would guard. It ships as a module of
[`x0k-folio`](x0k:publication/x0k-folio), because the projector shipped
there cannot draw without it.

This chapter is the crate's contract. The mechanism is four chapters,
read in the order a declaration travels: [parse](parse.md) reads the
block into a typed tree, [validate](validate.md) refuses it by rule or
accepts it, [bind](bind.md) says what each of its four roles is on a
surface, and [emit](emit.md) writes the forms a build needs. Here is what
composes them: the module list, the one face that runs the first two
together, and the fixtures — the design's own worked-example icons,
carried verbatim so every chapter tests against the marks the design
drew rather than marks of its own.

<a name="chunk-crate-doc"></a><sub>[`src/lib.rs`](../../crates/x0k-icon/src/lib.rs) · `#crate-doc`</sub>

```rust {#crate-doc}
//! The icon profile's one implementation: read a constrained,
//! role-painted SVG mark, refuse it by rule or accept it, bind its four
//! roles to a palette, and write every textual form a surface consumes.
//!
//! An icon is a fenced `svg x0k:icon` block declared on the entity it
//! depicts, in the profile `x0k:design/icon-profile` fixes: a 16-unit
//! grid (or 24), a one-unit safe area, two stroke weights, six elements,
//! a detail budget, and paints named as four roles — `ink`, `line`,
//! `paper`, `accent` — never as colours. The crate's verbs follow the
//! declaration's path:
//!
//! - **parse** — [`parse::parse`]: the block into an [`Icon`], kept
//!   permissive so the checker can name every rule at once.
//! - **validate** — [`validate::validate`]: every refusal rule of the
//!   profile, each breach a [`Defect`] naming its [`Rule`] and element;
//!   an accepted icon becomes an [`Accepted`], the type the rest takes.
//! - **bind** — [`bind::bind`]: roles to strings under a [`RoleBinding`]
//!   — a publication's [`Palette`] per scheme, CSS variables, or the
//!   role names.
//! - **emit** — [`emit::files`], [`emit::inline_svg`], [`emit::sprite`],
//!   [`emit::normalized`]: per-scheme files, inline SVG, a symbol sprite,
//!   and the declaration's canonical text.
//!
//! [`check`] runs the first two as one call. The path form the native
//! shell paints is `x0k-ui-draw`'s adapter over [`Accepted`]; the raster
//! a favicon needs is not this crate's.
```

## The modules and what a consumer may name

Every module is public — the crate is a library of parts — and the
re-exports are the names a consumer uses without knowing which chapter
owns them. The fixtures module exists only under test; it is the
design's icons, and a consumer wanting them reads the design.

<a name="chunk-modules"></a><sub>[`src/lib.rs`](../../crates/x0k-icon/src/lib.rs) · `#modules`</sub>

```rust {#modules}
pub mod bind;
pub mod emit;
pub mod parse;
pub mod validate;

#[cfg(test)]
pub(crate) mod fixtures;
```

<a name="chunk-exports"></a><sub>[`src/lib.rs`](../../crates/x0k-icon/src/lib.rs) · `#exports`</sub>

```rust {#exports}
pub use bind::{bind, BoundElement, BoundIcon, Palette, RoleBinding, Scheme};
pub use emit::{files, inline_svg, normalized, sprite, stem_of, svg, Label, SVG_NAMESPACE};
pub use parse::{parse, Element, Grid, Icon, Paint, ParseError, PathCommand, Role, Shape, Transform};
pub use validate::{one_per_grid, validate, Accepted, Defect, ElementRef, Rule};
```

## The face

An author, or the composing loop, has a block of text and one question:
is this an icon, and if not, why not. `check` is that question asked
once. It parses and validates, and its refusal is one of two kinds —
text that is not a declaration at all, or a declaration that breaks
rules, all of them listed — because the two are answered differently:
the first is fixed by writing SVG, the second by reading the rule.

<a name="chunk-refusal"></a><sub>[`src/lib.rs`](../../crates/x0k-icon/src/lib.rs) · `#refusal`</sub>

```rust {#refusal}
/// Why [`check`] refused a text: it was not a declaration, or it was one
/// that broke the rules listed.
#[derive(Debug, Clone, PartialEq)]
pub enum Refusal {
    /// Not well-formed, or a coordinate that is not a number.
    Malformed(ParseError),
    /// Every profile rule the declaration broke, in document order.
    Rules(Vec<Defect>),
}

impl std::fmt::Display for Refusal {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Refusal::Malformed(e) => write!(f, "not a declaration: {e}"),
            Refusal::Rules(defects) => {
                for (i, defect) in defects.iter().enumerate() {
                    if i > 0 {
                        f.write_str("\n")?;
                    }
                    write!(f, "{defect}")?;
                }
                Ok(())
            }
        }
    }
}

impl std::error::Error for Refusal {}
```

<a name="chunk-check"></a><sub>[`src/lib.rs`](../../crates/x0k-icon/src/lib.rs) · `#check`</sub>

```rust {#check}
/// Read a declaration and check it against the profile in one call:
/// the accepted icon, or the [`Refusal`] that says which rules it broke
/// and where — never a silently altered drawing.
pub fn check(text: &str) -> Result<Accepted, Refusal> {
    let icon = parse(text).map_err(Refusal::Malformed)?;
    validate(icon).map_err(Refusal::Rules)
}
```

That function is how a person or an agent holding this library [checks
an icon against the
profile](../../decisions/design/presentation/icon-profile/check-an-icon-against-the-profile.md "x0k:affordance/check_an_icon_against_the_profile"); its rustdoc
is the cue, and the record of *via what* lives beside the face:

<a name="folio-instance-68747470733a2f2f306b2e636f6d70757465722f6f6e746f6c6f6779237369676e69666965722f78306b2d69636f6e2d636865636b-1"></a><sub data-instance-iri="https://0k.computer/ontology#signifier/x0k-icon-check" data-concept-iri="https://0k.computer/ontology#Signifier" data-source-document="corpora/x0k/implementation/icon/crate.md"><strong>Signifier</strong> · The face · <code>https://0k.computer/ontology#signifier/x0k-icon-check</code> · <a href="#folio-source-68747470733a2f2f306b2e636f6d70757465722f6f6e746f6c6f6779237369676e69666965722f78306b2d69636f6e2d636865636b-1">source declaration</a></sub><a name="folio-source-68747470733a2f2f306b2e636f6d70757465722f6f6e746f6c6f6779237369676e69666965722f78306b2d69636f6e2d636865636b-1"></a>

```yaml x0k:signifier
id: x0k:signifier/x0k-icon-check
cue: check
edges:
  signifies:
    - x0k:affordance/check_an_icon_against_the_profile
  presentedOn:
    - x0k:surface/sdk
```

## The first inhabitants, as fixtures

The design draws thirteen icons — four affordance marks sharing a
document motif, three actors, and two ring families of three — and each
chapter's tests use them: the tangle mark is the carried example
throughout, and this chapter holds the whole population to one standard.
They are copied here verbatim, in the design's order, because a crate
under test in a published repository may not have the design in reach;
and when the design *is* in reach, a test below reads it and refuses any
drift between the two, so the copies cannot quietly become a second set
of drawings.

<a name="chunk-fixtures-doc"></a><sub>[`src/fixtures.rs`](../../crates/x0k-icon/src/fixtures.rs) · `#fixtures-doc`</sub>

```rust {#fixtures-doc file="src/fixtures.rs"}
//! The icon profile's first inhabitants, verbatim from
//! `x0k:design/icon-profile` § "The first inhabitants", in the design's
//! order. Test fixtures only; the design is the declaration.
```

<a name="chunk-fixtures"></a><sub>[`src/fixtures.rs`](../../crates/x0k-icon/src/fixtures.rs) · `#fixtures`</sub>

```rust {#fixtures file="src/fixtures.rs"}
/// Tangle — project source code out of a document.
pub const TANGLE: &str = r#"<svg viewBox="0 0 16 16">
  <path d="M2.5 1.5 H8.5 L11 4 V14.5 H2.5 Z" fill="none" stroke="line" stroke-width="1"/>
  <path d="M8.5 1.5 V4 H11" fill="none" stroke="line" stroke-width="1"/>
  <path d="M4.5 6.5 H9 M4.5 8.5 H7" fill="none" stroke="line" stroke-width="1"/>
  <rect x="6.5" y="9.5" width="7" height="4" rx="0.5" fill="paper" stroke="ink" stroke-width="1.5"/>
  <path d="M8 11.5 H12" fill="none" stroke="ink" stroke-width="1"/>
</svg>
"#;

/// Weave — read a document as the woven artifact.
pub const WEAVE: &str = r#"<svg viewBox="0 0 16 16">
  <path d="M2 1.5 H6 L7.5 3 V14.5 H2 Z" fill="none" stroke="line" stroke-width="1"/>
  <path d="M6 1.5 V3 H7.5" fill="none" stroke="line" stroke-width="1"/>
  <path d="M3.5 6 H6 M3.5 8 H6" fill="none" stroke="line" stroke-width="1"/>
  <path d="M7.5 1.5 L13.5 4 V12 L7.5 14.5 Z" fill="paper" stroke="ink" stroke-width="1.5"/>
  <path d="M9 6.5 H12 M9 8.5 H12 M9 10.5 H11" fill="none" stroke="ink" stroke-width="1"/>
</svg>
"#;

/// Check — check a document against its vocabulary.
pub const CHECK: &str = r#"<svg viewBox="0 0 16 16">
  <path d="M2 1.5 H7.5 L10 4 V14.5 H2 Z" fill="none" stroke="line" stroke-width="1"/>
  <path d="M7.5 1.5 V4 H10" fill="none" stroke="line" stroke-width="1"/>
  <path d="M3.5 6.5 H8 M3.5 8.5 H8" fill="none" stroke="line" stroke-width="1"/>
  <circle cx="11.5" cy="11" r="3.5" fill="paper" stroke="ink" stroke-width="1.5"/>
  <path d="M9.5 11 L11 12.5 L13.5 9.5" fill="none" stroke="ink" stroke-width="1.5"/>
</svg>
"#;

/// Affordances — read an affordance out of a document.
pub const AFFORDANCES: &str = r#"<svg viewBox="0 0 16 16">
  <path d="M2.5 3.5 H8.5 L11 6 V14.5 H2.5 Z" fill="none" stroke="line" stroke-width="1"/>
  <path d="M8.5 3.5 V6 H11" fill="none" stroke="line" stroke-width="1"/>
  <path d="M4.5 8 H8" fill="none" stroke="line" stroke-width="1"/>
  <rect x="4.5" y="9.5" width="4.5" height="3" fill="none" stroke="line" stroke-width="1" stroke-dasharray="1 2"/>
  <rect x="9" y="1" width="5" height="3" rx="0.5" fill="paper" stroke="ink" stroke-width="1.5"/>
  <path d="M10.5 2.5 H12.5" fill="none" stroke="ink" stroke-width="1"/>
</svg>
"#;

/// A person — the Human class.
pub const PERSON: &str = r#"<svg viewBox="0 0 16 16">
  <circle cx="8" cy="4.5" r="2.5" fill="none" stroke="ink" stroke-width="1.5"/>
  <path d="M2.5 14.5 V13 Q2.5 9 8 9 Q13.5 9 13.5 13 V14.5" fill="none" stroke="ink" stroke-width="1.5"/>
</svg>
"#;

/// An agent — the AIAgent class.
pub const AGENT: &str = r#"<svg viewBox="0 0 16 16">
  <rect x="4" y="4" width="8" height="8" rx="1" fill="none" stroke="ink" stroke-width="1.5"/>
  <path d="M6 4 V2 M10 4 V2 M6 12 V14 M10 12 V14 M4 6 H2 M4 10 H2 M12 6 H14 M12 10 H14" fill="none" stroke="line" stroke-width="1"/>
  <rect x="6.5" y="6.5" width="3" height="3" fill="ink"/>
</svg>
"#;

/// Both — the Actor genus.
pub const ACTOR: &str = r#"<svg viewBox="0 0 16 16">
  <circle cx="5" cy="5" r="2" fill="none" stroke="ink" stroke-width="1.5"/>
  <path d="M1.5 13.5 V12 Q1.5 8.5 5 8.5 Q8.5 8.5 8.5 12 V13.5" fill="none" stroke="ink" stroke-width="1.5"/>
  <rect x="9" y="7.5" width="5.5" height="5.5" rx="0.5" fill="paper" stroke="ink" stroke-width="1.5"/>
  <path d="M10.5 7.5 V6 M13 7.5 V6 M10.5 13 V14.5 M13 13 V14.5" fill="none" stroke="line" stroke-width="1"/>
  <rect x="11" y="9.5" width="1.5" height="1.5" fill="ink"/>
</svg>
"#;

/// proven.
pub const PROVEN: &str = r#"<svg viewBox="0 0 16 16">
  <circle cx="8" cy="8" r="6" fill="none" stroke="ink" stroke-width="1.5"/>
  <circle cx="8" cy="8" r="3" fill="ink"/>
</svg>
"#;

/// declared.
pub const DECLARED: &str = r#"<svg viewBox="0 0 16 16">
  <circle cx="8" cy="8" r="6" fill="none" stroke="ink" stroke-width="1.5"/>
</svg>
"#;

/// claimed.
pub const CLAIMED: &str = r#"<svg viewBox="0 0 16 16">
  <circle cx="8" cy="8" r="6" fill="none" stroke="line" stroke-width="1" stroke-dasharray="1 2"/>
</svg>
"#;

/// passed.
pub const PASSED: &str = r#"<svg viewBox="0 0 16 16">
  <circle cx="8" cy="8" r="6" fill="none" stroke="ink" stroke-width="1.5"/>
  <path d="M5 8 L7 10.5 L11 5.5" fill="none" stroke="ink" stroke-width="1.5"/>
</svg>
"#;

/// failed.
pub const FAILED: &str = r#"<svg viewBox="0 0 16 16">
  <circle cx="8" cy="8" r="6" fill="none" stroke="ink" stroke-width="1.5"/>
  <path d="M5.5 5.5 L10.5 10.5 M10.5 5.5 L5.5 10.5" fill="none" stroke="ink" stroke-width="1.5"/>
</svg>
"#;

/// not run.
pub const NOT_RUN: &str = r#"<svg viewBox="0 0 16 16">
  <circle cx="8" cy="8" r="6" fill="none" stroke="line" stroke-width="1" stroke-dasharray="1 2"/>
  <path d="M5.5 8 H10.5" fill="none" stroke="line" stroke-width="1"/>
</svg>
"#;

/// Every inhabitant with its name and the command and element counts
/// the design states for it, in the design's order.
pub fn all() -> Vec<(&'static str, &'static str, usize, usize)> {
    vec![
        ("tangle", TANGLE, 16, 5),
        ("weave", WEAVE, 24, 5),
        ("check", CHECK, 17, 5),
        ("affordances", AFFORDANCES, 15, 6),
        ("person", PERSON, 6, 2),
        ("agent", AGENT, 18, 3),
        ("actor", ACTOR, 16, 5),
        ("proven", PROVEN, 2, 2),
        ("declared", DECLARED, 1, 1),
        ("claimed", CLAIMED, 1, 1),
        ("passed", PASSED, 4, 2),
        ("failed", FAILED, 5, 2),
        ("not run", NOT_RUN, 3, 2),
    ]
}
```

## What the tests pin

Every one of the thirteen parses, is accepted with no defect, spends
exactly the commands and elements the design says it does, and
normalizes to its own bytes — which is what makes them the profile's
first inhabitants and not merely its illustrations. When the design file
is reachable from the crate, its `svg x0k:!icon` blocks are read and
compared to the copies, in order. And `check` refuses a broken drawing
with the rules it broke, listed, which is the one thing an author
[declaring an icon](x0k:affordance/declare_an_icon) needs the crate to
say.

<a name="chunk-tests"></a><sub>[`src/lib.rs`](../../crates/x0k-icon/src/lib.rs) · `#tests` · proves `x0k:affordance/declare_an_icon`</sub>

```rust {#tests proves="x0k:affordance/declare_an_icon"}
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_inhabitant_is_accepted_and_spends_what_the_design_says() {
        for (name, svg, commands, elements) in fixtures::all() {
            let icon = check(svg).unwrap_or_else(|refusal| panic!("{name}: {refusal}"));
            assert_eq!(icon.grid(), Grid::Sixteen, "{name}");
            assert_eq!(icon.command_count(), commands, "{name}: commands");
            assert_eq!(icon.element_count(), elements, "{name}: elements");
        }
    }

    #[test]
    fn every_inhabitant_normalizes_to_its_own_bytes() {
        for (name, svg, _, _) in fixtures::all() {
            let icon = check(svg).unwrap();
            assert_eq!(normalized(&icon), svg, "{name}");
        }
    }

    #[test]
    fn the_fixtures_are_the_designs_icons() {
        let design = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../decisions/design/presentation/icon-profile.md");
        let Ok(text) = std::fs::read_to_string(&design) else {
            return;
        };
        let blocks: Vec<&str> = text
            .split("```svg x0k:!icon\n")
            .skip(1)
            .map(|rest| rest.split("```").next().unwrap())
            .collect();
        let copies: Vec<&str> = fixtures::all().into_iter().map(|(_, svg, _, _)| svg).collect();
        assert_eq!(blocks, copies, "the fixtures drifted from the design's blocks");
    }

    #[test]
    fn check_refuses_by_rule_and_names_the_element() {
        let broken = fixtures::PERSON.replace(r#"stroke="ink""#, r##"stroke="#111111""##);
        let Err(Refusal::Rules(defects)) = check(&broken) else {
            panic!("a literal paint must be refused");
        };
        assert_eq!(defects.iter().map(|d| d.rule).collect::<Vec<_>>(), vec![Rule::LiteralPaint, Rule::LiteralPaint]);
        assert_eq!(defects[1].element.ordinal, 2);
        assert!(matches!(check("<svg"), Err(Refusal::Malformed(ParseError::Xml(_)))));
    }
}
```

## Composing the crate root

<a name="chunk-root"></a><sub>[`src/lib.rs`](../../crates/x0k-icon/src/lib.rs) · `#root` · assembles [crate-doc](#chunk-crate-doc) · [modules](#chunk-modules) · [exports](#chunk-exports) · [refusal](#chunk-refusal) · [check](#chunk-check) · [tests](#chunk-tests)</sub>

```rust {#root}
<<crate-doc>>

<<modules>>

<<exports>>

<<refusal>>

<<check>>

<<tests>>
```

<a name="chunk-fixtures-root"></a><sub>[`src/fixtures.rs`](../../crates/x0k-icon/src/fixtures.rs) · `#fixtures-root` · assembles [fixtures-doc](#chunk-fixtures-doc) · [fixtures](#chunk-fixtures)</sub>

```rust {#fixtures-root file="src/fixtures.rs"}
<<fixtures-doc>>

<<fixtures>>
```

The crate's boundary is worth stating as a negative. It reads no file,
opens no socket, and knows nothing about markdown: which fenced block in
which section is an icon, and what entity that section declares, are
questions for the document reader in `x0k-folio` and the projector in
`x0k-tangle`, which hand this crate a block's text and an entity's id and
title. It also does not paint. What it knows is the language — what a
mark may say, whether one said it, and how to write it down — and the
ADR's argument for one implementation is only as good as this crate's
refusal to know anything more.
