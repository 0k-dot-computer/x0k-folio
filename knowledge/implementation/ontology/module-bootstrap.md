---
x0k:
  format: folio/v1
  id: x0k:implementation/ontology/module-bootstrap
  type: implementation
  status: draft
  summary: The build script that loads the checked vocabulary modules through the library's own loader and emits the constant tables the crate root re-exports — so a consumer gets a linked table without folding anything.
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
      - x0k:implementation/ontology/load
      - x0k:implementation/ontology/views
---

# The vocabulary, folded once, at build time

Nearly every crate in the tree spells an ontology predicate at some point,
and most of them want the answer as a `&[&str]` and a `match`. That is the whole
reason this build script exists. The vocabulary — [RDF and
OWL](x0k:wiki/rdf-and-owl), written as Turtle — lives in the fact plane and is
materialized out to `ontology/modules/*.ttl`
([`concept-facts.md`](concept-facts.md)); consumers want tables
([`views.md`](views.md)). This script is the one place the two meet: it runs
once per build, loads the checked module files, and writes
`$OUT_DIR/generated.rs`.

It does not read them itself. [`load.md`](load.md) owns the parse, the fold,
and the refusals, because a caller at run time needs exactly the same three and
two readings of one vocabulary is the drift this crate exists to prevent. The
script pulls that module in by path, the same way it pulls
[`concept-facts.md`](concept-facts.md)'s fold, and what remains here is the
emission: eleven `emit_*` functions that turn a folded model into Rust text.

One example carries the chapter. The `work` module declares itself an
`owl:Ontology`, imports `core`, and defines terms like `x0k:Intent` that say so
with `rdfs:isDefinedBy`. Build a tree holding `work` and `core` and both files
are parsed, unioned, ordered `core` before `work`, and emitted as two `pub mod`s
of tables. Build a tree holding `work` alone and the build stops — the loader
returns the missing import and this script turns it into a panic.

## Contract

Every failure here is a panic. A build script has one channel, and a
half-emitted table is worse than a build that stopped: consumers would compile
against a vocabulary missing terms and only notice at the point of use. That is
the one place this script and the loader part company — a `LoadError` is a
`Result` there and a stopped build here — so the script reads the module
directory, writes exactly one file, panics on anything it cannot account for,
and touches nothing else: no network, no clock, no state between runs.

<a name="chunk-module-doc"></a><sub>[`build.rs`](../../../x0k-ontology/build.rs) · `#module-doc`</sub>

