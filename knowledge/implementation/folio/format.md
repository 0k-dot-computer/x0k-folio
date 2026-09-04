---
x0k:
  format: folio/v1
  id: x0k:implementation/folio/format
  type: implementation
  status: draft
  summary: The crate root — its chapter map, and the one feature flag that severs the substrate-facing half so a standalone build is pure functions over strings.
  concerns: [folio, format, crate, features, publishing]
  tangle:
    crate: x0k-folio
    root: src/lib.rs
  edges:
    implements:
      - x0k:design/literate-programming
    cites:
      - x0k:architecture/filesystem-graph-materialization
      - x0k:implementation/folio/colophon
      - x0k:implementation/folio/identity
      - x0k:implementation/folio/checking
      - x0k:implementation/folio/inline-entities
      - x0k:implementation/folio/segmentation
      - x0k:implementation/folio/provenance
      - x0k:implementation/folio/structural
      - x0k:implementation/folio/transclusion
      - x0k:implementation/folio/html-canonical
      - x0k:implementation/folio/canonical-patch
      - x0k:implementation/folio/projection
---
# x0k-folio: the format library

Every document in x0k's corpus — the body of markdown files, kept
under version control, from which the system's decisions, reference
pages, publication manifests, and its own code are all derived — is a
**folio/v1** document: a YAML envelope declaring identity, genus, and
graph edges, over a markdown or HTML body. This literate page is one
of them. `x0k-folio` is the library that makes that
sentence precise. It owns the envelope's one parser and renderer, the
block-level identity schemes that let judgments and proposals attach to
prose, the canonical HTML form, the one patch grammar editors speak to
either body dialect, transclusion, and the projection plugin that binds
the format into the live substrate.

The crate reads as chapters, each owning one idea:

- [`colophon.md`](colophon.md) — the envelope: one parser, one
  renderer, permissive keys, closed keywords.
- [`identity.md`](identity.md) — the id grammar: class and slug, the
  substrate's locator half deliberately absent.
- [`checking.md`](checking.md) — reading an envelope against the
  vocabulary the bundle ships; a missing term and a missing target are
  different answers.
- [`inline-entities.md`](inline-entities.md) — entities authored inside
  prose: the section is the record, and the extractor does not resolve.
- [`segmentation.md`](segmentation.md) — two identities per block: a
  stability key for underwriting, a content hash for staleness.
- [`provenance.md`](provenance.md) — the append-only event log and the
  fold that answers "has a human taken responsibility for this?"
- [`structural.md`](structural.md) — the parser-agnostic block tree
  both renderers consume; refusing to lose author markup.
- [`transclusion.md`](transclusion.md) — include-don't-copy: one
  canonical home per paragraph, degrade-to-link on any failure.
- [`html-canonical.md`](html-canonical.md) — one true serialization,
  idempotent, behavior-stripped.
- [`canonical-patch.md`](canonical-patch.md) — one patch grammar over
  both body dialects: structural addresses, attributes before text.
- [`projection.md`](projection.md) — the Loro round trip, behind the
  `plugins` feature.

A single document threads through those chapters: the publication
manifest `decisions/publications/x0k-folio.md`, whose
envelope the colophon parses, whose body the segmenter addresses, whose
sections transclusion can inline — and which, projected as a repository,
publishes this very crate.

## Two builds, one severance

The crate has two consumers with different appetites, and one feature
flag separates them. Two truths, one per build:

- **In the monorepo**, `x0k-folio` participates in the substrate. The
  `plugins` feature is on by default; it compiles the projection module
  and pulls in the `x0k-types` dependency, so daemons can register the
  folio/v1 projection into their class registries.
- **In the published repository** (the one this crate's own
  publication manifest projects), `x0k-types` is not published at all.
  The projector drops that dependency from the manifest and, with it,
  the `plugins` feature that named it, so the published `Cargo.toml`
  has no `plugins` feature to turn on. The `projection` module's source
  still ships — it is part of the literate corpus — but every
  `#[cfg(feature = "plugins")]` line below now gates it off
  unconditionally. What a public reader builds is a format library
  whose only workspace dependency is `x0k-ontology`; the module they can
  read but not compile is [`projection.md`](projection.md).

What the published half now *contains* is the other side of that same
cut. `x0k:architecture/publication-projection` §6 says the unit of the
allowlist is the module, and the residency question asked of one is the
one the platform asks of a cell: does it only read the region's
documents, or does it hold a store, a socket, a credential? Two modules
that had been private by history rather than by residency answered
"reads only" and moved down here — the envelope check
([`checking.md`](checking.md)) and the inline-entity extractor
([`inline-entities.md`](inline-entities.md)) — bringing the id grammar
they both need ([`identity.md`](identity.md)) with them. The dependency
they used to reach for was `x0k-types`, unconditionally, which severing
could not have touched; splitting the id type at its meaning is what
made the move possible.

This is the publish-scoping severance for `x0k-types`: the cut point
between "format" and "platform" is a cargo feature rather than a fork,
so the monorepo and the public repository build from the same source.
The projection module's absence from the public build is the design —
what is published is the format, not the substrate coupling.

