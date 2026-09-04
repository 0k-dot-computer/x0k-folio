---
x0k:
  format: folio/v1
  id: x0k:implementation/ontology/views
  type: implementation
  status: draft
  summary: How the crate root turns whichever module set is present into the class, property, and edge-predicate tables other crates check a document against.
  concerns: [ontology, vocabulary, codegen, predicates, crate-root]
  tangle:
    crate: x0k-ontology
    root: src/lib.rs
  edges:
    implements:
      - x0k:architecture/state-representation
    cites:
      - x0k:implementation/ontology/concept-facts
      - x0k:implementation/ontology/module-bootstrap
      - x0k:implementation/ontology/concept-region
      - x0k:implementation/ontology/declaration
---

# The crate root is a compatibility view

`x0k-ontology` is the crate every other crate reaches for when it needs to
spell a predicate. The authoritative vocabulary is the self-typed
`x0k:Concept` root in the fact plane; the module files under
`ontology/modules/` are the checked bootstrap that seeds an empty concept
region. What consumers actually link against is neither: it is a pair of
constant slices and one match function that
[`build.rs`](module-bootstrap.md) emits from the union of those files at
compile time, plus one `pub mod` per vocabulary module
carrying the same tables restricted to what that module defines. This
chapter is that root — the Rust module list, the `include!` of the
generated code, and the lookups over the generated tables.

The reason the root is this thin is dependency weight. The TTL parser
(`oxttl`) is a build-only dependency; the runtime crate graph never sees it.
A consumer that needs `KNOWN_EDGE_PREDICATES` gets a `&[&str]`, and a folio
envelope that needs to know that `motivated_by` is `x0k:motivatedBy` calls
`snake_to_camel` — a generated `match`, not a parser.

## The two spellings

Snake-case is the wire and frontmatter form on folio/v1 envelopes;
camelCase is the URI suffix in the ontology view. The map is deterministic
in both directions — insert `_` before each uppercase boundary (excluding
position 0) and lowercase — but the crate exposes only the generated table,
so the two forms can never drift apart from the TTL they were emitted from.

