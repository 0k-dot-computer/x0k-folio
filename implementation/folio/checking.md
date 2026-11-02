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
- a **dangling edge** is a well-formed target naming no document and no
  declaration in the set being checked. Expected. Reported, never counted
  as failure.

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
//!   a term no shipped ontology module declares, stated as an edge or as
//!   a literal, whatever the document's class. The last is a packaging
//!   fault or a typo: a publication selected a module set that does not
//!   span its own corpus, or the author misspelled a term it does.
//! - **Does this edge's target name a document or a declaration in the
//!   set being checked?** A `no` is a [`DanglingEdge`], which is ordinary
//!   and expected: a publication is a region of a graph, and an edge
//!   leaving the region is the boundary working, not a failure.
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
//! that vocabulary: a key naming no declared property, a literal on an object
//! property, a subject outside its property's domain, and a value contradicting
//! its property's XSD range, on any instance (`check_literal_fields`). A
//! header's literals answer to the same rules, in the same words
//! ([`Defect::LiteralOnEdge`], [`Defect::UnacceptedSubject`],
//! [`Defect::ContradictedLiteral`]).
//!
//! A header names a document and a block names a declaration, and each
//! pass sees only its own list. `resolve_across` (with the
//! `document-vocabulary` feature) reads the two as one: an edge from either
//! side resolves against both, and an id on both sides is refused
//! ([`DeclarationDefect::IdCollision`]).

use std::collections::{BTreeMap, BTreeSet};

use x0k_ontology::concept_facts::{OntologyModel, OntologyValue, RDFS_DOMAIN, RDFS_RANGE, X0K_NS};

use crate::colophon::{Colophon, Literal, XSD_NS};
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
`x0k:Decision`, or a class that reaches it by `rdfs:subClassOf`?

That subject set is [`OntologyModel::subclass_closure`](../ontology/concept-facts.md)
of `x0k:Decision`, which `decision_edge_predicates` reads too, so the two
answers cannot differ. The edge table cannot answer here itself, because it
keys by `x0k:` local name and a reader's term does not have one. `x0k-folio`
is packaged against the released `x0k-ontology`, and `cargo package` verifies
the tarball against the registry, so this crate needs a release of
`x0k-ontology` that carries the walk, and the two release together.

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

### A literal statement is a term too

Standing is about edges, and for a long time edges were all the header
pass asked about. Every other statement — `x0k:summary "…"`, `x0k:tell
"never poll"` — went into the header's `properties` and was never looked
up, so a misspelled literal term passed at exit 0 on every class: an
`x0k:fooBar "x"` on a design, and on 2026-10-03 an `x0k:tells` for
`x0k:tell` on a principle while the agent-guidance leaves were being
typed. The guide's promise was that a predicate no loaded module declares
is refused by name, and the promise did not say "an edge's predicate".

So a literal statement answers the same first question an edge does —
does some loaded module declare this term? — with a smaller answer set:
any declared property will do, object, datatype or annotation, because a
literal's term carries no subject judgment here (domains are not
enforced; see *What this deliberately does not check*). The class of the
document plays no part. A design, a principle from a module this crate
never compiled, a reader's `mycorp:Brief`: one rule, the term as written.

The terms the parser reads for itself (`x0k:status`, `x0k:summary`,
`folio:tangleRoot` and their neighbours) never reach this question; they
are declared anyway, by `document` and the `folio/` family, so nothing
is exempted by spelling.

A refusal names the line of the statement that wrote the term, which the
header parse does not keep: [`Defect::locate`] reads it back off the
document's text for a caller that has it, and the check face does. And
when a declared term in the same namespace is within two edits, the
refusal names it, by the rule the field check below uses: `x0k:tells`
is told `x0k:tell`.

A term that is declared then has its value read. A literal its term's XSD
range contradicts is refused by the rule a block's field answers to
(*Literal fields*, below): one test, asked from two places, so a value does
not pass in a header that a block would refuse.

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
/// The classes a property admits on each side, `None` for a side naming none.
struct AdmittedClasses {
    domain: Option<BTreeSet<String>>,
    range: Option<BTreeSet<String>>,
}

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
    /// Every term the model types a property — object, datatype or
    /// annotation — as a full IRI: what a literal statement's term must be.
    terms: BTreeSet<String>,
    /// Full IRI of each of `terms` → every `rdfs:range` value it declares:
    /// what a literal statement's value is read against.
    ranges: BTreeMap<String, Vec<String>>,
    /// Full IRI of each of `terms` → the classes it admits.
    admitted: BTreeMap<String, AdmittedClasses>,
    /// Every class the model declares, as a full IRI.
    classes: BTreeSet<String>,
    /// The extension namespaces, `(prefix, namespace)`, for spelling a term
    /// between its compact and full forms without a fold per term.
    namespaces: Vec<(String, String)>,
}

impl Vocabulary {
    fn of(model: &OntologyModel) -> Self {
        let terms = declared_properties(model);
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
            ranges: declared_ranges(model, &terms),
            classes: model.classes().iter().map(|class| model.expand(&class.uri)).collect(),
            admitted: terms
                .iter()
                .map(|iri| {
                    let domain = model.admitted_classes(iri, RDFS_DOMAIN);
                    let range = model.admitted_classes(iri, RDFS_RANGE);
                    (iri.clone(), AdmittedClasses { domain, range })
                })
                .collect(),
            terms,
            namespaces: model.extension_namespaces(),
        }
    }

    /// Does `predicate`'s domain admit a subject of `class`? A class the
    /// vocabulary does not declare has no place in it to be judged by: one of
    /// the ten genera, under a vocabulary that does not name it.
    fn admits_subject(&self, predicate: &str, class: &str) -> bool {
        let sides = self.admitted.get(&self.expand(predicate));
        !self.classes.contains(class)
            || sides.and_then(|sides| sides.domain.as_ref()).is_none_or(|classes| classes.contains(class))
    }

    /// Is `predicate` an object property, whose value is an id?
    fn names_ids(&self, predicate: &str) -> bool {
        self.properties.contains_key(&self.compact(&self.expand(predicate)))
    }

    /// Does `predicate`'s range admit a target of `class`? As for
    /// [`Self::admits_subject`].
    fn admits_target(&self, predicate: &str, class: &str) -> bool {
        let sides = self.admitted.get(&self.expand(predicate));
        !self.classes.contains(class)
            || sides.and_then(|sides| sides.range.as_ref()).is_none_or(|classes| classes.contains(class))
    }

    /// The refusal of an edge whose target is of `class`.
    fn unaccepted_target(&self, subject: &str, predicate: &str, target: &str, class: &str) -> Defect {
        let range = self.properties.get(predicate).and_then(|(_, range)| range.as_deref());
        Defect::UnacceptedTarget {
            subject: subject.to_string(),
            predicate: predicate.to_string(),
            target: target.to_string(),
            class: self.compact(class),
            expected: range.map_or_else(|| "its range".to_string(), |range| self.compact(&self.expand(range))),
            line: None,
        }
    }

    /// The full IRI of a compact term, as `OntologyModel::expand` spells it.
    fn expand(&self, compact: &str) -> String {
        if let Some(local) = compact.strip_prefix("x0k:") {
            return format!("{X0K_NS}{local}");
        }
        self.namespaces
            .iter()
            .find_map(|(prefix, namespace)| {
                compact.strip_prefix(prefix.as_str())?.strip_prefix(':').map(|local| format!("{namespace}{local}"))
            })
            .unwrap_or_else(|| compact.to_string())
    }

    /// The compact spelling of a full IRI, as `OntologyModel::compact` spells it.
    fn compact(&self, iri: &str) -> String {
        if let Some(local) = iri.strip_prefix(X0K_NS) {
            return format!("x0k:{local}");
        }
        self.namespaces
            .iter()
            .find_map(|(prefix, namespace)| iri.strip_prefix(namespace.as_str()).map(|local| format!("{prefix}:{local}")))
            .unwrap_or_else(|| iri.to_string())
    }

    /// Does some loaded module declare this term, in any kind of property?
    fn declares(&self, compact: &str) -> bool {
        self.terms.contains(&self.expand(compact))
    }

    /// The declared term nearest an undeclared one, compact.
    fn nearest(&self, compact: &str) -> Option<String> {
        nearest_declared(&self.terms, &self.expand(compact)).map(|iri| self.compact(iri))
    }

    /// The datatypes among a property's ranges that refuse this literal,
    /// the property given as a full IRI. The value rule's one test: the
    /// header pass and the field pass both ask it.
    fn contradicted(&self, property: &str, literal: &Literal) -> Vec<Datatype> {
        self.ranges
            .get(property)
            .into_iter()
            .flatten()
            .filter_map(|range| Datatype::named(range))
            .filter(|datatype| !datatype.admits(literal))
            .collect()
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

/// `x0k:Decision` and every class that reaches it by `rdfs:subClassOf`, in
/// the compact spelling `object_properties` reports a domain in.
fn decision_domains(model: &OntologyModel) -> BTreeSet<String> {
    model
        .subclass_closure(&format!("{X0K_NS}Decision"))
        .into_iter()
        .map(|class| model.compact(&class).unwrap_or(class))
        .collect()
}

/// Every IRI `model` types as a datatype, object or annotation property —
/// a term a statement may be made in.
fn declared_properties(model: &OntologyModel) -> BTreeSet<String> {
    use x0k_ontology::concept_facts::{OWL_ANNOTATION_PROPERTY, OWL_DATATYPE_PROPERTY, OWL_OBJECT_PROPERTY, RDF_TYPE};
    model
        .facts()
        .iter()
        .filter(|fact| fact.predicate == RDF_TYPE)
        .filter(|fact| {
            matches!(&fact.value, OntologyValue::Entity(kind)
                if [OWL_DATATYPE_PROPERTY, OWL_OBJECT_PROPERTY, OWL_ANNOTATION_PROPERTY].contains(&kind.as_str()))
        })
        .map(|fact| fact.entity.clone())
        .collect()
}

/// Property IRI → every `rdfs:range` value `model` states of it, for the
/// declared `properties`.
fn declared_ranges(model: &OntologyModel, properties: &BTreeSet<String>) -> BTreeMap<String, Vec<String>> {
    let mut ranges: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for fact in model.facts() {
        let OntologyValue::Entity(value) = &fact.value else { continue };
        if fact.predicate == RDFS_RANGE && properties.contains(&fact.entity) {
            ranges.entry(fact.entity.clone()).or_default().push(value.clone());
        }
    }
    ranges
}

/// The declared property nearest `iri` in its own namespace: at most two
/// edits between local names, fewer than the name is long, ties to the
/// alphabetically first. Shared by the header pass and the field pass, so a
/// misspelling is told the same neighbour wherever it is written.
fn nearest_declared<'a>(declared: &'a BTreeSet<String>, iri: &str) -> Option<&'a String> {
    let local = iri.rsplit(['#', '/']).next().unwrap_or(iri);
    let namespace = &iri[..iri.len() - local.len()];
    let length = local.chars().count();
    declared
        .iter()
        .filter_map(|candidate| {
            let name = candidate.strip_prefix(namespace)?;
            let distance = edit_distance(local, name);
            (distance <= 2 && distance < length).then_some((distance, name, candidate))
        })
        .min_by(|a, b| (a.0, a.1).cmp(&(b.0, b.1)))
        .map(|(_, _, candidate)| candidate)
}