```rust {#module-doc}
//! folio/v1 envelope types + HTML canonicalizer, and — under the `plugins`
//! feature — the projection plugin that binds the format into a live substrate.
//!
//! Substrate for ontology-aware markdown/HTML documents. Lives apart from
//! `x0k-types` so that `x0k-types` itself stays narrow (publishable
//! substrate primitives only) and so that consumers who don't care about
//! the folio format don't pull `html5ever` and friends.
//!
//! Modules:
//!
//! - [`colophon`] — envelope parser and types (`Colophon`, `DocType`, `Status`, `Materialization`)
//! - [`entity_id`] — the id grammar: `x0k:<class>/<slug>`, parsed and rendered
//! - [`envelope_check`] — an envelope read against the vocabulary this build compiled
//! - [`inline_entity`] — entities authored inside a document body, extracted
//! - [`html_canonical`] — HTML canonicalizer for folio/v1 HTML bodies (stable attribute ordering, whitespace policy, behavior stripping)
//! - `projection` — the `ColophonProjection` projection plugin. Compiled only
//!   under the `plugins` feature; the source ships either way, so a build
//!   without the feature has a module to read and no item to link to. Named
//!   in prose rather than as a doc link for exactly that reason.
//!
//! Plugin registration: at startup, a host that wants the `folio/v1` projection
//! active calls `projection::register_colophon_factory` before loading its class
//! registry. Nothing hard-wires that registration.
```

## The crate surface

The module list and re-exports are the crate's table of contents. The
`#[cfg(feature = "plugins")]` lines are the severance made visible:
exactly one module and its two names disappear from the public build.

```rust {#modules-and-exports}
pub mod block_provenance;
pub mod block_segment;
pub mod canonical_patch;
pub mod colophon;
pub mod entity_id;
pub mod envelope_check;
pub mod html_canonical;
pub mod inline_entity;
#[cfg(feature = "plugins")]
pub mod projection;
pub mod structural_block;
pub mod transclusion;

pub use block_provenance::{
    fold_events, viewer_state, AgentAttestation, AgentAttestationRecord, AttestationTarget,
    BlockProvenance, GenerationContext, ProposalDisposition, ProvenanceEvent, Underwriting,
    ViewerState,
};
pub use block_segment::{hash_block, segment_body, BlockKind, BlockSegment};
pub use entity_id::{EntityId, EntityIdError};
pub use envelope_check::{
    check_corpus, check_envelope, predicate_standing, CorpusReport, DanglingEdge, Defect,
    EnvelopeReport, PredicateStanding,
};
pub use inline_entity::{
    declared_facts, declared_facts_with, defined_in_fact, extract_from_markdown,
    inline_entity_facts, InlineEntity, InlineEntityError,
};
pub use canonical_patch::{
    apply_body_patches, apply_folio_patches, apply_markdown_patches,
    canonicalize_edited_folio_content, canonicalize_folio_content, CanonicalHtmlPatch,
    CanonicalPatch, CanonicalPatchError, CanonicalTextPoint,
};
#[cfg(feature = "plugins")]
pub use projection::{ColophonProjection, FOLIO_V1_PLUGIN_NAME};
pub use structural_block::{
    BlockId, BlockIdAllocator, FenceInfo, InlineSpan, MarkerId, StructuralBlock, StructuralDoc,
    StructuralListItem, TableAlignment,
};
pub use transclusion::{
    DocSource, Resolved, TranscludeRef, TranscludeWarning, MAX_TRANSCLUDE_DEPTH,
};
```

## Plugin registration

The one function at the crate root exists for a wiring reason: daemons
that want `plugin = "folio/v1"` entries in their projection-classes
config to resolve must register the factory before loading their
`ClassRegistry`, and the registration must be idempotent because
multiple startup paths may call it. A `std::sync::Once` makes duplicate
calls no-ops; the same registration is also triggered lazily by
`x0k-types`' inline `ensure_builtin_plugins_registered` path, so the
two routes converge on one factory entry.

```rust {#register-factory}
/// Idempotent registration of the `folio/v1` projection factory in
/// `x0k_types::class_registry::PLUGIN_FACTORIES`. Daemons call this once at
/// startup (before loading their `ClassRegistry`) so that
/// `plugin = "folio/v1"` entries in `config/projection-classes.toml`
/// resolve to a [`ColophonProjection`] instance.
///
/// Wrapped in a `std::sync::Once` so duplicate calls are no-ops — mirrors
/// the inline `ensure_builtin_plugins_registered` path that
/// `x0k_types::class_registry::ClassRegistry::from_config` triggers
/// lazily.
#[cfg(feature = "plugins")]
pub fn register_colophon_factory() {
    use std::sync::Arc;

    use x0k_types::class_registry::register_plugin_factory;

    static INIT: std::sync::Once = std::sync::Once::new();
    INIT.call_once(|| {
        register_plugin_factory(FOLIO_V1_PLUGIN_NAME, |pt, lep| {
            Arc::new(ColophonProjection::new(pt, lep))
        });
    });
}
```

## Composing the crate root

```rust {#root}
<<module-doc>>

<<modules-and-exports>>

<<register-factory>>
```

A candor note on the shape of the whole: this crate is a bundle of
sibling concerns — envelope, identity, canonicalization, transclusion —
rather than one algorithm, and the honest justification is dependency
geometry, not conceptual unity. Each module is what two or more
downstream crates need to share without depending on each other, and
"the format library" is the name for where such things live. The
individual chapters carry the ideas; the crate is their meeting point.
