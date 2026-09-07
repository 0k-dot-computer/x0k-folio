---
x0k:
  format: folio/v1
  id: x0k:implementation/folio/checking
  type: implementation
  status: draft
  summary: Reading an envelope against a vocabulary the caller names — and keeping a missing term, which is a packaging defect, apart from a missing target, which is the boundary working.
  concerns:
  - folio
  - ontology
  - publishing
  - validation
  - vocabulary
  tangle:
    crate: crates/x0k-folio
    root: src/envelope_check.rs
  edges:
    implements:
    - x0k:design/publish-a-region-as-a-repository
    cites:
    - x0k:architecture/publication-projection
    - x0k:architecture/ontology-modules
    - x0k:implementation/folio/colophon
    - x0k:implementation/folio/identity
    - x0k:implementation/ontology/load
---
# Checking a document against what shipped with it

A publication of this crate ships three things that ought to be able to
meet: the format, the vocabulary its `type` and `edges` are terms of (an
[RDF and OWL](x0k:wiki/rdf-and-owl) ontology, shipped as modules), and
the documents themselves. Until this module they could not. The parser
read `edges:` into a `BTreeMap<String, Vec<String>>` and its own comment
said predicate vocabularies were validated by the consumer — and the only
consumer that did was the daemon, which is not published. So the bundle
shipped a format, shipped a vocabulary, and shipped nothing that read one
against the other. That was a **missing layer, not a withheld one**
(`x0k:architecture/publication-projection` §6), and this module is the
layer.

## The check that matters is which of two things went wrong

Point a checker at a published bundle and almost every document will
have edges that resolve to nothing. That is not a bug; a publication is a
*region*, and an edge out of the region is the boundary doing its job, as
the [open-world assumption](x0k:wiki/open-world-assumption) says it should.
Point the same checker at a document whose `edges:` block uses a
predicate the shipped vocabulary never defines, and something is
genuinely wrong — not with the document, with the packaging. Somebody
selected a set of ontology modules that does not span what the corpus
says.

Conflating those two is how a check becomes noise. So the report keeps
them structurally apart, in two lists that never merge:

- a **defect** is the shipped vocabulary failing to express what a
  document says: a malformed id, a malformed edge target, a predicate no
  shipped module declares;
- a **dangling edge** is a well-formed target naming no document in the
  set being checked. Expected. Reported, never counted as failure.

`cites` was the live example of the first, and it is worth keeping the
story because it is a term this very document uses. It was declared only
by the agent-curated wiki vocabulary, in that vocabulary's own namespace,
which no consumer of an envelope's `cites:` key ever resolved to — so a
bundle shipping `core`, `document` and `software` did not define it, and
the checker over that bundle said so about thirty-eight of the documents
it was shipping. The answer was neither of the two the report suggests:
the term was not wiki-scoped and had no module to wait for. It is now
`x0k:cites` in `core`, with no domain and no range, and what the report
still names in that shape is a genuine module-selection question rather
than a term filed in the wrong house.

<a name="chunk-module-doc"></a><sub>[`src/envelope_check.rs`](../../crates/x0k-folio/src/envelope_check.rs) · `#module-doc`</sub>

```rust {#module-doc}
//! Reading a folio/v1 envelope against a vocabulary the caller names.
//!
//! Every function here takes an [`OntologyModel`]: the set this build
//! compiled (`OntologyModel::shipped()`), or one read off a module
//! directory (`OntologyModel::load`). The vocabulary is a parameter, not
//! a property of the binary.
//!
//! The check answers two different questions and never lets their
//! answers mix:
//!
//! - **Can the shipped vocabulary express what this document says?** A
//!   `no` is a [`Defect`] — a malformed id, a malformed edge target, or
//!   a predicate no shipped ontology module declares. The last is a
//!   packaging fault: a publication selected a module set that does not
//!   span its own corpus.
//! - **Does this edge's target name a document in the set being
//!   checked?** A `no` is a [`DanglingEdge`], which is ordinary and
//!   expected: a publication is a region of a graph, and an edge leaving
//!   the region is the boundary working, not a failure.
//!
//! Which modules the model holds decides the first answer, so the same
//! document checks differently against a monorepo vocabulary and a
//! published bundle's. That is the point: the check measures the
//! vocabulary it was pointed at, not the corpus.
//!
//! A third question is asked of the entities declared *inside* the
//! documents rather than of their envelopes — **does every claim made
//! to a human have a cue a human could perceive?** A `no` is a
//! [`DeclarationDefect`]: an affordance `claimedFor` a human that no
//! signifier signifies, which is a promise to a perception-dependent
//! actor with nothing to perceive.

use std::collections::{BTreeMap, BTreeSet};

use x0k_ontology::concept_facts::OntologyModel;

use crate::colophon::Colophon;
use crate::entity_id::EntityId;
use crate::inline_entity::{declared_facts, InlineEntity};
```

