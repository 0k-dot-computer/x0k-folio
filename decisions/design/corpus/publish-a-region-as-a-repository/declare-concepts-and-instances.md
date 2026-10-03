```turtle folio:document
design:publish-a-region-as-a-repository%23declare-concepts-and-instances a x0k:Design ;
    x0k:status "proposed" ;
    x0k:transcludes design:publish-a-region-as-a-repository .
```

### Declare concepts and instances

Define a vocabulary in a document, then describe particular things with it.

Turtle blocks define concepts and relationships; typed YAML blocks declare
instances. The same collection can describe papers, experiments, software,
or something else. Its declarations can be validated, queried, and
presented according to their concept, publication surface, and theme.

An affordance is one example: its instance says what a person or tool can
do, who can use it, and which software provides it.

<a name="folio-instance-78306b3a6166666f7264616e63652f726561645f6465636c617265645f6166666f7264616e636573-1"></a><sub data-instance-iri="x0k:affordance/read_declared_affordances" data-concept-iri="https://0k.computer/ontology#Affordance" data-source-document="corpora/x0k/decisions/design/corpus/publish-a-region-as-a-repository/declare-concepts-and-instances.md"><strong>Affordance</strong> · Unresolved instance · <code>x0k:affordance/read_declared_affordances</code> · <a href="#folio-source-78306b3a6166666f7264616e63652f726561645f6465636c617265645f6166666f7264616e636573-1">source declaration</a> · the graph block at line 21 is not Turtle: The prefix actor: has not been declared at corpora/x0k/decisions/design/corpus/publish-a-region-as-a-repository/declare-concepts-and-instances.md:19</sub><a name="folio-source-78306b3a6166666f7264616e63652f726561645f6465636c617265645f6166666f7264616e636573-1"></a>

```turtle folio:graph
@prefix actor: <https://0k.computer/ontology#actor/> .
@prefix software-module: <https://0k.computer/ontology#software-module/> .
affordance:read_declared_affordances a x0k:Affordance ;
    x0k:claimedFor actor:human,
        actor:ai_agent ;
    x0k:enabledBy software-module:x0k-folio .
```

<picture><source media="(prefers-color-scheme: dark)" srcset="../../../../assets/icons/actor-dark.svg"><img alt="Actor" src="../../../../assets/icons/actor-light.svg" height="20"></picture> <picture><source media="(prefers-color-scheme: dark)" srcset="../../../../assets/icons/proven-dark.svg"><img alt="proven" src="../../../../assets/icons/proven-light.svg" height="16"></picture> *proven* · for a person, an agent · reachable through `cli` `x0k-tangle affordances`, `sdk` `extract_from_markdown`

*realized in* [Entities authored inside prose](../../../../implementation/folio/inline-entities.md) · [The faces behind `check`, `affordances`, `declarations` and `icon`](../../../../implementation/tangle/cli-faces.md) · [x0k-tangle: the crate and its CLI](../../../../implementation/tangle/crate.md)

*proven by* each test below, as its chapter tangles it and as it ran at projection.

<details><summary><code>affordances_prints_each_declaration_as_a_record</code> · <picture><source media="(prefers-color-scheme: dark)" srcset="../../../../assets/icons/passed-dark.svg"><img alt="passed" src="../../../../assets/icons/passed-light.svg" height="16"></picture> passed · <a href="../../../../implementation/tangle/cli-faces.md#chunk-tests-affordances">#tests-affordances</a> in The faces behind `check`, `affordances`, `declarations` and `icon`</summary>

```rust
#[test]
fn affordances_prints_each_declaration_as_a_record() {
    let tmp = TempDir::new().unwrap();
    write(tmp.path(), "docs/fixture.md", &design_doc(&shipped_predicate()));

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
    assert_eq!(
        record["facts"]["x0k:claimedFor"],
        serde_json::json!([{"entity": "x0k:actor/human"}]),
        "the human claim reaches the record: {record}"
    );
}
```

</details>

<details><summary><code>affordances_reports_a_malformed_block_and_keeps_going</code> · <picture><source media="(prefers-color-scheme: dark)" srcset="../../../../assets/icons/passed-dark.svg"><img alt="passed" src="../../../../assets/icons/passed-light.svg" height="16"></picture> passed · <a href="../../../../implementation/tangle/cli-faces.md#chunk-tests-affordances">#tests-affordances</a> in The faces behind `check`, `affordances`, `declarations` and `icon`</summary>

````rust
#[test]
fn affordances_reports_a_malformed_block_and_keeps_going() {
    let tmp = TempDir::new().unwrap();
    write(
        tmp.path(),
        "docs/bad.md",
        "# Bad\n\n```turtle folio:document\ndesign:bad a x0k:Design .\n```\n\n\
         ## Affordances\n\n### No class here\n\n```turtle folio:graph\n\
         affordance:no_class x0k:status \"wip\" .\n```\n",
    );
    write(tmp.path(), "docs/good.md", &design_doc(&shipped_predicate()));

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
````

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
