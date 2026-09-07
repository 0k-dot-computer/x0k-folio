---
x0k:
  format: folio/v1
  id: x0k:implementation/folio/dialog
  type: implementation
  status: draft
  summary: Source-owned projections in a caller-selected local Dialog database.
  tangle:
    crate: x0k-folio-dialog
    root: src/lib.rs
  edges:
    motivated_by:
    - x0k:architecture/folio-backends
    - x0k:intent/c5ccd003-77d6-4b0d-8824-649f6221c259
---
# A local database for document facts

This standalone adapter opens a directory the caller selects. A single worker
owns the database and runtime. It admits eight queued requests, rejects excess
requests, and serializes writes and queries. Synchronous ingestion waits for its
reply; a slow sink can still delay sibling backends.

Source contribution records and their visible facts enter one Dialog commit.
A content hash attributes a revision; it never identifies the source that owns
a contribution. The database keeps a current view, not revision history.

## Worker boundary

Queries use native structural Dialog premises and rules. Row and time limits
bound normal consumption; cooperative cancellation is not a hard CPU or memory
limit. Dropping a query future closes its response channel. Query responses
carry typed values and complete Folio identifiers.

<a name="chunk-worker"></a><sub>[`src/lib.rs`](../../../../x0k-folio-dialog/src/lib.rs) · `#worker`</sub>

```rust {#worker file="src/lib.rs"}
use std::{path::Path, sync::{Arc, mpsc}};
use anyhow::{Result, anyhow};
use serde::{Deserialize, Serialize};
use tokio::sync::oneshot;
use x0k_folio_ingest::backend::{Backend, FactBatches, FactSink, QueryRow};
use x0k_fact_projection::FactEntry;

mod storage;
mod database;
mod query;

/// Native structural Dialog premises and rules with explicit output bindings.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct QueryRequest {
    pub premises: Vec<serde_json::Value>,
    pub rules: Vec<serde_json::Value>,
    pub select: Vec<String>,
    pub max_rows: usize,
    pub timeout_ms: u64,
}
impl Default for QueryRequest {
    fn default() -> Self { Self { premises: vec![], rules: vec![], select: vec![], max_rows: 1000, timeout_ms: 30_000 } }
}
/// Typed rows. Symbols are restored to complete Folio predicate identifiers.
#[derive(Debug)]
pub struct QueryResult {
    pub rows: Vec<QueryRow>,
    pub truncated: bool,
}

enum Request {
    Identity { response: mpsc::SyncSender<Result<Option<String>>> },
    Replace { source: String, batches: FactBatches, response: mpsc::SyncSender<Result<usize>> },
    Query { request: QueryRequest, response: oneshot::Sender<Result<QueryResult>> },
}

/// A cloneable handle to one exclusive local database worker.
#[derive(Clone)]
pub struct DialogBackend { sender: Arc<mpsc::SyncSender<Request>>, instance_id: Arc<str> }
impl DialogBackend {
    /// Open only the caller's directory. A worker owns its runtime and disk lock.
    pub fn open(directory: impl AsRef<Path>) -> Result<Self> { Self::open_mode(directory, false) }
    /// Read current committed snapshots while a writer remains open.
    pub fn open_reader(directory: impl AsRef<Path>) -> Result<Self> { Self::open_mode(directory, true) }
    fn open_mode(directory: impl AsRef<Path>, read_only: bool) -> Result<Self> {
        let directory = directory.as_ref().to_path_buf();
        let (sender, receiver) = mpsc::sync_channel(8);
        let (ready_tx, ready_rx) = mpsc::sync_channel(1);
        std::thread::Builder::new().name("folio-dialog".into()).spawn(move || {
            let runtime = match tokio::runtime::Builder::new_current_thread().enable_all().build() {
                Ok(runtime) => runtime,
                Err(error) => { let _ = ready_tx.send(Err(anyhow!(error))); return; }
            };
            let mut database = match runtime.block_on(database::Database::open(&directory, read_only)) {
                Ok(database) => { let _ = ready_tx.send(Ok(database.instance_id.clone())); database }
                Err(error) => { let _ = ready_tx.send(Err(error)); return; }
            };
            while let Ok(request) = receiver.recv() {
                match request {
                    Request::Identity { response } => {
                        let result=runtime.block_on(async {
                            database.check_instance()?;
                            database::Database::open_store(&database.storage,true).await?;
                            Ok(Some(database.instance_id.clone()))
                        });
                        let _=response.send(result);
                    }
                    Request::Replace { source, batches, response } => {
                        let result = runtime.block_on(database.replace(&source, &batches));
                        let _ = response.send(result);
                    }
                    Request::Query { request, mut response } => {
                        runtime.block_on(async {
                            let result = tokio::select! {
                                _ = response.closed() => return,
                                result = tokio::time::timeout(
                                    std::time::Duration::from_millis(request.timeout_ms),
                                    database.query(request)) => result.map_err(|_| anyhow!("query timed out")).and_then(|x| x),
                            };
                            let _ = response.send(result);
                        });
                    }
                }
            }
        })?;
        let instance_id = ready_rx.recv().map_err(|_| anyhow!("database worker stopped during open"))??;
        Ok(Self { sender: Arc::new(sender), instance_id: Arc::from(instance_id) })
    }
    /// Stable store incarnation. Hosts bind checkpoints to this identifier.
    pub fn instance_id(&self) -> &str { &self.instance_id }
    /// Build an independently named ingest sink sharing this database handle.
    pub fn backend(&self, name: impl Into<String>) -> Backend { Backend::new(name, self.clone()) }
    /// Evaluate native structural JSON. A full queue fails promptly.
    /// Dropping the future closes the response channel and cancels evaluation.
    pub async fn query(&self, request: QueryRequest) -> Result<QueryResult> {
        anyhow::ensure!((1..=100_000).contains(&request.max_rows), "max_rows must be 1..=100000");
        anyhow::ensure!((1..=300_000).contains(&request.timeout_ms), "timeout_ms must be 1..=300000");
        let timeout = std::time::Duration::from_millis(request.timeout_ms);
        let (response, receiver) = oneshot::channel();
        self.sender.try_send(Request::Query { request, response })
            .map_err(|_| anyhow!("database worker unavailable or request queue full"))?;
        tokio::time::timeout(timeout, receiver).await
            .map_err(|_| anyhow!("query timed out while queued or evaluating"))?
            .map_err(|_| anyhow!("database worker stopped"))?
    }
}
impl FactSink for DialogBackend {
    fn incarnation(&mut self) -> Result<Option<String>> {
        let (response,receiver)=mpsc::sync_channel(1);
        self.sender.try_send(Request::Identity {response})
            .map_err(|_|anyhow!("database worker unavailable or request queue full"))?;
        receiver.recv().map_err(|_|anyhow!("database worker stopped"))?
    }

    fn replace_source(&mut self, source_key: &str, batches: &FactBatches, _prior: &[FactEntry], _cause: &str) -> Result<usize> {
        let (response, receiver) = mpsc::sync_channel(1);
        self.sender.try_send(Request::Replace { source: source_key.into(), batches: batches.clone(), response })
            .map_err(|_| anyhow!("database worker unavailable or request queue full"))?;
        receiver.recv().map_err(|_| anyhow!("database worker stopped"))?
    }
    fn replace(&mut self, _entity: &str, _facts: &[FactEntry], _cause: &str) -> Result<usize> {
        anyhow::bail!("Dialog requires source-qualified delivery")
    }
    fn retract(&mut self, _facts: &[FactEntry], _cause: &str) -> Result<usize> {
        anyhow::bail!("Dialog requires source-qualified delivery")
    }
    fn retains_history(&self) -> bool { false }
}
```