## Standing: what a module has to say about a predicate

A model answers two different questions about a predicate, and the
difference between them is the difference between two `no`s. The
*Decision-domain slice* is the predicates whose subject reaches
`x0k:Decision`, which is what a decision document's `edges:` block
normally draws from. The object-property table covers every object
property the modules declare, whatever its subject.

A predicate can therefore be outside the slice and still perfectly real:
`child_of` has an `x0k:Intent` domain, and an intent's envelope is right
to use it. Calling that undeclared would be wrong. So standing has three
values, not two.

<a name="chunk-standing"></a><sub>[`src/envelope_check.rs`](../../crates/x0k-folio/src/envelope_check.rs) · `#standing`</sub>

```rust {#standing}
/// What a vocabulary has to say about an `edges:` predicate, in its
/// snake_case frontmatter form.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PredicateStanding {
    /// Declared with `x0k:Decision` in its domain — the slice a decision
    /// document's `edges:` block draws from.
    DocumentEdge { uri: String },
    /// Declared, but for some other subject (`child_of` is an intent's
    /// edge, not a decision's). Real, and not a defect on a document of
    /// the matching genus.
    DeclaredElsewhere {
        uri: String,
        domain: Option<String>,
        range: Option<String>,
    },
    /// No module of this vocabulary declares it. Either the term is not
    /// vocabulary at all, or the module that defines it was not selected.
    Undeclared,
}

/// Ask a vocabulary about one predicate.
pub fn predicate_standing(model: &OntologyModel, snake: &str) -> PredicateStanding {
    Vocabulary::of(model).standing(snake)
}
```

### One fold, not one per question

A model is a fact list, and every view over it — the class table, the
object properties, the Decision-domain slice — is a fold that walks the
whole list. Asking it per predicate is fine for one document and
quadratic over a corpus: a thousand documents with five edges apiece is
five thousand folds of the same twelve hundred facts. So the folds happen
once, into three lookup tables, and the rest of this module reads those.

`Vocabulary` is private on purpose. It is a cache of `model`'s answers and
nothing more — no judgment lives here that the model could not be asked
directly — and making it public would invite a caller to hold one and
outlive the vocabulary it came from.

<a name="chunk-vocabulary"></a><sub>[`src/envelope_check.rs`](../../crates/x0k-folio/src/envelope_check.rs) · `#vocabulary`</sub>

```rust {#vocabulary}
/// The folds this module needs, taken once from a model.
struct Vocabulary {
    /// snake_case → camelCase for the Decision-domain slice.
    document_edges: BTreeMap<String, String>,
    /// Compact URI → `(domain, range)` for every declared object property.
    properties: BTreeMap<String, (Option<String>, Option<String>)>,
    /// The namespace prefixes an id may carry.
    schemes: BTreeSet<String>,
}

impl Vocabulary {
    fn of(model: &OntologyModel) -> Self {
        Self {
            document_edges: model.decision_edge_predicates().into_iter().collect(),
            properties: model
                .object_properties()
                .into_iter()
                .map(|property| (property.uri, (property.domain, property.range)))
                .collect(),
            schemes: model.schemes(),
        }
    }

    fn standing(&self, snake: &str) -> PredicateStanding {
        if let Some(camel) = self.document_edges.get(snake) {
            return PredicateStanding::DocumentEdge {
                uri: format!("x0k:{camel}"),
            };
        }
        let uri = format!("x0k:{}", camel_form(snake));
        match self.properties.get(&uri) {
            Some((domain, range)) => PredicateStanding::DeclaredElsewhere {
                uri,
                domain: domain.clone(),
                range: range.clone(),
            },
            None => PredicateStanding::Undeclared,
        }
    }

    /// Parse an id, licensing whatever namespaces this vocabulary declares.
    fn id(&self, raw: &str) -> Result<EntityId, crate::entity_id::EntityIdError> {
        EntityId::parse_with_schemes(raw, &self.schemes)
    }
}
```

The camelCase form is derivable rather than looked up, because the two
spellings are one deterministic rule — the ontology's own view inserts
`_` before an interior uppercase and lowercases, so the inverse
capitalizes the letter after each `_`. Deriving it is what lets the
question reach properties outside the Decision-domain slice, which is the
whole reason `DeclaredElsewhere` can exist.

<a name="chunk-camel-form"></a><sub>[`src/envelope_check.rs`](../../crates/x0k-folio/src/envelope_check.rs) · `#camel-form`</sub>

