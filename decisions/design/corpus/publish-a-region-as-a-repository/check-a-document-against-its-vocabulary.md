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

Holding only the published repository, I check that a document's envelope is
well formed and that the predicates on its edges are terms the vocabulary in
this repository actually declares. I am told which of two things went wrong: a
predicate no shipped module declares, which is a gap in what this publication
selected, or a target naming no document here, which is an edge into the
private corpus this was projected from and is expected.

```yaml x0k:affordance
id: x0k:affordance/check_a_document_against_shipped_vocabulary
actors: [human, ai_agent]
edges:
  enabledBy:
    - x0k:software-module/x0k-folio
    - x0k:software-module/x0k-ontology
```

<picture><source media="(prefers-color-scheme: dark)" srcset="../../../../affordances/for-a-person-and-an-agent-dark.svg"><img alt="for a person and an agent" src="../../../../affordances/for-a-person-and-an-agent-light.svg" height="20"></picture> <picture><source media="(prefers-color-scheme: dark)" srcset="../../../../affordances/status-proven-dark.svg"><img alt="proven" src="../../../../affordances/status-proven-light.svg" height="16"></picture> *proven* · for a person, an agent · reachable through `cli` `x0k-tangle check`, `sdk` `check_envelope`

*realized in* [Checking a document against what shipped with it](../../../../knowledge/implementation/folio/checking.md) · [Entities authored inside prose](../../../../knowledge/implementation/folio/inline-entities.md) · [The faces behind `check` and `affordances`](../../../../knowledge/implementation/tangle/cli-faces.md) · [x0k-tangle: the crate and its CLI](../../../../knowledge/implementation/tangle/crate.md)

*proven by* each test below, as its chapter tangles it and as it ran at projection.

<details><summary><code>check_notes_an_edge_out_of_the_set_and_passes</code> · passed · <a href="../../../../knowledge/implementation/tangle/cli-faces.md#chunk-tests-check">#tests-check</a> in The faces behind `check` and `affordances`</summary>

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

<details><summary><code>check_names_an_undeclared_predicate_and_fails</code> · passed · <a href="../../../../knowledge/implementation/tangle/cli-faces.md#chunk-tests-check">#tests-check</a> in The faces behind `check` and `affordances`</summary>

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
```

</details>

<details><summary><code>check_reports_an_envelope_that_does_not_parse</code> · passed · <a href="../../../../knowledge/implementation/tangle/cli-faces.md#chunk-tests-check">#tests-check</a> in The faces behind `check` and `affordances`</summary>

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

<details><summary><code>check_names_a_human_claim_no_signifier_signifies_and_fails</code> · passed · <a href="../../../../knowledge/implementation/tangle/cli-faces.md#chunk-tests-check">#tests-check</a> in The faces behind `check` and `affordances`</summary>

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

<details><summary><code>check_passes_an_agent_only_claim_with_no_signifier</code> · passed · <a href="../../../../knowledge/implementation/tangle/cli-faces.md#chunk-tests-check">#tests-check</a> in The faces behind `check` and `affordances`</summary>

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

