//! The relation graph as a derived fold over fact entries
//! (single-spine plan C2).
//!
//! Per `corpora/x0k/decisions/architecture/substrate/state-representation.md`, the relation
//! graph is not a stored structure — it is a **fold** over the surviving
//! fact entries of the spine's current (or `as_of`) view. This module is
//! that fold, pure and substrate-free: callers hand it the facts the
//! entry store's view already resolved (LWW + region-barrier
//! prefix-pruning happen *in the store*, before this fold ever runs),
//! and it groups them into `entity → predicate → observed value set`.
//!
//! # Observed-set per predicate, LWW only as a tiebreak
//!
//! The default read-time projection per predicate is the
//! **observed set**: a predicate's value is the set of surviving
//! `(entity, predicate, value)` leaves. Multi-valued predicates (edges,
//! concerns) need nothing more — every surviving leaf is a member.
//!
//! Some predicates are semantically single-valued (a doc's status, an
//! intent's title). When more than one value survives for such a
//! predicate, the fold does **not** invent a second arbiter: each fact
//! arrives stamped with the willow `(timestamp, payload_digest)` pair of
//! the entry that carried it, and [`RelationGraph::latest`] picks the
//! maximum under exactly that ordering — the same LWW key the entry
//! store resolves coordinates with. One arbiter, used at two
//! granularities: the store applies it per coordinate, the fold applies
//! it across a predicate's surviving coordinates when a caller asks for
//! a single value.
//!
//! # Retraction: tombstones dominate on the valid-time axis
//!
//! A retraction (single-spine C3) is an ordinary fact whose value is the
//! [`crate::FactValue::Retracted`] tombstone marker wrapping the retracted
//! value. Because [`canonical_value_bytes`] unwraps the marker, a
//! tombstone lands in the SAME `(entity, predicate, value)` cell as the
//! assert it retracts, and the cell resolves by **dominance on the
//! valid-time axis** ([`StampedFact::dominance_key`]):
//!
//! 1. greater `valid_time` wins — entry-carried, never arrival order;
//! 2. at equal valid times, the **retraction wins** — deletion is the
//!    deliberate act, performed with knowledge of the fact it removes,
//!    while an assert at the same instant is ambiguous; safety prefers
//!    absence (the plan's "tombstone-dominates" guard);
//! 3. within the same kind, the willow `(timestamp, payload_digest)`
//!    LWW key breaks the tie, as everywhere else.
//!
//! A cell whose winner is a tombstone is **absent** from every read
//! surface ([`RelationGraph::observed`], [`RelationGraph::all_facts`],
//! …) and exposed typed through [`RelationGraph::retractions`]. The
//! converse holds too: an assert with a *strictly later* valid time
//! re-establishes the fact — deletion is not forever when someone
//! genuinely re-asserts afterward (per `entry-based-substrate.md` §1, a
//! later entry supersedes; the record that the fact was once retracted
//! stays answerable along both time axes).
//!
//! This is the **resurrection guard**: a late-*arriving* assert (synced
//! from a peer that wrote before it saw the removal) carries an earlier
//! valid time than the tombstone, so the tombstone dominates it on any
//! fold, on any peer, regardless of arrival order — B5's cut algebra is
//! untouched, the interpretation lives here in the fold layer exactly
//! as the seam in `x0k-entry-store/src/query.rs` prescribes.
//!
//! # Determinism
//!
//! The fold is order-free: facts are deduplicated by their canonical
//! value bytes ([`crate::payload::canonical_value_bytes`]) within each
//! `(entity, predicate)` cell (keeping the dominant stamp for
//! duplicates), and every iteration surface is sorted — entities and
//! predicates lexically, observed values by canonical bytes. Two folds
//! over the same fact set yield identical graphs regardless of input
//! order.

use std::collections::BTreeMap;

use crate::payload::canonical_value_bytes;
use crate::FactEntry;
#[cfg(test)]
use crate::FactValue;

/// A fact plus the stamps of the entry that carried it: the willow
/// `(timestamp, payload_digest)` pair the entry store's LWW resolution
/// orders by, and the entry's effective **valid time** (the dominance
/// axis for retraction). Spine readers construct these when decoding a
/// view so the fold can reuse the store's arbiters.
#[derive(Debug, Clone, PartialEq)]
pub struct StampedFact {
    /// The decoded fact.
    pub fact: FactEntry,
    /// The carrying entry's willow timestamp (microseconds).
    pub timestamp: u64,
    /// The carrying entry's effective valid time
    /// (`EntryRecord::effective_valid_time`): the explicit `valid_time`
    /// when set, else the willow `timestamp`. Entry-carried, so
    /// dominance over it is peer-order-independent.
    pub valid_time: u64,
    /// The carrying entry's payload digest — the LWW tiebreak at equal
    /// timestamps, mirroring willow's `is_newer_than`.
    pub payload_digest: [u8; 32],
}

