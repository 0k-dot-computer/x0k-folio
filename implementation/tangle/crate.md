---
x0k:
  format: folio/v1
  id: x0k:implementation/tangle/crate
  type: implementation
  status: draft
  summary: The crate's contract rather than a mechanism — the module list and re-exports that say what a consumer may name, and the plugin-less CLI that puts those verbs in a shell.
  concerns:
  - tangle
  - literate
  - crate
  - cli
  - features
  - publishing
  tangle:
    crate: crates/x0k-tangle
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
---
# x0k-tangle: the crate and its CLI

`x0k-tangle` is the crate that makes a [literate
document](../../background/literate-programming.md "x0k:wiki/literate-programming") executable:
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

<a name="chunk-crate-doc"></a><sub>[`src/lib.rs`](../../crates/x0k-tangle/src/lib.rs) · `#crate-doc`</sub>

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
//! - **check** — [`resolve::check_all_refs`],
//!   [`source_check::check_source_refs`], and [`faces::vocabulary`] +
//!   [`faces::check_vocabulary`]: verify every chunk reference resolves
//!   and no reference cycle exists, resolve every `from=`/`symbol=`
//!   chunk against its source file, and read every folio/v1 envelope
//!   against a vocabulary — one named with `--vocabulary`, one a
//!   projection recorded, or the set this build compiled — without
//!   writing anything.
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

<a name="chunk-modules"></a><sub>[`src/lib.rs`](../../crates/x0k-tangle/src/lib.rs) · `#modules`</sub>

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
pub mod region_gfm;
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
pub mod instance_rendering;
```

The re-exports are the names a consumer is expected to use without
knowing the module layout: the atlas, the pipeline protocol and its
runner, the presentation shell's file names, and the four outward
verbs — project a region to HTML, project it to a repository, publish
that repository, receive a contribution from it.

<a name="chunk-exports"></a><sub>[`src/lib.rs`](../../crates/x0k-tangle/src/lib.rs) · `#exports`</sub>

```rust {#exports}
pub use atlas::{
    atlas_json, build_atlas, Atlas, AtlasEdge, AtlasNode, AtlasPlacement, YearSource, ATLAS_FILE,
};
pub use identity_pipeline::{IdentityPipeline, IDENTITY_KIND};
pub use pipeline::{
    ChunkInput, ChunkVariant, ClobberPolicy, ClobberRefusal, CommentStyle, OutputProvenance,
    PipelineContext, PipelineError, PipelineErrorKind, PipelineOutput, PipelineRegistry,
    TanglePipeline,
};
pub use pipeline_runner::{
    doc_freshness, tangle_directory, tangle_directory_with, tangle_document, tangle_document_with,
    tangle_workspace, tangle_workspace_with, DirtyReason, DocFreshness, PipelineRunOutput,
    TangleResult, TangleSettings, WorkspaceTangleReport,
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

## Reading a `from=` without touching it

`sync` is the verb that *repairs* a source reference, and repairing means
writing: it opens the file the chunk names, extracts the symbol, and puts
the body back into the document. That makes it useless as a gate. A CI job
cannot run a verb that rewrites the tree it is judging, and a maintainer
who runs it locally and pushes gets a green pipeline over a document whose
prose sits above code that was deleted three months ago.

The hole is worse than "unchecked", because `sync` half-closes it in a way
that reads as closed. Sync a chunk, let the source method be renamed, and
`sync` exits 1 — but it leaves the old body in place. The document is
byte-identical, `git status` is clean, the re-run-and-diff gate the guide
recommends sees nothing, and `check` prints a pass. The stale body is the
one failure this whole feature exists to prevent, and it was the one
failure nothing could observe from the read side.

So the resolution half of `sync` gets a second caller that stops before the
write. Same file, same symbol, same extractor, same messages — including
the ambiguity report, which is the most useful sentence this tool prints and
is worth having in the mode a reader can run. What comes back is a list of
sentences and a count of what was resolved, so the CLI can print findings
under the document that holds them and say how much it read.

The policy lives here rather than in either binary because there are two
binaries. `main.rs` and the bundle's `bin/x0k-tangle.rs` are copies of each
other by construction, and a duplicated format string that drifts costs a
reader a confusing line; a duplicated *resolution rule* that drifts costs
them a gate that disagrees with itself about whether the tree is sound.

<a name="chunk-source-check"></a><sub>[`src/lib.rs`](../../crates/x0k-tangle/src/lib.rs) · `#source-check`</sub>

```rust {#source-check}
/// Resolving every `from=` chunk in a document without writing anything.
///
/// The read-only half of [`crate::sync`]: it opens the same files, calls
/// the same extractor, and reports the same sentences, but it never
/// touches the document. That is what makes it usable as a gate — `sync`
/// rewrites the tree it judges, so no CI job can run it.
pub mod source_check {
    use crate::parser::ParsedDocument;
    use crate::source_ref::{extract_symbol_in, SymbolLanguage};
    use std::path::Path;

    /// What one pass over a document's `from=` chunks found.
    ///
    /// `checked` counts the chunks that named a source, findings or not,
    /// so a caller's summary line can say how much it read rather than
    /// asserting a pass over work it may have skipped.
    pub struct SourceRefReport {
        pub checked: usize,
        pub findings: Vec<String>,
    }

    /// Resolve every `from=` chunk against the workspace root, reporting
    /// each one that does not.
    ///
    /// The resolution is `sync`'s, move for move: the same
    /// `workspace_root.join(from)`, the same language refusal before any
    /// file is opened, the same `extract_symbol_in`. A finding is phrased
    /// `chunk '<name>': <what sync would have said>`, so the two verbs
    /// name a broken reference identically and a reader who has seen one
    /// recognises the other.
    ///
    /// A chunk carrying `from=` and no `symbol=` is the one shape this
    /// verb judges on its own, because `sync` skips it in silence — see
    /// `bare_from_finding` below, which is private, so this is deliberately
    /// not an intra-doc link.
    pub fn check_source_refs(parsed: &ParsedDocument, workspace_root: &Path) -> SourceRefReport {
        let mut report = SourceRefReport {
            checked: 0,
            findings: Vec::new(),
        };

        for name in &parsed.chunk_order {
            let Some(chunk) = parsed.chunk(name) else {
                continue;
            };
            if chunk.is_media || !chunk.is_from_ref() {
                continue;
            }
            let Some(ref from_path) = chunk.from else {
                continue;
            };
            report.checked += 1;

            let source_file = workspace_root.join(from_path);
            let lang = SymbolLanguage::for_lang(chunk.lang.as_deref());

            let Some(symbol) = chunk.symbol.as_deref() else {
                if let Some(finding) = bare_from_finding(name, &source_file, lang.is_ok()) {
                    report.findings.push(finding);
                }
                continue;
            };

            // Which grammar reads the source is a property of the
            // document alone, so it is settled before any file is opened
            // — the order `sync` uses, and the reason an unwalkable
            // language never reads as a mistyped symbol.
            let lang = match lang {
                Ok(lang) => lang,
                Err(e) => {
                    report.findings.push(format!("chunk '{name}': {e}"));
                    continue;
                }
            };

            if !source_file.exists() {
                report.findings.push(not_found(name, &source_file));
                continue;
            }

            let source_content = match std::fs::read_to_string(&source_file) {
                Ok(content) => content,
                Err(e) => {
                    report.findings.push(format!(
                        "chunk '{name}': reading {}: {e}",
                        source_file.display()
                    ));
                    continue;
                }
            };

            if let Err(e) = extract_symbol_in(&source_content, symbol, lang) {
                report.findings.push(format!("chunk '{name}': {e}"));
            }
        }

        report
    }

    /// What a chunk naming a file and no symbol is worth saying about.
    ///
    /// Two different things wear this shape. One is deliberate: a whole
    /// file quoted into a document in a language symbol extraction cannot
    /// walk — a TOML effect definition, a shader — where there is no
    /// symbol to name and the document is showing the file. The corpus
    /// has one, and making it fatal would be this gate refusing a
    /// perfectly honest reference.
    ///
    /// The other is an accident, and it is the same accident this whole
    /// verb is about. In rust, typescript, python — a language `sync`
    /// *can* walk — a `from=` with no `symbol=` is a chunk `sync` skips
    /// in silence forever: the body is never refreshed, and nothing on
    /// either side of the tool ever says so. That one is a finding.
    ///
    /// Both are held to the half of the claim that is checkable from
    /// here: the file the chunk names has to be there.
    fn bare_from_finding(name: &str, source_file: &Path, walkable: bool) -> Option<String> {
        if !source_file.exists() {
            return Some(not_found(name, source_file));
        }
        if walkable {
            return Some(format!(
                "chunk '{name}': names a source file and no symbol=, in a language \
                 symbol extraction can walk — sync skips it in silence, so the body \
                 is never refreshed (add symbol=, or quote the file from a fence \
                 sync cannot walk)"
            ));
        }
        None
    }

    /// The one spelling of a source file that is not there. `sync` says
    /// this sentence too.
    fn not_found(name: &str, source_file: &Path) -> String {
        format!(
            "chunk '{name}': source file not found: {}",
            source_file.display()
        )
    }
}
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
all the literate verbs need; the other nine verbs, `workspace`
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

<a name="chunk-bin-doc"></a><sub>[`src/main.rs`](../../crates/x0k-tangle/src/main.rs) · `#bin-doc`</sub>

```rust {#bin-doc file="src/main.rs"}
//! The protocol-only `x0k-tangle` CLI: the crate's verbs in a shell,
//! with the built-in registry and no plugins. The binary a projected
//! repository ships and builds.
```

<a name="chunk-cli-imports"></a><sub>[`src/main.rs`](../../crates/x0k-tangle/src/main.rs) · `#cli-imports`</sub>

```rust {#cli-imports file="src/main.rs"}
use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
```

<a name="chunk-cli-struct"></a><sub>[`src/main.rs`](../../crates/x0k-tangle/src/main.rs) · `#cli-struct`</sub>

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
documents in place: `tangle`, `check`, `affordances`, `icon`, `sync`,
`index`, `weave`, `list`, and `workspace`. The publication verbs operate on a
region: `weave-region` and `project-repo` are the two projection
backends, `publish-repo` the pipeline that makes a projection public,
and `receive-repo` the inbound door. Each variant's doc comment is its
`--help` text, so the clap derive below is also the user-facing
contract.

