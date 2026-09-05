---
x0k:
  format: folio/v1
  id: x0k:implementation/ontology/concept-facts
  type: implementation
  status: draft
  summary: Why the vocabulary lives as facts in the concept region rather than compiled from a schema file, and how the module files are materialized back out of it.
  concerns: [ontology, facts, projection, turtle, materialized-view]
  tangle:
    crate: x0k-ontology
    root: src/concept_facts.rs
  edges:
    constrained_by:
      - x0k:architecture/state-representation
    cites:
      - x0k:implementation/entry-spine/spine
    motivated_by:
      - x0k:intent/ba2f3043-cb4f-4ec6-87c5-d58b5d71e30b
    presupposes:
      - x0k:wiki/rdf-and-owl
      - x0k:wiki/open-world-assumption
---
# Concepts Are Facts

An ontology stops being extensible by assertion when its vocabulary is
compiled directly out of a schema file. The file may be pleasant to review,
but it becomes a registry that must change before the running system can
recognize a new concept. The fact plane needs the opposite authority: a
concept is an anchor in the log, described by the same typed facts as every
other entity, and files and Rust tables are views of the fold.

One example carries the mechanism. `x0k:Intent` is an anchor with ordinary
facts saying that it is an `owl:Class` and an `x0k:Concept`, that its label is
"Intent", and that its prose description is an `rdfs:comment`. The root
`x0k:Concept` has the same shape, except its concept typing points back to
itself. Object properties are anchors of the same kind: `x0k:dependsOn` is an
`owl:ObjectProperty` and an `x0k:Concept`, with domain and range facts. RDF
list nodes used by union domains remain facts too; they merely use stable
skolem URIs in the log and blank-node labels in the Turtle view.

A vocabulary module is one more anchor of the same kind
(`x0k:architecture/ontology-modules` §1): `<https://0k.computer/ontology/work>` is
an `owl:Ontology` whose `owl:imports` name the modules it builds on, and
every term it defines says so with `rdfs:isDefinedBy`. Membership is that
fact and nothing else — not the term's namespace, not a file, not a table.

## The fact vocabulary is small

The model preserves RDF's own predicates rather than introducing a parallel
set of schema-about-schema names. These constants are the vocabulary the fold
needs to recognize classes, properties, descriptions, and RDF lists:

```rust {#vocabulary}
use std::collections::{BTreeMap, BTreeSet};

pub const X0K_NS: &str = "https://0k.computer/ontology#";
pub const CONCEPT_URI: &str = "https://0k.computer/ontology#Concept";
pub const MODULE_IRI_PREFIX: &str = "https://0k.computer/ontology/";
pub const STRUCTURAL_NODE_PREFIX: &str = "urn:x0k:ontology:blank:";

pub const RDF_TYPE: &str = "http://www.w3.org/1999/02/22-rdf-syntax-ns#type";
pub const RDF_FIRST: &str = "http://www.w3.org/1999/02/22-rdf-syntax-ns#first";
pub const RDF_REST: &str = "http://www.w3.org/1999/02/22-rdf-syntax-ns#rest";
pub const RDF_NIL: &str = "http://www.w3.org/1999/02/22-rdf-syntax-ns#nil";
pub const RDFS_LABEL: &str = "http://www.w3.org/2000/01/rdf-schema#label";
pub const RDFS_COMMENT: &str = "http://www.w3.org/2000/01/rdf-schema#comment";
pub const RDFS_SUBCLASS_OF: &str = "http://www.w3.org/2000/01/rdf-schema#subClassOf";
pub const RDFS_DOMAIN: &str = "http://www.w3.org/2000/01/rdf-schema#domain";
pub const RDFS_RANGE: &str = "http://www.w3.org/2000/01/rdf-schema#range";
pub const RDFS_IS_DEFINED_BY: &str = "http://www.w3.org/2000/01/rdf-schema#isDefinedBy";
pub const OWL_CLASS: &str = "http://www.w3.org/2002/07/owl#Class";
pub const OWL_OBJECT_PROPERTY: &str = "http://www.w3.org/2002/07/owl#ObjectProperty";
pub const OWL_DATATYPE_PROPERTY: &str = "http://www.w3.org/2002/07/owl#DatatypeProperty";
pub const OWL_ANNOTATION_PROPERTY: &str = "http://www.w3.org/2002/07/owl#AnnotationProperty";
pub const OWL_UNION_OF: &str = "http://www.w3.org/2002/07/owl#unionOf";
pub const OWL_ONTOLOGY: &str = "http://www.w3.org/2002/07/owl#Ontology";
pub const OWL_IMPORTS: &str = "http://www.w3.org/2002/07/owl#imports";
pub const VANN_PREFERRED_NAMESPACE_URI: &str = "http://purl.org/vocab/vann/preferredNamespaceUri";

pub const X0K_TARGET_CLASS: &str = "https://0k.computer/ontology#targetClass";
pub const X0K_SUBJECT_CLASS: &str = "https://0k.computer/ontology#subjectClass";
```

