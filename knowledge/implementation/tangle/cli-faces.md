---
x0k:
  format: folio/v1
  id: x0k:implementation/tangle/cli-faces
  type: implementation
  status: draft
  summary: "The two verbs that make a shipped affordance true from the command line: an envelope read against the vocabulary this build compiled, and an affordance declaration read out as data — each proven by running the binary the repository ships."
  concerns: [tangle, cli, folio, vocabulary, affordance, publishing]
  tangle:
    crate: x0k-tangle
    root: src/faces.rs
  edges:
    implements:
      - x0k:design/publish-a-region-as-a-repository
      - x0k:affordance/check_a_document_against_shipped_vocabulary
      - x0k:affordance/read_declared_affordances
    cites:
      - x0k:implementation/tangle/crate
      - x0k:implementation/folio/checking
      - x0k:implementation/folio/inline-entities
---
# The faces behind `check` and `affordances`

Two of the affordances the `x0k-folio` publication ships claim a human:
[checking a document against the vocabulary that shipped beside it](../../../decisions/design/corpus/publish-a-region-as-a-repository/check-a-document-against-its-vocabulary.md "x0k:affordance/check_a_document_against_shipped_vocabulary"), and
[reading the affordances a document declares as data](../../../decisions/design/corpus/publish-a-region-as-a-repository/read-an-affordance-out-of-a-document.md "x0k:affordance/read_declared_affordances"). Both were true of
the *library* — `x0k_folio::check_corpus` and
`x0k_folio::extract_from_markdown` are public and published — and false
of the *repository*, because nothing a person could run reached either.
The CLI's `check` verified chunk references and stopped; no verb
printed a declaration. A claim on a perception-dependent actor with
nothing to perceive is a false claim, and the design now says so
(`x0k:design/publish-a-region-as-a-repository`, amendment of
2026-09-05). This module is what makes the two claims true from a
shell.

It holds the mechanism and none of the printing. Both binaries — the
protocol-only `x0k-tangle/src/main.rs` the repository ships and the
monorepo's bundle mirror — call the two functions here and format the
reports themselves, so the verbs stay identical across the pair without
the bundle growing a dependency on `x0k-folio`. The signifiers for the
verbs live in [`crate.md`](crate.md), under each verb's own heading,
because a signifier is declared where its face lives.

<a name="chunk-doc"></a><sub>[`src/faces.rs`](../../../x0k-tangle/src/faces.rs) · `#doc`</sub>

```rust {#doc}
//! The mechanism behind the `check` and `affordances` CLI verbs: every
//! folio/v1 envelope under a set of paths read against the vocabulary
//! this build compiled, and every inline affordance declaration read
//! out as a record. Both `x0k-tangle` binaries call these and do their
//! own printing.
```

<a name="chunk-imports"></a><sub>[`src/faces.rs`](../../../x0k-tangle/src/faces.rs) · `#imports`</sub>

```rust {#imports}
use std::collections::{BTreeMap, HashSet};
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use serde::Serialize;
use x0k_folio::colophon::{is_colophon, parse_envelope, Colophon, DocType};
use x0k_folio::envelope_check::{DanglingEdge, Defect};
use x0k_folio::{
    check_corpus, check_declarations, declared_facts, document_edges, extract_from_markdown,
    CorpusReport, DeclarationReport, InlineEntity,
};

use crate::parser::{parse_document, ParsedDocument};
```

## The proving chunks

A third thing a document can say about an affordance lives in neither
the envelope nor a `yaml` block: a code fence that tangles a test may
carry `proves="<affordance id>"` ([`parsing.md`](parsing.md)), which
makes the test the evidence for the claim. All three faces here — the
check, the `affordances` verb, and the repository projector — read that
the same way, so the reader is one function: every chunk of a parsed
document with a non-empty `proves`, with the file it tangles to and the
`#[test]` functions in its bodies. A test function is the `fn` directly
after a `#[test]` line, other attributes between them allowed; nothing
subtler, because the projector asks cargo for these names and cargo's
own filter is no subtler either.

<a name="chunk-proving-chunks"></a><sub>[`src/faces.rs`](../../../x0k-tangle/src/faces.rs) · `#proving-chunks`</sub>

