# Checking a document against what shipped with it

```turtle folio:document
implementation:folio\/checking a x0k:Implementation ;
    x0k:status "draft" ;
    x0k:summary "Reading a header against a vocabulary the caller names — and keeping a missing term, which is a packaging defect, apart from a missing target, which is the boundary working." ;
    x0k:concerns "folio", "ontology", "publishing", "validation", "vocabulary" ;
    x0k:cites architecture:publication-projection,
        architecture:ontology-modules,
        implementation:folio\/colophon,
        implementation:folio\/identity,
        implementation:ontology\/load ;
    x0k:implements design:publish-a-region-as-a-repository ;
    folio:tangleCrate "crates/x0k-folio" ;
    folio:tangleRoot "src/envelope_check.rs" .
```

A publication of this crate ships three things that ought to be able to
meet: the format, the vocabulary a header's class and edges are terms of
(an [RDF and OWL](x0k:wiki/rdf-and-owl) ontology, shipped as modules), and
the documents themselves. Until this module they could not. The parser
read the edges into a `BTreeMap<String, Vec<String>>` and its own comment
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
Point the same checker at a document whose header uses a predicate the
shipped vocabulary never defines, and something is
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
which no consumer of a header's `cites` edge ever resolved to — so a
bundle shipping `core`, `document` and `software` did not define it, and
the checker over that bundle said so about thirty-eight of the documents
it was shipping. The answer was neither of the two the report suggests:
the term was not wiki-scoped and had no module to wait for. It is now
`x0k:cites` in `core`, with no domain and no range, and what the report
still names in that shape is a genuine module-selection question rather
than a term filed in the wrong house.

<a name="chunk-module-doc"></a><sub>[`src/envelope_check.rs`](../../crates/x0k-folio/src/envelope_check.rs) · `#module-doc`</sub>

```rust {#module-doc}
//! Reading a folio header against a vocabulary the caller names.
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
//!
//! A fourth is asked of those same inside-the-document declarations
//! against the vocabulary the set itself carries — **is this a class
//! something declares, a predicate something declares, a target the
//! predicate's range admits?** `check_instances` (with the `document-vocabulary` feature) answers it for every
//! namespace the collection loads, ours and the reader's alike, and
//! hands back the extended model so the envelope pass can resolve a
//! prefix the collection defined for itself.
//!
//! The same pass reads a block's other keys — its **fields** — against
//! that vocabulary: a key naming no declared property, on an instance of
//! a class the vocabulary describes, and a value contradicting its
//! property's XSD range, on any instance (`check_literal_fields`).

use std::collections::{BTreeMap, BTreeSet};

use x0k_ontology::concept_facts::{OntologyModel, OntologyValue, RDFS_SUBCLASS_OF, X0K_NS};

use crate::colophon::Colophon;
use crate::entity_id::EntityId;
use crate::inline_entity::{declared_facts, InlineEntity};
```

## Standing: what a module has to say about a predicate

A model answers two different questions about a predicate, and the
difference between them is the difference between two `no`s. The
*Decision-domain slice* is the predicates whose subject reaches
`x0k:Decision`, which is what a decision document's header normally draws
its edges from. The object-property table covers every object
property the modules declare, whatever its subject.

A predicate can therefore be outside the slice and still perfectly real:
`x0k:childOf` has an `x0k:Intent` domain, and an intent's header is right
to use it. Calling that undeclared would be wrong. So standing has three
values, not two.

Neither question is about a namespace. A Backstage maintainer who wrote the
module the guide invites, `bs:supersededBy` with an `rdfs:domain` of
`x0k:Decision`, was once told their own predicate "is declared by no
ontology module" (2026-09-22), because the check could only ask about
`x0k:` terms. A header states its predicates as terms, prefix and all, so
the question is asked of the term as written: `bs:supersededBy` is looked up
as `bs:supersededBy`. Whether the term is a decision's edge is then the test
it always was, asked of whatever module declared it: is its domain
`x0k:Decision`, or a class declared a subclass of it?

That subject set is folded here rather than asked of the model, and the
reason is the publication rather than the design. `decision_edge_predicates`
computes the same set and then keys its table by `x0k:` local name, which a
reader's term does not have — so the natural repair is a method on
`OntologyModel`. But `x0k-folio` is packaged against the *released*
`x0k-ontology`, and `cargo package` verifies the tarball against the
registry: a method added upstream in the same change does not exist for
that build until a release goes out. Six lines over `facts()` cost nothing
and keep the gate honest. When the two crates next release together this
belongs upstream.

<a name="chunk-standing"></a><sub>[`src/envelope_check.rs`](../../crates/x0k-folio/src/envelope_check.rs) · `#standing`</sub>

```rust {#standing}
/// What a vocabulary has to say about an edge predicate, in its compact
/// spelling: `x0k:motivatedBy`, `bs:supersededBy`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PredicateStanding {
    /// Declared with `x0k:Decision`, or a subclass of it, in its domain —
    /// the slice a decision document's header draws its edges from,
    /// whichever module declares the term.
    DocumentEdge { uri: String },
    /// Declared, but for some other subject (`x0k:childOf` is an intent's
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

/// Ask a vocabulary about one predicate, in its compact spelling.
pub fn predicate_standing(model: &OntologyModel, spelled: &str) -> PredicateStanding {
    Vocabulary::of(model).standing(spelled)
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
    /// The Decision-domain slice the model computes for `x0k:`, compact.
    /// Every other namespace is answered from `properties` and
    /// `decision_domains` instead, which is the same test spelled out.
    document_edges: BTreeSet<String>,
    /// Compact URI → `(domain, range)` for every declared object property.
    properties: BTreeMap<String, (Option<String>, Option<String>)>,
    /// `x0k:Decision` and its subclasses, compacted — the domains that
    /// make a declared property a document's edge.
    decision_domains: BTreeSet<String>,
    /// The namespace prefixes an id may carry.
    schemes: BTreeSet<String>,
}

impl Vocabulary {
    fn of(model: &OntologyModel) -> Self {
        Self {
            document_edges: model
                .decision_edge_predicates()
                .into_iter()
                .map(|(_, camel)| format!("x0k:{camel}"))
                .collect(),
            properties: model
                .object_properties()
                .into_iter()
                .map(|property| (property.uri, (property.domain, property.range)))
                .collect(),
            decision_domains: decision_domains(model),
            schemes: model.schemes(),
        }
    }

    fn standing(&self, spelled: &str) -> PredicateStanding {
        if self.document_edges.contains(spelled) {
            return PredicateStanding::DocumentEdge { uri: spelled.to_string() };
        }
        let uri = spelled.to_string();
        match self.properties.get(&uri) {
            Some((domain, range)) => {
                let subject_is_a_decision = domain
                    .as_deref()
                    .is_some_and(|domain| self.decision_domains.contains(domain));
                if subject_is_a_decision {
                    PredicateStanding::DocumentEdge { uri }
                } else {
                    PredicateStanding::DeclaredElsewhere {
                        uri,
                        domain: domain.clone(),
                        range: range.clone(),
                    }
                }
            }
            None => PredicateStanding::Undeclared,
        }
    }

    /// Parse an id, licensing whatever namespaces this vocabulary declares.
    fn id(&self, raw: &str) -> Result<EntityId, crate::entity_id::EntityIdError> {
        EntityId::parse_with_schemes(raw, &self.schemes)
    }
}

/// `x0k:Decision` and every class declared a direct subclass of it, in the
/// compact spelling `object_properties` reports a domain in. Direct
/// subclasses only, which is the definition the model's own edge table has
/// always used.
fn decision_domains(model: &OntologyModel) -> BTreeSet<String> {
    let decision = OntologyValue::Entity(format!("{X0K_NS}Decision"));
    let mut out = BTreeSet::from(["x0k:Decision".to_string()]);
    for fact in model.facts() {
        if fact.predicate == RDFS_SUBCLASS_OF && fact.value == decision {
            out.insert(model.compact(&fact.entity).unwrap_or_else(|| fact.entity.clone()));
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
    /// The header's subject is not `x0k:<class>/<slug>`.
    MalformedId { value: String, reason: String },
    /// An edge's target is not `x0k:<class>/<slug>`.
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
                write!(f, "the header's subject is not a well-formed id: {reason} (`{value}`)")
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
header reaches for it and gets back a report: the document's id, its
well-formed edges, and the defects, in the order found. That is all the
cue there is — a reader of this library learns the check is here from
the function's name and its rustdoc — so it is declared as the cue
below, beside the function it describes rather than on the affordance
it makes reachable (`x0k:design/publish-a-region-as-a-repository`, "a
face declares its signifier where the face lives"):

<a name="folio-instance-68747470733a2f2f306b2e636f6d70757465722f6f6e746f6c6f6779237369676e69666965722f78306b2d666f6c696f2d636865636b2d656e76656c6f7065-1"></a><sub data-instance-iri="https://0k.computer/ontology#signifier/x0k-folio-check-envelope" data-concept-iri="https://0k.computer/ontology#Signifier" data-source-document="corpora/x0k/implementation/folio/checking.md"><strong>Signifier</strong> · check_envelope: one document · <code>https://0k.computer/ontology#signifier/x0k-folio-check-envelope</code> · <a href="#folio-source-68747470733a2f2f306b2e636f6d70757465722f6f6e746f6c6f6779237369676e69666965722f78306b2d666f6c696f2d636865636b2d656e76656c6f7065-1">source declaration</a></sub><a name="folio-source-68747470733a2f2f306b2e636f6d70757465722f6f6e746f6c6f6779237369676e69666965722f78306b2d666f6c696f2d636865636b2d656e76656c6f7065-1"></a>

```turtle folio:graph
signifier:x0k-folio-check-envelope a x0k:Signifier ;
    x0k:cue "check_envelope" ;
    x0k:signifies affordance:check_a_document_against_shipped_vocabulary ;
    x0k:presentedOn surface:sdk .
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
    /// predicate then by statement — the header holds its edges in a
    /// `BTreeMap`, so the order is the vocabulary's, not the author's.
    /// Predicates are kept in their compact spelling.
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