/// Levenshtein distance in characters.
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
```

## Defects

Five ways a document can fail the vocabulary shipped beside it. Each
carries the offending string, because these are read in a report over a
whole corpus where the finding without its subject is unactionable. The
two undeclared-term defects also carry the line that wrote the term and
the nearest declared term, when there is one, and a contradicted literal
carries that line too: a refusal over a corpus is read by someone who has
to open the file and fix it.

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
    /// No module of the vocabulary declares this edge's predicate. Either
    /// the module that defines the term was not selected, or the term is
    /// not vocabulary at all — a typo, most often.
    UndeclaredPredicate {
        predicate: String,
        /// The file line of the statement, once [`Defect::locate`] has read
        /// it off the document's text.
        line: Option<usize>,
        /// A declared term within two edits of this one, compact.
        nearest: Option<String>,
    },
    /// No module of the vocabulary declares the term of this literal
    /// statement. The same two causes as an undeclared edge.
    UndeclaredLiteral {
        predicate: String,
        /// As for [`Defect::UndeclaredPredicate`].
        line: Option<usize>,
        /// As for [`Defect::UndeclaredPredicate`].
        nearest: Option<String>,
    },
    /// A literal statement whose term declares an XSD range its value
    /// contradicts: the rule a block's field answers to, read on a header.
    ContradictedLiteral {
        /// The header's subject, compact.
        subject: String,
        predicate: String,
        literal: Literal,
        /// The range it contradicts.
        datatype: Datatype,
        /// As for [`Defect::UndeclaredPredicate`].
        line: Option<usize>,
    },
    /// A literal on an object property, whose value is an id. Text there
    /// could never dangle, so a typo in it would pass every other check.
    LiteralOnEdge {
        /// The header's subject, compact.
        subject: String,
        predicate: String,
        /// As for [`Defect::UndeclaredPredicate`].
        line: Option<usize>,
    },
    /// An edge whose term's `rdfs:domain` does not reach the document's
    /// class: the rule a block's edge answers to, read on a header.
    UnacceptedSubject {
        /// The header's subject, compact.
        subject: String,
        predicate: String,
        /// The document's class, compact.
        class: String,
        /// As for [`Defect::UndeclaredPredicate`].
        line: Option<usize>,
    },
    /// An edge whose target the set types outside the term's `rdfs:range`.
    UnacceptedTarget {
        /// The header's subject, compact.
        subject: String,
        predicate: String,
        /// The target, compact.
        target: String,
        /// The target's class, compact.
        class: String,
        /// What the range names, compact.
        expected: String,
        /// As for [`Defect::UndeclaredPredicate`].
        line: Option<usize>,
    },
}

impl std::fmt::Display for Defect {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let undeclared = |f: &mut std::fmt::Formatter<'_>, what: &str, predicate: &str, line: &Option<usize>, nearest: &Option<String>| {
            write!(f, "{what} `{predicate}`")?;
            if let Some(line) = line {
                write!(f, " at line {line}")?;
            }
            write!(f, " is declared by no ontology module in this vocabulary")?;
            if let Some(term) = nearest {
                write!(f, "; the nearest declared term is `{term}`")?;
            }
            write!(f, "; either select the module that defines it or stop using the term")
        };
        let located = |f: &mut std::fmt::Formatter<'_>, line: &Option<usize>| match line {
            Some(line) => write!(f, " at line {line}"),
            None => Ok(()),
        };
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
            Self::UndeclaredPredicate { predicate, line, nearest } => {
                undeclared(f, "edge predicate", predicate, line, nearest)
            }
            Self::UndeclaredLiteral { predicate, line, nearest } => {
                undeclared(f, "literal statement", predicate, line, nearest)
            }
            Self::ContradictedLiteral { subject, predicate, literal, datatype, line } => {
                write!(f, "{}", contradiction(subject, predicate, literal, *datatype))?;
                located(f, line)
            }
            Self::LiteralOnEdge { subject, predicate, line } => {
                write!(f, "`{subject}` gives `{predicate}` a literal, but it names an id")?;
                located(f, line)
            }
            Self::UnacceptedSubject { subject, predicate, class, line } => {
                write!(f, "`{subject}`: `{predicate}` does not accept `{class}`")?;
                located(f, line)
            }
            Self::UnacceptedTarget { subject, predicate, target, class, expected, line } => {
                write!(f, "`{subject}`: `{predicate}` target `{target}` has type `{class}`, expected `{expected}`")?;
                located(f, line)
            }
        }
    }
}

impl std::error::Error for Defect {}

impl Defect {
    /// Give an undeclared term, a contradicted literal or a refused edge the
    /// file line of the header statement that writes its term, read off `content` — the
    /// whole document the header was parsed from, so the line is one an
    /// editor opens. A term the header spells some other way than its
    /// compact form (a full `<…>` IRI, a block-local prefix) is given the
    /// header's own line instead. Every other defect is left as it was.
    pub fn locate(&mut self, content: &str) {
        let (Self::UndeclaredPredicate { predicate, line, .. }
        | Self::UndeclaredLiteral { predicate, line, .. }
        | Self::ContradictedLiteral { predicate, line, .. }
        | Self::LiteralOnEdge { predicate, line, .. }
        | Self::UnacceptedSubject { predicate, line, .. }
        | Self::UnacceptedTarget { predicate, line, .. }) = self
        else {
            return;
        };
        let Some(header) = crate::colophon::find_header(content) else {
            return;
        };
        let spelled = predicate.replace("x0k:folio/", "folio:");
        let stated = header
            .text
            .lines()
            .position(|text| states_term(text, &spelled) || states_term(text, predicate));
        *line = Some(stated.map_or(header.line, |offset| header.line + 1 + offset));
    }
}

/// Does one Turtle line state `term` as a token — not as part of a longer
/// term, an id, or a word in a string?
fn states_term(text: &str, term: &str) -> bool {
    text.match_indices(term).any(|(at, _)| {
        let before = text[..at].chars().next_back();
        let after = text[at + term.len()..].chars().next();
        !before.is_some_and(|c| c.is_alphanumeric() || "_:/\\\"-".contains(c))
            && after.is_none_or(char::is_whitespace)
    })
}
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
    /// The document's class, as a full IRI.
    pub class: String,
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
    report.class = envelope.doc_type.class_iri();

    for (predicate, targets) in &envelope.edges {
        if matches!(vocabulary.standing(predicate), PredicateStanding::Undeclared) {
            report.defects.push(Defect::UndeclaredPredicate {
                predicate: predicate.clone(),
                line: None,
                nearest: vocabulary.nearest(predicate),
            });
        }
        if !vocabulary.admits_subject(predicate, &report.class) {
            report.defects.push(Defect::UnacceptedSubject {
                subject: envelope.id.clone(),
                predicate: predicate.clone(),
                class: vocabulary.compact(&report.class),
                line: None,
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

    // Every literal the parser did not read for itself, whatever the
    // document's class: its term ("A literal statement is a term too",
    // above), then its value ("Literal fields", below).
    for (predicate, literals) in &envelope.properties {
        if !vocabulary.declares(predicate) {
            report.defects.push(Defect::UndeclaredLiteral {
                predicate: predicate.clone(),
                line: None,
                nearest: vocabulary.nearest(predicate),
            });
            continue;
        }
        if vocabulary.names_ids(predicate) {
            report.defects.push(Defect::LiteralOnEdge {
                subject: envelope.id.clone(),
                predicate: predicate.clone(),
                line: None,
            });
            continue;
        }
        if !vocabulary.admits_subject(predicate, &report.class) {
            report.defects.push(Defect::UnacceptedSubject {
                subject: envelope.id.clone(),
                predicate: predicate.clone(),
                class: vocabulary.compact(&report.class),
                line: None,
            });
        }
        let property = vocabulary.expand(predicate);
        for literal in literals {
            for datatype in vocabulary.contradicted(&property, literal) {
                report.defects.push(Defect::ContradictedLiteral {
                    subject: envelope.id.clone(),
                    predicate: predicate.clone(),
                    literal: literal.clone(),
                    datatype,
                    line: None,
                });
            }
        }
    }

    report
}
```

## A set of documents

Resolution needs a set to resolve against, and the set is whatever the
caller hands over — a published bundle's documents, a directory, one
region. Everything an envelope points at that is not in that set is
reported as dangling, in a list of its own.

