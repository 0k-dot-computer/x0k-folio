---
x0k:
  format: folio/v1
  id: x0k:implementation/ontology/module-bootstrap
  type: implementation
  status: draft
  summary: The build script that reads the checked vocabulary modules, refuses a set whose imports do not close, and emits the constant tables the crate root re-exports — so nothing at runtime carries a Turtle parser.
  concerns: [ontology, vocabulary, bootstrap, turtle, build-script, codegen]
  tangle:
    crate: x0k-ontology
    root: build.rs
  edges:
    constrained_by:
      - x0k:architecture/state-representation
    cites:
      - x0k:architecture/ontology-modules
      - x0k:implementation/ontology/concept-facts
      - x0k:implementation/ontology/views
---

# The vocabulary, parsed once, at build time

Nearly every crate in the tree spells an ontology predicate at some point,
and none of them should link a Turtle parser to do it. That is the whole reason
this build script exists. The vocabulary lives in the fact plane and is
materialized out to `ontology/modules/*.ttl`
([`concept-facts.md`](concept-facts.md)); consumers want `&[&str]` and a
`match` ([`views.md`](views.md)). This script is the one place the two meet: it
runs once per build, reads the checked module files, and writes
`$OUT_DIR/generated.rs`. `oxttl` is a build dependency and the runtime crate
graph never sees it.

It does one thing more, and it is the more interesting one. A publication ships
a *set* of modules, chosen per build (`x0k:architecture/ontology-modules` §3),
and a set whose `owl:imports` name a module that is not present is not a
vocabulary — it is a vocabulary with a hole in it, which will fail somewhere
later and further from the cause. This script is where the set is first
checked, and it refuses rather than emits.

One example carries the chapter. The `work` module declares itself an
`owl:Ontology`, imports `core`, and defines terms like `x0k:Intent` that say so
with `rdfs:isDefinedBy`. Build a tree holding `work` and `core` and both files
are parsed, unioned, ordered `core` before `work`, and emitted as two `pub mod`s
of tables. Build a tree holding `work` alone and the build stops.

## Contract

Every failure here is a panic. A build script has one channel, and a
half-emitted table is worse than a build that stopped: consumers would compile
against a vocabulary missing terms and only notice at the point of use. So the
script reads the module directory, writes exactly one file, panics on anything
it cannot account for, and touches nothing else — no network, no clock, no state
between runs.

