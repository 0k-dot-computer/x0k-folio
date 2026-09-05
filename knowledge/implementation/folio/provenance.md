---
x0k:
  format: folio/v1
  id: x0k:implementation/folio/provenance
  type: implementation
  status: draft
  summary: "Per-block provenance as event sourcing: events appended and never deleted, folded to current state and then to a viewer-relative display, answering whether a human has taken responsibility for a block as it now stands."
  concerns: [folio, provenance, underwriting, trust, events]
  tangle:
    crate: x0k-folio
    root: src/block_provenance.rs
  edges:
    implements:
      - x0k:design/prose-provenance-and-underwriting
    cites:
      - x0k:implementation/folio/segmentation
    presupposes:
      - x0k:wiki/event-sourcing
---
# Provenance: an append-only log and a fold

A document in a human-agent workshop accumulates prose from many hands —
the operator (the human who runs the system and answers for what it
produces), several agents, an accepted AI proposal here, a mechanical
port there. The question a reader actually asks is not "who typed this?"
but "**has a human taken responsibility for this, as it currently
stands?**" This module is the data model for answering that question:
per-block provenance **events** appended to a log, a **fold** from log to
current state, and a **viewer-relative** display state.

The pieces come from `x0k:design/prose-provenance-and-underwriting`, and
the shape is event-sourcing at its most literal. Events are never
deleted: split or merged blocks leave their old ids in the log, a
rejected-then-re-accepted block keeps its full history, and current state
is always a fold over everything. That gives the one property a trust
record cannot do without — you can always reconstruct *why* the state is
what it is.

Everything here is pure data + pure functions. The log's physical home is
a per-document JSONL sidecar owned by the daemon (one event per line);
decisions stay filesystem-canonical and diff-clean, and the web client
reads and writes provenance only through MCP tools. This module never
sees a file.

The carried scenario: the operator *underwrites* — reads, and takes
responsibility for, as it currently stands — the lead paragraph of the
`x0k-folio` publication manifest —
[`segmentation.md`](segmentation.md) minted its address `_root/p/0` and
its content hash. That gesture appends one `Underwritten` event locking
the acceptance to that exact hash. Later an agent revises the paragraph
(an `Edited` event with a `GenerationContext`), and every question the UI
now needs to answer — "is the operator's acceptance still good?" — is a
fold plus a hash comparison.

```rust {#module-doc}
//! Per-block provenance + underwriting model.
//!
//! Implements the trust loop from the prose-provenance design (an internal
//! design decision, `prose-provenance-and-underwriting`): every block of a
//! document accumulates an append-only log of events, and **underwriting** —
//! "I have read this and accept it as currently stated" — locks an acceptance
//! to the block's exact [content hash](crate::block_segment::hash_block). When
//! the block is later edited the hash changes and the acceptance goes *stale*
//! rather than silently migrating forward.
//!
//! ## Substrate
//!
//! This is a pure data model: event types, a fold to current state, and the
//! viewer-relative display state. The append-only log itself is a per-doc
//! JSONL sidecar owned by the daemon (one [`ProvenanceEvent`] per line) —
//! decisions stay filesystem-canonical and diff-clean, and the web client
//! reads/writes provenance only through MCP tools (it has no Loro runtime).
//!
//! Events are never deleted: split/merged blocks leave their old ids in the
//! log, and rejected-then-re-accepted blocks keep their history. Current
//! state is always a fold over the full log.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
```

## The event vocabulary

Four events, and the boundary between them is the design's boundary.
`Edited` and `Underwritten` are the block-level trust loop.
`ProposalDisposition` records the operator's durable judgment over one
proposed replacement — judgment history, deliberately separate from
content history. `AgentAttestation` is typed evidence about an
*artifact* generation, carrying its own subject identity rather than a
block id.

```rust {#provenance-event}
/// One append-only event in a document's provenance log. The doc identity is
/// implicit in the log file; block events name a `block_id`, while artifact
/// attestations carry their own typed subject identity.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ProvenanceEvent {
    /// A block was created or edited to a new content hash.
    Edited {
        block_id: String,
        /// blake3 content hash after this edit.
        content_hash: String,
        /// DID of the actor that produced this content (human or agent).
        actor: String,
        /// ISO-8601 UTC timestamp.
        at: String,
        /// Set when an agent produced the content (an accepted AI proposal).
        #[serde(default, skip_serializing_if = "Option::is_none")]
        generation: Option<GenerationContext>,
    },
    /// An actor underwrote a block at a specific content hash.
    Underwritten {
        block_id: String,
        actor: String,
        /// The content hash the actor accepted. The acceptance is valid only
        /// while the block still hashes to this value.
        accepted_hash: String,
        at: String,
    },
    /// The operator's durable judgment over one proposed block replacement.
    ///
    /// `principal` is deliberately a plain string at the doc-author floor.
    /// A later spine-fact projection maps it to the actor DID without changing
    /// this record's anchor/instruction/text/disposition shape.
    ProposalDisposition {
        anchor: String,
        instruction: String,
        proposed_text: String,
        disposition: ProposalDisposition,
        principal: String,
        timestamp: String,
    },
    /// Typed evidence attached by an agent or scripted authoring principal.
    AgentAttestation {
        record: AgentAttestationRecord,
    },
}
```