```rust {#module-doc}
//! Load `ontology/modules/*.ttl` through the crate's own loader and emit
//! the compatibility registry views into `$OUT_DIR/generated.rs`.
//!
//! The checked Turtle is not the authority: the module files are the
//! bootstrap/materialized view used when a profile has no self-typed
//! `x0k:Concept` root. Emitting the fold as constants lets a consumer link
//! a table instead of folding one, while the entry-spine fold becomes
//! canonical at runtime. Which files are read, and which sets are refused,
//! is `src/load.rs`'s answer and not this script's — a publication ships a
//! set, and the set is checked in one place so a build and a run agree
//! about it.
```

Two of the crate's own source files are pulled in by path rather than by
dependency: the fold in `src/concept_facts.rs` and the loader in
`src/load.rs`. Both are the code the crate runs, and compiling one copy of
each into the build script is what keeps a build and a run from becoming two
readings of the same bytes. `dead_code` is allowed on both because the build
script uses a proper subset of what each offers.

<a name="chunk-imports"></a><sub>[`build.rs`](../../../x0k-ontology/build.rs) · `#imports`</sub>

```rust {#imports}
use std::env;
use std::path::PathBuf;
```

<a name="chunk-concept-facts-by-path"></a><sub>[`build.rs`](../../../x0k-ontology/build.rs) · `#concept-facts-by-path`</sub>

```rust {#concept-facts-by-path}
#[path = "src/concept_facts.rs"]
#[allow(dead_code)]
mod concept_facts;

#[path = "src/load.rs"]
#[allow(dead_code)]
mod load;

use concept_facts::{ModuleRecord, OntologyFact, OntologyModel, OntologyValue};
```

## The run

`main` reads as the outline of the whole script: find the modules, tell
cargo what to watch, fold, check, emit.

<a name="chunk-main"></a><sub>[`build.rs`](../../../x0k-ontology/build.rs) · `#main` · assembles [locate-modules](#chunk-locate-modules) · [declare-reruns](#chunk-declare-reruns) · [collect-module-paths](#chunk-collect-module-paths) · [fold-and-check](#chunk-fold-and-check) · [write-generated](#chunk-write-generated)</sub>

```rust {#main}
fn main() {
    <<locate-modules>>

    <<declare-reruns>>

    <<collect-module-paths>>

    <<fold-and-check>>

    <<write-generated>>
}
```

Where the modules are depends on who is building, and the loader answers
that: a packaged crate carries a vendored copy inside itself, the monorepo
reads the canonical directory one level up, and the in-crate copy wins when
it exists ([`load.md`](load.md)). The script asks rather than deciding, so a
test can ask the same question and get the same answer.

<a name="chunk-locate-modules"></a><sub>[`build.rs`](../../../x0k-ontology/build.rs) · `#locate-modules`</sub>

```rust {#locate-modules}
let manifest = PathBuf::from(env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR"));
let modules_dir = load::shipped_modules_dir(&manifest);
let shapes_dir = load::shapes_dir_for(&modules_dir);
```

The rerun declarations name the directories *and* every file in them. A
directory alone would miss an edit to a file already present; the files alone
would miss a module being added:

<a name="chunk-declare-reruns"></a><sub>[`build.rs`](../../../x0k-ontology/build.rs) · `#declare-reruns`</sub>

```rust {#declare-reruns}
println!("cargo:rerun-if-changed={}", modules_dir.display());
println!("cargo:rerun-if-changed={}", shapes_dir.display());
println!("cargo:rerun-if-changed=build.rs");
println!("cargo:rerun-if-changed=src/concept_facts.rs");
println!("cargo:rerun-if-changed=src/load.rs");
```

The file lists are the script's own because it needs them twice over: once to
declare the reruns, and once to `include_str!` each file's bytes into the
emitted tables.

<a name="chunk-collect-module-paths"></a><sub>[`build.rs`](../../../x0k-ontology/build.rs) · `#collect-module-paths`</sub>

```rust {#collect-module-paths}
let module_paths = load::module_file_paths(&modules_dir)
    .unwrap_or_else(|error| panic!("{error}"));
let shape_paths = load::shape_file_paths(&shapes_dir);
for path in module_paths.iter().chain(shape_paths.iter()) {
    println!("cargo:rerun-if-changed={}", path.display());
}
```

The two lines that follow are the check this script used to own and now
delegates. `load_files` refuses a set whose imports name an absent module or
form a cycle, a set whose declared modules and whose files disagree, and a
shape file naming no module of the set. All three are the same refusals a
caller loading a bundle's own vocabulary gets; here they are a stopped build.

<a name="chunk-fold-and-check"></a><sub>[`build.rs`](../../../x0k-ontology/build.rs) · `#fold-and-check`</sub>

```rust {#fold-and-check}
let model = OntologyModel::load_files(&module_paths, &shape_paths)
    .unwrap_or_else(|error| panic!("ontology module set under {}: {error}", modules_dir.display()));
let out = emit_generated(&model, &module_paths, &shape_paths);
```

<a name="chunk-write-generated"></a><sub>[`build.rs`](../../../x0k-ontology/build.rs) · `#write-generated`</sub>

```rust {#write-generated}
let out_dir = PathBuf::from(env::var("OUT_DIR").expect("OUT_DIR"));
let out_path = out_dir.join("generated.rs");
std::fs::write(&out_path, out)
    .unwrap_or_else(|e| panic!("write {}: {e}", out_path.display()));
```

## Emitting the tables

What comes out is one Rust file, built by string concatenation. This is
plumbing, and the chapter says so rather than deriving it: `emit_generated`
names the sections in order, and each `emit_*` below pushes one of them.

<a name="chunk-emit-generated"></a><sub>[`build.rs`](../../../x0k-ontology/build.rs) · `#emit-generated`</sub>

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

<a name="chunk-emit-module-set"></a><sub>[`build.rs`](../../../x0k-ontology/build.rs) · `#emit-module-set`</sub>

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
            load::module_file_name(path),
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
            load::module_file_name(path),
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

<a name="chunk-emit-module"></a><sub>[`build.rs`](../../../x0k-ontology/build.rs) · `#emit-module`</sub>

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

<a name="chunk-emit-bootstrap-facts"></a><sub>[`build.rs`](../../../x0k-ontology/build.rs) · `#emit-bootstrap-facts`</sub>

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

<a name="chunk-emit-classes"></a><sub>[`build.rs`](../../../x0k-ontology/build.rs) · `#emit-classes`</sub>

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

<a name="chunk-root"></a><sub>[`build.rs`](../../../x0k-ontology/build.rs) · `#root` · assembles [module-doc](#chunk-module-doc) · [imports](#chunk-imports) · [concept-facts-by-path](#chunk-concept-facts-by-path) · [main](#chunk-main) · [emit-generated](#chunk-emit-generated) · [emit-module-set](#chunk-emit-module-set) · [emit-module](#chunk-emit-module) · [emit-bootstrap-facts](#chunk-emit-bootstrap-facts) · [emit-classes](#chunk-emit-classes)</sub>

```rust {#root}
<<module-doc>>

<<imports>>

<<concept-facts-by-path>>

<<main>>

<<emit-generated>>

<<emit-module-set>>

<<emit-module>>

<<emit-bootstrap-facts>>

<<emit-classes>>
```

The file this script writes is a *view*, and saying so is not a hedge. The
authority is the self-typed `x0k:Concept` root in the fact plane; the module
files are the checked bootstrap that seeds an empty region; these tables are
what a linker can hold. Emitting the fold as constants is what lets the
static-table consumers compile unchanged while the runtime fold becomes
canonical underneath them — and running the check at the earliest moment is
why a set that does not close is caught here rather than at the publication
boundary. A set that does not close cannot be projected into a repository
someone else builds, and a stopped build is the earliest anyone can be told.
