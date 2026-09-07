//! The fact-genus stratum **start summary** (single-spine D5a, design
//! `.claude/plans/single-spine-d5-compaction-design.md` §4b).
//!
//! A fact stratum covers one writer subspace's contiguous chain
//! segment. Its start state is not a compressed op run — the fact fold
//! (`stamped_members_at` → [`crate::relation_graph::fold_relation_graph`])
//! is a per-cell dominance **max**, a commutative-semigroup fold, so
//! `max(covered ∪ loose) = max(max(covered), loose)` and the summary
//! only has to carry the per-coordinate maxima that can ever win:
//!
//! - `lww_max` — max by `(timestamp, payload_digest)` over all covered
//!   members at the coordinate. Serves coordinate-LWW reads (pins).
//! - `dominance_max` — max **assert** by
//!   [`StampedFact::dominance_key`], chosen among the asserts that
//!   survive the segment's own barriers (in-segment pruning is fully
//!   evaluated at seal time; the monotone-sink-clock invariant
//!   guarantees a barrier whose stamp falls between two covered
//!   asserts is itself covered).
//! - `tombstone_max` — max **tombstone** by dominance key, carried
//!   unconditionally (tombstones are barrier-exempt): it remains the
//!   resurrection guard across shed history.
//!
//! plus, per covered entity region, `max_barrier_ts` — the strict
//! threshold a covered barrier imposes on *deeper* segments' carried
//! asserts (cross-segment pruning needs only the max because pruning
//! is a strict timestamp comparison, no ties by the sink invariant).
//!
//! `lww_max` and `dominance_max` differ exactly when a correction
//! backdates valid time; carrying both keeps the two read disciplines
//! (pin LWW, relation-fold dominance) exact.
//!
//! The summary has a canonical byte encoding (sorted coordinates,
//! length-prefixed fields, big-endian integers) so its blake3
//! [`FactSummary::digest`] is identical on every node — that digest is
//! what `x0k_entry_store::strata::StartIdentity::FactSummary` hashes
//! into the stratum id. Everything here is pure and wasm-clean.
//!
//! D5a scope: the summary and its composition law exist and are
//! property-tested; no store materializes or reads through summaries
//! yet (that is D5b's `stamped_members_at` branch).

use std::collections::BTreeMap;

use crate::payload::{canonical_value_bytes, FactPayload};
use crate::relation_graph::StampedFact;

/// Domain separator for [`FactSummary::digest`]. Bump on any change to
/// the canonical encoding.
const SUMMARY_DOMAIN: &[u8] = b"x0k.fact_summary.v1\0";

/// One member of a chain segment, as the summary fold consumes it:
/// either a decoded fact leaf (assert or tombstone) or a region
/// barrier (`facts/<entity>`, empty payload — only its coordinate and
/// stamp matter).
#[derive(Debug, Clone)]
pub enum SegmentMember {
    /// A fact leaf (assert or retraction tombstone), stamped.
    Fact(StampedFact),
    /// A region barrier over `facts/<entity>` at `timestamp`.
    Barrier {
        /// The entity URI whose region the barrier replaces.
        entity: String,
        /// The barrier's willow timestamp.
        timestamp: u64,
    },
}

/// A carried fact: enough to re-materialize the [`StampedFact`] after
/// the covered member payloads are shed. `payload_bytes` is the
/// canonical postcard [`FactPayload`] encoding (= what the member's
/// payload digest pointed at).
#[derive(Debug, Clone, PartialEq)]
pub struct StampedCarry {
    /// Canonical `FactPayload` bytes of the carried fact.
    pub payload_bytes: Vec<u8>,
    /// The carried entry's willow timestamp.
    pub timestamp: u64,
    /// The carried entry's effective valid time.
    pub valid_time: u64,
    /// The carried entry's payload digest (LWW tiebreak).
    pub payload_digest: [u8; 32],
}

impl StampedCarry {
    /// Carry one stamped fact.
    pub fn from_stamped(stamped: &StampedFact) -> Self {
        Self {
            payload_bytes: FactPayload::from_fact(&stamped.fact).to_bytes(),
            timestamp: stamped.timestamp,
            valid_time: stamped.valid_time,
            payload_digest: stamped.payload_digest,
        }
    }

