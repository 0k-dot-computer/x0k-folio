# Entities authored inside prose

```turtle folio:document
implementation:folio\/inline-entities a x0k:Implementation ;
    x0k:status "draft" ;
    x0k:summary "Pulling an entity that was authored inside a document's prose back out of it — the section is the record, the heading is the title, the graph block is its statements, and the extractor reads declarations without resolving them." ;
    x0k:concerns "folio", "affordances", "extraction", "publishing", "markdown" ;
    x0k:cites architecture:publication-projection,
        implementation:folio\/colophon,
        implementation:folio\/identity,
        implementation:folio\/segmentation ;
    x0k:implements design:publish-a-region-as-a-repository ;
    folio:tangleCrate "crates/x0k-folio" ;
    folio:tangleRoot "src/inline_entity.rs" .
```

A design document does not merely mention the affordances it defines; it
*defines* them, in place, as a run of sections:

````markdown
### Publish a region as a repository

I project a demarcated region of my graph outward as a standalone
repository: its literate documents and the code they tangle to,
committed so anyone can clone, read, and build it.

```turtle folio:graph
affordance:publish_region_as_repository a x0k:Affordance ;
    x0k:status "wip" ;
    x0k:claimedFor actor:human ;
    x0k:requires affordance:demarcate_publication .
```
````

The prose is the affordance's description, the heading is its title, and
the `turtle folio:graph` block is its statements, in the vocabulary's own
terms and the document's one data language
(`x0k:architecture/filesystem-graph-materialization` §4). Nothing about that
arrangement is a convenience: an affordance exists *because* a design brought
it into being, so putting the declaration anywhere else would let the two
drift. The cost is that reading the affordance means reading the document, and
this module is what does the reading.

The example above sits inside a `markdown` fence, which is how this chapter
shows the grammar without performing it: a fence nested in another fence is
prose, and a document about the syntax does not ship the affordance it
describes.

It is one pure function — bytes in, structured records out. No store, no
socket, no resolution. That is why it belongs to the format library and not
to the daemon it grew up in: the question "what does this document declare?"
is document semantics, answerable by anyone holding the document.

## The extractor extracts; it does not resolve

One statement forces the boundary. An affordance may declare
`x0k:requiresResources` — a placement demand, saying the capability needs two
CPU cores or a GPU or a Linux host. Turning that into typed fleet values is a
different act from reading it: it means knowing what a `ResourceKind` is,
which arches exist, which currencies and token kinds the platform recognizes.

So the split runs through that statement. This module reads the demands and
hands them back **as declared** — JSON objects, unresolved. Interpreting them
is the host's job. The rule stated generally: *the extractor extracts; it
does not resolve.* The shape is still checked, because shape is grammar: each
demand is an `rdf:JSON` object, and a document that writes a bare string there
is malformed in a way any reader can see.

<a name="chunk-module-doc"></a><sub>[`src/inline_entity.rs`](../../crates/x0k-folio/src/inline_entity.rs) · `#module-doc`</sub>

```rust {#module-doc}
//! Inline-entity extraction for folio bodies.
//!
//! Walks the markdown body of a folio document looking for fenced blocks
//! marked `turtle folio:graph` that describe one instance of a class. Each
//! is an *inline entity* — an entity authored inside its parent document
//! rather than in a file of its own. The motivating case is an
//! `affordance` defined within its parent `design`, but the mechanism is
//! class-agnostic.
//!
//! ## Section-per-entity demarcation
//!
//! Each inline entity owns the heading section that encloses its block:
//!
//! - **Heading text = entity title.** The block does not state a title;
//!   the heading provides it.
//! - **All prose under the heading until the next heading at any level =
//!   description.** Graph blocks and the header are excised from it.
//! - **Exactly one block per class per section.** A second of the same
//!   class is an error against that block, not against the section.
//! - **No enclosing heading** is an error — an inline entity needs a title.
//!
//! A graph block that declares vocabulary (a class, a property, a module)
//! is not an instance and is left to the vocabulary loader. One class is
//! drawn rather than stated: an icon is an `svg x0k:icon` block
//! (`x0k:design/icon-profile`), whose record carries the drawing.
//!
//! ## What it does not do
//!
//! Pure: bytes in, records out. No IO, no global state, no resolution.
//! `x0k:requiresResources` is handed back as declared JSON, because turning
//! a placement demand into typed fleet values is the host's judgment.

use std::collections::{BTreeMap, HashSet};
use std::ops::Range;

use oxrdf::{NamedOrBlankNode, Term};
use pulldown_cmark::{CodeBlockKind, Event, Options, Parser, Tag, TagEnd};
use tracing::debug;
use x0k_ontology::concept_facts::{camel_to_kebab, OntologyModel, RDF_TYPE, X0K_NS};

use crate::colophon::{
    compact_iri, is_marker, parse_turtle, predeclared_prefixes, shipped_prefixes, Literal,
    FOLIO_NS, GRAPH_MARKER, HEADER_MARKER, OWL_NS, RDFS_NS, RDF_JSON, XSD_BOOLEAN, XSD_DECIMAL,
    XSD_DOUBLE, XSD_INTEGER, XSD_STRING,
};
use crate::entity_id::EntityId;
use crate::structural_block::FenceInfo;
use crate::transclusion::heading_slug;
```

## The record

The block states the instance's identity as its subject and its class with
`a`; the record keeps both, plus every other statement in order. The class
is carried twice, as the IRI the block names and as the kebab-case of its
local name (`affordance`), because the second is the word callers filter and
dispatch on and the first is what the vocabulary loader checks.

<a name="chunk-inline-entity"></a><sub>[`src/inline_entity.rs`](../../crates/x0k-folio/src/inline_entity.rs) · `#inline-entity`</sub>

```rust {#inline-entity}
/// The one class whose block is a drawing: `svg x0k:icon`, in the icon
/// profile. Its id is the section's, `x0k:icon/<heading anchor>`.
pub const ICON_CLASS: &str = "icon";

/// The object of one statement: another entity, or a literal.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Object {
    Iri(String),
    Literal(Literal),
}

/// A single inline entity extracted from a parent document's body.
#[derive(Debug, Clone, PartialEq)]
pub struct InlineEntity {
    /// Identity: the block's subject.
    pub uri: EntityId,
    /// Heading text of the enclosing section.
    pub title: String,
    /// Prose under that heading, with graph blocks excised. Trimmed.
    pub description: String,
    /// The class the block states, as the kebab-case of its local name
    /// (`affordance`); [`ICON_CLASS`] for a drawn mark.
    pub class: String,
    /// The class's IRI. Empty for an icon, which states none.
    pub class_iri: String,
    /// Every statement about the entity other than its class, in order:
    /// the predicate's IRI and the object.
    pub statements: Vec<(String, Object)>,
    /// An icon's drawing, as written.
    pub svg: Option<String>,
    /// `x0k:requiresResources` demands, **as declared**: each a JSON object.
    pub requires_resources: Vec<serde_json::Map<String, serde_json::Value>>,
    /// The block's byte range in the text the caller passed.
    pub span: Range<usize>,
    /// 1-based line of the block's opening fence in that text.
    pub line: usize,
}
```

## Errors, per block

A malformed block disqualifies itself and nothing else: one bad affordance in
a design does not cost the document its other seven. So extraction returns a
`Result` per attempted record, and every variant carries enough to name the
offending section.

One corrects rather than only refusing. The block's class and its id's class
segment are the same word in two spellings — `a paper:Paper` and
`paper:paper\/alpha` — so a mismatch that is only a casing difference is a
near miss with one intended spelling, and `ClassMismatch` says which.

<a name="chunk-error"></a><sub>[`src/inline_entity.rs`](../../crates/x0k-folio/src/inline_entity.rs) · `#error`</sub>

