---
x0k:
  format: folio/v1
  id: x0k:implementation/ontology/load
  type: implementation
  status: draft
  summary: Reading a set of vocabulary module files into an OntologyModel at run time — the same parse, the same fold, and the same refusals the build script applies, returned as a Result instead of a panic.
  concerns:
  - ontology
  - vocabulary
  - turtle
  - loading
  - publishing
  tangle:
    crate: crates/x0k-ontology
    root: src/load.rs
  edges:
    implements:
    - x0k:design/publish-a-region-as-a-repository
    constrained_by:
    - x0k:architecture/state-representation
    cites:
    - x0k:architecture/ontology-modules
    - x0k:implementation/ontology/concept-facts
    - x0k:implementation/ontology/module-bootstrap
    - x0k:implementation/ontology/views
---

# Reading a vocabulary the build did not compile

A publication ships a *set* of vocabulary modules, chosen per build
(`x0k:architecture/ontology-modules` §3), and until this module the only way
to read one was to compile it. [`module-bootstrap.md`](module-bootstrap.md)
parsed `ontology/modules/*.ttl` in a build script and froze the answer into
static tables; every consumer downstream asked those tables and could not ask
anything else. So a reader holding a *different* module set — their own
extension module, or the modules a bundle they received actually shipped —
had a vocabulary the tools could see as files and not as vocabulary.

This chapter is the same parse and the same fold, in the library, where a
caller can point it at a directory. `OntologyModel::load` reads a module
directory and returns the model; `load_files` takes the file lists directly.
The build script keeps its job — emitting the tables a linker can hold — but
it no longer owns the parser: it calls the two functions below, which is what
keeps the compiled tables and a loaded model from being two different
readings of the same bytes.

One example carries the chapter, and it is
[`module-bootstrap.md`](module-bootstrap.md)'s. A directory holding `work`
and `core` loads: both files are parsed, unioned, and folded. A directory
holding `work` alone does not, because `work` imports `core` and a set whose
imports do not close is a vocabulary with a hole in it. The build script
panics on that; this function returns `Err`, and that difference is the only
one between them.

## Contract

Everything here reads files and nothing else — no network, no clock, no
state between calls, no writes. A directory that cannot be read, a file that
is not Turtle, a literal that is not a plain string, a set whose imports do
not close, a module fact without its file: each is a [`LoadError`] naming
what it saw. The caller decides whether that is fatal; the build script
decides that it is.