The attestation vocabulary is a **closed enum**, and that is the design
speaking, not convenience: an attestation is a claim with agreed
semantics ("checked against source" means something specific), so
extending the vocabulary is a design change, never a free-text
annotation. Free text would rot into unqueryable prose within a month.

```rust {#agent-attestation}
/// The closed agent-attestation vocabulary accepted by the prose-provenance
/// design. Extending this enum is a design change, not a free-text annotation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum AgentAttestation {
    Drafted,
    Revised,
    CheckedAgainstSource,
    PortedMechanically,
}

impl AgentAttestation {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Drafted => "drafted",
            Self::Revised => "revised",
            Self::CheckedAgainstSource => "checked-against-source",
            Self::PortedMechanically => "ported-mechanically",
        }
    }
}

/// Stable identity of the document occurrence and artifact an attestation
/// concerns. `identity` is the host's equality key for that occurrence.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AttestationTarget {
    pub document_uri: String,
    pub artifact_uri: String,
    pub occurrence: usize,
    pub identity: String,
}

/// One attributable statement about an artifact generation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AgentAttestationRecord {
    pub attestation: AgentAttestation,
    pub target: AttestationTarget,
    pub actor: String,
    pub artifact_generation: u64,
    pub at: String,
}

/// The two terminal outcomes of a proposal. Pending proposals are not events.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProposalDisposition {
    Accepted,
    Rejected,
}
```

"Pending proposals are not events" is worth its comment: the log records
*judgments*, and an unjudged proposal is UI state, not history. If the
operator never acts, nothing enters the permanent record.

The accessor below is what routes an event to its block during the fold;
artifact attestations return `None` because their subject is not a block:

```rust {#event-block-id}
impl ProvenanceEvent {
    pub fn block_id(&self) -> Option<&str> {
        match self {
            ProvenanceEvent::Edited { block_id, .. }
            | ProvenanceEvent::Underwritten { block_id, .. } => Some(block_id),
            ProvenanceEvent::ProposalDisposition { anchor, .. } => Some(anchor),
            ProvenanceEvent::AgentAttestation { .. } => None,
        }
    }
}

/// Context recorded when an agent generated a block (an accepted proposal).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GenerationContext {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub prompt_hash: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub session_id: Option<String>,
    /// Id of the block this content replaced (for diff lineage).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub parent_block: Option<String>,
}
```

## Folded state

The fold's output shape: per block, the creation and last-edit
attribution, the most recent *recorded* hash (from the log — the live
file may already have moved on), whether the latest edit was
agent-generated, and the accumulated acceptances.

```rust {#folded-state}
/// A single acceptance, folded out of the log.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Underwriting {
    pub actor: String,
    pub accepted_hash: String,
    pub accepted_at: String,
}

/// Current provenance state for one block, folded from the event log.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BlockProvenance {
    pub block_id: String,
    pub created_by: Option<String>,
    pub created_at: Option<String>,
    pub last_edited_by: Option<String>,
    pub last_edited_at: Option<String>,
    /// Most recent recorded content hash (from the log, not the live file).
    pub content_hash: Option<String>,
    /// Whether the latest edit was agent-generated.
    pub ai_generated: bool,
    pub underwritings: Vec<Underwriting>,
}

impl BlockProvenance {
    fn empty(block_id: &str) -> Self {
        BlockProvenance {
            block_id: block_id.to_string(),
            created_by: None,
            created_at: None,
            last_edited_by: None,
            last_edited_at: None,
            content_hash: None,
            ai_generated: false,
            underwritings: Vec::new(),
        }
    }
}
```

## The fold

One pass, in log order. `Edited` sets creation attribution on first
sight and overwrites last-edit attribution every time — note that
`ai_generated` tracks only the *latest* edit, so a human rewrite of an
AI paragraph clears the flag, which is exactly the trust semantics
wanted (the human now owns the words). `Underwritten` dedupes on
`(actor, hash)` with a re-submit refreshing the timestamp, so
mashing the accept button twice is not two acceptances.
`ProposalDisposition` deliberately contributes nothing: accepted prose
gets its own `Edited` event, and inferring content state from a judgment
record would double-count. `AgentAttestation` reaching the block fold is
a caller bug — the `unreachable!` documents the contract that artifact
events are filtered out before folding.