/// Check one parsed header against `model`. Pure: no filesystem, no
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
the descriptor, and the published library is one, so `x0k:claimedFor
actor:ai_agent` alone is clean by construction. The check is only ever
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

It is a join, and a join needs both sides present to mean anything.
Affordances are declared beside the decision that shapes them;
signifiers are declared beside the face that presents them, one
directory over. A reader who scans only the decisions — which the
integration guide invites, saying "check the folder, any folder" — hands
this check every affordance and no signifier, and every human claim
comes back unsignified. That is not seven broken promises, it is a
question asked of a set that does not hold the answer, and on a pristine
clone it printed seven red errors at the reader's first command.

So the absence of a signifier is only evidence when the set declares at
least one. With none, the finding is a note: the check says what it
looked for and that nothing here could have answered. The predicate is
about the set rather than about the affordance, which is why it is
computed once and applied to all of them. Its limit is honest and worth
saying: a set holding *some* signifiers and not the ones these
affordances need still reports defects, because at that point silence is
evidence again. And a reader who knows the set is the whole collection
says `--closed`, under which every note is a defect.

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
    /// The vocabulary the set carries does not assemble: a
    /// definition block that will not parse, two blocks
    /// binding one prefix to different namespaces, a term with no
    /// owning module. Nothing downstream can be checked against a
    /// vocabulary that does not exist, so this one defect stands for
    /// every instance the set declares.
    Vocabulary { reason: String },
    /// An instance block does not hold against the loaded
    /// vocabulary: an unknown class, a predicate no module
    /// declares, a target whose type the predicate's range refuses. The
    /// reason names its document and line.
    Instance { reason: String },
}

impl std::fmt::Display for DeclarationDefect {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::HumanClaimWithoutSignifier { affordance } => write!(
                f,
                "affordance `{affordance}` is claimed for a human but no signifier signifies it; \
                 declare a signifier beside the face that presents it, or \
                 drop its `x0k:claimedFor actor:human`"
            ),
            Self::Vocabulary { reason } => write!(
                f,
                "the vocabulary this set carries does not load, so no declaration in it could \
                 be checked: {reason}"
            ),
            Self::Instance { reason } => write!(f, "declaration does not hold: {reason}"),
        }
    }
}

impl std::error::Error for DeclarationDefect {}

/// A declaration question the set was not equipped to answer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DeclarationNote {
    /// An affordance claimed for a human, in a set that declares no
    /// signifier at all. Signifiers live beside the faces that present
    /// them, so a set holding none is not the set where signification
    /// lives, and its silence about one is not evidence against the
    /// claim.
    HumanClaimUnsignifiable { affordance: EntityId },
}

impl std::fmt::Display for DeclarationNote {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::HumanClaimUnsignifiable { affordance } => write!(
                f,
                "affordance `{affordance}` is claimed for a human and nothing here signifies \
                 it; this set declares no signifier at all, so it cannot answer — scan the \
                 documents that hold the faces too"
            ),
        }
    }
}

/// A declared relationship whose target is well formed and names no
/// declaration in the set. The instance-level twin of [`DanglingEdge`],
/// and ordinary for the same reason: a collection is a region.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DanglingDeclaration {
    /// The caller's name for the document that declared the instance.
    pub source: String,
    /// The subject instance, in its compact spelling.
    pub subject: String,
    /// The predicate, compacted.
    pub predicate: String,
    /// The target that names nothing here, compacted.
    pub target: String,
}

/// What checking a set of inline declarations against each other found.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct DeclarationReport {
    /// How many declarations were examined, of every class.
    pub checked: usize,
    /// Faults, in the order the affordances were given.
    pub defects: Vec<DeclarationDefect>,
    /// Relationship targets that name no declaration in the set.
    pub dangling: Vec<DanglingDeclaration>,
    /// Questions this set could not answer, in the order asked.
    pub notes: Vec<DeclarationNote>,
}

impl DeclarationReport {
    /// True when every human claim has a cue and every instance holds.
    /// Dangling targets and notes do not affect this.
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
        .filter(|e| e.class == "signifier")
        .flat_map(|e| declared_facts(e))
        .filter(|(predicate, _)| predicate == "x0k:signifies")
        .filter_map(|(_, value)| value.strip_prefix("entity:")?.parse().ok())
        .collect();

    // Whether this set can speak to signification at all, asked once of
    // the set rather than once per affordance: with no signifier
    // anywhere in it, every answer it gives is the same non-answer.
    let signifies_anything = entities.iter().any(|e| e.class == "signifier");

    let mut report = DeclarationReport {
        checked: entities.len(),
        ..DeclarationReport::default()
    };
    for entity in entities {
        if entity.class != "affordance" {
            continue;
        }
        let claims_human = declared_facts(entity)
            .iter()
            .any(|(p, v)| p == "x0k:claimedFor" && v == "entity:x0k:actor/human");
        if !claims_human || signified.contains(&entity.uri) {
            continue;
        }
        if signifies_anything {
            report.defects.push(DeclarationDefect::HumanClaimWithoutSignifier {
                affordance: entity.uri.clone(),
            });
        } else {
            report.notes.push(DeclarationNote::HumanClaimUnsignifiable {
                affordance: entity.uri.clone(),
            });
        }
    }