## Durable root publication

Each block is written and synced before atomic rename; the directory is synced
before success. The same operation publishes the upstream root after its blocks.
An exclusive directory lock prevents two independent handles from opening stale
roots and overwriting each other. Clone one handle for multiple named sinks.

This adds filesystem durability to the upstream storage contract without changing
upstream code. Work includes the blocks written by Dialog; no history-independent
I/O bound is claimed for the underlying database.

<a name="chunk-durable-storage"></a><sub>[`src/storage.rs`](../../../../x0k-folio-dialog/src/storage.rs) · `#durable-storage`</sub>

```rust {#durable-storage file="src/storage.rs"}
use std::{fs::{self, File, OpenOptions}, io::Write, path::{Path, PathBuf}, sync::Arc};
use anyhow::{Context, Result};
use dialog_storage::{Blake3Hash, DialogStorageError, StorageBackend};
use fs2::FileExt;

/// All writers share the worker; the advisory lock excludes a second process.
#[derive(Clone)]
pub struct DurableStorage {
    root: PathBuf, _lock: Option<Arc<File>>, read_only: bool,
    #[cfg(test)]
    pub(crate) fail_root: Arc<std::sync::atomic::AtomicBool>,
}
impl DurableStorage {
    pub fn open(root: &Path, read_only: bool) -> Result<Self> {
        if read_only {
            anyhow::ensure!(root.is_dir(), "database directory does not exist");
            return Ok(Self { root: root.canonicalize()?, _lock: None, read_only,
                #[cfg(test)] fail_root: Arc::new(std::sync::atomic::AtomicBool::new(false)),
            });
        }
        // Persist every newly created ancestor before allowing any database writes.
        let mut missing = vec![];
        let mut next = root;
        while !next.exists() {
            missing.push(next.to_path_buf());
            next = next.parent().context("database directory has no existing ancestor")?;
        }
        for path in missing.into_iter().rev() {
            fs::create_dir(&path)?;
            File::open(&path)?.sync_all()?;
            File::open(path.parent().context("missing parent")?)?.sync_all()?;
        }
        let root = root.canonicalize()?;
        let lock = OpenOptions::new().read(true).write(true).create(true).truncate(false).open(root.join("writer.lock"))?;
        lock.try_lock_exclusive().context("database already open by another writer")?;
        File::open(&root)?.sync_all()?;
        Ok(Self { root, _lock: Some(Arc::new(lock)), read_only,
            #[cfg(test)] fail_root: Arc::new(std::sync::atomic::AtomicBool::new(false)),
        })
    }
    fn path(&self, key: &Blake3Hash) -> PathBuf {
        self.root.join(key.iter().map(|byte| format!("{byte:02x}")).collect::<String>())
    }
}
#[async_trait::async_trait]
impl StorageBackend for DurableStorage {
    type Key = Blake3Hash;
    type Value = Vec<u8>;
    type Error = DialogStorageError;
    async fn get(&self, key: &Blake3Hash) -> Result<Option<Vec<u8>>, Self::Error> {
        match fs::read(self.path(key)) {
            Ok(bytes) => Ok(Some(bytes)),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
            Err(error) => Err(DialogStorageError::Storage(error.to_string())),
        }
    }
    async fn set(&mut self, key: Blake3Hash, value: Vec<u8>) -> Result<(), Self::Error> {
        (|| -> Result<()> {
            anyhow::ensure!(!self.read_only, "read-only database");
            #[cfg(test)]
            if key == dialog_artifacts::make_reference(b"folio-v1")
                && self.fail_root.swap(false, std::sync::atomic::Ordering::SeqCst) {
                anyhow::bail!("injected root publication failure");
            }
            let mut file = tempfile::NamedTempFile::new_in(&self.root)?;
            file.write_all(&value)?;
            file.as_file().sync_all()?;
            file.persist(self.path(&key))?;
            File::open(&self.root)?.sync_all()?;
            Ok(())
        })().map_err(|error| DialogStorageError::Storage(error.to_string()))
    }
}
```

