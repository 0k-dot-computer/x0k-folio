---
x0k:
  format: folio/v1
  id: x0k:implementation/folio/ingestion
  type: implementation
  status: draft
  summary: A standalone ingestion engine with host-supplied document projection, named backend ports, and per-source durable recovery.
  concerns:
  - folio
  - backends
  - fact-plane
  - entry-spine
  - fan-out
  - delivery
  tangle:
    crate: crates/x0k-folio-ingest
    root: src/backend.rs
  edges:
    implements:
    - x0k:design/publish-a-region-as-a-repository
    motivated_by:
    - x0k:architecture/folio-backends
    - x0k:intent/c5ccd003-77d6-4b0d-8824-649f6221c259
    cites:
    - x0k:implementation/ontology/concept-region
    - x0k:implementation/entry-spine/spine
    - x0k:architecture/state-representation
    - x0k:architecture/applications-are-tenants
---
# Document ingestion without a substrate

This package is standalone document tooling. It owns filesystem checkpoint
I/O and coordinates database adapters; requiring a cell or an x0k process
would prevent the standalone tool from starting. The host supplies source
discovery, parsing and projection, runtime configuration, and backend
instances. The package holds no registry, profile, spine or activity sink.

The folio ingester folds a document into facts once, then delivers them to
a named list of backends. It records each backend's applied revision and
the facts an interrupted write might have delivered. It persists that
record before calling a sink and keeps deletions pending until every known
backend acknowledges them. One backend's error leaves the others'
acknowledgements intact. Each sink owns one worker. The coordinator snapshots
worker handles under the registry lock, releases it, and submits all ready sinks
before waiting for a bounded shared completion grace.

The carried example is the ADR itself. When
`corpora/x0k/decisions/architecture/delivery/folio-backends.md` is saved, the fold
yields seven envelope facts on `x0k:architecture/folio-backends` — status,
type, body format, two concerns, two `motivated_by` edges — every one
caused by `file-content:<blake3 of the file>`. With two backends attached,
say the spine and an in-memory mirror, exactly those seven facts reach
each of them, in one `replace` call each, and the state file afterwards
says the document's hash is acknowledged by `x0k` and by `mirror`. Delete
the file and each backend receives one `retract` of those seven. The
number of documents in the corpus never appears in that account, which is
the cost law the last section pins.

## The port is three small traits

A backend is whatever implements `FactSink`; it may also listen
(`Notifier`) and answer (`QueryEngine`). Three traits rather than one
because an archival mirror should not have to stub a query language, and a
search index should not have to accept facts. The sink is the required
half, and it carries the one contract the core relies on: a batch replayed
into a sink that already applied it is a no-op — idempotency is the sink's,
not the core's:

<a name="chunk-fact-sink"></a><sub>[`src/backend.rs`](../../crates/x0k-folio-ingest/src/backend.rs) · `#fact-sink`</sub>

```rust {#fact-sink}
use std::collections::{BTreeMap, BTreeSet, HashSet};

use anyhow::Result;
use tracing::{info, warn};
use x0k_fact_projection::payload::canonical_value_bytes;
use x0k_fact_projection::{FactEntry, FactValue};

use crate::events::{PathChangeEvent, ScanState};

/// Where a document's facts land. A sink owns its own idempotency —
/// replaying an applied batch is a no-op — and says whether it keeps
/// superseded facts (`folio-backends` §2, §5).
pub trait FactSink: Send {
    /// Durable store identity, checked before reusing acknowledgements.
    /// None is the compatibility contract for sinks without replaceable storage.
    fn incarnation(&mut self) -> Result<Option<String>> { Ok(None) }

    /// Replace a source contribution. Shared-support sinks override this
    /// boundary; the compatibility default retains entity-region semantics.
    fn replace_source(&mut self, source_key: &str, batches: &FactBatches,
        possible_prior: &[FactEntry], cause: &str) -> Result<usize> {
        let _ = source_key;
        let mut count = 0;
        let mut failure = None;
        for (entity, facts) in batches {
            match self.replace(entity, facts, cause) {
                Ok(n) => count += n,
                Err(error) => failure = Some(error),
            }
        }
        let dropped = dropped_cells(possible_prior.to_vec(), batches);
        if !dropped.is_empty() {
            match self.retract(&dropped, cause) {
                Ok(n) => count += n,
                Err(error) => failure = Some(error),
            }
        }
        if let Some(error) = failure { return Err(error); }
        Ok(count)
    }

    /// Replace `entity`'s whole fact region with `facts`, every one caused
    /// by `cause`. Returns how many facts the sink applied.
    fn replace(&mut self, entity: &str, facts: &[FactEntry], cause: &str) -> Result<usize>;
    /// Retract `facts` as previously asserted, the removal caused by `cause`.
    fn retract(&mut self, facts: &[FactEntry], cause: &str) -> Result<usize>;
    /// Whether superseded and retracted facts stay readable in this sink.
    fn retains_history(&self) -> bool;
    /// The current-view facts this sink holds under `cause` — `None` when
    /// the sink keeps no readable view at all (a write-only mirror).
    fn facts_caused_by(&self, cause: &str) -> Result<Option<Vec<FactEntry>>> {
        let _ = cause;
        Ok(None)
    }
}
```

`facts_caused_by` is the one method the ADR's port did not name, and it is
there for the re-ingest diff. The core's durable state is a record of
hashes, not of facts, so when a document changes the core has no copy of
what the previous content projected; it asks a sink that holds a view. The
spine answers from its cause index. A sink that keeps no view says so with
the default and the core moves on to the next.

A notifier hears what the *watch* half of the core sees: a document
changed or vanished, or a whole-tree scan finished with a tree hash. Both
events already exist as the scanner's own types, so the trait names them
rather than re-coining them; every method defaults to silence so a
notifier implements only the events it has a world for:

<a name="chunk-notifier"></a><sub>[`src/backend.rs`](../../crates/x0k-folio-ingest/src/backend.rs) · `#notifier`</sub>

```rust {#notifier}
/// A world outside the fact plane that wants to hear the core's change
/// events — a timeline, a search index, a webhook.
pub trait Notifier: Send {
    /// One folio document was written or removed under a watched root.
    fn document_changed(&mut self, event: &PathChangeEvent) {
        let _ = event;
    }
    /// A full scan of the tree finished; `scan` carries its content hash.
    fn scan_completed(&mut self, scan: &ScanState) {
        let _ = scan;
    }
}
```

A query engine answers in a language it names, over what its own sink
holds. It is declared here so a backend can carry one and a face can ask
"what can I ask, and of whom"; no backend in the tree implements it yet —
the spine's view surface and a Datalog store are both future engines, and
the port does not guess at their result shapes beyond a row of named
values:

<a name="chunk-query-engine"></a><sub>[`src/backend.rs`](../../crates/x0k-folio-ingest/src/backend.rs) · `#query-engine`</sub>

```rust {#query-engine}
/// One answer row: variable name → bound value.
pub type QueryRow = BTreeMap<String, FactValue>;

/// Answers queries in the language the backend names (`datalog`,
/// `spine-view`, `search`) over the facts its own sink holds.
pub trait QueryEngine: Send {
    fn language(&self) -> &str;
    fn query(&self, text: &str) -> Result<Vec<QueryRow>>;
}
```

A backend is the named bundle of the three. The name is what the state
file acknowledges against and what every fan-out event carries, so two
backends in one list may not share one:

<a name="chunk-backend"></a><sub>[`src/backend.rs`](../../crates/x0k-folio-ingest/src/backend.rs) · `#backend`</sub>

```rust {#backend}
/// A named backend: a required sink and its optional listener and engine.
pub struct Backend {
    pub name: String,
    pub(crate) worker: crate::delivery::Worker,
    pub(crate) grace: std::time::Duration,
    sink: Box<dyn FactSink>,
    pub notifier: Option<Box<dyn Notifier>>,
    pub query: Option<Box<dyn QueryEngine>>,
}

impl Backend {
    pub fn new(name: impl Into<String>, sink: impl FactSink + 'static) -> Self {
        let worker = crate::delivery::Worker::new(Box::new(sink));
        Self { name: name.into(), sink: Box::new(worker.clone()), worker,
            grace: std::time::Duration::from_millis(250), notifier: None, query: None }
    }
    /// Access the worker-backed sink without replacing its delivery identity.
    pub fn sink(&self) -> &dyn FactSink { self.sink.as_ref() }
    pub fn sink_mut(&mut self) -> &mut dyn FactSink { self.sink.as_mut() }
    /// Overall completion grace after fan-out admission, capped at 30 seconds.
    /// Late success remains unacknowledged and is safely replayed.
    pub fn with_delivery_grace(mut self, grace: std::time::Duration) -> Self {
        self.grace = grace.min(std::time::Duration::from_secs(30));
        self
    }
    pub fn with_notifier(mut self, notifier: impl Notifier + 'static) -> Self {
        self.notifier = Some(Box::new(notifier));
        self
    }
    pub fn with_query(mut self, query: impl QueryEngine + 'static) -> Self {
        self.query = Some(Box::new(query));
        self
    }
}

/// The names of `backends`, in list order.
pub fn backend_names(backends: &[Backend]) -> BTreeSet<String> {
    backends.iter().map(|backend| backend.name.clone()).collect()
}
```

## Every sink receives every batch, computed once

The fold produces, for one document, a list of `(entity, facts)` batches:
the document's own region first, then one region per inline entity found
in its body. The fan-out takes that list as given — it never folds, and it
never asks a backend to. What it computes once, before any sink is
touched, is the **diff**: the cells the previous content projected that
the new content no longer asserts. A cell is `(entity, predicate,
canonical value bytes)`, the same identity the spine's coordinate scheme
uses, so the retraction names exactly the facts a reader would otherwise
still see:

<a name="chunk-fact-batches-and-cells"></a><sub>[`src/backend.rs`](../../crates/x0k-folio-ingest/src/backend.rs) · `#fact-batches-and-cells`</sub>