    report
}
```

## Instances: a collection's own words, checked like ours

An affordance and a signifier are `x0k:` instances, and for a long time
they were the only instances anyone checked. A collection that brings
its own vocabulary — `paper:Paper` defined in a `turtle folio:graph`
block beside the papers that instantiate it — declared instances that
nothing read. Three maintainers reached that hole independently in one
afternoon (2026-09-22): each took the shipped `examples/papers`
collection, broke it, and watched `check` pass at exit 0 saying **`0
declaration(s) checked`**. The count was true, which was the worst part
of it: the blocks were not passing, they were not being read.

The validator already existed, one crate over. `x0k-folio-cli ingest`
assembles a collection's vocabulary and checks every instance against it
([`document-vocabulary.md`](document-vocabulary.md)), and catches all
three breakages by name. So this is not a new check; it is the check
`ingest` runs, reached from the gate a reader is told to put in CI. The
guide put `check` in CI and never said that `check` was blind to the
reader's own vocabulary while enforcing ours strictly, which is how a
green gate came to mean nothing for exactly the collections the custom
vocabulary story is for.

Two things follow from reading the vocabulary the way `ingest` does.
The model this returns is the caller's base *extended* by the blocks the
set carries, and it is the model the headers should then be read
against — `paper:paper/alpha` was refused as an undeclared prefix with
`vocabulary.md` sitting in the same directory, because the header pass
never saw the block. And the failure grain is per document: one
unreadable block should name its own file rather than stopping the set,
so instances are collected a document at a time, and the whole-set pass
runs only once every document has parsed — a range check needs the
target's declaration, and a duplicate id needs both.

A target that resolves to no declaration here is not a defect. It is the
same region boundary the header pass reports as a dangling edge, one
level down, so it is reported the same way and counted the same way:
noted, never fatal. What makes it fatal is `--closed`, which is a
different question about the collection and not this function's to
answer.

<a name="chunk-check-instances"></a><sub>[`src/envelope_check.rs`](../../crates/x0k-folio/src/envelope_check.rs) · `#check-instances`</sub>

```rust {#check-instances}
/// The vocabulary a set of documents carries, and what its typed
/// instance declarations came to.
#[cfg(feature = "document-vocabulary")]
pub struct InstanceCheck {
    /// The caller's base vocabulary, extended by every
    /// vocabulary block the set carries. Read headers
    /// against this, not against the base: a collection that defines a
    /// namespace may use it in an id.
    pub model: OntologyModel,
    /// Instances checked, faults found, and targets that left the set.
    pub report: DeclarationReport,
}

/// Assemble the vocabulary `documents` carry on top of `base`, then check
/// every typed instance they declare against it.
///
/// Each document's `body` is whatever text the caller wants byte offsets
/// and line numbers to be relative to; passing the whole file, header
/// included, is what makes a reported line openable in an editor.
#[cfg(feature = "document-vocabulary")]
pub fn check_instances(
    base: &OntologyModel,
    documents: &[crate::document_vocabulary::DocumentSource<'_>],
) -> InstanceCheck {
    use crate::document_vocabulary::{collect_instances, load_definitions, validate_relationships};

    let mut report = DeclarationReport::default();
    let vocabulary = match load_definitions(documents, base) {
        Ok(vocabulary) => vocabulary,
        Err(e) => {
            report.defects.push(DeclarationDefect::Vocabulary { reason: e.to_string() });
            return InstanceCheck { model: OntologyModel::new(base.facts().to_vec()), report };
        }
    };

    // A document at a time, so a block that will not parse names the file
    // it is in instead of taking the collection down with it.
    let mut instances = Vec::new();
    let mut every_document_parsed = true;
    for document in documents {
        match collect_instances(std::slice::from_ref(document), &vocabulary.model) {
            Ok(found) => instances.extend(found),
            Err(e) => {
                every_document_parsed = false;
                report.defects.push(DeclarationDefect::Instance { reason: e.to_string() });
            }
        }
    }
    report.checked = instances.len();

    // Every field of every block that parsed, against the vocabulary the
    // set assembled: a key it does not name, a value its range refuses
    // ("Literal fields", below). Asked before the whole-set pass, which a
    // block that failed to parse elsewhere would otherwise cut short.
    report.defects.extend(check_literal_fields(&vocabulary.model, &instances));

    // The questions one document cannot answer: an id declared twice in
    // two files, and a range constraint whose target lives elsewhere.
    let mut first_declared: BTreeMap<&str, &str> = BTreeMap::new();
    for instance in &instances {
        let document = instance.source.document.as_str();
        if let Some(prior) = first_declared.insert(instance.iri.as_str(), document) {
            report.defects.push(DeclarationDefect::Instance {
                reason: format!(
                    "duplicate instance {} at {}, already declared at {prior}",
                    instance.iri, instance.source
                ),
            });
        }
    }
    if !every_document_parsed {
        return InstanceCheck { model: vocabulary.model, report };
    }
    match validate_relationships(&vocabulary.model, &instances) {
        Ok(relationships) => {
            let spell = |iri: &str| vocabulary.model.compact(iri).unwrap_or_else(|| iri.to_string());
            for relationship in relationships {
                if first_declared.contains_key(relationship.object.as_str()) {
                    continue;
                }
                report.dangling.push(DanglingDeclaration {
                    source: relationship.source.document.clone(),
                    subject: spell(&relationship.subject),
                    predicate: spell(&relationship.predicate),
                    target: spell(&relationship.object),
                });
            }
        }
        Err(e) => report.defects.push(DeclarationDefect::Instance { reason: e.to_string() }),
    }

    InstanceCheck { model: vocabulary.model, report }
}
```

## Literal fields: a term the vocabulary names, a value its datatype admits

The instance pass read three things of a block — its class, its subject,
and its relationships — and let every other statement through. A Backstage
maintainer found the hole in their second round against the release
candidate and found it still open in their third (2026-09-23): both of
these, dropped into the shipped papers example, passed `check` at exit 0.

```turtle
paper:paper\/alpha paper:revieweddd true ;     # a property no module declares
    paper:reviewed "not-a-boolean" .           # a string where the example means a boolean
```

A misspelled term is the single most common typo in a block, and it was the
one shape still unchecked. The example did not help: `reviewed` and `pages`
sat in both papers and no module declared either, so there was no vocabulary
for the check to read them against — and the collector refused a
document-carried `owl:DatatypeProperty` outright
([`document-vocabulary.md`](document-vocabulary.md) admits them now).

### The rule

Every statement of an instance block whose object is a literal is a
**field**, except the placement demands (`requiresResources`, a host's to
interpret), and a field answers to the loaded vocabulary twice.

1. **A field names a declared property — on an instance of a class the
   vocabulary describes.** A field whose predicate no loaded module declares
   is refused, and the refusal names the term and the nearest declared
   property when one is close. A class is *described* when the vocabulary
   declares at least one `owl:DatatypeProperty` whose `rdfs:domain` is that
   class exactly.
2. **A field's value does not contradict the property's declared
   datatype.** A field whose predicate has an XSD datatype in its
   `rdfs:range`, and whose literal is not a value of that datatype, is
   refused, and the refusal names the property, the value and the datatype
   it expected. This holds on every instance, described class or not.

A property with no declared range is not refused, whatever the value. A
statement whose object is an IRI is a relationship, checked above, not a
field. A range naming an XSD datatype outside the three below is not
interpreted, and not refused.

