//! Substrate-neutral fact tuples and the read-path seam between fact
//! producers and fact consumers.
//!
//! This crate is Phase C0 of the single-spine substrate plan
//! (`.claude/plans/single-spine-substrate-impl.md`): the fact-projection
//! *contract*. It defines:
//!
//! - [`FactEntry`] / [`FactValue`] — the substrate-neutral fact tuple:
//!   entity URI, predicate, typed value, optional cause reference. This is
//!   the shape facts have *between* substrates — before a Dialog-DB cache
//!   writer applies its `string:`/`entity:` text encoding, and before the
//!   entry spine wraps facts in entry payloads (C1+).
//! - [`project_envelope`] — projects a folio/v1 envelope (viewed
//!   through the neutral [`ColophonView`]) into typed facts. Lifted
//!   from the folio ingester's `envelope_facts`; the Dialog-DB value
//!   encoding deliberately did NOT move here — it stays at the Dialog-DB
//!   write site (`x0k-folio-daemon/src/folio_ingester.rs`).
//! - [`project_grove_entity`] — the equivalent projection for grove
//!   entities (seeds/intents/…): an entity URI plus typed attribute
//!   assertions becomes a fact batch.
//! - [`FactSource`] — the read-side seam: "give me the facts for entity X /
//!   all facts in scope", substrate-agnostic. Grove reconstruction folds
//!   over this trait instead of raw Dialog-DB artifacts; later phases (C2)
//!   add a spine-backed implementation without touching the fold logic.
//! - [`IncrementalFactSource`] / [`CoverCursor`] — the *incremental* half of
//!   that seam: a watermark that advances iff the source can have changed,
//!   the entity worklist between two watermarks, and the cursor discipline
//!   that turns them into "re-derive only what moved". [`FactSource`] alone
//!   is snapshot-only, so a consumer that folds on every write had to reach
//!   past it to a concrete source and hand-roll the loop; this is where that
//!   capability lives instead.
//!
//! The crate is deliberately wasm-clean and dependency-free (see
//! `Cargo.toml`); the Dialog-DB `FactSource` implementation lives in
//! `x0k-grove`, not here.
//!
//! Governing decision: `corpora/x0k/decisions/architecture/substrate/state-representation.md`
//! (facts are entries on the spine; the relation graph is a derived fold;
//! Dialog-DB demotes to a rebuildable query cache).
//!
//! The [`payload`] module (single-spine C1) is the spine wire encoding:
//! [`payload::FactPayload`] is the versioned postcard form a FACT entry's
//! payload digest points at, and [`payload::file_content_cause`] is the
//! cause convention for file-ingested facts.
//!
//! The [`relation_graph`] module (single-spine C2) is the derived
//! relation-graph fold: [`relation_graph::fold_relation_graph`] groups a
//! resolved view's stamped facts into `entity → predicate → observed
//! set`, with the store's LWW ordering reused for single-value reads.
//!
//! The [`provenance`] module is the typed fact provenance of the
//! `cell-substrate` provenance-outbound amendment:
//! [`provenance::FactProvenance`] (producer identity + journal seq)
//! renders into the `cause` slot as `journal:<producer>@<seq>`,
//! replacing free-form cause strings for substrate-connected journaled
//! writers. Attribution metadata only — dominance rules do not change.

pub mod payload;
pub mod provenance;
pub mod relation_graph;
pub mod summary;

pub use provenance::FactProvenance;

/// A typed, substrate-neutral fact value.
///
/// Mirrors the value vocabulary of `dialog_artifacts::Value` without
/// depending on Dialog-DB, so producers and consumers can exchange facts
/// independent of which substrate stores them. Cache writers decide their
/// own encodings (e.g. Dialog-DB's `string:<text>` / `entity:<uri>` UTF-8
/// convention); spine writers decide payload framing (C1+).
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
    /// A **retraction marker** (single-spine C3): the tombstone that
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

