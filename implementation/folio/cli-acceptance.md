---
x0k:
  format: folio/v1
  id: x0k:implementation/folio/cli-acceptance
  type: implementation
  status: draft
  summary: Exercise the standalone CLI through separate processes and real Paper documents.
  tangle:
    crate: crates/x0k-folio-cli
    root: tests/acceptance.rs
  edges:
    implements:
    - x0k:design/domains-of-your-own
    cites:
    - x0k:implementation/folio/query-cli
    - x0k:implementation/folio/document-source
    - x0k:architecture/folio-backends
    motivated_by:
    - x0k:intent/c5ccd003-77d6-4b0d-8824-649f6221c259
---
# A collection survives its commands

The Paper example is useful only if the command a reader runs preserves its
meaning across edits and restarts. These acceptance tests execute the built CLI
in separate processes, against temporary copies of the public example. They
use the actual database adapter, parser and ingestion checkpoint.

A citation remains an entity reference, a page count remains an integer, and a
bad edit preserves the last good source contribution. Rebuilding makes a new
generation with the same answers; a running watcher permits independent readers.

## Process boundaries belong in the test

Each test owns its corpus, database and command logs. Commands have a bounded
wait, and every live child is killed and reaped on failure. No operator database
or private corpus fixture participates. Cargo supplies the executable by
default; an explicit `FOLIO_ACCEPTANCE_BIN` selects an already-built executable
for checking an integration candidate from another isolated workspace.

<a name="chunk-acceptance-harness"></a><sub>[`tests/acceptance.rs`](../../crates/x0k-folio-cli/tests/acceptance.rs) · `#acceptance-harness`</sub>

```rust {#acceptance-harness}
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, ExitStatus, Stdio};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::{Duration, Instant};
use serde_json::{json, Value};

fn binary() -> std::ffi::OsString {
    std::env::var_os("FOLIO_ACCEPTANCE_BIN")
        .unwrap_or_else(|| env!("CARGO_BIN_EXE_x0k-folio-cli").into())
}
const PAPER: &str = "https://example.org/papers#";
static SEQUENCE: AtomicUsize = AtomicUsize::new(0);

struct Fixture {
    _temporary: tempfile::TempDir,
    home: PathBuf,
    corpus: PathBuf,
    database: PathBuf,
}

struct Output {
    status: ExitStatus,
    stdout: String,
    stderr: String,
}

struct Running {
    child: Child,
    stdout: PathBuf,
    stderr: PathBuf,
}

impl Drop for Running {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

impl Running {
    fn finish(&mut self) -> Output {
        let deadline = Instant::now() + Duration::from_secs(30);
        let status = loop {
            if let Some(status) = self.child.try_wait().unwrap() { break status; }
            assert!(Instant::now() < deadline, "command timed out; stderr: {}",
                fs::read_to_string(&self.stderr).unwrap_or_default());
            std::thread::sleep(Duration::from_millis(20));
        };
        Output { status, stdout: fs::read_to_string(&self.stdout).unwrap(),
            stderr: fs::read_to_string(&self.stderr).unwrap() }
    }
}

impl Output {
    fn success(&self) -> Value {
        assert!(self.status.success(), "command failed: {}\n{}", self.stderr, self.stdout);
        serde_json::from_str(&self.stdout).expect("command returns one JSON report")
    }

    fn failure(&self, message: &str) {
        assert!(!self.status.success(), "command unexpectedly succeeded: {}", self.stdout);
        assert!(self.stderr.contains(message) || self.stdout.contains(message),
            "expected {message:?}, got stderr={} stdout={}", self.stderr, self.stdout);
    }
}
```

The fixture copies the exact example a public user receives. Query files are
native structural Dialog premises, with explicit result bindings. They select
full predicate IRIs; tests never reach into the adapter's private relation names.

<a name="chunk-prepare-fixture"></a><sub>[`tests/acceptance.rs`](../../crates/x0k-folio-cli/tests/acceptance.rs) · `#prepare-fixture`</sub>