<a name="chunk-command-enum"></a><sub>[`src/main.rs`](../../crates/x0k-tangle/src/main.rs) · `#command-enum` · assembles [tangle-command](#chunk-tangle-command) · [check-command](#chunk-check-command) · [affordances-command](#chunk-affordances-command) · [icon-command](#chunk-icon-command) · [sync-command](#chunk-sync-command) · [index-command](#chunk-index-command) · [weave-command](#chunk-weave-command) · [weave-region-command](#chunk-weave-region-command) · [project-repo-command](#chunk-project-repo-command) · [publish-repo-command](#chunk-publish-repo-command) · [receive-repo-command](#chunk-receive-repo-command) · [list-command](#chunk-list-command) · [workspace-command](#chunk-workspace-command)</sub>

```rust {#command-enum file="src/main.rs"}
#[derive(Subcommand)]
enum Command {
    <<tangle-command>>
    <<check-command>>
    <<affordances-command>>
    <<icon-command>>
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

Five of the verbs are the perceivable cue for an affordance a
publication of this crate claims for a human, and each declares that
under its own heading: a signifier is recorded where the face lives,
so the chapter that ships carries the cue with it
(`x0k:design/publish-a-region-as-a-repository`). The heading and the
prose around the block are the cue's own text, and the `///` line on
the variant — the line `--help` prints — says the same thing in the
same words.

### `x0k-tangle tangle`

Tangle `.md` documents to their source files, writing a
`.tangle-map.json` sidecar beside each: the affordance of [turning a
literate document into the code it describes](../../decisions/design/corpus/literate-programming/project-source-code-out-of-a-document.md "x0k:affordance/tangle_source_from_a_document"), from a shell.

<a name="folio-instance-68747470733a2f2f306b2e636f6d70757465722f6f6e746f6c6f6779237369676e69666965722f78306b2d74616e676c652d74616e676c65-1"></a><sub data-instance-iri="https://0k.computer/ontology#signifier/x0k-tangle-tangle" data-concept-iri="https://0k.computer/ontology#Signifier" data-source-document="corpora/x0k/implementation/tangle/crate.md"><strong>Signifier</strong> · x0k-tangle tangle · <code>https://0k.computer/ontology#signifier/x0k-tangle-tangle</code> · <a href="#folio-source-68747470733a2f2f306b2e636f6d70757465722f6f6e746f6c6f6779237369676e69666965722f78306b2d74616e676c652d74616e676c65-1">source declaration</a></sub><a name="folio-source-68747470733a2f2f306b2e636f6d70757465722f6f6e746f6c6f6779237369676e69666965722f78306b2d74616e676c652d74616e676c65-1"></a>

```yaml x0k:signifier
id: x0k:signifier/x0k-tangle-tangle
edges:
  signifies:
    - x0k:affordance/tangle_source_from_a_document
  presentedOn:
    - x0k:surface/cli
```

<a name="chunk-tangle-command"></a><sub>[`src/main.rs`](../../crates/x0k-tangle/src/main.rs) · `#tangle-command`</sub>

```rust {#tangle-command file="src/main.rs"}
/// Tangle .md documents to their source files (writes .tangle-map.json sidecars)
Tangle {
    /// Paths to scan for documents with tangle: frontmatter
    paths: Vec<PathBuf>,
    /// Workspace root (defaults to current directory)
    #[arg(long)]
    workspace: Option<PathBuf>,
    /// Overwrite outputs holding content this tangler did not write
    #[arg(long)]
    force: bool,
},
```

`--force` is the one flag on this verb that can lose work, and it is
here because the dispatcher's clobber guard
([`dispatcher.md`](dispatcher.md)) refuses a document whose output was
edited outside it. The refusal names the file and the three ways out;
this flag is the third. It is a per-run decision, never a default and
never a setting, because the operator saying "those bytes are
expendable" is a claim about *these* files at *this* moment.

### `x0k-tangle check`

Verify chunk references resolve and no cycles exist, and read every
folio/v1 envelope under the paths against a vocabulary. The second half
is the affordance of [checking a document against the vocabulary that
shipped beside it](../../decisions/design/corpus/publish-a-region-as-a-repository/check-a-document-against-its-vocabulary.md "x0k:affordance/check_a_document_against_shipped_vocabulary"), and its help text names
the two outcomes the affordance promises to tell apart: a predicate no
module of that vocabulary declares is a defect and fails the check; a
target naming no document here is an edge into the corpus the repository
was projected from, noted and expected.

Which vocabulary is the argument `--vocabulary` takes: a directory of
`*.ttl` module files, which is what "shipped beside it" now literally
means — a reader points the verb at the modules a bundle carries and is
told about *those*. Without the flag the verb finds the answer itself,
from this projection's own `PROVENANCE.json` and then from the compiled
set ([`cli-faces.md`](cli-faces.md)).

<a name="folio-instance-68747470733a2f2f306b2e636f6d70757465722f6f6e746f6c6f6779237369676e69666965722f78306b2d74616e676c652d636865636b-2"></a><sub data-instance-iri="https://0k.computer/ontology#signifier/x0k-tangle-check" data-concept-iri="https://0k.computer/ontology#Signifier" data-source-document="corpora/x0k/implementation/tangle/crate.md"><strong>Signifier</strong> · x0k-tangle check · <code>https://0k.computer/ontology#signifier/x0k-tangle-check</code> · <a href="#folio-source-68747470733a2f2f306b2e636f6d70757465722f6f6e746f6c6f6779237369676e69666965722f78306b2d74616e676c652d636865636b-2">source declaration</a></sub><a name="folio-source-68747470733a2f2f306b2e636f6d70757465722f6f6e746f6c6f6779237369676e69666965722f78306b2d74616e676c652d636865636b-2"></a>

```yaml x0k:signifier
id: x0k:signifier/x0k-tangle-check
edges:
  signifies:
    - x0k:affordance/check_a_document_against_shipped_vocabulary
  presentedOn:
    - x0k:surface/cli
```

<a name="chunk-check-command"></a><sub>[`src/main.rs`](../../crates/x0k-tangle/src/main.rs) · `#check-command`</sub>

```rust {#check-command file="src/main.rs"}
/// Verify chunk references resolve and no cycles exist, and read every
/// folio/v1 envelope against a vocabulary.
///
/// Two things can go wrong with an envelope, and they are reported
/// apart. A defect — a malformed id or edge target, a predicate no
/// module of the vocabulary declares, an envelope that does not parse —
/// is a gap in what this publication selected, and fails the check. An
/// edge whose target names no document under the paths scanned simply
/// leaves the set — often into a wider corpus this selection was drawn
/// from, and expected either way: printed as a note, never a failure.
/// A third thing is checked across the set: an
/// affordance claimed for a human that no signifier signifies is a
/// defect, because the audience has nothing to perceive.
///
/// Every `from=` chunk is resolved against its source file too —
/// missing file, missing symbol, ambiguous symbol, a language symbol
/// extraction cannot walk — and each failure fails the check. Nothing
/// is written: this is the read-only half of `sync`.
Check {
    /// Paths to scan
    paths: Vec<PathBuf>,
    /// Workspace root that `from=` paths resolve against
    /// (defaults to current directory)
    #[arg(long)]
    workspace: Option<PathBuf>,
    /// Directory of ontology module files (*.ttl) to check against.
    /// Defaults to the modules this projection's PROVENANCE.json names,
    /// then to the set this build compiled.
    #[arg(long)]
    vocabulary: Option<PathBuf>,
},
```

### `x0k-tangle affordances`

Print every affordance the documents under the paths declare, as a
JSON array on stdout: one record per `yaml x0k:affordance` block, with
its id, title, description, the document it is defined in, and its
declared facts grouped by predicate. What a declaration says becomes
data a reader's own tooling can consume, which is the affordance of
[reading an affordance out of a document](../../decisions/design/corpus/publish-a-region-as-a-repository/declare-concepts-and-instances.md "x0k:affordance/read_declared_affordances") rather than trusting the
document's summary of itself. A block the extractor refuses is
reported on stderr and skipped.

<a name="folio-instance-68747470733a2f2f306b2e636f6d70757465722f6f6e746f6c6f6779237369676e69666965722f78306b2d74616e676c652d6166666f7264616e636573-3"></a><sub data-instance-iri="https://0k.computer/ontology#signifier/x0k-tangle-affordances" data-concept-iri="https://0k.computer/ontology#Signifier" data-source-document="corpora/x0k/implementation/tangle/crate.md"><strong>Signifier</strong> · x0k-tangle affordances · <code>https://0k.computer/ontology#signifier/x0k-tangle-affordances</code> · <a href="#folio-source-68747470733a2f2f306b2e636f6d70757465722f6f6e746f6c6f6779237369676e69666965722f78306b2d74616e676c652d6166666f7264616e636573-3">source declaration</a></sub><a name="folio-source-68747470733a2f2f306b2e636f6d70757465722f6f6e746f6c6f6779237369676e69666965722f78306b2d74616e676c652d6166666f7264616e636573-3"></a>

```yaml x0k:signifier
id: x0k:signifier/x0k-tangle-affordances
edges:
  signifies:
    - x0k:affordance/read_declared_affordances
  presentedOn:
    - x0k:surface/cli
```

<a name="chunk-affordances-command"></a><sub>[`src/main.rs`](../../crates/x0k-tangle/src/main.rs) · `#affordances-command`</sub>

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


### `x0k-tangle icon`

Check every `svg x0k:icon` declaration under the paths against the
icon profile, and, given a publication's palette, write each as its
light and dark files: the affordances of [checking an icon against the
profile](../../decisions/design/presentation/icon-profile/check-an-icon-against-the-profile.md "x0k:affordance/check_an_icon_against_the_profile") and
[showing it on a surface](../../decisions/design/presentation/icon-profile/show-an-icon-on-any-surface.md "x0k:affordance/show_an_icon_on_a_surface"),
from a shell. A drawing outside the profile is printed with the rule it
broke and the element, and fails the run; the verb never redraws.

<a name="folio-instance-68747470733a2f2f306b2e636f6d70757465722f6f6e746f6c6f6779237369676e69666965722f78306b2d74616e676c652d69636f6e-4"></a><sub data-instance-iri="https://0k.computer/ontology#signifier/x0k-tangle-icon" data-concept-iri="https://0k.computer/ontology#Signifier" data-source-document="corpora/x0k/implementation/tangle/crate.md"><strong>Signifier</strong> · x0k-tangle icon · <code>https://0k.computer/ontology#signifier/x0k-tangle-icon</code> · <a href="#folio-source-68747470733a2f2f306b2e636f6d70757465722f6f6e746f6c6f6779237369676e69666965722f78306b2d74616e676c652d69636f6e-4">source declaration</a></sub><a name="folio-source-68747470733a2f2f306b2e636f6d70757465722f6f6e746f6c6f6779237369676e69666965722f78306b2d74616e676c652d69636f6e-4"></a>

```yaml x0k:signifier
id: x0k:signifier/x0k-tangle-icon
cue: x0k-tangle icon
edges:
  signifies:
    - x0k:affordance/check_an_icon_against_the_profile
    - x0k:affordance/show_an_icon_on_a_surface
  presentedOn:
    - x0k:surface/cli
```

<a name="chunk-icon-command"></a><sub>[`src/main.rs`](../../crates/x0k-tangle/src/main.rs) · `#icon-command`</sub>

```rust {#icon-command file="src/main.rs"}
/// Check every `svg x0k:icon` declaration in the folio/v1 documents
/// under the paths against the icon profile, and with `--out` write
/// each as its light and dark files bound to a publication's palette.
///
/// A drawing outside the profile is printed with the rule it broke and
/// the element, and fails the run; nothing is redrawn. `--out` needs
/// `--palette`: the publication document whose envelope carries the
/// `palette:` block the four paint roles are bound with.
Icon {
    /// Paths to scan for folio/v1 documents
    paths: Vec<PathBuf>,
    /// Directory to write `<stem>-light.svg` and `<stem>-dark.svg` into
    #[arg(long, requires = "palette")]
    out: Option<PathBuf>,
    /// The publication document whose `palette:` binds the roles
    #[arg(long, requires = "out")]
    palette: Option<PathBuf>,
},
```

### `x0k-tangle sync`

<a name="chunk-sync-command"></a><sub>[`src/main.rs`](../../crates/x0k-tangle/src/main.rs) · `#sync-command`</sub>

```rust {#sync-command file="src/main.rs"}
/// Sync from= chunks: populate code blocks from source files.
/// Symbol extraction reads rust, typescript, javascript, tsx, python, julia
Sync {
    /// Paths to scan for documents with from= chunks
    paths: Vec<PathBuf>,
    /// Workspace root (defaults to current directory)
    #[arg(long)]
    workspace: Option<PathBuf>,
},
```

### `x0k-tangle index`

<a name="chunk-index-command"></a><sub>[`src/main.rs`](../../crates/x0k-tangle/src/main.rs) · `#index-command`</sub>

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
of [reading a document as the woven page it describes](../../decisions/design/corpus/literate-programming/read-a-document-as-the-woven-artifact.md "x0k:affordance/weave_a_document"), from a shell.

<a name="folio-instance-68747470733a2f2f306b2e636f6d70757465722f6f6e746f6c6f6779237369676e69666965722f78306b2d74616e676c652d7765617665-5"></a><sub data-instance-iri="https://0k.computer/ontology#signifier/x0k-tangle-weave" data-concept-iri="https://0k.computer/ontology#Signifier" data-source-document="corpora/x0k/implementation/tangle/crate.md"><strong>Signifier</strong> · x0k-tangle weave · <code>https://0k.computer/ontology#signifier/x0k-tangle-weave</code> · <a href="#folio-source-68747470733a2f2f306b2e636f6d70757465722f6f6e746f6c6f6779237369676e69666965722f78306b2d74616e676c652d7765617665-5">source declaration</a></sub><a name="folio-source-68747470733a2f2f306b2e636f6d70757465722f6f6e746f6c6f6779237369676e69666965722f78306b2d74616e676c652d7765617665-5"></a>

```yaml x0k:signifier
id: x0k:signifier/x0k-tangle-weave
edges:
  signifies:
    - x0k:affordance/weave_a_document
  presentedOn:
    - x0k:surface/cli
```

<a name="chunk-weave-command"></a><sub>[`src/main.rs`](../../crates/x0k-tangle/src/main.rs) · `#weave-command`</sub>

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

<a name="chunk-weave-region-command"></a><sub>[`src/main.rs`](../../crates/x0k-tangle/src/main.rs) · `#weave-region-command`</sub>

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

<a name="chunk-project-repo-command"></a><sub>[`src/main.rs`](../../crates/x0k-tangle/src/main.rs) · `#project-repo-command`</sub>

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

<a name="chunk-publish-repo-command"></a><sub>[`src/main.rs`](../../crates/x0k-tangle/src/main.rs) · `#publish-repo-command`</sub>

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

<a name="chunk-receive-repo-command"></a><sub>[`src/main.rs`](../../crates/x0k-tangle/src/main.rs) · `#receive-repo-command`</sub>

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

<a name="chunk-list-command"></a><sub>[`src/main.rs`](../../crates/x0k-tangle/src/main.rs) · `#list-command`</sub>

```rust {#list-command file="src/main.rs"}
/// List chunks and their targets in a document
List {
    /// Path to a literate document
    path: PathBuf,
},
```

<a name="chunk-workspace-command"></a><sub>[`src/main.rs`](../../crates/x0k-tangle/src/main.rs) · `#workspace-command`</sub>

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
    /// Overwrite outputs holding content this tangler did not write
    #[arg(long)]
    force: bool,
},
```

The sweep carries the same flag for the same reason, and it means the
same thing document by document: a `--force` sweep is the operator
saying the tree's generated files are expendable, not that one file is.
Without it, a document whose output was edited outside the tangler lands
in the report's `errored` bucket and the rest of the sweep proceeds —
the guard is a per-document verdict, so one refusal costs one document
rather than the run.

## Dispatch

`main` is one `match` over the command; every arm resolves its
workspace root, calls the library, and prints a report to stderr; stdout
is reserved for data (`index` and `weave` without an output path,
`list`, and `affordances`). Exit codes carry the verdicts: `check` and
`workspace` exit non-zero on any error, `sync` when a chunk it was
asked to fill stayed empty, `publish-repo` when the projection fails to
build or test, `receive-repo` when any change was refused.

<a name="chunk-main-fn"></a><sub>[`src/main.rs`](../../crates/x0k-tangle/src/main.rs) · `#main-fn` · assembles [dispatch-tangle](#chunk-dispatch-tangle) · [dispatch-sync](#chunk-dispatch-sync) · [dispatch-check](#chunk-dispatch-check) · [dispatch-affordances](#chunk-dispatch-affordances) · [dispatch-icon](#chunk-dispatch-icon) · [dispatch-index](#chunk-dispatch-index) · [dispatch-weave](#chunk-dispatch-weave) · [dispatch-weave-region](#chunk-dispatch-weave-region) · [dispatch-project-repo](#chunk-dispatch-project-repo) · [dispatch-publish-repo](#chunk-dispatch-publish-repo) · [dispatch-receive-repo](#chunk-dispatch-receive-repo) · [dispatch-workspace](#chunk-dispatch-workspace) · [dispatch-list](#chunk-dispatch-list)</sub>