```rust {#error}
/// Extraction error. Non-fatal at the document level: the caller logs and
/// skips per record.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InlineEntityError {
    /// A block with no enclosing heading. Inline entities own a heading
    /// section; without one there is no title.
    MissingHeading { class: String },
    /// A second block of one class under the same heading.
    MultipleBlocksInSection { class: String, heading: String },
    /// The block is not Turtle; `line` is the line in the caller's text.
    InvalidTurtle { line: usize, reason: String },
    /// A block that declares no vocabulary declares exactly one instance:
    /// one named subject stating one class.
    NotOneInstance { line: usize, reason: String },
    /// The subject is not a well-formed entity id.
    InvalidUri { class: String, heading: String, value: String, reason: String },
    /// The id's class segment disagrees with the class the block states.
    ClassMismatch { class: String, heading: String, uri_class: String },
    /// The block stated `definedIn`. That edge is implicit from embedding.
    ExplicitDefinedIn { class: String, heading: String },
    /// The block stated a title. The heading is the title.
    DuplicateTitle { class: String, heading: String },
    /// `x0k:requiresResources` is not a JSON object. A host that interprets
    /// the demands reuses this variant when a well-shaped object still fails
    /// its own reading.
    InvalidResources { class: String, heading: String, reason: String },
}

impl std::fmt::Display for InlineEntityError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::MissingHeading { class } => write!(
                f,
                "`{class}` block has no enclosing heading; the section-per-entity rule requires a heading above the block"
            ),
            Self::MultipleBlocksInSection { class, heading } => write!(
                f,
                "more than one `{class}` block in section `{heading}`; one entity per section"
            ),
            Self::InvalidTurtle { line, reason } => {
                write!(f, "the graph block at line {line} is not Turtle: {reason}")
            }
            Self::NotOneInstance { line, reason } => write!(
                f,
                "the graph block at line {line} declares neither vocabulary nor one instance: {reason}"
            ),
            Self::InvalidUri { class, heading, value, reason } => write!(
                f,
                "`{class}` block in section `{heading}` has invalid id `{value}`: {reason}"
            ),
            Self::ClassMismatch { class, heading, uri_class } => {
                write!(
                    f,
                    "`{class}` block in section `{heading}` carries a `{uri_class}` id; the id's class and the stated class must agree"
                )?;
                if class.eq_ignore_ascii_case(uri_class) {
                    write!(f, " — they differ only in case: the id's class is the class's local name in kebab-case, `{class}`")?;
                }
                Ok(())
            }
            Self::ExplicitDefinedIn { class, heading } => write!(
                f,
                "`{class}` block in section `{heading}` stated `definedIn` — that edge is implicit from embedding and must not be stated"
            ),
            Self::DuplicateTitle { class, heading } => write!(
                f,
                "`{class}` block in section `{heading}` states a title — the heading is the title"
            ),
            Self::InvalidResources { class, heading, reason } => write!(
                f,
                "`{class}` block in section `{heading}` has invalid resource requirements: {reason}"
            ),
        }
    }
}

impl std::error::Error for InlineEntityError {}
```

## Reading a graph block

A graph block is classified by what it types. A block that types a subject as
an OWL term or module (`a owl:Class`, `a owl:Ontology`, …) or an RDFS datatype
declares vocabulary, and is the vocabulary loader's
([`document-vocabulary.md`](document-vocabulary.md)); this module passes over
it. A block whose every subject is a `folio:Parameter` is a document's
parameter panel — tool configuration, read by [`read_parameters`], not an
instance. Every other block declares exactly one instance: one named subject,
one class stated with `a`, no blank nodes. Holding a block to one instance is
what lets the section rule say whose heading and prose they are.

<a name="chunk-read-graph-block"></a><sub>[`src/inline_entity.rs`](../../crates/x0k-folio/src/inline_entity.rs) · `#read-graph-block`</sub>

```rust {#read-graph-block}
/// What one `turtle folio:graph` block holds.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GraphContent {
    /// Vocabulary: the block types a subject as an OWL term, a module, or
    /// an RDFS datatype.
    Definitions,
    /// A parameter panel: every subject is a `folio:Parameter`
    /// ([`read_parameters`] reads it).
    Parameters,
    /// One instance: its subject, its class, and every other statement.
    Instance { subject: String, class: String, statements: Vec<(String, Object)> },
}

/// True when `class` is a term of the vocabulary language itself rather
/// than a class of things.
fn is_vocabulary_type(class: &str) -> bool {
    class.starts_with(OWL_NS) || class.starts_with(RDFS_NS)
}

/// Classify and read one graph block. `first_line` is the line of the
/// block's first Turtle line in the caller's text.
pub fn read_graph_block(
    text: &str,
    prefixes: &[(String, String)],
    first_line: usize,
) -> Result<GraphContent, InlineEntityError> {
    let triples = parse_turtle(text, prefixes, first_line).map_err(|error| match error {
        crate::colophon::FolioError::Turtle { line, reason } => InlineEntityError::InvalidTurtle { line, reason },
        other => InlineEntityError::InvalidTurtle { line: first_line, reason: other.to_string() },
    })?;
    let fence_line = first_line.saturating_sub(1);
    let defines = triples.iter().any(|t| {
        t.predicate.as_str() == RDF_TYPE
            && matches!(&t.object, Term::NamedNode(class) if is_vocabulary_type(class.as_str()))
    });
    if defines {
        return Ok(GraphContent::Definitions);
    }
    if is_parameter_block(&triples) {
        return Ok(GraphContent::Parameters);
    }
    let not_one = |reason: &str| InlineEntityError::NotOneInstance { line: fence_line, reason: reason.to_string() };
    let mut subject: Option<String> = None;
    let mut classes: Vec<String> = Vec::new();
    let mut statements: Vec<(String, Object)> = Vec::new();
    for triple in &triples {
        let this = match &triple.subject {
            NamedOrBlankNode::NamedNode(node) => node.as_str().to_string(),
            NamedOrBlankNode::BlankNode(_) => return Err(not_one("a blank node")),
        };
        match &subject {
            Some(first) if *first != this => return Err(not_one("more than one subject")),
            Some(_) => {}
            None => subject = Some(this),
        }
        let object = match &triple.object {
            Term::NamedNode(node) if triple.predicate.as_str() == RDF_TYPE => {
                classes.push(node.as_str().to_string());
                continue;
            }
            Term::NamedNode(node) => Object::Iri(node.as_str().to_string()),
            Term::BlankNode(_) => return Err(not_one("a blank node")),
            Term::Literal(literal) => {
                if literal.language().is_some() {
                    return Err(not_one("a language-tagged literal"));
                }
                Object::Literal(Literal {
                    value: literal.value().to_string(),
                    datatype: literal.datatype().as_str().to_string(),
                })
            }
        };
        statements.push((triple.predicate.as_str().to_string(), object));
    }
    let subject = subject.ok_or_else(|| not_one("no statement"))?;
    let class = match classes.as_slice() {
        [one] => one.clone(),
        [] => return Err(not_one("no class stated with `a`")),
        _ => return Err(not_one("more than one class stated with `a`")),
    };
    Ok(GraphContent::Instance { subject, class, statements })
}

/// The class a parameter panel's subjects state.
pub fn parameter_class() -> String {
    format!("{FOLIO_NS}Parameter")
}

/// True when the block types something, and every subject it types is a
/// `folio:Parameter`.
fn is_parameter_block(triples: &[oxrdf::Triple]) -> bool {
    let class = parameter_class();
    let typed: Vec<&oxrdf::Triple> = triples.iter().filter(|t| t.predicate.as_str() == RDF_TYPE).collect();
    !typed.is_empty()
        && typed.iter().all(|t| matches!(&t.object, Term::NamedNode(node) if node.as_str() == class))
}

/// The kebab-case of an IRI's local name: `…#OpenQuestion` is `open-question`.
pub fn class_name(class_iri: &str) -> String {
    camel_to_kebab(class_iri.rsplit(['#', '/']).next().unwrap_or(class_iri))
}
```

## A document's parameters

A literate page that drives a live figure declares the figure's knobs in a
parameter panel: one `folio:Parameter` per knob, its label and help text
under `rdfs:label` and `rdfs:comment`, its kind (`float`, `int`, `uint`,
`bool`, …) under `folio:kind`, its range under `folio:min`, `folio:max` and
`folio:step`, and its starting value under `folio:default`.

```turtle
x0k:implementation\/canvas\/core\/num_levels a folio:Parameter ;
    rdfs:label "LOD Levels" ;
    rdfs:comment "Number of detail levels" ;
    folio:kind "uint" ;
    folio:min 2.0 ;
    folio:max 8.0 ;
    folio:default 4 .
