//! Typed fact provenance — which producer wrote a fact, from which
//! journal position.
//!
//! `cell-substrate`'s provenance-outbound amendment (2026-08-17) decides
//! that every fact a substrate-connected journaled cell writes carries
//! typed provenance: the producer's identity and the journal seq that
//! emitted it, replacing the free-form cause strings each writer had
//! invented for itself — a bare head reference, a verb, a retraction
//! marker, each meaning something only to the writer that minted it.
//! [`FactProvenance`] is that type, and this module is its one spelling.
//!
//! # Provenance is attribution, never an arbiter
//!
//! Provenance rides the existing `cause` slot and changes nothing about
//! how facts compete: coordinate identity stays `(entity, predicate,
//! value)` ([`canonical_value_bytes`](crate::payload::canonical_value_bytes)
//! ignores `cause`), and the fold's dominance rules are untouched. What
//! it buys is the join back: a fact can be traced to the journal
//! position that produced it, which is what lets a rewind enumerate the
//! emissions of a discarded suffix and a reader ask "why does this fact
//! hold?" against the producer's own record.
//!
//! # Serialization
//!
//! The typed form renders **into** the `cause` string —
//! `journal:<producer>@<seq>` — following the crate's existing
//! prefix-convention causes ([`file_content_cause`](crate::payload::file_content_cause),
//! [`file_deleted_cause`](crate::payload::file_deleted_cause)). The wire
//! payload ([`FactPayload`](crate::payload::FactPayload)) is unchanged:
//! `cause` stays `Option<String>`, so readers that treat causes as
//! opaque strings keep working, and readers that want the structure call
//! [`FactProvenance::parse`]. The producer may itself contain `:` or `@`
//! (URIs do); parsing splits on the **last** `@` and requires a numeric
//! seq after it, so rendering then parsing always round-trips.

use crate::FactEntry;

/// Prefix of a typed journal-provenance cause string (see
/// [`FactProvenance::to_cause`]).
pub const JOURNAL_CAUSE_PREFIX: &str = "journal:";

/// Typed provenance of one fact: the producer that wrote it and the
/// journal position that emitted it.
///
/// `producer` is the writing cell's identity — a session or cell URI
/// (e.g. `x0k:cell/<name>`), the same spelling the producer's other
/// facts hang off. `journal_seq` is the seq of the producer's journal
/// entry at emission: for a fact emitted by a journaled transition, the
/// entry that emitted it; for a host-side writer that bypasses the
/// journal (presence), the current head's seq — either way, the position
/// the fact is traceable to.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FactProvenance {
    /// The producing session/cell URI.
    pub producer: String,
    /// The seq of the producer's journal entry that emitted the fact.
    pub journal_seq: u64,
}

impl FactProvenance {
    /// Provenance for a fact `producer` emitted at journal position
    /// `journal_seq`.
    pub fn new(producer: impl Into<String>, journal_seq: u64) -> Self {
        Self {
            producer: producer.into(),
            journal_seq,
        }
    }

    /// Render into the `cause` slot: `journal:<producer>@<seq>`.
    pub fn to_cause(&self) -> String {
        format!(
            "{JOURNAL_CAUSE_PREFIX}{}@{}",
            self.producer, self.journal_seq
        )
    }

    /// Parse a cause string rendered by [`Self::to_cause`]. Returns
    /// `None` for anything else — legacy free-form causes and the
    /// file-projection causes are not journal provenance and stay
    /// opaque.
    pub fn parse(cause: &str) -> Option<Self> {
        let rest = cause.strip_prefix(JOURNAL_CAUSE_PREFIX)?;
        let (producer, seq) = rest.rsplit_once('@')?;
        if producer.is_empty() {
            return None;
        }
        Some(Self {
            producer: producer.to_string(),
            journal_seq: seq.parse().ok()?,
        })
    }
}

impl FactEntry {
    /// This fact's typed provenance, when its cause carries one.
    pub fn provenance(&self) -> Option<FactProvenance> {
        FactProvenance::parse(self.cause.as_deref()?)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::FactValue;

    #[test]
    fn journal_cause_roundtrips() {
        let prov = FactProvenance::new("x0k:cell/indexer", 42);
        let cause = prov.to_cause();
        assert_eq!(cause, "journal:x0k:cell/indexer@42");
        assert_eq!(FactProvenance::parse(&cause), Some(prov));
    }

    #[test]
    fn producer_may_contain_the_separator() {
        // rsplit on the LAST '@' keeps a producer with '@' in it intact.
        let prov = FactProvenance::new("x0k:cell/inbox@node-a", 7);
        assert_eq!(FactProvenance::parse(&prov.to_cause()), Some(prov));
    }

    #[test]
    fn legacy_and_malformed_causes_parse_as_none() {
        for cause in [
            "journal-head:abc123",          // a bare head reference
            "entity-supersede",             // a writer's own verb
            "query-contract-retraction",    // a retraction marker
            "file-content:deadbeef",        // file-projection provenance
            "journal:no-seq",               // missing position
            "journal:producer@not-a-seq",   // non-numeric position
            "journal:@42",                  // missing producer
        ] {
            assert_eq!(FactProvenance::parse(cause), None, "cause: {cause}");
        }
    }

    #[test]
    fn fact_joins_producing_journal_position() {
        // A minimal journal record: seq-addressed entries, the shape any
        // journaled producer exposes. The crate is substrate-neutral, so the
        // journal here is the join target's essence — positions keyed by seq.
        struct JournalEntry {
            seq: u64,
            kind: &'static str,
        }
        let journal = [
            JournalEntry { seq: 0, kind: "user" },
            JournalEntry { seq: 1, kind: "turn" },
            JournalEntry { seq: 2, kind: "turn" },
        ];

        // A fact written with provenance citing the producer and the head
        // position standing at emission.
        let producer = "x0k:cell/indexer";
        let head = journal.last().unwrap();
        let mut fact = FactEntry::new(
            producer,
            "x0k:session/lifecycle",
            FactValue::Text("running".into()),
        );
        fact.cause = Some(FactProvenance::new(producer, head.seq).to_cause());

        // The join: nothing but the fact's own cause resolves the producing
        // journal position.
        let prov = fact.provenance().expect("fact carries typed provenance");
        assert_eq!(prov.producer, producer);
        let produced_at = journal
            .iter()
            .find(|entry| entry.seq == prov.journal_seq)
            .expect("provenance joins back to a journal position");
        assert_eq!(produced_at.seq, 2, "the position is the stamped head");
        assert_eq!(produced_at.kind, "turn");
    }

    #[test]
    fn fact_entry_exposes_its_provenance() {
        let mut fact = FactEntry::new("x0k:seed/a", "p/title", FactValue::Text("A".into()));
        assert_eq!(fact.provenance(), None);
        fact.cause = Some(FactProvenance::new("x0k:cell/indexer", 3).to_cause());
        assert_eq!(
            fact.provenance(),
            Some(FactProvenance::new("x0k:cell/indexer", 3))
        );
        // A legacy cause is not provenance.
        fact.cause = Some("journal-head:abc".into());
        assert_eq!(fact.provenance(), None);
    }
}