impl StampedFact {
    /// The LWW ordering key: greater means newer, ties broken by digest.
    /// Public because the stratum summary fold ([`crate::summary`])
    /// carries per-coordinate maxima under exactly this ordering.
    pub fn lww_key(&self) -> (u64, [u8; 32]) {
        (self.timestamp, self.payload_digest)
    }

    /// Whether this fact is a retraction tombstone.
    pub fn is_retraction(&self) -> bool {
        self.fact.is_retraction()
    }

    /// The cell-resolution ordering (see the module docs): valid time
    /// first, then retraction-beats-assert at equal valid times, then
    /// the willow LWW key. Greater dominates. Total (the digest
    /// tiebreak), which is what makes per-coordinate maxima carry the
    /// fold exactly ([`crate::summary`]).
    pub fn dominance_key(&self) -> (u64, bool, u64, [u8; 32]) {
        (
            self.valid_time,
            self.is_retraction(),
            self.timestamp,
            self.payload_digest,
        )
    }
}

/// The derived relation graph: `entity → predicate → observed set`.
///
/// Inner cells map canonical value bytes → the **dominant** stamped fact
/// for that value (which may be a retraction tombstone — read surfaces
/// skip those cells), so set identity is `(entity, predicate, value)` —
/// the same identity the spine coordinate scheme encodes.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct RelationGraph {
    entities: BTreeMap<String, BTreeMap<String, BTreeMap<Vec<u8>, StampedFact>>>,
}

/// Fold stamped facts into the relation graph. Pure, total, order-free.
///
/// Each `(entity, predicate, value)` cell keeps the fact that wins
/// under [`StampedFact::dominance_key`] — asserts and tombstones
/// compete directly (the canonical-value-bytes cell key unwraps the
/// tombstone marker). Tombstone-winning cells read as absence.
pub fn fold_relation_graph(facts: impl IntoIterator<Item = StampedFact>) -> RelationGraph {
    let mut graph = RelationGraph::default();
    for stamped in facts {
        let key = canonical_value_bytes(&stamped.fact.value);
        let cell = graph
            .entities
            .entry(stamped.fact.entity.clone())
            .or_default()
            .entry(stamped.fact.predicate.clone())
            .or_default();
        match cell.get(&key) {
            Some(existing) if existing.dominance_key() >= stamped.dominance_key() => {}
            _ => {
                cell.insert(key, stamped);
            }
        }
    }
    graph
}

impl RelationGraph {
    /// All subject entities, lexically ordered.
    pub fn entities(&self) -> impl Iterator<Item = &str> {
        self.entities.keys().map(String::as_str)
    }

    /// All predicates observed on `entity`, lexically ordered.
    pub fn predicates(&self, entity: &str) -> impl Iterator<Item = &str> {
        self.entities
            .get(entity)
            .into_iter()
            .flat_map(|preds| preds.keys().map(String::as_str))
    }

    /// The observed set for `(entity, predicate)`: every surviving
    /// `(e, p, v)` leaf, in canonical-value-byte order. Cells whose
    /// dominant fact is a retraction tombstone are absent here.
    pub fn observed(&self, entity: &str, predicate: &str) -> impl Iterator<Item = &StampedFact> {
        self.entities
            .get(entity)
            .and_then(|preds| preds.get(predicate))
            .into_iter()
            .flat_map(|cell| cell.values())
            .filter(|stamped| !stamped.is_retraction())
    }

    /// The retracted cells: every `(entity, predicate, value)` cell
    /// whose dominant fact is a tombstone, as typed stamped facts
    /// (value = [`crate::FactValue::Retracted`] wrapping the retracted value),
    /// in deterministic graph order. First-class so consumers can
    /// materialize removals as removals, not as silent absence.
    ///
    /// The Dialog-DB rebuilder (single-spine C4,
    /// `x0k-grove/src/storage/dialog_db_rebuild.rs`) consumes these
    /// cells via [`RelationGraph::dominant_cells`]: each
    /// tombstone-winning cell materializes as the SAME
    /// `Instruction::Retract` → `State::Removed` block the live
    /// retraction path produces (a "skip-retracted" rebuild yields a
    /// different prolly root than a "retract-then" rebuild; the
    /// determinism/pin claim requires the latter).
    pub fn retractions(&self) -> impl Iterator<Item = &StampedFact> {
        self.entities
            .values()
            .flat_map(|preds| preds.values())
            .flat_map(|cell| cell.values())
            .filter(|stamped| stamped.is_retraction())
    }

