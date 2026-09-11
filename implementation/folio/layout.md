---
x0k:
  format: folio/v1
  id: x0k:implementation/folio/layout
  type: implementation
  status: draft
  summary: Where a genus lives inside a corpus, and the scope a caller must name to ask — one table replacing six copies, all of which had drifted and none of which said so.
  concerns:
  - folio
  - layout
  - corpus
  - genus
  - paths
  - relayout
  tangle:
    crate: crates/x0k-folio
    root: src/layout.rs
  edges:
    implements:
    - x0k:architecture/corpus-boundaries
    cites:
    - x0k:architecture/filesystem-graph-materialization
    - x0k:architecture/monorepo-layout
---

# Where a genus lives

A folio document's identity is its envelope `id:`, and its address on disk
is a function of two things the id does not carry: which **corpus** holds
it, and which **genus** it belongs to. This chapter is that function. It is
one table, and the reason it is worth a chapter is that it used to be six.

## The measurement that produced this chapter

On 2026-09-09 there were 1350 folio/v1 documents in this tree and the
production corpus walk reached **none of them**:

```
cold index_corpus: 0 docs from /home/lsp/src/l30/drive-hostdecomp-c in 597.108µs
```

Six hundred microseconds is the shape of the defect. Nothing was slow;
nothing errored; the walk opened directories that were not there, found
nothing, and returned success. The 2026-09 relayout moved every corpus under
`corpora/<name>/` and the constants naming those directories — `decisions`,
`knowledge/wiki`, `knowledge/implementation` — were not moved with them.

Two things kept it invisible for three months, and both are worth stating
because both are shapes that recur.

**The constants were corpus-relative and the callers passed a repo root.**
`folio_lint` and `unified_search_corpus` both did `repo_root.join(genus)`,
producing `<repo>/decisions`. The one test that walked the same table joined
`corpora/x0k` onto the root itself before calling in, so the same constants
resolved for the test and only for the test — a third copy of the layout,
inlined in a test body, holding the green.

**The one test that exercised the production path asserted nothing.** It
measured cost, printed a count, and was marked `#[ignore]`. It would have
printed `0 docs` on every run anybody made.

So the type here does not have a `Default`, and the caller cannot avoid
saying which corpus it means.

## The scope is named, never defaulted

<a name="chunk-corpus-scope"></a><sub>[`src/layout.rs`](../../crates/x0k-folio/src/layout.rs) · `#corpus-scope`</sub>

```rust {#corpus-scope}
/// The corpus a lookup is scoped to: a root directory, and nothing else.
///
/// Deliberately **no `Default`**. A default corpus is how a scope silently
/// widens two refactors later — some caller constructs one without thinking,
/// the wrong tree gets walked, and because a walk over an absent or foreign
/// directory returns an empty set rather than an error, nothing says so. The
/// 2026-09 relayout produced exactly that failure from the weaker version of
/// this mistake (a constant that was corpus-relative while its callers held a
/// repo root), and it cost three months of a search index that indexed
/// nothing.
///
/// Construct one from a corpus root — `corpora/x0k`, `corpora/sci` — never
/// from the repository root. The genus directories are per-corpus: `decisions`
/// exists under four corpora in this tree and `wiki` under three.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CorpusScope {
    root: PathBuf,
}

impl CorpusScope {
    /// Scope lookups to the corpus rooted at `corpus_root`.
    pub fn new(corpus_root: impl Into<PathBuf>) -> Self {
        Self {
            root: corpus_root.into(),
        }
    }

    /// The corpus root this scope resolves against.
    pub fn root(&self) -> &Path {
        &self.root
    }

    /// The directory holding one genus, rooted at this corpus.
    pub fn genus_dir(&self, t: &DocType) -> PathBuf {
        self.root.join(genus_subpath(t))
    }
}
```

## The table

Corpus-relative, because that is the only grain at which it is true. Every
value below was read off the tree rather than carried forward: `publications`
is top-level and was recorded under `decisions/` by the copy this replaces,
which is a third drift the relayout did not cause and nobody had noticed.

<a name="chunk-genus-subpath"></a><sub>[`src/layout.rs`](../../crates/x0k-folio/src/layout.rs) · `#genus-subpath`</sub>