## Package boundary

Only the public ingest and projection contracts accompany pinned upstream Dialog.
Publication preserves the repository's existing license metadata.

<a name="chunk-manifest"></a><sub>[`Cargo.toml`](../../../../x0k-folio-dialog/Cargo.toml) · `#manifest`</sub>

```toml {#manifest file="Cargo.toml"}
[package]
name = "x0k-folio-dialog"
version = "0.1.0"
edition = { workspace = true }
license = "MIT"
description = "Source-owned local Dialog database adapter for Folio"
rust-version = { workspace = true }
repository = "https://github.com/0k-dot-computer/x0k-folio"
readme = "../README.md"
keywords = ["literate-programming", "tangle", "markdown", "documentation"]
[dependencies]
x0k-folio-ingest = { path = "../x0k-folio-ingest" , version = "0.1.0" }
x0k-fact-projection = { path = "../x0k-fact-projection" , version = "0.1.0" }
anyhow = "1"
serde = { version = "1", features = ["derive"] }
serde_json = "1"
tokio = { version = "1", features = ["rt", "sync", "time", "macros"] }
futures-util = "0.3"
async-trait = "0.1"
blake3 = "1"
tempfile = "3"
fs2 = "0.4"
uuid = { version = "1", features = ["v4"] }
dialog-artifacts = { version = "0.1.0", git = "https://github.com/dialog-db/dialog-db", rev = "3fac7ad3e691d401fb5c18c18ffb23de74342742" }
dialog-query = { version = "0.1.0", git = "https://github.com/dialog-db/dialog-db", rev = "3fac7ad3e691d401fb5c18c18ffb23de74342742" }
dialog-storage = { version = "0.1.0", git = "https://github.com/dialog-db/dialog-db", rev = "3fac7ad3e691d401fb5c18c18ffb23de74342742" }
dialog-capability = { version = "0.1.0", git = "https://github.com/dialog-db/dialog-db", rev = "3fac7ad3e691d401fb5c18c18ffb23de74342742" }
```

## Source supports and native values

A source owns its serialized canonical contribution cells. Each cell also indexes
its supporting sources. Removal retracts the visible fact only when no other
source supports it. Contribution metadata, dictionary entries, and visible
projection share one commit root. The cost is the changed source footprint and
support lookups for its cells; dictionary and empty support metadata remain.

<a name="chunk-source-support"></a><sub>[`src/database.rs`](../../../../x0k-folio-dialog/src/database.rs) · `#source-support`</sub>

