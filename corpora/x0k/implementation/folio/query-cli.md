---
x0k:
  format: folio/v1
  id: x0k:implementation/folio/query-cli
  type: implementation
  status: draft
  summary: Explicit corpus and database selection for standalone Folio queries.
  concerns:
  - folio
  - query
  - cli
  tangle:
    crate: x0k-folio-cli
    root: src/main.rs
  edges:
    implements:
    - x0k:design/domains-of-your-own
    - x0k:affordance/query_the_documents
    motivated_by:
    - x0k:intent/c5ccd003-77d6-4b0d-8824-649f6221c259
    cites:
    - x0k:architecture/folio-backends
    - x0k:implementation/folio/ingestion
---
# Querying a directory of documents

The command takes explicit corpus and database paths. Its parser, document
source and database adapter remain separate so a query does not need an
x0k installation.

## The query command

The CLI exposes document queries through `x0k-folio-cli query`.

<a name="folio-instance-68747470733a2f2f306b2e636f6d70757465722f6f6e746f6c6f6779237369676e69666965722f78306b2d666f6c696f2d636c692d7175657279-1"></a><sub data-instance-iri="https://0k.computer/ontology#signifier/x0k-folio-cli-query" data-concept-iri="https://0k.computer/ontology#Signifier" data-source-document="corpora/x0k/implementation/folio/query-cli.md"><strong>Signifier</strong> · The query command · <code>https://0k.computer/ontology#signifier/x0k-folio-cli-query</code> · <a href="#folio-source-68747470733a2f2f306b2e636f6d70757465722f6f6e746f6c6f6779237369676e69666965722f78306b2d666f6c696f2d636c692d7175657279-1">source declaration</a></sub><a name="folio-source-68747470733a2f2f306b2e636f6d70757465722f6f6e746f6c6f6779237369676e69666965722f78306b2d666f6c696f2d636c692d7175657279-1"></a>

```yaml x0k:signifier
id: x0k:signifier/x0k-folio-cli-query
cue: x0k-folio-cli query
edges:
  signifies:
    - x0k:affordance/query_the_documents
  presentedOn:
    - x0k:surface/cli
```

## Read a collection

The examples define their own Paper concept. No supplied ontology is needed.

```sh
cargo run -p x0k-folio-cli -- ingest --root x0k-folio-cli/examples/papers --database /tmp/papers
cargo run -p x0k-folio-cli -- query --database /tmp/papers --file x0k-folio-cli/examples/queries/citations.json
```

These paths refer to the published repository. Within the development
workspace, the package is under substrate/crates/production.

The query joins a citation to its declaring document and that document's
path. Edit the query file to choose different properties and bindings.
Queries use Dialog's native structural JSON: each assertion supplies a
property description and binds its fields to variables. Rules use the same
descriptions. The citation-closure example derives transitive reachability;
it does not require a Rust concept declaration.

Use --format json for tagged values. References and text remain distinct.
Signed and unsigned integers are decimal strings, preserving 128-bit values.
Floats are strings too; byte and record values are arrays of bytes.
The table view quotes text and encloses references in angle brackets.
An empty result is successful. A rejected query returns a nonzero exit.

Query files set max_rows and timeout_ms; command flags can override them.
The defaults are 1,000 rows and 30 seconds. The row limit is at most 100,000
and the timeout at most five minutes. Timeout and cancellation are
cooperative. They are not hard CPU or memory budgets.

## Keep the database current

Use watch with the same root and database arguments to rescan periodically.
Each scan reloads the selected vocabulary. Unchanged acknowledged sources
are skipped by ingestion; discovery still reads the collection. A separate
query process can read committed snapshots while the watcher writes.
Ctrl-C stops the watcher after its current source operation returns.

By default, only definitions in the collection supply its ontology.
--shipped adds this build's supplied vocabulary; --vocabulary selects a
local directory of Turtle modules instead. These two choices are exclusive.
Neither requires an x0k installation.