```rust {#main-fn file="src/main.rs"}
fn main() -> Result<()> {
    let cli = Cli::parse();

    match cli.command {
        <<dispatch-tangle>>

        <<dispatch-sync>>

        <<dispatch-check>>

        <<dispatch-affordances>>

        <<dispatch-icon>>

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

Naming a document is an imperative — *write this one out* — so a named
document that names nowhere to write is a failed run, not a quiet zero.
`tangle_document` answers such a document with an empty result, which is
the right answer for a library and the wrong report for a shell: the
summary line used to say `tangled 0 file(s) from 1 document(s)` and exit
0 over a document with eight chunks and a root, and the only way to
notice was to go looking for a file that was never written. So the verb
reads what the document declares before it asks for the work, and says
which of the two things is missing.

<a name="chunk-dispatch-tangle"></a><sub>[`src/main.rs`](../../crates/x0k-tangle/src/main.rs) · `#dispatch-tangle`</sub>

```rust {#dispatch-tangle file="src/main.rs"}
Command::Tangle { paths, workspace, force } => {
    // Route identity tangling through the unified dispatcher.
    // The default registry has `IdentityPipeline` registered;
    // docs that also declare extra pipelines will error here
    // because this binary doesn't ship those plugins.
    let ws = workspace.unwrap_or_else(|| std::env::current_dir().unwrap());
    let docs = discover_documents(&paths)?;
    let registry = x0k_tangle::PipelineRegistry::default();
    let settings = clobber_settings(force);
    let mut total_files = 0;
    let mut tangled_docs = 0;
    let mut nowhere_to_write = 0;

    for doc_path in &docs {
        let declared = declares(doc_path);
        if !declared.target {
            eprintln!("  {}", nothing_to_write(doc_path, declared.chunks));
            nowhere_to_write += 1;
            continue;
        }
        tangled_docs += 1;
        let result = x0k_tangle::tangle_document_with(doc_path, &ws, &registry, &settings)?;
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
        total_files, tangled_docs
    );

    if nowhere_to_write > 0 {
        eprintln!("{nowhere_to_write} document(s) named nowhere to write");
        std::process::exit(1);
    }
}
```

`sync` counts a chunk it was asked to fill and could not as a failure of
the run, not a warning on the way past. The predicate is *any*, not *all*:
one document whose `from=` chunk still holds a stale body is exactly the
drift the verb removes, and a run that reports it and exits 0 lets a CI job
go green over a document no longer saying what its source says. A document
with nothing to fill — no `from=` chunks, or chunks that name a file and no
symbol — raises no error and passes, so "sync is clean" keeps meaning
something in a tree that mostly does not use the feature.

<a name="chunk-dispatch-sync"></a><sub>[`src/main.rs`](../../crates/x0k-tangle/src/main.rs) · `#dispatch-sync`</sub>

```rust {#dispatch-sync file="src/main.rs"}
Command::Sync { paths, workspace } => {
    let ws = workspace.unwrap_or_else(|| std::env::current_dir().unwrap());
    let docs = discover_documents_any(&paths)?;
    let mut total_populated = 0;
    let mut unfilled = 0;

    for doc_path in &docs {
        let result = x0k_tangle::sync::sync_document(doc_path, &ws)?;

        for err in &result.errors {
            eprintln!("  error: {}: {}", doc_path.display(), err);
            unfilled += 1;
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

    if unfilled > 0 {
        eprintln!("{unfilled} chunk(s) named a source and were left empty");
        std::process::exit(1);
    }
}
```