```rust {#proving-chunks}
/// One chunk of a tangled document that says what it proves.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProvingChunk {
    /// The chunk's `#name` — with the document's id, the address of
    /// the edge: `<document id>#<chunk>`.
    pub chunk: String,
    /// The crate-relative file the chunk tangles to: its own `file=`,
    /// else the document's `root:`. `None` when the document declares
    /// neither.
    pub file: Option<PathBuf>,
    /// The affordance ids it proves, as written.
    pub proves: Vec<String>,
    /// The `#[test]` functions in its bodies, in order.
    pub tests: Vec<String>,
    /// Each test's source as the chapter tangles it, aligned with
    /// `tests`: what an affordance's page shows under the test's name.
    pub sources: Vec<String>,
}

/// Every chunk of `parsed` carrying `proves=`, in declaration order.
pub fn proving_chunks(parsed: &ParsedDocument) -> Vec<ProvingChunk> {
    let mut out = Vec::new();
    for name in &parsed.chunk_order {
        for chunk in parsed.chunk_variants(name).unwrap_or_default() {
            if chunk.proves.is_empty() {
                continue;
            }
            let sources = test_fn_sources(&chunk.combined_body());
            out.push(ProvingChunk {
                chunk: name.clone(),
                file: chunk.file_target.clone().or_else(|| parsed.tangle_root.clone()),
                proves: chunk.proves.clone(),
                tests: sources.iter().map(|(name, _)| name.clone()).collect(),
                sources: sources.into_iter().map(|(_, source)| source).collect(),
            });
        }
    }
    out
}

/// The `#[test]` functions in a body: each `fn <name>` after a
/// `#[test]` line, with any further attributes between them skipped.
pub fn test_fn_names(body: &str) -> Vec<String> {
    test_fn_sources(body).into_iter().map(|(name, _)| name).collect()
}

/// Each `#[test]` function in a body with its source: the lines from
/// its `#[test]` to the line before the next, trailing blank lines and
/// any line closing an enclosing item (indented less than the attribute,
/// as a `mod tests` brace is) dropped, and the whole dedented to the
/// margin. A `#[test]` followed by no `fn` names nothing.
pub fn test_fn_sources(body: &str) -> Vec<(String, String)> {
    let lines: Vec<&str> = body.lines().collect();
    let starts: Vec<usize> = lines
        .iter()
        .enumerate()
        .filter(|(_, l)| l.trim().starts_with("#[test]"))
        .map(|(i, _)| i)
        .collect();
    let mut out = Vec::new();
    for (k, &start) in starts.iter().enumerate() {
        let end = starts.get(k + 1).copied().unwrap_or(lines.len());
        let indent = lines[start].len() - lines[start].trim_start().len();
        let mut span: Vec<&str> = lines[start..end].to_vec();
        while span.last().is_some_and(|l| l.trim().is_empty() || l.len() - l.trim_start().len() < indent) {
            span.pop();
        }
        let name = span.iter().skip(1).map(|l| l.trim()).find(|t| !t.starts_with("#[")).and_then(|t| {
            let i = t.find("fn ")?;
            let name: String = t[i + 3..].chars().take_while(|c| c.is_alphanumeric() || *c == '_').collect();
            (!name.is_empty()).then_some(name)
        });
        let Some(name) = name else { continue };
        let source: Vec<String> = span
            .iter()
            .map(|l| {
                let lead = l.len() - l.trim_start().len();
                l[lead.min(indent)..].to_string()
            })
            .collect();
        out.push((name, source.join("\n")));
    }
    out
}
```

## Which documents

The literate verbs discover documents by content sniff — a `.md` that
mentions `tangle:` — because a tangler only cares about those. These
two verbs care about every folio/v1 document, tangling or not: a design
document declares affordances and tangles nothing. So discovery here
is the format's own cheap gate, `is_colophon`, which reads the
frontmatter and nothing else. A file path is taken as given; a
directory is walked. The result is sorted so two runs over the same
tree print in the same order.

<a name="chunk-discover"></a><sub>[`src/faces.rs`](../../../x0k-tangle/src/faces.rs) · `#discover`</sub>