Status reports the last reconciliation, its vocabulary and checkpoint
revisions, rejected files, and pending sources. Invalid documents retain
their last good contribution; valid siblings can still advance. The
reconciliation command fails when any source is rejected or pending.
The standalone command waits up to 30 seconds per source transaction;
--delivery-grace-ms can shorten that bounded wait. A transaction completing
later remains unacknowledged until a subsequent reconciliation confirms it.
Definitions that make the collection inconsistent reject the entire scan.
Queries describe this last observed state; they cannot promise that
unscanned edits are already indexed.

A database is bound to its canonical corpus root and the store's persistent
identity. Moving to another corpus requires another database. The
standalone executable provides Dialog; the private host composes x0k and
Dialog through the shared named-backend interface.

Rebuild writes a new store and checkpoint generation. It selects that
generation only after a complete reconciliation. The previous generation
stays on disk, including when rebuilding fails. Generation selection is
atomic; source updates within a generation are individually atomic, not
one transaction over the whole collection. This command never edits
documents or removes old generations.

## Package boundary

<a name="chunk-cli-manifest"></a><sub>[`Cargo.toml`](../../../../x0k-folio-cli/Cargo.toml) · `#cli-manifest`</sub>

```toml {#cli-manifest file="Cargo.toml"}
[package]
name = "x0k-folio-cli"
version = "0.1.0"
edition = { workspace = true }
license = "MIT"
description = "Standalone Folio ingestion and Dialog queries"
rust-version = { workspace = true }
repository = "https://github.com/0k-dot-computer/x0k-folio"
readme = "../README.md"
keywords = ["literate-programming", "tangle", "markdown", "documentation"]

[dependencies]
anyhow = "1"
blake3 = "1"
clap = { version = "4", features = ["derive"] }
fs2 = "0.4"
serde = { version = "1", features = ["derive"] }
serde_json = "1"
serde_norway = "0.9"
walkdir = "2"
tracing-subscriber = { version = "0.3", features = ["env-filter", "json"] }
tokio = { version = "1", features = ["rt-multi-thread", "macros", "signal", "time"] }
x0k-folio = { path = "../x0k-folio", default-features = false, features = ["document-vocabulary"] , version = "0.1.0" }
x0k-ontology = { path = "../x0k-ontology", features = ["load"] , version = "0.1.0" }
x0k-fact-projection = { path = "../x0k-fact-projection" , version = "0.1.0" }
x0k-folio-ingest = { path = "../x0k-folio-ingest" , version = "0.1.0" }
x0k-folio-dialog = { path = "../x0k-folio-dialog" , version = "0.1.0" }

[dev-dependencies]
tempfile = "3"

[[bin]]
name = "x0k-folio-cli"
path = "src/main.rs"
```

## Command entry

<a name="chunk-cli-main"></a><sub>[`src/main.rs`](../../../../x0k-folio-cli/src/main.rs) · `#cli-main`</sub>