`check` has three halves — the arithmetic is wrong and the third one is
why. The first walks every markdown document under the paths and
verifies the chunk references of any that declares chunks. The second
reads every folio/v1 envelope under the same paths against the shipped
vocabulary ([`cli-faces.md`](cli-faces.md)) and prints what it found
in the affordance's own two categories: a defect as `<path>: <defect>`,
which fails the run, and a dangling edge as a `note:` that names the
target and the set it is missing from. The third rides the first walk:
it holds the id every envelope declared and fails the run when two
documents declare the same one.

The three run in one pass over one set, which is the repair. They used
to run over two: the envelope half asked
[`cli-faces.md`](cli-faces.md)'s discovery, which parses each file, and
the reference half asked `discover_documents`, which grepped for the
text `tangle:`. Two membership predicates over one argument is two
answers to "what did you check", and only the second one got counted.

A document id is the graph's primary key — every edge in every envelope
resolves through it, and a `cites:` naming a doubled id names both
documents or neither. Two documents holding one id is therefore a
defect of the set rather than of either file, so the check is over the
paths scanned (there is nothing else this process can see) and fails the
run rather than noting it. The message names both files, because the
answer is always to change one of them and the reader needs to know
which two are in play.

<a name="chunk-dispatch-check"></a><sub>[`src/main.rs`](../../crates/x0k-tangle/src/main.rs) · `#dispatch-check`</sub>

```rust {#dispatch-check file="src/main.rs"}
Command::Check { paths, workspace, vocabulary } => {
    let ws = workspace.unwrap_or_else(|| std::env::current_dir().unwrap());
    let model = x0k_tangle::faces::vocabulary(vocabulary.as_deref())?;
    let mut has_errors = false;
    let mut chunked_documents = 0;
    let mut source_refs = 0;
    let mut ids: HashMap<String, PathBuf> = HashMap::new();

    for doc_path in markdown_under(&paths) {
        let content = match std::fs::read_to_string(&doc_path) {
            Ok(content) => content,
            Err(e) => {
                // A file the gate cannot read is a file it cannot vouch
                // for, and saying so beats counting it as clean.
                eprintln!("{}: cannot be read: {e}", doc_path.display());
                has_errors = true;
                continue;
            }
        };
        let parsed = match x0k_tangle::parser::parse_document(&content) {
            Ok(parsed) => parsed,
            Err(e) => {
                eprintln!("{}: does not parse: {e}", doc_path.display());
                has_errors = true;
                continue;
            }
        };

        if !parsed.chunks.is_empty() {
            chunked_documents += 1;
            for err in &x0k_tangle::resolve::check_all_refs(&parsed)? {
                eprintln!("{}: {}", doc_path.display(), err);
                has_errors = true;
            }
            let sources = x0k_tangle::source_check::check_source_refs(&parsed, &ws);
            source_refs += sources.checked;
            for finding in &sources.findings {
                eprintln!("{}: {}", doc_path.display(), finding);
                has_errors = true;
            }
        }

        if let Some(id) = parsed.id {
            match ids.get(&id) {
                Some(first) => {
                    eprintln!(
                        "{}: duplicate document id `{id}`, already declared by {}",
                        doc_path.display(),
                        first.display()
                    );
                    has_errors = true;
                }
                None => {
                    ids.insert(id, doc_path.clone());
                }
            }
        }
    }

    let report = x0k_tangle::faces::check_vocabulary(&model, &paths)?;
    for (path, reason) in &report.unparsed {
        eprintln!("{path}: envelope does not parse: {reason}");
        has_errors = true;
    }
    for (path, defect) in &report.corpus.defects {
        eprintln!("{path}: {defect}");
        has_errors = true;
    }
    for defect in &report.declarations.defects {
        eprintln!("{defect}");
        has_errors = true;
    }
    for edge in &report.corpus.dangling {
        eprintln!(
            "{}",
            dangling_note(&edge.source, &edge.predicate, &edge.target)
        );
    }

    if has_errors {
        std::process::exit(1);
    } else {
        eprintln!(
            "{}; {} envelope(s) read against the vocabulary, {} declaration(s) checked, {} edge(s) leave the set",
            references_verdict(chunked_documents, source_refs),
            report.corpus.checked,
            report.declarations.checked,
            report.corpus.dangling.len()
        );
    }
}
```

The green line says what it did, and the counts are what make that
possible to read. `all references OK` used to print over a set whose
references had never been read — the same six words for a corpus of
forty chapters and for a directory the walk had dropped every document
out of. A verdict that asserts the work it skipped is worse than no
verdict at all: it is the gate reporting a pass it did not run, and a
reader has no way to tell the two apart.

The line names its two kinds separately because they are two claims,
and the same sentence has now overstated three separate times. A
`<<splice>>` resolves inside the document; a `from=` resolves against a
file on disk. A run can read forty documents of splices and open no
source file at all, and a line that folded both into "all references
OK" would say the same words either way.

<a name="chunk-references-verdict"></a><sub>[`src/main.rs`](../../crates/x0k-tangle/src/main.rs) · `#references-verdict`</sub>

```rust {#references-verdict file="src/main.rs"}
/// What `check` says about the reference half of a clean run.
///
/// The counts are load-bearing, and they are separate because they are
/// separate claims. Zero is a real and common answer for either — a
/// directory of decision documents declares no chunks, and most
/// documents that do declare chunks name no source file — and each has
/// to read as zero rather than as a pass, because the shape that
/// produces it is also the shape a broken walk produces.
fn references_verdict(chunked_documents: usize, source_refs: usize) -> String {
    if chunked_documents == 0 {
        return "no chunk references to check".to_string();
    }
    let docs = match chunked_documents {
        1 => "1 document with chunks".to_string(),
        n => format!("{n} documents with chunks"),
    };
    let sources = match source_refs {
        0 => "no from= source references declared".to_string(),
        1 => "1 from= source reference resolves".to_string(),
        n => format!("{n} from= source references resolve"),
    };
    format!("splice references resolve in {docs}, {sources}")
}
```

`affordances` is the one literate verb whose whole product is data, so
its records go to stdout as pretty JSON and only the extractor's
refusals go to stderr.

<a name="chunk-dispatch-affordances"></a><sub>[`src/main.rs`](../../crates/x0k-tangle/src/main.rs) · `#dispatch-affordances`</sub>

```rust {#dispatch-affordances file="src/main.rs"}
Command::Affordances { paths } => {
    let report = x0k_tangle::faces::declared_affordances(&paths)?;
    for (path, reason) in &report.skipped {
        eprintln!("{path}: skipped: {reason}");
    }
    println!("{}", serde_json::to_string_pretty(&report.records)?);
}
```


`icon` prints each refusal under where it was declared, one rule per
line as the checker names them, then one summary line; a refusal fails
the run. Writing is the second half of the same verb rather than a verb
of its own because a file is only ever written from an accepted
drawing.

<a name="chunk-dispatch-icon"></a><sub>[`src/main.rs`](../../crates/x0k-tangle/src/main.rs) · `#dispatch-icon`</sub>

```rust {#dispatch-icon file="src/main.rs"}
Command::Icon { paths, out, palette } => {
    let report = x0k_tangle::faces::declared_icons(&paths)?;
    for (path, reason) in &report.skipped {
        eprintln!("{path}: skipped: {reason}");
    }
    for (place, refusal) in &report.refused {
        eprintln!("{place}:\n{refusal}");
    }
    let mut written = 0;
    if let (Some(out), Some(palette)) = (out, palette) {
        let content = std::fs::read_to_string(&palette)
            .with_context(|| format!("reading {}", palette.display()))?;
        let palette = x0k_tangle::region_repo::envelope_palette(&content)?.ok_or_else(|| {
            anyhow::anyhow!("{} carries no `palette:` in its envelope", palette.display())
        })?;
        written = x0k_tangle::faces::write_icon_files(&report, &palette, &out)?.len();
    }
    eprintln!(
        "{} icon(s) checked, {} refused, {written} file(s) written",
        report.accepted.len() + report.refused.len(),
        report.refused.len()
    );
    if !report.refused.is_empty() {
        std::process::exit(1);
    }
}
```

<a name="chunk-dispatch-index"></a><sub>[`src/main.rs`](../../crates/x0k-tangle/src/main.rs) · `#dispatch-index`</sub>

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

<a name="chunk-dispatch-weave"></a><sub>[`src/main.rs`](../../crates/x0k-tangle/src/main.rs) · `#dispatch-weave`</sub>

```rust {#dispatch-weave file="src/main.rs"}
Command::Weave { path, output_dir } => {
    let content = std::fs::read_to_string(&path)?;
    let parsed = x0k_tangle::parser::parse_document(&content)?;
    let output = x0k_tangle::weave::weave_html(&content, &parsed)?;

    if let Some(dir) = output_dir {
        std::fs::create_dir_all(&dir)?;
        // Named after the document, so weaving a second chapter into one
        // directory no longer destroys the first (weave.md § the page's name).
        let html_path = dir.join(x0k_tangle::weave::page_file_name(&path));
        std::fs::write(&html_path, &output.html)?;
        eprintln!("wove {} → {}", path.display(), html_path.display());
    } else {
        print!("{}", output.html);
    }
}
```

The region arms print the report shapes their chapters define; the
prose about what each field means lives there, not here.

<a name="chunk-dispatch-weave-region"></a><sub>[`src/main.rs`](../../crates/x0k-tangle/src/main.rs) · `#dispatch-weave-region`</sub>

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

<a name="chunk-dispatch-project-repo"></a><sub>[`src/main.rs`](../../crates/x0k-tangle/src/main.rs) · `#dispatch-project-repo`</sub>

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

<a name="chunk-dispatch-publish-repo"></a><sub>[`src/main.rs`](../../crates/x0k-tangle/src/main.rs) · `#dispatch-publish-repo`</sub>

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

<a name="chunk-dispatch-receive-repo"></a><sub>[`src/main.rs`](../../crates/x0k-tangle/src/main.rs) · `#dispatch-receive-repo`</sub>

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

<a name="chunk-dispatch-workspace"></a><sub>[`src/main.rs`](../../crates/x0k-tangle/src/main.rs) · `#dispatch-workspace`</sub>

