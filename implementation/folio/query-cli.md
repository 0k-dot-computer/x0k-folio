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
    crate: crates/x0k-folio-cli
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
cargo run -p x0k-folio-cli -- ingest --root crates/x0k-folio-cli/examples/papers --database /tmp/papers
cargo run -p x0k-folio-cli -- query --database /tmp/papers --file crates/x0k-folio-cli/examples/queries/citations.json
```

These paths refer to the published repository. Within the development
workspace, the package is under substrate/crates/production.

The query joins a citation to its declaring document and that document's
path. Edit the query file to choose different properties and bindings.
Queries use Dialog's native structural JSON: each assertion supplies a
property description and binds its fields to variables. Rules use the same
descriptions. The citation-closure example derives transitive reachability;
it does not require a Rust concept declaration.

## Four questions that need no datalog at all

Sixty lines of nested `premises`/`assert`/`with`/`where`/`the` to ask which
document is superseded by which is a research project, and two maintainers
evaluating this tool independently reached the same verdict about it: they
could get there by pattern-matching against the examples, and would not ask
a colleague to. The datalog is the ceiling, not the floor. `--named` is the
floor — four questions a documentation maintainer actually asks, spelled as
one word each:

```sh
x0k-folio-cli query --database db --named status
x0k-folio-cli query --database db --named superseded
x0k-folio-cli query --database db --named edges --arg x0k:design/secure-config
x0k-folio-cli query --database db --named mentions --arg x0k:design/secure-config
```

`status` is the board — every typed document with its type, path, and its
status where it has one. `superseded` is the decision log's own question:
which document was replaced, by what, and where it lives. `edges` takes an id
and lists what leaves it; `mentions` takes an id and lists what arrives at it,
over every predicate the shipped vocabulary declares. The two that take an
`--arg` accept the compact id, because that is what is written in the file and
what a person pastes.

The prefix in that compact id is the reader's, not ours. A collection with a
`pyd` module of its own stores `pyd:concept/strict-mode` under the namespace
that module declared, and a question that expanded every prefix into the base
namespace would ask about a document nothing could have written — zero rows,
exit 0, and no way to tell that from an honest empty answer. So `--arg` is
expanded through the table the ingest resolved these documents with, and a
prefix no loaded module declares is refused by name, the way the `--named`
value itself would be. That is the hole the predicate side already closed:
an empty answer must never be indistinguishable from a typo.

That expansion is also what refuses an `--arg` which is not an id at all.
`--arg 'not an id'` used to reach the engine and come back as `Cannot assign
variable: Can not set "this" to Entity(…) because it is already set to
String("not an id")` — a true sentence about Dialog's binder and no help at
all. And when a well-formed id answers with nothing, the database is asked
whether it holds that document at all, because the old answer was one `no
fact in this database uses <…>` note per predicate the question enumerates —
twenty-two of them, none of them the answer, which was that no document with
that id is here. A question about one document reports on that document,
present or absent; the predicate census belongs to the questions that are
about predicates.

They are not a second query language. Each name builds the same
`QueryRequest` a file would, and `--named <name> --explain` prints that
request as JSON on stdout instead of running it — which makes the canned
queries the worked examples the format never had: start from one, redirect it
to a file, change a predicate.

That claim has to be literally true, and for a while it was not: every one of
the four questions runs several requests, so `--explain` printed them wrapped in a
`{"requests": […]}` envelope carrying each request's predicate label, and
`--file` refused the envelope as an unknown field (the jj re-evaluation,
2026-09-23). Two edits stood between a printed example and a running one,
which is exactly the distance a worked example exists to close. So `--file`
reads both shapes — one bare request, or the envelope — and the envelope is
part of the format rather than a printing convenience. A file with one
request stays the simplest thing that works.

<a name="chunk-named-queries"></a><sub>[`src/main.rs`](../../crates/x0k-folio-cli/src/main.rs) · `#named-queries`</sub>

```rust {#named-queries}
/// The four canned questions.
#[derive(Clone, Copy, ValueEnum)]
pub enum Named {
    /// Every document: type, status, path.
    Status,
    /// Superseded documents, what replaced them, and where they live.
    Superseded,
    /// What leaves one document, by predicate.
    Edges,
    /// What arrives at one document, by predicate.
    Mentions,
}

/// The base namespace every shipped term lives in.
const X0K: &str = "https://0k.computer/ontology#";

/// One `{"the": …, "cardinality": "many"}` property description — the unit a
/// premise's `with` map is built out of.
fn described(iri: &str) -> Value {
    json!({ "the": iri, "cardinality": "many" })
}

/// A compact id as the projection stores it. Entities are expanded IRIs in
/// the database; a person types `x0k:design/secure-config`, or
/// `pyd:concept/strict-mode` in a vocabulary of their own, so the prefix is
/// substituted for the namespace `namespaces` records for it and anything
/// already absolute is left alone. An undeclared prefix is an error naming
/// what is declared, because expanding it anyway asks about a document no
/// vocabulary here could have named.
fn expand_id(id: &str, namespaces: &BTreeMap<String, String>) -> Result<String> {
    let Some((prefix, rest)) = id.split_once(':') else {
        anyhow::bail!("{id} is not an id; write it as <prefix>:<class>/<name> or as a full IRI");
    };
    if prefix.starts_with("http") || rest.starts_with("//") { return Ok(id.to_string()); }
    match namespaces.get(prefix) {
        Some(namespace) => Ok(format!("{namespace}{rest}")),
        None => anyhow::bail!("no vocabulary this database was built with declares the prefix {prefix}:; it declares {}",
            namespaces.keys().map(|p| format!("{p}:")).collect::<Vec<_>>().join(", ")),
    }
}

/// What the ingest that built this generation wrote down about the
/// vocabulary it read these documents with: the namespace each compact
/// prefix expands to, and the edge predicates that vocabulary declares.
///
/// Read from the report rather than reloaded from the vocabulary directory,
/// because a question has to be asked in the terms the answers were *stored*
/// in, and a directory on disk may have moved on since.
struct Recorded {
    namespaces: BTreeMap<String, String>,
    edge_predicates: BTreeMap<String, String>,
}

fn recorded(database: &Path) -> Result<Recorded> {
    let manifest = read_manifest(database)?;
    let path = generation_path(database, &manifest).join("status.json");
    let report: Value = serde_json::from_slice(&std::fs::read(&path)
        .context("read the last reconciliation")?)?;
    let table = |key: &str| -> BTreeMap<String, String> {
        match report.get(key) {
            Some(Value::Object(fields)) => fields.iter().filter_map(|(name, value)|
                Some((name.clone(), value.as_str()?.to_string()))).collect(),
            _ => BTreeMap::new(),
        }
    };
    let mut namespaces = table("namespaces");
    namespaces.entry("x0k".to_string()).or_insert_with(|| X0K.to_string());
    // A generation older than this record still answers the shipped
    // questions, which is what it was built to answer.
    let mut edge_predicates = table("edge_predicates");
    if edge_predicates.is_empty() {
        edge_predicates = shipped_edge_predicates();
    }
    Ok(Recorded { namespaces, edge_predicates })
}

/// The edge predicates this build compiled, in the same shape a report
/// records: the `edges:` spelling against the IRI facts are stored under.
fn shipped_edge_predicates() -> BTreeMap<String, String> {
    x0k_ontology::KNOWN_EDGE_PREDICATES.iter().map(|snake| {
        let camel = x0k_ontology::snake_to_camel(snake).unwrap_or(snake);
        ((*snake).to_string(), format!("{X0K}{camel}"))
    }).collect()
}
```