```rust {#cli-main}
use std::{collections::BTreeMap, io::Write, path::{Path, PathBuf}, sync::{Mutex, Arc, atomic::{AtomicBool, Ordering}}, time::{SystemTime, UNIX_EPOCH}};
use anyhow::{Context, Result, ensure};
use clap::{Args, Parser, Subcommand, ValueEnum};
use fs2::FileExt;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use x0k_fact_projection::FactValue;
use x0k_folio_cli::source::FolioSource;
use x0k_folio_dialog::{DialogBackend, QueryRequest};
use x0k_folio_ingest::{checkpoint, lifecycle::{self, DocumentSource}};
use x0k_ontology::concept_facts::OntologyModel;

#[derive(Parser)]
#[command(version, about = "Query concepts and instances in a directory of Folio documents")]
struct Cli { #[command(subcommand)] command: Command }
#[derive(Subcommand)]
enum Command {
    /// Reconcile documents with an explicitly selected Dialog database.
    Ingest(Corpus),
    /// Evaluate a native Dialog query file against the committed database.
    Query(Query),
    /// Poll for document and vocabulary changes until interrupted.
    Watch { #[command(flatten)] corpus: Corpus, #[arg(long, default_value_t = 1000, value_parser = clap::value_parser!(u64).range(100..))] interval_ms: u64 },
    /// Show the last reconciliation, including rejected and pending sources.
    Status(Database),
    /// Build a new generation, keeping the previous one until this succeeds.
    Rebuild(Corpus),
}
#[derive(Args)]
struct Database { #[arg(long)] database: PathBuf }
#[derive(Args)]
struct Corpus {
    #[arg(long)] root: PathBuf,
    #[arg(long)] database: PathBuf,
    /// Load the ontology modules in this directory.
    #[arg(long, conflicts_with = "shipped")] vocabulary: Option<PathBuf>,
    /// Include the vocabulary bundled with this build.
    #[arg(long)] shipped: bool,
    #[arg(long, default_value = "dialog")] backend: String,
    /// Wait this long for a source transaction before reporting it pending.
    #[arg(long, default_value_t = 30_000, value_parser = clap::value_parser!(u64).range(1..=30_000))]
    delivery_grace_ms: u64,
}
#[derive(Args)]
struct Query {
    #[arg(long)] database: PathBuf,
    #[arg(long)] file: PathBuf,
    #[arg(long, default_value = "dialog")] backend: String,
    #[arg(long, value_enum, default_value_t = Output::Table)] format: Output,
    #[arg(long)] max_rows: Option<usize>,
    #[arg(long)] timeout_ms: Option<u64>,
}
#[derive(Clone, Copy, ValueEnum)]
enum Output { Table, Json }

#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Manifest {
    version: u32,
    generation: String,
    root: PathBuf,
    backend: String,
    instance: String,
}
struct Writer {
    directory: PathBuf,
    manifest: Manifest,
    backend: DialogBackend,
    _lock: std::fs::File,
    cancelled: Arc<AtomicBool>,
}
fn now() -> u128 { SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_nanos() }
fn emit(value: &Value) -> Result<()> {
    let mut out = std::io::stdout().lock();
    serde_json::to_writer_pretty(&mut out, value)?;
    writeln!(out)?;
    Ok(())
}
fn read_manifest(directory: &Path) -> Result<Manifest> {
    let manifest: Manifest = serde_json::from_slice(&std::fs::read(directory.join("current.json"))
        .context("database has no committed generation; run ingest first")?)?;
    ensure!(manifest.version == 1, "unsupported Folio database version");
    ensure!(!manifest.generation.is_empty() && manifest.generation.bytes().all(|b| b.is_ascii_hexdigit()),
        "invalid database generation");
    ensure!(!manifest.backend.trim().is_empty(), "empty backend name");
    Ok(manifest)
}
fn durable_directory(path: &Path) -> Result<()> {
    if path.is_dir() { return Ok(()); }
    let parent = path.parent().filter(|p| !p.as_os_str().is_empty()).unwrap_or(Path::new("."));
    durable_directory(parent)?;
    match std::fs::create_dir(path) {
        Ok(()) => {},
        Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists && path.is_dir() => {},
        Err(error) => return Err(error.into()),
    }
    std::fs::File::open(parent)?.sync_all()?;
    Ok(())
}
fn generation_path(directory: &Path, manifest: &Manifest) -> PathBuf {
    directory.join("generations").join(&manifest.generation)
}
fn save_json(path: &Path, value: &impl Serialize) -> Result<()> {
    checkpoint::atomic_write(path, &serde_json::to_vec_pretty(value)?)
}
impl Writer {
    fn open(args: &Corpus, rebuild: bool) -> Result<Self> {
        ensure!(args.backend == "dialog", "this standalone binary provides the dialog backend; x0k is enabled through its separate host adapter");
        let root = args.root.canonicalize().context("open corpus root")?;
        ensure!(root.is_dir(), "corpus root must be a directory");
        durable_directory(&args.database)?;
        let directory = args.database.canonicalize()?;
        let lock = std::fs::OpenOptions::new().read(true).write(true).create(true).truncate(false)
            .open(directory.join("writer.lock"))?;
        lock.try_lock_exclusive().context("another Folio writer is using this database")?;
        let existing = directory.join("current.json").exists();
        let mut manifest = if existing {
            let manifest = read_manifest(&directory)?;
            ensure!(manifest.root == root, "database belongs to {}; use another database for {}", manifest.root.display(), root.display());
            ensure!(manifest.backend == args.backend, "database backend is {}, not {}", manifest.backend, args.backend);
            manifest
        } else {
            Manifest { version: 1, generation: String::new(), root, backend: args.backend.clone(), instance: String::new() }
        };
        if !existing || rebuild {
            let seed = format!("{}:{}:{}", directory.display(), std::process::id(), now());
            manifest.generation = blake3::hash(seed.as_bytes()).to_hex().to_string();
            durable_directory(&directory.join("generations"))?;
            std::fs::create_dir(generation_path(&directory, &manifest))?;
            std::fs::File::open(directory.join("generations"))?.sync_all()?;
            manifest.instance.clear();
        }
        let store = generation_path(&directory, &manifest).join("store");
        if existing && !rebuild { ensure!(store.is_dir(), "database store is missing; use rebuild to create a new generation"); }
        let backend = DialogBackend::open(&store)?;
        let instance = backend.instance_id().to_string();
        if !manifest.instance.is_empty() {
            ensure!(manifest.instance == instance, "database store was replaced; use rebuild to reset its checkpoint");
        }
        manifest.instance = instance;
        Ok(Self { directory, manifest, backend, _lock: lock, cancelled: Arc::new(AtomicBool::new(false)) })
    }
    fn path(&self) -> PathBuf { generation_path(&self.directory, &self.manifest) }
    fn publish(&self) -> Result<()> { save_json(&self.directory.join("current.json"), &self.manifest) }
}
fn base_model(args: &Corpus) -> Result<OntologyModel> {
    if let Some(path) = &args.vocabulary { Ok(OntologyModel::load(path)?) }
    else if args.shipped { Ok(OntologyModel::shipped()) }
    else { Ok(OntologyModel::new(vec![])) }
}
async fn reconcile(writer: &Writer, args: &Corpus) -> Result<bool> {
    let started = now().to_string();
    let status_path = writer.path().join("status.json");
    save_json(&status_path, &json!({
        "generation": writer.manifest.generation, "backend": writer.manifest.backend,
        "root": writer.manifest.root, "started_unix_ns": started, "reconciling": true
    }))?;
    let result = reconcile_inner(writer, args, &started).await;
    match result {
        Ok(report) => {
            let complete = report["complete"].as_bool() == Some(true);
            save_json(&status_path, &report)?;
            emit(&report)?;
            Ok(complete)
        }
        Err(error) => {
            save_json(&status_path, &json!({
                "generation": writer.manifest.generation, "backend": writer.manifest.backend,
                "root": writer.manifest.root, "started_unix_ns": started,
                "finished_unix_ns": now().to_string(), "reconciling": false,
                "complete": false, "error": format!("{error:#}"),
                "view": "previously committed source contributions remain readable"
            }))?;
            Err(error)
        }
    }
}
async fn reconcile_inner(writer: &Writer, args: &Corpus, started: &str) -> Result<Value> {
    let origin = match &args.vocabulary {
        Some(path) => path.canonicalize()?.to_string_lossy().into_owned(),
        None if args.shipped => "shipped vocabulary".to_string(),
        None => "document definitions only".to_string(),
    };
    let mut source = FolioSource::prepare_with_provenance(&writer.manifest.root, base_model(args)?, &origin)?;
    let diagnostics = source.diagnostics().to_vec();
    let state_path = writer.path().join("checkpoint");
    let backends = Mutex::new(vec![writer.backend.backend(&writer.manifest.backend)
        .with_delivery_grace(std::time::Duration::from_millis(args.delivery_grace_ms))]);
    let (_seen, updated, delivered_facts) = lifecycle::reconcile(
        &mut source, &writer.manifest.root, &state_path, &backends, Some(&writer.cancelled)).await?;
    let state = checkpoint::load_state(&state_path)?;
    let mut pending = Vec::new();
    let mut checkpoint_revision = blake3::Hasher::new();
    for (path, file) in &state.files {
        checkpoint_revision.update(&(path.len() as u64).to_le_bytes());
        checkpoint_revision.update(path.as_bytes());
        checkpoint_revision.update(file.content_hash.as_bytes());
        if !file.acked_by.contains(&writer.manifest.backend) { pending.push(path.clone()); }
    }
    for (path, record) in &state.recovery {
        if record.deleted && !pending.contains(path) { pending.push(path.clone()); }
    }
    let mut changed_during_scan = Vec::new();
    // A prepared snapshot is immutable. Check again for edits that happened
    // while the shared lifecycle was reading it; do not call those sources fresh.
    for diagnostic in diagnostics.iter().filter(|d| d.error.is_none() && !d.non_folio) {
        match (source.read(&diagnostic.path), state.files.get(&diagnostic.path.to_string_lossy().to_string())) {
            (Ok(bytes), Some(file)) if source.revision_hash(&bytes) == file.content_hash => {},
            _ => changed_during_scan.push(diagnostic.path.clone()),
        }
    }
    let invalid = diagnostics.iter().filter(|d| d.error.is_some()).count();
    let non_folio = diagnostics.iter().filter(|d| d.non_folio).count();
    Ok(json!({
        "generation": writer.manifest.generation, "backend": writer.manifest.backend,
        "root": writer.manifest.root, "started_unix_ns": started, "finished_unix_ns": now().to_string(),
        "reconciling": false, "delivery_grace_ms": args.delivery_grace_ms, "complete": invalid == 0 && pending.is_empty() && changed_during_scan.is_empty() && !writer.cancelled.load(Ordering::Relaxed),
        "cancelled": writer.cancelled.load(Ordering::Relaxed),
        "vocabulary_revision": source.fingerprint(),
        "checkpoint_revision": checkpoint_revision.finalize().to_hex().to_string(),
        "markdown_files": diagnostics.len(),
        "desired_vocabulary_sources": source.vocabulary_source_count(),
        "acknowledged_vocabulary_sources": state.files.iter().filter(|(path,file)| source.is_vocabulary_source(Path::new(path)) && file.acked_by.contains(&writer.manifest.backend)).count(), "valid_documents": diagnostics.len() - invalid - non_folio,
        "non_folio_files": non_folio, "invalid_documents": invalid,
        "updated_documents": updated, "delivered_facts": delivered_facts,
        "checkpoint_sources": state.files.len(),
        "acknowledged_sources": state.files.values().filter(|file| file.acked_by.contains(&writer.manifest.backend)).count(),
        "checkpoint_fact_contributions": state.files.values().map(|f| f.fact_count).sum::<usize>(),
        "pending_sources": pending, "changed_during_scan": changed_during_scan,
        "diagnostics": diagnostics,
        "view": "each source commits atomically; rejected sources retain their last good contribution"
    }))
}
fn typed(value: &FactValue) -> Value {
    match value {
        FactValue::Text(v) => json!({"type":"text","value":v}),
        FactValue::EntityRef(v) => json!({"type":"entity","value":v}),
        FactValue::Boolean(v) => json!({"type":"boolean","value":v}),
        FactValue::UnsignedInt(v) => json!({"type":"unsigned","value":v.to_string()}),
        FactValue::SignedInt(v) => json!({"type":"signed","value":v.to_string()}),
        FactValue::Float(v) => json!({"type":"float","value":v.to_string()}),
        FactValue::Bytes(v) => json!({"type":"bytes","value":v}),
        FactValue::Record(v) => json!({"type":"record","value":v}),
        FactValue::Symbol(v) => json!({"type":"symbol","value":v}),
        FactValue::Retracted(v) => json!({"type":"retracted","value":typed(v)}),
    }
}
fn cell(value: &FactValue) -> String {
    match value {
        FactValue::EntityRef(v) | FactValue::Symbol(v) => format!("<{v}>"),
        FactValue::Text(v) => serde_json::to_string(v).unwrap_or_default(),
        _ => typed(value).to_string(),
    }
}
async fn query(args: Query) -> Result<()> {
    let manifest = read_manifest(&args.database)?;
    ensure!(args.backend == manifest.backend, "backend {} is not enabled; this database enables {}", args.backend, manifest.backend);
    let path = generation_path(&args.database, &manifest);
    let reader = DialogBackend::open_reader(path.join("store"))?;
    ensure!(reader.instance_id() == manifest.instance, "database store identity does not match its checkpoint");
    let mut request: QueryRequest = serde_json::from_slice(&std::fs::read(&args.file)?)?;
    if let Some(limit) = args.max_rows { request.max_rows = limit; }
    if let Some(timeout) = args.timeout_ms { request.timeout_ms = timeout; }
    let columns = request.select.clone();
    let before = std::fs::read(path.join("status.json")).ok();
    let result = reader.query(request).await?;
    let after = std::fs::read(path.join("status.json")).ok();
    let report = after.as_ref().map(|bytes| serde_json::from_slice::<Value>(bytes)).transpose()?;
    match args.format {
        Output::Json => {
            let rows: Vec<BTreeMap<_,_>> = result.rows.iter().map(|row| row.iter().map(|(key,value)| (key,typed(value))).collect()).collect();
            emit(&json!({
                "backend": manifest.backend, "generation": manifest.generation,
                "rows": rows, "truncated": result.truncated,
                "reconciliation_changed_during_query": before != after,
                "last_reconciliation": report,
                "freshness": "last reconciliation describes observed files; edits since that scan may not be indexed"
            }))?;
        }
        Output::Table => {
            let mut out = std::io::stdout().lock();
            writeln!(out, "{}", columns.join("\t"))?;
            for row in &result.rows {
                writeln!(out, "{}", columns.iter().map(|name| row.get(name).map(cell).unwrap_or_default()).collect::<Vec<_>>().join("\t"))?;
            }
            writeln!(out, "\n{} rows{} · backend {} · generation {}", result.rows.len(),
                if result.truncated { " (limit reached)" } else { "" }, manifest.backend, manifest.generation)?;
            writeln!(out, "Results reflect committed sources. Use status for the last reconciliation and rejected documents.")?;
        }
    }
    Ok(())
}
#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| "warn".into()))
        .json().with_writer(std::io::stderr).try_init()
        .map_err(|error| anyhow::anyhow!("initialize diagnostics: {error}"))?;
    match Cli::parse().command {
        Command::Query(args) => query(args).await,
        Command::Status(args) => {
            let manifest = read_manifest(&args.database)?;
            let path = generation_path(&args.database, &manifest);
            let reader = DialogBackend::open_reader(path.join("store"))?;
            ensure!(reader.instance_id() == manifest.instance, "database store identity mismatch");
            let report: Value = serde_json::from_slice(&std::fs::read(path.join("status.json"))?)?;
            emit(&json!({"database": manifest, "last_reconciliation": report}))
        }
        Command::Ingest(args) => {
            let writer = Writer::open(&args, false)?;
            let result = reconcile(&writer, &args).await;
            writer.publish()?;
            ensure!(result?, "some sources were rejected or remain pending; see the reconciliation report");
            Ok(())
        }
        Command::Rebuild(args) => {
            let writer = Writer::open(&args, true)?;
            ensure!(reconcile(&writer, &args).await?, "rebuild incomplete; the previous generation remains selected");
            writer.publish()?;
            Ok(())
        }
        Command::Watch { corpus, interval_ms } => {
            let writer = Writer::open(&corpus, false)?;
            let cancelled = writer.cancelled.clone();
            let mut interrupted = tokio::spawn(async move {
                tokio::signal::ctrl_c().await?;
                cancelled.store(true, Ordering::Relaxed);
                Ok::<_, std::io::Error>(())
            });
            loop {
                if writer.cancelled.load(Ordering::Relaxed) { break; }
                let result = reconcile(&writer, &corpus).await;
                writer.publish()?;
                if let Err(error) = result {
                    emit(&json!({"backend": writer.manifest.backend, "error": format!("{error:#}"), "retrying": true}))?;
                }
                tokio::select! {
                    signal = &mut interrupted => { signal??; break; },
                    _ = tokio::time::sleep(std::time::Duration::from_millis(interval_ms)) => {},
                }
            }
            Ok(())
        }
    }
}

```