```rust {#dispatch-workspace file="src/main.rs"}
Command::Workspace { root, force } => {
    let ws = resolve_workspace_root(root)?;
    let registry = x0k_tangle::PipelineRegistry::default();
    let settings = clobber_settings(force);
    let report = x0k_tangle::tangle_workspace_with(&ws, &registry, &settings)?;
    print_workspace_summary(&ws, &report);
    if !report.errored.is_empty() {
        std::process::exit(1);
    }
}
```

<a name="chunk-dispatch-list"></a><sub>[`src/main.rs`](../../crates/x0k-tangle/src/main.rs) · `#dispatch-list`</sub>

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

<a name="chunk-resolve-workspace-root"></a><sub>[`src/main.rs`](../../crates/x0k-tangle/src/main.rs) · `#resolve-workspace-root`</sub>

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

Two verbs take `--force` and both mean the same thing by it, so the
translation from flag to policy lives in one place. The default is the
absence of the flag rather than a configured value: a run that did not
say "overwrite" gets the guard.

<a name="chunk-clobber-settings"></a><sub>[`src/main.rs`](../../crates/x0k-tangle/src/main.rs) · `#clobber-settings`</sub>

```rust {#clobber-settings file="src/main.rs"}
/// The run-scoped settings a `--force` flag decides.
fn clobber_settings(force: bool) -> x0k_tangle::TangleSettings {
    x0k_tangle::TangleSettings {
        clobber: if force {
            x0k_tangle::ClobberPolicy::Force
        } else {
            x0k_tangle::ClobberPolicy::Refuse
        },
        ..Default::default()
    }
}
```

<a name="chunk-print-workspace-summary"></a><sub>[`src/main.rs`](../../crates/x0k-tangle/src/main.rs) · `#print-workspace-summary`</sub>

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

What `check` says about an edge whose target is not in the set it read
lives in one place, because it is a claim about the reader's tree and
the tree is the one thing this process cannot see past.

<a name="chunk-dangling-note"></a><sub>[`src/main.rs`](../../crates/x0k-tangle/src/main.rs) · `#dangling-note`</sub>

```rust {#dangling-note file="src/main.rs"}
/// The note `check` prints for an edge that leaves the set.
///
/// The set is whatever the paths on the command line contain, and nothing
/// beyond it is knowable from here: not whether a wider corpus exists, not
/// whether this tree was projected out of one. So the note names the
/// situation and stops. A note that instead told the reader their edge
/// pointed into "the corpus this was projected from" would be true of one
/// repository and read as a misconfiguration to everyone else.
fn dangling_note(source: &str, predicate: &str, target: impl std::fmt::Display) -> String {
    format!("{source}: note: edge `{predicate}` → `{target}` names no document under the paths scanned")
}
```

## Which documents a verb is about

Every verb here starts by turning paths into documents, and the shape of
that step decides what the verb can be trusted to have done. It has two
moves, and keeping them apart is the whole discipline: *find* the
markdown, then *decide membership by parsing it*.

The old code fused the two and did the deciding with `content.contains`.
That failed three ways at once. It admitted any prose that merely wrote
the word `tangle:`. It missed every document whose only declaration was
`pipelines:`, since this binary's grep did not name that key — and a
missed pipelines document is a loud error this binary would otherwise
have raised, silently not raised. And, worst, the grep lived only in the
directory branch: a named file was taken as given, so the *same
document* answered differently depending on how you named it.
`check dir/` walked past a document with a broken `<<ref>>` and no
`tangle:` block and then printed `all references OK`; `check dir/doc.md`
found the same broken reference and exited 1. The documents that shape
recommends first — reference-only pages built from `from=`/`symbol=`
chunks, which need no `tangle:` block at all — are exactly the ones the
directory form dropped, and the directory form is the one every document
here tells a reader to run.

<a name="chunk-markdown-under"></a><sub>[`src/main.rs`](../../crates/x0k-tangle/src/main.rs) · `#markdown-under`</sub>

```rust {#markdown-under file="src/main.rs"}
/// Every `.md` under `paths`, deduplicated and ordered: a file is taken
/// as given, a directory is walked.
///
/// This step finds files and nothing else. What a verb *does* with a
/// document is decided from its parse, below, so that the answer cannot
/// depend on whether the reader named the file or the directory holding
/// it.
fn markdown_under(paths: &[PathBuf]) -> Vec<PathBuf> {
    let mut found = Vec::new();
    for path in paths {
        if path.is_file() {
            if is_markdown(path) {
                found.push(path.clone());
            }
        } else if path.is_dir() {
            for entry in walkdir::WalkDir::new(path)
                .into_iter()
                .filter_map(|e| e.ok())
            {
                if is_markdown(entry.path()) {
                    found.push(entry.path().to_path_buf());
                }
            }
        }
    }
    found.sort();
    // Overlapping arguments (`check corpus corpus/implementation`) reach
    // one file twice, and a document counted twice collides with itself
    // on its own id.
    let mut seen = std::collections::HashSet::new();
    found.retain(|p| seen.insert(p.canonicalize().unwrap_or_else(|_| p.clone())));
    found
}

fn is_markdown(path: &Path) -> bool {
    path.extension().is_some_and(|e| e == "md")
}
```

What a document declares is read once, off the parse, into the three
facts the verbs actually ask about. `target` is `tangle_document`'s own
precondition, restated here so the CLI can tell "declared nothing" from
"declared something that produced nothing" and report the first without
guessing at the second.

<a name="chunk-declares"></a><sub>[`src/main.rs`](../../crates/x0k-tangle/src/main.rs) · `#declares`</sub>

```rust {#declares file="src/main.rs"}
/// What a document declares about itself, read off its parse.
struct Declares {
    /// It names somewhere to write: a `tangle:` crate or root,
    /// per-language roots, or a `pipelines:` block. The same predicate
    /// `tangle_document` applies before it does any work.
    target: bool,
    /// It has a chunk to fill from source (`from=`), which is what
    /// `sync` is about.
    fills: bool,
    /// How many chunks it declares — the number that makes "nothing to
    /// write" worth saying out loud instead of reporting as a zero.
    chunks: usize,
}

fn declares(path: &Path) -> Declares {
    let Ok(content) = std::fs::read_to_string(path) else {
        return Declares { target: false, fills: false, chunks: 0 };
    };
    let Ok(parsed) = x0k_tangle::parser::parse_document(&content) else {
        return Declares { target: false, fills: false, chunks: 0 };
    };
    Declares {
        target: parsed.tangle_crate.is_some()
            || parsed.tangle_root.is_some()
            || !parsed.tangle_roots.is_empty()
            || !parsed.pipelines.is_empty(),
        fills: parsed
            .chunks
            .values()
            .flatten()
            .any(|chunk| chunk.from.is_some()),
        chunks: parsed.chunks.len(),
    }
}
```

The two writing verbs sweep a directory for the documents they are
about, and take a named file as given. That asymmetry is deliberate and
it is not the one repaired above: `check` answers a *question* about a
document, so the answer must not depend on how the document was reached,
while `tangle` and `sync` take an *instruction*, and naming a file is a
different instruction from naming the tree it sits in. Naming one is how
a reader gets told that this document writes nothing —
`tangle`'s arm says it; a sweep stays quiet about the documents
that are simply not its business.

<a name="chunk-discover-documents"></a><sub>[`src/main.rs`](../../crates/x0k-tangle/src/main.rs) · `#discover-documents`</sub>

```rust {#discover-documents file="src/main.rs"}
fn discover_documents_any(paths: &[PathBuf]) -> Result<Vec<PathBuf>> {
    let named = named_files(paths);
    Ok(markdown_under(paths)
        .into_iter()
        .filter(|p| {
            named.contains(p) || {
                let d = declares(p);
                d.fills || d.target
            }
        })
        .collect())
}

fn discover_documents(paths: &[PathBuf]) -> Result<Vec<PathBuf>> {
    let named = named_files(paths);
    Ok(markdown_under(paths)
        .into_iter()
        .filter(|p| named.contains(p) || declares(p).target)
        .collect())
}

/// The paths the reader named as files rather than directories.
fn named_files(paths: &[PathBuf]) -> std::collections::HashSet<PathBuf> {
    paths.iter().filter(|p| p.is_file()).cloned().collect()
}
```

A document a writing verb was handed and cannot write from gets one
sentence, and the sentence names both halves of what is missing — the
chunks it does have, and the declaration it does not — because those are
the two things a reader is deciding between when nothing appeared on
disk.

<a name="chunk-nothing-to-write"></a><sub>[`src/main.rs`](../../crates/x0k-tangle/src/main.rs) · `#nothing-to-write`</sub>

```rust {#nothing-to-write file="src/main.rs"}
/// What `tangle` says about a document it was named and cannot write from.
fn nothing_to_write(path: &Path, chunks: usize) -> String {
    format!(
        "{}: declares {chunks} chunk(s) and no tangle target \
         (tangle.root, tangle.crate, tangle.roots, or pipelines:); nothing to write",
        path.display()
    )
}
```

## Composing the crate root and the binary

