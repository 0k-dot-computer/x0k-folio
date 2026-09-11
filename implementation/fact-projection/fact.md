---
x0k:
  format: folio/v1
  id: x0k:implementation/fact-projection/fact
  type: implementation
  status: draft
  summary: The substrate-neutral fact tuple, and the projection of a folio/v1 envelope into a batch of them — typed, ordered, and deliberately uncaused.
  concerns:
  - facts
  - projection
  - envelope
  - substrate
  - typing
  tangle:
    crate: crates/x0k-fact-projection
    root: src/fact.rs
  edges:
    implements:
    - x0k:architecture/state-representation
    cites:
    - x0k:architecture/filesystem-graph-materialization
---

# What a fact is, before any substrate has it

Every substrate in this system stores facts differently. Dialog-DB writes
them as artifacts with a `string:`/`entity:` text encoding; the entry spine
wraps them in versioned payloads under a content-addressed coordinate. This
chapter is the shape a fact has **between** those — after a document has been
read and before any store has had an opinion.

That between-shape is the whole reason the crate exists. Without it, "project
a document into facts" would have to be written once per substrate, and the
two copies would disagree about a document within a month.

## The tuple

A fact is an entity URI, a predicate, a typed value, and an optional cause.
The typing is the part that carries weight, so it comes first:

<a name="chunk-value"></a><sub>[`src/fact.rs`](../../crates/x0k-fact-projection/src/fact.rs) · `#value`</sub>

```rust {#value}
/// A typed, substrate-neutral fact value.
///
/// Mirrors the value vocabulary of `dialog_artifacts::Value` without
/// depending on Dialog-DB, so producers and consumers can exchange facts
/// independent of which substrate stores them. Cache writers decide their
/// own encodings (e.g. Dialog-DB's `string:<text>` / `entity:<uri>` UTF-8
/// convention); spine writers decide payload framing.
#[derive(Debug, Clone, PartialEq)]
pub enum FactValue {
    /// A UTF-8 string scalar.
    Text(String),
    /// A reference to another entity, by URI (e.g. `x0k:seed/<uuid>`).
    EntityRef(String),
    /// A boolean.
    Boolean(bool),
    /// A 128-bit unsigned integer.
    UnsignedInt(u128),
    /// A 128-bit signed integer.
    SignedInt(i128),
    /// A floating point number.
    Float(f64),
    /// An opaque byte buffer.
    Bytes(Vec<u8>),
    /// Opaque structured record bytes.
    Record(Vec<u8>),
    /// A symbol (an attribute/predicate used as a value).
    Symbol(String),
    /// A **retraction marker**: the tombstone that
    /// records "the fact `(entity, predicate, inner value)` no longer
    /// holds". The inner value is the retracted fact's value, carried so
    /// the tombstone reproduces the retracted fact's canonical value
    /// bytes ([`payload::canonical_value_bytes`] unwraps the marker) —
    /// which places the tombstone at the SAME spine coordinate and the
    /// SAME relation-graph cell as the assert it retracts, so the two
    /// compete directly under the fold's dominance rule
    /// ([`relation_graph::fold_relation_graph`]).
    Retracted(Box<FactValue>),
}
```

`EntityRef` being distinct from `Text` is the load-bearing distinction in the
enum. A URI stored as text is a string that happens to look like a reference;
a URI stored as `EntityRef` is an edge the graph can traverse. Every
downstream encoding — Dialog-DB's `entity:` prefix, the spine's coordinate —
is derived from that one bit, and no store has to guess it from the shape of
the characters.

`Retracted` is the other variant worth pausing on: a retraction is not the
absence of a fact but a fact whose value is a tombstone, so it converges the
same way every other assertion does rather than needing a second mechanism.

<a name="chunk-value-accessors"></a><sub>[`src/fact.rs`](../../crates/x0k-fact-projection/src/fact.rs) · `#value-accessors`</sub>

