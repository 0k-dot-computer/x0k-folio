//! Substrate-neutral fact tuples and the read-path seam between fact
//! producers and fact consumers.
//!
//! This crate is the fact-projection *contract*: the shape a fact has
//! between substrates, and the seam a consumer reads it through. It
//! defines:
//!
//! - [`FactEntry`] / [`FactValue`] — the substrate-neutral fact tuple:
//!   entity URI, predicate, typed value, optional cause reference. This is
//!   the shape facts have *between* substrates — before a Dialog-DB cache
//!   writer applies its `string:`/`entity:` text encoding, and before the
//!   entry spine wraps facts in entry payloads.
//! - [`project_envelope`] — projects a folio/v1 envelope (viewed
//!   through the neutral [`ColophonView`]) into typed facts. Lifted
//!   from the folio ingester's `envelope_facts`; the Dialog-DB value
//!   encoding deliberately did NOT move here — it stays at the Dialog-DB
//!   write site (`x0k-folio-daemon/src/folio_ingester.rs`).
//! - [`FactSource`] — the read-side seam: "give me the facts for entity X /
//!   all facts in scope", substrate-agnostic. Grove reconstruction folds
//!   over this trait instead of raw Dialog-DB artifacts, so a spine-backed
//!   implementation arrives without touching the fold logic.
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
//! The [`payload`] module is the spine wire encoding:
//! [`payload::FactPayload`] is the versioned postcard form a FACT entry's
//! payload digest points at, and [`payload::file_content_cause`] is the
//! cause convention for file-ingested facts.
//!
//! The [`relation_graph`] module is the derived
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


pub mod fact;
pub mod materialize;
pub mod payload;
#[cfg(feature = "fold")]
pub mod provenance;
#[cfg(feature = "fold")]
pub mod relation_graph;
#[cfg(feature = "seam")]
pub mod source;
#[cfg(feature = "fold")]
pub mod summary;

// The crate's surface is flat: `fact` and `source` are where the two ideas
// live so that each can be one literate chapter, but consumers name
// `x0k_fact_projection::FactEntry`, not the module it sits in.
pub use fact::{envelope_predicates, FactEntry, FactValue};
#[cfg(feature = "envelope")]
pub use fact::{project_envelope, ColophonView, DocFactSource};
pub use materialize::{
    LiveEditPolicy, MaterializeError, Materializer, Mount, MountPosture, PathTemplate,
    PathTemplateError, Rendered, DEFAULT_MOUNT,
};
#[cfg(feature = "fold")]
pub use provenance::FactProvenance;
#[cfg(feature = "seam")]
pub use source::{CoverCursor, CoverMotion, FactSource, FactSourceError, IncrementalFactSource};
