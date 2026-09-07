---
x0k:
  format: folio/v1
  id: x0k:implementation/folio/document-vocabulary
  type: implementation
  status: draft
  summary: Assemble document-carried Turtle vocabulary and typed YAML instances without a host registry.
  concerns:
  - folio
  - ontology
  - declarations
  tangle:
    crate: crates/x0k-folio
    root: src/document_vocabulary.rs
  edges:
    implements:
    - x0k:design/domains-of-your-own
    cites:
    - x0k:architecture/ontology-modules
    - x0k:implementation/ontology/load
    - x0k:implementation/folio/inline-entities
    motivated_by:
    - x0k:intent/c5ccd003-77d6-4b0d-8824-649f6221c259
---
# A vocabulary beside its examples

A paper collection can explain what a Paper means, define it in Turtle,
and declare particular papers in the same document. The collector reads
all definitions before checking any instance. File order is irrelevant;
the caller supplies the input set and every result retains its source
document and byte span.

The explicit carrier is `turtle folio:ontology`. Ordinary Turtle examples
are prose. Instances retain typed YAML fences, such as `yaml paper:paper`.
Their class marker is the kebab-case local class name; the namespace
remains part of the key, so paper:Paper and archive:Paper stay distinct.

This collector validates the named-node class/object-property subset:
one owning module per term, at most one domain and range per property, and exact
class membership. It refuses blank-node shapes, non-string Turtle
literals, and ambiguous class markers. It does not perform OWL entailment,
validate arbitrary scalar YAML fields, or choose a rendered appearance.
HTTP(S) and URN references retain their absolute IRI spelling; unknown compact
prefixes remain errors. Unresolved relationship targets remain explicit relationships; known
targets must match the declared range.

## The collection and its source locations

A module IRI names vocabulary. A document identity names the source;
neither replaces the other's identity. Namespace aliases normalize to
full IRIs for classes, instances and relationships. The format records
locations as offsets in the Markdown body passed by the caller. Each explicit
block also retains its complete normalized facts, including ontology-module
and namespace declarations, so a database projection does not discard them.

<a name="chunk-document-vocabulary"></a><sub>[`src/document_vocabulary.rs`](../../crates/x0k-folio/src/document_vocabulary.rs) · `#document-vocabulary`</sub>

```rust {#document-vocabulary}
use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
use std::ops::Range;
use std::path::Path;
use pulldown_cmark::{CodeBlockKind, Event, Parser, Tag, TagEnd};
use x0k_ontology::concept_facts::{
    camel_to_kebab, OntologyFact, OntologyModel, OntologyValue, OWL_CLASS,
    OWL_OBJECT_PROPERTY, OWL_ONTOLOGY, RDF_TYPE, RDFS_DOMAIN, RDFS_RANGE,
    RDFS_IS_DEFINED_BY, STRUCTURAL_NODE_PREFIX, X0K_NS,
};
use x0k_ontology::load::TurtleSource;
use crate::inline_entity::{declaration_marker, extract_from_markdown_in, InlineEntity};

/// The caller's stable document identity and its Markdown body.
pub struct DocumentSource<'a> {
    pub id: &'a str,
    pub body: &'a str,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BlockSource {
    pub document: String,
    pub bytes: Range<usize>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VocabularyErrorKind {
    Parse,
    Conflict,
    UnknownTerm,
    Unsupported,
    Duplicate,
    InvalidDeclaration,
    InvalidImport,
}

#[derive(Debug, Clone)]
pub struct VocabularyError {
    pub kind: VocabularyErrorKind,
    pub message: String,
    pub sources: Vec<BlockSource>,
}
impl fmt::Display for VocabularyError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} at {:?}", self.message, self.sources)
    }
}
impl std::error::Error for VocabularyError {}

#[derive(Debug, Clone)]
pub struct Definition {
    pub iri: String,
    pub sources: Vec<BlockSource>,
}

/// The normalized facts contributed by one explicit Turtle block.
#[derive(Debug, Clone)]
pub struct DefinitionBlock {
    pub source: BlockSource,
    pub facts: Vec<OntologyFact>,
}

#[derive(Debug, Clone)]
pub struct DeclaredInstance {
    pub iri: String,
    pub concept: String,
    pub source: BlockSource,
    pub entity: InlineEntity,
}

#[derive(Debug, Clone)]
pub struct Relationship {
    pub subject: String,
    pub predicate: String,
    pub object: String,
    pub source: BlockSource,
}

/// One vocabulary and its validated declarations; no storage or global registry.
pub struct DocumentVocabulary {
    pub model: OntologyModel,
    pub definitions: Vec<Definition>,
    pub definition_blocks: Vec<DefinitionBlock>,
    pub instances: Vec<DeclaredInstance>,
    pub relationships: Vec<Relationship>,
}


```

## Explicit blocks

The Markdown parser supplies fence ranges. Only the explicit ontology
marker contributes definitions; a plain Turtle example remains prose.

<a name="chunk-collect-blocks"></a><sub>[`src/document_vocabulary.rs`](../../crates/x0k-folio/src/document_vocabulary.rs) · `#collect-blocks`</sub>

