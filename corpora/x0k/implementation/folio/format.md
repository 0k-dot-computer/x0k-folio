---
x0k:
  format: folio/v1
  id: x0k:implementation/folio/format
  type: implementation
  status: draft
  summary: The crate root — its chapter map, and the one feature flag that severs the substrate-facing half so a standalone build is pure functions over strings.
  concerns:
  - folio
  - format
  - crate
  - features
  - publishing
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
graph edges, over a markdown or HTML body. This [literate page](../../wiki/literate-programming.md "x0k:wiki/literate-programming") is one
of them. `x0k-folio` is the library that makes that
sentence precise. It owns the envelope's one parser and renderer, the
block-level identity schemes that let judgments and proposals attach to
prose, the canonical HTML form, the one patch grammar editors speak to
either body dialect, transclusion, and the projection plugin that binds
the format into the live substrate.

The crate reads as chapters, each owning one idea:

- [`colophon.md`](colophon.md) — the envelope: one parser, one
  renderer, permissive keys, closed keywords.
- [`document-vocabulary.md`](document-vocabulary.md) — Turtle definitions
  and YAML instances assembled against a selected vocabulary.
- [`identity.md`](identity.md) — the id grammar: class and slug, the
  substrate's locator half deliberately absent.
- [`checking.md`](checking.md) — reading an envelope against the
  vocabulary the bundle ships; a missing term and a missing target are
  different answers. Also the declaration check: a human claim with no
  signifier is a defect.
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

## The optional substrate binding

A parser needs document text and a vocabulary. A live x0k host also needs
a projection plugin and its class registry. The `plugins` feature includes
`x0k-types` and the monorepo's `workspace-hack` dependency, which unifies
dependency features for host builds. It is enabled by default for existing
hosts. A standalone consumer sets `default-features = false`; its only
workspace dependency is the vocabulary library, `x0k-ontology`.

The dependency boundary matters as much as the module boundary. Disabling
the projection module while still pulling the monorepo's dependency
unifier would retain unrelated runtime and platform libraries. Both
dependencies therefore belong to the same optional feature.

The publication projector removes private dependencies and their feature
references from the published manifest. The format library and the host
plugin share the same source, while the public build uses only the format
modules. The host explicitly registers the plugin before loading its
class registry.

<a name="chunk-module-doc"></a><sub>[`src/lib.rs`](../../../../x0k-folio/src/lib.rs) · `#module-doc`</sub>

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
//! - [`entity_id`] — the id grammar: `<scheme>:<class>/<slug>`, parsed and rendered
//! - [`envelope_check`] — an envelope read against a vocabulary the caller names
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

<a name="chunk-modules-and-exports"></a><sub>[`src/lib.rs`](../../../../x0k-folio/src/lib.rs) · `#modules-and-exports`</sub>

```rust {#modules-and-exports}
pub mod block_provenance;
pub mod block_segment;
pub mod canonical_patch;
pub mod colophon;
#[cfg(feature = "document-vocabulary")]
pub mod document_vocabulary;
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
    check_corpus, check_declarations, check_envelope, predicate_standing, CorpusReport,
    DanglingEdge, DeclarationDefect, DeclarationReport, Defect, EnvelopeReport,
    PredicateStanding,
};
pub use inline_entity::{
    declared_facts, declared_facts_with, defined_in_fact, document_edges, extract_from_markdown,
    inline_entity_facts, prose_edges, InlineEntity, InlineEntityError, ICON_CLASS,
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

<a name="chunk-register-factory"></a><sub>[`src/lib.rs`](../../../../x0k-folio/src/lib.rs) · `#register-factory`</sub>

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

<a name="chunk-root"></a><sub>[`src/lib.rs`](../../../../x0k-folio/src/lib.rs) · `#root` · assembles [module-doc](#chunk-module-doc) · [modules-and-exports](#chunk-modules-and-exports) · [register-factory](#chunk-register-factory)</sub>

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

## Keeping the format build independent

The standalone gate inspects the actual normal and build dependency graph,
then runs the format library's tests with plugins disabled and
`document-vocabulary` enabled. That optional feature uses the same
ontology loader to read Turtle carried by Markdown documents. Any new local
dependency requires an explicit boundary decision: only the format crate
and its vocabulary are admitted. The monorepo dependency unifier is refused
by name because its registry-only dependencies would otherwise bypass that
check.

Run `bash substrate/crates/production/x0k-folio/tools/check-standalone.sh`
from the workspace root. The same script works in the projected repository
under `x0k-folio/tools/check-standalone.sh`.

<a name="chunk-check-standalone"></a><sub>[`tools/check-standalone.sh`](../../../../x0k-folio/tools/check-standalone.sh) · `#check-standalone`</sub>

```bash {#check-standalone file="tools/check-standalone.sh"}
#!/usr/bin/env bash
set -euo pipefail

dependency_tree=$(cargo tree -p x0k-folio --no-default-features --features document-vocabulary \
    --edges normal,build --prefix none --format '{p}')
while IFS= read -r dependency; do
    case "$dependency" in
        x0k-folio\ *|x0k-ontology\ *) ;;
        x0k\ *|x0k-*|workspace-hack\ *)
            printf 'Standalone format dependency refused: %s\n' "$dependency" >&2
            exit 1
            ;;
    esac
done <<< "$dependency_tree"

cargo test -p x0k-folio --no-default-features --features document-vocabulary --lib
```

The gate tests the same parser consumers link. Independence is a property
of its resolved dependencies, rather than a promise made by its feature name.