```rust {#genus-subpath}
/// The envelope the decision genera nest under, corpus-relative.
///
/// Exposed because two callers match it as a path COMPONENT rather than
/// joining it — `file_materializer` asks whether a projected path lies inside
/// the decisions envelope, and `projection` uses it as a containment check.
/// Those uses survived the relayout intact, since a path still contains
/// `decisions/<genus>` with the corpus prefix in front of it.
pub const DECISIONS_ROOT: &str = "decisions";

/// Where a genus lives inside a corpus, as a corpus-relative path.
///
/// The decision genera nest under `decisions/`; the rest are top-level.
/// No genus root is flat: decisions nest by topic, implementation by
/// subsystem, publications and manuscripts one level per work. A caller
/// takes its depth from [`walk_depth`] and resolves a single document by
/// stem through [`CorpusScope::find_folio`], never by joining a stem onto
/// this directory.
pub fn genus_subpath(t: &DocType) -> PathBuf {
    let under_decisions = |leaf: &str| PathBuf::from("decisions").join(leaf);
    match t {
        DocType::Architecture => under_decisions("architecture"),
        DocType::Design => under_decisions("design"),
        DocType::Commitment => under_decisions("commitments"),
        // Top-level, NOT under `decisions/`. The table this replaces had it
        // as `decisions/publications`, a directory that has never existed;
        // seven publications sit at `publications/<slug>/` today.
        DocType::Publication => PathBuf::from("publications"),
        DocType::Manuscript => PathBuf::from("manuscripts"),
        DocType::Wiki => PathBuf::from("wiki"),
        DocType::Implementation => PathBuf::from("implementation"),
        // Grove entity documents: the curated half of a planning entity,
        // flat and keyed by entity id. None are materialized in this tree
        // today — these are the paths a projection would write to, not
        // directories a walk will find.
        DocType::Seed => PathBuf::from("seeds"),
        DocType::Intent => PathBuf::from("intents"),
        DocType::Affordance => PathBuf::from("affordances"),
        // A genus this build has no variant for, named by a vocabulary
        // module the caller loaded. It gets the shape its neighbours have.
        DocType::Declared(name) => under_decisions(name),
    }
}

/// How deep a walk must go to see every document of a genus, counting the
/// genus directory itself as depth 1.
pub fn walk_depth(t: &DocType) -> usize {
    match t {
        // `decisions/<genus>/<topic>/<stem>.md`, plus headroom for a
        // sub-topic.
        DocType::Architecture | DocType::Design | DocType::Commitment => 3,
        // `publications/<slug>/<slug>.md`, plus headroom.
        DocType::Publication => 3,
        // Nested arbitrarily deep by subsystem / page hierarchy.
        DocType::Implementation | DocType::Wiki => 6,
        // `manuscripts/<work>/<part>.md`.
        DocType::Manuscript => 3,
        DocType::Seed | DocType::Intent | DocType::Affordance => 1,
        DocType::Declared(_) => 3,
    }
}
```

## Resolving one document by stem

A document's id carries no topic, so the topic directory has to be *found*
rather than computed. The stem is unique within a genus, so the first hit is
the answer.

<a name="chunk-find-folio"></a><sub>[`src/layout.rs`](../../crates/x0k-folio/src/layout.rs) · `#find-folio`</sub>

```rust {#find-folio}
impl CorpusScope {
    /// Resolve one document by stem — the id-to-path direction.
    ///
    /// A stem containing a path separator is refused rather than joined, so a
    /// malformed identifier cannot walk out of the genus root.
    pub fn find_folio(&self, t: &DocType, stem: &str) -> Option<PathBuf> {
        if stem.contains('/') || stem.contains('\\') {
            return None;
        }
        let dir = self.genus_dir(t);
        let wanted = format!("{stem}.md");
        WalkDir::new(&dir)
            .max_depth(walk_depth(t))
            .into_iter()
            .filter_map(Result::ok)
            .filter(|e| e.file_type().is_file())
            .map(|e| e.into_path())
            .filter(|p| !is_meta_file(p) && !is_excluded(p))
            .find(|p| p.file_name().and_then(|n| n.to_str()) == Some(wanted.as_str()))
    }
}
```

## What a walk skips

<a name="chunk-skips"></a><sub>[`src/layout.rs`](../../crates/x0k-folio/src/layout.rs) · `#skips`</sub>

```rust {#skips}
/// Per-directory meta files that are not folios and must be skipped by any
/// walker. Authored prose, not an envelope-bearing document.
///
/// Carried over verbatim from `decision_paths::META_FILES`. Deliberately
/// verbatim: an earlier draft of this module wrote the list from memory,
/// added `README.md` and dropped `GLOSSARY.md`, and would have changed which
/// files a walk parses as a side effect of moving a table. A port is not the
/// place to improve a list.
pub const META_FILES: &[&str] = &[
    "AGENTS.md",
    "CLAUDE.md",
    "GLOSSARY.md",
    "INDEX.md",
    "KERNEL.md",
];

/// Directory components a walker must never descend into. Listed verbatim;
/// [`is_excluded`] walks `path.components()` to match.
pub const EXCLUDED_PATHS: &[&str] =
    &[".git", ".jj", "target", ".0k", "node_modules", "vendor"];

pub fn is_meta_file(p: &Path) -> bool {
    p.file_name()
        .and_then(|s| s.to_str())
        .map(|name| META_FILES.contains(&name))
        .unwrap_or(false)
}

pub fn is_excluded(p: &Path) -> bool {
    p.components().any(|c| {
        matches!(
            c,
            std::path::Component::Normal(s)
                if s.to_str().is_some_and(|name| EXCLUDED_PATHS.contains(&name))
        )
    })
}
```

## Imports

<a name="chunk-imports"></a><sub>[`src/layout.rs`](../../crates/x0k-folio/src/layout.rs) · `#imports`</sub>

```rust {#imports}
use crate::colophon::DocType;
use std::path::{Path, PathBuf};
use walkdir::WalkDir;
```

## Composing the module

<a name="chunk-root"></a><sub>[`src/layout.rs`](../../crates/x0k-folio/src/layout.rs) · `#root` · assembles [imports](#chunk-imports) · [corpus-scope](#chunk-corpus-scope) · [genus-subpath](#chunk-genus-subpath) · [find-folio](#chunk-find-folio) · [skips](#chunk-skips)</sub>

```rust {#root}
//! Where a genus lives inside a corpus, and the scope a caller must name
//! to ask.
//!
//! One table. It replaced six copies on 2026-09-09, every one of which had
//! drifted through the corpus relayout and none of which failed: a walk over
//! a directory that is not there returns an empty set, not an error. See
//! `corpora/x0k/implementation/folio/layout.md` for the measurement.

<<imports>>

<<corpus-scope>>

<<genus-subpath>>

<<find-folio>>

<<skips>>
```