```rust {#value-accessors}
impl FactValue {
    /// The string payload, if this is a [`FactValue::Text`].
    pub fn as_text(&self) -> Option<&str> {
        match self {
            FactValue::Text(s) => Some(s),
            _ => None,
        }
    }

    /// The target URI, if this is a [`FactValue::EntityRef`].
    pub fn as_entity_ref(&self) -> Option<&str> {
        match self {
            FactValue::EntityRef(uri) => Some(uri),
            _ => None,
        }
    }

    /// The integer payload, if this is a [`FactValue::SignedInt`].
    pub fn as_signed_int(&self) -> Option<i128> {
        match self {
            FactValue::SignedInt(i) => Some(*i),
            _ => None,
        }
    }

    /// Whether this value is a retraction marker.
    pub fn is_retraction(&self) -> bool {
        matches!(self, FactValue::Retracted(_))
    }

    /// The retracted value, if this is a [`FactValue::Retracted`].
    pub fn retracted_value(&self) -> Option<&FactValue> {
        match self {
            FactValue::Retracted(inner) => Some(inner),
            _ => None,
        }
    }
}
```

<a name="chunk-entry"></a><sub>[`src/fact.rs`](../../crates/x0k-fact-projection/src/fact.rs) · `#entry`</sub>

```rust {#entry}
/// One substrate-neutral fact: `entity --predicate--> value`.
///
/// `cause` is the provenance slot: a reference to whatever produced or
/// superseded this fact. The form is producer-defined; the spine write
/// path pins file-ingested facts to the [`payload::file_content_cause`]
/// convention (`file-content:<blake3>` of the source file's bytes),
/// substrate-connected journaled writers stamp the typed
/// [`FactProvenance`] rendering (`journal:<producer>@<seq>`), and the
/// Dialog-DB write path asserts uncaused (`None`, the cache's
/// convergence policy) with its read adapter carrying a stored cause
/// through when one exists.
#[derive(Debug, Clone, PartialEq)]
pub struct FactEntry {
    /// Subject entity URI (e.g. `x0k:design/foo`, `x0k:intent/<uuid>`).
    pub entity: String,
    /// Predicate, in its stored spelling (e.g. `x0k:folio/status`,
    /// `vendor:intent/title`, `motivatedBy`). Projection does not
    /// normalize predicate spellings: where a reader and a writer
    /// disagree on a spelling, the skew is preserved verbatim rather
    /// than resolved here.
    pub predicate: String,
    /// Typed value.
    pub value: FactValue,
    /// Optional provenance/cause reference (see type-level docs).
    pub cause: Option<String>,
}
```

<a name="chunk-entry-impl"></a><sub>[`src/fact.rs`](../../crates/x0k-fact-projection/src/fact.rs) · `#entry-impl`</sub>

```rust {#entry-impl}
impl FactEntry {
    /// Convenience constructor for an uncaused fact.
    pub fn new(entity: impl Into<String>, predicate: impl Into<String>, value: FactValue) -> Self {
        Self {
            entity: entity.into(),
            predicate: predicate.into(),
            value,
            cause: None,
        }
    }

    /// Whether this fact is a retraction (its value is the tombstone
    /// marker [`FactValue::Retracted`]).
    pub fn is_retraction(&self) -> bool {
        self.value.is_retraction()
    }

    /// The retraction of this fact: same `(entity, predicate)`, value
    /// wrapped in [`FactValue::Retracted`] (idempotent — retracting a
    /// retraction targets the same inner value), with `cause` naming
    /// what motivated the removal (e.g.
    /// [`payload::file_content_cause`] of the file state that dropped
    /// the edge, or [`payload::file_deleted_cause`] when the source
    /// file disappeared).
    pub fn to_retraction(&self, cause: Option<String>) -> FactEntry {
        let inner = match &self.value {
            FactValue::Retracted(inner) => inner.clone(),
            other => Box::new(other.clone()),
        };
        FactEntry {
            entity: self.entity.clone(),
            predicate: self.predicate.clone(),
            value: FactValue::Retracted(inner),
            cause,
        }
    }
}
```

## Projection is a document's envelope, ordered

<a name="chunk-predicates"></a><sub>[`src/fact.rs`](../../crates/x0k-fact-projection/src/fact.rs) · `#predicates`</sub>

```rust {#predicates}
/// Stable predicates for envelope-level scalars asserted on each folio
/// URI. Moved here from the folio ingester (which re-exports them) so
/// every substrate that materializes envelope facts shares one spelling.
/// Kept out of `ontology/vocab.ttl` because these scalars describe the
/// *envelope* of any folio, not a class-specific property.
pub mod envelope_predicates {
    pub const STATUS: &str = "x0k:folio/status";
    pub const DOC_TYPE: &str = "x0k:folio/docType";
    pub const SUBTYPE: &str = "x0k:folio/subtype";
    pub const BODY_FORMAT: &str = "x0k:folio/bodyFormat";
    pub const CONCERN: &str = "x0k:folio/concern";
    pub const MATERIALIZATION_LORO_DOC: &str = "x0k:folio/materializationLoroDocId";
    pub const MATERIALIZATION_REVISION: &str = "x0k:folio/materializationDocumentRevisionId";
    pub const MATERIALIZATION_CONTENT_HASH: &str = "x0k:folio/materializationContentHash";
}
```