```

A parameter's subject is the document's id with the parameter's id as one
more path segment, written with `x0k:`, so the panel reads with the fixed
prefixes alone — a guest that renders the figure needs no vocabulary to read
its own knobs. The panel keeps the order the block states its parameters in,
because that is the order a person laid the controls out in.
[`Parameter::to_json`] is the panel's wire shape, the one a figure guest and
the woven page's control panel consume.

<a name="chunk-parameters"></a><sub>[`src/inline_entity.rs`](../../crates/x0k-folio/src/inline_entity.rs) · `#parameters`</sub>

```rust {#parameters}
/// One knob of a document's parameter panel.
#[derive(Debug, Clone, PartialEq)]
pub struct Parameter {
    /// The subject's last path segment: `threshold_gap`.
    pub id: String,
    pub label: Option<String>,
    pub comment: Option<String>,
    pub kind: Option<String>,
    pub min: Option<f64>,
    pub max: Option<f64>,
    pub step: Option<f64>,
    pub default: Option<ParameterValue>,
}

/// A parameter's starting value, as its literal's datatype types it.
#[derive(Debug, Clone, PartialEq)]
pub enum ParameterValue {
    Number(f64),
    Integer(i64),
    Boolean(bool),
    Text(String),
}

impl ParameterValue {
    fn of(literal: &oxrdf::Literal) -> Self {
        let value = literal.value();
        match literal.datatype().as_str() {
            XSD_INTEGER => value.parse().map(Self::Integer).unwrap_or_else(|_| Self::Text(value.to_string())),
            XSD_DECIMAL | XSD_DOUBLE => value.parse().map(Self::Number).unwrap_or_else(|_| Self::Text(value.to_string())),
            XSD_BOOLEAN => Self::Boolean(value == "true" || value == "1"),
            _ => Self::Text(value.to_string()),
        }
    }

    fn to_json(&self) -> serde_json::Value {
        match self {
            Self::Number(n) => serde_json::json!(n),
            Self::Integer(i) => serde_json::json!(i),
            Self::Boolean(b) => serde_json::json!(b),
            Self::Text(t) => serde_json::json!(t),
        }
    }
}

impl Parameter {
    /// The panel's wire shape: `{id, display_name, description,
    /// type: {kind, min, max, step}, default}`, absent fields left out.
    pub fn to_json(&self) -> serde_json::Value {
        let mut ty = serde_json::Map::new();
        if let Some(kind) = &self.kind {
            ty.insert("kind".into(), serde_json::json!(kind));
        }
        for (key, value) in [("min", self.min), ("max", self.max), ("step", self.step)] {
            if let Some(value) = value {
                ty.insert(key.into(), serde_json::json!(value));
            }
        }
        let mut out = serde_json::Map::new();
        out.insert("id".into(), serde_json::json!(self.id));
        if let Some(label) = &self.label {
            out.insert("display_name".into(), serde_json::json!(label));
        }
        if let Some(comment) = &self.comment {
            out.insert("description".into(), serde_json::json!(comment));
        }
        out.insert("type".into(), serde_json::Value::Object(ty));
        if let Some(default) = &self.default {
            out.insert("default".into(), default.to_json());
        }
        serde_json::Value::Object(out)
    }
}

/// Read a parameter panel's Turtle (the block's text, without its fences).
/// Every subject must be a `folio:Parameter`; the parameters come back in
/// the order the block first names them.
pub fn read_parameters(text: &str) -> Result<Vec<Parameter>, InlineEntityError> {
    let triples = parse_turtle(text, shipped_prefixes(), 1).map_err(|error| match error {
        crate::colophon::FolioError::Turtle { line, reason } => InlineEntityError::InvalidTurtle { line, reason },
        other => InlineEntityError::InvalidTurtle { line: 1, reason: other.to_string() },
    })?;
    let not_panel = |reason: String| InlineEntityError::NotOneInstance { line: 0, reason };
    if !is_parameter_block(&triples) {
        return Err(not_panel("a parameter panel types every subject `folio:Parameter`".into()));
    }
    let label = format!("{RDFS_NS}label");
    let comment = format!("{RDFS_NS}comment");
    let term = |local: &str| format!("{FOLIO_NS}{local}");
    let mut order: Vec<String> = Vec::new();
    let mut by_subject: BTreeMap<String, Parameter> = BTreeMap::new();
    for triple in &triples {
        let subject = match &triple.subject {
            NamedOrBlankNode::NamedNode(node) => node.as_str().to_string(),
            NamedOrBlankNode::BlankNode(_) => return Err(not_panel("a parameter is a blank node".into())),
        };
        let entry = by_subject.entry(subject.clone()).or_insert_with(|| {
            order.push(subject.clone());
            Parameter {
                id: subject.rsplit(['/', '#']).next().unwrap_or(&subject).to_string(),
                label: None,
                comment: None,
                kind: None,
                min: None,
                max: None,
                step: None,
                default: None,
            }
        });
        let predicate = triple.predicate.as_str();
        if predicate == RDF_TYPE {
            continue;
        }
        let Term::Literal(literal) = &triple.object else {
            return Err(not_panel(format!("`{}` states an IRI where a parameter takes a literal", compact_iri(predicate, shipped_prefixes()))));
        };
        let number = || literal.value().parse::<f64>().ok();
        if predicate == label {
            entry.label = Some(literal.value().to_string());
        } else if predicate == comment {
            entry.comment = Some(literal.value().to_string());
        } else if predicate == term("kind") {
            entry.kind = Some(literal.value().to_string());
        } else if predicate == term("min") {
            entry.min = number();
        } else if predicate == term("max") {
            entry.max = number();
        } else if predicate == term("step") {
            entry.step = number();
        } else if predicate == term("default") {
            entry.default = Some(ParameterValue::of(literal));
        }
    }
    Ok(order.into_iter().filter_map(|subject| by_subject.remove(&subject)).collect())
}
```

## `extract_from_markdown`: two passes over the body

`extract_from_markdown` is what a caller reaches for. Hand it a body and the
set of classes its parent may host, and it returns one `Result` per attempted
record: [the affordances a document declares, read back as
data](../../decisions/design/corpus/publish-a-region-as-a-repository/declare-concepts-and-instances.md "x0k:affordance/read_declared_affordances"). Its rustdoc is the whole of
the cue — a reader of this library finds the function by its name and its doc
line — and that is what the declaration below records:

<a name="folio-instance-68747470733a2f2f306b2e636f6d70757465722f6f6e746f6c6f6779237369676e69666965722f78306b2d666f6c696f2d657874726163742d66726f6d2d6d61726b646f776e-1"></a><sub data-instance-iri="https://0k.computer/ontology#signifier/x0k-folio-extract-from-markdown" data-concept-iri="https://0k.computer/ontology#Signifier" data-source-document="corpora/x0k/implementation/folio/inline-entities.md"><strong>Signifier</strong> · extract_from_markdown: two passes over the body · <code>https://0k.computer/ontology#signifier/x0k-folio-extract-from-markdown</code> · <a href="#folio-source-68747470733a2f2f306b2e636f6d70757465722f6f6e746f6c6f6779237369676e69666965722f78306b2d666f6c696f2d657874726163742d66726f6d2d6d61726b646f776e-1">source declaration</a></sub><a name="folio-source-68747470733a2f2f306b2e636f6d70757465722f6f6e746f6c6f6779237369676e69666965722f78306b2d666f6c696f2d657874726163742d66726f6d2d6d61726b646f776e-1"></a>

```turtle folio:graph
signifier:x0k-folio-extract-from-markdown a x0k:Signifier ;
    x0k:cue "extract_from_markdown" ;
    x0k:signifies affordance:read_declared_affordances ;
    x0k:presentedOn surface:sdk .