```rust {#collect-blocks}
struct Block {
    info: String,
    text: String,
    source: BlockSource,
}

fn blocks(document: &DocumentSource<'_>) -> Vec<Block> {
    let mut out = Vec::new();
    let mut active: Option<Block> = None;
    for (event, span) in Parser::new(document.body).into_offset_iter() {
        match event {
            Event::Start(Tag::CodeBlock(CodeBlockKind::Fenced(info))) => {
                active = Some(Block { info: info.to_string(), text: String::new(),
                    source: BlockSource { document: document.id.to_string(), bytes: span } });
            }
            Event::Text(text) if active.is_some() => active.as_mut().unwrap().text.push_str(&text),
            Event::End(TagEnd::CodeBlock) => {
                if let Some(mut block) = active.take() {
                    block.source.bytes.end = span.end;
                    out.push(block);
                }
            }
            _ => {}
        }
    }
    out
}

fn ontology_marker(info: &str) -> bool {
    let mut tokens = info.split_ascii_whitespace();
    tokens.next().is_some_and(|s| s.eq_ignore_ascii_case("turtle"))
        && tokens.next() == Some("folio:ontology") && tokens.next().is_none()
}

fn error(kind: VocabularyErrorKind, message: impl Into<String>, sources: &[BlockSource]) -> VocabularyError {
    VocabularyError { kind, message: message.into(), sources: sources.to_vec() }
}


```

## Definitions before instances

The definitions-only entry point lets readers assemble a vocabulary before
parsing envelopes and isolate malformed instances to their source document.
Both entry points share the same parser and validation. Definition triples merge by full IRI. Repeating an identical definition
preserves both sources; a different definition refuses the collection.
Namespace aliases share meaning, while conflicting bindings name both
source blocks in the diagnostic.

<a name="chunk-assemble-collection"></a><sub>[`src/document_vocabulary.rs`](../../crates/x0k-folio/src/document_vocabulary.rs) · `#assemble-collection`</sub>

