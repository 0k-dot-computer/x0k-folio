---
x0k:
  format: folio/v1
  id: x0k:design/publish-a-region-as-a-repository#check-a-document-against-its-vocabulary
  type: design
  status: proposed
  edges:
    transcludes:
      - x0k:design/publish-a-region-as-a-repository
---

### Check a document against its vocabulary

Find metadata and relationships that do not match the document vocabulary.

Holding only the published repository, I check that a document's envelope is
well formed and that the predicates on its edges are terms the vocabulary in
this repository actually declares. I am told which of two things went wrong: a
predicate no shipped module declares, which is a gap in what this publication
selected, or a target naming no document here, which is an edge into the
private corpus this was projected from and is expected.

<a name="folio-instance-68747470733a2f2f306b2e636f6d70757465722f6f6e746f6c6f6779236166666f7264616e63652f636865636b5f615f646f63756d656e745f616761696e73745f736869707065645f766f636162756c617279-1"></a><sub data-instance-iri="https://0k.computer/ontology#affordance/check_a_document_against_shipped_vocabulary" data-concept-iri="https://0k.computer/ontology#Affordance" data-source-document="corpora/x0k/decisions/design/corpus/publish-a-region-as-a-repository/check-a-document-against-its-vocabulary.md"><strong>Affordance</strong> · Check a document against its vocabulary · <code>https://0k.computer/ontology#affordance/check_a_document_against_shipped_vocabulary</code> · <a href="#folio-source-68747470733a2f2f306b2e636f6d70757465722f6f6e746f6c6f6779236166666f7264616e63652f636865636b5f615f646f63756d656e745f616761696e73745f736869707065645f766f636162756c617279-1">source declaration</a></sub><a name="folio-source-68747470733a2f2f306b2e636f6d70757465722f6f6e746f6c6f6779236166666f7264616e63652f636865636b5f615f646f63756d656e745f616761696e73745f736869707065645f766f636162756c617279-1"></a>

```yaml x0k:affordance
id: x0k:affordance/check_a_document_against_shipped_vocabulary
actors: [human, ai_agent]
edges:
  enabledBy:
    - x0k:software-module/x0k-folio
    - x0k:software-module/x0k-ontology
```

<picture><source media="(prefers-color-scheme: dark)" srcset="../../../../../../affordances/actor-dark.svg"><img alt="Actor" src="../../../../../../affordances/actor-light.svg" height="20"></picture> <picture><source media="(prefers-color-scheme: dark)" srcset="../../../../../../affordances/proven-dark.svg"><img alt="proven" src="../../../../../../affordances/proven-light.svg" height="16"></picture> *proven* · for a person, an agent · reachable through `cli` `x0k-tangle check`, `sdk` `check_envelope`

*realized in* [Checking a document against what shipped with it](../../../../implementation/folio/checking.md) · [Entities authored inside prose](../../../../implementation/folio/inline-entities.md) · [The faces behind `check`, `affordances` and `icon`](../../../../implementation/tangle/cli-faces.md) · [x0k-tangle: the crate and its CLI](../../../../implementation/tangle/crate.md)

*proven by* each test below, as its chapter tangles it and as it ran at projection.

<details><summary><code>check_notes_an_edge_out_of_the_set_and_passes</code> · <picture><source media="(prefers-color-scheme: dark)" srcset="../../../../../../affordances/passed-dark.svg"><img alt="passed" src="../../../../../../affordances/passed-light.svg" height="16"></picture> passed · <a href="../../../../implementation/tangle/cli-faces.md#chunk-tests-check">#tests-check</a> in The faces behind `check`, `affordances` and `icon`</summary>

```rust
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
```

</details>

<details><summary><code>check_names_an_undeclared_predicate_and_fails</code> · <picture><source media="(prefers-color-scheme: dark)" srcset="../../../../../../affordances/passed-dark.svg"><img alt="passed" src="../../../../../../affordances/passed-light.svg" height="16"></picture> passed · <a href="../../../../implementation/tangle/cli-faces.md#chunk-tests-check">#tests-check</a> in The faces behind `check`, `affordances` and `icon`</summary>

```rust
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

/// A vocabulary a reader could write: `mycorp` in its own namespace,
/// declaring one genus class. The smallest set that closes — `core` has
/// no imports, and `mycorp` imports it.
fn write_scratch_vocabulary(dir: &Path) {
    fs::create_dir_all(dir).unwrap();
    fs::write(
        dir.join("core.ttl"),
        "<https://0k.computer/ontology/core> \
         <http://www.w3.org/1999/02/22-rdf-syntax-ns#type> \
         <http://www.w3.org/2002/07/owl#Ontology> .\n",
    )
    .unwrap();
    fs::write(
        dir.join("mycorp.ttl"),
        concat!(
            "<https://0k.computer/ontology/mycorp> <http://www.w3.org/1999/02/22-rdf-syntax-ns#type> <http://www.w3.org/2002/07/owl#Ontology> .\n",
            "<https://0k.computer/ontology/mycorp> <http://www.w3.org/2002/07/owl#imports> <https://0k.computer/ontology/core> .\n",
            "<https://0k.computer/ontology/mycorp> <http://purl.org/vocab/vann/preferredNamespaceUri> \"https://mycorp.example/ontology#\" .\n",
            "<https://mycorp.example/ontology#Brief> <http://www.w3.org/1999/02/22-rdf-syntax-ns#type> <http://www.w3.org/2002/07/owl#Class> .\n",
            "<https://mycorp.example/ontology#Brief> <http://www.w3.org/2000/01/rdf-schema#isDefinedBy> <https://0k.computer/ontology/mycorp> .\n",
            "<https://mycorp.example/ontology#Brief> <http://www.w3.org/2000/01/rdf-schema#label> \"Brief\" .\n",
        ),
    )
    .unwrap();
}
```

