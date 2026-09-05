---
x0k:
  format: folio/v1
  id: x0k:implementation/tangle/crate
  type: implementation
  status: draft
  summary: The crate's contract rather than a mechanism — the module list and re-exports that say what a consumer may name, and the plugin-less CLI that puts those verbs in a shell.
  concerns: [tangle, literate, crate, cli, features, publishing]
  tangle:
    crate: x0k-tangle
    root: src/lib.rs
  edges:
    implements:
      - x0k:design/literate-programming
      - x0k:design/publish-a-region-as-a-repository
    cites:
      - x0k:implementation/tangle/protocol
      - x0k:implementation/tangle/parsing
      - x0k:implementation/tangle/resolution
      - x0k:implementation/tangle/identity-pipeline
      - x0k:implementation/tangle/dispatcher
      - x0k:implementation/tangle/weave
      - x0k:implementation/tangle/region-project
      - x0k:implementation/tangle/region-repo
      - x0k:implementation/tangle/publishing
      - x0k:implementation/tangle/receiving
      - x0k:implementation/tangle/cli-faces
      - x0k:implementation/tangle/bundle
    presupposes:
      - x0k:wiki/literate-programming
---
# x0k-tangle: the crate and its CLI

`x0k-tangle` is the crate that makes a literate document executable:
it parses the folio/v1 pages under `knowledge/implementation/`, expands
their named chunks into source files, weaves them into HTML, and — the
outward-facing half — projects a whole publication region into a reader
site or a buildable public repository, and receives what comes back.
This chapter is the crate's contract: the module list and re-exports in
`src/lib.rs` that say what a consumer may name, and the plugin-less CLI
in `src/main.rs` that exposes those verbs to a shell. Everything with a
mechanism worth deriving lives in a sibling chapter; this one is the
map and the face.

The crate reads as chapters, each owning one idea. Reading order for a
newcomer is [`protocol.md`](protocol.md) first, then inward to outward:

- [`chunk.md`](chunk.md), [`parsing.md`](parsing.md),
  [`chunk-refs.md`](chunk-refs.md) — a document becomes named chunks
  and the references between them.
- [`resolution.md`](resolution.md),
  [`multi-doc-resolve.md`](multi-doc-resolve.md) — `<<refs>>` expand,
  within one document and across the corpus.
- [`source-refs.md`](source-refs.md),
  [`source-sync.md`](source-sync.md),
  [`reverse-stitch.md`](reverse-stitch.md) — the reverse direction:
  code pulled into documents, and edited outputs read back through
  their sidecars.
- [`pipeline.md`](pipeline.md),
  [`identity-pipeline.md`](identity-pipeline.md),
  [`dispatcher.md`](dispatcher.md) — tangling as one plugin among
  many, and the runner that discovers, dispatches, and tracks
  freshness.
- [`weave.md`](weave.md), [`region-weave.md`](region-weave.md),
  [`presentation.md`](presentation.md), [`atlas.md`](atlas.md),
  [`region-project.md`](region-project.md) — one document, then a
  region, rendered as HTML and wrapped in the canvas shell.
- [`doc-index.md`](doc-index.md) — the corpus seen from outside.
- [`region-repo.md`](region-repo.md), [`publishing.md`](publishing.md),
  [`receiving.md`](receiving.md) — the repository projector, the
  irreversible publish step, and the inbound contribution.
- [`cli-faces.md`](cli-faces.md) — the vocabulary check and the
  affordance read-out behind the `check` and `affordances` verbs, the
  two that make a shipped human claim true from a shell.

One document threads through those chapters: the publication manifest
`decisions/publications/x0k-folio.md`, which names this
crate among the four it publishes. The parser reads it as a folio/v1
page, `project-repo` projects it as a repository, and the repository's
own CI runs the `x0k-tangle` built from that projection over the
literate documents that produced it — this chapter's own `lib.rs`
included.

## Two builds, one feature

The monorepo build and the published build differ by one cargo
feature. `motifs` wires `x0k-surface-build` into the HTML region
weaver so `x0k:media` embeds are bundled as canvas wasm. In the
monorepo it is on by default. In a projected repository it is
severed: the surface-build crate is publish-excluded and never
published, so the projector's manifest rewrite cuts the optional
dependency out from under the feature, the projection builds as the
monorepo's `--no-default-features` build does, and the HTML weave
degrades embeds to their static labels. The repository backend needs
neither motifs nor syntax highlighting and is feature-independent —
which is what lets the crate publish itself.

## The crate surface

Every module is public: the crate is a library of parts as much as a
pipeline, and downstream consumers (the monorepo's plugin-bundle binary
and its dev-daemon tools) reach into `parser`, `resolve`, and `weave`
by path.

The crate root opens with the documentation a `docs.rs` reader or a
`cargo doc` browser sees first: what the crate is, the three verbs a
user needs, and where the document format is specified.

