# x0k-tangle: the crate and its CLI

```turtle folio:document
implementation:tangle\/crate a x0k:Implementation ;
    x0k:status "draft" ;
    x0k:summary "The crate's contract rather than a mechanism — the module list and re-exports that say what a consumer may name, and the plugin-less CLI that puts those verbs in a shell." ;
    x0k:concerns "tangle", "literate", "crate", "cli", "features", "publishing" ;
    x0k:cites implementation:tangle\/protocol,
        implementation:tangle\/parsing,
        implementation:tangle\/resolution,
        implementation:tangle\/identity-pipeline,
        implementation:tangle\/dispatcher,
        implementation:tangle\/weave,
        implementation:tangle\/cli-faces,
        implementation:tangle\/bundle ;
    x0k:implements design:literate-programming,
        design:publish-a-region-as-a-repository ;
    folio:tangleCrate "crates/x0k-tangle" ;
    folio:tangleRoot "src/lib.rs" .
```

`x0k-tangle` is the crate that makes a [literate
document](../../background/literate-programming.md "x0k:wiki/literate-programming") executable:
it parses literate pages, expands
their named chunks into source files, weaves them into HTML, and checks
both the code and the documents' typed headers against the tree.
This chapter is the crate's contract: the module list and re-exports in
`src/lib.rs` that say what a consumer may name, and the CLI in
`src/cli.rs` that exposes those verbs to a shell, which the plugin-less
`src/main.rs` runs. Everything with a
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
- [`weave.md`](weave.md), [`instance-rendering.md`](instance-rendering.md),
  [`region-gfm.md`](region-gfm.md) — one document rendered as HTML, and
  one chapter woven for a forge's renderer.
- [`doc-index.md`](doc-index.md) — the corpus seen from outside.
- [`cli-faces.md`](cli-faces.md) — the vocabulary check and the
  affordance read-out behind the `check` and `affordances` verbs, the
  two that make a shipped human claim true from a shell, and the
  `declarations` read-out tools call instead of parsing Turtle.
- [`repository-verbs.md`](repository-verbs.md),
  [`collection.md`](collection.md), [`region-repo.md`](region-repo.md),
  [`publishing.md`](publishing.md), [`receiving.md`](receiving.md) — a
  collection published as a repository: projected, published, and a
  contribution received back as patches.

The monorepo's build carries more: the verbs that weave a publication
of this corpus into our reader website and sweep this corpus's own
layout, and the chapters that implement them. They stay on our side of
a publication (§ "Two builds, two features"). What a published
repository's CI runs is the `x0k-tangle` built from that repository, or
the released one, over the literate documents that produced it — this
chapter's own `lib.rs` included.

## Two builds, two features

The monorepo build and the published build differ by two cargo
features, both on by default in the monorepo and both severed in a
projected repository, which therefore builds as the monorepo's
`--no-default-features` build does.

`corpus` is the larger. It compiles the two verbs that are ours alone —
weaving a publication into our reader website, and the whole-tree sweep
of this corpus's layout — and every module only they use. Its whole
surface in this chapter is two lines: one `include!` in the crate root
and one flattened variant in the CLI. Everything behind them is a
chapter of its own, held back from a publication together with the
chapters it names, so a published repository carries no source for
them. The verbs that publish a collection as a repository are not
behind it: they are in every build (§ "Publishing a collection").

`motifs` lives inside that machinery: it wires `x0k-surface-build`
into the HTML region weaver so `x0k:media` embeds are bundled as
canvas wasm. The surface-build crate is publish-excluded, so the
projector's manifest rewrite cuts the optional dependency out from
under the feature.

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
//! Literate programming for folio documents: tangle a document's
//! named code chunks into source files, weave it into HTML, and
//! reconcile edits made on either side.
//!
//! A literate document is a markdown page whose header — its first
//! fenced block, `turtle folio:document` — states a tangle target
//! (`folio:tangleCrate`, `folio:tangleRoot`) and whose fenced code blocks
//! carry chunk names
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
//!   chunk against its source file, and read every folio header
//!   against a vocabulary — one named with `--vocabulary`, one a
//!   projection recorded, or the set this build compiled — without
//!   writing anything.
//!
//! A fourth, **affordances** — [`faces::declared_affordances`] — reads
//! the affordance declarations out of a document as data, and
//! **declarations** — [`faces::declared_instances`] — every declared
//! instance of any class.
//!
//! Everything else in the crate builds outward from those: the
//! pipeline protocol that lets other generators ride the same
//! dispatcher, and the forge weave that renders a chapter for a code
//! host's Markdown renderer and reads it back.
```

<a name="chunk-modules"></a><sub>[`src/lib.rs`](../../crates/x0k-tangle/src/lib.rs) · `#modules`</sub>

```rust {#modules}
pub mod chunk;
pub mod chunk_refs;
pub mod cli;
pub mod faces;
pub mod identity_pipeline;
pub mod index;
pub mod multi_doc_resolve;
pub mod parser;
pub mod pipeline;
pub mod pipeline_runner;
pub mod region_gfm;
pub mod resolve;
pub mod source_ref;
pub mod stitch;
pub mod sync;
pub mod weave;
pub mod instance_rendering;

include!("repository_verbs.rs");

#[cfg(feature = "corpus")]
include!("corpus.rs");
```

The last two lines are chapters of their own. The first is the verbs
that publish a collection as a repository, with the modules they use;
the second is the corpus build's: the modules only its verbs use, and
those verbs. Each is declared where the file it names is tangled, and
included rather than declared module by module, so the names stay with
the chapter that owns them, and a build without the feature reads no
`corpus.rs` at all.

The re-exports are the names a consumer is expected to use without
knowing the module layout: the pipeline protocol and its runner. The
repository verbs and the corpus build re-export their own outward verbs
from their own files.

<a name="chunk-exports"></a><sub>[`src/lib.rs`](../../crates/x0k-tangle/src/lib.rs) · `#exports`</sub>

```rust {#exports}
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

The policy lives here rather than in the CLI because a gate is not only a
shell verb: anything that links the library and asks "is this tree sound?"
has to get the answer `check` gets. It was first put here when there were
two copies of the CLI — `main.rs` and the bundle's — and a duplicated
format string that drifts costs a reader a confusing line, while a
duplicated *resolution rule* that drifts costs them a gate that disagrees
with itself about whether the tree is sound.

Resolution alone was not enough, and the corpus proved it. Asking whether
a `from=` still resolves asks whether the mirror still POINTS somewhere;
it never asks whether the mirror still SHOWS what is there. Measured
2026-09-09, with resolution the only question being put: 100 of the 150
mirror bodies under `corpora/x0k/implementation` would have changed
under `sync`, across 12 of the 25 documents that carry one — all of them
green. The cause is mostly benign and therefore permanent: an author
trims the `///` doc comments out of a mirrored body so the prose around
it does not say everything twice — `author-literate-program`'s rule
against duplicating prose, colliding with a mirror that is verbatim by
construction. The consequence is not benign. The document shows a reader
code that is not the code, and the next `sync` replaces the trim without
asking.

So the extracted body is compared against the body the document shows,
and a difference is a finding like any other. The predicate is exactly
"would `sync` rewrite this file?" — line sequences, because that is what
`apply_from_patches` writes — which is what lets a gate run `check` and
believe the answer. `tools/mirror-drift` is that gate, with a baseline
holding the 100 mirrors that were already drifted when the comparison
landed.

<a name="chunk-source-check"></a><sub>[`src/lib.rs`](../../crates/x0k-tangle/src/lib.rs) · `#source-check`</sub>