```rust {#source-support file="src/database.rs"}
use std::{collections::{BTreeMap, BTreeSet}, path::Path};
use anyhow::{Result, Context};
use dialog_artifacts::{Artifact, ArtifactSelector, ArtifactStoreMut, Artifacts, Entity, Instruction, Value};
use futures_util::TryStreamExt;
use x0k_fact_projection::{FactEntry, FactValue, payload::FactPayload};
use x0k_folio_ingest::backend::FactBatches;
use crate::storage::DurableStorage;

pub(crate) struct Database {
    pub(crate) store: Artifacts<DurableStorage>,
    pub(crate) storage: DurableStorage,
    pub(crate) read_only: bool,
    pub(crate) instance_id: String,
    marker_path: std::path::PathBuf,
}
fn decode_marker(bytes: &[u8]) -> Result<String> {
    let marker: serde_json::Value = serde_json::from_slice(bytes)?;
    anyhow::ensure!(marker.get("version").and_then(|v| v.as_u64()) == Some(1), "unsupported database format");
    let id = marker.get("instance_id").and_then(|v| v.as_str()).context("missing database incarnation")?;
    uuid::Uuid::parse_str(id)?;
    Ok(id.into())
}
pub(crate) fn digest(bytes: &[u8]) -> String { blake3::hash(bytes).to_hex().to_string() }
pub(crate) fn relation(full: &str) -> String { format!("f/p{}", &digest(full.as_bytes())[..56]) }
fn identity(kind: &str, bytes: &[u8]) -> Result<Entity> {
    Ok(format!("urn:folio:{kind}:{}", digest(bytes)).parse()?)
}
fn artifact(entity: Entity, predicate: &str, value: Value) -> Result<Artifact> {
    Ok(Artifact { of: entity, the: predicate.parse()?, is: value, cause: None })
}
fn normalized(fact: &FactEntry) -> FactEntry {
    let mut fact = fact.clone();
    fact.cause = None;
    fact
}
impl Database {
    pub(crate) async fn open(path: &Path, read_only: bool) -> Result<Self> {
        let storage = DurableStorage::open(path, read_only)?;
        use dialog_storage::StorageBackend;
        let marker_path = path.canonicalize()?.join("store.json");
        let mut store = Self::open_store(&storage, read_only).await?;
        let instance_id = match std::fs::read(&marker_path) {
            Ok(bytes) => decode_marker(&bytes)?,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                anyhow::ensure!(!read_only, "database instance marker is missing");
                anyhow::ensure!(storage.get(&dialog_artifacts::make_reference(b"folio-v1")).await?.is_none(),
                    "database has root but no instance marker; rebuild into a fresh directory");
                store.commit(futures_util::stream::empty()).await?;
                let id = uuid::Uuid::new_v4().to_string();
                let bytes = serde_json::to_vec(&serde_json::json!({"version":1,"instance_id":id}))?;
                use std::io::Write;
                let mut marker = tempfile::NamedTempFile::new_in(marker_path.parent().context("marker parent")?)?;
                marker.write_all(&bytes)?;
                marker.as_file().sync_all()?;
                marker.persist(&marker_path)?;
                std::fs::File::open(marker_path.parent().context("marker parent")?)?.sync_all()?;
                id
            }
            Err(error) => return Err(error.into()),
        };
        // A previously initialized store must never become an empty store.
        anyhow::ensure!(storage.get(&dialog_artifacts::make_reference(b"folio-v1")).await?.is_some(),
            "initialized database root is missing; rebuild into a fresh directory");
        Ok(Self { store, storage, read_only, instance_id, marker_path })
    }
    pub(crate) fn check_instance(&self) -> Result<()> {
        anyhow::ensure!(decode_marker(&std::fs::read(&self.marker_path)?)? == self.instance_id,
            "database incarnation changed while open");
        Ok(())
    }
    pub(crate) async fn open_store(storage: &DurableStorage, require_root: bool) -> Result<Artifacts<DurableStorage>> {
        use dialog_storage::StorageBackend;
        let root = storage.get(&dialog_artifacts::make_reference(b"folio-v1")).await?;
        anyhow::ensure!(!require_root || root.is_some(), "database has no committed root");
        if let Some(bytes) = root {
            anyhow::ensure!(bytes.len() == 32, "invalid database root pointer");
            if bytes != dialog_artifacts::NULL_REVISION_HASH {
                let hash: [u8;32] = bytes.try_into().map_err(|_| anyhow::anyhow!("invalid root length"))?;
                anyhow::ensure!(storage.get(&hash).await?.is_some(), "database root block is missing");
            }
        }
        Ok(Artifacts::open("folio-v1".into(), storage.clone()).await?)
    }
    pub(crate) async fn values(&self, entity: &Entity, predicate: &str) -> Result<Vec<Value>> {
        let selector = ArtifactSelector::new().of(entity.clone()).the(predicate.parse()?);
        Ok(self.store.select(selector).and_then(|a| async move { Ok(a.to_owned()?.is) }).try_collect().await?)
    }
    async fn check_identity(&self, entity: &Entity, predicate: &str, expected: &Value) -> Result<()> {
        let values = self.values(entity, predicate).await?;
        anyhow::ensure!(values.iter().all(|v| v == expected), "stored identity collision or corruption at {entity}");
        Ok(())
    }
    async fn dictionary(&self, full: &str, instructions: &mut Vec<Instruction>) -> Result<()> {
        let short = relation(full);
        let key: Entity = format!("urn:folio:predicate:{short}").parse()?;
        let value = Value::String(full.into());
        self.check_identity(&key, "folio/predicate", &value).await?;
        instructions.push(Instruction::Assert(artifact(key, "folio/predicate", value)?));
        Ok(())
    }
    pub(crate) async fn full_relation(&self, short: &str) -> Result<String> {
        let key: Entity = format!("urn:folio:predicate:{short}").parse()?;
        let values = self.values(&key, "folio/predicate").await?;
        match values.as_slice() {
            [Value::String(full)] if relation(full) == short => Ok(full.clone()),
            _ => anyhow::bail!("missing or corrupt predicate dictionary for {short}"),
        }
    }
    fn visible(&self, fact: &FactEntry) -> Result<Artifact> {
        let value = match &fact.value {
            FactValue::Text(x) => Value::String(x.clone()),
            FactValue::EntityRef(x) => Value::Entity(x.parse()?),
            FactValue::Boolean(x) => Value::Boolean(*x),
            FactValue::UnsignedInt(x) => Value::UnsignedInt(*x),
            FactValue::SignedInt(x) => Value::SignedInt(*x),
            FactValue::Float(x) => Value::Float(*x),
            FactValue::Bytes(x) => Value::Bytes(x.clone()),
            FactValue::Record(x) => Value::Record(x.clone()),
            FactValue::Symbol(x) => Value::Symbol(relation(x).parse()?),
            FactValue::Retracted(_) => anyhow::bail!("source projection must contain current facts, not retraction markers"),
        };
        artifact(fact.entity.parse()?, &relation(&fact.predicate), value)
    }
    /// Persist support changes and visible facts under one published root.
    pub(crate) async fn replace(&mut self, source: &str, batches: &FactBatches) -> Result<usize> {
        anyhow::ensure!(!self.read_only, "read-only database");
        self.check_instance()?;
        let source_id = identity("source", source.as_bytes())?;
        let source_value = Value::String(source.into());
        self.check_identity(&source_id, "folio/source", &source_value).await?;
        let mut old = BTreeMap::new();
        for value in self.values(&source_id, "folio/contribution").await? {
            let Value::Bytes(bytes) = value else { anyhow::bail!("invalid source contribution"); };
            let fact = FactPayload::from_bytes(&bytes)?.into_fact();
            anyhow::ensure!(FactPayload::from_fact(&normalized(&fact)).to_bytes() == bytes, "noncanonical source contribution");
            old.insert(bytes, fact);
        }
        let mut desired = BTreeMap::new();
        for (_, facts) in batches {
            for fact in facts {
                let fact = normalized(fact);
                anyhow::ensure!(!matches!(fact.value, FactValue::Retracted(_)), "retraction marker in desired source");
                desired.insert(FactPayload::from_fact(&fact).to_bytes(), fact);
            }
        }
        let mut instructions = vec![Instruction::Assert(artifact(source_id.clone(), "folio/source", source_value)?)];
        let cells: BTreeSet<_> = old.keys().chain(desired.keys()).cloned().collect();
        let mut pending_cells = BTreeMap::new();
        let mut predicates = BTreeMap::new();
        for (bytes, fact) in old.iter().chain(desired.iter()) {
            let cell = digest(bytes);
            if let Some(previous) = pending_cells.insert(cell, bytes) {
                anyhow::ensure!(previous == bytes, "fact collision within pending transaction");
            }
            let mut names = vec![&fact.predicate];
            if let FactValue::Symbol(symbol) = &fact.value { names.push(symbol); }
            for full in names {
                if let Some(previous) = predicates.insert(relation(full), full) {
                    anyhow::ensure!(previous == full, "predicate collision within pending transaction");
                }
            }
        }
        for full in predicates.values() { self.dictionary(full, &mut instructions).await?; }
        let mut touched = 0;
        for bytes in cells {
            let fact = desired.get(&bytes).or_else(|| old.get(&bytes)).context("missing fact")?;
            let cell = identity("fact", &bytes)?;
            let encoded = Value::Bytes(bytes.clone());
            self.check_identity(&cell, "folio/fact", &encoded).await?;
            let support = artifact(cell.clone(), "folio/support", Value::Entity(source_id.clone()))?;
            let contribution = artifact(source_id.clone(), "folio/contribution", encoded.clone())?;
            let visible = self.visible(fact)?;
            if desired.contains_key(&bytes) {
                instructions.push(Instruction::Assert(artifact(cell, "folio/fact", encoded)?));
                instructions.push(Instruction::Assert(support));
                instructions.push(Instruction::Assert(contribution));
                instructions.push(Instruction::Assert(visible));
            } else {
                let others = self.values(&cell, "folio/support").await?;
                anyhow::ensure!(others.iter().all(|x| matches!(x, Value::Entity(_))), "invalid source support");
                if !others.iter().any(|x| x != &Value::Entity(source_id.clone())) {
                    instructions.push(Instruction::Retract(visible));
                }
                instructions.push(Instruction::Retract(support));
                instructions.push(Instruction::Retract(contribution));
            }
            touched += 1;
        }
        // Failed publication is retried from durable source contributions; never
        // infer ownership from revision causes or a host's previous projection.
        self.store.commit(futures_util::stream::iter(instructions)).await?;
        Ok(touched)
    }
    pub(crate) async fn value(&self, value: Value) -> Result<FactValue> {
        Ok(match value {
            Value::String(x) => FactValue::Text(x),
            Value::Entity(x) => FactValue::EntityRef(x.to_string()),
            Value::Boolean(x) => FactValue::Boolean(x),
            Value::UnsignedInt(x) => FactValue::UnsignedInt(x),
            Value::SignedInt(x) => FactValue::SignedInt(x),
            Value::Float(x) => FactValue::Float(x),
            Value::Bytes(x) => FactValue::Bytes(x),
            Value::Record(x) => FactValue::Record(x),
            Value::Symbol(x) => FactValue::Symbol(self.full_relation(&x.to_string()).await?),
        })
    }
}
```