/// Project a grove entity (seed, intent, season, …) into substrate-neutral
/// facts: one [`FactEntry`] per typed attribute assertion, all on the same
/// subject URI, uncaused.
///
/// This is the neutral core of what the grove SDK write path
/// (`x0k-grove/src/storage/sdk/`) produces as per-attribute artifact
/// asserts (`SdkArtifact::new(uri, predicate, value)`): the *what* — which
/// (predicate, typed value) pairs an entity yields — minus the Dialog-DB
/// specifics (instruction batching, dual-write predicate spellings, value
/// text encoding), which stay at the Dialog-DB write site. The spine write
/// path (C2+) appends these same facts as fact entries.
pub fn project_grove_entity(
    entity_uri: &str,
    attributes: impl IntoIterator<Item = (String, FactValue)>,
) -> Vec<FactEntry> {
    attributes
        .into_iter()
        .map(|(predicate, value)| FactEntry::new(entity_uri, predicate, value))
        .collect()
}

/// The substrate-agnostic read seam for facts.
///
/// Consumers (grove reconstruction folds, future relation-graph folds) ask
/// a `FactSource` for facts instead of reading a storage backend directly,
/// so the backing substrate can move from Dialog-DB artifacts (today's
/// `x0k-grove` adapter) to the entry spine (C2) without touching fold
/// logic.
///
/// `all_facts` returns every fact in the source's *scope* — sources are
/// typically already scoped (e.g. "the artifacts loaded for one entity",
/// "one subspace of the spine"); the scope is the constructor's contract.
pub trait FactSource {
    /// All facts in this source's scope whose subject is `entity_uri`.
    fn facts_for_entity(&self, entity_uri: &str) -> Vec<FactEntry>;

    /// Every fact in this source's scope.
    fn all_facts(&self) -> Vec<FactEntry>;
}

/// Any in-memory fact slice is a `FactSource` over exactly those facts.
impl FactSource for [FactEntry] {
    fn facts_for_entity(&self, entity_uri: &str) -> Vec<FactEntry> {
        self.iter()
            .filter(|f| f.entity == entity_uri)
            .cloned()
            .collect()
    }

    fn all_facts(&self) -> Vec<FactEntry> {
        self.to_vec()
    }
}

impl FactSource for Vec<FactEntry> {
    fn facts_for_entity(&self, entity_uri: &str) -> Vec<FactEntry> {
        self.as_slice().facts_for_entity(entity_uri)
    }

    fn all_facts(&self) -> Vec<FactEntry> {
        self.clone()
    }
}

/// The failure a fallible fact-source read can report, kept substrate-neutral
/// so this crate stays wasm-clean and free of storage types.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FactSourceError(pub String);

impl core::fmt::Display for FactSourceError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for FactSourceError {}

/// The incremental half of a fact source: a position on the source's
/// transaction-time axis, and the entity worklist between two positions.
///
/// [`FactSource`] is snapshot-only by design — it exists so a fold can move
/// between substrates, and `all_facts()` is as unbounded as the store behind
/// it. That makes it the wrong seam for a consumer that re-derives *on every
/// write*: such a consumer needs to know whether anything it depends on
/// actually moved, and if so, which entities. Without that question in the
/// type system, every incremental consumer re-invents the same three fields
/// (a concrete source, a `u64` watermark, an early-out) and one of them
/// eventually forgets, which is how a one-fact delta comes to cost a
/// whole-spine fold (`pairing.fleet_grants.reconciled`, 2026-08-24).
///
/// The watermark's only contract is **monotone and complete**: it advances if
/// and only if the source can have changed. Its numeric value is the source's
/// private business — it is a resume token, never a cross-node coordinate.
pub trait IncrementalFactSource: FactSource {
    /// The source's current position. Equal on two reads means nothing was
    /// written between them.
    fn fact_watermark(&self) -> Result<u64, FactSourceError>;

