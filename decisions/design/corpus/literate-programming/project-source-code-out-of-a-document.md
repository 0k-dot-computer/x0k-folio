---
x0k:
  format: folio/v1
  id: x0k:design/literate-programming#project-source-code-out-of-a-document
  type: design
  status: proposed
  edges:
    transcludes:
      - x0k:design/literate-programming
---

### Project source code out of a document

I write the program and its explanation as one document, and get compilable
source out of it. The document is the artifact I maintain; the code is what it
projects. Nothing asks me to keep the two in agreement, because only one of
them is authored.

```yaml x0k:affordance
id: x0k:affordance/tangle_source_from_a_document
actors: [human, ai_agent]
edges:
  enabledBy:
    - x0k:software-module/x0k-tangle
    - x0k:software-module/x0k-folio
```
