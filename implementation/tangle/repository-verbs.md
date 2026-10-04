# x0k-tangle: publishing a collection as a repository

```turtle folio:document
implementation:tangle\/repository-verbs a x0k:Implementation ;
    x0k:status "draft" ;
    x0k:summary "The three verbs that take a collection of documents to a public repository and back — project a publication as a repository, publish it, and receive a contributor's clone of it as patches — with the modules they share, in every build of the tangler." ;
    x0k:concerns "tangle", "cli", "publishing" ;
    x0k:cites implementation:tangle\/crate,
        implementation:tangle\/collection,
        implementation:tangle\/region-repo,
        implementation:tangle\/publishing,
        implementation:tangle\/receiving ;
    x0k:implements design:publish-a-region-as-a-repository ;
    folio:tangleCrate "crates/x0k-tangle" ;
    folio:tangleRoot "src/repository_verbs.rs" .
```

A collection of documents becomes a public repository in three steps, and
each is a verb. `project-repo` reads a publication — a document that names
the documents and crates it publishes, the licence they go out under, and
how the result is built — and writes a standalone repository: the
documents, the code they generate, the licence, a README tangled from the
publication, `PROVENANCE.json` recording where each file came from, and a
CI script that re-tangles everything and fails on any difference.
`publish-repo` does that, proves the result builds and tests when it
ships crates, rehearses, and — only under `--really` — pushes it to the
publication's git remote (and uploads its crates to crates.io when the
publication asks for that). `receive-repo` goes the other way: a
contributor's clone of the published repository, read back into patches
against the files it was projected from.

These verbs ship in every build. Through 0.3.0 they were in the published
build marked `[corpus-only]`, refusing to run anywhere but our own corpus,
and the next change, never released, took them out of the published build
altogether. The operator's ruling for 0.4.0 was that publishing must work
for anybody's collection, so the work behind this chapter made each step
read the collection it is
pointed at — members found by the id their headers declare
([`collection.md`](collection.md)), the collection's own vocabulary
named with `--vocabulary`, build policy stated by the publication rather
than baked into the projector ([`region-repo.md`](region-repo.md)), a
collection with no crate at all — and this chapter moves them out from
behind the feature that held them back. What stays behind it is ours
alone: the reader website a publication is woven into and the sweep of
our corpus's own layout.

## The modules

Four modules exist for these verbs: the collection reader every verb
shares ([`collection.md`](collection.md)), the repository projector
([`region-repo.md`](region-repo.md)), the publish pipeline
([`publishing.md`](publishing.md)) and the receiver
([`receiving.md`](receiving.md)). The forge weave
([`region-gfm.md`](region-gfm.md)), which the projector uses to write
each chapter for a forge's renderer, is the crate's own and is declared
with the rest of its modules.

This file is included at the crate root rather than declared as a module
of its own, so every module keeps the path it always had —
`x0k_tangle::region_repo`, not a path under a new parent — and the
re-exports a consumer names stay where they were. Being included, it
takes `//` comments only: an inner `//!` is not legal at the point it
lands.

<a name="chunk-repository-doc"></a><sub>[`src/repository_verbs.rs`](../../crates/x0k-tangle/src/repository_verbs.rs) · `#repository-doc`</sub>

```rust {#repository-doc file="src/repository_verbs.rs"}
// Publishing a collection as a repository: the three verbs that project a
// publication, publish the projection, and receive a contribution back,
// and the modules they use. `src/lib.rs` includes this file in every build.
```

<a name="chunk-repository-modules"></a><sub>[`src/repository_verbs.rs`](../../crates/x0k-tangle/src/repository_verbs.rs) · `#repository-modules`</sub>

```rust {#repository-modules file="src/repository_verbs.rs"}
pub mod collection;
pub mod publish_repo;
pub mod receive;
pub mod region_repo;
```

The re-exports are the three verbs as library calls and the collection
reader they share.

<a name="chunk-repository-exports"></a><sub>[`src/repository_verbs.rs`](../../crates/x0k-tangle/src/repository_verbs.rs) · `#repository-exports`</sub>