```rust {#fold-events}
/// Fold an event log into per-block current state, preserving block order of
/// first appearance.
pub fn fold_events(events: &[ProvenanceEvent]) -> BTreeMap<String, BlockProvenance> {
    let mut map: BTreeMap<String, BlockProvenance> = BTreeMap::new();
    for ev in events {
        let Some(block_id) = ev.block_id() else {
            continue;
        };
        let entry = map
            .entry(block_id.to_string())
            .or_insert_with(|| BlockProvenance::empty(block_id));
        match ev {
            ProvenanceEvent::Edited {
                content_hash,
                actor,
                at,
                generation,
                ..
            } => {
                if entry.created_by.is_none() {
                    entry.created_by = Some(actor.clone());
                    entry.created_at = Some(at.clone());
                }
                entry.last_edited_by = Some(actor.clone());
                entry.last_edited_at = Some(at.clone());
                entry.content_hash = Some(content_hash.clone());
                entry.ai_generated = generation.is_some();
            }
            ProvenanceEvent::Underwritten {
                actor,
                accepted_hash,
                at,
                ..
            } => {
                // Dedupe on (actor, accepted_hash); a re-submit refreshes the
                // timestamp to the latest.
                if let Some(existing) = entry
                    .underwritings
                    .iter_mut()
                    .find(|u| u.actor == *actor && u.accepted_hash == *accepted_hash)
                {
                    existing.accepted_at = at.clone();
                } else {
                    entry.underwritings.push(Underwriting {
                        actor: actor.clone(),
                        accepted_hash: accepted_hash.clone(),
                        accepted_at: at.clone(),
                    });
                }
            }
            ProvenanceEvent::ProposalDisposition { .. } => {
                // Dispositions are judgment history. Accepted prose receives
                // a separate Edited event, so this fold never infers content
                // state from a proposal outcome.
            }
            ProvenanceEvent::AgentAttestation { .. } => unreachable!(
                "artifact attestations are excluded before the block fold"
            ),
        }
    }
    map
}
```

## What the viewer sees

Display state is relative to *who is looking*: the same block is fresh
to the actor who underwrote it and stale to one whose acceptance
predates the last edit. The state is computed by joining the folded
provenance against the block's **live** hash (from the file, via
[`segmentation.md`](segmentation.md)) and the viewer's DID.

```rust {#viewer-state-enum}
/// Viewer-relative display state for a block, per the design's §"Display
/// states". Computed by joining folded provenance against the block's *live*
/// content hash and the viewer's DID.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ViewerState {
    /// Human-authored, no AI, no underwriting needed — no marker.
    Clean,
    /// AI-touched, nobody has underwritten — needs a human to take responsibility.
    AiUntouched,
    /// Underwritten by others at the current hash, but not by the viewer.
    UnderwrittenByOthers,
    /// The viewer's acceptance is current.
    UnderwrittenByViewerFresh,
    /// The viewer accepted an earlier version; the block has since changed.
    StaleForViewer,
    /// Acceptances exist but none match the current hash (stale for everyone).
    StaleBroadly,
}
```

The classification is a strict priority ladder — the viewer's own fresh
acceptance beats everything, their stale acceptance beats other people's
state, and only when the viewer has no acceptance at all do we look at
others and finally at the AI flag. Reading it as a chain of early
returns keeps the precedence visible:

```rust {#viewer-state-fn}
/// Compute the viewer-relative state for a block given its folded provenance,
/// the block's current (live) content hash, and the viewer's DID.
pub fn viewer_state(prov: &BlockProvenance, current_hash: &str, viewer_did: &str) -> ViewerState {
    let viewer_current = prov
        .underwritings
        .iter()
        .any(|u| u.actor == viewer_did && u.accepted_hash == current_hash);
    if viewer_current {
        return ViewerState::UnderwrittenByViewerFresh;
    }

    let viewer_stale = prov
        .underwritings
        .iter()
        .any(|u| u.actor == viewer_did && u.accepted_hash != current_hash);
    if viewer_stale {
        return ViewerState::StaleForViewer;
    }

    let others_current = prov
        .underwritings
        .iter()
        .any(|u| u.accepted_hash == current_hash);
    if others_current {
        return ViewerState::UnderwrittenByOthers;
    }

    // No acceptance matches the current hash.
    if !prov.underwritings.is_empty() {
        // There were acceptances, all now stale.
        return ViewerState::StaleBroadly;
    }

    if prov.ai_generated {
        ViewerState::AiUntouched
    } else {
        ViewerState::Clean
    }
}
```

## Tests

The tests build tiny logs with helper constructors and pin the fold's
attribution rules, the dedupe, and — in one matrix test — every arm of
the viewer ladder. The JSONL round-trip test pins the wire shape the
daemon's sidecar depends on (`"kind":"underwritten"` tagging).