## Native query execution

Descriptors use complete Folio predicate names at the API. Their internal
relations fit upstream's identifier grammar. Returned symbols use the persistent
dictionary to recover full identifiers. Native entity identifiers remain intact.

<a name="chunk-native-query"></a><sub>[`src/query.rs`](../../../../x0k-folio-dialog/src/query.rs) · `#native-query`</sub>

```rust {#native-query file="src/query.rs"}
use anyhow::Result;
use async_trait::async_trait;
use dialog_artifacts::{ArtifactSelector, ArtifactStream, DialogArtifactsError, Select, inspect::Load, selector::Constrained};
use dialog_capability::Provider;
use dialog_query::{ConceptDescriptor, Environment, Match, Premise, RuleRegistry, Term, types::Any};
use dialog_query::{concept::query::ConceptRules, error::EvaluationError, planner::Planner, source::SelectRules, rule::deductive::DeductiveRule};
use dialog_storage::{Blake3Hash, StorageBackend};
use futures_util::TryStreamExt;
use crate::{database::{Database, relation}, QueryRequest, QueryResult};

struct Source<'a> { database: &'a Database, rules: RuleRegistry }
#[async_trait]
impl<'a> Provider<Select<'a>> for Source<'a> {
    async fn execute(&self, input: ArtifactSelector<Constrained>) -> Result<ArtifactStream<'a>, DialogArtifactsError> {
        Ok(Box::pin(self.database.store.select(input)))
    }
}
#[async_trait]
impl Provider<SelectRules> for Source<'_> {
    async fn execute(&self, input: ConceptDescriptor) -> Result<ConceptRules, EvaluationError> { self.rules.acquire(&input) }
}
#[async_trait]
impl Provider<Load> for Source<'_> {
    async fn execute(&self, input: Blake3Hash) -> Result<Option<Vec<u8>>, DialogArtifactsError> {
        Ok(self.database.storage.get(&input).await?)
    }
}

/// Only descriptor keys identify predicates; ordinary text values stay text.
fn map_descriptor(value: &mut serde_json::Value, names: &mut std::collections::BTreeMap<String,String>) -> Result<()> {
    if let Some(object) = value.as_object_mut() {
        if let Some(serde_json::Value::String(full)) = object.get_mut("the") {
            let short = relation(full);
            if let Some(previous) = names.insert(short.clone(), full.clone()) {
                anyhow::ensure!(previous == *full, "predicate collision within query");
            }
            *full = short;
        }
        if let Some(fields) = object.get_mut("with").and_then(|value| value.as_object_mut()) {
            for field in fields.values_mut() { map_descriptor(field, names)?; }
        }
    }
    Ok(())
}
fn map_descriptors(value: &mut serde_json::Value, names: &mut std::collections::BTreeMap<String,String>) -> Result<()> {
    match value {
        serde_json::Value::Object(object) => {
            for key in ["assert", "deduce"] {
                if let Some(descriptor) = object.get_mut(key) { map_descriptor(descriptor, names)?; }
            }
            for key in ["when", "unless"] {
                if let Some(premises) = object.get_mut(key) { map_descriptors(premises, names)?; }
            }
        }
        serde_json::Value::Array(values) => for value in values { map_descriptors(value, names)?; },
        _ => {}
    }
    Ok(())
}
impl Database {
    pub(crate) async fn query(&mut self, mut request: QueryRequest) -> Result<QueryResult> {
        self.check_instance()?;
        if self.read_only {
            self.store = Self::open_store(&self.storage, true).await?;
        }
        let mut names = std::collections::BTreeMap::new();
        let mut rules = RuleRegistry::new();
        for rule in &mut request.rules {
            map_descriptors(rule, &mut names)?;
            rules.register(serde_json::from_value::<DeductiveRule>(rule.clone())?)?;
        }
        anyhow::ensure!(rules.validate()?.is_empty(), "invalid recursive rule program");
        let mut premises = vec![];
        for premise in &mut request.premises {
            map_descriptors(premise, &mut names)?;
            premises.push(if let Some(unless) = premise.get("unless") {
                Premise::Unless(dialog_query::Negation(serde_json::from_value(unless.clone())?))
            } else { Premise::Assert(serde_json::from_value(premise.clone())?) });
        }
        for (short, full) in &names {
            let key = format!("urn:folio:predicate:{short}").parse()?;
            let values = self.values(&key, "folio/predicate").await?;
            anyhow::ensure!(values.iter().all(|v| v == &dialog_artifacts::Value::String(full.clone())),
                "stored predicate collision while planning query");
        }
        let source = Source { database: self, rules };
        let plan = Planner::from(premises).plan(&Environment::new())?;
        let stream = plan.evaluate(Match::new().seed(), &source);
        tokio::pin!(stream);
        let mut rows = vec![];
        let mut truncated = false;
        while let Some(matched) = stream.try_next().await? {
            if rows.len() == request.max_rows { truncated = true; break; }
            let mut row = std::collections::BTreeMap::new();
            for name in &request.select {
                let binding = matched.lookup(&Term::<Any>::var(name))?;
                if binding.is_present() {
                    row.insert(name.clone(), self.value(binding.content()?).await?);
                }
            }
            rows.push(row);
        }
        Ok(QueryResult { rows, truncated })
    }
}
```

