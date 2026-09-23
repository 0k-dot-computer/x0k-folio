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

One test runs at a time, and the fixture is what enforces it. A deadline
measures wall clock, so a deadline over a command that is sharing a disk with
five other tests measures the other five. Cargo's harness runs `#[test]`
functions on as many threads as there are cores, and these tests each spawn a
child process that writes and syncs — under a loaded `tools/ci` that made the
30-second budget a coin flip, losing `watcher_allows_concurrent_queries_and_…`
and `rebuild_matches_incremental_answers_and_…` on one gate run in two while
the same tests passed serially in about 2 seconds. Serializing costs a few
seconds of suite time and buys a deadline that means what it says.

A lock in the fixture rather than `--test-threads=1` because nothing in a
Cargo.toml can set that flag: it would have to be a harness the runner
remembers to pass, and a gate you have to remember is the failure being fixed.

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

/// Held for the life of a fixture, so exactly one test is spawning commands
/// and measuring deadlines against them. Incident test: the suite itself —
/// under `tools/ci` load the concurrent form failed one run in two.
static ONE_AT_A_TIME: std::sync::Mutex<()> = std::sync::Mutex::new(());

struct Fixture {
    _temporary: tempfile::TempDir,
    home: PathBuf,
    corpus: PathBuf,
    database: PathBuf,
    /// A module directory holding one module that declares nothing but
    /// itself — as close to "no base vocabulary" as a selected base gets,
    /// since a directory with no `*.ttl` at all is refused.
    empty_vocabulary: PathBuf,
    /// Declared last so it is dropped last: the next test may not start
    /// while this one's temporary directory is still being deleted, or its
    /// deletion becomes the disk load under the next one's deadline.
    _serial: std::sync::MutexGuard<'static, ()>,
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
    /// Thirty seconds is the budget for every command these tests run, the
    /// shipped-vocabulary ingest included. It used to need its own minutes:
    /// `set` synced each block inline on the async task, so upstream's
    /// sixteen-way flush ran one journal commit at a time and a fresh
    /// `--shipped` store cost 449 serialized `fsync`s — 82.7s of a 90.3s
    /// run against 0.77s of CPU, and 26s or 117s depending on the disk.
    /// The fan-out is real now (`x0k:implementation/folio/dialog`) and the
    /// same 449 `fsync`s take 0.28s, so one budget covers everything and no
    /// command needs a budget of its own.
    /// It is a hang detector and nothing finer: a deadline measures wall
    /// clock, so it means even this much only while `ONE_AT_A_TIME` keeps a
    /// second test off the disk.
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
        // A panicking test poisons the lock; its fixture is gone either way,
        // so the next test takes the permit rather than cascading.
        let serial = ONE_AT_A_TIME.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
        let temporary = tempfile::tempdir().unwrap();
        let home = temporary.path().to_path_buf();
        let corpus = home.join("papers");
        let database = home.join("database");
        let empty_vocabulary = home.join("no-modules");
        fs::create_dir(&corpus).unwrap();
        fs::create_dir(&empty_vocabulary).unwrap();
        // The filename carries the prefix the module fact declares.
        fs::write(empty_vocabulary.join("fixture.ttl"), concat!(
            "<https://example.test/module/fixture> <http://www.w3.org/1999/02/22-rdf-syntax-ns#type> <http://www.w3.org/2002/07/owl#Ontology> .\n",
            "<https://example.test/module/fixture> <http://purl.org/vocab/vann/preferredNamespacePrefix> \"fixture\" .\n",
            "<https://example.test/module/fixture> <http://purl.org/vocab/vann/preferredNamespaceUri> \"https://example.test/fixture#\" .\n",
        )).unwrap();
        let example = Path::new(env!("CARGO_MANIFEST_DIR")).join("examples/papers");
        for name in ["alpha.md", "beta.md", "vocabulary.md"] {
            fs::copy(example.join(name), corpus.join(name)).unwrap();
        }
        Self { _temporary: temporary, home, corpus, database, empty_vocabulary, _serial: serial }
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