```

The description is "everything under the heading except the blocks", and
that phrasing is why the walk is two passes rather than one. A streaming pass
knows what came before a block but not what comes after, and the prose after
a block is as much the entity's description as the prose before — the corpus
writes it both ways. So the first pass records byte ranges for every heading
and every graph block (and the header, which is never prose either), and the
second does arithmetic on them: the section runs from the heading's end to the
next heading's start, minus the blocks' spans.

A class the caller did not ask for is skipped at `debug`: `check` asks for
affordances and signifiers, and every correctly declared icon or germ in the
set goes past it.

<a name="chunk-extract"></a><sub>[`src/inline_entity.rs`](../../crates/x0k-folio/src/inline_entity.rs) · `#extract`</sub>

```rust {#extract}
/// Walk a markdown body and return one `Result` per attempted record, so a
/// caller can warn per error without dropping the batch. Blocks are read
/// against the vocabulary this build compiled; classes outside
/// `allowed_classes` (and outside the shared namespace) are skipped.
pub fn extract_from_markdown(
    body: &str,
    allowed_classes: &HashSet<String>,
) -> Vec<Result<InlineEntity, InlineEntityError>> {
    extract_located(body, Some(allowed_classes), None).into_iter().map(|located| located.result).collect()
}

/// Extract every instance a body declares against a caller-selected
/// vocabulary: its prefixes are in scope, and ids expand through its
/// namespaces, so a reader's `paper:` instances are read as theirs.
pub fn extract_from_markdown_in(
    body: &str,
    model: &OntologyModel,
) -> Vec<Result<InlineEntity, InlineEntityError>> {
    extract_located(body, None, Some(model)).into_iter().map(|located| located.result).collect()
}

/// One attempted record and where its block sits in the caller's text:
/// the byte range and the 1-based line of the opening fence. A caller that
/// reports against a source location — the vocabulary loader — reads this.
pub struct Located {
    pub span: Range<usize>,
    pub line: usize,
    pub result: Result<InlineEntity, InlineEntityError>,
}

struct PendingHeading {
    text: String,
    /// End offset of the heading line — where its section's prose starts.
    end: usize,
    /// Start offset of the heading line — the previous section's end.
    start: usize,
}

struct PendingBlock {
    kind: BlockKind,
    text: String,
    span: Range<usize>,
    line: usize,
}

enum BlockKind {
    Header,
    Graph,
    Icon,
}

/// The walk behind both entry points: every attempted record, located.
pub fn extract_located(
    body: &str,
    allowed: Option<&HashSet<String>>,
    model: Option<&OntologyModel>,
) -> Vec<Located> {
    let owned;
    let prefixes: &[(String, String)] = match model {
        Some(model) => {
            owned = predeclared_prefixes(model);
            &owned
        }
        None => shipped_prefixes(),
    };
    let icons = allowed.is_some_and(|set| set.contains(ICON_CLASS));

    let options =
        Options::ENABLE_STRIKETHROUGH | Options::ENABLE_TABLES | Options::ENABLE_HEADING_ATTRIBUTES;
    let mut headings: Vec<PendingHeading> = Vec::new();
    let mut blocks: Vec<PendingBlock> = Vec::new();
    let mut active: Option<PendingBlock> = None;
    let mut in_heading = false;
    let mut heading_buf = String::new();
    let mut heading_start = 0usize;

    for (event, range) in Parser::new_ext(body, options).into_offset_iter() {
        match event {
            Event::Start(Tag::Heading { .. }) => {
                in_heading = true;
                heading_buf.clear();
                heading_start = range.start;
            }
            Event::End(TagEnd::Heading(_)) => {
                in_heading = false;
                headings.push(PendingHeading {
                    text: heading_buf.trim().to_string(),
                    end: range.end,
                    start: heading_start,
                });
                heading_buf.clear();
            }
            Event::Text(t) if in_heading => heading_buf.push_str(&t),
            Event::Code(c) if in_heading => heading_buf.push_str(&c),
            Event::Start(Tag::CodeBlock(CodeBlockKind::Fenced(info))) => {
                let kind = if is_marker(&info, GRAPH_MARKER) {
                    Some(BlockKind::Graph)
                } else if is_marker(&info, HEADER_MARKER) {
                    Some(BlockKind::Header)
                } else if icons && is_icon_fence(&info) {
                    Some(BlockKind::Icon)
                } else {
                    None
                };
                if let Some(kind) = kind {
                    let line = 1 + body[..range.start].matches('\n').count();
                    active = Some(PendingBlock { kind, text: String::new(), span: range, line });
                }
            }
            Event::Text(t) if active.is_some() => {
                if let Some(block) = active.as_mut() {
                    block.text.push_str(&t);
                }
            }
            Event::End(TagEnd::CodeBlock) => {
                if let Some(mut block) = active.take() {
                    block.span.end = range.end;
                    blocks.push(block);
                }
            }
            _ => {}
        }
    }

    // First pass over the blocks: read each graph block once, so the
    // per-section count below compares classes rather than markers.
    let read: Vec<Option<Result<GraphContent, InlineEntityError>>> = blocks
        .iter()
        .map(|block| match block.kind {
            BlockKind::Graph => Some(read_graph_block(&block.text, prefixes, block.line + 1)),
            _ => None,
        })
        .collect();
    let class_of = |index: usize| -> Option<String> {
        match (&blocks[index].kind, &read[index]) {
            (BlockKind::Icon, _) => Some(ICON_CLASS.to_string()),
            (_, Some(Ok(GraphContent::Instance { class, .. }))) => Some(class.clone()),
            _ => None,
        }
    };

    let mut out = Vec::new();
    for (index, block) in blocks.iter().enumerate() {
        let content = match (&block.kind, &read[index]) {
            (BlockKind::Header, _)
            | (_, Some(Ok(GraphContent::Definitions)))
            | (_, Some(Ok(GraphContent::Parameters))) => continue,
            (BlockKind::Graph, Some(Err(error))) => {
                out.push(Located { span: block.span.clone(), line: block.line, result: Err(error.clone()) });
                continue;
            }
            (BlockKind::Graph, Some(Ok(GraphContent::Instance { subject, class, statements }))) => {
                Some((subject.clone(), class.clone(), statements.clone()))
            }
            _ => None,
        };
        if let Some((_, class, _)) = &content {
            if let Some(allowed) = allowed {
                if !class.starts_with(X0K_NS) || !allowed.contains(&class_name(class)) {
                    debug!(class = %class, "inline-entity: a class this caller did not ask for; skipping");
                    continue;
                }
            }
        }

        let enclosing = headings.iter().rev().find(|h| h.end <= block.span.start);
        let section_end = match enclosing {
            Some(h) => headings.iter().find(|other| other.start > h.start).map_or(body.len(), |next| next.start),
            None => body.len(),
        };
        let prose_start = enclosing.map_or(0, |h| h.end);
        let in_section = |b: &PendingBlock| b.span.start >= prose_start && b.span.start < section_end;
        let this_class = class_of(index);
        let earlier_of_class = if matches!(block.kind, BlockKind::Icon) {
            0
        } else {
            (0..index)
                .filter(|&i| in_section(&blocks[i]) && class_of(i) == this_class)
                .count()
        };
        let spans: Vec<Range<usize>> = blocks.iter().filter(|b| in_section(b)).map(|b| b.span.clone()).collect();
        let description = section_description(body, prose_start, section_end, &spans);
        let heading = enclosing.map(|h| h.text.as_str()).filter(|h| !h.is_empty());

        let record = match content {
            None => match heading {
                None => Err(InlineEntityError::MissingHeading { class: ICON_CLASS.to_string() }),
                Some(heading) => Ok(icon_record(&block.text, heading, description, block)),
            },
            Some((subject, class, statements)) => {
                let name = class_name(&class);
                match heading {
                    None => Err(InlineEntityError::MissingHeading { class: name }),
                    Some(_) if earlier_of_class > 0 => Err(InlineEntityError::MultipleBlocksInSection {
                        class: name,
                        heading: heading.unwrap_or_default().to_string(),
                    }),
                    Some(heading) => instance_record(
                        &subject, &class, statements, heading, description, block, prefixes, model,
                    ),
                }
            }
        };
        out.push(Located { span: block.span.clone(), line: block.line, result: record });
    }
    out
}

/// `svg x0k:icon`: the one declaring fence written in something other than
/// Turtle. An illustrative `svg x0k:!icon` declares nothing.
fn is_icon_fence(info: &str) -> bool {
    let carrier = FenceInfo::parse(info);
    carrier.info().is_none()
        && carrier.language().is_some_and(|language| language.eq_ignore_ascii_case("svg"))
        && carrier.x0k_type() == Some(ICON_CLASS)
}
```