```rust {#tests}
#[cfg(test)]
mod tests {
    use super::*;

    fn edited(block: &str, hash: &str, actor: &str) -> ProvenanceEvent {
        ProvenanceEvent::Edited {
            block_id: block.into(),
            content_hash: hash.into(),
            actor: actor.into(),
            at: "2026-06-01T00:00:00Z".into(),
            generation: None,
        }
    }
    fn ai_edited(block: &str, hash: &str, actor: &str) -> ProvenanceEvent {
        ProvenanceEvent::Edited {
            block_id: block.into(),
            content_hash: hash.into(),
            actor: actor.into(),
            at: "2026-06-01T00:00:00Z".into(),
            generation: Some(GenerationContext {
                model: Some("claude-opus".into()),
                prompt_hash: None,
                session_id: None,
                parent_block: None,
            }),
        }
    }
    fn under(block: &str, actor: &str, hash: &str) -> ProvenanceEvent {
        ProvenanceEvent::Underwritten {
            block_id: block.into(),
            actor: actor.into(),
            accepted_hash: hash.into(),
            at: "2026-06-01T01:00:00Z".into(),
        }
    }

    #[test]
    fn fold_tracks_created_and_last_edited() {
        let log = vec![
            edited("b1", "h1", "did:alice"),
            edited("b1", "h2", "did:bob"),
        ];
        let folded = fold_events(&log);
        let p = &folded["b1"];
        assert_eq!(p.created_by.as_deref(), Some("did:alice"));
        assert_eq!(p.last_edited_by.as_deref(), Some("did:bob"));
        assert_eq!(p.content_hash.as_deref(), Some("h2"));
    }

    #[test]
    fn underwriting_dedupes_by_actor_and_hash() {
        let log = vec![
            edited("b1", "h1", "did:alice"),
            under("b1", "did:alice", "h1"),
            under("b1", "did:alice", "h1"),
        ];
        let folded = fold_events(&log);
        assert_eq!(folded["b1"].underwritings.len(), 1);
    }

    #[test]
    fn viewer_states_cover_the_matrix() {
        // Fresh acceptance by the viewer.
        let log = vec![
            edited("b", "h1", "did:alice"),
            under("b", "did:alice", "h1"),
        ];
        let f = fold_events(&log);
        assert_eq!(
            viewer_state(&f["b"], "h1", "did:alice"),
            ViewerState::UnderwrittenByViewerFresh
        );

        // Viewer accepted h1, block now at h2 → stale for viewer.
        assert_eq!(
            viewer_state(&f["b"], "h2", "did:alice"),
            ViewerState::StaleForViewer
        );

        // Bob accepted current, alice (viewer) has not.
        let log2 = vec![edited("b", "h1", "did:x"), under("b", "did:bob", "h1")];
        let f2 = fold_events(&log2);
        assert_eq!(
            viewer_state(&f2["b"], "h1", "did:alice"),
            ViewerState::UnderwrittenByOthers
        );

        // Acceptance exists but none match current → stale broadly.
        assert_eq!(
            viewer_state(&f2["b"], "h9", "did:alice"),
            ViewerState::StaleBroadly
        );

        // AI-generated, never underwritten.
        let log3 = vec![ai_edited("b", "h1", "did:agent")];
        let f3 = fold_events(&log3);
        assert_eq!(
            viewer_state(&f3["b"], "h1", "did:alice"),
            ViewerState::AiUntouched
        );

        // Human-authored, never underwritten → clean.
        let log4 = vec![edited("b", "h1", "did:alice")];
        let f4 = fold_events(&log4);
        assert_eq!(viewer_state(&f4["b"], "h1", "did:bob"), ViewerState::Clean);
    }

    #[test]
    fn events_round_trip_as_jsonl() {
        let ev = under("brief/p/0", "did:alice", "abc123");
        let line = serde_json::to_string(&ev).unwrap();
        let back: ProvenanceEvent = serde_json::from_str(&line).unwrap();
        assert_eq!(ev, back);
        assert!(line.contains("\"kind\":\"underwritten\""));
    }
}
```

## Composing the module

```rust {#root}
<<module-doc>>

<<provenance-event>>

<<agent-attestation>>

<<event-block-id>>

<<folded-state>>

<<fold-events>>

<<viewer-state-enum>>

<<viewer-state-fn>>

<<tests>>
```

The honest gap: nothing in this module verifies that an `Edited` event
was actually appended when the file changed. The model assumes a
disciplined writer (the daemon's save path); a file edited behind the
daemon's back simply has a live hash no log event mentions, which the
viewer ladder reports as stale — safe, but indistinguishable from a real
stale acceptance. Distinguishing "edited outside the loop" from "edited
inside it" would need file-watch integration the sidecar deliberately
does not attempt.