```rust {#discover}
/// Every `.md` under `paths` whose frontmatter claims folio/v1, sorted.
/// A path that is a file is taken as given; a directory is walked.
pub fn discover_folio_documents(paths: &[PathBuf]) -> Result<Vec<PathBuf>> {
    let mut docs = Vec::new();
    for path in paths {
        if path.is_file() {
            if claims_folio(path) {
                docs.push(path.clone());
            }
        } else if path.is_dir() {
            for entry in walkdir::WalkDir::new(path)
                .into_iter()
                .filter_map(|e| e.ok())
            {
                let p = entry.path();
                if p.extension().is_some_and(|e| e == "md") && claims_folio(p) {
                    docs.push(p.to_path_buf());
                }
            }
        }
    }
    docs.sort();
    Ok(docs)
}

fn claims_folio(path: &Path) -> bool {
    std::fs::read_to_string(path)
        .map(|content| is_colophon(&content))
        .unwrap_or(false)
}
```

## The check

The affordance's text names the two outcomes, and
[`checking.md`](../folio/checking.md) keeps them structurally apart: a
**defect** is the shipped vocabulary failing to express what a document
says, and a **dangling edge** is a well-formed target naming no
document in the set — the publication boundary doing its job. The
report here adds one thing in front of the corpus report: a document
that claims folio/v1 and does not parse as one. The affordance promises
that the envelope is well formed, and a parse failure is that promise
broken, so it counts as a defect and not as a document to skip.

The check runs over the whole set at once rather than one document at
a time, because "names no document here" is a question about the set.

So is the third outcome. The set's inline declarations — affordances
and the signifiers that present them — are read together and handed to
[`check_declarations`](../folio/checking.md): an affordance claimed for
a human that no signifier signifies is a defect, because a
perception-dependent actor has been promised something with nothing to
perceive (`publish-a-region-as-a-repository`, amendment of 2026-09-05).
It has to be the set, because the signifier is declared where the face
lives — the CLI chapter, the library function's section — and never
beside the claim.

And so is the fourth, which is the second outcome again from the code's
side. A chunk's `proves=` is an edge from the chapter to an affordance,
and one naming no affordance declared in the set is a dangling edge —
reported beside the envelope ones, as `proves`, with the chunk's
document as its source. Expected when the set is a projection and the
design stayed home; the thing to read when a test was renamed or a
declaration deleted, since the edge on the chunk outlives both. A
`proves=` value that is not an id at all is a defect, the same one a
malformed edge target is.

<a name="chunk-vocabulary-report"></a><sub>[`src/faces.rs`](../../../x0k-tangle/src/faces.rs) · `#vocabulary-report`</sub>

```rust {#vocabulary-report}
/// What `check` found reading a set of envelopes against the shipped
/// vocabulary.
#[derive(Debug, Default)]
pub struct VocabularyReport {
    /// Documents whose frontmatter claims folio/v1 but does not parse
    /// as one, each with the parser's reason. A defect: the envelope is
    /// not well formed.
    pub unparsed: Vec<(String, String)>,
    /// The corpus check over every document that parsed: defects, and
    /// the edges that leave the set.
    pub corpus: CorpusReport,
    /// The declarations the set carries — affordances and signifiers —
    /// checked together: a human claim no signifier signifies is a
    /// defect, and it is a question about the set, because the
    /// signifier lives in the chapter that holds the face, not beside
    /// the claim.
    pub declarations: DeclarationReport,
}

impl VocabularyReport {
    /// True when every envelope parsed and the shipped vocabulary
    /// expressed everything every document said. Dangling edges do not
    /// affect this.
    pub fn is_clean(&self) -> bool {
        self.unparsed.is_empty() && self.corpus.is_clean()
    }
}
```

<a name="chunk-check-vocabulary"></a><sub>[`src/faces.rs`](../../../x0k-tangle/src/faces.rs) · `#check-vocabulary`</sub>