```rust {#assemble-collection}
/// Collect and validate definitions without interpreting instance declarations.
/// Imports refer only to the supplied base and document collection.
pub fn load_definitions(
    documents: &[DocumentSource<'_>],
    base: &OntologyModel,
) -> Result<DocumentVocabulary, VocabularyError> {
    let mut ids = BTreeSet::new();
    for document in documents {
        if !ids.insert(document.id) {
            return Err(error(VocabularyErrorKind::Duplicate, format!("duplicate source document {}", document.id), &[]));
        }
    }
    let mut declarations: Vec<Block> = documents.iter().flat_map(blocks)
        .filter(|block| ontology_marker(&block.info)).collect();
    declarations.sort_by(|a, b| (&a.source.document, a.source.bytes.start)
        .cmp(&(&b.source.document, b.source.bytes.start)));
    let mut facts = base.facts().to_vec();
    let mut definition_blocks = Vec::new();
    let mut definitions: BTreeMap<String, (BTreeSet<OntologyFact>, Vec<BlockSource>)> = BTreeMap::new();
    let base_definitions: BTreeMap<_, BTreeSet<_>> = declared_terms(base).into_iter().map(|iri| {
        let signature = base.facts().iter().filter(|fact| fact.entity == iri).cloned().collect();
        (iri, signature)
    }).collect();
    let mut prefix_sources = BTreeMap::new();
    for (prefix, namespace) in base.extension_namespaces() {
        prefix_sources.insert(prefix, (namespace, Vec::new()));
    }
    prefix_sources.insert("x0k".into(), (X0K_NS.into(), Vec::new()));
    for block in &declarations {
        let source_name = format!("{}#bytes={}", block.source.document, block.source.bytes.start);
        let parsed = OntologyModel::parse_turtle_sources(&[TurtleSource {
            name: Path::new(&source_name), text: &block.text,
        }]).map_err(|e| {
            let kind = match &e {
                x0k_ontology::load::LoadError::TypedLiteral { .. } => VocabularyErrorKind::Unsupported,
                _ => VocabularyErrorKind::Parse,
            };
            error(kind, e.to_string(), std::slice::from_ref(&block.source))
        })?;
        validate_definition_block(&parsed, &block.source)?;
        for (prefix, namespace) in parsed.extension_namespaces() {
            if !x0k_ontology::load::is_absolute_iri(&namespace) {
                return Err(error(VocabularyErrorKind::InvalidDeclaration,
                    format!("namespace {namespace:?} is not an absolute IRI"), std::slice::from_ref(&block.source)));
            }
            let valid = prefix.chars().next().is_some_and(|c| c.is_ascii_lowercase())
                && prefix.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || "+.-".contains(c));
            if !valid {
                return Err(error(VocabularyErrorKind::InvalidDeclaration, format!("invalid namespace prefix {prefix}"), std::slice::from_ref(&block.source)));
            }
            if let Some((prior_namespace, prior_sources)) = prefix_sources.get(&prefix) {
                if prior_namespace != &namespace {
                    let mut sources = prior_sources.clone();
                    sources.push(block.source.clone());
                    return Err(error(VocabularyErrorKind::Conflict, format!("namespace prefix {prefix} names both {prior_namespace} and {namespace}"), &sources));
                }
            }
            prefix_sources.entry(prefix).or_insert_with(|| (namespace, Vec::new())).1.push(block.source.clone());
        }
        for iri in declared_terms(&parsed) {
            let signature: BTreeSet<_> = parsed.facts().iter()
                .filter(|fact| fact.entity == iri).cloned().collect();
            if base_definitions.get(&iri).is_some_and(|prior| prior != &signature) {
                return Err(error(VocabularyErrorKind::Conflict, format!("definition of {iri} conflicts with the selected base vocabulary"),
                    &[BlockSource { document: "<base vocabulary>".into(), bytes: 0..0 }, block.source.clone()]));
            }
            match definitions.get_mut(&iri) {
                Some((prior, sources)) if prior == &signature => sources.push(block.source.clone()),
                Some((_, sources)) => {
                    let mut both = sources.clone();
                    both.push(block.source.clone());
                    return Err(error(VocabularyErrorKind::Conflict, format!("conflicting definition of {iri}"), &both));
                }
                None => { definitions.insert(iri, (signature, vec![block.source.clone()])); }
            }
        }
        definition_blocks.push(DefinitionBlock { source: block.source.clone(), facts: parsed.facts().to_vec() });
        facts.extend_from_slice(parsed.facts());
    }
    let model = OntologyModel::new(facts);
    let sources: Vec<_> = declarations.iter().map(|b| b.source.clone()).collect();
    model.import_order().map_err(|e| error(VocabularyErrorKind::InvalidImport, e, &sources))?;
    validate_term_modules(&model, &definitions)?;
    class_index(&model).map_err(|e| error(VocabularyErrorKind::Conflict, e, &sources))?;
    Ok(DocumentVocabulary {
        model, definitions: definitions.into_iter().map(|(iri, (_, sources))| Definition { iri, sources }).collect(),
        definition_blocks, instances: Vec::new(), relationships: Vec::new(),
    })
}

/// Parse instances against an already assembled vocabulary.
pub fn collect_instances(
    documents: &[DocumentSource<'_>],
    model: &OntologyModel,
) -> Result<Vec<DeclaredInstance>, VocabularyError> {
    let classes = class_index(model).map_err(|e| error(VocabularyErrorKind::Conflict, e, &[]))?;
    let mut instances = Vec::new();
    let mut instance_sources = BTreeMap::new();
    for document in documents {
        let locations: Vec<_> = blocks(document).into_iter()
            .filter(|block| declaration_marker(&block.info).is_some()).map(|block| block.source).collect();
        let records = extract_from_markdown_in(document.body, model);
        if records.len() != locations.len() {
            return Err(error(VocabularyErrorKind::InvalidDeclaration, "instance extraction and source spans disagree", &locations));
        }
        for (record, source) in records.into_iter().zip(locations) {
            let entity = record.map_err(|e| error(VocabularyErrorKind::InvalidDeclaration, e.to_string(), std::slice::from_ref(&source)))?;
            let key = (model.expand(&format!("{}:", entity.uri.scheme)), entity.uri.class.clone());
            let concept = classes.get(&key).ok_or_else(|| error(VocabularyErrorKind::UnknownTerm,
                format!("unknown concept {}:{}", entity.uri.scheme, entity.uri.class), std::slice::from_ref(&source)))?.clone();
            let iri = model.expand(&entity.uri.to_string());
            if let Some(prior) = instance_sources.insert(iri.clone(), source.clone()) {
                return Err(error(VocabularyErrorKind::Duplicate, format!("duplicate instance {iri}"), &[prior, source]));
            }
            instances.push(DeclaredInstance { iri, concept, source, entity });
        }
    }
    validate_relationships(model, &instances)?;
    Ok(instances)
}

/// Collect definitions first, then validate the complete instance collection.
pub fn load_documents(
    documents: &[DocumentSource<'_>],
    base: &OntologyModel,
) -> Result<DocumentVocabulary, VocabularyError> {
    let mut vocabulary = load_definitions(documents, base)?;
    vocabulary.instances = collect_instances(documents, &vocabulary.model)?;
    vocabulary.relationships = validate_relationships(&vocabulary.model, &vocabulary.instances)?;
    Ok(vocabulary)
}


```

## Definition ownership and imports

Every term names its owning module. Cross-module domain and range
references require an explicit import path through the selected modules.
The collector never downloads an import.

<a name="chunk-validate-definitions"></a><sub>[`src/document_vocabulary.rs`](../../crates/x0k-folio/src/document_vocabulary.rs) · `#validate-definitions`</sub>