A module IRI is the module prefix plus a bare name; an extension's own
namespace (`https://0k.computer/ontology/aec#`) shares the prefix but carries a
`#`, so the distinction is one test, shared by the region fold and the
materializer:

```rust {#module-iri}
/// The module name of a module IRI (`https://0k.computer/ontology/work` → `work`),
/// or `None` for anything else under the prefix, such as a term in an
/// extension's namespace.
pub fn module_name(iri: &str) -> Option<&str> {
    let name = iri.strip_prefix(MODULE_IRI_PREFIX)?;
    let well_formed = name.chars().next().is_some_and(|ch| ch.is_ascii_lowercase())
        && name.chars().all(|ch| ch.is_ascii_lowercase() || ch.is_ascii_digit() || ch == '-');
    well_formed.then_some(name)
}
```

The substrate-neutral value is deliberately narrower than arbitrary RDF. The
checked vocabulary uses entity references and string literals, exactly the
two `FactValue` variants the folio path projects. A future typed literal is a
new fact-plane value decision, not something this view silently erases:

```rust {#fact-value}
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum OntologyValue {
    Text(String),
    Entity(String),
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct OntologyFact {
    pub entity: String,
    pub predicate: String,
    pub value: OntologyValue,
}
```

Constructors keep the generated bootstrap projection terse while leaving the
three stored fields completely visible to folds:

```rust {#fact-constructors}
impl OntologyFact {
    pub fn text(entity: impl Into<String>, predicate: impl Into<String>, value: impl Into<String>) -> Self {
        Self { entity: entity.into(), predicate: predicate.into(), value: OntologyValue::Text(value.into()) }
    }

    pub fn entity(entity: impl Into<String>, predicate: impl Into<String>, value: impl Into<String>) -> Self {
        Self { entity: entity.into(), predicate: predicate.into(), value: OntologyValue::Entity(value.into()) }
    }
}
```

## A fold makes the registry views

`OntologyModel` is not a store. It is the deterministic fold over whatever
fact source the caller already has: generated bootstrap facts at build time,
or `FactEntry` values folded from the entry spine at runtime. Duplicate
assertions collapse without reordering the first observation:

```rust {#model}
#[derive(Debug, Clone)]
pub struct OntologyModel {
    facts: Vec<OntologyFact>,
}

impl OntologyModel {
    pub fn new(facts: impl IntoIterator<Item = OntologyFact>) -> Self {
        let mut seen = BTreeSet::new();
        let facts = facts.into_iter().filter(|fact| seen.insert(fact.clone())).collect();
        Self { facts }
    }

    pub fn facts(&self) -> &[OntologyFact] {
        &self.facts
    }
}
```

The bootstrap projection has one permitted addition to the historical
Turtle: it closes the meta-level. It asserts the root class and makes every
declared class or property a concept. Once these assertions are in the log,
the method is no longer involved; the fold is authoritative:

```rust {#close-root}
impl OntologyModel {
    pub fn with_fact_plane_root(mut self) -> Self {
        self.ensure(OntologyFact::entity(CONCEPT_URI, RDF_TYPE, OWL_CLASS));
        self.ensure(OntologyFact::entity(CONCEPT_URI, RDF_TYPE, CONCEPT_URI));
        self.ensure(OntologyFact::text(CONCEPT_URI, RDFS_LABEL, "Concept"));
        self.ensure(OntologyFact::text(
            CONCEPT_URI,
            RDFS_COMMENT,
            "The root ontology concept: every registered class and property is typed by it, and it is typed by itself.",
        ));

        for entity in self.declared_entities() {
            self.ensure(OntologyFact::entity(entity, RDF_TYPE, CONCEPT_URI));
        }
        self
    }

    fn ensure(&mut self, fact: OntologyFact) {
        if !self.facts.contains(&fact) { self.facts.push(fact); }
    }
}
```

Declaration kinds remain RDF/OWL facts. That makes the class and property
views derivable without another enum becoming authoritative:

```rust {#declared-entities}
impl OntologyModel {
    fn declared_entities(&self) -> Vec<String> {
        let kinds = [OWL_CLASS, OWL_OBJECT_PROPERTY, OWL_DATATYPE_PROPERTY, OWL_ANNOTATION_PROPERTY];
        self.facts.iter().filter_map(|fact| match &fact.value {
            OntologyValue::Entity(kind) if fact.predicate == RDF_TYPE && kinds.contains(&kind.as_str()) => Some(fact.entity.clone()),
            _ => None,
        }).collect::<BTreeSet<_>>().into_iter().collect()
    }

    pub fn concept_entities(&self) -> BTreeSet<String> {
        self.entities_typed_as(CONCEPT_URI)
    }

    fn entities_typed_as(&self, kind: &str) -> BTreeSet<String> {
        self.facts.iter().filter_map(|fact| match &fact.value {
            OntologyValue::Entity(value) if fact.predicate == RDF_TYPE && value == kind => Some(fact.entity.clone()),
            _ => None,
        }).collect()
    }
}
```

The owned records below are intermediate views used both by `build.rs` and by
the runtime parity check. They contain no `'static` references and therefore
cannot masquerade as a compiled registry:

```rust {#view-records}
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct ClassRecord {
    pub uri: String,
    pub label: String,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct PropertyRecord {
    pub uri: String,
    pub domain: Option<String>,
    pub range: Option<String>,
}
```

Classes use the established UI label contract: the URI suffix, not the
display-oriented `rdfs:label`. This preserves existing consumers while
moving where the list comes from.

A term's compact name follows its namespace. The kernel and the
infrastructure extension share the base namespace and compact to
`x0k:<Name>`; a domain extension owns a namespace it declares on its
module fact (`vann:preferredNamespaceUri`, `ontology-modules` §2), and a
term there compacts to `<module>:<Name>` — `paracosm:Place` — so two
extensions can never collide and a kernel consumer can tell an
extension's class at a glance. Until 2026-09-05 the fold stripped only
the base namespace and silently dropped everything else, so the first
extension in its own namespace was invisible to every table; the rule
was declared and not exercised.

```rust {#class-view}
impl OntologyModel {
    /// The extension namespaces the modules declare, as `(prefix, namespace)`.
    fn extension_namespaces(&self) -> Vec<(String, String)> {
        self.modules().into_iter()
            .filter_map(|module| module.namespace.map(|namespace| (module.name, namespace)))
            .collect()
    }

    /// The compact name of a term IRI: `x0k:<Name>` in the base namespace,
    /// `<module>:<Name>` in a declared extension namespace, `None` for an
    /// IRI no module claims.
    pub fn compact(&self, iri: &str) -> Option<String> {
        if let Some(local) = iri.strip_prefix(X0K_NS) {
            return Some(format!("x0k:{local}"));
        }
        self.extension_namespaces().into_iter()
            .find_map(|(prefix, namespace)| iri.strip_prefix(namespace.as_str()).map(|local| format!("{prefix}:{local}")))
    }

    /// The inverse of [`compact`](Self::compact); a name with no known
    /// prefix is returned as it came.
    pub fn expand(&self, bare: &str) -> String {
        if let Some(local) = bare.strip_prefix("x0k:") {
            return format!("{X0K_NS}{local}");
        }
        self.extension_namespaces().into_iter()
            .find_map(|(prefix, namespace)| bare.strip_prefix(&format!("{prefix}:")).map(|local| format!("{namespace}{local}")))
            .unwrap_or_else(|| bare.to_string())
    }

    pub fn classes(&self) -> Vec<ClassRecord> {
        self.entities_typed_as(OWL_CLASS).into_iter().filter_map(|full| {
            let uri = self.compact(&full)?;
            let label = uri.rsplit(':').next().unwrap_or(&uri).to_string();
            Some(ClassRecord { uri, label })
        }).collect()
    }

    pub fn object_properties(&self) -> Vec<PropertyRecord> {
        self.entities_typed_as(OWL_OBJECT_PROPERTY).into_iter().filter_map(|full| {
            let uri = self.compact(&full)?;
            Some(PropertyRecord {
                uri,
                domain: self.first_class_value(&full, RDFS_DOMAIN),
                range: self.first_class_value(&full, RDFS_RANGE),
            })
        }).collect()
    }
}
```

Domain and range may point straight at a class or at an OWL union. The fold
walks the same RDF list facts in either the bootstrap model or the spine. The
compatibility table keeps the first member of an RDF list, preserving the
meaning of the old single-slot projection without relying on append order:

```rust {#class-values}
impl OntologyModel {
    fn first_class_value(&self, entity: &str, predicate: &str) -> Option<String> {
        self.facts.iter().filter(|fact| fact.entity == entity && fact.predicate == predicate)
            .filter_map(|fact| match &fact.value { OntologyValue::Entity(value) => Some(value), _ => None })
            .flat_map(|value| self.resolve_class_set(value))
            .next().map(|iri| self.compact(&iri).unwrap_or(iri))
    }

    fn resolve_class_set(&self, node: &str) -> Vec<String> {
        if !node.starts_with(STRUCTURAL_NODE_PREFIX) { return vec![node.to_string()]; }
        self.entity_values(node, OWL_UNION_OF).into_iter()
            .flat_map(|head| self.rdf_list_members(&head)).collect()
    }

    fn entity_values(&self, entity: &str, predicate: &str) -> Vec<String> {
        self.facts.iter().filter_map(|fact| match &fact.value {
            OntologyValue::Entity(value) if fact.entity == entity && fact.predicate == predicate => Some(value.clone()),
            _ => None,
        }).collect()
    }

    fn text_values(&self, entity: &str, predicate: &str) -> Vec<String> {
        self.facts.iter().filter_map(|fact| match &fact.value {
            OntologyValue::Text(value) if fact.entity == entity && fact.predicate == predicate => Some(value.clone()),
            _ => None,
        }).collect()
    }

    /// The classes an axiom names, with `owl:unionOf` lists resolved to
    /// their members. The closure check reads cross-module references here.
    pub fn class_references(&self, entity: &str, predicate: &str) -> Vec<String> {
        self.entity_values(entity, predicate).into_iter()
            .flat_map(|node| self.resolve_class_set(&node)).collect()
    }
}
```

List traversal is defensive against a malformed cycle. The current
vocabulary is well formed, but a view generator must terminate on facts a
peer asserted independently:

```rust {#rdf-list}
impl OntologyModel {
    fn rdf_list_members(&self, head: &str) -> Vec<String> {
        let mut out = Vec::new();
        let mut seen = BTreeSet::new();
        let mut cursor = head.to_string();
        while cursor != RDF_NIL && seen.insert(cursor.clone()) {
            if let Some(first) = self.entity_values(&cursor, RDF_FIRST).into_iter().next() { out.push(first); }
            let Some(rest) = self.entity_values(&cursor, RDF_REST).into_iter().next() else { break };
            cursor = rest;
        }
        out
    }
}
```

The decision-edge table is another projection over the same facts. It keeps
the historical direct-subclass definition and resolves union domains before
testing membership. It reads the subject from two places, and that is not a
convenience: a shape file is part of the T-box union
(`x0k:architecture/vocabulary-shapes` §6), so once clause 8 moves a union
domain out of a term file, a derivation that consulted only `rdfs:domain`
would be reading a truncated union and would silently drop the predicate from
the slice every decision document draws on. `realizes` is the live case.

```rust {#decision-predicates}
impl OntologyModel {
    pub fn decision_edge_predicates(&self) -> Vec<(String, String)> {
        let decision = format!("{X0K_NS}Decision");
        let mut classes = BTreeSet::from([decision.clone()]);
        for fact in &self.facts {
            if fact.predicate == RDFS_SUBCLASS_OF && fact.value == OntologyValue::Entity(decision.clone()) {
                classes.insert(fact.entity.clone());
            }
        }
        self.entities_typed_as(OWL_OBJECT_PROPERTY).into_iter().filter_map(|iri| {
            let in_scope = self.entity_values(&iri, RDFS_DOMAIN).into_iter()
                .flat_map(|node| self.resolve_class_set(&node))
                .chain(self.entity_values(&iri, X0K_SUBJECT_CLASS))
                .any(|class| classes.contains(&class));
            let camel = iri.strip_prefix(X0K_NS)?.to_string();
            in_scope.then(|| (camel_to_snake(&camel), camel))
        }).collect()
    }
}
```

## Modules are declared regions

A module is read off the same facts: the `owl:Ontology` individuals under the
module prefix, each with its sorted imports and, for a domain extension, the
namespace it declares. The record is owned data like the class and property
records, so `build.rs` and the region share one reading:

```rust {#module-records}
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct ModuleRecord {
    pub iri: String,
    pub name: String,
    pub imports: Vec<String>,
    pub namespace: Option<String>,
}

impl OntologyModel {
    pub fn modules(&self) -> Vec<ModuleRecord> {
        self.entities_typed_as(OWL_ONTOLOGY).into_iter().filter_map(|iri| {
            let name = module_name(&iri)?.to_string();
            let mut imports = self.entity_values(&iri, OWL_IMPORTS);
            imports.sort();
            imports.dedup();
            let namespace = self.text_values(&iri, VANN_PREFERRED_NAMESPACE_URI).into_iter().next();
            Some(ModuleRecord { iri, name, imports, namespace })
        }).collect()
    }

    /// The modules an entity says define it. Exactly one for a well-formed
    /// term; the closure check reports every other count.
    pub fn defining_modules(&self, entity: &str) -> Vec<String> {
        self.entity_values(entity, RDFS_IS_DEFINED_BY)
    }

    fn defined_in(&self, module_iri: &str) -> BTreeSet<String> {
        self.facts.iter().filter(|fact| {
            fact.predicate == RDFS_IS_DEFINED_BY && fact.value == OntologyValue::Entity(module_iri.to_string())
        }).map(|fact| fact.entity.clone()).collect()
    }
}
```

The import graph must be a DAG whose every edge lands on a module of the
set. Both failures are named at the module that carries them, because a
publication ships a set and the maker of that set is who can fix it:

```rust {#import-order}
impl OntologyModel {
    /// Modules with imports before importers, or the missing import or cycle
    /// that makes the set unloadable.
    pub fn import_order(&self) -> Result<Vec<ModuleRecord>, String> {
        let modules = self.modules();
        let by_iri: BTreeMap<&str, &ModuleRecord> = modules.iter().map(|module| (module.iri.as_str(), module)).collect();
        let mut state: BTreeMap<String, bool> = BTreeMap::new();
        let mut order = Vec::new();
        for module in &modules {
            visit_module(module, &by_iri, &mut state, &mut order, &mut Vec::new())?;
        }
        Ok(order)
    }

    /// The module and everything it transitively imports; references into
    /// this set are licensed, references outside it are closure errors.
    pub fn import_closure(&self, module_iri: &str) -> BTreeSet<String> {
        let modules = self.modules();
        let mut closure = BTreeSet::new();
        let mut pending = vec![module_iri.to_string()];
        while let Some(iri) = pending.pop() {
            if !closure.insert(iri.clone()) { continue; }
            if let Some(module) = modules.iter().find(|module| module.iri == iri) {
                pending.extend(module.imports.iter().cloned());
            }
        }
        closure
    }
}

fn visit_module(
    module: &ModuleRecord,
    by_iri: &BTreeMap<&str, &ModuleRecord>,
    state: &mut BTreeMap<String, bool>,
    order: &mut Vec<ModuleRecord>,
    path: &mut Vec<String>,
) -> Result<(), String> {
    match state.get(&module.iri) {
        Some(true) => return Ok(()),
        Some(false) => {
            path.push(module.iri.clone());
            return Err(format!("module import cycle: {}", path.join(" -> ")));
        }
        None => {}
    }
    state.insert(module.iri.clone(), false);
    path.push(module.iri.clone());
    for import in &module.imports {
        let Some(target) = by_iri.get(import.as_str()) else {
            return Err(format!("module {} imports {import}, which is not a module of the set", module.iri));
        };
        visit_module(target, by_iri, state, order, path)?;
    }
    path.pop();
    state.insert(module.iri.clone(), true);
    order.push(module.clone());
    Ok(())
}
```

Each compatibility view has a per-module form that is the same view filtered
by declared membership. The Decision-domain test still runs over the whole
model — `software`'s `publishedOn`, whose domain is `Publication`, is a
Decision edge because `Publication` subclasses `Decision` in `document` —
and only the answer is partitioned:

```rust {#module-views}
impl OntologyModel {
    pub fn classes_in(&self, module_iri: &str) -> Vec<ClassRecord> {
        let defined = self.defined_in(module_iri);
        self.classes().into_iter().filter(|class| defined.contains(&self.expand(&class.uri))).collect()
    }

    pub fn object_properties_in(&self, module_iri: &str) -> Vec<PropertyRecord> {
        let defined = self.defined_in(module_iri);
        self.object_properties().into_iter().filter(|property| defined.contains(&self.expand(&property.uri))).collect()
    }

    pub fn decision_edge_predicates_in(&self, module_iri: &str) -> Vec<(String, String)> {
        let defined = self.defined_in(module_iri);
        self.decision_edge_predicates().into_iter()
            .filter(|(_, camel)| defined.contains(&format!("{X0K_NS}{camel}"))).collect()
    }
}
```

A module's file carries the module fact, every axiom of every term it
defines, and the structural nodes those axioms own — found by walking from
the owned entities into the skolem prefix, never by namespace. An instance
that no module claims is therefore unreachable from every file:

```rust {#module-facts}
impl OntologyModel {
    pub fn module_facts(&self, module_iri: &str) -> Vec<&OntologyFact> {
        let mut owners = self.defined_in(module_iri);
        owners.insert(module_iri.to_string());
        let mut pending: Vec<String> = owners.iter().cloned().collect();
        while let Some(entity) = pending.pop() {
            for fact in self.facts.iter().filter(|fact| fact.entity == entity) {
                if let OntologyValue::Entity(value) = &fact.value {
                    if value.starts_with(STRUCTURAL_NODE_PREFIX) && owners.insert(value.clone()) {
                        pending.push(value.clone());
                    }
                }
            }
        }
        self.facts.iter().filter(|fact| owners.contains(&fact.entity)).collect()
    }
}
```

The small naming helpers are shared rather than duplicated between the build
and runtime projections:

```rust {#naming}
pub fn bare_c0k(iri: &str) -> String {
    iri.strip_prefix(X0K_NS).map(|local| format!("x0k:{local}")).unwrap_or_else(|| iri.to_string())
}

pub fn full_c0k(bare: &str) -> String {
    bare.strip_prefix("x0k:").map(|local| format!("{X0K_NS}{local}")).unwrap_or_else(|| bare.to_string())
}

pub fn camel_to_snake(camel: &str) -> String {
    let mut out = String::with_capacity(camel.len() + 4);
    for (index, ch) in camel.chars().enumerate() {
        if index > 0 && ch.is_ascii_uppercase() { out.push('_'); }
        out.push(ch.to_ascii_lowercase());
    }
    out
}
```

## A shape is the module's other file

A term's facts say what the term *is*. A shape says what a well-formed
document does with it — "the target of `publishedBy` should be a `Profile`" —
and that is a different kind of claim about the same subject
(`x0k:architecture/vocabulary-shapes` §1). Both are the module's, so both are
folded from the module's facts; they are separated on the way out, into
`modules/<name>.ttl` and `shapes/<name>.ttl`.

The split is **by predicate**, not by subject, and that is forced rather than
chosen: `module_facts` gathers a module's facts by walking `rdfs:isDefinedBy`
from the subject, so the two claims about `publishedBy` arrive together and
nothing about the subject can tell them apart. The predicate can.

```rust {#shape-predicates}
/// True for a predicate that states a shape rather than a term. A shape is
/// applied to a document, never imported by a term, so it may name a class
/// from any module — which is why the import-closure check (which walks
/// `subClassOf`, `domain` and `range`) must not see one of these.
pub fn is_shape_predicate(predicate: &str) -> bool {
    predicate == X0K_TARGET_CLASS || predicate == X0K_SUBJECT_CLASS
}

impl OntologyModel {
    /// The module's facts minus its shapes: what its terms mean.
    pub fn term_facts(&self, module_iri: &str) -> Vec<&OntologyFact> {
        self.module_facts(module_iri).into_iter().filter(|fact| !is_shape_predicate(&fact.predicate)).collect()
    }

    /// The module's shapes: what a document must do to satisfy them.
    pub fn shape_facts(&self, module_iri: &str) -> Vec<&OntologyFact> {
        self.module_facts(module_iri).into_iter().filter(|fact| is_shape_predicate(&fact.predicate)).collect()
    }
}
```

`x0k:targetClass` and `x0k:subjectClass` are the whole shape vocabulary, and
the reason they are two predicates rather than a language is the shape of the
constraints that asked for them. Most say "the target of this predicate is a
`q`" — `motivatedBy`, `informedBy`, `publishedBy`, `publishedFor`, `excludes` —
and a disjunction is expressed by asserting the fact once per member rather
than by unioning. `subjectClass` arrived with the first union *domain* to leave
a term file: `realizes` is carried by a `Techne`, a `Decision` and a `Genome`,
which live in three different modules, so the domain is a constraint on current
usage rather than what the word means and the property places in `core`
(`x0k:architecture/vocabulary-shapes` clause 8). A shape file therefore holds
no RDF list structure at all, which is checked rather than assumed below: the
flat form is what keeps a shape file readable as a set of independent claims,
and it is what a SHACL encoding would have spent a node apiece to say.

## Turtle is a deterministic view

The materialization uses the N-Triples subset of Turtle: one sorted triple
per line, full IRIs, explicit blank-node list structure. That normalization
trades the old hand-arranged headings for a diff where every changed fact is
one changed line. Stable skolem URIs never leak into the file; they return to
blank labels at this boundary. One file per module for its terms and, when it
has any, one more for its shapes; the third header line names which. Sorting is by line, so the module fact — whose IRI
sorts after the `#`-terms it defines — is found by its IRI, not its line.

The blank labels are a function of the module alone, never of the set it
was folded with. A skolem URI is minted over the whole fold (`build.rs`
numbers structural roots across every file it read), so rendering it as-is
would make `document.ttl` say `_:b0013…` in a build that ships every
module and `_:b0000…` in one that ships only `core` and `document` — which
is exactly what
the first `[core, document]` projection did (2026-09-02), failing the
round-trip test inside the projected repository. So the renderer relabels:
the module's structural roots, in the order of the (subject, predicate)
that owns them, are `b0000`, `b0001`, …; each root's component, walked
breadth-first with children in sorted order, is `n0000`, `n0001`, … — the
same walk `build.rs` makes when it mints the skolems, restricted to the
module. A subject with two structural objects under one predicate would
tie on (subject, predicate) and fall back to skolem order, which is
set-dependent; no term does that, and the projection's own round-trip
test is where it would show.

```rust {#render-view}
impl OntologyModel {
    pub fn render_module(&self, module: &ModuleRecord) -> String {
        render_view(&self.term_facts(&module.iri), &format!("# Module: {}", module.name))
    }

    /// The module's shape file, or `None` when the module constrains nothing.
    /// A module with no shapes materializes no file rather than an empty one,
    /// so the directory listing answers "which modules constrain anything".
    pub fn render_shapes(&self, module: &ModuleRecord) -> Option<String> {
        let facts = self.shape_facts(&module.iri);
        if facts.is_empty() {
            return None;
        }
        for fact in &facts {
            let structural = matches!(&fact.value, OntologyValue::Entity(value) if value.starts_with(STRUCTURAL_NODE_PREFIX));
            assert!(
                !structural,
                "shape fact {} {} points at a structural node; the native shape form is flat — \
                 a disjunction repeats the fact rather than unioning it, so a shape file holds \
                 no list cells and reading one needs no list walk",
                fact.entity, fact.predicate,
            );
        }
        Some(render_view(&facts, &format!("# Shapes: {}", module.name)))
    }
}

/// The two views differ in one header line and in nothing else.
fn render_view(facts: &[&OntologyFact], subject_line: &str) -> String {
    let labels = module_blank_labels(facts);
    let mut lines: Vec<String> = facts.iter().map(|fact| render_fact(fact, &labels)).collect();
    lines.sort();
    lines.dedup();
    let mut out = format!(
        "# @generated from the concept region of the fact plane. DO NOT AUTHOR HERE.\n\
         # Bootstrap source: this view seeds an empty region exactly once.\n\
         {subject_line}\n\n",
    );
    for line in lines { out.push_str(&line); out.push('\n'); }
    out
}

/// Module-local blank labels for every structural node among `facts`.
fn module_blank_labels(facts: &[&OntologyFact]) -> BTreeMap<String, String> {
    let is_structural = |value: &str| value.starts_with(STRUCTURAL_NODE_PREFIX);
    let mut roots: Vec<(&str, &str, &str)> = facts.iter().filter_map(|fact| match &fact.value {
        OntologyValue::Entity(value) if is_structural(value) && !is_structural(&fact.entity) => {
            Some((fact.entity.as_str(), fact.predicate.as_str(), value.as_str()))
        }
        _ => None,
    }).collect();
    roots.sort();
    roots.dedup();
    let mut labels = BTreeMap::new();
    let assign_component = |root: &str, root_index: usize, labels: &mut BTreeMap<String, String>| {
        let mut queue = std::collections::VecDeque::from([root.to_string()]);
        let mut node_index = 0usize;
        while let Some(node) = queue.pop_front() {
            if labels.contains_key(&node) { continue; }
            labels.insert(node.clone(), format!("b{root_index:04}n{node_index:04}"));
            node_index += 1;
            let mut children: Vec<&str> = facts.iter().filter_map(|fact| match &fact.value {
                OntologyValue::Entity(value) if fact.entity == node && is_structural(value) => Some(value.as_str()),
                _ => None,
            }).collect();
            children.sort();
            children.dedup();
            queue.extend(children.into_iter().map(str::to_string));
        }
    };
    for (root_index, (_, _, root)) in roots.iter().enumerate() {
        assign_component(root, root_index, &mut labels);
    }
    // A structural node no owned term reaches is still labelled, after the
    // rooted ones, so the file never holds an unrendered skolem.
    let mut stray: Vec<&str> = facts.iter().flat_map(|fact| {
        let object = match &fact.value {
            OntologyValue::Entity(value) if is_structural(value) => Some(value.as_str()),
            _ => None,
        };
        is_structural(&fact.entity).then_some(fact.entity.as_str()).into_iter().chain(object)
    }).collect();
    stray.sort();
    stray.dedup();
    let mut next_root = roots.len();
    for node in stray {
        if !labels.contains_key(node) {
            assign_component(node, next_root, &mut labels);
            next_root += 1;
        }
    }
    labels
}

fn render_fact(fact: &OntologyFact, labels: &BTreeMap<String, String>) -> String {
    let subject = render_entity(&fact.entity, labels);
    let predicate = format!("<{}>", escape_iri(&fact.predicate));
    let object = match &fact.value {
        OntologyValue::Entity(value) => render_entity(value, labels),
        OntologyValue::Text(value) => format!("\"{}\"", escape_literal(value)),
    };
    format!("{subject} {predicate} {object} .")
}
```

Only syntax-significant characters are escaped. Unicode remains readable in
the review artifact, while control characters use N-Triples escapes:

```rust {#render-atoms}
fn render_entity(value: &str, labels: &BTreeMap<String, String>) -> String {
    if let Some(local) = value.strip_prefix(STRUCTURAL_NODE_PREFIX) {
        return match labels.get(value) {
            Some(label) => format!("_:{label}"),
            None => format!("_:{}", local.replace(|ch: char| !ch.is_ascii_alphanumeric(), "_")),
        };
    }
    format!("<{}>", escape_iri(value))
}

fn escape_iri(value: &str) -> String {
    value.replace('>', "\\u003E")
}

fn escape_literal(value: &str) -> String {
    let mut out = String::with_capacity(value.len());
    for ch in value.chars() {
        match ch {
            '\\' => out.push_str("\\\\"), '"' => out.push_str("\\\""),
            '\n' => out.push_str("\\n"), '\r' => out.push_str("\\r"), '\t' => out.push_str("\\t"),
            ch if ch.is_control() => out.push_str(&format!("\\u{:04X}", ch as u32)),
            ch => out.push(ch),
        }
    }
    out
}
```

The hard boundary is authority, not parsing. On an empty profile the checked
module files are parsed into these facts and asserted; the final assertion is
the self-typed root. Once that root folds, this bootstrap path is never
consulted again for that profile. The same model then runs in the opposite
direction: folded facts produce the module files and the compiler's
compatibility tables.

## The model boundary carries the named contract

The folio daemon exercises these invariants through a real spine. They also
live here, at the package that owns the bootstrap facts and compatibility
tables, so an ontology-only test selection cannot silently skip the contract.
The projection test compares complete name sets and compatibility rows rather
than sampling known entries:

```rust {#concept-fact-contract-tests}
#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;

    use super::*;

    fn bootstrap_model() -> OntologyModel {
        OntologyModel::new(crate::bootstrap_concept_facts())
    }

    #[test]
    fn concept_facts_projection_parity() {
        let model = bootstrap_model();
        let folded_classes: Vec<_> = model.classes().into_iter()
            .map(|class| (class.uri, class.label)).collect();
        let generated_classes: Vec<_> = crate::ONTOLOGY_CLASSES.iter()
            .map(|class| (class.uri.to_string(), class.label.to_string())).collect();
        assert_eq!(folded_classes, generated_classes, "class counts and names differ");

        let folded_predicates: Vec<_> = model.object_properties().into_iter()
            .map(|property| (property.uri, property.domain, property.range)).collect();
        let generated_predicates: Vec<_> = crate::ONTOLOGY_OBJECT_PROPERTIES.iter()
            .map(|property| (
                property.uri.to_string(),
                property.domain.map(str::to_string),
                property.range.map(str::to_string),
            )).collect();
        assert_eq!(folded_predicates, generated_predicates, "predicate counts, names, or shapes differ");

        let folded_edges: Vec<_> = model.decision_edge_predicates().into_iter()
            .map(|(snake, _)| snake).collect();
        let generated_edges: Vec<_> = crate::KNOWN_EDGE_PREDICATES.iter()
            .map(|predicate| predicate.to_string()).collect();
        assert_eq!(folded_edges, generated_edges, "Decision predicate view differs");
    }

    #[test]
    fn concept_facts_root_closes() {
        let model = bootstrap_model();
        let concepts = model.concept_entities();
        let declared: BTreeSet<_> = model.declared_entities().into_iter().collect();
        assert_eq!(concepts, declared, "every declaration must occupy the one Concept level");

        for concept in &concepts {
            let typings = model.facts().iter().filter(|fact| {
                fact.entity == *concept
                    && fact.predicate == RDF_TYPE
                    && fact.value == OntologyValue::Entity(CONCEPT_URI.to_string())
            }).count();
            assert_eq!(typings, 1, "{concept} must have exactly one Concept typing");
        }

        let root_concept_types: BTreeSet<_> = model.facts().iter().filter_map(|fact| {
            match &fact.value {
                OntologyValue::Entity(target)
                    if fact.entity == CONCEPT_URI
                        && fact.predicate == RDF_TYPE
                        && concepts.contains(target) => Some(target.clone()),
                _ => None,
            }
        }).collect();
        assert_eq!(
            root_concept_types,
            BTreeSet::from([CONCEPT_URI.to_string()]),
            "Concept must close on itself with no concept above it",
        );
    }

    #[test]
    fn concept_facts_view_roundtrip() {
        let model = bootstrap_model();
        let modules = model.modules();
        let names: Vec<_> = modules.iter().map(|module| module.name.as_str()).collect();
        assert_eq!(names, crate::MODULES, "folded module set differs from the compiled one");
        for module in &modules {
            let checked = crate::checked_module_file(&module.name)
                .unwrap_or_else(|| panic!("module {} has no checked file", module.name));
            assert_eq!(
                model.render_module(module),
                checked,
                "checked ontology/modules/{}.ttl is not the exact materialized module view",
                module.name,
            );
            // A shape file exists exactly when the module has shapes, so the
            // `Option`s must agree in both directions: a checked file with no
            // shapes behind it is as much a drift as shapes with no file.
            assert_eq!(
                model.render_shapes(module).as_deref(),
                crate::checked_shape_file(&module.name),
                "checked ontology/shapes/{}.ttl is not the exact materialized shape view",
                module.name,
            );
        }
    }

    #[test]
    fn concept_facts_modules_import_in_order() {
        let model = bootstrap_model();
        let order = model.import_order().expect("module imports form a DAG over the set");
        for (index, module) in order.iter().enumerate() {
            for import in &module.imports {
                let position = order.iter().position(|other| &other.iri == import)
                    .unwrap_or_else(|| panic!("{} imports {import}, which is not in the order", module.name));
                assert!(position < index, "{} is ordered before its import {import}", module.name);
            }
        }
        for concept in model.concept_entities() {
            let defining = model.defining_modules(&concept);
            assert_eq!(defining.len(), 1, "{concept} must be defined by exactly one module: {defining:?}");
        }
    }
}
```