## Store-level regressions

These tests cross the public handle, native query planner, durable root, and
reader/writer boundary. Identical content contributes from two distinct sources;
removing one does not erase the other.

<a name="chunk-store-regressions"></a><sub>[`tests/store.rs`](../../../../x0k-folio-dialog/tests/store.rs) · `#store-regressions`</sub>

```rust {#store-regressions file="tests/store.rs"}
use anyhow::Result;
use x0k_folio_dialog::{DialogBackend, QueryRequest};
use x0k_folio_ingest::backend::FactSink;
use x0k_fact_projection::{FactEntry, FactValue};

fn fact(value: FactValue) -> FactEntry {
    FactEntry { entity: "https://example.org/document/one".into(),
        predicate: format!("https://example.org/ontology/{}", "a-long-predicate/".repeat(8)),
        value, cause: Some("identical-content".into()) }
}
fn request(predicate: &str) -> QueryRequest {
    QueryRequest {
        premises: vec![serde_json::json!({
            "assert": { "with": { "value": { "the": predicate, "cardinality": "many" } } },
            "where": { "this": {"?":{"name":"entity"}}, "value": {"?":{"name":"value"}} }
        })],
        select: vec!["entity".into(), "value".into()], ..Default::default()
    }
}
#[tokio::test]
async fn native_values_source_support_update_delete_reopen_and_live_reader() -> Result<()> {
    let directory = tempfile::tempdir()?;
    let path = directory.path().join("database");
    let mut writer = DialogBackend::open(&path)?;
    assert!(DialogBackend::open(&path).is_err());
    let values = vec![
        FactValue::Text("urn:text-is-not-reference".into()), FactValue::EntityRef("urn:reference".into()),
        FactValue::Boolean(true), FactValue::UnsignedInt(u128::MAX), FactValue::SignedInt(i128::MIN),
        FactValue::Float(1.25), FactValue::Bytes(vec![0,255]), FactValue::Record(vec![1,255]),
        FactValue::Symbol("https://other.example/ontology/full-symbol".into()),
    ];
    let facts: Vec<_> = values.iter().cloned().map(fact).collect();
    let batches = vec![(facts[0].entity.clone(), facts.clone())];
    writer.replace_source("/source/a", &batches, &[], "same-content")?;
    writer.replace_source("/source/b", &batches, &[], "same-content")?;
    let reader = DialogBackend::open_reader(&path)?;
    let response = reader.query(request(&facts[0].predicate)).await?;
    assert_eq!(response.rows.len(), values.len());
    for value in &values { assert!(response.rows.iter().any(|row| row.get("value") == Some(value)), "{value:?}"); }
    assert!(response.rows.iter().all(|r| r.get("entity") == Some(&FactValue::EntityRef(facts[0].entity.clone()))));
    writer.replace_source("/source/a", &vec![], &facts, "deleted")?;
    assert_eq!(reader.query(request(&facts[0].predicate)).await?.rows.len(), 9);
    let updated = vec![(facts[0].entity.clone(), vec![fact(FactValue::Text("changed".into()))])];
    writer.replace_source("/source/b", &updated, &facts, "new-revision")?;
    let response = reader.query(request(&facts[0].predicate)).await?;
    assert_eq!(response.rows.len(), 1);
    assert_eq!(response.rows[0]["value"], FactValue::Text("changed".into()));
    // A lost host acknowledgement replays against source state already committed.
    writer.replace_source("/source/b", &updated, &facts, "new-revision")?;
    assert_eq!(reader.query(request(&facts[0].predicate)).await?.rows.len(), 1);
    // Readers never expose a write path.
    let mut read_only = reader.clone();
    assert!(read_only.replace_source("/illegal", &batches, &[], "x").is_err());
    drop(writer);
    // Worker stops after last sending handle disappears; poll lock acquisition,
    // rather than assuming the detached thread has already released the lock.
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
    let mut reopened = loop {
        anyhow::ensure!(std::time::Instant::now() < deadline, "writer lock did not release");
        match DialogBackend::open(&path) {
            Ok(writer) => break writer,
            Err(_) => tokio::time::sleep(std::time::Duration::from_millis(5)).await,
        }
    };
    assert_eq!(reader.query(request(&facts[0].predicate)).await?.rows.len(), 1);
    reopened.replace_source("/source/b", &vec![], &facts, "deleted")?;
    assert!(reader.query(request(&facts[0].predicate)).await?.rows.is_empty());
    Ok(())
}
#[tokio::test]
async fn native_query_limits_and_read_only_missing_directory() -> Result<()> {
    let directory = tempfile::tempdir()?;
    assert!(DialogBackend::open_reader(directory.path().join("missing")).is_err());
    assert!(!directory.path().join("missing").exists());
    let mut writer = DialogBackend::open(directory.path())?;
    let facts: Vec<_> = (0..3).map(|n| fact(FactValue::UnsignedInt(n))).collect();
    writer.replace_source("a", &vec![(facts[0].entity.clone(),facts.clone())], &[], "r")?;
    let mut query = request(&facts[0].predicate);
    query.max_rows = 2;
    let result = writer.query(query.clone()).await?;
    assert_eq!(result.rows.len(), 2); assert!(result.truncated);
    query.max_rows = 0; assert!(writer.query(query).await.is_err());
    Ok(())
}
```