The edge questions ask over the predicates this collection's vocabulary
declares, not the ones this binary compiled — same reason and same table as
`--arg`. A reader who declared `jj:superseded_by` and wrote it in an envelope
gets it back from `--named edges`, labelled the way they spelled it.

Not one of the four is a single request, and each is plural for its own
reason: the fields in a premise are a conjunction, so a single request would
return only the documents carrying every alternative at once. The edge
questions ask about *every* predicate, so they run one request per predicate
and the CLI concatenates the rows,
labelling each with the predicate that produced it. The label is a column the
database never held — it is which question was asked, not what was answered —
which is why it is added here rather than selected.

The board is the same shape for a quieter reason. `status:` is optional in the
envelope, and a conjunction over `docType ∧ status ∧ path` silently drops
every document that has not been triaged yet — which is the one part of the
index a hand-maintained one is worst at and this is supposed to replace. So
the board asks twice: once for the documents that carry a status, then once
for every typed document, and keeps the first row it got for each document.
The richer answer comes first, so the second request fills in only what the
first could not reach, and the status column is simply empty there.

`superseded` is plural for a third reason. `core.ttl` says the edge may be
authored from either end — a replacement that claims what it replaces and a
superseded document that points forward are the same edge read from opposite
sides — so a board that reads only `supersededBy` answers half the question
and answers it silently. It did, to a maintainer who typed fifteen ADRs the
way a person actually edits a log, `supersedes:` on the new one:
`check --closed` passed, the board came back empty, and the note under it
said no fact in the database used `supersededBy`, which was true and useless.
So it runs both spellings. The backward one needs two premises rather than
one: the path a row prints belongs to the *superseded* document, and when the
edge was authored on the replacement that fact is about a different entity,
reached by joining on the shared `document` variable. Authoring both ends of
one supersession is legal and yields the same row twice, so identical rows are
collapsed where the requests are concatenated.

<a name="chunk-named-requests"></a><sub>[`src/main.rs`](../../crates/x0k-folio-cli/src/main.rs) · `#named-requests`</sub>

```rust {#named-requests}
/// Every IRI this collection stores one predicate under, keyed by the
/// `edges:` spelling the collection declares it with.
///
/// A project that declares its own `supersededBy` writes
/// `bs:superseded_by:` in its envelopes, `check` admits it, `ingest`
/// projects it and `--named edges` returns it — and then the decision board
/// asked about the shipped IRI alone and came back empty (Backstage,
/// 2026-09-23). The board asks about every spelling of the edge the
/// collection was ingested with, matched on the part after the prefix,
/// which is the same rule `check` reads an envelope key by. The shipped
/// spelling is always among them, because a collection that declares none
/// of its own still has documents typed in ours.
fn declared_as(edges: &BTreeMap<String, String>, snake: &str, shipped: &str) -> Vec<String> {
    let mut found: Vec<String> = edges
        .iter()
        .filter(|(spelled, _)| spelled.rsplit(':').next() == Some(snake))
        .map(|(_, iri)| iri.clone())
        .collect();
    found.push(shipped.to_string());
    found.sort();
    found.dedup();
    found
}

impl Named {
    /// Whether this question is about one named document.
    fn takes_argument(self) -> bool {
        matches!(self, Named::Edges | Named::Mentions)
    }

    /// Whether this question is asked in the collection's own predicates
    /// rather than only in the shipped ones. `status` is not: the three
    /// terms it reads are the folio/v1 envelope's own, and a vocabulary of
    /// your own declares classes and edges, never a second spelling of
    /// `status:`.
    fn reads_the_collections_predicates(self) -> bool {
        !matches!(self, Named::Status)
    }

    /// This question's own name, for the errors that are about the question.
    fn label(self) -> &'static str {
        match self {
            Named::Status => "status",
            Named::Superseded => "superseded",
            Named::Edges => "edges",
            Named::Mentions => "mentions",
        }
    }

    /// The column at most one row may repeat, when this question asks the
    /// same thing more than one way and the first answer is the best one.
    fn one_row_per(self) -> Option<&'static str> {
        matches!(self, Named::Status).then_some("document")
    }

    /// The columns a table prints, in order.
    fn columns(self) -> Vec<String> {
        match self {
            Named::Status => ["document", "type", "status", "path"].map(Into::into).into(),
            Named::Superseded => ["document", "replacement", "path"].map(Into::into).into(),
            Named::Edges | Named::Mentions => ["predicate", "other"].map(Into::into).into(),
        }
    }

    /// The requests this question runs, each paired with the predicate label
    /// its rows carry (`None` when the request needs no label). `subject` is
    /// already an absolute IRI and `edges` is already the collection's own
    /// predicate table: both need the database, which is the caller's to
    /// read.
    fn requests(self, subject: Option<&str>, edges: &BTreeMap<String, String>)
        -> Vec<(Option<String>, QueryRequest)> {
        match self {
            Named::Status => vec![
                (None, QueryRequest {
                    premises: vec![json!({
                        "assert": { "with": {
                            "docType": described(&format!("{X0K}docType")),
                            "status": described(&format!("{X0K}status")),
                            "path": described(&format!("{X0K}folio/sourcePath")),
                        } },
                        "where": {
                            "this": {"?":{"name":"document"}},
                            "docType": {"?":{"name":"type"}},
                            "status": {"?":{"name":"status"}},
                            "path": {"?":{"name":"path"}},
                        }
                    })],
                    select: self.columns(),
                    ..Default::default()
                }),
                (None, QueryRequest {
                    premises: vec![json!({
                        "assert": { "with": {
                            "docType": described(&format!("{X0K}docType")),
                            "path": described(&format!("{X0K}folio/sourcePath")),
                        } },
                        "where": {
                            "this": {"?":{"name":"document"}},
                            "docType": {"?":{"name":"type"}},
                            "path": {"?":{"name":"path"}},
                        }
                    })],
                    select: vec!["document".into(), "type".into(), "path".into()],
                    ..Default::default()
                }),
            ],
            Named::Superseded => {
                // Authored on the superseded document, looking forward.
                let forward = declared_as(edges, "superseded_by", &format!("{X0K}supersededBy"))
                    .into_iter().map(|predicate| (None, QueryRequest {
                        premises: vec![json!({
                            "assert": { "with": {
                                "by": described(&predicate),
                                "path": described(&format!("{X0K}folio/sourcePath")),
                            } },
                            "where": {
                                "this": {"?":{"name":"document"}},
                                "by": {"?":{"name":"replacement"}},
                                "path": {"?":{"name":"path"}},
                            }
                        })],
                        select: self.columns(),
                        ..Default::default()
                    }));
                // Authored on the replacement, looking back. The path
                // wanted is still the superseded document's, and that
                // fact is about a different entity — hence the second
                // premise, joined on `document`.
                let backward = declared_as(edges, "supersedes", &format!("{X0K}supersedes"))
                    .into_iter().map(|predicate| (None, QueryRequest {
                        premises: vec![
                            json!({
                                "assert": { "with": { "replaces": described(&predicate) } },
                                "where": {
                                    "this": {"?":{"name":"replacement"}},
                                    "replaces": {"?":{"name":"document"}},
                                }
                            }),
                            json!({
                                "assert": { "with": { "path": described(&format!("{X0K}folio/sourcePath")) } },
                                "where": {
                                    "this": {"?":{"name":"document"}},
                                    "path": {"?":{"name":"path"}},
                                }
                            }),
                        ],
                        select: self.columns(),
                        ..Default::default()
                    }));
                forward.chain(backward).collect()
            }
            Named::Edges | Named::Mentions => {
                let subject = Value::String(subject.unwrap_or_default().to_string());
                let outgoing = matches!(self, Named::Edges);
                edges.iter().map(|(spelled, iri)| {
                    let (this, edge) = if outgoing {
                        (subject.clone(), json!({"?":{"name":"other"}}))
                    } else {
                        (json!({"?":{"name":"other"}}), subject.clone())
                    };
                    (Some(spelled.clone()), QueryRequest {
                        premises: vec![json!({
                            "assert": { "with": { "edge": described(iri) } },
                            "where": { "this": this, "edge": edge }
                        })],
                        select: vec!["other".into()],
                        ..Default::default()
                    })
                }).collect()
            }
        }
    }
}
```

