---
x0k:
  format: folio/v1
  id: x0k:implementation/folio/materialize
  type: implementation
  status: draft
  summary: The folio/v1 materializer — facts back out to an envelope, with no Loro anywhere in it, so the published build ships a facts→file implementation instead of a feature-gated one.
  concerns:
  - folio
  - materialization
  - facts
  - projection
  - publication
  tangle:
    crate: crates/x0k-folio
    root: src/materialize.rs
  edges:
    implements:
    - x0k:architecture/filesystem-graph-materialization
    cites:
    - x0k:architecture/publication-is-the-shipping-unit
---

# Facts back out to a folio

The sibling chapter `projection.md` is the folio plugin the daemon calls,
and it is exactly the thing this publication cannot ship: it reads a Loro
document, it speaks `DocumentOp`, and both of those live in `x0k-types`,
which the folio publication excludes. So it hides behind the `plugins`
feature, the repository projector severs that feature at publish, and a
reader who installs `x0k-folio` gets a format library with no way to write
a document back out.

This chapter is the other one. It implements
[`Materializer`](x0k_fact_projection::Materializer) — facts in, canonical
bytes plus a placement out — and it does it with no Loro, no ops and no
daemon. It is not a smaller version of the plugin; it is the direction the
plugin was carrying that never needed the substrate.

## What the facts actually hold

Start with the honest limit, because it decides the shape of everything
below. [`project_envelope`](x0k_fact_projection::project_envelope) projects
a folio *envelope*: status, type, subtype, body format, concerns, the
materialization pointers, and the edges. It does not project the body, and
no predicate on the fact plane names one today.

So this materializer reconstructs the envelope exactly and renders the body
from `x0k:folio/body` when a producer has put one there. Absent that fact,
the document materializes as its envelope with an empty body — which is the
true statement about what the facts contain, not a gap papered over. The
round-trip this chapter proves is therefore envelope-exact, and that is
precisely as much as the fact plane currently supports.

<a name="chunk-body-predicate"></a><sub>[`src/materialize.rs`](../../crates/x0k-folio/src/materialize.rs) · `#body-predicate`</sub>