    /// Entity URIs touched by any write strictly after `since`.
    fn entities_changed_since(
        &self,
        since: u64,
    ) -> Result<std::collections::BTreeSet<String>, FactSourceError>;

    /// [`Self::entities_changed_since`] restricted to entities whose URI is
    /// prefixed by one of `cover_prefixes` — and, where the source can, priced
    /// by that cover rather than by the arrival range. An empty cover admits
    /// nothing; a consumer that depends on everything asks
    /// [`Self::entities_changed_since`] instead.
    fn entities_changed_within(
        &self,
        since: u64,
        cover_prefixes: &[&str],
    ) -> Result<std::collections::BTreeSet<String>, FactSourceError>;
}

/// What one poll of a [`CoverCursor`] found.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CoverMotion {
    /// The source has not advanced past the cursor. Nothing to do, and
    /// nothing was read beyond the watermark probe.
    Unmoved,
    /// The source advanced, but nothing under the cover moved. The derived
    /// value still stands; commit `watermark` so the next poll starts here.
    Advanced { watermark: u64 },
    /// Entities under the cover changed. Re-derive from `entities` (or from
    /// the cover wholesale, if the fold is not entity-incremental), then
    /// commit `watermark`.
    Moved {
        entities: std::collections::BTreeSet<String>,
        watermark: u64,
    },
}

impl CoverMotion {
    /// The watermark this motion permits committing, if any.
    pub fn watermark(&self) -> Option<u64> {
        match self {
            CoverMotion::Unmoved => None,
            CoverMotion::Advanced { watermark } | CoverMotion::Moved { watermark, .. } => {
                Some(*watermark)
            }
        }
    }

    /// Whether the caller must re-derive.
    pub fn requires_rederivation(&self) -> bool {
        matches!(self, CoverMotion::Moved { .. })
    }
}

/// One consumer's position on a fact source, plus the cover it depends on.
///
/// This is the "hold a watermark, early-out when it has not advanced,
/// re-derive only the changed slice" shape, as a type rather than as three
/// fields each consumer remembers to declare. Two properties are structural
/// rather than conventional:
///
/// - **Polling cannot advance the cursor.** [`Self::poll`] takes `&self`; only
///   [`Self::commit`] moves the position. A consumer whose re-derivation fails
///   therefore cannot skip the work it dropped — the failure mode the
///   hand-rolled copies avoid by discipline, and which nothing checked.
/// - **The cover is declared once, at construction.** The cover is what makes
///   both the *wake* and the *read* narrow, so it is one declaration serving
///   both, not a prefix list re-derived at each call site.
///
/// An empty cover means "everything", and polls fall back to the unscoped
/// worklist: a whole-corpus consumer genuinely depends on every write and has
/// no cheaper question to ask.
#[derive(Debug, Clone)]
pub struct CoverCursor {
    cover: Vec<String>,
    watermark: u64,
}

impl CoverCursor {
    /// A cursor at the beginning of time over `cover` (entity-URI prefixes;
    /// empty means the whole source).
    pub fn new(cover: impl IntoIterator<Item = impl Into<String>>) -> Self {
        Self {
            cover: cover.into_iter().map(Into::into).collect(),
            watermark: 0,
        }
    }

    /// A cursor already positioned at `watermark` — the shape a consumer that
    /// bootstrapped from a full read starts in, so its first poll does not
    /// re-derive what the bootstrap already covered.
    pub fn at(cover: impl IntoIterator<Item = impl Into<String>>, watermark: u64) -> Self {
        let mut cursor = Self::new(cover);
        cursor.watermark = watermark;
        cursor
    }

    /// The committed position.
    pub fn watermark(&self) -> u64 {
        self.watermark
    }

    /// The declared cover, empty meaning everything.
    pub fn cover(&self) -> &[String] {
        &self.cover
    }