The predicate a field is checked under is the predicate the block states,
which is the predicate `x0k-folio-cli ingest` writes the fact under
([`document-source.md`](document-source.md)): what `check` reads and what
`ingest` stores are one term. A property is *declared* when the loaded
vocabulary types it `owl:DatatypeProperty`, `owl:ObjectProperty` or
`owl:AnnotationProperty`, in whichever module.

**Why a class has to be described before its field names are judged.**
Because the shipped vocabulary is silent about most of its own classes'
fields, and silence is not a claim. `software` declares `Signifier` and no
literal property of it at all, while every signifier in this bundle carries
an `x0k:cue`. Judging every field would have the check refuse the chapter
that specifies it, over a term only the vocabulary can add. So the rule
reads the vocabulary for where it has *started* to speak about a class's
literals: once one datatype property names the class as its domain, the
class's fields are a closed list and a term outside it is a typo or an
omission. The papers example declares `reviewed` and `pages` over
`paper:Paper`, which is what makes `revieweddd` refusable there. Exact
class, as the collector reads membership: no subclass entailment.

**The nearest declared property** is chosen by edit distance: the
Levenshtein distance, in characters, between the local name as written and
each declared property's local name in the same namespace. The nearest is
named when it is at most two edits away and fewer edits than the name has
characters; a tie goes to the alphabetically first.

### What "contradicts" means, datatype by datatype

The shipped modules name two XSD datatypes in a range, and no other —
`xsd:string` (`bodyFormat`, `concerns`, `status`, `summary` and the
provenance terms in `document`, `specStatus` in `product`, `level` in
`work`) and `xsd:integer` (`folio/sourceStart`, `folio/sourceEnd`). The
papers example adds a third, `xsd:boolean`, for `reviewed`. Those three are
what this check interprets.

The value judged is the literal's datatype as the block wrote it. RDF would
accept `"1"^^xsd:boolean`, but `ingest` stores a literal by its datatype — a
quoted `"12"` as text, a bare `12` as a signed integer, a bare `true` as a
boolean — so a literal of the wrong datatype becomes a fact of the wrong
type, and that is the contradiction worth refusing.

| Range | Conforms | Contradicts |
|---|---|---|
| `xsd:string` | a string literal | a boolean, a number, a JSON literal, any other datatype |
| `xsd:integer` | a bare integer (`12`, `-3`) | a string — `"12"` included —, a decimal or double (`12.5`, `12.0`, `1e3`), a boolean |
| `xsd:boolean` | a bare `true` or `false` | a string — `"true"` included —, a number, `0` and `1` included |

### What the refusals say

Each is a `DeclarationDefect::Instance`, one per field, at the line of the
block's opening fence:

```text
declaration does not hold: `paper:paper/alpha` states `paper:revieweddd`, which names no property the loaded vocabulary declares; the nearest declared property is `paper:reviewed` at alpha.md:11
declaration does not hold: `paper:paper/alpha` gives `paper:reviewed` the string "not-a-boolean", which contradicts its declared datatype `xsd:boolean`: a boolean is a bare `true` or `false` at alpha.md:11
```

`ingest` does not refuse either; `check` is the gate a reader puts in CI, and
a document `check` accepts is still a document `ingest` projects.

### The code

Three things are folded from the model once, as the header pass folds its
tables: which IRIs are declared properties, the ranges each carries, and
which classes are described. They are read off `facts()` rather than asked
of an `OntologyModel` method, for the reason `decision_domains` gives above —
this crate is packaged against the released `x0k-ontology`.

<a name="chunk-check-literals"></a><sub>[`src/envelope_check.rs`](../../crates/x0k-folio/src/envelope_check.rs) · `#check-literals`</sub>