```rust {#check-vocabulary}
/// Read every folio/v1 document under `paths` against the vocabulary
/// this build compiled. Documents are named by their path in the
/// report.
pub fn check_vocabulary(paths: &[PathBuf]) -> Result<VocabularyReport> {
    let mut unparsed = Vec::new();
    let mut envelopes: Vec<(String, Colophon)> = Vec::new();
    let classes: HashSet<String> =
        HashSet::from(["affordance".to_string(), "signifier".to_string()]);
    let mut entities: Vec<InlineEntity> = Vec::new();
    // `(document name, document id, proving chunk)` for every tangled
    // document, judged once the set's affordances are all known.
    let mut proofs: Vec<(String, String, ProvingChunk)> = Vec::new();
    for path in discover_folio_documents(paths)? {
        let content = std::fs::read_to_string(&path)
            .with_context(|| format!("reading {}", path.display()))?;
        let name = path.display().to_string();
        match parse_envelope(&content) {
            Ok((envelope, body)) => {
                // A chapter's prose link is an edge — `presupposes` to a
                // wiki page, `realizes` to an affordance — and is checked as
                // one. The rule is a chapter's; a wiki page or a publication
                // linking a concept page is linking.
                let mut envelope = envelope;
                if matches!(envelope.doc_type, DocType::Implementation) {
                    envelope.edges = document_edges(&envelope.edges, &body);
                }
                // A block the extractor refuses is the `affordances` verb's
                // report; the declaration check reads what parsed.
                entities.extend(extract_from_markdown(&body, &classes).into_iter().flatten());
                if envelope.tangle.is_some() {
                    if let Ok(parsed) = parse_document(&content) {
                        for chunk in proving_chunks(&parsed) {
                            proofs.push((name.clone(), envelope.id.clone(), chunk));
                        }
                    }
                }
                envelopes.push((name, envelope));
            }
            Err(e) => unparsed.push((name, e.to_string())),
        }
    }
    let mut corpus = check_corpus(envelopes.iter().map(|(name, env)| (name.as_str(), env)));
    let declarations = check_declarations(entities.iter());
    let declared: HashSet<String> = entities
        .iter()
        .filter(|e| e.marker_class == "affordance")
        .map(|e| e.uri.to_string())
        .collect();
    for (name, doc_id, chunk) in proofs {
        for target in chunk.proves {
            if declared.contains(&target) {
                continue;
            }
            match (doc_id.parse(), target.parse()) {
                (Ok(subject), Ok(target)) => corpus.dangling.push(DanglingEdge {
                    source: name.clone(),
                    subject,
                    predicate: "proves".to_string(),
                    target,
                }),
                (_, Err(e)) => corpus.defects.push((
                    name.clone(),
                    Defect::MalformedTarget {
                        predicate: format!("proves (chunk `{}`)", chunk.chunk),
                        value: target,
                        reason: e.to_string(),
                    },
                )),
                (Err(e), _) => corpus.defects.push((
                    name.clone(),
                    Defect::MalformedId {
                        value: doc_id.clone(),
                        reason: e.to_string(),
                    },
                )),
            }
        }
    }
    Ok(VocabularyReport {
        unparsed,
        corpus,
        declarations,
    })
}
```

## The declarations

An affordance is authored inline — a `yaml x0k:affordance` block under
its own heading — and the extractor in
[`inline-entities.md`](../folio/inline-entities.md) turns the block
into an `InlineEntity` and its facts into `(predicate, value)` pairs.
The record this verb prints is that entity with its facts grouped by
predicate, plus the one fact the document does not declare and the
extractor does not mint: `defined_in`, the parent's `x0k.id`.

The extractor prefixes every fact value with its kind — `entity:` for
an id, `string:` for a literal — so a consumer can tell a reference
from a name. The record keeps that distinction and drops the prefix
grammar: a value is `{"entity": "x0k:actor/human"}` or
`{"string": "wip"}`, which JSON tooling can key on without knowing the
extractor's string form. The title and description are fields of the
record, so their `x0k:title` / `x0k:description` facts are not repeated
under `facts`.

<a name="chunk-fact-value"></a><sub>[`src/faces.rs`](../../../x0k-tangle/src/faces.rs) · `#fact-value`</sub>

```rust {#fact-value}
/// One value of a declared fact, with the extractor's kind prefix made
/// a tag: an `entity:` value is an id, a `string:` value a literal.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum FactValue {
    Entity(String),
    String(String),
}

impl FactValue {
    fn from_extracted(raw: &str) -> Self {
        if let Some(id) = raw.strip_prefix("entity:") {
            Self::Entity(id.to_string())
        } else if let Some(s) = raw.strip_prefix("string:") {
            Self::String(s.to_string())
        } else {
            Self::String(raw.to_string())
        }
    }
}
```

<a name="chunk-affordance-record"></a><sub>[`src/faces.rs`](../../../x0k-tangle/src/faces.rs) · `#affordance-record`</sub>