<a name="chunk-root"></a><sub>[`src/lib.rs`](../../crates/x0k-tangle/src/lib.rs) · `#root` · assembles [crate-doc](#chunk-crate-doc) · [modules](#chunk-modules) · [exports](#chunk-exports) · [source-check](#chunk-source-check)</sub>

```rust {#root}
<<crate-doc>>

<<modules>>

<<exports>>

<<source-check>>
```

<a name="chunk-bin-root"></a><sub>[`src/main.rs`](../../crates/x0k-tangle/src/main.rs) · `#bin-root` · assembles [bin-doc](#chunk-bin-doc) · [cli-imports](#chunk-cli-imports) · [cli-struct](#chunk-cli-struct) · [command-enum](#chunk-command-enum) · [main-fn](#chunk-main-fn) · [resolve-workspace-root](#chunk-resolve-workspace-root) · [clobber-settings](#chunk-clobber-settings) · [print-workspace-summary](#chunk-print-workspace-summary) · [dangling-note](#chunk-dangling-note) · [references-verdict](#chunk-references-verdict) · [nothing-to-write](#chunk-nothing-to-write) · [markdown-under](#chunk-markdown-under) · [declares](#chunk-declares) · [discover-documents](#chunk-discover-documents)</sub>

```rust {#bin-root file="src/main.rs"}
<<bin-doc>>

<<cli-imports>>

<<cli-struct>>

<<command-enum>>

<<main-fn>>

<<resolve-workspace-root>>

<<clobber-settings>>

<<print-workspace-summary>>

<<dangling-note>>

<<references-verdict>>

<<nothing-to-write>>

<<markdown-under>>

<<declares>>

<<discover-documents>>
```

The crate's boundary is the thing to keep honest. Every module is
public and the CLI is thin, so there is no place for behaviour to hide
that a consumer could not reach by name. When a verb grows a mechanism,
it moves to a chapter; when a chapter's type is meant to be named from
outside, it appears in the export list above.

`source_check` is the one thing this chapter keeps, and it is here for
a reason the layout cannot express anywhere else: it is a policy two
binaries have to agree on. Every other duplicated line between
`main.rs` and the bundle's copy is a sentence; this one is the rule that
decides whether a tree is sound, and a copy of it would eventually
disagree with itself.

## Pinning the verdicts

Most of this chapter's claims are about what the process does rather than
what it computes — an exit code, or a sentence a reader believes or does
not — and none of those is reachable from a unit test of a library
function. So they are pinned the way [`cli-faces.md`](cli-faces.md) pins
the other faces: run the built binary over a temp fixture, and let what
it prints and how it exits be the claim. `sync` exits non-zero when a
chunk it was asked to fill stayed empty. `check`'s dangling-edge note
says only what is true of the tree it was pointed at.

Three more are pins on the gate itself, and they exist because the gate
failed open on all three. `check` returns the same verdict for a
document whether it is handed the file or the directory holding it. Its
green line names the work it did rather than asserting work it skipped.
Two documents holding one id fail the run. And `tangle`, handed a
document that names nowhere to write, says so instead of reporting a
successful zero.

The source-reference pins are the fourth failure-open, and the largest.
One per way a `from=` breaks — a symbol the file does not have, a file
that is not there, a symbol two definitions answer to, a language the
extractor cannot walk, a chunk that names a file and no symbol in a
language it could have — plus the two that say what the verb is for:
`check` writes nothing, and it catches the stale body. That last one
runs the whole hazard: sync a chunk, rename the symbol in the source,
and the document is byte-identical to what `sync` wrote. `git status`
is clean, a re-tangle sees nothing, and until this the gate said the
tree was sound.

Three more pin `--force` and what it is an escape from, because the
guard is only worth having if it is reachable from the shell the
maintainer actually ran: a hand-edited generated file refuses the
document and survives, `--force` overwrites it, and a document that
simply moved forward re-tangles with no flag at all. That last one is
the important one — it is the whole corpus, and a guard that got it
wrong would refuse everything.

<a name="chunk-cli-verdicts"></a><sub>[`tests/cli_verdicts.rs`](../../crates/x0k-tangle/tests/cli_verdicts.rs) · `#cli-verdicts`</sub>

`````rust {#cli-verdicts file="tests/cli_verdicts.rs"}
//! Pins for the verdicts the CLI returns
//! (`x0k:implementation/tangle/crate`): what `sync` exits with when a
//! chunk it was asked to fill stayed empty, what `check` says about an
//! edge that leaves the set, how `check` reports a `from=` that no
//! longer resolves without writing a byte, and the ways `check` and
//! `tangle` used to report a pass they had not run.

use std::fs;
use std::path::Path;
use std::process::{Command, Output};

use tempfile::TempDir;

const JS_SOURCE: &str =
    "export function createHorizonRemap(scale) {\n  return (u) => u * scale;\n}\n";

/// A folio/v1 document with a chunk whose `<<ref>>` names nothing, and
/// no `tangle:` block — the reference-only shape an adopter writes
/// first. `{id}` distinguishes copies.
fn broken_reference_doc(id: &str) -> String {
    format!(
        "---\nx0k:\n  format: folio/v1\n  id: x0k:implementation/{id}\n  \
         type: implementation\n  status: draft\n  summary: A document with a \
         broken chunk reference and nowhere to write.\n---\n# Doc\n\n\
         ```rust {{#root file=\"src/lib.rs\"}}\nfn f() {{\n    <<nope>>\n}}\n```\n"
    )
}

fn write(dir: &Path, rel: &str, content: &str) {
    let path = dir.join(rel);
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, content).unwrap();
}

fn run(args: &[&str], dir: &Path) -> Output {
    Command::new(env!("CARGO_BIN_EXE_x0k-tangle"))
        .args(args)
        .arg(dir)
        .output()
        .expect("the x0k-tangle binary runs")
}

fn sync(dir: &Path) -> Output {
    Command::new(env!("CARGO_BIN_EXE_x0k-tangle"))
        .arg("sync")
        .arg(dir)
        .arg("--workspace")
        .arg(dir)
        .output()
        .expect("the x0k-tangle binary runs")
}

#[test]
fn sync_fills_a_javascript_chunk_and_passes() {
    let tmp = TempDir::new().unwrap();
    write(tmp.path(), "remap.js", JS_SOURCE);
    write(
        tmp.path(),
        "doc.md",
        "# Remap\n\n```javascript {#remap from=\"remap.js\" symbol=\"createHorizonRemap\"}\n```\n",
    );

    let out = sync(tmp.path());
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(out.status.success(), "sync failed: {stderr}");
    assert!(stderr.contains("synced 1 chunk(s)"), "got {stderr}");

    let synced = fs::read_to_string(tmp.path().join("doc.md")).unwrap();
    assert!(
        synced.contains("export function createHorizonRemap(scale)"),
        "got {synced}"
    );
}

#[test]
fn sync_exits_nonzero_when_a_chunk_it_was_asked_to_fill_stayed_empty() {
    let tmp = TempDir::new().unwrap();
    write(
        tmp.path(),
        "doc.md",
        "# Remap\n\n```rust {#remap from=\"missing.rs\" symbol=\"remap\"}\n```\n",
    );

    let out = sync(tmp.path());
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        !out.status.success(),
        "a sync that filled nothing reported success: {stderr}"
    );
    assert!(stderr.contains("error:"), "got {stderr}");
}

#[test]
fn sync_names_the_language_limit_rather_than_the_symbol() {
    let tmp = TempDir::new().unwrap();
    write(tmp.path(), "remap.rb", "def create_remap\n  1\nend\n");
    write(
        tmp.path(),
        "doc.md",
        "# Remap\n\n```ruby {#remap from=\"remap.rb\" symbol=\"create_remap\"}\n```\n",
    );

    let out = sync(tmp.path());
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(!out.status.success(), "got {stderr}");
    assert!(stderr.contains("symbol extraction supports"), "got {stderr}");
    assert!(
        !stderr.contains("not found"),
        "an unwalkable language must not read as a mistyped symbol: {stderr}"
    );
}

#[test]
fn sync_passes_a_document_with_nothing_to_fill() {
    let tmp = TempDir::new().unwrap();
    write(
        tmp.path(),
        "doc.md",
        "---\nx0k:\n  format: folio/v1\n  id: x0k:implementation/fixture\n  \
         type: implementation\n  status: draft\n  tangle:\n    crate: fixture\n    \
         root: src/lib.rs\n---\n# Doc\n\n```rust {#root}\nfn f() {}\n```\n",
    );

    let out = sync(tmp.path());
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(out.status.success(), "sync failed: {stderr}");
    assert!(stderr.contains("synced 0 chunk(s)"), "got {stderr}");
}

// ---- `check` resolves every `from=` -------------------------------------
//
// The read-only gate. `sync` is the verb that repairs a source
// reference and it repairs by writing, so no CI job can run it; these
// pin the verb that can. Each names one way a reference breaks, and the
// last two are the reason the feature exists: `check` must write
// nothing, and it must catch the stale body `sync` leaves behind.

/// Two definitions one `symbol=` matches, so the ambiguity report has
/// something to report.
const AMBIGUOUS_SOURCE: &str = "use std::fmt;\n\npub struct Horizon;\n\nimpl fmt::Debug for Horizon {\n    fn render(&self) -> u8 {\n        1\n    }\n}\n\nimpl fmt::Display for Horizon {\n    fn render(&self) -> u8 {\n        2\n    }\n}\n";

fn check_in(dir: &Path) -> Output {
    Command::new(env!("CARGO_BIN_EXE_x0k-tangle"))
        .arg("check")
        .arg(dir)
        .arg("--workspace")
        .arg(dir)
        .output()
        .expect("the x0k-tangle binary runs")
}

#[test]
fn check_fails_a_symbol_the_source_does_not_have() {
    let tmp = TempDir::new().unwrap();
    write(tmp.path(), "remap.js", JS_SOURCE);
    write(
        tmp.path(),
        "doc.md",
        "# Remap\n\n```javascript {#remap from=\"remap.js\" symbol=\"createHorizonRemapp\"}\n```\n",
    );

    let out = check_in(tmp.path());
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        !out.status.success(),
        "a misspelled symbol passed the gate: {stderr}"
    );
    assert!(
        stderr.contains("symbol 'createHorizonRemapp' not found"),
        "the finding names the symbol it looked for: {stderr}"
    );
    assert!(
        stderr.contains("chunk 'remap'"),
        "the finding names the chunk it is about: {stderr}"
    );
}

#[test]
fn check_fails_a_from_naming_a_file_that_is_not_there() {
    let tmp = TempDir::new().unwrap();
    write(
        tmp.path(),
        "doc.md",
        "# Remap\n\n```rust {#remap from=\"gone/remap.rs\" symbol=\"remap\"}\n```\n",
    );

    let out = check_in(tmp.path());
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(!out.status.success(), "a dangling file passed: {stderr}");
    assert!(
        stderr.contains("source file not found") && stderr.contains("gone/remap.rs"),
        "the finding names the path it resolved to: {stderr}"
    );
}

#[test]
fn check_reports_the_candidates_for_an_ambiguous_symbol() {
    let tmp = TempDir::new().unwrap();
    write(tmp.path(), "horizon.rs", AMBIGUOUS_SOURCE);
    write(
        tmp.path(),
        "doc.md",
        "# Horizon\n\n```rust {#h from=\"horizon.rs\" symbol=\"Horizon::render\"}\n```\n",
    );

    let out = check_in(tmp.path());
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(!out.status.success(), "an ambiguous symbol passed: {stderr}");
    assert!(
        stderr.contains("is ambiguous — 2 definitions match"),
        "the finding counts the candidates: {stderr}"
    );
    assert!(
        stderr.contains("select it with symbol=\"<Horizon as fmt::Display>::render\""),
        "the finding hands the reader the spelling that resolves it: {stderr}"
    );
}