<a name="chunk-view"></a><sub>[`src/fact.rs`](../../crates/x0k-fact-projection/src/fact.rs) · `#view`</sub>

```rust {#view}
/// A folio/v1 envelope viewed substrate-neutrally.
///
/// The daemon's `Folio` type (which carries `EntityUri`, ontology
/// lookups, and the parsed body) is not wasm-clean, so the ingester builds
/// this view from it: plain strings, edge predicates already resolved to
/// their ontology (camelCase) spelling, edges in the parser's iteration
/// order. [`project_envelope`] then owns *which facts an envelope yields*
/// and their typing — the part that must agree across substrates.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct ColophonView {
    /// The doc's URI identity (frontmatter `id`), e.g. `x0k:design/foo`.
    pub uri: String,
    /// Envelope `status`, in its canonical string form (`proposed`, …).
    pub status: String,
    /// Envelope `type`, in its canonical string form (`design`, `wiki`, …).
    pub doc_type: String,
    /// Optional envelope `subtype`.
    pub subtype: Option<String>,
    /// Canonical body format flag (`markdown` or `html`).
    pub body_format: String,
    /// Envelope `concerns`, in declaration order.
    pub concerns: Vec<String>,
    /// Materialization pointers (only present when populated).
    pub materialization_loro_doc_id: Option<String>,
    pub materialization_document_revision_id: Option<String>,
    pub materialization_content_hash: Option<String>,
    /// Known edges: `(resolved predicate, target URIs)`, in the parser's
    /// iteration order. Predicate resolution (snake_case → ontology
    /// camelCase, with verbatim fallback) happens at the view-construction
    /// site, which holds the ontology tables.
    pub edges: Vec<(String, Vec<String>)>,
    /// Unknown-predicate edges, preserved verbatim (forward-compat).
    pub unknown_edges: Vec<(String, Vec<String>)>,
}
```

`project_envelope` takes a view, not a parsed folio document. The crate sits
below the folio parser and must not depend on it — a substrate-neutral
projection that imports a document parser is neutral in name only. The view
is the narrow set of envelope facts projection actually reads, and the caller
fills it in from whatever it has.

`unknown_edges` is the forward-compatibility seam: an edge predicate this
build does not know is carried through verbatim rather than dropped, so a
document written against a newer vocabulary loses nothing by passing through
an older node.

<a name="chunk-project"></a><sub>[`src/fact.rs`](../../crates/x0k-fact-projection/src/fact.rs) · `#project`</sub>

```rust {#project}
/// Project a folio/v1 envelope into substrate-neutral facts.
///
/// Lifted from the folio ingester's `envelope_facts`. Emission order is
/// preserved exactly: status, doc type, subtype, body format, concerns,
/// materialization fields, known edges, unknown edges. Scalars become
/// [`FactValue::Text`]; edge targets become [`FactValue::EntityRef`] —
/// the typing the Dialog-DB writer encodes as `string:`/`entity:` and a
/// spine writer will frame as fact-entry payloads.
///
/// Projection emits uncaused facts (`cause: None`): the cause is
/// write-site knowledge, not envelope knowledge. The Dialog-DB writer
/// asserts uncaused (its convergence policy); the spine writer stamps
/// [`payload::file_content_cause`] of the source file before appending.
pub fn project_envelope(view: &ColophonView) -> Vec<FactEntry> {
    let mut out: Vec<FactEntry> = Vec::new();
    let uri = view.uri.as_str();

    // Envelope scalars
    out.push(FactEntry::new(
        uri,
        envelope_predicates::STATUS,
        FactValue::Text(view.status.clone()),
    ));
    out.push(FactEntry::new(
        uri,
        envelope_predicates::DOC_TYPE,
        FactValue::Text(view.doc_type.clone()),
    ));
    if let Some(s) = &view.subtype {
        out.push(FactEntry::new(
            uri,
            envelope_predicates::SUBTYPE,
            FactValue::Text(s.clone()),
        ));
    }
    out.push(FactEntry::new(
        uri,
        envelope_predicates::BODY_FORMAT,
        FactValue::Text(view.body_format.clone()),
    ));
    for c in &view.concerns {
        out.push(FactEntry::new(
            uri,
            envelope_predicates::CONCERN,
            FactValue::Text(c.clone()),
        ));
    }

    // Materialization fields (only when populated)
    if let Some(v) = &view.materialization_loro_doc_id {
        out.push(FactEntry::new(
            uri,
            envelope_predicates::MATERIALIZATION_LORO_DOC,
            FactValue::Text(v.clone()),
        ));
    }
    if let Some(v) = &view.materialization_document_revision_id {
        out.push(FactEntry::new(
            uri,
            envelope_predicates::MATERIALIZATION_REVISION,
            FactValue::Text(v.clone()),
        ));
    }
    if let Some(v) = &view.materialization_content_hash {
        out.push(FactEntry::new(
            uri,
            envelope_predicates::MATERIALIZATION_CONTENT_HASH,
            FactValue::Text(v.clone()),
        ));
    }

    // Edges (known, predicate already resolved by the view builder), then
    // unknown-predicate edges verbatim.
    for (predicate, targets) in view.edges.iter().chain(view.unknown_edges.iter()) {
        for target in targets {
            out.push(FactEntry::new(
                uri,
                predicate.clone(),
                FactValue::EntityRef(target.clone()),
            ));
        }
    }

    out
}
```