## Turning one block into a record

The checks run in the order that lets each assume the last: an id before a
class comparison, a class before the prohibitions. Two of them are
prohibitions rather than validations — a title and `definedIn` are both
*forbidden*, because both would let a document state something the embedding
already says, and two sources for one fact is how they come to disagree.

<a name="chunk-finalize"></a><sub>[`src/inline_entity.rs`](../../crates/x0k-folio/src/inline_entity.rs) · `#finalize`</sub>

```rust {#finalize}
#[allow(clippy::too_many_arguments)]
fn instance_record(
    subject: &str,
    class_iri: &str,
    statements: Vec<(String, Object)>,
    heading: &str,
    description: String,
    block: &PendingBlock,
    prefixes: &[(String, String)],
    model: Option<&OntologyModel>,
) -> Result<InlineEntity, InlineEntityError> {
    let class = class_name(class_iri);
    let compact = compact_iri(subject, prefixes);
    let parsed = match model {
        Some(model) => EntityId::parse_in(model, &compact),
        None => compact.parse(),
    };
    let uri: EntityId = parsed.map_err(|e| InlineEntityError::InvalidUri {
        class: class.clone(),
        heading: heading.to_string(),
        value: compact.clone(),
        reason: e.to_string(),
    })?;
    let expand = |text: &str| match model {
        Some(model) => model.expand(text),
        None => crate::colophon::expand_compact(text, prefixes),
    };
    let id_namespace = expand(&format!("{}:", uri.scheme));
    let class_namespace = &class_iri[..class_iri.len() - class_iri.rsplit(['#', '/']).next().unwrap_or("").len()];
    if uri.class != class || id_namespace != class_namespace {
        return Err(InlineEntityError::ClassMismatch {
            class,
            heading: heading.to_string(),
            uri_class: uri.class.clone(),
        });
    }
    let local = |predicate: &str| predicate.rsplit(['#', '/']).next().unwrap_or(predicate).to_string();
    if statements.iter().any(|(p, _)| *p == format!("{id_namespace}title")) {
        return Err(InlineEntityError::DuplicateTitle { class, heading: heading.to_string() });
    }
    if statements.iter().any(|(p, _)| local(p) == "definedIn") {
        return Err(InlineEntityError::ExplicitDefinedIn { class, heading: heading.to_string() });
    }
    let requires_resources = declared_resources(&statements, &id_namespace, &class, heading)?;
    Ok(InlineEntity {
        uri,
        title: heading.to_string(),
        description,
        class,
        class_iri: class_iri.to_string(),
        statements,
        svg: None,
        requires_resources,
        span: block.span.clone(),
        line: block.line,
    })
}
```

An icon has no statements to check. The profile's rules — the grid, the
paints, the budget — are the checker's (`x0k-icon`), applied by whoever shows
the mark, and this module hands the drawing over as written. What it decides
is the identity: the icon depicts the thing its section declares, so the
record's id is the section's own, `x0k:icon/<anchor>`, the heading slugged by
the rule a transclusion reference spells it with
([`transclusion.md`](transclusion.md)).

<a name="chunk-icon-record"></a><sub>[`src/inline_entity.rs`](../../crates/x0k-folio/src/inline_entity.rs) · `#icon-record`</sub>

```rust {#icon-record}
/// The record of an `svg x0k:icon` block: the drawing, the section's
/// heading as the title, and an id derived from the heading.
fn icon_record(svg: &str, heading: &str, description: String, block: &PendingBlock) -> InlineEntity {
    let uri: EntityId = format!("x0k:{ICON_CLASS}/{}", heading_slug(heading))
        .parse()
        .expect("a heading slug is non-empty and carries no whitespace");
    InlineEntity {
        uri,
        title: heading.to_string(),
        description,
        class: ICON_CLASS.to_string(),
        class_iri: String::new(),
        statements: Vec::new(),
        svg: Some(svg.to_string()),
        requires_resources: Vec::new(),
        span: block.span.clone(),
        line: block.line,
    }
}
```

The description is the section with holes in it — one per graph block, the
header, and an icon's drawing — and the remaining fragments are joined by
blank lines so the result reads as markdown rather than as paragraphs run
together.

<a name="chunk-description"></a><sub>[`src/inline_entity.rs`](../../crates/x0k-folio/src/inline_entity.rs) · `#description`</sub>

```rust {#description}
/// Section prose with every block's span excised, rejoined with blank lines.
fn section_description(body: &str, prose_start: usize, prose_end: usize, spans: &[Range<usize>]) -> String {
    let mut fragments: Vec<&str> = Vec::new();
    let mut cursor = prose_start;
    for span in spans {
        fragments.push(body.get(cursor..span.start).unwrap_or(""));
        cursor = span.end;
    }
    fragments.push(body.get(cursor..prose_end).unwrap_or(""));
    fragments
        .iter()
        .map(|f| f.trim())
        .filter(|f| !f.is_empty())
        .collect::<Vec<_>>()
        .join("\n\n")
}
```

## Declared demands, unread

Each demand is one `rdf:JSON` literal holding one object; several demands are
several statements. Beyond that the object is passed through untouched.

<a name="chunk-resources"></a><sub>[`src/inline_entity.rs`](../../crates/x0k-folio/src/inline_entity.rs) · `#resources`</sub>

```rust {#resources}
/// Read `requiresResources` statements as declared: each a JSON object.
fn declared_resources(
    statements: &[(String, Object)],
    namespace: &str,
    class: &str,
    heading: &str,
) -> Result<Vec<serde_json::Map<String, serde_json::Value>>, InlineEntityError> {
    let predicate = format!("{namespace}requiresResources");
    let invalid = |reason: String| InlineEntityError::InvalidResources {
        class: class.to_string(),
        heading: heading.to_string(),
        reason,
    };
    statements
        .iter()
        .filter(|(p, _)| *p == predicate)
        .enumerate()
        .map(|(index, (_, object))| match object {
            Object::Literal(literal) if literal.datatype == RDF_JSON || literal.datatype == XSD_STRING => {
                match serde_json::from_str::<serde_json::Value>(&literal.value) {
                    Ok(serde_json::Value::Object(map)) => Ok(map),
                    Ok(_) => Err(invalid(format!("resource #{} is not a JSON object", index + 1))),
                    Err(e) => Err(invalid(format!("resource #{}: {e}", index + 1))),
                }
            }
            _ => Err(invalid(format!(
                "resource #{} must be an rdf:JSON object literal",
                index + 1
            ))),
        })
        .collect()
}
```

## Flattening a record to facts

An extracted entity becomes a flat `(predicate, value)` list on the way to a
fact store. The predicate is the statement's own term, compact
(`x0k:status`, `x0k:claimedFor`): what the block states is what the store
holds, with no per-class namespace minted for it. Values keep their kind — a
statement whose object is an entity is an `entity:` target, a literal is a
`string:` value — and the `definedIn` edge is appended by the parent, from the
embedding location: the source never states it, so the fact is minted rather
than copied. `x0k:requiresResources` is not flattened here; a host that
interprets the demands owns their facts.