```rust {#fact-batches-and-cells}
/// One document's fold: the document's own region, then one region per
/// inline entity its body declares.
pub type FactBatches = Vec<(String, Vec<FactEntry>)>;

/// The fold-cell identity of a fact — `(entity, predicate, canonical
/// value bytes)` — shared with the spine coordinate scheme.
pub fn fact_cell_key(fact: &FactEntry) -> (String, String, Vec<u8>) {
    (fact.entity.clone(), fact.predicate.clone(), canonical_value_bytes(&fact.value))
}

/// The facts of `batches` the prior projection asserted and the new one
/// does not: what every sink must retract.
fn dropped_cells(prior: Vec<FactEntry>, batches: &FactBatches) -> Vec<FactEntry> {
    let kept: HashSet<(String, String, Vec<u8>)> =
        batches.iter().flat_map(|(_, facts)| facts.iter().map(fact_cell_key)).collect();
    prior.into_iter().filter(|fact| !kept.contains(&fact_cell_key(fact))).collect()
}
```

The ingester owns the previous projection for each backend in durable
state. If the mirror misses an edit removing an inline entity, the entity
remains in its pending record until retraction succeeds. A newer spine view
cannot erase this obligation. A deletion has no new facts.

The compatibility helper below accepts only a cause and cannot provide
restart recovery. The ingester uses the explicit-prior replacement with
its own persisted facts. Source ownership remains entity-wide; independent
support from multiple documents requires a separate contribution model.
A hash-only checkpoint whose failed backend already lost an older revision
cannot reconstruct that backend's vanished facts. Such pre-upgrade stores
need a clean rebuild. An enabled backend absent from a legacy acknowledgement
set is refused with that instruction, since a new empty backend cannot be
distinguished from an old backend whose previous revision was lost. A malformed or unreadable checkpoint fails
before writes rather than being treated as an empty journal.

Recovery work scales with the union of facts from unacknowledged attempts,
which can grow across repeated failed edits. Healthy delivery still scales
with the changed document's footprint. Checkpoints are stored per source:
a delivery writes only that source's pending contribution. A cold scan
writes bytes proportional to its contributions, with two durable writes
per source. Loading all checkpoints still reads the whole index, including
on each watcher event. Tombstones remain while the legacy snapshot exists;
neither read scaling nor tombstone compaction is solved here.

<a name="chunk-prior-projection"></a><sub>[`src/backend.rs`](../../crates/x0k-folio-ingest/src/backend.rs) · `#prior-projection`</sub>

```rust {#prior-projection}
/// What the document's previous content projected, from the first sink
/// that holds a view. Empty when no sink does, or the read fails — then
/// nothing is retracted, and the region barrier is the only pruning.
fn prior_projection(backends: &[Backend], cause: &str) -> Vec<FactEntry> {
    for backend in backends {
        match backend.sink.facts_caused_by(cause) {
            Ok(Some(facts)) => return facts,
            Ok(None) => continue,
            Err(error) => {
                warn!(backend = %backend.name, error = %error, "folio.backend.prior_read_failed");
                return Vec::new();
            }
        }
    }
    Vec::new()
}
```

What one fan-out did is reported back as counts and names, because the
caller has two uses for it: the state file wants to know which backends
acknowledged the document, and the cost law wants to know how much work
was handed out. `facts` and `sink_calls` sum over backends — the very
quantities the law bounds:

<a name="chunk-fan-out-record"></a><sub>[`src/backend.rs`](../../crates/x0k-folio-ingest/src/backend.rs) · `#fan-out-record`</sub>

```rust {#fan-out-record}
/// What one document's fan-out did, summed over the backends it reached.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct FanOut {
    /// Backends whose every call succeeded — including those already
    /// acknowledged before this fan-out and therefore skipped.
    pub acked: BTreeSet<String>,
    /// Facts handed to sinks, replace and retract calls together.
    pub facts: usize,
    /// `replace` and `retract` calls made.
    pub sink_calls: usize,
    /// The diff, computed once and retracted from every reached sink.
    pub retracted: Vec<FactEntry>,
}
```

Replacing a document is then a loop with no cross-backend state in it.
`already` names the backends that acknowledged this exact content on an
earlier pass — a backend added after the corpus existed, or one that was
down, is reconciled by being the only one *not* in that set. A backend
that fails any call is left unacknowledged and its siblings proceed; the
next reconcile offers it the document again. Every backend's outcome is a
structured event, so the timeline can answer "what did the mirror receive
last night" without a code read:

<a name="chunk-replace-document"></a><sub>[`src/backend.rs`](../../crates/x0k-folio-ingest/src/backend.rs) · `#replace-document` · assembles [deliver-one-backend](#chunk-deliver-one-backend)</sub>

```rust {#replace-document}
/// Hand one document's fold to every backend not in `already`, retracting
/// what its previous content (under `prior_cause`) projected and the new
/// content does not. Sinks are independent: one failing leaves the
/// others' acknowledgement intact.
pub fn replace_document(
    backends: &mut [Backend],
    already: &BTreeSet<String>,
    batches: &FactBatches,
    prior_cause: Option<&str>,
    cause: &str,
) -> FanOut {
    let prior = prior_cause.map(|c| prior_projection(backends, c)).unwrap_or_default();
    replace_document_with_prior(backends, already, batches, prior, cause)
}

/// Deliver against a caller-owned durable projection; reads no sink.
pub fn replace_document_with_prior(
    backends: &mut [Backend],
    already: &BTreeSet<String>,
    batches: &FactBatches,
    prior: Vec<FactEntry>,
    cause: &str,
) -> FanOut {
    let retracted = dropped_cells(prior, batches);
    let mut out = FanOut { acked: already.clone(), retracted, ..FanOut::default() };
    for backend in backends.iter_mut().filter(|b| !already.contains(&b.name)) {
        <<deliver-one-backend>>
    }
    out
}
```

Delivery to one backend is the batches in order, then the diff. The
counts are taken from what the sink reports it applied, so a sink that
dedups within a batch counts what it kept:

<a name="chunk-deliver-one-backend"></a><sub>[`src/backend.rs`](../../crates/x0k-folio-ingest/src/backend.rs) · `#deliver-one-backend`</sub>

```rust {#deliver-one-backend}
let mut facts = 0usize;
let mut ok = true;
for (entity, entity_facts) in batches {
    out.sink_calls += 1;
    match backend.sink.replace(entity, entity_facts, cause) {
        Ok(applied) => facts += applied,
        Err(error) => {
            warn!(backend = %backend.name, entity = %entity, error = %error, "folio.backend.replace_failed");
            ok = false;
        }
    }
}
if !out.retracted.is_empty() {
    out.sink_calls += 1;
    match backend.sink.retract(&out.retracted, cause) {
        Ok(applied) => facts += applied,
        Err(error) => {
            warn!(backend = %backend.name, error = %error, "folio.backend.retract_failed");
            ok = false;
        }
    }
}
out.facts += facts;
info!(backend = %backend.name, facts, batches = batches.len(), retracted = out.retracted.len(), ok, "folio.backend.replace");
if ok {
    out.acked.insert(backend.name.clone());
}
```

A deleted document — or one that stopped being a folio — is the diff with
nothing on the new side: everything the last content projected is
retracted from every backend, caused by the deletion. There is no
reason to discard failed acknowledgements. The ingester retains the deletion
record and uses explicit-prior replacement with an empty batch. This
compatibility helper remains for callers owning their recovery:

<a name="chunk-retract-document"></a><sub>[`src/backend.rs`](../../crates/x0k-folio-ingest/src/backend.rs) · `#retract-document`</sub>

```rust {#retract-document}
/// Retract from every backend everything the document's last content
/// (under `prior_cause`) projected; `cause` names the deletion.
pub fn retract_document(backends: &mut [Backend], prior_cause: &str, cause: &str) -> FanOut {
    let retracted = prior_projection(backends, prior_cause);
    let mut out = FanOut { retracted, ..FanOut::default() };
    if out.retracted.is_empty() {
        return out;
    }
    for backend in backends.iter_mut() {
        out.sink_calls += 1;
        match backend.sink.retract(&out.retracted, cause) {
            Ok(applied) => {
                out.facts += applied;
                out.acked.insert(backend.name.clone());
            }
            Err(error) => warn!(backend = %backend.name, error = %error, "folio.backend.retract_failed"),
        }
        info!(backend = %backend.name, facts = out.retracted.len(), "folio.backend.retract");
    }
    out
}
```


## Durable state belongs to one source

A mirror's missed deletion must survive even when no file remains to
trigger another event. Each source therefore owns one durable checkpoint
beside the legacy state path, in a directory whose name ends in
".sources". Its filename is the digest of its exact source path; its
contents repeat that path so a mismatched key is rejected before writing.
The record carries either both file metadata and recovery state, or
neither: the latter is a deletion tombstone.

The source records overlay an existing aggregate snapshot. We leave that
snapshot unchanged during migration. A tombstone must remain on disk to
prevent an older snapshot from resurrecting its file. Incomplete temporary
files are ignored; malformed completed records stop ingestion.

<a name="chunk-source-checkpoint"></a><sub>[`src/checkpoint.rs`](../../crates/x0k-folio-ingest/src/checkpoint.rs) · `#source-checkpoint` · assembles [source-checkpoint-record](#chunk-source-checkpoint-record) · [source-checkpoint-paths](#chunk-source-checkpoint-paths) · [validate-source-checkpoint](#chunk-validate-source-checkpoint) · [load-source-checkpoints](#chunk-load-source-checkpoints) · [ensure-checkpoint-directory](#chunk-ensure-checkpoint-directory) · [write-checkpoint-atomically](#chunk-write-checkpoint-atomically) · [write-source-checkpoint](#chunk-write-source-checkpoint)</sub>

```rust {#source-checkpoint file="src/checkpoint.rs"}
use std::path::{Path, PathBuf};
use anyhow::{Context, Result};
use crate::lifecycle::{IngesterState, IngestedFile, RecoveryRecord};

<<source-checkpoint-record>>
<<source-checkpoint-paths>>
<<validate-source-checkpoint>>
<<load-source-checkpoints>>
<<ensure-checkpoint-directory>>
<<write-checkpoint-atomically>>
<<write-source-checkpoint>>
```

The version belongs to the storage envelope. Fact payloads inside the
recovery record retain their own versioned encoding.

<a name="chunk-source-checkpoint-record"></a><sub>[`src/backend.rs`](../../crates/x0k-folio-ingest/src/backend.rs) · `#source-checkpoint-record`</sub>

```rust {#source-checkpoint-record}
#[derive(serde::Serialize, serde::Deserialize)]
struct SourceCheckpoint {
    version: u32,
    path: String,
    file: Option<IngestedFile>,
    recovery: Option<RecoveryRecord>,
}
```

The hash bounds filename length without normalizing two distinct source
paths into the same key. The stored path is checked whenever a completed
record is read, including before an existing file is replaced.

<a name="chunk-source-checkpoint-paths"></a><sub>[`src/backend.rs`](../../crates/x0k-folio-ingest/src/backend.rs) · `#source-checkpoint-paths`</sub>

```rust {#source-checkpoint-paths}
pub fn source_directory(state_path: &Path) -> PathBuf {
    let mut name = state_path.as_os_str().to_os_string();
    name.push(".sources");
    PathBuf::from(name)
}

fn source_name(path: &str) -> String {
    format!("{}.json", blake3::hash(path.as_bytes()).to_hex())
}

fn read_source(path: &Path) -> Result<SourceCheckpoint> {
    let record: SourceCheckpoint = serde_json::from_slice(&std::fs::read(path)?)
        .with_context(|| format!("decode source checkpoint {}", path.display()))?;
    validate_source(path, &record)?;
    Ok(record)
}
```

A digest collision or misplaced record must never authorize overwriting a
different source. Both the filename and the requested source path are
checked, and an unsupported storage version is an error.

<a name="chunk-validate-source-checkpoint"></a><sub>[`src/backend.rs`](../../crates/x0k-folio-ingest/src/backend.rs) · `#validate-source-checkpoint`</sub>

```rust {#validate-source-checkpoint}
fn validate_source(path: &Path, record: &SourceCheckpoint) -> Result<()> {
    anyhow::ensure!(record.version == 1, "unsupported source checkpoint version");
    anyhow::ensure!(
        path.file_name().and_then(|s| s.to_str()) == Some(source_name(&record.path).as_str()),
        "source checkpoint filename does not match its source: {}", path.display()
    );
    anyhow::ensure!(
        record.file.is_some() == record.recovery.is_some(),
        "source checkpoint has incomplete recovery state: {}", path.display()
    );
    Ok(())
}
```

Startup first reads the legacy snapshot, if present, then applies each
completed source record. This is a linear read of stored state; source
writes do not serialize that aggregate again.

<a name="chunk-load-source-checkpoints"></a><sub>[`src/backend.rs`](../../crates/x0k-folio-ingest/src/backend.rs) · `#load-source-checkpoints`</sub>

```rust {#load-source-checkpoints}
pub fn load_state(state_path: &Path) -> Result<IngesterState> {
    let mut state = match std::fs::read(state_path) {
        Ok(bytes) => serde_json::from_slice(&bytes).context("decode ingester checkpoint")?,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => IngesterState::default(),
        Err(error) => return Err(error).context("read ingester checkpoint"),
    };
    let directory = source_directory(state_path);
    let entries = match std::fs::read_dir(&directory) {
        Ok(entries) => entries,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(state),
        Err(error) => return Err(error).context("read source checkpoint directory"),
    };
    for entry in entries {
        let path = entry?.path();
        if path.extension().and_then(|s| s.to_str()) != Some("json") { continue; }
        let record = read_source(&path)?;
        match (record.file, record.recovery) {
            (Some(file), Some(recovery)) => {
                state.files.insert(record.path.clone(), file);
                state.recovery.insert(record.path, recovery);
            }
            (None, None) => {
                state.files.remove(&record.path);
                state.recovery.remove(&record.path);
            }
            _ => unreachable!("validated source checkpoint shape"),
        }
    }
    Ok(state)
}
```

The first checkpoint also creates a directory. Syncing its contents does
not make its parent's new directory entry durable, so each newly created
directory syncs its parent before the first source write.

<a name="chunk-ensure-checkpoint-directory"></a><sub>[`src/backend.rs`](../../crates/x0k-folio-ingest/src/backend.rs) · `#ensure-checkpoint-directory`</sub>

```rust {#ensure-checkpoint-directory}
fn ensure_directory(path: &Path) -> Result<()> {
    if path.is_dir() { return Ok(()); }
    let parent = path.parent().filter(|p| !p.as_os_str().is_empty()).unwrap_or(Path::new("."));
    ensure_directory(parent)?;
    match std::fs::create_dir(path) {
        Ok(()) => {}
        Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists && path.is_dir() => {}
        Err(error) => return Err(error).context("create checkpoint directory"),
    }
    std::fs::File::open(parent)?.sync_all().context("sync checkpoint directory parent")
}
```

A completed record appears only after the temporary file is synced and
renamed. Syncing the containing directory makes that replacement durable.
The ingester owns the directory exclusively; this is not a multi-process
locking protocol.

<a name="chunk-write-checkpoint-atomically"></a><sub>[`src/backend.rs`](../../crates/x0k-folio-ingest/src/backend.rs) · `#write-checkpoint-atomically`</sub>

```rust {#write-checkpoint-atomically}
pub fn atomic_write(path: &Path, bytes: &[u8]) -> Result<()> {
    use std::io::Write;
    let directory = path.parent().filter(|p| !p.as_os_str().is_empty()).unwrap_or(Path::new("."));
    ensure_directory(directory)?;
    let temporary = path.with_extension("tmp");
    let mut file = std::fs::File::create(&temporary).context("create checkpoint temporary")?;
    file.write_all(bytes).context("write checkpoint temporary")?;
    file.sync_all().context("sync checkpoint temporary")?;
    std::fs::rename(&temporary, path).context("replace checkpoint")?;
    std::fs::File::open(directory)?.sync_all().context("sync checkpoint directory")
}
```

Before effects, the record contains all possible writes. After effects, it
contains either the narrower acknowledged projection or the still-pending
obligation. The same operation stores a tombstone when deletion finishes.

<a name="chunk-write-source-checkpoint"></a><sub>[`src/backend.rs`](../../crates/x0k-folio-ingest/src/backend.rs) · `#write-source-checkpoint`</sub>

```rust {#write-source-checkpoint}
pub fn write_source_state(state_path: &Path, key: &str, state: &IngesterState) -> Result<()> {
    let path = source_directory(state_path).join(source_name(key));
    match std::fs::symlink_metadata(&path) {
        Ok(_) => {
            let prior = read_source(&path)?;
            anyhow::ensure!(prior.path == key, "source checkpoint digest collision");
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(error) => return Err(error).context("inspect source checkpoint"),
    }
    let record = SourceCheckpoint {
        version: 1,
        path: key.to_string(),
        file: state.files.get(key).cloned(),
        recovery: state.recovery.get(key).cloned(),
    };
    validate_source(&path, &record)?;
    atomic_write(&path, &serde_json::to_vec(&record)?)
}
```

## A sink that is only a `Vec`

The smallest legitimate backend holds the current view in memory and
nothing else: a mirror, an archival tap, a test's witness. It is a
`FactSink` in eleven lines of logic and it holds a view, so it can answer
`facts_caused_by` too. The handle is shared so the process that built the
backend list can still read what the mirror received after the list has
taken ownership of the sink. Both methods report the batch they accepted,
not the cells they changed — a retraction of a cell the region replacement
already removed is applied, idempotently, and counts as applied:

<a name="chunk-memory-sink"></a><sub>[`src/backend.rs`](../../crates/x0k-folio-ingest/src/backend.rs) · `#memory-sink`</sub>

```rust {#memory-sink}
/// A current-view sink over a `Vec<FactEntry>`: the smallest backend, and
/// the witness the tests below read.
#[derive(Debug, Clone, Default)]
pub struct MemorySink(std::sync::Arc<std::sync::Mutex<Vec<FactEntry>>>);

impl MemorySink {
    /// The current view — replaced regions and retracted cells applied.
    pub fn facts(&self) -> Vec<FactEntry> {
        self.0.lock().unwrap_or_else(|poisoned| poisoned.into_inner()).clone()
    }
}

impl FactSink for MemorySink {
    fn replace(&mut self, entity: &str, facts: &[FactEntry], _cause: &str) -> Result<usize> {
        let mut view = self.0.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
        view.retain(|fact| fact.entity != entity);
        view.extend(facts.iter().cloned());
        Ok(facts.len())
    }
    fn retract(&mut self, facts: &[FactEntry], _cause: &str) -> Result<usize> {
        let gone: HashSet<_> = facts.iter().map(fact_cell_key).collect();
        let mut view = self.0.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
        view.retain(|fact| !gone.contains(&fact_cell_key(fact)));
        Ok(facts.len())
    }
    fn retains_history(&self) -> bool {
        false
    }
    fn facts_caused_by(&self, cause: &str) -> Result<Option<Vec<FactEntry>>> {
        Ok(Some(self.facts().into_iter().filter(|f| f.cause.as_deref() == Some(cause)).collect()))
    }
}
```


## Package and host contract

The package has no parser dependency. It reuses the shared typed facts and
requires the caller to distinguish non-documents from invalid documents.
An invalid document keeps its last-good view and reports an error; this is
not a claim of fresh results or a complete freshness/status interface.

<a name="chunk-ingest-manifest"></a><sub>[`Cargo.toml`](../../crates/x0k-folio-ingest/Cargo.toml) · `#ingest-manifest`</sub>

```toml {#ingest-manifest file="Cargo.toml"}
[package]
name = "x0k-folio-ingest"
version = "0.1.0"
edition = { workspace = true }
license = "MIT"
description = "Standalone document ingestion and independent backend recovery"
rust-version = { workspace = true }
repository = "https://github.com/0k-dot-computer/x0k-folio"
readme = "../../README.md"
keywords = ["literate-programming", "tangle", "markdown", "documentation"]

[dependencies]
x0k-fact-projection = { path = "../x0k-fact-projection" , version = "0.1.0" }
anyhow = "1"
serde = { version = "1", features = ["derive"] }
serde_json = "1"
blake3 = "1"
tracing = "0.1"
tokio = { version = "1", features = ["rt", "time", "sync", "macros"] }

[dev-dependencies]
tempfile = "3"
```

The host and standalone executable use the same library modules.

<a name="chunk-ingest-modules"></a><sub>[`src/lib.rs`](../../crates/x0k-folio-ingest/src/lib.rs) · `#ingest-modules`</sub>

```rust {#ingest-modules file="src/lib.rs"}
pub mod backend;
mod delivery;
pub mod events;
pub mod lifecycle;
#[doc(hidden)]
pub mod checkpoint;
```

Change notifications and scan summaries are plain data shared with host notifiers.

<a name="chunk-ingestion-events"></a><sub>[`src/events.rs`](../../crates/x0k-folio-ingest/src/events.rs) · `#ingestion-events`</sub>

```rust {#ingestion-events file="src/events.rs"}
use std::path::PathBuf;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PathChangeEvent {
    /// `path` still exists and parsed as folio/v1.
    Upserted(PathBuf),
    /// `path` no longer exists on disk.
    Removed(PathBuf),
}

/// Per-run summary written to `state_path` and embedded in the milestone row.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct ScanState {
    pub scan_root: String,
    pub tree_content_hash: String,
    pub file_count: usize,
    pub started_at_us: u64,
    pub finished_at_us: u64,
}
```

## One lifecycle, supplied by the host

Filesystem paths and shared fact values are the only data crossing the host boundary.

<a name="chunk-lifecycle-imports"></a><sub>[`src/lifecycle.rs`](../../crates/x0k-folio-ingest/src/lifecycle.rs) · `#lifecycle-imports`</sub>

```rust {#lifecycle-imports file="src/lifecycle.rs"}
use std::collections::{BTreeMap, BTreeSet, HashSet};
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::Duration;
use anyhow::Result;
use tracing::{info, trace, warn};
use x0k_fact_projection::{FactEntry};
use x0k_fact_projection::payload::{file_content_cause, file_deleted_cause, FactPayload};
use crate::backend::{self, Backend, FactBatches, backend_names};
use crate::checkpoint::{load_state, write_source_state};
```

Discovery and projection remain explicit host policies; the engine retains their distinction between a non-document and a rejected document.

<a name="chunk-source-contract"></a><sub>[`src/lifecycle.rs`](../../crates/x0k-folio-ingest/src/lifecycle.rs) · `#source-contract`</sub>

```rust {#source-contract file="src/lifecycle.rs"}
/// The host's complete fact projection for one source revision.
#[derive(Debug, Clone)]
pub struct DocumentProjection {
    pub uri: String,
    pub content_hash: String,
    pub batches: FactBatches,
}

/// None means a non-document; Err preserves its last-good projection.
/// Discovery must cover the complete source set owned by this checkpoint.
pub trait DocumentSource: Send {
    /// Read a discovered source; virtual sources may override filesystem I/O.
    fn read(&self, path: &Path) -> Result<Vec<u8>> { Ok(std::fs::read(path)?) }

    /// Include prepared vocabulary dependencies when they affect projection.
    fn revision_hash(&self, bytes: &[u8]) -> String {
        blake3::hash(bytes).to_hex().to_string()
    }

    fn discover(&self, root: &Path) -> Result<Vec<PathBuf>>;
    fn accepts(&self, path: &Path) -> bool;
    fn project(&self, path: &Path, bytes: &[u8], hash: &str) -> Result<Option<DocumentProjection>>;
    fn pause_after_write(&mut self, _facts: usize) -> Option<Duration> { None }
}

fn validate_backend_names(backends: &[Backend]) -> Result<()> {
    let mut seen = BTreeSet::new();
    for backend in backends {
        anyhow::ensure!(!backend.name.trim().is_empty(), "backend name must not be empty");
        anyhow::ensure!(seen.insert(&backend.name), "duplicate backend name: {}", backend.name);
    }
    Ok(())
}

fn lock_backends(backends: &Mutex<Vec<Backend>>) -> std::sync::MutexGuard<'_, Vec<Backend>> {
    backends.lock().unwrap_or_else(|poisoned| poisoned.into_inner())
}
```

The state representation preserves existing checkpoints, including the legacy spine acknowledgement spelling. That spelling carries no substrate dependency.

<a name="chunk-checkpoint-state"></a><sub>[`src/lifecycle.rs`](../../crates/x0k-folio-ingest/src/lifecycle.rs) · `#checkpoint-state`</sub>

```rust {#checkpoint-state file="src/lifecycle.rs"}
/// Per-file record persisted in its source checkpoint. `content_hash` is the blake3
/// of the file bytes; a matching hash acknowledged by every configured
/// backend means "no work needed for this file".
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(from = "IngestedFileWire")]
pub struct IngestedFile {
    pub content_hash: String,
    /// EntityUri this file ingested under (string form).
    pub uri: String,
    /// Count of facts the fold emitted on the most recent ingest (one
    /// backend's share, not the fan-out total). Informational; the
    /// ingester does not gate on this.
    pub fact_count: usize,
    /// The backends, by name, that applied this content hash without
    /// error (`folio-backends` §3). A backend absent here is offered the
    /// document again on the next reconcile.
    pub acked_by: BTreeSet<String>,
}

/// The on-disk shape, which also reads records written before backends
/// were named: those carried `spine: true` for "the x0k backend applied
/// it", and a record with neither field acknowledges nothing.
#[derive(serde::Deserialize)]
struct IngestedFileWire {
    content_hash: String,
    uri: String,
    fact_count: usize,
    #[serde(default)]
    spine: bool,
    #[serde(default)]
    acked_by: Option<BTreeSet<String>>,
}

impl From<IngestedFileWire> for IngestedFile {
    fn from(wire: IngestedFileWire) -> Self {
        let acked_by = wire.acked_by.unwrap_or_else(|| {
            if wire.spine {
                BTreeSet::from(["x0k".to_string()])
            } else {
                BTreeSet::new()
            }
        });
        Self {
            content_hash: wire.content_hash,
            uri: wire.uri,
            fact_count: wire.fact_count,
            acked_by,
        }
    }
}

/// State file shape. Top-level map: absolute path string → `IngestedFile`.
#[derive(Debug, Clone, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct IngesterState {
    pub files: BTreeMap<String, IngestedFile>,
    #[serde(default)]
    pub recovery: BTreeMap<String, RecoveryRecord>,
}

/// Each backend retains its own possible writes until it acknowledges them.
#[derive(Debug, Clone, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct RecoveryRecord {
    pub revisions: BTreeMap<String, BackendRevision>,
    pub deleted: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct BackendRevision {
    #[serde(default)]
    pub incarnation: Option<String>,
    pub applied_hash: Option<String>,
    /// None means a legacy checkpoint still needs its own sink's view.
    pub facts: Option<Vec<Vec<u8>>>,
}
```

Delivery retains the same recovery protocol: persist possible writes, deliver independently, narrow only after acknowledgement.

<a name="chunk-deliver-source"></a><sub>[`src/lifecycle.rs`](../../crates/x0k-folio-ingest/src/lifecycle.rs) · `#deliver-source` · assembles [recover-source-prior](#chunk-recover-source-prior) · [collect-current-facts](#chunk-collect-current-facts) · [initialize-backend-revisions](#chunk-initialize-backend-revisions) · [select-target-revision](#chunk-select-target-revision) · [remember-possible-effects](#chunk-remember-possible-effects) · [checkpoint-before-effects](#chunk-checkpoint-before-effects) · [apply-pending-backends](#chunk-apply-pending-backends) · [checkpoint-acknowledgements](#chunk-checkpoint-acknowledgements)</sub>

```rust {#deliver-source file="src/lifecycle.rs"}
fn probe_incarnations(sinks: &[(String, crate::delivery::Worker)], deadline: std::time::Instant)
    -> BTreeMap<String, Result<Option<String>>> {
    let submitted: Vec<_> = sinks.iter().map(|(name,worker)| (name.clone(),worker.start_identity())).collect();
    submitted.into_iter().map(|(name,ticket)| (name,ticket.and_then(|ticket|ticket.identity_until(deadline)))).collect()
}
fn bind_incarnation(revision: &mut BackendRevision, value: &Result<Option<String>>,
    acked: &mut BTreeSet<String>, name: &str) -> bool {
    match value {
        Ok(Some(identity)) if !identity.is_empty() => {
            if revision.incarnation.as_ref() != Some(identity) {
                acked.remove(name);
                if revision.incarnation.is_some() {
                    // A fresh store cannot contain the old store's possible writes.
                    revision.applied_hash = None;
                    revision.facts = Some(Vec::new());
                }
                revision.incarnation = Some(identity.clone());
            }
            true
        }
        Ok(None) if revision.incarnation.is_none() => true,
        result => {
            acked.remove(name);
            warn!(backend=%name,result=?result,"folio.backend.identity_unavailable");
            false
        }
    }
}
/// Persist possible effects before delivering them; failed backends keep their
/// own prior facts. Regression: recovery_survives_backend_outage_and_restart.
pub fn deliver(
    state: &mut IngesterState,
    state_path: &Path,
    path_key: &str,
    backends: &Mutex<Vec<Backend>>,
    desired: Option<&DocumentProjection>,
) -> Result<usize> {
    <<recover-source-prior>>
    <<collect-current-facts>>
    <<initialize-backend-revisions>>
    <<select-target-revision>>
    <<remember-possible-effects>>
    <<checkpoint-before-effects>>
    <<apply-pending-backends>>
    <<checkpoint-acknowledgements>>
}
```

A source without prior state needs no deletion. A legacy checkpoint with ambiguous backend history cannot authorize a write.

<a name="chunk-recover-source-prior"></a><sub>[`src/backend.rs`](../../crates/x0k-folio-ingest/src/backend.rs) · `#recover-source-prior`</sub>

```rust {#recover-source-prior}
let (sinks, grace) = {
    let registry = lock_backends(backends);
    validate_backend_names(&registry)?;
    (registry.iter().map(|backend| (backend.name.clone(),backend.worker.clone())).collect::<Vec<_>>(),
        registry.iter().map(|backend| backend.grace).max().unwrap_or_default())
};
let delivery_started = std::time::Instant::now();
let identities = probe_incarnations(&sinks, delivery_started + grace / 3);
let prior = state.files.get(path_key).cloned();
if prior.is_none() && desired.is_none() {
    return Ok(0);
}
if !state.recovery.contains_key(path_key) {
    if let Some(previous) = &prior {
        anyhow::ensure!(
            sinks.iter().all(|sink| previous.acked_by.contains(&sink.0)),
            "legacy checkpoint for {path_key} lacks a backend acknowledgement; rebuild the backend stores and checkpoint from the corpus before retrying"
        );
    }
}
let batches = desired.map(|projection| projection.batches.clone()).unwrap_or_default();
```

The supplied projection is flattened once for checkpoint ownership and fact accounting.

<a name="chunk-collect-current-facts"></a><sub>[`src/backend.rs`](../../crates/x0k-folio-ingest/src/backend.rs) · `#collect-current-facts`</sub>

```rust {#collect-current-facts}
let current: Vec<FactEntry> = batches.iter().flat_map(|(_, facts)| facts.iter().cloned()).collect();
let footprint = current.len();
let recovery = state.recovery.entry(path_key.to_string()).or_default();
```

Each named backend starts from its own known applied revision; a newly attached backend starts with no facts.

<a name="chunk-initialize-backend-revisions"></a><sub>[`src/backend.rs`](../../crates/x0k-folio-ingest/src/backend.rs) · `#initialize-backend-revisions`</sub>

```rust {#initialize-backend-revisions}
let names: BTreeSet<String> = sinks.iter().map(|b| b.0.clone())
    .chain(prior.iter().flat_map(|p| p.acked_by.iter().cloned())).collect();
for name in names {
    recovery.revisions.entry(name.clone()).or_insert_with(|| {
        let applied = prior.as_ref().filter(|p| p.acked_by.contains(&name));
        BackendRevision {
            incarnation: None,
            applied_hash: applied.map(|p| p.content_hash.clone()),
            facts: if applied.is_some() { None } else { Some(Vec::new()) },
        }
    });
}
```

Acknowledgements apply to one revision or to its deletion. A different desired state clears them.

<a name="chunk-select-target-revision"></a><sub>[`src/backend.rs`](../../crates/x0k-folio-ingest/src/backend.rs) · `#select-target-revision`</sub>

```rust {#select-target-revision}
let deleted = desired.is_none();
let same = prior.as_ref().is_some_and(|p| desired.is_some_and(|projection| p.content_hash == projection.content_hash))
    && !recovery.deleted;
let mut acked = if same || (deleted && recovery.deleted) {
    prior.as_ref().map(|p| p.acked_by.clone()).unwrap_or_default()
} else { BTreeSet::new() };
recovery.deleted = deleted;
let hash = desired.map(|projection| projection.content_hash.clone())
    .unwrap_or_else(|| prior.as_ref().unwrap().content_hash.clone());
let cause = if deleted { file_deleted_cause(&hash) } else { file_content_cause(&hash) };
```

Before any effect, each pending backend retains the union of facts it may already hold and facts about to be delivered.

<a name="chunk-remember-possible-effects"></a><sub>[`src/backend.rs`](../../crates/x0k-folio-ingest/src/backend.rs) · `#remember-possible-effects`</sub>

```rust {#remember-possible-effects}
let mut available = BTreeSet::new();
for sink in &sinks {
    if bind_incarnation(recovery.revisions.get_mut(&sink.0).unwrap(), &identities[&sink.0], &mut acked, &sink.0) {
        available.insert(sink.0.clone());
    }
}
let mut pending_reads = BTreeMap::new();
for sink in &sinks {
    if !available.contains(&sink.0) { continue; }
    let revision = &recovery.revisions[&sink.0];
    if revision.facts.is_none() {
        let cause = file_content_cause(revision.applied_hash.as_deref().unwrap_or_default());
        pending_reads.insert(sink.0.clone(), sink.1.start_read(&cause));
    }
}
let read_wait_started = delivery_started;
let read_deadline = read_wait_started + grace.mul_f32(2.0 / 3.0);
let mut ready = BTreeMap::new();
for sink in sinks.iter() {
    if !available.contains(&sink.0) { continue; }
    let revision = recovery.revisions.get_mut(&sink.0).unwrap();
    if revision.facts.is_none() {
        let recovered = pending_reads.remove(&sink.0).unwrap()
            .and_then(|ticket| ticket.facts_until(read_deadline));
        match recovered {
            Ok(Some(facts)) => {
                revision.facts = Some(facts.iter().map(|f| FactPayload::from_fact(f).to_bytes()).collect());
            }
            result => {
                warn!(backend = %sink.0, result = ?result, "folio.backend.recovery_read_failed");
                acked.remove(&sink.0);
                continue;
            }
        }
    }
    if acked.contains(&sink.0) {
        continue;
    }
    let mut possible = revision.facts.as_ref().unwrap().iter()
        .map(|bytes| FactPayload::from_bytes(bytes).map(FactPayload::into_fact))
        .collect::<std::result::Result<Vec<_>, _>>()?;
    let mut cells: HashSet<_> = possible.iter().map(backend::fact_cell_key).collect();
    possible.extend(current.iter().filter(|f| cells.insert(backend::fact_cell_key(f))).cloned());
    revision.facts = Some(possible.iter().map(|f| FactPayload::from_fact(f).to_bytes()).collect());
    ready.insert(sink.0.clone(), possible);
}
```

The desired metadata and possible writes must reach durable storage before the first sink call.

<a name="chunk-checkpoint-before-effects"></a><sub>[`src/backend.rs`](../../crates/x0k-folio-ingest/src/backend.rs) · `#checkpoint-before-effects`</sub>

```rust {#checkpoint-before-effects}
let completion_grace = grace.saturating_sub(read_wait_started.elapsed());
state.files.insert(path_key.to_string(), IngestedFile {
    content_hash: hash.clone(),
    uri: desired.map(|projection| projection.uri.clone()).unwrap_or_else(|| prior.unwrap().uri),
    fact_count: footprint,
    acked_by: acked.clone(),
});
// No sink call is permitted before this durable write succeeds.
write_source_state(state_path, path_key, state)?;
```

Only complete backend success narrows its possible facts to the new projection. Its siblings retain independent obligations.

<a name="chunk-apply-pending-backends"></a><sub>[`src/backend.rs`](../../crates/x0k-folio-ingest/src/backend.rs) · `#apply-pending-backends`</sub>

```rust {#apply-pending-backends}
let mut submitted = Vec::new();
for sink in &sinks {
    let Some(possible) = ready.remove(&sink.0) else { continue };
    submitted.push((sink.0.clone(),sink.1.source(path_key,&batches,possible,&cause)));
}
let deadline = std::time::Instant::now() + completion_grace;
for (name, ticket) in submitted {
    let result = ticket.and_then(|ticket| ticket.count_until(deadline));
    if let Err(error) = &result {
        warn!(path = %path_key, backend = %name, error = %error, "folio.backend.source_failed");
    }
    if result.is_ok() {
        acked.insert(name.clone());
        let revision = state.recovery.get_mut(path_key).unwrap()
            .revisions.get_mut(&name).unwrap();
        revision.applied_hash = if deleted { None } else { Some(hash.clone()) };
        revision.facts = Some(current.iter().map(|f| FactPayload::from_fact(f).to_bytes()).collect());
    }
}
```

Deletion completes after every known backend acknowledges it; the source checkpoint then stores a tombstone.

<a name="chunk-checkpoint-acknowledgements"></a><sub>[`src/backend.rs`](../../crates/x0k-folio-ingest/src/backend.rs) · `#checkpoint-acknowledgements`</sub>

```rust {#checkpoint-acknowledgements}
state.files.get_mut(path_key).unwrap().acked_by = acked.clone();
let known: BTreeSet<_> = state.recovery[path_key].revisions.keys().cloned().collect();
if deleted && known.is_subset(&acked) {
    state.files.remove(path_key);
    state.recovery.remove(path_key);
}
write_source_state(state_path, path_key, state)?;
Ok(footprint)
```

A complete discovery pass may retract absent sources. Cancellation exits before that deletion pass, because an incomplete scan cannot establish absence.

<a name="chunk-reconcile-source-set"></a><sub>[`src/lifecycle.rs`](../../crates/x0k-folio-ingest/src/lifecycle.rs) · `#reconcile-source-set`</sub>

```rust {#reconcile-source-set file="src/lifecycle.rs"}
pub async fn reconcile(
    source: &mut dyn DocumentSource,
    repo_root: &Path,
    state_path: &Path,
    backends: &Mutex<Vec<Backend>>,
    cancel: Option<&std::sync::atomic::AtomicBool>,
) -> Result<(usize, usize, usize)> {
    validate_backend_names(&lock_backends(backends))?;
    let paths = source.discover(repo_root)?;
    let names = backend_names(&lock_backends(backends));
    let mut state = load_state(state_path)?;
    let (identity_sinks, identity_grace) = {
        let registry=lock_backends(backends);
        (registry.iter().map(|b|(b.name.clone(),b.worker.clone())).collect::<Vec<_>>(),
            registry.iter().map(|b|b.grace).max().unwrap_or_default())
    };
    let identities=probe_incarnations(&identity_sinks,std::time::Instant::now()+identity_grace);
    let mut ingested = 0usize;
    let mut total_facts = 0usize;
    let mut seen_paths: HashSet<String> = HashSet::new();

    for path in &paths {
        tokio::task::yield_now().await;
        if let Some(cancel) = cancel {
            if cancel.load(std::sync::atomic::Ordering::Relaxed) {
                info!(
                    files_seen = seen_paths.len(),
                    files_total = paths.len(),
                    ingested,
                    "folio ingester: reconcile cancelled by shutdown; \
                     resuming from the state file on next boot"
                );
                return Ok((seen_paths.len(), ingested, total_facts));
            }
        }
        let path_key = path.to_string_lossy().to_string();
        seen_paths.insert(path_key.clone());
        if let (Some(record),Some(file))=(state.recovery.get_mut(&path_key),state.files.get_mut(&path_key)) {
            let before=(record.clone(),file.clone());
            for (name,identity) in &identities {
                if let Some(revision)=record.revisions.get_mut(name) {
                    bind_incarnation(revision,identity,&mut file.acked_by,name);
                }
            }
            if before != (record.clone(),file.clone()) {
                write_source_state(state_path,&path_key,&state)?;
            }
        }
        let bytes = match source.read(path) {
            Ok(b) => b,
            Err(e) => {
                warn!(path = %path.display(), error = %e, "ingester read failed");
                continue;
            }
        };
        let hash = source.revision_hash(&bytes);
        // Backends that already applied this exact content are not
        // written again; the fold is offered to the rest, if any.
        let already: BTreeSet<String> = state
            .files
            .get(&path_key)
            .filter(|prior| prior.content_hash == hash)
            .map(|prior| prior.acked_by.clone())
            .unwrap_or_default();
        if names.is_subset(&already) && state.recovery.get(&path_key)
            .is_some_and(|r| !r.deleted && names.iter().all(|n|
                r.revisions.get(n).is_some_and(|v| v.facts.is_some()))) {
            trace!(path = %path.display(), "ingester: hash unchanged and acknowledged, skipping");
            continue;
        }

        let projection = match source.project(path, &bytes, &hash) {
            Ok(projection) => projection,
            Err(error) => {
                warn!(path = %path.display(), error = %error, "folio.ingester.projection_failed");
                continue;
            }
        };
        let facts_written = deliver(
            &mut state, state_path, &path_key, backends, projection.as_ref())?;
        if projection.is_none() { continue; }
        total_facts += facts_written;
        ingested += 1;
        if let Some(pause) = source.pause_after_write(facts_written) {
            tokio::time::sleep(pause).await;
        }

    }

    // Tracked files that disappeared between runs: retract everything
    // their last content projected (single-spine C3 — same path as a
    // watcher-observed deletion).
    let gone: Vec<String> = state
        .files
        .keys()
        .filter(|key| !seen_paths.contains(*key))
        .cloned()
        .collect();
    for key in gone {
        deliver(&mut state, state_path, &key, backends, None)?;
    }
    Ok((paths.len(), ingested, total_facts))
}
```

An individual event uses the same projection and delivery functions, so the watcher adds scheduling rather than a second ingestion algorithm.

<a name="chunk-apply-source-event"></a><sub>[`src/lifecycle.rs`](../../crates/x0k-folio-ingest/src/lifecycle.rs) · `#apply-source-event`</sub>

```rust {#apply-source-event file="src/lifecycle.rs"}
/// Apply one source event using the same checkpoint and delivery protocol.
pub fn apply_path_change(
    source: &dyn DocumentSource,
    path: &Path,
    state_path: &Path,
    backends: &Mutex<Vec<Backend>>,
) -> Result<usize> {
    validate_backend_names(&lock_backends(backends))?;
    if !source.accepts(path) { return Ok(0); }
    let mut state = load_state(state_path)?;
    let key = path.to_string_lossy().to_string();
    if !path.exists() {
        return deliver(&mut state, state_path, &key, backends, None);
    }
    let bytes = source.read(path)?;
    let hash = source.revision_hash(&bytes);
    let projection = source.project(path, &bytes, &hash)?;
    deliver(&mut state, state_path, &key, backends, projection.as_ref())
}
```

The host chooses what a document means; the engine owns what a failed delivery still owes. Neither role requires the other to own its runtime.

## A source independent of Folio and x0k

The public API tests use a tiny line format so they cannot accidentally rely
on the private parser or registry. They exercise the same lifecycle the
x0k adapter calls, against temporary stores only.

The fixture holds only neutral types.

<a name="chunk-standalone-imports"></a><sub>[`tests/standalone.rs`](../../crates/x0k-folio-ingest/tests/standalone.rs) · `#standalone-imports`</sub>

```rust {#standalone-imports file="tests/standalone.rs"}
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, atomic::{AtomicBool, Ordering}};
use anyhow::Result;
use x0k_fact_projection::{FactEntry, FactValue};
use x0k_fact_projection::payload::file_content_cause;
use x0k_folio_ingest::backend::{Backend, FactSink, MemorySink};
use x0k_folio_ingest::lifecycle::{self, DocumentSource, DocumentProjection};
struct Lines;
```

An empty file is not a document; a missing separator is an invalid document. Their outcomes must remain different.

<a name="chunk-standalone-source"></a><sub>[`tests/standalone.rs`](../../crates/x0k-folio-ingest/tests/standalone.rs) · `#standalone-source`</sub>

```rust {#standalone-source file="tests/standalone.rs"}
impl DocumentSource for Lines {
    fn discover(&self, root: &Path) -> Result<Vec<PathBuf>> {
        Ok(std::fs::read_dir(root)?.map(|entry| entry.map(|e| e.path()))
            .collect::<std::io::Result<Vec<_>>>()?
            .into_iter().filter(|path| self.accepts(path)).collect())
    }
    fn accepts(&self, path: &Path) -> bool {
        path.extension().and_then(|s| s.to_str()) == Some("txt")
    }
    fn project(&self, _path: &Path, bytes: &[u8], hash: &str) -> Result<Option<DocumentProjection>> {
        let text = std::str::from_utf8(bytes)?.trim();
        if text.is_empty() { return Ok(None); }
        let (uri, label) = text.split_once('=').ok_or_else(|| anyhow::anyhow!("invalid line document"))?;
        let fact = FactEntry {
            entity: uri.to_string(), predicate: "urn:label".to_string(),
            value: FactValue::Text(label.to_string()), cause: Some(file_content_cause(hash)),
        };
        Ok(Some(DocumentProjection {
            uri: uri.to_string(), content_hash: hash.to_string(),
            batches: vec![(uri.to_string(), vec![fact])],
        }))
    }
}
```

The offline sink deliberately offers no readable view. Recovery must use checkpoints rather than asking another store.

<a name="chunk-standalone-outage-sink"></a><sub>[`tests/standalone.rs`](../../crates/x0k-folio-ingest/tests/standalone.rs) · `#standalone-outage-sink`</sub>

```rust {#standalone-outage-sink file="tests/standalone.rs"}
struct Offline {
    view: MemorySink,
    down: Arc<AtomicBool>,
}
impl FactSink for Offline {
    fn replace(&mut self, entity: &str, facts: &[FactEntry], cause: &str) -> Result<usize> {
        anyhow::ensure!(!self.down.load(Ordering::SeqCst), "offline");
        self.view.replace(entity, facts, cause)
    }
    fn retract(&mut self, facts: &[FactEntry], cause: &str) -> Result<usize> {
        anyhow::ensure!(!self.down.load(Ordering::SeqCst), "offline");
        self.view.retract(facts, cause)
    }
    fn retains_history(&self) -> bool { false }
}
```

Reconstructing the backend list on every pass makes persisted source state the only continuity available to the engine.

<a name="chunk-standalone-recovery-test"></a><sub>[`tests/standalone.rs`](../../crates/x0k-folio-ingest/tests/standalone.rs) · `#standalone-recovery-test`</sub>

```rust {#standalone-recovery-test file="tests/standalone.rs"}
#[tokio::test]
async fn standalone_source_recovers_without_a_backend_read_view() {
    let root = tempfile::tempdir().unwrap();
    let scratch = tempfile::tempdir().unwrap();
    let state = scratch.path().join("state.json");
    let file = root.path().join("source.txt");
    let a = MemorySink::default();
    let b = MemorySink::default();
    let down = Arc::new(AtomicBool::new(false));
    let stores = || Mutex::new(vec![
        Backend::new("a", a.clone()),
        Backend::new("b", Offline { view: b.clone(), down: down.clone() }),
    ]);
    std::fs::write(&file, "urn:old=Before").unwrap();
    lifecycle::reconcile(&mut Lines, root.path(), &state, &stores(), None).await.unwrap();
    down.store(true, Ordering::SeqCst);
    std::fs::write(&file, "urn:new=After").unwrap();
    lifecycle::reconcile(&mut Lines, root.path(), &state, &stores(), None).await.unwrap();
    assert_ne!(a.facts(), b.facts());
    down.store(false, Ordering::SeqCst);
    lifecycle::reconcile(&mut Lines, root.path(), &state, &stores(), None).await.unwrap();
    assert_eq!(a.facts(), b.facts());
    assert_eq!(b.facts()[0].entity, "urn:new");
    down.store(true, Ordering::SeqCst);
    std::fs::remove_file(&file).unwrap();
    lifecycle::reconcile(&mut Lines, root.path(), &state, &stores(), None).await.unwrap();
    assert!(a.facts().is_empty() && !b.facts().is_empty());
    down.store(false, Ordering::SeqCst);
    lifecycle::reconcile(&mut Lines, root.path(), &state, &stores(), None).await.unwrap();
    assert!(b.facts().is_empty());
}
```

A rejected edit remains visibly an error at the event boundary; treating it as an empty document would erase the last-good view.

<a name="chunk-standalone-invalid-test"></a><sub>[`tests/standalone.rs`](../../crates/x0k-folio-ingest/tests/standalone.rs) · `#standalone-invalid-test`</sub>

```rust {#standalone-invalid-test file="tests/standalone.rs"}
#[tokio::test]
async fn invalid_source_keeps_last_good_but_non_document_retracts() {
    let root = tempfile::tempdir().unwrap();
    let scratch = tempfile::tempdir().unwrap();
    let state = scratch.path().join("state.json");
    let file = root.path().join("source.txt");
    let view = MemorySink::default();
    let stores = Mutex::new(vec![Backend::new("memory", view.clone())]);
    std::fs::write(&file, "urn:paper=Readable").unwrap();
    lifecycle::apply_path_change(&Lines, &file, &state, &stores).unwrap();
    let before = view.facts();
    std::fs::write(&file, "invalid").unwrap();
    assert!(lifecycle::apply_path_change(&Lines, &file, &state, &stores).is_err());
    let (_, ingested, _) = lifecycle::reconcile(&mut Lines, root.path(), &state, &stores, None).await.unwrap();
    assert_eq!(ingested, 0);
    assert_eq!(view.facts(), before);
    std::fs::write(&file, "").unwrap();
    lifecycle::apply_path_change(&Lines, &file, &state, &stores).unwrap();
    assert!(view.facts().is_empty());
}
```

These tests establish a usable host boundary. They do not establish a Dialog adapter, arbitrary query semantics, or a complete freshness model.

A backend name identifies its independent checkpoint obligation. Empty or
duplicate names are refused before checkpoints or sink effects.

<a name="chunk-reject-ambiguous-backends"></a><sub>[`tests/standalone.rs`](../../crates/x0k-folio-ingest/tests/standalone.rs) · `#reject-ambiguous-backends`</sub>

```rust {#reject-ambiguous-backends file="tests/standalone.rs"}
#[tokio::test]
async fn ambiguous_backend_names_cannot_write_checkpoints_or_facts() {
    for names in [["same", "same"], ["", "second"], ["   ", "second"]] {
        let root = tempfile::tempdir().unwrap();
        let scratch = tempfile::tempdir().unwrap();
        let state = scratch.path().join("state.json");
        let file = root.path().join("source.txt");
        std::fs::write(&file, "urn:paper=Untouched").unwrap();
        let a = MemorySink::default();
        let b = MemorySink::default();
        let stores = Mutex::new(vec![Backend::new(names[0], a.clone()), Backend::new(names[1], b.clone())]);
        assert!(lifecycle::reconcile(&mut Lines, root.path(), &state, &stores, None).await.is_err());
        assert!(lifecycle::apply_path_change(&Lines, &file, &state, &stores).is_err());
        assert!(!x0k_folio_ingest::checkpoint::source_directory(&state).exists());
        assert!(a.facts().is_empty() && b.facts().is_empty());
    }
}
```

Independent backend instances require independent names; the same adapter type may appear more than once.

## Source-qualified revision boundary

Changing a prepared vocabulary must invalidate unchanged source bytes. The host
supplies that fingerprint; a source-owning sink receives the source path even
when two sources have identical content.

<a name="chunk-source-revision-boundary"></a><sub>[`tests/standalone.rs`](../../crates/x0k-folio-ingest/tests/standalone.rs) · `#source-revision-boundary`</sub>

```rust {#source-revision-boundary file="tests/standalone.rs"}
#[tokio::test]
async fn source_revision_changes_replay_unchanged_bytes_at_source_boundary() {
    struct Versioned(u8);
    impl DocumentSource for Versioned {
        fn discover(&self, root: &Path) -> Result<Vec<PathBuf>> { Lines.discover(root) }
        fn accepts(&self, path: &Path) -> bool { Lines.accepts(path) }
        fn revision_hash(&self, bytes: &[u8]) -> String { format!("{}:{}", self.0, blake3::hash(bytes)) }
        fn project(&self, path: &Path, bytes: &[u8], hash: &str) -> Result<Option<DocumentProjection>> { Lines.project(path, bytes, hash) }
    }
    struct SourceSink(Arc<Mutex<Vec<String>>>);
    impl FactSink for SourceSink {
        fn replace_source(&mut self, source: &str, _batches: &x0k_folio_ingest::backend::FactBatches,
            _prior: &[FactEntry], _cause: &str) -> Result<usize> {
            self.0.lock().unwrap().push(source.into()); Ok(1)
        }
        fn replace(&mut self, _: &str, _: &[FactEntry], _: &str) -> Result<usize> { panic!("entity delivery bypassed source") }
        fn retract(&mut self, _: &[FactEntry], _: &str) -> Result<usize> { panic!("retraction bypassed source") }
        fn retains_history(&self) -> bool { false }
    }
    let root = tempfile::tempdir().unwrap();
    let scratch = tempfile::tempdir().unwrap();
    let state = scratch.path().join("state");
    for name in ["a.txt", "b.txt"] { std::fs::write(root.path().join(name), "urn:same=value").unwrap(); }
    let calls = Arc::new(Mutex::new(Vec::new()));
    let sinks = Mutex::new(vec![Backend::new("source", SourceSink(calls.clone()))]);
    lifecycle::reconcile(&mut Versioned(1), root.path(), &state, &sinks, None).await.unwrap();
    assert_eq!(calls.lock().unwrap().len(), 2);
    lifecycle::reconcile(&mut Versioned(1), root.path(), &state, &sinks, None).await.unwrap();
    assert_eq!(calls.lock().unwrap().len(), 2);
    lifecycle::reconcile(&mut Versioned(2), root.path(), &state, &sinks, None).await.unwrap();
    assert_eq!(calls.lock().unwrap().len(), 4);
    let unique: std::collections::BTreeSet<_> = calls.lock().unwrap().iter().cloned().collect();
    assert_eq!(unique.len(), 2);
}
```

## Independent backend execution

A sink owns its worker. A blocked operation cannot retain the registry lock or
prevent an already admitted sibling from running. This test uses a finite gate
so a regression fails without leaving a permanently blocked test thread.

<a name="chunk-independent-delivery-regression"></a><sub>[`tests/standalone.rs`](../../crates/x0k-folio-ingest/tests/standalone.rs) · `#independent-delivery-regression`</sub>

```rust {#independent-delivery-regression file="tests/standalone.rs"}
#[test]
fn blocked_backend_does_not_own_registry_or_hold_healthy_delivery() {
    use std::sync::Condvar;
    struct Blocked { entered: std::sync::mpsc::Sender<()>, gate: Arc<(Mutex<bool>, Condvar)> }
    impl FactSink for Blocked {
        fn replace(&mut self, _: &str, _: &[FactEntry], _: &str) -> Result<usize> {
            self.entered.send(()).unwrap();
            let (lock, wake) = &*self.gate;
            let mut release = lock.lock().unwrap();
            while !*release { release = wake.wait(release).unwrap(); }
            Ok(1)
        }
        fn retract(&mut self, _: &[FactEntry], _: &str) -> Result<usize> { Ok(0) }
        fn retains_history(&self) -> bool { false }
    }
    let (entered, waiting) = std::sync::mpsc::channel();
    let gate = Arc::new((Mutex::new(false),Condvar::new()));
    let healthy = MemorySink::default();
    let backends = Arc::new(Mutex::new(vec![
        Backend::new("blocked",Blocked { entered,gate:gate.clone() }),
        Backend::new("healthy",healthy.clone()),
    ]));
    let scratch = tempfile::tempdir().unwrap();
    let path = scratch.path().join("state");
    let projection = Lines.project(Path::new("a.txt"),b"urn:example=value","revision").unwrap().unwrap();
    let shared = backends.clone();
    let operation = std::thread::spawn(move || {
        lifecycle::deliver(&mut Default::default(),&path,"source",&shared,Some(&projection)).unwrap();
    });
    waiting.recv_timeout(std::time::Duration::from_secs(3)).unwrap();
    let deadline = std::time::Instant::now()+std::time::Duration::from_millis(200);
    while healthy.facts().is_empty() && std::time::Instant::now()<deadline { std::thread::yield_now(); }
    let progressed = !healthy.facts().is_empty();
    let registry_free = backends.try_lock().is_ok();
    *gate.0.lock().unwrap() = true;
    gate.1.notify_all();
    operation.join().unwrap();
    assert!(progressed,"healthy sink waited for unrelated blocked sink");
    assert!(registry_free,"registry remained locked across sink effects");
}
```

## Worker ownership and completion

One thread owns each sink and admits at most one in-flight operation. A busy
sink retains its unacknowledged source in the durable journal; subsequent
sources can still reach healthy workers. All source effects are submitted before
the coordinator waits, using one completion deadline. Late acknowledgements are
not applied to newer revisions; the next reconciliation replays safely.

<a name="chunk-backend-worker"></a><sub>[`src/delivery.rs`](../../crates/x0k-folio-ingest/src/delivery.rs) · `#backend-worker`</sub>

```rust {#backend-worker file="src/delivery.rs"}
use std::{sync::{Arc, mpsc, atomic::{AtomicBool, Ordering}}, time::{Duration, Instant}};
use anyhow::{Result, anyhow};
use x0k_fact_projection::FactEntry;
use crate::backend::{FactBatches, FactSink};

enum Operation {
    Source(String, FactBatches, Vec<FactEntry>, String),
    Replace(String, Vec<FactEntry>, String),
    Retract(Vec<FactEntry>, String),
    Read(String),
    Identity,
}
enum Reply { Count(usize), Facts(Option<Vec<FactEntry>>), Identity(Option<String>) }
struct Request { operation: Operation, response: mpsc::SyncSender<Result<Reply>> }
/// One fixed worker and one in-flight operation; busy work never queues history.
#[derive(Clone)]
pub(crate) struct Worker {
    sender: mpsc::SyncSender<Request>,
    busy: Arc<AtomicBool>,
    history: bool,
}
pub(crate) struct Ticket(mpsc::Receiver<Result<Reply>>);
impl Ticket {
    pub(crate) fn identity_until(self, deadline: Instant) -> Result<Option<String>> {
        match self.0.recv_timeout(deadline.saturating_duration_since(Instant::now()))
            .map_err(|_| anyhow!("backend identity pending"))?? {
            Reply::Identity(identity) => Ok(identity),
            _ => Err(anyhow!("unexpected backend response")),
        }
    }

    pub(crate) fn facts_until(self, deadline: Instant) -> Result<Option<Vec<FactEntry>>> {
        match self.0.recv_timeout(deadline.saturating_duration_since(Instant::now()))
            .map_err(|_| anyhow!("backend read pending"))?? {
            Reply::Facts(facts) => Ok(facts),
            _ => Err(anyhow!("unexpected backend response")),
        }
    }
    pub(crate) fn count_until(self, deadline: Instant) -> Result<usize> {
        match self.0.recv_timeout(deadline.saturating_duration_since(Instant::now()))
            .map_err(|_| anyhow!("backend completion pending"))?? {
            Reply::Count(count) => Ok(count),
            _ => Err(anyhow!("unexpected backend response")),
        }
    }
}
impl Worker {
    pub(crate) fn new(mut sink: Box<dyn FactSink>) -> Self {
        let history = sink.retains_history();
        let (sender, receiver) = mpsc::sync_channel::<Request>(1);
        let busy = Arc::new(AtomicBool::new(false));
        let running = busy.clone();
        // A failed spawn drops receiver; every submission then fails closed.
        let _ = std::thread::Builder::new().name("folio-backend".into()).spawn(move || {
            while let Ok(request) = receiver.recv() {
                let result = match request.operation {
                    Operation::Source(source,batches,prior,cause) => sink.replace_source(&source,&batches,&prior,&cause).map(Reply::Count),
                    Operation::Replace(entity,facts,cause) => sink.replace(&entity,&facts,&cause).map(Reply::Count),
                    Operation::Retract(facts,cause) => sink.retract(&facts,&cause).map(Reply::Count),
                    Operation::Identity => sink.incarnation().map(Reply::Identity),
                    Operation::Read(cause) => sink.facts_caused_by(&cause).map(Reply::Facts),
                };
                running.store(false,Ordering::Release);
                let _ = request.response.send(result);
            }
        });
        Self { sender,busy,history }
    }
    fn submit(&self, operation: Operation) -> Result<Ticket> {
        anyhow::ensure!(self.busy.compare_exchange(false,true,Ordering::AcqRel,Ordering::Acquire).is_ok(),
            "backend still busy; source remains pending");
        let (response, receiver) = mpsc::sync_channel(1);
        if self.sender.try_send(Request { operation,response }).is_err() {
            self.busy.store(false,Ordering::Release);
            return Err(anyhow!("backend worker unavailable"));
        }
        Ok(Ticket(receiver))
    }
    pub(crate) fn source(&self, source:&str,batches:&FactBatches,prior:Vec<FactEntry>,cause:&str) -> Result<Ticket> {
        self.submit(Operation::Source(source.into(),batches.clone(),prior,cause.into()))
    }
    pub(crate) fn start_identity(&self) -> Result<Ticket> { self.submit(Operation::Identity) }
    pub(crate) fn start_read(&self,cause:&str) -> Result<Ticket> { self.submit(Operation::Read(cause.into())) }
    pub(crate) fn read(&self,cause:&str,grace:Duration) -> Result<Option<Vec<FactEntry>>> {
        let ticket = self.submit(Operation::Read(cause.into()))?;
        match ticket.0.recv_timeout(grace).map_err(|_| anyhow!("backend read pending"))?? {
            Reply::Facts(facts) => Ok(facts),
            _ => Err(anyhow!("unexpected backend response")),
        }
    }
}
impl FactSink for Worker {
    fn incarnation(&mut self) -> Result<Option<String>> {
        self.start_identity()?.identity_until(Instant::now()+Duration::from_secs(30))
    }

    fn replace_source(&mut self,source:&str,batches:&FactBatches,prior:&[FactEntry],cause:&str) -> Result<usize> {
        self.source(source,batches,prior.to_vec(),cause)?.count_until(Instant::now()+Duration::from_secs(30))
    }
    fn replace(&mut self,entity:&str,facts:&[FactEntry],cause:&str) -> Result<usize> {
        self.submit(Operation::Replace(entity.into(),facts.into(),cause.into()))?.count_until(Instant::now()+Duration::from_secs(30))
    }
    fn retract(&mut self,facts:&[FactEntry],cause:&str) -> Result<usize> {
        self.submit(Operation::Retract(facts.into(),cause.into()))?.count_until(Instant::now()+Duration::from_secs(30))
    }
    fn facts_caused_by(&self,cause:&str) -> Result<Option<Vec<FactEntry>>> { self.read(cause,Duration::from_secs(30)) }
    fn retains_history(&self) -> bool { self.history }
}
```

<a name="chunk-busy-backend-retry"></a><sub>[`tests/standalone.rs`](../../crates/x0k-folio-ingest/tests/standalone.rs) · `#busy-backend-retry`</sub>

```rust {#busy-backend-retry file="tests/standalone.rs"}
#[tokio::test]
async fn timed_out_backend_stays_pending_while_next_source_reaches_healthy_sink() {
    use std::sync::Condvar;
    struct Slow { view: MemorySink, gate: Arc<(Mutex<bool>,Condvar)> }
    impl FactSink for Slow {
        fn replace(&mut self, entity:&str,facts:&[FactEntry],cause:&str)->Result<usize> {
            let mut ready = self.gate.0.lock().unwrap();
            while !*ready { ready = self.gate.1.wait(ready).unwrap(); }
            self.view.replace(entity,facts,cause)
        }
        fn retract(&mut self,facts:&[FactEntry],cause:&str)->Result<usize> { self.view.retract(facts,cause) }
        fn retains_history(&self)->bool { false }
    }
    for reverse in [false,true] {
        let root = tempfile::tempdir().unwrap();
        let scratch = tempfile::tempdir().unwrap();
        let state_path = scratch.path().join("state");
        let slow = MemorySink::default(); let healthy = MemorySink::default();
        let gate = Arc::new((Mutex::new(false),Condvar::new()));
        let mut list = vec![
            Backend::new("slow",Slow {view:slow.clone(),gate:gate.clone()}).with_delivery_grace(std::time::Duration::from_millis(20)),
            Backend::new("healthy",healthy.clone()).with_delivery_grace(std::time::Duration::from_millis(20)),
        ];
        if reverse {list.reverse();}
        let backends = Mutex::new(list);
        for (name,text) in [("a.txt","urn:a=A"),("b.txt","urn:b=B")] {
            let path=root.path().join(name); std::fs::write(&path,text).unwrap();
            lifecycle::apply_path_change(&Lines,&path,&state_path,&backends).unwrap();
        }
        let pending = x0k_folio_ingest::checkpoint::load_state(&state_path).unwrap();
        assert_eq!(healthy.facts().len(),2);
        assert!(pending.files.values().all(|file| file.acked_by.contains("healthy") && !file.acked_by.contains("slow")));
        *gate.0.lock().unwrap()=true;gate.1.notify_all();
        let deadline=std::time::Instant::now()+std::time::Duration::from_secs(3);
        while slow.facts().is_empty() && std::time::Instant::now()<deadline {std::thread::yield_now();}
        assert_eq!(slow.facts().len(),1,"busy backend queued a second source");
        drop(backends);
        let restarted=Mutex::new(vec![Backend::new("slow",slow.clone()),Backend::new("healthy",healthy.clone())]);
        lifecycle::reconcile(&mut Lines,root.path(),&state_path,&restarted,None).await.unwrap();
        assert_eq!(slow.facts().len(),2);
        let caught_up=x0k_folio_ingest::checkpoint::load_state(&state_path).unwrap();
        assert!(caught_up.files.values().all(|file|file.acked_by.len()==2));
    }
}
```

## Store identity is part of an acknowledgement

The coordinator probes named stores before reusing scan acknowledgements.
An identity change discards only that store's old possible footprint and
replays the current projection. A failed identity read, or loss of an identity
once known, removes its acknowledgement and preserves recovery state. The
identity is checked again at delivery; scan skipping uses the identities read
at the beginning of that pass, not an atomic corpus-wide store snapshot.
Identity probes receive the first third of the overall delivery grace; legacy
reads may use the second third. The remainder stays available for healthy
source writes even when a sibling hangs while reporting its identity.

<a name="chunk-incarnation-regression"></a><sub>[`tests/standalone.rs`](../../crates/x0k-folio-ingest/tests/standalone.rs) · `#incarnation-regression`</sub>

```rust {#incarnation-regression file="tests/standalone.rs"}
#[tokio::test]
async fn identity_failure_and_loss_do_not_reuse_acks_and_new_store_replays_only_itself() {
    use std::sync::atomic::AtomicUsize;
    struct Identified { mode:Arc<AtomicUsize>,writes:Arc<AtomicUsize>,view:MemorySink }
    impl FactSink for Identified {
        fn retains_history(&self)->bool {false}
        fn incarnation(&mut self)->Result<Option<String>> {
            match self.mode.load(Ordering::SeqCst) {
                1=>anyhow::bail!("identity unavailable"),
                2=>Ok(None),
                3=>Ok(Some("new-store".into())),
                _=>Ok(Some("initial-store".into())),
            }
        }
        fn replace(&mut self,entity:&str,facts:&[FactEntry],cause:&str)->Result<usize> {
            self.writes.fetch_add(1,Ordering::SeqCst);
            self.view.replace(entity,facts,cause)
        }
        fn retract(&mut self,facts:&[FactEntry],cause:&str)->Result<usize> {self.view.retract(facts,cause)}
    }
    for reverse in [false,true] {
        let root=tempfile::tempdir().unwrap();
        let checkpoint_dir=tempfile::tempdir().unwrap();
        let checkpoint=checkpoint_dir.path().join("state");
        let source=root.path().join("source.txt");
        std::fs::write(&source,"urn:source=unchanged").unwrap();
        let mode=Arc::new(AtomicUsize::new(0));
        let writes=Arc::new(AtomicUsize::new(0));
        let healthy=Arc::new(AtomicUsize::new(0));
        let mut list=vec![
            Backend::new("replaceable",Identified {mode:mode.clone(),writes:writes.clone(),view:MemorySink::default()}),
            Backend::new("healthy",Identified {mode:Arc::new(AtomicUsize::new(0)),writes:healthy.clone(),view:MemorySink::default()})];
        if reverse {list.reverse();}
        let backends=Mutex::new(list);
        lifecycle::reconcile(&mut Lines,root.path(),&checkpoint,&backends,None).await.unwrap();
        for failed in [1,2] {
            mode.store(failed,Ordering::SeqCst);
            lifecycle::reconcile(&mut Lines,root.path(),&checkpoint,&backends,None).await.unwrap();
            let state=x0k_folio_ingest::checkpoint::load_state(&checkpoint).unwrap();
            assert_eq!(state.files[source.to_str().unwrap()].acked_by.iter().cloned().collect::<Vec<_>>(),vec!["healthy"]);
            assert_eq!(writes.load(Ordering::SeqCst),1);
        }
        mode.store(3,Ordering::SeqCst);
        lifecycle::reconcile(&mut Lines,root.path(),&checkpoint,&backends,None).await.unwrap();
        assert_eq!(writes.load(Ordering::SeqCst),2);
        assert_eq!(healthy.load(Ordering::SeqCst),1);
        let state=x0k_folio_ingest::checkpoint::load_state(&checkpoint).unwrap();
        assert_eq!(state.files[source.to_str().unwrap()].acked_by.len(),2);
        assert_eq!(state.recovery[source.to_str().unwrap()].revisions["replaceable"].incarnation.as_deref(),Some("new-store"));
    }
}
#[test]
fn blocked_identity_reserves_time_for_healthy_source_acknowledgement() {
    struct BlockedIdentity(std::sync::mpsc::Receiver<()>);
    impl FactSink for BlockedIdentity {
        fn incarnation(&mut self)->Result<Option<String>> {
            self.0.recv().map_err(|_|anyhow::anyhow!("identity released"))?;
            Ok(Some("blocked".into()))
        }
        fn replace(&mut self,_:&str,_:&[FactEntry],_:&str)->Result<usize> {Ok(0)}
        fn retract(&mut self,_:&[FactEntry],_:&str)->Result<usize> {Ok(0)}
        fn retains_history(&self)->bool {false}
    }
    for reverse in [false,true] {
        let (release,blocked)=std::sync::mpsc::channel();
        let healthy=MemorySink::default();
        let mut list=vec![Backend::new("blocked",BlockedIdentity(blocked)),Backend::new("healthy",healthy.clone())];
        if reverse {list.reverse();}
        let backends=Mutex::new(list.into_iter().map(|b|b.with_delivery_grace(std::time::Duration::from_millis(90))).collect());
        let scratch=tempfile::tempdir().unwrap();
        let mut state=lifecycle::IngesterState::default();
        for source in ["urn:first","urn:second"] {
            let desired=Lines.project(Path::new("source.txt"),format!("{source}=value").as_bytes(),"revision").unwrap().unwrap();
            lifecycle::deliver(&mut state,&scratch.path().join("state"),source,&backends,Some(&desired)).unwrap();
            assert_eq!(state.files[source].acked_by.iter().cloned().collect::<Vec<_>>(),vec!["healthy"]);
        }
        assert_eq!(healthy.facts().len(),2);
        drop(release);
    }
}

```