```rust {#affordance-record}
/// One affordance declaration, as the `affordances` verb prints it.
#[derive(Debug, Clone, Serialize)]
pub struct AffordanceRecord {
    /// The declaration's `id:`.
    pub id: String,
    /// The enclosing heading's text.
    pub title: String,
    /// The prose under that heading, with the block excised.
    pub description: String,
    /// The `x0k.id` of the document the block was authored in.
    pub defined_in: String,
    /// Every other declared fact, grouped by predicate in the order the
    /// extractor emitted them.
    pub facts: BTreeMap<String, Vec<FactValue>>,
    /// The chunks under the paths that tangle tests for it (`proves=`),
    /// in the order met. The relation the verb relays, not derives:
    /// whether the tests pass is the projector's business, at projection.
    pub proofs: Vec<ProofRecord>,
}

/// One proof of an affordance, as the `affordances` verb prints it: the
/// chapter's id and the chunk's name — the edge's address,
/// `<chapter>#<chunk>` — and the `#[test]` functions in the chunk.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ProofRecord {
    pub chapter: String,
    pub chunk: String,
    pub tests: Vec<String>,
}

/// What `affordances` found under a set of paths.
#[derive(Debug, Default)]
pub struct AffordanceReport {
    pub records: Vec<AffordanceRecord>,
    /// Documents the parser refused and blocks the extractor refused,
    /// each with its path and the reason. Reported and skipped, per the
    /// extractor's contract; never fatal to the batch.
    pub skipped: Vec<(String, String)>,
    /// Proving chunks naming an affordance no document under the paths
    /// declares, as `<chapter>#<chunk>` → the id named. The check reports
    /// these as dangling edges; this verb only relays what it read.
    pub unmatched_proofs: Vec<(String, String)>,
}
```

A malformed block is the extractor's error, not the batch's: the
extractor returns one `Result` per attempted record so a caller can
report per error without dropping the rest, and this keeps that shape.
The record also carries the affordance's **proofs** — the proving
chunks under the same paths that name it, each as its chapter's id, its
chunk's name and its test functions — so the relation reaches a
reader's tooling as data. Relayed, not judged: this verb reads what a
chunk says it proves; whether the test passes is settled where the
tests run, at projection ([`region-repo.md`](region-repo.md)).

<a name="chunk-declared-affordances"></a><sub>[`src/faces.rs`](../../../x0k-tangle/src/faces.rs) · `#declared-affordances`</sub>

```rust {#declared-affordances}
/// Read every inline affordance declaration under `paths`.
pub fn declared_affordances(paths: &[PathBuf]) -> Result<AffordanceReport> {
    let classes: HashSet<String> = HashSet::from(["affordance".to_string()]);
    let mut report = AffordanceReport::default();
    let mut proofs: Vec<(String, ProofRecord)> = Vec::new();
    for path in discover_folio_documents(paths)? {
        let content = std::fs::read_to_string(&path)
            .with_context(|| format!("reading {}", path.display()))?;
        let name = path.display().to_string();
        let (envelope, body) = match parse_envelope(&content) {
            Ok(parsed) => parsed,
            Err(e) => {
                report.skipped.push((name, e.to_string()));
                continue;
            }
        };
        for extracted in extract_from_markdown(&body, &classes) {
            match extracted {
                Ok(entity) => report.records.push(record_of(&entity, &envelope.id)),
                Err(e) => report.skipped.push((name.clone(), e.to_string())),
            }
        }
        if envelope.tangle.is_some() {
            if let Ok(parsed) = parse_document(&content) {
                for chunk in proving_chunks(&parsed) {
                    let record = ProofRecord {
                        chapter: envelope.id.clone(),
                        chunk: chunk.chunk,
                        tests: chunk.tests,
                    };
                    for target in chunk.proves {
                        proofs.push((target, record.clone()));
                    }
                }
            }
        }
    }
    // Joined once every declaration under the paths is known, so a proof
    // that precedes its affordance in the walk still finds it.
    for (target, proof) in proofs {
        match report.records.iter_mut().find(|r| r.id == target) {
            Some(record) => record.proofs.push(proof),
            None => report
                .unmatched_proofs
                .push((format!("{}#{}", proof.chapter, proof.chunk), target)),
        }
    }
    Ok(report)
}

fn record_of(entity: &InlineEntity, defined_in: &str) -> AffordanceRecord {
    let mut facts: BTreeMap<String, Vec<FactValue>> = BTreeMap::new();
    for (predicate, value) in declared_facts(entity) {
        if predicate == "x0k:title" || predicate == "x0k:description" {
            continue;
        }
        facts
            .entry(predicate)
            .or_default()
            .push(FactValue::from_extracted(&value));
    }
    AffordanceRecord {
        id: entity.uri.to_string(),
        title: entity.title.clone(),
        description: entity.description.clone(),
        defined_in: defined_in.to_string(),
        facts,
        proofs: Vec::new(),
    }
}
```