</details>

<details><summary><code>check_reads_a_document_against_the_vocabulary_it_is_pointed_at</code> · <picture><source media="(prefers-color-scheme: dark)" srcset="../../../../../../affordances/passed-dark.svg"><img alt="passed" src="../../../../../../affordances/passed-light.svg" height="16"></picture> passed · <a href="../../../../implementation/tangle/cli-faces.md#chunk-tests-check">#tests-check</a> in The faces behind `check`, `affordances` and `icon`</summary>

```rust
#[test]
fn check_reads_a_document_against_the_vocabulary_it_is_pointed_at() {
    let tmp = TempDir::new().unwrap();
    let modules = tmp.path().join("vocab/modules");
    write_scratch_vocabulary(&modules);
    write(
        tmp.path(),
        "docs/brief.md",
        "---\nx0k:\n  format: folio/v1\n  id: mycorp:brief/tender-process\n  \
         type: brief\n  status: proposed\n---\n# A brief\n",
    );
    let docs = tmp.path().join("docs");

    let out = run(
        &["check", "--vocabulary", modules.to_str().unwrap()],
        &docs,
    );
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        out.status.success(),
        "a genus and a namespace the named vocabulary declares must check clean: {stderr}"
    );

    // The same document against the vocabulary this build compiled: the
    // genus is not a class it declares, so the envelope does not parse.
    let out = run(&["check"], &docs);
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        !out.status.success(),
        "the shipped vocabulary declares no `brief` genus: {stderr}"
    );
    assert!(stderr.contains("brief.md"), "the document is named: {stderr}");
}
```

</details>

<details><summary><code>check_reports_an_envelope_that_does_not_parse</code> · <picture><source media="(prefers-color-scheme: dark)" srcset="../../../../../../affordances/passed-dark.svg"><img alt="passed" src="../../../../../../affordances/passed-light.svg" height="16"></picture> passed · <a href="../../../../implementation/tangle/cli-faces.md#chunk-tests-check">#tests-check</a> in The faces behind `check`, `affordances` and `icon`</summary>

```rust
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
```

</details>

<details><summary><code>check_names_a_human_claim_no_signifier_signifies_and_fails</code> · <picture><source media="(prefers-color-scheme: dark)" srcset="../../../../../../affordances/passed-dark.svg"><img alt="passed" src="../../../../../../affordances/passed-light.svg" height="16"></picture> passed · <a href="../../../../implementation/tangle/cli-faces.md#chunk-tests-check">#tests-check</a> in The faces behind `check`, `affordances` and `icon`</summary>

```rust
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
```

</details>

<details><summary><code>check_passes_an_agent_only_claim_with_no_signifier</code> · <picture><source media="(prefers-color-scheme: dark)" srcset="../../../../../../affordances/passed-dark.svg"><img alt="passed" src="../../../../../../affordances/passed-light.svg" height="16"></picture> passed · <a href="../../../../implementation/tangle/cli-faces.md#chunk-tests-check">#tests-check</a> in The faces behind `check`, `affordances` and `icon`</summary>

```rust
#[test]
fn check_passes_an_agent_only_claim_with_no_signifier() {
    let tmp = TempDir::new().unwrap();
    write(tmp.path(), "docs/lonely.md", &lonely_doc("ai_agent"));

    let out = run(&["check"], tmp.path());
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(out.status.success(), "an agent reads the descriptor; no cue is owed: {stderr}");
}
```

</details>


Its mark, in the icon profile: a document beside a small seal carrying
one tick.

```svg x0k:icon
<svg viewBox="0 0 16 16">
  <path d="M2 1.5 H7.5 L10 4 V14.5 H2 Z" fill="none" stroke="line" stroke-width="1"/>
  <path d="M7.5 1.5 V4 H10" fill="none" stroke="line" stroke-width="1"/>
  <path d="M3.5 6.5 H8 M3.5 8.5 H8" fill="none" stroke="line" stroke-width="1"/>
  <circle cx="11.5" cy="11" r="3.5" fill="paper" stroke="ink" stroke-width="1.5"/>
  <path d="M9.5 11 L11 12.5 L13.5 9.5" fill="none" stroke="ink" stroke-width="1.5"/>
</svg>
```