    /// Re-materialize the carried [`StampedFact`].
    pub fn to_stamped(&self) -> Result<StampedFact, crate::payload::FactPayloadError> {
        Ok(StampedFact {
            fact: FactPayload::from_bytes(&self.payload_bytes)?.into_fact(),
            timestamp: self.timestamp,
            valid_time: self.valid_time,
            payload_digest: self.payload_digest,
        })
    }
}

/// The identity of one fold cell within a subspace: the spine
/// coordinate `(entity, predicate, blake3(canonical value bytes))`.
pub type CoordinateKey = (String, String, [u8; 32]);

/// The three per-coordinate maxima (see the module docs).
#[derive(Debug, Clone, Default, PartialEq)]
pub struct CoordinateCarries {
    /// Max over all covered members by `(timestamp, payload_digest)`.
    pub lww_max: Option<StampedCarry>,
    /// Max covered **assert** by dominance key, among in-segment
    /// barrier survivors.
    pub dominance_max: Option<StampedCarry>,
    /// Max covered **tombstone** by dominance key (barrier-exempt).
    pub tombstone_max: Option<StampedCarry>,
}

/// The canonical summary of one writer subspace's chain segment.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct FactSummary {
    /// Per covered coordinate, sorted by the coordinate key.
    pub coordinates: BTreeMap<CoordinateKey, CoordinateCarries>,
    /// Per covered entity region: the max covered barrier timestamp.
    pub regions: BTreeMap<String, u64>,
}

/// Fold a segment's members into its summary. Pure, total, order-free:
/// every output is a maximum under a total order over the member SET,
/// so any permutation of `members` yields an identical (and
/// identically-encoded) summary.
pub fn summarize_segment(members: impl IntoIterator<Item = SegmentMember>) -> FactSummary {
    let mut facts: Vec<StampedFact> = Vec::new();
    let mut regions: BTreeMap<String, u64> = BTreeMap::new();
    for member in members {
        match member {
            SegmentMember::Fact(stamped) => facts.push(stamped),
            SegmentMember::Barrier { entity, timestamp } => {
                let slot = regions.entry(entity).or_insert(timestamp);
                *slot = (*slot).max(timestamp);
            }
        }
    }

    let mut coordinates: BTreeMap<CoordinateKey, CoordinateCarries> = BTreeMap::new();
    for stamped in &facts {
        let key = coordinate_key(stamped);
        let carries = coordinates.entry(key).or_default();

        let beats = |slot: &Option<StampedCarry>, key: (u64, bool, u64, [u8; 32])| {
            slot.as_ref()
                .and_then(|c| c.to_stamped().ok())
                .is_none_or(|prev| key > prev.dominance_key())
        };

        // lww_max: every covered member competes.
        let lww_beats = carries
            .lww_max
            .as_ref()
            .map(|c| (c.timestamp, c.payload_digest))
            .is_none_or(|prev| stamped.lww_key() > prev);
        if lww_beats {
            carries.lww_max = Some(StampedCarry::from_stamped(stamped));
        }

        if stamped.is_retraction() {
            // tombstone_max: barrier-exempt, carried unconditionally.
            if beats(&carries.tombstone_max, stamped.dominance_key()) {
                carries.tombstone_max = Some(StampedCarry::from_stamped(stamped));
            }
        } else {
            // dominance_max: only in-segment barrier SURVIVORS compete
            // — in-segment pruning is fully evaluated at seal time.
            let pruned = regions
                .get(&stamped.fact.entity)
                .is_some_and(|barrier_ts| *barrier_ts > stamped.timestamp);
            if !pruned && beats(&carries.dominance_max, stamped.dominance_key()) {
                carries.dominance_max = Some(StampedCarry::from_stamped(stamped));
            }
        }
    }

    FactSummary {
        coordinates,
        regions,
    }
}