## Composing the module

<a name="chunk-root"></a><sub>[`src/faces.rs`](../../../x0k-tangle/src/faces.rs) · `#root` · assembles [doc](#chunk-doc) · [imports](#chunk-imports) · [proving-chunks](#chunk-proving-chunks) · [discover](#chunk-discover) · [vocabulary-report](#chunk-vocabulary-report) · [check-vocabulary](#chunk-check-vocabulary) · [fact-value](#chunk-fact-value) · [affordance-record](#chunk-affordance-record) · [declared-affordances](#chunk-declared-affordances)</sub>

```rust {#root}
<<doc>>

<<imports>>

<<proving-chunks>>

<<discover>>

<<vocabulary-report>>

<<check-vocabulary>>

<<fact-value>>

<<affordance-record>>

<<declared-affordances>>
```

## Proving the faces

The claim is about the binary — that a person holding the repository
can run `check` and be told, and run `affordances` and read — so the
lowest rung that expresses it runs the built binary
(`CARGO_BIN_EXE_x0k-tangle`) over a fixture in a temp dir. That is
milliseconds, and it is the same binary the repository's CI builds, so
the proof travels with the publication.

The fixtures name no shipped predicate by hand. This crate lives in
two builds that compile different vocabulary slices, and a fixture
that wrote `implements` would measure the module selection, not the
face — the lesson [`checking.md`](../folio/checking.md) records. The
well-formed fixture takes its edge predicate from the compiled slice at
runtime; the defective one uses a term no module will ever declare.

<a name="chunk-tests-doc"></a><sub>[`tests/cli_faces.rs`](../../../x0k-tangle/tests/cli_faces.rs) · `#tests-doc`</sub>

```rust {#tests-doc file="tests/cli_faces.rs"}
//! Pins for the `check` and `affordances` faces of the shipped CLI
//! (`x0k:implementation/tangle/cli-faces`): the built binary is run
//! over a temp fixture, and what it prints and how it exits is the
//! claim.
```

<a name="chunk-tests-uses"></a><sub>[`tests/cli_faces.rs`](../../../x0k-tangle/tests/cli_faces.rs) · `#tests-uses`</sub>

```rust {#tests-uses file="tests/cli_faces.rs"}
use std::fs;
use std::path::Path;
use std::process::{Command, Output};

use tempfile::TempDir;
```

<a name="chunk-tests-fixture"></a><sub>[`tests/cli_faces.rs`](../../../x0k-tangle/tests/cli_faces.rs) · `#tests-fixture`</sub>

```rust {#tests-fixture file="tests/cli_faces.rs"}
/// A predicate this build is certain to accept, so the well-formed
/// fixture measures the face and not the module selection.
fn shipped_predicate() -> &'static str {
    x0k_ontology::KNOWN_EDGE_PREDICATES
        .first()
        .copied()
        .expect("a build whose vocabulary declares no document edge ships no document module")
}

/// A folio/v1 design document with one edge and one affordance.
fn design_doc(predicate: &str) -> String {
    format!(
        "---\nx0k:\n  format: folio/v1\n  id: x0k:design/fixture\n  type: design\n  \
         status: draft\n  edges:\n    {predicate}:\n      - x0k:design/elsewhere\n---\n\
         # Fixture\n\n## Affordances\n\n### Frob the widget\n\nI frob a widget from here.\n\n\
         ```yaml x0k:affordance\nid: x0k:affordance/frob_the_widget\nstatus: wip\n\
         actors: [human]\n```\n\n### The frob verb\n\n`frob`, on the command line.\n\n\
         ```yaml x0k:signifier\nid: x0k:signifier/frob\nedges:\n\
         \x20 signifies: [x0k:affordance/frob_the_widget]\n\
         \x20 presentedOn: [x0k:surface/cli]\n```\n"
    )
}

fn write(dir: &Path, rel: &str, content: &str) {
    let path = dir.join(rel);
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, content).unwrap();
}

fn run(args: &[&str], dir: &Path) -> Output {
    Command::new(env!("CARGO_BIN_EXE_x0k-tangle"))
        .args(args)
        .arg(dir)
        .output()
        .expect("the x0k-tangle binary runs")
}
```

The check's two outcomes, each on its own fixture: an edge into the
private corpus is noted and passes; a predicate no module declares is
named and fails.

<a name="chunk-tests-check"></a><sub>[`tests/cli_faces.rs`](../../../x0k-tangle/tests/cli_faces.rs) · `#tests-check` · proves [Check a document against its vocabulary](../../../decisions/design/corpus/publish-a-region-as-a-repository/check-a-document-against-its-vocabulary.md)</sub>

```rust {#tests-check file="tests/cli_faces.rs" proves="x0k:affordance/check_a_document_against_shipped_vocabulary"}
#[test]
fn check_notes_an_edge_out_of_the_set_and_passes() {
    let tmp = TempDir::new().unwrap();
    write(tmp.path(), "docs/fixture.md", &design_doc(shipped_predicate()));

    let out = run(&["check"], tmp.path());
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(out.status.success(), "check failed: {stderr}");
    assert!(
        stderr.contains("x0k:design/elsewhere"),
        "the dangling target is named: {stderr}"
    );
    assert!(
        stderr.contains("note:"),
        "a dangling edge is informational, not a defect: {stderr}"
    );
}

#[test]
fn check_names_an_undeclared_predicate_and_fails() {
    let tmp = TempDir::new().unwrap();
    write(tmp.path(), "docs/fixture.md", &design_doc("frobnicates"));

    let out = run(&["check"], tmp.path());
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(!out.status.success(), "check passed a defect: {stderr}");
    assert!(
        stderr.contains("frobnicates") && stderr.contains("fixture.md"),
        "the defect names the predicate and the document: {stderr}"
    );
}

#[test]
fn check_reports_an_envelope_that_does_not_parse() {
    let tmp = TempDir::new().unwrap();
    write(
        tmp.path(),
        "docs/broken.md",
        "---\nx0k:\n  format: folio/v1\n  id: x0k:design/broken\n  type: nonsense\n---\n# Broken\n",
    );

    let out = run(&["check"], tmp.path());
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(!out.status.success(), "check passed a malformed envelope: {stderr}");
    assert!(stderr.contains("broken.md"), "the document is named: {stderr}");
}

/// A design declaring one affordance for the given actors and no signifier.
fn lonely_doc(actors: &str) -> String {
    format!(
        "---\nx0k:\n  format: folio/v1\n  id: x0k:design/lonely\n  type: design\n  \
         status: draft\n---\n# Lonely\n\n### Frob alone\n\nI frob, and nothing shows me how.\n\n\
         ```yaml x0k:affordance\nid: x0k:affordance/frob_alone\nstatus: wip\n\
         actors: [{actors}]\n```\n"
    )
}

#[test]
fn check_names_a_human_claim_no_signifier_signifies_and_fails() {
    let tmp = TempDir::new().unwrap();
    write(tmp.path(), "docs/lonely.md", &lonely_doc("human"));

    let out = run(&["check"], tmp.path());
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(!out.status.success(), "check passed a human claim nothing signifies: {stderr}");
    assert!(
        stderr.contains("x0k:affordance/frob_alone") && stderr.contains("signifier"),
        "the defect names the affordance and what is missing: {stderr}"
    );
}

#[test]
fn check_passes_an_agent_only_claim_with_no_signifier() {
    let tmp = TempDir::new().unwrap();
    write(tmp.path(), "docs/lonely.md", &lonely_doc("ai_agent"));

    let out = run(&["check"], tmp.path());
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(out.status.success(), "an agent reads the descriptor; no cue is owed: {stderr}");
}
```

The declaration read back as data. The assertion is on identity,
title, and parent: the shape of the `actors:` fact is the extractor's
to decide (it is moving from a bare `x0k:affordance/actors` string to a
`claimedFor` entity edge), and this face relays whichever it emits.
The test asks only that the human claim survived into the record under
some predicate.

```rust {#tests-affordances file="tests/cli_faces.rs" proves="x0k:affordance/read_declared_affordances"}
#[test]
fn affordances_prints_each_declaration_as_a_record() {
    let tmp = TempDir::new().unwrap();
    write(tmp.path(), "docs/fixture.md", &design_doc(shipped_predicate()));

    let out = run(&["affordances"], tmp.path());
    assert!(
        out.status.success(),
        "affordances failed: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    let records: serde_json::Value =
        serde_json::from_slice(&out.stdout).expect("stdout is a JSON array");
    let records = records.as_array().expect("an array of records");
    assert_eq!(records.len(), 1, "one declaration: {records:?}");

    let record = &records[0];
    assert_eq!(record["id"], "x0k:affordance/frob_the_widget");
    assert_eq!(record["title"], "Frob the widget");
    assert_eq!(record["defined_in"], "x0k:design/fixture");
    assert!(
        record["description"]
            .as_str()
            .unwrap()
            .contains("I frob a widget"),
        "the prose under the heading is the description: {record}"
    );
    assert!(
        record["facts"].to_string().contains("human"),
        "the human claim reaches the record under some predicate: {record}"
    );
}

#[test]
fn affordances_reports_a_malformed_block_and_keeps_going() {
    let tmp = TempDir::new().unwrap();
    write(
        tmp.path(),
        "docs/bad.md",
        "---\nx0k:\n  format: folio/v1\n  id: x0k:design/bad\n  type: design\n---\n\
         # Bad\n\n## Affordances\n\n### No id here\n\n```yaml x0k:affordance\nstatus: wip\n```\n",
    );
    write(tmp.path(), "docs/good.md", &design_doc(shipped_predicate()));

    let out = run(&["affordances"], tmp.path());
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(out.status.success(), "a malformed block is not fatal: {stderr}");
    assert!(
        stderr.contains("bad.md") && stderr.contains("skipped"),
        "the malformed block is reported on stderr: {stderr}"
    );
    let records: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    assert_eq!(records.as_array().unwrap().len(), 1, "the good record survives");
}
```

The proofs, from both faces. A chapter whose one chunk tangles a test and
says what it proves is relayed by `affordances` as the record's `proofs`
— chapter, chunk, test names — and, when the id it names is declared by
no document under the paths, noted by `check` as a dangling `proves` edge
that fails nothing.

```rust {#tests-proofs file="tests/cli_faces.rs"}
/// A tangled chapter whose one chunk tangles a test and says it proves
/// `proves`. Nothing here tangles it: both faces read the document.
fn proof_doc(proves: &str) -> String {
    format!(
        "---\nx0k:\n  format: folio/v1\n  id: x0k:implementation/fixture/proof\n  \
         type: implementation\n  status: draft\n  tangle:\n    crate: fixture\n    \
         root: tests/proof.rs\n---\n# Proof\n\n```rust {{#root proves=\"{proves}\"}}\n\
         #[test]\nfn the_widget_frobs() {{}}\n```\n"
    )
}

#[test]
fn affordances_relays_the_proofs_a_chunk_declares() {
    let tmp = TempDir::new().unwrap();
    write(tmp.path(), "docs/fixture.md", &design_doc(shipped_predicate()));
    write(tmp.path(), "docs/proof.md", &proof_doc("x0k:affordance/frob_the_widget"));

    let out = run(&["affordances"], tmp.path());
    assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
    let records: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    assert_eq!(
        records[0]["proofs"],
        serde_json::json!([{
            "chapter": "x0k:implementation/fixture/proof",
            "chunk": "root",
            "tests": ["the_widget_frobs"],
        }]),
        "the relation, relayed: {records}"
    );
}

#[test]
fn check_notes_a_proof_naming_no_affordance_here_and_passes() {
    let tmp = TempDir::new().unwrap();
    write(tmp.path(), "docs/fixture.md", &design_doc(shipped_predicate()));
    write(tmp.path(), "docs/proof.md", &proof_doc("x0k:affordance/absent"));

    let out = run(&["check"], tmp.path());
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(out.status.success(), "a dangling proof is a note: {stderr}");
    assert!(
        stderr.contains("note:") && stderr.contains("`proves`") && stderr.contains("x0k:affordance/absent"),
        "the edge is named as such: {stderr}"
    );
}
```

```rust {#tests-root file="tests/cli_faces.rs"}
<<tests-doc>>

<<tests-uses>>

<<tests-fixture>>

<<tests-check>>

<<tests-affordances>>

<<tests-proofs>>
```

What this chapter deliberately leaves to the binaries is the wording.
A report is data; how a defect reads on a terminal is the face's
decision, and the two binaries make it identically because they share
the sentence, not because the library imposes it.