```rust {#repository-exports file="src/repository_verbs.rs"}
pub use collection::{collection_documents, documents_declaring, CorpusLayout, Vocabulary};
pub use publish_repo::{publish_repo, publish_repo_in, PublishRepoOptions, PublishRepoReport, RemoteSource};
pub use receive::{receive_repo, receive_repo_in, ReceiveOptions, ReceiveReport};
pub use region_repo::{
    project_publication_repo, project_publication_repo_in, LicenseSource, RepoProjectOptions,
    RepoProjectReport,
};
```

## The verbs

The variants are a subcommand enum of their own, which the CLI flattens
into its list, so they sit in `--help` beside the literate verbs and
dispatch from the same `run`. Each variant's doc comment is its help
text, written for somebody publishing their own collection: what the
verb does, in the words of a repository and a document, with nothing
that assumes our corpus.

Each verb takes `--vocabulary <dir>`, repeatable: a directory of
vocabulary modules the publication and its members are read against
beside the modules this build compiled — what `check --vocabulary`
names. A collection typed in its own module is published by naming
that module; naming none reads the shipped vocabulary alone
([`collection.md`](collection.md) § "The vocabulary a publication is
read in").

<a name="chunk-repository-command-enum"></a><sub>[`src/repository_verbs.rs`](../../crates/x0k-tangle/src/repository_verbs.rs) · `#repository-command-enum` · assembles [project-repo-command](#chunk-project-repo-command) · [publish-repo-command](#chunk-publish-repo-command) · [receive-repo-command](#chunk-receive-repo-command)</sub>

```rust {#repository-command-enum file="src/repository_verbs.rs"}
// Flattened into the CLI's command list (crate.md § "Publishing a
// collection"); clap reads no help from the enum itself. Each variant's
// name is its verb's (`ProjectRepo` is `project-repo`), so the shared
// postfix is the verbs' own and stays.
#[derive(clap::Subcommand)]
#[allow(clippy::enum_variant_names)]
pub(crate) enum RepositoryCommand {
    <<project-repo-command>>
    <<publish-repo-command>>
    <<receive-repo-command>>
}
```

`project-repo` has no silent licence default: the flag is an explicit
override of the publication's `x0k:license`, and with neither the
projection refuses ([`region-repo.md`](region-repo.md)). `--allow-dirty`
is the escape hatch past the disclosure and closure guards, for
inspecting a projection that is not yet clean.

<a name="chunk-project-repo-command"></a><sub>[`src/repository_verbs.rs`](../../crates/x0k-tangle/src/repository_verbs.rs) · `#project-repo-command`</sub>

```rust {#project-repo-command file="src/repository_verbs.rs"}
/// Project a publication into a standalone repository.
///
/// Writes the documents the publication names, the code they generate,
/// its licence, a README tangled from the publication, PROVENANCE.json
/// (where every file came from) and a CI script that re-tangles the
/// documents and fails on any difference — then commits it with git.
/// Projecting into a directory that already holds a projection adds one
/// commit to its history.
ProjectRepo {
    /// The publication document: a Markdown file whose header is
    /// `a x0k:Publication` and names what it publishes.
    #[arg(value_name = "PUBLICATION")]
    region: PathBuf,
    /// Directory to write the repository into (created if absent).
    #[arg(long)]
    output_dir: PathBuf,
    /// Root of the collection the publication's documents and crates are
    /// found under (defaults to the current directory).
    #[arg(long)]
    workspace: Option<PathBuf>,
    /// SPDX licence to publish under, overriding the publication's
    /// `x0k:license`. With neither, the projection refuses: there is no
    /// default licence.
    #[arg(long)]
    license: Option<String>,
    /// Do not `git init` / commit the output directory.
    #[arg(long)]
    no_git: bool,
    /// Do not write `.github/workflows/` (the forge-agnostic `tools/ci`
    /// and `tools/x0k-guard-generated` are always written).
    #[arg(long)]
    no_github: bool,
    /// Project even when a guard refuses — an unpublished dependency, a
    /// document above public access — to inspect the result. Never for a
    /// repository you publish.
    #[arg(long)]
    allow_dirty: bool,
    /// A directory of vocabulary modules (`*.ttl`) to read the publication
    /// and its members against, in addition to the modules this build
    /// compiled. Repeatable; each is loaded as `check --vocabulary` loads one.
    #[arg(long = "vocabulary", value_name = "DIR")]
    vocabulary: Vec<PathBuf>,
},
```

`publish-repo` is the one verb with an irreversible half, and it sits
behind `--really` ([`publishing.md`](publishing.md)).

<a name="chunk-publish-repo-command"></a><sub>[`src/repository_verbs.rs`](../../crates/x0k-tangle/src/repository_verbs.rs) · `#publish-repo-command`</sub>

```rust {#publish-repo-command file="src/repository_verbs.rs"}
/// Publish a publication's repository: project, prove, rehearse, and
/// (only under --really) push.
///
/// Projects the publication with every guard on, builds and tests the
/// result when it ships crates, and reports the commit, remote and branch
/// a push would use. Under --really it pushes to the remote: --remote,
/// else the publication's `x0k:publishRemote`, else its `x0k:repository`,
/// else its `x0k:publishedOn` surface mapped in config/x0k-tangle.toml.
/// A publication that states
/// `x0k:publishRegistry "crates.io"` also has its crates checked against
/// the crates.io index, rehearsed with `cargo publish --dry-run`, and —
/// under --really — uploaded before the push.
PublishRepo {
    /// The publication document: a Markdown file whose header is
    /// `a x0k:Publication` and names what it publishes.
    #[arg(value_name = "PUBLICATION")]
    region: PathBuf,
    /// Directory to project the repository into (created if absent).
    #[arg(long)]
    output_dir: PathBuf,
    /// Root of the collection the publication's documents and crates are
    /// found under (defaults to the current directory).
    #[arg(long)]
    workspace: Option<PathBuf>,
    /// SPDX licence to publish under, overriding the publication's
    /// `x0k:license`.
    #[arg(long)]
    license: Option<String>,
    /// Do not write `.github/workflows/`.
    #[arg(long)]
    no_github: bool,
    /// Git remote to push to — a URL or a path — overriding whatever the
    /// publication names.
    #[arg(long, value_name = "URL")]
    remote: Option<String>,
    /// Actually push (and upload crates, when the publication states a
    /// registry). Without it the run stops after the rehearsal.
    #[arg(long)]
    really: bool,
    /// A directory of vocabulary modules (`*.ttl`) to read the publication
    /// and its members against, in addition to the modules this build
    /// compiled. Repeatable; each is loaded as `check --vocabulary` loads one.
    #[arg(long = "vocabulary", value_name = "DIR")]
    vocabulary: Vec<PathBuf>,
},
```

<a name="chunk-receive-repo-command"></a><sub>[`src/repository_verbs.rs`](../../crates/x0k-tangle/src/repository_verbs.rs) · `#receive-repo-command`</sub>

```rust {#receive-repo-command file="src/repository_verbs.rs"}
/// Turn a contributor's clone of a published repository into patches
/// against the collection it was projected from.
///
/// Rebuilds the projection the clone started from (at the commit its
/// PROVENANCE.json records), diffs the clone against it, and sorts every
/// changed path: an edited document or hand-written source becomes a
/// unified patch against the file it came from; an edit to a generated
/// file is refused, naming the document that produces it. Writes the
/// patches and receipt.json, and under --apply patches the working copy
/// (it never commits). Exits non-zero when any change was refused.
ReceiveRepo {
    /// The contributor's clone of the published repository.
    clone: PathBuf,
    /// Root of the collection the patches apply to (defaults to the
    /// current directory).
    #[arg(long)]
    workspace: Option<PathBuf>,
    /// Directory for the patches and receipt.json (default: a temporary
    /// directory).
    #[arg(long)]
    out: Option<PathBuf>,
    /// Apply the patches to the working copy. Refused when a target file
    /// already has uncommitted changes.
    #[arg(long)]
    apply: bool,
    /// The publication document, when it cannot be found by the id the
    /// clone's PROVENANCE.json records.
    #[arg(long)]
    publication: Option<PathBuf>,
    /// Where to build the reference projection (must be outside the
    /// collection; default: the system temporary directory).
    #[arg(long)]
    scratch: Option<PathBuf>,
    /// A directory of vocabulary modules (`*.ttl`) to read the publication
    /// and its members against, in addition to the modules this build
    /// compiled. Repeatable; each is loaded as `check --vocabulary` loads one.
    #[arg(long = "vocabulary", value_name = "DIR")]
    vocabulary: Vec<PathBuf>,
},
```

## Dispatch

`run` hands one of these verbs here. Each arm resolves the collection
root, calls the library, and prints a report to stderr. Exit codes carry
the verdicts: `publish-repo` exits non-zero when the projection fails to
build or test, and `receive-repo` when any change was refused. The arms
print the report shapes their chapters define; the prose about what
each field means lives there, not here.

<a name="chunk-repository-run"></a><sub>[`src/repository_verbs.rs`](../../crates/x0k-tangle/src/repository_verbs.rs) · `#repository-run` · assembles [dispatch-project-repo](#chunk-dispatch-project-repo) · [dispatch-publish-repo](#chunk-dispatch-publish-repo) · [dispatch-receive-repo](#chunk-dispatch-receive-repo)</sub>

```rust {#repository-run file="src/repository_verbs.rs"}
/// Run one of the verbs that publish a collection as a repository.
pub(crate) fn run_repository(command: RepositoryCommand) -> Result<()> {
    match command {
        <<dispatch-project-repo>>

        <<dispatch-publish-repo>>

        <<dispatch-receive-repo>>
    }

    Ok(())
}
```

<a name="chunk-dispatch-project-repo"></a><sub>[`src/repository_verbs.rs`](../../crates/x0k-tangle/src/repository_verbs.rs) · `#dispatch-project-repo`</sub>

```rust {#dispatch-project-repo file="src/repository_verbs.rs"}
RepositoryCommand::ProjectRepo {
    region,
    output_dir,
    workspace,
    license,
    no_git,
    no_github,
    allow_dirty,
    vocabulary,
} => {
    let ws = workspace.unwrap_or_else(|| std::env::current_dir().unwrap());
    let vocabulary = crate::Vocabulary::load(&vocabulary)?;
    let opts = crate::RepoProjectOptions {
        license,
        git_init: !no_git,
        allow_dirty,
        emit_github: !no_github,
    };
    let report = crate::project_publication_repo_in(
        &region,
        &output_dir,
        &ws,
        &opts,
        &crate::region_repo::Proofs::Cargo,
        &vocabulary,
    )?;
    eprintln!(
        "projected repo {} → {} ({} crate(s), {} literate doc(s), license {} [{}]{})",
        region.display(),
        output_dir.display(),
        report.crates.len(),
        report.literate_docs.len(),
        report.license,
        match report.license_source {
            crate::LicenseSource::PublicationDoc => "from publication doc",
            crate::LicenseSource::Override => "explicit override",
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

The publish report says what each stage did, including the stages a
publication did not ask for, so a reader never has to infer from a
missing line whether crates.io was asked.

<a name="chunk-dispatch-publish-repo"></a><sub>[`src/repository_verbs.rs`](../../crates/x0k-tangle/src/repository_verbs.rs) · `#dispatch-publish-repo`</sub>

```rust {#dispatch-publish-repo file="src/repository_verbs.rs"}
RepositoryCommand::PublishRepo {
    region,
    output_dir,
    workspace,
    license,
    no_github,
    remote,
    really,
    vocabulary,
} => {
    let ws = workspace.unwrap_or_else(|| std::env::current_dir().unwrap());
    let vocabulary = crate::Vocabulary::load(&vocabulary)?;
    let opts = crate::PublishRepoOptions {
        license,
        emit_github: !no_github,
        really,
        remote,
    };
    let report = crate::publish_repo_in(&region, &output_dir, &ws, &opts, &vocabulary)?;
    eprintln!(
        "publish-repo {} → {} (license {})",
        region.display(),
        output_dir.display(),
        report.projection.license,
    );
    if report.cargo {
        eprintln!(
            "  build: {}  test: {}",
            if report.build_ok { "ok" } else { "FAILED" },
            if report.test_ok { "ok" } else { "FAILED" },
        );
    } else {
        eprintln!("  build: skipped — no crate ships");
    }
    match &report.registry {
        None => eprintln!("  registry: none stated (x0k:publishRegistry) — no crate is uploaded"),
        Some(registry) => {
            eprintln!("  publish order: {}", report.publish_order.join(" → "));
            eprintln!("  registry ({registry} index):");
            let width = report.plan.iter().map(|c| c.name.len() + c.version.len()).max().unwrap_or(0);
            for c in &report.plan {
                use crate::publish_repo::CrateDisposition as D;
                let said = match c.disposition {
                    D::Unpublishable => "never attempted (publish = false)",
                    D::AlreadyPublished => "already published — skipped",
                    D::Pending => "not on the index — will publish",
                };
                let pad = width - c.name.len() - c.version.len();
                eprintln!("    {} {}{:pad$}  {said}", c.name, c.version, "");
            }
            let pending: Vec<&str> = report
                .plan
                .iter()
                .filter(|c| c.disposition == crate::publish_repo::CrateDisposition::Pending)
                .map(|c| c.name.as_str())
                .collect();
            if pending.is_empty() {
                eprintln!("  to publish: nothing — the index already serves every publishable crate's version");
            } else {
                eprintln!("  to publish: {}", pending.join(" → "));
            }
            if let Some(r) = &report.rehearsal {
                eprintln!(
                    "  dry-run ({} pending): {}",
                    pending.len(),
                    if r.ok { "ok" } else { "FAILED" }
                );
                if !r.ok {
                    for line in r.output_tail.lines() {
                        eprintln!("      {line}");
                    }
                }
            }
        }
    }
    if !report.build_ok || !report.test_ok {
        eprintln!("  stopped: the projection must build and test green before any rehearsal");
        std::process::exit(1);
    }
    match (&report.remote, report.remote_source, &report.surface) {
        (Some(r), Some(crate::RemoteSource::Flag), _) => eprintln!("  remote: {r} (--remote)"),
        (Some(r), Some(crate::RemoteSource::Publication), _) => {
            eprintln!("  remote: {r} (x0k:publishRemote)")
        }
        (Some(r), Some(crate::RemoteSource::Repository), _) => {
            eprintln!("  remote: {r} (x0k:repository)")
        }
        (Some(r), _, Some(s)) => eprintln!("  remote: {s} → {r}"),
        (Some(r), _, None) => eprintln!("  remote: {r}"),
        (None, _, Some(s)) => eprintln!(
            "  remote: none — {s} has no [publish.remotes] entry in config/x0k-tangle.toml; \
             pass --remote or state x0k:publishRemote or x0k:repository"
        ),
        (None, _, None) => eprintln!(
            "  remote: none — pass --remote or state x0k:publishRemote or x0k:repository in the \
             publication"
        ),
    }
    if let Some(head) = &report.head {
        eprintln!("  head: {head}");
    }
    if report.published || report.pushed {
        eprintln!(
            "  PUBLISHED: crates.io={} push={}{}",
            report.published,
            report.pushed,
            report.branch.as_deref().map(|b| format!(" (branch {b})")).unwrap_or_default(),
        );
    } else if !really {
        eprintln!("  stopped before publishing (pass --really to publish)");
    }
}
```

<a name="chunk-dispatch-receive-repo"></a><sub>[`src/repository_verbs.rs`](../../crates/x0k-tangle/src/repository_verbs.rs) · `#dispatch-receive-repo`</sub>

```rust {#dispatch-receive-repo file="src/repository_verbs.rs"}
RepositoryCommand::ReceiveRepo {
    clone,
    workspace,
    out,
    apply,
    publication,
    scratch,
    vocabulary,
} => {
    let ws = workspace.unwrap_or_else(|| std::env::current_dir().unwrap());
    let vocabulary = crate::Vocabulary::load(&vocabulary)?;
    let opts = crate::ReceiveOptions {
        apply,
        out_dir: out.clone(),
        publication,
        scratch,
    };
    let report = crate::receive_repo_in(&clone, &ws, &opts, &vocabulary)?;
    eprintln!(
        "receive-repo {} ({}): clone rev {} vs reference {}{}{}",
        clone.display(),
        report.publication_uri,
        if report.clone_rev.is_empty() { "(none)" } else { &report.clone_rev },
        if report.reference_rev.is_empty() { "(none)" } else { &report.reference_rev },
        report.reference_commit.as_deref().map(|c| format!(" @ {c}")).unwrap_or_default(),
        if report.rev_exact { "" } else { "  [REV SKEW: diff includes the collection's own drift, reversed]" },
    );
    for c in &report.changes {
        let class = match c.class {
            crate::receive::Class::Literate => "literate (received)",
            crate::receive::Class::Source => "source (received)",
            crate::receive::Class::Generated => "GENERATED (refused)",
            crate::receive::Class::ProjectionLocal => "overlay (projection-local, not received)",
            crate::receive::Class::ProjectionOwned => "projection-owned (not received)",
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

## Composing the file

The CLI half is a private module, so its imports stay out of the crate
root this file lands in; the crate's CLI names the two things it needs
through the re-export at the bottom.

<a name="chunk-root"></a><sub>[`src/repository_verbs.rs`](../../crates/x0k-tangle/src/repository_verbs.rs) · `#root` · assembles [repository-doc](#chunk-repository-doc) · [repository-modules](#chunk-repository-modules) · [repository-exports](#chunk-repository-exports) · [repository-command-enum](#chunk-repository-command-enum) · [repository-run](#chunk-repository-run)</sub>

```rust {#root}
<<repository-doc>>

<<repository-modules>>

<<repository-exports>>

mod repository_cli {
    use std::path::PathBuf;

    use anyhow::Result;

    <<repository-command-enum>>

    <<repository-run>>
}

pub(crate) use repository_cli::{run_repository, RepositoryCommand};
```

## Pinning what ships

The claim this chapter exists to make true is a `--help` line: the three
verbs are in every build, and the two that stay ours are not in the
published one. It is pinned the way the crate's chapter pins its other
verdicts, by running the built binary. The first test runs in every
build; the second only where the `corpus` feature is off, which is the
build a published repository makes of itself.

<a name="chunk-repository-verdicts"></a><sub>[`tests/repository_verbs.rs`](../../crates/x0k-tangle/tests/repository_verbs.rs) · `#repository-verdicts`</sub>

`````rust {#repository-verdicts file="tests/repository_verbs.rs"}
//! Pins for the verbs that publish a collection as a repository
//! (`x0k:implementation/tangle/repository-verbs`), run through the binary.

use std::process::Command;

/// `x0k-tangle --help`, as the binary prints it.
fn help() -> String {
    let out = Command::new(env!("CARGO_BIN_EXE_x0k-tangle"))
        .arg("--help")
        .output()
        .expect("the x0k-tangle binary runs");
    assert!(out.status.success(), "--help exits 0");
    String::from_utf8_lossy(&out.stdout).into_owned()
}

/// A subcommand line of `--help`: two spaces, the verb, then its summary.
fn lists(help: &str, verb: &str) -> bool {
    help.lines().any(|line| line.starts_with("  ") && line.split_whitespace().next() == Some(verb))
}

#[test]
fn every_build_lists_the_three_publishing_verbs() {
    let help = help();
    for verb in ["project-repo", "publish-repo", "receive-repo"] {
        assert!(lists(&help, verb), "`{verb}` is in --help:\n{help}");
    }
    assert!(!help.contains("[corpus-only]"), "no verb is marked as needing our corpus:\n{help}");
}

#[cfg(not(feature = "corpus"))]
#[test]
fn the_published_build_lists_neither_the_reader_site_nor_the_sweep() {
    let help = help();
    for verb in ["weave-region", "workspace"] {
        assert!(!lists(&help, verb), "`{verb}` is not in the published build's --help:\n{help}");
    }
}

#[test]
fn publish_repo_help_names_the_remote_and_the_registry_opt_in() {
    let out = Command::new(env!("CARGO_BIN_EXE_x0k-tangle"))
        .args(["publish-repo", "--help"])
        .output()
        .expect("the x0k-tangle binary runs");
    let help = String::from_utf8_lossy(&out.stdout);
    assert!(help.contains("--remote <URL>"), "{help}");
    assert!(help.contains("x0k:publishRemote"), "{help}");
    assert!(help.contains("x0k:repository"), "{help}");
    assert!(help.contains("x0k:publishRegistry"), "{help}");
}
`````