```rust {#crate-doc}
//! Literate programming for folio/v1 documents: tangle a document's
//! named code chunks into source files, weave it into HTML, and
//! reconcile edits made on either side.
//!
//! A literate document is a markdown page whose frontmatter declares a
//! `tangle:` block and whose fenced code blocks carry chunk names
//! (`{#name}`), file targets (`file="…"`), and `<<references>>` to
//! other chunks. The format is specified in the `protocol` chapter of
//! the crate's own literate source (`knowledge/implementation/tangle/`),
//! which this crate is tangled from.
//!
//! The three verbs a user needs, as the `x0k-tangle` binary exposes
//! them and as the library entry points behind them:
//!
//! - **tangle** — [`tangle_document`] / [`tangle_workspace`]: expand
//!   every chunk to its output file, writing a `.tangle-map.json`
//!   sidecar next to the document that records what was produced.
//! - **weave** — [`weave::weave_html`]: render the document, prose and
//!   highlighted code together, as a single HTML page.
//! - **check** — [`resolve::check_all_refs`] and
//!   [`faces::check_vocabulary`]: verify every chunk reference resolves
//!   and no reference cycle exists, and read every folio/v1 envelope
//!   against the vocabulary this build compiled, without writing
//!   anything.
//!
//! A fourth, **affordances** — [`faces::declared_affordances`] — reads
//! the affordance declarations out of a document as data.
//!
//! Everything else in the crate builds outward from those: the
//! pipeline protocol that lets other generators ride the same
//! dispatcher, the region weaver that renders a whole publication as a
//! site, and the repository projector that turns one into a buildable
//! public repository.
```

```rust {#modules}
pub mod atlas;
pub mod chunk;
pub mod chunk_refs;
pub mod faces;
pub mod identity_pipeline;
pub mod index;
pub mod multi_doc_resolve;
pub mod parser;
pub mod pipeline;
pub mod pipeline_runner;
pub mod presentation;
pub mod region_project;
pub mod publish_repo;
pub mod receive;
pub mod region_repo;
pub mod region_weave;
pub mod resolve;
pub mod source_ref;
pub mod stitch;
pub mod sync;
pub mod weave;
```

The re-exports are the names a consumer is expected to use without
knowing the module layout: the atlas, the pipeline protocol and its
runner, the presentation shell's file names, and the four outward
verbs — project a region to HTML, project it to a repository, publish
that repository, receive a contribution from it.

```rust {#exports}
pub use atlas::{
    atlas_json, build_atlas, Atlas, AtlasEdge, AtlasNode, AtlasPlacement, YearSource, ATLAS_FILE,
};
pub use identity_pipeline::{IdentityPipeline, IDENTITY_KIND};
pub use pipeline::{
    ChunkInput, ChunkVariant, CommentStyle, PipelineContext, PipelineError, PipelineErrorKind,
    PipelineOutput, PipelineRegistry, TanglePipeline,
};
pub use pipeline_runner::{
    doc_freshness, tangle_directory, tangle_document, tangle_workspace, DirtyReason, DocFreshness,
    PipelineRunOutput, TangleResult, WorkspaceTangleReport,
};
pub use presentation::{
    apply_publication_shell, build_members_json, BOOT_FILE, FALLBACK_DIR, MEMBERS_FILE,
    NARRATIVE_FILE, SHELL_FILE,
};
pub use region_project::{
    parse_publication_region, project_publication, project_publication_content, RegionProjectReport,
};
pub use publish_repo::{publish_repo, PublishRepoOptions, PublishRepoReport};
pub use receive::{receive_repo, ReceiveOptions, ReceiveReport};
pub use region_repo::{
    project_publication_repo, LicenseSource, RepoProjectOptions, RepoProjectReport,
};
pub use region_weave::{
    build_uri_to_path, rewrite_cross_doc_links, validate_artifact, weave_region, ArtifactFile,
    RegionInput, RegionMember, RegionWeaveOutput, UnresolvedLink,
};
```

## The CLI face

`src/main.rs` is the *protocol* binary: it ships only the built-in
`PipelineRegistry::default()`, which carries the `identity-tangle`
plugin and nothing else. A host that registers further plugins builds
its own binary around the same library (the monorepo does); this one
exists for callers that want the tangler without plugin dependencies —
the projected repository among them, where it is the only `x0k-tangle`
there is.

Four of the verbs read the publication corpus itself — the
`decisions/publications/` manifests and the decision documents they
name — and so need a corpus checkout. A projected repository carries
only the literate documents under `knowledge/implementation/`, which is
all the literate verbs need; the other eight verbs, `workspace`
included, run there unchanged.

Those four are marked `[corpus-only]` in the *first* line of their help,
which is the line `--help` prints in the command list, and the
`after_help` note below repeats the rule once for the whole binary. The
alternative was to compile them out of the published build behind a
feature. We did not, and the reason is what the published repository
already carries: the literate documents that *describe* these verbs —
[`region-repo.md`](region-repo.md), [`publishing.md`](publishing.md),
[`receiving.md`](receiving.md) — ship with it, and the README's reading
route points a reader at them. A feature gate would leave those
chapters describing commands the binary does not have, which is a worse
lie than a command that names its own precondition: one is a sentence a
reader can act on, the other is a discrepancy they can only be confused
by.

```rust {#bin-doc file="src/main.rs"}
//! The protocol-only `x0k-tangle` CLI: the crate's verbs in a shell,
//! with the built-in registry and no plugins. The binary a projected
//! repository ships and builds.
```

```rust {#cli-imports file="src/main.rs"}
use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use std::path::PathBuf;
```

```rust {#cli-struct file="src/main.rs"}
#[derive(Parser)]
#[command(
    name = "x0k-tangle",
    about = "Literate programming tangler with bidirectional sync",
    after_help = "Commands marked [corpus-only] read the publication corpus \
(decisions/publications/ and the decision documents it names). They are not \
runnable from a projected repository, which carries only the literate documents \
under knowledge/implementation/ — everything else here works there."
)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}
```

The subcommands fall into two groups. The literate verbs operate on
documents in place: `tangle`, `check`, `affordances`, `sync`, `index`,
`weave`, `list`, and `workspace`. The publication verbs operate on a
region: `weave-region` and `project-repo` are the two projection
backends, `publish-repo` the pipeline that makes a projection public,
and `receive-repo` the inbound door. Each variant's doc comment is its
`--help` text, so the clap derive below is also the user-facing
contract.

```rust {#command-enum file="src/main.rs"}
#[derive(Subcommand)]
enum Command {
    <<tangle-command>>
    <<check-command>>
    <<affordances-command>>
    <<sync-command>>
    <<index-command>>
    <<weave-command>>
    <<weave-region-command>>
    <<project-repo-command>>
    <<publish-repo-command>>
    <<receive-repo-command>>
    <<list-command>>
    <<workspace-command>>
}
```

Four of the verbs are the perceivable cue for an affordance a
publication of this crate claims for a human, and each declares that
under its own heading: a signifier is recorded where the face lives,
so the chapter that ships carries the cue with it
(`x0k:design/publish-a-region-as-a-repository`). The heading and the
prose around the block are the cue's own text, and the `///` line on
the variant — the line `--help` prints — says the same thing in the
same words.

### `x0k-tangle tangle`

Tangle `.md` documents to their source files, writing a
`.tangle-map.json` sidecar beside each: the affordance of turning a
literate document into the code it describes, from a shell.

```yaml x0k:signifier
id: x0k:signifier/x0k-tangle-tangle
edges:
  signifies:
    - x0k:affordance/tangle_source_from_a_document
  presentedOn:
    - x0k:surface/cli
```

```rust {#tangle-command file="src/main.rs"}
/// Tangle .md documents to their source files (writes .tangle-map.json sidecars)
Tangle {
    /// Paths to scan for documents with tangle: frontmatter
    paths: Vec<PathBuf>,
    /// Workspace root (defaults to current directory)
    #[arg(long)]
    workspace: Option<PathBuf>,
},
```

### `x0k-tangle check`

Verify chunk references resolve and no cycles exist, and read every
folio/v1 envelope under the paths against the vocabulary this build
compiled. The second half is the affordance of checking a document
against the vocabulary that shipped beside it, and its help text names
the two outcomes the affordance promises to tell apart: a predicate no
shipped module declares is a defect and fails the check; a target
naming no document here is an edge into the corpus the repository was
projected from, noted and expected.

```yaml x0k:signifier
id: x0k:signifier/x0k-tangle-check
edges:
  signifies:
    - x0k:affordance/check_a_document_against_shipped_vocabulary
  presentedOn:
    - x0k:surface/cli
```

```rust {#check-command file="src/main.rs"}
/// Verify chunk references resolve and no cycles exist, and read every
/// folio/v1 envelope against the vocabulary this build compiled.
///
/// Two things can go wrong with an envelope, and they are reported
/// apart. A defect — a malformed id or edge target, a predicate no
/// shipped ontology module declares, an envelope that does not parse —
/// is a gap in what this publication selected, and fails the check. An
/// edge whose target names no document under the paths is an edge into
/// the corpus this was projected from: expected, printed as a note,
/// never a failure. A third thing is checked across the set: an
/// affordance claimed for a human that no signifier signifies is a
/// defect, because the audience has nothing to perceive.
Check {
    /// Paths to scan
    paths: Vec<PathBuf>,
},
```

### `x0k-tangle affordances`

Print every affordance the documents under the paths declare, as a
JSON array on stdout: one record per `yaml x0k:affordance` block, with
its id, title, description, the document it is defined in, and its
declared facts grouped by predicate. What a declaration says becomes
data a reader's own tooling can consume, which is the affordance of
reading an affordance out of a document rather than trusting the
document's summary of itself. A block the extractor refuses is
reported on stderr and skipped.

```yaml x0k:signifier
id: x0k:signifier/x0k-tangle-affordances
edges:
  signifies:
    - x0k:affordance/read_declared_affordances
  presentedOn:
    - x0k:surface/cli
```

```rust {#affordances-command file="src/main.rs"}
/// Print every affordance the folio/v1 documents under the paths
/// declare, as a JSON array on stdout.
///
/// One record per `yaml x0k:affordance` block: `id`, `title` (the
/// enclosing heading), `description` (the prose under it), `defined_in`
/// (the parent document's id), and `facts` — every other declared fact
/// grouped by predicate, each value tagged `{"entity": …}` for an id or
/// `{"string": …}` for a literal. A block the extractor refuses is
/// reported on stderr and skipped.
Affordances {
    /// Paths to scan for folio/v1 documents
    paths: Vec<PathBuf>,
},
```

### `x0k-tangle sync`

```rust {#sync-command file="src/main.rs"}
/// Sync from= chunks: populate code blocks from source files
Sync {
    /// Paths to scan for documents with from= chunks
    paths: Vec<PathBuf>,
    /// Workspace root (defaults to current directory)
    #[arg(long)]
    workspace: Option<PathBuf>,
},
```

### `x0k-tangle index`

```rust {#index-command file="src/main.rs"}
/// Build a JSON index of all folio/v1 files
Index {
    /// Paths to scan for folio/v1 documents
    paths: Vec<PathBuf>,
    /// Workspace root (defaults to current directory)
    #[arg(long)]
    workspace: Option<PathBuf>,
    /// Output file (defaults to stdout)
    #[arg(short, long)]
    output: Option<PathBuf>,
},
```

### `x0k-tangle weave`

Weave one literate document into HTML — prose and highlighted code
together as a single page, to a directory or to stdout: the affordance
of reading a document as the woven page it describes, from a shell.

```yaml x0k:signifier
id: x0k:signifier/x0k-tangle-weave
edges:
  signifies:
    - x0k:affordance/weave_a_document
  presentedOn:
    - x0k:surface/cli
```

```rust {#weave-command file="src/main.rs"}
/// Weave a literate document into HTML
Weave {
    /// Path to a literate document
    path: PathBuf,
    /// Output directory (defaults to stdout if not set)
    #[arg(long)]
    output_dir: Option<PathBuf>,
},
```

### The publication verbs

```rust {#weave-region-command file="src/main.rs"}
/// [corpus-only] Project a publication region into a self-contained,
/// navigable multi-page web artifact.
///
/// `region` is the publication decision doc
/// (`decisions/publications/<slug>.md`, `type: publication`). Its
/// `publishes:` membership + `entryPoint:` define the region; each member's
/// decision doc is woven (wrapping the single-doc weaver), cross-doc links
/// are rewritten to artifact-relative paths, and a site nav is injected.
///
/// Reads `decisions/publications/` and the decision documents it names,
/// so it needs a corpus checkout; a projected repository carries only
/// `knowledge/implementation/` and this verb refuses there.
WeaveRegion {
    /// Path to the publication decision doc.
    region: PathBuf,
    /// Directory to write the artifact into (created if absent).
    #[arg(long)]
    output_dir: PathBuf,
    /// Workspace root the `decisions/<subtype>/...` tree hangs off of
    /// (defaults to current directory). Member sources + motif scanning
    /// resolve against this.
    #[arg(long)]
    workspace: Option<PathBuf>,
    /// Skip motif wasm bundling (page/nav/link-rewrite core only). Motif
    /// refs are still reported but no `.wasm`/`host.js` is emitted.
    #[arg(long)]
    no_motifs: bool,
},
```

`project-repo` has no silent license default: the flag is an explicit
override of the publication doc's `license:` field, and with neither the
projection refuses ([`region-repo.md`](region-repo.md)). `--allow-dirty`
is the escape hatch past the disclosure and closure guards, for
inspecting a projection that is not yet clean.

```rust {#project-repo-command file="src/main.rs"}
/// [corpus-only] Project a publication region into a standalone,
/// buildable Cargo repository.
///
/// Literate `.md` source + committed tangled code + workspace manifest +
/// the publication's declared license + README + CI, git-init'd. Sibling
/// to `weave-region` (which emits an HTML reader site).
///
/// Reads `decisions/publications/` and the decision documents it names,
/// so it needs a corpus checkout; a projected repository carries only
/// `knowledge/implementation/` and this verb refuses there.
ProjectRepo {
    /// Path to the publication decision doc (`type: publication`).
    region: PathBuf,
    /// Directory to write the standalone repo into (created if absent).
    #[arg(long)]
    output_dir: PathBuf,
    /// Workspace root the published crates resolve against (defaults to cwd).
    #[arg(long)]
    workspace: Option<PathBuf>,
    /// Explicit SPDX license override. Without this flag the license comes
    /// from the publication doc's `license:` envelope field (the manifest
    /// is authoritative); with neither, the projection refuses. There is
    /// no silent default.
    #[arg(long)]
    license: Option<String>,
    /// Do not `git init` / commit the output dir.
    #[arg(long)]
    no_git: bool,
    /// Do not emit `.github/workflows/` wrappers (the forge-agnostic
    /// `tools/ci` + `tools/x0k-guard-generated` are always emitted).
    #[arg(long)]
    no_github: bool,
    /// Bypass leak / closure / publish-exclusion guards (escape hatch).
    #[arg(long)]
    allow_dirty: bool,
},
```

`publish-repo` is the one verb with an irreversible half, and it sits
behind `--really` ([`publishing.md`](publishing.md)).

```rust {#publish-repo-command file="src/main.rs"}
/// [corpus-only] Publish pipeline for a projected repository: project,
/// prove, rehearse, and (only under --really) publish.
///
/// Projects with guards on, builds + tests the projection standalone,
/// rehearses with one `cargo publish --dry-run --workspace` over the
/// whole bundle, and reports. The real `cargo publish --workspace` and
/// the `git push` to the publication's configured remote run ONLY under
/// `--really` (operator-only; refuses unless the rehearsal passed).
///
/// Reads `decisions/publications/` and the decision documents it names,
/// so it needs a corpus checkout; a projected repository carries only
/// `knowledge/implementation/` and this verb refuses there.
PublishRepo {
    /// Path to the publication decision doc (`type: publication`).
    region: PathBuf,
    /// Directory to project the repo into (created if absent).
    #[arg(long)]
    output_dir: PathBuf,
    /// Workspace root the published crates resolve against (defaults to cwd).
    #[arg(long)]
    workspace: Option<PathBuf>,
    /// Explicit SPDX license override (default: the publication doc's
    /// `license:` field is authoritative).
    #[arg(long)]
    license: Option<String>,
    /// Do not emit `.github/workflows/` wrappers.
    #[arg(long)]
    no_github: bool,
    /// Actually publish to crates.io and push to the configured remote.
    /// Operator-only.
    #[arg(long)]
    really: bool,
},
```

```rust {#receive-repo-command file="src/main.rs"}
/// [corpus-only] Receive changes made in a projected repository (a
/// contributor's clone) back into the corpus as a proposed change.
///
/// Diffs the clone against a reference projection at the clone's
/// `corpus_rev`, classifies every changed path, writes unified diffs +
/// `receipt.json`, and — under `--apply` — patches the working copy
/// (never commits). Exits non-zero when any change was refused (an
/// `@generated` edit).
///
/// Reads `decisions/publications/` and the decision documents it names,
/// so it needs a corpus checkout; a projected repository carries only
/// `knowledge/implementation/` and this verb refuses there.
ReceiveRepo {
    /// The contributor's clone of the projected repository.
    clone: PathBuf,
    /// Workspace root the patches apply to (defaults to cwd).
    #[arg(long)]
    workspace: Option<PathBuf>,
    /// Directory for the patch set + receipt.json (default: temp).
    #[arg(long)]
    out: Option<PathBuf>,
    /// Apply the receivable patches to the working copy. Refused when a
    /// target path already has uncommitted changes.
    #[arg(long)]
    apply: bool,
    /// Publication doc override (default: resolved from the clone's
    /// PROVENANCE.json `publication_uri` under decisions/publications/).
    #[arg(long)]
    publication: Option<PathBuf>,
    /// Root for the reference projection's temp dir (must be outside the
    /// workspace; default: the system temp dir).
    #[arg(long)]
    scratch: Option<PathBuf>,
},
```

```rust {#list-command file="src/main.rs"}
/// List chunks and their targets in a document
List {
    /// Path to a literate document
    path: PathBuf,
},
```

```rust {#workspace-command file="src/main.rs"}
/// Tangle every dirty literate document this binary's registry
/// can handle.
///
/// This binary ships only the built-in `PipelineRegistry::default()`,
/// which carries only the `identity-tangle` plugin. It walks every
/// literate root claimed by that plugin (`knowledge/implementation/**`) and
/// re-tangles dirty docs. Docs that declare additional pipelines
/// land in the `errored` bucket as "unknown pipeline kind"; a host
/// that registers more plugins builds its own binary around the
/// library.
Workspace {
    /// Workspace root (defaults to the current directory)
    #[arg(long)]
    root: Option<PathBuf>,
},
```

## Dispatch

`main` is one `match` over the command; every arm resolves its
workspace root, calls the library, and prints a report to stderr; stdout
is reserved for data (`index` and `weave` without an output path,
`list`, and `affordances`). Exit codes carry the verdicts: `check` and
`workspace` exit non-zero on any error, `publish-repo` when the
projection fails to build or test, `receive-repo` when any change was
refused.

```rust {#main-fn file="src/main.rs"}
fn main() -> Result<()> {
    let cli = Cli::parse();

    match cli.command {
        <<dispatch-tangle>>

        <<dispatch-sync>>

        <<dispatch-check>>

        <<dispatch-affordances>>

        <<dispatch-index>>

        <<dispatch-weave>>

        <<dispatch-weave-region>>

        <<dispatch-project-repo>>

        <<dispatch-publish-repo>>

        <<dispatch-receive-repo>>

        <<dispatch-workspace>>

        <<dispatch-list>>
    }

    Ok(())
}
```

`tangle` routes through the unified dispatcher
([`dispatcher.md`](dispatcher.md)) rather than the identity plugin
directly, so a document that declares a pipeline this binary does not
ship errors loudly instead of tangling half of itself.

```rust {#dispatch-tangle file="src/main.rs"}
Command::Tangle { paths, workspace } => {
    // Route identity tangling through the unified dispatcher.
    // The default registry has `IdentityPipeline` registered;
    // docs that also declare extra pipelines will error here
    // because this binary doesn't ship those plugins.
    let ws = workspace.unwrap_or_else(|| std::env::current_dir().unwrap());
    let docs = discover_documents(&paths)?;
    let registry = x0k_tangle::PipelineRegistry::default();
    let mut total_files = 0;

    for doc_path in &docs {
        let result = x0k_tangle::tangle_document(doc_path, &ws, &registry)?;
        for out in &result.identity_outputs {
            eprintln!("  {} → {}", doc_path.display(), out.path.display());
            total_files += 1;
        }
        for out in &result.pipeline_outputs {
            if out.kind == x0k_tangle::IDENTITY_KIND {
                // Already reported via identity_outputs.
                continue;
            }
            eprintln!("  {} → {}", doc_path.display(), out.path.display());
            total_files += 1;
        }
    }

    eprintln!(
        "tangled {} file(s) from {} document(s)",
        total_files,
        docs.len()
    );
}
```

```rust {#dispatch-sync file="src/main.rs"}
Command::Sync { paths, workspace } => {
    let ws = workspace.unwrap_or_else(|| std::env::current_dir().unwrap());
    let docs = discover_documents_any(&paths)?;
    let mut total_populated = 0;

    for doc_path in &docs {
        let result = x0k_tangle::sync::sync_document(doc_path, &ws)?;

        for err in &result.errors {
            eprintln!("  warn: {}", err);
        }

        if result.chunks_populated > 0 {
            eprintln!(
                "  {} — populated {} chunk(s)",
                doc_path.display(),
                result.chunks_populated
            );
            total_populated += result.chunks_populated;
        }
    }

    eprintln!(
        "synced {} chunk(s) across {} document(s)",
        total_populated,
        docs.len()
    );
}
```

`check` has two halves. The first walks the tangling documents and
verifies their chunk references, as it always did. The second reads
every folio/v1 envelope under the same paths against the shipped
vocabulary ([`cli-faces.md`](cli-faces.md)) and prints what it found
in the affordance's own two categories: a defect as `<path>: <defect>`,
which fails the run, and a dangling edge as a `note:` that names the
target and says why it is expected. The summary line counts both, so a
clean run still says how many envelopes were read and how many edges
left the set.

```rust {#dispatch-check file="src/main.rs"}
Command::Check { paths } => {
    let docs = discover_documents(&paths)?;
    let mut has_errors = false;

    for doc_path in &docs {
        let content = std::fs::read_to_string(doc_path)?;
        let parsed = x0k_tangle::parser::parse_document(&content)?;
        let errors = x0k_tangle::resolve::check_all_refs(&parsed)?;

        for err in &errors {
            eprintln!("{}: {}", doc_path.display(), err);
            has_errors = true;
        }
    }

    let vocabulary = x0k_tangle::faces::check_vocabulary(&paths)?;
    for (path, reason) in &vocabulary.unparsed {
        eprintln!("{path}: envelope does not parse: {reason}");
        has_errors = true;
    }
    for (path, defect) in &vocabulary.corpus.defects {
        eprintln!("{path}: {defect}");
        has_errors = true;
    }
    for defect in &vocabulary.declarations.defects {
        eprintln!("{defect}");
        has_errors = true;
    }
    for edge in &vocabulary.corpus.dangling {
        eprintln!(
            "{}: note: edge `{}` → `{}` names no document here (an edge into the corpus this was projected from; expected)",
            edge.source, edge.predicate, edge.target
        );
    }

    if has_errors {
        std::process::exit(1);
    } else {
        eprintln!(
            "all references OK; {} envelope(s) read against the shipped vocabulary, {} declaration(s) checked, {} edge(s) leave the set",
            vocabulary.corpus.checked,
            vocabulary.declarations.checked,
            vocabulary.corpus.dangling.len()
        );
    }
}
```

`affordances` is the one literate verb whose whole product is data, so
its records go to stdout as pretty JSON and only the extractor's
refusals go to stderr.

```rust {#dispatch-affordances file="src/main.rs"}
Command::Affordances { paths } => {
    let report = x0k_tangle::faces::declared_affordances(&paths)?;
    for (path, reason) in &report.skipped {
        eprintln!("{path}: skipped: {reason}");
    }
    println!("{}", serde_json::to_string_pretty(&report.records)?);
}
```

```rust {#dispatch-index file="src/main.rs"}
Command::Index {
    paths,
    workspace,
    output,
} => {
    let ws = workspace.unwrap_or_else(|| std::env::current_dir().unwrap());
    let index = x0k_tangle::index::build_index(&paths, &ws)?;
    let json = serde_json::to_string_pretty(&index)?;

    if let Some(out_path) = output {
        std::fs::write(&out_path, &json)?;
        eprintln!(
            "indexed {} documents → {}",
            index.docs.len(),
            out_path.display()
        );
    } else {
        println!("{}", json);
    }
}
```

```rust {#dispatch-weave file="src/main.rs"}
Command::Weave { path, output_dir } => {
    let content = std::fs::read_to_string(&path)?;
    let parsed = x0k_tangle::parser::parse_document(&content)?;
    let output = x0k_tangle::weave::weave_html(&content, &parsed)?;

    if let Some(dir) = output_dir {
        std::fs::create_dir_all(&dir)?;
        let html_path = dir.join("index.html");
        std::fs::write(&html_path, &output.html)?;
        eprintln!("wove {} → {}", path.display(), html_path.display());
    } else {
        print!("{}", output.html);
    }
}
```

The region arms print the report shapes their chapters define; the
prose about what each field means lives there, not here.

```rust {#dispatch-weave-region file="src/main.rs"}
Command::WeaveRegion {
    region,
    output_dir,
    workspace,
    no_motifs,
} => {
    let ws = workspace.unwrap_or_else(|| std::env::current_dir().unwrap());
    let report = x0k_tangle::project_publication(&region, &output_dir, &ws, no_motifs)?;
    eprintln!(
        "wove region {} → {} ({} page(s), {} media ref(s), {} unresolved link(s))",
        region.display(),
        output_dir.join(&report.entry_rel_path).display(),
        report.page_count,
        report.media_refs.len(),
        report.unresolved_links.len(),
    );
    if !report.unresolved_links.is_empty() {
        for href in &report.unresolved_links {
            eprintln!("  unresolved link: {href}");
        }
    }
    if !report.degraded_embeds.is_empty() {
        eprintln!(
            "  {} embed(s) show their static fallback label",
            report.degraded_embeds.len()
        );
    }
    eprintln!(
        "  atlas.json: {} node(s), {} edge(s), {} thread(s) [{}]",
        report.atlas_node_count,
        report.atlas_edge_count,
        report.atlas_threads.len(),
        report.atlas_threads.join(", "),
    );
    eprintln!(
        "  presentation: render-vello wasm {}, narrative {}",
        if report.wasm_bundled {
            format!("bundled ({} KiB)", report.wasm_bytes / 1024)
        } else {
            "MISSING (set X0K_RENDER_VELLO_WASM_DIR or build it)".to_string()
        },
        if report.narrative_bundled {
            "bundled"
        } else {
            "stub (no sidecar)"
        },
    );
    if !report.atlas_unresolved_years.is_empty() {
        for uri in &report.atlas_unresolved_years {
            eprintln!("  atlas: unresolved year for {uri}");
        }
    }
}
```

```rust {#dispatch-project-repo file="src/main.rs"}
Command::ProjectRepo {
    region,
    output_dir,
    workspace,
    license,
    no_git,
    no_github,
    allow_dirty,
} => {
    let ws = workspace.unwrap_or_else(|| std::env::current_dir().unwrap());
    let opts = x0k_tangle::RepoProjectOptions {
        license,
        git_init: !no_git,
        allow_dirty,
        emit_github: !no_github,
    };
    let report = x0k_tangle::project_publication_repo(&region, &output_dir, &ws, &opts)?;
    eprintln!(
        "projected repo {} → {} ({} crate(s), {} literate doc(s), license {} [{}]{})",
        region.display(),
        output_dir.display(),
        report.crates.len(),
        report.literate_docs.len(),
        report.license,
        match report.license_source {
            x0k_tangle::LicenseSource::PublicationDoc => "from publication doc",
            x0k_tangle::LicenseSource::Override => "explicit override",
        },
        if report.committed {
            ", committed"
        } else if !no_git {
            ", unchanged (no new commit)"
        } else {
            ""
        },
    );
    if !report.excluded.is_empty() {
        eprintln!("  publish-excluded: {}", report.excluded.join(", "));
    }
    for v in report
        .leak_violations
        .iter()
        .chain(report.closure_violations.iter())
    {
        eprintln!("  WARNING: {v}");
    }
}
```

```rust {#dispatch-publish-repo file="src/main.rs"}
Command::PublishRepo {
    region,
    output_dir,
    workspace,
    license,
    no_github,
    really,
} => {
    let ws = workspace.unwrap_or_else(|| std::env::current_dir().unwrap());
    let opts = x0k_tangle::PublishRepoOptions {
        license,
        emit_github: !no_github,
        really,
    };
    let report = x0k_tangle::publish_repo(&region, &output_dir, &ws, &opts)?;
    eprintln!(
        "publish-repo {} → {} (license {})",
        region.display(),
        output_dir.display(),
        report.projection.license,
    );
    eprintln!(
        "  build: {}  test: {}",
        if report.build_ok { "ok" } else { "FAILED" },
        if report.test_ok { "ok" } else { "FAILED" },
    );
    eprintln!("  publish order: {}", report.publish_order.join(" → "));
    if let Some(r) = &report.rehearsal {
        eprintln!(
            "  dry-run (whole bundle): {}",
            if r.ok { "ok" } else { "FAILED" }
        );
        if !r.ok {
            for line in r.output_tail.lines() {
                eprintln!("      {line}");
            }
        }
    }
    if !report.build_ok || !report.test_ok {
        eprintln!("  stopped: the projection must build and test green before any rehearsal");
        std::process::exit(1);
    }
    match (&report.surface, &report.remote) {
        (Some(s), Some(r)) => eprintln!("  remote: {s} → {r}"),
        (Some(s), None) => eprintln!(
            "  remote: {s} has no [publish.remotes] entry in config/x0k-tangle.toml"
        ),
        (None, _) => eprintln!("  remote: publication has no publishedOn edge"),
    }
    if report.published || report.pushed {
        eprintln!(
            "  PUBLISHED: crates.io={} push={}",
            report.published, report.pushed
        );
    } else if !really {
        eprintln!("  stopped before publishing (pass --really to publish; operator-only)");
    }
}
```

```rust {#dispatch-receive-repo file="src/main.rs"}
Command::ReceiveRepo {
    clone,
    workspace,
    out,
    apply,
    publication,
    scratch,
} => {
    let ws = workspace.unwrap_or_else(|| std::env::current_dir().unwrap());
    let opts = x0k_tangle::ReceiveOptions {
        apply,
        out_dir: out.clone(),
        publication,
        scratch,
    };
    let report = x0k_tangle::receive_repo(&clone, &ws, &opts)?;
    eprintln!(
        "receive-repo {} ({}): clone rev {} vs reference {}{}",
        clone.display(),
        report.publication_uri,
        if report.clone_rev.is_empty() { "(none)" } else { &report.clone_rev },
        if report.reference_rev.is_empty() { "(none)" } else { &report.reference_rev },
        if report.rev_exact { "" } else { "  [REV SKEW: diff includes the corpus's own drift, reversed]" },
    );
    for c in &report.changes {
        let class = match c.class {
            x0k_tangle::receive::Class::Literate => "literate (received)",
            x0k_tangle::receive::Class::Source => "source (received)",
            x0k_tangle::receive::Class::Generated => "GENERATED (refused)",
            x0k_tangle::receive::Class::ProjectionLocal => "overlay (projection-local, not received)",
            x0k_tangle::receive::Class::ProjectionOwned => "projection-owned (not received)",
        };
        let size = c.patch.as_ref().map(|p| p.lines().count()).unwrap_or(0);
        match (&c.target, &c.produced_by) {
            (Some(t), _) => eprintln!("  {:<9} {}  {class}  → {t}  ({size} patch lines)", c.kind, c.path),
            (None, Some(o)) => eprintln!(
                "  {:<9} {}  {class}  produced by {}{}",
                c.kind,
                c.path,
                o.doc,
                if o.chunks.is_empty() { String::new() } else { format!(" chunks {}", o.chunks.join(", ")) }
            ),
            (None, None) => eprintln!("  {:<9} {}  {class}", c.kind, c.path),
        }
    }
    eprintln!(
        "  {} change(s): {} received, {} refused{}{}",
        report.changes.len(),
        report.received(),
        report.refused(),
        match &out {
            Some(d) => format!("; patch set in {}", d.display()),
            None => String::new(),
        },
        if report.applied { format!("; applied to working copy (dirty check: {})", report.dirty_check) } else { "" .to_string() },
    );
    if report.refused() > 0 {
        std::process::exit(1);
    }
}
```

```rust {#dispatch-workspace file="src/main.rs"}
Command::Workspace { root } => {
    let ws = resolve_workspace_root(root)?;
    let registry = x0k_tangle::PipelineRegistry::default();
    let report = x0k_tangle::tangle_workspace(&ws, &registry)?;
    print_workspace_summary(&ws, &report);
    if !report.errored.is_empty() {
        std::process::exit(1);
    }
}
```

```rust {#dispatch-list file="src/main.rs"}
Command::List { path } => {
    let content = std::fs::read_to_string(&path)?;
    let parsed = x0k_tangle::parser::parse_document(&content)?;

    if let Some(ref c) = parsed.tangle_crate {
        println!("crate: {}", c);
    }
    if let Some(ref r) = parsed.tangle_root {
        println!("root:  {}", r.display());
    }
    println!();

    for name in &parsed.chunk_order {
        let Some(chunk) = parsed.chunk(name) else {
            continue;
        };
        let kind = if chunk.is_media {
            "media"
        } else if chunk.is_from_ref() {
            "from"
        } else {
            "owned"
        };
        let target = chunk
            .file_target
            .as_ref()
            .map(|p| p.display().to_string())
            .or_else(|| chunk.from.as_ref().map(|p| p.display().to_string()))
            .unwrap_or_default();
        let symbol = chunk.symbol.as_deref().unwrap_or("");
        let lines: usize = chunk.bodies.iter().map(|b| b.text.lines().count()).sum();

        println!(
            "  {:<6} {:<24} {:>4} lines  {}{}",
            kind,
            name,
            lines,
            target,
            if symbol.is_empty() {
                String::new()
            } else {
                format!("  symbol={}", symbol)
            }
        );
    }
}
```

## Helpers

The workspace root — the `--root` flag, else the current directory —
is canonicalized before anything is written so the tree being tangled
is named, not implied by the current directory; the library refuses
writes outside it regardless. Document discovery is a
content sniff — a `.md` mentioning `tangle:` (or, for `sync`, `from=`) —
because the parse that would confirm it is what the verb is about to do
anyway. The rest is report formatting.

```rust {#resolve-workspace-root file="src/main.rs"}
/// Resolve a workspace root from the CLI flag, else the current directory.
fn resolve_workspace_root(flag: Option<PathBuf>) -> Result<PathBuf> {
    let raw = match flag {
        Some(p) => p,
        None => std::env::current_dir()?,
    };
    // Canonicalize so the tree being written is named, not implied by
    // cwd. The library refuses writes outside this root regardless.
    std::fs::canonicalize(&raw)
        .with_context(|| format!("resolving workspace root {}", raw.display()))
}
```

```rust {#print-workspace-summary file="src/main.rs"}
/// Pretty-print a `WorkspaceTangleReport` to stderr.
fn print_workspace_summary(
    workspace_root: &std::path::Path,
    report: &x0k_tangle::WorkspaceTangleReport,
) {
    eprintln!("tangle workspace summary:");
    eprintln!("  tangled:    {}", report.tangled.len());
    eprintln!("  up-to-date: {}", report.up_to_date.len());
    eprintln!("  errored:    {}", report.errored.len());

    for tr in &report.tangled {
        let rel_source = tr
            .source_path
            .strip_prefix(workspace_root)
            .unwrap_or(&tr.source_path)
            .display();
        let total_outputs = tr.identity_outputs.len() + tr.pipeline_outputs.len();
        let first = tr
            .identity_outputs
            .first()
            .map(|o| o.path.clone())
            .or_else(|| tr.pipeline_outputs.first().map(|o| o.path.clone()));
        if let Some(first) = first {
            let rel_first = first
                .strip_prefix(workspace_root)
                .unwrap_or(&first)
                .display()
                .to_string();
            if total_outputs > 1 {
                eprintln!(
                    "  {} → {} (+{} more)",
                    rel_source,
                    rel_first,
                    total_outputs - 1
                );
            } else {
                eprintln!("  {} → {}", rel_source, rel_first);
            }
        } else {
            eprintln!("  {} → (no outputs)", rel_source);
        }
    }

    for (path, err) in &report.errored {
        let rel = path.strip_prefix(workspace_root).unwrap_or(path).display();
        eprintln!("  ERROR {}: {}", rel, err);
    }
}
```

```rust {#discover-documents file="src/main.rs"}
fn discover_documents_any(paths: &[PathBuf]) -> Result<Vec<PathBuf>> {
    let mut docs = Vec::new();
    for path in paths {
        if path.is_file() && path.extension().is_some_and(|e| e == "md") {
            docs.push(path.clone());
        } else if path.is_dir() {
            for entry in walkdir::WalkDir::new(path)
                .into_iter()
                .filter_map(|e| e.ok())
            {
                let p = entry.path();
                if p.extension().is_some_and(|e| e == "md") {
                    if let Ok(content) = std::fs::read_to_string(p) {
                        if content.contains("from=") || content.contains("tangle:") {
                            docs.push(p.to_path_buf());
                        }
                    }
                }
            }
        }
    }
    Ok(docs)
}

fn discover_documents(paths: &[PathBuf]) -> Result<Vec<PathBuf>> {
    let mut docs = Vec::new();

    for path in paths {
        if path.is_file() && path.extension().is_some_and(|e| e == "md") {
            docs.push(path.clone());
        } else if path.is_dir() {
            for entry in walkdir::WalkDir::new(path)
                .into_iter()
                .filter_map(|e| e.ok())
            {
                let p = entry.path();
                if p.extension().is_some_and(|e| e == "md") {
                    if let Ok(content) = std::fs::read_to_string(p) {
                        if content.contains("tangle:") {
                            docs.push(p.to_path_buf());
                        }
                    }
                }
            }
        }
    }

    Ok(docs)
}
```

## Composing the crate root and the binary

```rust {#root}
<<crate-doc>>

<<modules>>

<<exports>>
```

```rust {#bin-root file="src/main.rs"}
<<bin-doc>>

<<cli-imports>>

<<cli-struct>>

<<command-enum>>

<<main-fn>>

<<resolve-workspace-root>>

<<print-workspace-summary>>

<<discover-documents>>
```

The crate's boundary is the thing to keep honest. Every module is
public and the CLI is thin, so there is no place for behaviour to hide
that a consumer could not reach by name — which is also why this
chapter has no mechanism of its own to derive. When a verb grows a
mechanism, it moves to a chapter; when a chapter's type is meant to be
named from outside, it appears in the export list above.