```rust {#validate-definitions}
fn declared_terms(model: &OntologyModel) -> BTreeSet<String> {
    model.facts().iter().filter(|fact| fact.predicate == RDF_TYPE
        && matches!(&fact.value, OntologyValue::Entity(kind) if kind == OWL_CLASS || kind == OWL_OBJECT_PROPERTY))
        .map(|fact| fact.entity.clone()).collect()
}

fn validate_definition_block(model: &OntologyModel, source: &BlockSource) -> Result<(), VocabularyError> {
    if model.facts().iter().any(|f| f.entity.starts_with(STRUCTURAL_NODE_PREFIX)) {
        return Err(error(VocabularyErrorKind::Unsupported, "document definitions currently require named RDF nodes", std::slice::from_ref(source)));
    }
    for fact in model.facts().iter().filter(|fact| fact.predicate == RDF_TYPE) {
        if !matches!(&fact.value, OntologyValue::Entity(kind)
            if kind == OWL_ONTOLOGY || kind == OWL_CLASS || kind == OWL_OBJECT_PROPERTY) {
            return Err(error(VocabularyErrorKind::Unsupported,
                format!("document definition has unsupported RDF type {:?}", fact.value), std::slice::from_ref(source)));
        }
    }
    for iri in declared_terms(model) {
        for predicate in [RDFS_DOMAIN, RDFS_RANGE] {
            let values: BTreeSet<_> = model.facts().iter()
                .filter(|fact| fact.entity == iri && fact.predicate == predicate).map(|fact| &fact.value).collect();
            if values.len() > 1 {
                return Err(error(VocabularyErrorKind::Unsupported,
                    format!("{iri} has multiple {predicate} values"), std::slice::from_ref(source)));
            }
        }
        let owners: BTreeSet<_> = model.facts().iter().filter(|f| f.entity == iri && f.predicate == RDFS_IS_DEFINED_BY)
            .map(|f| &f.value).collect();
        if owners.len() != 1 {
            return Err(error(VocabularyErrorKind::InvalidDeclaration, format!("{iri} must name one rdfs:isDefinedBy module"), std::slice::from_ref(source)));
        }
    }
    let ontologies: BTreeSet<_> = model.facts().iter().filter(|f| f.predicate == RDF_TYPE
        && f.value == OntologyValue::Entity(OWL_ONTOLOGY.into())).map(|f| f.entity.clone()).collect();
    for iri in ontologies {
        if !model.modules().iter().any(|module| module.iri == iri && module.namespace.is_some()) {
            return Err(error(VocabularyErrorKind::InvalidDeclaration, format!("{iri} needs a namespace URI and prefix"), std::slice::from_ref(source)));
        }
    }
    Ok(())
}

fn validate_term_modules(
    model: &OntologyModel,
    definitions: &BTreeMap<String, (BTreeSet<OntologyFact>, Vec<BlockSource>)>,
) -> Result<(), VocabularyError> {
    let modules: BTreeMap<_, _> = model.modules().into_iter().map(|m| (m.iri.clone(), m)).collect();
    let classes: BTreeSet<_> = model.classes().iter().map(|class| model.expand(&class.uri)).collect();
    for (iri, (_, sources)) in definitions {
        let owners = model.defining_modules(iri);
        let owner = owners.first().and_then(|owner| modules.get(owner))
            .ok_or_else(|| error(VocabularyErrorKind::UnknownTerm, format!("{iri} names a module outside the selected collection"), sources))?;
        if owners.len() != 1 {
            return Err(error(VocabularyErrorKind::InvalidDeclaration, format!("{iri} must have one defining module"), sources));
        }
        if model.compact(iri).is_none() {
            return Err(error(VocabularyErrorKind::UnknownTerm, format!("{iri} has no declared namespace"), sources));
        }
        let closure = model.import_closure(&owner.iri);
        for predicate in [RDFS_DOMAIN, RDFS_RANGE] {
            for target in model.class_references(iri, predicate) {
                if !classes.contains(&target) {
                    return Err(error(VocabularyErrorKind::UnknownTerm, format!("{iri} {predicate} names undeclared class {target}"), sources));
                }
                let target_owners = model.defining_modules(&target);
                if !target_owners.iter().any(|owner| closure.contains(owner)) {
                    return Err(error(VocabularyErrorKind::UnknownTerm, format!("{iri} uses {target} without importing its defining module"), sources));
                }
            }
        }
    }
    Ok(())
}

fn class_index(model: &OntologyModel) -> Result<BTreeMap<(String, String), String>, String> {
    let mut out = BTreeMap::new();
    for class in model.classes() {
        let (prefix, local) = class.uri.split_once(':').ok_or("class has no namespace")?;
        let key = (model.expand(&format!("{prefix}:")), camel_to_kebab(local));
        let iri = model.expand(&class.uri);
        if let Some(prior) = out.insert(key, iri.clone()) {
            if prior != iri { return Err(format!("class marker collision between {prior} and {iri}")); }
        }
    }
    Ok(out)
}


```

## Relationships retain their types

An object property connects instance identities. Its domain constrains
the source, and its range constrains a target when that target is present.
An absent target remains a relationship whose resolution is unknown.

<a name="chunk-validate-relationships"></a><sub>[`src/document_vocabulary.rs`](../../crates/x0k-folio/src/document_vocabulary.rs) · `#validate-relationships`</sub>