```rust {#camel-form}
/// snake_case → camelCase, the inverse of `camel_to_snake`. Used only to
/// *ask* about a predicate outside the Decision-domain slice; inside it,
/// the model's own map is authoritative.
fn camel_form(snake: &str) -> String {
    let mut out = String::with_capacity(snake.len());
    let mut capitalize_next = false;
    for ch in snake.chars() {
        if ch == '_' {
            capitalize_next = true;
        } else if capitalize_next {
            out.extend(ch.to_uppercase());
            capitalize_next = false;
        } else {
            out.push(ch);
        }
    }
    out
}
```

## Defects

Three ways a document can outrun the vocabulary shipped beside it. Each
carries the offending string, because these are read in a report over a
whole corpus where the finding without its subject is unactionable.

<a name="chunk-defect"></a><sub>[`src/envelope_check.rs`](../../crates/x0k-folio/src/envelope_check.rs) · `#defect`</sub>

```rust {#defect}
/// The shipped vocabulary cannot express something this document says.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Defect {
    /// `x0k.id` is not `x0k:<class>/<slug>`.
    MalformedId { value: String, reason: String },
    /// An `edges:` target is not `x0k:<class>/<slug>`.
    MalformedTarget {
        predicate: String,
        value: String,
        reason: String,
    },
    /// No module of the vocabulary declares this predicate. A packaging
    /// fault, not a document fault: the module that defines the term was
    /// not selected, or the term is not vocabulary at all.
    UndeclaredPredicate { predicate: String },
}

impl std::fmt::Display for Defect {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::MalformedId { value, reason } => {
                write!(f, "`x0k.id` is not a well-formed id: {reason} (`{value}`)")
            }
            Self::MalformedTarget {
                predicate,
                value,
                reason,
            } => write!(
                f,
                "edge `{predicate}` has a malformed target: {reason} (`{value}`)"
            ),
            Self::UndeclaredPredicate { predicate } => write!(
                f,
                "edge predicate `{predicate}` is declared by no ontology module in this \
                 vocabulary; \
                 either select the module that defines it or stop using the term"
            ),
        }
    }
}

impl std::error::Error for Defect {}
```

## `check_envelope`: one document