    /// Every cell's dominance winner — surviving asserts AND tombstones
    /// interleaved — in the graph's canonical cell order: `(entity,
    /// predicate, canonical value bytes)`, each component ascending.
    ///
    /// This is the canonical fact stream a cache materializer folds
    /// (single-spine C4): the order is **total** because the triple IS
    /// the cell's identity (each cell appears exactly once; components
    /// compare bytewise), and **arrival-independent** because both the
    /// cell set and each cell's winner are pure functions of the fact
    /// SET (the fold's determinism, above) — no arrival index
    /// participates anywhere in the ordering or the resolution.
    pub fn dominant_cells(&self) -> impl Iterator<Item = &StampedFact> {
        self.entities
            .values()
            .flat_map(|preds| preds.values())
            .flat_map(|cell| cell.values())
    }

    /// Single-value read for a semantically single-valued predicate:
    /// the latest surviving value under the store's own LWW ordering
    /// (`(timestamp, payload_digest)` max). `None` when nothing
    /// survives for the cell.
    pub fn latest(&self, entity: &str, predicate: &str) -> Option<&StampedFact> {
        self.observed(entity, predicate)
            .max_by_key(|stamped| stamped.lww_key())
    }

    /// Edge targets: the [`crate::FactValue::EntityRef`] members of the
    /// observed set, in canonical order.
    pub fn edge_targets(&self, entity: &str, predicate: &str) -> Vec<&str> {
        self.observed(entity, predicate)
            .filter_map(|stamped| stamped.fact.value.as_entity_ref())
            .collect()
    }

    /// Flatten the graph back into facts (one per observed leaf,
    /// retracted cells excluded), in the graph's deterministic order.
    pub fn all_facts(&self) -> Vec<FactEntry> {
        self.all_stamped().into_iter().map(|s| s.fact).collect()
    }

    /// Flatten the graph back into stamped facts (one per observed
    /// leaf, retracted cells excluded), in the graph's deterministic
    /// order.
    pub fn all_stamped(&self) -> Vec<StampedFact> {
        self.entities
            .values()
            .flat_map(|preds| preds.values())
            .flat_map(|cell| cell.values())
            .filter(|stamped| !stamped.is_retraction())
            .cloned()
            .collect()
    }

    /// Total number of observed `(entity, predicate, value)` leaves
    /// (retracted cells excluded).
    pub fn len(&self) -> usize {
        self.entities
            .values()
            .flat_map(|preds| preds.values())
            .flat_map(|cell| cell.values())
            .filter(|stamped| !stamped.is_retraction())
            .count()
    }

    /// Whether the graph holds no facts at all.
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn stamped(
        entity: &str,
        predicate: &str,
        value: FactValue,
        timestamp: u64,
        digest_byte: u8,
    ) -> StampedFact {
        StampedFact {
            fact: FactEntry::new(entity, predicate, value),
            timestamp,
            // Mirrors `EntryRecord::effective_valid_time` with no
            // explicit valid time: valid == willow timestamp.
            valid_time: timestamp,
            payload_digest: [digest_byte; 32],
        }
    }

    fn tombstone(
        entity: &str,
        predicate: &str,
        value: FactValue,
        timestamp: u64,
        digest_byte: u8,
    ) -> StampedFact {
        StampedFact {
            fact: FactEntry::new(entity, predicate, FactValue::Retracted(Box::new(value))),
            timestamp,
            valid_time: timestamp,
            payload_digest: [digest_byte; 32],
        }
    }

    #[test]
    fn observed_set_accretes_multi_valued_predicates() {
        let graph = fold_relation_graph(vec![
            stamped(
                "x0k:design/x",
                "motivatedBy",
                FactValue::EntityRef("x0k:commitment/a".into()),
                1,
                1,
            ),
            stamped(
                "x0k:design/x",
                "motivatedBy",
                FactValue::EntityRef("x0k:commitment/b".into()),
                2,
                2,
            ),
        ]);
        let targets = graph.edge_targets("x0k:design/x", "motivatedBy");
        assert_eq!(targets.len(), 2);
        assert!(targets.contains(&"x0k:commitment/a"));
        assert!(targets.contains(&"x0k:commitment/b"));
    }

    #[test]
    fn fold_is_order_free_and_dedups_by_value() {
        let facts = vec![
            stamped("e", "p", FactValue::Text("v".into()), 5, 9),
            stamped("e", "p", FactValue::Text("v".into()), 3, 1),
            stamped("e", "p", FactValue::Text("w".into()), 4, 2),
        ];
        let forward = fold_relation_graph(facts.clone());
        let reverse = fold_relation_graph(facts.into_iter().rev().collect::<Vec<_>>());
        assert_eq!(forward, reverse);
        // Duplicate value collapsed to one leaf, keeping the later stamp.
        assert_eq!(forward.len(), 2);
        let v = forward
            .observed("e", "p")
            .find(|s| s.fact.value.as_text() == Some("v"))
            .unwrap();
        assert_eq!(v.timestamp, 5);
    }