## Failed publication and dictionary integrity

The injected fault is at the actual storage publication call, after content
blocks have been written and before the durable root advances. Reopening must
see the old support and projection together; unchanged retry then converges.

<a name="chunk-publication-regressions"></a><sub>[`src/database.rs`](../../../../x0k-folio-dialog/src/database.rs) · `#publication-regressions`</sub>

```rust {#publication-regressions file="src/database.rs"}
#[cfg(test)]
mod tests {
    use super::*;
    #[tokio::test]
    async fn failed_root_publication_reopens_old_support_then_retries() -> Result<()> {
        let directory = tempfile::tempdir()?;
        let mut db = Database::open(directory.path(), false).await?;
        let make = |text: &str| FactEntry { entity: "urn:example".into(), predicate: "https://example.org/name".into(),
            value: FactValue::Text(text.into()), cause: None };
        let old = make("old"); let new = make("new");
        db.replace("a", &vec![(old.entity.clone(),vec![old.clone()])]).await?;
        db.storage.fail_root.store(true, std::sync::atomic::Ordering::SeqCst);
        assert!(db.replace("a", &vec![(new.entity.clone(),vec![new.clone()])]).await.is_err());
        drop(db);
        let mut db = Database::open(directory.path(), false).await?;
        let source = identity("source", b"a")?;
        assert_eq!(db.values(&old.entity.parse()?, &relation(&old.predicate)).await?, vec![Value::String("old".into())]);
        assert_eq!(db.values(&source,"folio/contribution").await?,
            vec![Value::Bytes(FactPayload::from_fact(&old).to_bytes())]);
        db.replace("a", &vec![(new.entity.clone(),vec![new.clone()])]).await?;
        assert_eq!(db.values(&source,"folio/contribution").await?,
            vec![Value::Bytes(FactPayload::from_fact(&new).to_bytes())]);
        db.replace("a", &vec![]).await?;
        assert!(db.values(&new.entity.parse()?, &relation(&new.predicate)).await?.is_empty());
        assert!(db.values(&source,"folio/contribution").await?.is_empty());
        Ok(())
    }
    #[tokio::test]
    async fn corrupt_root_reference_does_not_open_an_empty_store() -> Result<()> {
        use dialog_storage::StorageBackend;
        let directory = tempfile::tempdir()?;
        let mut storage = DurableStorage::open(directory.path(), false)?;
        storage.set(dialog_artifacts::make_reference(b"folio-v1"), vec![42;32]).await?;
        drop(storage);
        assert!(Database::open(directory.path(), true).await.is_err());
        assert!(Database::open(directory.path(), false).await.is_err());
        Ok(())
    }
    #[tokio::test]
    async fn dictionary_mismatch_fails_before_source_or_visible_writes() -> Result<()> {
        let directory = tempfile::tempdir()?;
        let mut db = Database::open(directory.path(), false).await?;
        let predicate = "https://example.org/predicate";
        let key: Entity = format!("urn:folio:predicate:{}", relation(predicate)).parse()?;
        let poisoned = artifact(key, "folio/predicate", Value::String("different full identity".into()))?;
        db.store.commit(futures_util::stream::iter(vec![Instruction::Assert(poisoned)])).await?;
        let before = db.store.revision().await?;
        let fact = FactEntry { entity:"urn:item".into(),predicate:predicate.into(),value:FactValue::Boolean(true),cause:None };
        assert!(db.replace("source", &vec![(fact.entity.clone(),vec![fact])]).await.is_err());
        assert_eq!(before,db.store.revision().await?);
        assert!(db.values(&identity("source",b"source")?,"folio/source").await?.is_empty());
        Ok(())
    }
}
```