```rust {#prepare-fixture}
impl Fixture {
    fn new() -> Self {
        let temporary = tempfile::tempdir().unwrap();
        let home = temporary.path().to_path_buf();
        let corpus = home.join("papers");
        let database = home.join("database");
        fs::create_dir(&corpus).unwrap();
        let example = Path::new(env!("CARGO_MANIFEST_DIR")).join("examples/papers");
        for name in ["alpha.md", "beta.md", "vocabulary.md"] {
            fs::copy(example.join(name), corpus.join(name)).unwrap();
        }
        Self { _temporary: temporary, home, corpus, database }
    }

    fn spawn(&self, args: &[&str]) -> Running {
        let sequence = SEQUENCE.fetch_add(1, Ordering::Relaxed);
        let stdout = self.home.join(format!("command-{sequence}.stdout"));
        let stderr = self.home.join(format!("command-{sequence}.stderr"));
        let child = Command::new(binary()).args(args)
            .stdout(Stdio::from(fs::File::create(&stdout).unwrap()))
            .stderr(Stdio::from(fs::File::create(&stderr).unwrap()))
            .spawn().unwrap();
        Running { child, stdout, stderr }
    }

    fn run(&self, args: &[&str]) -> Output { self.spawn(args).finish() }

    fn corpus_command(&self, command: &str) -> Output {
        self.run(&[command, "--root", self.corpus.to_str().unwrap(),
            "--database", self.database.to_str().unwrap()])
    }

    fn status(&self) -> Value {
        self.run(&["status", "--database", self.database.to_str().unwrap()]).success()
    }

    fn query_request(&self, request: Value) -> Value {
        let path = self.home.join(format!("query-{}.json", SEQUENCE.fetch_add(1, Ordering::Relaxed)));
        fs::write(&path, serde_json::to_vec(&request).unwrap()).unwrap();
        self.run(&["query", "--database", self.database.to_str().unwrap(),
            "--file", path.to_str().unwrap(), "--format", "json"]).success()
    }

    fn query(&self, predicate: &str) -> Value {
        self.query_request(json!({
            "premises": [premise(predicate, "entity", "value")],
            "select": ["entity", "value"]
        }))
    }

    fn edit_pages(&self, pages: u32) {
        let path = self.corpus.join("alpha.md");
        let body = fs::read_to_string(&path).unwrap();
        fs::write(path, body.replace("pages: 12", &format!("pages: {pages}"))).unwrap();
    }
}

fn premise(predicate: &str, entity: &str, value: &str) -> Value {
    json!({
        "assert": {"with": {"value": {"the": predicate, "cardinality": "many"}}},
        "where": {"this": {"?": {"name": entity}}, "value": {"?": {"name": value}}}
    })
}

fn rows(query: &Value) -> Vec<Value> {
    let mut rows = query["rows"].as_array().expect("query rows").clone();
    rows.sort_by_key(|row| serde_json::to_string(row).unwrap());
    rows
}
```

## Separate commands recover the same typed collection

Ingest exits before the first query starts. Every subsequent query opens a new
reader, so retained values cannot be explained by a live process cache. The
join follows Alpha's citation to Beta and reads Beta's page count.

<a name="chunk-accept-types-and-reopen"></a><sub>[`tests/acceptance.rs`](../../crates/x0k-folio-cli/tests/acceptance.rs) · `#accept-types-and-reopen`</sub>

```rust {#accept-types-and-reopen}
#[test]
fn ingest_query_reopen_preserve_types_and_native_join() {
    let fixture = Fixture::new();
    let report = fixture.corpus_command("ingest").success();
    assert_eq!(report["valid_documents"], 3);
    assert_eq!(report["invalid_documents"], 0);
    assert_eq!(report["complete"], true);
    let pages = fixture.query(&format!("{PAPER}pages"));
    assert_eq!(rows(&pages).len(), 2);
    assert!(rows(&pages).iter().all(|row| row["value"] == json!({"type":"signed","value":"12"})));
    let reviewed = rows(&fixture.query(&format!("{PAPER}reviewed")));
    assert_eq!(reviewed.len(), 2);
    assert!(reviewed.iter().all(|row| row["value"] == json!({"type":"boolean","value":true})));
    let citations = fixture.query(&format!("{PAPER}cites"));
    assert_eq!(rows(&citations).len(), 1);
    assert_eq!(rows(&citations)[0]["value"], json!({"type":"entity","value":format!("{PAPER}paper/beta")}));
    assert_eq!(rows(&pages), rows(&fixture.query(&format!("{PAPER}pages"))));
    let joined = fixture.query_request(json!({
        "premises": [
            premise(&format!("{PAPER}cites"), "paper", "target"),
            premise(&format!("{PAPER}pages"), "target", "pages")
        ],
        "select": ["paper", "target", "pages"]
    }));
    assert_eq!(rows(&joined).len(), 1);
    assert_eq!(rows(&joined)[0]["pages"]["value"], "12");
    assert_eq!(rows(&joined)[0]["paper"]["value"], format!("{PAPER}paper/alpha"));
    assert_eq!(fixture.corpus_command("ingest").success()["updated_documents"], 0);
}
```

