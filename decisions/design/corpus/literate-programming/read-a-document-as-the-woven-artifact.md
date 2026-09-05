---
x0k:
  format: folio/v1
  id: x0k:design/literate-programming#read-a-document-as-the-woven-artifact
  type: design
  status: proposed
  edges:
    transcludes:
      - x0k:design/literate-programming
---

### Read a document as the woven artifact

I read a literate document as a rendered whole — its prose and its code in one
continuous argument, code spans highlighted, chunks resolved where they are
referenced rather than where they happen to be defined. The reading order is
the one the author chose, not the one the compiler needs.

```yaml x0k:affordance
id: x0k:affordance/weave_a_document
actors: [human]
edges:
  enabledBy:
    - x0k:software-module/x0k-tangle
    - x0k:software-module/x0k-syntax
    - x0k:software-module/x0k-folio
  requires:
    - x0k:affordance/tangle_source_from_a_document
```