Two things the loader does *not* decide. It does not choose the directory —
[`shipped_modules_dir`](#the-directory-a-build-reads) answers that for the
one caller that has no choice, and everyone else passes a path. And it does
not rank a loaded model against the compiled one: `OntologyModel::shipped()`
([`views.md`](views.md)) is a model like any other, and the tests below pin
that the two agree over the same files rather than privileging either.

<a name="chunk-module-doc"></a><sub>[`src/load.rs`](../../crates/x0k-ontology/src/load.rs) · `#module-doc`</sub>

```rust {#module-doc}
//! Reading vocabulary module files into an [`OntologyModel`] at run time.
//!
//! The same Turtle parse and the same fold the build script emits its
//! tables from ([`module-bootstrap.md`]), returned as a `Result`. A
//! publication ships a set of modules chosen per build
//! (`x0k:architecture/ontology-modules` §3); this is how a caller reads a
//! set it was handed rather than the one it was compiled against.
//!
//! `build.rs` pulls this module in by path and calls the same two
//! functions a consumer calls, so the compiled tables and a loaded model
//! are one reading of the bytes rather than two.
//!
//! [`module-bootstrap.md`]: https://0k.computer/x0k:implementation/ontology/module-bootstrap
```

<a name="chunk-imports"></a><sub>[`src/load.rs`](../../crates/x0k-ontology/src/load.rs) · `#imports`</sub>

```rust {#imports}
use std::collections::{BTreeMap, BTreeSet, VecDeque};
use std::fmt;
use std::path::{Path, PathBuf};

use oxttl::TurtleParser;

use crate::concept_facts::{ModuleRecord, OntologyFact, OntologyModel, STRUCTURAL_NODE_PREFIX};
```

Turtle carries typed literals, and the fact plane at present carries only
text. Rather than lose the distinction silently, the parser refuses anything
that is not a plain `xsd:string`:

<a name="chunk-xsd-string"></a><sub>[`src/load.rs`](../../crates/x0k-ontology/src/load.rs) · `#xsd-string`</sub>

```rust {#xsd-string}
const XSD_STRING: &str = "http://www.w3.org/2001/XMLSchema#string";
```

## What can go wrong, named

Each variant carries the file or the module it is about, because these are
read by whoever selected the set — a publication's maker, or a reader whose
own module directory does not close — and a refusal without its subject is
unactionable. `Imports` and `ModuleFiles` are the two the build script exists
to catch, and they stay word-for-word what it used to panic with.

<a name="chunk-load-error"></a><sub>[`src/load.rs`](../../crates/x0k-ontology/src/load.rs) · `#load-error`</sub>

```rust {#load-error}
/// Why a set of vocabulary module files is not a vocabulary.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LoadError {
    /// The module directory could not be read.
    ReadDir { path: PathBuf, reason: String },
    /// A module or shape file could not be read.
    ReadFile { path: PathBuf, reason: String },
    /// A file is not well-formed Turtle.
    Parse { path: PathBuf, reason: String },
    /// A literal that is not a plain `xsd:string`. The fact plane carries
    /// text and entity references; a typed value needs a fact-plane
    /// decision before this view can project it.
    TypedLiteral { path: PathBuf, predicate: String },
    /// The directory holds no `.ttl` files at all.
    EmptySet { path: PathBuf },
    /// An `owl:imports` naming a module the set does not hold, or a cycle.
    Imports(String),
    /// The declared module set and the files disagree — a file with no
    /// module fact, or a module fact with no file.
    ModuleFiles {
        declared: Vec<String>,
        files: Vec<String>,
    },
    /// A shape file named for no module of the set. A shape belongs to a
    /// module, so its file must name one; a module owing no shapes owes no
    /// file, which is why the converse is not an error.
    ShapeFile { name: String },
    /// A blank node reached by no root. Structural, and a bug here rather
    /// than in the files.
    UnassignedBlankNode(String),
}

impl fmt::Display for LoadError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ReadDir { path, reason } => {
                write!(f, "read module directory {}: {reason}", path.display())
            }
            Self::ReadFile { path, reason } => write!(f, "read {}: {reason}", path.display()),
            Self::Parse { path, reason } => write!(f, "parse {}: {reason}", path.display()),
            Self::TypedLiteral { path, predicate } => write!(
                f,
                "{} contains a non-string literal at predicate {predicate}; the fact plane needs \
                 a typed-value decision before this can be projected",
                path.display()
            ),
            Self::EmptySet { path } => {
                write!(f, "no ontology module files under {}", path.display())
            }
            Self::Imports(reason) => write!(f, "ontology module set: {reason}"),
            Self::ModuleFiles { declared, files } => write!(
                f,
                "ontology module facts {declared:?} do not match the module files {files:?}"
            ),
            Self::ShapeFile { name } => {
                write!(f, "shape file {name}.ttl names no module of the set")
            }
            Self::UnassignedBlankNode(blank) => {
                write!(f, "unassigned ontology blank node {blank}")
            }
        }
    }
}

impl std::error::Error for LoadError {}
```

## Loading a directory

`load` is the whole face: hand it the directory holding the module files and
get the folded model or the reason it is not one. Shapes are the second half
of the same vocabulary and live in a sibling directory — the layout the
corpus has, and the layout a projection records in `PROVENANCE.json`'s
`modules_dir` ([`region-repo.md`](../tangle/region-repo.md)) — so the caller
names the half the record names and the loader finds the other.

<a name="chunk-load"></a><sub>[`src/load.rs`](../../crates/x0k-ontology/src/load.rs) · `#load`</sub>

```rust {#load}
impl OntologyModel {
    /// Load the vocabulary modules under `modules_dir`, with the shapes
    /// from its sibling `shapes/` directory when that exists.
    ///
    /// `modules_dir` is the directory a projection records as
    /// `modules_dir` in `PROVENANCE.json`, so a reader can point this at
    /// what a bundle actually shipped.
    pub fn load(modules_dir: &Path) -> Result<Self, LoadError> {
        let module_paths = module_file_paths(modules_dir)?;
        let shape_paths = shape_file_paths(&shapes_dir_for(modules_dir));
        Self::load_files(&module_paths, &shape_paths)
    }

    /// Load an explicit set of module and shape files. The three refusals
    /// are here rather than in [`load`](Self::load) because they are about
    /// the *set*, and a caller that assembled its own file list is making
    /// the same claim about it that a directory does.
    pub fn load_files(
        module_paths: &[PathBuf],
        shape_paths: &[PathBuf],
    ) -> Result<Self, LoadError> {
        let model = parse_model(module_paths, shape_paths)?;
        let modules = model.import_order().map_err(LoadError::Imports)?;
        check_module_files(&modules, module_paths)?;
        check_shape_files(&modules, shape_paths)?;
        Ok(model)
    }
}
```

### The directory a build reads

One caller has no directory to be handed: the build script, which runs
wherever the crate was unpacked. A packaged crate has to be self-contained,
so the repository projector vendors a copy of the module files inside the
crate ([`region-repo.md`](../tangle/region-repo.md)) and a published tarball
builds from that; the monorepo has no in-crate copy and reads the canonical
directory from the repository. The in-crate copy wins when it exists, which
is the only rule that makes both builds work without a feature flag.

**The repository's copy is found by SEARCHING upward, not by counting.**
This used to read `manifest_dir.parent()` and call the result the repository
root, which is true only while the crate is a direct child of it. The
monorepo relayout moved the crate to `substrate/crates/production/`, and the
build script then went looking for `substrate/crates/production/ontology/
modules` and panicked in a build script — a failure that surfaces as every
`cargo metadata` and every gate going red at once, three layers from the
line that caused it (2026-09-06, stage 4).

So it walks ancestors and takes the first that actually holds the modules,
which is depth-independent, and it knows both names the directory has:
`ontology/` while the corpus stage has not yet run, and
`corpora/x0k/ontology/` after it. Two candidates rather than one is the
honest encoding of a tree that is mid-move — and when the stage lands, the
first candidate simply stops matching anywhere. Nothing needs to be edited
again.

This lives here rather than in the build script so that a test can ask the
same question the build asked, and get the same answer in both trees:

<a name="chunk-shipped-modules-dir"></a><sub>[`src/load.rs`](../../crates/x0k-ontology/src/load.rs) · `#shipped-modules-dir`</sub>

```rust {#shipped-modules-dir}
/// The module directory a build of this crate reads, given its manifest
/// directory: the in-crate copy a packaged crate carries
/// (`<crate>/ontology/modules/`), else the repository's, found by walking
/// up from the crate rather than assuming how deep in the tree it sits.
pub fn shipped_modules_dir(manifest_dir: &Path) -> PathBuf {
    let in_crate = manifest_dir.join("ontology");
    if in_crate.join("modules").is_dir() {
        return in_crate.join("modules");
    }
    // `ontology/` is where the canonical modules live until the corpus
    // stage of the monorepo relayout moves them under `corpora/x0k/`;
    // both names are tried so a tree mid-relayout builds either way.
    const ROOTS: [&str; 2] = ["ontology", "corpora/x0k/ontology"];
    for ancestor in manifest_dir.ancestors().skip(1) {
        for root in ROOTS {
            let candidate = ancestor.join(root).join("modules");
            if candidate.is_dir() {
                return candidate;
            }
        }
    }
    // Nothing found: name the path the caller's error will be about, which
    // is the one the pre-relayout layout would have used.
    manifest_dir
        .parent()
        .unwrap_or(manifest_dir)
        .join("ontology")
        .join("modules")
}

/// The shape directory beside a module directory. Shapes are read from
/// here and nowhere else: a shape constrains a document, so nothing the
/// crate compiles depends on one (`x0k:architecture/vocabulary-shapes` §4).
pub fn shapes_dir_for(modules_dir: &Path) -> PathBuf {
    match modules_dir.parent() {
        Some(parent) => parent.join("shapes"),
        None => PathBuf::from("shapes"),
    }
}
```

## Reading the files

The module files are read in sorted order and unioned into one fact set.
Sorting is what makes a loaded model reproducible: `read_dir` order is a
filesystem detail, and a model whose row order followed it would fold to a
different table on two machines holding identical trees.

<a name="chunk-module-file-paths"></a><sub>[`src/load.rs`](../../crates/x0k-ontology/src/load.rs) · `#module-file-paths`</sub>

```rust {#module-file-paths}
/// Every `<name>.ttl` under the module directory, sorted by name.
pub fn module_file_paths(modules_dir: &Path) -> Result<Vec<PathBuf>, LoadError> {
    let entries = std::fs::read_dir(modules_dir).map_err(|e| LoadError::ReadDir {
        path: modules_dir.to_path_buf(),
        reason: e.to_string(),
    })?;
    let mut paths: Vec<PathBuf> = entries
        .filter_map(|entry| entry.ok().map(|entry| entry.path()))
        .filter(|path| path.extension().is_some_and(|extension| extension == "ttl"))
        .collect();
    paths.sort();
    if paths.is_empty() {
        return Err(LoadError::EmptySet {
            path: modules_dir.to_path_buf(),
        });
    }
    Ok(paths)
}

/// The module name a file carries: its stem.
pub fn module_file_name(path: &Path) -> String {
    path.file_stem().unwrap_or_default().to_string_lossy().into_owned()
}

/// Every `<name>.ttl` under the shape directory, sorted by name. A tree
/// with no shapes at all has no directory, and that is not an error: a
/// shape file exists only for a module that constrains something.
pub fn shape_file_paths(shapes_dir: &Path) -> Vec<PathBuf> {
    let Ok(entries) = std::fs::read_dir(shapes_dir) else {
        return Vec::new();
    };
    let mut paths: Vec<PathBuf> = entries
        .filter_map(|entry| entry.ok().map(|entry| entry.path()))
        .filter(|path| path.extension().is_some_and(|extension| extension == "ttl"))
        .collect();
    paths.sort();
    paths
}
```

The module set the facts declare must be exactly the set of files: a file
whose module fact is missing, or a module fact without its file, is a
materialization the tree did not receive in full. A shape file admits the
weaker question — a shape belongs to a module, so its file must name one of
the set, but a module owing no shapes owes no file, which is why one is an
equality and the other a subset test.

<a name="chunk-check-files"></a><sub>[`src/load.rs`](../../crates/x0k-ontology/src/load.rs) · `#check-files`</sub>

```rust {#check-files}
fn check_module_files(modules: &[ModuleRecord], module_paths: &[PathBuf]) -> Result<(), LoadError> {
    let declared: BTreeSet<String> = modules.iter().map(|module| module.name.clone()).collect();
    let files: BTreeSet<String> = module_paths.iter().map(|path| module_file_name(path)).collect();
    if declared == files {
        return Ok(());
    }
    Err(LoadError::ModuleFiles {
        declared: declared.into_iter().collect(),
        files: files.into_iter().collect(),
    })
}

fn check_shape_files(modules: &[ModuleRecord], shape_paths: &[PathBuf]) -> Result<(), LoadError> {
    let declared: BTreeSet<&str> = modules.iter().map(|module| module.name.as_str()).collect();
    for path in shape_paths {
        let name = module_file_name(path);
        if !declared.contains(name.as_str()) {
            return Err(LoadError::ShapeFile { name });
        }
    }
    Ok(())
}
```

## The parse has its own vocabulary

A raw triple still distinguishes an IRI from a blank node, because blank-node
labels are file-scoped and have to be namespaced before the files are
unioned. An [`OntologyFact`](concept-facts.md) has already lost that
distinction, and should have.

<a name="chunk-raw-terms"></a><sub>[`src/load.rs`](../../crates/x0k-ontology/src/load.rs) · `#raw-terms`</sub>

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

Blank-node labels are scoped to the file that contains them, so two module
files may each hand out `_:b0000n0000`. Namespacing every label by its file
before the union keeps them apart even when a materializer bug hands out the
same label twice:

<a name="chunk-parse-model"></a><sub>[`src/load.rs`](../../crates/x0k-ontology/src/load.rs) · `#parse-model`</sub>

```rust {#parse-model}
/// Parse every file and fold the union, closing the meta-level as the
/// bootstrap projection does.
fn parse_model(
    module_paths: &[PathBuf],
    shape_paths: &[PathBuf],
) -> Result<OntologyModel, LoadError> {
    let mut triples = Vec::new();
    for path in module_paths.iter().chain(shape_paths.iter()) {
        let bytes = std::fs::read(path).map_err(|e| LoadError::ReadFile {
            path: path.clone(),
            reason: e.to_string(),
        })?;
        // The scope carries the directory as well as the stem, because
        // `modules/document.ttl` and `shapes/document.ttl` share a stem.
        let scope = file_scope(path);
        parse_module_file(&bytes, path, &scope, &mut triples)?;
    }

    fold_triples(triples).map(OntologyModel::with_fact_plane_root)
}

fn fold_triples(triples: Vec<RawTriple>) -> Result<OntologyModel, LoadError> {
    let blank_uris = assign_structural_uris(&triples);
    let mut facts = Vec::with_capacity(triples.len());
    for triple in triples {
        let entity = node_entity(&triple.subject, &blank_uris)?;
        facts.push(match triple.object {
            RawTerm::Entity(node) => {
                OntologyFact::entity(entity, triple.predicate, node_entity(&node, &blank_uris)?)
            }
            RawTerm::Text(value) => OntologyFact::text(entity, triple.predicate, value),
        });
    }
    Ok(OntologyModel::new(facts))
}
```

<a name="chunk-parse-module-file"></a><sub>[`src/load.rs`](../../crates/x0k-ontology/src/load.rs) · `#parse-module-file`</sub>

```rust {#parse-module-file}
fn parse_module_file(
    bytes: &[u8],
    path: &Path,
    scope: &str,
    triples: &mut Vec<RawTriple>,
) -> Result<(), LoadError> {
    for triple in TurtleParser::new().for_slice(bytes) {
        let triple = triple.map_err(|e| LoadError::Parse {
            path: path.to_path_buf(),
            reason: e.to_string(),
        })?;
        let subject = named_or_blank(triple.subject, scope);
        let predicate = triple.predicate.into_string();
        let object = match triple.object {
            oxrdf::Term::NamedNode(node) => RawTerm::Entity(RawNode::Iri(node.into_string())),
            oxrdf::Term::BlankNode(node) => {
                RawTerm::Entity(RawNode::Blank(format!("{scope}:{}", node.into_string())))
            }
            oxrdf::Term::Literal(literal) => {
                if literal.language().is_some() || literal.datatype().as_str() != XSD_STRING {
                    return Err(LoadError::TypedLiteral {
                        path: path.to_path_buf(),
                        predicate,
                    });
                }
                RawTerm::Text(literal.value().to_string())
            }
        };
        triples.push(RawTriple {
            subject,
            predicate,
            object,
        });
    }
    Ok(())
}

fn named_or_blank(node: oxrdf::NamedOrBlankNode, scope: &str) -> RawNode {
    match node {
        oxrdf::NamedOrBlankNode::NamedNode(node) => RawNode::Iri(node.into_string()),
        oxrdf::NamedOrBlankNode::BlankNode(node) => {
            RawNode::Blank(format!("{scope}:{}", node.into_string()))
        }
    }
}
```

## Blank nodes get durable names

A blank node is Turtle syntax, not an identity. It cannot go into the fact
plane as a label, because the label means nothing outside the file it came
from — and the fact plane has no files. The answer is a skolem URI per
connected blank-node component, assigned deterministically so the same files
always yield the same URIs, and the renderer turns the prefix back into blank
labels when it materializes the file again.

The determinism is bought by ordering: the roots are the IRI-subject triples
that point at a blank node, sorted and deduped, so component numbering
follows the vocabulary rather than the parse. Any blank node the roots did
not reach — one reachable only from another blank node, which a well-formed
file should not produce — is numbered after them rather than left unassigned.

<a name="chunk-assign-structural-uris"></a><sub>[`src/load.rs`](../../crates/x0k-ontology/src/load.rs) · `#assign-structural-uris`</sub>

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

<a name="chunk-assign-blank-component"></a><sub>[`src/load.rs`](../../crates/x0k-ontology/src/load.rs) · `#assign-blank-component`</sub>

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
            format!("{STRUCTURAL_NODE_PREFIX}b{root_index:04}n{node_index:04}"),
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

<a name="chunk-node-entity"></a><sub>[`src/load.rs`](../../crates/x0k-ontology/src/load.rs) · `#node-entity`</sub>

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

fn node_entity(node: &RawNode, blank_uris: &BTreeMap<String, String>) -> Result<String, LoadError> {
    match node {
        RawNode::Iri(iri) => Ok(iri.clone()),
        RawNode::Blank(blank) => blank_uris
            .get(blank)
            .cloned()
            .ok_or_else(|| LoadError::UnassignedBlankNode(blank.clone())),
    }
}
```

## Tests

The first test is the one that makes this chapter worth having: load the
files this build compiled and check that the model reads them the way the
tables say they read. Class labels, the object properties with their domains
and ranges, the Decision-domain slice, and the module set are compared whole
rather than sampled — a loader that agreed on most rows would be worse than
one that disagreed on all of them, because the disagreement would surface
somewhere far from here.

<a name="chunk-tests"></a><sub>[`src/load.rs`](../../crates/x0k-ontology/src/load.rs) · `#tests`</sub>

```rust {#tests}
#[cfg(test)]
mod tests {
    use super::*;

    fn shipped_dir() -> PathBuf {
        shipped_modules_dir(Path::new(env!("CARGO_MANIFEST_DIR")))
    }

    #[test]
    fn a_loaded_module_set_folds_to_the_compiled_tables() {
        let dir = shipped_dir();
        let model = OntologyModel::load(&dir)
            .unwrap_or_else(|e| panic!("load {}: {e}", dir.display()));

        let loaded: Vec<(String, String)> = model
            .classes()
            .into_iter()
            .map(|class| (class.uri, class.label))
            .collect();
        let compiled: Vec<(String, String)> = crate::ONTOLOGY_CLASSES
            .iter()
            .map(|class| (class.uri.to_string(), class.label.to_string()))
            .collect();
        assert_eq!(loaded, compiled, "class names or labels differ");

        let loaded: Vec<(String, Option<String>, Option<String>)> = model
            .object_properties()
            .into_iter()
            .map(|property| (property.uri, property.domain, property.range))
            .collect();
        let compiled: Vec<(String, Option<String>, Option<String>)> =
            crate::ONTOLOGY_OBJECT_PROPERTIES
                .iter()
                .map(|property| {
                    (
                        property.uri.to_string(),
                        property.domain.map(str::to_string),
                        property.range.map(str::to_string),
                    )
                })
                .collect();
        assert_eq!(loaded, compiled, "predicate domains or ranges differ");

        let loaded: Vec<String> = model
            .decision_edge_predicates()
            .into_iter()
            .map(|(snake, _)| snake)
            .collect();
        let compiled: Vec<String> = crate::KNOWN_EDGE_PREDICATES
            .iter()
            .map(|predicate| predicate.to_string())
            .collect();
        assert_eq!(loaded, compiled, "Decision-domain slice differs");

        let loaded: Vec<String> = model
            .modules()
            .into_iter()
            .map(|module| module.name)
            .collect();
        assert_eq!(loaded, crate::MODULES, "module set differs");
    }

    #[test]
    fn the_shipped_model_and_a_loaded_one_are_the_same_fold() {
        // `shipped()` folds the facts the build script emitted; `load`
        // folds the files it emitted them from. Same facts, so same model.
        let loaded = OntologyModel::load(&shipped_dir()).expect("load");
        assert_eq!(loaded.facts(), OntologyModel::shipped().facts());
    }

    /// A module directory written from scratch: one file per `(name, body)`,
    /// each body a Turtle fragment appended to the module's own declaration.
    fn scratch_modules(dir: &Path, modules: &[(&str, &str)]) {
        std::fs::create_dir_all(dir).expect("create module directory");
        for (name, body) in modules {
            std::fs::write(dir.join(format!("{name}.ttl")), body).expect("write module");
        }
    }

    #[test]
    fn a_set_whose_imports_do_not_close_is_refused() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let dir = tmp.path().join("modules");
        scratch_modules(
            &dir,
            &[(
                "mycorp",
                "<https://0k.computer/ontology/mycorp> \
                 <http://www.w3.org/1999/02/22-rdf-syntax-ns#type> \
                 <http://www.w3.org/2002/07/owl#Ontology> .\n\
                 <https://0k.computer/ontology/mycorp> \
                 <http://www.w3.org/2002/07/owl#imports> \
                 <https://0k.computer/ontology/core> .\n",
            )],
        );
        let error = OntologyModel::load(&dir).expect_err("a set missing an import is not a set");
        assert!(
            matches!(&error, LoadError::Imports(reason) if reason.contains("not a module of the set")),
            "wrong error: {error}"
        );
    }

    #[test]
    fn a_file_whose_module_fact_is_missing_is_refused() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let dir = tmp.path().join("modules");
        scratch_modules(&dir, &[("mycorp", "# no module fact at all\n")]);
        assert!(
            matches!(
                OntologyModel::load(&dir),
                Err(LoadError::ModuleFiles { .. })
            ),
            "a file declaring no module is a set the tree did not receive in full"
        );
    }

    #[test]
    fn a_directory_with_no_module_files_is_refused() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let dir = tmp.path().join("modules");
        std::fs::create_dir_all(&dir).expect("create");
        assert!(matches!(
            OntologyModel::load(&dir),
            Err(LoadError::EmptySet { .. })
        ));
        assert!(matches!(
            OntologyModel::load(&tmp.path().join("absent")),
            Err(LoadError::ReadDir { .. })
        ));
    }
}
```

## Composing the module

<a name="chunk-root"></a><sub>[`src/load.rs`](../../crates/x0k-ontology/src/load.rs) · `#root` · assembles [module-doc](#chunk-module-doc) · [imports](#chunk-imports) · [xsd-string](#chunk-xsd-string) · [load-error](#chunk-load-error) · [load](#chunk-load) · [shipped-modules-dir](#chunk-shipped-modules-dir) · [module-file-paths](#chunk-module-file-paths) · [check-files](#chunk-check-files) · [raw-terms](#chunk-raw-terms) · [parse-model](#chunk-parse-model) · [parse-module-file](#chunk-parse-module-file) · [assign-structural-uris](#chunk-assign-structural-uris) · [assign-blank-component](#chunk-assign-blank-component) · [node-entity](#chunk-node-entity) · [parse-turtle-sources](#chunk-parse-turtle-sources) · [tests](#chunk-tests)</sub>

```rust {#root}
<<module-doc>>

<<imports>>

<<xsd-string>>

<<load-error>>

<<load>>

<<shipped-modules-dir>>

<<module-file-paths>>

<<check-files>>

<<raw-terms>>

<<parse-model>>

<<parse-module-file>>

<<assign-structural-uris>>

<<assign-blank-component>>

<<node-entity>>

<<parse-turtle-sources>>

<<tests>>
```

What this changes about the crate is smaller than it looks and larger than it
reads. The tables are still what a linker holds and still what a consumer
gets for free; `shipped()` is still the answer when nobody has a better one.
What is new is that "which vocabulary" became a *parameter* rather than a
property of the binary — and every check that used to measure the build now
measures whatever set it was pointed at, which is the only way a reader with
their own module can be told the truth about their own documents.

## Turtle carried by documents

A vocabulary can arrive as named text fragments instead of files. The
caller supplies a unique source name for each fragment; parse errors keep
that name, and blank nodes remain scoped to their source. Parsing still
uses the same Turtle parser and fact fold. Import and declaration checks
belong to the assembled collection, so this entry point only parses.

<a name="chunk-parse-turtle-sources"></a><sub>[`src/load.rs`](../../crates/x0k-ontology/src/load.rs) · `#parse-turtle-sources`</sub>

```rust {#parse-turtle-sources}
/// Validate an absolute RDF IRI with the Turtle parser's own RDF term type.
pub fn is_absolute_iri(value: &str) -> bool {
    oxrdf::NamedNode::new(value).is_ok()
}

/// One Turtle fragment with a caller-assigned source identity.
pub struct TurtleSource<'a> {
    pub name: &'a Path,
    pub text: &'a str,
}

impl OntologyModel {
    /// Parse named Turtle fragments without reading files or fetching imports.
    pub fn parse_turtle_sources(sources: &[TurtleSource<'_>]) -> Result<Self, LoadError> {
        let mut triples = Vec::new();
        for source in sources {
            parse_module_file(source.text.as_bytes(), source.name,
                &source.name.to_string_lossy(), &mut triples)?;
        }
        fold_triples(triples)
    }
}
```