<a name="chunk-recursive-rule-regression"></a><sub>[`tests/store.rs`](../../../../x0k-folio-dialog/tests/store.rs) · `#recursive-rule-regression`</sub>

```rust {#recursive-rule-regression file="tests/store.rs"}
#[tokio::test]
async fn native_recursive_rules_reach_fixpoint_over_source_facts() -> Result<()> {
    let directory = tempfile::tempdir()?;
    let mut db = DialogBackend::open(directory.path())?;
    let parent = "https://example.org/family/parent";
    let ancestor = "https://example.org/family/ancestor";
    let facts: Vec<_> = [("urn:a","urn:b"),("urn:b","urn:c"),("urn:c","urn:a")].into_iter()
        .map(|(entity,target)| FactEntry { entity:entity.into(),predicate:parent.into(),
            value:FactValue::EntityRef(target.into()),cause:None }).collect();
    db.replace_source("family", &vec![("urn:family".into(),facts)], &[], "revision")?;
    let parent_descriptor = serde_json::json!({"with":{"parent":{"the":parent,"as":"Entity","cardinality":"many"}}});
    let ancestor_descriptor = serde_json::json!({"with":{"ancestor":{"the":ancestor,"as":"Entity","cardinality":"many"}}});
    let base = serde_json::json!({
        "deduce":ancestor_descriptor,
        "when":[{"assert":parent_descriptor,"where":{"this":{"?":{"name":"this"}},"parent":{"?":{"name":"ancestor"}}}}]
    });
    let recursive = serde_json::json!({
        "deduce":ancestor_descriptor,
        "when":[
            {"assert":parent_descriptor,"where":{"this":{"?":{"name":"this"}},"parent":{"?":{"name":"p"}}}},
            {"assert":ancestor_descriptor,"where":{"this":{"?":{"name":"p"}},"ancestor":{"?":{"name":"ancestor"}}}}
        ]
    });
    let query = QueryRequest {
        rules: vec![base,recursive],
        premises:vec![serde_json::json!({"assert":ancestor_descriptor,"where":{
            "this":{"?":{"name":"who"}},"ancestor":{"?":{"name":"relative"}}
        }})],
        select:vec!["who".into(),"relative".into()], ..Default::default()
    };
    let result = db.query(query).await?;
    assert_eq!(result.rows.len(),9);
    assert!(!result.truncated);
    Ok(())
}
```

<a name="chunk-query-syntax-boundary"></a><sub>[`src/query.rs`](../../../../x0k-folio-dialog/src/query.rs) · `#query-syntax-boundary`</sub>

```rust {#query-syntax-boundary file="src/query.rs"}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn query_mapping_preserves_record_literals_and_rejects_misspelled_options() -> Result<()> {
        let literal = serde_json::json!({"the":"literal predicate text","assert":{"the":"also literal"}});
        let mut query = serde_json::json!({
            "assert":{"with":{"data":{"the":"https://example.org/record","as":"Record"}}},
            "where":{"data":literal}
        });
        let mut names = std::collections::BTreeMap::new();
        map_descriptors(&mut query, &mut names)?;
        assert_eq!(query["where"]["data"],literal);
        assert_eq!(names.len(),1);
        assert!(serde_json::from_value::<QueryRequest>(serde_json::json!({"max_row":10})).is_err());
        Ok(())
    }
}
```

<a name="chunk-instance-boundary"></a><sub>[`tests/store.rs`](../../../../x0k-folio-dialog/tests/store.rs) · `#instance-boundary`</sub>

```rust {#instance-boundary file="tests/store.rs"}
#[tokio::test]
async fn empty_store_has_stable_incarnation_and_missing_root_is_not_new_store() -> Result<()> {
    let directory = tempfile::tempdir()?;
    let writer = DialogBackend::open(directory.path())?;
    let reader = DialogBackend::open_reader(directory.path())?;
    assert_eq!(writer.instance_id(), reader.instance_id());
    assert!(reader.query(request("urn:empty")).await?.rows.is_empty());
    // Keep marker but remove the published root. A checkpoint bound to the old
    // incarnation must never be accepted as acknowledgement of an empty store.
    let root = dialog_artifacts::make_reference(b"folio-v1");
    let filename = root.iter().map(|b| format!("{b:02x}")).collect::<String>();
    std::fs::remove_file(directory.path().join(filename))?;
    assert!(DialogBackend::open_reader(directory.path()).is_err());
    assert!(reader.query(request("urn:empty")).await.is_err());
    Ok(())
}
```

## Reopening and limits

`store.json` records format version one and a stable instance UUID. The first
writer publishes an empty root before that marker. Subsequent opens require both;
an interrupted initialization requires a fresh generation. Hosts bind their
checkpoints to `instance_id()` and refuse a mismatched store.

Readers open an existing directory without creation or lock acquisition. Each
query reloads a committed root, then holds that snapshot while it evaluates.
There is no atomic multi-document publication guarantee. A reader must reopen
when its store incarnation changes.

Selecting an unknown binding produces the native lookup error. A binding absent
through optional evaluation is omitted from that row. Queue admission is bounded
at eight requests; a full queue fails promptly. Query cancellation and timeouts
are cooperative, including recursive plans, and are not hard CPU/memory limits.
Sources retain support metadata and predicate dictionaries after retraction.
