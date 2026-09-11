---
x0k:
  format: folio/v1
  id: x0k:implementation/fact-projection/materialize
  type: implementation
  status: draft
  summary: The facts→file half of materialization — a materializer renders facts to bytes and names where they land, with no Loro, no ops, and no daemon in the contract.
  concerns:
  - facts
  - materialization
  - projection
  - seam
  - publication
  tangle:
    crate: crates/x0k-fact-projection
    root: src/materialize.rs
  edges:
    implements:
    - x0k:architecture/filesystem-graph-materialization
    cites:
    - x0k:architecture/state-representation
    - x0k:architecture/publication-is-the-shipping-unit
---

# Rendering facts back to a file

The storage half of materialization ships and is published: a `FactSink`
takes facts, with an optional notifier and query engine beside it, and
`x0k-folio-dialog` is one implementation of it. Ask for the other
direction — facts back out to a file — and there is nothing to name. That
absence is why "control over materialization" is a promise the corpus makes
and the code cannot keep: a reader who wants their own file layout has
nowhere to plug it in.

The contract for that direction exists in the tree, but one crate too high.
`ProjectionPlugin` lives in `x0k-types`, which the folio publication
*excludes*; the one shipped implementation is therefore feature-gated out
of the public build, and its own literate doc opens by admitting "this
module does not build outside the monorepo." The extraction is off by
exactly that one crate, and this chapter is where the seam lands instead.

## Why here, and what stayed behind

This crate is already the contract both sides name. It owns the fact tuple,
the read seam a consumer folds over, and `project_envelope` — which turns a
folio envelope *into* facts. A materializer is that last function's
inverse, so it belongs beside it rather than in a substrate crate the
publication cannot ship.

The split that makes the move possible is directional. `ProjectionPlugin`
bundles two operations pointing opposite ways: render (state → bytes) and
parse-diff (bytes → `DocumentOp`s). Only the second needs `DocumentOp`, and
`DocumentOp` is what drags in the whole substrate tree. Rendering needs
facts and nothing else. So the outbound half comes here and the inbound
half stays where its op vocabulary is — they were never one thing except
by being declared in one trait.

Two consequences worth stating plainly, because they are what the seam
buys:

- **A materializer never sees Loro.** It is handed facts and returns bytes.
  Whether those facts came from a CRDT, a spine fold, or a literal in a
  test is not its business, and a test can therefore exercise a real
  materializer with a `Vec<FactEntry>`.
- **A materializer never sees the daemon.** The mirror lifecycle, the CRDT
  binding, the watcher and the scope gate all hold a runtime and all stay
  in the daemon. What leaves is the pure part.

## Where the bytes land

A materializer answers two questions, and the second one is not "what is
the path". One document class puts every entity at its own file; another
projects a single document to one well-known location. The template carries
that distinction so callers do not re-derive it from the shape of a string.

<a name="chunk-path-template"></a><sub>[`src/materialize.rs`](../../crates/x0k-fact-projection/src/materialize.rs) · `#path-template`</sub>