    #[test]
    fn latest_uses_store_lww_ordering() {
        let graph = fold_relation_graph(vec![
            stamped("e", "status", FactValue::Text("proposed".into()), 1, 200),
            stamped("e", "status", FactValue::Text("accepted".into()), 2, 0),
        ]);
        // Greater timestamp wins regardless of digest.
        assert_eq!(
            graph.latest("e", "status").unwrap().fact.value.as_text(),
            Some("accepted")
        );

        // Equal timestamps: greater payload digest wins — the willow
        // tiebreak, not insertion order.
        let tie = fold_relation_graph(vec![
            stamped("e", "status", FactValue::Text("a".into()), 7, 1),
            stamped("e", "status", FactValue::Text("b".into()), 7, 2),
        ]);
        assert_eq!(
            tie.latest("e", "status").unwrap().fact.value.as_text(),
            Some("b")
        );
    }

    #[test]
    fn tombstone_dominates_earlier_valid_assert() {
        // The resurrection guard at fold grain: the assert carries an
        // earlier valid time than the tombstone (a late-arriving sync of
        // a pre-removal write); input order is irrelevant.
        let edge = || FactValue::EntityRef("x0k:commitment/a".into());
        let facts = vec![
            tombstone("e", "motivatedBy", edge(), 300, 1),
            stamped("e", "motivatedBy", edge(), 200, 9),
        ];
        for ordering in [facts.clone(), facts.into_iter().rev().collect()] {
            let graph = fold_relation_graph(ordering);
            assert!(graph.edge_targets("e", "motivatedBy").is_empty());
            assert_eq!(graph.len(), 0);
            let tombs: Vec<_> = graph.retractions().collect();
            assert_eq!(tombs.len(), 1);
            assert!(tombs[0].is_retraction());
            assert_eq!(
                tombs[0].fact.value.retracted_value(),
                Some(&FactValue::EntityRef("x0k:commitment/a".into()))
            );
        }
    }

    #[test]
    fn tombstone_wins_valid_time_tie() {
        // Equal valid times: the retraction wins — deletion is the
        // deliberate act, performed knowing the fact; a same-instant
        // assert is ambiguous and safety prefers absence. Digest bytes
        // are arranged so plain willow LWW would pick the ASSERT,
        // proving the tombstone preference is load-bearing.
        let v = || FactValue::Text("x".into());
        let graph = fold_relation_graph(vec![
            stamped("e", "p", v(), 100, 200),
            tombstone("e", "p", v(), 100, 1),
        ]);
        assert_eq!(graph.observed("e", "p").count(), 0);
        assert_eq!(graph.retractions().count(), 1);
    }

    #[test]
    fn later_valid_assert_reestablishes_after_tombstone() {
        // Deletion is not forever: a strictly later assertion
        // re-establishes the fact (entry-based-substrate §1 — a later
        // entry supersedes; the tombstone stays in history).
        let v = || FactValue::Text("x".into());
        let graph = fold_relation_graph(vec![
            stamped("e", "p", v(), 100, 1),
            tombstone("e", "p", v(), 200, 2),
            stamped("e", "p", v(), 300, 3),
        ]);
        let observed: Vec<_> = graph.observed("e", "p").collect();
        assert_eq!(observed.len(), 1);
        assert_eq!(observed[0].timestamp, 300);
        assert_eq!(graph.retractions().count(), 0);
    }

    #[test]
    fn tombstone_only_affects_its_own_value_cell() {
        let graph = fold_relation_graph(vec![
            stamped(
                "e",
                "motivatedBy",
                FactValue::EntityRef("x0k:commitment/a".into()),
                100,
                1,
            ),
            stamped(
                "e",
                "motivatedBy",
                FactValue::EntityRef("x0k:commitment/b".into()),
                100,
                2,
            ),
            tombstone(
                "e",
                "motivatedBy",
                FactValue::EntityRef("x0k:commitment/a".into()),
                200,
                3,
            ),
        ]);
        assert_eq!(
            graph.edge_targets("e", "motivatedBy"),
            vec!["x0k:commitment/b"]
        );
    }

    #[test]
    fn entities_and_predicates_are_isolated() {
        let graph = fold_relation_graph(vec![
            stamped("a", "p", FactValue::Text("1".into()), 1, 1),
            stamped("b", "p", FactValue::Text("2".into()), 1, 1),
            stamped("a", "q", FactValue::Text("3".into()), 1, 1),
        ]);
        assert_eq!(graph.entities().collect::<Vec<_>>(), vec!["a", "b"]);
        assert_eq!(graph.predicates("a").collect::<Vec<_>>(), vec!["p", "q"]);
        assert_eq!(graph.observed("b", "q").count(), 0);
        assert!(!graph.is_empty());
    }
}