## Invalid edits retain only their own last good contribution

An invalid Alpha must neither erase Alpha's old page count nor prevent Beta's
valid update. Rename changes the source path while preserving the authored
entity identity. Deleting that renamed source removes its contribution.

<a name="chunk-accept-source-lifecycle"></a><sub>[`tests/acceptance.rs`](../../crates/x0k-folio-cli/tests/acceptance.rs) · `#accept-source-lifecycle`</sub>

```rust {#accept-source-lifecycle}
#[test]
fn edit_invalid_rename_delete_preserve_source_ownership() {
    let fixture = Fixture::new();
    fixture.corpus_command("ingest").success();
    fixture.edit_pages(99);
    fixture.corpus_command("ingest").success();
    let last_good = rows(&fixture.query(&format!("{PAPER}pages")));
    assert!(last_good.iter().any(|row| row["value"]["value"] == "99"));
    let alpha = fixture.corpus.join("alpha.md");
    let valid = fs::read_to_string(&alpha).unwrap();
    fs::write(&alpha, "---\nx0k:\n  format: folio/v1\n---\n").unwrap();
    let beta = fixture.corpus.join("beta.md");
    fs::write(&beta, fs::read_to_string(&beta).unwrap().replace("pages: 12", "pages: 33")).unwrap();
    fixture.corpus_command("ingest").failure("rejected");
    let partial = rows(&fixture.query(&format!("{PAPER}pages")));
    assert!(partial.iter().any(|row| row["value"]["value"] == "99"));
    assert!(partial.iter().any(|row| row["value"]["value"] == "33"));
    assert_eq!(fixture.status()["last_reconciliation"]["invalid_documents"], 1);
    fs::write(&alpha, valid).unwrap();
    let renamed = fixture.corpus.join("renamed.md");
    fs::rename(alpha, &renamed).unwrap();
    fixture.corpus_command("ingest").success();
    assert_eq!(rows(&fixture.query(&format!("{PAPER}pages"))), partial);
    assert_eq!(fixture.status()["last_reconciliation"]["checkpoint_sources"], 3);
    fs::remove_file(renamed).unwrap();
    fixture.corpus_command("ingest").success();
    let remaining = rows(&fixture.query(&format!("{PAPER}pages")));
    assert_eq!(remaining.len(), 1);
    assert_eq!(remaining[0]["value"]["value"], "33");
    assert!(rows(&fixture.query(&format!("{PAPER}cites"))).is_empty());
}
```

## Rebuild publishes only a complete generation

A failed rebuild leaves the selected generation and its answers intact. A
successful rebuild has a fresh generation identity and the same typed rows.
The old generation remains available on disk for already-open readers.

<a name="chunk-accept-rebuild"></a><sub>[`tests/acceptance.rs`](../../crates/x0k-folio-cli/tests/acceptance.rs) · `#accept-rebuild`</sub>

```rust {#accept-rebuild}
#[test]
fn rebuild_matches_incremental_answers_and_retains_last_good_on_failure() {
    let fixture = Fixture::new();
    fixture.corpus_command("ingest").success();
    fixture.edit_pages(77);
    fixture.corpus_command("ingest").success();
    let expected = rows(&fixture.query(&format!("{PAPER}pages")));
    let original = fixture.status()["database"]["generation"].as_str().unwrap().to_string();
    let alpha = fixture.corpus.join("alpha.md");
    let valid = fs::read_to_string(&alpha).unwrap();
    fs::write(&alpha, "---\nx0k:\n  format: folio/v1\n---\n").unwrap();
    fixture.corpus_command("rebuild").failure("rebuild incomplete");
    assert_eq!(fixture.status()["database"]["generation"], original);
    assert_eq!(rows(&fixture.query(&format!("{PAPER}pages"))), expected);
    fs::write(alpha, valid).unwrap();
    fixture.corpus_command("rebuild").success();
    assert_ne!(fixture.status()["database"]["generation"], original);
    assert_eq!(rows(&fixture.query(&format!("{PAPER}pages"))), expected);
    assert!(fixture.database.join("generations").join(original).is_dir());
}
```