The envelope IRIs these questions read are a published contract now — the
guide prints the field-to-predicate table — so the test asserts the exact
strings rather than round-tripping them through a database. A rename that
skipped the guide would otherwise show up only as a query that quietly
returns nothing, which is the failure mode the whole of this was about.

<a name="chunk-named-query-tests"></a><sub>[`src/main.rs`](../../crates/x0k-folio-cli/src/main.rs) · `#named-query-tests`</sub>

```rust {#named-query-tests}
#[cfg(test)]
mod named_query_tests {
    use super::*;

    /// Every predicate IRI a canned question names.
    fn predicates(named: Named, subject: Option<&str>) -> Vec<String> {
        let mut found = std::collections::BTreeSet::new();
        for (_, request) in named.requests(subject, &shipped_edge_predicates()) {
            collect_described(&request, &mut found);
        }
        found.into_iter().collect()
    }

    #[test]
    fn the_status_board_reads_the_declared_envelope_terms() {
        assert_eq!(predicates(Named::Status, None), vec![
            "https://0k.computer/ontology#docType".to_string(),
            "https://0k.computer/ontology#folio/sourcePath".to_string(),
            "https://0k.computer/ontology#status".to_string(),
        ]);
    }

    /// A document with no `status:` is still a document on the board, so the
    /// question asks a second time without that premise and keeps one row
    /// per document.
    #[test]
    fn the_board_asks_again_without_status_and_keeps_one_row_per_document() {
        let requests = Named::Status.requests(None, &BTreeMap::new());
        assert_eq!(requests.len(), 2);
        let second = &requests[1].1;
        let mut described = std::collections::BTreeSet::new();
        collect_described(second, &mut described);
        assert!(!described.contains("https://0k.computer/ontology#status"),
            "the second request must not require a status");
        assert_eq!(Named::Status.one_row_per(), Some("document"));
        assert_eq!(Named::Superseded.one_row_per(), None);
    }

    #[test]
    fn the_decision_log_question_reads_superseded_by() {
        assert!(predicates(Named::Superseded, None)
            .contains(&"https://0k.computer/ontology#supersededBy".to_string()));
    }

    /// The board reads the edge from both ends, because the vocabulary
    /// says it may be authored from either. A log typed `supersedes:`
    /// on the replacement came back empty (Backstage, 2026-09-23).
    #[test]
    fn the_board_reads_supersession_from_both_ends() {
        assert!(predicates(Named::Superseded, None)
            .contains(&"https://0k.computer/ontology#supersedes".to_string()));
        let requests = Named::Superseded.requests(None, &BTreeMap::new());
        assert_eq!(requests.len(), 2, "one request per spelling of the edge");
        for (_, request) in &requests {
            assert_eq!(request.select, Named::Superseded.columns(),
                "both spellings answer in the same columns");
        }
        // Read from the replacement's side, the path belongs to the
        // superseded document, so it is a second premise joined on
        // `document` rather than a field of the first.
        let backward = &requests[1].1;
        assert_eq!(backward.premises.len(), 2);
        assert_eq!(backward.premises[0]["where"]["replaces"],
            json!({"?":{"name":"document"}}));
        assert_eq!(backward.premises[1]["where"]["this"],
            json!({"?":{"name":"document"}}));
    }

    /// And it reads the collection's own spelling of the edge beside ours.
    /// A project that declares its own `supersededBy` got it through
    /// `check`, `index`, `ingest` and `--named edges`, and then the board
    /// asked about the shipped IRI alone and returned nothing (Backstage,
    /// 2026-09-23).
    #[test]
    fn the_board_reads_the_collections_own_supersession_predicate() {
        let mut edges = shipped_edge_predicates();
        edges.insert("bs:superseded_by".to_string(),
            "https://backstage.io/ontology#supersededBy".to_string());
        let described: Vec<String> = {
            let mut found = std::collections::BTreeSet::new();
            for (_, request) in Named::Superseded.requests(None, &edges) {
                collect_described(&request, &mut found);
            }
            found.into_iter().collect()
        };
        assert!(described.contains(&"https://backstage.io/ontology#supersededBy".to_string()),
            "the collection's own spelling is asked about: {described:?}");
        assert!(described.contains(&"https://0k.computer/ontology#supersededBy".to_string()),
            "and ours still is, because documents typed in ours are still here: {described:?}");
        assert_eq!(Named::Superseded.requests(None, &edges).len(), 3,
            "two forward spellings and one back");
        // `status` is not asked in the collection's terms: its three terms
        // are the folio/v1 envelope's own, and no module renames them.
        assert!(!Named::Status.reads_the_collections_predicates());
        assert!(Named::Superseded.reads_the_collections_predicates());
    }

    /// A predicate whose local name only looks like ours is not ours.
    #[test]
    fn a_predicate_that_merely_ends_in_the_same_word_is_not_collected() {
        let edges = BTreeMap::from([
            ("bs:not_superseded_by".to_string(), "https://backstage.io/ontology#notSupersededBy".to_string()),
        ]);
        assert_eq!(declared_as(&edges, "superseded_by", "https://0k.computer/ontology#supersededBy"),
            vec!["https://0k.computer/ontology#supersededBy".to_string()]);
    }

    #[test]
    fn an_edge_question_covers_every_shipped_predicate_one_request_each() {
        let requests = Named::Edges.requests(
            Some("https://0k.computer/ontology#design/a"), &shipped_edge_predicates());
        assert_eq!(requests.len(), x0k_ontology::KNOWN_EDGE_PREDICATES.len(),
            "one request per predicate: several premises in one request would be a conjunction");
        assert!(requests.iter().all(|(label, _)| label.is_some()),
            "every edge row is labelled with the predicate that produced it");
    }

    /// And a predicate no shipped module declares is asked about too, when
    /// the collection's own vocabulary declared it — the reader's edge is
    /// the one they most want back.
    #[test]
    fn an_edge_question_asks_over_the_collections_own_predicates() {
        let edges = BTreeMap::from([
            ("jj:superseded_by".to_string(), "https://jj-vcs.github.io/ontology#supersededBy".to_string()),
        ]);
        let requests = Named::Edges.requests(Some("https://jj-vcs.github.io/ontology#design/a"), &edges);
        assert_eq!(requests.len(), 1);
        assert_eq!(requests[0].0.as_deref(), Some("jj:superseded_by"));
        let mut described = std::collections::BTreeSet::new();
        collect_described(&requests[0].1, &mut described);
        assert!(described.contains("https://jj-vcs.github.io/ontology#supersededBy"), "got {described:?}");
    }

    fn table() -> BTreeMap<String, String> {
        BTreeMap::from([
            ("x0k".to_string(), X0K.to_string()),
            ("pyd".to_string(), "https://pydantic.dev/ontology#".to_string()),
        ])
    }

    #[test]
    fn a_compact_id_is_expanded_and_an_absolute_one_is_left_alone() {
        assert_eq!(expand_id("x0k:design/secure-config", &table()).unwrap(),
            "https://0k.computer/ontology#design/secure-config");
        assert_eq!(expand_id("https://example.org/papers#alpha", &table()).unwrap(),
            "https://example.org/papers#alpha");
    }

    /// The reader's own prefix expands into the reader's own namespace —
    /// the whole point of a vocabulary you declare yourself.
    #[test]
    fn a_declared_extension_prefix_expands_into_its_own_namespace() {
        assert_eq!(expand_id("pyd:concept/strict-mode", &table()).unwrap(),
            "https://pydantic.dev/ontology#concept/strict-mode");
    }

    /// And one nothing declares is refused by name, rather than silently
    /// becoming a question about a document in our namespace.
    #[test]
    fn an_undeclared_prefix_is_refused_and_names_what_is_declared() {
        let error = expand_id("zzz:concept/strict-mode", &table()).unwrap_err().to_string();
        assert!(error.contains("zzz:"), "got {error}");
        assert!(error.contains("pyd:") && error.contains("x0k:"), "got {error}");
        assert!(expand_id("strict-mode", &table()).is_err(), "a bare word is not an id");
    }

    /// The same expansion is what refuses an `--arg` that is not an id at
    /// all, before a database is opened (Backstage, 2026-09-23: `--arg
    /// 'not an id'` surfaced Dialog's `Cannot assign variable: …`).
    #[test]
    fn an_arg_that_is_not_an_id_is_refused_before_the_database_opens() {
        assert!(expand_id("not an id", &table()).is_err());
        assert!(expand_id("adr014", &table()).is_err());
        assert!(expand_id("x0k:design/secure-config", &table()).is_ok());
    }

    /// A failed ingest names the file and the reason on stderr
    /// (Backstage, 2026-09-23: stderr said only "see the reconciliation
    /// report", and `pending_sources` was empty).
    #[test]
    fn an_incomplete_reconciliation_names_its_sources() {
        assert_eq!(incomplete_reason(&json!({"complete": true})), None);
        let reason = incomplete_reason(&json!({
            "complete": false,
            "diagnostics": [
                {"path": "docs/ok.md", "error": null, "non_folio": false},
                {"path": "docs/bad.md", "error": "envelope: unknown type `adr`", "non_folio": false},
            ],
            "pending_sources": ["docs/slow.md"],
            "changed_during_scan": [],
        })).expect("an incomplete reconciliation has a reason");
        assert!(reason.contains("docs/bad.md: envelope: unknown type `adr`"), "{reason}");
        assert!(reason.contains("docs/slow.md"), "{reason}");
        assert!(!reason.contains("docs/ok.md"), "{reason}");
        assert!(reason.starts_with("2 sources did not reconcile:"), "{reason}");
    }

    #[test]
    fn mentions_binds_the_subject_on_the_object_side() {
        let subject = "https://0k.computer/ontology#design/a";
        let (_, request) = Named::Mentions
            .requests(Some(subject), &shipped_edge_predicates()).remove(0);
        let clause = &request.premises[0]["where"];
        assert_eq!(clause["edge"], Value::String(subject.into()));
        assert!(clause["this"]["?"].is_object(), "the other end stays a variable");
    }
}
```

## What a query file is made of

A query file is one JSON object with four keys — `premises`, optional
`rules`, `select`, and the `max_rows`/`timeout_ms` budgets. All the shape is
in a premise, and a premise is smaller than it looks:

```json
{ "assert": { "with": { "<field>": { "the": "<predicate IRI>", "cardinality": "many" } } },
  "where":  { "this": <term>, "<field>": <term> } }