This pass sees headers, so it knows document ids and nothing else. Its
report keeps those ids, because a set holds a second kind:
[`resolve_across`](#one-set-one-space-of-ids), below, joins this list to the
declarations and takes back every edge that lands on one. A caller that has
only headers gets the answer those headers can give.

One property of this pass is deliberate. A document whose own id is
malformed contributes no id to the set, so its edges are still checked and
its inbound edges dangle — the fault is reported once, at its source, and
its consequences are visible rather than swallowed.

<a name="chunk-check-corpus"></a><sub>[`src/envelope_check.rs`](../../crates/x0k-folio/src/envelope_check.rs) · `#check-corpus`</sub>

```rust {#check-corpus}
/// A well-formed edge whose target is not in the checked set. Ordinary on a
/// published region: an edge out of the region is the publication boundary,
/// not a fault.
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
    /// The id of every document in the set whose id parsed, with the
    /// caller's name for the document.
    pub ids: BTreeMap<EntityId, String>,
    /// The class of each of `ids`, as a full IRI.
    pub classes: BTreeMap<EntityId, String>,
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

    let ids: BTreeMap<EntityId, String> = reports
        .iter()
        .filter_map(|(source, report)| Some((report.id.clone()?, source.to_string())))
        .collect();
    let classes: BTreeMap<EntityId, String> = reports
        .iter()
        .filter_map(|(_, report)| Some((report.id.clone()?, report.class.clone())))
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
            match classes.get(target) {
                Some(class) if !vocabulary.admits_target(predicate, class) => out.defects.push((
                    source.to_string(),
                    vocabulary.unaccepted_target(&subject.to_string(), predicate, &target.to_string(), class),
                )),
                Some(_) => {}
                None => out.dangling.push(DanglingEdge {
                    source: source.to_string(),
                    subject: subject.clone(),
                    predicate: predicate.clone(),
                    target: target.clone(),
                }),
            }
        }
    }

    out.ids = ids;
    out.classes = classes;
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
    /// One id on a document's header and on a block's subject. An edge to it
    /// could mean either, so it names neither.
    IdCollision {
        id: EntityId,
        /// The caller's name for the document.
        document: String,
        /// The block's document and line.
        declaration: String,
    },
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
            Self::IdCollision { id, document, declaration } => write!(
                f,
                "`{id}` names both the document {document} and the declaration at {declaration}; \
                 an id names one thing, so give one of them another"
            ),
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

/// A declared relationship whose target is well formed and is not in the
/// set. The instance-level twin of [`DanglingEdge`], and ordinary for the
/// same reason: a collection is a region.
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
    /// Relationship targets that are not in the set.
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
answer. Whether a document holds the id is not this function's question
either, because it runs before the first header is parsed:
[`resolve_across`](#one-set-one-space-of-ids) answers it, from the ids this
function hands back.

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
    /// The full IRI of every instance that parsed, with the block that
    /// declares it and its class. A document whose blocks did not all parse
    /// has none here.
    pub declared: BTreeMap<String, DeclaredInstanceAt>,
}

/// Where a set declares an instance, and what it declares it to be.
#[cfg(feature = "document-vocabulary")]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeclaredInstanceAt {
    pub source: crate::document_vocabulary::BlockSource,
    /// The instance's class, as a full IRI.
    pub class: String,
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
            return InstanceCheck { model: OntologyModel::new(base.facts().to_vec()), report, declared: BTreeMap::new() };
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
    let mut declared = BTreeMap::new();
    for instance in &instances {
        let at = DeclaredInstanceAt { source: instance.source.clone(), class: instance.concept.clone() };
        if let Some(prior) = declared.insert(instance.iri.clone(), at) {
            report.defects.push(DeclarationDefect::Instance {
                reason: format!(
                    "duplicate instance {} at {}, already declared at {}",
                    instance.iri, instance.source, prior.source.document
                ),
            });
        }
    }
    if !every_document_parsed {
        return InstanceCheck { model: vocabulary.model, report, declared };
    }
    match validate_relationships(&vocabulary.model, &instances) {
        Ok(relationships) => {
            let spell = |iri: &str| vocabulary.model.compact(iri).unwrap_or_else(|| iri.to_string());
            for relationship in relationships {
                if declared.contains_key(&relationship.object) {
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

    InstanceCheck { model: vocabulary.model, report, declared }
}
```

## One set, one space of ids

A set holds two kinds of id. A header names a document, and a graph block
names a declaration. An edge is written in one of those places and may aim at
either kind: a report's header cites `paper:paper/alpha`, which a block
declares, and a block's `paper:cites` names `paper:report/one`, which a header
declares. Both targets are in the set, so neither edge has left it. Each pass
sees only its own list, though, and would call its edge dangling, and under
`--closed` that fails a correct collection.

**An edge resolves when its target is a document id or a declaration id in the
set.** `ingest` already reads the set this way: a header edge to a block is an
ordinary fact, and a query joins it. `check` agrees with what the query sees.

That rule is safe because of a second one. **An id names one thing.** A block
must not stand in for a document. If a block could share a document's id, an
edge would resolve whichever of the two it meant, and a fault in the document
would be hidden behind the block. So an id on both lists is a defect that names
both places, [`DeclarationDefect::IdCollision`], and it fails the run with or
without `--closed`. A document whose header fails is reported at its source,
whatever points at it. `--closed` loses nothing: an id on neither list is a
misspelling or an unfinished rename, and it fails as it did.

Ids compare as the IRIs they expand to, so two prefixes for one namespace name
one id. The join runs after both passes because of their order: the header pass
reads under the model the instance pass extends, so neither has the other's
list when it runs.

The join finishes the typing as well as the resolution. Each pass knows only
its own list's classes, so an edge that crosses is the one neither pass could
judge, and it is judged here: a target the other list types outside the term's
`rdfs:range` is refused, in the words its own side uses. A header's edges meet
the same rule at home. Its subject's class answers to the term's `rdfs:domain`,
and a target among the set's documents to its range, through the one question
blocks ask, `OntologyModel::admitted_classes`. A target outside the set has no
class to read, so it stays a dangling edge and is refused only by `--closed`.
Nor is a class the vocabulary does not declare judged: the ten genera parse
under any vocabulary, and one that omits `x0k:Design` says nothing of where
it stands.

<a name="chunk-resolve-across"></a><sub>[`src/envelope_check.rs`](../../crates/x0k-folio/src/envelope_check.rs) · `#resolve-across`</sub>

```rust {#resolve-across}
/// Read a set's two lists of ids as one. Take back every edge that lands on
/// either list, and refuse an id on both. `declared` is
/// [`InstanceCheck::declared`], and `model` the one [`InstanceCheck`] returned.
#[cfg(feature = "document-vocabulary")]
pub fn resolve_across(
    model: &OntologyModel,
    declared: &BTreeMap<String, DeclaredInstanceAt>,
    corpus: &mut CorpusReport,
    declarations: &mut DeclarationReport,
) {
    let documents: BTreeMap<String, (&EntityId, &String)> = corpus
        .ids
        .iter()
        .map(|(id, document)| (model.expand(&id.to_string()), (id, document)))
        .collect();
    let vocabulary = Vocabulary::of(model);
    let header_edges: Vec<(String, Defect)> = corpus
        .dangling
        .iter()
        .filter_map(|edge| {
            let target = edge.target.to_string();
            let class = &declared.get(&model.expand(&target))?.class;
            (!vocabulary.admits_target(&edge.predicate, class)).then(|| {
                (edge.source.clone(), vocabulary.unaccepted_target(&edge.subject.to_string(), &edge.predicate, &target, class))
            })
        })
        .collect();
    corpus.defects.extend(header_edges);
    for edge in &declarations.dangling {
        let target = model.expand(&edge.target);
        let Some(class) = documents.get(&target).and_then(|(id, _)| corpus.classes.get(*id)) else {
            continue;
        };
        if !vocabulary.admits_target(&edge.predicate, class) {
            let predicate = model.expand(&edge.predicate);
            let expected = model.class_references(&predicate, RDFS_RANGE).join(", ");
            declarations.defects.push(DeclarationDefect::Instance {
                reason: format!("{predicate} target {target} has type {class}, expected {expected} at {}", edge.source),
            });
        }
    }
    let in_the_set = |iri: String| declared.contains_key(&iri) || documents.contains_key(&iri);
    corpus.dangling.retain(|edge| !in_the_set(model.expand(&edge.target.to_string())));
    declarations.dangling.retain(|edge| !in_the_set(model.expand(&edge.target)));
    for (iri, (id, document)) in documents {
        if let Some(block) = declared.get(&iri) {
            declarations.defects.push(DeclarationDefect::IdCollision {
                id: id.clone(),
                document: document.clone(),
                declaration: block.source.to_string(),
            });
        }
    }
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

1. **A field names a declared property.** A field whose predicate no
   loaded module declares is refused, on an instance of any class, and the
   refusal names the term and the nearest declared property when one is
   close.
2. **A field's value does not contradict the property's declared
   datatype.** A field whose predicate has an XSD datatype in its
   `rdfs:range`, and whose literal is not a value of that datatype, is
   refused, and the refusal names the property, the value and the datatype
   it expected.

A property with no declared range is not refused, whatever the value. A
statement whose object is an IRI is a relationship, checked above, not a
field. A range naming an XSD datatype outside the five below is not
interpreted, and not refused.

The predicate a field is checked under is the predicate the block states,
which is the predicate `x0k-folio-cli ingest` writes the fact under
([`document-source.md`](document-source.md)): what `check` reads and what
`ingest` stores are one term. A property is *declared* when the loaded
vocabulary types it `owl:DatatypeProperty`, `owl:ObjectProperty` or
`owl:AnnotationProperty`, in whichever module.

**Why every class, and not only the ones the vocabulary describes.** The
first version of this rule judged field names only on an instance of a
*described* class — one some `owl:DatatypeProperty` names as its exact
domain — on the reasoning that the shipped vocabulary was silent about most
of its classes' literals and silence is not a claim. `software` declared
`Signifier` and no literal property of it, while every signifier in this
bundle carried an `x0k:cue`, so judging every field would have refused the
chapter that specifies the check. What that bought was a hole the size of
the vocabulary's silence: a field misspelled on an undescribed class passed,
and so did the header's literals, which this module never read at all
("A literal statement is a term too", above). The guide promised a
predicate no loaded module declares is refused by name, without a class
condition, and the promise is the rule now. Where the vocabulary was silent
about a term the corpus really uses, the repair is the vocabulary's:
`software` declares `x0k:cue` over `Signifier`, and `document` declares the
literals a publication manifest carries for the projector (2026-10-03).

**The nearest declared property** is chosen by edit distance: the
Levenshtein distance, in characters, between the local name as written and
each declared property's local name in the same namespace. The nearest is
named when it is at most two edits away and fewer edits than the name has
characters; a tie goes to the alphabetically first.

### What "contradicts" means, datatype by datatype

The shipped modules name five XSD datatypes in a range, and no other:
`xsd:string`, `xsd:integer`, `xsd:boolean`, `xsd:decimal` and `xsd:date`.
Those five are what this check interprets.

A literal conforms to a range when two things hold: its datatype matches the
range, and its text lies in that datatype's lexical space. The two tests
catch different mistakes.

The datatype test reads what the statement wrote. `ingest` stores a literal
by its datatype — a quoted `"12"` as text, a bare `12` as a signed integer, a
bare `true` as a boolean — so a literal of the wrong datatype becomes a fact
of the wrong type. A plain string on a date fails this test, as `"12"` fails
on an integer. Turtle has no bare date, so `"2026-11-01"^^xsd:date` is the
one spelling of one.

The text test reads what the datatype claims. `"not a date"^^xsd:date` has
the right datatype and names no day. `"2026-02-30"^^xsd:date` names a day the
calendar does not hold. The text answers to the literal's own datatype.

One derivation is read. XSD derives `xsd:integer` from `xsd:decimal`, and a
bare `12` in Turtle is an integer, so an `xsd:integer` literal also conforms
to an `xsd:decimal` range. It is one explicit arm in `Datatype::admits`, not
a lattice of types: a decimal does not conform to an integer range, and no
other datatype stands in for another.

| Range | Conforms | Contradicts |
|---|---|---|
| `xsd:string` | a string literal | a boolean, a number, a date, a JSON literal, any other datatype |
| `xsd:integer` | a bare integer (`12`, `-3`), `"12"^^xsd:integer` | a string — `"12"` included —, a decimal or double (`12.5`, `12.0`, `1e3`), a boolean, `"twelve"^^xsd:integer` |
| `xsd:boolean` | a bare `true` or `false`, `"1"^^xsd:boolean`, `"0"^^xsd:boolean` | a string — `"true"` included —, a number, `0` and `1` included, `"yes"^^xsd:boolean` |
| `xsd:decimal` | a bare decimal (`12.5`), a bare integer (`12`), `"12.5"^^xsd:decimal` | a string — `"12.5"` included —, a double (`1e3`), a boolean, `"1e3"^^xsd:decimal` |
| `xsd:date` | `"2026-11-01"^^xsd:date`, `"2026-11-01Z"^^xsd:date`, `"2026-11-01+05:30"^^xsd:date` | a string — `"2026-11-01"` included —, a number, a boolean, a day the calendar does not hold (`"2025-02-29"^^xsd:date`) |

The lexical spaces are these:

- `xsd:string`: any text.
- `xsd:integer`: `[+-]?[0-9]+`.
- `xsd:boolean`: `true`, `false`, `1`, `0`.
- `xsd:decimal`: `[+-]?([0-9]+(\.[0-9]*)?|\.[0-9]+)`. A decimal has no exponent.
- `xsd:date`: `-?[0-9]{4,}-[0-9]{2}-[0-9]{2}`, then an optional `Z` or
  `[+-]hh:mm` up to fourteen hours, naming a real calendar date.

### A header's literals answer to the same value rule

A header statement whose object is a literal is read by the second rule
exactly as a field is, on a document of any class. The first rule has its
own home in the header pass ("A literal statement is a term too", above);
the second is one test, `Vocabulary::contradicted`, which both passes ask, so
a table above is a table of both surfaces. The terms the parser reads for
itself (`x0k:status`, `x0k:summary`, `folio:tangleRoot` and their neighbours)
never reach it, as they never reach the first.

### What the refusals say

Each is a `DeclarationDefect::Instance`, one per field, at the line of the
block's opening fence:

```text
declaration does not hold: `paper:paper/alpha` states `paper:revieweddd`, which names no property the loaded vocabulary declares; the nearest declared property is `paper:reviewed` at alpha.md:11
declaration does not hold: `paper:paper/alpha` gives `paper:reviewed` the string "not-a-boolean", which contradicts its declared datatype `xsd:boolean`: a boolean is a bare `true` or `false` at alpha.md:11
```

A header's value is a `Defect::ContradictedLiteral`, in the same words, at
the line of the statement:

```text
`paper:paper/alpha` gives `paper:reviewed` the string "yes", which contradicts its declared datatype `xsd:boolean`: a boolean is a bare `true` or `false` at line 6
```

A literal its own datatype does not hold is named as written, because it is
not a value of that datatype and its datatype is not the mistake:

```text
`paper:paper/alpha` gives `paper:published` "not a date"^^xsd:date, which contradicts its declared datatype `xsd:date`: a date is a real calendar day, written `"2026-11-01"^^xsd:date` at line 6
```

`ingest` refuses neither an undeclared term nor a datatype its range
contradicts; `check` is the gate a reader puts in CI. `ingest` reads a
literal's text only to store a typed value, so `"yes"^^xsd:boolean` fails
there too, and a date is stored as text, unread.

### The code

The field pass reads the tables the header pass folds, [`Vocabulary`]: which
IRIs are declared properties, and the ranges each carries. They are read off
`facts()`, because `OntologyModel` has no view of either. The value test and
its sentence need no document vocabulary,
because a header is read without one; only the walk over a set's instances
does.

<a name="chunk-check-literals"></a><sub>[`src/envelope_check.rs`](../../crates/x0k-folio/src/envelope_check.rs) · `#check-literals` · assembles [lexical-spaces](#chunk-lexical-spaces)</sub>

```rust {#check-literals}
/// The XSD datatypes a range can name that this check interprets: the five
/// the shipped modules name. A range naming any other refuses nothing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum Datatype {
    String,
    Integer,
    Boolean,
    Decimal,
    Date,
}

impl Datatype {
    /// The datatype a range IRI names, or `None` for one not interpreted.
    fn named(iri: &str) -> Option<Self> {
        match iri.strip_prefix(XSD_NS)? {
            "string" => Some(Self::String),
            "integer" => Some(Self::Integer),
            "boolean" => Some(Self::Boolean),
            "decimal" => Some(Self::Decimal),
            "date" => Some(Self::Date),
            _ => None,
        }
    }

    fn spelled(self) -> &'static str {
        match self {
            Self::String => "xsd:string",
            Self::Integer => "xsd:integer",
            Self::Boolean => "xsd:boolean",
            Self::Decimal => "xsd:decimal",
            Self::Date => "xsd:date",
        }
    }

    /// What a conforming value looks like, said to the person who wrote
    /// the wrong one.
    fn hint(self) -> &'static str {
        match self {
            Self::String => "text is a quoted string",
            Self::Integer => "an integer is written bare, with no fraction or exponent",
            Self::Boolean => "a boolean is a bare `true` or `false`",
            Self::Decimal => "a decimal is written bare, as `12` or `12.5`, with no exponent",
            Self::Date => "a date is a real calendar day, written `\"2026-11-01\"^^xsd:date`",
        }
    }

    /// Does one literal conform to this range? Its datatype must match and
    /// its text must lie in that datatype's lexical space. An `xsd:integer`
    /// literal matches `xsd:decimal` too: the one derivation read here.
    fn admits(self, literal: &Literal) -> bool {
        let Some(written) = Self::named(&literal.datatype) else { return false };
        let matches = written == self || (self == Self::Decimal && written == Self::Integer);
        matches && written.holds(&literal.value)
    }

    /// Does this text lie in the datatype's lexical space?
    fn holds(self, text: &str) -> bool {
        match self {
            Self::String => true,
            Self::Integer => digits(text.strip_prefix(['+', '-']).unwrap_or(text)),
            Self::Boolean => matches!(text, "true" | "false" | "1" | "0"),
            Self::Decimal => is_decimal(text),
            Self::Date => is_date(text),
        }
    }
}

