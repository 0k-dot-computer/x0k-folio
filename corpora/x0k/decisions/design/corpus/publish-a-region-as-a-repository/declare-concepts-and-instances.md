---
x0k:
  format: folio/v1
  id: x0k:design/publish-a-region-as-a-repository#declare-concepts-and-instances
  type: design
  status: proposed
  edges:
    transcludes:
      - x0k:design/publish-a-region-as-a-repository
---

### Declare concepts and instances

Define a vocabulary in a document, then describe particular things with it.

Turtle blocks define concepts and relationships; typed YAML blocks declare
instances. The same collection can describe papers, experiments, software,
or something else. Its declarations can be validated, queried, and
presented according to their concept, publication surface, and theme.

An affordance is one example: its instance says what a person or tool can
do, who can use it, and which software provides it.

<a name="folio-instance-68747470733a2f2f306b2e636f6d70757465722f6f6e746f6c6f6779236166666f7264616e63652f726561645f6465636c617265645f6166666f7264616e636573-1"></a><sub data-instance-iri="https://0k.computer/ontology#affordance/read_declared_affordances" data-concept-iri="https://0k.computer/ontology#Affordance" data-source-document="corpora/x0k/decisions/design/corpus/publish-a-region-as-a-repository/declare-concepts-and-instances.md"><strong>Affordance</strong> · Declare concepts and instances · <code>https://0k.computer/ontology#affordance/read_declared_affordances</code> · <a href="#folio-source-68747470733a2f2f306b2e636f6d70757465722f6f6e746f6c6f6779236166666f7264616e63652f726561645f6465636c617265645f6166666f7264616e636573-1">source declaration</a></sub><a name="folio-source-68747470733a2f2f306b2e636f6d70757465722f6f6e746f6c6f6779236166666f7264616e63652f726561645f6465636c617265645f6166666f7264616e636573-1"></a>

```yaml x0k:affordance
id: x0k:affordance/read_declared_affordances
actors: [human, ai_agent]
edges:
  enabledBy:
    - x0k:software-module/x0k-folio
```

<picture><source media="(prefers-color-scheme: dark)" srcset="../../../../../../affordances/actor-dark.svg"><img alt="Actor" src="../../../../../../affordances/actor-light.svg" height="20"></picture> <picture><source media="(prefers-color-scheme: dark)" srcset="../../../../../../affordances/proven-dark.svg"><img alt="proven" src="../../../../../../affordances/proven-light.svg" height="16"></picture> *proven* · for a person, an agent · reachable through `cli` `x0k-tangle affordances`, `sdk` `extract_from_markdown`

*realized in* [Entities authored inside prose](../../../../implementation/folio/inline-entities.md) · [The faces behind `check`, `affordances` and `icon`](../../../../implementation/tangle/cli-faces.md) · [x0k-tangle: the crate and its CLI](../../../../implementation/tangle/crate.md)

*proven by* each test below, as its chapter tangles it and as it ran at projection.

<details><summary><code>affordances_prints_each_declaration_as_a_record</code> · <picture><source media="(prefers-color-scheme: dark)" srcset="../../../../../../affordances/passed-dark.svg"><img alt="passed" src="../../../../../../affordances/passed-light.svg" height="16"></picture> passed · <a href="../../../../implementation/tangle/cli-faces.md#chunk-tests-affordances">#tests-affordances</a> in The faces behind `check`, `affordances` and `icon`</summary>

```rust
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
```

</details>

<details><summary><code>affordances_reports_a_malformed_block_and_keeps_going</code> · <picture><source media="(prefers-color-scheme: dark)" srcset="../../../../../../affordances/passed-dark.svg"><img alt="passed" src="../../../../../../affordances/passed-light.svg" height="16"></picture> passed · <a href="../../../../implementation/tangle/cli-faces.md#chunk-tests-affordances">#tests-affordances</a> in The faces behind `check`, `affordances` and `icon`</summary>

```rust
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

</details>


Its mark: a document with one block lifted out and held above it; the
place it came from is dotted, the profile's mark of absence.

```svg x0k:icon
<svg viewBox="0 0 16 16">
  <path d="M2.5 3.5 H8.5 L11 6 V14.5 H2.5 Z" fill="none" stroke="line" stroke-width="1"/>
  <path d="M8.5 3.5 V6 H11" fill="none" stroke="line" stroke-width="1"/>
  <path d="M4.5 8 H8" fill="none" stroke="line" stroke-width="1"/>
  <rect x="4.5" y="9.5" width="4.5" height="3" fill="none" stroke="line" stroke-width="1" stroke-dasharray="1 2"/>
  <rect x="9" y="1" width="5" height="3" rx="0.5" fill="paper" stroke="ink" stroke-width="1.5"/>
  <path d="M10.5 2.5 H12.5" fill="none" stroke="ink" stroke-width="1"/>
</svg>
```
