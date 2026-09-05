---
x0k:
  format: folio/v1
  id: x0k:implementation/tangle/region-repo
  type: implementation
  status: draft
  summary: "The projector behind this repository: crates vendored, literate documents carried beside the code they generate, licensing applied at the boundary, and guards that refuse a projection which would leak an unpublished dependency."
  concerns: [tangle, publication, projection, repository, licensing, provenance]
  tangle:
    crate: x0k-tangle
    root: src/region_repo.rs
  edges:
    implements:
      - x0k:design/publish-a-region-as-a-repository
    cites:
      - x0k:design/author-and-publish-the-same-surface
      - x0k:implementation/tangle/publishing
      - x0k:implementation/tangle/receiving
      - x0k:implementation/tangle/region-project
      - x0k:implementation/tangle/pipeline
      - x0k:implementation/folio/colophon
---

# The repository projector: a region, made buildable

A publication names a region of the graph. The reader-site projector
([`region-project.md`](region-project.md)) turns that region into HTML;
this chapter turns it into a **git repository** that anyone can clone
and build with plain `cargo` — the published crates' source, the
literate documents that produce the generated half of it, a workspace
manifest, the license bodies, a README tangled from the publication
document itself, and a CI script that proves the committed code is
what the documents tangle to. The design commitment
it implements is one sentence: *the repository is the region, made
buildable*. Nothing in the output is tended beside the corpus; every
run regenerates the whole tree from the publication doc and the crates
it names, so the public repository cannot drift from the region it came
from.

Four decisions shape everything below. The projection is **guarded**:
before a byte is written, every published crate must be `public`-tier
and every path dependency must resolve inside the publication or be
severed behind a feature flag — the disclosure boundary from the design,
enforced as a property of the output. The projection is **re-entrant**:
an output directory that already holds a projection is projected *into*,
keeping its history and the paths the publication declares as overlay,
so repeated publishes append to one public history. And **licensing is
part of the act of publishing**: the source tree stays proprietary, the
publication doc's `license:` field says what the projection is released
under, and the projector refuses to invent one. And **the README is an
authored artifact, not a template**: the publication document carries
a `tangle:` block whose output is the projected repository's
`README.md`, and the projector tangles it with the projection
directory as the workspace — the public face of the repository is
written in the corpus, per publication, and carries the same
`@generated` header as every other projection. Its **contents page is
not authored**: the chunk carries a marker naming the concepts the book
is organized into, and the projector writes every shipped document there
under the group that claims it, each described by the `summary:` its own
envelope declares. Membership is exhaustive, so a chapter added to the
corpus cannot go unlisted in the public face. Nor are its **affordance
figures** authored: a second marker asks for one figure per affordance
the publication publishes, drawn from the extracted declaration and the
signifiers that point at it, so the picture cannot show a face nobody
declared or a module the repository does not ship.

The carried example is the publication that publishes this very crate,
`decisions/publications/x0k-folio.md`: three crates
(`x0k-folio`, `x0k-syntax`, `x0k-tangle`), three publish-excluded
parts (`x0k-types` and `x0k-surface-build`, severed behind features;
`x0k-ontology`, whose vocabulary is the private ontology and which
nothing shipped uses), an overlay of `CONTRIBUTING.md`, `license: MIT`
with a `copyright:` holder, and a `## README` section holding the
markdown chunk its `tangle: root: README.md` projects. It also
carries the projector's one genuinely circular constraint: the tangler
that regenerates the bundle's code *is a crate in the bundle*, so a
fresh clone has no `x0k-tangle` until it builds one. Committing both the
literate `.md` sources and their tangled `@generated` output is what
breaks that bootstrap circle, and the projected `tools/ci` is what keeps
the two from drifting apart afterwards.