```

`with` names the predicates this premise reads, each under a local field
name. `where` binds two kinds of position: `this` is the entity the facts are
*about*, and each field is that predicate's value. A `<term>` is either a
variable, `{"?": {"name": "doc"}}`, or a literal — and a bare IRI string is a
literal entity, which is how a question about one document is asked:
`"this": "https://0k.computer/ontology#design/secure-config"`.

Two rules follow from that and account for most first attempts going wrong.
**Fields in one premise are a conjunction**: a row exists only where the
entity has every predicate the `with` map names, so adding `summary` to a
status query silently drops every document that has none. **Premises join on
shared variable names**, so the way to follow an edge is to bind one
premise's value variable as another premise's `this` — that is exactly what
the citation example's second premise does to reach the document's path.
`select` then lists the variable names to print, in column order.

The names in `select` must be bound somewhere; an unknown one is the native
lookup error. What is *not* an error is a predicate no fact uses: to a
datalog engine that is a join matching nothing, so the query succeeds with
zero rows. Because a misspelled IRI and an honest empty answer are the same
result, an empty answer carries a note naming each described predicate the
database holds no fact for.

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
reconciliation command fails when any source is rejected or pending, and
it fails *by name*: the same diagnostics the report carries are written to
stderr, one line per source, saying which file and what was wrong with it.
That sounds like a restatement and was not one — the command used to exit
with "some sources were rejected or remain pending; see the reconciliation
report", which is a true sentence that leaves a person `jq`-ing two hundred
lines of stdout for a filename they are about to fix.
What the command waits for depends on which verb it is. `watch` runs a loop
and cannot stall it, so it waits a bounded 30 seconds per source transaction
and calls a slower one pending, to be replayed next pass. `ingest` and
`rebuild` read a directory that is sitting still and wait for the store to
finish: there is nothing racing the write, and abandoning one leaves the
worker busy and every later source refused — a fifteen-document collection
ending with sixteen pending sources and an empty database. `--delivery-grace-ms`
imposes the bounded wait on any of them. A transaction completing after a
bounded wait remains unacknowledged until a subsequent reconciliation confirms
it.

Which is to say what `ingest` is *for*: a nightly job or a pre-merge step,
seconds per hundred documents rather than milliseconds, and not a
save-hook. `watch` is the save-hook shape.
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

0.1.1 because the query surface changed after 0.1.0 went out: `--named`
answers four questions, `--explain` prints a file that runs, `--arg` expands
a reader's own prefix, and `ingest` defaults to the shipped vocabulary rather
than to none. A caller pinned to 0.1.0 gets none of that, and the CHANGELOG's
`x0k-folio-cli` section is the list.

<a name="chunk-cli-manifest"></a><sub>[`Cargo.toml`](../../crates/x0k-folio-cli/Cargo.toml) · `#cli-manifest`</sub>