    /// Ask the source what moved since the committed position.
    ///
    /// Cheap when nothing did: the watermark probe alone answers
    /// [`CoverMotion::Unmoved`], and a cover-scoped worklist answers
    /// [`CoverMotion::Advanced`] without reading a fact.
    pub fn poll<S>(&self, source: &S) -> Result<CoverMotion, FactSourceError>
    where
        S: IncrementalFactSource + ?Sized,
    {
        let watermark = source.fact_watermark()?;
        if watermark <= self.watermark {
            return Ok(CoverMotion::Unmoved);
        }
        let entities = if self.cover.is_empty() {
            source.entities_changed_since(self.watermark)?
        } else {
            let prefixes: Vec<&str> = self.cover.iter().map(String::as_str).collect();
            source.entities_changed_within(self.watermark, &prefixes)?
        };
        if entities.is_empty() {
            return Ok(CoverMotion::Advanced { watermark });
        }
        Ok(CoverMotion::Moved {
            entities,
            watermark,
        })
    }

    /// Advance to a watermark whose work the caller has finished.
    ///
    /// Monotone: a stale commit is ignored rather than rewinding the cursor
    /// into work it already did.
    pub fn commit(&mut self, watermark: u64) {
        self.watermark = self.watermark.max(watermark);
    }