`check_envelope` is the face of [checking a document against what
shipped with it](../../decisions/design/corpus/publish-a-region-as-a-repository/check-a-document-against-its-vocabulary.md "x0k:affordance/check_a_document_against_shipped_vocabulary"). A caller holding one parsed
envelope reaches for it and gets back a report: the document's id, its
well-formed edges, and the defects, in the order found. That is all the
cue there is — a reader of this library learns the check is here from
the function's name and its rustdoc — so it is declared as the cue
below, beside the function it describes rather than on the affordance
it makes reachable (`x0k:design/publish-a-region-as-a-repository`, "a
face declares its signifier where the face lives"):

<a name="folio-instance-68747470733a2f2f306b2e636f6d70757465722f6f6e746f6c6f6779237369676e69666965722f78306b2d666f6c696f2d636865636b2d656e76656c6f7065-1"></a><sub data-instance-iri="https://0k.computer/ontology#signifier/x0k-folio-check-envelope" data-concept-iri="https://0k.computer/ontology#Signifier" data-source-document="corpora/x0k/implementation/folio/checking.md"><strong>Signifier</strong> · check_envelope: one document · <code>https://0k.computer/ontology#signifier/x0k-folio-check-envelope</code> · <a href="#folio-source-68747470733a2f2f306b2e636f6d70757465722f6f6e746f6c6f6779237369676e69666965722f78306b2d666f6c696f2d636865636b2d656e76656c6f7065-1">source declaration</a></sub><a name="folio-source-68747470733a2f2f306b2e636f6d70757465722f6f6e746f6c6f6779237369676e69666965722f78306b2d666f6c696f2d636865636b2d656e76656c6f7065-1"></a>

```yaml x0k:signifier
id: x0k:signifier/x0k-folio-check-envelope
cue: check_envelope
edges:
  signifies:
    - x0k:affordance/check_a_document_against_shipped_vocabulary
  presentedOn:
    - x0k:surface/sdk
```

The report keeps the well-formed edges as well as the faults, because
the corpus pass needs them and a caller checking a single document
usually wants to know what it points at.

<a name="chunk-check-envelope"></a><sub>[`src/envelope_check.rs`](../../crates/x0k-folio/src/envelope_check.rs) · `#check-envelope`</sub>

```rust {#check-envelope}
/// What checking one envelope against the shipped vocabulary found.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct EnvelopeReport {
    /// The document's own id, when it parsed.
    pub id: Option<EntityId>,
    /// Every `(predicate, target)` pair whose target parsed, ordered by
    /// predicate then by declaration — the envelope holds `edges:` in a
    /// `BTreeMap`, so the order is the vocabulary's, not the author's.
    /// Predicates are kept in their snake_case form.
    pub edges: Vec<(String, EntityId)>,
    /// Faults, in the order found.
    pub defects: Vec<Defect>,
}

impl EnvelopeReport {
    /// True when the shipped vocabulary expressed everything the
    /// document said. Says nothing about whether its edges resolve.
    pub fn is_clean(&self) -> bool {
        self.defects.is_empty()
    }
}

/// Check one parsed envelope against `model`. Pure: no filesystem, no
/// corpus, no resolution — a `DanglingEdge` cannot be found from one
/// document.
pub fn check_envelope(model: &OntologyModel, envelope: &Colophon) -> EnvelopeReport {
    check_envelope_with(&Vocabulary::of(model), envelope)
}

fn check_envelope_with(vocabulary: &Vocabulary, envelope: &Colophon) -> EnvelopeReport {
    let mut report = EnvelopeReport::default();

    match vocabulary.id(&envelope.id) {
        Ok(id) => report.id = Some(id),
        Err(e) => report.defects.push(Defect::MalformedId {
            value: envelope.id.clone(),
            reason: e.to_string(),
        }),
    }

    for (predicate, targets) in &envelope.edges {
        if matches!(vocabulary.standing(predicate), PredicateStanding::Undeclared) {
            report.defects.push(Defect::UndeclaredPredicate {
                predicate: predicate.clone(),
            });
        }
        for target in targets {
            match vocabulary.id(target) {
                Ok(id) => report.edges.push((predicate.clone(), id)),
                Err(e) => report.defects.push(Defect::MalformedTarget {
                    predicate: predicate.clone(),
                    value: target.clone(),
                    reason: e.to_string(),
                }),
            }
        }
    }

    report
}
```

## A set of documents

Resolution needs a set to resolve against, and the set is whatever the
caller hands over — a published bundle's documents, a directory, one
region. Everything reachable from an envelope but not in that set is
reported as dangling, in a list of its own.

Two properties of this pass are deliberate. It resolves against
**envelope ids only**, not against inline entities, because an
affordance's home is its parent document and a checker that silently
resolved through inline definitions would hide a broken parent. And a
document whose own id is malformed contributes no id to the set, so its
edges are still checked and its inbound edges dangle — the fault is
reported once, at its source, and its consequences are visible rather
than swallowed.

<a name="chunk-check-corpus"></a><sub>[`src/envelope_check.rs`](../../crates/x0k-folio/src/envelope_check.rs) · `#check-corpus`</sub>

```rust {#check-corpus}
/// A well-formed edge whose target names no document in the checked set.
/// Ordinary on a published region: an edge out of the region is the
/// publication boundary, not a fault.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DanglingEdge {
    /// Caller-supplied name for the document that declared the edge —
    /// a path, a URL, whatever the caller identifies documents by.
    pub source: String,
    pub subject: EntityId,
    pub predicate: String,
    pub target: EntityId,
}

/// What checking a set of envelopes against each other and the shipped
/// vocabulary found.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CorpusReport {
    /// How many envelopes were examined.
    pub checked: usize,
    /// Faults, each tagged with the caller's name for its document.
    pub defects: Vec<(String, Defect)>,
    /// Edges leaving the checked set.
    pub dangling: Vec<DanglingEdge>,
}

impl CorpusReport {
    /// True when the shipped vocabulary expressed everything every
    /// document said. Dangling edges do not affect this.
    pub fn is_clean(&self) -> bool {
        self.defects.is_empty()
    }
}

/// Check a set of envelopes against `model`. Each item is the caller's
/// name for a document paired with its parsed envelope. The vocabulary is
/// folded once for the whole set.
pub fn check_corpus<'a, I>(model: &OntologyModel, documents: I) -> CorpusReport
where
    I: IntoIterator<Item = (&'a str, &'a Colophon)>,
{
    let vocabulary = Vocabulary::of(model);
    let reports: Vec<(&str, EnvelopeReport)> = documents
        .into_iter()
        .map(|(source, envelope)| (source, check_envelope_with(&vocabulary, envelope)))
        .collect();

    let present: BTreeSet<&EntityId> = reports
        .iter()
        .filter_map(|(_, report)| report.id.as_ref())
        .collect();

    let mut out = CorpusReport {
        checked: reports.len(),
        ..CorpusReport::default()
    };

    for (source, report) in &reports {
        for defect in &report.defects {
            out.defects.push((source.to_string(), defect.clone()));
        }
        let Some(subject) = report.id.as_ref() else {
            continue;
        };
        for (predicate, target) in &report.edges {
            if !present.contains(target) {
                out.dangling.push(DanglingEdge {
                    source: source.to_string(),
                    subject: subject.clone(),
                    predicate: predicate.clone(),
                    target: target.clone(),
                });
            }
        }
    }

    out
}
```

## Declarations: a human claim needs a cue

The envelope check reads what a document says about itself. The
entities declared inside a document say something more, and one of the
things they say can be false in a way the vocabulary alone cannot
catch. An affordance `claimedFor` a human is a claim that a person can
reach it, and by the `Actor` definition a person reaches an affordance
through a perceivable cue — a `Signifier` `presentedOn` a `Surface`. An
affordance claimed for a human that no signifier signifies is therefore
a claim with nothing behind it: the audience is told they can do
something and given no way to find where.

An agent claim carries no such obligation. A structured actor reads
the descriptor, and the published library is one, so `actors:
[ai_agent]` alone is clean by construction. The check is only ever
about the human.

The check runs over extracted [`InlineEntity`](inline-entities.md)
records, whichever documents they came from, and it does not resolve
anything: it reads the `claimedFor` facts of every `affordance` and the
`signifies` facts of every `signifier`, and reports the affordances
that claim a human and are named by no signifier. What it measured
when first run over this publication: two of its four affordances —
`check_a_document_against_shipped_vocabulary` and
`read_declared_affordances` — claimed a human and were reachable only
through Rust, with nothing declared to say so. The two signifiers in
this crate's chapters are what made those claims true.

<a name="chunk-check-declarations"></a><sub>[`src/envelope_check.rs`](../../crates/x0k-folio/src/envelope_check.rs) · `#check-declarations`</sub>

```rust {#check-declarations}
/// A declaration the shipped vocabulary expresses but that does not
/// hold.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DeclarationDefect {
    /// An affordance `claimedFor` `x0k:actor/human` that no signifier
    /// signifies. A human reaches an affordance through a perceivable
    /// cue; with none declared, the claim cannot be kept.
    HumanClaimWithoutSignifier { affordance: EntityId },
}

impl std::fmt::Display for DeclarationDefect {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::HumanClaimWithoutSignifier { affordance } => write!(
                f,
                "affordance `{affordance}` is claimed for a human but no signifier signifies it; \
                 declare a `yaml x0k:signifier` block beside the face that presents it, or \
                 drop `human` from its actors"
            ),
        }
    }
}

impl std::error::Error for DeclarationDefect {}

/// What checking a set of inline declarations against each other found.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct DeclarationReport {
    /// How many inline entities were examined, of every class.
    pub checked: usize,
    /// Faults, in the order the affordances were given.
    pub defects: Vec<DeclarationDefect>,
}

impl DeclarationReport {
    /// True when every human claim has a cue.
    pub fn is_clean(&self) -> bool {
        self.defects.is_empty()
    }
}

/// Check a set of inline declarations. Pure: reads the `claimedFor` facts
/// of each `affordance` and the `signifies` facts of each `signifier`,
/// and resolves nothing beyond the set it was given.
pub fn check_declarations<'a, I>(entities: I) -> DeclarationReport
where
    I: IntoIterator<Item = &'a InlineEntity>,
{
    let entities: Vec<&InlineEntity> = entities.into_iter().collect();

    let signified: BTreeSet<EntityId> = entities
        .iter()
        .filter(|e| e.marker_class == "signifier")
        .flat_map(|e| declared_facts(e))
        .filter(|(predicate, _)| predicate == "signifies")
        .filter_map(|(_, value)| value.strip_prefix("entity:")?.parse().ok())
        .collect();

    let mut report = DeclarationReport {
        checked: entities.len(),
        ..DeclarationReport::default()
    };
    for entity in entities {
        if entity.marker_class != "affordance" {
            continue;
        }
        let claims_human = declared_facts(entity)
            .iter()
            .any(|(p, v)| p == "claimedFor" && v == "entity:x0k:actor/human");
        if claims_human && !signified.contains(&entity.uri) {
            report.defects.push(DeclarationDefect::HumanClaimWithoutSignifier {
                affordance: entity.uri.clone(),
            });
        }
    }

    report
}
```

## What this deliberately does not check

The domain and range a shipped module declares are *carried* by
`DeclaredElsewhere` and not enforced. Enforcing them looks tempting and
is presently wrong: `mentions` is declared with an `x0k:Decision` range,
and decisions in the corpus routinely mention wiki pages, so a range
check would fire on correct documents. Making that check meaningful is
a vocabulary job — widen the range, or split the predicate — and until
it is done, a check that fired would train its readers to ignore it.
The information is in the report; the judgment is not yet the checker's
to make.

The document's own genus is not checked here either, and now for a
better reason than before: it is checked *earlier*, by the same model.
`parse_envelope_in` admits a `type:` naming any class the vocabulary
declares and refuses everything else ([`colophon.md`](colophon.md)), so a
document that reached this module has already had its genus read against
the same set its edges are about to be. Answering the question twice, in
two reports, is the thing this module exists to avoid.

## Tests

The vocabulary questions are asked against whatever model the caller
hands over, and the default model differs between the two builds this
crate lives in. So the fixtures name no predicate: they take one from the
model at runtime. Writing `motivated_by` into a fixture was the first
version of these tests, and it passed in the monorepo and failed in the
published bundle — correctly, which is the point, but a test that
measures the module selection is not measuring the checker.

The last test is the one the model-parameterization is for: a vocabulary
written to a scratch directory, holding a genus and a namespace this
build compiled nothing about, checks a document that uses both — and the
shipped model, asked the same question, calls the same document
undeclared.

<a name="chunk-tests"></a><sub>[`src/envelope_check.rs`](../../crates/x0k-folio/src/envelope_check.rs) · `#tests`</sub>

`````rust {#tests}
#[cfg(test)]
mod tests {
    use super::*;
    use crate::colophon::parse_envelope;

    /// A predicate this build is certain to accept: the first entry of
    /// the compiled Decision-domain slice. Fixtures are built around it
    /// rather than around a named term, so these tests measure the
    /// checker and not the module selection — which is the one thing
    /// that legitimately differs between the two builds this crate
    /// lives in.
    fn shipped() -> OntologyModel {
        OntologyModel::shipped()
    }

    fn shipped_predicate() -> &'static str {
        x0k_ontology::KNOWN_EDGE_PREDICATES
            .first()
            .copied()
            .expect("a build whose vocabulary declares no document edge ships no document module")
    }

    fn envelope(content: &str) -> Colophon {
        parse_envelope(content).expect("fixture parses").0
    }

    /// `edges` is the frontmatter block verbatim, or empty.
    fn doc(id: &str, edges: &str) -> Colophon {
        envelope(&format!(
            "---\nx0k:\n  format: folio/v1\n  id: {id}\n  type: design\n  status: proposed\n{edges}---\nBody.\n"
        ))
    }

    /// One shipped predicate, two targets, in declaration order.
    fn two_edged() -> Colophon {
        let p = shipped_predicate();
        doc(
            "x0k:design/example",
            &format!("  edges:\n    {p}:\n      - x0k:commitment/local-first\n      - x0k:design/other\n"),
        )
    }

    #[test]
    fn camel_form_inverts_the_ontology_spelling_rule() {
        assert_eq!(camel_form("motivated_by"), "motivatedBy");
        assert_eq!(camel_form("bundles"), "bundles");
        assert_eq!(camel_form("presents_signifier"), "presentsSignifier");
    }

    #[test]
    fn generated_slice_members_stand_as_document_edges() {
        for snake in x0k_ontology::KNOWN_EDGE_PREDICATES {
            assert!(
                matches!(
                    predicate_standing(&shipped(), snake),
                    PredicateStanding::DocumentEdge { .. }
                ),
                "`{snake}` is in the generated slice but did not stand as a document edge"
            );
        }
    }

    #[test]
    fn a_term_no_module_declares_is_undeclared() {
        assert_eq!(
            predicate_standing(&shipped(), "definitely_not_a_predicate"),
            PredicateStanding::Undeclared
        );
    }

    #[test]
    fn cites_is_declared_by_core_and_constrained_by_nothing() {
        // The inverse of the assertion this test used to make. `cites` was
        // the publication decision's example of an undeclared term, filed
        // in the wiki vocabulary's own namespace and reachable from no
        // build. It is a `core` term now, so every build has it — `core`
        // is the one module with no imports and every module imports it —
        // and it carries neither a domain nor a range, which is what
        // `DeclaredElsewhere` with two `None`s means.
        assert_eq!(
            predicate_standing(&shipped(), "cites"),
            PredicateStanding::DeclaredElsewhere {
                uri: "x0k:cites".to_string(),
                domain: None,
                range: None,
            }
        );
    }

    #[test]
    fn a_clean_envelope_yields_its_id_and_edges() {
        let report = check_envelope(&shipped(), &two_edged());
        assert!(report.is_clean(), "unexpected defects: {:?}", report.defects);
        assert_eq!(
            report.id.as_ref().map(ToString::to_string).as_deref(),
            Some("x0k:design/example")
        );
        assert_eq!(report.edges.len(), 2);
    }

    #[test]
    fn a_malformed_id_is_a_defect_and_leaves_the_edges_checked() {
        let p = shipped_predicate();
        let report = check_envelope(&shipped(), &doc(
            "not-an-id",
            &format!("  edges:\n    {p}:\n      - x0k:design/other\n"),
        ));
        assert!(report.id.is_none());
        assert!(matches!(
            report.defects.as_slice(),
            [Defect::MalformedId { .. }]
        ));
        assert_eq!(report.edges.len(), 1, "edges are still read");
    }

    #[test]
    fn an_undeclared_predicate_is_a_defect_and_a_bad_target_is_another() {
        let p = shipped_predicate();
        let report = check_envelope(&shipped(), &doc(
            "x0k:design/example",
            &format!(
                "  edges:\n    not_a_predicate:\n      - x0k:wiki/somewhere\n    {p}:\n      - design/missing-scheme\n"
            ),
        ));
        assert!(report
            .defects
            .iter()
            .any(|d| matches!(d, Defect::UndeclaredPredicate { predicate } if predicate == "not_a_predicate")));
        assert!(report
            .defects
            .iter()
            .any(|d| matches!(d, Defect::MalformedTarget { .. })));
    }

    #[test]
    fn a_target_outside_the_set_dangles_and_is_not_a_defect() {
        let a = two_edged();
        let report = check_corpus(&shipped(), [("a.md", &a)]);
        assert_eq!(report.checked, 1);
        assert!(report.is_clean(), "dangling must not be a defect");
        let targets: Vec<String> = report
            .dangling
            .iter()
            .map(|e| e.target.to_string())
            .collect();
        assert_eq!(
            targets,
            vec!["x0k:commitment/local-first", "x0k:design/other"]
        );
    }

    #[test]
    fn a_target_inside_the_set_does_not_dangle() {
        let a = two_edged();
        let b = doc("x0k:design/other", "");
        let report = check_corpus(&shipped(), [("a.md", &a), ("b.md", &b)]);
        assert_eq!(report.checked, 2);
        let targets: Vec<String> = report
            .dangling
            .iter()
            .map(|e| e.target.to_string())
            .collect();
        assert_eq!(targets, vec!["x0k:commitment/local-first"]);
    }

    /// A vocabulary a reader could write: a `mycorp` module in its own
    /// namespace, declaring one genus class and one edge predicate over
    /// it. Written to a scratch directory and loaded, because what is
    /// under test is that a check can be made against files this build
    /// compiled nothing about.
    fn scratch_vocabulary(dir: &std::path::Path) -> OntologyModel {
        const CORE: &str = "\
<https://0k.computer/ontology/core> <http://www.w3.org/1999/02/22-rdf-syntax-ns#type> <http://www.w3.org/2002/07/owl#Ontology> .
";
        const MYCORP: &str = "\
<https://0k.computer/ontology/mycorp> <http://www.w3.org/1999/02/22-rdf-syntax-ns#type> <http://www.w3.org/2002/07/owl#Ontology> .
<https://0k.computer/ontology/mycorp> <http://www.w3.org/2002/07/owl#imports> <https://0k.computer/ontology/core> .
<https://0k.computer/ontology/mycorp> <http://purl.org/vocab/vann/preferredNamespaceUri> \"https://mycorp.example/ontology#\" .
<https://mycorp.example/ontology#Brief> <http://www.w3.org/1999/02/22-rdf-syntax-ns#type> <http://www.w3.org/2002/07/owl#Class> .
<https://mycorp.example/ontology#Brief> <http://www.w3.org/2000/01/rdf-schema#isDefinedBy> <https://0k.computer/ontology/mycorp> .
<https://mycorp.example/ontology#Brief> <http://www.w3.org/2000/01/rdf-schema#label> \"Brief\" .
";
        std::fs::create_dir_all(dir).expect("scratch module directory");
        std::fs::write(dir.join("core.ttl"), CORE).expect("write core");
        std::fs::write(dir.join("mycorp.ttl"), MYCORP).expect("write mycorp");
        OntologyModel::load(dir).expect("the scratch module set loads")
    }

    #[test]
    fn a_document_in_a_loaded_vocabulary_checks_clean_and_fails_the_shipped_one() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let model = scratch_vocabulary(&tmp.path().join("modules"));
        let content = "---\nx0k:\n  format: folio/v1\n  id: mycorp:brief/tender-process\n  \
                       type: brief\n  status: proposed\n---\nBody.\n";

        // The genus arrives from the loaded module, so the envelope parses.
        let (envelope, _) = crate::colophon::parse_envelope_in(&model, content)
            .expect("a genus the loaded vocabulary declares");
        assert_eq!(
            envelope.doc_type,
            crate::colophon::DocType::Declared("brief".to_string())
        );

        // So does the scheme, so the id parses and the check is clean.
        let report = check_envelope(&model, &envelope);
        assert!(report.is_clean(), "unexpected defects: {:?}", report.defects);
        assert_eq!(
            report.id.as_ref().map(ToString::to_string).as_deref(),
            Some("mycorp:brief/tender-process")
        );

        // Against the shipped vocabulary the same document is a defect —
        // which is the check doing its job, not the document being wrong.
        let report = check_envelope(&shipped(), &envelope);
        assert!(
            matches!(report.defects.as_slice(), [Defect::MalformedId { .. }]),
            "expected the shipped vocabulary to refuse `mycorp:`, got {:?}",
            report.defects
        );
    }

    /// Inline declarations as a chapter would carry them: an affordance
    /// under one heading, optionally a signifier under another.
    fn declarations(body: &str) -> Vec<InlineEntity> {
        let classes = ["affordance", "signifier"]
            .into_iter()
            .map(str::to_string)
            .collect();
        crate::inline_entity::extract_from_markdown(body, &classes)
            .into_iter()
            .map(|r| r.expect("fixture declares well-formed entities"))
            .collect()
    }

    const HUMAN_CLAIM: &str = r#"### Read an affordance out of a document

```yaml x0k:affordance
id: x0k:affordance/read_declared_affordances
status: wip
actors: [human, ai_agent]
```
"#;

    #[test]
    fn a_human_claim_with_no_signifier_is_a_defect() {
        let entities = declarations(HUMAN_CLAIM);
        let report = check_declarations(&entities);
        assert_eq!(report.checked, 1);
        match report.defects.as_slice() {
            [DeclarationDefect::HumanClaimWithoutSignifier { affordance }] => {
                assert_eq!(affordance.to_string(), "x0k:affordance/read_declared_affordances");
            }
            other => panic!("expected one HumanClaimWithoutSignifier, got {other:?}"),
        }
    }

    #[test]
    fn a_human_claim_with_a_signifier_is_clean() {
        let body = format!(
            "{HUMAN_CLAIM}
### `extract_from_markdown`

```yaml x0k:signifier
id: x0k:signifier/x0k-folio-extract-from-markdown
edges:
  signifies:
    - x0k:affordance/read_declared_affordances
  presentedOn:
    - x0k:surface/sdk
```
"
        );
        let entities = declarations(&body);
        let report = check_declarations(&entities);
        assert_eq!(report.checked, 2);
        assert!(report.is_clean(), "unexpected defects: {:?}", report.defects);
    }

    #[test]
    fn an_agent_only_claim_needs_no_signifier() {
        let body = HUMAN_CLAIM.replace("actors: [human, ai_agent]", "actors: [ai_agent]");
        let entities = declarations(&body);
        assert!(check_declarations(&entities).is_clean());
    }
}
`````

## Composing the module

<a name="chunk-root"></a><sub>[`src/envelope_check.rs`](../../crates/x0k-folio/src/envelope_check.rs) · `#root` · assembles [module-doc](#chunk-module-doc) · [standing](#chunk-standing) · [vocabulary](#chunk-vocabulary) · [camel-form](#chunk-camel-form) · [defect](#chunk-defect) · [check-envelope](#chunk-check-envelope) · [check-corpus](#chunk-check-corpus) · [check-declarations](#chunk-check-declarations) · [tests](#chunk-tests)</sub>

```rust {#root}
<<module-doc>>

<<standing>>

<<vocabulary>>

<<camel-form>>

<<defect>>

<<check-envelope>>

<<check-corpus>>

<<check-declarations>>

<<tests>>
```

The check is worth having mostly for what it makes visible about a
*publication* rather than about a document. Run it over the bundle this
crate ships in and the defect list is a reading of the module selection:
empty means the vocabulary spans the corpus, and every entry names a
term the selection left behind. Since the vocabulary is a parameter, that
question can now be asked of a selection nobody compiled — the modules a
received bundle carries, or a reader's own — which is what makes the
report about the publication rather than about the binary reading it. That is a question nobody could ask from
outside the monorepo before, and it is the question a contributor
arriving at the public repository is most likely to trip over first.

It already has an answer, and the answer used to be embarrassing. The
bundle ships `core`, `document` and `software`, and their union declared
eighteen document edges while leaving out `motivated_by` — carried by 281
documents in the corpus — and `published_by` and `published_for`, which a
publication manifest carries *by construction*, including the manifest
that produces this very bundle. All three had their subject class in
`document` and sat in `work` and `product` anyway, exiled by a range
naming a class from elsewhere. So the bundle shipped a checker that
reported its own manifest as undeclared.

They are `document`'s now, and the union declares twenty-one. What moved
them was not a narrowing: a range that reaches across a module boundary
is a shape, not part of the term (`x0k:architecture/vocabulary-shapes`
§3), so the constraint is still in the vocabulary — in
`ontology/shapes/document.ttl`, which ships with `document` — and only
the term's placement changed.

`cites` was the one left, and it was a different gap: not exiled by a
range, not in the module system at all. It is `core`'s now, domain-free
and range-free, because a term about any concept has no lower module to
sit in — and the thirty-eight documents in this bundle that cite
something include the literate chapters you are reading.