```rust {#validate-relationships}
/// Resolve a declared property; bare camelCase and snake_case keys retain x0k compatibility.
pub fn resolve_property(model: &OntologyModel, key: &str) -> Option<String> {
    model.object_properties().iter().find_map(|property| {
        let iri = model.expand(&property.uri);
        let legacy = iri.strip_prefix(X0K_NS).is_some_and(|local|
            local == key || x0k_ontology::concept_facts::camel_to_snake(local) == key);
        (iri == model.expand(key) || (!key.contains(':') && legacy)).then_some(iri)
    })
}
/// Resolve a selected CURIE or an explicit HTTP(S)/URN IRI.
/// Other compact prefixes remain errors rather than becoming invented namespaces.
pub fn resolve_reference(model: &OntologyModel, target: &str) -> Result<String, String> {
    if target.starts_with("https:") || target.starts_with("http:") || target.starts_with("urn:") {
        return x0k_ontology::load::is_absolute_iri(target)
            .then(|| target.to_string()).ok_or_else(|| format!("invalid absolute IRI {target:?}"));
    }
    crate::entity_id::EntityId::parse_in(model, target)
        .map(|identity| model.expand(&identity.to_string())).map_err(|error| error.to_string())
}
pub fn validate_relationships(model: &OntologyModel, instances: &[DeclaredInstance]) -> Result<Vec<Relationship>, VocabularyError> {
    let mut out = Vec::new();
    let by_id: BTreeMap<_, _> = instances.iter().map(|instance| (&instance.iri, &instance.concept)).collect();
    let properties: BTreeSet<_> = model.object_properties().iter().map(|property| model.expand(&property.uri)).collect();
    for instance in instances {
        let Some(edges) = instance.entity.yaml.get(serde_norway::Value::String("edges".into())) else { continue; };
        let edges = edges.as_mapping().ok_or_else(|| error(VocabularyErrorKind::InvalidDeclaration, "edges must be a mapping", std::slice::from_ref(&instance.source)))?;
        for (key, targets) in edges {
            let predicate = key.as_str().ok_or_else(|| error(VocabularyErrorKind::InvalidDeclaration, "edge predicate must be a string", std::slice::from_ref(&instance.source)))?;
            let predicate = resolve_property(model, predicate).unwrap_or_else(|| model.expand(predicate));
            if !properties.contains(&predicate) {
                return Err(error(VocabularyErrorKind::UnknownTerm, format!("unknown object property {predicate}"), std::slice::from_ref(&instance.source)));
            }
            let constraints = |kind: &str| -> Vec<String> { model.facts().iter().filter_map(|f| {
                if f.entity == predicate && f.predicate == kind {
                    if let OntologyValue::Entity(value) = &f.value { return Some(value.clone()); }
                }
                None
            }).collect() };
            let domain = constraints(RDFS_DOMAIN);
            let range = constraints(RDFS_RANGE);
            if domain.len() > 1 || range.len() > 1 {
                return Err(error(VocabularyErrorKind::Unsupported, format!("{predicate}: multiple domains/ranges are not supported by document instance validation"), std::slice::from_ref(&instance.source)));
            }
            // Resolve an existing union through the shared model; its structural
            // RDF node is not itself an allowed class. Regression: supplied_union_domain_and_range_accept_members_and_reject_outsiders.
            let allowed_domain = model.class_references(&predicate, RDFS_DOMAIN);
            let allowed_range = model.class_references(&predicate, RDFS_RANGE);
            for (declared, allowed) in [(&domain, &allowed_domain), (&range, &allowed_range)] {
                if !declared.is_empty() && (allowed.is_empty() || allowed.iter().any(|class| class.starts_with(STRUCTURAL_NODE_PREFIX))) {
                    return Err(error(VocabularyErrorKind::Unsupported, format!("{predicate}: unsupported class constraint"), std::slice::from_ref(&instance.source)));
                }
            }
            if !domain.is_empty() && !allowed_domain.contains(&instance.concept) {
                return Err(error(VocabularyErrorKind::InvalidDeclaration, format!("{predicate} does not accept {}", instance.concept), std::slice::from_ref(&instance.source)));
            }
            let targets = targets.as_sequence().ok_or_else(|| error(VocabularyErrorKind::InvalidDeclaration, "edge targets must be a sequence", std::slice::from_ref(&instance.source)))?;
            for target in targets {
                let target = target.as_str().ok_or_else(|| error(VocabularyErrorKind::InvalidDeclaration, "edge target must be an identity string", std::slice::from_ref(&instance.source)))?;
                let object = resolve_reference(model, target)
                    .map_err(|e| error(VocabularyErrorKind::InvalidDeclaration, e, std::slice::from_ref(&instance.source)))?;
                if let Some(actual) = by_id.get(&object) {
                    if !range.is_empty() && !allowed_range.contains(actual) {
                        let required = allowed_range.join(", ");
                        return Err(error(VocabularyErrorKind::InvalidDeclaration, format!("{predicate} target {object} has type {actual}, expected {required}"), std::slice::from_ref(&instance.source)));
                    }
                }
                out.push(Relationship { subject: instance.iri.clone(), predicate: predicate.clone(), object, source: instance.source.clone() });
            }
        }
    }
    Ok(out)
}

```

Supplied vocabularies may use a union of named classes in one domain or
range. Validation uses the model's existing union resolver and accepts an
explicit member; it does not infer subclass membership. Multiple domain
axioms and nested anonymous constraints remain unsupported. Document-carried
definitions still require named nodes.

The returned model and declarations are values. A database adapter can
project them without owning parsing or inventing a second vocabulary.

## The Paper collection

These tests change definitions, input order and namespace spelling while
checking the normalized identities and source locations.

<a name="chunk-tests"></a><sub>[`src/document_vocabulary.rs`](../../crates/x0k-folio/src/document_vocabulary.rs) · `#tests`</sub>