```rust {#check-literals}
#[cfg(feature = "document-vocabulary")]
const XSD_NS: &str = "http://www.w3.org/2001/XMLSchema#";

/// The XSD datatypes a range can name that this check interprets: the two
/// the shipped modules use and the one the shipped papers example adds.
#[cfg(feature = "document-vocabulary")]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Datatype {
    String,
    Integer,
    Boolean,
}

#[cfg(feature = "document-vocabulary")]
impl Datatype {
    /// The datatype a range IRI names, or `None` for one not interpreted.
    fn named(iri: &str) -> Option<Self> {
        match iri.strip_prefix(XSD_NS)? {
            "string" => Some(Self::String),
            "integer" => Some(Self::Integer),
            "boolean" => Some(Self::Boolean),
            _ => None,
        }
    }

    fn spelled(self) -> &'static str {
        match self {
            Self::String => "xsd:string",
            Self::Integer => "xsd:integer",
            Self::Boolean => "xsd:boolean",
        }
    }

    /// What a conforming value looks like, said to the person who wrote
    /// the wrong one.
    fn hint(self) -> &'static str {
        match self {
            Self::String => "text is a quoted string",
            Self::Integer => "an integer is written bare, with no fraction or exponent",
            Self::Boolean => "a boolean is a bare `true` or `false`",
        }
    }

    /// Does one literal belong to this datatype? Judged by the datatype the
    /// block wrote, which is the type `ingest` stores.
    fn admits(self, literal: &crate::colophon::Literal) -> bool {
        literal.datatype == format!("{XSD_NS}{}", &self.spelled()[4..])
    }
}

/// One literal, the way a refusal names it.
#[cfg(feature = "document-vocabulary")]
fn describe_value(literal: &crate::colophon::Literal) -> String {
    match literal.datatype.strip_prefix(XSD_NS) {
        Some("string") => format!("the string {:?}", literal.value),
        Some("boolean") => format!("the boolean `{}`", literal.value),
        Some("integer") => format!("the integer `{}`", literal.value),
        Some("decimal") | Some("double") => format!("the number `{}`", literal.value),
        _ if literal.datatype == crate::colophon::RDF_JSON => "a JSON value".to_string(),
        _ => format!("`{}` typed `{}`", literal.value, literal.datatype),
    }
}

/// The folds the literal rule reads, taken once from a model.
#[cfg(feature = "document-vocabulary")]
struct FieldVocabulary {
    /// Every IRI typed as a datatype, object or annotation property.
    properties: BTreeSet<String>,
    /// Property IRI → every `rdfs:range` value it declares.
    ranges: BTreeMap<String, Vec<String>>,
    /// Classes some datatype property names as its exact domain.
    described: BTreeSet<String>,
}

#[cfg(feature = "document-vocabulary")]
impl FieldVocabulary {
    fn of(model: &OntologyModel) -> Self {
        use x0k_ontology::concept_facts::{
            OWL_ANNOTATION_PROPERTY, OWL_DATATYPE_PROPERTY, OWL_OBJECT_PROPERTY, RDFS_DOMAIN,
            RDFS_RANGE, RDF_TYPE,
        };
        let mut properties = BTreeSet::new();
        let mut datatype_properties = BTreeSet::new();
        for fact in model.facts() {
            if fact.predicate != RDF_TYPE {
                continue;
            }
            if let OntologyValue::Entity(kind) = &fact.value {
                if [OWL_DATATYPE_PROPERTY, OWL_OBJECT_PROPERTY, OWL_ANNOTATION_PROPERTY].contains(&kind.as_str()) {
                    properties.insert(fact.entity.clone());
                }
                if kind == OWL_DATATYPE_PROPERTY {
                    datatype_properties.insert(fact.entity.clone());
                }
            }
        }
        let mut ranges: BTreeMap<String, Vec<String>> = BTreeMap::new();
        let mut described = BTreeSet::new();
        for fact in model.facts() {
            let OntologyValue::Entity(value) = &fact.value else { continue };
            if fact.predicate == RDFS_RANGE && properties.contains(&fact.entity) {
                ranges.entry(fact.entity.clone()).or_default().push(value.clone());
            }
            if fact.predicate == RDFS_DOMAIN && datatype_properties.contains(&fact.entity) {
                described.insert(value.clone());
            }
        }
        Self { properties, ranges, described }
    }

    /// The declared property nearest `predicate` in its own namespace: at
    /// most two edits between local names, fewer than the name is long,
    /// ties to the alphabetically first.
    fn nearest(&self, model: &OntologyModel, predicate: &str) -> Option<String> {
        let local = predicate.rsplit(['#', '/']).next().unwrap_or(predicate);
        let namespace = &predicate[..predicate.len() - local.len()];
        let length = local.chars().count();
        self.properties
            .iter()
            .filter_map(|iri| {
                let candidate = iri.strip_prefix(namespace)?;
                let distance = edit_distance(local, candidate);
                (distance <= 2 && distance < length).then_some((distance, candidate, iri))
            })
            .min_by(|a, b| (a.0, a.1).cmp(&(b.0, b.1)))
            .map(|(_, _, iri)| model.compact(iri).unwrap_or_else(|| iri.clone()))
    }
}

/// Levenshtein distance in characters.
#[cfg(feature = "document-vocabulary")]
fn edit_distance(a: &str, b: &str) -> usize {
    let b: Vec<char> = b.chars().collect();
    let mut previous: Vec<usize> = (0..=b.len()).collect();
    for (i, left) in a.chars().enumerate() {
        let mut current = vec![i + 1; b.len() + 1];
        for (j, right) in b.iter().enumerate() {
            let substitution = previous[j] + usize::from(left != *right);
            current[j + 1] = substitution.min(previous[j + 1] + 1).min(current[j] + 1);
        }
        previous = current;
    }
    previous[b.len()]
}

/// Read every field of every instance against the vocabulary: a predicate
/// no module declares, on an instance of a described class, and a literal
/// its property's XSD range contradicts, on any instance. One defect per
/// field, each at its block's line.
#[cfg(feature = "document-vocabulary")]
pub fn check_literal_fields(
    model: &OntologyModel,
    instances: &[crate::document_vocabulary::DeclaredInstance],
) -> Vec<DeclarationDefect> {
    use crate::inline_entity::Object;
    let vocabulary = FieldVocabulary::of(model);
    let spell = |iri: &str| model.compact(iri).unwrap_or_else(|| iri.to_string());
    let mut defects = Vec::new();
    for instance in instances {
        let described = vocabulary.described.contains(&instance.concept);
        let subject = spell(&instance.iri);
        let mut reported: BTreeSet<&str> = BTreeSet::new();
        for (property, object) in &instance.entity.statements {
            let Object::Literal(literal) = object else { continue };
            if property.ends_with("requiresResources") {
                continue;
            }
            if !vocabulary.properties.contains(property) {
                if described && reported.insert(property.as_str()) {
                    let nearest = match vocabulary.nearest(model, property) {
                        Some(term) => format!("; the nearest declared property is `{term}`"),
                        None => String::new(),
                    };
                    defects.push(DeclarationDefect::Instance {
                        reason: format!(
                            "`{subject}` states `{}`, which names no property the loaded \
                             vocabulary declares{nearest} at {}",
                            spell(property),
                            instance.source
                        ),
                    });
                }
                continue;
            }
            let datatypes: Vec<Datatype> = vocabulary
                .ranges
                .get(property)
                .into_iter()
                .flatten()
                .filter_map(|range| Datatype::named(range))
                .collect();
            for datatype in datatypes {
                if datatype.admits(literal) {
                    continue;
                }
                defects.push(DeclarationDefect::Instance {
                    reason: format!(
                        "`{subject}` gives `{}` {}, which contradicts its declared datatype \
                         `{}`: {} at {}",
                        spell(property),
                        describe_value(literal),
                        datatype.spelled(),
                        datatype.hint(),
                        instance.source
                    ),
                });
            }
        }
    }
    defects
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

The same holds one level down. A field's property is read for its range
and not for its domain: `x0k:status` has an `x0k:Decision` domain, and
nearly every affordance in the corpus states one, so a domain check on
fields would fire on correct blocks for the reason a range check on
`mentions` would. The literal rule above asks what the vocabulary has
said about a *value*, which it has said precisely; what it has said about
*subjects* is the vocabulary job named here, not yet done.

The document's own genus is not checked here either, and now for a
better reason than before: it is checked *earlier*, by the same model.
`parse_envelope_in` admits a class the vocabulary declares and refuses
everything else ([`colophon.md`](colophon.md)), so a
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

    fn shipped_predicate() -> String {
        let (_, camel) = shipped()
            .decision_edge_predicates()
            .into_iter()
            .next()
            .expect("a build whose vocabulary declares no document edge ships no document module");
        format!("x0k:{camel}")
    }

    fn envelope(content: &str) -> Colophon {
        parse_envelope(content).expect("fixture parses").0
    }

    /// A design header: `subject` and `statements` as Turtle.
    fn header(subject: &str, statements: &str) -> String {
        format!("```turtle folio:document\n{subject} a x0k:Design ;\n    x0k:status \"proposed\"{statements} .\n```\nBody.\n")
    }

    fn doc(subject: &str, statements: &str) -> Colophon {
        envelope(&header(subject, statements))
    }

    /// One shipped predicate, two targets, in statement order.
    fn two_edged() -> Colophon {
        let p = shipped_predicate();
        doc("design:example", &format!(" ;\n    {p} commitment:local-first, design:other"))
    }

    #[test]
    fn generated_slice_members_stand_as_document_edges() {
        for (_, camel) in shipped().decision_edge_predicates() {
            let term = format!("x0k:{camel}");
            assert!(
                matches!(predicate_standing(&shipped(), &term), PredicateStanding::DocumentEdge { .. }),
                "`{term}` is in the generated slice but did not stand as a document edge"
            );
        }
    }

    #[test]
    fn a_term_no_module_declares_is_undeclared() {
        assert_eq!(
            predicate_standing(&shipped(), "x0k:definitelyNotAPredicate"),
            PredicateStanding::Undeclared
        );
    }

    #[test]
    fn cites_is_declared_by_core_and_constrained_by_nothing() {
        // `cites` was the publication decision's example of an undeclared
        // term. It is a `core` term now, so every build has it, and it
        // carries neither a domain nor a range.
        assert_eq!(
            predicate_standing(&shipped(), "x0k:cites"),
            PredicateStanding::DeclaredElsewhere {
                uri: "x0k:cites".to_string(),
                domain: None,
                range: None,
            }
        );
    }

    /// The Backstage maintainer persona's blocker (2026-09-22): they wrote
    /// the module the guide invites, declaring `supersededBy` over
    /// `x0k:Decision`. The predicate was refused as declared by no module.
    #[test]
    fn a_readers_own_predicate_over_a_decision_stands_as_a_document_edge() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let model = scratch_vocabulary(&tmp.path().join("modules"));
        assert_eq!(
            predicate_standing(&model, "mycorp:supersededBy"),
            PredicateStanding::DocumentEdge { uri: "mycorp:supersededBy".to_string() }
        );
        assert_eq!(predicate_standing(&shipped(), "mycorp:supersededBy"), PredicateStanding::Undeclared);
        assert_eq!(predicate_standing(&model, "mycorp:shreds"), PredicateStanding::Undeclared);
    }

    #[test]
    fn a_header_edge_in_a_readers_namespace_checks_clean() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let model = scratch_vocabulary(&tmp.path().join("modules"));
        let read = |statements: &str| {
            crate::colophon::parse_envelope_in(&model, &header("mycorp:design\\/adr013", statements))
                .expect("fixture parses")
                .0
        };
        let report = check_envelope(&model, &read(" ;\n    mycorp:supersededBy mycorp:design\\/adr014"));
        assert!(report.is_clean(), "unexpected defects: {:?}", report.defects);
        assert_eq!(
            report.edges.first().map(|(p, t)| (p.as_str(), t.to_string())),
            Some(("mycorp:supersededBy", "mycorp:design/adr014".to_string()))
        );
        match check_envelope(&model, &read(" ;\n    mycorp:shreds mycorp:design\\/adr014")).defects.as_slice() {
            [Defect::UndeclaredPredicate { predicate }] => assert_eq!(predicate, "mycorp:shreds"),
            other => panic!("expected an UndeclaredPredicate, got {other:?}"),
        }
    }

    #[test]
    fn a_clean_header_yields_its_id_and_edges() {
        let report = check_envelope(&shipped(), &two_edged());
        assert!(report.is_clean(), "unexpected defects: {:?}", report.defects);
        assert_eq!(report.id.as_ref().map(ToString::to_string).as_deref(), Some("x0k:design/example"));
        assert_eq!(report.edges.len(), 2);
    }

    #[test]
    fn a_malformed_id_is_a_defect_and_leaves_the_edges_checked() {
        let p = shipped_predicate();
        let report = check_envelope(&shipped(), &doc("<https://example.org/not-an-id>", &format!(" ;\n    {p} design:other")));
        assert!(report.id.is_none());
        assert!(matches!(report.defects.as_slice(), [Defect::MalformedId { .. }]));
        assert_eq!(report.edges.len(), 1, "edges are still read");
    }

    #[test]
    fn an_undeclared_predicate_is_a_defect_and_a_bad_target_is_another() {
        let p = shipped_predicate();
        let report = check_envelope(&shipped(), &doc(
            "design:example",
            &format!(" ;\n    x0k:notAPredicate wiki:somewhere ;\n    {p} <urn:missing-scheme>"),
        ));
        assert!(report.defects.iter().any(|d| matches!(d, Defect::UndeclaredPredicate { predicate } if predicate == "x0k:notAPredicate")));
        assert!(report.defects.iter().any(|d| matches!(d, Defect::MalformedTarget { .. })));
    }

    #[test]
    fn a_target_outside_the_set_dangles_and_is_not_a_defect() {
        let a = two_edged();
        let report = check_corpus(&shipped(), [("a.md", &a)]);
        assert_eq!(report.checked, 1);
        assert!(report.is_clean(), "dangling must not be a defect");
        let targets: Vec<String> = report.dangling.iter().map(|e| e.target.to_string()).collect();
        assert_eq!(targets, vec!["x0k:commitment/local-first", "x0k:design/other"]);
    }

    #[test]
    fn a_target_inside_the_set_does_not_dangle() {
        let a = two_edged();
        let b = doc("design:other", "");
        let report = check_corpus(&shipped(), [("a.md", &a), ("b.md", &b)]);
        assert_eq!(report.checked, 2);
        let targets: Vec<String> = report.dangling.iter().map(|e| e.target.to_string()).collect();
        assert_eq!(targets, vec!["x0k:commitment/local-first"]);
    }

    /// A vocabulary a reader could write: a `mycorp` module in its own
    /// namespace, declaring one genus class and one edge predicate whose
    /// subject is a decision. Written to a scratch directory and loaded,
    /// because what is under test is that a check can be made against
    /// files this build compiled nothing about.
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
<https://mycorp.example/ontology#supersededBy> <http://www.w3.org/1999/02/22-rdf-syntax-ns#type> <http://www.w3.org/2002/07/owl#ObjectProperty> .
<https://mycorp.example/ontology#supersededBy> <http://www.w3.org/2000/01/rdf-schema#domain> <https://0k.computer/ontology#Decision> .
<https://mycorp.example/ontology#supersededBy> <http://www.w3.org/2000/01/rdf-schema#range> <https://0k.computer/ontology#Decision> .
<https://mycorp.example/ontology#supersededBy> <http://www.w3.org/2000/01/rdf-schema#isDefinedBy> <https://0k.computer/ontology/mycorp> .
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
        let content = "```turtle folio:document\nmycorp:brief\\/tender-process a mycorp:Brief ;\n    x0k:status \"proposed\" .\n```\nBody.\n";
        let (envelope, _) = crate::colophon::parse_envelope_in(&model, content)
            .expect("a genus the loaded vocabulary declares");
        assert_eq!(envelope.doc_type, crate::colophon::DocType::Declared("brief".to_string()));
        let report = check_envelope(&model, &envelope);
        assert!(report.is_clean(), "unexpected defects: {:?}", report.defects);
        assert_eq!(report.id.as_ref().map(ToString::to_string).as_deref(), Some("mycorp:brief/tender-process"));
        // Against the shipped vocabulary the same header's id is a defect —
        // the check doing its job, not the document being wrong.
        let report = check_envelope(&shipped(), &envelope);
        assert!(
            matches!(report.defects.as_slice(), [Defect::MalformedId { .. }]),
            "expected the shipped vocabulary to refuse `mycorp:`, got {:?}",
            report.defects
        );
    }

    /// Inline declarations as a chapter would carry them.
    fn declarations(body: &str) -> Vec<InlineEntity> {
        let classes = ["affordance", "signifier"].into_iter().map(str::to_string).collect();
        crate::inline_entity::extract_from_markdown(body, &classes)
            .into_iter()
            .map(|r| r.expect("fixture declares well-formed entities"))
            .collect()
    }

    const HUMAN_CLAIM: &str = "### Read an affordance out of a document\n\n```turtle folio:graph\naffordance:read_declared_affordances a x0k:Affordance ;\n    x0k:status \"wip\" ;\n    x0k:claimedFor x0k:actor\\/human, x0k:actor\\/ai_agent .\n```\n";

    /// A signifier for something else entirely, so the set can speak to
    /// signification without answering for the affordance under test.
    const UNRELATED_SIGNIFIER: &str = "### `weave`\n\n```turtle folio:graph\nsignifier:x0k-tangle-weave a x0k:Signifier ;\n    x0k:signifies affordance:weave_a_document ;\n    x0k:presentedOn surface:cli .\n```\n";

    #[test]
    fn a_human_claim_with_no_signifier_is_a_defect_in_a_set_that_signifies() {
        let entities = declarations(&format!("{HUMAN_CLAIM}{UNRELATED_SIGNIFIER}"));
        let report = check_declarations(&entities);
        assert_eq!(report.checked, 2);
        assert!(report.notes.is_empty(), "the set can answer: {:?}", report.notes);
        match report.defects.as_slice() {
            [DeclarationDefect::HumanClaimWithoutSignifier { affordance }] => {
                assert_eq!(affordance.to_string(), "x0k:affordance/read_declared_affordances");
            }
            other => panic!("expected one HumanClaimWithoutSignifier, got {other:?}"),
        }
    }

    /// `check decisions` on a pristine clone: the affordances are here
    /// and every signifier is one directory over, so the absence of a
    /// signifier is the scan's shape and not a broken promise.
    #[test]
    fn a_human_claim_is_a_note_in_a_set_that_declares_no_signifier() {
        let report = check_declarations(&declarations(HUMAN_CLAIM));
        assert_eq!(report.checked, 1);
        assert!(report.is_clean(), "unexpected defects: {:?}", report.defects);
        assert!(matches!(report.notes.as_slice(), [DeclarationNote::HumanClaimUnsignifiable { .. }]));
    }

    #[test]
    fn a_human_claim_with_a_signifier_is_clean() {
        let body = format!(
            "{HUMAN_CLAIM}\n### `extract_from_markdown`\n\n```turtle folio:graph\nsignifier:x0k-folio-extract-from-markdown a x0k:Signifier ;\n    x0k:signifies affordance:read_declared_affordances ;\n    x0k:presentedOn surface:sdk .\n```\n"
        );
        let report = check_declarations(&declarations(&body));
        assert_eq!(report.checked, 2);
        assert!(report.is_clean(), "unexpected defects: {:?}", report.defects);
    }

    #[test]
    fn an_agent_only_claim_needs_no_signifier() {
        let body = HUMAN_CLAIM.replace("x0k:actor\\/human, x0k:actor\\/ai_agent", "x0k:actor\\/ai_agent");
        let report = check_declarations(&declarations(&body));
        assert!(report.is_clean());
        assert!(report.notes.is_empty(), "nothing was asked: {:?}", report.notes);
    }

    /// The shipped `crates/x0k-folio-cli/examples/papers` collection,
    /// carried inline: a vocabulary document and two papers, one citing
    /// the other. The alpha block is the parameter because every test
    /// below is that block broken a different way — which is exactly what
    /// three maintainers did to the real directory on 2026-09-22, each
    /// watching `check` pass at exit 0.
    #[cfg(feature = "document-vocabulary")]
    fn papers(alpha_block: &str) -> [(&'static str, String); 3] {
        papers_with("", alpha_block)
    }

    /// [`papers`] with `extra` Turtle appended to the vocabulary block.
    #[cfg(feature = "document-vocabulary")]
    fn papers_with(extra: &str, alpha_block: &str) -> [(&'static str, String); 3] {
        const VOCABULARY: &str = r#"# Papers and citations

```turtle folio:document
wiki:paper-vocabulary a x0k:Wiki .
```

```turtle folio:graph
@prefix paper: <https://example.org/papers#> .

<https://example.org/paper-vocabulary> a owl:Ontology ;
    vann:preferredNamespacePrefix "paper" ;
    vann:preferredNamespaceUri "https://example.org/papers#" .
paper:Paper a owl:Class ;
    rdfs:isDefinedBy <https://example.org/paper-vocabulary> .
paper:cites a owl:ObjectProperty ;
    rdfs:domain paper:Paper ;
    rdfs:range paper:Paper ;
    rdfs:isDefinedBy <https://example.org/paper-vocabulary> .
paper:reviewed a owl:DatatypeProperty ;
    rdfs:domain paper:Paper ;
    rdfs:range xsd:boolean ;
    rdfs:isDefinedBy <https://example.org/paper-vocabulary> .
paper:pages a owl:DatatypeProperty ;
    rdfs:domain paper:Paper ;
    rdfs:range xsd:integer ;
    rdfs:isDefinedBy <https://example.org/paper-vocabulary> .
"#;
        const BETA: &str = "# Beta\n\n```turtle folio:document\nwiki:paper-beta a x0k:Wiki .\n```\n\n```turtle folio:graph\npaper:paper\\/beta a paper:Paper ;\n    paper:reviewed true ;\n    paper:pages 12 .\n```\n";
        let vocabulary = format!("{VOCABULARY}{extra}```\n");
        let alpha = format!(
            "# Alpha\n\n```turtle folio:document\nwiki:paper-alpha a x0k:Wiki .\n```\n\nA paper about reading a collection as a graph.\n\n{alpha_block}"
        );
        [("vocabulary.md", vocabulary), ("alpha.md", alpha), ("beta.md", BETA.to_string())]
    }

    #[cfg(feature = "document-vocabulary")]
    const ALPHA_CITES_BETA: &str = "```turtle folio:graph\npaper:paper\\/alpha a paper:Paper ;\n    paper:reviewed true ;\n    paper:pages 12 ;\n    paper:cites paper:paper\\/beta .\n```\n";

    #[cfg(feature = "document-vocabulary")]
    fn check_collection(documents: &[(&'static str, String)]) -> InstanceCheck {
        let sources: Vec<_> = documents
            .iter()
            .map(|(id, body)| crate::document_vocabulary::DocumentSource { id, body })
            .collect();
        check_instances(&OntologyModel::new([]), &sources)
    }

    #[cfg(feature = "document-vocabulary")]
    fn check_papers(alpha_block: &str) -> InstanceCheck {
        check_collection(&papers(alpha_block))
    }

    /// The one defect a broken block produced, rendered as `check` prints it.
    #[cfg(feature = "document-vocabulary")]
    fn only_defect(report: &DeclarationReport) -> String {
        match report.defects.as_slice() {
            [defect] => defect.to_string(),
            other => panic!("expected one defect, got {other:?}"),
        }
    }

    #[cfg(feature = "document-vocabulary")]
    #[test]
    fn the_shipped_papers_collection_checks_clean_and_is_counted() {
        let report = check_papers(ALPHA_CITES_BETA).report;
        assert!(report.is_clean(), "unexpected defects: {:?}", report.defects);
        assert_eq!(report.checked, 2, "both papers are declarations, and both were read");
        assert!(report.dangling.is_empty());
    }

    #[cfg(feature = "document-vocabulary")]
    #[test]
    fn a_predicate_the_collections_vocabulary_never_declares_is_a_defect() {
        let rendered = only_defect(&check_papers(&ALPHA_CITES_BETA.replace("paper:cites", "paper:shreds")).report);
        assert!(rendered.contains("unknown object property"), "{rendered}");
        assert!(rendered.contains("alpha.md:"), "the defect names a line: {rendered}");
    }

    #[cfg(feature = "document-vocabulary")]
    #[test]
    fn an_instance_of_a_class_that_does_not_exist_is_a_defect() {
        let broken = ALPHA_CITES_BETA
            .replace("paper:paper\\/alpha a paper:Paper", "paper:monograph\\/alpha a paper:Monograph");
        let rendered = only_defect(&check_papers(&broken).report);
        assert!(rendered.contains("unknown concept paper:Monograph"), "{rendered}");
    }

    #[cfg(feature = "document-vocabulary")]
    #[test]
    fn a_citation_of_a_paper_that_is_not_here_is_noted_and_not_a_defect() {
        let report = check_papers(&ALPHA_CITES_BETA.replace("paper:paper\\/beta", "paper:paper\\/nowhere")).report;
        assert!(report.is_clean(), "a region boundary is not a fault: {:?}", report.defects);
        match report.dangling.as_slice() {
            [edge] => {
                assert_eq!(edge.source, "alpha.md");
                assert_eq!(edge.subject, "paper:paper/alpha");
                assert_eq!(edge.predicate, "paper:cites");
                assert_eq!(edge.target, "paper:paper/nowhere");
            }
            other => panic!("expected one dangling declaration, got {other:?}"),
        }
    }

    /// The block that declares `paper` has to reach the header pass: an id
    /// in the collection's own namespace is refused without it.
    #[cfg(feature = "document-vocabulary")]
    #[test]
    fn the_collections_own_prefix_reaches_the_header_pass() {
        let model = check_papers(ALPHA_CITES_BETA).model;
        assert!(EntityId::parse_in(&model, "paper:paper/alpha").is_ok());
        assert!(EntityId::parse_in(&OntologyModel::new([]), "paper:paper/alpha").is_err());
    }

    /// Incident: the Backstage maintainer's second and third rounds
    /// (2026-09-23) — a misspelled field in the shipped papers example
    /// passed at exit 0.
    #[cfg(feature = "document-vocabulary")]
    #[test]
    fn a_misspelled_field_is_refused_naming_the_nearest_declared_property() {
        let rendered = only_defect(&check_papers(&ALPHA_CITES_BETA.replace("paper:reviewed", "paper:revieweddd")).report);
        assert!(rendered.contains("states `paper:revieweddd`"), "names the term: {rendered}");
        assert!(rendered.contains("nearest declared property is `paper:reviewed`"), "names the nearest: {rendered}");
        assert!(rendered.contains("alpha.md:9"), "names the block's line: {rendered}");
    }

    /// Incident: the same rounds — a string where the vocabulary declares a
    /// boolean passed at exit 0.
    #[cfg(feature = "document-vocabulary")]
    #[test]
    fn a_string_where_a_boolean_is_declared_is_refused_naming_property_value_and_datatype() {
        let rendered = only_defect(&check_papers(&ALPHA_CITES_BETA.replace("paper:reviewed true", "paper:reviewed \"not-a-boolean\"")).report);
        for part in ["`paper:reviewed`", "\"not-a-boolean\"", "`xsd:boolean`", "alpha.md:9"] {
            assert!(rendered.contains(part), "missing {part}: {rendered}");
        }
    }

    /// One conforming and one contradicting literal for each datatype the
    /// check interprets.
    #[cfg(feature = "document-vocabulary")]
    #[test]
    fn each_interpreted_datatype_admits_its_values_and_refuses_the_rest() {
        const VENUE: &str = "paper:venue a owl:DatatypeProperty ;\n    rdfs:domain paper:Paper ;\n    \
                             rdfs:range xsd:string ;\n    \
                             rdfs:isDefinedBy <https://example.org/paper-vocabulary> .\n";
        let cases: [(&str, &[&str], &[&str]); 3] = [
            ("venue", &["\"Proceedings\"", "\"a\", \"b\""], &["12", "true", "'{\"at\":\"home\"}'^^rdf:JSON"]),
            ("pages", &["12", "-3", "1, 2"], &["\"12\"", "12.5", "12.0", "true", "1, \"two\""]),
            ("reviewed", &["true", "false"], &["\"true\"", "1", "0", "\"not-a-boolean\""]),
        ];
        for (field, conforming, contradicting) in cases {
            let block = |value: &str| format!("```turtle folio:graph\npaper:paper\\/alpha a paper:Paper ;\n    paper:{field} {value} .\n```\n");
            for value in conforming {
                let report = check_collection(&papers_with(VENUE, &block(value))).report;
                assert!(report.is_clean(), "{field}: {value} conforms: {:?}", report.defects);
            }
            for value in contradicting {
                let rendered = only_defect(&check_collection(&papers_with(VENUE, &block(value))).report);
                assert!(rendered.contains(&format!("`paper:{field}`")), "{field}: {value}: {rendered}");
                assert!(rendered.contains("contradicts its declared datatype"), "{rendered}");
            }
        }
    }

    #[cfg(feature = "document-vocabulary")]
    #[test]
    fn no_range_or_an_uninterpreted_datatype_refuses_nothing() {
        const OPEN: &str = "paper:note a owl:DatatypeProperty ;\n    rdfs:domain paper:Paper ;\n    \
                            rdfs:isDefinedBy <https://example.org/paper-vocabulary> .\n\
                            paper:published a owl:DatatypeProperty ;\n    rdfs:domain paper:Paper ;\n    \
                            rdfs:range xsd:date ;\n    \
                            rdfs:isDefinedBy <https://example.org/paper-vocabulary> .\n";
        let block = "```turtle folio:graph\npaper:paper\\/alpha a paper:Paper ;\n    paper:note 12 ;\n    paper:published \"not a date\" .\n```\n";
        let report = check_collection(&papers_with(OPEN, block)).report;
        assert!(report.is_clean(), "{:?}", report.defects);
    }

    /// A class no datatype property describes keeps its field names open —
    /// the shape of every signifier and its `x0k:cue` — while a range the
    /// vocabulary does declare is still read on it.
    #[cfg(feature = "document-vocabulary")]
    #[test]
    fn an_undescribed_class_keeps_its_field_names_open_and_its_ranges_checked() {
        const NOTE: &str = "paper:Note a owl:Class ;\n    \
                            rdfs:isDefinedBy <https://example.org/paper-vocabulary> .\n\
                            paper:count a owl:DatatypeProperty ;\n    rdfs:range xsd:integer ;\n    \
                            rdfs:isDefinedBy <https://example.org/paper-vocabulary> .\n";
        let note = |extra: &str| format!("```turtle folio:graph\npaper:note\\/one a paper:Note ;\n    paper:cue \"anything\"{extra} .\n```\n");
        let report = check_collection(&papers_with(NOTE, &note(""))).report;
        assert!(report.is_clean(), "an undescribed class is open: {:?}", report.defects);
        let report = check_collection(&papers_with(NOTE, &note(" ;\n    paper:count \"three\""))).report;
        assert!(only_defect(&report).contains("`xsd:integer`"));
    }

    #[cfg(feature = "document-vocabulary")]
    #[test]
    fn placement_demands_are_not_fields() {
        let block = "```turtle folio:graph\npaper:paper\\/alpha a paper:Paper ;\n    paper:requiresResources '{\"kind\":\"gpu\"}'^^rdf:JSON ;\n    paper:reviewed true .\n```\n";
        let report = check_papers(block).report;
        assert!(report.is_clean(), "{:?}", report.defects);
    }

    /// The shipped modules' own ranges, read on the shipped model: an
    /// affordance's `x0k:status` is an `xsd:string`. A signifier's `x0k:cue`
    /// names no term and `Signifier` is described by neither, so it passes.
    #[cfg(feature = "document-vocabulary")]
    #[test]
    fn the_shipped_string_range_is_read_and_an_undescribed_signifier_passes() {
        let body = |status: &str| {
            format!(
                "# Example\n\n```turtle folio:document\ndesign:example a x0k:Design .\n```\n\n## Read\n\n```turtle folio:graph\naffordance:read a x0k:Affordance ;\n    x0k:status {status} .\n```\n\n## Face\n\n```turtle folio:graph\nsignifier:read-face a x0k:Signifier ;\n    x0k:cue \"read\" .\n```\n"
            )
        };
        let check = |status: &str| {
            let text = body(status);
            let sources = [crate::document_vocabulary::DocumentSource { id: "example.md", body: &text }];
            check_instances(&OntologyModel::shipped(), &sources).report
        };
        let report = check("\"designed\"");
        assert!(report.is_clean(), "{:?}", report.defects);
        let rendered = only_defect(&check("3"));
        assert!(rendered.contains("`x0k:status` the integer `3`") && rendered.contains("`xsd:string`"), "{rendered}");
    }
}
`````

## Composing the module

<a name="chunk-root"></a><sub>[`src/envelope_check.rs`](../../crates/x0k-folio/src/envelope_check.rs) · `#root` · assembles [module-doc](#chunk-module-doc) · [standing](#chunk-standing) · [vocabulary](#chunk-vocabulary) · [defect](#chunk-defect) · [check-envelope](#chunk-check-envelope) · [check-corpus](#chunk-check-corpus) · [check-declarations](#chunk-check-declarations) · [check-instances](#chunk-check-instances) · [check-literals](#chunk-check-literals) · [tests](#chunk-tests)</sub>

```rust {#root}
<<module-doc>>

<<standing>>

<<vocabulary>>

<<defect>>

<<check-envelope>>

<<check-corpus>>

<<check-declarations>>

<<check-instances>>

<<check-literals>>

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