```rust {#module-doc}
//! Fold-derived ontology compatibility views.
//!
//! `KNOWN_EDGE_PREDICATES` and `snake_to_camel` are emitted by
//! `build.rs` from [`concept_facts::OntologyModel`] and re-exported here.
//! `ontology/modules/*.ttl` are the checked bootstrap/materialized views,
//! one per vocabulary module (`x0k:architecture/ontology-modules`): their
//! union seeds an empty concept region, after which the self-typed
//! `x0k:Concept` root in the fact plane is authoritative. `MODULES` names
//! the set, sorted by name; each `pub mod <name>` (`core`, `document`,
//! …) carries that module's `IRI`, `IMPORTS`, and its slice of the tables,
//! and `MODULE_TABLES` indexes those slices by name for code that must
//! not assume which modules a build shipped — a publication compiles a
//! subset, and only `core` is certain to be present.
//!
//! The runtime crate graph never sees `oxttl` — the TTL parser is a
//! build-only dependency, and a consumer only ever sees the generated
//! `&[&str]` constant slices and the `snake_to_camel` match function.
//!
//! The hand-authored vocabularies that spell x0k's own internal
//! subjects live behind the off-by-default `product` feature; a build
//! that does not ask for it gets the fold and its views and nothing
//! else.
//!
//! Snake-case is the wire/frontmatter form on folio/v1 envelopes;
//! camelCase is the URI suffix that appears in the ontology view. The two
//! forms map deterministically by inserting `_` before uppercase
//! letter boundaries (excluding position 0) and lowercasing.
```

## Two kinds of module, and only one of them always ships

The word does double duty here, so the root is where the two are kept
apart. A **vocabulary module** is a declared region of the concept region
— `core`, `document`, `work` — and it has no chapter at all: the generated
file supplies `MODULES`, `MODULE_FILES`, one `pub mod` per shipped
vocabulary module, `MODULE_TABLES` over them, and the crate-level union
`KNOWN_EDGE_PREDICATES`, `ONTOLOGY_CLASSES`, `ONTOLOGY_OBJECT_PROPERTIES`,
and `snake_to_camel`. Which of those exist is decided by the module files
the build was given, so nothing here names one.

The hand-written `pub mod`s below are the other kind: chapters spelling a
vocabulary that has not entered the concept region yet, each a set of
URI-string constants and closed enumerations for one internal subject —
grants, calendars, declarations, entries, findings, proposals,
settlements, usage. They are about x0k's own workings, no consumer of the
fold calls them, and they cannot be checked against the fold because the
region does not know their terms. So they sit behind a `product` feature
that is off by default: `concept_facts` and the generated views are what a
plain build gets, and a workspace that spells one of these vocabularies
asks for the feature on its own dependency line.

That is a feature gate rather than a second crate because nothing here is
being built — the eight chapters already exist and twenty-five call sites
already import them. The gate says which of them a build knows; splitting
the crate would say it with a version seam instead, and there is no
adopter for whom that is the better answer.

```rust {#modules}
pub mod concept_facts;

#[cfg(feature = "product")]
pub mod admission_grant;
#[cfg(feature = "product")]
pub mod calendar;
#[cfg(feature = "product")]
pub mod declaration;
#[cfg(feature = "product")]
pub mod entry_vocab;
#[cfg(feature = "product")]
pub mod finding_vocab;
#[cfg(feature = "product")]
pub mod proposal_vocab;
#[cfg(feature = "product")]
pub mod settlement;
#[cfg(feature = "product")]
pub mod usage_vocab;

include!(concat!(env!("OUT_DIR"), "/generated.rs"));
```

## Lookups over the generated tables

Two questions the UI and the envelope validator ask. A class URI's label,
or `None` for a synthetic UI-only class the vocabulary does not declare; and
an object property's `(domain, range)`, either half of which may itself be
`None` when the property omits that constraint.

```rust {#lookups}
/// The `rdfs:label` for a class URI (bare `x0k:` form), or `None` if the
/// URI is not a vocabulary class (e.g. a synthetic UI-only class like
/// `x0k:Ontology` / `x0k:Home` / `x0k:Population`).
pub fn class_label(uri: &str) -> Option<&'static str> {
    ONTOLOGY_CLASSES
        .iter()
        .find(|c| c.uri == uri)
        .map(|c| c.label)
}

/// The resolved `(domain, range)` for an object-property URI (bare `x0k:`
/// form), or `None` if no shipped module declares it as an object
/// property — `x0k:synthesizes`, a UI-only edge no vocabulary declares at
/// all, is the live example. Either component may itself be `None` when
/// the property omits that constraint, which is a different answer
/// entirely: `cites` is a `core` term with neither a domain nor a range,
/// so it resolves to `Some((None, None))` — declared, and constrained
/// nowhere, since a citation's target may be a document in the corpus or
/// one on the web.
pub fn predicate_domain_range(uri: &str) -> Option<(Option<&'static str>, Option<&'static str>)> {
    ONTOLOGY_OBJECT_PROPERTIES
        .iter()
        .find(|p| p.uri == uri)
        .map(|p| (p.domain, p.range))
}

/// The checked `ontology/modules/<name>.ttl` this crate was built from, so
/// round-trip tests compare against the bytes the compiler saw.
pub fn checked_module_file(name: &str) -> Option<&'static str> {
    MODULE_FILES
        .iter()
        .find(|(module, _)| *module == name)
        .map(|(_, contents)| *contents)
}

/// The checked `ontology/shapes/<name>.ttl` this crate was built from, or
/// `None` when the module constrains nothing. Absence is the ordinary case
/// and never a missing file: a shape file exists only where there are shapes.
pub fn checked_shape_file(name: &str) -> Option<&'static str> {
    SHAPE_FILES
        .iter()
        .find(|(module, _)| *module == name)
        .map(|(_, contents)| *contents)
}
```

## Tests

The tests pin the codegen contract rather than any particular predicate
list — and, since the module set is chosen per build (a publication ships a
subset, `x0k:architecture/ontology-modules` §6), they never name a module
that might be absent. They walk `MODULE_TABLES`: the set is closed under
imports, `core` is always in it (it is the one module with no imports and
every other module imports it), each term lives in exactly one module and
the union is the sum, and the placement the ADR's table fixes holds for
whichever modules are present. Predicates the corpus actively uses are
checked module by module for the same reason: `supersedes` sits in `work`
(its declared domain and range are both the union of `Decision` and
`Observation`, and `Observation` is `work`'s own term, so `document`
could not carry the predicate without using a term it does not import)
and `constrains` in `product`, so a `[core, document]` build legitimately
lacks both. `defines` and `verified_by` are `software`'s, and a
`[core, document, software]` build has them: that set is exactly what the
split was for. `implements` is pinned to `document` because the corpus's 273
implementation documents carrying it are the reason the module set exists — a
build that ships the document genus and cannot spell the edge its own
documents carry would be the failure the split was drawn to prevent. It sits
there with a domain and no range: the subject is what puts a predicate in this
table at all (the fold reads `rdfs:domain`, and `x0k:subjectClass` where a
shape carries the subject instead; `rdfs:range` plays no part either way), and
the missing range is what let the predicate land in `document` without naming a
class `document` cannot import. `realizes` is the one whose subject arrives
from a shape: its three carriers live in three modules, so it is `core`'s and
its domain is in `ontology/shapes/core.ttl`.

```rust {#tests}
#[cfg(test)]
mod tests {
    use super::*;

    fn shipped(name: &str) -> bool {
        MODULES.contains(&name)
    }

    #[test]
    fn generated_slice_is_non_empty() {
        if !shipped("document") {
            return;
        }
        assert!(
            !KNOWN_EDGE_PREDICATES.is_empty(),
            "build.rs failed to emit any Decision-domain predicates from \
             ontology/modules/*.ttl; either the parser silently dropped triples \
             or the domain-resolution logic regressed."
        );
    }

    #[test]
    fn every_known_predicate_has_camel_mapping() {
        for snake in KNOWN_EDGE_PREDICATES {
            assert!(
                snake_to_camel(snake).is_some(),
                "KNOWN_EDGE_PREDICATES contains `{snake}` but \
                 snake_to_camel has no mapping for it. The two are emitted \
                 from the same source set in build.rs, so a missing pair \
                 means the codegen regressed."
            );
        }
        // Unknown predicates return None.
        assert_eq!(snake_to_camel("definitely_not_a_predicate"), None);
    }

    /// Predicates the corpus actively uses, by the module that defines them.
    const CANONICAL: &[(&str, &[(&str, &str)])] = &[
        (
            // A predicate whose subject reaches `Decision` through a shape
            // rather than an `rdfs:domain`, which is what `core` residency
            // costs and what clause 8 buys. If this line goes missing the
            // fold has stopped reading the shape half of the T-box union and
            // every decision document's `realizes` edge has gone unrecognised.
            "core",
            &[("realizes", "realizes")],
        ),
        (
            "document",
            &[
                ("supports", "supports"),
                ("refined_by", "refinedBy"),
                ("mentions", "mentions"),
                ("raises", "raises"),
                ("resolves", "resolves"),
                ("implements", "implements"),
                // Placed here by its `x0k:Decision` domain, with its former
                // `Intent` range now a shape
                // (`x0k:architecture/vocabulary-shapes` §3). Before that it
                // sat in `work`, and a `[core, document]` build — what an
                // adopter of the document format takes — could not spell the
                // commonest edge in the corpus.
                ("motivated_by", "motivatedBy"),
                ("published_by", "publishedBy"),
                ("published_for", "publishedFor"),
                // Same treatment, same reason: an `x0k:Decision` domain, and
                // a range unioning `Decision` with `Observation` — a `work`
                // term — that is now a shape.
                ("informed_by", "informedBy"),
                // Declared 2026-09-04 rather than moved: the publication
                // manifest had carried it as a forward-compat edge no module
                // spelled. Domain `Publication`, which subclasses `Decision`,
                // so it lands in the slice; its two admissible targets span
                // `document` and `product` and are a shape.
                ("excludes", "excludes"),
            ],
        ),
        (
            "work",
            &[
                ("supersedes", "supersedes"),
                ("superseded_by", "supersededBy"),
            ],
        ),
        (
            "software",
            &[
                ("defines", "defines"),
                ("verified_by", "verifiedBy"),
            ],
        ),
        (
            "product",
            &[
                ("constrains", "constrains"),
                ("serves", "serves"),
                ("depends_on", "dependsOn"),
            ],
        ),
    ];

    #[test]
    fn canonical_predicates_are_present() {
        // Smoke check: for every shipped module, the predicates the corpus
        // uses from it are emitted, in the module's own table and in the
        // union. Adding a new Decision-domain predicate to a module file is
        // enough to make it appear; removing one of these (other than
        // during a deliberate ontology edit) should fail loudly.
        for (module, predicates) in CANONICAL {
            if !shipped(module) {
                continue;
            }
            let tables = MODULE_TABLES.iter().find(|t| t.name == *module).unwrap();
            for (snake, camel) in *predicates {
                assert!(
                    tables.edge_predicates.contains(snake),
                    "expected canonical predicate `{snake}` in {module}::EDGE_PREDICATES"
                );
                assert!(
                    KNOWN_EDGE_PREDICATES.contains(snake),
                    "expected canonical predicate `{snake}` in KNOWN_EDGE_PREDICATES"
                );
                assert_eq!((tables.snake_to_camel)(snake), Some(*camel));
                assert_eq!(snake_to_camel(snake), Some(*camel));
            }
        }
    }

    #[test]
    fn modules_partition_the_union() {
        // The index and the name list agree; a checked file exists for
        // every module; the set is closed under imports and rooted in
        // `core`.
        let names: Vec<&str> = MODULE_TABLES.iter().map(|t| t.name).collect();
        assert_eq!(names, MODULES);
        assert!(!MODULES.is_empty(), "x0k-ontology built from an empty module set");
        assert!(shipped("core"), "every module set is rooted in core");
        assert_eq!(core::IRI, "https://0k.computer/ontology/core");
        assert!(core::IMPORTS.is_empty());
        for tables in MODULE_TABLES {
            assert!(checked_module_file(tables.name).is_some(), "no checked file for module {}", tables.name);
            assert_eq!(tables.iri, format!("https://0k.computer/ontology/{}", tables.name));
            for import in tables.imports {
                assert!(
                    MODULE_TABLES.iter().any(|t| t.iri == *import),
                    "{} imports {import}, which the build did not ship",
                    tables.name
                );
            }
        }

        // Each term lives in exactly one module and the union is the sum.
        let class_count: usize = MODULE_TABLES.iter().map(|t| t.classes.len()).sum();
        assert_eq!(class_count, ONTOLOGY_CLASSES.len());
        let property_count: usize = MODULE_TABLES.iter().map(|t| t.object_properties.len()).sum();
        assert_eq!(property_count, ONTOLOGY_OBJECT_PROPERTIES.len());
        let edge_count: usize = MODULE_TABLES.iter().map(|t| t.edge_predicates.len()).sum();
        assert_eq!(edge_count, KNOWN_EDGE_PREDICATES.len());

        // Placement follows the ADR's table, for whichever modules are here.
        for (module, class) in [
            ("core", "x0k:Concept"),
            ("document", "x0k:Decision"),
            ("work", "x0k:Intent"),
            ("actor", "x0k:Actor"),
            ("software", "x0k:Bundle"),
            ("product", "x0k:SoftwareModule"),
        ] {
            if let Some(tables) = MODULE_TABLES.iter().find(|t| t.name == module) {
                assert!(
                    tables.classes.iter().any(|c| c.uri == class),
                    "{module} should define {class}"
                );
            }
        }
        for (module, imports) in [
            ("document", &["core"][..]),
            ("work", &["core", "document"]),
            ("actor", &["core"]),
            ("software", &["core", "document"]),
            ("product", &["actor", "core", "document", "software", "work"]),
        ] {
            if let Some(tables) = MODULE_TABLES.iter().find(|t| t.name == module) {
                let expected: Vec<String> =
                    imports.iter().map(|i| format!("https://0k.computer/ontology/{i}")).collect();
                assert_eq!(tables.imports, expected, "{module} imports");
            }
        }
        if let Some(document) = MODULE_TABLES.iter().find(|t| t.name == "document") {
            assert!(document.edge_predicates.contains(&"supports"));
            // `motivated_by` is `document`'s since the shape extraction, and
            // `depends_on` is not: its domain is a union of four classes that
            // do not join, so clause 8 places it in `core`… which it has not
            // been moved to yet. What it is NOT is document's, which is the
            // property this line is pinning.
            assert_eq!((document.snake_to_camel)("depends_on"), None);
        }
    }
}
```

## The file

```rust {#root}
<<module-doc>>

<<modules>>

<<lookups>>

<<tests>>
```

`build.rs` runs before this crate exists, so it cannot depend on it: it
pulls `src/concept_facts.rs` in by path and folds the module set itself.
That is its own chapter — [`module-bootstrap.md`](module-bootstrap.md).