```rust {#body-predicate}
/// Predicate carrying a document's body text.
///
/// In the `x0k:folio/*` namespace `x0k-fact-projection` already owns
/// (beside `bodyFormat`, which presumes a body exists), but deliberately
/// NOT emitted by `project_envelope` — that function projects the
/// envelope, and a body reaching the fact plane is a producer's choice.
/// A materializer reads it if it is there.
pub const BODY_PREDICATE: &str = "x0k:folio/body";
```

## Rebuilding the envelope

The reconstruction is a fold over one entity's facts, and its only real
decisions are what to do with a value that does not belong. Two rules, both
refusals:

- A **text fact under a known envelope predicate whose value is not a legal
  member of that field** — a status of `"perhaps"`, a type of `"poem"` — is
  an error, not a field to drop. Dropping it would materialize a document
  that silently disagrees with the facts it came from.
- An **entity reference under any predicate** is an edge. That is the same
  division `project_envelope` makes on the way in, read backwards, so a
  predicate nobody has heard of round-trips rather than being lost.

<a name="chunk-from-facts"></a><sub>[`src/materialize.rs`](../../crates/x0k-folio/src/materialize.rs) · `#from-facts`</sub>

```rust {#from-facts}
/// Rebuild a folio envelope from one entity's facts.
///
/// The inverse of [`project_envelope`](x0k_fact_projection::project_envelope),
/// including its treatment of unknown predicates: anything carrying an
/// entity reference is an edge, so a predicate this build has never heard
/// of survives the trip.
pub fn colophon_from_facts(facts: &[FactEntry]) -> Result<(Colophon, String), MaterializeError> {
    let Some(first) = facts.first() else {
        return Err(MaterializeError("no facts to materialize".into()));
    };
    // Written out rather than spread from a default: `Colophon` has no
    // `Default`, and giving it one so this line could be shorter would put a
    // genus-less envelope one `..` away from every other caller.
    let mut envelope = Colophon {
        id: first.entity.clone(),
        doc_type: DocType::Implementation,
        subtype: None,
        status: None,
        concerns: Vec::new(),
        summary: None,
        updated_by: None,
        created_at: None,
        updated_at: None,
        edges: BTreeMap::new(),
        materialization: None,
        tangle: None,
        pipelines: Vec::new(),
        body_format: BODY_FORMAT_MARKDOWN.to_string(),
    };
    let mut body = String::new();
    let mut materialization = Materialization::default();
    let mut saw_materialization = false;
    let mut saw_type = false;

    for fact in facts {
        if fact.entity != envelope.id {
            return Err(MaterializeError(format!(
                "materializing `{}` but got a fact about `{}` — a materializer renders one entity",
                envelope.id, fact.entity
            )));
        }
        match &fact.value {
            FactValue::EntityRef(target) => {
                envelope
                    .edges
                    .entry(fact.predicate.clone())
                    .or_default()
                    .push(target.clone());
            }
            FactValue::Text(text) => match fact.predicate.as_str() {
                envelope_predicates::STATUS => {
                    envelope.status = Some(Status::from_str(text).ok_or_else(|| {
                        MaterializeError(format!("`{text}` is not a folio status"))
                    })?);
                }
                envelope_predicates::DOC_TYPE => {
                    envelope.doc_type = DocType::from_str(text).ok_or_else(|| {
                        MaterializeError(format!("`{text}` is not a folio type"))
                    })?;
                    saw_type = true;
                }
                envelope_predicates::SUBTYPE => envelope.subtype = Some(text.clone()),
                envelope_predicates::BODY_FORMAT => envelope.body_format = text.clone(),
                envelope_predicates::CONCERN => envelope.concerns.push(text.clone()),
                envelope_predicates::MATERIALIZATION_LORO_DOC => {
                    materialization.loro_doc_id = Some(text.clone());
                    saw_materialization = true;
                }
                envelope_predicates::MATERIALIZATION_REVISION => {
                    materialization.document_revision_id = Some(text.clone());
                    saw_materialization = true;
                }
                envelope_predicates::MATERIALIZATION_CONTENT_HASH => {
                    materialization.content_hash = Some(text.clone());
                    saw_materialization = true;
                }
                BODY_PREDICATE => body = text.clone(),
                // A text value under an unknown predicate is not an edge and
                // has no envelope field. Refusing is what keeps a lossy
                // render from looking like a successful one.
                other => {
                    return Err(MaterializeError(format!(
                        "no envelope field for text predicate `{other}`"
                    )))
                }
            },
            other => {
                return Err(MaterializeError(format!(
                    "folio envelopes carry text and entity refs; got {other:?} at `{}`",
                    fact.predicate
                )))
            }
        }
    }

    if !saw_type {
        return Err(MaterializeError(format!(
            "`{}` has no `{}` fact, so its genus is unknown",
            envelope.id,
            envelope_predicates::DOC_TYPE
        )));
    }
    if saw_materialization {
        envelope.materialization = Some(materialization);
    }
    Ok((envelope, body))
}
```

## The materializer

Everything above is the work; the trait implementation is the seam it
plugs into. The placement and the policy are held rather than hardcoded,
because one `folio/v1` name serves many classes — `x0k:wiki/*` and
`x0k:design/*` are the same format at different paths under different
policies.

<a name="chunk-materializer"></a><sub>[`src/materialize.rs`](../../crates/x0k-folio/src/materialize.rs) · `#materializer`</sub>

```rust {#materializer}
/// The `folio/v1` materializer: facts → envelope bytes.
///
/// Parameterized per class by placement and policy, so several instances
/// of one format serve `x0k:wiki/*`, `x0k:design/*` and the rest.
#[derive(Debug, Clone)]
pub struct FolioMaterializer {
    placement: PathTemplate,
    live_edit_policy: LiveEditPolicy,
}

impl FolioMaterializer {
    pub fn new(placement: PathTemplate, live_edit_policy: LiveEditPolicy) -> Self {
        Self {
            placement,
            live_edit_policy,
        }
    }
}

impl Materializer for FolioMaterializer {
    fn name(&self) -> &str {
        FOLIO_V1_MATERIALIZER_NAME
    }

    fn placement(&self) -> &PathTemplate {
        &self.placement
    }

    fn live_edit_policy(&self) -> LiveEditPolicy {
        self.live_edit_policy
    }

    fn render(&self, facts: &[FactEntry]) -> Result<Vec<u8>, MaterializeError> {
        let (envelope, body) = colophon_from_facts(facts)?;
        let mut out = render_envelope(&envelope);
        out.push_str(&body);
        Ok(out.into_bytes())
    }
}

/// The name this materializer answers to, matching the `plugin = "..."`
/// spelling a class manifest uses.
pub const FOLIO_V1_MATERIALIZER_NAME: &str = "folio/v1";
```

## The module

<a name="chunk-root"></a><sub>[`src/materialize.rs`](../../crates/x0k-folio/src/materialize.rs) · `#root` · assembles [body-predicate](#chunk-body-predicate) · [from-facts](#chunk-from-facts) · [materializer](#chunk-materializer) · [tests](#chunk-tests)</sub>

```rust {#root}
//! The `folio/v1` materializer — facts back out to an envelope.
//!
//! [`FolioMaterializer`] implements
//! [`Materializer`](x0k_fact_projection::Materializer) with no Loro, no
//! `DocumentOp` and no daemon, which is what lets the published build ship
//! a facts→file implementation. The sibling `projection` module is the
//! Loro-backed plugin the daemon calls; it is behind the `plugins` feature
//! and severed at publish, and this module is not a smaller version of it
//! but the direction it was carrying that never needed the substrate.
//!
//! The round-trip is envelope-exact: the fact plane holds a folio's
//! envelope, and [`BODY_PREDICATE`] is where a producer may put the body
//! it does not otherwise carry.
//!
//! Governing decision:
//! `corpora/x0k/decisions/architecture/production/filesystem-graph-materialization.md` §8.

use crate::colophon::{
    render_envelope, Colophon, DocType, Materialization, Status, BODY_FORMAT_MARKDOWN,
};
use x0k_fact_projection::materialize::{
    LiveEditPolicy, MaterializeError, Materializer, PathTemplate,
};
use x0k_fact_projection::{envelope_predicates, FactEntry, FactValue};
use std::collections::BTreeMap;

<<body-predicate>>

<<from-facts>>

<<materializer>>

<<tests>>
```

## Proving it

The claim is a round-trip, and the only trustworthy shape for it is the
full loop: a view, projected to facts, materialized to bytes, parsed back,
projected again — with the two fact vectors compared. Comparing the bytes
instead would prove that a renderer is deterministic, which is not the
question.

One asymmetry is real and worth naming rather than hiding: `ColophonView`
carries edges in the parser's iteration order, while `Colophon` holds them
in a `BTreeMap` and renders them sorted. A document whose edges arrive
unsorted therefore round-trips its edge SET exactly and its edge ORDER to
sorted order. The test says so by using sorted predicates, so a future
change that starts losing edges fails rather than being absorbed.

<a name="chunk-tests"></a><sub>[`src/materialize.rs`](../../crates/x0k-folio/src/materialize.rs) · `#tests`</sub>

```rust {#tests}
#[cfg(test)]
mod tests {
    use super::*;
    use crate::colophon::parse_envelope;
    use x0k_fact_projection::{project_envelope, ColophonView};

    fn view() -> ColophonView {
        ColophonView {
            uri: "x0k:design/code-ingestion".to_string(),
            status: "accepted".to_string(),
            doc_type: "design".to_string(),
            subtype: None,
            body_format: BODY_FORMAT_MARKDOWN.to_string(),
            concerns: vec!["corpus".to_string(), "literate".to_string()],
            materialization_loro_doc_id: Some("loro:abc".to_string()),
            materialization_document_revision_id: None,
            materialization_content_hash: Some("blake3:def".to_string()),
            // Sorted, because rendering sorts: see the note above.
            edges: vec![
                ("informedBy".to_string(), vec!["x0k:wiki/literate-programming".to_string()]),
                ("motivatedBy".to_string(), vec!["x0k:commitment/a".to_string()]),
            ],
            unknown_edges: vec![(
                "predicateFromTheFuture".to_string(),
                vec!["x0k:design/other".to_string()],
            )],
        }
    }

    fn materializer() -> FolioMaterializer {
        FolioMaterializer::new(
            PathTemplate::parse("corpora/x0k/decisions/design/{slug}.md").unwrap(),
            LiveEditPolicy::Forbidden,
        )
    }

    /// The oracle: facts → bytes → facts, with nothing Loro-shaped in the
    /// loop and the two fact vectors equal.
    #[test]
    fn a_document_round_trips_through_the_materializer() {
        let facts = project_envelope(&view());
        let rendered = materializer()
            .materialize(Some("code-ingestion"), &facts)
            .unwrap();
        assert_eq!(rendered.path, "corpora/x0k/decisions/design/code-ingestion.md");

        let text = String::from_utf8(rendered.bytes).unwrap();
        let (envelope, _body) = parse_envelope(&text).expect("materialized bytes parse");

        // Back to a view the way an ingester builds one, then to facts.
        let mut back = ColophonView {
            uri: envelope.id.clone(),
            status: envelope.status.map(|s| s.as_str().to_string()).unwrap_or_default(),
            doc_type: envelope.doc_type.as_str().to_string(),
            subtype: envelope.subtype.clone(),
            body_format: envelope.body_format.clone(),
            concerns: envelope.concerns.clone(),
            materialization_loro_doc_id: None,
            materialization_document_revision_id: None,
            materialization_content_hash: None,
            edges: envelope.edges.iter().map(|(p, t)| (p.clone(), t.clone())).collect(),
            unknown_edges: Vec::new(),
        };
        if let Some(m) = &envelope.materialization {
            back.materialization_loro_doc_id = m.loro_doc_id.clone();
            back.materialization_document_revision_id = m.document_revision_id.clone();
            back.materialization_content_hash = m.content_hash.clone();
        }
        assert_eq!(project_envelope(&back), facts);
    }

    /// A body a producer put on the fact plane comes back out with it.
    #[test]
    fn a_body_fact_is_rendered_after_the_envelope() {
        let mut facts = project_envelope(&view());
        facts.push(FactEntry::new(
            "x0k:design/code-ingestion",
            BODY_PREDICATE,
            FactValue::Text("\n# Ingesting code you already have\n".to_string()),
        ));
        let bytes = materializer().render(&facts).unwrap();
        let text = String::from_utf8(bytes).unwrap();
        assert!(text.ends_with("\n# Ingesting code you already have\n"), "{text}");
        let (_, body) = parse_envelope(&text).unwrap();
        assert!(body.contains("Ingesting code you already have"));
    }

    /// Without a body fact the document is its envelope — the true
    /// statement about what the facts hold.
    #[test]
    fn no_body_fact_materializes_an_envelope_alone() {
        let text = String::from_utf8(materializer().render(&project_envelope(&view())).unwrap())
            .unwrap();
        let (_, body) = parse_envelope(&text).unwrap();
        assert!(body.trim().is_empty(), "unexpected body: {body:?}");
    }

    /// A value no envelope field accepts is a refusal, not a dropped
    /// field: materializing it would produce a document that disagrees
    /// with the facts it came from.
    #[test]
    fn an_illegal_field_value_refuses_rather_than_dropping() {
        let mut view = view();
        view.status = "perhaps".to_string();
        let err = materializer()
            .render(&project_envelope(&view))
            .expect_err("illegal status");
        assert!(err.to_string().contains("not a folio status"), "{err}");
    }

    /// One materializer call renders one entity. Facts about two would
    /// otherwise interleave into a single envelope.
    #[test]
    fn facts_about_two_entities_are_refused() {
        let mut facts = project_envelope(&view());
        facts.push(FactEntry::new(
            "x0k:design/somebody-else",
            envelope_predicates::STATUS,
            FactValue::Text("proposed".to_string()),
        ));
        let err = materializer().render(&facts).expect_err("two entities");
        assert!(err.to_string().contains("renders one entity"), "{err}");
    }
}
```
