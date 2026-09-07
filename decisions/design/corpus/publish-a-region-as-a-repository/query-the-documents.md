---
x0k:
  format: folio/v1
  id: x0k:design/publish-a-region-as-a-repository#query-the-documents
  type: design
  status: proposed
  edges:
    transcludes:
      - x0k:design/publish-a-region-as-a-repository
---

### Query the documents

Ask questions across a collection's concepts, instances, and relationships.

<a name="folio-instance-68747470733a2f2f306b2e636f6d70757465722f6f6e746f6c6f6779236166666f7264616e63652f71756572795f7468655f646f63756d656e7473-1"></a><sub data-instance-iri="https://0k.computer/ontology#affordance/query_the_documents" data-concept-iri="https://0k.computer/ontology#Affordance" data-source-document="corpora/x0k/decisions/design/corpus/publish-a-region-as-a-repository/query-the-documents.md"><strong>Affordance</strong> · Query the documents · <code>https://0k.computer/ontology#affordance/query_the_documents</code> · <a href="#folio-source-68747470733a2f2f306b2e636f6d70757465722f6f6e746f6c6f6779236166666f7264616e63652f71756572795f7468655f646f63756d656e7473-1">source declaration</a></sub><a name="folio-source-68747470733a2f2f306b2e636f6d70757465722f6f6e746f6c6f6779236166666f7264616e63652f71756572795f7468655f646f63756d656e7473-1"></a>

```yaml x0k:affordance
id: x0k:affordance/query_the_documents
actors: [human, ai_agent]
edges:
  enabledBy:
    - x0k:software-module/x0k-folio-cli
    - x0k:software-module/x0k-folio-dialog
```

<picture><source media="(prefers-color-scheme: dark)" srcset="../../../../assets/icons/actor-dark.svg"><img alt="Actor" src="../../../../assets/icons/actor-light.svg" height="20"></picture> <picture><source media="(prefers-color-scheme: dark)" srcset="../../../../assets/icons/declared-dark.svg"><img alt="declared" src="../../../../assets/icons/declared-light.svg" height="16"></picture> *declared* · for a person, an agent · reachable through `cli` `x0k-folio-cli query`

*realized in* [Querying a directory of documents](../../../../implementation/folio/query-cli.md)


```svg x0k:icon
<svg viewBox="0 0 16 16">
  <path d="M2.5 2 H9.5 V14 H2.5 Z" fill="none" stroke="line" stroke-width="1"/>
  <path d="M4 4.5 H8 M4 7 H7" fill="none" stroke="line" stroke-width="1"/>
  <circle cx="10.5" cy="9.5" r="3" fill="paper" stroke="ink" stroke-width="1.5"/>
  <path d="M12.5 11.5 L15 14" fill="none" stroke="ink" stroke-width="1.5"/>
</svg>
```

Documents remain the authoring surface. Dialog provides a database view of
their declarations: find which designs have implementations, which software
enables an affordance, or relationships defined by your own vocabulary.
The command runs without x0k. The backend interface also lets an x0k host
enable Dialog, x0k, or both for different purposes.

#### The query command

`x0k-folio-cli query` evaluates a query against an indexed collection.

From the repository root, supply a Folio collection and a query file:

```sh
cargo run -p x0k-folio-cli -- ingest --root path/to/documents --database /tmp/folio
cargo run -p x0k-folio-cli -- query --database /tmp/folio --file query.json --format json
```

Definitions in the collection supply its vocabulary. Add `--shipped` to
`ingest` to include the supplied vocabulary, or use `--vocabulary path/to/modules`
for a directory of Turtle modules.
[Query files](../../../../crates/x0k-folio-cli/examples/queries) use Dialog's native
JSON query and rule format. Edit their properties and bindings to ask a
different question. JSON output preserves the distinction between references,
text, numbers, and other value types.

`watch` follows edits; `status` reports rejected documents and pending
deliveries; `rebuild` prepares a fresh database generation. For larger
collections, build with `cargo build --release -p x0k-folio-cli` and use
`target/release/x0k-folio-cli` with the same arguments.

#### Try the Paper collection

The [example collection](../../../../crates/x0k-folio-cli/examples/papers) defines
its own Paper concept and citation relationship. Ingest it, then ask which
paper cites which:

```sh
cargo run -p x0k-folio-cli -- ingest --root crates/x0k-folio-cli/examples/papers --database /tmp/folio-papers
cargo run -p x0k-folio-cli -- query --database /tmp/folio-papers --file crates/x0k-folio-cli/examples/queries/citations.json --format json
```

The result links Alpha's citation of Beta to the document that declares it.
The [command documentation](../../../../implementation/folio/query-cli.md)
explains native rules, query limits, and keeping the database current.