```rust {#module-doc}
//! Bootstrap `ontology/modules/*.ttl` into concept facts, fold the
//! compatibility registry views from those facts, and emit them into
//! `$OUT_DIR/generated.rs`.
//!
//! The checked Turtle is not the authority: the module files are the
//! bootstrap/materialized view used when a profile has no self-typed
//! `x0k:Concept` root. Keeping the parser in the build graph lets existing
//! static-table consumers compile while the entry-spine fold becomes
//! canonical at runtime. The build reads every module file and every shape
//! file (sorted), unions the facts, and refuses a set whose `owl:imports`
//! name a module that is not in it or form a cycle — a publication ships a
//! set, and this is where the set is first checked. Shapes join the union but
//! not the closure check: a shape is applied to a document rather than
//! imported by a term, so it names classes from anywhere
//! (`x0k:architecture/vocabulary-shapes` §4).
```

`src/concept_facts.rs` is pulled in by path rather than by dependency. The
fold this script performs is the same fold the crate performs at runtime, and
compiling one copy of it into the build script is what keeps them from drifting
into two. `dead_code` is allowed because the build script uses a proper subset
of what the runtime module offers.

```rust {#imports}
use std::collections::{BTreeMap, BTreeSet, VecDeque};
use std::env;
use std::path::{Path, PathBuf};

use oxttl::TurtleParser;
```

```rust {#concept-facts-by-path}
#[path = "src/concept_facts.rs"]
#[allow(dead_code)]
mod concept_facts;

use concept_facts::{ModuleRecord, OntologyFact, OntologyModel, OntologyValue};
```

Turtle carries typed literals, and the fact plane at present carries only
text. Rather than lose the distinction silently, the parser below refuses
anything that is not a plain `xsd:string`:

```rust {#xsd-string}
const XSD_STRING: &str = "http://www.w3.org/2001/XMLSchema#string";
```

The parse has its own vocabulary, deliberately separate from
[`OntologyFact`](concept-facts.md). A raw triple still distinguishes an IRI from
a blank node, because blank-node labels are file-scoped and have to be
namespaced before the files are unioned; an `OntologyFact` has already lost that
distinction, and should have.

```rust {#raw-terms}
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
enum RawNode {
    Iri(String),
    Blank(String),
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
enum RawTerm {
    Entity(RawNode),
    Text(String),
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
struct RawTriple {
    subject: RawNode,
    predicate: String,
    object: RawTerm,
}
```

## The run

`main` reads as the outline of the whole script: find the modules, tell
cargo what to watch, fold, check, emit.

```rust {#main}
fn main() {
    <<locate-modules>>

    <<declare-reruns>>

    <<collect-module-paths>>

    <<fold-and-check>>

    <<write-generated>>
}
```

Where the modules are depends on who is building. A packaged crate has to be
self-contained, so the repository projector vendors a copy of the module files
inside the crate ([`region-repo.md`](../tangle/region-repo.md)) and a published
tarball builds from that. The monorepo has no in-crate copy and reads the
canonical directory one level up. The in-crate copy wins when it exists, which
is the only rule that makes both builds work without a feature flag:

```rust {#locate-modules}
let manifest = PathBuf::from(env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR"));
// A packaged crate is self-contained: the repository projector vendors a
// copy of the module files at `<crate>/ontology/modules/`, and a published
// tarball builds from that. The monorepo (and the projected workspace's
// dev build) has no in-crate copy and reads the canonical
// `<repo-root>/ontology/` one level up.
let in_crate = manifest.join("ontology");
let ontology_dir = if in_crate.join("modules").is_dir() {
    in_crate
} else {
    let repo_root = manifest.parent().expect("x0k-ontology has a parent");
    repo_root.join("ontology")
};
let modules_dir = ontology_dir.join("modules");
// Shapes are the second half of the same bootstrap. They are read here and
// nowhere else in the build: a shape constrains a document, so nothing the
// crate compiles depends on one, and the set-closure checks below stay over
// the module files alone (`x0k:architecture/vocabulary-shapes` §4).
let shapes_dir = ontology_dir.join("shapes");
```

The rerun declarations name the directory *and* every file in it. The
directory alone would miss an edit to a file already present; the files alone
would miss a module being added:

```rust {#declare-reruns}
println!("cargo:rerun-if-changed={}", modules_dir.display());
println!("cargo:rerun-if-changed={}", shapes_dir.display());
println!("cargo:rerun-if-changed=build.rs");
println!("cargo:rerun-if-changed=src/concept_facts.rs");
```

```rust {#collect-module-paths}
let module_paths = module_file_paths(&modules_dir);
let shape_paths = shape_file_paths(&shapes_dir);
for path in module_paths.iter().chain(shape_paths.iter()) {
    println!("cargo:rerun-if-changed={}", path.display());
}
```

The four lines that follow are the check this script exists for.
`import_order` refuses a set whose imports name an absent module or form a
cycle, and `check_module_files` refuses a set whose declared modules and whose
files disagree — a file with no module fact, or a module fact with no file, is a
materialization the tree did not receive in full. `check_shape_files` asks the
weaker question a shape file admits: a shape belongs to a module, so its file
must name one of the set, but a module owing no shapes owes no file.

```rust {#fold-and-check}
let model = parse_bootstrap_model(&module_paths, &shape_paths);
let modules = model.import_order().unwrap_or_else(|error| panic!("ontology module set under {}: {error}", modules_dir.display()));
check_module_files(&modules, &module_paths, &modules_dir);
check_shape_files(&modules, &shape_paths, &shapes_dir);
let out = emit_generated(&model, &module_paths, &shape_paths);
```

```rust {#write-generated}
let out_dir = PathBuf::from(env::var("OUT_DIR").expect("OUT_DIR"));
let out_path = out_dir.join("generated.rs");
std::fs::write(&out_path, out)
    .unwrap_or_else(|e| panic!("write {}: {e}", out_path.display()));
```

## Reading the files

The module files are read in sorted order and unioned into one fact set.
Sorting is what makes the emitted file reproducible: `read_dir` order is a
filesystem detail, and a table whose row order follows it would produce a
different `generated.rs` on two machines holding identical trees.

```rust {#module-file-paths}
/// Every `<name>.ttl` under the module directory, sorted by name.
fn module_file_paths(modules_dir: &Path) -> Vec<PathBuf> {
    let entries = std::fs::read_dir(modules_dir)
        .unwrap_or_else(|e| panic!("read module directory {}: {e}", modules_dir.display()));
    let mut paths: Vec<PathBuf> = entries
        .map(|entry| entry.expect("module directory entry").path())
        .filter(|path| path.extension().is_some_and(|extension| extension == "ttl"))
        .collect();
    paths.sort();
    assert!(!paths.is_empty(), "no ontology module files under {}", modules_dir.display());
    paths
}

fn module_file_name(path: &Path) -> String {
    path.file_stem().expect("module file stem").to_string_lossy().into_owned()
}

/// Every `<name>.ttl` under the shape directory, sorted by name. A tree with
/// no shapes at all has no directory, and that is not an error: a shape file
/// exists only for a module that constrains something.
fn shape_file_paths(shapes_dir: &Path) -> Vec<PathBuf> {
    let Ok(entries) = std::fs::read_dir(shapes_dir) else {
        return Vec::new();
    };
    let mut paths: Vec<PathBuf> = entries
        .map(|entry| entry.expect("shape directory entry").path())
        .filter(|path| path.extension().is_some_and(|extension| extension == "ttl"))
        .collect();
    paths.sort();
    paths
}
```

```rust {#check-module-files}
/// The module set the facts declare must be exactly the set of files: a file
/// whose module fact is missing, or a module fact without its file, is a
/// materialization the tree did not receive in full.
fn check_module_files(modules: &[ModuleRecord], module_paths: &[PathBuf], modules_dir: &Path) {
    let declared: BTreeSet<&str> = modules.iter().map(|module| module.name.as_str()).collect();
    let files: BTreeSet<String> = module_paths.iter().map(|path| module_file_name(path)).collect();
    let files: BTreeSet<&str> = files.iter().map(String::as_str).collect();
    assert!(
        declared == files,
        "ontology module facts {declared:?} do not match the module files {files:?} under {}",
        modules_dir.display()
    );
}

/// A shape file is named for the module whose shapes it holds, so every file
/// must name a module of the set. The converse does not hold: a module that
/// constrains nothing has no file, which is why this is a subset test where
/// `check_module_files` is an equality.
fn check_shape_files(modules: &[ModuleRecord], shape_paths: &[PathBuf], shapes_dir: &Path) {
    let declared: BTreeSet<&str> = modules.iter().map(|module| module.name.as_str()).collect();
    for path in shape_paths {
        let name = module_file_name(path);
        assert!(
            declared.contains(name.as_str()),
            "shape file {}/{name}.ttl names no module of the set {declared:?}",
            shapes_dir.display()
        );
    }
}
```

Blank-node labels are scoped to the file that contains them, so two module
files may each hand out `_:b0000n0000`. Namespacing every label by its module
before the union keeps them apart even when a materializer bug hands out the
same label twice:

```rust {#parse-bootstrap-model}
fn parse_bootstrap_model(module_paths: &[PathBuf], shape_paths: &[PathBuf]) -> OntologyModel {
    let mut triples = Vec::new();
    for path in module_paths.iter().chain(shape_paths.iter()) {
        let bytes = std::fs::read(path).unwrap_or_else(|e| panic!("read {}: {e}", path.display()));
        // Blank-node labels are file-scoped in Turtle; namespacing them by
        // file keeps two files' `_:b0000n0000` apart even when a
        // materializer bug hands out the same label twice. The scope carries
        // the directory as well as the stem, because `modules/document.ttl`
        // and `shapes/document.ttl` share a stem.
        let scope = file_scope(path);
        parse_module_file(&bytes, path, &scope, &mut triples);
    }

    let blank_uris = assign_structural_uris(&triples);
    let facts = triples.into_iter().map(|triple| {
        let entity = node_entity(&triple.subject, &blank_uris);
        match triple.object {
            RawTerm::Entity(node) => {
                OntologyFact::entity(entity, triple.predicate, node_entity(&node, &blank_uris))
            }
            RawTerm::Text(value) => OntologyFact::text(entity, triple.predicate, value),
        }
    });
    OntologyModel::new(facts).with_fact_plane_root()
}
```

```rust {#parse-module-file}
fn parse_module_file(bytes: &[u8], path: &Path, scope: &str, triples: &mut Vec<RawTriple>) {
    for triple in TurtleParser::new().for_slice(bytes) {
        let triple = triple.unwrap_or_else(|e| panic!("parse {}: {e}", path.display()));
        let subject = named_or_blank(triple.subject, scope);
        let predicate = triple.predicate.into_string();
        let object = match triple.object {
            oxrdf::Term::NamedNode(node) => RawTerm::Entity(RawNode::Iri(node.into_string())),
            oxrdf::Term::BlankNode(node) => RawTerm::Entity(RawNode::Blank(format!("{scope}:{}", node.into_string()))),
            oxrdf::Term::Literal(literal) => {
                assert!(
                    literal.language().is_none() && literal.datatype().as_str() == XSD_STRING,
                    "{} contains a non-string literal at predicate {predicate}; the fact plane needs a typed-value decision before this can be projected",
                    path.display()
                );
                RawTerm::Text(literal.value().to_string())
            }
        };
        triples.push(RawTriple {
            subject,
            predicate,
            object,
        });
    }
}

fn named_or_blank(node: oxrdf::NamedOrBlankNode, scope: &str) -> RawNode {
    match node {
        oxrdf::NamedOrBlankNode::NamedNode(node) => RawNode::Iri(node.into_string()),
        oxrdf::NamedOrBlankNode::BlankNode(node) => RawNode::Blank(format!("{scope}:{}", node.into_string())),
    }
}
```

## Blank nodes get durable names

A blank node is Turtle syntax, not an identity. It cannot go into the fact
plane as a label, because the label means nothing outside the file it came from
— and the fact plane has no files. The answer is a skolem URI per connected
blank-node component, assigned deterministically so the same tree always yields
the same URIs, and the renderer turns the prefix back into blank labels when it
materializes the file again.

The determinism is bought by ordering: the roots are the IRI-subject triples
that point at a blank node, sorted and deduped, so component numbering follows
the vocabulary rather than the parse. Any blank node the roots did not reach —
one reachable only from another blank node, which a well-formed file should not
produce — is numbered after them rather than left unassigned.

```rust {#assign-structural-uris}
/// Blank nodes are Turtle syntax, not durable identities. Give every
/// connected blank-node component a deterministic skolem URI while it is in
/// the fact plane; the renderer turns this prefix back into blank labels.
fn assign_structural_uris(triples: &[RawTriple]) -> BTreeMap<String, String> {
    let mut roots: Vec<(String, String, String)> = triples
        .iter()
        .filter_map(|triple| match (&triple.subject, &triple.object) {
            (RawNode::Iri(subject), RawTerm::Entity(RawNode::Blank(blank))) => {
                Some((subject.clone(), triple.predicate.clone(), blank.clone()))
            }
            _ => None,
        })
        .collect();
    roots.sort();
    roots.dedup();

    let mut assigned = BTreeMap::new();
    for (root_index, (_, _, blank)) in roots.iter().enumerate() {
        assign_blank_component(triples, blank, root_index, &mut assigned);
    }

    let mut all_blanks = BTreeSet::new();
    for triple in triples {
        if let RawNode::Blank(blank) = &triple.subject {
            all_blanks.insert(blank.clone());
        }
        if let RawTerm::Entity(RawNode::Blank(blank)) = &triple.object {
            all_blanks.insert(blank.clone());
        }
    }
    let mut next_root = roots.len();
    for blank in all_blanks {
        if !assigned.contains_key(&blank) {
            assign_blank_component(triples, &blank, next_root, &mut assigned);
            next_root += 1;
        }
    }
    assigned
}
```

Within a component the walk is breadth-first with sorted children, for the
same reason: node numbering must not depend on triple order in the file.

```rust {#assign-blank-component}
fn assign_blank_component(
    triples: &[RawTriple],
    root: &str,
    root_index: usize,
    assigned: &mut BTreeMap<String, String>,
) {
    let mut queue = VecDeque::from([root.to_string()]);
    let mut node_index = 0usize;
    while let Some(blank) = queue.pop_front() {
        if assigned.contains_key(&blank) {
            continue;
        }
        assigned.insert(
            blank.clone(),
            format!(
                "{}b{root_index:04}n{node_index:04}",
                concept_facts::STRUCTURAL_NODE_PREFIX
            ),
        );
        node_index += 1;

        let mut children: Vec<String> = triples
            .iter()
            .filter_map(|triple| match (&triple.subject, &triple.object) {
                (RawNode::Blank(subject), RawTerm::Entity(RawNode::Blank(child)))
                    if subject == &blank =>
                {
                    Some(child.clone())
                }
                _ => None,
            })
            .collect();
        children.sort();
        children.dedup();
        queue.extend(children);
    }
}
```

```rust {#node-entity}
/// A blank-node namespace unique per file: `<parent>/<stem>`.
fn file_scope(path: &Path) -> String {
    let parent = path
        .parent()
        .and_then(|parent| parent.file_name())
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_default();
    format!("{parent}/{}", module_file_name(path))
}

fn node_entity(node: &RawNode, blank_uris: &BTreeMap<String, String>) -> String {
    match node {
        RawNode::Iri(iri) => iri.clone(),
        RawNode::Blank(blank) => blank_uris
            .get(blank)
            .unwrap_or_else(|| panic!("unassigned ontology blank node {blank}"))
            .clone(),
    }
}
```

## Emitting the tables

What comes out is one Rust file, built by string concatenation. This is
plumbing, and the chapter says so rather than deriving it: `emit_generated`
names the sections in order, and each `emit_*` below pushes one of them.

```rust {#emit-generated}
fn emit_generated(model: &OntologyModel, module_paths: &[PathBuf], shape_paths: &[PathBuf]) -> String {
    let classes = model.classes();
    let properties = model.object_properties();
    let decision_predicates = model.decision_edge_predicates();
    let modules = model.modules();

    let mut out = String::from(
        "// @generated by x0k-ontology/build.rs from the folded bootstrap concept facts\n\
         // Do not edit by hand. The concept region is authoritative; ontology/modules/*.ttl\n\
         // are its checked bootstrap/materialized views, one per vocabulary module.\n\n",
    );
    emit_module_set(&mut out, &modules, module_paths, shape_paths);
    emit_bootstrap_facts(&mut out, model.facts());
    emit_decision_predicates(&mut out, &decision_predicates);
    emit_classes(&mut out, &classes);
    emit_properties(&mut out, &properties);
    for module in &modules {
        emit_module(&mut out, model, module);
    }
    out
}
```

Two of these carry a decision worth naming. `MODULE_FILES` — and `SHAPE_FILES`
beside it, on the same terms — embeds each file's text by `include_str!` at its
canonicalized absolute path, so the compiled crate carries the exact bytes it
was built from and a consumer can compare them against the tree. And `MODULE_TABLES` exists so that code can walk
the shipped set without naming a member: which modules a build ships is a
per-publication choice, and nothing compiled in may assume a particular one
beyond `core`.

```rust {#emit-module-set}
fn emit_module_set(out: &mut String, modules: &[ModuleRecord], module_paths: &[PathBuf], shape_paths: &[PathBuf]) {
    out.push_str(
        "/// Every vocabulary module of the bootstrap set, sorted by name. Each has a\n\
         /// `pub mod` of the same name (`-` spelled `_`) carrying its slice of the tables.\n\
         pub const MODULES: &[&str] = &[\n",
    );
    for module in modules {
        out.push_str(&format!("    {:?},\n", module.name));
    }
    out.push_str("];\n\n");

    out.push_str(
        "/// The checked module files this crate was built from, by module name.\n\
         pub const MODULE_FILES: &[(&str, &str)] = &[\n",
    );
    for path in module_paths {
        let absolute = path.canonicalize().unwrap_or_else(|e| panic!("canonicalize {}: {e}", path.display()));
        out.push_str(&format!(
            "    ({:?}, include_str!({:?})),\n",
            module_file_name(path),
            absolute.to_string_lossy()
        ));
    }
    out.push_str("];\n\n");

    out.push_str(
        "/// The checked shape files this crate was built from, by module name. A\n\
         /// module that constrains nothing has no entry.\n\
         pub const SHAPE_FILES: &[(&str, &str)] = &[\n",
    );
    for path in shape_paths {
        let absolute = path.canonicalize().unwrap_or_else(|e| panic!("canonicalize {}: {e}", path.display()));
        out.push_str(&format!(
            "    ({:?}, include_str!({:?})),\n",
            module_file_name(path),
            absolute.to_string_lossy()
        ));
    }
    out.push_str("];\n\n");

    out.push_str(
        "/// One shipped module's tables by reference, so a consumer can walk the\n\
         /// set without naming any module: the set is chosen per build (a\n\
         /// publication ships a subset), so nothing compiled in may assume a\n\
         /// particular member beyond `core`.\n\
         pub struct ModuleTables {\n\
             pub name: &'static str,\n\
             pub iri: &'static str,\n\
             pub imports: &'static [&'static str],\n\
             pub classes: &'static [OntologyClass],\n\
             pub object_properties: &'static [OntologyObjectProperty],\n\
             pub edge_predicates: &'static [&'static str],\n\
             pub snake_to_camel: fn(&str) -> Option<&'static str>,\n\
         }\n\n\
         /// The shipped modules' tables, in `MODULES` order.\n\
         pub const MODULE_TABLES: &[ModuleTables] = &[\n",
    );
    for module in modules {
        let rust_name = module.name.replace('-', "_");
        out.push_str(&format!(
            "    ModuleTables {{\n        name: {:?},\n        iri: {rust_name}::IRI,\n        imports: {rust_name}::IMPORTS,\n        classes: {rust_name}::CLASSES,\n        object_properties: {rust_name}::OBJECT_PROPERTIES,\n        edge_predicates: {rust_name}::EDGE_PREDICATES,\n        snake_to_camel: {rust_name}::snake_to_camel,\n    }},\n",
            module.name
        ));
    }
    out.push_str("];\n\n");
}
```

Each module's own tables are emitted whether or not they are empty, so
`MODULE_TABLES` can name every field of every member uniformly:

```rust {#emit-module}
fn emit_module(out: &mut String, model: &OntologyModel, module: &ModuleRecord) {
    let rust_name = module.name.replace('-', "_");
    out.push_str(&format!(
        "/// The `{}` vocabulary module: `{}`.\n\
         pub mod {rust_name} {{\n\
             pub const IRI: &str = {:?};\n\
             pub const IMPORTS: &[&str] = &[\n",
        module.name, module.iri, module.iri
    ));
    for import in &module.imports {
        out.push_str(&format!("        {:?},\n", import));
    }
    out.push_str("    ];\n\n");

    out.push_str("    pub const CLASSES: &[crate::OntologyClass] = &[\n");
    for class in model.classes_in(&module.iri) {
        out.push_str(&format!(
            "        crate::OntologyClass {{ uri: {:?}, label: {:?} }},\n",
            class.uri, class.label
        ));
    }
    out.push_str("    ];\n\n");

    out.push_str("    pub const OBJECT_PROPERTIES: &[crate::OntologyObjectProperty] = &[\n");
    for property in model.object_properties_in(&module.iri) {
        let domain = option_literal(property.domain.as_deref());
        let range = option_literal(property.range.as_deref());
        out.push_str(&format!(
            "        crate::OntologyObjectProperty {{ uri: {:?}, domain: {domain}, range: {range} }},\n",
            property.uri
        ));
    }
    out.push_str("    ];\n");

    // Emitted for every module, empty or not, so `MODULE_TABLES` can name
    // them uniformly.
    let predicates = model.decision_edge_predicates_in(&module.iri);
    out.push_str("\n    /// This module's Decision-domain predicates (snake_case wire form).\n    pub const EDGE_PREDICATES: &[&str] = &[\n");
    for (snake, _) in &predicates {
        out.push_str(&format!("        {:?},\n", snake));
    }
    out.push_str("    ];\n\n");
    out.push_str(
        "    /// snake_case to camelCase for this module's Decision-domain predicates.\n\
         pub fn snake_to_camel(snake: &str) -> Option<&'static str> {\n",
    );
    if predicates.is_empty() {
        out.push_str("        let _ = snake;\n        None\n    }\n");
    } else {
        out.push_str("        Some(match snake {\n");
        for (snake, camel) in &predicates {
            out.push_str(&format!("            {:?} => {:?},\n", snake, camel));
        }
        out.push_str("            _ => return None,\n        })\n    }\n");
    }
    out.push_str("}\n\n");
}
```

The rest is the same shape, once per table — the bootstrap facts that seed
an empty concept region, the Decision-domain predicates in both spellings, and
the class and property records:

```rust {#emit-bootstrap-facts}
fn emit_bootstrap_facts(out: &mut String, facts: &[OntologyFact]) {
    out.push_str(
        "/// Bootstrap projection for an empty concept region. Once the self-typed\n\
         /// `x0k:Concept` root folds from the log, callers must use that fold instead.\n\
         pub fn bootstrap_concept_facts() -> Vec<crate::concept_facts::OntologyFact> {\n\
             use crate::concept_facts::OntologyFact;\n\
             vec![\n",
    );
    for fact in facts {
        let entity = format!("{:?}", fact.entity);
        let predicate = format!("{:?}", fact.predicate);
        match &fact.value {
            OntologyValue::Text(value) => out.push_str(&format!(
                "        OntologyFact::text({entity}, {predicate}, {:?}),\n",
                value
            )),
            OntologyValue::Entity(value) => out.push_str(&format!(
                "        OntologyFact::entity({entity}, {predicate}, {:?}),\n",
                value
            )),
        }
    }
    out.push_str("    ]\n}\n\n");
}

fn emit_decision_predicates(out: &mut String, predicates: &[(String, String)]) {
    out.push_str(
        "/// Decision-domain predicates (snake_case wire form), materialized from\n\
         /// the folded concept facts.\n\
         pub const KNOWN_EDGE_PREDICATES: &[&str] = &[\n",
    );
    for (snake, _) in predicates {
        out.push_str(&format!("    {:?},\n", snake));
    }
    out.push_str("];\n\n");

    out.push_str(
        "/// snake_case to camelCase ontology predicate name.\n\
         pub fn snake_to_camel(snake: &str) -> Option<&'static str> {\n\
             Some(match snake {\n",
    );
    for (snake, camel) in predicates {
        out.push_str(&format!("        {:?} => {:?},\n", snake, camel));
    }
    out.push_str("        _ => return None,\n    })\n}\n\n");
}
```

```rust {#emit-classes}
fn emit_classes(out: &mut String, classes: &[concept_facts::ClassRecord]) {
    out.push_str(
        "/// One ontology class in the bare `x0k:LocalName` form consumers use.\n\
         #[derive(Clone, Copy, Debug)]\n\
         pub struct OntologyClass { pub uri: &'static str, pub label: &'static str }\n\n\
         /// Every class folded from the concept region, sorted by URI.\n\
         pub const ONTOLOGY_CLASSES: &[OntologyClass] = &[\n",
    );
    for class in classes {
        out.push_str(&format!(
            "    OntologyClass {{ uri: {:?}, label: {:?} }},\n",
            class.uri, class.label
        ));
    }
    out.push_str("];\n\n");
}

fn emit_properties(out: &mut String, properties: &[concept_facts::PropertyRecord]) {
    out.push_str(
        "/// One object property's compatibility-table domain and range.\n\
         #[derive(Clone, Copy, Debug)]\n\
         pub struct OntologyObjectProperty {\n\
             pub uri: &'static str,\n\
             pub domain: Option<&'static str>,\n\
             pub range: Option<&'static str>,\n\
         }\n\n\
         /// Every object property folded from the concept region, sorted by URI.\n\
         pub const ONTOLOGY_OBJECT_PROPERTIES: &[OntologyObjectProperty] = &[\n",
    );
    for property in properties {
        let domain = option_literal(property.domain.as_deref());
        let range = option_literal(property.range.as_deref());
        out.push_str(&format!(
            "    OntologyObjectProperty {{ uri: {:?}, domain: {domain}, range: {range} }},\n",
            property.uri
        ));
    }
    out.push_str("];\n");
}

fn option_literal(value: Option<&str>) -> String {
    value
        .map(|value| format!("Some({value:?})"))
        .unwrap_or_else(|| "None".to_string())
}
```

## Composing the file

```rust {#root}
<<module-doc>>

<<imports>>

<<concept-facts-by-path>>

<<xsd-string>>

<<raw-terms>>

<<main>>

<<module-file-paths>>

<<check-module-files>>

<<parse-bootstrap-model>>

<<parse-module-file>>

<<assign-structural-uris>>

<<assign-blank-component>>

<<node-entity>>

<<emit-generated>>

<<emit-module-set>>

<<emit-module>>

<<emit-bootstrap-facts>>

<<emit-classes>>
```

The file this script writes is a *view*, and saying so is not a hedge. The
authority is the self-typed `x0k:Concept` root in the fact plane; the module
files are the checked bootstrap that seeds an empty region; these tables are
what a linker can hold. Keeping the parser in the build graph is what lets the
static-table consumers compile unchanged while the runtime fold becomes
canonical underneath them — and it is why the closure check lives here rather
than at the publication boundary. A set that does not close cannot be projected
into a repository someone else builds, and this is the earliest place anyone
can be told.