/// The cell coordinate of a stamped fact — the same identity the spine
/// path encodes (`facts/<entity>/<predicate>/<value-digest>`; the
/// canonical value bytes unwrap the tombstone marker, so a tombstone
/// shares its assert's coordinate).
pub fn coordinate_key(stamped: &StampedFact) -> CoordinateKey {
    (
        stamped.fact.entity.clone(),
        stamped.fact.predicate.clone(),
        *blake3::hash(&canonical_value_bytes(&stamped.fact.value)).as_bytes(),
    )
}

impl FactSummary {
    /// The canonical encoding [`FactSummary::digest`] hashes —
    /// hand-rolled, length-prefixed, big-endian, maps iterated in their
    /// (sorted) key order. The byte layout is the schema; the domain
    /// separator versions it.
    pub fn canonical_bytes(&self) -> Vec<u8> {
        let mut out = Vec::new();
        out.extend_from_slice(&(self.coordinates.len() as u32).to_be_bytes());
        for ((entity, predicate, value_digest), carries) in &self.coordinates {
            put_bytes(&mut out, entity.as_bytes());
            put_bytes(&mut out, predicate.as_bytes());
            out.extend_from_slice(value_digest);
            for carry in [
                &carries.lww_max,
                &carries.dominance_max,
                &carries.tombstone_max,
            ] {
                match carry {
                    None => out.push(0u8),
                    Some(carry) => {
                        out.push(1u8);
                        put_bytes(&mut out, &carry.payload_bytes);
                        out.extend_from_slice(&carry.timestamp.to_be_bytes());
                        out.extend_from_slice(&carry.valid_time.to_be_bytes());
                        out.extend_from_slice(&carry.payload_digest);
                    }
                }
            }
        }
        out.extend_from_slice(&(self.regions.len() as u32).to_be_bytes());
        for (entity, max_barrier_ts) in &self.regions {
            put_bytes(&mut out, entity.as_bytes());
            out.extend_from_slice(&max_barrier_ts.to_be_bytes());
        }
        out
    }

    /// blake3 over the domain-separated canonical encoding — the
    /// content address `StartIdentity::FactSummary` hashes into the
    /// stratum id. Deterministic on every node holding the member set.
    pub fn digest(&self) -> [u8; 32] {
        let mut hasher = blake3::Hasher::new();
        hasher.update(SUMMARY_DOMAIN);
        hasher.update(&self.canonical_bytes());
        *hasher.finalize().as_bytes()
    }

    /// The summary's contribution to a relation-graph fold, with
    /// **live** (loose, chain-later) barriers of the same subspace
    /// applied: carried asserts whose stamp falls below a loose
    /// barrier over their region are dropped (a loose barrier is
    /// chain-later, hence strictly later than every covered stamp —
    /// it prunes all carried asserts of its region or none); carried
    /// tombstones are exempt, exactly as in `stamped_members_at`.
    /// `lww_max` carries do not participate — they serve coordinate-LWW
    /// pin reads, and an in-segment-pruned assert must not re-enter the
    /// fold through them.
    ///
    /// This is the read-time composition law D5b's member-selection
    /// branch implements; here it exists pure so the fold-preservation
    /// differential can prove `fold(summary ∘ loose) == fold(members)`.
    pub fn fold_members(
        &self,
        loose_barriers: &[(String, u64)],
    ) -> Result<Vec<StampedFact>, crate::payload::FactPayloadError> {
        let mut out = Vec::new();
        for carries in self.coordinates.values() {
            if let Some(carry) = &carries.dominance_max {
                let stamped = carry.to_stamped()?;
                let pruned = loose_barriers.iter().any(|(entity, barrier_ts)| {
                    *entity == stamped.fact.entity && *barrier_ts > stamped.timestamp
                });
                if !pruned {
                    out.push(stamped);
                }
            }
            if let Some(carry) = &carries.tombstone_max {
                out.push(carry.to_stamped()?);
            }
        }
        Ok(out)
    }
}