#[test]
fn check_names_the_language_limit_rather_than_the_symbol() {
    let tmp = TempDir::new().unwrap();
    write(tmp.path(), "remap.rb", "def create_remap\n  1\nend\n");
    write(
        tmp.path(),
        "doc.md",
        "# Remap\n\n```ruby {#remap from=\"remap.rb\" symbol=\"create_remap\"}\n```\n",
    );

    let out = check_in(tmp.path());
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(!out.status.success(), "got {stderr}");
    assert!(stderr.contains("symbol extraction supports"), "got {stderr}");
    assert!(
        !stderr.contains("not found"),
        "an unwalkable language must not read as a mistyped symbol: {stderr}"
    );
}

#[test]
fn check_fails_a_bare_from_in_a_language_sync_can_walk() {
    let tmp = TempDir::new().unwrap();
    write(tmp.path(), "remap.js", JS_SOURCE);
    write(
        tmp.path(),
        "doc.md",
        "# Remap\n\n```javascript {#remap from=\"remap.js\"}\nexport function gone() {}\n```\n",
    );

    let out = check_in(tmp.path());
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        !out.status.success(),
        "a chunk sync will skip forever passed the gate: {stderr}"
    );
    assert!(
        stderr.contains("no symbol=") && stderr.contains("sync skips it in silence"),
        "the finding says why the body will never be refreshed: {stderr}"
    );
}

/// The deliberate shape: a whole file quoted in a language symbol
/// extraction cannot walk. The corpus has one, and the gate has no
/// business refusing it.
#[test]
fn check_passes_a_whole_file_from_in_a_language_sync_cannot_walk() {
    let tmp = TempDir::new().unwrap();
    write(tmp.path(), "fx/glitch.toml", "name = \"glitch\"\n");
    write(
        tmp.path(),
        "doc.md",
        "# Glitch\n\n```toml {#glitch from=\"fx/glitch.toml\"}\nname = \"glitch\"\n```\n",
    );

    let out = check_in(tmp.path());
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(out.status.success(), "check failed: {stderr}");
    assert!(
        stderr.contains("1 from= source reference resolves"),
        "the whole-file reference is counted as read: {stderr}"
    );
}

/// …but only while the file is there. The half of that claim which is
/// checkable from here still is checked.
#[test]
fn check_fails_a_whole_file_from_whose_file_is_gone() {
    let tmp = TempDir::new().unwrap();
    write(
        tmp.path(),
        "doc.md",
        "# Glitch\n\n```toml {#glitch from=\"fx/glitch.toml\"}\nname = \"glitch\"\n```\n",
    );

    let out = check_in(tmp.path());
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(!out.status.success(), "got {stderr}");
    assert!(
        stderr.contains("source file not found") && stderr.contains("glitch.toml"),
        "got {stderr}"
    );
}

#[test]
fn check_counts_the_source_references_it_resolved() {
    let tmp = TempDir::new().unwrap();
    write(tmp.path(), "remap.js", JS_SOURCE);
    write(
        tmp.path(),
        "doc.md",
        "# Remap\n\n```javascript {#remap from=\"remap.js\" symbol=\"createHorizonRemap\"}\n```\n",
    );

    let out = check_in(tmp.path());
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(out.status.success(), "check failed: {stderr}");
    assert!(
        stderr.contains("splice references resolve in 1 document with chunks")
            && stderr.contains("1 from= source reference resolves"),
        "the green line names both kinds it read: {stderr}"
    );
    assert!(
        !stderr.contains("all references OK"),
        "the line that overstated three times is gone: {stderr}"
    );
}

#[test]
fn check_says_a_document_of_splices_declared_no_source_references() {
    let tmp = TempDir::new().unwrap();
    write(
        tmp.path(),
        "doc.md",
        "# Doc\n\n```rust {#root file=\"src/lib.rs\"}\nfn f() {\n    <<inner>>\n}\n```\n\n\
         ```rust {#inner}\nlet x = 1;\n```\n",
    );

    let out = check_in(tmp.path());
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(out.status.success(), "check failed: {stderr}");
    assert!(
        stderr.contains("no from= source references declared"),
        "a run that opened no source file says so: {stderr}"
    );
}

/// The whole point of the verb: it is the one that can run in CI, and it
/// can only run in CI if it never writes. Nothing on disk moves, on the
/// failing path or the passing one.
#[test]
fn check_writes_nothing() {
    let tmp = TempDir::new().unwrap();
    write(tmp.path(), "remap.js", JS_SOURCE);
    write(
        tmp.path(),
        "doc.md",
        "# Remap\n\n```javascript {#remap from=\"remap.js\" symbol=\"nope\"}\nstale body\n```\n",
    );

    let before = fs::read_to_string(tmp.path().join("doc.md")).unwrap();
    let source_before = fs::read_to_string(tmp.path().join("remap.js")).unwrap();

    let out = check_in(tmp.path());
    assert!(!out.status.success());

    assert_eq!(
        fs::read_to_string(tmp.path().join("doc.md")).unwrap(),
        before,
        "check rewrote the document it was judging"
    );
    assert_eq!(
        fs::read_to_string(tmp.path().join("remap.js")).unwrap(),
        source_before,
        "check rewrote the source"
    );
    assert!(
        !tmp.path().join("doc.tangle-map.json").exists(),
        "check wrote a sidecar"
    );
}

/// The hazard this closes, end to end. Sync a chunk successfully, rename
/// the symbol in the source, and the document is now byte-identical to
/// what `sync` left: `git status` is clean and a re-tangle sees nothing.
/// Before this, `check` printed a pass over it.
#[test]
fn check_catches_the_stale_body_sync_left_behind() {
    let tmp = TempDir::new().unwrap();
    write(tmp.path(), "remap.js", JS_SOURCE);
    write(
        tmp.path(),
        "doc.md",
        "# Remap\n\n```javascript {#remap from=\"remap.js\" symbol=\"createHorizonRemap\"}\n```\n",
    );

    assert!(sync(tmp.path()).status.success(), "the fixture must sync");
    let synced = fs::read_to_string(tmp.path().join("doc.md")).unwrap();
    assert!(synced.contains("createHorizonRemap"));

    // The rename happens in the source. The document is not touched.
    write(
        tmp.path(),
        "remap.js",
        &JS_SOURCE.replace("createHorizonRemap", "createHorizonMap"),
    );

    let out = check_in(tmp.path());
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        !out.status.success(),
        "the document still shows a body its source no longer has, and check passed: {stderr}"
    );
    assert!(
        stderr.contains("symbol 'createHorizonRemap' not found"),
        "the finding names the symbol the document is still displaying: {stderr}"
    );
    assert_eq!(
        fs::read_to_string(tmp.path().join("doc.md")).unwrap(),
        synced,
        "the gate that caught it also left the document alone"
    );
}

/// A predicate this build is certain to accept, so the fixture measures
/// the note and not the module selection.
fn shipped_predicate() -> &'static str {
    x0k_ontology::KNOWN_EDGE_PREDICATES
        .first()
        .copied()
        .expect("a build whose vocabulary declares no document edge ships no document module")
}

#[test]
fn the_dangling_edge_note_claims_only_what_is_true_of_any_tree() {
    let tmp = TempDir::new().unwrap();
    write(
        tmp.path(),
        "docs/fixture.md",
        &format!(
            "---\nx0k:\n  format: folio/v1\n  id: x0k:design/fixture\n  type: design\n  \
             status: draft\n  edges:\n    {}:\n      - x0k:design/elsewhere\n---\n# Fixture\n",
            shipped_predicate()
        ),
    );

    let out = run(&["check"], tmp.path());
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(out.status.success(), "check failed: {stderr}");
    assert!(
        stderr.contains("names no document under the paths scanned"),
        "the note names the set it scanned: {stderr}"
    );
    assert!(
        !stderr.contains("projected from"),
        "the note claims nothing about a corpus the reader may not have: {stderr}"
    );
}

#[test]
fn check_answers_the_same_for_a_file_and_for_the_directory_holding_it() {
    let tmp = TempDir::new().unwrap();
    write(tmp.path(), "docs/d.md", &broken_reference_doc("no-target"));

    let by_dir = run(&["check"], &tmp.path().join("docs"));
    let by_file = run(&["check"], &tmp.path().join("docs/d.md"));

    let dir_err = String::from_utf8_lossy(&by_dir.stderr).into_owned();
    let file_err = String::from_utf8_lossy(&by_file.stderr).into_owned();
    assert_eq!(
        by_dir.status.code(),
        by_file.status.code(),
        "the same document answered differently by path shape.\n dir: {dir_err}\nfile: {file_err}"
    );
    assert!(
        !by_dir.status.success(),
        "a directory walk let a broken reference through: {dir_err}"
    );
    assert!(
        dir_err.contains("references undefined chunk"),
        "the directory form names the broken reference: {dir_err}"
    );
}

#[test]
fn check_says_it_checked_nothing_rather_than_asserting_a_pass() {
    let tmp = TempDir::new().unwrap();
    write(
        tmp.path(),
        "docs/prose.md",
        "---\nx0k:\n  format: folio/v1\n  id: x0k:design/prose\n  type: design\n  \
         status: draft\n---\n# Prose\n\nNo chunks here.\n",
    );

    let out = run(&["check"], &tmp.path().join("docs"));
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(out.status.success(), "check failed: {stderr}");
    assert!(
        stderr.contains("no chunk references to check"),
        "a run that read no references says so: {stderr}"
    );
    assert!(
        !stderr.contains("all references OK"),
        "a gate must not report a pass it never ran: {stderr}"
    );
}

#[test]
fn check_fails_two_documents_that_declare_one_id() {
    let tmp = TempDir::new().unwrap();
    let doc = "---\nx0k:\n  format: folio/v1\n  id: x0k:design/collision\n  \
               type: design\n  status: draft\n---\n# Copy\n";
    write(tmp.path(), "docs/a.md", doc);
    write(tmp.path(), "docs/b.md", doc);

    let out = run(&["check"], &tmp.path().join("docs"));
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        !out.status.success(),
        "the graph's primary key was held twice and the check passed: {stderr}"
    );
    assert!(
        stderr.contains("duplicate document id `x0k:design/collision`"),
        "the message names the colliding id: {stderr}"
    );
    assert!(
        stderr.contains("a.md") && stderr.contains("b.md"),
        "the message names both documents in play: {stderr}"
    );
}