```toml {#cli-manifest file="Cargo.toml"}
[package]
name = "x0k-folio-cli"
version = "0.1.1"
# Not on crates.io: depends on `x0k-folio-dialog`, which cannot be
# published. The binary ships in the GitHub release and the npm wrapper.
publish = false
edition = { workspace = true }
license = "MIT"
description = "Standalone Folio ingestion and Dialog queries"
rust-version = { workspace = true }
repository = "https://github.com/0k-dot-computer/x0k-folio"
readme = "../../README.md"
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
x0k-folio = { path = "../x0k-folio", default-features = false, features = ["document-vocabulary"] , version = "0.1.1" }
x0k-ontology = { path = "../x0k-ontology", features = ["load"] , version = "0.1.0" }
x0k-fact-projection = { path = "../x0k-fact-projection" , version = "0.1.1" }
x0k-folio-ingest = { path = "../x0k-folio-ingest" , version = "0.1.0" }
x0k-folio-dialog = { path = "../x0k-folio-dialog" , version = "0.1.0" }

[dev-dependencies]
tempfile = "3"

[[bin]]
name = "x0k-folio-cli"
path = "src/main.rs"
```

## Command entry

<a name="chunk-cli-main"></a><sub>[`src/main.rs`](../../crates/x0k-folio-cli/src/main.rs) · `#cli-main` · assembles [named-queries](#chunk-named-queries) · [named-requests](#chunk-named-requests) · [named-query-tests](#chunk-named-query-tests)</sub>

```rust {#cli-main}
use std::{collections::BTreeMap, io::Write, path::{Path, PathBuf}, sync::{Mutex, Arc, atomic::{AtomicBool, Ordering}}, time::{SystemTime, UNIX_EPOCH}};
use anyhow::{Context, Result, ensure};
use clap::{Args, Parser, Subcommand, ValueEnum};
use fs2::FileExt;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use x0k_fact_projection::FactValue;
use x0k_folio::document_vocabulary as vocabulary;
use x0k_folio_cli::source::FolioSource;
use x0k_folio_dialog::{DialogBackend, QueryRequest, QueryResult};
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
    /// Load the ontology modules in this directory, in addition to the
    /// vocabulary bundled with this build.
    #[arg(long)] vocabulary: Option<PathBuf>,
    /// Read --vocabulary alone, without the bundled set.
    #[arg(long, requires = "vocabulary")] only_vocabulary: bool,
    /// The bundled vocabulary, which is also the default; naming it says so.
    /// It does not conflict with `--vocabulary`, because `--vocabulary` is
    /// additive: naming both is a reader saying out loud what is already
    /// true. The rule that refused the pair was right when the two flags
    /// were alternatives and was left behind when they stopped being
    /// (jj, 2026-09-23).
    #[arg(long)] shipped: bool,
    #[arg(long, default_value = "dialog")] backend: String,
    /// Wait this long (1..=30000 ms) for a source transaction before
    /// reporting it pending. Default: `watch` waits 30000 ms so its loop
    /// keeps moving; `ingest` and `rebuild` wait for the store to finish,
    /// because a directory that is sitting still is not racing anything.
    #[arg(long, value_parser = clap::value_parser!(u64).range(1..=30_000))]
    delivery_grace_ms: Option<u64>,
}
#[derive(Args)]
struct Query {
    #[arg(long)] database: PathBuf,
    /// A native Dialog query file. Exclusive with --named.
    #[arg(long, required_unless_present = "named", conflicts_with = "named")] file: Option<PathBuf>,
    /// One of the canned questions, needing no query file.
    #[arg(long, value_enum)] named: Option<Named>,
    /// The document a canned question is about, as its compact id.
    #[arg(long, requires = "named")] arg: Option<String>,
    /// Print the request a canned question would run, instead of running it.
    #[arg(long, requires = "named")] explain: bool,
    #[arg(long, default_value = "dialog")] backend: String,
    #[arg(long, value_enum, default_value_t = Output::Table)] format: Output,
    #[arg(long)] max_rows: Option<usize>,
    #[arg(long)] timeout_ms: Option<u64>,
}
#[derive(Clone, Copy, ValueEnum)]
enum Output { Table, Json }

<<named-queries>>

<<named-requests>>

<<named-query-tests>>

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
/// The batch verbs' delivery policy: an explicit `--delivery-grace-ms` when
/// the caller insisted on one, and otherwise quiescence — `ingest` and
/// `rebuild` read a directory that is sitting still, so there is nothing for
/// a clock to protect them from.
fn batch_grace(args: &Corpus) -> Option<std::time::Duration> {
    args.delivery_grace_ms.map(std::time::Duration::from_millis)
}
/// The watcher's: the same flag when given, and otherwise the thirty-second
/// bound its loop has always run under, because a loop must keep moving.
fn watch_grace(args: &Corpus) -> std::time::Duration {
    std::time::Duration::from_millis(args.delivery_grace_ms.unwrap_or(30_000))
}
/// The vocabulary this reconciliation reads documents against. The choice
/// is [`select_vocabulary`](x0k_folio::document_vocabulary::select_vocabulary),
/// the one `check` makes, so a document the checker accepts is a document
/// the ingest can project.
///
/// There is no "no vocabulary at all" any more. Forgetting the flag used to
/// mean every document carrying an `edges:` block was rejected for a term
/// folio/v1 itself declares — a store 55% populated and an exit code, which
/// is worse than a refusal and much worse than the obvious default.
fn base_model(args: &Corpus) -> Result<OntologyModel> {
    vocabulary::select_vocabulary(args.vocabulary.as_deref(), args.only_vocabulary)
        .map_err(|error| anyhow::anyhow!(error))
}
/// Why a reconciliation did not complete, said in full: which documents
/// were rejected and what was wrong with each, which sources are still
/// pending, which changed under the scan. `None` when it completed.
///
/// The report has always carried this. What stderr carried was "some
/// sources were rejected or remain pending; see the reconciliation
/// report", which put the one fact a person needs — *which file* — behind
/// a `jq .diagnostics` over two hundred lines of stdout. Failing is right;
/// making the reader excavate the filename is not.
fn incomplete_reason(report: &Value) -> Option<String> {
    if report["complete"].as_bool() == Some(true) {
        return None;
    }
    let mut lines = Vec::new();
    let strings = |key: &str| -> Vec<String> {
        report[key].as_array().into_iter().flatten()
            .filter_map(|value| value.as_str().map(str::to_string)).collect()
    };
    for diagnostic in report["diagnostics"].as_array().into_iter().flatten() {
        if let Some(error) = diagnostic["error"].as_str() {
            lines.push(format!("  {}: {error}",
                diagnostic["path"].as_str().unwrap_or("<unnamed source>")));
        }
    }
    for path in strings("pending_sources") {
        lines.push(format!("  {path}: delivered, not yet acknowledged by the backend"));
    }
    for path in strings("changed_during_scan") {
        lines.push(format!("  {path}: edited while the scan was reading it"));
    }
    if report["cancelled"].as_bool() == Some(true) {
        lines.push("  cancelled before every source was reconciled".to_string());
    }
    let head = match lines.len() {
        0 => "reconciliation did not complete; see the reconciliation report on stdout".to_string(),
        1 => "1 source did not reconcile:".to_string(),
        count => format!("{count} sources did not reconcile:"),
    };
    Some(std::iter::once(head).chain(lines).collect::<Vec<_>>().join("\n"))
}

async fn reconcile(writer: &Writer, args: &Corpus, grace: Option<std::time::Duration>) -> Result<Value> {
    let started = now().to_string();
    let status_path = writer.path().join("status.json");
    save_json(&status_path, &json!({
        "generation": writer.manifest.generation, "backend": writer.manifest.backend,
        "root": writer.manifest.root, "started_unix_ns": started, "reconciling": true
    }))?;
    let result = reconcile_inner(writer, args, &started, grace).await;
    match result {
        Ok(report) => {
            save_json(&status_path, &report)?;
            emit(&report)?;
            Ok(report)
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
async fn reconcile_inner(writer: &Writer, args: &Corpus, started: &str, grace: Option<std::time::Duration>) -> Result<Value> {
    let origin = match &args.vocabulary {
        Some(path) if args.only_vocabulary => path.canonicalize()?.to_string_lossy().into_owned(),
        Some(path) => format!("shipped vocabulary and {}", path.canonicalize()?.display()),
        None => "shipped vocabulary".to_string(),
    };
    let mut source = FolioSource::prepare_with_provenance(&writer.manifest.root, base_model(args)?, &origin)?;
    let diagnostics = source.diagnostics().to_vec();
    let state_path = writer.path().join("checkpoint");
    let backends = Mutex::new(vec![writer.backend.backend(&writer.manifest.backend)
        .grace_policy(grace)]);
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
        "reconciling": false, "delivery_grace_ms": grace.map(|grace| grace.as_millis() as u64), "complete": invalid == 0 && pending.is_empty() && changed_during_scan.is_empty() && !writer.cancelled.load(Ordering::Relaxed),
        "cancelled": writer.cancelled.load(Ordering::Relaxed),
        "vocabulary_revision": source.fingerprint(),
        "namespaces": source.namespaces(),
        "edge_predicates": source.edge_predicates(),
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
/// Every predicate IRI a request describes: the `the` of each entry in each
/// premise's `with` map, found wherever it sits, since a rule nests them.
fn collect_described(request: &QueryRequest, into: &mut std::collections::BTreeSet<String>) {
    fn walk(value: &Value, into: &mut std::collections::BTreeSet<String>) {
        match value {
            Value::Object(fields) => {
                if let Some(Value::String(iri)) = fields.get("the") {
                    into.insert(iri.clone());
                }
                for nested in fields.values() { walk(nested, into); }
            }
            Value::Array(values) => for nested in values { walk(nested, into); },
            _ => {}
        }
    }
    for premise in request.premises.iter().chain(request.rules.iter()) { walk(premise, into); }
}

/// Of the predicates a query described, those no fact in the database uses.
/// One unconstrained single-row probe each — run only when the answer was
/// empty, which is when the distinction between "asked and got nothing" and
/// "asked for a word this collection does not speak" is the one a reader
/// needs.
async fn unused_predicates(
    reader: &DialogBackend,
    predicates: &std::collections::BTreeSet<String>,
) -> Result<Vec<String>> {
    let mut unused = Vec::new();
    for predicate in predicates {
        let probe = QueryRequest {
            premises: vec![json!({
                "assert": { "with": { "value": described(predicate) } },
                "where": { "this": {"?":{"name":"entity"}}, "value": {"?":{"name":"value"}} }
            })],
            select: vec!["entity".into()],
            max_rows: 1,
            ..Default::default()
        };
        if reader.query(probe).await?.rows.is_empty() { unused.push(predicate.clone()); }
    }
    Ok(unused)
}

/// Whether this database holds a document at this id — one single-row probe
/// for the source path every projected document carries. Run only when a
/// question about one named document came back empty, because "this document
/// has no edges of that kind" and "nothing here is this document" are
/// different answers, and the second one is a mistyped or unindexed id.
async fn document_at(reader: &DialogBackend, subject: &str) -> Result<bool> {
    let probe = QueryRequest {
        premises: vec![json!({
            "assert": { "with": { "path": described(&format!("{X0K}folio/sourcePath")) } },
            "where": { "this": subject, "path": {"?":{"name":"path"}} }
        })],
        select: vec!["path".into()],
        max_rows: 1,
        ..Default::default()
    };
    Ok(!reader.query(probe).await?.rows.is_empty())
}

/// A query file: one bare request, or the `{"requests": […]}` envelope
/// `--explain` prints — which carries a `predicate` label per request and
/// the column a repeated answer may not repeat. Reading both is what makes
/// the printed request a file you can run unedited.
fn read_query_file(bytes: &[u8]) -> Result<QueryPlan> {
    let document: Value = serde_json::from_slice(bytes)?;
    let Some(entries) = document.get("requests").and_then(Value::as_array) else {
        let request: QueryRequest = serde_json::from_value(document)?;
        let columns = request.select.clone();
        return Ok((vec![(None, request)], columns, None));
    };
    let one_row_per = document.get("one_row_per")
        .and_then(Value::as_str).map(str::to_string);
    let mut requests = Vec::new();
    for entry in entries {
        let mut entry = entry.clone();
        let label = entry.as_object_mut()
            .and_then(|fields| fields.remove("predicate"))
            .and_then(|label| label.as_str().map(str::to_string));
        requests.push((label, serde_json::from_value::<QueryRequest>(entry)?));
    }
    ensure!(!requests.is_empty(), "a query file's requests must not be empty");
    let mut columns = requests[0].1.select.clone();
    if requests.iter().any(|(label, _)| label.is_some()) {
        columns.insert(0, "predicate".into());
    }
    Ok((requests, columns, one_row_per))
}

/// What a question resolves to before it is run: the requests, the columns a
/// table prints, and the column a repeated answer may not repeat.
type QueryPlan = (Vec<(Option<String>, QueryRequest)>, Vec<String>, Option<String>);

fn cell(value: &FactValue) -> String {
    match value {
        FactValue::EntityRef(v) | FactValue::Symbol(v) => format!("<{v}>"),
        FactValue::Text(v) => serde_json::to_string(v).unwrap_or_default(),
        _ => typed(value).to_string(),
    }
}

/// A path inside this database's corpus root, as the reader wrote it.
///
/// The stored fact stays absolute — it has to, because the ingester is the
/// only thing that knows where the root was — but a board whose every row
/// carries the same forty-character prefix is a board you have to strip
/// before you can publish it. `None` for anything that is not a path under
/// the root, which is every other cell.
fn relative_to_root(value: &FactValue, root: &Path) -> Option<String> {
    let FactValue::Text(text) = value else { return None };
    let relative = Path::new(text).strip_prefix(root).ok()?;
    Some(relative.to_string_lossy().into_owned())
}

/// A table cell, rooted.
fn under_root(value: &FactValue, root: &Path) -> String {
    match relative_to_root(value, root) {
        Some(relative) => serde_json::to_string(&relative).unwrap_or_default(),
        None => cell(value),
    }
}

/// A JSON cell, rooted the same way.
///
/// The two views used to disagree: the table stripped the prefix and the
/// JSON kept it, so the board you read by eye and the board you script were
/// shaped differently, and the scripted one still needed a `sed` (jj,
/// 2026-09-23). One convention, and it is the reader's. The cell keeps its
/// `{type, value}` shape — a script reads the same field it always did, and
/// finds a path it can open.
fn typed_under_root(value: &FactValue, root: &Path) -> Value {
    match relative_to_root(value, root) {
        Some(relative) => json!({"type":"text","value":relative}),
        None => typed(value),
    }
}
async fn query(args: Query) -> Result<()> {
    // A canned question may need answering before the store is opened:
    // --explain is about the request, not about any stored facts. Expanding
    // --arg still reads the database's recorded prefix table, because the
    // request carries the expanded IRI and only that table knows it.
    let mut subject = None;
    let (mut requests, columns, one_row_per) = match args.named {
        Some(named) => {
            let edges = if named.reads_the_collections_predicates() {
                let recorded = recorded(&args.database)?;
                if named.takes_argument() {
                    let arg = args.arg.as_deref().with_context(||
                        format!("--named {} is about one document; name it with --arg <id>", named.label()))?;
                    subject = Some(expand_id(arg, &recorded.namespaces)
                        .map_err(|error| anyhow::anyhow!("--arg {arg}: {error:#}"))?);
                }
                recorded.edge_predicates
            } else {
                BTreeMap::new()
            };
            (named.requests(subject.as_deref(), &edges), named.columns(),
                named.one_row_per().map(str::to_string))
        }
        None => {
            let file = args.file.as_ref().expect("clap requires --file without --named");
            read_query_file(&std::fs::read(file)?)?
        }
    };
    for (_, request) in &mut requests {
        if let Some(limit) = args.max_rows { request.max_rows = limit; }
        if let Some(timeout) = args.timeout_ms { request.timeout_ms = timeout; }
    }
    if args.explain {
        let printed: Vec<_> = requests.iter().map(|(label, request)| {
            let mut entry = json!({
                "premises": request.premises, "rules": request.rules,
                "select": request.select, "max_rows": request.max_rows,
                "timeout_ms": request.timeout_ms,
            });
            // A null label is not a label; printing one would put a field in
            // the file that says nothing and has to be deleted before it runs.
            if let Some(label) = label {
                entry["predicate"] = Value::String(label.clone());
            }
            entry
        }).collect();
        let mut document = json!({
            "requests": printed,
            "view": "a query file: run it with --file, or edit it first"
        });
        if let Some(key) = args.named.and_then(Named::one_row_per) {
            document["one_row_per"] = Value::String(key.into());
        }
        return emit(&document);
    }

    let manifest = read_manifest(&args.database)?;
    ensure!(args.backend == manifest.backend, "backend {} is not enabled; this database enables {}", args.backend, manifest.backend);
    let path = generation_path(&args.database, &manifest);
    let reader = DialogBackend::open_reader(path.join("store"))?;
    ensure!(reader.instance_id() == manifest.instance, "database store identity does not match its checkpoint");
    let before = std::fs::read(path.join("status.json")).ok();
    let mut described_predicates = std::collections::BTreeSet::new();
    let mut result = QueryResult { rows: Vec::new(), truncated: false };
    let mut seen = std::collections::BTreeSet::new();
    for (label, request) in requests {
        collect_described(&request, &mut described_predicates);
        let mut answered = reader.query(request).await?;
        if let Some(label) = &label {
            for row in &mut answered.rows {
                row.insert("predicate".into(), FactValue::Text(label.clone()));
            }
        }
        result.truncated |= answered.truncated;
        // One supersession authored from both ends is one edge, and both
        // requests find it. Rows carry no identity of their own, so the
        // rendered row is the identity.
        for row in answered.rows {
            let fingerprint = serde_json::to_string(
                &row.iter().map(|(name, value)| (name, typed(value))).collect::<BTreeMap<_, _>>(),
            )?;
            if seen.insert(fingerprint) { result.rows.push(row); }
        }
    }
    // A question asked more than one way answers some documents twice; the
    // first request is the one with the most to say, so its row stands.
    if let Some(key) = one_row_per.as_deref() {
        let mut seen = std::collections::BTreeSet::new();
        result.rows.retain(|row| match row.get(key) {
            Some(value) => seen.insert(cell(value)),
            None => true,
        });
    }
    // An empty answer is a legitimate result and also the shape a mistyped id
    // or a misspelled predicate takes. Which of those it is, is one probe: if
    // the document itself is not here, say that and stop — the thirty
    // predicates it does not carry are noise beside it.
    let missing_subject = match &subject {
        Some(subject) if result.rows.is_empty() && !document_at(&reader, subject).await? =>
            Some(subject.clone()),
        _ => None,
    };
    // They are noise when the document *is* here, too. A question about one
    // document answers about that document, so an empty answer says this one
    // carries no edge of the kind asked for, and the predicate census is left
    // to the questions that are about predicates.
    let empty_subject = subject.clone()
        .filter(|_| result.rows.is_empty() && missing_subject.is_none());
    let unused = if result.rows.is_empty() && subject.is_none() {
        unused_predicates(&reader, &described_predicates).await?
    } else {
        Vec::new()
    };
    let after = std::fs::read(path.join("status.json")).ok();
    let report = after.as_ref().map(|bytes| serde_json::from_slice::<Value>(bytes)).transpose()?;
    match args.format {
        Output::Json => {
            let rows: Vec<BTreeMap<_,_>> = result.rows.iter().map(|row|
                row.iter().map(|(key,value)| (key, typed_under_root(value, &manifest.root))).collect()).collect();
            // The whole reconciliation report used to sit in this object —
            // three hundred lines of diagnostics ahead of the answer, since
            // the keys print sorted. What a query's reader needs from it is
            // whether the last scan finished and what it refused; `status`
            // is the verb that prints the rest.
            let summary = report.as_ref().map(|report| json!({
                "complete": report["complete"], "finished_unix_ns": report["finished_unix_ns"],
                "valid_documents": report["valid_documents"], "invalid_documents": report["invalid_documents"],
                "view": "run `status --database <db>` for the full reconciliation report",
            }));
            emit(&json!({
                "backend": manifest.backend, "generation": manifest.generation,
                "rows": rows, "truncated": result.truncated,
                "predicates_no_fact_uses": unused,
                "no_document_has_this_id": missing_subject,
                "no_edge_here_for_this_id": empty_subject,
                "reconciliation_changed_during_query": before != after,
                "last_reconciliation": summary,
                "freshness": "last reconciliation describes observed files; edits since that scan may not be indexed"
            }))?;
        }
        Output::Table => {
            let mut out = std::io::stdout().lock();
            writeln!(out, "{}", columns.join("\t"))?;
            for row in &result.rows {
                writeln!(out, "{}", columns.iter()
                    .map(|name| row.get(name).map(|value| under_root(value, &manifest.root)).unwrap_or_default())
                    .collect::<Vec<_>>().join("\t"))?;
            }
            writeln!(out, "\n{} {}{} · backend {} · generation {}", result.rows.len(),
                if result.rows.len() == 1 { "row" } else { "rows" },
                if result.truncated { " (limit reached)" } else { "" },
                manifest.backend, manifest.generation)?;
            if let Some(subject) = &missing_subject {
                writeln!(out, "note: no document in this database has the id <{subject}>")?;
            }
            for predicate in &unused {
                writeln!(out, "note: no fact in this database uses <{predicate}>")?;
            }
            if let Some(subject) = &empty_subject {
                writeln!(out, "note: <{subject}> is in this database; no edge of the kind asked for is recorded here")?;
            }
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
            let result = reconcile(&writer, &args, batch_grace(&args)).await;
            writer.publish()?;
            match incomplete_reason(&result?) {
                Some(reason) => Err(anyhow::anyhow!(reason)),
                None => Ok(()),
            }
        }
        Command::Rebuild(args) => {
            let writer = Writer::open(&args, true)?;
            let report = reconcile(&writer, &args, batch_grace(&args)).await?;
            if let Some(reason) = incomplete_reason(&report) {
                anyhow::bail!("rebuild incomplete; the previous generation remains selected\n{reason}");
            }
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
                let result = reconcile(&writer, &corpus, Some(watch_grace(&corpus))).await;
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