/// One literal, the way a refusal names it.
fn describe_value(literal: &Literal) -> String {
    if let Some(written) = Datatype::named(&literal.datatype).filter(|written| !written.holds(&literal.value)) {
        return format!("{:?}^^{}", literal.value, written.spelled());
    }
    match literal.datatype.strip_prefix(XSD_NS) {
        Some("string") => format!("the string {:?}", literal.value),
        Some("boolean") => format!("the boolean `{}`", literal.value),
        Some("integer") => format!("the integer `{}`", literal.value),
        Some("decimal") | Some("double") => format!("the number `{}`", literal.value),
        Some("date") => format!("the date `{}`", literal.value),
        _ if literal.datatype == crate::colophon::RDF_JSON => "a JSON value".to_string(),
        _ => format!("`{}` typed `{}`", literal.value, literal.datatype),
    }
}

/// The refusal of one value, in the words a header and a block share.
fn contradiction(subject: &str, property: &str, literal: &Literal, datatype: Datatype) -> String {
    format!(
        "`{subject}` gives `{property}` {}, which contradicts its declared datatype `{}`: {}",
        describe_value(literal),
        datatype.spelled(),
        datatype.hint()
    )
}

<<lexical-spaces>>

/// Read every field of every instance against the vocabulary: a predicate
/// no module declares, a literal on an object property, a subject its
/// property's domain does not admit, and a literal its property's XSD range
/// contradicts, on any instance. One defect
/// per field, each at its block's line.
#[cfg(feature = "document-vocabulary")]
pub fn check_literal_fields(
    model: &OntologyModel,
    instances: &[crate::document_vocabulary::DeclaredInstance],
) -> Vec<DeclarationDefect> {
    use crate::inline_entity::Object;
    let vocabulary = Vocabulary::of(model);
    let spell = |iri: &str| model.compact(iri).unwrap_or_else(|| iri.to_string());
    let mut defects = Vec::new();
    for instance in instances {
        let subject = spell(&instance.iri);
        let mut reported: BTreeSet<&str> = BTreeSet::new();
        for (property, object) in &instance.entity.statements {
            let Object::Literal(literal) = object else { continue };
            if property.ends_with("requiresResources") {
                continue;
            }
            if !vocabulary.terms.contains(property) {
                if reported.insert(property.as_str()) {
                    let nearest = match nearest_declared(&vocabulary.terms, property) {
                        Some(term) => format!("; the nearest declared property is `{}`", spell(term)),
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
            if vocabulary.names_ids(property) {
                if reported.insert(property.as_str()) {
                    defects.push(DeclarationDefect::Instance {
                        reason: format!(
                            "`{subject}` gives `{}` a literal, but it names an id at {}",
                            spell(property),
                            instance.source
                        ),
                    });
                }
                continue;
            }
            if !vocabulary.admits_subject(property, &instance.concept) && reported.insert(property.as_str()) {
                defects.push(DeclarationDefect::Instance {
                    reason: format!("{property} does not accept {} at {}", instance.concept, instance.source),
                });
            }
            for datatype in vocabulary.contradicted(property, literal) {
                defects.push(DeclarationDefect::Instance {
                    reason: format!(
                        "{} at {}",
                        contradiction(&subject, &spell(property), literal, datatype),
                        instance.source
                    ),
                });
            }
        }
    }
    defects
}
```

The lexical spaces are read by hand. The crate is published, and these small
functions cost less than a dependency. An integer and a decimal are short
patterns. A date is the one with a rule no pattern states: month
lengths and leap years. A year may have any number of digits, so `is_date`
reads only its last four, which settle the leap rule because 10000 is a
multiple of 400. The sign of a year does not change that rule.

<a name="chunk-lexical-spaces"></a><sub>[`src/envelope_check.rs`](../../crates/x0k-folio/src/envelope_check.rs) · `#lexical-spaces`</sub>

```rust {#lexical-spaces}
/// One or more ASCII digits.
fn digits(text: &str) -> bool {
    !text.is_empty() && text.bytes().all(|byte| byte.is_ascii_digit())
}

/// `[+-]?([0-9]+(\.[0-9]*)?|\.[0-9]+)`: a decimal has no exponent.
fn is_decimal(text: &str) -> bool {
    let unsigned = text.strip_prefix(['+', '-']).unwrap_or(text);
    match unsigned.split_once('.') {
        Some(("", fraction)) => digits(fraction),
        Some((whole, "")) => digits(whole),
        Some((whole, fraction)) => digits(whole) && digits(fraction),
        None => digits(unsigned),
    }
}

/// `-?[0-9]{4,}-[0-9]{2}-[0-9]{2}` with an optional zone, naming a day the
/// calendar holds.
fn is_date(text: &str) -> bool {
    let Some((year, rest)) = text.strip_prefix('-').unwrap_or(text).split_once('-') else { return false };
    let (Some(month), Some("-"), Some(day), Some(zone)) = (rest.get(..2), rest.get(2..3), rest.get(3..5), rest.get(5..)) else {
        return false;
    };
    if year.len() < 4 || !digits(year) || !digits(month) || !digits(day) || !is_zone(zone) {
        return false;
    }
    let number = |text: &str| text.parse::<u32>().unwrap_or(0);
    (1..=days_in_month(number(&year[year.len() - 4..]), number(month))).contains(&number(day))
}

/// No zone, `Z`, or `[+-]hh:mm` up to the fourteen hours XSD allows.
fn is_zone(zone: &str) -> bool {
    let Some(offset) = zone.strip_prefix(['+', '-']) else { return zone.is_empty() || zone == "Z" };
    let Some((hours, minutes)) = offset.split_once(':') else { return false };
    if hours.len() != 2 || minutes.len() != 2 || !digits(hours) || !digits(minutes) {
        return false;
    }
    let number = |text: &str| text.parse::<u32>().unwrap_or(u32::MAX / 120);
    number(minutes) < 60 && number(hours) * 60 + number(minutes) <= 14 * 60
}

/// The days a month holds, in a year known by its last four digits; none
/// for a month that does not exist.
fn days_in_month(year: u32, month: u32) -> u32 {
    match month {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        2 if year.is_multiple_of(400) || (year.is_multiple_of(4) && !year.is_multiple_of(100)) => 29,
        2 => 28,
        _ => 0,
    }
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

The same holds one level down. A field's property, and a header literal's
term, is read for its range and not for its domain: `x0k:status` has an
`x0k:Decision` domain, and
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

    /// A subject two axioms under `x0k:Decision` is a decision's subject: the
    /// rule the guide states has no depth.
    #[test]
    fn a_readers_predicate_over_a_grandchild_of_decision_stands_as_a_document_edge() {
        use x0k_ontology::concept_facts::{OntologyFact, RDFS_DOMAIN, RDFS_SUBCLASS_OF};
        const MYCORP: &str = "https://mycorp.example/ontology#";
        let entity = |subject: &str, predicate: &str, value: String| OntologyFact {
            entity: format!("{MYCORP}{subject}"),
            predicate: predicate.to_string(),
            value: OntologyValue::Entity(value),
        };
        let tmp = tempfile::tempdir().expect("tempdir");
        let mut facts = scratch_vocabulary(&tmp.path().join("modules")).facts().to_vec();
        facts.push(entity("supersededBy", RDFS_DOMAIN, format!("{MYCORP}Brief")));
        facts.retain(|fact| !(fact.predicate == RDFS_DOMAIN && fact.value == OntologyValue::Entity(format!("{X0K_NS}Decision"))));
        facts.push(entity("Brief", RDFS_SUBCLASS_OF, format!("{MYCORP}Proposal")));
        facts.push(entity("Proposal", RDFS_SUBCLASS_OF, format!("{X0K_NS}Decision")));
        assert_eq!(
            predicate_standing(&OntologyModel::new(facts), "mycorp:supersededBy"),
            PredicateStanding::DocumentEdge { uri: "mycorp:supersededBy".to_string() }
        );
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
            [Defect::UndeclaredPredicate { predicate, .. }] => assert_eq!(predicate, "mycorp:shreds"),
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
        assert!(report.defects.iter().any(|d| matches!(d, Defect::UndeclaredPredicate { predicate, .. } if predicate == "x0k:notAPredicate")));
        assert!(report.defects.iter().any(|d| matches!(d, Defect::MalformedTarget { .. })));
    }

    /// The defects of one report, rendered as `check` prints them.
    fn rendered(report: &EnvelopeReport) -> Vec<String> {
        report.defects.iter().map(ToString::to_string).collect()
    }

    /// Found 2026-10-03: a literal statement whose term no module declares
    /// passed on a design, a class this crate names.
    #[test]
    fn a_literal_no_module_declares_is_refused_on_a_built_in_class() {
        let report = check_envelope(&shipped(), &doc("design:example", " ;\n    x0k:fooBar \"x\""));
        match rendered(&report).as_slice() {
            [one] => {
                assert!(one.contains("`x0k:fooBar`"), "names the term: {one}");
                assert!(one.contains("declared by no ontology module"), "{one}");
            }
            other => panic!("expected one refusal, got {other:?}"),
        }
    }

    /// Found the same day: on a class a loaded module declares (a
    /// principle, there), a misspelled literal term passed, and the
    /// refusal it should have met names the term it meant.
    #[test]
    fn a_literal_term_is_refused_on_a_class_a_loaded_module_declares() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let model = scratch_vocabulary(&tmp.path().join("modules"));
        let read = |statements: &str| {
            crate::colophon::parse_envelope_in(&model, &format!(
                "```turtle folio:document\nmycorp:brief\\/tender a mycorp:Brief ;\n    x0k:status \"proposed\"{statements} .\n```\nBody.\n"
            ))
            .expect("fixture parses")
            .0
        };
        let report = check_envelope(&model, &read(" ;\n    mycorp:owner \"procurement\""));
        assert!(report.is_clean(), "a declared literal term: {:?}", report.defects);
        match rendered(&check_envelope(&model, &read(" ;\n    mycorp:ownr \"procurement\""))).as_slice() {
            [one] => {
                assert!(one.contains("`mycorp:ownr`"), "names the term: {one}");
                assert!(one.contains("nearest declared term is `mycorp:owner`"), "names the nearest: {one}");
            }
            other => panic!("expected one refusal, got {other:?}"),
        }
        // An edge on the same class answers the same question.
        let report = check_envelope(&model, &read(" ;\n    mycorp:shreds mycorp:brief\\/other"));
        assert!(rendered(&report).iter().any(|one| one.contains("`mycorp:shreds`")), "{:?}", report.defects);
    }

    #[test]
    fn locating_a_refusal_reads_the_line_that_wrote_the_term() {
        let content = format!("# Example\n\n{}", header("design:example", " ;\n    x0k:summary \"s\" ;\n    x0k:fooBar \"x\""));
        let mut report = check_envelope(&shipped(), &envelope(&content));
        for defect in &mut report.defects {
            defect.locate(&content);
        }
        match report.defects.as_slice() {
            [Defect::UndeclaredLiteral { predicate, line, .. }] => {
                assert_eq!(predicate, "x0k:fooBar");
                assert_eq!(*line, Some(7), "the line of `x0k:fooBar` in the file");
            }
            other => panic!("expected one UndeclaredLiteral, got {other:?}"),
        }
        assert!(report.defects[0].to_string().contains("`x0k:fooBar` at line 7"));
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
<https://mycorp.example/ontology#owner> <http://www.w3.org/1999/02/22-rdf-syntax-ns#type> <http://www.w3.org/2002/07/owl#DatatypeProperty> .
<https://mycorp.example/ontology#owner> <http://www.w3.org/2000/01/rdf-schema#domain> <https://mycorp.example/ontology#Brief> .
<https://mycorp.example/ontology#owner> <http://www.w3.org/2000/01/rdf-schema#range> <http://www.w3.org/2001/XMLSchema#string> .
<https://mycorp.example/ontology#owner> <http://www.w3.org/2000/01/rdf-schema#isDefinedBy> <https://0k.computer/ontology/mycorp> .
<https://mycorp.example/ontology#due> <http://www.w3.org/1999/02/22-rdf-syntax-ns#type> <http://www.w3.org/2002/07/owl#DatatypeProperty> .
<https://mycorp.example/ontology#due> <http://www.w3.org/2000/01/rdf-schema#range> <http://www.w3.org/2001/XMLSchema#date> .
<https://mycorp.example/ontology#due> <http://www.w3.org/2000/01/rdf-schema#isDefinedBy> <https://0k.computer/ontology/mycorp> .
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
        assert_eq!(envelope.doc_type.as_str(), "brief");
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

    /// A set read the way `check` reads it: the instance pass, the header
    /// pass under the model it extended, then the two lists joined.
    #[cfg(feature = "document-vocabulary")]
    fn check_set(documents: &[(&'static str, String)]) -> (CorpusReport, DeclarationReport) {
        let sources: Vec<_> = documents
            .iter()
            .map(|(id, body)| crate::document_vocabulary::DocumentSource { id, body })
            .collect();
        let instances = check_instances(&OntologyModel::shipped(), &sources);
        let headers: Vec<(&str, Colophon)> = documents
            .iter()
            .map(|(name, body)| {
                let (header, _) = crate::colophon::parse_envelope_in(&instances.model, body).expect("fixture header parses");
                (*name, header)
            })
            .collect();
        let mut corpus = check_corpus(&instances.model, headers.iter().map(|(name, header)| (*name, header)));
        let mut declarations = instances.report;
        resolve_across(&instances.model, &instances.declared, &mut corpus, &mut declarations);
        (corpus, declarations)
    }

    /// The vocabulary, and a report document whose id is in the paper
    /// namespace. `report_edge` is Turtle appended to its header; `block` is
    /// the block `alpha.md` carries.
    #[cfg(feature = "document-vocabulary")]
    fn report_beside(report_edge: &str, block: &str) -> Vec<(&'static str, String)> {
        let mut documents = papers(block).to_vec();
        documents.push((
            "report.md",
            format!("# Report\n\n```turtle folio:document\npaper:paper\\/report a paper:Paper{report_edge} .\n```\n"),
        ));
        documents
    }

    #[cfg(feature = "document-vocabulary")]
    #[test]
    fn a_header_edge_to_a_declaration_stays_in_the_set() {
        let documents = report_beside(" ;\n    x0k:cites paper:paper\\/alpha", ALPHA_CITES_BETA);
        let (corpus, declarations) = check_set(&documents);
        assert_eq!(corpus.dangling, vec![], "the block is in the set");
        assert!(declarations.is_clean(), "{:?}", declarations.defects);
    }

    #[cfg(feature = "document-vocabulary")]
    #[test]
    fn a_block_edge_to_a_document_stays_in_the_set() {
        let block = ALPHA_CITES_BETA.replace("paper:paper\\/beta", "paper:paper\\/report");
        let (_, declarations) = check_set(&report_beside("", &block));
        assert_eq!(declarations.dangling, vec![], "the document is in the set");
        assert!(declarations.is_clean(), "{:?}", declarations.defects);
    }

    /// `paper:Note` beside `paper:Paper`, which `paper:cites` does not name,
    /// and `paper:Letter`, a kind of paper.
    #[cfg(feature = "document-vocabulary")]
    const NOTES: &str = "paper:Note a owl:Class ;\n    rdfs:isDefinedBy <https://example.org/paper-vocabulary> .\npaper:Letter a owl:Class ;\n    rdfs:subClassOf paper:Paper ;\n    rdfs:isDefinedBy <https://example.org/paper-vocabulary> .\n";

    /// The papers set, with the notes vocabulary, and one more document.
    #[cfg(feature = "document-vocabulary")]
    fn noted(block: &str, header: &str) -> (CorpusReport, DeclarationReport) {
        let mut documents = papers_with(NOTES, block).to_vec();
        documents.push(("report.md", format!("# Report\n\n```turtle folio:document\n{header} .\n```\n")));
        check_set(&documents)
    }

    #[cfg(feature = "document-vocabulary")]
    fn corpus_refusals(corpus: &CorpusReport) -> Vec<String> {
        corpus.defects.iter().map(|(_, defect)| defect.to_string()).collect()
    }

    #[cfg(feature = "document-vocabulary")]
    #[test]
    fn a_header_edge_outside_its_domain_is_refused_in_a_blocks_words() {
        let (corpus, _) = noted(ALPHA_CITES_BETA, "paper:note\\/n a paper:Note ;\n    paper:cites paper:paper\\/alpha");
        assert_eq!(corpus_refusals(&corpus), vec!["`paper:note/n`: `paper:cites` does not accept `paper:Note`"]);
    }

    #[cfg(feature = "document-vocabulary")]
    #[test]
    fn a_header_edge_to_a_document_outside_its_range_is_refused() {
        let mut documents = papers_with(NOTES, ALPHA_CITES_BETA).to_vec();
        documents.push(("note.md", "# Note\n\n```turtle folio:document\npaper:note\\/n a paper:Note .\n```\n".to_string()));
        documents.push(("report.md", "# Report\n\n```turtle folio:document\npaper:paper\\/report a paper:Paper ;\n    paper:cites paper:note\\/n .\n```\n".to_string()));
        let (corpus, _) = check_set(&documents);
        assert_eq!(corpus_refusals(&corpus), vec!["`paper:paper/report`: `paper:cites` target `paper:note/n` has type `paper:Note`, expected `paper:Paper`"]);
    }

    #[cfg(feature = "document-vocabulary")]
    #[test]
    fn a_header_edge_to_a_declaration_outside_its_range_is_refused() {
        let block = format!("{ALPHA_CITES_BETA}\n```turtle folio:graph\npaper:note\\/n a paper:Note .\n```\n");
        let (corpus, _) = noted(&block, "paper:paper\\/report a paper:Paper ;\n    paper:cites paper:note\\/n");
        assert_eq!(corpus_refusals(&corpus), vec!["`paper:paper/report`: `paper:cites` target `paper:note/n` has type `paper:Note`, expected `paper:Paper`"]);
    }

    #[cfg(feature = "document-vocabulary")]
    #[test]
    fn a_block_edge_to_a_document_outside_its_range_is_refused() {
        let block = ALPHA_CITES_BETA.replace("paper:paper\\/beta", "paper:note\\/n");
        let (_, declarations) = noted(&block, "paper:note\\/n a paper:Note");
        let reasons: Vec<String> = declarations.defects.iter().map(ToString::to_string).collect();
        assert_eq!(reasons.len(), 1, "{reasons:?}");
        assert!(reasons[0].contains("has type https://example.org/papers#Note, expected"), "{reasons:?}");
        assert!(reasons[0].contains("alpha.md"), "the refusal names its block: {reasons:?}");
    }

    /// The same walk a block gets: a subclass stands on either side.
    #[cfg(feature = "document-vocabulary")]
    #[test]
    fn a_header_edge_takes_a_subclass_on_either_side() {
        let (corpus, _) = noted(ALPHA_CITES_BETA, "paper:letter\\/l a paper:Letter ;\n    paper:cites paper:paper\\/alpha");
        assert!(corpus.is_clean(), "{:?}", corpus.defects);
    }

    /// A target outside the set has no class to read, and an open run stays open.
    #[cfg(feature = "document-vocabulary")]
    #[test]
    fn a_header_edge_leaving_the_set_is_dangling_and_not_refused() {
        let (corpus, _) = noted(ALPHA_CITES_BETA, "paper:paper\\/report a paper:Paper ;\n    paper:cites paper:paper\\/elsewhere");
        assert!(corpus.is_clean(), "{:?}", corpus.defects);
        assert_eq!(corpus.dangling.len(), 1);
    }

    /// An object property's value is an id, so text there is a mistake
    /// however well it reads, and a typo in it could never dangle.
    #[cfg(feature = "document-vocabulary")]
    #[test]
    fn a_literal_on_an_object_property_is_refused_on_both_surfaces() {
        let block = format!("{ALPHA_CITES_BETA}\n```turtle folio:graph\npaper:letter\\/c a paper:Letter ;\n    paper:cites \"beta\" .\n```\n");
        let (corpus, declarations) = noted(&block, "paper:paper\\/report a paper:Paper ;\n    paper:cites \"beta\"");
        assert_eq!(corpus_refusals(&corpus), vec!["`paper:paper/report` gives `paper:cites` a literal, but it names an id"]);
        let reasons: Vec<String> = declarations.defects.iter().map(ToString::to_string).collect();
        assert_eq!(reasons.len(), 1, "{reasons:?}");
        assert!(reasons[0].contains("`paper:letter/c` gives `paper:cites` a literal, but it names an id"), "{reasons:?}");
        assert!(reasons[0].contains("alpha.md"), "the refusal names its block: {reasons:?}");
    }

    /// `x0k:mentions` is soft by design: a reader's class may mention, with
    /// no place in the shipped hierarchy, and a mention stays a document edge.
    #[cfg(feature = "document-vocabulary")]
    #[test]
    fn any_class_may_mention() {
        let (corpus, _) = noted(ALPHA_CITES_BETA, "paper:note\\/n a paper:Note ;\n    x0k:mentions paper:paper\\/alpha");
        assert!(corpus.is_clean(), "{:?}", corpus.defects);
        assert_eq!(corpus.dangling, vec![], "the mention resolves");
        assert!(OntologyModel::shipped().decision_edge_predicates().iter().any(|(_, camel)| camel == "mentions"));
    }

    #[cfg(feature = "document-vocabulary")]
    #[test]
    fn a_header_literal_outside_its_terms_domain_is_refused() {
        let (corpus, _) = noted(ALPHA_CITES_BETA, "paper:note\\/n a paper:Note ;\n    paper:reviewed true");
        assert_eq!(corpus_refusals(&corpus), vec!["`paper:note/n`: `paper:reviewed` does not accept `paper:Note`"]);
    }

    #[cfg(feature = "document-vocabulary")]
    #[test]
    fn a_block_literal_outside_its_terms_domain_is_refused() {
        let block = format!("{ALPHA_CITES_BETA}\n```turtle folio:graph\npaper:note\\/n a paper:Note ;\n    paper:reviewed true .\n```\n");
        let (_, declarations) = noted(&block, "paper:paper\\/report a paper:Paper");
        let reasons: Vec<String> = declarations.defects.iter().map(ToString::to_string).collect();
        assert_eq!(reasons.len(), 1, "{reasons:?}");
        assert!(reasons[0].contains("does not accept https://example.org/papers#Note"), "{reasons:?}");
        assert!(reasons[0].contains("alpha.md"), "the refusal names its block: {reasons:?}");
    }

    #[cfg(feature = "document-vocabulary")]
    #[test]
    fn a_literal_takes_a_subclass_for_its_terms_domain() {
        let block = format!("{ALPHA_CITES_BETA}\n```turtle folio:graph\npaper:letter\\/b a paper:Letter ;\n    paper:reviewed true .\n```\n");
        let (corpus, declarations) = noted(&block, "paper:letter\\/h a paper:Letter ;\n    paper:reviewed true");
        assert!(corpus.is_clean(), "{:?}", corpus.defects);
        assert!(declarations.is_clean(), "{:?}", declarations.defects);
    }

    /// Resolution reaches both lists and nothing further: a misspelled id
    /// names neither, whichever side wrote it.
    #[cfg(feature = "document-vocabulary")]
    #[test]
    fn a_target_in_neither_list_still_dangles_from_either_side() {
        let block = ALPHA_CITES_BETA.replace("paper:paper\\/beta", "paper:paper\\/reportt");
        let documents = report_beside(" ;\n    x0k:cites paper:paper\\/alphaa", &block);
        let (corpus, declarations) = check_set(&documents);
        let header: Vec<String> = corpus.dangling.iter().map(|edge| edge.target.to_string()).collect();
        assert_eq!(header, vec!["paper:paper/alphaa"]);
        let block: Vec<&str> = declarations.dangling.iter().map(|edge| edge.target.as_str()).collect();
        assert_eq!(block, vec!["paper:paper/reportt"]);
    }

    #[cfg(feature = "document-vocabulary")]
    #[test]
    fn one_id_on_a_document_and_a_declaration_is_a_defect_naming_both() {
        let block = ALPHA_CITES_BETA.replace("paper:paper\\/alpha", "paper:paper\\/report");
        let (_, declarations) = check_set(&report_beside("", &block));
        match declarations.defects.as_slice() {
            [DeclarationDefect::IdCollision { id, document, declaration }] => {
                assert_eq!(id.to_string(), "paper:paper/report");
                assert_eq!(document, "report.md");
                assert!(declaration.starts_with("alpha.md:"), "{declaration}");
            }
            other => panic!("expected one IdCollision, got {other:?}"),
        }
        let rendered = declarations.defects[0].to_string();
        for part in ["`paper:paper/report`", "report.md", "alpha.md:"] {
            assert!(rendered.contains(part), "missing {part}: {rendered}");
        }
    }

    /// Ids compare by the IRI they expand to, so a second prefix for the
    /// namespace does not hide a collision.
    #[cfg(feature = "document-vocabulary")]
    #[test]
    fn a_collision_is_found_across_two_spellings_of_one_namespace() {
        let alias = "<https://example.org/paper-vocabulary> vann:preferredNamespacePrefix \"papers\" .\n";
        let mut documents = papers_with(alias, &ALPHA_CITES_BETA.replace("paper:paper\\/alpha", "papers:paper\\/report")).to_vec();
        documents.push(("report.md", "# Report\n\n```turtle folio:document\npaper:paper\\/report a paper:Paper .\n```\n".to_string()));
        let (_, declarations) = check_set(&documents);
        assert!(
            declarations.defects.iter().any(|defect| matches!(defect, DeclarationDefect::IdCollision { .. })),
            "{:?}",
            declarations.defects
        );
    }

    /// A block lost to a parse failure contributes no id, and what pointed
    /// at it is visible: the failure is reported at its source and the edge
    /// still dangles.
    #[cfg(feature = "document-vocabulary")]
    #[test]
    fn a_block_that_does_not_parse_contributes_no_id() {
        let block = ALPHA_CITES_BETA.replace("paper:paper\\/alpha a paper:Paper", "paper:monograph\\/alpha a paper:Monograph");
        let documents = report_beside(" ;\n    x0k:cites paper:monograph\\/alpha", &block);
        let (corpus, declarations) = check_set(&documents);
        assert!(only_defect(&declarations).contains("unknown concept paper:Monograph"));
        assert_eq!(corpus.dangling.len(), 1, "{:?}", corpus.dangling);
    }

    /// `check_instances` stops before its whole-set pass when a block fails,
    /// and the ids that parsed still have to reach the header pass.
    #[cfg(feature = "document-vocabulary")]
    #[test]
    fn a_failed_block_elsewhere_leaves_the_ids_that_parsed_in_the_set() {
        let mut documents = report_beside(" ;\n    x0k:cites paper:paper\\/alpha", ALPHA_CITES_BETA);
        documents.push((
            "broken.md",
            "# Broken\n\n```turtle folio:document\nwiki:broken a x0k:Wiki .\n```\n\n```turtle folio:graph\npaper:paper\\/broken a paper:Monograph .\n```\n".to_string(),
        ));
        let (corpus, declarations) = check_set(&documents);
        assert_eq!(corpus.dangling, vec![], "alpha parsed, and it is in the set");
        assert!(only_defect(&declarations).contains("broken.md:"));
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
        let cases: [(&str, &[&str], &[&str]); 5] = [
            ("venue", &["\"Proceedings\"", "\"a\", \"b\""], &["12", "true", "'{\"at\":\"home\"}'^^rdf:JSON"]),
            ("pages", &["12", "-3", "1, 2"], &["\"12\"", "12.5", "12.0", "true", "1, \"two\""]),
            ("reviewed", &["true", "false"], &["\"true\"", "1", "0", "\"not-a-boolean\""]),
            ("score", &["12.5", "-3.0", "12"], &["\"12.5\"", "1e3", "true"]),
            ("published", &["\"2026-11-01\"^^xsd:date"], &["\"2026-11-01\"", "20261101", "true"]),
        ];
        for (field, conforming, contradicting) in cases {
            let block = |value: &str| format!("```turtle folio:graph\npaper:paper\\/alpha a paper:Paper ;\n    paper:{field} {value} .\n```\n");
            for value in conforming {
                let report = check_collection(&papers_with(FIELDS, &block(value))).report;
                assert!(report.is_clean(), "{field}: {value} conforms: {:?}", report.defects);
            }
            for value in contradicting {
                let rendered = only_defect(&check_collection(&papers_with(FIELDS, &block(value))).report);
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
                            paper:homepage a owl:DatatypeProperty ;\n    rdfs:domain paper:Paper ;\n    \
                            rdfs:range xsd:anyURI ;\n    \
                            rdfs:isDefinedBy <https://example.org/paper-vocabulary> .\n";
        let block = "```turtle folio:graph\npaper:paper\\/alpha a paper:Paper ;\n    paper:note 12 ;\n    paper:homepage \"not a uri\" .\n```\n";
        let report = check_collection(&papers_with(OPEN, block)).report;
        assert!(report.is_clean(), "{:?}", report.defects);
    }

    /// A field's term is refused whatever its instance's class: a class no
    /// datatype property names as its domain — the shape every signifier
    /// had before `x0k:cue` was declared — is not a licence to misspell, and
    /// a range the vocabulary does declare is still read on it.
    #[cfg(feature = "document-vocabulary")]
    #[test]
    fn a_field_no_module_declares_is_refused_on_a_class_nothing_describes() {
        const NOTE: &str = "paper:Note a owl:Class ;\n    \
                            rdfs:isDefinedBy <https://example.org/paper-vocabulary> .\n\
                            paper:count a owl:DatatypeProperty ;\n    rdfs:range xsd:integer ;\n    \
                            rdfs:isDefinedBy <https://example.org/paper-vocabulary> .\n";
        let note = |extra: &str| format!("```turtle folio:graph\npaper:note\\/one a paper:Note ;\n    paper:count 3{extra} .\n```\n");
        let report = check_collection(&papers_with(NOTE, &note(""))).report;
        assert!(report.is_clean(), "a declared field on an undescribed class: {:?}", report.defects);
        let rendered = only_defect(&check_collection(&papers_with(NOTE, &note(" ;\n    paper:cue \"anything\""))).report);
        assert!(rendered.contains("states `paper:cue`"), "names the term: {rendered}");
        assert!(rendered.contains("alpha.md:"), "names the block's line: {rendered}");
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
    /// is a term `software` declares, so it passes.
    #[cfg(feature = "document-vocabulary")]
    #[test]
    fn the_shipped_string_range_is_read_and_a_signifiers_cue_passes() {
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

    /// A range of each kind the two passes have to agree about: the
    /// datatypes the check interprets that the papers vocabulary does not
    /// already declare, two it leaves uninterpreted, and none at all.
    #[cfg(feature = "document-vocabulary")]
    const FIELDS: &str = "paper:venue a owl:DatatypeProperty ;\n    rdfs:domain paper:Paper ;\n    \
                          rdfs:range xsd:string ;\n    \
                          rdfs:isDefinedBy <https://example.org/paper-vocabulary> .\n\
                          paper:published a owl:DatatypeProperty ;\n    rdfs:domain paper:Paper ;\n    \
                          rdfs:range xsd:date ;\n    \
                          rdfs:isDefinedBy <https://example.org/paper-vocabulary> .\n\
                          paper:score a owl:DatatypeProperty ;\n    rdfs:domain paper:Paper ;\n    \
                          rdfs:range xsd:decimal ;\n    \
                          rdfs:isDefinedBy <https://example.org/paper-vocabulary> .\n\
                          paper:homepage a owl:DatatypeProperty ;\n    rdfs:domain paper:Paper ;\n    \
                          rdfs:range xsd:anyURI ;\n    \
                          rdfs:isDefinedBy <https://example.org/paper-vocabulary> .\n\
                          paper:seen a owl:DatatypeProperty ;\n    rdfs:domain paper:Paper ;\n    \
                          rdfs:range xsd:dateTime ;\n    \
                          rdfs:isDefinedBy <https://example.org/paper-vocabulary> .\n\
                          paper:note a owl:DatatypeProperty ;\n    rdfs:domain paper:Paper ;\n    \
                          rdfs:isDefinedBy <https://example.org/paper-vocabulary> .\n";

    /// One statement in the header of a paper, read as `check` reads it: the
    /// set's vocabulary first, then the header against that vocabulary.
    #[cfg(feature = "document-vocabulary")]
    fn check_header(statement: &str) -> EnvelopeReport {
        let model = check_collection(&papers_with(FIELDS, ALPHA_CITES_BETA)).model;
        let document = format!("```turtle folio:document\npaper:paper\\/alpha a paper:Paper ;\n    {statement} .\n```\nBody.\n");
        let (header, _) = crate::colophon::parse_envelope_in(&model, &document).expect("fixture parses");
        check_envelope(&model, &header)
    }

    /// The refusals of one statement, in the words a header and a block
    /// share, with the two surfaces held equal: every caller proves both.
    #[cfg(feature = "document-vocabulary")]
    fn verdict(statement: &str) -> Vec<String> {
        let words = |refusal: String| {
            let refusal = refusal.strip_prefix("declaration does not hold: ").unwrap_or(&refusal);
            refusal.split(" at ").next().unwrap_or(refusal).to_string()
        };
        let block = format!("```turtle folio:graph\npaper:paper\\/alpha a paper:Paper ;\n    {statement} .\n```\n");
        let report = check_collection(&papers_with(FIELDS, &block)).report;
        let in_block: Vec<String> = report.defects.iter().map(|defect| words(defect.to_string())).collect();
        let in_header: Vec<String> = rendered(&check_header(statement)).into_iter().map(words).collect();
        assert_eq!(in_block, in_header, "{statement}");
        in_block
    }

    #[cfg(feature = "document-vocabulary")]
    #[test]
    fn a_header_literal_its_range_contradicts_is_refused_naming_property_value_and_datatype() {
        match rendered(&check_header("paper:reviewed \"yes\"")).as_slice() {
            [one] => {
                for part in ["`paper:paper/alpha`", "`paper:reviewed`", "\"yes\"", "`xsd:boolean`"] {
                    assert!(one.contains(part), "missing {part}: {one}");
                }
            }
            other => panic!("expected one refusal, got {other:?}"),
        }
    }

    /// A value a block refuses, a header refuses in the same words, and a
    /// value a block lets through, a header lets through: one rule, read
    /// from two places.
    #[cfg(feature = "document-vocabulary")]
    #[test]
    fn a_header_and_a_block_give_one_value_the_same_verdict() {
        for statement in [
            "paper:reviewed true",
            "paper:reviewed \"true\"",
            "paper:reviewed 1",
            "paper:pages 12",
            "paper:pages \"12\"",
            "paper:pages 12.5",
            "paper:pages 1, \"two\"",
            "paper:venue \"Proceedings\"",
            "paper:venue 12",
            "paper:venue '{\"at\":\"home\"}'^^rdf:JSON",
            "paper:published \"not a date\"",
            "paper:published \"not a date\"^^xsd:date",
            "paper:note 12",
        ] {
            verdict(statement);
        }
    }

    /// A value outside its range is refused on both surfaces in one
    /// sentence, and the sentence is true of the value: a literal typed
    /// correctly is named as written, never as the wrong datatype, and the
    /// hint spells the right form.
    #[cfg(feature = "document-vocabulary")]
    #[test]
    fn a_value_outside_its_range_is_refused_in_one_true_sentence() {
        const DATE: (&str, &str) = ("xsd:date", "a date is a real calendar day, written `\"2026-11-01\"^^xsd:date`");
        const DECIMAL: (&str, &str) = ("xsd:decimal", "a decimal is written bare, as `12` or `12.5`, with no exponent");
        const INTEGER: (&str, &str) = ("xsd:integer", "an integer is written bare, with no fraction or exponent");
        const BOOLEAN: (&str, &str) = ("xsd:boolean", "a boolean is a bare `true` or `false`");
        for (statement, value, (datatype, hint)) in [
            ("paper:published \"not a date\"^^xsd:date", "\"not a date\"^^xsd:date", DATE),
            ("paper:published \"not a date\"", "the string \"not a date\"", DATE),
            ("paper:reviewed \"yes\"^^xsd:boolean", "\"yes\"^^xsd:boolean", BOOLEAN),
            ("paper:pages \"twelve\"^^xsd:integer", "\"twelve\"^^xsd:integer", INTEGER),
            ("paper:published \"2026-11-01\"", "the string \"2026-11-01\"", DATE),
            ("paper:published \"2026-02-30\"^^xsd:date", "\"2026-02-30\"^^xsd:date", DATE),
            ("paper:published \"2026-13-01\"^^xsd:date", "\"2026-13-01\"^^xsd:date", DATE),
            ("paper:published \"2025-02-29\"^^xsd:date", "\"2025-02-29\"^^xsd:date", DATE),
            ("paper:score \"1e3\"^^xsd:decimal", "\"1e3\"^^xsd:decimal", DECIMAL),
            ("paper:score 1e3", "the number `1e3`", DECIMAL),
            ("paper:pages \"2026-11-01\"^^xsd:date", "the date `2026-11-01`", INTEGER),
            ("paper:pages \"yes\"^^xsd:boolean", "\"yes\"^^xsd:boolean", INTEGER),
        ] {
            let (term, _) = statement.split_once(' ').expect("a statement has a term");
            let refusal = format!("`paper:paper/alpha` gives `{term}` {value}, which contradicts its declared datatype `{datatype}`: {hint}");
            assert_eq!(verdict(statement), [refusal], "{statement}");
        }
    }

    /// A value inside its range passes on both surfaces, in every spelling
    /// its lexical space holds.
    #[cfg(feature = "document-vocabulary")]
    #[test]
    fn a_value_inside_its_range_passes_in_every_spelling_of_its_lexical_space() {
        for statement in [
            "paper:published \"2026-11-01\"^^xsd:date",
            "paper:published \"2024-02-29\"^^xsd:date",
            "paper:published \"2000-02-29\"^^xsd:date",
            "paper:published \"2026-11-01Z\"^^xsd:date",
            "paper:published \"2026-11-01+05:30\"^^xsd:date",
            "paper:published \"2026-11-01-14:00\"^^xsd:date",
            "paper:published \"-0044-03-15\"^^xsd:date",
            "paper:published \"12026-11-01\"^^xsd:date",
            "paper:score 12.5",
            "paper:score \"12.5\"^^xsd:decimal",
            "paper:score \"-.5\"^^xsd:decimal",
            "paper:score \"+12.\"^^xsd:decimal",
            "paper:score \"12\"^^xsd:decimal",
            "paper:pages 12",
            "paper:pages \"12\"^^xsd:integer",
            "paper:pages \"+12\"^^xsd:integer",
            "paper:pages \"-012\"^^xsd:integer",
            "paper:reviewed true",
            "paper:reviewed \"true\"^^xsd:boolean",
            "paper:reviewed \"false\"^^xsd:boolean",
            "paper:reviewed \"1\"^^xsd:boolean",
            "paper:reviewed \"0\"^^xsd:boolean",
            "paper:venue \"\"",
        ] {
            assert_eq!(verdict(statement), Vec::<String>::new(), "{statement}");
        }
    }

    /// Each lexical space ends where the datatype says it does: a calendar
    /// that knows its months and centuries, a sign that needs a digit, a
    /// zone with a real offset.
    #[cfg(feature = "document-vocabulary")]
    #[test]
    fn the_text_of_a_typed_literal_is_refused_past_the_edge_of_its_lexical_space() {
        for statement in [
            "paper:published \"1900-02-29\"^^xsd:date",
            "paper:published \"2026-04-31\"^^xsd:date",
            "paper:published \"2026-00-10\"^^xsd:date",
            "paper:published \"2026-01-00\"^^xsd:date",
            "paper:published \"2026-1-01\"^^xsd:date",
            "paper:published \"2026-01-1\"^^xsd:date",
            "paper:published \"026-01-01\"^^xsd:date",
            "paper:published \"+2026-01-01\"^^xsd:date",
            "paper:published \"2026-01-01z\"^^xsd:date",
            "paper:published \"2026-01-01+0530\"^^xsd:date",
            "paper:published \"2026-01-01+14:01\"^^xsd:date",
            "paper:published \"2026-01-01+05:60\"^^xsd:date",
            "paper:published \"2026-11-01T00:00:00\"^^xsd:date",
            "paper:published \"\"^^xsd:date",
            "paper:score \".\"^^xsd:decimal",
            "paper:score \"\"^^xsd:decimal",
            "paper:score \"1.2.3\"^^xsd:decimal",
            "paper:score \"INF\"^^xsd:decimal",
            "paper:pages \"\"^^xsd:integer",
            "paper:pages \"-\"^^xsd:integer",
            "paper:pages \"1.0\"^^xsd:integer",
            "paper:reviewed \"TRUE\"^^xsd:boolean",
            "paper:reviewed \"2\"^^xsd:boolean",
            "paper:reviewed \"\"^^xsd:boolean",
        ] {
            match verdict(statement).as_slice() {
                [one] => assert!(one.contains("contradicts its declared datatype"), "{statement}: {one}"),
                other => panic!("{statement}: expected one refusal, got {other:?}"),
            }
        }
    }

    /// An integer is a decimal, and that one derivation is the only one: an
    /// integer literal must still be an integer, and a decimal never stands
    /// in for an integer.
    #[cfg(feature = "document-vocabulary")]
    #[test]
    fn an_integer_stands_on_a_decimal_range_and_nothing_else_stands_for_another() {
        for statement in ["paper:score 12", "paper:score -3", "paper:score \"12\"^^xsd:integer"] {
            assert_eq!(verdict(statement), Vec::<String>::new(), "{statement}");
        }
        for statement in [
            "paper:score \"twelve\"^^xsd:integer",
            "paper:score \"12.5\"^^xsd:integer",
            "paper:pages 12.0",
            "paper:pages \"12\"^^xsd:decimal",
            "paper:venue \"12\"^^xsd:integer",
            "paper:published 12",
        ] {
            assert_eq!(verdict(statement).len(), 1, "{statement}");
        }
    }

    /// A range the check does not interpret, or no range, says nothing
    /// about a value, on either surface and in any spelling.
    #[cfg(feature = "document-vocabulary")]
    #[test]
    fn an_uninterpreted_range_refuses_nothing_on_either_surface() {
        for statement in [
            "paper:homepage \"not a uri\"^^xsd:anyURI",
            "paper:homepage \"not a uri\"",
            "paper:homepage 12",
            "paper:seen \"not a time\"^^xsd:dateTime",
            "paper:seen \"2026-11-01\"",
            "paper:note \"not a date\"^^xsd:date",
            "paper:note \"yes\"^^xsd:boolean",
        ] {
            assert_eq!(verdict(statement), Vec::<String>::new(), "{statement}");
        }
    }

    /// A property with no declared range says nothing about a value, in a
    /// header as in a block.
    #[cfg(feature = "document-vocabulary")]
    #[test]
    fn a_header_literal_with_no_declared_range_passes() {
        for statement in ["paper:note \"anything\"", "paper:note 12", "paper:note true"] {
            let report = check_header(statement);
            assert!(report.is_clean(), "{statement}: {:?}", report.defects);
        }
    }

    #[cfg(feature = "document-vocabulary")]
    #[test]
    fn a_well_formed_date_passes_in_a_header() {
        let report = check_header("paper:published \"2026-11-01\"^^xsd:date");
        assert!(report.is_clean(), "{:?}", report.defects);
    }

    /// The header rule needs no document vocabulary: a module directory
    /// declares the range, and the refusal reads as a block's does, at the
    /// line that wrote the term.
    #[test]
    fn a_header_value_is_refused_at_the_line_that_wrote_it() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let model = scratch_vocabulary(&tmp.path().join("modules"));
        let content = "# Brief\n\n```turtle folio:document\nmycorp:brief\\/tender a mycorp:Brief ;\n    x0k:status \"proposed\" ;\n    mycorp:owner 3 .\n```\nBody.\n";
        let (header, _) = crate::colophon::parse_envelope_in(&model, content).expect("fixture parses");
        let mut report = check_envelope(&model, &header);
        for defect in &mut report.defects {
            defect.locate(content);
        }
        match rendered(&report).as_slice() {
            [one] => assert_eq!(
                one,
                "`mycorp:brief/tender` gives `mycorp:owner` the integer `3`, which contradicts its declared datatype `xsd:string`: text is a quoted string at line 6"
            ),
            other => panic!("expected one refusal, got {other:?}"),
        }
    }

    /// The text is read with no document vocabulary either: a date a module
    /// declares answers on a header alone.
    #[test]
    fn a_header_date_is_read_by_its_text_against_a_module_range() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let model = scratch_vocabulary(&tmp.path().join("modules"));
        let refusals = |value: &str| {
            let content = format!("```turtle folio:document\nmycorp:brief\\/tender a mycorp:Brief ;\n    mycorp:due {value} .\n```\nBody.\n");
            let (header, _) = crate::colophon::parse_envelope_in(&model, &content).expect("fixture parses");
            rendered(&check_envelope(&model, &header))
        };
        assert_eq!(refusals("\"2024-02-29\"^^xsd:date"), Vec::<String>::new());
        for value in ["\"2025-02-29\"^^xsd:date", "\"2024-02-29\""] {
            match refusals(value).as_slice() {
                [one] => assert!(one.contains("contradicts its declared datatype `xsd:date`"), "{one}"),
                other => panic!("{value}: expected one refusal, got {other:?}"),
            }
        }
    }
}
`````

## Composing the module

<a name="chunk-root"></a><sub>[`src/envelope_check.rs`](../../crates/x0k-folio/src/envelope_check.rs) · `#root` · assembles [module-doc](#chunk-module-doc) · [standing](#chunk-standing) · [vocabulary](#chunk-vocabulary) · [defect](#chunk-defect) · [check-envelope](#chunk-check-envelope) · [check-corpus](#chunk-check-corpus) · [check-declarations](#chunk-check-declarations) · [check-instances](#chunk-check-instances) · [resolve-across](#chunk-resolve-across) · [check-literals](#chunk-check-literals) · [tests](#chunk-tests)</sub>

```rust {#root}
<<module-doc>>

<<standing>>

<<vocabulary>>

<<defect>>

<<check-envelope>>

<<check-corpus>>

<<check-declarations>>

<<check-instances>>

<<resolve-across>>

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