```rust {#path-template}
/// Where a class's materialized files land.
///
/// Two shapes, because two things are being described:
///
/// - `Slug` — per-entity classes like `x0k:wiki/*`. The template carries a
///   `{slug}` token filled with the URI's tail segment, so each entity
///   lands at its own file.
/// - `Fixed` — singleton classes where one document projects to one
///   well-known file (a workstream registry → `.0k/workstreams.toml`). No
///   substitution; the path is literal.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PathTemplate {
    /// Per-entity template with `{slug}` substitution.
    Slug { template: String },
    /// Singleton class — one file at a fixed relative path.
    Fixed { path: String },
}

impl PathTemplate {
    /// Build a slug-templated path. Validates that the string contains
    /// `{slug}`: a per-entity template without the token would map every
    /// URI in the class to the same file, which is never what a caller
    /// reaching for `parse` means.
    pub fn parse(s: impl Into<String>) -> Result<Self, PathTemplateError> {
        let template = s.into();
        if !template.contains("{slug}") {
            return Err(PathTemplateError::MissingToken {
                got: template,
                token: "{slug}",
            });
        }
        Ok(Self::Slug { template })
    }

    /// Build a fixed-path template. Infallible — the path is taken
    /// verbatim. This is the singleton spelling, and choosing it is how a
    /// caller says the class has exactly one file.
    pub fn fixed(s: impl Into<String>) -> Self {
        Self::Fixed { path: s.into() }
    }

    /// Resolve to a relative path. `Slug` substitutes, or leaves the token
    /// literal when there is no slug — which is a diagnostic string, not a
    /// path to write to. `Fixed` ignores the argument.
    pub fn render(&self, slug: Option<&str>) -> String {
        match self {
            Self::Slug { template } => match slug {
                Some(s) => template.replace("{slug}", s),
                None => template.clone(),
            },
            Self::Fixed { path } => path.clone(),
        }
    }

    /// The raw template string. Collision detection compares these, since
    /// two classes rendering to the same literal path is a registry error
    /// rather than a runtime one.
    pub fn as_str(&self) -> &str {
        match self {
            Self::Slug { template } => template,
            Self::Fixed { path } => path,
        }
    }

    /// `true` for a singleton template. Callers that only make sense
    /// per-entity — a walker that promotes each entity's mirror — check
    /// this before dispatching.
    pub fn is_fixed(&self) -> bool {
        matches!(self, Self::Fixed { .. })
    }
}

/// The one way a template can be malformed.
#[derive(Debug)]
pub enum PathTemplateError {
    MissingToken { got: String, token: &'static str },
}

impl core::fmt::Display for PathTemplateError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::MissingToken { got, token } => write!(
                f,
                "path template `{got}` is missing required token `{token}`"
            ),
        }
    }
}

impl std::error::Error for PathTemplateError {}
```

## How much the store may own

The third value the seam carries is the one the canonicality guard reasons
over, and its variants are ordered rather than merely distinct. `width` is
that order: how much of the document the store is permitted to own, from
none to all of it. A declaration nearer the document may narrow what a
registry declared and may never widen it, so `narrowed_by` is a meet and
the only comparison the guard needs.

<a name="chunk-live-edit-policy"></a><sub>[`src/materialize.rs`](../../crates/x0k-fact-projection/src/materialize.rs) · `#live-edit-policy`</sub>

```rust {#live-edit-policy}
/// Whether a class's documents may be owned by a live store, and on whose
/// initiative.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LiveEditPolicy {
    /// The file is ground truth. No mirror; edits land through jj.
    Forbidden,
    /// A mirror is created on demand (an editor opens it, an agent asks)
    /// and demoted on idle.
    OnRequest,
    /// A mirror is hydrated at startup and stays live.
    AutoOn,
}

impl LiveEditPolicy {
    /// How wide this policy is, for narrowing comparisons.
    ///
    /// `Forbidden` is narrowest and `AutoOn` widest: the order is how much
    /// the store is permitted to own. A declaration may be narrowed by
    /// something closer to the document and never widened
    /// (`filesystem-graph-materialization` §7).
    pub fn width(self) -> u8 {
        match self {
            LiveEditPolicy::Forbidden => 0,
            LiveEditPolicy::OnRequest => 1,
            LiveEditPolicy::AutoOn => 2,
        }
    }

    /// The narrower of two policies — the guard's meet.
    pub fn narrowed_by(self, other: LiveEditPolicy) -> LiveEditPolicy {
        if other.width() < self.width() {
            other
        } else {
            self
        }
    }

    /// The wire spelling, as it appears in a manifest.
    pub fn as_str(self) -> &'static str {
        match self {
            LiveEditPolicy::Forbidden => "forbidden",
            LiveEditPolicy::OnRequest => "on-request",
            LiveEditPolicy::AutoOn => "auto-on",
        }
    }

    /// Read a wire spelling. `None` for anything else — an unrecognized
    /// policy is a manifest to refuse, never a default to assume.
    pub fn from_str(s: &str) -> Option<Self> {
        Some(match s {
            "forbidden" => LiveEditPolicy::Forbidden,
            "on-request" => LiveEditPolicy::OnRequest,
            "auto-on" => LiveEditPolicy::AutoOn,
            _ => return None,
        })
    }
}
```

## The root the placement is relative to

A [`PathTemplate`] renders a path, and a path is relative to something. That
something is a **mount**: a named root with a declared posture, and the reason
"one class, one truth" and "project this corpus into five tools" stop
contradicting each other.

The cardinality is the whole design. A class has **exactly one** mount — its
home, the root its canonicality lives at — while two classes **may** share
one, because a mount is a root and not a namespace. A class rendered into a
second root is not a second mount but a *surface*, one-way unless it declares
its own parser, and a surface never carries canonicality. So the question
"where is the truth for this class?" always has one answer, and the question
"where else does it appear?" can have many.

Posture is not decoration. A `working-tree` mount is the profile's own
jj-versioned workspace, where a projected file *is* the corpus; an `external`
mount is a directory some other tool owns, often inside a third-party sync
domain. Projecting into the second hands content to software the trust mesh
never admitted, which makes it an egress decision rather than a write — so the
posture rides on the mount, where the projection boundary can read it, instead
of being inferred from whether a path looks unfamiliar.

<a name="chunk-mount"></a><sub>[`src/materialize.rs`](../../crates/x0k-fact-projection/src/materialize.rs) · `#mount`</sub>

```rust {#mount}
/// What kind of root a mount is, and therefore what writing to it means.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MountPosture {
    /// The profile's own jj-versioned workspace root: a projected file
    /// there IS the corpus.
    WorkingTree,
    /// A configured directory outside it, typically owned by another tool
    /// and possibly inside a third-party sync domain. Writing to one is an
    /// egress decision, not merely a write, which is why the posture is
    /// declared rather than guessed from the shape of a path.
    External,
}

impl MountPosture {
    pub fn as_str(self) -> &'static str {
        match self {
            MountPosture::WorkingTree => "working-tree",
            MountPosture::External => "external",
        }
    }

    /// Read a declared posture. `None` for anything else — an
    /// unrecognized posture is a config to refuse, never a default to
    /// assume, because the default that would be assumed is the
    /// permissive one.
    pub fn from_str(s: &str) -> Option<Self> {
        Some(match s {
            "working-tree" => MountPosture::WorkingTree,
            "external" => MountPosture::External,
            _ => return None,
        })
    }
}

/// The mount every class calls home unless it names another. Its root is
/// the profile's workspace, so the common case declares nothing.
pub const DEFAULT_MOUNT: &str = "working-tree";

/// A named root a placement resolves against.
///
/// Identity is `(profile, name)` and never a bare name — this type carries
/// the name, and the profile is the scope it is looked up in. The ROOT is
/// deliberately not here: resolving one goes through the profile's
/// `X0K_HOME` resolver, which touches the environment, and this crate
/// holds the declaration rather than the resolution.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Mount {
    /// The mount's name within its profile.
    pub name: String,
    pub posture: MountPosture,
    /// For an `External` mount, the configured directory naming its root,
    /// as written in the profile-scoped config entry that opted it in.
    /// `None` for a `working-tree` mount, whose root the profile already
    /// knows.
    pub root: Option<String>,
}

impl Mount {
    /// The default working-tree mount, which every profile has without
    /// declaring it.
    pub fn working_tree() -> Self {
        Self {
            name: DEFAULT_MOUNT.to_string(),
            posture: MountPosture::WorkingTree,
            root: None,
        }
    }

    /// Whether writing here leaves the trust mesh's own tree.
    pub fn is_egress(&self) -> bool {
        matches!(self.posture, MountPosture::External)
    }
}
```

## The seam

Rendered bytes plus a placement. That return type is the decision this
chapter rests on, and it is settled rather than open: `x0k-ontology`'s
`render_module` is already facts→`String` in a crate whose entire
dependency list is blake3, oxttl and oxrdf, which is the existence proof
that this direction needs no substrate. Returning ops instead would have
made every implementor speak the daemon's op vocabulary to write a file.

<a name="chunk-rendered"></a><sub>[`src/materialize.rs`](../../crates/x0k-fact-projection/src/materialize.rs) · `#rendered`</sub>

```rust {#rendered}
/// One materialized file: the bytes, and the relative path they belong at.
///
/// The path is relative to a root the caller owns. A materializer knows
/// the shape of a placement and never where the tree is mounted — that is
/// the caller's, because it is the part that touches a filesystem.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Rendered {
    /// Path relative to the caller's materialization root.
    pub path: String,
    /// The canonical bytes for that path.
    pub bytes: Vec<u8>,
}

/// Why a render could not happen.
///
/// One string, in the crate's existing neutral-error style
/// ([`FactSourceError`](crate::source::FactSourceError)): this crate stays
/// wasm-clean and dependency-free, so it carries no error library, and a
/// materializer's failures are reported to a human rather than matched on.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MaterializeError(pub String);

impl core::fmt::Display for MaterializeError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for MaterializeError {}
```

`render` takes a fact slice and returns bytes; `materialize` composes it
with the placement so a caller that just wants a file does not assemble the
path itself. The provided body is the whole point of the split — one
implementor writes `render`, and every caller gets the same placement
rules.

<a name="chunk-materializer"></a><sub>[`src/materialize.rs`](../../crates/x0k-fact-projection/src/materialize.rs) · `#materializer`</sub>

```rust {#materializer}
/// The facts→file half of materialization: one strategy per document
/// class, with no substrate in the contract.
///
/// The counterpart of the storage half (`FactSink` and its optional
/// notifier and query engine), and the seam a reader implements to control
/// where and how their graph lands on disk.
pub trait Materializer: Send + Sync {
    /// Stable identifier, for diagnostics and registry validation
    /// (today: `"folio/v1"`). Several class entries may share a name while
    /// carrying different placements.
    fn name(&self) -> &str;

    /// Where this class's files land.
    fn placement(&self) -> &PathTemplate;

    /// How much of this class a live store may own.
    fn live_edit_policy(&self) -> LiveEditPolicy;

    /// Render `facts` — all facts about ONE entity — to the canonical
    /// bytes of its file.
    fn render(&self, facts: &[FactEntry]) -> Result<Vec<u8>, MaterializeError>;

    /// The bytes and where they go, in one call.
    ///
    /// Provided rather than required so the placement rules have exactly
    /// one implementation: an implementor supplies `render`, and a caller
    /// that wants a file never re-derives a path from a template.
    fn materialize(
        &self,
        slug: Option<&str>,
        facts: &[FactEntry],
    ) -> Result<Rendered, MaterializeError> {
        Ok(Rendered {
            path: self.placement().render(slug),
            bytes: self.render(facts)?,
        })
    }
}
```

## The module

<a name="chunk-root"></a><sub>[`src/materialize.rs`](../../crates/x0k-fact-projection/src/materialize.rs) · `#root` · assembles [path-template](#chunk-path-template) · [live-edit-policy](#chunk-live-edit-policy) · [mount](#chunk-mount) · [rendered](#chunk-rendered) · [materializer](#chunk-materializer) · [tests](#chunk-tests)</sub>

```rust {#root}
//! The facts→file half of materialization.
//!
//! [`Materializer`] renders one entity's facts to the canonical bytes of
//! its file and names where they land, with no Loro, no `DocumentOp` and
//! no daemon in the contract. It is the counterpart of the storage half
//! that already ships (`FactSink` plus its optional notifier and query
//! engine), and the inverse of [`project_envelope`](crate::project_envelope)
//! — which is why it lives beside that function rather than in a substrate
//! crate the folio publication excludes.
//!
//! [`PathTemplate`] and [`LiveEditPolicy`] moved here with it. The inbound
//! direction — file edits parsed back into ops — stayed behind with the op
//! vocabulary it needs; the two were one trait only by declaration.
//!
//! Governing decision:
//! `corpora/x0k/decisions/architecture/production/filesystem-graph-materialization.md` §8.

use crate::FactEntry;

<<path-template>>

<<live-edit-policy>>

<<mount>>

<<rendered>>

<<materializer>>

<<tests>>
```

## Proving it

The oracle for the extraction is that a materializer with no Loro anywhere
in it implements the trait and round-trips a document. The folio
materializer is that implementation and its round-trip lives in `x0k-folio`
beside the renderer it uses; what belongs here is the smaller claim the
seam itself makes — that the provided `materialize` composes placement with
render, for both template shapes.

<a name="chunk-tests"></a><sub>[`src/materialize.rs`](../../crates/x0k-fact-projection/src/materialize.rs) · `#tests`</sub>

```rust {#tests}
#[cfg(test)]
mod tests {
    use super::*;
    use crate::FactValue;

    /// A materializer with no substrate at all: it renders the fact values
    /// it was handed, one per line. Enough to exercise the seam, and proof
    /// that implementing it costs an implementor nothing but a render.
    struct LineDump {
        placement: PathTemplate,
    }

    impl Materializer for LineDump {
        fn name(&self) -> &str {
            "line-dump"
        }

        fn placement(&self) -> &PathTemplate {
            &self.placement
        }

        fn live_edit_policy(&self) -> LiveEditPolicy {
            LiveEditPolicy::Forbidden
        }

        fn render(&self, facts: &[FactEntry]) -> Result<Vec<u8>, MaterializeError> {
            let mut out = String::new();
            for fact in facts {
                match &fact.value {
                    FactValue::Text(text) => out.push_str(text),
                    _ => return Err(MaterializeError(format!(
                        "line-dump renders text values only, got {:?}",
                        fact.value
                    ))),
                }
                out.push('\n');
            }
            Ok(out.into_bytes())
        }
    }

    fn facts() -> Vec<FactEntry> {
        vec![
            FactEntry::new("x0k:wiki/loro", "p/a", FactValue::Text("first".into())),
            FactEntry::new("x0k:wiki/loro", "p/b", FactValue::Text("second".into())),
        ]
    }

    #[test]
    fn a_slug_class_lands_each_entity_at_its_own_file() {
        let dump = LineDump {
            placement: PathTemplate::parse("knowledge/wiki/{slug}.md").unwrap(),
        };
        let rendered = dump.materialize(Some("loro"), &facts()).unwrap();
        assert_eq!(rendered.path, "knowledge/wiki/loro.md");
        assert_eq!(rendered.bytes, b"first\nsecond\n");
    }

    #[test]
    fn a_singleton_class_ignores_the_slug() {
        let dump = LineDump {
            placement: PathTemplate::fixed(".0k/workstreams.toml"),
        };
        assert_eq!(
            dump.materialize(Some("anything"), &facts()).unwrap().path,
            ".0k/workstreams.toml"
        );
    }

    /// A per-entity template with no token would put every entity in the
    /// class at one path — silently, and destructively.
    #[test]
    fn a_slug_template_without_its_token_is_refused() {
        let err = PathTemplate::parse("knowledge/wiki/static.md").expect_err("missing token");
        assert!(matches!(
            err,
            PathTemplateError::MissingToken { token: "{slug}", .. }
        ));
    }

    /// The guard's meet: narrowing wins from either side, and widening is
    /// not expressible.
    #[test]
    fn a_policy_narrows_and_never_widens() {
        use LiveEditPolicy::*;
        assert_eq!(AutoOn.narrowed_by(Forbidden), Forbidden);
        assert_eq!(Forbidden.narrowed_by(AutoOn), Forbidden);
        assert_eq!(OnRequest.narrowed_by(AutoOn), OnRequest);
        assert!(Forbidden.width() < OnRequest.width());
        assert!(OnRequest.width() < AutoOn.width());
    }

    #[test]
    fn policies_read_back_from_their_wire_spelling() {
        for policy in [
            LiveEditPolicy::Forbidden,
            LiveEditPolicy::OnRequest,
            LiveEditPolicy::AutoOn,
        ] {
            assert_eq!(LiveEditPolicy::from_str(policy.as_str()), Some(policy));
        }
        assert_eq!(LiveEditPolicy::from_str("nonsense"), None);
    }

    /// A render failure is the caller's to see, not a file to write.
    #[test]
    fn a_render_error_is_reported_rather_than_materialized() {
        let dump = LineDump {
            placement: PathTemplate::fixed("out.txt"),
        };
        let facts = vec![FactEntry::new(
            "x0k:wiki/loro",
            "p/link",
            FactValue::EntityRef("x0k:wiki/other".into()),
        )];
        let err = dump.materialize(None, &facts).expect_err("unrenderable");
        assert!(err.to_string().contains("text values only"), "{err}");
    }
}
```
