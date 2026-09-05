---
x0k:
  format: folio/v1
  id: x0k:design/publish-a-region-as-a-repository#read-an-affordance-out-of-a-document
  type: design
  status: proposed
  edges:
    transcludes:
      - x0k:design/publish-a-region-as-a-repository
---

### Read an affordance out of a document

I extract the affordances a document declares — their identity, actors
and edges — as data rather than as prose I have to interpret. What the
declaration says is available to my own tooling, so "what can I do with this"
is a question I can answer by reading the corpus rather than by trusting its
summary of itself.

```yaml x0k:affordance
id: x0k:affordance/read_declared_affordances
actors: [human, ai_agent]
edges:
  enabledBy:
    - x0k:software-module/x0k-folio
```