fn put_bytes(out: &mut Vec<u8>, bytes: &[u8]) {
    out.extend_from_slice(&(bytes.len() as u32).to_be_bytes());
    out.extend_from_slice(bytes);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{FactEntry, FactValue};

    fn stamped(entity: &str, value: &str, ts: u64, valid: u64, digest: u8) -> StampedFact {
        StampedFact {
            fact: FactEntry::new(entity, "p", FactValue::Text(value.into())),
            timestamp: ts,
            valid_time: valid,
            payload_digest: [digest; 32],
        }
    }

    fn tombstone(entity: &str, value: &str, ts: u64, digest: u8) -> StampedFact {
        StampedFact {
            fact: FactEntry::new(
                entity,
                "p",
                FactValue::Retracted(Box::new(FactValue::Text(value.into()))),
            ),
            timestamp: ts,
            valid_time: ts,
            payload_digest: [digest; 32],
        }
    }

    #[test]
    fn summary_is_order_free_and_digest_canonical() {
        let members = vec![
            SegmentMember::Fact(stamped("e", "v", 100, 100, 1)),
            SegmentMember::Barrier {
                entity: "e".into(),
                timestamp: 150,
            },
            SegmentMember::Fact(stamped("e", "v", 200, 200, 2)),
            SegmentMember::Fact(tombstone("e", "w", 210, 3)),
        ];
        let forward = summarize_segment(members.clone());
        let reverse = summarize_segment(members.into_iter().rev().collect::<Vec<_>>());
        assert_eq!(forward, reverse);
        assert_eq!(forward.digest(), reverse.digest());
    }

    #[test]
    fn dominance_max_respects_in_segment_barriers_lww_max_does_not() {
        // a1(ts=100) is pruned by the in-segment barrier at 150;
        // a2(ts=200, but BACKDATED valid=90) survives it. The
        // dominance carry must be a2 (the survivor), while lww_max is
        // free to be the raw newest (also a2 here by timestamp).
        let a1 = stamped("e", "v", 100, 100, 1);
        let a2 = stamped("e", "v", 200, 90, 2);
        let summary = summarize_segment(vec![
            SegmentMember::Fact(a1.clone()),
            SegmentMember::Barrier {
                entity: "e".into(),
                timestamp: 150,
            },
            SegmentMember::Fact(a2.clone()),
        ]);
        let carries = summary.coordinates.values().next().unwrap();
        assert_eq!(
            carries
                .dominance_max
                .as_ref()
                .unwrap()
                .to_stamped()
                .unwrap(),
            a2,
            "in-segment pruning is evaluated at seal time"
        );
        assert_eq!(summary.regions.get("e"), Some(&150));

        // With the barrier at 250 (pruning BOTH asserts), nothing is
        // carried for dominance — but lww_max still records the raw
        // newest member for pin reads.
        let summary = summarize_segment(vec![
            SegmentMember::Fact(a1),
            SegmentMember::Fact(a2.clone()),
            SegmentMember::Barrier {
                entity: "e".into(),
                timestamp: 250,
            },
        ]);
        let carries = summary.coordinates.values().next().unwrap();
        assert!(carries.dominance_max.is_none());
        assert_eq!(carries.lww_max.as_ref().unwrap().to_stamped().unwrap(), a2);
        // …and fold_members surfaces nothing for the cell.
        assert!(summary.fold_members(&[]).unwrap().is_empty());
    }

    #[test]
    fn tombstone_carry_is_barrier_exempt_and_loose_barriers_prune_asserts() {
        let summary = summarize_segment(vec![
            SegmentMember::Fact(stamped("e", "v", 100, 100, 1)),
            SegmentMember::Fact(tombstone("e", "w", 110, 2)),
        ]);
        // A loose (chain-later) barrier prunes the carried assert but
        // never the tombstone — the resurrection guard outlives
        // region replacement.
        let folded = summary.fold_members(&[("e".into(), 500)]).unwrap();
        assert_eq!(folded.len(), 1);
        assert!(folded[0].is_retraction());
        // A loose barrier over a DIFFERENT entity prunes nothing.
        let folded = summary.fold_members(&[("other".into(), 500)]).unwrap();
        assert_eq!(folded.len(), 2);
    }

    #[test]
    fn carry_roundtrips_through_payload_bytes() {
        let original = stamped("e", "v", 100, 90, 7);
        let carry = StampedCarry::from_stamped(&original);
        assert_eq!(carry.to_stamped().unwrap(), original);
    }
}