<a name="chunk-facts"></a><sub>[`src/inline_entity.rs`](../../crates/x0k-folio/src/inline_entity.rs) · `#facts`</sub>

```rust {#facts}
/// The declared half of an entity's facts: title, description, and every
/// statement, under the statement's compact term. No `definedIn` — that is
/// the parent's to add, via [`defined_in_fact`] — and no resource demands.
pub fn declared_facts(entity: &InlineEntity) -> Vec<(String, String)> {
    let prefixes = shipped_prefixes();
    let mut out: Vec<(String, String)> = Vec::new();
    out.push(("x0k:title".to_string(), format!("string:{}", entity.title)));
    if !entity.description.is_empty() {
        out.push(("x0k:description".to_string(), format!("string:{}", entity.description)));
    }
    for (predicate, object) in &entity.statements {
        if predicate.ends_with("requiresResources") {
            continue;
        }
        let name = compact_iri(predicate, prefixes);
        match object {
            Object::Iri(target) => out.push((name, format!("entity:{}", compact_iri(target, prefixes)))),
            Object::Literal(literal) => out.push((name, format!("string:{}", literal.value))),
        }
    }
    out
}

/// The implicit edge back to the document the entity was authored in.
/// Minted from the embedding location; never read from the source.
pub fn defined_in_fact(parent_uri: &str) -> (String, String) {
    ("x0k:definedIn".to_string(), format!("entity:{parent_uri}"))
}

/// Declared facts plus the implicit `definedIn` edge — the whole record for
/// a consumer that does not interpret resource demands.
pub fn inline_entity_facts(entity: &InlineEntity, parent_uri: &str) -> Vec<(String, String)> {
    let mut out = declared_facts(entity);
    out.push(defined_in_fact(parent_uri));
    out
}
```

## Links authored inside prose

An affordance is not the only thing a document declares in its own prose. A
chapter that says *the parser reads the fence grammar [literate
programming](../../background/literate-programming.md "x0k:wiki/literate-programming") fixed* has named the concept a
reader needs first, and a chapter that says *this is the
[check](../../decisions/design/corpus/publish-a-region-as-a-repository/check-a-document-against-its-vocabulary.md "x0k:affordance/check_a_document_against_shipped_vocabulary") the CLI
puts in a shell* has said which affordance it realizes. Each of those links is
an edge of the graph, written in the sentence that needs it — the same rule
that puts `proves=` on the fence tangling the test rather than in a design
that names the test. The header still admits both predicates; it is not where
they live.

Two link classes are edges and no other. A link whose target is a wiki page is
`x0k:presupposes`; one whose target is an affordance is `x0k:realizes`. A link
to an implementation or a design is a link. A wiki target loses its fragment,
because a concept page crosses whole. Fenced code and inline code are not
prose: a chapter showing the grammar is not presupposing what its example
names.

<a name="chunk-prose-edges"></a><sub>[`src/inline_entity.rs`](../../crates/x0k-folio/src/inline_entity.rs) · `#prose-edges`</sub>

```rust {#prose-edges}
/// The edges a body's prose links declare: `(predicate, target id)`, once
/// each in first-seen order, the predicate compact (`x0k:presupposes`). A
/// markdown link `[…](x0k:wiki/<stem>)` is a `presupposes` edge and
/// `[…](x0k:affordance/<slug>)` a `realizes` edge; every other link is a
/// link. Text inside a fenced block or an inline code span is not read.
pub fn prose_edges(body: &str) -> Vec<(String, String)> {
    let mut out: Vec<(String, String)> = Vec::new();
    let mut fence: Option<(char, usize)> = None;
    for line in body.lines() {
        let trimmed = line.trim_start();
        let fence_run = trimmed
            .chars()
            .next()
            .filter(|c| *c == '`' || *c == '~')
            .map(|c| (c, trimmed.chars().take_while(|x| *x == c).count()))
            .filter(|(_, n)| *n >= 3);
        match (fence, fence_run) {
            (Some((c, n)), Some((c2, n2))) if c == c2 && n2 >= n => {
                fence = None;
                continue;
            }
            (Some(_), _) => continue,
            (None, Some(open)) => {
                fence = Some(open);
                continue;
            }
            (None, None) => {}
        }
        for target in link_targets_outside_code(line) {
            let edge = if let Some(rest) = target.strip_prefix("x0k:wiki/") {
                let stem = rest.split('#').next().unwrap_or(rest);
                Some(("x0k:presupposes", format!("x0k:wiki/{stem}")))
            } else if target.starts_with("x0k:affordance/") {
                Some(("x0k:realizes", target.to_string()))
            } else {
                None
            };
            if let Some((predicate, id)) = edge {
                if !out.iter().any(|(p, t)| p == predicate && *t == id) {
                    out.push((predicate.to_string(), id));
                }
            }
        }
    }
    out
}

/// The `x0k:` link targets on one line of prose, with inline code spans
/// (a run of backticks closed by a run of the same length) skipped. A
/// target ends at `)` or at the space before a link title.
fn link_targets_outside_code(line: &str) -> Vec<&str> {
    let mut out = Vec::new();
    let mut rest = line;
    while !rest.is_empty() {
        let next_code = rest.find('`');
        let next_link = rest.find("](x0k:");
        match (next_code, next_link) {
            (Some(c), Some(l)) if c < l => rest = skip_code_span(&rest[c..]),
            (Some(c), None) => rest = skip_code_span(&rest[c..]),
            (_, Some(l)) => {
                let after = &rest[l + 2..];
                let end = after.find(|ch: char| ch == ')' || ch.is_whitespace()).unwrap_or(after.len());
                out.push(&after[..end]);
                rest = &after[end..];
            }
            (None, None) => break,
        }
    }
    out
}

/// `s` starts with a backtick run; return what follows the matching
/// closing run, or nothing when the span never closes.
fn skip_code_span(s: &str) -> &str {
    let n = s.chars().take_while(|c| *c == '`').count();
    let body = &s[n..];
    let mut i = 0;
    while i < body.len() {
        if body[i..].starts_with('`') {
            let m = body[i..].chars().take_while(|c| *c == '`').count();
            if m == n {
                return &body[i + m..];
            }
            i += m;
        } else {
            i += body[i..].chars().next().map(char::len_utf8).unwrap_or(1);
        }
    }
    ""
}

/// The document's edges as one map: the header's edges and the edges its
/// prose links declare, each target once per predicate, the header's first.
pub fn document_edges(
    header_edges: &BTreeMap<String, Vec<String>>,
    body: &str,
) -> BTreeMap<String, Vec<String>> {
    let mut edges = header_edges.clone();
    for (predicate, target) in prose_edges(body) {
        let targets = edges.entry(predicate).or_default();
        if !targets.contains(&target) {
            targets.push(target);
        }
    }
    edges
}
```

## Instances in a caller's vocabulary

The model-aware entry point keeps the same heading and section rules. What it
adds is the vocabulary: the prefixes the caller's model declares are in scope
in every graph block, an id expands through the model's namespaces, and a
class from any namespace the model holds is extracted — `a paper:Paper`
declares an instance whose class name is `paper`. Namespace aliases compare by
expanded IRI. The collection loader checks that the class exists; reading a
block does not assert that.

A code fence is recognized by its marker alone: `turtle folio:graph` or
nothing. There is no shape heuristic to get wrong — a Docusaurus page's
```` ```yaml title:"app-config.yaml" ```` is a code fence because it is not a
graph block, not because its info string failed a test for looking like one.

## Tests

The carried example is the affordance at the top of this document; the rest
pin one refusal each, and one pins the boundary — that a declared resource
comes back as an object and not as an interpretation.

