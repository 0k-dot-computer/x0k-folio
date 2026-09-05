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
checking a document against the vocabulary that shipped beside it, and
reading the affordances a document declares as data. Both were true of
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

```rust {#doc}
//! The mechanism behind the `check` and `affordances` CLI verbs: every
//! folio/v1 envelope under a set of paths read against the vocabulary
//! this build compiled, and every inline affordance declaration read
//! out as a record. Both `x0k-tangle` binaries call these and do their
//! own printing.
```

```rust {#imports}
use std::collections::{BTreeMap, HashSet};
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use serde::Serialize;
use x0k_folio::colophon::{is_colophon, parse_envelope, Colophon};
use x0k_folio::{
    check_corpus, check_declarations, declared_facts, extract_from_markdown, CorpusReport,
    DeclarationReport, InlineEntity,
};
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
    for path in discover_folio_documents(paths)? {
        let content = std::fs::read_to_string(&path)
            .with_context(|| format!("reading {}", path.display()))?;
        let name = path.display().to_string();
        match parse_envelope(&content) {
            Ok((envelope, body)) => {
                // A block the extractor refuses is the `affordances` verb's
                // report; the declaration check reads what parsed.
                entities.extend(extract_from_markdown(&body, &classes).into_iter().flatten());
                envelopes.push((name, envelope));
            }
            Err(e) => unparsed.push((name, e.to_string())),
        }
    }
    let corpus = check_corpus(envelopes.iter().map(|(name, env)| (name.as_str(), env)));
    let declarations = check_declarations(entities.iter());
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
}

/// What `affordances` found under a set of paths.
#[derive(Debug, Default)]
pub struct AffordanceReport {
    pub records: Vec<AffordanceRecord>,
    /// Documents the parser refused and blocks the extractor refused,
    /// each with its path and the reason. Reported and skipped, per the
    /// extractor's contract; never fatal to the batch.
    pub skipped: Vec<(String, String)>,
}
```

A malformed block is the extractor's error, not the batch's: the
extractor returns one `Result` per attempted record so a caller can
report per error without dropping the rest, and this keeps that shape.

```rust {#declared-affordances}
/// Read every inline affordance declaration under `paths`.
pub fn declared_affordances(paths: &[PathBuf]) -> Result<AffordanceReport> {
    let classes: HashSet<String> = HashSet::from(["affordance".to_string()]);
    let mut report = AffordanceReport::default();
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
    }
}
```

## Composing the module

```rust {#root}
<<doc>>

<<imports>>

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

```rust {#tests-doc file="tests/cli_faces.rs"}
//! Pins for the `check` and `affordances` faces of the shipped CLI
//! (`x0k:implementation/tangle/cli-faces`): the built binary is run
//! over a temp fixture, and what it prints and how it exits is the
//! claim.
```

```rust {#tests-uses file="tests/cli_faces.rs"}
use std::fs;
use std::path::Path;
use std::process::{Command, Output};

use tempfile::TempDir;
```

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

```rust {#tests-check file="tests/cli_faces.rs"}
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

```rust {#tests-affordances file="tests/cli_faces.rs"}
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

```rust {#tests-root file="tests/cli_faces.rs"}
<<tests-doc>>

<<tests-uses>>

<<tests-fixture>>

<<tests-check>>

<<tests-affordances>>
```

What this chapter deliberately leaves to the binaries is the wording.
A report is data; how a defect reads on a terminal is the face's
decision, and the two binaries make it identically because they share
the sentence, not because the library imposes it.
