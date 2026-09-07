---
x0k:
  format: folio/v1
  id: x0k:wiki/event-graph-crdts
  type: wiki
  subtype: wiki:Article
  status: stable
  summary: A CRDT design that persists only the append-only DAG of original edit operations and rebuilds a transient CRDT on demand to merge concurrent branches — yielding OT-class steady-state cost with CRDT-class correctness. This is the lineage behind Diamond Types and Loro.
  updated_by: mcp-agent
  created_at: 2026-06-04T23:30:37.710865Z
  updated_at: 2026-06-08T16:52:02.910353Z
  concerns:
    - concept
    - crdt
    - collaborative-editing
    - algorithms
    - local-first
    - sync
    - data-structures
    - sync-engine
  edges:
    cites:
      - x0k:wiki/loro
      - x0k:wiki/automerge
      - x0k:wiki/accretion-and-bitemporal-data
---
# Event-Graph CRDTs (Eg-walker / Replayable Event Graph)

The **event-graph** approach (formalized as **Eg-walker**, Event Graph Walker, by Joseph Gentle & Martin Kleppmann, EuroSys 2025) inverts the usual CRDT bargain. Classic list CRDTs (RGA, YATA, Fugue) bake conflict-resolution metadata — per-character IDs, tombstones, origin pointers — into the *persistent* document representation. Eg-walker instead persists only the **append-only DAG of original edit operations** (the event graph) and treats the CRDT as a transient computation.

## The core idea: store the event graph, not the expanded state
The history is "a directed acyclic graph (DAG) in which every node is an event consisting of an operation (insert/delete a character), a unique ID, and the set of IDs of its parent events" (Eg-walker paper). Each op records its **causal parents** and a *simple integer index* — the cursor position at the moment of editing — rather than a stable position descriptor. [[loro]]'s docs put it sharply: Eg-walker "can record just the original description of operations, not the metadata of CRDTs … only recording the index at the time of insertion."

The set of childless events is the **frontier** — the current version. Diamond Types uses the frontier instead of a growing version vector because "it doesn't grow over time." Locally, each op also gets an **order number** (sequential local-observation index), but "they are not shared between peers — peers may see the same operations in different orders."

## Replay / merge
The expanded CRDT state is never stored. To integrate a remote change, you replay only the operations **between the current version and the remote version, back to their lowest common ancestor (LCA)**, building a *temporary* internal CRDT to compute where each concurrent op actually lands, then discard it. The paper's transient structure tracks, per record, a *prepare* state and an *effect* state, walking the DAG in topological order via `apply`/`retreat`/`advance` to move the cursor frame between the version an op was authored against and the version being materialized. "We discard its state as soon as the merge is complete. We never write the CRDT state to disk and never send it over the network."

Complexity: merging *n* concurrent operations is **O(n log n)** (B-trees give log-n index↔character mapping), versus operational transformation's **O(n²)** pairwise transforms. The headline result: a heavily-diverged trace that takes **~1 hour to merge under OT merges in 24 ms** under Eg-walker. Critically, in the **common non-concurrent case** the cost is near zero — sequential edits need no replay, the on-disk event graph "can remain on disk without using any space in memory or any CPU time," and internal state is built only when handling concurrency.

## Relationship to OT and to list CRDTs
Eg-walker is a deliberate synthesis. From OT it borrows the idea of recording *intent as a simple index* and *transforming on demand*; unlike OT it tolerates arbitrary offline divergence without quadratic blowup. From CRDTs it borrows commutative, peer-order-independent convergence; unlike classic CRDTs it does not pay for that metadata in steady state. Reported wins: **1–2 orders of magnitude less memory** than the best CRDTs at rest, **orders-of-magnitude faster document load** (only plain text loads; metadata stays on disk), and OT-beating merges (~160,000× on the worst async trace). Hence "better, faster, smaller."

## Lineage / History
This page sits at the *end* of a long arc; see [local-first-and-crdt-lineage](x0k:manuscript/genealogy-of-the-humane-computer/local-first-and-crdt-lineage) for the full history. In brief: collaborative-text merge began with **Operational Transformation** (Ellis & Gibbs, GROVE, 1989), which recorded edits as simple positional ops and *transformed* incoming ops against applied ones — cheap in steady state but coordination-dependent (it needed a central order, à la Jupiter 1995, to be shippable) and notoriously fragile (the **dOPT puzzle** and the difficulty of provably-correct TP2 transforms). **CRDTs** (Shapiro et al., 2011; sequence prequels **WOOT** 2006, **Treedoc**/**Logoot** 2009, **RGA** 2011, **YATA**/Yjs 2016) removed the server by making concurrent ops *commute by construction* — coordination-free convergence, paid for with persistent metadata (IDs, tombstones). Eg-walker **closes the loop**: it re-imports OT's "store the intent as an index, compute on demand" instinct on top of a coordination-free op-DAG, recovering OT's near-zero steady-state cost *and* CRDT correctness. The direct origin is Joseph Gentle's **Diamond Types** ("the world's fastest CRDT") whose B-tree + run-length/columnar encoding produced a claimed **~5000× speedup vs Automerge** (291 s / 880 MB → 0.056 s / 1.1 MB on one trace) before being generalized into Eg-walker (Gentle & Kleppmann, EuroSys 2025). [[automerge]] remains on the older RGA-style persistent-metadata side of this divide; Loro adapted Eg-walker for production with rich text, lists, maps, and movable trees.

## Bearing on our sync engine
We are building a standalone local-first sync engine in Rust on Loro, so the event-graph model is the substrate, not just an influence:
- **Wire = original ops.** We ship the *original operations* (op + causal parents + index), not expanded CRDT state. The receiver replays them against its own graph. This is what lets sync payloads scope cleanly to Willow entries — each entry carries the ops that fall under its path/subspace, and the receiver merges by replay rather than by reconciling fat CRDT documents.
- **Storage stays compact.** Persist the append-only DAG (run-length/columnar encoded), not tombstone-laden materialized state. (Inferred for our engine: this is exactly the failure mode behind the 626 MB append-only Loro doc OOM in x0k's memory — the lesson is to keep the op-log itself bounded/GC-able, since the event graph is append-only by design.)
- **Frontiers as version identity.** Use frontiers (childless-event sets) as the version token on the wire and for "what does the peer already have" diffing, rather than monotonically-growing vectors.
- **Replay cost is concurrency-proportional.** Steady-state sync between mostly-in-sync peers is near-free; expensive replay happens only on genuinely divergent (long-offline) branches — budget and test for *that* case specifically.

The event-graph model is the **multi-writer** sibling of the **single-writer accretion** tradition ([[accretion-and-bitemporal-data]] — Datomic, XTDB, "the log is the database"): both make history primary and current state a fold over it, but accretion has one transactor and a total order while the op-DAG is intrinsically concurrent. The unexplored middle is the *hybrid* — per-node accretion (a plain ordered log) unioned across nodes by exactly this op-DAG / set-reconciliation machinery; see that page's *hybrid* section.

## Sources
- https://arxiv.org/abs/2409.14252 (Gentle & Kleppmann, Eg-walker, EuroSys 2025)
- https://www.loro.dev/docs/concepts/event_graph_walker
- https://josephg.com/blog/crdts-go-brrr/
- https://github.com/josephg/diamond-types/blob/master/INTERNALS.md
- https://github.com/josephg/eg-walker-reference
