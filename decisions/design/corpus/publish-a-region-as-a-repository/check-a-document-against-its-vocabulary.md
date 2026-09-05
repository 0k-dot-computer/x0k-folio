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