    /// Commit whatever position `motion` reported, if any. The one-line form
    /// of "I processed exactly what the poll told me about".
    pub fn commit_motion(&mut self, motion: &CoverMotion) {
        if let Some(watermark) = motion.watermark() {
            self.commit(watermark);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A fact source whose watermark and changed-entity worklist are set by
    /// the test — enough to exercise the cursor's decisions without a store.
    #[derive(Default)]
    struct FakeSource {
        watermark: u64,
        /// `(watermark_at_write, entity)` — everything written, in order.
        writes: Vec<(u64, String)>,
    }

    impl FakeSource {
        fn write(&mut self, entity: &str) {
            self.watermark += 1;
            self.writes.push((self.watermark, entity.to_string()));
        }

        fn since(&self, since: u64) -> std::collections::BTreeSet<String> {
            self.writes
                .iter()
                .filter(|(at, _)| *at > since)
                .map(|(_, entity)| entity.clone())
                .collect()
        }
    }

    impl FactSource for FakeSource {
        fn facts_for_entity(&self, _entity_uri: &str) -> Vec<FactEntry> {
            Vec::new()
        }
        fn all_facts(&self) -> Vec<FactEntry> {
            Vec::new()
        }
    }

    impl IncrementalFactSource for FakeSource {
        fn fact_watermark(&self) -> Result<u64, FactSourceError> {
            Ok(self.watermark)
        }
        fn entities_changed_since(
            &self,
            since: u64,
        ) -> Result<std::collections::BTreeSet<String>, FactSourceError> {
            Ok(self.since(since))
        }
        fn entities_changed_within(
            &self,
            since: u64,
            cover_prefixes: &[&str],
        ) -> Result<std::collections::BTreeSet<String>, FactSourceError> {
            Ok(self
                .since(since)
                .into_iter()
                .filter(|entity| cover_prefixes.iter().any(|p| entity.starts_with(p)))
                .collect())
        }
    }

    #[test]
    fn cursor_is_unmoved_when_the_source_has_not_advanced() {
        let source = FakeSource::default();
        let cursor = CoverCursor::new(["x0k:fleet-grant-policy/"]);
        assert_eq!(cursor.poll(&source).unwrap(), CoverMotion::Unmoved);
    }

    /// The defect this whole seam exists for: writes land, the source advances,
    /// and a consumer whose cover they miss must learn that WITHOUT re-deriving.
    #[test]
    fn writes_outside_the_cover_advance_without_requiring_rederivation() {
        let mut source = FakeSource::default();
        let cursor = CoverCursor::new(["x0k:fleet-grant-policy/"]);
        for i in 0..500 {
            source.write(&format!("x0k:design/unrelated-{i}"));
        }
        let motion = cursor.poll(&source).unwrap();
        assert_eq!(motion, CoverMotion::Advanced { watermark: 500 });
        assert!(!motion.requires_rederivation());
    }

    #[test]
    fn writes_inside_the_cover_report_exactly_the_changed_entities() {
        let mut source = FakeSource::default();
        let cursor = CoverCursor::new(["x0k:fleet-grant-policy/"]);
        source.write("x0k:design/unrelated");
        source.write("x0k:fleet-grant-policy/handheld/http");
        let CoverMotion::Moved {
            entities,
            watermark,
        } = cursor.poll(&source).unwrap()
        else {
            panic!("a covered write must report as moved");
        };
        assert_eq!(watermark, 2);
        assert_eq!(
            entities.into_iter().collect::<Vec<_>>(),
            ["x0k:fleet-grant-policy/handheld/http"]
        );
    }

    /// An empty cover means "everything" — the whole-corpus consumer's shape.
    #[test]
    fn an_empty_cover_admits_every_changed_entity() {
        let mut source = FakeSource::default();
        let cursor = CoverCursor::new(Vec::<String>::new());
        source.write("x0k:design/anything");
        assert!(cursor.poll(&source).unwrap().requires_rederivation());
    }

    /// Polling must not advance the cursor: a consumer whose re-derivation
    /// fails has to see the same slice again, not skip it. This is the
    /// property the hand-rolled copies held by discipline and nothing checked.
    #[test]
    fn polling_does_not_advance_the_cursor_only_committing_does() {
        let mut source = FakeSource::default();
        let mut cursor = CoverCursor::new(["x0k:fleet-grant-policy/"]);
        source.write("x0k:fleet-grant-policy/handheld/http");

        let first = cursor.poll(&source).unwrap();
        assert!(first.requires_rederivation());
        assert_eq!(cursor.watermark(), 0, "poll must be read-only");

        // Pretend the re-derivation failed: no commit. The same work is still
        // pending.
        assert_eq!(cursor.poll(&source).unwrap(), first);

        cursor.commit_motion(&first);
        assert_eq!(cursor.watermark(), 1);
        assert_eq!(cursor.poll(&source).unwrap(), CoverMotion::Unmoved);
    }

    /// Commits are monotone, so a stale one cannot rewind the cursor into work
    /// it already finished.
    #[test]
    fn commit_never_rewinds() {
        let mut cursor = CoverCursor::new(["x0k:fleet-grant-policy/"]);
        cursor.commit(9);
        cursor.commit(4);
        assert_eq!(cursor.watermark(), 9);
    }

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

    #[test]
    fn project_grove_entity_stamps_uri_on_every_fact() {
        let facts = project_grove_entity(
            "x0k:intent/abc",
            vec![
                (
                    "vendor:intent/title".to_string(),
                    FactValue::Text("Do the thing".to_string()),
                ),
                (
                    "vendor:intent/from-seed".to_string(),
                    FactValue::EntityRef("x0k:seed/xyz".to_string()),
                ),
                (
                    "vendor:intent/created-at".to_string(),
                    FactValue::SignedInt(42),
                ),
            ],
        );
        assert_eq!(facts.len(), 3);
        assert!(facts.iter().all(|f| f.entity == "x0k:intent/abc"));
        assert_eq!(facts[0].value.as_text(), Some("Do the thing"));
        assert_eq!(facts[1].value.as_entity_ref(), Some("x0k:seed/xyz"));
        assert_eq!(facts[2].value.as_signed_int(), Some(42));
    }

    #[test]
    fn slice_fact_source_scopes_by_entity() {
        let facts = vec![
            FactEntry::new("x0k:seed/a", "p/title", FactValue::Text("A".into())),
            FactEntry::new("x0k:seed/b", "p/title", FactValue::Text("B".into())),
        ];
        assert_eq!(facts.all_facts().len(), 2);
        let for_a = facts.facts_for_entity("x0k:seed/a");
        assert_eq!(for_a.len(), 1);
        assert_eq!(for_a[0].value.as_text(), Some("A"));
    }
}
