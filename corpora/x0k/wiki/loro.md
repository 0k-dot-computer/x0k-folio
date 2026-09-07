---
x0k:
  format: folio/v1
  id: x0k:wiki/loro
  type: wiki
  subtype: wiki:Entity
  status: stable
  summary: Loro is an MIT-licensed, Rust-built CRDT library (with JS/WASM, Swift, Python bindings) for local-first and collaborative apps, combining an Eg-walker-style replayable event graph with Fugue text, a movable tree, and a movable list, plus Git-like versioning and shallow-snapshot history GC.
  updated_by: mcp-agent
  created_at: 2026-06-04T23:27:00.694421Z
  updated_at: 2026-06-08T16:52:37.585932Z
  concerns:
    - crdt
    - local-first
    - rust
    - wasm
    - collaborative-editing
    - sync
    - event-graph
    - sync-engine
  edges:
    cites:
      - x0k:wiki/automerge
      - x0k:wiki/event-graph-crdts
      - x0k:wiki/willow-protocol
      - x0k:wiki/ucan
      - x0k:wiki/crdt-formal-verification
---
# Loro

Loro is a CRDT library that "makes building local-first and collaborative apps easier" by making JSON data collaborative and version-controlled with P2P sync, automatic merging, and local availability ([github.com/loro-dev/loro](https://github.com/loro-dev/loro)). It is MIT-licensed, written in Rust with JavaScript/WASM, Swift, and Python (`loro-ffi`) bindings, and reached 1.0 in 2026 (current crate releases in the 1.12.x range as of mid-2026). It is positioned as the fastest of the mainstream CRDT libraries in benchmarks, though the youngest in ecosystem maturity relative to [[automerge]] and Yjs. **Loro is x0k's chosen CRDT substrate.**

## Core model: operation-based + replayable event graph
Loro is operation-based and is "heavily inspired by Eg-walker's design philosophy" rather than a strict implementation ([event_graph_walker](https://loro.dev/docs/advanced/event_graph_walker)). The [[event-graph-crdts|Event Graph Walker]] (Eg-walker, from Gentle & Kleppmann's "Collaborative Text Editing with Eg-walker: Better, Faster, Smaller", [arXiv:2409.14252](https://arxiv.org/abs/2409.14252)) stores "an append-only, immutable list of original operations" as a DAG of edit history, rather than transformed ops (OT) or intermediate ordered items (classic CRDTs). Each op records simple indices at execution time. On merge, it "only needs to replay the operations between the current version and the remote version up to their lowest common ancestor," building a temporary CRDT to compute the remote effect. Concurrent merges of n ops each cost O(n log n) vs OT's O(n²). Loro adapts this from Diamond-types "to reduce computation and space usage" ([README](https://github.com/loro-dev/loro)).

## Algorithms per container
Loro ships typed containers, each with an algorithm tuned to its semantics: Text uses **Fugue**, "the first algorithm that guarantees maximal non-interleaving" ([arXiv:2305.00583](https://arxiv.org/abs/2305.00583)); Rich Text complies with Peritext's criteria (the `crdt-richtext` crate implementing Peritext + Fugue); the **Movable List** reuses the Fugue tracker, treating each element as a "character"; the **Movable Tree** implements Kleppmann et al.'s "A highly-available move operation for replicated trees" and uses a **fractional index** to order siblings; plus an LWW Map and a Counter.

## Lineage / History
Loro is the *current frontier* of a 35-year arc; see [local-first-and-crdt-lineage](x0k:manuscript/genealogy-of-the-humane-computer/local-first-and-crdt-lineage) for the whole story. Its two load-bearing algorithms each sit at the end of their own sub-lineage. **Text/list — Fugue** (Weidner & Kleppmann, 2023) is the latest in the collaborative-text CRDT line that began with **WOOT** (2006, the first text CRDT), passed through **Logoot**/**Treedoc** (2009), **RGA** (2011), and **YATA**/Yjs (2016); Fugue's contribution is *maximal non-interleaving* — eliminating the anomaly where concurrent runs of typed text get shuffled. **Merge engine — Eg-walker** (Diamond Types → Gentle & Kleppmann, EuroSys 2025; see [[event-graph-crdts]]) is the synthesis that re-imports **Operational Transformation**'s "store the intent as an index, compute on demand" instinct (OT: Ellis & Gibbs, GROVE, 1989; Jupiter, 1995) on top of a coordination-free op-DAG — recovering OT's near-zero steady-state cost while keeping CRDT correctness. So Loro is best read as *the productization of the OT↔CRDT reconciliation*: where [[automerge]] productized the RGA-style persistent-metadata branch (and Yjs the YATA branch), Loro productized the event-graph branch. Same lineage, different bet on where to pay the cost.

## Versioning: OpLog vs DocState, Frontiers vs Version Vectors
Loro splits **OpLog** (the full causal DAG of operations) from **DocState** (current materialized state). Local edits update DocState and append to OpLog; remote merges append ops, compute a Delta against DocState, apply it, and emit it as an event. A version is expressible as a **Version Vector** or, more compactly, as **Frontiers** — a set of OpIds whose causal closure (everything ≤ them) defines the version. `Frontiers = [A]` is exactly "the document version right after operation A was executed" ([version_deep_dive](https://loro.dev/docs/advanced/version_deep_dive)). `doc.checkout(frontiers)` time-travels to any version (Git-like), putting the doc in a detached read-only state until `doc.attach()`.

## Sync & storage encoding
Peers sync via `export`/`import`. `export({mode:"update"})` "only encodes the operations that occurred after the specified version" (delta sync); `export({mode:"snapshot"})` "encodes both OpLog and DocState" for fast loading ([encoding](https://loro.dev/docs/tutorial/encoding)). **Shallow Snapshot** is Loro 1.0's history GC: it encodes recent ops + the starting state of the truncated history + the latest state, "truncat[ing] all the history before this version" — analogous to a Git shallow clone, with pre-cutoff history archivable to cold storage. Encoded output is binary and compact.

## Bearing on our sync engine
**For free:** a battle-tested Rust CRDT core with typed containers (text/richtext/list/movable-list/map/tree/counter), so our engine need not invent merge semantics; an append-only causal DAG that maps cleanly onto a [[willow-protocol|Willow]]-style entry log; Frontiers as a compact version cursor; native delta (`update`) and full (`snapshot`) encodings to plug under our wire boundary; and shallow snapshots for bounded history — important because x0k has already hit Loro append-only OOM when history was never GC'd.

**Sync-model implications:** Loro's `update`/`snapshot` framing is *its own* encoding, not Willow's range-based reconciliation — we must layer Willow set-reconciliation and [[ucan]] capability checks *around* Loro export/import, treating Loro updates as opaque payloads scoped to namespaces/paths. Frontiers/Version-Vectors give us per-peer causal cursors to drive that reconciliation. Eg-walker's "replay up to LCA" merge means import cost scales with concurrent divergence, not total history — favorable, but the temporary-CRDT reconstruction is the spot to watch for memory.

**Claimed formal properties (verified from docs):** strong eventual consistency across replicas; Fugue's **maximal non-interleaving** for lists/text; movable-tree convergence without cycles (Kleppmann). These are invariants our literate proofs can *assume at the Loro boundary* and need only prove are *preserved* by our transport/capability layer (see [[crdt-formal-verification]]).

**Cautions:** Eg-walker/REG is documented as inspiration, not a verbatim spec — Loro's exact merge has implementation-specific behavior we can't treat as a paper-equivalent black box. Append-only OpLog growth is real (x0k's 626MB Loro OOM incident); shallow snapshots are mandatory operational hygiene, not optional. Loro is not Automerge- or Willow-wire-compatible, so all interop is our adapter's responsibility. Movable-list/tree use fractional indexing, which can degrade under pathological concurrent reordering.

## Sources
- https://github.com/loro-dev/loro
- https://loro.dev/docs/advanced/event_graph_walker
- https://loro.dev/docs/concepts/oplog_docstate
- https://loro.dev/docs/advanced/version_deep_dive
- https://loro.dev/docs/tutorial/encoding
- https://loro.dev/docs/advanced/shallow_snapshot
- https://loro.dev/docs/tutorial/time_travel
- https://deepwiki.com/loro-dev/loro/6.1-crdt-algorithms
- https://arxiv.org/abs/2409.14252 (Eg-walker)
- https://arxiv.org/abs/2305.00583 (Fugue)
- https://github.com/loro-dev/crdt-richtext