<a name="chunk-tests"></a><sub>[`src/inline_entity.rs`](../../crates/x0k-folio/src/inline_entity.rs) · `#tests`</sub>

`````rust {#tests}
#[cfg(test)]
mod tests {
    use super::*;

    fn allowed_set() -> HashSet<String> {
        HashSet::from(["affordance".to_string()])
    }

    fn one(body: &str) -> InlineEntity {
        let mut results = extract_from_markdown(body, &allowed_set());
        assert_eq!(results.len(), 1, "expected exactly one inline entity");
        results.remove(0).expect("entity parsed")
    }

    const CARRIED: &str = "## Affordances\n\n### Publish a region as a repository\n\n```turtle folio:graph\naffordance:publish_region_as_repository a x0k:Affordance ;\n    x0k:status \"wip\" ;\n    x0k:claimedFor x0k:actor\\/human ;\n    x0k:requires affordance:demarcate_publication .\n```\n\nI project a demarcated region of my graph outward as a standalone\nrepository.\n";

    #[test]
    fn extracts_the_carried_example() {
        let entity = one(CARRIED);
        assert_eq!(entity.uri.class, "affordance");
        assert_eq!(entity.uri.identifier, "publish_region_as_repository");
        assert_eq!(entity.title, "Publish a region as a repository");
        assert_eq!(entity.class, "affordance");
        assert_eq!(entity.class_iri, "https://0k.computer/ontology#Affordance");
        assert!(entity.description.contains("I project a demarcated region"), "{}", entity.description);

        let facts = inline_entity_facts(&entity, "x0k:design/publish-a-region-as-a-repository");
        let has = |p: &str, v: &str| facts.iter().any(|(fp, fv)| fp == p && fv == v);
        assert!(has("x0k:title", "string:Publish a region as a repository"));
        assert!(has("x0k:status", "string:wip"));
        assert!(has("x0k:claimedFor", "entity:x0k:actor/human"));
        assert!(has("x0k:requires", "entity:x0k:affordance/demarcate_publication"));
        assert_eq!(
            facts.last(),
            Some(&("x0k:definedIn".to_string(), "entity:x0k:design/publish-a-region-as-a-repository".to_string()))
        );
    }

    #[test]
    fn a_nested_example_declares_nothing() {
        let body = "### Publish\n\n````markdown\n```turtle folio:graph\naffordance:publish a x0k:Affordance .\n```\n````\n";
        assert!(extract_from_markdown(body, &allowed_set()).is_empty());
        let plain = "### Publish\n\n```turtle\naffordance:publish a x0k:Affordance .\n```\n";
        assert!(extract_from_markdown(plain, &allowed_set()).is_empty());
    }

    // The icon profile's carried example: an affordance with its mark
    // declared beside it. Two records from one section, and neither
    // description carries the other's block.
    #[test]
    fn an_icon_is_declared_beside_the_thing_it_depicts() {
        let body = "### Tangle a document\n\nI project code out of a document.\n\n```turtle folio:graph\naffordance:tangle a x0k:Affordance ;\n    x0k:claimedFor x0k:actor\\/human .\n```\n\nIts mark: a document with a block sliding out.\n\n```svg x0k:icon\n<svg viewBox=\"0 0 16 16\">\n  <circle cx=\"8\" cy=\"8\" r=\"6\" fill=\"none\" stroke=\"ink\" stroke-width=\"1.5\"/>\n</svg>\n```\n\nAfter both.\n";
        let mut allowed = allowed_set();
        allowed.insert(ICON_CLASS.to_string());
        let results: Vec<InlineEntity> =
            extract_from_markdown(body, &allowed).into_iter().map(|r| r.expect("both records parse")).collect();
        assert_eq!(results.len(), 2);
        let (affordance, icon) = (&results[0], &results[1]);
        assert_eq!(affordance.uri.to_string(), "x0k:affordance/tangle");
        assert_eq!(icon.uri.to_string(), "x0k:icon/tangle-a-document");
        assert_eq!(icon.class, ICON_CLASS);
        assert_eq!(
            icon.svg.as_deref(),
            Some("<svg viewBox=\"0 0 16 16\">\n  <circle cx=\"8\" cy=\"8\" r=\"6\" fill=\"none\" stroke=\"ink\" stroke-width=\"1.5\"/>\n</svg>\n")
        );
        let expected = "I project code out of a document.\n\nIts mark: a document with a block sliding out.\n\nAfter both.";
        assert_eq!(affordance.description, expected);
        assert_eq!(icon.description, expected);
    }

    #[test]
    fn one_block_per_class_per_section() {
        let body = "### Authenticate\n\n```turtle folio:graph\naffordance:authenticate a x0k:Affordance .\n```\n\n```turtle folio:graph\nsignifier:login a x0k:Signifier .\n```\n\n```turtle folio:graph\naffordance:authenticate_again a x0k:Affordance .\n```\n";
        let allowed = HashSet::from(["affordance".to_string(), "signifier".to_string()]);
        let results = extract_from_markdown(body, &allowed);
        assert_eq!(results.len(), 3);
        assert!(results[0].is_ok());
        assert!(results[1].is_ok(), "a signifier beside an affordance: {:?}", results[1]);
        match &results[2] {
            Err(InlineEntityError::MultipleBlocksInSection { heading, .. }) => assert_eq!(heading, "Authenticate"),
            other => panic!("expected MultipleBlocksInSection, got {other:?}"),
        }
    }

    #[test]
    fn description_captures_prose_before_the_block_and_skips_the_header() {
        let body = "# Doc\n\n```turtle folio:document\ndesign:doc a x0k:Design .\n```\n\nIntro prose.\n\n```turtle folio:graph\naffordance:authenticate a x0k:Affordance .\n```\n";
        let entity = one(body);
        assert_eq!(entity.description, "Intro prose.");
    }

    #[test]
    fn declared_resources_come_back_as_declared() {
        let body = "### Run an agent\n\n```turtle folio:graph\naffordance:run_agent a x0k:Affordance ;\n    x0k:requiresResources '{\"kind\":{\"os\":\"linux\"},\"origin\":\"operator_declared\"}'^^rdf:JSON,\n        '{\"kind\":\"cpu_cores\",\"quantity\":{\"numeric\":2}}'^^rdf:JSON .\n```\n";
        let entity = one(body);
        assert_eq!(entity.requires_resources.len(), 2);
        assert_eq!(entity.requires_resources[1]["kind"], "cpu_cores");
        let facts = inline_entity_facts(&entity, "x0k:design/test-doc");
        assert!(!facts.iter().any(|(p, _)| p.ends_with("requiresResources")));
    }

    #[test]
    fn a_resource_that_is_not_an_object_is_an_error() {
        let body = "### Run an agent\n\n```turtle folio:graph\naffordance:run_agent a x0k:Affordance ;\n    x0k:requiresResources \"two cores\" .\n```\n";
        let results = extract_from_markdown(body, &allowed_set());
        assert!(matches!(results[0], Err(InlineEntityError::InvalidResources { .. })));
    }

    #[test]
    fn a_block_without_a_heading_errors() {
        let body = "```turtle folio:graph\naffordance:orphan a x0k:Affordance .\n```\n";
        let results = extract_from_markdown(body, &allowed_set());
        assert!(matches!(results[0], Err(InlineEntityError::MissingHeading { .. })));
    }

    #[test]
    fn a_class_this_caller_did_not_ask_for_is_skipped_not_reported() {
        let body = "### Foo\n\n```turtle folio:graph\nsignifier:foo a x0k:Signifier .\n```\n";
        assert!(extract_from_markdown(body, &allowed_set()).is_empty());
    }

    #[test]
    fn a_vocabulary_block_is_not_an_instance() {
        let body = "### Terms\n\n```turtle folio:graph\nx0k:Thing a owl:Class .\n```\n";
        assert!(extract_from_markdown(body, &allowed_set()).is_empty());
    }

    #[test]
    fn a_block_states_one_instance_with_one_class() {
        for (body, reason) in [
            ("### A\n\n```turtle folio:graph\naffordance:a a x0k:Affordance .\naffordance:b a x0k:Affordance .\n```\n", "more than one subject"),
            ("### A\n\n```turtle folio:graph\naffordance:a x0k:status \"wip\" .\n```\n", "no class"),
            ("### A\n\n```turtle folio:graph\naffordance:a a x0k:Affordance ; x0k:requires [ a x0k:Affordance ] .\n```\n", "blank node"),
        ] {
            match &extract_from_markdown(body, &allowed_set())[..] {
                [Err(InlineEntityError::NotOneInstance { reason: got, .. })] => assert!(got.contains(reason), "{got}"),
                other => panic!("expected NotOneInstance ({reason}), got {other:?}"),
            }
        }
    }

    #[test]
    fn broken_turtle_names_its_line() {
        let body = "### A\n\n```turtle folio:graph\naffordance:a a x0k:Affordance ;\n    x0k:status .\n```\n";
        match &extract_from_markdown(body, &allowed_set())[..] {
            [Err(InlineEntityError::InvalidTurtle { line, .. })] => assert_eq!(*line, 5),
            other => panic!("expected InvalidTurtle, got {other:?}"),
        }
    }

    #[test]
    fn the_id_class_and_the_stated_class_must_agree() {
        let body = "### Section\n\n```turtle folio:graph\ndesign:not-an-affordance a x0k:Affordance .\n```\n";
        match &extract_from_markdown(body, &allowed_set())[0] {
            Err(InlineEntityError::ClassMismatch { uri_class, class, .. }) => {
                assert_eq!(class, "affordance");
                assert_eq!(uri_class, "design");
            }
            other => panic!("expected ClassMismatch, got {other:?}"),
        }
    }

    #[test]
    fn stating_the_implicit_edge_or_a_title_errors() {
        let defined = "### A\n\n```turtle folio:graph\naffordance:a a x0k:Affordance ; x0k:definedIn design:foo .\n```\n";
        assert!(matches!(extract_from_markdown(defined, &allowed_set())[0], Err(InlineEntityError::ExplicitDefinedIn { .. })));
        let titled = "### A\n\n```turtle folio:graph\naffordance:a a x0k:Affordance ; x0k:title \"Other\" .\n```\n";
        assert!(matches!(extract_from_markdown(titled, &allowed_set())[0], Err(InlineEntityError::DuplicateTitle { .. })));
    }

    #[test]
    fn an_id_carrying_a_content_state_pin_errors() {
        let body = "### A\n\n```turtle folio:graph\n<https://0k.computer/ontology#affordance/authenticate@file-content:abcd> a x0k:Affordance .\n```\n";
        assert!(matches!(extract_from_markdown(body, &allowed_set())[0], Err(InlineEntityError::InvalidUri { .. })));
    }

    /// The regression the old shape heuristic existed for: an ordinary fence
    /// whose info string holds a colon is a code fence
    /// (`docs/backend-system/building-backends/08-migrating.md:896` in the
    /// Backstage tree, 2026-09-23). Only the marker declares.
    #[test]
    fn a_fence_title_is_a_code_fence() {
        let body = "### The Auth Plugin\n\n```yaml title:\"app-config.yaml\"\nauth: {}\n```\n";
        assert!(extract_from_markdown(body, &allowed_set()).is_empty());
    }

    #[test]
    fn prose_links_to_wiki_and_affordance_are_edges_and_nothing_else_is() {
        let body = "Reads [literate programming](x0k:wiki/literate-programming#history) and\n\
                    [the design](x0k:design/some-design); it is the\n\
                    [check](x0k:affordance/check_it \"the check\") face.\n\
                    Again [lp](x0k:wiki/literate-programming).\n";
        assert_eq!(
            prose_edges(body),
            vec![
                ("x0k:presupposes".to_string(), "x0k:wiki/literate-programming".to_string()),
                ("x0k:realizes".to_string(), "x0k:affordance/check_it".to_string()),
            ]
        );
    }

    #[test]
    fn prose_links_inside_code_are_not_read() {
        let body = "Write `[x](x0k:wiki/in-span)` like so:\n\n\
                    ````markdown\n```\n[y](x0k:wiki/in-fence)\n```\n````\n\n\
                    but [z](x0k:wiki/real) counts.\n";
        assert_eq!(prose_edges(body), vec![("x0k:presupposes".to_string(), "x0k:wiki/real".to_string())]);
    }

    #[test]
    fn a_parameter_panel_reads_in_order_and_is_not_an_instance() {
        let panel = "x0k:implementation\\/canvas\\/core\\/threshold_gap a folio:Parameter ;\n    rdfs:label \"Hysteresis Gap\" ;\n    rdfs:comment \"Ratio\" ;\n    folio:kind \"float\" ;\n    folio:min 0.1 ;\n    folio:max 3.0 ;\n    folio:step 0.1 ;\n    folio:default 1.5 .\n\nx0k:implementation\\/canvas\\/core\\/num_levels a folio:Parameter ;\n    rdfs:label \"LOD Levels\" ;\n    folio:kind \"uint\" ;\n    folio:min 2.0 ;\n    folio:max 8.0 ;\n    folio:default 4 .\n";
        let parameters = read_parameters(panel).expect("panel");
        assert_eq!(parameters.iter().map(|p| p.id.as_str()).collect::<Vec<_>>(), ["threshold_gap", "num_levels"]);
        assert_eq!(parameters[0].min, Some(0.1));
        assert_eq!(parameters[1].default, Some(ParameterValue::Integer(4)));
        assert_eq!(
            parameters[0].to_json(),
            serde_json::json!({"id": "threshold_gap", "display_name": "Hysteresis Gap", "description": "Ratio",
                "type": {"kind": "float", "min": 0.1, "max": 3.0, "step": 0.1}, "default": 1.5})
        );
        let body = format!("# Core\n\n## Knobs\n\n```turtle folio:graph {{source=\"hysteresis-parameters\"}}\n{panel}```\n");
        assert!(extract_from_markdown(&body, &HashSet::new()).is_empty());
        assert!(read_parameters("affordance:x a x0k:Affordance .\n").is_err());
    }

    #[test]
    fn document_edges_unions_the_header_and_the_prose_once_each() {
        let mut header = BTreeMap::new();
        header.insert("x0k:presupposes".to_string(), vec!["x0k:wiki/a".to_string()]);
        let edges = document_edges(&header, "See [a](x0k:wiki/a) and [b](x0k:wiki/b).");
        assert_eq!(edges["x0k:presupposes"], vec!["x0k:wiki/a".to_string(), "x0k:wiki/b".to_string()]);
    }
}
`````

## Composing the module

<a name="chunk-root"></a><sub>[`src/inline_entity.rs`](../../crates/x0k-folio/src/inline_entity.rs) · `#root` · assembles [module-doc](#chunk-module-doc) · [inline-entity](#chunk-inline-entity) · [error](#chunk-error) · [read-graph-block](#chunk-read-graph-block) · [parameters](#chunk-parameters) · [extract](#chunk-extract) · [finalize](#chunk-finalize) · [icon-record](#chunk-icon-record) · [description](#chunk-description) · [resources](#chunk-resources) · [facts](#chunk-facts) · [prose-edges](#chunk-prose-edges) · [tests](#chunk-tests)</sub>

```rust {#root}
<<module-doc>>

<<inline-entity>>

<<error>>

<<read-graph-block>>

<<parameters>>

<<extract>>

<<finalize>>

<<icon-record>>

<<description>>

<<resources>>

<<facts>>

<<prose-edges>>

<<tests>>
```

The section-per-entity rule is the load-bearing idea and it is worth saying
what it costs. Binding an entity to a heading means renaming a heading renames
the entity, and moving a block between sections re-parents it — the
document's shape *is* the data model, with no indirection to absorb an edit.
That is the trade the design took deliberately: an affordance that cannot
drift from the design that defines it, at the price of a document whose
structure has to be edited with that in mind.