```rust {#source-check}
/// Resolving every `from=` chunk in a document without writing anything.
///
/// The read-only half of [`crate::sync`]: it opens the same files, calls
/// the same extractor, and reports the same sentences, but it never
/// touches the document. That is what makes it usable as a gate — `sync`
/// rewrites the tree it judges, so no CI job can run it.
pub mod source_check {
    use crate::chunk::Chunk;
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

            let span = match extract_symbol_in(&source_content, symbol, lang) {
                Ok(span) => span,
                Err(e) => {
                    report.findings.push(format!("chunk '{name}': {e}"));
                    continue;
                }
            };

            if let Some(finding) = drift_finding(name, chunk, from_path, &span.body) {
                report.findings.push(finding);
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

    /// The body a document shows for a mirrored chunk, against the body
    /// its source holds now.
    ///
    /// This is the half of `sync` that `check` used to leave out, and it
    /// is the half that was wrong in the corpus: resolving a symbol
    /// proves the mirror still POINTS somewhere, never that it still
    /// SHOWS what is there. Measured 2026-09-09 — 100 of the 150 mirror
    /// bodies under `corpora/x0k/implementation` would change under
    /// `sync`, across 12 of the 25 documents that carry one, and every
    /// one of them passed `check` green. The dominant cause is benign and therefore
    /// permanent: an author trims `///` doc comments out of a mirrored
    /// body so the prose around it does not say everything twice, which
    /// is `author-literate-program`'s rule colliding with a verbatim
    /// mirror. Benign or not, the document is then showing a reader code
    /// that is not the code, and the next `sync` silently replaces it.
    ///
    /// The comparison is line sequences, not strings, because that is
    /// exactly the predicate "would `sync` rewrite this file?":
    /// `apply_from_patches` emits `new_body.lines()` between the fences,
    /// so trailing-newline differences are not drift and a single changed
    /// line is.
    ///
    /// An EMPTY body is not drift. That is an unfilled mirror — the shape
    /// an author writes before the first `sync`, and the one `sync` exists
    /// to fill. Reporting it here would make the ordinary first-fill a
    /// failure.
    ///
    /// The finding does not say WHICH side moved, because nothing here
    /// knows: two live artifacts are compared and no record of the last
    /// sync exists to arbitrate them. The sentence used to read "the
    /// mirrored body is not what <source> holds now … sync would rewrite
    /// it", which an adopter read as an accusation against their source
    /// file (2026-09-22) — and the measurement above says the document is
    /// the usual mover. So it names the disagreement, says the record is
    /// not there, and names what `sync` will do about it.
    fn drift_finding(
        name: &str,
        chunk: &Chunk,
        from_path: &Path,
        source_body: &str,
    ) -> Option<String> {
        let shown = chunk.bodies.first()?;
        if shown.text.trim().is_empty() {
            return None;
        }
        if shown.text.lines().eq(source_body.lines()) {
            return None;
        }
        // Name the first line that differs. A mirror is usually dozens of
        // lines and usually drifted in one place; "they differ" sends the
        // reader to a diff, "they differ at line 7" sends them to line 7.
        let at = shown
            .text
            .lines()
            .zip(source_body.lines())
            .position(|(a, b)| a != b)
            .map(|i| i + 1)
            .unwrap_or_else(|| shown.text.lines().count().min(source_body.lines().count()) + 1);
        Some(format!(
            "chunk '{name}': the mirrored body and {} disagree — \
             first difference at body line {at} (document {} line(s), source {} line(s)); \
             nothing records which side moved, and `sync` resolves it the one way it can, \
             by rewriting the document from the source",
            from_path.display(),
            shown.text.lines().count(),
            source_body.lines().count()
        ))
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

## Diagnostics

The crate warns, and until 2026-09-09 nobody heard it.
`tangle.output.unrecorded` is the only signal that a document is about
to write a file this tangler has never written — a first-time
graduation, or a hand-authored file whose path a chunk has quietly
started targeting — and `screen_outputs` emits it as a `warn!` before
the write lands. Neither binary installed a subscriber, so it went
nowhere: verified silent at the default level *and* under
`RUST_LOG=warn` while a run deleted a module header and a helper and
exited 0. A `warn!` with no subscriber is not a quiet warning, it is an
absent one, and the guard above it had been working into a closed pipe
for as long as the crate has had it.

The subscriber is installed here, next to `source_check` and for the
same reason: every binary that links the CLI gets it, and a diagnostics
policy that drifted between binaries would be a tool telling two authors
different things about the same tree. `cli::run` (below) calls
`init_diagnostics()` as its first statement, before it parses the
arguments, so a clap failure is the only path that can precede it.

Three choices in it carry weight. **stderr**, because stdout is
reserved for data — `index`, `weave` without an output path, `list` and
`affordances` all write parseable output there, and a diagnostic mixed
into it corrupts the caller's parse. **`warn` by default**, because the
CLI's ordinary reporting is its own summary and a default of `info`
would bury the two lines that matter under twenty-three that do not;
`RUST_LOG` raises it for anyone who wants the rest. **`try_init`**,
because a host that has already installed a subscriber — a test
harness, an embedding daemon — should keep it; a second subscriber is
not a reason to fail a tangle.

<a name="chunk-diagnostics"></a><sub>[`src/lib.rs`](../../crates/x0k-tangle/src/lib.rs) · `#diagnostics`</sub>

```rust {#diagnostics}
/// Install the process-wide tracing subscriber for an `x0k-tangle` binary.
///
/// [`cli::run`] calls this first. Diagnostics go to **stderr** (stdout carries
/// data), the default level is `warn` (the CLI's own report is its
/// summary), and `RUST_LOG` overrides. Idempotent: a host that already
/// installed a subscriber keeps it.
///
/// Incident (2026-09-09): before this existed, `tangle.output.unrecorded`
/// — the only warning that a document is about to overwrite a file the
/// tangler never wrote — was emitted into no subscriber and printed
/// nothing at any level. Incident test:
/// `tests/cli_verdicts.rs::tangle_warns_before_overwriting_a_file_it_never_wrote`.
pub fn init_diagnostics() {
    let _ = tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("warn")),
        )
        .with_writer(std::io::stderr)
        .without_time()
        .try_init();
}
```

## The CLI face

**What this package is for.** `x0k-tangle` is the tangler itself — the
library every other tangle consumer links, and the `x0k-tangle` binary
that puts it in a shell with the built-in `PipelineRegistry::default()`,
which carries the `identity-tangle` plugin and nothing else. It is the
package that publishes: crates.io, the published repository, and every
install instruction name this binary, and in a projected repository it
is the only tangler there is. Anything that needs a pipeline beyond
identity tangling is not this package's job; a host that registers more
plugins links the same CLI with its own registry and ships it under its
own binary name (the monorepo's is `x0k-tangle-bundle`,
[`bundle.md`](bundle.md)).

So the verbs live in the library, in `src/cli.rs`, and `src/main.rs` is
one call. What a binary decides when it links them is a `Host`: the name
and version `--help` and `--version` print, the registry `tangle`
dispatches through, and the two places the corpus build's whole-tree
sweep is allowed to be more lenient for one host than for another. Everything else — every
verb, every sentence it prints, every exit code — is one copy. There used
to be two: the bundle carried a 730-line second copy of this file, a fix
landed in one and not the other, and the two binaries shared the output
path `target/debug/x0k-tangle`, so which copy a caller ran depended on
which package cargo linked last. The 0.1.1 release projection failed with
"unrecognized subcommand" that way.

<a name="chunk-cli-host"></a><sub>[`src/cli.rs`](../../crates/x0k-tangle/src/cli.rs) · `#cli-host`</sub>

```rust {#cli-host file="src/cli.rs"}
/// What a binary decides when it links this CLI. Everything not named
/// here is the same for every binary.
pub struct Host {
    /// The name `--help` and `--version` print. Distinct per binary, so
    /// `--version` alone says which one ran.
    pub name: &'static str,
    /// The version `--version` prints after the name.
    pub version: &'static str,
    /// The one-line description at the top of `--help`.
    pub about: &'static str,
    /// The registry `tangle` dispatches through.
    pub registry: fn() -> PipelineRegistry,
    /// An environment variable the corpus build's whole-tree sweep falls
    /// back to before the current directory when it is not handed a root.
    /// A build without that sweep reads nothing from it.
    pub root_env: Option<&'static str>,
    /// Whether that sweep fails the run when its only errors are
    /// output-path collisions. A collision needs a person to pick the
    /// source of truth; a host whose build pipelines run the sweep may
    /// report it loudly and pass.
    pub collisions_fatal: bool,
}

impl Host {
    /// The protocol binary: this package's name and version, the built-in
    /// registry, the current directory, and every errored document fatal.
    pub const PROTOCOL: Host = Host {
        name: env!("CARGO_PKG_NAME"),
        version: env!("CARGO_PKG_VERSION"),
        about: "Literate programming tangler: documents to code, code quoted back into documents, and a collection published as a repository",
        registry: <PipelineRegistry as Default>::default,
        root_env: None,
        collisions_fatal: true,
    };
}
```

Every verb below runs wherever the binary is built: it needs the
documents it is pointed at and nothing else. The monorepo's build has
two more, which are ours alone — the reader website a publication is
woven into, and the whole-tree sweep of this corpus's own layout. They
are compiled only under the `corpus` feature (§ "The corpus build's
verbs").

Through 0.3.0 five verbs shipped, each saying in the first line of its
help that it needed our corpus, and refusing to run in a projected
repository. Every outside evaluator who read the repository flagged the
commands that refused. Three of them — projecting a publication as a
repository, publishing it, receiving a contribution back — now run on
anybody's collection and ship (§ "Publishing a collection"); the other
two and their chapters leave together: the feature gate takes the verbs
out of the published build, and the publication holds their chapters
back, so nothing in a projected repository describes a command it
lacks.

<a name="chunk-bin-doc"></a><sub>[`src/main.rs`](../../crates/x0k-tangle/src/main.rs) · `#bin-doc`</sub>

```rust {#bin-doc file="src/main.rs"}
//! The protocol-only `x0k-tangle` CLI: the crate's verbs in a shell,
//! with the built-in registry and no plugins. The binary a projected
//! repository ships and builds.
```

<a name="chunk-bin-main"></a><sub>[`src/main.rs`](../../crates/x0k-tangle/src/main.rs) · `#bin-main`</sub>

```rust {#bin-main file="src/main.rs"}
fn main() -> anyhow::Result<()> {
    x0k_tangle::cli::run(&x0k_tangle::cli::Host::PROTOCOL)
}
```

<a name="chunk-cli-doc"></a><sub>[`src/cli.rs`](../../crates/x0k-tangle/src/cli.rs) · `#cli-doc`</sub>

```rust {#cli-doc file="src/cli.rs"}
//! The tangler's command line as a library: every verb, its dispatch, and
//! the sentences it prints. A binary calls [`run`] with the [`Host`] it
//! is — the protocol `x0k-tangle` passes [`Host::PROTOCOL`]; a host that
//! registers more plugins passes its own registry and its own name.
```

<a name="chunk-cli-imports"></a><sub>[`src/cli.rs`](../../crates/x0k-tangle/src/cli.rs) · `#cli-imports`</sub>

```rust {#cli-imports file="src/cli.rs"}
use anyhow::{Context, Result};
use clap::{CommandFactory, FromArgMatches, Parser, Subcommand};
use std::collections::HashMap;
use std::path::{Path, PathBuf};

use crate::PipelineRegistry;
```

`--version` prints the host's name and the version cargo stamped into
its build. It is the one line a CI log needs to say which tangler ran,
and 0.1.1 shipped without it while `x0k-folio-cli --version` already
worked. Name, version and description come from the `Host` at run time
rather than from the derive, because the derive would stamp this
package's name into every binary that links it.

<a name="chunk-cli-struct"></a><sub>[`src/cli.rs`](../../crates/x0k-tangle/src/cli.rs) · `#cli-struct`</sub>

```rust {#cli-struct file="src/cli.rs"}
#[derive(Parser)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}
```

The subcommands are the literate verbs, and they operate on documents
in place: `tangle`, `check`, `affordances`, `declarations`, `icon`,
`sync`, `index`, `weave`, and `list`. The verbs that publish a
collection follow them (§ "Publishing a collection"), and the corpus
build adds its own last (§ "The corpus build's verbs"). Each variant's doc comment is
its `--help` text, so the clap derive below is also the user-facing
contract.

<a name="chunk-command-enum"></a><sub>[`src/cli.rs`](../../crates/x0k-tangle/src/cli.rs) · `#command-enum` · assembles [tangle-command](#chunk-tangle-command) · [check-command](#chunk-check-command) · [affordances-command](#chunk-affordances-command) · [declarations-command](#chunk-declarations-command) · [icon-command](#chunk-icon-command) · [sync-command](#chunk-sync-command) · [index-command](#chunk-index-command) · [weave-command](#chunk-weave-command) · [list-command](#chunk-list-command) · [repository-commands](#chunk-repository-commands) · [corpus-commands](#chunk-corpus-commands)</sub>

```rust {#command-enum file="src/cli.rs"}
#[derive(Subcommand)]
enum Command {
    <<tangle-command>>
    <<check-command>>
    <<affordances-command>>
    <<declarations-command>>
    <<icon-command>>
    <<sync-command>>
    <<index-command>>
    <<weave-command>>
    <<list-command>>
    <<repository-commands>>
    <<corpus-commands>>
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

```turtle folio:graph
signifier:x0k-tangle-tangle a x0k:Signifier ;
    x0k:signifies affordance:tangle_source_from_a_document ;
    x0k:presentedOn surface:cli .
```

<a name="chunk-tangle-command"></a><sub>[`src/cli.rs`](../../crates/x0k-tangle/src/cli.rs) · `#tangle-command`</sub>

```rust {#tangle-command file="src/cli.rs"}
/// Tangle .md documents to their source files (writes .tangle-map.json sidecars)
Tangle {
    /// Paths to scan for documents whose header states a tangle target
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
folio header under the paths against a vocabulary. The second half
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

```turtle folio:graph
signifier:x0k-tangle-check a x0k:Signifier ;
    x0k:signifies affordance:check_a_document_against_shipped_vocabulary ;
    x0k:presentedOn surface:cli .
```

<a name="chunk-check-command"></a><sub>[`src/cli.rs`](../../crates/x0k-tangle/src/cli.rs) · `#check-command`</sub>

```rust {#check-command file="src/cli.rs"}
/// Verify chunk references resolve and no cycles exist, and read every
/// folio header against a vocabulary.
///
/// Two things can go wrong with a header, and they are reported
/// apart. A defect — a malformed id or edge target, a predicate no
/// module of the vocabulary declares, a header that does not parse —
/// is a gap in what this publication selected, and fails the check. An
/// edge whose target names no document under the paths scanned simply
/// leaves the set — often into a wider corpus this selection was drawn
/// from, and expected either way: printed as a note, never a failure.
/// `--closed` is the reader saying there is no wider corpus: under
/// it, an edge that leaves the set is a defect like any other.
/// A Markdown file with no folio header at all — or an untyped `<>`
/// one, which names nothing — is skipped, and counted on the summary
/// line so that skipping it is not silent; `--require-header` is the
/// reader saying every file here is supposed to be typed, under which
/// such a file is a defect too.
/// A third thing is checked across the set: an
/// affordance claimed for a human that no signifier signifies is a
/// defect, because the audience has nothing to perceive — unless the
/// set declares no signifier anywhere, in which case it is a note,
/// because signifiers live beside the faces that present them and a
/// set holding none is not the set that could answer.
///
/// Every `from=` chunk is resolved against its source file too —
/// missing file, missing symbol, ambiguous symbol, a language symbol
/// extraction cannot walk — and the body it resolves to is compared
/// against the body the document shows, so a mirror that still points
/// somewhere but no longer shows what is there fails as well. Each
/// failure fails the check. Nothing is written: this is the read-only
/// half of `sync`.
Check {
    /// Paths to scan
    paths: Vec<PathBuf>,
    /// Workspace root that `from=` paths resolve against
    /// (defaults to current directory)
    #[arg(long)]
    workspace: Option<PathBuf>,
    /// Directory of ontology module files (*.ttl) to check against,
    /// read in addition to the set this build compiled. Defaults to the
    /// modules this projection's PROVENANCE.json names, then to the set
    /// this build compiled.
    #[arg(long)]
    vocabulary: Option<PathBuf>,
    /// Read --vocabulary alone, without the compiled set. Say this only
    /// when that directory holds every term the documents use, the
    /// header's own included.
    #[arg(long, requires = "vocabulary")]
    only_vocabulary: bool,
    /// Fail on an edge whose target names no document under the paths
    /// scanned. Say this when the collection is a closed set: every
    /// document an edge can name is here, so a target that resolves to
    /// nothing is a broken link rather than a boundary.
    #[arg(long)]
    closed: bool,
    /// Fail on a Markdown file under the paths scanned that carries no
    /// folio header. Say this when every file in the set is supposed to
    /// be typed: a file without one is skipped in silence, which is what
    /// lets a corpus adopt one directory at a time and what leaves a
    /// generated board quietly one row short.
    #[arg(long)]
    require_header: bool,
},
```

### `x0k-tangle affordances`

Print every affordance the documents under the paths declare, as a
JSON array on stdout: one record per affordance a graph block declares, with
its id, title, description, the document it is defined in, and its
declared facts grouped by predicate. What a declaration says becomes
data a reader's own tooling can consume, which is the affordance of
[reading an affordance out of a document](../../decisions/design/corpus/publish-a-region-as-a-repository/declare-concepts-and-instances.md "x0k:affordance/read_declared_affordances") rather than trusting the
document's summary of itself. A block the extractor refuses is
reported on stderr and skipped.

<a name="folio-instance-68747470733a2f2f306b2e636f6d70757465722f6f6e746f6c6f6779237369676e69666965722f78306b2d74616e676c652d6166666f7264616e636573-3"></a><sub data-instance-iri="https://0k.computer/ontology#signifier/x0k-tangle-affordances" data-concept-iri="https://0k.computer/ontology#Signifier" data-source-document="corpora/x0k/implementation/tangle/crate.md"><strong>Signifier</strong> · x0k-tangle affordances · <code>https://0k.computer/ontology#signifier/x0k-tangle-affordances</code> · <a href="#folio-source-68747470733a2f2f306b2e636f6d70757465722f6f6e746f6c6f6779237369676e69666965722f78306b2d74616e676c652d6166666f7264616e636573-3">source declaration</a></sub><a name="folio-source-68747470733a2f2f306b2e636f6d70757465722f6f6e746f6c6f6779237369676e69666965722f78306b2d74616e676c652d6166666f7264616e636573-3"></a>

```turtle folio:graph
signifier:x0k-tangle-affordances a x0k:Signifier ;
    x0k:signifies affordance:read_declared_affordances ;
    x0k:presentedOn surface:cli .
```

<a name="chunk-affordances-command"></a><sub>[`src/cli.rs`](../../crates/x0k-tangle/src/cli.rs) · `#affordances-command`</sub>

```rust {#affordances-command file="src/cli.rs"}
/// Print every affordance the folio documents under the paths
/// declare, as a JSON array on stdout.
///
/// One record per `turtle folio:graph` block stating `a x0k:Affordance`:
/// `id`, `title` (the enclosing heading), `description` (the prose under
/// it), `defined_in` (the parent document's id), and `facts` — every other
/// declared fact grouped by compact predicate (`x0k:claimedFor`), each
/// value tagged `{"entity": …}` for an id or `{"string": …}` for a
/// literal. A block the extractor refuses is reported on stderr and
/// skipped.
Affordances {
    /// Paths to scan for folio documents
    paths: Vec<PathBuf>,
},
```

### `x0k-tangle declarations`

Every instance a graph block under the paths declares, of any class, as
one JSON array on stdout — the read-out a tool in another language calls
instead of parsing Turtle ([`cli-faces.md`](cli-faces.md) § Every
declaration, as data). It has no signifier: it is plumbing for tools,
and makes no claim on a person.

<a name="chunk-declarations-command"></a><sub>[`src/cli.rs`](../../crates/x0k-tangle/src/cli.rs) · `#declarations-command`</sub>

```rust {#declarations-command file="src/cli.rs"}
/// Print every instance the graph blocks under the paths declare, as a
/// JSON array on stdout.
///
/// One record per `turtle folio:graph` instance block, sorted by document
/// and line: `id` (compact), `class` (kebab-case), `title`,
/// `description`, `document` (the path), and `statements` — compact
/// predicate → values, an IRI as its compact id, a literal as the JSON
/// its datatype names (`rdf:JSON` parsed). A block the extractor refuses
/// is reported on stderr and skipped.
Declarations {
    /// Only instances of this class (kebab-case, e.g. `interface`);
    /// repeatable. Every class when absent.
    #[arg(long = "class")]
    classes: Vec<String>,
    /// Paths to scan for Markdown documents
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

```turtle folio:graph
signifier:x0k-tangle-icon a x0k:Signifier ;
    x0k:cue "x0k-tangle icon" ;
    x0k:signifies affordance:check_an_icon_against_the_profile,
        affordance:show_an_icon_on_a_surface ;
    x0k:presentedOn surface:cli .
```

<a name="chunk-icon-command"></a><sub>[`src/cli.rs`](../../crates/x0k-tangle/src/cli.rs) · `#icon-command`</sub>

```rust {#icon-command file="src/cli.rs"}
/// Check every `svg x0k:icon` declaration in the folio documents
/// under the paths against the icon profile, and with `--out` write
/// each as its light and dark files bound to a publication's palette.
///
/// A drawing outside the profile is printed with the rule it broke and
/// the element, and fails the run; nothing is redrawn. `--out` needs
/// `--palette`: the publication document whose header carries the
/// `x0k:palette` the four paint roles are bound with.
Icon {
    /// Paths to scan for folio documents
    paths: Vec<PathBuf>,
    /// Directory to write `<stem>-light.svg` and `<stem>-dark.svg` into
    #[arg(long, requires = "palette")]
    out: Option<PathBuf>,
    /// The publication document whose `x0k:palette` binds the roles
    #[arg(long, requires = "out")]
    palette: Option<PathBuf>,
},
```

### `x0k-tangle sync`

<a name="chunk-sync-command"></a><sub>[`src/cli.rs`](../../crates/x0k-tangle/src/cli.rs) · `#sync-command`</sub>

```rust {#sync-command file="src/cli.rs"}
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

<a name="chunk-index-command"></a><sub>[`src/cli.rs`](../../crates/x0k-tangle/src/cli.rs) · `#index-command`</sub>

```rust {#index-command file="src/cli.rs"}
/// Build a JSON index of every document whose folio header names it
Index {
    /// Paths to scan for folio documents
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

```turtle folio:graph
signifier:x0k-tangle-weave a x0k:Signifier ;
    x0k:signifies affordance:weave_a_document ;
    x0k:presentedOn surface:cli .
```

<a name="chunk-weave-command"></a><sub>[`src/cli.rs`](../../crates/x0k-tangle/src/cli.rs) · `#weave-command`</sub>

```rust {#weave-command file="src/cli.rs"}
/// Weave a literate document into HTML
Weave {
    /// Path to a literate document
    path: PathBuf,
    /// Output directory (defaults to stdout if not set)
    #[arg(long)]
    output_dir: Option<PathBuf>,
},
```

### `x0k-tangle list`

<a name="chunk-list-command"></a><sub>[`src/cli.rs`](../../crates/x0k-tangle/src/cli.rs) · `#list-command`</sub>

```rust {#list-command file="src/cli.rs"}
/// List chunks and their targets in a document
List {
    /// Path to a literate document
    path: PathBuf,
},
```

### Publishing a collection

Three verbs take a collection of documents to a public repository and
back: `project-repo` writes a publication as a standalone repository,
`publish-repo` proves, rehearses and pushes it, and `receive-repo` reads
a contributor's clone back into patches. They are one variant here, and
their variants, dispatch and the modules they use are a chapter of
their own ([`repository-verbs.md`](repository-verbs.md)). The variant is
flattened, so they sit in `--help` beside the verbs above, in every
build.

<a name="chunk-repository-commands"></a><sub>[`src/cli.rs`](../../crates/x0k-tangle/src/cli.rs) · `#repository-commands`</sub>

```rust {#repository-commands file="src/cli.rs"}
#[command(flatten)]
Repository(crate::RepositoryCommand),
```

### The corpus build's verbs

The monorepo's build has two more verbs, and they are one variant here.
`weave-region` weaves a publication into our reader website — the
canvas shell, the atlas, the motif embeds — and `workspace` sweeps the
literate roots of this corpus's layout. Neither has a use outside our
corpus, so they are compiled only under the `corpus` feature, which the
monorepo turns on by default and a publication severs. Their variants,
their dispatch and the modules only they use are a chapter of their own
that stays in our corpus, the way `motifs` keeps the motif system out
of a published build. The variant below is flattened, so in the corpus
build those verbs sit in `--help` beside the ones above, and in the
published build the variant, the verbs and their source do not exist.

<a name="chunk-corpus-commands"></a><sub>[`src/cli.rs`](../../crates/x0k-tangle/src/cli.rs) · `#corpus-commands`</sub>

```rust {#corpus-commands file="src/cli.rs"}
#[cfg(feature = "corpus")]
#[command(flatten)]
Corpus(crate::CorpusCommand),
```

## Dispatch

`run` is one `match` over the command; every arm resolves its
workspace root, calls the library, and prints a report to stderr; stdout
is reserved for data (`index` and `weave` without an output path,
`list`, and `affordances`). Exit codes carry the verdicts: `check` exits
non-zero on any error, `sync` when a chunk it was asked to fill stayed
empty. The repository verbs and the corpus verbs dispatch in their own
chapters and keep their own exit codes.

The host's name, version and description are stamped onto the parsed
command before the arguments are read — a help text is part of the
contract, and a sentence true of one binary must not print in another.
The corpus build gets one more say over its help lines first, for the
same reason.

<a name="chunk-main-fn"></a><sub>[`src/cli.rs`](../../crates/x0k-tangle/src/cli.rs) · `#main-fn` · assembles [dispatch-tangle](#chunk-dispatch-tangle) · [dispatch-sync](#chunk-dispatch-sync) · [dispatch-check](#chunk-dispatch-check) · [dispatch-affordances](#chunk-dispatch-affordances) · [dispatch-declarations](#chunk-dispatch-declarations) · [dispatch-icon](#chunk-dispatch-icon) · [dispatch-index](#chunk-dispatch-index) · [dispatch-weave](#chunk-dispatch-weave) · [dispatch-list](#chunk-dispatch-list)</sub>

```rust {#main-fn file="src/cli.rs"}
/// Parse the process arguments and run the verb they name, as `host`.
pub fn run(host: &Host) -> Result<()> {
    crate::init_diagnostics();
    let command = Cli::command()
        .name(host.name)
        .bin_name(host.name)
        .version(host.version)
        .about(host.about);
    #[cfg(feature = "corpus")]
    let command = crate::corpus_help(command, host);
    let cli = Cli::from_arg_matches(&command.get_matches()).unwrap_or_else(|e| e.exit());

    match cli.command {
        <<dispatch-tangle>>

        <<dispatch-sync>>

        <<dispatch-check>>

        <<dispatch-affordances>>
        <<dispatch-declarations>>

        <<dispatch-icon>>

        <<dispatch-index>>

        <<dispatch-weave>>

        <<dispatch-list>>

        Command::Repository(verb) => crate::run_repository(verb)?,

        #[cfg(feature = "corpus")]
        Command::Corpus(verb) => crate::run_corpus(verb, host)?,
    }

    Ok(())
}
```

`tangle` routes through the unified dispatcher
([`dispatcher.md`](dispatcher.md)) rather than the identity plugin
directly, so a document that declares a pipeline the host's registry
does not carry errors loudly instead of tangling half of itself.

Naming a document is an imperative — *write this one out* — so a named
document that names nowhere to write is a failed run, not a quiet zero.
`tangle_document` answers such a document with an empty result, which is
the right answer for a library and the wrong report for a shell: the
summary line used to say `tangled 0 file(s) from 1 document(s)` and exit
0 over a document with eight chunks and a root, and the only way to
notice was to go looking for a file that was never written. So the verb
reads what the document declares before it asks for the work, and says
which of the two things is missing.

Two shapes reach that arm and only one is a mistake. A document with
chunks and no tangle target in its header is usually an author who
forgot to state one,
and the sentence above is for them. A document whose chunks are *all*
`from=` mirrors owns no code at all: it shows what other files hold, and
the integration guide recommends it as the first thing an existing
codebase writes. Naming nowhere to write is that document's shape, so
the run says what it saw and passes. The old arm could not tell them
apart and failed both — which meant the explicit-file form refused
exactly the document the guide had just told the reader to write, while
the directory form skipped it and exited 0. The two forms now agree,
and the predicate that separates them is a property of the document
rather than of how it was reached.

<a name="chunk-dispatch-tangle"></a><sub>[`src/cli.rs`](../../crates/x0k-tangle/src/cli.rs) · `#dispatch-tangle`</sub>

```rust {#dispatch-tangle file="src/cli.rs"}
Command::Tangle { paths, workspace, force } => {
    // Route every document through the unified dispatcher with the
    // host's registry: a document declaring a pipeline that registry
    // does not carry errors here rather than tangling half of itself.
    let ws = workspace.unwrap_or_else(|| std::env::current_dir().unwrap());
    let docs = discover_documents(&paths)?;
    let registry = (host.registry)();
    let settings = clobber_settings(force);
    let mut total_files = 0;
    let mut tangled_docs = 0;
    let mut nowhere_to_write = 0;

    for doc_path in &docs {
        let declared = declares(doc_path);
        if !declared.target {
            if declared.mirrors_only {
                eprintln!("  {}", mirror_only(doc_path, declared.chunks));
            } else {
                eprintln!("  {}", nothing_to_write(doc_path, declared.chunks));
                nowhere_to_write += 1;
            }
            continue;
        }
        tangled_docs += 1;
        let result = crate::tangle_document_with(doc_path, &ws, &registry, &settings)?;
        // Both halves of the arrow are written the way the reader named
        // them. The tangler resolves an output against the workspace
        // root and holds it absolute, so this line used to pair
        // `docs/ratelimit.md` with `/tmp/rl/crates/…` — one path a
        // reader can act on and one they cannot (jj, 2026-09-23).
        for out in &result.identity_outputs {
            eprintln!("  {} → {}", doc_path.display(), under(&out.path, &ws));
            total_files += 1;
        }
        for out in &result.pipeline_outputs {
            if out.kind == crate::IDENTITY_KIND {
                // Already reported via identity_outputs.
                continue;
            }
            eprintln!("  {} → {}", doc_path.display(), under(&out.path, &ws));
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

<a name="chunk-dispatch-sync"></a><sub>[`src/cli.rs`](../../crates/x0k-tangle/src/cli.rs) · `#dispatch-sync`</sub>

```rust {#dispatch-sync file="src/cli.rs"}
Command::Sync { paths, workspace } => {
    let ws = workspace.unwrap_or_else(|| std::env::current_dir().unwrap());
    let docs = discover_documents_any(&paths)?;
    let mut total_populated = 0;
    let mut unfilled = 0;

    for doc_path in &docs {
        let result = crate::sync::sync_document(doc_path, &ws)?;

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
reads every folio header under the same paths against the shipped
vocabulary *extended by the one the set carries*
([`cli-faces.md`](cli-faces.md)) and prints what it found
in the affordance's own two categories: a defect as `<path>: <defect>`,
which fails the run, and a dangling edge as a `note:` that names the
target and the set it is missing from. The typed instances declared
inside those documents are read in the same two categories, which is
why the same loop prints a second kind of note: `0 declaration(s)
checked` over a collection that declares two papers was the gate
reporting a pass it had not run. The third rides the first walk:
it holds the id every header declared and fails the run when two
documents declare the same one.

The three run in one pass over one set, which is the repair. They used
to run over two: the header half asked
[`cli-faces.md`](cli-faces.md)'s discovery, which parses each file, and
the reference half asked `discover_documents`, which grepped for the
tangle key. Two membership predicates over one argument is two
answers to "what did you check", and only the second one got counted.

A document id is the graph's primary key — every edge in every header
resolves through it, and an `x0k:cites` naming a doubled id names both
documents or neither. Two documents holding one id is therefore a
defect of the set rather than of either file, so the check is over the
paths scanned (there is nothing else this process can see) and fails the
run rather than noting it. The message names both files, because the
answer is always to change one of them and the reader needs to know
which two are in play.

<a name="chunk-dispatch-check"></a><sub>[`src/cli.rs`](../../crates/x0k-tangle/src/cli.rs) · `#dispatch-check`</sub>

```rust {#dispatch-check file="src/cli.rs"}
Command::Check {
    paths,
    workspace,
    vocabulary,
    only_vocabulary,
    closed,
    require_header,
} => {
    let ws = workspace.unwrap_or_else(|| std::env::current_dir().unwrap());
    let model = crate::faces::vocabulary(vocabulary.as_deref(), only_vocabulary)?;
    let mut has_errors = false;
    let mut chunked_documents = 0;
    let mut splice_failed = 0;
    let mut source_refs = 0;
    let mut source_ref_failures = 0;
    let mut header_less = Vec::new();
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
        let carries_header = crate::faces::carries_header(&content);
        let parsed = match crate::parser::parse_document(&content) {
            Ok(parsed) => parsed,
            Err(e) => {
                // The only thing that stops a document parsing is its
                // header, and a header that does not read is the vocabulary
                // pass's to report — once, with the parser's reason.
                if !carries_header {
                    eprintln!("{}: does not parse: {e}", doc_path.display());
                    has_errors = true;
                }
                continue;
            }
        };

        // A file the vocabulary pass will never see, because it carries
        // no header that names it. Skipping it is what makes adoption
        // incremental; holding on to its name is what keeps skipping it
        // from being silent.
        if !carries_header {
            header_less.push(doc_path.clone());
        } else if let Some(names) = crate::index::title_disagreement(&content) {
            // Two names, and neither is wrong: a warning, never the verdict.
            eprintln!("{}", title_warning(&doc_path, &names));
        }

        if !parsed.chunks.is_empty() {
            chunked_documents += 1;
            let splices = crate::resolve::check_all_refs(&parsed)?;
            if !splices.is_empty() {
                splice_failed += 1;
            }
            for err in &splices {
                eprintln!("{}: {}", doc_path.display(), err);
                has_errors = true;
            }
            let sources = crate::source_check::check_source_refs(&parsed, &ws);
            source_refs += sources.checked;
            source_ref_failures += sources.findings.len();
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

    let report = crate::faces::check_vocabulary(&model, &paths)?;
    for (path, reason) in &report.unparsed {
        eprintln!("{path}: header does not parse: {reason}");
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
    for note in &report.declarations.notes {
        let standing = if closed { "" } else { "note: " };
        eprintln!("{standing}{note}");
        has_errors |= closed;
    }
    for edge in &report.corpus.dangling {
        eprintln!(
            "{}",
            dangling_finding(closed, &edge.source, &edge.predicate, &edge.target)
        );
        has_errors |= closed;
    }
    for edge in &report.declarations.dangling {
        eprintln!(
            "{}",
            dangling_declaration_finding(closed, &edge.source, &edge.predicate, &edge.target)
        );
        has_errors |= closed;
    }

    if require_header {
        for path in &header_less {
            eprintln!(
                "{}: carries no folio header, so nothing in this set reads it",
                path.display()
            );
        }
        has_errors |= !header_less.is_empty();
    }

    // The counts are the denominator, and a reader wants them most
    // when something failed: one dangling edge reads differently over
    // fifteen headers than over seven hundred. So the line prints
    // either way, and the exit code carries the verdict.
    eprintln!(
        "{}; {} header(s) read against the vocabulary, {} declaration(s) checked, {} edge(s) leave the set{}",
        references_verdict(chunked_documents, splice_failed, source_refs, source_ref_failures),
        report.corpus.checked,
        report.declarations.checked,
        report.corpus.dangling.len() + report.declarations.dangling.len(),
        untyped_clause(header_less.len())
    );
    if has_errors {
        std::process::exit(1);
    }
}
```

The line says what the run did, and the counts are what make that
possible to read. It prints on a failing run too, which it did not
always: a maintainer running `--closed` over a fifteen-ADR log lost
`15 header(s) read` at the moment the denominator was worth most,
because one dangling edge over fifteen documents and one over seven
hundred are different situations and only the count tells them apart.
The exit code carries the verdict; the line carries the arithmetic.
`all references OK` used to print over a set whose
references had never been read — the same six words for a corpus of
forty chapters and for a directory the walk had dropped every document
out of. A verdict that asserts the work it skipped is worse than no
verdict at all: it is the gate reporting a pass it did not run, and a
reader has no way to tell the two apart.

The line names its two kinds separately because they are two claims,
and the same sentence has now overstated four separate times. A
`<<splice>>` resolves inside the document; a `from=` resolves against a
file on disk. A run can read forty documents of splices and open no
source file at all, and a line that folded both into "all references
OK" would say the same words either way.

The fourth time was printing on a failing run without rewriting the
sentence for it. `7 from= source references resolve` under a defect
naming the one that did not is the reader's last line contradicting the
exit code — "the one thing I want before I let the job block a merge,
and it's a sentence" (jj, 2026-09-23). Each kind is therefore reported
as resolved-of-read the moment any of them did not resolve, and as a
plain count when they all did, because `7 of 7` is arithmetic nobody
asked for.

And the line says what it walked past. A Markdown file with no folio
header is skipped — that is what lets a corpus adopt this one directory
at a time — and so is a file whose header is an untyped `<>` one, which
configures a tool and names no document. But a maintainer generating an
ADR board from the fifteen headers in a directory of sixteen files gets
a board that is silently one row short, and every count on this line
agrees with the board rather than with the directory. `--require-header`
is the reader saying the set is
meant to be wholly typed; the count is there either way, because the
reader who most needs it is the one who did not know to ask.

<a name="chunk-references-verdict"></a><sub>[`src/cli.rs`](../../crates/x0k-tangle/src/cli.rs) · `#references-verdict`</sub>

```rust {#references-verdict file="src/cli.rs"}
/// What `check` says about the reference half of a run.
///
/// The counts are load-bearing, and they are separate because they are
/// separate claims. Zero is a real and common answer for either — a
/// directory of decision documents declares no chunks, and most
/// documents that do declare chunks name no source file — and each has
/// to read as zero rather than as a pass, because the shape that
/// produces it is also the shape a broken walk produces.
///
/// Each kind is reported as resolved-of-read once any of them did not
/// resolve, because this line prints on a failing run: over one broken
/// mirror in seven, `7 from= source references resolve` is the last
/// sentence a reader meets and it says the opposite of the verdict.
fn references_verdict(
    chunked_documents: usize,
    splice_failed: usize,
    source_refs: usize,
    source_ref_failures: usize,
) -> String {
    if chunked_documents == 0 {
        return "no chunk references to check".to_string();
    }
    let docs = match chunked_documents {
        1 => "1 document with chunks".to_string(),
        n => format!("{n} documents with chunks"),
    };
    let splices = match splice_failed {
        0 => format!("splice references resolve in {docs}"),
        failed => format!(
            "splice references resolve in {} of {docs}",
            chunked_documents - failed
        ),
    };
    let sources = match (source_refs, source_ref_failures) {
        (0, _) => "no from= source references declared".to_string(),
        (1, 0) => "1 from= source reference resolves".to_string(),
        (n, 0) => format!("{n} from= source references resolve"),
        (n, failed) => format!("{} of {n} from= source references resolve", n - failed),
    };
    format!("{splices}, {sources}")
}

/// What `check` says about the Markdown it walked past.
///
/// A file with no folio header is not a defect — ignoring Markdown it
/// does not own is why a corpus can adopt this verb one directory at a
/// time — but it is invisible to every other count on the line, and a
/// board generated from those counts is quietly one row short. Saying how
/// many were skipped costs a clause and is the only way a reader learns
/// the set is not the set they think it is. Saying it when there were
/// none would be noise, so the clause is empty then.
fn untyped_clause(header_less: usize) -> String {
    match header_less {
        0 => String::new(),
        1 => ", 1 markdown file carried no header".to_string(),
        n => format!(", {n} markdown files carried no header"),
    }
}
```

`affordances` is the one literate verb whose whole product is data, so
its records go to stdout as pretty JSON and only the extractor's
refusals go to stderr.

<a name="chunk-dispatch-affordances"></a><sub>[`src/cli.rs`](../../crates/x0k-tangle/src/cli.rs) · `#dispatch-affordances`</sub>

```rust {#dispatch-affordances file="src/cli.rs"}
Command::Affordances { paths } => {
    let report = crate::faces::declared_affordances(&paths)?;
    for (path, reason) in &report.skipped {
        eprintln!("{path}: skipped: {reason}");
    }
    println!("{}", serde_json::to_string_pretty(&report.records)?);
}
```

`declarations` has the same shape, over every class: the records on
stdout, the refused blocks on stderr, each named by path and line. Its
vocabulary is the one `check` would read against, so a projection's own
modules name the prefixes its blocks use.

<a name="chunk-dispatch-declarations"></a><sub>[`src/cli.rs`](../../crates/x0k-tangle/src/cli.rs) · `#dispatch-declarations`</sub>

```rust {#dispatch-declarations file="src/cli.rs"}
Command::Declarations { classes, paths } => {
    let model = crate::faces::vocabulary(None, false)?;
    let report = crate::faces::declared_instances(&model, &paths, &classes)?;
    for (place, reason) in &report.skipped {
        eprintln!("{place}: skipped: {reason}");
    }
    println!("{}", serde_json::to_string_pretty(&report.records)?);
}
```


`icon` prints each refusal under where it was declared, one rule per
line as the checker names them, then one summary line; a refusal fails
the run. Writing is the second half of the same verb rather than a verb
of its own because a file is only ever written from an accepted
drawing.

<a name="chunk-dispatch-icon"></a><sub>[`src/cli.rs`](../../crates/x0k-tangle/src/cli.rs) · `#dispatch-icon`</sub>

```rust {#dispatch-icon file="src/cli.rs"}
Command::Icon { paths, out, palette } => {
    let report = crate::faces::declared_icons(&paths)?;
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
        let palette = crate::faces::header_palette(&content)?.ok_or_else(|| {
            anyhow::anyhow!("{} carries no `x0k:palette` in its header", palette.display())
        })?;
        written = crate::faces::write_icon_files(&report, &palette, &out)?.len();
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

<a name="chunk-dispatch-index"></a><sub>[`src/cli.rs`](../../crates/x0k-tangle/src/cli.rs) · `#dispatch-index`</sub>

```rust {#dispatch-index file="src/cli.rs"}
Command::Index {
    paths,
    workspace,
    output,
} => {
    let ws = workspace.unwrap_or_else(|| std::env::current_dir().unwrap());
    let index = crate::index::build_index(&paths, &ws)?;
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

<a name="chunk-dispatch-weave"></a><sub>[`src/cli.rs`](../../crates/x0k-tangle/src/cli.rs) · `#dispatch-weave`</sub>

```rust {#dispatch-weave file="src/cli.rs"}
Command::Weave { path, output_dir } => {
    let content = std::fs::read_to_string(&path)?;
    let parsed = crate::parser::parse_document(&content)?;
    let output = crate::weave::weave_html(&content, &parsed)?;

    if let Some(dir) = output_dir {
        std::fs::create_dir_all(&dir)?;
        // Named after the document, so weaving a second chapter into one
        // directory no longer destroys the first (weave.md § the page's name).
        let html_path = dir.join(crate::weave::page_file_name(&path));
        std::fs::write(&html_path, &output.html)?;
        eprintln!("wove {} → {}", path.display(), html_path.display());
    } else {
        print!("{}", output.html);
    }
}
```

<a name="chunk-dispatch-list"></a><sub>[`src/cli.rs`](../../crates/x0k-tangle/src/cli.rs) · `#dispatch-list`</sub>

```rust {#dispatch-list file="src/cli.rs"}
Command::List { path } => {
    let content = std::fs::read_to_string(&path)?;
    let parsed = crate::parser::parse_document(&content)?;

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

Document discovery is a
content sniff — a `.md` mentioning `folio:tangle` (or, for `sync`, `from=`) —
because the parse that would confirm it is what the verb is about to do
anyway. The rest is report formatting.

`tangle` takes `--force`, and so does the corpus build's sweep, and both
mean the same thing by it, so the translation from flag to policy lives
in one place, visible to the crate. The default is the
absence of the flag rather than a configured value: a run that did not
say "overwrite" gets the guard.

<a name="chunk-clobber-settings"></a><sub>[`src/cli.rs`](../../crates/x0k-tangle/src/cli.rs) · `#clobber-settings`</sub>

```rust {#clobber-settings file="src/cli.rs"}
/// The run-scoped settings a `--force` flag decides.
pub(crate) fn clobber_settings(force: bool) -> crate::TangleSettings {
    crate::TangleSettings {
        clobber: if force {
            crate::ClobberPolicy::Force
        } else {
            crate::ClobberPolicy::Refuse
        },
        ..Default::default()
    }
}
```

What `check` says about an edge whose target is not in the set it read
lives in one place, because it is a claim about the reader's tree and
the tree is the one thing this process cannot see past.

Which is why the reader gets to settle it. A collection whose edges can
only name documents inside it — an ADR directory, a docs tree that is
the whole world it links into — has no boundary for an edge to cross, so
every edge that leaves is a rename nobody finished. `--closed` is that
sentence said once on the command line, and it moves the finding from a
note to a defect without changing what the finding observed. The default
stays the other way because this corpus is the opposite case: its edges
leave for a private corpus all day, and a gate that failed on them would
be a gate nobody ran.

<a name="chunk-dangling-finding"></a><sub>[`src/cli.rs`](../../crates/x0k-tangle/src/cli.rs) · `#dangling-finding`</sub>

```rust {#dangling-finding file="src/cli.rs"}
/// What `check` says about an edge that leaves the set.
///
/// The set is whatever the paths on the command line contain, and nothing
/// beyond it is knowable from here: not whether a wider corpus exists, not
/// whether this tree was projected out of one. So the line names the
/// situation and stops. A line that instead told the reader their edge
/// pointed into "the corpus this was projected from" would be true of one
/// repository and read as a misconfiguration to everyone else.
///
/// `closed` is the reader answering the one question the process cannot:
/// whether anything exists outside these paths. It changes the standing of
/// the finding and not a word of what it observed — the same sentence,
/// with `note:` dropped, because under `--closed` there is nothing left
/// for the reader to go and look at.
fn dangling_finding(
    closed: bool,
    source: &str,
    predicate: &str,
    target: impl std::fmt::Display,
) -> String {
    let standing = if closed { "" } else { "note: " };
    format!("{source}: {standing}edge `{predicate}` → `{target}` names no document under the paths scanned")
}

/// The same finding one level down: a declared instance's edge target that
/// names no declaration in the set. Separate wording because a
/// declaration lives inside a document, so "names no document" would
/// send its reader looking for the wrong thing.
fn dangling_declaration_finding(
    closed: bool,
    source: &str,
    predicate: &str,
    target: impl std::fmt::Display,
) -> String {
    let standing = if closed { "" } else { "note: " };
    format!("{source}: {standing}declared edge `{predicate}` → `{target}` names no declaration under the paths scanned")
}
```

A third kind of finding is neither a defect nor a note. A document whose
host frontmatter `title:` and first `# ` heading are two different names
([`doc-index.md`](doc-index.md) § When the two names disagree — the rule
and its normalisation live with the resolver that picks between them) is
told so as a `warning:`. `--closed` does not touch it and it never sets
`has_errors`: the heading is presentation, so a difference is a choice the
author may have made on purpose, and a gate that failed on it would be
enforcing a house style. The line names both strings, and which one
`index` took, so the reader can decide from the line alone.

<a name="chunk-title-warning"></a><sub>[`src/cli.rs`](../../crates/x0k-tangle/src/cli.rs) · `#title-warning`</sub>

```rust {#title-warning file="src/cli.rs"}
/// What `check` says about a document that names itself twice. Never part
/// of the verdict.
fn title_warning(path: &Path, names: &crate::index::TitleDisagreement) -> String {
    format!(
        "{}: warning: frontmatter title `{}` and first heading `{}` disagree; `index` takes the frontmatter title",
        path.display(),
        names.frontmatter,
        names.heading
    )
}
```

## Which documents a verb is about

Every verb here starts by turning paths into documents, and the shape of
that step decides what the verb can be trusted to have done. It has two
moves, and keeping them apart is the whole discipline: *find* the
markdown, then *decide membership by parsing it*.

The old code fused the two and did the deciding with `content.contains`.
That failed three ways at once. It admitted any prose that merely wrote
the tangle key. It missed every document whose only declaration was
its pipelines, since this binary's grep did not name that key — and a
missed pipelines document is a loud error this binary would otherwise
have raised, silently not raised. And, worst, the grep lived only in the
directory branch: a named file was taken as given, so the *same
document* answered differently depending on how you named it.
`check dir/` walked past a document with a broken `<<ref>>` and no
tangle target and then printed `all references OK`; `check dir/doc.md`
found the same broken reference and exited 1. The documents that shape
recommends first — reference-only pages built from `from=`/`symbol=`
chunks, which need no tangle target at all — are exactly the ones the
directory form dropped, and the directory form is the one every document
here tells a reader to run.

<a name="chunk-markdown-under"></a><sub>[`src/cli.rs`](../../crates/x0k-tangle/src/cli.rs) · `#markdown-under`</sub>

```rust {#markdown-under file="src/cli.rs"}
/// Every `.md` under `paths`, deduplicated and ordered: a file is taken
/// as given, a directory is walked.
///
/// This step finds files and nothing else. What a verb *does* with a
/// document is decided from its parse, below, so that the answer cannot
/// depend on whether the reader named the file or the directory holding
/// it.
/// A path as the reader named it: relative to the workspace root when it
/// is under one, and unchanged when it is not.
///
/// The root is tried twice, because the reader writes `--workspace .`
/// and the tangler resolves outputs against the real directory. A
/// literal strip against `.` matches nothing, which is how the absolute
/// destination survived the first attempt at this line.
fn under<'a>(path: &'a Path, workspace_root: &Path) -> std::path::Display<'a> {
    path.strip_prefix(workspace_root)
        .ok()
        .or_else(|| {
            workspace_root
                .canonicalize()
                .ok()
                .and_then(|root| path.strip_prefix(root).ok())
        })
        .unwrap_or(path)
        .display()
}

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

<a name="chunk-declares"></a><sub>[`src/cli.rs`](../../crates/x0k-tangle/src/cli.rs) · `#declares`</sub>

```rust {#declares file="src/cli.rs"}
/// What a document declares about itself, read off its parse.
struct Declares {
    /// It names somewhere to write: a `folio:tangleCrate` or
    /// `folio:tangleRoot`, per-language `folio:tangleRoots`, or
    /// `folio:pipelines`. The same predicate
    /// `tangle_document` applies before it does any work.
    target: bool,
    /// It has a chunk to fill from source (`from=`), which is what
    /// `sync` is about.
    fills: bool,
    /// Every chunk it declares is a `from=` mirror, and there is at
    /// least one. Such a document owns no code: it shows what other
    /// files hold, so naming nowhere to write is its shape rather
    /// than an omission.
    mirrors_only: bool,
    /// How many chunks it declares — the number that makes "nothing to
    /// write" worth saying out loud instead of reporting as a zero.
    chunks: usize,
}

impl Declares {
    /// A file this process cannot read or parse declares nothing it
    /// can act on, and saying so in one place keeps the three
    /// answers from drifting apart as the struct grows.
    fn nothing() -> Self {
        Self { target: false, fills: false, mirrors_only: false, chunks: 0 }
    }
}

fn declares(path: &Path) -> Declares {
    let Ok(content) = std::fs::read_to_string(path) else {
        return Declares::nothing();
    };
    let Ok(parsed) = crate::parser::parse_document(&content) else {
        return Declares::nothing();
    };
    let bodies: Vec<_> = parsed.chunks.values().flatten().collect();
    Declares {
        target: parsed.tangle_crate.is_some()
            || parsed.tangle_root.is_some()
            || !parsed.tangle_roots.is_empty()
            || !parsed.pipelines.is_empty(),
        fills: bodies.iter().any(|chunk| chunk.from.is_some()),
        mirrors_only: !bodies.is_empty()
            && bodies.iter().all(|chunk| chunk.from.is_some()),
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

<a name="chunk-discover-documents"></a><sub>[`src/cli.rs`](../../crates/x0k-tangle/src/cli.rs) · `#discover-documents`</sub>

```rust {#discover-documents file="src/cli.rs"}
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

<a name="chunk-nothing-to-write"></a><sub>[`src/cli.rs`](../../crates/x0k-tangle/src/cli.rs) · `#nothing-to-write`</sub>

```rust {#nothing-to-write file="src/cli.rs"}
/// What `tangle` says about a document it was named and cannot write from.
fn nothing_to_write(path: &Path, chunks: usize) -> String {
    format!(
        "{}: declares {chunks} chunk(s) and no tangle target \
         (folio:tangleRoot, folio:tangleCrate, folio:tangleRoots, or folio:pipelines); nothing to write",
        path.display()
    )
}

/// What `tangle` says about a document whose every chunk mirrors code it
/// does not own. It names nowhere to write because writing is not what
/// it is for, so the run says what it saw and passes.
fn mirror_only(path: &Path, chunks: usize) -> String {
    format!(
        "{}: mirrors {chunks} chunk(s) from source it does not own; \
         nothing to tangle (`sync` fills these and `check` catches them going stale)",
        path.display()
    )
}
```

## Composing the crate root and the binary

<a name="chunk-root"></a><sub>[`src/lib.rs`](../../crates/x0k-tangle/src/lib.rs) · `#root` · assembles [crate-doc](#chunk-crate-doc) · [modules](#chunk-modules) · [exports](#chunk-exports) · [source-check](#chunk-source-check) · [diagnostics](#chunk-diagnostics)</sub>

```rust {#root}
<<crate-doc>>

<<modules>>

<<exports>>

<<source-check>>

<<diagnostics>>
```

<a name="chunk-bin-root"></a><sub>[`src/main.rs`](../../crates/x0k-tangle/src/main.rs) · `#bin-root` · assembles [bin-doc](#chunk-bin-doc) · [bin-main](#chunk-bin-main)</sub>

```rust {#bin-root file="src/main.rs"}
<<bin-doc>>

<<bin-main>>
```

<a name="chunk-cli-root"></a><sub>[`src/cli.rs`](../../crates/x0k-tangle/src/cli.rs) · `#cli-root` · assembles [cli-doc](#chunk-cli-doc) · [cli-imports](#chunk-cli-imports) · [cli-host](#chunk-cli-host) · [cli-struct](#chunk-cli-struct) · [command-enum](#chunk-command-enum) · [main-fn](#chunk-main-fn) · [clobber-settings](#chunk-clobber-settings) · [dangling-finding](#chunk-dangling-finding) · [title-warning](#chunk-title-warning) · [references-verdict](#chunk-references-verdict) · [nothing-to-write](#chunk-nothing-to-write) · [markdown-under](#chunk-markdown-under) · [declares](#chunk-declares) · [discover-documents](#chunk-discover-documents)</sub>

```rust {#cli-root file="src/cli.rs"}
<<cli-doc>>

<<cli-imports>>

<<cli-host>>

<<cli-struct>>

<<command-enum>>

<<main-fn>>

<<clobber-settings>>

<<dangling-finding>>

<<title-warning>>

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

`source_check` is the one mechanism this chapter keeps, and it is here
for a reason the layout cannot express anywhere else: it is the rule that
decides whether a tree is sound, and anything that links the library —
not only the CLI — has to reach the same answer. The CLI itself is one
copy for every binary for the same reason, one step further out: a
sentence two binaries print differently is a reader told two things
about one tree.

## Pinning the verdicts

Most of this chapter's claims are about what the process does rather than
what it computes — an exit code, or a sentence a reader believes or does
not — and none of those is reachable from a unit test of a library
function. So they are pinned the way [`cli-faces.md`](cli-faces.md) pins
the other faces: run the built binary over a temp fixture, and let what
it prints and how it exits be the claim. `sync` exits non-zero when a
chunk it was asked to fill stayed empty. `check`'s dangling-edge note
says only what is true of the tree it was pointed at, and under
`--closed` the same edge is a defect that fails the run.

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

The last pins what folding the bundle's copy into this one must not
change: `--version` still names this package and nothing else. The
corpus build's sweep is pinned in its own chapter. What the bundle adds —
its registry, its root variable, its tolerance for collisions — is
pinned in its own suite, against its own binary.

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

/// A document with a chunk whose `<<ref>>` names nothing, and no tangle
/// target in its header — the reference-only shape an adopter writes
/// first. `{id}` distinguishes copies.
fn broken_reference_doc(id: &str) -> String {
    format!(
        "# Doc\n\n```turtle folio:document\nimplementation:{id} a x0k:Implementation ;\n    \
         x0k:status \"draft\" ;\n    x0k:summary \"A document with a broken chunk reference \
         and nowhere to write.\" .\n```\n\n\
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

/// A class documented the way a Python library documents one: a fenced
/// example indented inside the docstring.
const PY_WITH_FENCE: &str = "class Thing:\n    \"\"\"A thing.\n\n    Example:\n        ```python\n        from thing import Thing\n        t = Thing()\n        ```\n    \"\"\"\n\n    x: int = 1\n";

/// The adopter's reproducer at the face they actually run. `sync` used
/// to end the chunk at the indented nested fence, splice the new body
/// ahead of it, and leave the tail standing as prose — so the document
/// grew on every run while the run reported success, and `check` then
/// blamed the source file (2026-09-22). Four syncs, one body, and a
/// green `check` over the result.
#[test]
fn sync_is_idempotent_over_a_mirror_whose_body_holds_a_fence() {
    let tmp = TempDir::new().unwrap();
    write(tmp.path(), "thing.py", PY_WITH_FENCE);
    write(
        tmp.path(),
        "doc.md",
        "# doc\n\n```python {#thing from=\"thing.py\" symbol=\"Thing\"}\n```\n",
    );

    let mut runs: Vec<String> = Vec::new();
    for _ in 0..4 {
        let out = sync(tmp.path());
        assert!(
            out.status.success(),
            "sync failed: {}",
            String::from_utf8_lossy(&out.stderr)
        );
        runs.push(fs::read_to_string(tmp.path().join("doc.md")).unwrap());
    }
    assert_eq!(runs[0], runs[3], "the document grew across syncs:\n{}", runs[3]);
    assert_eq!(
        runs[3].matches("from thing import Thing").count(),
        1,
        "the docstring example was duplicated:\n{}",
        runs[3]
    );

    let out = check_in(tmp.path());
    assert!(
        out.status.success(),
        "check on a freshly-synced mirror: {}",
        String::from_utf8_lossy(&out.stderr)
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
        "# Doc\n\n```turtle folio:document\nimplementation:fixture a x0k:Implementation ;\n    \
         x0k:status \"draft\" ;\n    folio:tangleCrate \"fixture\" ;\n    \
         folio:tangleRoot \"src/lib.rs\" .\n```\n\n```rust {#root}\nfn f() {}\n```\n",
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

/// The summary line prints on a failing run, which is when it is worth
/// most and when it was written for the other case: seven references
/// read, one of them broken, and the last line the reader met said all
/// seven resolved (jj, 2026-09-23).
#[test]
fn check_counts_the_source_references_that_did_not_resolve() {
    let tmp = TempDir::new().unwrap();
    write(tmp.path(), "remap.js", JS_SOURCE);
    write(
        tmp.path(),
        "doc.md",
        "# Remap\n\n\
         ```javascript {#remap from=\"remap.js\" symbol=\"createHorizonRemap\"}\n```\n\n\
         ```javascript {#gone from=\"remap.js\" symbol=\"noSuchSymbol\"}\n```\n",
    );

    let out = check_in(tmp.path());
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(!out.status.success(), "the broken mirror passed: {stderr}");
    assert!(
        stderr.contains("1 of 2 from= source references resolve"),
        "the summary counts what resolved, not what it read: {stderr}"
    );
    assert!(
        !stderr.contains("chunks, 2 from= source references resolve"),
        "the line that said the opposite of the exit code is gone: {stderr}"
    );
}

/// A Markdown file with no folio header is skipped — that is what makes
/// adoption incremental — and the count is what keeps skipping it from
/// being silent. An untyped `<>` header names no document, so it counts
/// the same. `--require-header` is the reader saying every file under
/// these paths is supposed to be typed.
#[test]
fn check_counts_the_markdown_it_walked_past_and_can_be_told_to_refuse_it() {
    let tmp = TempDir::new().unwrap();
    write(
        tmp.path(),
        "typed.md",
        "# Typed\n\n```turtle folio:document\ndesign:typed a x0k:Design ;\n    \
         x0k:status \"draft\" .\n```\n",
    );
    write(tmp.path(), "untyped.md", "# Untyped\n\nNo header here.\n");
    write(
        tmp.path(),
        "tool-only.md",
        "# Tool only\n\n```turtle folio:document\n<> folio:tangleCrate \"x\" .\n```\n",
    );

    let out = check_in(tmp.path());
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(out.status.success(), "an untyped file is not a defect: {stderr}");
    assert!(
        stderr.contains("1 header(s) read against the vocabulary")
            && stderr.contains("2 markdown files carried no header"),
        "the line says what it read and what it walked past: {stderr}"
    );

    let out = Command::new(env!("CARGO_BIN_EXE_x0k-tangle"))
        .arg("check")
        .arg(tmp.path())
        .arg("--workspace")
        .arg(tmp.path())
        .arg("--require-header")
        .output()
        .expect("the x0k-tangle binary runs");
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(!out.status.success(), "--require-header let it through: {stderr}");
    assert!(
        stderr.contains("untyped.md: carries no folio header")
            && stderr.contains("tool-only.md: carries no folio header"),
        "the defect names the files: {stderr}"
    );

    let out = Command::new(env!("CARGO_BIN_EXE_x0k-tangle"))
        .arg("check")
        .arg(tmp.path())
        .arg("--workspace")
        .arg(tmp.path())
        .arg("--require-envelope")
        .output()
        .expect("the x0k-tangle binary runs");
    assert!(
        !out.status.success(),
        "only --require-header names this refusal; another spelling is an unknown flag"
    );
}

/// Every path a verb prints is written the way the reader named it. The
/// tangler holds an output absolute, so this line paired a relative
/// document with an absolute destination (jj, 2026-09-23).
#[test]
fn tangle_names_its_outputs_the_way_the_reader_named_the_workspace() {
    let tmp = TempDir::new().unwrap();
    write(
        tmp.path(),
        "docs/ratelimit.md",
        "# Bucket\n\n```turtle folio:document\nimplementation:ratelimit a x0k:Implementation ;\n    \
         x0k:status \"draft\" ;\n    folio:tangleCrate \"crates/ratelimit\" ;\n    \
         folio:tangleRoot \"src/bucket.rs\" .\n```\n\n\
         ```rust {#root}\npub fn take() {}\n```\n",
    );

    // `--workspace .` from the repository root is how the guide says to
    // run it, and the shape the absolute destination survived under.
    let out = Command::new(env!("CARGO_BIN_EXE_x0k-tangle"))
        .arg("tangle")
        .arg("docs")
        .arg("--workspace")
        .arg(".")
        .current_dir(tmp.path())
        .output()
        .expect("the x0k-tangle binary runs");
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(out.status.success(), "tangle failed: {stderr}");
    assert!(
        stderr.contains("→ crates/ratelimit/src/bucket.rs"),
        "the destination is workspace-relative: {stderr}"
    );
    assert!(
        !stderr.contains(&format!("→ {}", tmp.path().display())),
        "no absolute destination survives: {stderr}"
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
fn shipped_predicate() -> String {
    let snake = x0k_ontology::KNOWN_EDGE_PREDICATES
        .first()
        .copied()
        .expect("a build whose vocabulary declares no document edge ships no document module");
    let camel = x0k_ontology::snake_to_camel(snake).expect("every known predicate has a term");
    format!("x0k:{camel}")
}

#[test]
fn the_dangling_edge_note_claims_only_what_is_true_of_any_tree() {
    let tmp = TempDir::new().unwrap();
    write(
        tmp.path(),
        "docs/fixture.md",
        &format!(
            "# Fixture\n\n```turtle folio:document\ndesign:fixture a x0k:Design ;\n    \
             x0k:status \"draft\" ;\n    {} design:elsewhere .\n```\n",
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
fn closed_makes_an_edge_that_leaves_the_set_fail_the_run() {
    let tmp = TempDir::new().unwrap();
    write(
        tmp.path(),
        "docs/fixture.md",
        &format!(
            "# Fixture\n\n```turtle folio:document\ndesign:fixture a x0k:Design ;\n    \
             x0k:status \"draft\" ;\n    {} design:elsewhere .\n```\n",
            shipped_predicate()
        ),
    );

    let out = run(&["check", "--closed"], tmp.path());
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        !out.status.success(),
        "the set is closed and an edge left it, and check passed: {stderr}"
    );
    assert!(
        stderr.contains("`x0k:design/elsewhere` names no document under the paths scanned"),
        "the defect names the target that resolves to nothing: {stderr}"
    );
    assert!(
        !stderr.contains("note:"),
        "under --closed the finding is a defect, not a note: {stderr}"
    );
}

#[test]
fn a_declared_edge_that_leaves_the_set_fails_the_run_under_closed_too() {
    let tmp = TempDir::new().unwrap();
    write(
        tmp.path(),
        "docs/fixture.md",
        "# Fixture\n\n```turtle folio:document\nwiki:fixture a x0k:Wiki ;\n    \
         x0k:status \"draft\" .\n```\n\n```turtle folio:graph\n\
         affordance:do_the_thing a x0k:Affordance ;\n    \
         x0k:enabledBy x0k:software-module\\/elsewhere .\n```\n",
    );

    let open = run(&["check"], tmp.path());
    assert!(
        open.status.success(),
        "the default still notes it: {}",
        String::from_utf8_lossy(&open.stderr)
    );

    let out = run(&["check", "--closed"], tmp.path());
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        !out.status.success(),
        "a declared edge left a closed set and check passed: {stderr}"
    );
    assert!(
        stderr.contains("names no declaration under the paths scanned"),
        "the defect says which level it is about: {stderr}"
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
        "# Prose\n\n```turtle folio:document\ndesign:prose a x0k:Design ;\n    \
         x0k:status \"draft\" .\n```\n\nNo chunks here.\n",
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

/// One name typeset twice, and two names: only the second warns, it names
/// both strings, and the run exits the same for both — plain and under
/// `--closed`, because a heading is presentation and never the verdict
/// (`doc-index.md` § When the two names disagree). The disagreeing file is
/// the Backstage evaluator's `titletest/c.md` (2026-09-23).
#[test]
fn check_warns_when_the_frontmatter_title_and_the_h1_disagree_and_exits_the_same() {
    let agree = TempDir::new().unwrap();
    write(
        agree.path(),
        "docs/a.md",
        "---\ntitle: 'ADR013: Proper use of HTTP fetching libraries'\n---\n\
         # ADR013: Proper use of *HTTP* fetching libraries.\n\n\
         ```turtle folio:document\narchitecture:adr013 a x0k:Architecture .\n```\n",
    );
    let disagree = TempDir::new().unwrap();
    write(
        disagree.path(),
        "docs/c.md",
        "---\nid: adrs-adrZ\ntitle: 'ADRZ: Frontmatter wins?'\n---\n# Body H1 Different\n\n\
         ```turtle folio:document\narchitecture:adrz a x0k:Architecture .\n```\n\nText.\n",
    );

    for flags in [&["check"][..], &["check", "--closed"][..]] {
        let quiet = run(flags, agree.path());
        let loud = run(flags, disagree.path());
        let quiet_err = String::from_utf8_lossy(&quiet.stderr);
        let loud_err = String::from_utf8_lossy(&loud.stderr);
        assert!(
            !quiet_err.contains("warning:"),
            "one name typeset twice was reported as two ({flags:?}): {quiet_err}"
        );
        assert_eq!(
            loud_err.matches("warning:").count(),
            1,
            "two names, one warning ({flags:?}): {loud_err}"
        );
        assert!(
            loud_err.contains("frontmatter title `ADRZ: Frontmatter wins?`")
                && loud_err.contains("first heading `Body H1 Different`"),
            "the warning names both strings ({flags:?}): {loud_err}"
        );
        assert!(
            quiet.status.success() && loud.status.success(),
            "a title disagreement failed the run ({flags:?}): {quiet_err} / {loud_err}"
        );
        assert_eq!(
            quiet.status.code(),
            loud.status.code(),
            "the warning moved the exit code ({flags:?})"
        );
    }
}

#[test]
fn check_fails_two_documents_that_declare_one_id() {
    let tmp = TempDir::new().unwrap();
    let doc = "# Copy\n\n```turtle folio:document\ndesign:collision a x0k:Design ;\n    \
               x0k:status \"draft\" .\n```\n";
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
        "# Once\n\n```turtle folio:document\ndesign:once a x0k:Design ;\n    \
         x0k:status \"draft\" .\n```\n",
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

/// The shape the integration guide tells an existing codebase to write
/// first: chunks that mirror symbols out of code the document does not
/// own, and no tangle target, because there is nothing to write.
fn mirror_only_doc() -> String {
    "# Doc\n\n```turtle folio:document\nimplementation:mirror a x0k:Implementation ;\n    \
     x0k:status \"draft\" ;\n    x0k:summary \"A document that mirrors code it does not own.\" .\n\
     ```\n\n```javascript {#remap from=\"remap.js\" symbol=\"createHorizonRemap\"}\n```\n"
        .to_string()
}

#[test]
fn tangle_passes_a_mirror_only_document_it_was_named() {
    let tmp = TempDir::new().unwrap();
    write(tmp.path(), "remap.js", JS_SOURCE);
    write(tmp.path(), "docs/d.md", &mirror_only_doc());

    let out = Command::new(env!("CARGO_BIN_EXE_x0k-tangle"))
        .arg("tangle")
        .arg(tmp.path().join("docs/d.md"))
        .arg("--workspace")
        .arg(tmp.path())
        .output()
        .expect("the x0k-tangle binary runs");
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        out.status.success(),
        "the document the guide recommends writing first was refused: {stderr}"
    );
    assert!(
        stderr.contains("mirrors 1 chunk(s) from source it does not own"),
        "the run says why nothing was written: {stderr}"
    );
    assert!(
        !tmp.path().join("src").exists(),
        "a mirror-only document wrote something"
    );
}

#[test]
fn tangle_answers_the_same_for_a_mirror_only_file_and_its_directory() {
    let tmp = TempDir::new().unwrap();
    write(tmp.path(), "remap.js", JS_SOURCE);
    write(tmp.path(), "docs/d.md", &mirror_only_doc());

    let by_file = Command::new(env!("CARGO_BIN_EXE_x0k-tangle"))
        .arg("tangle")
        .arg(tmp.path().join("docs/d.md"))
        .arg("--workspace")
        .arg(tmp.path())
        .output()
        .expect("the x0k-tangle binary runs");
    let by_dir = Command::new(env!("CARGO_BIN_EXE_x0k-tangle"))
        .arg("tangle")
        .arg(tmp.path().join("docs"))
        .arg("--workspace")
        .arg(tmp.path())
        .output()
        .expect("the x0k-tangle binary runs");

    assert_eq!(
        by_file.status.code(),
        by_dir.status.code(),
        "the two forms disagree about a mirror-only document: file said {:?}, directory said {:?}",
        String::from_utf8_lossy(&by_file.stderr),
        String::from_utf8_lossy(&by_dir.stderr)
    );
}

#[test]
fn tangle_writes_a_document_that_names_a_target() {
    let tmp = TempDir::new().unwrap();
    write(
        tmp.path(),
        "docs/d.md",
        "# Doc\n\n```turtle folio:document\nimplementation:writes a x0k:Implementation ;\n    \
         x0k:status \"draft\" ;\n    folio:tangleCrate \".\" ;\n    \
         folio:tangleRoot \"src/lib.rs\" .\n```\n\n```rust {#root}\npub fn f() {}\n```\n",
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
        "# Doc\n\n```turtle folio:document\nimplementation:guard a x0k:Implementation ;\n    \
         x0k:status \"draft\" ;\n    folio:tangleCrate \".\" ;\n    \
         folio:tangleRoot \"src/lib.rs\" .\n```\n\n```rust {{#root}}\npub fn {body}() {{}}\n```\n"
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

/// The warning is audible. `Unrecorded` is the one verdict `guard_outputs`
/// lets through — a file on disk that no sidecar claims is as likely a
/// first-time graduation as a lost sidecar, and refusing graduations would
/// make the common adoption move impossible — so the write proceeds and the
/// `warn!` is the whole of the signal. Until 2026-09-09 neither binary
/// installed a subscriber, and that signal reached nobody: a run overwrote
/// an unclaimed file at the default level and under `RUST_LOG=warn`, printed
/// nothing about it, and exited 0. This asserts through the shell, which is
/// the only place the absence was observable.
#[test]
fn tangle_warns_before_overwriting_a_file_it_never_wrote() {
    let tmp = TempDir::new().unwrap();
    // No prior tangle, so no sidecar claims the path: the file on disk is
    // Unrecorded rather than Foreign, and the run is allowed to proceed.
    write(tmp.path(), "src/lib.rs", "pub fn hand_authored() {}\n");
    write(tmp.path(), "docs/d.md", &tangling_doc("first"));

    let out = tangle_in(tmp.path(), false);
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        out.status.success(),
        "an unrecorded output is a graduation, not a refusal: {stderr}"
    );
    assert!(
        stderr.contains("tangle.output.unrecorded"),
        "the overwrite of an unclaimed file was silent: {stderr}"
    );
    assert!(
        stderr.contains("src/lib.rs"),
        "the warning names the file it is about to overwrite: {stderr}"
    );
    let text = fs::read_to_string(tmp.path().join("src/lib.rs")).unwrap();
    assert!(
        text.contains("first") && !text.contains("hand_authored"),
        "the write did proceed, which is what makes the warning the signal: {text}"
    );
}

/// The mirror comparison, through the shell. `check` resolved a symbol and
/// stopped, so a document showing a body its source no longer held passed
/// green — the state 100 of the corpus's 150 mirrors were in on 2026-09-09,
/// every one of them invisible to every gate. Ratchet: `tools/mirror-drift`.
#[test]
fn check_fails_a_mirror_whose_body_the_source_no_longer_holds() {
    let tmp = TempDir::new().unwrap();
    write(tmp.path(), "remap.js", JS_SOURCE);
    // The symbol still resolves; only the body the document shows is old.
    write(
        tmp.path(),
        "doc.md",
        "# Remap\n\n```javascript {#remap from=\"remap.js\" \
         symbol=\"createHorizonRemap\"}\nexport function createHorizonRemap(scale) \
         {\n  return (u) => u / scale;\n}\n```\n",
    );

    let out = check_in(tmp.path());
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        !out.status.success(),
        "a mirror showing a body its source does not hold passed: {stderr}"
    );
    assert!(
        stderr.contains("the mirrored body and remap.js disagree"),
        "the finding names the source it disagrees with: {stderr}"
    );
    assert!(
        !stderr.contains("holds now"),
        "and does not accuse the source of having moved: {stderr}"
    );
    assert!(
        stderr.contains("first difference at body line 2"),
        "the finding sends the reader to the line, not to a diff: {stderr}"
    );
}

/// A mirror that agrees stays green. The predicate is `sync`'s — line
/// sequences, the thing `apply_from_patches` writes — so a document is
/// clean exactly when `sync` would leave it alone.
#[test]
fn check_passes_a_mirror_whose_body_matches_its_source() {
    let tmp = TempDir::new().unwrap();
    write(tmp.path(), "remap.js", JS_SOURCE);
    write(
        tmp.path(),
        "doc.md",
        "# Remap\n\n```javascript {#remap from=\"remap.js\" \
         symbol=\"createHorizonRemap\"}\nexport function createHorizonRemap(scale) \
         {\n  return (u) => u * scale;\n}\n```\n",
    );

    let out = check_in(tmp.path());
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(out.status.success(), "a matching mirror was called drift: {stderr}");
}

/// An EMPTY mirror is not drift. It is what an author writes before the
/// first `sync` — the shape `sync_fills_a_javascript_chunk_and_passes`
/// exercises from the other side — and reporting it here would make the
/// ordinary first fill a failure.
#[test]
fn check_passes_an_unfilled_mirror() {
    let tmp = TempDir::new().unwrap();
    write(tmp.path(), "remap.js", JS_SOURCE);
    write(
        tmp.path(),
        "doc.md",
        "# Remap\n\n```javascript {#remap from=\"remap.js\" \
         symbol=\"createHorizonRemap\"}\n```\n",
    );

    let out = check_in(tmp.path());
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        out.status.success(),
        "an unfilled mirror is sync's ordinary first fill, not drift: {stderr}"
    );
}

/// `--version` is the line a CI log reads to say which tangler ran, and
/// the name on it is this package's: another binary linking the same CLI
/// prints its own.
#[test]
fn version_names_this_package() {
    let out = Command::new(env!("CARGO_BIN_EXE_x0k-tangle"))
        .arg("--version")
        .output()
        .expect("the x0k-tangle binary runs");
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(out.status.success(), "--version failed: {stdout}");
    assert_eq!(
        stdout.trim(),
        format!("x0k-tangle {}", env!("CARGO_PKG_VERSION")),
        "the version line names the package and its version"
    );
}
`````

## The package manifest

Document instance rendering uses the format crate’s optional document-vocabulary API.
The manifest is a complete chunk so repository projection can carry its public form.

<a name="chunk-package-manifest"></a><sub>[`Cargo.toml`](../../crates/x0k-tangle/Cargo.toml) · `#package-manifest`</sub>

```toml {#package-manifest file="Cargo.toml"}
[package]
name = "x0k-tangle"
version = "0.4.0"
edition = { workspace = true }
description = "Literate programming for folio Markdown: writes source files from a document's named code blocks, fills quoted blocks from source that already exists, renders a document as an HTML page, and checks the code against the tree and each document's header against the vocabulary."
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
# `corpus` compiles the two verbs that are ours alone (the reader website a
# publication is woven into, the whole-tree sweep of our corpus's layout)
# and the modules only they use. The verbs that publish a collection as a
# repository are in every build. Default in the monorepo; a publication
# severs it by name,
# so the published manifest declares it with an empty list, out of
# `default`, and the published source does not carry what it gates.
# `motifs` wires the x0k:media / surface-wasm bundling into the corpus
# build's HTML region weaver. x0k-surface-build is publish-excluded, so the
# projector severs this feature too: declared, empty, out of `default`.
default = []
corpus = [] # severed in this publication: not supported here; the crate declares it, this publication does not enable it
motifs = [] # severed in this publication: its dependency is not published; enabling it does not build

[dependencies]
x0k-folio = { path = "../x0k-folio", features = ["document-vocabulary"] , version = "0.3.0" }
# The triples `colophon::parse_turtle` hands back: `index` lists a header's
# literal statements off them.
oxrdf = "0.3"
# The vocabulary a `check` reads documents against. Default features carry
# the runtime module loader, which is what `--vocabulary <dir>` and the
# PROVENANCE-recorded default are: a projected repository checks its own
# documents against the module files it actually shipped.
x0k-ontology = { path = "../x0k-ontology" , version = "0.3.1" }
# Shared renderer-agnostic syntax tokenizer; weave uses it to emit
# highlighted <span class="tok-*"> spans in the HTML output.
x0k-syntax = { path = "../x0k-syntax" , version = "0.1.0" }
# The icon profile's one implementation: `icon` reads every mark a document
# declares as `svg x0k:icon` through this crate's checker, binds it to a
# publication's palette, and writes the per-scheme files. Nothing in this
# crate draws a mark.
x0k-icon = { path = "../x0k-icon" , version = "0.2.0" }

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
# YAML the repository projector writes (a release workflow) is read back with it.
serde_norway = "0.9"
anyhow = { workspace = true }
clap = { workspace = true }
tracing = { workspace = true }
# The subscriber both binaries install (`init_diagnostics`). Without it the
# crate's `warn!`s — `tangle.output.unrecorded` above all — are emitted into
# no collector and print nothing at any level.
tracing-subscriber = { workspace = true, features = ["env-filter"] }
walkdir = "2"
# Format-preserving TOML editing: the repository projector rewrites crate
# manifests without disturbing hand-authored layout.
toml_edit = "0.22"
# In-process unified diffs for the projector and the receiver, computed
# without shelling out, so a report is complete wherever the tool runs.
similar = "2"
# Scratch directories the projector and the receiver remove with their report.
tempfile = "3"

[dev-dependencies]
tempfile = "3"
```