Two things about that function are contracts rather than implementation.

**Emission order is preserved exactly** — status, doc type, subtype, body
format, concerns, materialization fields, known edges, then unknown edges. A
caller that diffs two projections of the same document is comparing
sequences, so a reordering here would read as every fact having changed.

**The facts come out uncaused**, and that is a statement about where
knowledge lives rather than an omission. An envelope knows what it asserts;
it does not know why it is being read. The Dialog-DB writer asserts uncaused
because that is its convergence policy; the spine writer stamps the source
file's content hash before appending. Both are write-site decisions, and
putting a cause here would force this function to hold one of them.

## What a tenant adds

<a name="chunk-doc-facts"></a><sub>[`src/fact.rs`](../../crates/x0k-fact-projection/src/fact.rs) · `#doc-facts`</sub>

```rust {#doc-facts}
/// A tenant's contribution to the file→spine projection.
///
/// `applications-are-tenants` §4 says a tenant's filesystem projections are
/// "registered with the substrate's generic projection mechanism, which
/// parses through the tenant's library rather than compiling the tenant's
/// vocabulary into itself." This trait is that seam, and it lives here
/// rather than in the ingester because this crate is already the neutral
/// contract both sides name: the substrate's file→spine walker
/// (`x0k-folio-daemon`) consumes `DocFactSource`, a tenant
/// (`x0k_grove::file_projection::GroveDocFacts`) implements it, and neither
/// crate has to name the other.
///
/// The substrate keeps everything that is generic — discovering files,
/// hashing them, deciding what changed, stamping the `file-content:` cause,
/// diffing a re-ingest against the prior projection, retracting on delete.
/// The tenant supplies only meaning: given the neutral view of a parsed
/// folio envelope and its body, which additional facts does this document
/// assert in the tenant's own vocabulary?
///
/// Implementations must return an empty vector for documents they do not
/// claim — every document in the corpus is offered to every registered
/// source, and a tenant recognizes its own by `view.doc_type`.
///
/// Facts come back uncaused, exactly like [`project_envelope`]'s: the cause
/// is write-site knowledge. The ingester stamps them and they are then
/// indistinguishable from envelope facts for diffing and retraction.
pub trait DocFactSource: Send + Sync {
    /// Extra facts this document asserts, in the tenant's vocabulary.
    ///
    /// `view` carries the envelope (URI, doc type, resolved edges); `body`
    /// is the document text below the frontmatter.
    fn doc_facts(&self, view: &ColophonView, body: &str) -> Vec<FactEntry>;
}
```

Envelope facts are the ones every document has. A tenant that wants more —
facts in its own vocabulary, read out of the body — implements this. The
contract is that an implementation returns an empty vector for documents it
does not claim: every document is offered to every registered source, and a
tenant recognises its own by `view.doc_type` rather than by being routed.

## Composing the module

<a name="chunk-module-doc"></a><sub>[`src/fact.rs`](../../crates/x0k-fact-projection/src/fact.rs) · `#module-doc`</sub>