```rust {#tests}

#[cfg(test)]
mod tests {
    use super::*;

    fn ttl(prefix: &str, namespace: &str) -> String {
        format!(r#"@prefix owl: <http://www.w3.org/2002/07/owl#> .
@prefix rdfs: <http://www.w3.org/2000/01/rdf-schema#> .
@prefix vann: <http://purl.org/vocab/vann/> .
@prefix p: <{namespace}> .
<https://example.test/module/{prefix}> a owl:Ontology ;
    vann:preferredNamespacePrefix "{prefix}" ;
    vann:preferredNamespaceUri "{namespace}" .
p:Paper a owl:Class ; rdfs:isDefinedBy <https://example.test/module/{prefix}> .
p:cites a owl:ObjectProperty ; rdfs:domain p:Paper ; rdfs:range p:Paper ;
    rdfs:isDefinedBy <https://example.test/module/{prefix}> .
"#)
    }

    fn definition(prefix: &str, namespace: &str) -> String {
        format!("# Vocabulary\n\n```turtle folio:ontology\n{}\n```\n", ttl(prefix, namespace))
    }

    fn instance(prefix: &str, name: &str, target: Option<&str>) -> String {
        let edges = target.map(|target| format!("edges:\n  {prefix}:cites: [{target}]\n")).unwrap_or_default();
        format!("## {name}\nA paper.\n\n```yaml {prefix}:paper\nid: {prefix}:paper/{name}\n{edges}```\n")
    }

    fn load<'a>(docs: &'a [(&'a str, &'a str)]) -> Result<DocumentVocabulary, VocabularyError> {
        let docs: Vec<_> = docs.iter().map(|(id, body)| DocumentSource { id, body }).collect();
        load_documents(&docs, &OntologyModel::new([]))
    }


    #[test]
    fn supplied_union_domain_and_range_accept_members_and_reject_outsiders() {
        let text = ttl("paper", "https://example.test/paper#")
            .replace("rdfs:domain p:Paper ; rdfs:range p:Paper",
                "rdfs:domain [ owl:unionOf (p:Paper p:Note) ] ; rdfs:range [ owl:unionOf (p:Paper p:Note) ]")
            + "\np:Note a owl:Class ; rdfs:isDefinedBy <https://example.test/module/paper> .\np:Other a owl:Class ; rdfs:isDefinedBy <https://example.test/module/paper> .\n";
        let base = OntologyModel::parse_turtle_sources(&[TurtleSource { name: Path::new("selected.ttl"), text: &text }]).unwrap();
        let note = instance("paper", "note-one", Some("paper:paper/target"))
            .replace("yaml paper:paper", "yaml paper:note").replace("id: paper:paper/note-one", "id: paper:note/note-one");
        let paper = instance("paper", "target", Some("paper:note/note-one"));
        let docs = [DocumentSource { id: "note.md", body: &note }, DocumentSource { id: "paper.md", body: &paper }];
        assert_eq!(load_documents(&docs, &base).unwrap().relationships.len(), 2);
        let other = note.replace("paper:note", "paper:other");
        let error = load_documents(&[DocumentSource { id: "other.md", body: &other }], &base).err().unwrap();
        assert!(error.message.contains("does not accept"));
        let other_target = instance("paper", "target", None)
            .replace("yaml paper:paper", "yaml paper:other").replace("id: paper:paper/target", "id: paper:other/target");
        let note = note.replace("paper:paper/target", "paper:other/target");
        let error = load_documents(&[DocumentSource { id: "note.md", body: &note },
            DocumentSource { id: "other.md", body: &other_target }], &base).err().unwrap();
        assert!(error.message.contains("expected"));
    }

    #[test]
    fn svg_icons_do_not_become_generic_ontology_instances() {
        let body=format!("{}{}\n```svg x0k:icon\n<svg viewBox=\"0 0 16 16\"/>\n```\n",
            definition("paper","https://example.test/paper#"),instance("paper","alpha",None));
        let loaded=load(&[("mixed",&body)]).unwrap();
        assert_eq!(loaded.instances.len(),1);
        assert_eq!(loaded.instances[0].concept,"https://example.test/paper#Paper");
        let unknown=format!("{body}\n## Unknown\n\n```yaml paper:missing\nid: paper:missing/bad\n```\n");
        let error=load(&[("mixed",&unknown)]).err().unwrap();
        assert_eq!(error.kind,VocabularyErrorKind::UnknownTerm);
        assert!(error.message.contains("paper:missing"));
        assert!(unknown[error.sources[0].bytes.clone()].contains("yaml paper:missing"));
    }

    #[test]
    fn public_software_slice_validates_enabled_by_without_importing_product() {
        use x0k_ontology::load::TurtleSource;
        let sources:Vec<_>=["core","document","software"].into_iter().flat_map(|module| {
            let mut sources=vec![TurtleSource {name:std::path::Path::new(module),
                text:x0k_ontology::checked_module_file(module).unwrap()}];
            if let Some(text)=x0k_ontology::checked_shape_file(module) {
                sources.push(TurtleSource {name:std::path::Path::new(module),text});
            }
            sources
        }).collect();
        let base=OntologyModel::parse_turtle_sources(&sources).unwrap();
        assert_eq!(base.modules().len(),3);
        assert!(!base.classes().iter().any(|class|base.expand(&class.uri)=="https://0k.computer/ontology#SoftwareModule"));
        assert!(base.facts().iter().any(|fact|fact.entity=="https://0k.computer/ontology#enabledBy"
            && fact.predicate=="https://0k.computer/ontology#targetClass"
            && matches!(&fact.value,OntologyValue::Entity(target) if target=="https://0k.computer/ontology#SoftwareModule")));
        let source="---\nx0k:\n  format: folio/v1\n  id: x0k:design/example\n  type: design\n---\n# Example\n\n## Read a document\nRead it.\n\n```yaml x0k:affordance\nid: x0k:affordance/read\nedges:\n  enabledBy: [x0k:software-module/x0k-folio]\n```\n";
        let declarations=load_documents(
            &[DocumentSource {id:"example.md",body:source}],&base).unwrap();
        assert_eq!(declarations.relationships[0].predicate,"https://0k.computer/ontology#enabledBy");
        assert_eq!(declarations.instances[0].concept,"https://0k.computer/ontology#Affordance");
    }

    #[test]
    fn definitions_and_instances_share_a_document_and_cross_file_order_is_irrelevant() {
        let alpha = instance("paper", "alpha", Some("paper:paper/beta"));
        let beta = format!("{}{}", definition("paper", "https://example.test/paper#"),
            instance("paper", "beta", None));
        for docs in [[("alpha.md", alpha.as_str()), ("beta.md", beta.as_str())],
            [("beta.md", beta.as_str()), ("alpha.md", alpha.as_str())]] {
            let collected = load(&docs).unwrap();
            assert_eq!(collected.instances.len(), 2);
            let relation = &collected.relationships[0];
            assert_eq!(relation.predicate, "https://example.test/paper#cites");
            assert_eq!(relation.subject, "https://example.test/paper#paper/alpha");
            assert_eq!(relation.object, "https://example.test/paper#paper/beta");
            let definition = collected.definitions.iter().find(|d| d.iri.ends_with("#Paper")).unwrap();
            assert_eq!(definition.sources[0].document, "beta.md");
            assert!(beta[definition.sources[0].bytes.clone()].contains("turtle folio:ontology"));
        }
    }

    #[test]
    fn external_references_preserve_iris_without_accepting_unknown_compact_prefixes() {
        let vocabulary = definition("paper", "https://example.test/paper#");
        for target in ["https://example.org/article/42#abstract", "urn:isbn:9780000000000"] {
            let body = format!("{vocabulary}{}", instance("paper", "one", Some(target)));
            let result = load(&[("article.md", &body)]).unwrap();
            assert_eq!(result.relationships[0].object, target);
        }
        for target in ["pape:paper/two", "https://bad IRI", "../article"] {
            let body = format!("{vocabulary}{}", instance("paper", "one", Some(target)));
            assert!(load(&[("article.md", &body)]).is_err());
        }
    }

    #[test]
    fn aliases_expand_to_one_identity_but_same_local_names_in_other_namespaces_stay_distinct() {
        let mut vocabulary = ttl("paper", "https://example.test/paper#");
        vocabulary.push_str("<https://example.test/module/paper> <http://purl.org/vocab/vann/preferredNamespacePrefix> \"papers\" .");
        let one = format!("```turtle folio:ontology\n{vocabulary}\n```\n{}", instance("papers", "one", None));
        let two = format!("{}{}", definition("archive", "https://example.test/archive#"), instance("archive", "two", None));
        let result = load(&[("one.md", &one), ("two.md", &two)]).unwrap();
        assert_eq!(result.instances[0].concept, "https://example.test/paper#Paper");
        assert_eq!(result.instances[1].concept, "https://example.test/archive#Paper");
        assert_eq!(result.model.expand("paper:Paper"), result.model.expand("papers:Paper"));
        let duplicate = format!("{one}{}", instance("paper", "one", None));
        let error = load(&[("one.md", &duplicate)]).err().unwrap();
        assert_eq!(error.kind, VocabularyErrorKind::Duplicate);
        assert!(error.message.contains("duplicate instance"));
        assert_eq!(error.sources.len(), 2);
    }

    #[test]
    fn repeated_definitions_ignore_triple_order_and_keep_both_sources() {
        let first = definition("paper", "https://example.test/paper#");
        let second = first.replace("rdfs:domain p:Paper ; rdfs:range p:Paper", "rdfs:range p:Paper ; rdfs:domain p:Paper");
        let result = load(&[("a.md", &first), ("b.md", &second)]).unwrap();
        assert!(result.definitions.iter().all(|d| d.sources.len() == 2));
        let changed = second.replace("rdfs:range p:Paper", "rdfs:range p:Other");
        let error = load(&[("a.md", &first), ("b.md", &changed)]).err().unwrap();
        assert_eq!(error.kind, VocabularyErrorKind::Conflict);
        assert!(error.message.contains("conflicting definition"));
        assert_eq!(error.sources.iter().map(|s| s.document.as_str()).collect::<Vec<_>>(), vec!["a.md", "b.md"]);
    }

    #[test]
    fn prefix_conflicts_and_missing_imports_are_explicit() {
        let first = definition("paper", "https://example.test/paper#");
        let second = definition("paper", "https://elsewhere.test/paper#");
        let error = load(&[("a.md", &first), ("b.md", &second)]).err().unwrap();
        assert!(error.message.contains("namespace prefix"));
        assert_eq!(error.sources.len(), 2);
        let missing = first.replace("a owl:Ontology ;", "a owl:Ontology ; owl:imports <https://example.test/missing> ;");
        let error = load(&[("missing.md", &missing)]).err().unwrap();
        assert!(error.message.contains("not a module"));
    }

    #[test]
    fn removal_and_unknown_class_cannot_silently_reclassify_instances() {
        let vocabulary = definition("paper", "https://example.test/paper#");
        let record = instance("paper", "one", None);
        assert!(load(&[("vocab.md", &vocabulary), ("one.md", &record)]).is_ok());
        assert!(load(&[("one.md", &record)]).is_err());
        let changed = vocabulary.replace("p:Paper", "p:Other");
        let error = load(&[("vocab.md", &changed), ("one.md", &record)]).err().unwrap();
        assert_eq!(error.kind, VocabularyErrorKind::UnknownTerm);
        assert!(error.message.contains("unknown concept"));
    }

    #[test]
    fn tutorials_do_not_declare_and_typed_turtle_literals_are_refused() {
        let tutorial = definition("paper", "https://example.test/paper#").replace("turtle folio:ontology", "turtle");
        assert!(load(&[("tutorial.md", &tutorial)]).unwrap().definitions.is_empty());
        let invalid_iri = definition("paper", "https://example.test/paper#")
            .replace("vann:preferredNamespaceUri \"https://example.test/paper#\"", "vann:preferredNamespaceUri \"relative/path\"");
        let error = load(&[("invalid-iri.md", &invalid_iri)]).err().unwrap();
        assert_eq!(error.kind, VocabularyErrorKind::InvalidDeclaration);
        assert!(error.message.contains("absolute IRI"));
        let invalid_turtle = "
```turtle folio:ontology
not turtle
```";
        assert_eq!(load(&[("invalid.md", invalid_turtle)]).err().unwrap().kind, VocabularyErrorKind::Parse);
        let malformed = definition("paper", "https://example.test/paper#").replace(
            "p:Paper a owl:Class", "p:Paper rdfs:label 42 ; a owl:Class");
        let error = load(&[("bad.md", &malformed)]).err().unwrap();
        assert_eq!(error.kind, VocabularyErrorKind::Unsupported);
        assert!(error.message.contains("non-string literal"));
        assert_eq!(error.sources[0].document, "bad.md");
    }

    #[test]
    fn unsupported_shapes_and_types_are_refused_even_without_instances() {
        let vocabulary = definition("paper", "https://example.test/paper#");
        for unsupported in [
            vocabulary.replace("rdfs:range p:Paper", "rdfs:range p:Paper, p:Other"),
            vocabulary.replace("owl:ObjectProperty", "owl:DatatypeProperty"),
            vocabulary.replace("rdfs:range p:Paper", "rdfs:range [ owl:unionOf (p:Paper) ]"),
        ] {
            let error = load(&[("vocabulary.md", &unsupported)]).err().unwrap();
            assert_eq!(error.kind, VocabularyErrorKind::Unsupported);
            assert_eq!(error.sources[0].document, "vocabulary.md");
        }
    }

    #[test]
    fn definitions_need_selected_owners_and_imports_for_cross_module_ranges() {
        let paper = definition("paper", "https://example.test/paper#");
        let archive = definition("archive", "https://example.test/archive#");
        let cross = paper.replace("rdfs:range p:Paper", "rdfs:range <https://example.test/archive#Paper>");
        let error = load(&[("p.md", &cross), ("a.md", &archive)]).err().unwrap();
        assert!(error.message.contains("without importing"));
        let imported = cross.replace("a owl:Ontology ;",
            "a owl:Ontology ; owl:imports <https://example.test/module/archive> ;");
        assert!(load(&[("p.md", &imported), ("a.md", &archive)]).is_ok());
        let missing_owner = paper.replace("rdfs:isDefinedBy <https://example.test/module/paper>",
            "rdfs:isDefinedBy <https://example.test/module/missing>");
        assert!(load(&[("p.md", &missing_owner)]).err().unwrap().message.contains("outside the selected collection"));
    }

    #[test]
    fn colliding_kebab_names_and_wrong_relationship_ranges_are_refused() {
        let vocabulary = definition("paper", "https://example.test/paper#");
        let other = definition("archive", "https://example.test/archive#");
        let first = format!("{vocabulary}{}", instance("paper", "one", Some("archive:paper/two")));
        let second = format!("{other}{}", instance("archive", "two", None));
        let error = load(&[("one.md", &first), ("two.md", &second)]).err().unwrap();
        assert!(error.message.contains("expected https://example.test/paper#Paper"));
        let collision = vocabulary.replace("p:cites a owl:ObjectProperty",
            "p:paper a owl:Class ; rdfs:isDefinedBy <https://example.test/module/paper> .\np:cites a owl:ObjectProperty");
        let error = load(&[("vocab.md", &collision)]).err().unwrap();
        assert!(error.message.contains("class marker collision"));
    }
}

```