    /// The papers corpus declares its whole vocabulary inside its own
    /// documents, so these runs select an empty module directory rather than
    /// the shipped set — the selected base is a source of its own, and these
    /// tests are about the documents. `the_default_base_is_the_shipped_set`
    /// below is the one that ingests the shipped modules, and it is also
    /// where the cost of doing so is held to a number.
    fn corpus_command(&self, command: &str) -> Output {
        self.run(&[command, "--root", self.corpus.to_str().unwrap(),
            "--database", self.database.to_str().unwrap(),
            "--vocabulary", self.empty_vocabulary.to_str().unwrap(), "--only-vocabulary"])
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

## Forgetting the vocabulary flag is not a half-filled store

`ingest` used to read no vocabulary at all when no flag named one, which
rejected every document carrying an `edges:` block — `refined_by` is a term
the shipped modules declare and nothing else does — and left the store partly
filled behind a nonzero exit. Partly filled is worse than refused: the exit
code is the only thing that says so, and a scheduled job that logs the exit
code and moves on leaves a database that answers questions wrongly rather than
not at all.

So no flag means the shipped set, and this is the test that pays its cost on
purpose: a corpus using a shipped edge ingests clean with nothing said.

It does not time that cost, and the two measurements that say why are worth
keeping. A fresh `--shipped` store is the one command in this tool whose cost
was ever measured in minutes, so a wall-clock budget over it is the obvious
regression gate; it is also the wrong one, in both directions at once. Put the
inline `sync_all` back and run this test on an idle disk and it finishes in
4.09s — green, on the defect. Keep the fix and run it against a disk with a
dozen competing `fsync`s and it needs more than five seconds — red, on correct
code. A bound loose enough never to fail the fix is loose enough to pass the
defect, because the disk moves further between two machines than the bug moves
on one.

So this test asserts what it can actually see — the shipped base is read, no
flag said, and nothing is rejected — under the same 30-second budget as every
other command here, which is a hang detector. The mechanism has its own guard
where the mechanism lives: `a_batch_of_blocks_syncs_concurrently` in
`x0k:implementation/folio/dialog` counts how many block writes are on the
blocking pool at once, each held long enough to be seen, so it reads the same
on a fast disk and a slow one. A count can separate the defect from the
weather; a clock cannot — its first cut compared wall clocks and failed on
the GitHub runner, whose disk coalesces concurrent commits less than ext4
here does (2026-09-23).

<a name="chunk-accept-default-vocabulary"></a><sub>[`tests/acceptance.rs`](../../crates/x0k-folio-cli/tests/acceptance.rs) · `#accept-default-vocabulary`</sub>

```rust {#accept-default-vocabulary}
#[test]
fn the_default_base_is_the_shipped_set() -> Result<(), String> {
    let fixture = Fixture::new();
    let decisions = fixture.home.join("decisions");
    fs::create_dir(&decisions).unwrap();
    let document = |id: &str, edges: &str| format!(
        "---\nx0k:\n  format: folio/v1\n  id: {id}\n  type: design\n  status: accepted\n{edges}---\n# Body\n");
    fs::write(decisions.join("alpha.md"),
        document("x0k:design/alpha", "  edges:\n    refined_by: [x0k:design/beta]\n")).unwrap();
    fs::write(decisions.join("beta.md"), document("x0k:design/beta", "")).unwrap();
    let report = fixture.run(&["ingest", "--root", decisions.to_str().unwrap(),
        "--database", fixture.home.join("default-db").to_str().unwrap()]).success();
    assert_eq!(report["invalid_documents"], 0,
        "a shipped edge with no flag said is not an unknown property");
    assert_eq!(report["valid_documents"], 2);
    assert_eq!(report["complete"], true);
    Ok(())
}
```

## The board reads the collection, and says so the same way twice

Three complaints from the same evening, and one fixture answers all of them.
A decision log whose vocabulary declares its own `supersededBy` got that
edge through `check`, `index`, `ingest` and `--named edges`, and then the
board — asking about the shipped IRI alone — returned nothing (Backstage).
`--vocabulary` and `--shipped` refused each other, a rule that was right
when they were alternatives and stale once `--vocabulary` became additive.
And the two output views disagreed about paths: the table stripped the
corpus root, the JSON kept it, so the board you read and the board you
script were shaped differently (jj).

The custom module here declares `bs:supersededBy` as an object property and
nothing else; the log is typed in it. A board that answers over one document
also settles `1 rows`.

<a name="chunk-accept-collection-board"></a><sub>[`tests/acceptance.rs`](../../crates/x0k-folio-cli/tests/acceptance.rs) · `#accept-collection-board`</sub>

```rust {#accept-collection-board}
#[test]
fn the_board_reads_the_collections_predicates_and_roots_both_views() {
    let fixture = Fixture::new();
    let modules = fixture.home.join("bs-modules");
    fs::create_dir(&modules).unwrap();
    fs::write(modules.join("bs.ttl"), concat!(
        "<https://backstage.io/module/bs> <http://www.w3.org/1999/02/22-rdf-syntax-ns#type> <http://www.w3.org/2002/07/owl#Ontology> .\n",
        "<https://backstage.io/module/bs> <http://purl.org/vocab/vann/preferredNamespacePrefix> \"bs\" .\n",
        "<https://backstage.io/module/bs> <http://purl.org/vocab/vann/preferredNamespaceUri> \"https://backstage.io/ontology#\" .\n",
        "<https://backstage.io/ontology#supersededBy> <http://www.w3.org/1999/02/22-rdf-syntax-ns#type> <http://www.w3.org/2002/07/owl#ObjectProperty> .\n",
        "<https://backstage.io/ontology#supersededBy> <http://www.w3.org/2000/01/rdf-schema#label> \"superseded by\" .\n",
    )).unwrap();
    let log = fixture.home.join("adrs");
    fs::create_dir(&log).unwrap();
    fs::write(log.join("adr013.md"), concat!(
        "---\nx0k:\n  format: folio/v1\n  id: x0k:architecture/adr013\n  type: architecture\n",
        "  status: superseded\n  edges:\n    bs:superseded_by: [x0k:architecture/adr014]\n---\n# ADR013\n")).unwrap();
    fs::write(log.join("adr014.md"), concat!(
        "---\nx0k:\n  format: folio/v1\n  id: x0k:architecture/adr014\n  type: architecture\n",
        "  status: accepted\n---\n# ADR014\n")).unwrap();
    let database = fixture.home.join("board-db");

    // Both flags at once: `--shipped` names the default out loud and
    // `--vocabulary` adds to it, so there is nothing left for them to
    // disagree about.
    let report = fixture.run(&["ingest", "--root", log.to_str().unwrap(),
        "--database", database.to_str().unwrap(),
        "--vocabulary", modules.to_str().unwrap(), "--shipped"]).success();
    assert_eq!(report["invalid_documents"], 0, "the custom edge is not an unknown property");
    assert_eq!(report["valid_documents"], 2);

    let board = fixture.run(&["query", "--database", database.to_str().unwrap(),
        "--named", "superseded", "--format", "json"]).success();
    let found = rows(&board);
    assert_eq!(found.len(), 1, "the collection's own supersession is on the board: {board}");
    assert_eq!(found[0]["document"]["value"], "https://0k.computer/ontology#architecture/adr013");
    assert_eq!(found[0]["path"]["value"], "adr013.md",
        "the JSON view roots its paths the way the table does: {board}");

    let table = fixture.run(&["query", "--database", database.to_str().unwrap(),
        "--named", "superseded", "--format", "table"]);
    assert!(table.status.success(), "table query failed: {}", table.stderr);
    assert!(table.stdout.contains("\"adr013.md\""), "got {}", table.stdout);
    assert!(table.stdout.contains("\n1 row ·"), "one answer is one row: {}", table.stdout);
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
    // Three documents and the selected base vocabulary, which is a source of
    // its own. `ingest` defaults to the shipped set now, so that fourth
    // source is there whether or not a flag named it.
    assert_eq!(fixture.status()["last_reconciliation"]["checkpoint_sources"], 4);
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