```rust {#module-doc}
//! Repository projection — the git-repo Surface for a Publication.
//!
//! Sibling to [`region_project`](crate::region_project) (which projects a region
//! to an HTML reader site). This backend projects a publication region into a
//! standalone, buildable Cargo repository: the published crates' source (both
//! `@generated` and hand-written), the literate documents that back the
//! generated code, a standalone workspace manifest, the license bodies for
//! the publication's declared license (the `license:` field of the
//! publication doc is authoritative — licensing is part of the act of
//! publishing; the source tree stays proprietary), a README tangled from the
//! publication doc's own `tangle:` block (the projection dir is the tangle
//! workspace, so `root: README.md` lands at the repo root; chunks routed to
//! declared `overlay:` paths seed those files once, and the README's
//! `<!-- x0k:contents -->` marker is replaced by the generated contents
//! page, grouped by the concepts that marker names, and its
//! `<!-- x0k:affordances -->` marker by one figure per affordance declaration
//! the publication publishes, drawn from the extracted record into
//! `affordances/`), a committed
//! `Cargo.lock`, and a forge-agnostic `tools/ci` that re-tangles and diffs
//! against the committed generated files. It realizes
//! `x0k:design/publish-a-region-as-a-repository`.
//!
//! Membership is **crate-granular**: the publication's `publishes` edges name
//! `x0k:software-module/<crate>` members. `excludes` names publish-flagged parts
//! held back (the publishing-flag severances); the projector enforces that an
//! excluded crate is reachable only through an optional, feature-gated
//! dependency — a hard dependency on an excluded crate fails the projection.
//! `excludes` may also hold back a single literate document
//! (`x0k:implementation/<area>/<stem>`): a feature can sever a dependency but
//! not keep a feature-gated module's source out of the tarball, so the
//! chapters behind a default-off feature are excluded by id and their
//! `@generated` outputs are dropped with them.
//! A vendored `@generated` file whose source document is outside the
//! projection's literate set is dropped and recorded, never shipped.
//!
//! `publishes` may also name **vocabulary modules** (`x0k:ontology-module/<name>`,
//! `x0k:architecture/ontology-modules` §6): the projector carries exactly
//! those `ontology/modules/<name>.ttl` files, refuses a selection whose
//! `owl:imports` closure is not inside it, refuses a module file carrying
//! an instance fact, and stamps each shipped module's `owl:versionIRI`
//! with the publication's version. When the vocabulary crate itself is
//! published the modules travel inside its package
//! (`x0k-ontology/ontology/modules/`) so `cargo package` can build the
//! tarball; otherwise they stay at the projection root. One copy either
//! way; `modules_rel_dir` below decides which, and `PROVENANCE.json`
//! records the answer.
//!
//! `publishes` may also name a **decision document**, whole or by section
//! (`x0k:design/<stem>#<heading-path>`), which is how an affordance's
//! declaration reaches the audience as data. What a publication affords is
//! derived from the modules it publishes, never claimed, so the projector
//! holds the closure rule of
//! `x0k:architecture/publication-is-the-shipping-unit` §7: every module a
//! published declaration names under `enabledBy` is published, or excluded —
//! a declaration naming a module the audience will not have is refused.
//!
//! The projector emits a `PROVENANCE.json` (the inbound-contribution seam) and
//! refuses to disclose anything above `public` access tier.
```

```rust {#uses}
use anyhow::{anyhow, bail, Context, Result};
use std::collections::{BTreeMap, BTreeSet, HashSet};
use std::path::{Path, PathBuf};

use x0k_folio::colophon::{parse_envelope, split_frontmatter, Colophon, DocType};
use x0k_folio::transclusion::extract_section;
use x0k_folio::{EntityId, InlineEntity};

use crate::pipeline::PipelineRegistry;
use crate::pipeline_runner::{tangle_document, TangleSidecar};
```

Members are named by `x0k:software-module/<crate>` URIs for crates and
`x0k:ontology-module/<name>` URIs for vocabulary modules; a module URI
resolves to the module IRI `https://0k.computer/ontology/<name>` and the
tree file `ontology/modules/<name>.ttl`. `excludes` alone may also name
`x0k:implementation/<area>/<stem>` — a single literate document held
back, which is the third grain the severances need. `publishes` alone
may also name a *document* under `decisions/`, whole or by section —
see [Documents the publication names](#documents-the-publication-names). The standalone workspace needs
concrete versions for the dependency keys the monorepo inherits from
its root manifest. The resolutions are a fixed table mirroring the
monorepo root; an inherited key with no entry here surfaces as a build
failure in the projection's own CI, not a silent substitution.

```rust {#constants}
const SOFTWARE_MODULE_PREFIX: &str = "x0k:software-module/";
const ONTOLOGY_MODULE_PREFIX: &str = "x0k:ontology-module/";
/// A literate document, by the `id:` its own envelope declares. Only
/// `excludes` may name one — see [`member_names`].
const IMPLEMENTATION_DOC_PREFIX: &str = "x0k:implementation/";
/// A vocabulary module's IRI is this base plus its name; its file in the
/// tree is `ontology/modules/<name>.ttl` (`x0k:architecture/ontology-modules` §4).
const MODULE_IRI_BASE: &str = "https://0k.computer/ontology/";
const MODULES_DIR: &str = "ontology/modules";

/// A module's shapes travel with it (`x0k:architecture/vocabulary-shapes`
/// §5), so this directory is projected alongside `MODULES_DIR` for exactly
/// the selected modules and is never selected from separately.
const SHAPES_DIR: &str = "ontology/shapes";
/// Where the corpus keeps decision documents. A named document is looked
/// up under here by its class and stem; nothing enumerates the directory.
const DECISIONS_ROOT: &str = "decisions";
/// The crate that compiles the module files. When a publication ships it, the
/// modules travel *inside* the package — see [`modules_rel_dir`].
const ONTOLOGY_CRATE: &str = "x0k-ontology";
/// The predicates the module-file reader looks at, as N-Triples tokens.
const RDF_TYPE: &str = "<http://www.w3.org/1999/02/22-rdf-syntax-ns#type>";
const RDFS_COMMENT: &str = "<http://www.w3.org/2000/01/rdf-schema#comment>";
const OWL_ONTOLOGY: &str = "<http://www.w3.org/2002/07/owl#Ontology>";
const OWL_IMPORTS: &str = "<http://www.w3.org/2002/07/owl#imports>";
const OWL_VERSION_IRI: &str = "<http://www.w3.org/2002/07/owl#versionIRI>";
/// Any triple under this namespace is an instance fact, and a module file
/// never carries one (ADR §3).
const INSTANCE_NAMESPACE: &str = "<https://0k.computer/instance/";
/// Toolchain floor the standalone workspace declares (`workspace.package.rust-version`),
/// inherited by every published crate. Set by the newest std item the
/// published code uses: `File::lock` (the tangle lock in
/// [`crate::pipeline_runner`]), stable since 1.89. There is no portable
/// pre-1.89 std file lock, and buying a lower floor with a `fs2`-style
/// dependency in a published crate is the worse trade — so the floor moves
/// to meet the code rather than the claim staying convenient.
///
/// Not a comment anybody has to keep true by hand: `tools/ci` runs clippy
/// with warnings denied, and clippy's `incompatible_msrv` compares every
/// item the published crates touch against this value. The floor is the one
/// claim in the projection with a machine check behind it.
const RUST_VERSION: &str = "1.89";
/// Toolchain the projection pins for CI and for a contributor running
/// `tools/ci` locally (`rust-toolchain.toml`). This is the *ceiling* — the
/// version the tree is known to build and test green on — and it is a
/// different claim from [`RUST_VERSION`], which is the floor.
const PINNED_TOOLCHAIN: &str = "1.95.0";
/// Trailing comment on a severed feature's (now empty) list.
const SEVERED_FEATURE_NOTE: &str =
    "# severed in this publication: its dependency is not published; enabling it does not build";
/// Workspace-inherited dependency keys the projector knows how to resolve into
/// the standalone root `[workspace.dependencies]`. Values mirror the monorepo
/// root `Cargo.toml`; keep in sync if a published crate adopts a new inherited
/// key (the closure check flags an inherited key with no resolution).
///
/// This is a *table of resolutions*, not the list emitted: a projection
/// declares only the keys its own crates actually inherit
/// ([`emit_workspace_manifest`]). An entry nobody references is a pin a
/// reader would take for a real dependency, and the last one that shipped
/// (`thiserror = "1.0"`, referenced by no published crate) advertised a
/// major version the lockfile did not even contain.
const RESOLVED_WORKSPACE_DEPS: &[(&str, &str)] = &[
    ("anyhow", "\"1.0\""),
    ("clap", "{ version = \"4\", features = [\"derive\"] }"),
    ("serde", "{ version = \"1.0\", features = [\"derive\"] }"),
    ("serde_json", "\"1.0\""),
    ("thiserror", "\"1.0\""),
    ("tracing", "\"0.1\""),
];
```

## Contract

The options are deliberately few. `license` is `None` by default so the
publication doc is the carrier; `allow_dirty` is the one escape hatch
past the guards, for inspecting a projection that is not yet clean; the
`.github/workflows/` wrappers are optional because the CI contract is
two forge-agnostic scripts, and a caller targeting another forge skips
the wrappers entirely.

```rust {#options}
/// Options controlling a repository projection.
#[derive(Debug, Clone)]
pub struct RepoProjectOptions {
    /// Explicit SPDX license override. `None` (the default) means the license
    /// comes from the publication doc's `license:` envelope field — the
    /// manifest is authoritative, per "licensing is part of the act of
    /// publishing". `Some` is a deliberate caller override. The projector
    /// never invents a license and never silently relicenses: with neither
    /// carrier present the projection refuses.
    pub license: Option<String>,
    /// `git init` + initial commit in the output dir (outside the monorepo).
    pub git_init: bool,
    /// Bypass the leak / closure / publish-exclusion guards. Escape hatch only.
    pub allow_dirty: bool,
    /// Emit the `.github/workflows/` thin wrappers. The forge-agnostic
    /// contract is `tools/ci` + `tools/x0k-guard-generated`; the workflows
    /// only call those scripts, and callers targeting another forge skip
    /// them entirely.
    pub emit_github: bool,
}

impl Default for RepoProjectOptions {
    fn default() -> Self {
        Self {
            license: None,
            git_init: true,
            allow_dirty: false,
            emit_github: true,
        }
    }
}
```

The report is what the CLI prints and what
[`publishing.md`](publishing.md) carries forward. Two revisions travel
in it: `corpus_rev` is the jj change id of the working copy — stable
across amendments, but naming a *mutable* change — and `corpus_commit`
is the immutable git commit id of the described parent (`@-`). The
receiver ([`receiving.md`](receiving.md)) reproduces its reference
projection from the second, precisely because the first can be amended
after the fact. When the run re-projects over an earlier projection,
the previous pair is read from that projection's `PROVENANCE.json` so
the new commit message can name the link it follows.

```rust {#report}
/// Summary of a repository-projection run.
#[derive(Debug, Clone, Default)]
pub struct RepoProjectReport {
    pub output_dir: PathBuf,
    pub crates: Vec<String>,
    pub excluded: Vec<String>,
    pub literate_docs: Vec<PathBuf>,
    pub leak_violations: Vec<String>,
    pub closure_violations: Vec<String>,
    /// A git commit was made this run. False when `git_init` is off, and
    /// false when re-projecting into an existing repo changed nothing (no
    /// empty commits — history stays navigable by real change).
    pub committed: bool,
    /// The SPDX expression actually applied, and where it came from.
    pub license: String,
    pub license_source: LicenseSource,
    /// The corpus revision this projection was taken from (recorded in
    /// `PROVENANCE.json` and the commit message): the jj change id, which
    /// is stable across amendments but names a mutable commit.
    pub corpus_rev: String,
    /// The immutable git commit id of the described corpus commit the
    /// projection was taken from (`@-` in a jj workspace, `HEAD` in a git
    /// checkout). A receiver reproduces the reference projection from
    /// this, since a change id's content can be amended.
    pub corpus_commit: String,
    /// The `corpus_rev` of the projection previously in `output_dir`
    /// (from its `PROVENANCE.json`), when re-projecting over one.
    pub previous_corpus_rev: Option<String>,
    /// The `corpus_commit` of the previous projection, when it recorded one.
    pub previous_corpus_commit: Option<String>,
    /// Resolved `overlay:` paths — projected-repo-relative paths the
    /// projector preserves exactly as found instead of regenerating.
    pub overlay: Vec<String>,
    /// Projected-repo-relative paths of `@generated` files that arrived
    /// with a vendored crate but whose source document is not in this
    /// projection's literate set — dropped from the tree (their document
    /// is outside the region) and recorded in `PROVENANCE.json`.
    pub dropped_generated: Vec<String>,
    /// The vocabulary modules the publication ships
    /// (`x0k:ontology-module/<name>` under `publishes`), in declaration
    /// order; each is written to `<modules_dir>/<name>.ttl`.
    pub modules: Vec<String>,
    /// The version stamped into every shipped module's `owl:versionIRI`
    /// and where it came from; `None` when no module ships.
    pub module_version: Option<(String, ModuleVersionSource)>,
    /// Literate documents held back by `excludes`
    /// (`x0k:implementation/<area>/<stem>`), in sorted order. Their
    /// `@generated` outputs are dropped with them, and appear in
    /// `dropped_generated`.
    pub excluded_docs: Vec<String>,
    /// Documents the publication named under `publishes`, as
    /// reference → projection-relative path. A reference carrying a
    /// `#anchor` projected one section; one without projected the whole
    /// document.
    pub documents: BTreeMap<String, String>,
    /// The projection-relative directory the shipped modules were written
    /// to — `x0k-ontology/ontology/modules` when the vocabulary crate is
    /// published, `ontology/modules` when it is not (`modules_rel_dir`).
    /// `None` when no module ships.
    pub modules_dir: Option<String>,
    /// The affordance figures drawn where the README's
    /// `<!-- x0k:affordances -->` marker stood: affordance id → the
    /// projection-relative path of its light figure, the dark twin beside
    /// it as `-dark.svg`. Empty when the README carries no marker. Not in
    /// `PROVENANCE.json`'s `path_map`: a figure has no corpus source to
    /// route an edit back to.
    pub figures: BTreeMap<String, String>,
}
```

A module's `owl:versionIRI` is the version of the publication that
ships it (ADR §6), and a publication has no version of its own yet —
the projector reads each crate's manifest `version` and never one for
the whole. The choice is made here: the **`entryPoint` crate's
version** when the publication declares one (x0k-folio's is
`x0k-tangle`, so its modules carry the tangler's version, which is the
version a consumer of the bundle already tracks), else the **corpus
revision** the provenance already records. Which one was used is
recorded in `PROVENANCE.json` beside the version.

```rust {#module-version-source}
/// Where the version stamped into shipped modules' `owl:versionIRI` came from.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ModuleVersionSource {
    /// The manifest `version` of the crate the publication's `entryPoint`
    /// names.
    EntryPointCrate,
    /// The corpus revision (`corpus_rev`) — the publication declares no
    /// entry point.
    CorpusRev,
}

impl ModuleVersionSource {
    pub fn as_str(self) -> &'static str {
        match self {
            ModuleVersionSource::EntryPointCrate => "entry-point-crate",
            ModuleVersionSource::CorpusRev => "corpus-rev",
        }
    }
}
```

```rust {#license-source}
/// Where the applied license expression came from.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum LicenseSource {
    /// The publication doc's `license:` envelope field (the default path).
    #[default]
    PublicationDoc,
    /// An explicit caller/CLI override.
    Override,
}
```

## The projection, as an outline

The entry point reads as the whole algorithm: parse the publication,
settle the license and the overlay, open the report, run the guards,
select and check the vocabulary modules, discover the literate
documents, resolve the documents the publication named and check
that what they declare is closed over the crate set,
prepare the output directory, vendor the crates and the
modules, copy the documents, emit the scaffolding, put the overlay
back, commit. The order matters in two places — every guard runs
before any write (the module checks included), and the literate set is
known before a crate is vendored, because vendoring judges each
`@generated` file against it — and the fragments below hang off this
outline in that order.

```rust {#project-publication-repo}
/// Project the publication decision doc at `region_doc` into a standalone repo
/// under `output_dir`, resolving crate sources against `workspace`.
pub fn project_publication_repo(
    region_doc: &Path,
    output_dir: &Path,
    workspace: &Path,
    opts: &RepoProjectOptions,
) -> Result<RepoProjectReport> {
    <<read-publication>>

    <<resolve-license>>

    <<resolve-overlay>>

    <<open-report>>

    <<gather-manifests-and-guard>>
    <<refuse-on-violations>>

    <<select-modules>>

    <<discover-literate-docs>>

    <<select-documents>>

    <<prepare-output-dir>>

    <<vendor-crates>>

    <<vendor-modules>>

    <<copy-docs-and-scaffold>>

    <<restore-overlay-and-commit>>
}
```

A publication doc is a folio/v1 document whose `type` is `publication`
(the envelope is parsed by the shared colophon parser,
[`colophon.md`](../folio/colophon.md)). Membership is crate-granular:
`publishes` is the in-set, `excludes` the severances, and an in-set
with no crate is a refusal rather than an empty repository — a
publication that ships only vocabulary modules is not a buildable
repository, and this projector makes nothing else. Modules ride along
in the same edge; nothing severs a module, so `excludes` may not name
one.

`excludes` has a third grain, finer than a crate. A feature can sever a
crate's *dependency*, but it cannot keep the source of a
feature-gated module out of the tarball: the chapters behind a
default-off feature still name their crate in `tangle.crate`, so
document discovery selects them and their `@generated` output rides
along as dead code a public reader is invited to read. The vocabulary
crate is exactly that case — eight product chapters behind a
default-off `product` feature. So a publication may exclude a
**document** by the `id:` its own envelope declares, and the rest
follows from a rule the projector already had: a `@generated` file
whose source document is outside the literate set is dropped. Excluding
the document is therefore the whole act; nothing separate deletes the
code.

```rust {#read-publication}
let content = std::fs::read_to_string(region_doc)
    .with_context(|| format!("reading publication doc {}", region_doc.display()))?;
let (env, _body) =
    parse_envelope(&content).map_err(|e| anyhow!("not a folio/v1 document: {e:?}"))?;
if env.doc_type != DocType::Publication {
    bail!(
        "document is not a publication (type is `{}`)",
        env.doc_type.as_str()
    );
}

// `docs` is always empty here: `member_names` refuses a *literate*
// document URI outside `excludes`. `documents` is the other grain —
// decision documents this publication names, whole or by section.
let Members { crates, modules, docs: _, documents } =
    member_names(env.edges.get("publishes"), "publishes")?;
if crates.is_empty() {
    bail!("publication has an empty `publishes` membership (no crate)");
}
let excluded = member_names(env.edges.get("excludes"), "excludes")?;
if let Some(m) = excluded.modules.first() {
    bail!(
        "`excludes` names vocabulary module `{m}` — nothing severs a module; \
         leave it out of `publishes` instead"
    );
}
let excluded_docs: BTreeSet<String> = excluded.docs.into_iter().collect();
let excluded: BTreeSet<String> = excluded.crates.into_iter().collect();
let published: BTreeSet<String> = crates.iter().cloned().collect();
```

The license has exactly two carriers, and neither is a default. The
publication doc's `license:` field is authoritative; a caller's explicit
override is a deliberate act recorded as such in the provenance. With
neither present the projection refuses, naming the doc that should
declare it — publishing the same region under a different license is
authoring a different publication, not passing a different flag. The
expression is resolved to its license bodies here, before any guard
or write, so an identifier the projector has no body for refuses the
projection up front rather than leaving a repository with a `license`
field and no text behind it.

The MIT text opens with a `Copyright (c) <year> <holder>` line, and
the holder is a fact the projector cannot invent: the publication's
`copyright:` envelope field names it, and an MIT publication without
one refuses. The year is the projection year — the notice dates the
act of publishing, which is what the projector performs.

```rust {#resolve-license}
// Licensing is part of the act of publishing: the publication doc's
// `license:` field is authoritative, and an explicit caller override is
// the only alternative. There is no silent default.
let (license, license_source) = match &opts.license {
    Some(l) => (l.clone(), LicenseSource::Override),
    None => (
        envelope_scalar(&content, "license").ok_or_else(|| {
            anyhow!(
                "publication `{}` carries no `license:` in its envelope and no explicit \
                 license override was given — declare the license in the publication doc \
                 (or pass --license to override deliberately)",
                env.id
            )
        })?,
        LicenseSource::PublicationDoc,
    ),
};
let copyright = envelope_scalar(&content, "copyright");
let license_bodies = license_files(&license, copyright.as_deref(), current_year())?;
```

Deliberate divergence has one channel. The publication's `overlay:`
lists projected-repo-relative paths the *public side* owns — community
files such as `CONTRIBUTING.md`, a forge's issue templates — and the
projector preserves them exactly as found instead of regenerating them.
Everything not listed is authoritative from the corpus and is
regenerated wholesale.

```rust {#resolve-overlay}
// Deliberate divergence: `overlay:` names projected-repo-relative paths
// the public side owns (community files such as CONTRIBUTING.md). They
// are preserved exactly as found on re-projection; everything else is
// authoritative from the corpus.
let overlay = overlay_paths(&content)?;
```

The report is opened before the guards run so that violations have
somewhere to accumulate; the two revisions and the previous
projection's pair are resolved here too. The crates.io keys
(`repository:`, `keywords:`) are optional envelope keys the shared
parser tolerates but does not own, so this module reads them itself.

```rust {#open-report}
let mut report = RepoProjectReport {
    output_dir: output_dir.to_path_buf(),
    crates: crates.clone(),
    excluded: excluded.iter().cloned().collect(),
    excluded_docs: excluded_docs.iter().cloned().collect(),
    license: license.clone(),
    license_source,
    corpus_rev: current_corpus_rev(workspace),
    corpus_commit: current_corpus_commit(workspace),
    previous_corpus_rev: previous_provenance_field(output_dir, "corpus_rev"),
    previous_corpus_commit: previous_provenance_field(output_dir, "corpus_commit"),
    overlay: overlay.clone(),
    modules: modules.clone(),
    ..Default::default()
};

// crates.io metadata sourced from the publication doc (optional keys the
// shared envelope parser tolerates and this reader owns).
let crates_io = CratesIoMeta {
    repository: envelope_scalar(&content, "repository"),
    keywords: envelope_string_list(&content, "keywords"),
};
```

### The guards

One pass over the published crates' manifests collects what the
vendoring step needs later (each crate's version, its source-tree
license) and applies the two guards from the design. The **leak guard**
is the disclosure boundary: a crate whose `[package.metadata.x0k]
access` is anything but `public` cannot be projected. The **closure
guard** is self-containment: each `path` dependency must be in the
publication, or be publish-excluded *and* optional (a feature-gated
severance the manifest rewrite can cut), or it is an escape into the
monorepo. `workspace-hack` is the one exemption — a build-performance
artifact the vendoring step strips, never shipped. The same pass reads
each crate's `edition`, because the standalone workspace declares one
edition under `[workspace.package]` that every crate inherits.

```rust {#gather-manifests-and-guard}
let mut versions: BTreeMap<String, String> = BTreeMap::new();
let mut source_licenses: BTreeMap<String, String> = BTreeMap::new();
let mut editions: BTreeSet<String> = BTreeSet::new();
for name in &crates {
    let manifest = read_manifest(workspace, name)?;
    if let Some(v) = manifest_package_str(&manifest, "version") {
        versions.insert(name.clone(), v);
    }
    if let Some(e) = manifest_package_str(&manifest, "edition") {
        editions.insert(e);
    }
    source_licenses.insert(
        name.clone(),
        manifest_package_str(&manifest, "license").unwrap_or_else(|| "(none)".to_string()),
    );
    // Leak guard: every published crate must be public-tier.
    let access = manifest_access(&manifest);
    if access != "public" {
        report.leak_violations.push(format!(
            "crate `{name}` is access = \"{access}\", not public"
        ));
    }
    // Closure + publish-exclusion: examine each path dependency.
    for (dep, optional, target) in path_deps(&manifest) {
        // `workspace-hack` is a monorepo build-perf artifact stripped by the
        // vendoring step (never shipped) — treat it as implicitly excluded.
        if target == "workspace-hack" {
            continue;
        }
        if published.contains(&target) {
            continue; // in-set
        }
        if excluded.contains(&target) {
            if !optional {
                report.closure_violations.push(format!(
                    "crate `{name}` depends on publish-excluded `{target}` non-optionally (dep `{dep}`) — gate it behind a feature first"
                ));
            }
            continue; // severable exclusion
        }
        report.closure_violations.push(format!(
            "crate `{name}` has escaping path dep `{dep}` → `{target}` (not published, not excluded)"
        ));
    }
}
```

A violation refuses the whole projection with every finding listed;
`allow_dirty` downgrades the refusal to the warnings the CLI prints. No
partial tree is ever written under a guard failure, because the guards
run before the output directory is even created.

```rust {#refuse-on-violations}
if !opts.allow_dirty
    && (!report.leak_violations.is_empty() || !report.closure_violations.is_empty())
{
    let mut msg = String::from("repository projection refused — guard violations:\n");
    for v in report
        .leak_violations
        .iter()
        .chain(report.closure_violations.iter())
    {
        msg.push_str("  - ");
        msg.push_str(v);
        msg.push('\n');
    }
    bail!(msg);
}
```

### The vocabulary modules

A shipped module is read from the tree, checked, and held until the
write phase. Three refusals stand between the selection and the
output, none of them reachable by `allow_dirty` — the instance refusal
is a disclosure guard with no inspection use, and a broken import
closure has no partial tree worth looking at. First, **closure**: every
module a shipped module imports must be shipped too. ADR §6 also
admits an import "resolved by version to a module another publication
ships"; that cross-publication resolution is a follow-up this unit
names and does not build, so today an import outside the selection
refuses, naming the module and the import it is missing. Second,
**instances**: a line under `https://0k.computer/instance/` refuses the
module — the materializer never writes one (ADR §3), and this is the
projector's last line of defence should it slip. Third, a module that
already carries an `owl:versionIRI` refuses: the tree never holds one;
the projector stamps it (ADR §4).

The version is settled here too, once for every shipped module, and
only when a module ships — a publication without modules records no
module version. Finally, the empty case: `x0k-ontology` published with
no module selected would be a crate whose build script finds no
modules directory in the projection, so the projector refuses up front
and says what to add.

```rust {#select-modules}
let mut vocab_modules: Vec<VocabModule> = Vec::new();
for name in &modules {
    vocab_modules.push(read_vocab_module(workspace, name)?);
}
let selected: BTreeSet<&str> = modules.iter().map(String::as_str).collect();
for m in &vocab_modules {
    for import in &m.imports {
        if !selected.contains(import.as_str()) {
            bail!(
                "vocabulary module `{}` imports `{import}`, which the publication does not \
                 ship — add `x0k:ontology-module/{import}` under `publishes` (resolving an \
                 import to a module another publication ships is not supported yet)",
                m.name
            );
        }
    }
}
if published.contains("x0k-ontology") && modules.is_empty() {
    bail!(
        "x0k-ontology needs at least one vocabulary module; name it under `publishes` \
         as `x0k:ontology-module/<name>`"
    );
}
report.module_version = if modules.is_empty() {
    None
} else {
    let entry = member_names(env.edges.get("entryPoint"), "entryPoint")?;
    Some(match entry.crates.first() {
        Some(c) => {
            let v = versions.get(c).ok_or_else(|| {
                anyhow!(
                    "`entryPoint` crate `{c}` is not a published crate with a manifest \
                     `version`, so no version can be stamped into the shipped modules"
                )
            })?;
            (v.clone(), ModuleVersionSource::EntryPointCrate)
        }
        None if !report.corpus_rev.is_empty() => {
            (report.corpus_rev.clone(), ModuleVersionSource::CorpusRev)
        }
        None => bail!(
            "the publication declares no `entryPoint` and the workspace has no corpus \
             revision — nothing to stamp into the shipped modules' owl:versionIRI"
        ),
    })
};
```

### The literate set

Which documents travel with the crates is settled before anything is
written, because two later steps consult the answer: vendoring drops a
`@generated` file whose source is not in the set, and the document
copy is the set itself. A workspace with more than one edition among
its published crates is refused here too — the standalone workspace
declares one — and a crate that declares none inherits 2021.

```rust {#discover-literate-docs}
let literate = discover_literate_docs(workspace, &published, &excluded_docs)?;
let literate_set: BTreeSet<String> = literate
    .iter()
    .map(|d| d.rel.to_string_lossy().to_string())
    .collect();
let edition = match editions.len() {
    0 => "2021".to_string(),
    1 => editions.iter().next().cloned().unwrap_or_default(),
    _ => bail!(
        "published crates disagree on `edition` ({:?}) — the standalone workspace declares one",
        editions
    ),
};
```

### The named documents

The decision documents the publication named are resolved next, and for
the same reason the literate set is settled early: this step refuses —
on an id that names no document, on an anchor that heads no section —
and a refusal must land before the output directory is touched.

```rust {#select-documents}
let projected_docs = project_named_documents(workspace, &documents)?;
affordance_closure(&projected_docs, &published, &excluded)?;
let affordances = affordance_records(&projected_docs, workspace, &literate, &published)?;
```

The records the README's affordance figures are drawn from are read
here too, while the named documents and the literate set are both in
hand — the declarations from the former, the signifiers from both
(§ "Each affordance, as a figure"). Nothing is drawn yet; whether the
README asked for figures is known only once it is tangled.

### Projecting into an existing repository

The obvious move is to wipe the output directory and start over. That
would throw away the public repository's history on every publish, and
with it the base an outside contribution needs. Instead a prior
projection — recognized by its `.git` or its `PROVENANCE.json` — is
projected *into*: the overlay paths are moved aside, the regenerated
region is cleared so nothing stale survives, and after the write the
overlay is restored over whatever the projector produced. `.git` and
`target/` are the only things that persist through the clear.

```rust {#prepare-output-dir}
// A prior projection (a `.git`, or a PROVENANCE.json) is projected INTO,
// not beside: the overlay paths are stashed, the regenerated region is
// cleared so nothing stale survives, and the overlay is restored after
// the write. History (`.git`) and build state (`target/`) are kept.
std::fs::create_dir_all(output_dir)
    .with_context(|| format!("creating output dir {}", output_dir.display()))?;
let existing_repo = output_dir.join(".git").exists();
let prior_projection = existing_repo || output_dir.join("PROVENANCE.json").is_file();
let stash = if prior_projection {
    let stash = stash_overlay(output_dir, &overlay)?;
    clear_regenerated_region(output_dir)?;
    Some(stash)
} else {
    None
};
```

Each published crate is copied whole, stripped of any `@generated`
file the region does not own, and its manifest rewritten (the
mechanics are below).

```rust {#vendor-crates}
let vendor_ctx = VendorCtx {
    license: &license,
    published: &published,
    excluded: &excluded,
    versions: &versions,
    crates_io: &crates_io,
    literate_set: &literate_set,
};
for name in &crates {
    let dropped = vendor_crate(workspace, output_dir, name, &vendor_ctx)
        .with_context(|| format!("vendoring crate `{name}`"))?;
    report.dropped_generated.extend(dropped);
}
```

Where the shipped modules land depends on whether the crate that reads
them is published, and the deciding fact is `cargo package`. A crate's
package tarball is the crate directory and nothing above it, so a build
script that reads `../ontology/modules` cannot build from one: every
crates.io path — including this pipeline's own per-crate rehearsal —
refuses with `read module directory …/package/ontology/modules: No such
file or directory`. So when `x0k-ontology` is among the published
crates the modules are written *inside* it, at
`x0k-ontology/ontology/modules/<name>.ttl`, which its build script
prefers over the repository-root copy for exactly this reason; when it
is not published the root layout stays as the tree has it. **One copy
either way** — two spellings of one vocabulary in a single repository
is a drift hazard, and the projector never writes both. Which layout a
given projection used is recorded in `PROVENANCE.json`
(`modules_dir`), so the record says where to look rather than leaving a
reader to guess.

```rust {#modules-rel-dir}
/// The projection-relative directory the shipped `*.ttl` module files go in.
///
/// `ontology/modules` at the projection root when the vocabulary crate is
/// not published — the layout the corpus tree has, and the one
/// `x0k-ontology`'s build script falls back to one level up. When the
/// crate *is* published the modules are its build-script input and must
/// travel inside the package or `cargo package` cannot verify the tarball,
/// so they go to `x0k-ontology/ontology/modules`, which the build script
/// prefers. Never both: one vocabulary, one copy.
/// The shapes directory beside [`modules_rel_dir`]'s answer.
fn shapes_rel_dir(crates: &[String]) -> PathBuf {
    if crates.iter().any(|c| c == ONTOLOGY_CRATE) {
        Path::new(ONTOLOGY_CRATE).join(SHAPES_DIR)
    } else {
        PathBuf::from(SHAPES_DIR)
    }
}

fn modules_rel_dir(crates: &[String]) -> PathBuf {
    if crates.iter().any(|c| c == ONTOLOGY_CRATE) {
        Path::new(ONTOLOGY_CRATE).join(MODULES_DIR)
    } else {
        PathBuf::from(MODULES_DIR)
    }
}
```

Each file is the tree's bytes with one line added: the `owl:versionIRI`
stamped at its sorted position among the triples, so the shipped file is
still the sorted N-Triples set the module contract promises —
`x0k-ontology`'s round-trip test re-renders the file from its facts,
sorted, and compares bytes, and a stamp placed anywhere else fails it
(found on the first `[core, document]` projection, 2026-09-02).

```rust {#vendor-modules}
let modules_rel = modules_rel_dir(&crates);
if let Some((version, _)) = &report.module_version {
    let modules_dir = output_dir.join(&modules_rel);
    std::fs::create_dir_all(&modules_dir)?;
    let shapes_dir = output_dir.join(shapes_rel_dir(&crates));
    for m in &vocab_modules {
        let path = modules_dir.join(format!("{}.ttl", m.name));
        std::fs::write(&path, m.stamped(version))
            .with_context(|| format!("writing vocabulary module {}", path.display()))?;
        tracing::info!(module = %m.name, version = %version, "region_repo.module.written");
        let Some(shapes) = &m.shapes else { continue };
        std::fs::create_dir_all(&shapes_dir)?;
        let path = shapes_dir.join(format!("{}.ttl", m.name));
        std::fs::write(&path, shapes)
            .with_context(|| format!("writing vocabulary shapes {}", path.display()))?;
        tracing::info!(module = %m.name, "region_repo.shapes.written");
    }
    report.modules_dir = Some(modules_rel.to_string_lossy().to_string());
}
```

The literate documents that back the published crates come next — they
are what make the generated half of the source honest — and then the
scaffolding: the workspace manifest, the license bodies (at the root
and inside each crate, so a `cargo package` tarball carries its
terms), the CI scripts, a `.gitignore` (build state and direnv's shell
state, never the lockfile), the lockfile, `PROVENANCE.json`
(the seam the receiver reads), and last the README and any overlay
seeds, tangled from the publication doc into a projection that already
names its provenance — the dispatcher admits a publication document
only into a root carrying `PROVENANCE.json`
([`dispatcher.md`](dispatcher.md) § "tangle_document"), so the order
here is the guard being satisfied, not a coincidence. Last of all, if
the README asked for them, the affordance figures: one themed pair per
published declaration under `affordances/`, standing where the
README's marker was.

The lockfile is committed, not ignored: the workspace ships a binary,
and `tools/ci` diffs the whole tree after a build, so an ignored lock
would be a file every CI run rewrote and nobody pinned. It is
generated in the projection with `cargo generate-lockfile` (a private
target dir; no build) — about half a second against a warm registry
index, and the projection's own `cargo build --locked` afterwards is
what proves it.

```rust {#copy-docs-and-scaffold}
let mut path_map = copy_literate_docs(workspace, output_dir, &literate, &mut report)?;
write_projected_documents(output_dir, &projected_docs, &mut path_map, &mut report)?;

emit_workspace_manifest(output_dir, &crates, &edition)?;
emit_licenses(output_dir, &crates, &license_bodies)?;
emit_ci_and_guard(output_dir, opts.emit_github)?;
// `.direnv/` is a contributor's shell state; without the entry `tools/ci`
// reports it as drift to anyone running CI from a direnv checkout.
std::fs::write(output_dir.join(".gitignore"), "/target\n**/target\n.direnv/\n")?;
generate_lockfile(output_dir)?;
emit_provenance(
    output_dir,
    &env.id,
    &path_map,
    &report,
    &license,
    license_source,
    &source_licenses,
)?;
tangle_publication_doc(region_doc, workspace, output_dir, &overlay)?;
write_readme_contents(output_dir, &literate, &vocab_modules, &modules_rel)?;
write_readme_affordances(output_dir, &affordances, &mut report)?;
```

The README is authored, not templated — with one section the
publication document does not write: its **contents page**. Two facts
meet there and neither belongs in the authored chunk. *What* the
repository ships the projector already computes — the crates, the
literate set, the vocabulary modules. What each of those documents is
*about* the document itself already carries, in the `summary:` its
envelope declares. Restating either in the README chunk makes it a
second authority for a sentence something else owns, and the failure
mode is silence: add a chapter, forget the line, and the public
contents page is quietly short one entry with nothing to catch it.

So the contract is a division of labour. The authored README declares
*where* the contents go, and *how the book is organized*, by carrying
one marker — an HTML comment, which every Markdown renderer hides and
this projector can find unambiguously:

```text
<!-- x0k:contents -->
```

The projector replaces that marker with the whole page: every shipped
literate document, linked at its title and followed by its own
`summary:`, then the shipped vocabulary modules followed by their module
fact's `rdfs:comment`. Nothing about a document is written twice.

What the projector cannot derive is **the shape of the book**. A
repository is a set of crates; a book is a sequence of concepts, and the
two do not coincide — the chapter that says what an envelope is and the
chapter that checks one against its vocabulary teach together and ship
from different crates. So the marker carries named **groups**, each
heading a concept, with explicit membership:

```text
<!-- x0k:contents
# What a document is
> The envelope, the block tree beneath it, and the identity that names it.
  folio/format
  folio/colophon
  folio/structural
# Chunks, and resolving them
> Named blocks, the references between them, and the order a compiler needs.
  tangle/chunk
  tangle/resolution
-->
```

A `# ` line opens a group and its text is the heading, verbatim: the
concept an author named, not a crate the projector inferred. The `> `
line directly under it is that group's one-line blurb — the only
sentence on the page nothing else can supply, because every entry gets
its sentence from a document's `summary:` and a concept has no file to
carry one. Every other line is a member, written `<area>/<stem>`: a bare
stem is unambiguous only inside its area, and a group crosses areas by
design.

Group membership is **exhaustive**, and that is the whole reason to
prefer it. A member naming a document the publication does not ship
refuses, as a stale name always did. The dual — a shipped document that
*no* group names — is the failure the older form could not see, because
it listed the leftovers by path and so always looked complete. Here it
refuses, naming what has no place, and so does a document that two
groups both claim. The page accounts for every chapter or it does not
render.

The older form is still parsed, so nothing had to move at once. A line
`<area>: <stem> <stem> …` is a per-area reading **spine**: it reorders
but never selects, the documents it names leading and the rest following
by path, each area heading under the crate its chapters back. That form
cannot express a group and cannot be exhaustive, so a marker may not mix
the two — one that does refuses rather than half-applying either.

Three refusals stand around the page, and each one closes a way for it
to say less than the repository holds. A publication with documents or
modules to list and **no marker** refuses — a README with no contents
section is the silence the marker exists to break, and it is the same
refusal the module table used to make. A **second marker** refuses: the
page has one home. And a shipped document with **no `summary:`**
refuses, naming the document. The alternative was to fall back to the
title, which is already the link text — a line that reads as a
description and carries no information, printed for exactly the
document nobody has described yet. The fix is one line in that
document's own envelope, which is where the sentence belonged anyway;
a module with no `rdfs:comment` refuses for the same reason.

```rust {#write-readme-contents}
/// The marker a publication's README carries where its contents page goes.
/// An HTML comment: hidden in every rendered README, unambiguous to find.
const CONTENTS_MARKER: &str = "<!-- x0k:contents";

/// One named group of the contents page: the heading verbatim from the
/// marker, the one-line blurb under it, and the `<area>/<stem>` members it
/// claims, in the order it names them.
#[derive(Debug, PartialEq)]
struct ContentsGroup {
    heading: String,
    blurb: Option<String>,
    members: Vec<String>,
}

/// What a marker says about the page beneath it. `Groups` is the concept
/// form — named groups with explicit, exhaustive membership. `Spine` is the
/// older per-area reading order, which reorders but never selects; an empty
/// one is a bare `<!-- x0k:contents -->`. A marker declares one or the other.
#[derive(Debug, PartialEq)]
enum ContentsPlan {
    Spine(Vec<(String, Vec<String>)>),
    Groups(Vec<ContentsGroup>),
}

/// A parsed contents marker: the README lines it occupies (`start`..`end`)
/// and the plan it declares.
#[derive(Debug)]
struct ContentsMarker {
    start: usize,
    end: usize,
    plan: ContentsPlan,
}

/// Find the README's one contents marker and parse the plan it carries. Two
/// markers refuse (the page has one home) and so does one that never closes;
/// no marker at all is `None`, and the caller decides whether this
/// publication was allowed to omit it.
fn find_contents_marker(lines: &[&str]) -> Result<Option<ContentsMarker>> {
    let hits: Vec<usize> = lines
        .iter()
        .enumerate()
        .filter(|(_, l)| l.trim_start().starts_with(CONTENTS_MARKER))
        .map(|(i, _)| i)
        .collect();
    if hits.len() > 1 {
        bail!(
            "the README carries {} `{CONTENTS_MARKER} -->` markers; the contents \
             page has exactly one home",
            hits.len()
        );
    }
    let Some(start) = hits.first().copied() else {
        return Ok(None);
    };
    let mut spine: Vec<(String, Vec<String>)> = Vec::new();
    let mut groups: Vec<ContentsGroup> = Vec::new();
    // A blurb belongs to the heading it follows, so the parser remembers
    // whether the last line it took was one.
    let mut under_heading = false;
    let mut end = start + 1;
    // `<!-- x0k:contents -->` on one line carries no plan; otherwise every
    // line up to the closing `-->` is a heading, a blurb, a member, or an
    // `<area>: <stem> …` spine line.
    if !lines[start].trim_end().ends_with("-->") {
        loop {
            let Some(line) = lines.get(end) else {
                bail!("the README's `{CONTENTS_MARKER}` marker is never closed with `-->`");
            };
            end += 1;
            let line = line.trim();
            if line == "-->" {
                break;
            }
            if line.is_empty() {
                continue;
            }
            if let Some(heading) = line.strip_prefix("# ") {
                groups.push(ContentsGroup {
                    heading: heading.trim().to_string(),
                    blurb: None,
                    members: Vec::new(),
                });
                under_heading = true;
                continue;
            }
            if let Some(blurb) = line.strip_prefix("> ") {
                let Some(group) = groups.last_mut().filter(|_| under_heading) else {
                    bail!(
                        "the contents marker's blurb line `{line}` does not sit directly \
                         under a `# ` group heading"
                    );
                };
                group.blurb = Some(blurb.trim().to_string());
                under_heading = false;
                continue;
            }
            under_heading = false;
            if let Some((area, stems)) = line.split_once(':') {
                spine.push((
                    area.trim().to_string(),
                    stems.split_whitespace().map(str::to_string).collect(),
                ));
                continue;
            }
            let Some(group) = groups.last_mut() else {
                bail!(
                    "the contents marker's line `{line}` is neither a `# ` group heading \
                     nor an `<area>: <stem> …` reading order"
                );
            };
            let mut parts = line.split('/');
            match (parts.next(), parts.next(), parts.next()) {
                (Some(a), Some(s), None) if !a.is_empty() && !s.is_empty() => {}
                _ => bail!(
                    "the contents marker's group member `{line}` is not `<area>/<stem>`; a \
                     bare stem is unambiguous only inside one area, and a group crosses areas"
                ),
            }
            group.members.push(line.to_string());
        }
    }
    // Group membership is exhaustive and a spine is not, so a marker holding
    // both is two different promises about the same page.
    if !spine.is_empty() && !groups.is_empty() {
        bail!(
            "the contents marker mixes `# ` groups with `<area>: <stem> …` reading-order \
             lines; a marker declares one form or the other, because groups are exhaustive \
             and a reading order is not"
        );
    }
    let plan = if groups.is_empty() {
        ContentsPlan::Spine(spine)
    } else {
        ContentsPlan::Groups(groups)
    };
    Ok(Some(ContentsMarker { start, end, plan }))
}

/// The name a contents group calls a document by. Bare stems collide once a
/// group spans areas, so membership is written `<area>/<stem>`.
fn doc_member_key(rel: &Path) -> String {
    format!("{}/{}", doc_area(rel), doc_stem(rel))
}

/// One entry of the page: the document's title as the link, and its own
/// envelope `summary:` as the sentence. A document that does not say what it
/// is about cannot be listed — the fallback would be its title printed
/// twice, which reads as a description and carries no information.
fn contents_entry(doc: &LiterateDoc) -> Result<String> {
    let summary = doc
        .summary
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .ok_or_else(|| {
            anyhow!(
                "literate document {} ships with no `summary:` in its envelope — \
                 the contents page is written from the documents, so a document \
                 that does not say what it is about cannot be listed",
                doc.rel.display()
            )
        })?;
    Ok(format!(
        "- [{}]({}) — {}\n",
        doc.title,
        doc.rel.display(),
        summary
    ))
}

/// Render the concept groups the marker names. The heading is the group's,
/// verbatim — the crate a chapter backs stays visible in the link's own path,
/// and is no longer what the page is organized by. Membership is exhaustive
/// in both directions: a member the publication does not ship refuses, a
/// shipped document no group names refuses, and a document two groups claim
/// refuses. A contents page either accounts for every chapter or it does not
/// render.
fn render_by_group(docs: &[LiterateDoc], groups: &[ContentsGroup]) -> Result<String> {
    let shipped: BTreeMap<String, &LiterateDoc> =
        docs.iter().map(|d| (doc_member_key(&d.rel), d)).collect();
    let mut claimed: BTreeMap<&str, &str> = BTreeMap::new();
    let mut out = String::new();
    for group in groups {
        out.push_str(&format!("### {}\n\n", group.heading));
        if let Some(blurb) = group.blurb.as_deref() {
            out.push_str(blurb);
            out.push_str("\n\n");
        }
        for member in &group.members {
            let Some(doc) = shipped.get(member.as_str()) else {
                bail!(
                    "the contents marker's group `{}` names `{member}`, which this \
                     publication does not ship",
                    group.heading
                );
            };
            if let Some(first) = claimed.insert(member.as_str(), group.heading.as_str()) {
                bail!(
                    "the contents marker names `{member}` twice: in group `{first}` and in \
                     group `{}`; a shipped document belongs to exactly one group",
                    group.heading
                );
            }
            out.push_str(&contents_entry(doc)?);
        }
        out.push('\n');
    }
    let unclaimed: Vec<&str> = shipped
        .keys()
        .map(String::as_str)
        .filter(|k| !claimed.contains_key(k))
        .collect();
    if !unclaimed.is_empty() {
        bail!(
            "the contents marker's groups name no place for {} shipped document(s): {} — \
             every shipped document belongs to exactly one group, so a chapter cannot drop \
             out of the book unseen",
            unclaimed.len(),
            unclaimed.join(", ")
        );
    }
    Ok(out)
}

/// Render the older per-area form: every shipped document under its area,
/// the areas the spine names leading in the order it names them and the rest
/// following by path, each area heading under the crate its chapters back.
/// The spine selects nothing — a document it does not name still ships — so
/// a name that ships nothing is stale, and stale is silent unless it refuses.
fn render_by_area(docs: &[LiterateDoc], order: &[(String, Vec<String>)]) -> Result<String> {
    // Group by area directory, keeping the path order discovery produced.
    let mut areas: Vec<(String, Vec<&LiterateDoc>)> = Vec::new();
    for doc in docs {
        let area = doc_area(&doc.rel);
        match areas.iter_mut().find(|(a, _)| *a == area) {
            Some((_, members)) => members.push(doc),
            None => areas.push((area, vec![doc])),
        }
    }
    for (area, _) in order {
        if !areas.iter().any(|(a, _)| a == area) {
            bail!(
                "the contents marker names area `{area}`, which this publication \
                 ships no documents from"
            );
        }
    }
    // Stable, so the areas the marker does not name keep their path order.
    areas.sort_by_key(|(a, _)| order.iter().position(|(o, _)| o == a).unwrap_or(usize::MAX));

    let mut out = String::new();
    for (area, members) in areas.iter_mut() {
        let spine: &[String] = order
            .iter()
            .find(|(a, _)| a == area)
            .map(|(_, s)| s.as_slice())
            .unwrap_or_default();
        for stem in spine {
            if !members.iter().any(|d| doc_stem(&d.rel) == *stem) {
                bail!(
                    "the contents marker's reading order for area `{area}` names \
                     `{stem}`, which this publication does not ship"
                );
            }
        }
        members.sort_by_key(|d| {
            spine.iter().position(|s| *s == doc_stem(&d.rel)).unwrap_or(usize::MAX)
        });
        // The area heads its own section under the crate its chapters back;
        // an area whose documents name no single crate heads under its path.
        let crates: BTreeSet<&str> = members.iter().filter_map(|d| d.crate_name.as_deref()).collect();
        let heading = match crates.iter().copied().collect::<Vec<_>>()[..] {
            [one] => format!("`{one}`"),
            _ => format!("`knowledge/implementation/{area}/`"),
        };
        out.push_str(&format!("### {heading}\n\n"));
        for doc in members.iter() {
            out.push_str(&contents_entry(doc)?);
        }
        out.push('\n');
    }
    Ok(out)
}

/// Render the contents page: the documents under whichever plan the marker
/// declared, then the shipped vocabulary modules, each described by its own
/// module fact's `rdfs:comment`.
fn render_contents(
    docs: &[LiterateDoc],
    plan: &ContentsPlan,
    modules: &[VocabModule],
    modules_rel: &Path,
) -> Result<String> {
    let mut out = match plan {
        ContentsPlan::Groups(groups) => render_by_group(docs, groups)?,
        ContentsPlan::Spine(order) => render_by_area(docs, order)?,
    };

    if !modules.is_empty() {
        out.push_str("### Vocabulary modules\n\n");
        for m in modules {
            let comment = m
                .comment
                .as_deref()
                .map(str::trim)
                .filter(|c| !c.is_empty())
                .ok_or_else(|| {
                    anyhow!(
                        "vocabulary module `{}` carries no `rdfs:comment` — the contents \
                         page is written from the module fact, so a module that does not \
                         say what it is cannot be listed",
                        m.name
                    )
                })?;
            out.push_str(&format!(
                "- [`{dir}/{name}.ttl`]({dir}/{name}.ttl) — {comment}\n",
                dir = modules_rel.display(),
                name = m.name
            ));
        }
        out.push('\n');
    }
    Ok(out.trim_end_matches('\n').to_string())
}

/// Replace the README's contents marker with the generated page. A projection
/// with documents or modules to list and no marker refuses: the alternative is
/// a public README that silently says less than the repository ships.
fn write_readme_contents(
    output_dir: &Path,
    docs: &[LiterateDoc],
    modules: &[VocabModule],
    modules_rel: &Path,
) -> Result<()> {
    let path = output_dir.join("README.md");
    let text = std::fs::read_to_string(&path).context("reading the tangled README")?;
    let lines: Vec<&str> = text.split_inclusive('\n').collect();
    let Some(marker) = find_contents_marker(&lines)? else {
        if docs.is_empty() && modules.is_empty() {
            return Ok(());
        }
        bail!(
            "the publication ships {} literate document(s) and {} vocabulary module(s) \
             but its README carries no `{CONTENTS_MARKER} -->` marker to write the \
             contents page into",
            docs.len(),
            modules.len()
        );
    };
    let contents = render_contents(docs, &marker.plan, modules, modules_rel)?;
    let mut out = String::new();
    out.extend(lines[..marker.start].iter().copied());
    out.push_str(&contents);
    out.push('\n');
    out.extend(lines[marker.end..].iter().copied());
    std::fs::write(&path, out).context("writing the README with its contents page")?;
    tracing::info!(
        documents = docs.len(),
        modules = modules.len(),
        "region_repo.readme.contents_written"
    );
    Ok(())
}
```

Finally the overlay goes back over the projector's output, and the
result is committed: a root commit for a fresh repository, or one
commit on top of the existing history — none at all when the
re-projection changed nothing, so an idle publish leaves no empty
commit and `committed` says which happened.

```rust {#restore-overlay-and-commit}
if let Some(stash) = stash {
    restore_overlay(output_dir, &stash)?;
}

if opts.git_init {
    report.committed = if existing_repo {
        git_commit_projection(output_dir, &env.id, &report)
            .context("committing the re-projection into the existing repo")?
    } else {
        git_init_commit(output_dir, &env.id, &report)
            .context("git init/commit of the projected repo")?;
        true
    };
}

Ok(report)
```

## Reading the envelope

`overlay:` entries are validated as plain repo-relative paths: no
absolute paths, no `..`, no trailing slash (a directory is named by its
bare path), and never `.git` or `PROVENANCE.json`, which the projector
owns. A bad entry refuses the projection rather than silently
preserving something the publication did not mean.

```rust {#overlay-paths}
/// Read the `overlay:` list from the publication envelope and validate each
/// entry as a plain projected-repo-relative path (no absolute paths, no `..`,
/// no trailing slash — a directory is named by its bare path).
fn overlay_paths(content: &str) -> Result<Vec<String>> {
    let mut out = Vec::new();
    for raw in envelope_string_list(content, "overlay") {
        let p = raw.trim().trim_end_matches('/').to_string();
        let path = Path::new(&p);
        if p.is_empty()
            || path.is_absolute()
            || path.components().any(|c| {
                matches!(
                    c,
                    std::path::Component::ParentDir | std::path::Component::RootDir
                )
            })
            || p == "." || p == ".git" || p == "PROVENANCE.json"
        {
            bail!("`overlay:` entry `{raw}` is not a plain projected-repo-relative path");
        }
        if !out.contains(&p) {
            out.push(p);
        }
    }
    Ok(out)
}
```

The previous projection's revisions come from the `PROVENANCE.json`
already in the output directory. A missing field is tolerated — an
older projection recorded no `corpus_commit` — so the chain simply
starts where the record does.

```rust {#previous-provenance-field}
/// One string field of the `PROVENANCE.json` already in `output_dir`, if
/// there is one — the previous link in the projected history. Absent
/// fields (an older projection without `corpus_commit`) are tolerated.
fn previous_provenance_field(output_dir: &Path, key: &str) -> Option<String> {
    let text = std::fs::read_to_string(output_dir.join("PROVENANCE.json")).ok()?;
    let json: serde_json::Value = serde_json::from_str(&text).ok()?;
    json.get(key)
        .and_then(|v| v.as_str())
        .filter(|s| !s.is_empty())
        .map(|s| s.to_string())
}
```

## The overlay stash

Overlay paths wait under `target/.overlay-stash` while the regenerated
region is cleared. `target/` is the right place because the clear never
touches it, the repository's `.gitignore` already hides it, and a
rename within one filesystem is atomic and cheap even for a directory
tree. Only entries that actually exist are stashed; the publication may
declare an overlay path the public side has not created yet.

```rust {#overlay-stash}
/// Where overlay paths wait while the regenerated region is cleared and
/// rewritten. Lives under `target/` (never cleared, git-ignored) so a rename
/// stays on one filesystem.
const OVERLAY_STASH: &str = "target/.overlay-stash";

/// Overlay paths moved aside for the duration of a re-projection.
struct OverlayStash {
    root: PathBuf,
    /// The overlay entries that existed and were moved, repo-relative.
    stashed: Vec<String>,
}

/// Move every existing overlay path out of the way.
fn stash_overlay(output_dir: &Path, overlay: &[String]) -> Result<OverlayStash> {
    let root = output_dir.join(OVERLAY_STASH);
    if root.exists() {
        std::fs::remove_dir_all(&root)?;
    }
    let mut stashed = Vec::new();
    for rel in overlay {
        let src = output_dir.join(rel);
        if !src.exists() {
            continue;
        }
        let dst = root.join(rel);
        if let Some(parent) = dst.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::rename(&src, &dst)
            .with_context(|| format!("stashing overlay path {rel}"))?;
        tracing::info!(path = %rel, "region_repo.overlay.stashed");
        stashed.push(rel.clone());
    }
    Ok(OverlayStash { root, stashed })
}
```

Restoring replaces whatever the projector wrote at an overlay path —
the corpus never wins over the overlay — and removes the stash root so
a clean projection leaves no trace of the mechanism.

```rust {#restore-overlay}
/// Put the stashed overlay paths back, replacing anything the projector
/// wrote at those paths — the overlay is preserved exactly as found.
fn restore_overlay(output_dir: &Path, stash: &OverlayStash) -> Result<()> {
    for rel in &stash.stashed {
        let dest = output_dir.join(rel);
        if dest.is_dir() {
            std::fs::remove_dir_all(&dest)?;
        } else if dest.exists() {
            std::fs::remove_file(&dest)?;
        }
        if let Some(parent) = dest.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::rename(stash.root.join(rel), &dest)
            .with_context(|| format!("restoring overlay path {rel}"))?;
        tracing::info!(path = %rel, "region_repo.overlay.restored");
    }
    if stash.root.exists() {
        std::fs::remove_dir_all(&stash.root)?;
    }
    Ok(())
}
```

```rust {#clear-regenerated-region}
/// Remove everything in `output_dir` except `.git` and `target/` — the
/// regenerated region is authoritative from the corpus, so nothing a prior
/// projection wrote may linger past a re-projection.
fn clear_regenerated_region(output_dir: &Path) -> Result<()> {
    for entry in std::fs::read_dir(output_dir)? {
        let entry = entry?;
        let name = entry.file_name();
        if name == ".git" || name == "target" {
            continue;
        }
        let path = entry.path();
        if entry.file_type()?.is_dir() {
            std::fs::remove_dir_all(&path)
                .with_context(|| format!("clearing {}", path.display()))?;
        } else {
            std::fs::remove_file(&path)
                .with_context(|| format!("clearing {}", path.display()))?;
        }
    }
    Ok(())
}
```

## Envelope keys the colophon does not own

Member URIs are split by their kind prefix: `x0k:software-module/`
names a crate (an `excludes` entry may carry a `#feature` suffix
naming the severance, and only the crate name matters here) and
`x0k:ontology-module/` names a vocabulary module. Any other URI under
a membership edge is an error naming it.

```rust {#member-names}
/// The members a membership edge names, split by kind.
#[derive(Debug, Default)]
struct Members {
    crates: Vec<String>,
    modules: Vec<String>,
    /// Literate documents held back, by the full `x0k:implementation/…` URI.
    /// Kept whole rather than stripped: the match is against a document's own
    /// envelope `id:`, which is the identity that survives a file move.
    docs: Vec<String>,
    /// Documents the publication selects — whole, or one section of one.
    /// Only `publishes` fills this.
    documents: Vec<DocSelection>,
}

/// Read a membership edge (`publishes`, `excludes`, `entryPoint`): crates
/// lose their `x0k:software-module/` prefix (and any `#feature` suffix),
/// vocabulary modules their `x0k:ontology-module/` prefix, and — under
/// `excludes` only — literate documents keep their whole URI. Any other
/// x0k id under `publishes` is a document selection.
fn member_names(uris: Option<&Vec<String>>, edge: &str) -> Result<Members> {
    let mut out = Members::default();
    for u in uris.into_iter().flatten() {
        if let Some(c) = u.strip_prefix(SOFTWARE_MODULE_PREFIX) {
            out.crates.push(c.split('#').next().unwrap_or(c).to_string());
        } else if let Some(m) = u.strip_prefix(ONTOLOGY_MODULE_PREFIX) {
            out.modules.push(m.to_string());
        } else if u.starts_with(IMPLEMENTATION_DOC_PREFIX) {
            // Only a severance. In `publishes` a document would be
            // meaningless — documents are not selected, they follow the
            // crate they tangle to — and silently accepting one there would
            // read as a membership the projector does not honour.
            if edge != "excludes" {
                bail!(
                    "`{edge}` member `{u}` is a literate document; documents are \
                     not selected but follow the crate they tangle to, so only \
                     `excludes` may name one"
                );
            }
            out.docs.push(u.clone());
        } else if edge == "publishes" && u.starts_with("x0k:") {
            // A document, addressed the way the format already addresses
            // one: the transclusion reference `<id>#<heading-path>`, with
            // no anchor meaning the whole document. One address, two
            // directions — the string that pulls a section into a document
            // is the string that projects one out of a region.
            let id: EntityId = u.parse().map_err(|e| {
                anyhow!("`publishes` member `{u}` is not a readable x0k id: {e}")
            })?;
            out.documents.push(DocSelection {
                reference: u.clone(),
                id,
            });
        } else {
            bail!(
                "`{edge}` member `{u}` is not an x0k:software-module/, an \
                 x0k:ontology-module/, (under `excludes`) an \
                 x0k:implementation/, or (under `publishes`) a document URI"
            );
        }
    }
    Ok(out)
}
```

The shared envelope parser deliberately drops keys it does not own
(`license:`, `repository:`, `keywords:`, `overlay:`). Publication-
specific keys are read here, by the consumer that owns them, with a
tolerant line scan over the raw YAML: a scalar at two-space indent, or
a list in either inline or block form. This is grungy string handling,
and it is confined to these two functions.

```rust {#envelope-scalar}
/// Read one top-level scalar (`key: value`) out of a publication doc's
/// `x0k:` envelope. The shared envelope parser deliberately drops keys it
/// doesn't own (`license:`, `repository:`, …); publication-specific keys are
/// read here, by the consumer that owns them, with a tolerant line scan.
fn envelope_scalar(content: &str, key: &str) -> Option<String> {
    let (yaml, _) = split_frontmatter(content)?;
    let prefix = format!("{key}:");
    for line in yaml.lines() {
        let indent = line.len() - line.trim_start().len();
        let trimmed = line.trim_start();
        if indent == 2 {
            if let Some(val) = trimmed.strip_prefix(&prefix) {
                let val = val.trim().trim_matches('"').trim_matches('\'').trim();
                if !val.is_empty() {
                    return Some(val.to_string());
                }
            }
        }
    }
    None
}
```

```rust {#envelope-string-list}
/// Read a top-level string list (`key: [a, b]` inline, or a `- item` block
/// sequence) out of a publication doc's `x0k:` envelope. Same ownership
/// rationale as [`envelope_scalar`].
fn envelope_string_list(content: &str, key: &str) -> Vec<String> {
    let Some((yaml, _)) = split_frontmatter(content) else {
        return Vec::new();
    };
    let prefix = format!("{key}:");
    let mut out = Vec::new();
    let mut in_seq = false;
    for line in yaml.lines() {
        let indent = line.len() - line.trim_start().len();
        let trimmed = line.trim_start();
        if in_seq {
            if trimmed.starts_with("- ") && indent > 2 {
                let item = trimmed[2..].trim().trim_matches('"').trim_matches('\'');
                if !item.is_empty() {
                    out.push(item.to_string());
                }
                continue;
            }
            if trimmed.is_empty() {
                continue;
            }
            break;
        }
        if indent == 2 {
            if let Some(rest) = trimmed.strip_prefix(&prefix) {
                let rest = rest.trim();
                if let Some(inline) = rest.strip_prefix('[').and_then(|r| r.strip_suffix(']')) {
                    return inline
                        .split(',')
                        .map(|s| s.trim().trim_matches('"').trim_matches('\'').to_string())
                        .filter(|s| !s.is_empty())
                        .collect();
                }
                if rest.is_empty() {
                    in_seq = true;
                }
            }
        }
    }
    out
}
```

## Vendoring a crate

Manifests are read and rewritten through `toml_edit`, so a hand-authored
`Cargo.toml` keeps its layout and comments through the projection; only
the keys the publication act owns change.

```rust {#manifest-readers}
fn manifest_package_str(doc: &toml_edit::DocumentMut, key: &str) -> Option<String> {
    doc.get("package")
        .and_then(|p| p.get(key))
        .and_then(|v| v.as_str())
        .map(|s| s.to_string())
}

/// crates.io metadata sourced from the publication doc.
#[derive(Debug, Clone, Default)]
struct CratesIoMeta {
    repository: Option<String>,
    keywords: Vec<String>,
}

/// Everything the vendoring step needs to rewrite a crate manifest and to
/// judge the `@generated` files that arrive with the crate.
struct VendorCtx<'a> {
    license: &'a str,
    published: &'a BTreeSet<String>,
    excluded: &'a BTreeSet<String>,
    versions: &'a BTreeMap<String, String>,
    crates_io: &'a CratesIoMeta,
    /// Workspace-relative paths of every document in the projection's
    /// literate set — the sources a vendored `@generated` file may name.
    literate_set: &'a BTreeSet<String>,
}

fn read_manifest(workspace: &Path, crate_name: &str) -> Result<toml_edit::DocumentMut> {
    let path = workspace.join(crate_name).join("Cargo.toml");
    let text =
        std::fs::read_to_string(&path).with_context(|| format!("reading {}", path.display()))?;
    text.parse::<toml_edit::DocumentMut>()
        .with_context(|| format!("parsing {}", path.display()))
}

fn manifest_access(doc: &toml_edit::DocumentMut) -> String {
    doc.get("package")
        .and_then(|p| p.get("metadata"))
        .and_then(|m| m.get("x0k"))
        .and_then(|x| x.get("access"))
        .and_then(|a| a.as_str())
        .unwrap_or("public")
        .to_string()
}
```

```rust {#path-deps}
/// `(dep-key, optional, target-crate)` for each `path = "..."` dependency.
fn path_deps(doc: &toml_edit::DocumentMut) -> Vec<(String, bool, String)> {
    let Some(deps) = doc.get("dependencies").and_then(|d| d.as_table()) else {
        return Vec::new();
    };
    let mut out = Vec::new();
    for (key, item) in deps.iter() {
        let Some(t) = item.as_table_like() else {
            continue;
        };
        let Some(path) = t.get("path").and_then(|p| p.as_str()) else {
            continue;
        };
        let target = Path::new(path)
            .file_name()
            .and_then(|s| s.to_str())
            .unwrap_or(path)
            .to_string();
        let optional = t.get("optional").and_then(|o| o.as_bool()).unwrap_or(false);
        out.push((key.to_string(), optional, target));
    }
    out
}
```

Copying skips `target/` and `.git` — a crate directory in a jj
workspace can carry both — and hands the vendored manifest to the
rewrite.

A crate directory can also carry a `@generated` file whose document
lives outside the region: a demo test tangled from a chapter about
another subsystem, say, that happens to land under the crate's
`tests/`. Its header names a source the projection does not carry, so
inside the public repository it would be a file nobody can regenerate,
guarded against edits, describing something the repository does not
contain. The projector drops such a file rather than ship it, and
records the path in the report so `PROVENANCE.json` says what was held
back. The judgment is the header alone — `@generated by x0k-tangle …
from <doc> — DO NOT EDIT` — read against the literate set the
projection settled before vendoring began.

```rust {#vendor-crate}
/// Copy `<workspace>/<crate>/` into `<output>/<crate>/`, drop any
/// `@generated` file whose source document is outside the projection's
/// literate set, then rewrite the vendored manifest (drop workspace-hack +
/// publish-excluded optional deps, sever the features that reference them;
/// set the license + crates.io metadata). Returns the projected-repo-relative
/// paths of the files it dropped.
fn vendor_crate(
    workspace: &Path,
    output_dir: &Path,
    crate_name: &str,
    ctx: &VendorCtx<'_>,
) -> Result<Vec<String>> {
    let src = workspace.join(crate_name);
    let dst = output_dir.join(crate_name);
    let mut dropped = Vec::new();
    for entry in walkdir::WalkDir::new(&src).into_iter().filter_entry(|e| {
        let n = e.file_name().to_string_lossy();
        n != "target" && n != ".git"
    }) {
        let entry = entry?;
        let rel = entry.path().strip_prefix(&src).unwrap();
        let dest = dst.join(rel);
        if entry.file_type().is_dir() {
            std::fs::create_dir_all(&dest)?;
            continue;
        }
        if let Some(source) = generated_source(entry.path()) {
            if !ctx.literate_set.contains(&source) {
                let projected = Path::new(crate_name).join(rel).to_string_lossy().to_string();
                tracing::info!(
                    path = %projected,
                    source = %source,
                    "region_repo.generated.dropped"
                );
                dropped.push(projected);
                continue;
            }
        }
        if let Some(parent) = dest.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::copy(entry.path(), &dest)
            .with_context(|| format!("copying {}", entry.path().display()))?;
    }
    rewrite_vendored_manifest(&dst.join("Cargo.toml"), ctx)?;
    Ok(dropped)
}

/// The source document a file's `@generated by x0k-tangle … from <doc> —
/// DO NOT EDIT` header names, if its first line is such a header.
fn generated_source(path: &Path) -> Option<String> {
    let text = std::fs::read_to_string(path).ok()?;
    let first = text.lines().next()?;
    if !first.contains("@generated by x0k-tangle") {
        return None;
    }
    let after = first.split(" from ").nth(1)?;
    let source = after.split(" — DO NOT EDIT").next()?.trim();
    (!source.is_empty()).then(|| source.to_string())
}
```

The rewrite is where the publication act reaches into each crate. It
does four things, in an order that matters: set the license, inherit
the workspace's edition and toolchain floor, and fill crates.io
metadata gaps; give in-bundle path dependencies a `version`; drop the
dependencies that do not ship; and sever the features that referenced
them.

```rust {#rewrite-vendored-manifest}
/// Rewrite one vendored `Cargo.toml`: strip workspace-hack + any publish-excluded
/// optional dep, sever features that referenced a dropped dep (declared, empty,
/// out of `default`), set the license, inherit `edition`/`rust-version` from
/// the workspace, strip the corpus-only `[package.metadata.x0k]`, add the
/// crates.io metadata (`repository`, `readme`, `keywords`), and give in-bundle
/// path deps a `version` so `cargo publish` can resolve them off crates.io.
fn rewrite_vendored_manifest(path: &Path, ctx: &VendorCtx<'_>) -> Result<()> {
    let excluded = ctx.excluded;
    let text =
        std::fs::read_to_string(path).with_context(|| format!("reading {}", path.display()))?;
    let mut doc = text
        .parse::<toml_edit::DocumentMut>()
        .with_context(|| format!("parsing {}", path.display()))?;

    <<rewrite-package-metadata>>

    <<rewrite-in-bundle-versions>>

    <<rewrite-drop-deps>>

    <<rewrite-prune-features>>

    std::fs::write(path, doc.to_string()).with_context(|| format!("writing {}", path.display()))?;
    Ok(())
}
```

Fields the crate author set (`description`, `readme`, `keywords`) are
kept and the projection only fills gaps — except `license`, which the
publication act always sets, because the source tree's
`LicenseRef-Proprietary` is exactly what must not ship, and `edition`
and `rust-version`, which every published crate inherits from the
standalone workspace so the toolchain floor is declared in one place.
The corpus-only `[package.metadata.x0k]` table (module naming, access
class) is stripped: it is monorepo registry vocabulary, and it would
ship inside every crates.io tarball otherwise.

```rust {#rewrite-package-metadata}
// License + crates.io metadata. Existing fields the crate author set
// (description, readme, keywords) are kept; the projection only fills
// gaps — except `license`, which the publication act always sets, and
// `edition`/`rust-version`, which are inherited from the workspace.
if let Some(pkg) = doc.get_mut("package").and_then(|p| p.as_table_mut()) {
    pkg.insert("license", toml_edit::value(ctx.license));
    let mut inherit = toml_edit::InlineTable::new();
    inherit.insert("workspace", toml_edit::Value::from(true));
    pkg.insert("edition", toml_edit::value(inherit.clone()));
    pkg.insert("rust-version", toml_edit::value(inherit));
    // `[package.metadata.x0k]` is corpus registry vocabulary, not crates.io's.
    let metadata_empty = pkg
        .get_mut("metadata")
        .and_then(|m| m.as_table_like_mut())
        .map(|m| {
            m.remove("x0k");
            m.is_empty()
        });
    if metadata_empty == Some(true) {
        pkg.remove("metadata");
    }
    if let Some(repo) = &ctx.crates_io.repository {
        if pkg.get("repository").is_none() {
            pkg.insert("repository", toml_edit::value(repo.as_str()));
        }
    }
    if pkg.get("readme").is_none() {
        pkg.insert("readme", toml_edit::value("../README.md"));
    }
    if !ctx.crates_io.keywords.is_empty() && pkg.get("keywords").is_none() {
        let mut arr = toml_edit::Array::new();
        // crates.io caps keywords at 5.
        for k in ctx.crates_io.keywords.iter().take(5) {
            arr.push(k.as_str());
        }
        pkg.insert("keywords", toml_edit::value(arr));
    }
}
```

`cargo publish` strips `path` from a dependency and falls back to its
version requirement, so an in-bundle path dependency without a
`version` is unpublishable. The version comes from the target's own
vendored manifest, collected during the guard pass. A dev-dependency is
the same case — `cargo deny` reads it as a wildcard just the same, and
the published tarball carries it — so all three dependency tables are
walked (2026-09-05: a test-only path to `x0k-ontology` was the first).

```rust {#rewrite-in-bundle-versions}
// In-bundle path deps gain a `version` (from the target's vendored
// manifest) so the crate is publishable: cargo strips `path` on publish
// and falls back to the version requirement. Every dependency table,
// because a dev-dependency without one is a wildcard too.
for table in ["dependencies", "dev-dependencies", "build-dependencies"] {
    let Some(deps) = doc.get_mut(table).and_then(|d| d.as_table_mut()) else {
        continue;
    };
    let keys: Vec<String> = deps.iter().map(|(k, _)| k.to_string()).collect();
    for key in keys {
        let Some(t) = deps.get_mut(&key).and_then(|it| it.as_table_like_mut()) else {
            continue;
        };
        let Some(target) = t.get("path").and_then(|p| p.as_str()).map(|p| {
            Path::new(p)
                .file_name()
                .and_then(|s| s.to_str())
                .unwrap_or(p)
                .to_string()
        }) else {
            continue;
        };
        if t.get("version").is_none() && ctx.published.contains(&target) {
            if let Some(v) = ctx.versions.get(&target) {
                t.insert("version", toml_edit::value(v.as_str()));
            }
        }
    }
}
```

Two kinds of dependency are dropped: `workspace-hack`, and any
dependency whose path target is publish-excluded. The closure guard has
already ensured the latter are optional, so removing them cannot break a
default build.

```rust {#rewrite-drop-deps}
// Collect dep keys to drop: workspace-hack + any dep whose path target is
// publish-excluded.
let mut dropped: BTreeSet<String> = BTreeSet::new();
if let Some(deps) = doc.get_mut("dependencies").and_then(|d| d.as_table_mut()) {
    let keys: Vec<String> = deps.iter().map(|(k, _)| k.to_string()).collect();
    for key in keys {
        let drop = key == "workspace-hack"
            || deps
                .get(&key)
                .and_then(|it| it.as_table_like())
                .and_then(|t| t.get("path"))
                .and_then(|p| p.as_str())
                .map(|p| {
                    let target = Path::new(p)
                        .file_name()
                        .and_then(|s| s.to_str())
                        .unwrap_or(p);
                    excluded.contains(target)
                })
                .unwrap_or(false);
        if drop {
            deps.remove(&key);
            dropped.insert(key);
        }
    }
}
```

A feature that named a dropped dependency (`dep:<name>`) would fail to
resolve, so it is *severed*: its name is cleared out of `default` and
its list is emptied, but the declaration stays, carrying a comment that
says why. The declaration has to stay because the crate's
`#[cfg(feature = "…")]` sites ship unchanged — an undeclared feature
turns every one of them into an `unexpected_cfgs` warning, and
`--features motifs` into a hard error instead of an empty build. This
is the mechanical half of the `motifs` severance: the monorepo manifest
keeps the feature live, the projection keeps it as a name that builds
nothing.

```rust {#rewrite-prune-features}
// Sever features that reference a dropped dep (via `dep:<name>`): the
// declaration stays (the crate's `#[cfg(feature = …)]` sites ship), its
// list is emptied, and the name leaves `default`.
if let Some(features) = doc.get_mut("features").and_then(|f| f.as_table_mut()) {
    let feat_names: Vec<String> = features.iter().map(|(k, _)| k.to_string()).collect();
    let mut removed_feats: BTreeSet<String> = BTreeSet::new();
    for feat in feat_names {
        if feat == "default" {
            continue;
        }
        let refs_dropped = features
            .get(&feat)
            .and_then(|v| v.as_array())
            .map(|arr| {
                arr.iter().any(|e| {
                    e.as_str()
                        .map(|s| {
                            let dep = s.strip_prefix("dep:").unwrap_or(s);
                            dropped.contains(dep)
                        })
                        .unwrap_or(false)
                })
            })
            .unwrap_or(false);
        if refs_dropped {
            let mut empty = toml_edit::Array::new();
            empty.decor_mut().set_suffix(format!(" {SEVERED_FEATURE_NOTE}"));
            features.insert(&feat, toml_edit::value(empty));
            removed_feats.insert(feat);
        }
    }
    if let Some(default) = features.get_mut("default").and_then(|d| d.as_array_mut()) {
        default.retain(|e| {
            e.as_str()
                .map(|s| !removed_feats.contains(s))
                .unwrap_or(true)
        });
    }
}
```

## Reading a vocabulary module

A module file is N-Triples in the fixed shape the materializer writes
(ADR §4; the contract is restated in the ontology-modules brief): one
triple per line, sorted and deduped, string literals only, a `#`
header, and the module fact's lines all opening with the module IRI.
That shape is why the reader is **line-oriented** rather than an RDF
parser: it needs four facts — the `rdf:type owl:Ontology` line to
stamp after, the `owl:imports` objects, the `rdfs:comment` literal,
and the absence of anything under the instance namespace — and every
one is a prefix test on a line. Pulling `oxttl` into `x0k-tangle` for
that would add a parser and its dependency tree to a published crate
whose readers would gain nothing; if the module format ever admits
multi-line or non-canonical Turtle, this reader is what to replace.

An import must be another x0k-hosted module (`https://0k.computer/ontology/<name>`);
a foreign IRI refuses, because admitting foreign terms into the closure
is the trust question the ADR leaves open. The file's bytes are kept
so the write is the tree's file plus one line.

```rust {#vocab-module}
/// A vocabulary module selected for the projection, read from the tree.
struct VocabModule {
    name: String,
    /// Names of the modules its `owl:imports` name.
    imports: Vec<String>,
    /// The module fact's `rdfs:comment`, one line, unescaped.
    comment: Option<String>,
    /// The file as read, byte for byte.
    text: String,
    /// `ontology/shapes/<name>.ttl` as read, when the module has shapes.
    /// Not stamped: `owl:versionIRI` is a fact about the module, and the
    /// module file is where the module's facts are.
    shapes: Option<String>,
}

/// `[a-z][a-z0-9-]*` — the module name grammar of the file contract.
fn is_module_name(name: &str) -> bool {
    let mut chars = name.chars();
    matches!(chars.next(), Some(c) if c.is_ascii_lowercase())
        && chars.all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
}

/// The N-Triples IRI object of `rest` (`<iri> .`), if that is its shape.
fn ntriples_iri_object(rest: &str) -> Option<&str> {
    rest.trim()
        .strip_suffix('.')
        .map(str::trim_end)
        .and_then(|o| o.strip_prefix('<'))
        .and_then(|o| o.strip_suffix('>'))
}

/// The N-Triples string-literal object of `rest` (`"…" .`), unescaped
/// enough for a README entry: `\"`, `\\`, `\n` (to a space), `\t`.
fn ntriples_string_object(rest: &str) -> Option<String> {
    let body = rest
        .trim()
        .strip_suffix('.')
        .map(str::trim_end)
        .and_then(|o| o.strip_prefix('"'))
        .and_then(|o| o.strip_suffix('"'))?;
    let mut out = String::with_capacity(body.len());
    let mut chars = body.chars();
    while let Some(c) = chars.next() {
        if c != '\\' {
            out.push(c);
            continue;
        }
        match chars.next() {
            Some('n') | Some('r') => out.push(' '),
            Some('t') => out.push('\t'),
            Some(other) => out.push(other),
            None => out.push('\\'),
        }
    }
    Some(out)
}

fn read_vocab_module(workspace: &Path, name: &str) -> Result<VocabModule> {
    if !is_module_name(name) {
        bail!("`{name}` is not a vocabulary module name ([a-z][a-z0-9-]*)");
    }
    let rel = format!("{MODULES_DIR}/{name}.ttl");
    let path = workspace.join(&rel);
    let text = std::fs::read_to_string(&path)
        .with_context(|| format!("reading vocabulary module `{name}` at {rel}"))?;
    let iri = format!("<{MODULE_IRI_BASE}{name}>");
    let mut imports = Vec::new();
    let mut comment = None;
    let mut has_module_fact = false;
    for (i, line) in text.split_inclusive('\n').enumerate() {
        if line.contains(INSTANCE_NAMESPACE) {
            bail!(
                "vocabulary module `{name}` carries an instance fact ({rel}:{} names \
                 {INSTANCE_NAMESPACE}…>) — a module ships its vocabulary and nothing a \
                 product says about itself",
                i + 1
            );
        }
        let Some(rest) = line.strip_prefix(&iri) else {
            continue;
        };
        let rest = rest.trim_start();
        if let Some(obj) = rest.strip_prefix(OWL_IMPORTS) {
            let Some(target) = ntriples_iri_object(obj) else {
                bail!("vocabulary module `{name}`: malformed owl:imports at {rel}:{}", i + 1);
            };
            match target.strip_prefix(MODULE_IRI_BASE).filter(|d| is_module_name(d)) {
                Some(dep) => imports.push(dep.to_string()),
                None => bail!(
                    "vocabulary module `{name}` imports `{target}`, which is not an x0k-hosted \
                     module (`{MODULE_IRI_BASE}<name>`); foreign imports are not admitted"
                ),
            }
        } else if let Some(obj) = rest.strip_prefix(RDF_TYPE) {
            if obj.trim_start().starts_with(OWL_ONTOLOGY) {
                has_module_fact = true;
            }
        } else if let Some(obj) = rest.strip_prefix(RDFS_COMMENT) {
            comment = ntriples_string_object(obj);
        } else if rest.starts_with(OWL_VERSION_IRI) {
            bail!(
                "vocabulary module `{name}` carries an owl:versionIRI in the tree ({rel}:{}); \
                 the projector stamps it",
                i + 1
            );
        }
    }
    if !has_module_fact {
        bail!("vocabulary module `{name}` has no `{iri} rdf:type owl:Ontology` fact in {rel}");
    }
    let shapes_rel = format!("{SHAPES_DIR}/{name}.ttl");
    let shapes = match std::fs::read_to_string(workspace.join(&shapes_rel)) {
        Ok(text) => Some(text),
        // A module that constrains nothing has no shape file. Absence is the
        // ordinary case, so it must not be an error — the alternative would
        // make every module owe a file it may have nothing to put in.
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => None,
        Err(error) => return Err(error).with_context(|| format!("reading shapes for `{name}` at {shapes_rel}")),
    };
    if let Some(text) = &shapes {
        if let Some(line) = text.lines().position(|line| line.contains(INSTANCE_NAMESPACE)) {
            bail!(
                "shapes for `{name}` carry an instance fact ({shapes_rel}:{} names \
                 {INSTANCE_NAMESPACE}…>) — a shape constrains a vocabulary and says \
                 nothing a product says about itself",
                line + 1
            );
        }
    }
    Ok(VocabModule {
        name: name.to_string(),
        imports,
        comment,
        text,
        shapes,
    })
}

impl VocabModule {
    /// The file with `owl:versionIRI <module IRI>/<version>` inserted at
    /// its sorted position among the triple lines (the header comments and
    /// the blank line after them stay put), otherwise byte-identical.
    fn stamped(&self, version: &str) -> String {
        let stamp = format!(
            "<{base}{name}> {OWL_VERSION_IRI} <{base}{name}/{version}> .\n",
            base = MODULE_IRI_BASE,
            name = self.name
        );
        let mut out = String::with_capacity(self.text.len() + stamp.len());
        let mut placed = false;
        for line in self.text.split_inclusive('\n') {
            let is_triple = !line.starts_with('#') && !line.trim().is_empty();
            if !placed && is_triple && line.as_bytes() > stamp.as_bytes() {
                out.push_str(&stamp);
                placed = true;
            }
            out.push_str(line);
        }
        if !placed {
            if !out.ends_with('\n') && !out.is_empty() {
                out.push('\n');
            }
            out.push_str(&stamp);
        }
        out
    }
}
```

## The literate documents

The literate set is discovered in two passes. First, every document
under `knowledge/implementation/` whose `tangle.crate` names a
published crate — the tangled chapters, each with a sidecar. Then the
*areas* those chapters live in (`knowledge/implementation/<area>/`)
admit their prose-only chapters: a document in such an area with no
`tangle:` block at all is an overview or protocol description that the
tangled chapters cite (`tangle/protocol.md` is the one the crate chapter
tells a newcomer to read first), and it ships with them. A document
that *does* tangle, to a crate that is not published (`tangle/bundle.md`
→ `x0k-tangle-bundle`), stays out: its area is published but its
subject is not. The set is sorted by path so `PROVENANCE.json` is
stable across runs.

Each document arrives carrying what the contents page will say about
it — the crate its chapters back, the title it heads with, and its
envelope's `summary:` — read here because the envelope is already
parsed here, and asked for again nowhere.

```rust {#discover-literate-docs-fn}
/// One document of the literate set: its workspace-relative path, whether it
/// tangles (and so has a sidecar to carry) or is prose only, and the three
/// facts the contents page is written from — the crate its chapters back, the
/// title it heads with, and the `summary:` its own envelope declares.
struct LiterateDoc {
    rel: PathBuf,
    tangled: bool,
    crate_name: Option<String>,
    title: String,
    summary: Option<String>,
}

/// A document's area — the `knowledge/implementation/<area>/` directory it
/// lives in — and its stem, the two names a contents marker orders by.
fn doc_area(rel: &Path) -> String {
    rel.parent()
        .and_then(Path::file_name)
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_default()
}

fn doc_stem(rel: &Path) -> String {
    rel.file_stem().unwrap_or_default().to_string_lossy().to_string()
}

/// A document's title: its first `# ` heading, which is what a reader is
/// offered as the link text. A document with none falls back to its stem —
/// the title is the *name* of the entry, not its description, so a missing
/// one degrades to a filename rather than refusing the projection.
fn heading_title(body: &str, rel: &Path) -> String {
    body.lines()
        .find_map(|l| l.strip_prefix("# "))
        .map(|t| t.trim().to_string())
        .filter(|t| !t.is_empty())
        .unwrap_or_else(|| doc_stem(rel))
}

/// Discover the literate set: docs whose `tangle.crate` names a published
/// crate, plus the prose-only docs (no `tangle:` block) in the same
/// `knowledge/implementation/<area>/` directories. Docs that tangle to an
/// unpublished crate stay out even when their area is published.
fn discover_literate_docs(
    workspace: &Path,
    published: &BTreeSet<String>,
    excluded_docs: &BTreeSet<String>,
) -> Result<Vec<LiterateDoc>> {
    let impl_root = workspace.join("knowledge/implementation");
    if !impl_root.is_dir() {
        return Ok(Vec::new());
    }
    // An excluded id that matches no document excludes nothing, silently —
    // which is the shape of the defect this severance exists to close. Track
    // what was matched and refuse a name that hit nothing.
    let mut unmatched: BTreeSet<String> = excluded_docs.clone();
    // Each candidate, paired with whether its envelope declares `tangle:` at
    // all — the fact that separates a prose-only chapter from one that tangles
    // to a crate this publication does not ship.
    let mut candidates: Vec<(LiterateDoc, bool)> = Vec::new();
    for entry in walkdir::WalkDir::new(&impl_root)
        .into_iter()
        .filter_map(|e| e.ok())
    {
        if entry.file_type().is_dir() || entry.path().extension().map(|e| e != "md").unwrap_or(true)
        {
            continue;
        }
        let Ok(text) = std::fs::read_to_string(entry.path()) else {
            continue;
        };
        let Some((_, body)) = split_frontmatter(&text) else {
            continue;
        };
        let Ok((env, _)) = parse_envelope(&text) else {
            continue;
        };
        if excluded_docs.contains(&env.id) {
            unmatched.remove(&env.id);
            tracing::info!(doc = %env.id, "region_repo.document.excluded");
            continue;
        }
        let rel = entry.path().strip_prefix(workspace).unwrap().to_path_buf();
        let doc = LiterateDoc {
            title: heading_title(body, &rel),
            crate_name: env.tangle.as_ref().and_then(|t| t.crate_name.clone()),
            summary: env.summary.clone(),
            tangled: false,
            rel,
        };
        candidates.push((doc, env.tangle.is_some()));
    }
    if !unmatched.is_empty() {
        bail!(
            "`excludes` names literate document(s) no document under \
             knowledge/implementation/ declares as its `id:`: {:?} — an id that \
             matches nothing severs nothing",
            unmatched
        );
    }
    let ships = |doc: &LiterateDoc| doc.crate_name.as_deref().is_some_and(|c| published.contains(c));
    let areas: BTreeSet<PathBuf> = candidates
        .iter()
        .filter(|(doc, has_tangle)| *has_tangle && ships(doc))
        .filter_map(|(doc, _)| doc.rel.parent().map(Path::to_path_buf))
        .collect();
    let mut docs: Vec<LiterateDoc> = candidates
        .into_iter()
        .filter_map(|(mut doc, has_tangle)| {
            if has_tangle {
                if !ships(&doc) {
                    return None;
                }
                doc.tangled = true;
                return Some(doc);
            }
            doc.rel
                .parent()
                .is_some_and(|a| areas.contains(a))
                .then_some(doc)
        })
        .collect();
    docs.sort_by(|a, b| a.rel.cmp(&b.rel));
    Ok(docs)
}
```

Each document is copied at its canonical path, so the sidecar's
recorded output paths stay valid inside the projection; a tangled
chapter's sidecar travels with it, a prose chapter has none. The
returned map (canonical path → projected path, identity today) is the
`path_map` in `PROVENANCE.json` — the seam a receiver uses to route a
contributor's edit back to the document it came from.

```rust {#copy-literate-docs}
/// Copy each literate doc (preserving its `knowledge/implementation/<area>/`
/// path so sidecar output paths stay valid) and, for tangled docs, its
/// `<stem>.tangle-map.json` sidecar (rewriting the sidecar's absolute `source`
/// to repo-relative). Returns the published-path → canonical-path map for the
/// provenance seam.
fn copy_literate_docs(
    workspace: &Path,
    output_dir: &Path,
    docs: &[LiterateDoc],
    report: &mut RepoProjectReport,
) -> Result<BTreeMap<String, String>> {
    let mut path_map = BTreeMap::new();
    for doc in docs {
        let src = workspace.join(&doc.rel);
        let rel_str = doc.rel.to_string_lossy().to_string();
        let dst = output_dir.join(&doc.rel);
        if let Some(parent) = dst.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::copy(&src, &dst)
            .with_context(|| format!("copying literate doc {}", src.display()))?;
        report.literate_docs.push(doc.rel.clone());
        path_map.insert(rel_str.clone(), rel_str);

        // Sidecar: <stem>.tangle-map.json next to the doc.
        let sidecar = src.with_extension("tangle-map.json");
        if doc.tangled && sidecar.is_file() {
            copy_sidecar_rewriting_source(&sidecar, workspace, output_dir)?;
        }
    }
    Ok(path_map)
}
```

The sidecar is the one file where byte identity is the whole point.
The projected repository's CI re-tangles every document and requires a
clean tree, so the sidecar the projector writes must equal the sidecar
the tangler inside the projection would write. A `serde_json::Value`
round-trip does not: it sorts keys, where the tangler's own
[`TangleSidecar`](pipeline.md) serializes in declaration order, and the
two differ on every sidecar. Re-serializing through the tangler's type
is what makes the projection tangle into itself; the workspace-relative
`source` is what the projection's tangler records.

```rust {#copy-sidecar}
/// Copy one sidecar into the projection, re-serialized through the tangler's
/// own [`TangleSidecar`] type so the bytes equal what a re-tangle inside the
/// projection writes. The projected repo's CI re-tangles every literate doc
/// and requires a clean tree, so the sidecar must be byte-identical to the
/// tangler's output — not a `serde_json::Value` round-trip, which sorts keys
/// and so differs from the struct-ordered tangler output on every sidecar
/// (pinned by `tests/integration/tests/publication_self_tangle.rs`).
fn copy_sidecar_rewriting_source(
    sidecar: &Path,
    workspace: &Path,
    output_dir: &Path,
) -> Result<()> {
    let text = std::fs::read_to_string(sidecar)?;
    let mut parsed: TangleSidecar = serde_json::from_str(&text)
        .with_context(|| format!("parsing sidecar {}", sidecar.display()))?;
    // A stale absolute `source` (an older tangler wrote those) is
    // made workspace-relative; a relative one is already what the
    // projection's tangler would write.
    let abs = Path::new(&parsed.source);
    let rel_src = abs.strip_prefix(workspace).unwrap_or(abs);
    parsed.source = rel_src.to_string_lossy().to_string();
    let rel = sidecar.strip_prefix(workspace).unwrap();
    let dst = output_dir.join(rel);
    if let Some(parent) = dst.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(&dst, serde_json::to_string_pretty(&parsed)?)?;
    Ok(())
}
```

## Documents the publication names

The literate set above travels because its crates do — a chapter follows
the code it tangles to, and no publication lists it. Decision documents
are the opposite case. An affordance is declared inside the design that
owns it, under that design's own heading, and the rest of that design —
its context, its rationale, the questions still open on it — is usually
broader than the repository being published. So a publication *names*
what crosses, and it names it at the grain of a section.

The name is the address the format already has. A transclusion pulls a
section into a document by writing `<id>#<heading-path>`; a publication
projects one out of a region by writing the same string, and no reader
who understands the first needs anything new for the second. With no
anchor it is the whole document. Selection is **default-deny**: nothing
under `decisions/` is walked, nothing reachable from a named document
follows it out, and a document crosses because it was named and for no
other reason.

```rust {#doc-selection}
/// A document a publication names under `publishes`: its id, whose
/// `fragment` is the heading-path anchor selecting one section (`None`
/// selects the whole document). The reference as authored travels too,
/// so a refusal names what the publication wrote rather than what the
/// projector made of it.
#[derive(Debug, Clone)]
struct DocSelection {
    reference: String,
    id: EntityId,
}
```

Resolution is by name and never by discovery. The id's class picks a
directory under `decisions/` — `design` for a design, `commitments` for
the class the corpus pluralised — and the identifier is the file stem,
looked for beneath that directory because a decision moves between topic
subdirectories without changing its id. Every candidate's own envelope
`id:` must agree, so a stem collision across topics is caught rather than
guessed at, and a document that has been renamed refuses rather than
resolving to a stranger with the same filename.

```rust {#resolve-named-document}
/// The tree file a named document lives in. The lookup is keyed by the
/// name the publication wrote: the class picks the directory under
/// `decisions/`, the identifier is the stem, and the topic subdirectory a
/// decision may have moved into is searched for that stem — ids survive a
/// move, paths do not. Nothing here enumerates what `decisions/` holds; a
/// document nobody named is never opened.
fn resolve_named_document(workspace: &Path, sel: &DocSelection) -> Result<PathBuf> {
    let want = sel.id.without_fragment().to_string();
    let stem = format!("{}.md", sel.id.identifier);
    let class = &sel.id.class;
    let mut hits: Vec<PathBuf> = Vec::new();
    // The corpus directory for a class is its name, or its plural.
    for dir in [
        workspace.join(DECISIONS_ROOT).join(class),
        workspace.join(DECISIONS_ROOT).join(format!("{class}s")),
    ] {
        if !dir.is_dir() {
            continue;
        }
        for entry in walkdir::WalkDir::new(&dir).into_iter().filter_map(|e| e.ok()) {
            if entry.file_name().to_string_lossy() != stem {
                continue;
            }
            let Ok(text) = std::fs::read_to_string(entry.path()) else {
                continue;
            };
            // The envelope's own `id:` is the identity; the filename is a
            // convenience that a move may have left behind.
            if parse_envelope(&text).map(|(env, _)| env.id == want).unwrap_or(false) {
                hits.push(entry.path().to_path_buf());
            }
        }
    }
    match hits.len() {
        1 => Ok(hits.remove(0)),
        0 => bail!(
            "`publishes` names `{}`, and no document under {DECISIONS_ROOT}/ declares \
             `{want}` as its `id:` — a name that selects nothing is the defect",
            sel.reference
        ),
        n => bail!(
            "`publishes` names `{}`, and {n} documents declare `{want}` as their \
             `id:`: {hits:?} — an id addresses one document",
            sel.reference
        ),
    }
}
```

A section is cut by the format's own extractor, which matches the final
segment of the heading path, carries the deeper subsections under it, and
returns nothing rather than guessing. Nothing is what the refusal is
built on: an anchor that heads no section is **refused, not skipped**,
the same rule an `excludes` id matching no document already gets. The
message lists the anchors the document does offer, because the slug is
minted from the heading text and capped, and a reader who guessed the
long form should be told the short one rather than left to derive it.

```rust {#project-named-documents}
/// One named document, resolved to the markdown that enters the
/// projection and the path it is written to.
struct ProjectedDoc {
    /// Projection-relative destination.
    rel: PathBuf,
    /// Corpus-relative path of the document it was cut from — the
    /// provenance seam a receiver routes an edit back through.
    source_rel: PathBuf,
    /// The reference the publication wrote.
    reference: String,
    text: String,
}

/// Resolve every named document, cutting the sections the anchors select.
/// Runs with the guards, before anything is written: a refusal here must
/// leave no half-built repository behind.
fn project_named_documents(
    workspace: &Path,
    selections: &[DocSelection],
) -> Result<Vec<ProjectedDoc>> {
    let mut out = Vec::new();
    for sel in selections {
        let path = resolve_named_document(workspace, sel)?;
        let source_rel = path.strip_prefix(workspace).unwrap_or(&path).to_path_buf();
        let content = std::fs::read_to_string(&path)
            .with_context(|| format!("reading named document {}", path.display()))?;
        let (env, body) = parse_envelope(&content)
            .map_err(|e| anyhow!("`{}` is not a folio/v1 document: {e:?}", source_rel.display()))?;
        let Some(anchor) = sel.id.fragment.clone() else {
            out.push(ProjectedDoc {
                rel: source_rel.clone(),
                source_rel,
                reference: sel.reference.clone(),
                text: content,
            });
            continue;
        };
        let Some(section) = extract_section(&body, &anchor) else {
            bail!(
                "`publishes` names `{}`, and `{anchor}` heads no section of {} — \
                 an anchor that selects nothing is the defect, not the convenience. \
                 That document offers: {}",
                sel.reference,
                source_rel.display(),
                offered_anchors(&body).join(", ")
            );
        };
        out.push(ProjectedDoc {
            // `<parent stem>/<anchor>.md`, beside the whole document's own
            // path, so where a section came from is legible from the tree.
            rel: source_rel.with_extension("").join(format!("{anchor}.md")),
            source_rel,
            reference: sel.reference.clone(),
            text: section_document(&env, &sel.id, &section),
        });
    }
    Ok(out)
}

/// The anchors a document offers, for the refusal above. The heading scan
/// is deliberately re-derived rather than shared: this is an error
/// message, and the answer that matters — does this anchor resolve — is
/// the extractor's alone.
fn offered_anchors(body: &str) -> Vec<String> {
    body.lines()
        .filter_map(|l| {
            let t = l.trim_start();
            let hashes = t.chars().take_while(|&c| c == '#').count();
            (1..=6)
                .contains(&hashes)
                .then(|| t[hashes..].strip_prefix(' '))
                .flatten()
                .map(|h| x0k_folio::transclusion::heading_slug(h.trim()))
        })
        .collect()
}
```

A fragment carrying no envelope is not a folio document and cannot be
read as one, so a projected section takes an envelope of its own: the
parent's identity qualified by the anchor, the parent's genus and status
(a section of a proposed design is proposed), and one edge naming the
document it was cut from. The predicate is `transcludes`, which is the
same relation read the other way — the projected section says its body
came from that address, which is exactly what the publication said when
it named it. That edge dangles in the projection, the parent having
stayed in the corpus, and a dangling edge out of a published region is
the ordinary case this design already accounts for rather than a new kind
of gap.

The section's own markdown is carried at the level it was written at — an
`###` affordance under a design's `## Affordances` arrives as an `###`,
not promoted to the document's `#`. Promoting it would make the projected
file no longer the bytes that were cut, and the extractor has an
edit-through inverse ([`replace_section`](../folio/transclusion.md)) that
splices a replacement over exactly the span it took. Keeping the level is
what leaves that inverse true across the projection.

```rust {#section-document}
/// A projected section, as a document: the parent's identity qualified by
/// the anchor, the parent's genus and status, and a `transcludes` edge
/// naming the document it was cut from. That edge dangles — the parent
/// stayed in the corpus — which is what projecting a region out of a
/// larger graph means, not a defect in this file.
fn section_document(env: &Colophon, id: &EntityId, section: &str) -> String {
    let mut out = String::from("---\nx0k:\n  format: folio/v1\n");
    out.push_str(&format!("  id: {id}\n"));
    out.push_str(&format!("  type: {}\n", env.doc_type.as_str()));
    if let Some(status) = env.status {
        out.push_str(&format!("  status: {}\n", status.as_str()));
    }
    out.push_str("  edges:\n    transcludes:\n");
    out.push_str(&format!("      - {}\n", id.without_fragment()));
    out.push_str("---\n\n");
    out.push_str(section.trim_end());
    out.push('\n');
    out
}
```

Writing them is the same act as copying a literate chapter, and records
the same two things: the provenance seam (projected path → the corpus
path it was cut from) and the report line a publisher reads back.

```rust {#write-projected-documents}
/// Write the projected documents, recording each in the provenance seam
/// and in the report.
fn write_projected_documents(
    output_dir: &Path,
    docs: &[ProjectedDoc],
    path_map: &mut BTreeMap<String, String>,
    report: &mut RepoProjectReport,
) -> Result<()> {
    for doc in docs {
        let dst = output_dir.join(&doc.rel);
        if let Some(parent) = dst.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(&dst, &doc.text)
            .with_context(|| format!("writing projected document {}", dst.display()))?;
        let rel = doc.rel.to_string_lossy().to_string();
        path_map.insert(rel.clone(), doc.source_rel.to_string_lossy().to_string());
        report.documents.insert(doc.reference.clone(), rel);
        tracing::info!(reference = %doc.reference, "region_repo.document.projected");
    }
    Ok(())
}
```

A published declaration is a claim the audience reads as data, and the
crate guard above already says what closure means for this projector:
in the set, or severed by name. The same rule holds for what a
declaration names. What a publication affords is derived from the
modules it publishes, never claimed
(`x0k:architecture/publication-is-the-shipping-unit` §3), so an
affordance whose `enabledBy` names a module that is neither published nor
excluded is a declaration the audience can read and cannot exercise —
and the projector refuses it rather than ship it (§7). `excludes` is the
publication's own statement that the audience will not have that module;
the reason lives in the publication's prose, and the projector holds the
publication to the edge. This is the residency audit
`publish-a-region-as-a-repository` states as a consequence, and the check
that found `x0k-folio-daemon` straddling the x0k-folio boundary, made
mechanical. It is not downgradable, for the reason the anchor refusal is
not: a name that promises what the region does not hold is the defect.

```rust {#affordance-closure}
/// The closure rule for published declarations: every module an affordance
/// names under `enabledBy` is published, or excluded. Runs on the projected
/// documents (whole or cut to a section), before anything is written.
fn affordance_closure(
    docs: &[ProjectedDoc],
    published: &BTreeSet<String>,
    excluded: &BTreeSet<String>,
) -> Result<()> {
    let classes: HashSet<String> = HashSet::from(["affordance".to_string()]);
    let mut violations: Vec<String> = Vec::new();
    for doc in docs {
        let (_, body) = parse_envelope(&doc.text)
            .map_err(|e| anyhow!("projected document `{}` lost its envelope: {e:?}", doc.reference))?;
        for record in x0k_folio::extract_from_markdown(&body, &classes) {
            // A malformed block is the extractor's report, not this guard's.
            let Ok(entity) = record else { continue };
            for (predicate, value) in x0k_folio::declared_facts(&entity) {
                if predicate != "enabledBy" {
                    continue;
                }
                let Some(krate) = value
                    .strip_prefix("entity:")
                    .and_then(|v| v.strip_prefix("x0k:software-module/"))
                else {
                    continue;
                };
                if published.contains(krate) || excluded.contains(krate) {
                    continue;
                }
                violations.push(format!(
                    "affordance `{}` (published via `{}`) is enabledBy `{krate}`, which this \
                     publication neither publishes nor excludes — the audience would read a \
                     claim it cannot exercise",
                    entity.uri, doc.reference
                ));
            }
        }
    }
    if violations.is_empty() {
        return Ok(());
    }
    let mut msg = String::from(
        "repository projection refused — a published declaration is not closed over the crate set:\n",
    );
    for v in &violations {
        msg.push_str("  - ");
        msg.push_str(v);
        msg.push('\n');
    }
    bail!(msg);
}
```

## Each affordance, as a figure

The contents page answers *what is here*. The question a reader actually
arrives with is *what can I do with it*, and the corpus already holds
that answer as data: the affordance declarations the publication names
under `publishes`, each claimed for an actor and `enabledBy` modules,
and — since a face declares its signifier where the face lives — pointed
at by `signifier` blocks in the chapters that hold the faces. So the
README may carry a second marker, `<!-- x0k:affordances -->`, and the
projector replaces it with one figure per published declaration, drawn
from that record and nothing else. That is the whole point of drawing
it: a figure rendered from the record cannot show a face nobody declared
or a module the publication does not ship, so the picture is honest by
construction where prose would be honest by discipline.

The record is small. The declaration gives the identity, the title (its
section's heading), the status, and the actors it is claimed for — read
in both spellings the extractor has used, the vocabulary's `claimedFor →
x0k:actor/<kind>` and the older bare `actors` list, because the figure
should not care which extractor drew the facts. The modules `enabledBy`
names are each marked shipped or not; the closure guard above has
already refused any third state, so "not shipped" here means "excluded
by name". The surfaces come from signifiers: a signifier `signifies` the
affordance and is `presentedOn` a surface, and the heading it was
declared under is the cue's own name — a CLI verb's, a library
function's.

```rust {#affordance-record}
/// One affordance the publication publishes, as the figure sees it: the
/// declaration's own fields, the modules it names each marked shipped or
/// not, and the surfaces the signifiers pointing at it are presented on.
/// Nothing a figure draws is outside this record.
struct AffordanceRecord {
    /// The declared `id:`, e.g. `x0k:affordance/read_a_line`.
    id: String,
    /// The identifier, hyphenated: the stem of the figure files.
    slug: String,
    /// The heading of the section that declares it.
    title: String,
    /// The declared `status:`, when there is one.
    status: Option<String>,
    /// Actor kinds it is claimed for (`human`, `ai_agent`), in declaration
    /// order.
    actors: Vec<String>,
    /// Each module `enabledBy` names, with whether this publication ships
    /// it. The closure guard has refused any module that is neither
    /// published nor excluded, so `false` means excluded by name.
    modules: Vec<(String, bool)>,
    /// `(surface, cue)` per signifier that signifies it: the surface named
    /// by `presentedOn`, and the heading the signifier was declared under.
    surfaces: Vec<(String, String)>,
}

/// A signifier, as far as the figure needs it.
struct Signifier {
    signifies: Vec<String>,
    surfaces: Vec<String>,
    cue: String,
}

/// The `entity:` targets of `predicate` among an entity's facts, with
/// `prefix` stripped — a bare crate, surface or actor name.
fn entity_targets(facts: &[(String, String)], predicate: &str, prefix: &str) -> Vec<String> {
    facts
        .iter()
        .filter(|(p, _)| p == predicate)
        .filter_map(|(_, v)| v.strip_prefix("entity:"))
        .filter_map(|v| v.strip_prefix(prefix))
        .map(str::to_string)
        .collect()
}

/// The record of one declaration. Actors are read in both spellings the
/// extractor has used — `claimedFor → x0k:actor/<kind>`, the vocabulary's
/// word, and the older bare `actors` list — so the figure does not care
/// which extractor drew the facts.
fn affordance_record(entity: &InlineEntity, published: &BTreeSet<String>) -> AffordanceRecord {
    let facts = x0k_folio::declared_facts(entity);
    let mut actors: Vec<String> = Vec::new();
    for (predicate, value) in &facts {
        let kind = match predicate.as_str() {
            "claimedFor" => value.strip_prefix("entity:x0k:actor/"),
            "actors" | "x0k:affordance/actors" => value.strip_prefix("string:"),
            _ => None,
        };
        if let Some(kind) = kind.filter(|k| !actors.iter().any(|a| a == k)) {
            actors.push(kind.to_string());
        }
    }
    let status = facts
        .iter()
        .find(|(p, _)| p == "status" || p == "x0k:affordance/status")
        .and_then(|(_, v)| v.strip_prefix("string:"))
        .map(str::to_string);
    let modules = entity_targets(&facts, "enabledBy", SOFTWARE_MODULE_PREFIX)
        .into_iter()
        .map(|m| {
            let shipped = published.contains(&m);
            (m, shipped)
        })
        .collect();
    AffordanceRecord {
        id: entity.uri.to_string(),
        slug: entity.uri.identifier.replace('_', "-"),
        title: entity.title.clone(),
        status,
        actors,
        modules,
        surfaces: Vec::new(),
    }
}
```

Where signifiers are looked for is the one decision with reach.
Affordances come from the named documents alone: they are what the
publication chose to publish, in the order it named them. Signifiers
come from those documents *and* from every literate chapter in the
projection, because the cue for a face is declared in the chapter that
holds the face — a tangler verb is signified in the chapter that defines
it — and that chapter ships with its crate, not by name. A signifier in
a chapter the projection does not carry is a cue the audience cannot
see, and so it is not drawn.

```rust {#affordance-records}
/// Read the declarations in one body: affordances when `declarations` is
/// on, signifiers always. A malformed block is the extractor's report, not
/// the figure's, and is skipped here as the closure guard skips it.
fn read_declarations(
    body: &str,
    declarations: bool,
    published: &BTreeSet<String>,
    records: &mut Vec<AffordanceRecord>,
    signifiers: &mut Vec<Signifier>,
) {
    let classes: HashSet<String> =
        HashSet::from(["affordance".to_string(), "signifier".to_string()]);
    for record in x0k_folio::extract_from_markdown(body, &classes) {
        let Ok(entity) = record else { continue };
        match entity.marker_class.as_str() {
            "affordance" if declarations => records.push(affordance_record(&entity, published)),
            "signifier" => {
                let facts = x0k_folio::declared_facts(&entity);
                // The cue is the heading the block sits under, unless the
                // block names it: a chapter's heading is often a sentence
                // about the function, and the cue a person reaches for is
                // the function's name.
                let cue = facts
                    .iter()
                    .find(|(p, _)| p == "x0k:signifier/cue")
                    .and_then(|(_, v)| v.strip_prefix("string:"))
                    .map(str::to_string)
                    .unwrap_or_else(|| entity.title.clone());
                signifiers.push(Signifier {
                    signifies: entity_targets(&facts, "signifies", ""),
                    surfaces: entity_targets(&facts, "presentedOn", "x0k:surface/"),
                    cue,
                });
            }
            _ => {}
        }
    }
}

/// The affordance records the figures are drawn from: the declarations in
/// the named documents, in `publishes` order, each joined to the
/// signifiers — from those documents and from every literate chapter —
/// that point at it. Two declarations with one id would draw over each
/// other's files, so that refuses.
fn affordance_records(
    docs: &[ProjectedDoc],
    workspace: &Path,
    literate: &[LiterateDoc],
    published: &BTreeSet<String>,
) -> Result<Vec<AffordanceRecord>> {
    let mut records: Vec<AffordanceRecord> = Vec::new();
    let mut signifiers: Vec<Signifier> = Vec::new();
    for doc in docs {
        let (_, body) = parse_envelope(&doc.text)
            .map_err(|e| anyhow!("projected document `{}` lost its envelope: {e:?}", doc.reference))?;
        read_declarations(&body, true, published, &mut records, &mut signifiers);
    }
    for doc in literate {
        let text = std::fs::read_to_string(workspace.join(&doc.rel))
            .with_context(|| format!("reading literate doc {}", doc.rel.display()))?;
        if let Some((_, body)) = split_frontmatter(&text) {
            read_declarations(body, false, published, &mut records, &mut signifiers);
        }
    }
    let mut seen: BTreeSet<&str> = BTreeSet::new();
    for record in &records {
        if !seen.insert(record.id.as_str()) {
            bail!(
                "affordance `{}` is declared twice among the published documents — one id, \
                 one figure",
                record.id
            );
        }
    }
    for s in &signifiers {
        for record in records.iter_mut().filter(|r| s.signifies.contains(&r.id)) {
            for surface in &s.surfaces {
                record.surfaces.push((surface.clone(), s.cue.clone()));
            }
        }
    }
    Ok(records)
}
```

The caption is written twice from the same sentence — once as Markdown
under the picture, once as plain text for the image's `alt` and the
SVG's `aria-label` — so a reader who cannot see the figure is told
exactly what it shows, and told it in the order the figure reads: who it
is for, how they reach it, what makes it true, and how far along it is.

```rust {#affordance-caption}
/// What an actor kind is called in the caption: the two the corpus
/// declares by name, and any other as itself.
fn actor_phrase(kind: &str) -> String {
    match kind {
        "human" => "a person".to_string(),
        "ai_agent" => "an agent".to_string(),
        other => other.to_string(),
    }
}

/// The caption, from the record and nothing else. `markdown` sets the
/// title bold and the names in code spans for the line under the picture;
/// plain, it is the image's `alt` and the SVG's `aria-label`.
fn affordance_caption(rec: &AffordanceRecord, markdown: bool) -> String {
    let code = |s: &str| if markdown { format!("`{s}`") } else { s.to_string() };
    let title = if markdown { format!("**{}**", rec.title) } else { rec.title.clone() };
    let actors = if rec.actors.is_empty() {
        "claimed for no actor".to_string()
    } else {
        let each: Vec<String> = rec.actors.iter().map(|k| format!("for {}", actor_phrase(k))).collect();
        each.join(", ")
    };
    let surfaces = if rec.surfaces.is_empty() {
        "no signifier declared".to_string()
    } else {
        let each: Vec<String> = rec
            .surfaces
            .iter()
            .map(|(surface, cue)| format!("{} as {}", code(surface), code(cue)))
            .collect();
        format!("reachable on {}", each.join(", "))
    };
    let modules = if rec.modules.is_empty() {
        "enabled by no named module".to_string()
    } else {
        let each: Vec<String> = rec
            .modules
            .iter()
            .map(|(name, shipped)| {
                if *shipped {
                    code(name)
                } else {
                    format!("{} (excluded here)", code(name))
                }
            })
            .collect();
        format!("enabled by {}", each.join(", "))
    };
    let status = rec.status.as_deref().unwrap_or("undeclared");
    format!("{title} — {actors}; {surfaces}; {modules}; status {status}.")
}
```

The figure borrows the README's own plates: their two palettes (paper,
gold and ink under a serif; a dark surface, cyan and slate under a
monospace) and their three glyphs — the eye for a person, the machine
for an agent, the sheet with a folded corner for a module. Three
columns, left to right, read as the caption does: the actors the
affordance is claimed for, the surfaces it is reachable through (one
pill per signifier, the surface bold and the cue after it — set in a `text`
element told to preserve its spaces, because a renderer otherwise folds the
space at a `tspan`'s edge and the cue runs into the surface — measured
with librsvg, where a non-breaking space folded the same way), and the
modules that enable it, an excluded one drawn dashed and said so. An
affordance no signifier points at gets a dashed pill saying so, because
an undeclared path is a fact about the record — and, for a human claim,
the defect the shipped checker reports.

```rust {#affordance-palette}
/// One of the two palettes the README's plates set. `label_style` is the
/// attribute the plates give their small labels — italic on paper,
/// upright in monospace.
struct Palette {
    font: &'static str,
    ink: &'static str,
    sheet: &'static str,
    stroke: &'static str,
    rule: &'static str,
    fill: &'static str,
    iris: &'static str,
    arrow: &'static str,
    label: &'static str,
    label_style: &'static str,
    accent: &'static str,
}

/// Paper, gold and ink, under a serif: the light plates.
const LIGHT: Palette = Palette {
    font: "Georgia,'Palatino Linotype',Palatino,serif",
    ink: "#111111",
    sheet: "#fffff8",
    stroke: "#b88e44",
    rule: "#d9d0b6",
    fill: "#ede4c6",
    iris: "#f6efd8",
    arrow: "#8e6a30",
    label: "#8e6a30",
    label_style: " font-style=\"italic\"",
    accent: "#b22222",
};

/// A dark surface, cyan and slate, under a monospace: the dark plates.
const DARK: Palette = Palette {
    font: "ui-monospace,'IBM Plex Mono',Menlo,monospace",
    ink: "#e2e8f0",
    sheet: "#1e1e2a",
    stroke: "#3a3a4e",
    rule: "#2f2f42",
    fill: "#2a2a3a",
    iris: "#2a2a3a",
    arrow: "#96b4dc",
    label: "#94a3b8",
    label_style: "",
    accent: "#22d3ee",
};

/// Escape text for an XML attribute or text node.
fn xml_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

/// The eye from the plates, centred at `(cx, cy)`: a person.
fn eye_glyph(cx: f64, cy: f64, p: &Palette) -> String {
    format!(
        "<path d=\"M{x0},{cy} C{x1},{yt} {x2},{yt} {x3},{cy} C{x2},{yb} {x1},{yb} {x0},{cy} Z\" \
         fill=\"{sheet}\" stroke=\"{stroke}\" stroke-width=\"1.3\"/>\n\
         <circle cx=\"{cx}\" cy=\"{cy}\" r=\"10\" fill=\"{iris}\" stroke=\"{stroke}\" stroke-width=\"1.1\"/>\n\
         <circle cx=\"{cx}\" cy=\"{cy}\" r=\"4\" fill=\"{ink}\"/>\n\
         <circle cx=\"{hx}\" cy=\"{hy}\" r=\"1.4\" fill=\"{sheet}\"/>\n",
        x0 = cx - 32.0,
        x1 = cx - 16.0,
        x2 = cx + 16.0,
        x3 = cx + 32.0,
        yt = cy - 20.0,
        yb = cy + 20.0,
        hx = cx + 3.6,
        hy = cy - 3.6,
        sheet = p.sheet,
        stroke = p.stroke,
        iris = p.iris,
        ink = p.ink,
    )
}

/// The machine from the plates — a row of chips with a token stepping
/// across them — centred at `(cx, cy)`: an agent, or any structured actor.
fn machine_glyph(cx: f64, cy: f64, p: &Palette) -> String {
    let mut s = String::new();
    let x0 = cx - 35.0;
    for i in 0..4 {
        let x = x0 + 18.0 * i as f64;
        s.push_str(&format!(
            "<rect x=\"{x}\" y=\"{y}\" width=\"16\" height=\"16\" fill=\"{sheet}\" stroke=\"{stroke}\" stroke-width=\"1.1\"/>\n",
            y = cy - 6.0,
            sheet = p.sheet,
            stroke = p.stroke,
        ));
        if i != 1 {
            s.push_str(&format!(
                "<rect x=\"{x}\" y=\"{cy}\" width=\"8\" height=\"3\" fill=\"{fill}\"/>\n",
                x = x + 4.0,
                fill = p.fill,
            ));
        }
    }
    s.push_str(&format!(
        "<g><path d=\"M{x},{y} h10 v5 l-5,5 l-5,-5 z\" fill=\"{accent}\" opacity=\"0.85\"/>\
         <animateTransform attributeName=\"transform\" type=\"translate\" calcMode=\"discrete\" \
         values=\"0,0;18,0;36,0;54,0;36,0;18,0\" dur=\"4s\" repeatCount=\"indefinite\"/></g>\n",
        x = x0 + 3.0,
        y = cy - 22.0,
        accent = p.accent,
    ));
    s
}

/// The sheet with a folded corner from the plates, top-left at `(x, y)`,
/// 40 by 56: a module. Dashed when the publication excludes it.
fn sheet_glyph(x: f64, y: f64, p: &Palette, dashed: bool) -> String {
    let dash = if dashed { " stroke-dasharray=\"4 3\"" } else { "" };
    let inner = if dashed {
        format!("fill=\"none\" stroke=\"{}\" stroke-dasharray=\"3 2\"", p.rule)
    } else {
        format!("fill=\"{}\"", p.fill)
    };
    format!(
        "<path d=\"M{x},{y} h30 l10,10 v46 h-40 z\" fill=\"{sheet}\" stroke=\"{stroke}\" stroke-width=\"1.2\"{dash}/>\n\
         <path d=\"M{fx},{y} v10 h10\" fill=\"none\" stroke=\"{stroke}\" stroke-width=\"1.2\"{dash}/>\n\
         <rect x=\"{ix}\" y=\"{y1}\" width=\"24\" height=\"6\" {inner}/>\n\
         <line x1=\"{ix}\" y1=\"{y2}\" x2=\"{ex}\" y2=\"{y2}\" stroke=\"{rule}\" stroke-width=\"2\"/>\n\
         <rect x=\"{ix}\" y=\"{y3}\" width=\"24\" height=\"6\" {inner}/>\n",
        fx = x + 30.0,
        ix = x + 8.0,
        ex = x + 32.0,
        y1 = y + 20.0,
        y2 = y + 32.0,
        y3 = y + 38.0,
        sheet = p.sheet,
        stroke = p.stroke,
        rule = p.rule,
    )
}
```

Each column is spread evenly down one band whose height is set by the
longest of the three, so a claim for two actors reachable through one
verb and enabled by three crates reads as two, one and three items
centred against each other rather than as a ragged table. Arrows run
from every actor to every pill and from every pill to every sheet; with
the small numbers a declaration carries that is a fan, not a thicket,
and it is dashed through a pill that stands for no signifier, because
that path is exactly the one nobody has declared.

```rust {#affordance-figure}
/// Draw one affordance's figure in one palette: 880 wide, as tall as its
/// longest column needs. Title and id across the top; then three columns
/// — actor glyphs, one surface pill per signifier, one module sheet per
/// `enabledBy` — each spread evenly down the band and joined left to
/// right by the plates' arrows; the status bottom right.
fn affordance_figure(rec: &AffordanceRecord, p: &Palette) -> String {
    const W: f64 = 880.0;
    const BAND_Y: f64 = 92.0;
    const ROW_H: f64 = 76.0;
    let surfaces: Vec<Option<&(String, String)>> = if rec.surfaces.is_empty() {
        vec![None]
    } else {
        rec.surfaces.iter().map(Some).collect()
    };
    let rows = rec.actors.len().max(surfaces.len()).max(rec.modules.len()).max(1);
    let band = rows as f64 * ROW_H;
    let height = BAND_Y + band + 34.0;
    // The centre line of item `i` in a column of `n`, spread down the band.
    let centre = |i: usize, n: usize| BAND_Y + band * (i as f64 + 0.5) / n as f64;
    let caption = xml_escape(&affordance_caption(rec, false));
    let label = format!("font-size=\"12\" fill=\"{}\"{}", p.label, p.label_style);

    let mut s = format!(
        "<svg xmlns=\"http://www.w3.org/2000/svg\" viewBox=\"0 0 {W} {height}\" width=\"{W}\" \
         height=\"{height}\" role=\"img\" aria-label=\"{caption}\">\n\
         <defs><marker id=\"a\" viewBox=\"0 0 10 10\" refX=\"9\" refY=\"5\" markerWidth=\"7\" \
         markerHeight=\"7\" orient=\"auto-start-reverse\"><path d=\"M0,0 L10,5 L0,10 z\" \
         fill=\"{arrow}\"/></marker></defs>\n\
         <g font-family=\"{font}\">\n",
        arrow = p.arrow,
        font = p.font,
    );
    // Title, id, and a rule under both; then the column headings.
    s.push_str(&format!(
        "<text x=\"24\" y=\"34\" font-size=\"20\" fill=\"{}\">{}</text>\n",
        p.ink,
        xml_escape(&rec.title)
    ));
    s.push_str(&format!("<text x=\"24\" y=\"52\" {label}>{}</text>\n", xml_escape(&rec.id)));
    s.push_str(&format!(
        "<line x1=\"24\" y1=\"62\" x2=\"{}\" y2=\"62\" stroke=\"{}\" stroke-width=\"0.8\"/>\n",
        W - 24.0,
        p.stroke
    ));
    s.push_str(&format!("<text x=\"40\" y=\"80\" {label}>claimed for</text>\n"));
    s.push_str(&format!(
        "<text x=\"430\" y=\"80\" text-anchor=\"middle\" {label}>reachable through</text>\n"
    ));
    s.push_str(&format!("<text x=\"636\" y=\"80\" {label}>enabled by</text>\n"));

    // Actors: a glyph and its name.
    let mut actor_ys: Vec<f64> = Vec::new();
    for (i, kind) in rec.actors.iter().enumerate() {
        let cy = centre(i, rec.actors.len());
        actor_ys.push(cy);
        s.push_str(&if kind == "human" {
            eye_glyph(72.0, cy, p)
        } else {
            machine_glyph(72.0, cy, p)
        });
        s.push_str(&format!(
            "<text x=\"116\" y=\"{}\" font-size=\"14\" fill=\"{}\">{}</text>\n",
            cy + 5.0,
            p.ink,
            xml_escape(&actor_phrase(kind))
        ));
    }
    // Surfaces: one pill per signifier, or one dashed pill saying there is none.
    let mut pills: Vec<(f64, f64, f64, &str)> = Vec::new();
    for (i, surface) in surfaces.iter().enumerate() {
        let cy = centre(i, surfaces.len());
        let (text, chars, dash) = match surface {
            Some((name, cue)) => (
                format!(
                    "<tspan font-weight=\"bold\">{}</tspan><tspan fill=\"{}\">  ·  {}</tspan>",
                    xml_escape(name),
                    p.label,
                    xml_escape(cue)
                ),
                name.chars().count() + cue.chars().count() + 3,
                "",
            ),
            None => (
                format!("<tspan fill=\"{}\"{}>no signifier declared</tspan>", p.label, p.label_style),
                21,
                " stroke-dasharray=\"4 3\"",
            ),
        };
        let w = (chars as f64 * 8.4 + 28.0).clamp(140.0, 260.0);
        let x = 430.0 - w / 2.0;
        pills.push((x, x + w, cy, dash));
        s.push_str(&format!(
            "<rect x=\"{x}\" y=\"{}\" width=\"{w}\" height=\"34\" rx=\"17\" fill=\"{}\" stroke=\"{}\" stroke-width=\"1.2\"{dash}/>\n",
            cy - 17.0,
            p.sheet,
            p.stroke
        ));
        s.push_str(&format!(
            "<text xml:space=\"preserve\" x=\"430\" y=\"{}\" text-anchor=\"middle\" font-size=\"14\" fill=\"{}\">{text}</text>\n",
            cy + 5.0,
            p.ink
        ));
    }
    // Modules: a sheet and the crate name; an excluded one dashed and said so.
    let mut module_ys: Vec<f64> = Vec::new();
    for (i, (name, shipped)) in rec.modules.iter().enumerate() {
        let cy = centre(i, rec.modules.len());
        module_ys.push(cy);
        s.push_str(&sheet_glyph(636.0, cy - 28.0, p, !shipped));
        s.push_str(&format!(
            "<text x=\"690\" y=\"{}\" font-size=\"14\" fill=\"{}\">{}</text>\n",
            cy + 5.0,
            p.ink,
            xml_escape(name)
        ));
        if !shipped {
            s.push_str(&format!("<text x=\"690\" y=\"{}\" {label}>excluded here</text>\n", cy + 22.0));
        }
    }
    // Arrows: every actor to every pill, every pill to every sheet.
    for &ay in &actor_ys {
        for &(px, _, py, dash) in &pills {
            s.push_str(&format!(
                "<line x1=\"200\" y1=\"{ay}\" x2=\"{}\" y2=\"{py}\" stroke=\"{}\" stroke-width=\"1.2\"{dash} marker-end=\"url(#a)\"/>\n",
                px - 6.0,
                p.arrow
            ));
        }
    }
    for &(_, px, py, dash) in &pills {
        for &my in &module_ys {
            s.push_str(&format!(
                "<line x1=\"{}\" y1=\"{py}\" x2=\"628\" y2=\"{my}\" stroke=\"{}\" stroke-width=\"1.2\"{dash} marker-end=\"url(#a)\"/>\n",
                px + 6.0,
                p.arrow
            ));
        }
    }
    s.push_str(&format!(
        "<text x=\"{}\" y=\"{}\" text-anchor=\"end\" {label}>status: {}</text>\n",
        W - 24.0,
        height - 14.0,
        xml_escape(rec.status.as_deref().unwrap_or("undeclared"))
    ));
    s.push_str("</g>\n</svg>\n");
    s
}
```

The marker is opt-in, as the contents marker is, and refused on the same
principle when it would render nothing: a README asking for figures in a
publication that names no declaration is the marker that silently says
less than it promises. The figures are written under `affordances/` at
the projection root — not `docs/`, where the carried example keeps its
plates, because `docs` is an overlay path the public side owns and the
projector never regenerates, and a generated figure is precisely the
kind of file that must be regenerated on every publish. Each figure is
recorded in the report by affordance id; none goes in
`PROVENANCE.json`'s `path_map`, which routes edits back to corpus
sources, and a figure has none.

```rust {#write-readme-affordances}
/// The marker a README carries where its affordance figures go. Bare, on
/// its own line; opt-in like the contents marker, and refused when it
/// would render nothing.
const AFFORDANCES_MARKER: &str = "<!-- x0k:affordances -->";
/// Projection-relative directory the figures are written to. Not `docs/`:
/// in the carried example that is an overlay path the public side owns
/// and the projector never regenerates, and a generated figure is the
/// opposite kind of file.
const AFFORDANCES_DIR: &str = "affordances";

/// The README line the affordances marker occupies, if any. Two refuse:
/// the figures have one home.
fn find_affordances_marker(lines: &[&str]) -> Result<Option<usize>> {
    let hits: Vec<usize> = lines
        .iter()
        .enumerate()
        .filter(|(_, l)| l.trim() == AFFORDANCES_MARKER)
        .map(|(i, _)| i)
        .collect();
    match hits[..] {
        [] => Ok(None),
        [at] => Ok(Some(at)),
        _ => bail!(
            "the README carries {} `{AFFORDANCES_MARKER}` markers; the affordance figures \
             have exactly one home",
            hits.len()
        ),
    }
}

/// Draw the figures and replace the README's affordances marker with them:
/// per affordance, a `<picture>` choosing the dark or light figure by the
/// reader's colour scheme, then the caption. No marker draws nothing; a
/// marker with nothing to draw refuses, naming itself.
fn write_readme_affordances(
    output_dir: &Path,
    records: &[AffordanceRecord],
    report: &mut RepoProjectReport,
) -> Result<()> {
    let path = output_dir.join("README.md");
    let text = std::fs::read_to_string(&path).context("reading the tangled README")?;
    let lines: Vec<&str> = text.split_inclusive('\n').collect();
    let Some(at) = find_affordances_marker(&lines)? else {
        return Ok(());
    };
    if records.is_empty() {
        bail!(
            "the README carries `{AFFORDANCES_MARKER}`, and the publication names no \
             affordance declaration to draw there — a marker that renders nothing is the \
             defect; name the sections that declare them under `publishes`, or drop the marker"
        );
    }
    std::fs::create_dir_all(output_dir.join(AFFORDANCES_DIR))?;
    let mut section = String::new();
    for (i, rec) in records.iter().enumerate() {
        let light = format!("{AFFORDANCES_DIR}/{}-light.svg", rec.slug);
        let dark = format!("{AFFORDANCES_DIR}/{}-dark.svg", rec.slug);
        std::fs::write(output_dir.join(&light), affordance_figure(rec, &LIGHT))
            .with_context(|| format!("writing affordance figure {light}"))?;
        std::fs::write(output_dir.join(&dark), affordance_figure(rec, &DARK))
            .with_context(|| format!("writing affordance figure {dark}"))?;
        report.figures.insert(rec.id.clone(), light.clone());
        tracing::info!(affordance = %rec.id, figure = %light, "region_repo.figure.written");
        if i > 0 {
            section.push('\n');
        }
        section.push_str(&format!(
            "<picture>\n  <source media=\"(prefers-color-scheme: dark)\" srcset=\"{dark}\">\n  \
             <img alt=\"{}\" src=\"{light}\">\n</picture>\n\n{}\n",
            xml_escape(&affordance_caption(rec, false)),
            affordance_caption(rec, true)
        ));
    }
    let mut out = String::new();
    out.extend(lines[..at].iter().copied());
    out.push_str(&section);
    out.extend(lines[at + 1..].iter().copied());
    std::fs::write(&path, out).context("writing the README with its affordance figures")?;
    Ok(())
}
```

## Scaffolding

The workspace manifest lists the published crates, declares the
edition and toolchain floor every crate inherits, and resolves the
inherited dependency keys from the fixed table above.

```rust {#emit-workspace-manifest}
fn emit_workspace_manifest(output_dir: &Path, crates: &[String], edition: &str) -> Result<()> {
    let inherited = inherited_dep_keys(output_dir, crates)?;
    let mut s = String::from("[workspace]\nresolver = \"2\"\nmembers = [\n");
    for c in crates {
        s.push_str(&format!("    \"{c}\",\n"));
    }
    s.push_str("]\n\n[workspace.package]\n");
    s.push_str(&format!("edition = \"{edition}\"\nrust-version = \"{RUST_VERSION}\"\n"));
    s.push_str("\n[workspace.dependencies]\n");
    for (k, v) in RESOLVED_WORKSPACE_DEPS {
        if inherited.contains(*k) {
            s.push_str(&format!("{k} = {v}\n"));
        }
    }
    std::fs::write(output_dir.join("Cargo.toml"), s)?;
    Ok(())
}

/// The inherited dependency keys the vendored crates actually reference
/// (`<key>.workspace = true`), across all three dependency tables.
///
/// Read back off the vendored manifests rather than tracked through the
/// vendoring pass, because the vendored manifest is the authority on what
/// the projection asks for: it is the file cargo will read.
fn inherited_dep_keys(output_dir: &Path, crates: &[String]) -> Result<BTreeSet<String>> {
    let mut keys = BTreeSet::new();
    for name in crates {
        let path = output_dir.join(name).join("Cargo.toml");
        let text = std::fs::read_to_string(&path)
            .with_context(|| format!("reading {}", path.display()))?;
        let doc = text
            .parse::<toml_edit::DocumentMut>()
            .with_context(|| format!("parsing {}", path.display()))?;
        for table in ["dependencies", "dev-dependencies", "build-dependencies"] {
            let Some(deps) = doc.get(table).and_then(|d| d.as_table_like()) else {
                continue;
            };
            for (key, item) in deps.iter() {
                let inherits = item
                    .as_table_like()
                    .and_then(|t| t.get("workspace"))
                    .and_then(|w| w.as_bool())
                    .unwrap_or(false);
                if inherits {
                    keys.insert(key.to_string());
                }
            }
        }
    }
    Ok(keys)
}
```

The license bodies follow from the SPDX expression, read as an
expression: identifiers joined by `OR` / `AND`, optionally
parenthesized. Each identifier maps to exactly one file — `MIT` to
`LICENSE-MIT`, `Apache-2.0` to `LICENSE-APACHE`, `MPL-2.0` to
`LICENSE-MPL` — and an identifier with no entry in that table is a
refusal, never a silently missing body: a manifest saying `MIT-0` or
`LicenseRef-Something` with no text behind it would be a repository
lying about its terms. The earlier substring match (`contains("MIT")`)
is exactly what the table replaces; it would have let `MIT-0` through
as MIT and `Apache` trip on any identifier that mentioned it. `WITH`
exceptions are refused for the same reason — the projector carries no
exception texts. The projector never fakes a license body either:
MIT's text is short and canonical and is emitted verbatim; Apache-2.0
and MPL-2.0 get a clearly-marked placeholder naming the canonical
text, filled in by the maintainer at the ratify-and-publish step.

MIT's text opens with a copyright line, and a copyright line names a
holder: the envelope's `copyright:` key. Its absence under `MIT` is a
refusal — a `LICENSE-MIT` with the line missing is a license file
nobody can attribute — and the year is the projection year, because
the notice attaches to the act of publishing, not to the corpus
revision the act projected.

```rust {#license-files}
/// `(file name, body)` for every license identifier in the SPDX
/// expression `expr`. Parsed as an expression — identifiers joined by
/// `OR` / `AND`, parentheses tolerated — not substring-matched, so `MIT`
/// never trips `Apache` and `MIT-0` is not `MIT`. An identifier with no
/// body here, or a `WITH` exception, refuses: a projection must never
/// carry a license field its tree has no text for. `MIT` needs the
/// `copyright` holder for its notice line; without one it refuses too.
fn license_files(
    expr: &str,
    copyright: Option<&str>,
    year: i32,
) -> Result<Vec<(&'static str, String)>> {
    let mut out: Vec<(&'static str, String)> = Vec::new();
    let mut expect_exception = false;
    for token in expr
        .split_whitespace()
        .map(|t| t.trim_matches(|c| c == '(' || c == ')'))
        .filter(|t| !t.is_empty())
    {
        if expect_exception {
            bail!(
                "license expression `{expr}`: `WITH {token}` names a license exception, \
                 and the projector carries no exception texts — declare a plain SPDX \
                 identifier the projector has a body for"
            );
        }
        match token {
            "OR" | "AND" => continue,
            "WITH" => {
                expect_exception = true;
                continue;
            }
            _ => {}
        }
        let entry = match token {
            "MIT" => {
                let Some(holder) = copyright.map(str::trim).filter(|h| !h.is_empty()) else {
                    bail!(
                        "license expression `{expr}`: MIT's notice line names a copyright \
                         holder, and the publication declares no `copyright:` — add one to \
                         the envelope"
                    );
                };
                ("LICENSE-MIT", MIT_LICENSE.replace("{copyright}", &format!("{year} {holder}")))
            }
            // The projector never fakes a license body: Apache-2.0 and
            // MPL-2.0 get a clearly-marked placeholder naming the canonical
            // text, filled in by the maintainer at the ratify-and-publish
            // step (the publication stays `proposed`).
            "Apache-2.0" => ("LICENSE-APACHE", APACHE_PLACEHOLDER.to_string()),
            "MPL-2.0" => ("LICENSE-MPL", MPL_PLACEHOLDER.to_string()),
            other => bail!(
                "license expression `{expr}`: no license body for identifier `{other}` \
                 (known: MIT, Apache-2.0, MPL-2.0) — the projector never emits a license \
                 field its tree has no text for"
            ),
        };
        if !out.iter().any(|(f, _)| *f == entry.0) {
            out.push(entry);
        }
    }
    if out.is_empty() {
        bail!("license expression `{expr}` names no license identifier");
    }
    Ok(out)
}

/// The projection year, from the system clock (UTC): the year the
/// license notice is dated.
fn current_year() -> i32 {
    let secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0);
    // Civil-from-days (Howard Hinnant), enough for the year alone.
    let z = secs.div_euclid(86_400) + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    (if m <= 2 { y + 1 } else { y }) as i32
}
```

Every license body is written at the repository root and again into
each published crate's directory: `cargo package` builds a tarball from
the crate directory alone, and a crates.io tarball with a `license`
field and no license text is the same lie as a repository without one.

```rust {#emit-licenses}
fn emit_licenses(output_dir: &Path, crates: &[String], bodies: &[(&str, String)]) -> Result<()> {
    for (file, body) in bodies {
        std::fs::write(output_dir.join(file), body)?;
        for c in crates {
            std::fs::write(output_dir.join(c).join(file), body)?;
        }
    }
    Ok(())
}
```

The lockfile comes from cargo itself: `cargo generate-lockfile`
resolves the workspace without building anything. The target directory
is a private one under the projection's `target/` (git-ignored, never
cleared) so the resolve leaves nothing in the tree, and a registry
that cannot be reached falls back to `--offline` — a warm local index
resolves the same set — before the run refuses.

```rust {#generate-lockfile}
/// Write `Cargo.lock` for the projected workspace with `cargo
/// generate-lockfile` (no build; a private target dir). Tries the
/// network first, then `--offline` against the local registry index.
fn generate_lockfile(output_dir: &Path) -> Result<()> {
    let cargo = std::env::var_os("CARGO").unwrap_or_else(|| "cargo".into());
    let target = output_dir.join("target/.lockfile-resolve");
    let run = |offline: bool| -> Result<std::process::Output> {
        let mut cmd = std::process::Command::new(&cargo);
        cmd.arg("generate-lockfile")
            .current_dir(output_dir)
            .env("CARGO_TARGET_DIR", &target);
        if offline {
            cmd.arg("--offline");
        }
        cmd.output().with_context(|| {
            format!(
                "running `{} generate-lockfile` (cargo must be on PATH or named by $CARGO)",
                cargo.to_string_lossy()
            )
        })
    };
    let out = run(false)?;
    let out = if out.status.success() { out } else { run(true)? };
    let _ = std::fs::remove_dir_all(&target);
    let _ = std::fs::remove_dir(output_dir.join("target"));
    if !out.status.success() {
        bail!(
            "`cargo generate-lockfile` failed in {} (online, then --offline):\n{}",
            output_dir.display(),
            String::from_utf8_lossy(&out.stderr)
        );
    }
    tracing::info!(dir = %output_dir.display(), "region_repo.lockfile.generated");
    Ok(())
}
```

The README is the entry document the design asks for, and it is
**tangled, not templated**. The publication document carries a
`tangle:` block whose `root:` is `README.md` — with no `crate:`, the
identity pipeline resolves that relative to the tangle workspace — and
a named markdown chunk in its body that is the whole file. The
projector copies the publication doc into the projection at its
corpus-relative path, tangles it with the projection directory as the
workspace, and removes the copy again. The copy is what makes the
tangler's own machinery produce the right bytes without any special
case: the `@generated` header names the corpus path
(`from decisions/publications/<stem>.md`) because that is where the
copy sat relative to the workspace root; the dispatcher's containment
check accepts `README.md` because it lands under the root; and the
dispatcher's publication guard accepts the run because
`PROVENANCE.json` is already there. Tangling the corpus's own file in
place would fail both ways — the header would carry an absolute
checkout path, and the sidecar would be written beside the doc, into
the corpus.

The same tangle may *seed* the overlay. A README that links
`CONTRIBUTING.md` links an overlay path — one the public side owns and
that does not exist on a first projection — so the publication doc may
carry further chunks routed to declared overlay paths
(`{#contributing file="CONTRIBUTING.md"}`), and the projector writes
each one only when the path is absent. Absent is the whole rule: a
seed is a first draft handed to the public side, and from the moment
it exists the projector never touches it again — an existing file at
the path is put back over the tangle's output here, and on a
re-projection the stash restore does the same for anything the
public side kept. A seed also loses its `@generated` header line: the
public side owns the file, and the projection's own
`tools/x0k-guard-generated` keys on exactly that line, so a seed that
kept it would refuse the overlay's first contribution. Outputs are
checked against the declared set — `README.md` plus the overlay, an
exact path or a path under an overlay directory — and anything else
is a refusal: a chunk routed to a regenerated path would be a second
writer for a file the crates or the corpus already own.

```rust {#tangle-readme}
/// Tangle the publication doc's own `tangle:` block into the projection:
/// its `root: README.md` chunk becomes `<output_dir>/README.md`, and any
/// chunk routed (`file="…"`) to a declared overlay path seeds that path
/// when it is absent. The doc is copied to its corpus-relative path inside
/// the projection for the duration of the tangle so the `@generated`
/// header names that path and every write stays under the projection
/// root; the copy and its sidecar are removed afterwards (the publication
/// doc is corpus-private, and `tools/ci` in the projection never
/// re-tangles the README).
fn tangle_publication_doc(
    region_doc: &Path,
    workspace: &Path,
    output_dir: &Path,
    overlay: &[String],
) -> Result<()> {
    let rel = region_doc
        .canonicalize()
        .ok()
        .and_then(|abs| {
            workspace
                .canonicalize()
                .ok()
                .and_then(|ws| abs.strip_prefix(&ws).ok().map(|r| r.to_path_buf()))
        })
        .unwrap_or_else(|| {
            Path::new("decisions/publications")
                .join(region_doc.file_name().unwrap_or_default())
        });
    let copy = output_dir.join(&rel);
    if let Some(parent) = copy.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::copy(region_doc, &copy)
        .with_context(|| format!("staging {} for the README tangle", rel.display()))?;

    // Overlay files present before the tangle are the public side's; the
    // tangle may overwrite them, and they go back afterwards.
    let is_overlay = |p: &Path| {
        overlay.iter().any(|o| {
            let o = Path::new(o);
            p == o || p.starts_with(o)
        })
    };
    let mut kept: Vec<(PathBuf, Vec<u8>)> = Vec::new();
    for entry in overlay {
        let path = output_dir.join(entry);
        if path.is_file() {
            kept.push((PathBuf::from(entry), std::fs::read(&path)?));
        }
    }

    let tangled = tangle_document(&copy, output_dir, &PipelineRegistry::default());

    // The staged copy and its sidecar never ship, whatever the tangle did.
    let _ = std::fs::remove_file(&copy);
    let _ = std::fs::remove_file(copy.with_extension("tangle-map.json"));
    let mut dir = copy.parent();
    while let Some(d) = dir {
        if d == output_dir || std::fs::remove_dir(d).is_err() {
            break;
        }
        dir = d.parent();
    }

    let tangled = tangled.with_context(|| {
        format!("tangling the README from publication doc {}", rel.display())
    })?;
    let outputs: Vec<PathBuf> = tangled
        .identity_outputs
        .iter()
        .map(|o| o.path.strip_prefix(output_dir).unwrap_or(&o.path).to_path_buf())
        .collect();
    let readme = Path::new("README.md");
    // A publication may tangle root-level Markdown: the README, a declared
    // overlay seed like CONTRIBUTING.md, and corpus-owned guidance such as
    // AGENTS.md. A path with a directory component would be scattering files
    // into the crates, which is the projector's job and never the doc's.
    let root_markdown = |p: &Path| {
        p.parent().is_none_or(|d| d.as_os_str().is_empty())
            && p.extension().is_some_and(|e| e == "md")
    };
    let stray: Vec<&PathBuf> = outputs
        .iter()
        .filter(|p| p.as_path() != readme && !is_overlay(p) && !root_markdown(p))
        .collect();
    if !outputs.iter().any(|p| p == readme) || !stray.is_empty() {
        bail!(
            "publication {} must tangle `README.md` (a `tangle:` block with \
             `root: README.md` and one named markdown chunk) plus, at most, other \
             root-level Markdown documents — its declared overlay paths {:?} are \
             seeded once, any other root-level Markdown is regenerated every \
             projection; it tangled {:?}",
            rel.display(),
            overlay,
            outputs
        );
    }
    // Only a declared overlay path is *seeded*: the public side owns it from the
    // moment it exists, so the `@generated` line comes off and a prior version
    // wins. Everything else the publication tangles — the README, AGENTS.md —
    // stays corpus-owned and is rewritten on every projection, header and all,
    // which is what keeps the guard refusing edits to it downstream.
    for seed in outputs.iter().filter(|p| is_overlay(p)) {
        let path = output_dir.join(seed);
        if let Some((_, bytes)) = kept.iter().find(|(k, _)| k == seed) {
            std::fs::write(&path, bytes)?;
            tracing::info!(path = %seed.display(), "region_repo.overlay.seed_kept");
            continue;
        }
        // The public side owns a seed from the moment it exists; the
        // `@generated` line would make the guard refuse its first edit.
        let text = std::fs::read_to_string(&path)?;
        let body = match text.split_once('\n') {
            Some((first, rest)) if first.contains("@generated by x0k-tangle") => rest.to_string(),
            _ => text,
        };
        std::fs::write(&path, body)?;
        tracing::info!(path = %seed.display(), "region_repo.overlay.seeded");
    }
    tracing::info!(source = %rel.display(), "region_repo.readme.tangled");
    Ok(())
}
```

The CI contract is two forge-agnostic scripts: `tools/ci` builds, tests,
lints, documents, re-tangles with the bundle's own freshly built
`x0k-tangle`, and fails on any change to the tree;
`tools/x0k-guard-generated` refuses commits that edit an `@generated`
file over a range. The GitHub workflow files only call those scripts,
which is why they are optional.

Beside them the projection pins a toolchain, in `rust-toolchain.toml`
rather than a workflow step. A workflow step would pin only the one
forge whose wrappers are optional, and the contract is that any runner
calls `tools/ci`; a `rust-toolchain.toml` pins every runner *and* the
contributor running the same script locally, which is the point — the
projection's green is reproducible or it is decoration. The cost is that
a contributor with rustup fetches that version; the alternative is a
tree whose CI result depends on the month `ubuntu-latest` was imaged.

The pin is the ceiling and says nothing about the floor. The floor is
`rust-version` in the workspace manifest, and the thing that makes it
true rather than aspirational is the clippy step below: clippy's
`incompatible_msrv` fails the build on any item stable later than the
declared floor. That pairing is the whole of the MSRV claim — a pinned
modern toolchain proves the code compiles *at all*, and only the lint
proves it compiles for the oldest reader the manifest invites.

```rust {#emit-ci-and-guard}
fn emit_ci_and_guard(output_dir: &Path, emit_github: bool) -> Result<()> {
    // The CI contract is two forge-agnostic scripts any runner calls:
    // `tools/ci` (build + test + clippy + doc + re-tangle-and-diff) and
    // `tools/x0k-guard-generated` (refuse hand-edits to @generated files over
    // a commit range). Forge workflow files are thin wrappers over these.
    // `rust-toolchain.toml` pins what all of them run on.
    std::fs::write(
        output_dir.join("rust-toolchain.toml"),
        TOOLCHAIN_FILE.replace("{channel}", PINNED_TOOLCHAIN),
    )?;
    // Supply-chain policy: generated scaffolding, not an overlay. The
    // projector regenerates every non-overlay path, so a hand-added
    // `deny.toml` in the public repo would be deleted on the next
    // projection; as scaffolding the policy lives in the corpus under
    // review, and every projection carries the same one.
    std::fs::write(output_dir.join("deny.toml"), DENY_CONFIG)?;
    let tools = output_dir.join("tools");
    std::fs::create_dir_all(&tools)?;
    let mut mode_755 = vec![tools.join("x0k-guard-generated"), tools.join("ci")];
    std::fs::write(&mode_755[0], GUARD_SCRIPT)?;
    std::fs::write(&mode_755[1], CI_SCRIPT)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        for path in mode_755.drain(..) {
            let mut perm = std::fs::metadata(&path)?.permissions();
            perm.set_mode(0o755);
            std::fs::set_permissions(&path, perm)?;
        }
    }
    if emit_github {
        let wf_dir = output_dir.join(".github/workflows");
        std::fs::create_dir_all(&wf_dir)?;
        std::fs::write(wf_dir.join("ci.yml"), CI_WORKFLOW)?;
        std::fs::write(wf_dir.join("guard-generated.yml"), GUARD_WORKFLOW)?;
    }
    Ok(())
}
```

`PROVENANCE.json` (schema `x0k.provenance/v1`) records everything the
receiver needs and everything the relicense act should leave behind:
the publication URI, both corpus revisions, the path map, the overlay,
the crate list, the publish-excluded crates and documents, the literate
documents that back the generated code, the vocabulary modules shipped with the
version stamped into them and where that version came from, the
license applied, where that decision came from, and what each crate
declared in the source tree it was projected from. The three list
fields are what the old templated README computed and restated; the
authored README points at them instead.

```rust {#emit-provenance}
fn emit_provenance(
    output_dir: &Path,
    publication_uri: &str,
    path_map: &BTreeMap<String, String>,
    report: &RepoProjectReport,
    license: &str,
    license_source: LicenseSource,
    source_licenses: &BTreeMap<String, String>,
) -> Result<()> {
    let prov = serde_json::json!({
        "schema": "x0k.provenance/v1",
        "publication_uri": publication_uri,
        "corpus_rev": report.corpus_rev,
        // The immutable companion of `corpus_rev`: a change id can be
        // amended, a commit id cannot, and the receiver's reference
        // projection is reproduced from this one.
        "corpus_commit": report.corpus_commit,
        "path_map": path_map,
        "link_rewrites": [],
        // The facts a templated README used to restate, kept where the
        // authored README can point at them: what the bundle carries,
        // what the publishing flags hold back, which documents back the
        // generated code.
        "crates": report.crates,
        "excluded": report.excluded,
        // Documents held back by `excludes`. Recorded beside the excluded
        // crates for the same reason: a reader comparing this record to the
        // tree should find every absence accounted for, and the receiver
        // needs to know a missing document was a decision, not a gap.
        "excluded_docs": report.excluded_docs,
        "literate_docs": report
            .literate_docs
            .iter()
            .map(|p| p.to_string_lossy().to_string())
            .collect::<Vec<_>>(),
        // Deliberate divergence: paths the public side owns. The projector
        // preserves them as found; a receiver does not flow them back.
        "overlay": report.overlay,
        // `@generated` files that arrived with a vendored crate but whose
        // source document is outside this projection's literate set.
        "dropped_generated": report.dropped_generated,
        // Documents the publication named (reference → projected path).
        // A section's reference carries the `#anchor` that cut it, so a
        // receiver can route an edit back to the heading it came from.
        "documents": report.documents,
        // The vocabulary modules shipped, the directory they were written
        // to (which depends on whether the vocabulary crate is published —
        // `modules_rel_dir`), the version stamped into their
        // `owl:versionIRI`, and where that version came from
        // (`entry-point-crate` or `corpus-rev`).
        "modules": report.modules,
        "modules_dir": report.modules_dir,
        "module_version": report.module_version.as_ref().map(|(v, _)| v.as_str()),
        "module_version_source": report.module_version.as_ref().map(|(_, s)| s.as_str()),
        // The relicense act, recorded: what this projection is released
        // under, where that decision came from, and what each crate declared
        // in the source tree it was projected from. The last one needs its
        // own note: read bare, a map of `LicenseRef-Proprietary` beside a
        // LICENSE-MIT and four MIT manifests reads as a contradiction about
        // this repository's terms, when what it records is the *before* side
        // of a relicense.
        "license_applied": license,
        "license_source": match license_source {
            LicenseSource::PublicationDoc => "publication-doc",
            LicenseSource::Override => "cli-override",
        },
        "relicensed_from": {
            "note": "What each crate's manifest declared in the private source \
tree this projection was taken from. Historical record of the relicense act, \
not this repository's terms: those are `license_applied` above and the \
LICENSE-* files beside this record.",
            "declared": source_licenses,
        },
    });
    std::fs::write(
        output_dir.join("PROVENANCE.json"),
        serde_json::to_string_pretty(&prov)?,
    )?;
    Ok(())
}
```

## Revisions and git

Both revision lookups are best-effort: a jj workspace answers first, a
plain git checkout second, and a workspace with neither yields an empty
string that the commit message and provenance carry honestly rather
than fabricating.

```rust {#current-corpus-rev}
/// Best-effort canonical revision (jj change id, else git commit) the projection
/// was taken from. Empty when neither is available.
fn current_corpus_rev(workspace: &Path) -> String {
    for args in [vec![
        "--no-pager",
        "log",
        "-r",
        "@",
        "--no-graph",
        "-T",
        "change_id",
    ]] {
        if let Ok(out) = std::process::Command::new("jj")
            .current_dir(workspace)
            .args(&args)
            .output()
        {
            if out.status.success() {
                let s = String::from_utf8_lossy(&out.stdout).trim().to_string();
                if !s.is_empty() {
                    return s;
                }
            }
        }
    }
    if let Ok(out) = std::process::Command::new("git")
        .current_dir(workspace)
        .args(["rev-parse", "HEAD"])
        .output()
    {
        if out.status.success() {
            return String::from_utf8_lossy(&out.stdout).trim().to_string();
        }
    }
    String::new()
}
```

```rust {#current-corpus-commit}
/// The immutable git commit id the projection was taken from: the parent of
/// the jj working copy (`@-`, the described commit an operator lands), else
/// `HEAD` of a git checkout. Empty when neither is available.
fn current_corpus_commit(workspace: &Path) -> String {
    if let Ok(out) = std::process::Command::new("jj")
        .current_dir(workspace)
        .args(["--no-pager", "log", "-r", "@-", "--no-graph", "-T", "commit_id"])
        .output()
    {
        if out.status.success() {
            let s = String::from_utf8_lossy(&out.stdout).trim().to_string();
            if !s.is_empty() {
                return s;
            }
        }
    }
    if let Ok(out) = std::process::Command::new("git")
        .current_dir(workspace)
        .args(["rev-parse", "HEAD"])
        .output()
    {
        if out.status.success() {
            return String::from_utf8_lossy(&out.stdout).trim().to_string();
        }
    }
    String::new()
}
```

Git is driven through the binary with a fixed committer identity, so a
projection's commits are recognizably the projector's. The commit
message is the navigable link back into the corpus: the subject names
the publication and change id, the trailers carry the immutable commit
and — on a re-projection — the previous projection's pair.

```rust {#git-run-and-commit}
/// Run one git command in `output_dir`, failing on a non-zero exit.
fn git_run(output_dir: &Path, args: &[&str]) -> Result<()> {
    let status = std::process::Command::new("git")
        .current_dir(output_dir)
        .args(args)
        .status()
        .with_context(|| format!("git {:?}", args))?;
    if !status.success() {
        bail!("git {:?} failed", args);
    }
    Ok(())
}

/// `git commit` with the projector's fixed identity.
fn git_commit(output_dir: &Path, message: &str) -> Result<()> {
    git_run(
        output_dir,
        &[
            "-c",
            "user.name=0k-dot-computer",
            "-c",
            "user.email=hello@0k.computer",
            "commit",
            "-q",
            "-m",
            message,
        ],
    )
}
```

```rust {#projection-message}
/// Commit message for a projection: subject names the publication and the
/// corpus revision; the body carries the immutable corpus commit and the
/// previous projection's revision and commit, so the projected history is
/// navigable back into the corpus.
fn projection_message(publication_uri: &str, report: &RepoProjectReport, initial: bool) -> String {
    let rev = if report.corpus_rev.is_empty() {
        "(unknown corpus rev)".to_string()
    } else {
        report.corpus_rev.clone()
    };
    let mut msg = if initial {
        format!("Initial projection of {publication_uri} at {rev}")
    } else {
        format!("Projection of {publication_uri} at {rev}")
    };
    msg.push_str("\n\nCorpus-Rev: ");
    msg.push_str(&rev);
    if !report.corpus_commit.is_empty() {
        msg.push_str("\nCorpus-Commit: ");
        msg.push_str(&report.corpus_commit);
    }
    if let Some(prev) = &report.previous_corpus_rev {
        msg.push_str("\nPrevious-Corpus-Rev: ");
        msg.push_str(prev);
    }
    if let Some(prev) = &report.previous_corpus_commit {
        msg.push_str("\nPrevious-Corpus-Commit: ");
        msg.push_str(prev);
    }
    msg.push('\n');
    msg
}
```

```rust {#git-init-and-reproject}
/// Fresh repository: `git init` + the root commit.
fn git_init_commit(output_dir: &Path, publication_uri: &str, report: &RepoProjectReport) -> Result<()> {
    git_run(output_dir, &["init", "-q"])?;
    git_run(output_dir, &["add", "-A"])?;
    git_commit(output_dir, &projection_message(publication_uri, report, true))
}

/// Existing repository: stage the re-projection and commit it on top of the
/// current history. Returns `false` (and commits nothing) when the tree is
/// unchanged, so an idle re-publish leaves no empty commit.
fn git_commit_projection(
    output_dir: &Path,
    publication_uri: &str,
    report: &RepoProjectReport,
) -> Result<bool> {
    git_run(output_dir, &["add", "-A"])?;
    let out = std::process::Command::new("git")
        .current_dir(output_dir)
        .args(["status", "--porcelain"])
        .output()
        .context("git status --porcelain")?;
    if !out.status.success() {
        bail!("git status --porcelain failed");
    }
    if out.stdout.iter().all(|b| b.is_ascii_whitespace()) {
        tracing::info!(
            corpus_rev = %report.corpus_rev,
            "region_repo.reprojection.unchanged"
        );
        return Ok(false);
    }
    git_commit(output_dir, &projection_message(publication_uri, report, false))?;
    Ok(true)
}
```

## The emitted texts

The license bodies, the CI script, its two workflow wrappers, and the
generated-file guard are committed into every projection verbatim.

```rust {#license-texts}
const MIT_LICENSE: &str = r#"MIT License

Copyright (c) {copyright}

Permission is hereby granted, free of charge, to any person obtaining a copy
of this software and associated documentation files (the "Software"), to deal
in the Software without restriction, including without limitation the rights
to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
copies of the Software, and to permit persons to whom the Software is
furnished to do so, subject to the following conditions:

The above copyright notice and this permission notice shall be included in all
copies or substantial portions of the Software.

THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE
SOFTWARE.
"#;

const APACHE_PLACEHOLDER: &str = "Apache License 2.0\n\n\
This is a placeholder. Before publication, replace this file with the canonical\n\
Apache License 2.0 text from https://www.apache.org/licenses/LICENSE-2.0.txt\n";

const MPL_PLACEHOLDER: &str = "Mozilla Public License Version 2.0\n\n\
This is a placeholder. Before publication, replace this file with the canonical\n\
MPL-2.0 text from https://www.mozilla.org/media/MPL/2.0/index.txt\n";
```

```rust {#ci-script}
/// The forge-agnostic CI entry point committed into the projected repo. Any
/// runner — GitHub Actions, a bare cron job, a pre-push hook — calls this one
/// script; the emitted workflow files are thin wrappers over it.
const CI_SCRIPT: &str = r#"#!/bin/sh
# Forge-agnostic CI for this projected repo: build, test, and prove the
# committed @generated code is byte-identical to a fresh tangle of the
# literate sources. Any runner calls this script; nothing here is specific
# to a hosting forge.
set -eu
# `--locked`: Cargo.lock is committed, and a lock cargo would rewrite is
# drift the diff below must see, not silently absorb.
cargo build --workspace --locked
cargo test --workspace --locked
# Lints, warnings denied. Two defect classes this closes. First, an item newer
# than the `rust-version` the manifests declare: clippy's `incompatible_msrv`
# is the only thing in this repository that checks that claim, so without this
# step the MSRV is a comment. Second, ordinary lint rot in code a reader is
# invited to read as documentation.
cargo clippy --workspace --locked --all-targets -- -D warnings
# The prose ships as the crate documentation, so a link in it that resolves to
# nothing is a broken promise, not a cosmetic warning.
RUSTDOCFLAGS="-D warnings" cargo doc --workspace --locked --no-deps
# Supply-chain policy (deny.toml): advisories, licences, duplicate bans,
# sources. cargo-deny is the ONLY check here that can see YANKED crates —
# cargo-audit needs a git-format registry index and cargo has defaulted to the
# sparse one since 1.70 — so this is the mechanized guard on both the yanked
# check and the licence story of an MIT release, not belt-and-braces.
#
# Guarded twice, because neither failure mode is a policy finding: absent
# tooling is a skip, and a failure to FETCH the advisory database is a skip
# too. Somebody cloning this repo on a flaky network must not see a red CI run
# that says nothing about the code. An actual finding IS a hard failure —
# `check` runs `--offline` against the database `fetch` just proved it has.
if command -v cargo-deny >/dev/null 2>&1; then
  if cargo deny fetch; then
    cargo deny --offline check
  else
    echo "note: skipping cargo-deny — could not fetch the advisory database" >&2
  fi
else
  echo "note: skipping cargo-deny (install it: cargo install cargo-deny)" >&2
fi
# Re-tangle every literate document (the tangler discovers the ones with a
# `tangle:` block). A tangle failure fails CI — it is never swallowed.
# `cargo run` finds the binary wherever CARGO_TARGET_DIR put it.
cargo run --locked -q -p x0k-tangle --bin x0k-tangle -- tangle knowledge/implementation --workspace .
# The committed @generated code and .tangle-map.json sidecars must be exactly
# what the tangler just wrote: any modified or untracked file is drift.
if [ -n "$(git status --porcelain)" ]; then
  echo "error: re-tangling the literate sources changed the tree:" >&2
  git status --porcelain >&2
  git --no-pager diff >&2
  exit 1
fi
"#;
```

The supply-chain policy is the fourth emitted text. It answers four
standing questions — is anything we ship vulnerable, unsound,
unmaintained or *yanked*; is every dependency's licence one an MIT
release may carry; has the graph grown a duplicate nobody accepted; does
everything come from crates.io — and every value in it was measured
against a real projection's 119-crate lockfile rather than copied from a
template.

Two entries carry their weight by being *unusual*, and both are
commented in the file so a later tidy-up does not remove them. The
`Unicode-3.0` allowance is there because `unicode-ident` requires it
through an `AND`, not an `OR`, so it is an obligation and not a choice.
And LGPL is deliberately *absent*: `r-efi` offers it as one of three
legs, so leaving it out of the allowlist is what proves the permissive
leg is always the one taken — a crate whose only option were LGPL would
fail loudly instead of quietly relicensing the bundle's obligations.

The candid part: the `[bans] skip` list names three duplicate crates at
exact versions, and the projection regenerates its lockfile on every
run, so those pins go stale as the registry moves and
`multiple-versions = "deny"` will eventually fail on a version that has
merely drifted. That is the intended failure — a duplicate the policy
has not seen should stop and be looked at — but it means the list is
maintenance, not a set-and-forget.

```rust {#deny-config}
/// `deny.toml` — the projected repository's supply-chain policy, committed as
/// generated scaffolding so the policy lives in the corpus under review rather
/// than drifting on the public side. `tools/ci` runs it when `cargo-deny` is
/// installed.
const DENY_CONFIG: &str = r#"# cargo-deny policy for the x0k-folio publication.
#
# Four questions, each with a standing answer:
#   advisories — is anything we ship vulnerable, unsound, unmaintained or yanked?
#   licenses   — is every dependency's licence one an MIT release may carry?
#   bans       — has the dependency graph grown a duplicate we did not accept?
#   sources    — does everything come from crates.io and nowhere else?
#
# Verified against the projection's committed Cargo.lock (119 third-party
# crates). Every entry below was measured, not assumed.

[graph]
# The published manifests sever `plugins` (x0k-folio) and `motifs`
# (x0k-tangle): both are declared-but-empty and out of `default`, so the
# default feature set IS the shipped surface. Checking all-features would
# audit a graph nobody can build.
all-features = false

[advisories]
db-path = "~/.cargo/advisory-db"
db-urls = ["https://github.com/RustSec/advisory-db"]
# A yanked dependency in a lockfile we are publishing is a release blocker,
# not a note. This is the check `cargo audit` could not run here: it needs a
# git-format registry index, and this machine has only the sparse index.
yanked = "deny"
# Report unmaintained crates anywhere in the graph, not just direct deps.
# Measured clean today; this is the tripwire for when it stops being clean.
unmaintained = "all"
# No ignores. The graph is clean, so an empty ignore list is honest and any
# future entry has to be argued for in a diff.
ignore = []

[licenses]
# Measured inventory of the 119 third-party crates: every one resolves to a
# permissive licence. This allowlist is the *chosen* leg of each OR
# expression, so it doubles as the notice list a distribution must carry.
allow = [
    "MIT",
    "Apache-2.0",
    "Apache-2.0 WITH LLVM-exception",
    "BSD-2-Clause",
    "BSL-1.0",
    "CC0-1.0",
    "MIT-0",
    "Unicode-3.0",
    "Unlicense",
]
# NOT allowed, deliberately: LGPL-2.1-or-later. `r-efi` 5.3.0/6.0.0 offer it
# as one of three options (MIT OR Apache-2.0 OR LGPL-2.1-or-later); omitting
# it from this list is what proves we always take the permissive leg. If a
# crate ever appears whose ONLY option is LGPL, this check fails loudly
# instead of silently relicensing our obligations.
confidence-threshold = 0.93

[bans]
# The three duplicates below are accepted and enumerated. Denying by default
# with named exceptions means a NEW duplicate major trips the check, rather
# than hiding in a permanent "warn".
multiple-versions = "deny"
wildcards = "deny"
skip = [
    # proc-macro/build-script territory only; neither reaches a runtime
    # artifact of the four published crates.
    { crate = "syn@2.0.119" },      # older proc-macro crates (serde_derive, …)
    { crate = "getrandom@0.3.4" },  # rand's older backend
    { crate = "r-efi@5.3.0" },      # getrandom's UEFI backend
]

[sources]
# The publication must be buildable from crates.io alone: no git deps, no
# vendored registries, no patch overrides. This is the check that keeps a
# path/git dependency from sneaking into a projected manifest.
unknown-registry = "deny"
unknown-git = "deny"
allow-registry = ["https://github.com/rust-lang/crates.io-index"]
allow-git = []
"#;
```

```rust {#toolchain-file}
/// The toolchain `tools/ci` and every contributor run on. Pinned so a green
/// run means the same thing everywhere; the floor a reader may build with is
/// the separate `rust-version` claim, checked by the clippy step.
const TOOLCHAIN_FILE: &str = r#"# The toolchain this repository is built and tested on. Pinned so that a CI
# run and a local `tools/ci` run are the same build.
#
# This is the CEILING. The FLOOR — the oldest Rust that can build this
# repository — is `rust-version` in the workspace Cargo.toml, and `tools/ci`
# proves it: clippy's `incompatible_msrv` lint fails on any item stable later
# than that version.
[toolchain]
channel = "{channel}"
components = ["clippy"]
"#;
```

```rust {#workflow-wrappers}
const CI_WORKFLOW: &str = r#"name: ci
on:
  push:
  pull_request:
  schedule:
    # A new advisory invalidates a dependency graph that nothing has changed,
    # so supply-chain runs on a timer as well as on every push.
    - cron: "0 6 * * 1"
jobs:
  ci:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v7
      - run: ./tools/ci
  supply-chain:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v7
      # `tools/ci` skips cargo-deny when it is not installed, so that a clone
      # builds without it. CI is where the policy has to actually run, so this
      # job installs it and fails on a finding. Separate job: it is slower than
      # the build, and parallel means it costs the fast signal nothing.
      - run: cargo install cargo-deny --locked
      - run: cargo deny check
"#;

const GUARD_WORKFLOW: &str = r#"name: guard-generated
on: [pull_request]
jobs:
  guard:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v7
        with:
          fetch-depth: 0
      - run: ./tools/x0k-guard-generated "${{ github.event.pull_request.base.sha }}" "${{ github.sha }}"
"#;
```

```rust {#guard-script}
const GUARD_SCRIPT: &str = r#"#!/bin/sh
# Reject PR edits to @generated files. Generated code is not an edit surface —
# edit the literate .md the file's header points to; the code is re-tangled from
# it on the next publish.
set -eu
base="${1:-HEAD~1}"
head="${2:-HEAD}"
fail=0
for f in $(git diff --name-only "$base" "$head"); do
  [ -f "$f" ] || continue
  grep -Iq . "$f" || continue  # skip binary files
  first=$(head -n1 "$f" 2>/dev/null || true)
  case "$first" in
    *"@generated by x0k-tangle"*"DO NOT EDIT"*)
      src=$(printf '%s\n' "$first" | sed -n 's/.* from \(.*\) — DO NOT EDIT.*/\1/p')
      echo "error: $f is generated${src:+ from $src} and cannot be edited directly."
      echo "       edit the literate document instead; the code is regenerated on the next publish."
      fail=1
      ;;
  esac
done
exit $fail
"#;
```

## Tests

The license-expression reader is the one piece of this module with
enough cases to pin in-module: a single identifier yields a single
body, the dual expression yields both in declaration order, a
parenthesized expression is read through, and an unknown identifier —
`MIT-0` is the trap the substring match would have fallen into — or a
`WITH` exception refuses. MIT carries its notice line, dated with the
year it was given and naming the holder, and refuses without one; the
year reader is checked against a date whose year is not in doubt.

`````rust {#tests}
#[cfg(test)]
mod tests {
    use super::*;

    const HOLDER: Option<&str> = Some("0k.computer");

    #[test]
    fn license_files_reads_the_expression_not_substrings() {
        let mit = license_files("MIT", HOLDER, 2026).unwrap();
        assert_eq!(mit.iter().map(|(f, _)| *f).collect::<Vec<_>>(), ["LICENSE-MIT"]);
        assert!(mit[0].1.starts_with("MIT License\n\nCopyright (c) 2026 0k.computer\n"));

        let dual = license_files("MIT OR Apache-2.0", HOLDER, 2026).unwrap();
        assert_eq!(
            dual.iter().map(|(f, _)| *f).collect::<Vec<_>>(),
            ["LICENSE-MIT", "LICENSE-APACHE"]
        );

        let parenthesized = license_files("(MIT OR Apache-2.0) AND MPL-2.0", HOLDER, 2026).unwrap();
        assert_eq!(
            parenthesized.iter().map(|(f, _)| *f).collect::<Vec<_>>(),
            ["LICENSE-MIT", "LICENSE-APACHE", "LICENSE-MPL"]
        );
    }

    #[test]
    fn license_files_refuses_unknown_identifiers_and_exceptions() {
        let err = license_files("MIT-0", HOLDER, 2026).unwrap_err().to_string();
        assert!(err.contains("`MIT-0`"), "names the identifier: {err}");
        let err = license_files("LicenseRef-Proprietary", HOLDER, 2026).unwrap_err().to_string();
        assert!(err.contains("LicenseRef-Proprietary"), "{err}");
        let err = license_files("Apache-2.0 WITH LLVM-exception", HOLDER, 2026)
            .unwrap_err()
            .to_string();
        assert!(err.contains("exception"), "{err}");
        assert!(license_files("OR", HOLDER, 2026).is_err(), "no identifier at all refuses");
    }

    #[test]
    fn mit_refuses_without_a_copyright_holder() {
        let err = license_files("MIT", None, 2026).unwrap_err().to_string();
        assert!(err.contains("copyright"), "{err}");
        assert!(license_files("MIT", Some("  "), 2026).is_err(), "blank holder refuses");
        // Apache-2.0 alone carries no notice line and needs no holder.
        assert!(license_files("Apache-2.0", None, 2026).is_ok());
    }

    #[test]
    fn current_year_is_a_plausible_year() {
        let y = current_year();
        assert!((2026..2200).contains(&y), "{y}");
    }

    /// A document just complete enough to be rendered into the contents page.
    fn doc(rel: &str, title: &str, summary: Option<&str>) -> LiterateDoc {
        LiterateDoc {
            rel: PathBuf::from(rel),
            tangled: true,
            crate_name: Some("demo-crate".to_string()),
            title: title.to_string(),
            summary: summary.map(str::to_string),
        }
    }

    fn marker_of(text: &str) -> Result<Option<ContentsMarker>> {
        find_contents_marker(&text.split_inclusive('\n').collect::<Vec<_>>())
    }

    #[test]
    fn a_bare_marker_carries_no_plan() {
        let m = marker_of("# R\n\n<!-- x0k:contents -->\n\n## After\n")
            .unwrap()
            .expect("the marker is found");
        assert_eq!((m.start, m.end), (2, 3), "the marker is its own one line");
        assert_eq!(m.plan, ContentsPlan::Spine(Vec::new()));
        assert!(marker_of("# R\n\nno marker here\n").unwrap().is_none());
    }

    #[test]
    fn a_multi_line_marker_carries_one_reading_order_per_area() {
        let m = marker_of("<!-- x0k:contents\ntangle: protocol parsing\nfolio: format\n-->\nafter\n")
            .unwrap()
            .expect("the marker is found");
        assert_eq!((m.start, m.end), (0, 4), "start through the closing `-->`");
        assert_eq!(
            m.plan,
            ContentsPlan::Spine(vec![
                ("tangle".to_string(), vec!["protocol".to_string(), "parsing".to_string()]),
                ("folio".to_string(), vec!["format".to_string()]),
            ])
        );
    }

    #[test]
    fn a_group_marker_carries_a_heading_a_blurb_and_area_qualified_members() {
        let m = marker_of(
            "<!-- x0k:contents\n# What a document is\n> The envelope and the tree.\n  folio/format\n  tangle/chunk\n# Just a heading\n  folio/colophon\n-->\n",
        )
        .unwrap()
        .expect("the marker is found");
        assert_eq!((m.start, m.end), (0, 8), "start through the closing `-->`");
        let ContentsPlan::Groups(groups) = m.plan else {
            panic!("the marker declares groups");
        };
        assert_eq!(groups[0].heading, "What a document is");
        assert_eq!(groups[0].blurb.as_deref(), Some("The envelope and the tree."));
        assert_eq!(groups[0].members, vec!["folio/format", "tangle/chunk"]);
        // A group need not carry a blurb; nothing else supplies one either.
        assert_eq!(groups[1].blurb, None);
        assert_eq!(groups[1].members, vec!["folio/colophon"]);
    }

    #[test]
    fn a_marker_mixing_groups_with_reading_order_lines_refuses() {
        let err = marker_of("<!-- x0k:contents\n# A concept\n  demo/a\ndemo: b\n-->\n")
            .unwrap_err()
            .to_string();
        assert!(err.contains("mixes"), "{err}");
        // Either order, because neither form is the one that arrived second.
        let err = marker_of("<!-- x0k:contents\ndemo: b\n# A concept\n  demo/a\n-->\n")
            .unwrap_err()
            .to_string();
        assert!(err.contains("mixes"), "{err}");
    }

    #[test]
    fn a_bare_stem_and_a_stray_blurb_both_refuse() {
        let err = marker_of("<!-- x0k:contents\n# A concept\n  format\n-->\n")
            .unwrap_err()
            .to_string();
        assert!(err.contains("`format`") && err.contains("<area>/<stem>"), "{err}");
        let err = marker_of("<!-- x0k:contents\n> orphaned blurb\n-->\n")
            .unwrap_err()
            .to_string();
        assert!(err.contains("blurb"), "{err}");
    }

    #[test]
    fn a_second_marker_an_unclosed_one_and_a_malformed_order_line_all_refuse() {
        let err = marker_of("<!-- x0k:contents -->\n<!-- x0k:contents -->\n")
            .unwrap_err()
            .to_string();
        assert!(err.contains("one home"), "{err}");
        let err = marker_of("<!-- x0k:contents\ntangle: protocol\n").unwrap_err().to_string();
        assert!(err.contains("never closed"), "{err}");
        let err = marker_of("<!-- x0k:contents\ntangle protocol\n-->\n").unwrap_err().to_string();
        assert!(err.contains("tangle protocol"), "{err}");
    }

    #[test]
    fn the_reading_order_leads_and_the_rest_follows_by_path() {
        let docs = vec![
            doc("knowledge/implementation/demo/a.md", "A", Some("The first.")),
            doc("knowledge/implementation/demo/b.md", "B", Some("The second.")),
            doc("knowledge/implementation/demo/c.md", "C", Some("The third.")),
        ];
        let plan = ContentsPlan::Spine(vec![("demo".to_string(), vec!["c".to_string()])]);
        let page = render_contents(&docs, &plan, &[], Path::new("ontology/modules")).unwrap();
        let expected = [
            "### `demo-crate`",
            "",
            "- [C](knowledge/implementation/demo/c.md) — The third.",
            "- [A](knowledge/implementation/demo/a.md) — The first.",
            "- [B](knowledge/implementation/demo/b.md) — The second.",
        ]
        .join("\n");
        assert_eq!(page, expected, "{page}");
    }

    /// A group of the page, named the way a marker names one.
    fn group(heading: &str, blurb: Option<&str>, members: &[&str]) -> ContentsGroup {
        ContentsGroup {
            heading: heading.to_string(),
            blurb: blurb.map(str::to_string),
            members: members.iter().map(|m| m.to_string()).collect(),
        }
    }

    #[test]
    fn a_group_heads_the_page_verbatim_and_crosses_areas() {
        let docs = vec![
            doc("knowledge/implementation/demo/a.md", "A", Some("The first.")),
            doc("knowledge/implementation/other/b.md", "B", Some("The second.")),
        ];
        let plan = ContentsPlan::Groups(vec![
            group("What a document is", Some("The envelope and the tree."), &["other/b"]),
            group("Chunks", None, &["demo/a"]),
        ]);
        let page = render_contents(&docs, &plan, &[], Path::new("ontology/modules")).unwrap();
        let expected = [
            "### What a document is",
            "",
            "The envelope and the tree.",
            "",
            "- [B](knowledge/implementation/other/b.md) — The second.",
            "",
            "### Chunks",
            "",
            "- [A](knowledge/implementation/demo/a.md) — The first.",
        ]
        .join("\n");
        assert_eq!(page, expected, "{page}");
        assert!(!page.contains("demo-crate"), "no crate is derived into a heading");
    }

    #[test]
    fn a_group_naming_an_unshipped_document_refuses() {
        let docs = vec![doc("knowledge/implementation/demo/a.md", "A", Some("The first."))];
        let plan = ContentsPlan::Groups(vec![group("Concept", None, &["demo/a", "demo/renamed-away"])]);
        let err = render_contents(&docs, &plan, &[], Path::new("ontology/modules"))
            .unwrap_err()
            .to_string();
        assert!(
            err.contains("demo/renamed-away") && err.contains("Concept"),
            "names the stale member and its group: {err}"
        );
    }

    #[test]
    fn a_shipped_document_no_group_names_refuses() {
        let docs = vec![
            doc("knowledge/implementation/demo/a.md", "A", Some("The first.")),
            doc("knowledge/implementation/demo/b.md", "B", Some("The second.")),
        ];
        let plan = ContentsPlan::Groups(vec![group("Concept", None, &["demo/a"])]);
        let err = render_contents(&docs, &plan, &[], Path::new("ontology/modules"))
            .unwrap_err()
            .to_string();
        assert!(err.contains("demo/b"), "names the chapter with no place: {err}");
        assert!(!err.contains("demo/a"), "and only that one: {err}");
    }

    #[test]
    fn a_document_two_groups_both_claim_refuses() {
        let docs = vec![doc("knowledge/implementation/demo/a.md", "A", Some("The first."))];
        let plan = ContentsPlan::Groups(vec![
            group("First", None, &["demo/a"]),
            group("Second", None, &["demo/a"]),
        ]);
        let err = render_contents(&docs, &plan, &[], Path::new("ontology/modules"))
            .unwrap_err()
            .to_string();
        assert!(
            err.contains("demo/a") && err.contains("First") && err.contains("Second"),
            "names the document and both groups: {err}"
        );
    }

    #[test]
    fn a_document_with_no_summary_refuses_rather_than_printing_its_title_twice() {
        let docs = vec![doc("knowledge/implementation/demo/a.md", "A", None)];
        let bare = ContentsPlan::Spine(Vec::new());
        let err = render_contents(&docs, &bare, &[], Path::new("ontology/modules"))
            .unwrap_err()
            .to_string();
        assert!(err.contains("demo/a.md") && err.contains("summary"), "{err}");
        // An empty summary is the same silence as an absent one.
        let docs = vec![doc("knowledge/implementation/demo/a.md", "A", Some("  "))];
        assert!(render_contents(&docs, &bare, &[], Path::new("ontology/modules")).is_err());
        // And a grouped page is no more forgiving than an ungrouped one.
        let grouped = ContentsPlan::Groups(vec![group("Concept", None, &["demo/a"])]);
        assert!(render_contents(&docs, &grouped, &[], Path::new("ontology/modules")).is_err());
    }
}
`````

## What a publication ships, pinned

The guards above decide what may leave the corpus; the tests in this section
watch them decide it, over a projection made by the real projector into a real
directory. They share one fixture, because the questions share one subject —
*what did this publication actually ship, and does the repository say so?* Three
tiny vocabulary modules stand in for the ontology: `core`, which imports
nothing; `document`, which imports `core`; and `orphan`, which imports a module
the tree does not hold.

```rust {#modules-doc file="tests/region_repo_modules.rs"}
//! Pins for the vocabulary modules a publication ships
//! (`x0k:implementation/tangle/region-repo`, against ADR
//! `x0k:architecture/ontology-modules` §3, §4 and §6), together with the
//! document severance and the contents page that share its fixture.
//!
//! The fixture under `tests/fixtures/ontology-modules/` is three tiny
//! modules: `core` (no imports), `document` (imports `core`), and `orphan`
//! (imports a module the tree does not hold).
```

```rust {#modules-uses file="tests/region_repo_modules.rs"}
use std::path::Path;

use x0k_tangle::{project_publication_repo, tangle_document, PipelineRegistry, RepoProjectOptions};
```

The fixture documents are string constants so a test can read as one
paragraph. `demo-crate` is the published crate; `colophon` and `extra` are two
chapters of it in one area, which is the minimum shape in which a severance can
hold one document back and keep the other.

```rust {#modules-consts file="tests/region_repo_modules.rs"}
const FIXTURE: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/ontology-modules");
/// Shapes for those fixtures: `document` constrains one predicate, `core`
/// constrains nothing, so the two halves of §5 are both in the fixture set.
const SHAPE_FIXTURE: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/ontology-shapes");

const DOC_REL: &str = "knowledge/implementation/demo/colophon.md";
/// A second chapter of the same crate, in the same area, tangling to its own
/// file — the subject of the document-severance pins below.
const EXTRA_REL: &str = "knowledge/implementation/demo/extra.md";
const EXTRA_ID: &str = "x0k:implementation/demo/extra";
const PUB_REL: &str = "decisions/publications/demo.md";
/// A decision document in the corpus, in a topic subdirectory the way the
/// real corpus files them. Two affordances under one `## Affordances`
/// heading: one the demo publication can honestly ship, one it cannot.
const DESIGN_REL: &str = "decisions/design/corpus/demo-design.md";
const DESIGN_ID: &str = "x0k:design/demo-design";
const SHIPPABLE: &str = "read-a-line-out-of-a-document";

const DOC: &str = "---\nx0k:\n  format: folio/v1\n  id: x0k:implementation/demo/colophon\n  type: implementation\n  status: draft\n  summary: The demo crate's one exported function, and where it trims.\n  tangle:\n    crate: demo-crate\n    root: src/lib.rs\n---\n# The demo colophon\n\n```rust {#root}\n/// First line of `s`, trimmed.\npub fn parse_line(s: &str) -> &str {\n    s.lines().next().unwrap_or(\"\").trim()\n}\n```\n";
```

The publication doc is built by `format!` rather than kept as a fixture file
because almost every test varies one field of it — which crates, which modules,
whether there is an entry point, what `excludes` names. The `excludes` list is
interpolated verbatim, so a test can name a document, a module, or nonsense and
see what the reader does with it.

```rust {#modules-publication-fixture file="tests/region_repo_modules.rs"}
/// A publication doc publishing `demo-crate` plus `modules`, with
/// `demo-crate` as the entry point (so the stamped version is its
/// manifest `version`, `0.1.0`).
fn publication(crates: &[&str], modules: &[&str], entry_point: bool) -> String {
    publication_full(crates, modules, entry_point, &[], &[])
}

/// As [`publication`], with an `excludes:` edge carrying `excludes` verbatim
/// (whole URIs, so a test can name a document, a module, or nonsense).
fn publication_excluding(
    crates: &[&str],
    modules: &[&str],
    entry_point: bool,
    excludes: &[&str],
) -> String {
    publication_full(crates, modules, entry_point, excludes, &[])
}

/// As [`publication`], additionally naming `documents` under `publishes`
/// verbatim — a document id, or one with a `#anchor` selecting a section.
fn publication_publishing(crates: &[&str], documents: &[&str]) -> String {
    publication_full(crates, &[], true, &[], documents)
}

fn publication_full(
    crates: &[&str],
    modules: &[&str],
    entry_point: bool,
    excludes: &[&str],
    documents: &[&str],
) -> String {
    let mut publishes = String::new();
    for c in crates {
        publishes.push_str(&format!("      - x0k:software-module/{c}\n"));
    }
    for m in modules {
        publishes.push_str(&format!("      - x0k:ontology-module/{m}\n"));
    }
    for d in documents {
        publishes.push_str(&format!("      - {d}\n"));
    }
    let entry = if entry_point {
        "    entryPoint:\n      - x0k:software-module/demo-crate\n"
    } else {
        ""
    };
    let excluded = if excludes.is_empty() {
        String::new()
    } else {
        let mut b = String::from("    excludes:\n");
        for e in excludes {
            b.push_str(&format!("      - {e}\n"));
        }
        b
    };
    format!(
        "---\nx0k:\n  format: folio/v1\n  type: publication\n  id: x0k:publication/demo\n  status: proposed\n  license: MIT\n  copyright: Demo Authors\n  edges:\n    publishes:\n{publishes}{excluded}{entry}  tangle:\n    root: README.md\n---\n# Demo\n\n```markdown {{#readme}}\n# Demo\n\nA demo publication.\n\n## What is here\n\n<!-- x0k:contents -->\n\n## Afterwards\n\nText after the contents.\n```\n"
    )
}
```

The workspace is assembled with the real tangler, not with plausible files.
A hand-written `lib.rs` would let a test pass while the projector's actual input
— a `@generated` file with a sidecar naming its document — had stopped being
produced.

```rust {#modules-write-crate file="tests/region_repo_modules.rs"}
fn write_crate(ws: &Path, name: &str) {
    std::fs::create_dir_all(ws.join(name).join("src")).unwrap();
    std::fs::write(
        ws.join(name).join("Cargo.toml"),
        format!(
            "[package]\nname = \"{name}\"\nversion = \"0.1.0\"\nedition = \"2021\"\nlicense = \"LicenseRef-Proprietary\"\n\n[package.metadata.x0k]\naccess = \"public\"\n\n[dependencies]\n"
        ),
    )
    .unwrap();
    std::fs::write(ws.join(name).join("src/lib.rs"), "pub fn one() -> u8 {\n    1\n}\n").unwrap();
}
```

```rust {#modules-workspace file="tests/region_repo_modules.rs"}
/// A workspace with `demo-crate` (tangled by the real tangler), the
/// fixture modules copied under `ontology/modules/`, and a publication doc
/// naming `modules`.
fn workspace(modules: &[&str], entry_point: bool) -> tempfile::TempDir {
    let tmp = tempfile::tempdir().expect("tempdir");
    let ws = tmp.path();
    write_crate(ws, "demo-crate");
    std::fs::create_dir_all(ws.join(DOC_REL).parent().unwrap()).unwrap();
    std::fs::write(ws.join(DOC_REL), DOC).unwrap();
    tangle_document(&ws.join(DOC_REL), ws, &PipelineRegistry::default()).expect("tangle");
    // A second chapter of the same crate, tangling to its own file, so a
    // document severance has something to take out without emptying the crate.
    std::fs::write(
        ws.join(EXTRA_REL),
        format!("---\nx0k:\n  format: folio/v1\n  id: {EXTRA_ID}\n  type: implementation\n  status: draft\n  summary: A second chapter, so a document severance has something to take out.\n  tangle:\n    crate: demo-crate\n    root: src/extra.rs\n---\n# The extra chapter\n\n```rust {{#root}}\n/// Length of `s`.\npub fn measure(s: &str) -> usize {{\n    s.len()\n}}\n```\n"),
    )
    .unwrap();
    tangle_document(&ws.join(EXTRA_REL), ws, &PipelineRegistry::default()).expect("tangle extra");
    std::fs::create_dir_all(ws.join("ontology/modules")).unwrap();
    for entry in std::fs::read_dir(FIXTURE).unwrap() {
        let entry = entry.unwrap();
        std::fs::copy(entry.path(), ws.join("ontology/modules").join(entry.file_name())).unwrap();
    }
    std::fs::create_dir_all(ws.join("ontology/shapes")).unwrap();
    for entry in std::fs::read_dir(SHAPE_FIXTURE).unwrap() {
        let entry = entry.unwrap();
        std::fs::copy(entry.path(), ws.join("ontology/shapes").join(entry.file_name())).unwrap();
    }
    // A decision document sits in the corpus for every projection here.
    // Most of these tests never name it, which is the point: it must not
    // cross unless a publication says so.
    std::fs::create_dir_all(ws.join(DESIGN_REL).parent().unwrap()).unwrap();
    std::fs::write(ws.join(DESIGN_REL), demo_design()).unwrap();
    std::fs::create_dir_all(ws.join(PUB_REL).parent().unwrap()).unwrap();
    std::fs::write(
        ws.join(PUB_REL),
        publication(&["demo-crate"], modules, entry_point),
    )
    .unwrap();
    std::fs::write(ws.join(".gitignore"), "/target\n").unwrap();
    tmp
}
```

Two shapes of call, because roughly half of these tests are about refusal:
one that expects a report and one that expects an error message to read.

```rust {#modules-project-helpers file="tests/region_repo_modules.rs"}
/// The decision-document fixture: a design whose `## Affordances` heading
/// holds two `###` sections, the shippable one claimed for a human. Built
/// rather than kept as a constant so the affordance fences read as
/// themselves.
fn demo_design() -> String {
    let mut d = String::new();
    d.push_str("---\nx0k:\n  format: folio/v1\n  id: x0k:design/demo-design\n  type: design\n  status: proposed\n---\n");
    d.push_str("# The demo design\n\nContext this repository has no use for.\n\n");
    d.push_str("### Affordances\n\n");
    d.push_str("### Read a line out of a document\n\nI read the first line, and the shipped crate is what lets me.\n\n");
    d.push_str("```yaml x0k:affordance\nid: x0k:affordance/read_a_line\nstatus: wip\nactors: [human]\nedges:\n  enabledBy:\n    - x0k:software-module/demo-crate\n```\n\n");
    d.push_str("### Run the whole fleet\n\nI do a thing this bundle cannot do.\n\n");
    d.push_str("```yaml x0k:affordance\nid: x0k:affordance/run_the_whole_fleet\nstatus: wip\nedges:\n  enabledBy:\n    - x0k:software-module/unshipped-crate\n```\n");
    d
}

/// Whether any file under `dir` carries `needle` — the shape of "this did
/// not cross", asked of the whole projection rather than of one path.
fn tree_carries(dir: &Path, needle: &str) -> bool {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return false;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            if path.file_name().is_some_and(|n| n == "target") {
                continue;
            }
            if tree_carries(&path, needle) {
                return true;
            }
        } else if std::fs::read_to_string(&path).is_ok_and(|t| t.contains(needle)) {
            return true;
        }
    }
    false
}

fn project(ws: &Path, out: &Path) -> anyhow::Result<x0k_tangle::RepoProjectReport> {
    project_publication_repo(
        &ws.join(PUB_REL),
        out,
        ws,
        &RepoProjectOptions {
            license: None,
            git_init: false,
            allow_dirty: false,
            emit_github: false,
        },
    )
}

fn project_err(ws: &Path) -> String {
    let out = tempfile::tempdir().unwrap();
    let err = project(ws, out.path()).expect_err("projection is refused");
    format!("{err:#}")
}

/// Rewrite the fixture's contents marker and project, expecting a refusal.
/// Every contents-marker guard is asked the same way: swap the one line the
/// publication's README carries and read what the projector says back.
fn project_err_with_marker(marker: &str) -> String {
    let ws = workspace(&[], true);
    let doc = std::fs::read_to_string(ws.path().join(PUB_REL)).unwrap();
    std::fs::write(
        ws.path().join(PUB_REL),
        doc.replace("<!-- x0k:contents -->", marker),
    )
    .unwrap();
    project_err(ws.path())
}

/// The fixture's README with a `## What you can do here` section carrying
/// the affordances marker, ahead of the afterword.
fn with_affordances_marker(publication: &str) -> String {
    publication.replace(
        "## Afterwards\n",
        "## What you can do here\n\n<!-- x0k:affordances -->\n\n## Afterwards\n",
    )
}

/// A prose-only chapter in the demo crate's area declaring a signifier: the
/// `demo-line` verb, presented on the CLI, signifying the shippable
/// affordance. It ships because its area does, not by name — which is how
/// a face's cue reaches the projection.
fn declare_signifier(ws: &Path) {
    std::fs::write(
        ws.join("knowledge/implementation/demo/verbs.md"),
        "---\nx0k:\n  format: folio/v1\n  id: x0k:implementation/demo/verbs\n  type: implementation\n  status: draft\n  summary: The demo crate's one verb, and the cue that reaches it.\n---\n# The demo verbs\n\n## demo-line\n\nPrints the first line of a file.\n\n```yaml x0k:signifier\nid: x0k:signifier/demo-line\nedges:\n  signifies:\n    - x0k:affordance/read_a_line\n  presentedOn:\n    - x0k:surface/cli\n```\n",
    )
    .unwrap();
}
```

### The module set

The first test is the whole positive contract in one pass. A closed
selection `[core, document]` ships both files with exactly one `owl:versionIRI`
line inserted at its sorted position among the triples and every other byte
identical to the source, lists them under the README's contents marker, and
records them in `PROVENANCE.json` with the stamped version and where it came
from. Byte-identity is the part worth stating: a vocabulary that is *rewritten*
on the way out is a second vocabulary, and no consumer could tell which one it
had.

```rust {#modules-closed-selection file="tests/region_repo_modules.rs"}
#[test]
fn a_closed_selection_projects_each_module_stamped_and_otherwise_verbatim() {
    let ws = workspace(&["core", "document"], true);
    let out = tempfile::tempdir().unwrap();
    let report = project(ws.path(), out.path()).expect("projection");
    assert_eq!(report.modules, vec!["core".to_string(), "document".to_string()]);

    for name in ["core", "document"] {
        let source =
            std::fs::read_to_string(Path::new(FIXTURE).join(format!("{name}.ttl"))).unwrap();
        let shipped = std::fs::read_to_string(
            out.path().join("ontology/modules").join(format!("{name}.ttl")),
        )
        .unwrap_or_else(|e| panic!("{name}.ttl is shipped: {e}"));
        let stamp = format!(
            "<https://0k.computer/ontology/{name}> <http://www.w3.org/2002/07/owl#versionIRI> \
             <https://0k.computer/ontology/{name}/0.1.0> .\n"
        );
        assert_eq!(
            shipped.matches("owl#versionIRI").count(),
            1,
            "{name}: exactly one versionIRI"
        );
        assert!(
            shipped.contains(&stamp),
            "{name}: the versionIRI stamp is present verbatim:\n{shipped}"
        );
        let triples: Vec<&str> = shipped
            .lines()
            .filter(|l| !l.starts_with('#') && !l.trim().is_empty())
            .collect();
        let mut sorted = triples.clone();
        sorted.sort();
        assert_eq!(triples, sorted, "{name}: the shipped file is still sorted N-Triples");
        assert_eq!(
            shipped.replacen(&stamp, "", 1),
            source,
            "{name}: byte-identical apart from the stamp, header included"
        );
    }

    // The contents page stands where the marker was: the crate's documents
    // (path order, with no reading order authored), then the modules, each
    // described by its own module fact's `rdfs:comment`.
    let readme = std::fs::read_to_string(out.path().join("README.md")).unwrap();
    let expected = "\
## What is here

### `demo-crate`

- [The demo colophon](knowledge/implementation/demo/colophon.md) — The demo crate's one exported function, and where it trims.
- [The extra chapter](knowledge/implementation/demo/extra.md) — A second chapter, so a document severance has something to take out.

### Vocabulary modules

- [`ontology/modules/core.ttl`](ontology/modules/core.ttl) — The self-typed root and the subclass relation every other module stands on.
- [`ontology/modules/document.ttl`](ontology/modules/document.ttl) — The document genus, its kinds, and the edges between documents.

## Afterwards
";
    assert!(readme.contains(expected), "contents page:\n{readme}");

    let prov: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(out.path().join("PROVENANCE.json")).unwrap())
            .unwrap();
    assert_eq!(prov["modules"], serde_json::json!(["core", "document"]));
    assert_eq!(prov["module_version"], "0.1.0");
    assert_eq!(prov["module_version_source"], "entry-point-crate");
}
```

Shapes ride with their modules rather than being selected. A publication
names `document`, and `document`'s shapes cross with it — no publication
mentions a shape file, and none can select one on its own
(`x0k:architecture/vocabulary-shapes` §5). The negative half is what makes it
a rule instead of a copy: a selection that leaves a module out leaves its
shapes out too, and a module that constrains nothing ships no file at all.

```rust {#modules-shapes-travel file="tests/region_repo_modules.rs"}
#[test]
fn a_module_carries_its_shapes_and_only_its_own() {
    let ws = workspace(&["core", "document"], true);
    let out = tempfile::tempdir().unwrap();
    project(ws.path(), out.path()).expect("projection");

    let source = std::fs::read_to_string(Path::new(SHAPE_FIXTURE).join("document.ttl")).unwrap();
    let shipped = std::fs::read_to_string(out.path().join("ontology/shapes/document.ttl"))
        .expect("document's shapes crossed with document");
    // Unstamped, unlike the module file: `owl:versionIRI` is a fact about the
    // module, and the module file is where the module's facts are.
    assert_eq!(shipped, source, "a shape file crosses byte for byte");
    assert!(
        !out.path().join("ontology/shapes/core.ttl").exists(),
        "core constrains nothing, so it ships no shape file"
    );
}

#[test]
fn a_module_left_out_leaves_its_shapes_behind() {
    let ws = workspace(&["core"], true);
    let out = tempfile::tempdir().unwrap();
    project(ws.path(), out.path()).expect("projection");
    assert!(
        !out.path().join("ontology/shapes/document.ttl").exists(),
        "document was not selected, so its shapes must not cross either"
    );
}
```


A publication ships a *set*, not a list of files, and the three ways a set
can fail to be closed each get a test. `[document]` alone names an import the
selection does not carry; `[orphan]` names one the tree does not hold; and a
name that is neither refuses before anything is projected. Each refusal names
both ends, because "incomplete module set" without the two module names leaves
the maintainer to guess.

```rust {#modules-import-outside-selection file="tests/region_repo_modules.rs"}
#[test]
fn an_import_outside_the_selection_is_refused_naming_both_modules() {
    let ws = workspace(&["document"], true);
    let err = project_err(ws.path());
    assert!(
        err.contains("`document`") && err.contains("`core`"),
        "names the importing module and the missing import: {err}"
    );
    assert!(
        err.contains("x0k:ontology-module/core"),
        "says what to add under publishes: {err}"
    );
}
```

```rust {#modules-import-absent file="tests/region_repo_modules.rs"}
#[test]
fn an_import_absent_from_the_tree_is_refused_naming_it() {
    let ws = workspace(&["orphan"], true);
    let err = project_err(ws.path());
    assert!(
        err.contains("`orphan`") && err.contains("`absent`"),
        "names the module and its absent import: {err}"
    );
}
```

A module file is a vocabulary, and a vocabulary is not a place to keep
instances. A file carrying an instance line is refused rather than trimmed —
trimming would make the published file differ from the corpus file, which is the
one property the stamping rule above works to preserve.

```rust {#modules-instance-line file="tests/region_repo_modules.rs"}
#[test]
fn a_module_file_carrying_an_instance_line_is_refused() {
    let ws = workspace(&["core"], true);
    let path = ws.path().join("ontology/modules/core.ttl");
    let mut text = std::fs::read_to_string(&path).unwrap();
    text.push_str(
        "<https://0k.computer/instance/ab12> <http://www.w3.org/1999/02/22-rdf-syntax-ns#type> \
         <https://0k.computer/ontology#Concept> .\n",
    );
    std::fs::write(&path, text).unwrap();
    let err = project_err(ws.path());
    assert!(
        err.contains("`core`") && err.contains("https://0k.computer/instance/"),
        "names the module and the instance namespace: {err}"
    );
}
```

```rust {#modules-not-in-tree file="tests/region_repo_modules.rs"}
#[test]
fn a_module_that_is_not_in_the_tree_is_refused() {
    let ws = workspace(&["missing"], true);
    let err = project_err(ws.path());
    assert!(err.contains("`missing`"), "names the module: {err}");
}
```

Publishing `x0k-ontology` with no module is the coherent-looking mistake:
the crate builds its tables from module files at build time, so a tarball with
the crate and none of its input cannot compile. The refusal is on the pair, not
on either half.

```rust {#modules-ontology-without-module file="tests/region_repo_modules.rs"}
#[test]
fn x0k_ontology_without_a_module_is_refused() {
    let ws = workspace(&[], true);
    write_crate(ws.path(), "x0k-ontology");
    std::fs::write(
        ws.path().join(PUB_REL),
        publication(&["demo-crate", "x0k-ontology"], &[], true),
    )
    .unwrap();
    let err = project_err(ws.path());
    assert!(
        err.contains("x0k-ontology needs at least one vocabulary module"),
        "{err}"
    );
}
```

The stamp needs a version, and the version comes from the entry point's
manifest. Without an entry point it falls back to the corpus revision, and with
neither it refuses — an unstamped module file would be a vocabulary nobody can
cite.

```rust {#modules-stamp-fallback file="tests/region_repo_modules.rs"}
#[test]
fn without_an_entry_point_the_stamp_falls_back_to_the_corpus_rev_or_refuses() {
    let ws = workspace(&["core"], false);
    let out = tempfile::tempdir().unwrap();
    match project(ws.path(), out.path()) {
        Ok(report) => {
            let (version, _) = report.module_version.clone().expect("a stamp");
            assert_eq!(version, report.corpus_rev);
            let prov: serde_json::Value = serde_json::from_str(
                &std::fs::read_to_string(out.path().join("PROVENANCE.json")).unwrap(),
            )
            .unwrap();
            assert_eq!(prov["module_version_source"], "corpus-rev");
        }
        Err(err) => {
            let err = format!("{err:#}");
            assert!(
                err.contains("no `entryPoint`") && err.contains("no corpus revision"),
                "{err}"
            );
        }
    }
}
```

A publication that ships no vocabulary at all still has documents to list.
The contents marker is replaced by the document pages and no vocabulary section
appears — the marker is a position, not a promise that modules exist.

```rust {#modules-no-module-still-lists file="tests/region_repo_modules.rs"}
#[test]
fn a_publication_shipping_no_module_still_lists_its_documents() {
    let ws = workspace(&[], true);
    let out = tempfile::tempdir().unwrap();
    let report = project(ws.path(), out.path()).expect("projection");
    assert!(report.modules.is_empty());
    assert!(report.module_version.is_none());
    assert!(!out.path().join("ontology").exists(), "no modules directory");
    let readme = std::fs::read_to_string(out.path().join("README.md")).unwrap();
    assert!(
        readme.contains(
            "- [The extra chapter](knowledge/implementation/demo/extra.md) — A second \
             chapter, so a document severance has something to take out.\n\n## Afterwards\n"
        ),
        "the documents are listed and no vocabulary section follows them:\n{readme}"
    );
    assert!(!readme.contains("Vocabulary modules"), "{readme}");
    let prov: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(out.path().join("PROVENANCE.json")).unwrap())
            .unwrap();
    assert_eq!(prov["modules"], serde_json::json!([]));
    assert!(prov["module_version"].is_null());
}
```

### Where the modules land

The layout has two cases and they are mutually exclusive, which is why they
are two tests rather than one. When `x0k-ontology` is among the published
crates, the module files are its build-script input and must travel *inside* its
package or `cargo package` fails verifying the tarball. When the crate is held
back, nothing in the bundle reads them, and they keep the tree's own root
layout. Never both: one vocabulary, one copy.

```rust {#modules-layout-published file="tests/region_repo_modules.rs"}
#[test]
fn publishing_the_vocabulary_crate_puts_the_modules_inside_it() {
    let ws = workspace(&["core", "document"], true);
    write_crate(ws.path(), "x0k-ontology");
    std::fs::write(
        ws.path().join(PUB_REL),
        publication(&["demo-crate", "x0k-ontology"], &["core", "document"], true),
    )
    .unwrap();
    let out = tempfile::tempdir().unwrap();
    let report = project(ws.path(), out.path()).expect("projection");

    for name in ["core", "document"] {
        let inside = out
            .path()
            .join("x0k-ontology/ontology/modules")
            .join(format!("{name}.ttl"));
        assert!(inside.is_file(), "{} is not inside the crate", inside.display());
    }
    // One copy: the root layout must NOT also exist.
    assert!(
        !out.path().join("ontology").exists(),
        "the root copy shipped too — two spellings of one vocabulary"
    );
    // The record says which layout this projection used, and the README's
    // module entries point at the same place.
    assert_eq!(
        report.modules_dir.as_deref(),
        Some("x0k-ontology/ontology/modules")
    );
    let prov: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(out.path().join("PROVENANCE.json")).unwrap())
            .unwrap();
    assert_eq!(prov["modules_dir"], "x0k-ontology/ontology/modules");
    let readme = std::fs::read_to_string(out.path().join("README.md")).unwrap();
    assert!(
        readme.contains("- [`x0k-ontology/ontology/modules/core.ttl`](x0k-ontology/ontology/modules/core.ttl) —"),
        "{readme}"
    );
}
```

```rust {#modules-layout-unpublished file="tests/region_repo_modules.rs"}
#[test]
fn without_the_vocabulary_crate_the_modules_stay_at_the_root() {
    let ws = workspace(&["core", "document"], true);
    let out = tempfile::tempdir().unwrap();
    let report = project(ws.path(), out.path()).expect("projection");

    for name in ["core", "document"] {
        assert!(out
            .path()
            .join("ontology/modules")
            .join(format!("{name}.ttl"))
            .is_file());
    }
    assert!(
        !out.path().join("x0k-ontology").exists(),
        "no vocabulary crate is published, so nothing goes inside one"
    );
    assert_eq!(report.modules_dir.as_deref(), Some("ontology/modules"));
    let prov: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(out.path().join("PROVENANCE.json")).unwrap())
            .unwrap();
    assert_eq!(prov["modules_dir"], "ontology/modules");
}
```

### Severing a document

A publication may hold back a single literate *document*, by the `id:` its
own envelope declares. A cargo feature can sever a crate's dependency but cannot
keep a feature-gated chapter's source out of the tarball: the chapter still
names its crate in `tangle.crate`, so discovery selects it and its `@generated`
output ships as dead code a public reader is invited to read. Excluding the
document is the whole act — the projector's existing rule, that a generated file
whose source is outside the literate set is dropped, takes the code out with it.
Nothing separate deletes anything.

```rust {#modules-excluded-document file="tests/region_repo_modules.rs"}
#[test]
fn an_excluded_document_takes_its_generated_output_with_it() {
    let ws = workspace(&[], true);
    std::fs::write(
        ws.path().join(PUB_REL),
        publication_excluding(&["demo-crate"], &[], true, &[EXTRA_ID]),
    )
    .unwrap();
    // Both chapters and both outputs exist in the corpus before the projection.
    assert!(ws.path().join(EXTRA_REL).is_file());
    assert!(ws.path().join("demo-crate/src/extra.rs").is_file());

    let out = tempfile::tempdir().unwrap();
    let report = project(ws.path(), out.path()).expect("projection");

    // The document does not travel, and neither does its sidecar.
    assert!(!out.path().join(EXTRA_REL).exists(), "the excluded chapter shipped");
    assert!(!out
        .path()
        .join("knowledge/implementation/demo/extra.tangle-map.json")
        .exists());
    // Nor its `@generated` output — dropped by the rule that was already
    // there, and recorded as dropped rather than silently missing.
    assert!(
        !out.path().join("demo-crate/src/extra.rs").exists(),
        "the excluded chapter's generated code shipped"
    );
    assert!(
        report
            .dropped_generated
            .iter()
            .any(|p| p == "demo-crate/src/extra.rs"),
        "{:?}",
        report.dropped_generated
    );
    // The sibling chapter is untouched: a severance is one document, not an area.
    assert!(out.path().join(DOC_REL).is_file());
    assert!(out.path().join("demo-crate/src/lib.rs").is_file());
    assert!(!report.literate_docs.iter().any(|p| p.ends_with("extra.md")));
    assert!(report.literate_docs.iter().any(|p| p.ends_with("colophon.md")));

    // Recorded beside the excluded crates, so a reader comparing the record to
    // the tree finds every absence accounted for.
    assert_eq!(report.excluded_docs, vec![EXTRA_ID.to_string()]);
    let prov: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(out.path().join("PROVENANCE.json")).unwrap())
            .unwrap();
    assert_eq!(prov["excluded_docs"], serde_json::json!([EXTRA_ID]));
    assert!(prov["dropped_generated"]
        .as_array()
        .unwrap()
        .iter()
        .any(|p| p == "demo-crate/src/extra.rs"));
}
```

The document grain widens what `excludes` accepts; it does not open the
edge. A URI of no recognised kind is still an error naming it, a document named
under `publishes` is refused rather than ignored — a document is not a
membership, it follows the crate it tangles to — and an `excludes` id that
matches nothing refuses too. That last one is the quiet form of the failure this
severance exists to close: an exclusion that severs nothing, silently.

```rust {#modules-unrecognised-excludes file="tests/region_repo_modules.rs"}
#[test]
fn an_unrecognised_excludes_uri_is_still_refused_naming_it() {
    let ws = workspace(&[], true);
    std::fs::write(
        ws.path().join(PUB_REL),
        publication_excluding(&["demo-crate"], &[], true, &["x0k:seed/something-else"]),
    )
    .unwrap();
    let err = project_err(ws.path());
    assert!(err.contains("x0k:seed/something-else"), "names the URI: {err}");
    assert!(err.contains("`excludes` member"), "{err}");
}
```

```rust {#modules-document-under-publishes file="tests/region_repo_modules.rs"}
#[test]
fn a_document_under_publishes_is_refused() {
    let ws = workspace(&[], true);
    std::fs::write(
        ws.path().join(PUB_REL),
        publication(&["demo-crate", "x0k:implementation/demo/extra"], &[], true)
            .replace(
                "x0k:software-module/x0k:implementation/demo/extra",
                "x0k:implementation/demo/extra",
            ),
    )
    .unwrap();
    let err = project_err(ws.path());
    assert!(err.contains("only `excludes` may name one"), "{err}");
}
```

```rust {#modules-excluded-matches-nothing file="tests/region_repo_modules.rs"}
#[test]
fn an_excluded_document_that_matches_nothing_is_refused() {
    let ws = workspace(&[], true);
    std::fs::write(
        ws.path().join(PUB_REL),
        publication_excluding(
            &["demo-crate"],
            &[],
            true,
            &["x0k:implementation/demo/typo"],
        ),
    )
    .unwrap();
    let err = project_err(ws.path());
    assert!(err.contains("x0k:implementation/demo/typo"), "{err}");
    assert!(err.contains("severs nothing"), "{err}");
}
```

### Naming a document, and a section of one

An affordance is declared inside the design that owns it, and the rest of
that design is broader than the repository. So the publication names the
section, at the address the format already has, and gets a document back:
the section's own prose and its affordance fence, under an envelope whose
id is the parent's qualified by the anchor and whose one edge names the
document it was cut from. Everything else about that design — its
context, and the sibling affordance this bundle could not honestly claim
— stays behind.

```rust {#modules-named-section file="tests/region_repo_modules.rs"}
#[test]
fn a_named_section_crosses_as_a_document_and_the_rest_of_its_design_does_not() {
    let ws = workspace(&[], true);
    let reference = format!("{DESIGN_ID}#{SHIPPABLE}");
    std::fs::write(
        ws.path().join(PUB_REL),
        publication_publishing(&["demo-crate"], &[reference.as_str()]),
    )
    .unwrap();

    let out = tempfile::tempdir().unwrap();
    let report = project(ws.path(), out.path()).expect("projection");

    // Beside the document's own path, under its stem: where a section came
    // from is legible from the tree.
    let rel = format!("decisions/design/corpus/demo-design/{SHIPPABLE}.md");
    let text = std::fs::read_to_string(out.path().join(&rel)).expect("the section shipped");

    // A fragment carrying no envelope is not a folio document.
    assert!(text.contains(&format!("id: {reference}")), "{text}");
    assert!(text.contains("type: design"), "{text}");
    assert!(text.contains("status: proposed"), "{text}");
    assert!(
        text.contains(&format!("transcludes:\n      - {DESIGN_ID}")),
        "the edge naming the document it was cut from: {text}"
    );
    // The section itself, heading and all, with its affordance declaration.
    assert!(text.contains("### Read a line out of a document"), "{text}");
    assert!(text.contains("x0k:affordance/read_a_line"), "{text}");

    // The sibling affordance names a crate this publication does not ship,
    // and section granularity is exactly what keeps it out.
    assert!(!tree_carries(out.path(), "run_the_whole_fleet"));
    // Nor does the design's own prose cross, nor the whole document.
    assert!(!tree_carries(out.path(), "Context this repository has no use for"));
    assert!(!out.path().join(DESIGN_REL).exists());

    // Recorded twice, as every membership is: in the report a publisher
    // reads, and in the provenance a receiver routes an edit back through.
    assert_eq!(report.documents.get(&reference), Some(&rel));
    let prov: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(out.path().join("PROVENANCE.json")).unwrap())
            .unwrap();
    assert_eq!(prov["documents"][&reference], serde_json::json!(rel));
    assert_eq!(prov["path_map"][&rel], serde_json::json!(DESIGN_REL));
}
```

Naming the document without an anchor takes the whole of it, at its own
path — the same address, read the way a transclusion with no anchor reads
it. The whole design carries the fleet affordance too, so the publication
has to say, in `excludes`, that the crate it names is one the audience
will not have; the closure rule below is what asks.

```rust {#modules-named-whole-document file="tests/region_repo_modules.rs"}
#[test]
fn a_document_named_without_an_anchor_crosses_whole() {
    let ws = workspace(&[], true);
    std::fs::write(
        ws.path().join(PUB_REL),
        publication_full(
            &["demo-crate"],
            &[],
            true,
            &["x0k:software-module/unshipped-crate"],
            &[DESIGN_ID],
        ),
    )
    .unwrap();

    let out = tempfile::tempdir().unwrap();
    let report = project(ws.path(), out.path()).expect("projection");

    let text = std::fs::read_to_string(out.path().join(DESIGN_REL)).expect("the design shipped");
    assert_eq!(text, demo_design(), "a whole document crosses verbatim");
    assert_eq!(report.documents.get(DESIGN_ID), Some(&DESIGN_REL.to_string()));
}
```

Selection is default-deny, and this is the test that says so: the design
sits in the corpus for every projection in this file, and crosses in
exactly the two above. Nothing reachable from a published crate, and
nothing under `decisions/`, drags it out.

```rust {#modules-unnamed-document file="tests/region_repo_modules.rs"}
#[test]
fn a_decision_document_the_publication_did_not_name_does_not_cross() {
    let ws = workspace(&[], true);
    assert!(ws.path().join(DESIGN_REL).is_file(), "the fixture is in the corpus");

    let out = tempfile::tempdir().unwrap();
    let report = project(ws.path(), out.path()).expect("projection");

    assert!(report.documents.is_empty(), "{:?}", report.documents);
    assert!(!out.path().join("decisions").exists());
    assert!(!tree_carries(out.path(), DESIGN_ID));
    assert!(!tree_carries(out.path(), "x0k:affordance/read_a_line"));
}
```

And an anchor that heads no section is **refused, not skipped** — the same
rule an `excludes` id matching no document already gets, for the same
reason: a name that silently selects nothing is the defect. The message
names the anchor and lists the ones the document offers, because the slug
is minted from the heading text and capped, so the anchor a reader
guesses is not always the one that resolves.

```rust {#modules-anchor-matches-nothing file="tests/region_repo_modules.rs"}
#[test]
fn an_anchor_that_heads_no_section_is_refused_naming_it() {
    let ws = workspace(&[], true);
    std::fs::write(
        ws.path().join(PUB_REL),
        publication_publishing(&["demo-crate"], &["x0k:design/demo-design#read-a-line-out-of-a-file"]),
    )
    .unwrap();
    let err = project_err(ws.path());
    assert!(err.contains("read-a-line-out-of-a-file"), "names the anchor: {err}");
    assert!(err.contains("heads no section"), "{err}");
    // And tells the reader what the document does offer.
    assert!(err.contains(SHIPPABLE), "{err}");
}
```

The id half is refused the same way, and by the document's own `id:`
rather than by its filename: a publication naming a document the corpus
does not declare has selected nothing.

```rust {#modules-document-id-matches-nothing file="tests/region_repo_modules.rs"}
#[test]
fn a_named_document_no_corpus_document_declares_is_refused() {
    let ws = workspace(&[], true);
    std::fs::write(
        ws.path().join(PUB_REL),
        publication_publishing(&["demo-crate"], &["x0k:design/demo-desgin#brief"]),
    )
    .unwrap();
    let err = project_err(ws.path());
    assert!(err.contains("x0k:design/demo-desgin"), "{err}");
    assert!(err.contains("selects nothing"), "{err}");
}
```

What a publication affords is derived from the modules it publishes, so a
declaration it discloses may not name a module the audience will not have.
The fleet affordance names a crate this publication does not ship, and
naming its section is refused with the affordance, the reference that
published it, and the crate — everything a publisher needs to either ship
the crate or sever it by name.

```rust {#modules-affordance-closure-refused file="tests/region_repo_modules.rs"}
#[test]
fn a_published_affordance_naming_an_unshipped_module_is_refused() {
    let ws = workspace(&[], true);
    let reference = format!("{DESIGN_ID}#run-the-whole-fleet");
    std::fs::write(
        ws.path().join(PUB_REL),
        publication_publishing(&["demo-crate"], &[reference.as_str()]),
    )
    .unwrap();
    let err = project_err(ws.path());
    assert!(err.contains("x0k:affordance/run_the_whole_fleet"), "{err}");
    assert!(err.contains(&reference), "names what published it: {err}");
    assert!(err.contains("unshipped-crate"), "{err}");
    assert!(err.contains("neither publishes nor excludes"), "{err}");
}
```

Severing the crate by name is the publication saying so, and then the
declaration crosses: the audience reads an affordance whose enabling module
the publication has told them, in the same edge that severs a dependency,
they do not get.

```rust {#modules-affordance-closure-excluded file="tests/region_repo_modules.rs"}
#[test]
fn a_published_affordance_naming_an_excluded_module_crosses() {
    let ws = workspace(&[], true);
    let reference = format!("{DESIGN_ID}#run-the-whole-fleet");
    std::fs::write(
        ws.path().join(PUB_REL),
        publication_full(
            &["demo-crate"],
            &[],
            true,
            &["x0k:software-module/unshipped-crate"],
            &[reference.as_str()],
        ),
    )
    .unwrap();
    let out = tempfile::tempdir().unwrap();
    project(ws.path(), out.path()).expect("an excluded module is a stated absence");
    assert!(tree_carries(out.path(), "run_the_whole_fleet"));
}
```

### The contents page

An area's teaching order lives in no field of any envelope, so the contents
marker may carry one: a line per area, `<area>: <stem> …`. It names a *spine*,
not a membership list — a named chapter is pulled ahead of path order, and
everything the spine does not name still ships, after the named ones, by path.

```rust {#modules-reading-order file="tests/region_repo_modules.rs"}
#[test]
fn the_contents_marker_may_author_a_reading_order() {
    let ws = workspace(&[], true);
    let doc = std::fs::read_to_string(ws.path().join(PUB_REL)).unwrap();
    std::fs::write(
        ws.path().join(PUB_REL),
        doc.replace(
            "<!-- x0k:contents -->",
            "<!-- x0k:contents\ndemo: extra\n-->",
        ),
    )
    .unwrap();
    let out = tempfile::tempdir().unwrap();
    project(ws.path(), out.path()).expect("projection");
    let readme = std::fs::read_to_string(out.path().join("README.md")).unwrap();
    let extra = readme.find("demo/extra.md").expect("extra is listed");
    let colophon = readme.find("demo/colophon.md").expect("colophon is listed");
    assert!(
        extra < colophon,
        "the spine orders `extra` ahead of the path order:\n{readme}"
    );
}
```

A spine name that ships nothing is stale, and stale is silent unless it
refuses — the ordering would simply stop applying and nobody would see it. The
same holds for an area the publication ships nothing from: an ordering with no
subject.

```rust {#modules-reading-order-unshipped-doc file="tests/region_repo_modules.rs"}
#[test]
fn a_reading_order_naming_an_unshipped_document_is_refused() {
    let ws = workspace(&[], true);
    let doc = std::fs::read_to_string(ws.path().join(PUB_REL)).unwrap();
    std::fs::write(
        ws.path().join(PUB_REL),
        doc.replace(
            "<!-- x0k:contents -->",
            "<!-- x0k:contents\ndemo: colophon renamed-away\n-->",
        ),
    )
    .unwrap();
    let err = project_err(ws.path());
    assert!(
        err.contains("renamed-away") && err.contains("`demo`"),
        "names the stale stem and its area: {err}"
    );
}
```

```rust {#modules-reading-order-unshipped-area file="tests/region_repo_modules.rs"}
#[test]
fn a_reading_order_naming_an_unshipped_area_is_refused() {
    let ws = workspace(&[], true);
    let doc = std::fs::read_to_string(ws.path().join(PUB_REL)).unwrap();
    std::fs::write(
        ws.path().join(PUB_REL),
        doc.replace("<!-- x0k:contents -->", "<!-- x0k:contents\nfolio: format\n-->"),
    )
    .unwrap();
    let err = project_err(ws.path());
    assert!(err.contains("`folio`"), "names the absent area: {err}");
}
```

The concept form is what the book uses. A `# ` line heads a group, the `> `
line under it is the group's own sentence, and the members are
`<area>/<stem>` — so the page reads as concepts rather than as crates, and a
crate is visible only where it always was, in a chapter's own path.

```rust {#modules-concept-groups file="tests/region_repo_modules.rs"}
#[test]
fn concept_groups_head_the_page_verbatim_with_their_blurbs() {
    let ws = workspace(&[], true);
    let doc = std::fs::read_to_string(ws.path().join(PUB_REL)).unwrap();
    std::fs::write(
        ws.path().join(PUB_REL),
        doc.replace(
            "<!-- x0k:contents -->",
            "<!-- x0k:contents\n# What the demo is\n> Two chapters, and the order they teach in.\n  demo/extra\n  demo/colophon\n-->",
        ),
    )
    .unwrap();
    let out = tempfile::tempdir().unwrap();
    project(ws.path(), out.path()).expect("projection");
    let readme = std::fs::read_to_string(out.path().join("README.md")).unwrap();
    let expected = "\
### What the demo is

Two chapters, and the order they teach in.

- [The extra chapter](knowledge/implementation/demo/extra.md) — A second chapter, so a document severance has something to take out.
- [The demo colophon](knowledge/implementation/demo/colophon.md) — The demo crate's one exported function, and where it trims.
";
    assert!(readme.contains(expected), "the grouped contents page:\n{readme}");
    assert!(
        !readme.contains("### `demo-crate`"),
        "the crate no longer heads a section:\n{readme}"
    );
}
```

Four refusals guard the concept form, and the two that matter most are the
ones the spine could not make. A member that ships nothing is stale, as it
always was. A shipped document that *no* group names is the chapter that
would drop out of the book unseen — the spine listed it by path and looked
complete. A document two groups both claim is the same chapter printed twice
under contradictory concepts. And a marker carrying both forms is two
different promises about one page, so it refuses rather than half-applying
either.

```rust {#modules-group-unshipped-member file="tests/region_repo_modules.rs"}
#[test]
fn a_group_naming_an_unshipped_document_is_refused() {
    let err = project_err_with_marker(
        "<!-- x0k:contents\n# Everything\n  demo/colophon\n  demo/extra\n  demo/renamed-away\n-->",
    );
    assert!(
        err.contains("demo/renamed-away") && err.contains("Everything"),
        "names the stale member and the group that names it: {err}"
    );
}
```

```rust {#modules-group-unnamed-document file="tests/region_repo_modules.rs"}
#[test]
fn a_shipped_document_that_no_group_names_is_refused() {
    let err = project_err_with_marker("<!-- x0k:contents\n# Half of it\n  demo/colophon\n-->");
    assert!(
        err.contains("demo/extra"),
        "names the chapter that would have dropped out of the book: {err}"
    );
}
```

```rust {#modules-group-claimed-twice file="tests/region_repo_modules.rs"}
#[test]
fn a_document_two_groups_both_claim_is_refused() {
    let err = project_err_with_marker(
        "<!-- x0k:contents\n# First\n  demo/colophon\n  demo/extra\n# Second\n  demo/extra\n-->",
    );
    assert!(
        err.contains("demo/extra") && err.contains("First") && err.contains("Second"),
        "names the document and both groups: {err}"
    );
}
```

```rust {#modules-group-mixed-forms file="tests/region_repo_modules.rs"}
#[test]
fn a_marker_mixing_groups_with_a_reading_order_is_refused() {
    let err = project_err_with_marker(
        "<!-- x0k:contents\n# Everything\n  demo/colophon\n  demo/extra\ndemo: extra\n-->",
    );
    assert!(err.contains("mixes"), "refuses rather than half-applying either: {err}");
}
```

The last two refusals are both about a README that would be quietly less
than the repository. A publication with documents and modules to list and no
marker has nowhere to put them; a document with no `summary:` cannot be
described, and a link with no description is exactly the degradation the
contents page exists to prevent.

```rust {#modules-no-contents-marker file="tests/region_repo_modules.rs"}
#[test]
fn a_publication_with_no_contents_marker_is_refused() {
    let ws = workspace(&["core", "document"], true);
    let doc = std::fs::read_to_string(ws.path().join(PUB_REL)).unwrap();
    std::fs::write(
        ws.path().join(PUB_REL),
        doc.replace("<!-- x0k:contents -->\n\n", ""),
    )
    .unwrap();
    let err = project_err(ws.path());
    assert!(
        err.contains("x0k:contents") && err.contains("2 vocabulary module"),
        "names the missing marker and what had nowhere to go: {err}"
    );
}
```

```rust {#modules-document-without-summary file="tests/region_repo_modules.rs"}
#[test]
fn a_document_without_a_summary_is_refused_naming_it() {
    let ws = workspace(&[], true);
    let doc = std::fs::read_to_string(ws.path().join(EXTRA_REL)).unwrap();
    let stripped: String = doc
        .lines()
        .filter(|l| !l.starts_with("  summary:"))
        .map(|l| format!("{l}\n"))
        .collect();
    std::fs::write(ws.path().join(EXTRA_REL), stripped).unwrap();
    let err = project_err(ws.path());
    assert!(
        err.contains("demo/extra.md") && err.contains("summary"),
        "names the undescribed document: {err}"
    );
}
```


### The affordance figures

The README's second marker asks for a figure per affordance the publication
publishes. The shippable section of the demo design is one declaration —
claimed for a human, enabled by the demo crate, with no signifier anywhere
in the fixture — so the projection draws one pair, light and dark, under
`affordances/`, and the README carries a `<picture>` choosing between them
with the caption under it. The caption is the record in a sentence: the
title, who it is for, that no cue has been declared, what enables it, and
its status. The figure files are named in the report, not in the provenance
seam: nothing in the corpus produced them.

```rust {#modules-affordance-figure file="tests/region_repo_modules.rs"}
#[test]
fn the_affordances_marker_draws_one_figure_pair_per_published_declaration() {
    let ws = workspace(&[], true);
    let reference = format!("{DESIGN_ID}#{SHIPPABLE}");
    std::fs::write(
        ws.path().join(PUB_REL),
        with_affordances_marker(&publication_publishing(&["demo-crate"], &[reference.as_str()])),
    )
    .unwrap();
    let out = tempfile::tempdir().unwrap();
    let report = project(ws.path(), out.path()).expect("projection");

    let light = std::fs::read_to_string(out.path().join("affordances/read-a-line-light.svg"))
        .expect("the light figure is drawn");
    let dark = std::fs::read_to_string(out.path().join("affordances/read-a-line-dark.svg"))
        .expect("the dark figure is drawn");
    for svg in [&light, &dark] {
        assert!(svg.contains("role=\"img\""), "{svg}");
        assert!(svg.contains(">Read a line out of a document<"), "the title is drawn: {svg}");
        assert!(svg.contains(">a person<"), "the claimed actor is drawn: {svg}");
        assert!(svg.contains(">demo-crate<"), "the enabling crate is drawn: {svg}");
        assert!(svg.contains("no signifier declared"), "an undeclared cue is drawn as such: {svg}");
        assert!(svg.contains(">status: wip<"), "{svg}");
    }
    // The README's own two palettes, not a third.
    assert!(light.contains("Georgia") && light.contains("#b88e44"), "paper and gold:\n{light}");
    assert!(dark.contains("monospace") && dark.contains("#e2e8f0"), "surface and slate:\n{dark}");

    let readme = std::fs::read_to_string(out.path().join("README.md")).unwrap();
    let expected = "\
## What you can do here

<picture>
  <source media=\"(prefers-color-scheme: dark)\" srcset=\"affordances/read-a-line-dark.svg\">
  <img alt=\"Read a line out of a document — for a person; no signifier declared; enabled by demo-crate; status wip.\" src=\"affordances/read-a-line-light.svg\">
</picture>

**Read a line out of a document** — for a person; no signifier declared; enabled by `demo-crate`; status wip.

## Afterwards
";
    assert!(readme.contains(expected), "the figure and its caption stand where the marker was:\n{readme}");
    assert_eq!(
        report.figures.get("x0k:affordance/read_a_line").map(String::as_str),
        Some("affordances/read-a-line-light.svg")
    );
    let prov = std::fs::read_to_string(out.path().join("PROVENANCE.json")).unwrap();
    assert!(!prov.contains("affordances/"), "a figure has no corpus source to record: {prov}");
}
```

A signifier is declared where its face lives. The fixture's verb chapter is a
prose-only document in the demo crate's area — it ships because its area
does, never by name — and the signifier in it says `demo-line` on the CLI
signifies the shippable affordance. That is enough for the figure to grow a
surface pill and the caption a clause, and for the "no signifier" note to go.

```rust {#modules-affordance-signifier file="tests/region_repo_modules.rs"}
#[test]
fn a_signifier_in_a_shipped_chapter_puts_its_surface_on_the_figure() {
    let ws = workspace(&[], true);
    declare_signifier(ws.path());
    let reference = format!("{DESIGN_ID}#{SHIPPABLE}");
    std::fs::write(
        ws.path().join(PUB_REL),
        with_affordances_marker(&publication_publishing(&["demo-crate"], &[reference.as_str()])),
    )
    .unwrap();
    let out = tempfile::tempdir().unwrap();
    project(ws.path(), out.path()).expect("projection");

    let readme = std::fs::read_to_string(out.path().join("README.md")).unwrap();
    assert!(
        readme.contains("for a person; reachable on `cli` as `demo-line`; enabled by `demo-crate`"),
        "the caption names the surface and the cue:\n{readme}"
    );
    assert!(!readme.contains("no signifier declared"), "{readme}");

    let svg = std::fs::read_to_string(out.path().join("affordances/read-a-line-light.svg")).unwrap();
    assert!(svg.contains(">cli</tspan>"), "the surface heads the pill: {svg}");
    assert!(svg.contains("demo-line</tspan>"), "the cue follows it: {svg}");
    assert!(!svg.contains("no signifier declared"), "{svg}");
}
```

The marker is opt-in, and a marker that renders nothing is refused on the
rule the contents marker set: a README asking for figures in a publication
that names no declaration would silently say less than it promises. Without
the marker nothing is drawn — no directory, no report line, no `<picture>`.

```rust {#modules-affordance-marker-empty file="tests/region_repo_modules.rs"}
#[test]
fn an_affordances_marker_with_nothing_to_draw_is_refused_naming_it() {
    let ws = workspace(&[], true);
    let doc = std::fs::read_to_string(ws.path().join(PUB_REL)).unwrap();
    std::fs::write(ws.path().join(PUB_REL), with_affordances_marker(&doc)).unwrap();
    let err = project_err(ws.path());
    assert!(err.contains("<!-- x0k:affordances -->"), "names the marker: {err}");
    assert!(err.contains("no affordance declaration"), "{err}");
}
```

```rust {#modules-affordance-no-marker file="tests/region_repo_modules.rs"}
#[test]
fn without_the_marker_no_figure_is_drawn() {
    let ws = workspace(&[], true);
    let reference = format!("{DESIGN_ID}#{SHIPPABLE}");
    std::fs::write(
        ws.path().join(PUB_REL),
        publication_publishing(&["demo-crate"], &[reference.as_str()]),
    )
    .unwrap();
    let out = tempfile::tempdir().unwrap();
    let report = project(ws.path(), out.path()).expect("projection");
    assert!(!out.path().join("affordances").exists(), "no marker, no directory");
    assert!(report.figures.is_empty(), "{:?}", report.figures);
    let readme = std::fs::read_to_string(out.path().join("README.md")).unwrap();
    assert!(!readme.contains("<picture"), "{readme}");
}
```

```rust {#modules-root file="tests/region_repo_modules.rs"}
<<modules-doc>>

<<modules-uses>>

<<modules-consts>>

<<modules-publication-fixture>>

<<modules-write-crate>>

<<modules-workspace>>

<<modules-project-helpers>>

<<modules-closed-selection>>

<<modules-shapes-travel>>

<<modules-import-outside-selection>>

<<modules-import-absent>>

<<modules-instance-line>>

<<modules-not-in-tree>>

<<modules-ontology-without-module>>

<<modules-stamp-fallback>>

<<modules-no-module-still-lists>>

<<modules-layout-published>>

<<modules-layout-unpublished>>

<<modules-excluded-document>>

<<modules-unrecognised-excludes>>

<<modules-document-under-publishes>>

<<modules-excluded-matches-nothing>>

<<modules-named-section>>

<<modules-named-whole-document>>

<<modules-unnamed-document>>

<<modules-anchor-matches-nothing>>

<<modules-document-id-matches-nothing>>

<<modules-affordance-closure-refused>>

<<modules-affordance-closure-excluded>>

<<modules-reading-order>>

<<modules-reading-order-unshipped-doc>>

<<modules-reading-order-unshipped-area>>

<<modules-concept-groups>>

<<modules-group-unshipped-member>>

<<modules-group-unnamed-document>>

<<modules-group-claimed-twice>>

<<modules-group-mixed-forms>>

<<modules-no-contents-marker>>

<<modules-document-without-summary>>

<<modules-affordance-figure>>

<<modules-affordance-signifier>>

<<modules-affordance-marker-empty>>

<<modules-affordance-no-marker>>
```

## Composing the module

```rust {#root}
<<module-doc>>

<<uses>>

<<constants>>

<<options>>

<<report>>

<<module-version-source>>

<<license-source>>

<<project-publication-repo>>

<<overlay-paths>>

<<previous-provenance-field>>

<<overlay-stash>>

<<restore-overlay>>

<<clear-regenerated-region>>

<<member-names>>

<<envelope-scalar>>

<<envelope-string-list>>

<<manifest-readers>>

<<path-deps>>

<<vendor-crate>>

<<rewrite-vendored-manifest>>

<<vocab-module>>

<<modules-rel-dir>>

<<discover-literate-docs-fn>>

<<copy-literate-docs>>

<<copy-sidecar>>

<<doc-selection>>

<<resolve-named-document>>

<<project-named-documents>>

<<section-document>>

<<write-projected-documents>>

<<affordance-closure>>

<<affordance-record>>

<<affordance-records>>

<<affordance-caption>>

<<affordance-palette>>

<<affordance-figure>>

<<emit-workspace-manifest>>

<<license-files>>

<<emit-licenses>>

<<generate-lockfile>>

<<tangle-readme>>

<<write-readme-contents>>

<<write-readme-affordances>>

<<emit-ci-and-guard>>

<<emit-provenance>>

<<current-corpus-rev>>

<<current-corpus-commit>>

<<git-run-and-commit>>

<<projection-message>>

<<git-init-and-reproject>>

<<license-texts>>

<<ci-script>>

<<deny-config>>

<<toolchain-file>>

<<workflow-wrappers>>

<<guard-script>>

<<tests>>
```

## What the tests pin, and what stays hard

`tests/integration/tests/publication_repo_bootstrap.rs` projects the
carried example and pins the contract from the outside: the license
comes from the publication doc and is recorded, with exactly the
bodies its expression names (`LICENSE-MIT` alone for `MIT`; a scratch
`MIT OR Apache-2.0` publication keeps the dual path covered, and an
identifier with no body refuses); a publication without a license
refuses; the README is byte-for-byte the tangled chunk of the
publication doc under the `@generated` header with its contents
marker replaced by the generated page, and no corpus identifiers in
it; a publication that tangles no README refuses; a document with no
`summary:` refuses; a reading-order name that ships nothing refuses;
`PROVENANCE.json` carries the crate, excluded, and literate-document
lists; tangled sources and generated outputs arrive in pairs while a
prose-only chapter in a published area arrives alone; a vendored
`@generated` file from outside the literate set is dropped and named
in `dropped_generated`; a severed feature stays declared, empty and
annotated, and `[package.metadata.x0k]` does not ship; the lockfile is
committed and every crate carries the license text with its notice
line; an overlay path the publication doc seeds is written once,
without a `@generated` header, and never overwritten; a chunk routed
anywhere else refuses; the CI contract is forge-agnostic with GitHub
optional; a re-projection appends to the existing history; overlay
paths survive re-projection while other hand edits do not; and an
overlay entry must be a plain relative path.
The shipped set — the vocabulary modules, the severed document, the
contents page, and the affordance figures — is pinned above against the fixture under
`tests/fixtures/ontology-modules/`, over projections this chapter's own
projector makes.
`tests/integration/tests/publication_self_tangle.rs` closes the circle
this chapter opened with: it projects the bundle, re-tangles every
document inside the projection with the in-tree tangler — the same
work `tools/ci` does — and requires an unchanged tree. It lives in the
monorepo's integration crate, not in `x0k-tangle/tests`, because it is
a test *of the publication* and must not ship inside it.

The hard part is not the vendoring; it is that the projector's output
is judged by a tangler it does not control. Any byte the tangler
inside the projection writes differently — a sidecar field order, a
header format, a `source` path shape — is drift the public CI reports
against the maintainer's own repository. The projector cannot check
that property by reading code; it can only produce it by reusing the
tangler's own types, and let the self-tangle test say when that stops
being enough.