```rust {#module-doc}
//! The substrate-neutral fact tuple, and the projection of a folio/v1
//! envelope into one.
//!
//! A fact is `(entity URI, predicate, typed value, optional cause)` — the
//! shape facts have *between* substrates, before a Dialog-DB cache writer
//! applies its `string:`/`entity:` text encoding and before the entry spine
//! wraps them in payloads. [`project_envelope`] turns a document's envelope,
//! viewed through the neutral [`ColophonView`], into a batch of them, and
//! [`DocFactSource`] is the seam a tenant extends to add its own.
```

<a name="chunk-tests"></a><sub>[`src/fact.rs`](../../crates/x0k-fact-projection/src/fact.rs) · `#tests`</sub>

```rust {#tests}
#[cfg(test)]
mod tests {
    use super::*;

    fn sample_view() -> ColophonView {
        ColophonView {
            uri: "x0k:design/example".to_string(),
            status: "proposed".to_string(),
            doc_type: "design".to_string(),
            subtype: Some("ux".to_string()),
            body_format: "markdown".to_string(),
            concerns: vec!["a".to_string(), "b".to_string()],
            materialization_loro_doc_id: Some("doc-1".to_string()),
            materialization_document_revision_id: None,
            materialization_content_hash: Some("abc123".to_string()),
            edges: vec![(
                "motivatedBy".to_string(),
                vec!["x0k:commitment/owner-controlled-ai".to_string()],
            )],
            unknown_edges: vec![(
                "future_predicate".to_string(),
                vec!["x0k:thing/x".to_string()],
            )],
        }
    }

    #[test]
    fn project_envelope_emits_typed_facts_in_order() {
        let facts = project_envelope(&sample_view());

        // Every fact is on the doc URI and uncaused.
        assert!(facts
            .iter()
            .all(|f| f.entity == "x0k:design/example" && f.cause.is_none()));

        let shorthand: Vec<(&str, &FactValue)> = facts
            .iter()
            .map(|f| (f.predicate.as_str(), &f.value))
            .collect();
        let text = |s: &str| FactValue::Text(s.to_string());
        let entity = |s: &str| FactValue::EntityRef(s.to_string());
        let expected_values = [
            (envelope_predicates::STATUS, text("proposed")),
            (envelope_predicates::DOC_TYPE, text("design")),
            (envelope_predicates::SUBTYPE, text("ux")),
            (envelope_predicates::BODY_FORMAT, text("markdown")),
            (envelope_predicates::CONCERN, text("a")),
            (envelope_predicates::CONCERN, text("b")),
            (envelope_predicates::MATERIALIZATION_LORO_DOC, text("doc-1")),
            (
                envelope_predicates::MATERIALIZATION_CONTENT_HASH,
                text("abc123"),
            ),
            ("motivatedBy", entity("x0k:commitment/owner-controlled-ai")),
            ("future_predicate", entity("x0k:thing/x")),
        ];
        let expected: Vec<(&str, &FactValue)> =
            expected_values.iter().map(|(p, v)| (*p, v)).collect();
        assert_eq!(shorthand, expected);
    }


}
```

<a name="chunk-root"></a><sub>[`src/fact.rs`](../../crates/x0k-fact-projection/src/fact.rs) · `#root` · assembles [module-doc](#chunk-module-doc) · [value](#chunk-value) · [value-accessors](#chunk-value-accessors) · [entry](#chunk-entry) · [entry-impl](#chunk-entry-impl) · [predicates](#chunk-predicates) · [view](#chunk-view) · [project](#chunk-project) · [doc-facts](#chunk-doc-facts) · [tests](#chunk-tests)</sub>

```rust {#root file="src/fact.rs"}
<<module-doc>>

<<value>>

<<value-accessors>>

<<entry>>

<<entry-impl>>

<<predicates>>

// The envelope PROJECTION is severable; the predicate NAMES above are not.
// `publication-is-the-shipping-unit` §9 severs `envelope` from the
// published crate, and a published consumer (`x0k-folio`'s materializer)
// reads those names to rebuild a folio from facts — it needs the
// vocabulary, never the projection. Splitting them here is what lets the
// severance cut the ~450 lines below without taking eight constants and a
// published caller with them.
#[cfg(feature = "envelope")]
mod envelope {
    use super::*;

    <<view>>

    <<project>>

    <<doc-facts>>
}

#[cfg(feature = "envelope")]
pub use envelope::{ColophonView, DocFactSource, project_envelope};

<<tests>>
```