#[test]
fn check_does_not_see_one_document_twice_through_overlapping_paths() {
    let tmp = TempDir::new().unwrap();
    write(
        tmp.path(),
        "docs/inner/d.md",
        "---\nx0k:\n  format: folio/v1\n  id: x0k:design/once\n  type: design\n  \
         status: draft\n---\n# Once\n",
    );

    let out = Command::new(env!("CARGO_BIN_EXE_x0k-tangle"))
        .arg("check")
        .arg(tmp.path().join("docs"))
        .arg(tmp.path().join("docs/inner"))
        .output()
        .expect("the x0k-tangle binary runs");
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        out.status.success(),
        "a document reached twice collided with itself: {stderr}"
    );
}

#[test]
fn tangle_refuses_a_document_that_names_nowhere_to_write() {
    let tmp = TempDir::new().unwrap();
    write(tmp.path(), "docs/d.md", &broken_reference_doc("nowhere"));

    let out = Command::new(env!("CARGO_BIN_EXE_x0k-tangle"))
        .arg("tangle")
        .arg(tmp.path().join("docs/d.md"))
        .arg("--workspace")
        .arg(tmp.path())
        .output()
        .expect("the x0k-tangle binary runs");
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        !out.status.success(),
        "a document that wrote nothing reported a successful zero: {stderr}"
    );
    assert!(
        stderr.contains("nothing to write") && stderr.contains("declares 1 chunk(s)"),
        "the run says what the document has and what it lacks: {stderr}"
    );
}

#[test]
fn tangle_writes_a_document_that_names_a_target() {
    let tmp = TempDir::new().unwrap();
    write(
        tmp.path(),
        "docs/d.md",
        "---\nx0k:\n  format: folio/v1\n  id: x0k:implementation/writes\n  \
         type: implementation\n  status: draft\n  tangle:\n    crate: .\n    \
         root: src/lib.rs\n---\n# Doc\n\n```rust {#root}\npub fn f() {}\n```\n",
    );

    let out = Command::new(env!("CARGO_BIN_EXE_x0k-tangle"))
        .arg("tangle")
        .arg(tmp.path().join("docs/d.md"))
        .arg("--workspace")
        .arg(tmp.path())
        .output()
        .expect("the x0k-tangle binary runs");
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(out.status.success(), "tangle failed: {stderr}");
    assert!(
        stderr.contains("tangled 1 file(s) from 1 document(s)"),
        "got {stderr}"
    );
}

/// A document with one chunk and one output. `body` distinguishes one
/// generation of it from the next.
fn tangling_doc(body: &str) -> String {
    format!(
        "---\nx0k:\n  format: folio/v1\n  id: x0k:implementation/guard\n  \
         type: implementation\n  status: draft\n  tangle:\n    crate: .\n    \
         root: src/lib.rs\n---\n# Doc\n\n```rust {{#root}}\npub fn {body}() {{}}\n```\n"
    )
}

fn tangle_in(dir: &Path, force: bool) -> Output {
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_x0k-tangle"));
    cmd.arg("tangle")
        .arg(dir.join("docs/d.md"))
        .arg("--workspace")
        .arg(dir);
    if force {
        cmd.arg("--force");
    }
    cmd.output().expect("the x0k-tangle binary runs")
}

/// The maintainer's report, through the shell: a line added to a
/// generated file is not silently eaten by the next tangle.
#[test]
fn tangle_refuses_to_overwrite_a_hand_edited_output() {
    let tmp = TempDir::new().unwrap();
    write(tmp.path(), "docs/d.md", &tangling_doc("first"));
    assert!(tangle_in(tmp.path(), false).status.success());

    let out_file = tmp.path().join("src/lib.rs");
    let edited = format!("{}// HAND EDIT\n", fs::read_to_string(&out_file).unwrap());
    fs::write(&out_file, &edited).unwrap();
    write(tmp.path(), "docs/d.md", &tangling_doc("second"));

    let out = tangle_in(tmp.path(), false);
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        !out.status.success(),
        "tangling over a hand edit reported success: {stderr}"
    );
    assert!(
        stderr.contains("refused to overwrite") && stderr.contains("src/lib.rs"),
        "the refusal names what it would have destroyed: {stderr}"
    );
    assert!(
        stderr.contains("--force"),
        "the refusal names the way out: {stderr}"
    );
    assert_eq!(
        fs::read_to_string(&out_file).unwrap(),
        edited,
        "the hand edit survived"
    );
}

/// And the way out works.
#[test]
fn tangle_force_overwrites_a_hand_edited_output() {
    let tmp = TempDir::new().unwrap();
    write(tmp.path(), "docs/d.md", &tangling_doc("first"));
    assert!(tangle_in(tmp.path(), false).status.success());

    let out_file = tmp.path().join("src/lib.rs");
    fs::write(&out_file, "// HAND EDIT\n").unwrap();
    write(tmp.path(), "docs/d.md", &tangling_doc("second"));

    let out = tangle_in(tmp.path(), true);
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(out.status.success(), "--force did not overwrite: {stderr}");
    let text = fs::read_to_string(&out_file).unwrap();
    assert!(
        text.contains("second") && !text.contains("HAND EDIT"),
        "got {text}"
    );
}

/// The common path stays common: a document that moves forward rewrites
/// the output it last wrote, with no flag and no complaint.
#[test]
fn tangle_rewrites_the_output_it_last_wrote() {
    let tmp = TempDir::new().unwrap();
    write(tmp.path(), "docs/d.md", &tangling_doc("first"));
    assert!(tangle_in(tmp.path(), false).status.success());
    write(tmp.path(), "docs/d.md", &tangling_doc("second"));

    let out = tangle_in(tmp.path(), false);
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(out.status.success(), "an ordinary re-tangle failed: {stderr}");
    let text = fs::read_to_string(tmp.path().join("src/lib.rs")).unwrap();
    assert!(text.contains("second") && !text.contains("first"), "got {text}");
}
`````

## The package manifest

Document instance rendering uses the format crate’s optional document-vocabulary API.
The manifest is a complete chunk so repository projection can carry its public form.

<a name="chunk-package-manifest"></a><sub>[`Cargo.toml`](../../crates/x0k-tangle/Cargo.toml) · `#package-manifest`</sub>

```toml {#package-manifest file="Cargo.toml"}
[package]
name = "x0k-tangle"
version = "0.1.0"
edition = { workspace = true }
description = "Literate programming tangler/weaver with bidirectional sync. Extracts compilable source from folio/v1 documents and reconciles edits from either side."
license = "MIT"
keywords = ["literate-programming", "tangle", "markdown", "codegen", "documentation"]
categories = ["development-tools", "development-tools::build-utils", "command-line-utilities", "text-processing"]
rust-version = { workspace = true }
repository = "https://github.com/0k-dot-computer/x0k-folio"
readme = "../../README.md"

[lib]
name = "x0k_tangle"

[[bin]]
name = "x0k-tangle"
path = "src/main.rs"

[features]
# `motifs` wires the x0k:media / surface-wasm bundling into the HTML region
# weaver (region_project/region_weave). Default in the monorepo. x0k-surface-build
# is publish-excluded, so the repository projector severs this feature in the
# published manifest: it stays declared (the `#[cfg(feature = "motifs")]` sites
# ship) with an empty list, out of `default`, so the motif system never ships.
# The repo backend (region_repo) needs neither motifs nor syntax and stays
# feature-independent.
default = []
motifs = [] # severed in this publication: its dependency is not published; enabling it does not build

[dependencies]
x0k-folio = { path = "../x0k-folio", features = ["document-vocabulary"] , version = "0.1.0" }
# The vocabulary a `check` reads documents against. Default features carry
# the runtime module loader, which is what `--vocabulary <dir>` and the
# PROVENANCE-recorded default are: a projected repository checks its own
# documents against the module files it actually shipped.
x0k-ontology = { path = "../x0k-ontology" , version = "0.1.0" }
# Shared renderer-agnostic syntax tokenizer; weave uses it to emit
# highlighted <span class="tok-*"> spans in the HTML output.
x0k-syntax = { path = "../x0k-syntax" , version = "0.1.0" }
# The icon profile's one implementation: the repository projector reads
# every mark it shows — an affordance's own, the actor and status marks —
# from its `svg x0k:icon` declaration through this crate's checker, binds
# it to the publication's palette, and writes the per-scheme files. Nothing
# in this crate draws a mark.
x0k-icon = { path = "../x0k-icon" , version = "0.1.0" }

pulldown-cmark = { version = "0.12", default-features = false, features = ["simd"] }
# TeX → MathML at weave time. Browsers (the Chromium-only 0k.computer target)
# render MathML Core natively, so prose math needs no client-side JS library.
latex2mathml = "0.2"
tree-sitter = "0.24"
tree-sitter-rust = "0.23"
# Symbol extraction (`from=`/`symbol=` chunks) and the language-aware chunk
# ref scan both dispatch on the language a chunk declares. `x0k-syntax` owns
# the fence-tag vocabulary for the toolchain but hands out classified tokens,
# not grammars, so the TypeScript grammar is linked here beside the Rust one.
tree-sitter-typescript = "0.23"
# Python and Julia grammars for `from=` symbol extraction. Both are ABI-14
# grammars (`LANGUAGE_VERSION 14`), the ABI `tree-sitter = "0.24"` speaks;
# they move in lockstep with it and with `x0k-syntax`, which pins the same
# generation for highlighting.
tree-sitter-python = "0.23"
tree-sitter-julia = "0.23"

serde = { workspace = true }
serde_json = { workspace = true }
# A publication's `palette:` block is read out of its envelope as YAML into
# x0k-icon's own palette type.
serde_norway = "0.9"
anyhow = { workspace = true }
clap = { workspace = true }
tracing = { workspace = true }
walkdir = "2"
# Format-preserving TOML editing for the repository projector (region_repo):
# rewrites vendored crate manifests (strip workspace-hack + publish-excluded
# optional deps, set license) without disturbing hand-authored layout.
toml_edit = "0.22"
# In-process unified diffs for the receiver (receive.rs): a contribution's
# patch set is computed without shelling out, so the report is complete
# wherever the tool runs.
similar = "2"
# Reference projections for receive-repo live in a temp dir removed with
# the report.
tempfile = "3"

[dev-dependencies]
tempfile = "3"
```