## Backend identity cannot stand in for the physical store

An unsupported backend name is an error. Replacing a store beneath an existing
checkpoint also fails explicitly; rebuilding makes a fresh, paired store and
checkpoint instead of treating old acknowledgements as current data.

<a name="chunk-accept-store-identity"></a><sub>[`tests/acceptance.rs`](../../crates/x0k-folio-cli/tests/acceptance.rs) · `#accept-store-identity`</sub>

```rust {#accept-store-identity}
#[test]
fn unsupported_backend_and_replaced_store_are_refused() {
    let fixture = Fixture::new();
    fixture.run(&["ingest", "--root", fixture.corpus.to_str().unwrap(),
        "--database", fixture.database.to_str().unwrap(), "--backend", "x0k"]).failure("backend");
    assert!(!fixture.database.join("current.json").exists());
    fixture.corpus_command("ingest").success();
    let query = fixture.home.join("empty-query.json");
    fs::write(&query, "{}").unwrap();
    fixture.run(&["query", "--database", fixture.database.to_str().unwrap(),
        "--file", query.to_str().unwrap(), "--backend", "unknown"]).failure("backend");
    let status = fixture.status();
    let generation = status["database"]["generation"].as_str().unwrap();
    let store = fixture.database.join("generations").join(generation).join("store");
    fs::rename(&store, store.with_extension("original")).unwrap();
    fs::create_dir(&store).unwrap();
    fixture.corpus_command("ingest").failure("store");
    fixture.run(&["status", "--database", fixture.database.to_str().unwrap()]).failure("identity");
    fixture.corpus_command("rebuild").success();
    assert_eq!(rows(&fixture.query(&format!("{PAPER}pages"))).len(), 2);
}
```

## A watcher and a reader use independent processes

A second writer is refused while the watcher holds its lock. Queries remain
available and eventually observe the edited page count. Interrupting the watcher
reaps its process without making the last committed answer unreadable.

<a name="chunk-accept-watch"></a><sub>[`tests/acceptance.rs`](../../crates/x0k-folio-cli/tests/acceptance.rs) · `#accept-watch`</sub>

```rust {#accept-watch}
#[test]
#[cfg(unix)]
fn watcher_allows_concurrent_queries_and_rejects_another_writer() {
    let fixture = Fixture::new();
    fixture.corpus_command("ingest").success();
    let mut watcher = fixture.spawn(&["watch", "--root", fixture.corpus.to_str().unwrap(),
        "--database", fixture.database.to_str().unwrap(), "--interval-ms", "100"]);
    let deadline = Instant::now() + Duration::from_secs(20);
    while fs::metadata(&watcher.stdout).unwrap().len() == 0 {
        assert!(watcher.child.try_wait().unwrap().is_none(), "watcher exited");
        assert!(Instant::now() < deadline, "watcher produced no report");
        std::thread::sleep(Duration::from_millis(20));
    }
    fixture.corpus_command("ingest").failure("writer");
    assert_eq!(rows(&fixture.query(&format!("{PAPER}pages"))).len(), 2);
    fixture.edit_pages(55);
    loop {
        assert!(watcher.child.try_wait().unwrap().is_none(), "watcher exited during query");
        if rows(&fixture.query(&format!("{PAPER}pages"))).iter().any(|row| row["value"]["value"] == "55") { break; }
        assert!(Instant::now() < deadline, "watcher did not index the edit");
        std::thread::sleep(Duration::from_millis(50));
    }
    assert!(Command::new("kill").args(["-INT", &watcher.child.id().to_string()]).status().unwrap().success());
    let stopped = watcher.finish();
    assert!(stopped.status.success(), "watcher failed on interrupt: {}", stopped.stderr);
    assert!(rows(&fixture.query(&format!("{PAPER}pages"))).iter().any(|row| row["value"]["value"] == "55"));
}
```

These tests prove command-visible source replacement and process reopening on a
local filesystem. They do not simulate power loss or establish a whole-corpus
transaction: the command deliberately exposes partial progress source by source.
