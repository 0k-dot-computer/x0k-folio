# Receiving a contribution from a projected repository

```turtle folio:document
implementation:tangle\/receiving a x0k:Implementation ;
    x0k:status "draft" ;
    x0k:summary "The door the world comes back through: a contributor's clone read as patches against the corpus files it was projected from, because a contribution is a proposal against the graph and never a merge into the projection." ;
    x0k:concerns "tangle", "publication", "contribution", "receive", "git", "provenance" ;
    x0k:cites implementation:tangle\/publishing,
        implementation:folio\/colophon ;
    x0k:implements design:publish-a-region-as-a-repository,
        affordance:receive_contribution_as_proposal ;
    folio:tangleCrate "crates/x0k-tangle" ;
    folio:tangleRoot "src/receive.rs" .
```

`region_repo` projects a publication outward as a buildable git
repository; `publish_repo` pushes it to the world. This module is the
door the world comes back through. Its central idea is the one the
design commits to: **a contribution is a proposal against the graph,
not a merge into the projection.** The public repository is regenerated
from the corpus on every publish, so nothing merged *into* it survives;
a change only lands if it is carried back to the monorepo file the
projection was made from. `receive_repo` does that carrying — it
reads a contributor's clone, works out what the clone was projected
from, and turns every change into either a patch against a monorepo
file or a stated reason why no such patch can exist.

The carried example: Carol clones the public bundle, notices a typo in
`corpora/x0k/implementation/folio/colophon.md` ("the header parser
tolerates keys it does not own"), and fixes it. Nothing about her
change mentions x0k; she edited a markdown file in a git repository. The
maintainer runs `x0k-tangle receive-repo <carol's clone> --workspace .`
and gets a report with one line — the doc, its classification
(literate, receivable), its monorepo target (the same path), and the
patch size — plus a unified diff that applies to the monorepo doc.
With `--apply` the doc in the working copy is patched; the maintainer
re-tangles, reviews, and commits under an intent, and Carol's fix goes
back out on the next projection. Had Carol instead edited
`x0k-folio/src/colophon.rs` — the `@generated` file that doc tangles
to — the report would refuse the change and name the document and
chunk that produce those lines, because generated code is not an edit
surface.

Placement: like the publisher, this is build-time tooling that reads
two trees and shells out to `git` — orchestration around edges, not
pure logic — so it lives beside the projector in `x0k-tangle`.

<a name="chunk-module-doc"></a><sub>[`src/receive.rs`](../../crates/x0k-tangle/src/receive.rs) · `#module-doc`</sub>

```rust {#module-doc}
//! Receive a contribution made in a projected repository back into the
//! monorepo as a proposed change — the inward leg of the projection
//! cycle (`x0k:design/publish-a-region-as-a-repository`).
//!
//! Reads the clone's `PROVENANCE.json`, rebuilds the projection it was
//! taken from as a reference, diffs the two trees, and classifies every
//! changed path: literate docs and hand-written source become patches
//! against their monorepo files; `@generated` outputs are refused with
//! the doc + chunk that produce them; overlay and projection-owned paths
//! are reported and left alone. Never commits — `--apply` patches the
//! working copy for the operator to review.

use anyhow::{bail, Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use crate::parser::parse_document;
use crate::region_gfm::unweave_chapter;
use crate::collection::{documents_declaring, CorpusLayout, Vocabulary};
use crate::region_repo::{
    project_publication_repo_in, source_package_roots, ProofOutcome, Proofs, RepoProjectOptions,
};
use crate::resolve::expand_chunk;
```

## Contract

Every changed path in the clone lands in exactly one of five classes.
The first two are *receivable* — a patch exists and can be applied; the
other three are not, and the report says why in a form a contributor
can act on:

<a name="chunk-classification"></a><sub>[`src/receive.rs`](../../crates/x0k-tangle/src/receive.rs) · `#classification`</sub>

```rust {#classification}
/// What a changed path in the clone is, and therefore what can be done
/// with it. Only `Literate` and `Source` produce a patch.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Class {
    /// A literate `.md` carried by the projection; the edit surface.
    Literate,
    /// Hand-written source inside a vendored crate.
    Source,
    /// An `@generated` output — refused; the doc that produces it is
    /// the place to make the change.
    Generated,
    /// A path the maintainer preserves on the public side (`overlay`
    /// in PROVENANCE.json); projection-local by declaration.
    ProjectionLocal,
    /// Scaffolding the projector regenerates from the corpus (README,
    /// CI, manifests, licenses, provenance, sidecars).
    ProjectionOwned,
}

impl Class {
    pub fn receivable(self) -> bool {
        matches!(self, Class::Literate | Class::Source)
    }
    pub fn refused(self) -> bool {
        self == Class::Generated
    }
}
```

A generated-file refusal carries its attribution — the document and,
when the tangler can localize it, the chunk names whose expansion
contains the touched lines:

<a name="chunk-origin"></a><sub>[`src/receive.rs`](../../crates/x0k-tangle/src/receive.rs) · `#origin`</sub>

```rust {#origin}
/// Where a refused `@generated` edit should have been made.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GeneratedOrigin {
    /// Monorepo path of the literate doc that tangles the output.
    pub doc: String,
    /// Chunk names whose expansion contains the changed lines
    /// (innermost match per line). Empty when localization failed.
    pub chunks: Vec<String>,
}
```

One entry per changed path. `target` is the monorepo path a receivable
patch applies to — usually the clone path itself, since the projector
preserves paths, but always looked up rather than assumed. `patch` is a
unified diff with `a/`–`b/` headers against the *target*, ready for
`git apply` from the workspace root:

<a name="chunk-change"></a><sub>[`src/receive.rs`](../../crates/x0k-tangle/src/receive.rs) · `#change`</sub>

```rust {#change}
/// One changed path in the clone, classified.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReceivedChange {
    /// Path relative to the clone root.
    pub path: String,
    pub class: Class,
    /// `added` | `modified` | `deleted`, from the clone's point of view.
    pub kind: String,
    /// Monorepo path the patch applies to (receivable classes only).
    pub target: Option<String>,
    /// Unified diff against `target` (receivable classes only).
    pub patch: Option<String>,
    /// For `Generated`: the doc + chunks that produce the file.
    pub produced_by: Option<GeneratedOrigin>,
    /// Patch file written under `--out`, relative to it.
    pub patch_file: Option<String>,
}
```

The options and the report. The report states which revision the
reference was built at and whether that is the revision the clone was
projected from — the honesty the design's "never a stale snapshot of a
hidden truth" demands in the other direction:

<a name="chunk-options-and-report"></a><sub>[`src/receive.rs`](../../crates/x0k-tangle/src/receive.rs) · `#options-and-report`</sub>

```rust {#options-and-report}
/// Options for a receive-repo run.
#[derive(Debug, Clone, Default)]
pub struct ReceiveOptions {
    /// Patch the workspace working copy (never commits). Refused when
    /// any target path is already dirty.
    pub apply: bool,
    /// Where to write the patch set + `receipt.json`.
    pub out_dir: Option<PathBuf>,
    /// The publication doc; default is the document declaring the clone's
    /// `publication_uri` — under the publications directory, else anywhere
    /// in the collection.
    pub publication: Option<PathBuf>,
    /// Root for the reference projection's temp dir. Must be OUTSIDE
    /// the workspace — a jj workspace auto-tracks new files.
    pub scratch: Option<PathBuf>,
}

/// Outcome of a receive-repo run.
#[derive(Debug, Serialize)]
pub struct ReceiveReport {
    pub publication_uri: String,
    /// The corpus revision the clone's PROVENANCE.json records.
    pub clone_rev: String,
    /// The revision the reference projection was actually built from.
    pub reference_rev: String,
    /// The commit the reference's corpus was materialized at — the
    /// clone's recorded `corpus_commit` when it resolves. `None` when the
    /// reference is the current tree (`rev_exact: false`).
    pub reference_commit: Option<String>,
    /// `reference_rev` is `clone_rev`. When false, the diff includes
    /// the corpus's own drift since the clone was projected, reversed.
    pub rev_exact: bool,
    pub changes: Vec<ReceivedChange>,
    /// Which VCS answered the dirty check before `--apply`
    /// (`jj` | `git` | `none`).
    pub dirty_check: String,
    /// The patch set is in the working copy: `--apply` ran and every
    /// patch landed.
    pub applied: bool,
    /// Why `--apply` did not land, when it was asked and refused or
    /// failed — the same message the run exits with. `None` when it
    /// landed or was not asked.
    pub apply_error: Option<String>,
}

impl ReceiveReport {
    pub fn received(&self) -> usize {
        self.changes.iter().filter(|c| c.class.receivable()).count()
    }
    pub fn refused(&self) -> usize {
        self.changes.iter().filter(|c| c.class.refused()).count()
    }
}
```

Negative space: `receive_repo` never commits, never pushes, never
writes inside the clone, and never writes inside the workspace unless
`apply` is set — and then only through `git apply`, which is all-or-
nothing across the patch set. The reference projection is built in a
temp dir that is removed when the report is returned.

## The pipeline

Five stages, each a chunk below. The order is forced: classification
needs the reference tree (to tell a generated file from a hand-written
one by reading its first line, and to find the sidecar that names its
doc), and the reference needs the provenance (to know which publication
and which revision).

<a name="chunk-receive-repo"></a><sub>[`src/receive.rs`](../../crates/x0k-tangle/src/receive.rs) · `#receive-repo`</sub>

```rust {#receive-repo}
/// Receive the changes in `clone` (a projected repository) as patches
/// against `workspace`. Reports every change; applies the receivable
/// ones only under `opts.apply`.
pub fn receive_repo(clone: &Path, workspace: &Path, opts: &ReceiveOptions) -> Result<ReceiveReport> {
    receive_repo_in(clone, workspace, opts, &Vocabulary::shipped())
}

/// As [`receive_repo`], reading the publication and its members against
/// `vocabulary` — the reference must be projected the way the clone was.
pub fn receive_repo_in(
    clone: &Path,
    workspace: &Path,
    opts: &ReceiveOptions,
    vocabulary: &Vocabulary,
) -> Result<ReceiveReport> {
    let clone = clone
        .canonicalize()
        .with_context(|| format!("clone dir {}", clone.display()))?;
    let workspace = workspace
        .canonicalize()
        .with_context(|| format!("workspace {}", workspace.display()))?;
    let prov = read_provenance(&clone)?;
    // The corpus this receiver reads and the projection it compares against
    // are laid out the same way, and both follow from the workspace's own
    // class registry rather than from anything compiled in here.
    let layout = CorpusLayout::read(&workspace);
    let pub_doc = match &opts.publication {
        Some(p) => p.clone(),
        None => find_publication_doc(&workspace, &layout, &prov.publication_uri, vocabulary)?,
    };
    let reference =
        build_reference(&workspace, &pub_doc, &prov, &clone, opts.scratch.as_deref(), vocabulary)?;
    let mut report = ReceiveReport {
        publication_uri: prov.publication_uri.clone(),
        clone_rev: prov.corpus_rev.clone(),
        reference_rev: reference.rev.clone(),
        reference_commit: reference.commit.clone(),
        rev_exact: reference.exact,
        changes: diff_and_classify(&clone, &reference.dir, &prov, &layout, &reference.crate_roots)?,
        dirty_check: "none".to_string(),
        applied: false,
        apply_error: None,
    };
    tracing::info!(
        changes = report.changes.len(),
        received = report.received(),
        refused = report.refused(),
        rev_exact = report.rev_exact,
        "tangle.receive.classified"
    );
    let patch_dir = match &opts.out_dir {
        Some(d) => d.clone(),
        None => reference.dir.join("patches"),
    };
    write_patch_set(&patch_dir, &mut report)?;
    // The receipt records what `--apply` did, so it is written after it —
    // a refused or failed apply included, before the run exits with it.
    let applied = if opts.apply {
        apply_patch_set(&workspace, &patch_dir, &mut report)
    } else {
        Ok(())
    };
    if let Err(e) = &applied {
        report.apply_error = Some(format!("{e:#}"));
    }
    write_receipt(&patch_dir, &report)?;
    applied?;
    Ok(report)
}
```

## Provenance: what the clone says about itself

`PROVENANCE.json` is the seam the projector left for exactly this
purpose (schema `x0k.provenance/v1`). We read four fields and tolerate
a fifth. `corpus_rev` and `corpus_commit` are the same snapshot named
twice — the revision as the operator's VCS names it (a jj change id, or
a git sha) and the commit it was at — and the reference is built from
the commit, because a change id is mutable and a commit is not. `overlay` is a list the maintainer may add on the public
side — paths, or directory prefixes ending in `/`, that the projector
does not own and the receiver does not carry back. It is the design's
"deliberate divergence" made explicit: a `CONTRIBUTING.md` written for
the forge's audience, a forge-specific funding file.

<a name="chunk-provenance"></a><sub>[`src/receive.rs`](../../crates/x0k-tangle/src/receive.rs) · `#provenance`</sub>

```rust {#provenance}
/// The fields of `PROVENANCE.json` the receiver reads.
#[derive(Debug, Clone, Deserialize)]
pub struct Provenance {
    pub publication_uri: String,
    #[serde(default)]
    pub corpus_rev: String,
    /// The commit `corpus_rev` was at when the projection was taken. Empty
    /// in a clone projected before the projector recorded it.
    #[serde(default)]
    pub corpus_commit: String,
    /// Projected path → the corpus path it was projected from. Identity
    /// for a chapter in the canonical layout; a different path under the
    /// organized layout and for a decision document's projected section.
    #[serde(default)]
    pub path_map: BTreeMap<String, String>,
    /// Projected paths (or `dir/` prefixes) preserved on the public side.
    #[serde(default)]
    pub overlay: Vec<String>,
    /// The crates the projection vendored. Empty for a projection that
    /// ships documents only — which then has no package to look up.
    #[serde(default)]
    pub crates: Vec<String>,
    /// Hand-written files a document's `from=` mirror quotes, carried
    /// outside any crate: projected path → collection path.
    #[serde(default)]
    pub sources: BTreeMap<String, String>,
    /// What each proof test did when the clone was projected, by test id
    /// (`passed` | `failed`). The reference replays these rather than
    /// running the tests again. Empty when the projection skipped its
    /// proofs or published no affordance that names one.
    #[serde(default)]
    pub proofs: BTreeMap<String, String>,
}

impl Provenance {
    /// Projected path → monorepo path, the direction receiving needs and
    /// the direction the projector writes the map in.
    fn canonical_for(&self, projected: &str) -> Option<&str> {
        self.path_map.get(projected).map(String::as_str)
    }
    fn is_overlay(&self, path: &str) -> bool {
        self.overlay.iter().any(|o| {
            if let Some(dir) = o.strip_suffix('/') {
                path == dir || path.starts_with(o) || path.starts_with(&format!("{dir}/"))
            } else {
                path == o
            }
        })
    }
}

fn read_provenance(clone: &Path) -> Result<Provenance> {
    let path = clone.join("PROVENANCE.json");
    let text = std::fs::read_to_string(&path)
        .with_context(|| format!("reading {} — is this a projected repository?", path.display()))?;
    serde_json::from_str(&text).with_context(|| format!("parsing {}", path.display()))
}
```

The provenance names the publication by URI, not by path — the clone
does not carry the publication doc (it is a decision, outside the
disclosed region). Ours are few and live under one directory — flat, or
one directory per publication beside its assets, which is how ours are
kept (`publications/x0k-folio/x0k-folio.md`) — so a shallow scan is the
first resolver. A collection that is not ours files its publication
wherever its authors file things, and no registry says where; the id is
still the identity, so a publication the scan does not find is looked
for across the whole collection by the id its header declares, the walk
the projector finds named documents with
([`collection.md`](collection.md) § "Finding a member by its
id"). Only an id nothing declares — or two documents declaring it —
asks for `--publication`.

<a name="chunk-find-publication-doc"></a><sub>[`src/receive.rs`](../../crates/x0k-tangle/src/receive.rs) · `#find-publication-doc`</sub>

```rust {#find-publication-doc}
/// Find the publication doc whose header subject is `uri` under
/// the corpus's publications directory.
fn find_publication_doc(
    workspace: &Path,
    layout: &CorpusLayout,
    uri: &str,
    vocabulary: &Vocabulary,
) -> Result<PathBuf> {
    let dir = workspace.join(layout.class_dir("publication"));
    // The directory itself and one level of per-publication directories.
    if dir.is_dir() {
        for entry in walkdir::WalkDir::new(&dir).max_depth(2).into_iter().filter_map(|e| e.ok()) {
            let path = entry.into_path();
            if !path.is_file() || path.extension().map(|e| e != "md").unwrap_or(true) {
                continue;
            }
            let Ok(text) = std::fs::read_to_string(&path) else { continue };
            if let Ok((env, _)) = vocabulary.read(&text) {
                if env.id == uri {
                    return Ok(path);
                }
            }
        }
    }
    // Not where ours are kept: wherever the collection declares it.
    let mut declaring = documents_declaring(workspace, vocabulary, &BTreeSet::from([uri.to_string()]))
        .remove(uri)
        .unwrap_or_default();
    match declaring.len() {
        1 => Ok(declaring.remove(0)),
        0 => bail!(
            "no publication with id `{uri}` under {} or anywhere else in the collection \
             (pass --publication)",
            dir.display()
        ),
        n => bail!(
            "{n} documents in the collection declare `{uri}` as their id: {declaring:?} \
             (pass --publication)"
        ),
    }
}
```

## The reference: the projection the contributor started from

The diff has to be taken against *the projection Carol cloned*, not
against the clone's own git history (a contributor may squash, rebase,
or have cloned a tarball) and not against the monorepo directly (the
projector rewrites manifests, so a raw comparison would drown the
signal). The reference is therefore a fresh run of the projector into a
temp dir. The question is which corpus to run it over.

The honest answer is the corpus at the commit the clone records. That
is cheaper than it sounds: the corpus is a jj repo colocated with a git
store, and `git archive` materializes the tree at any commit with no
checkout. It materializes the *whole* tree, and that is deliberate. What
the projector reads is the projector's business and keeps growing: it
asks Cargo where each published package lives, which loads the root
manifest and therefore every workspace member, published or not; it
follows path dependencies; it reads the vocabulary from wherever the
vocabulary crate or the corpus keeps it, and decision documents, concept
pages and icons from wherever the class registry puts them. A list of
those paths kept here went stale exactly that way — it archived crate
*names* as root paths, left the root manifest out, and archived a
vocabulary directory the corpus had moved away from, so receiving our own
publication refused before it diffed anything. The tree the projection
was made from is the one input that cannot drift from the projector; for
the monorepo it is about 16,500 files and well under a second to
materialize.

When that works, the reference is exact. When it cannot (no VCS, a
commit the store no longer holds, a workspace that is not git-backed),
we project the *current* tree and the report says so: `rev_exact:
false`, with both revisions named. The consequence of a skewed reference
is stated, not hidden — the diff then contains the corpus's own edits
since the projection, reversed, and the maintainer reads it as "rebase
the contribution" rather than as Carol's intent.

The commit comes first, the revision second. In a jj workspace the
projector's `corpus_rev` is the *change id* of the snapshot, and a
change id names a mutable change — jj resolves it to whatever that
change contains now, so an amended or rebased change would hand the
receiver a tree Carol never saw. `corpus_commit` is the fixed point the
projector records beside it. The revision is tried only when the commit
is absent (a clone projected before the commit was recorded) or no
longer resolves.

The reference also answers one question the classifier cannot answer
from the clone: where each vendored crate lives in the tree. A projection
flattens every crate to `<name>/`; the corpus keeps it wherever its
workspace says (`substrate/crates/production/x0k-folio` in ours). The
roots are read from the same tree the reference was projected from, the
same way the projector read them.

What the reference does *not* do is run the proofs. A projection runs the
tests its published affordances name and draws each row's status mark
from what they did; the reference needs those marks, since they are bytes
of the tree Carol cloned, but not the run behind them. Running them made
every receive a `cargo test` of the published crates in a temp dir — for
x0k-folio, measured on 2026-10-04, 36–41 s and 1.6 GB of peak scratch per
receive, against 4–5 s and 0.4 GB (the materialized corpus) without — to
reproduce an answer the clone already carries: its `PROVENANCE.json` records every
proof's outcome under `proofs`. So the reference replays that record
(`Proofs::Recorded`, [`region-repo.md`](region-repo.md) § "Contract") and
the projector runs nothing. A clone whose projection skipped its proofs
records none, and its reference draws the same `not run` marks; one whose
record a contributor edited gets marks to match, which only moves
projection-owned scaffolding the receiver never carries back.

<a name="chunk-reference-proofs"></a><sub>[`src/receive.rs`](../../crates/x0k-tangle/src/receive.rs) · `#reference-proofs`</sub>

```rust {#reference-proofs}
/// How the reference treats the proofs: replay the outcomes the clone's
/// projection recorded, never run them.
fn reference_proofs(prov: &Provenance) -> Proofs {
    Proofs::Recorded(
        prov.proofs
            .iter()
            .filter_map(|(id, outcome)| ProofOutcome::parse(outcome).map(|o| (id.clone(), o)))
            .collect(),
    )
}
```

<a name="chunk-reference"></a><sub>[`src/receive.rs`](../../crates/x0k-tangle/src/receive.rs) · `#reference`</sub>

```rust {#reference}
/// The reference projection: a temp dir holding a fresh projection of
/// the publication, which corpus revision it came from, and where each
/// package it vendored lives in that corpus.
struct Reference {
    dir: PathBuf,
    rev: String,
    /// The commit the corpus was materialized at; `None` when the
    /// reference is the current tree.
    commit: Option<String>,
    exact: bool,
    /// Package name → its root in the corpus the reference was projected
    /// from, relative to that corpus.
    crate_roots: BTreeMap<String, PathBuf>,
    _tmp: tempfile::TempDir,
}

fn build_reference(
    workspace: &Path,
    pub_doc: &Path,
    prov: &Provenance,
    clone: &Path,
    scratch: Option<&Path>,
    vocabulary: &Vocabulary,
) -> Result<Reference> {
    let tmp = match scratch {
        Some(s) => tempfile::Builder::new().prefix("x0k-receive-").tempdir_in(s)?,
        None => tempfile::Builder::new().prefix("x0k-receive-").tempdir()?,
    };
    // The commit is immutable; the revision may be a jj change id that
    // has moved since. The revision is the fallback for a clone that
    // records no commit.
    let materialized = [prov.corpus_commit.as_str(), prov.corpus_rev.as_str()]
        .into_iter()
        .find_map(|rev| materialize_corpus_at(workspace, rev, tmp.path()));
    let (source_root, source_doc, commit) = match materialized {
        Some((root, commit)) => {
            let rel = pub_doc.strip_prefix(workspace).unwrap_or(pub_doc);
            let doc = root.join(rel);
            let doc = if doc.is_file() { doc } else { pub_doc.to_path_buf() };
            (root, doc, Some(commit))
        }
        None => (workspace.to_path_buf(), pub_doc.to_path_buf(), None),
    };
    let exact = commit.is_some();
    let rev = if exact {
        prov.corpus_rev.clone()
    } else {
        current_rev(workspace)
    };
    tracing::info!(exact, rev = %rev, commit = commit.as_deref().unwrap_or(""), "tangle.receive.reference");
    let dir = tmp.path().join("reference");
    let opts = RepoProjectOptions {
        license: None,
        git_init: false,
        // The reference is private scratch, not a disclosure; a guard
        // that fails on today's tree must not block receiving.
        allow_dirty: true,
        emit_github: clone.join(".github/workflows").is_dir(),
    };
    project_publication_repo_in(&source_doc, &dir, &source_root, &opts, &reference_proofs(prov), vocabulary)
        .context("projecting the reference")?;
    // A projection of documents only vendored no package, and its
    // collection need not be a Cargo workspace at all.
    let crate_roots = if prov.crates.is_empty() {
        BTreeMap::new()
    } else {
        source_package_roots(&source_root).context("reading where the reference's packages live")?
    };
    Ok(Reference { dir, rev, commit, exact, crate_roots, _tmp: tmp })
}
```

Materializing the corpus at a revision is two subprocesses: resolve
the revision to a git commit, then `git archive … | tar -x`. The
revision is a commit id, a jj change id, or a git ref, so we try jj
first (`--ignore-working-copy`, so the lookup neither snapshots nor
locks the operator's working copy) and fall back to git. Any failure
anywhere returns `None`; the caller tries the next candidate and then
degrades to the current tree rather than erroring, because a skewed
reference with an honest label is more useful than no report.

<a name="chunk-materialize-corpus"></a><sub>[`src/receive.rs`](../../crates/x0k-tangle/src/receive.rs) · `#materialize-corpus`</sub>

```rust {#materialize-corpus}
/// Materialize the whole corpus tree at `rev` under `<scratch>/corpus`;
/// the root and the commit `rev` resolved to. `None` when the revision
/// cannot be resolved or archived.
fn materialize_corpus_at(workspace: &Path, rev: &str, scratch: &Path) -> Option<(PathBuf, String)> {
    if rev.is_empty() {
        return None;
    }
    let (git_dir, commit) = resolve_commit(workspace, rev)?;
    let root = scratch.join("corpus");
    // A candidate that failed half-way leaves files behind; the next one
    // starts from an empty directory.
    if root.exists() {
        std::fs::remove_dir_all(&root).ok()?;
    }
    std::fs::create_dir_all(&root).ok()?;
    let mut archive = std::process::Command::new("git")
        .arg("--git-dir")
        .arg(&git_dir)
        .args(["archive", "--format=tar", &commit])
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::null())
        .spawn()
        .ok()?;
    let status = std::process::Command::new("tar")
        .args(["-x", "-C"])
        .arg(&root)
        .stdin(archive.stdout.take()?)
        .status()
        .ok()?;
    if !archive.wait().ok()?.success() || !status.success() {
        return None;
    }
    Some((root, commit))
}
```

<a name="chunk-resolve-commit"></a><sub>[`src/receive.rs`](../../crates/x0k-tangle/src/receive.rs) · `#resolve-commit`</sub>

```rust {#resolve-commit}
/// `(git dir, commit sha)` for `rev`, via jj (change id → commit) or
/// git (sha or ref). `None` when neither resolves it.
fn resolve_commit(workspace: &Path, rev: &str) -> Option<(PathBuf, String)> {
    if let Some(git_dir) = vcs_query(workspace, &["jj", "--ignore-working-copy", "git", "root"]) {
        if let Some(commit) = vcs_query(
            workspace,
            &["jj", "--ignore-working-copy", "log", "-r", rev, "--no-graph", "-T", "commit_id"],
        ) {
            return Some((PathBuf::from(git_dir), commit));
        }
    }
    let git_dir = vcs_query(workspace, &["git", "rev-parse", "--absolute-git-dir"])?;
    let commit = vcs_query(workspace, &["git", "rev-parse", "--verify", &format!("{rev}^{{commit}}")])?;
    Some((PathBuf::from(git_dir), commit))
}

/// The workspace's current revision, in the same form the projector
/// records (jj change id, else git sha, else empty).
fn current_rev(workspace: &Path) -> String {
    vcs_query(workspace, &["jj", "--ignore-working-copy", "log", "-r", "@", "--no-graph", "-T", "change_id"])
        .or_else(|| vcs_query(workspace, &["git", "rev-parse", "HEAD"]))
        .unwrap_or_default()
}

/// Run one VCS command in `workspace`; its trimmed stdout on success
/// and non-empty output, else `None`.
fn vcs_query(workspace: &Path, args: &[&str]) -> Option<String> {
    let out = std::process::Command::new(args[0])
        .current_dir(workspace)
        .args(&args[1..])
        .stderr(std::process::Stdio::null())
        .output()
        .ok()?;
    out.status
        .success()
        .then(|| String::from_utf8_lossy(&out.stdout).trim().to_string())
        .filter(|s| !s.is_empty())
}
```

## Diff and classify

Both trees are walked into path sets, skipping `.git`, `target`, and
`PROVENANCE.json` — the last because it is the receiver's *input*, and
the reference's copy can never match the clone's (its `corpus_rev`
names the projection that produced it, not the one being received).
A path is *changed* when it is missing from one side or its bytes
differ. Each changed path is then classified with the provenance and
the reference tree in hand, and a hand-written file's target is moved
from the crate's projected directory to its root in the corpus (a crate
at `substrate/crates/production/x0k-folio` is `x0k-folio/` in the clone,
or `crates/x0k-folio/` under the organized layout):

<a name="chunk-diff-and-classify"></a><sub>[`src/receive.rs`](../../crates/x0k-tangle/src/receive.rs) · `#diff-and-classify`</sub>

```rust {#diff-and-classify}
fn diff_and_classify(
    clone: &Path,
    reference: &Path,
    prov: &Provenance,
    layout: &CorpusLayout,
    crate_roots: &BTreeMap<String, PathBuf>,
) -> Result<Vec<ReceivedChange>> {
    let clone_files = collect_files(clone)?;
    let ref_files = collect_files(reference)?;
    let mut changes = Vec::new();
    for path in clone_files.union(&ref_files) {
        let new = read_opt(&clone.join(path))?;
        let old = read_opt(&reference.join(path))?;
        if new == old {
            continue;
        }
        let kind = match (&old, &new) {
            (None, Some(_)) => "added",
            (Some(_), None) => "deleted",
            _ => "modified",
        };
        let (class, target, produced_by) = classify(path, &old, &new, reference, prov, layout);
        // The classifier speaks projection paths; a crate's source lives
        // at its own root in the corpus, not at `<name>/`.
        let target = match class {
            Class::Source => target.map(|t| source_target(&t, reference, crate_roots)),
            _ => target,
        };
        // A literate chapter crossed woven for the forge; the contributor
        // edited the woven text, and the corpus holds the source. Both
        // sides are unwoven before the diff, so the patch is against the
        // document the target names.
        let (old, new) = if matches!(class, Class::Literate) {
            (old.map(|t| unweave_chapter(&t)), new.map(|t| unweave_chapter(&t)))
        } else {
            (old, new)
        };
        let patch = if class.receivable() {
            let target = target.as_deref().unwrap_or(path);
            Some(unified_patch(target, old.as_deref(), new.as_deref()))
        } else {
            None
        };
        changes.push(ReceivedChange {
            path: path.clone(),
            class,
            kind: kind.to_string(),
            target,
            patch,
            produced_by,
            patch_file: None,
        });
    }
    Ok(changes)
}

/// The corpus path of a hand-written file the clone holds inside a
/// vendored crate: the crate's root in the corpus joined with the path
/// inside it. The projection names a crate's directory after its package
/// (`<name>/`, or `crates/<name>/` organized). A crate the reference does
/// not know keeps its projected path.
fn source_target(
    projected: &str,
    reference: &Path,
    crate_roots: &BTreeMap<String, PathBuf>,
) -> String {
    let Some((dir, rest)) = projected_crate(reference, projected) else {
        return projected.to_string();
    };
    let name = dir.rsplit('/').next().unwrap_or(dir);
    match crate_roots.get(name) {
        Some(root) => root.join(rest).to_string_lossy().to_string(),
        None => projected.to_string(),
    }
}

fn collect_files(root: &Path) -> Result<BTreeSet<String>> {
    let mut out = BTreeSet::new();
    for entry in walkdir::WalkDir::new(root)
        .into_iter()
        .filter_entry(|e| {
            let n = e.file_name().to_string_lossy();
            !(e.depth() >= 1 && (n == ".git" || n == "target"))
                && !(e.depth() == 1 && n == "PROVENANCE.json")
        })
        .filter_map(|e| e.ok())
    {
        if entry.file_type().is_file() {
            let rel = entry.path().strip_prefix(root).unwrap();
            out.insert(rel.to_string_lossy().to_string());
        }
    }
    Ok(out)
}

/// File contents as text; `None` when absent. Non-UTF-8 content is
/// read lossily — the receiver is a text-patch tool and says so.
fn read_opt(path: &Path) -> Result<Option<String>> {
    match std::fs::read(path) {
        Ok(bytes) => Ok(Some(String::from_utf8_lossy(&bytes).into_owned())),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(e) => Err(e).with_context(|| format!("reading {}", path.display())),
    }
}
```

A literate chapter in the clone is the *woven* chapter — the projector
wove it for the forge ([`region-gfm.md`](region-gfm.md)), and the
contributor edited that. The corpus holds the source, and the patch has
to apply there, so both the reference's copy and the clone's are unwoven
before they are diffed; the weave inverts line for line, which is the
invariant that makes this a routing step and not a merge.

The classifier is a short ladder, ordered so the more specific claim
wins. Overlay first — a maintainer's declaration outranks every
inference. Then the literate docs: a path in `path_map` maps back
through it; a *new* `.md` under the corpus's implementation root (Carol
contributing a literate document for a module that had none — the
on-ramp the publication doc invites) is receivable at the same path.
The map is keyed by the projected path, which is how the projector
writes it; under the organized layout the two sides differ, so reading
it the other way round would miss every chapter. A hand-written file a
mirror quotes, carried outside any crate, maps back through `sources`
the same way and is source. Sidecars and crate
manifests are projector output. Anything left under a published crate —
found as the outermost directory holding a `Cargo.toml` in the
reference, so `<name>/` and `crates/<name>/` both count — is source, unless its first line carries the
`@generated` marker — the reference copy's first line when it exists,
the clone's when the file is new — in which case it is refused with
its origin. Outside a crate the same refusal holds for a file some
shipped chapter tangles — the reference's sidecars name it — so an edit
to a chapter's `.py` output is refused with the chapter it came from
rather than shrugged off as scaffolding. Everything else is scaffolding.

<a name="chunk-classify"></a><sub>[`src/receive.rs`](../../crates/x0k-tangle/src/receive.rs) · `#classify`</sub>

```rust {#classify}
/// `(class, monorepo target, generated origin)` for one changed path.
fn classify(
    path: &str,
    old: &Option<String>,
    new: &Option<String>,
    reference: &Path,
    prov: &Provenance,
    layout: &CorpusLayout,
) -> (Class, Option<String>, Option<GeneratedOrigin>) {
    if prov.is_overlay(path) {
        return (Class::ProjectionLocal, None, None);
    }
    if let Some(canonical) = prov.canonical_for(path) {
        return (Class::Literate, Some(canonical.to_string()), None);
    }
    if let Some(source) = prov.sources.get(path) {
        return (Class::Source, Some(source.clone()), None);
    }
    let literate_prefix = format!("{}/", layout.implementation_root().display());
    if path.starts_with(&literate_prefix) && path.ends_with(".md") {
        return (Class::Literate, Some(path.to_string()), None);
    }
    if path.ends_with(".tangle-map.json") {
        return (Class::ProjectionOwned, None, None);
    }
    let Some((_, crate_rel)) = projected_crate(reference, path) else {
        // A chapter's output outside any crate: some sidecar names it.
        if doc_for_output(reference, path).is_some() {
            let origin = generated_origin(reference, prov, path, old.as_deref(), new.as_deref());
            return (Class::Generated, None, Some(origin));
        }
        return (Class::ProjectionOwned, None, None);
    };
    if crate_rel == "Cargo.toml" || crate_rel.starts_with("ontology/modules/") {
        // Rewritten (license, versions) or projected from `ontology/modules/`
        // (versionIRI stamped) at projection time; the monorepo original is
        // the edit surface.
        return (Class::ProjectionOwned, None, None);
    }
    let first_line = old
        .as_deref()
        .or(new.as_deref())
        .and_then(|t| t.lines().next())
        .unwrap_or("");
    if first_line.contains("@generated") {
        let origin = generated_origin(reference, prov, path, old.as_deref(), new.as_deref());
        return (Class::Generated, None, Some(origin));
    }
    (Class::Source, Some(path.to_string()), None)
}

/// The vendored crate `path` sits in, as `(crate dir, path inside it)`.
/// The crate dir is the outermost directory below the projection root that
/// holds a `Cargo.toml` in the reference: `<name>/` in the canonical
/// layout, `crates/<name>/` in the organized one. Outermost, because a
/// crate's own test fixtures may carry manifests of their own.
fn projected_crate<'a>(reference: &Path, path: &'a str) -> Option<(&'a str, &'a str)> {
    path.match_indices('/').find_map(|(i, _)| {
        let dir = &path[..i];
        reference
            .join(dir)
            .join("Cargo.toml")
            .is_file()
            .then(|| (dir, &path[i + 1..]))
    })
}
```

## Naming the chunk

A refusal that only says "generated, go away" is the fork-tending
experience the design exists to prevent. The doc is cheap to name: the
projection carries every literate doc's `<stem>.tangle-map.json`
sidecar, and the sidecar's `outputs[].path` names the tangled files.
The sidecar names the doc at its *projected* path; the refusal names it
at its corpus path through `path_map`, because under the organized
layout the two differ (`implementation/folio/colophon.md` in the clone
is `corpora/x0k/implementation/folio/colophon.md` in the corpus) and
the corpus path is where the change has to be made.
The chunk is a little more work and is done best-effort: we parse the
doc, expand every named chunk, and for each line the contributor
touched on the reference side (a deleted line, or for an insertion the
nearest reference line above it, deleted or kept) pick the *innermost*
chunk whose expansion contains it
— innermost meaning smallest, since an outline chunk's expansion
contains everything beneath it. A line that appears in no chunk (the
generated header) contributes nothing; the doc alone is still named.

<a name="chunk-generated-origin"></a><sub>[`src/receive.rs`](../../crates/x0k-tangle/src/receive.rs) · `#generated-origin`</sub>

```rust {#generated-origin}
/// The doc (and, best-effort, the chunks) that produce `output` in the
/// reference projection. The doc is found at its projected path and named
/// at its corpus path, which differ under the organized layout.
fn generated_origin(
    reference: &Path,
    prov: &Provenance,
    output: &str,
    old: Option<&str>,
    new: Option<&str>,
) -> GeneratedOrigin {
    let Some(projected) =
        doc_for_output(reference, output).or_else(|| doc_from_header(old.or(new)?))
    else {
        return GeneratedOrigin {
            doc: "(unknown — no sidecar names this output)".to_string(),
            chunks: Vec::new(),
        };
    };
    let chunks = match (old, new, std::fs::read_to_string(reference.join(&projected))) {
        (Some(old), Some(new), Ok(text)) => touched_chunks(&text, old, new),
        _ => Vec::new(),
    };
    let doc = prov.canonical_for(&projected).map(str::to_string).unwrap_or(projected);
    GeneratedOrigin { doc, chunks }
}

/// Walk the reference's sidecars for one whose outputs name `output`.
/// The whole projection is walked: its chapters sit under the corpus's
/// implementation root in the canonical layout and under
/// `implementation/` in the organized one.
fn doc_for_output(reference: &Path, output: &str) -> Option<String> {
    let walk = walkdir::WalkDir::new(reference).into_iter().filter_entry(|e| {
        let n = e.file_name().to_string_lossy();
        !(e.depth() >= 1 && (n == ".git" || n == "target"))
    });
    for entry in walk.filter_map(|e| e.ok()) {
        let name = entry.file_name().to_string_lossy();
        if !name.ends_with(".tangle-map.json") {
            continue;
        }
        let Ok(text) = std::fs::read_to_string(entry.path()) else { continue };
        let Ok(json) = serde_json::from_str::<serde_json::Value>(&text) else { continue };
        let names_it = json["pipelines"]
            .as_array()
            .into_iter()
            .flatten()
            .flat_map(|p| p["outputs"].as_array().into_iter().flatten())
            .any(|o| o["path"].as_str() == Some(output));
        if names_it {
            let doc = entry.path().with_extension("").with_extension("md");
            return Some(doc.strip_prefix(reference).unwrap_or(&doc).to_string_lossy().to_string());
        }
    }
    None
}

/// Fallback: the `@generated by x0k-tangle … from <doc>` header names
/// the doc directly.
fn doc_from_header(text: &str) -> Option<String> {
    let first = text.lines().next()?;
    let rest = first.split(" from ").nth(1)?;
    Some(rest.split([' ', '\u{2014}']).next()?.trim().to_string())
}
```

<a name="chunk-touched-chunks"></a><sub>[`src/receive.rs`](../../crates/x0k-tangle/src/receive.rs) · `#touched-chunks`</sub>

```rust {#touched-chunks}
/// Innermost chunk per touched reference line, deduplicated.
fn touched_chunks(doc_text: &str, old: &str, new: &str) -> Vec<String> {
    let Ok(doc) = parse_document(doc_text) else { return Vec::new() };
    let expansions: Vec<(String, Vec<String>)> = doc
        .chunk_order
        .iter()
        .filter(|n| doc.chunk(n).map(|c| !c.is_media && !c.is_from_ref()).unwrap_or(false))
        .filter_map(|n| expand_chunk(&doc, n).ok().map(|e| (n.clone(), e.lines().map(|l| l.trim().to_string()).collect())))
        .collect();
    let mut touched: BTreeSet<String> = BTreeSet::new();
    let old_lines: Vec<&str> = old.lines().collect();
    let diff = similar::TextDiff::from_lines(old, new);
    let mut last_old: Option<usize> = None;
    for change in diff.iter_all_changes() {
        let line_idx = match change.tag() {
            similar::ChangeTag::Equal => {
                last_old = change.old_index();
                continue;
            }
            similar::ChangeTag::Delete => {
                last_old = change.old_index();
                last_old
            }
            similar::ChangeTag::Insert => last_old,
        };
        let Some(idx) = line_idx else { continue };
        let line = old_lines.get(idx).map(|l| l.trim()).unwrap_or("");
        if line.is_empty() {
            continue;
        }
        let innermost = expansions
            .iter()
            .filter(|(_, lines)| lines.iter().any(|l| l == line))
            .min_by_key(|(_, lines)| lines.len());
        if let Some((name, _)) = innermost {
            touched.insert(name.clone());
        }
    }
    touched.into_iter().collect()
}
```

## Patches

A receivable change becomes one unified diff against its monorepo
target, in the `a/`–`b/` form `git apply` strips by default, with
`/dev/null` on the absent side for additions and deletions. The diff is
computed in-process (`similar`) rather than by shelling out, so the
report is complete even where `git` is not the operator's tool:

<a name="chunk-unified-patch"></a><sub>[`src/receive.rs`](../../crates/x0k-tangle/src/receive.rs) · `#unified-patch`</sub>

```rust {#unified-patch}
/// Unified diff of `old` → `new` addressed at `target`.
fn unified_patch(target: &str, old: Option<&str>, new: Option<&str>) -> String {
    let a = if old.is_some() { format!("a/{target}") } else { "/dev/null".to_string() };
    let b = if new.is_some() { format!("b/{target}") } else { "/dev/null".to_string() };
    let diff = similar::TextDiff::from_lines(old.unwrap_or(""), new.unwrap_or(""));
    diff.unified_diff().context_radius(3).header(&a, &b).to_string()
}
```

The patch set on disk is one file per receivable change plus a
`receipt.json` carrying the whole report — the artifact a CI check or
a reviewer reads. File names are numbered and derived from the target
so a directory listing reads as a table of contents:

<a name="chunk-write-patch-set"></a><sub>[`src/receive.rs`](../../crates/x0k-tangle/src/receive.rs) · `#write-patch-set`</sub>

```rust {#write-patch-set}
fn write_patch_set(dir: &Path, report: &mut ReceiveReport) -> Result<()> {
    std::fs::create_dir_all(dir).with_context(|| format!("creating {}", dir.display()))?;
    let mut n = 0usize;
    for change in report.changes.iter_mut() {
        let Some(patch) = &change.patch else { continue };
        n += 1;
        let target = change.target.as_deref().unwrap_or(&change.path);
        let name = format!("{n:04}-{}.patch", target.replace('/', "__"));
        std::fs::write(dir.join(&name), patch)?;
        change.patch_file = Some(name);
    }
    Ok(())
}
```

The receipt is written last, after `--apply` has run or refused, because
`applied` is a fact about the working copy and only the apply knows it. A
receipt written beside the patches, before the apply, would say `applied:
false` on every run — including the ones that patched the tree. A refused
or failed apply is recorded as well, with the message the run then exits
with, so a CI check reading the receipt sees the same verdict the exit
code carries:

<a name="chunk-write-receipt"></a><sub>[`src/receive.rs`](../../crates/x0k-tangle/src/receive.rs) · `#write-receipt`</sub>

```rust {#write-receipt}
fn write_receipt(dir: &Path, report: &ReceiveReport) -> Result<()> {
    std::fs::write(
        dir.join("receipt.json"),
        serde_json::to_string_pretty(&serde_json::json!({
            "schema": "x0k.receipt/v1",
            "report": report,
        }))?,
    )
    .with_context(|| format!("writing {}", dir.join("receipt.json").display()))
}
```

## Applying

`--apply` patches the working copy and stops; the commit is the
operator's, under an intent, after re-tangling and review. Two refusals
guard it. A target that is already dirty in the working copy is refused
outright — a patch over uncommitted work is unreviewable — and the
dirty check asks jj first (the monorepo's VCS; the `@` commit *is* the
uncommitted state, and it must snapshot to see it), then git, and
records which answered. Then `git apply --check` over the whole set
precedes the real `git apply`; the set lands entirely or not at all.

<a name="chunk-apply-patch-set"></a><sub>[`src/receive.rs`](../../crates/x0k-tangle/src/receive.rs) · `#apply-patch-set`</sub>

```rust {#apply-patch-set}
fn apply_patch_set(workspace: &Path, patch_dir: &Path, report: &mut ReceiveReport) -> Result<()> {
    let targets: Vec<String> = report
        .changes
        .iter()
        .filter(|c| c.patch_file.is_some())
        .filter_map(|c| c.target.clone())
        .collect();
    if targets.is_empty() {
        return Ok(());
    }
    let (how, dirty) = dirty_targets(workspace, &targets);
    report.dirty_check = how.to_string();
    if !dirty.is_empty() {
        bail!(
            "refusing --apply: working copy already has changes to {} (per {how}) — commit or restore first",
            dirty.join(", ")
        );
    }
    let files: Vec<PathBuf> = report
        .changes
        .iter()
        .filter_map(|c| c.patch_file.as_ref())
        .map(|f| patch_dir.join(f))
        .collect();
    for check in [true, false] {
        let mut cmd = std::process::Command::new("git");
        cmd.current_dir(workspace).arg("apply");
        if check {
            cmd.arg("--check");
        }
        let out = cmd.args(&files).output().context("running git apply")?;
        if !out.status.success() {
            bail!(
                "git apply{} failed:\n{}",
                if check { " --check" } else { "" },
                String::from_utf8_lossy(&out.stderr)
            );
        }
    }
    report.applied = true;
    tracing::info!(patches = files.len(), "tangle.receive.applied");
    Ok(())
}

/// `(which VCS answered, targets with uncommitted changes)`.
fn dirty_targets(workspace: &Path, targets: &[String]) -> (&'static str, Vec<String>) {
    let listing = |args: &[&str]| -> Option<String> {
        let out = std::process::Command::new(args[0])
            .current_dir(workspace)
            .args(&args[1..])
            .stderr(std::process::Stdio::null())
            .output()
            .ok()?;
        out.status.success().then(|| String::from_utf8_lossy(&out.stdout).to_string())
    };
    let (how, changed) = if let Some(s) = listing(&["jj", "diff", "--name-only"]) {
        ("jj", s.lines().map(|l| l.trim().to_string()).collect::<Vec<_>>())
    } else if let Some(s) = listing(&["git", "status", "--porcelain", "--untracked-files=all"]) {
        ("git", s.lines().filter(|l| l.len() > 3).map(|l| l[3..].trim().to_string()).collect())
    } else {
        return ("none", Vec::new());
    };
    let dirty = targets.iter().filter(|t| changed.iter().any(|c| c == *t)).cloned().collect();
    (how, dirty)
}
```

## Tests

The classification ladder and the patch shape are pinned here without
a projection on disk; the end-to-end path — a real projection, an edit
in the clone, the receive, the apply and its refusal — follows below in
`tests/receive_repo.rs`, over a synthetic workspace with a git history
so the exact-reference path is exercised too.

<a name="chunk-tests"></a><sub>[`src/receive.rs`](../../crates/x0k-tangle/src/receive.rs) · `#tests`</sub>

```rust {#tests}
#[cfg(test)]
mod tests {
    use super::*;

    fn prov() -> Provenance {
        Provenance {
            publication_uri: "x0k:publication/demo".into(),
            corpus_rev: String::new(),
            corpus_commit: String::new(),
            path_map: [(
                "corpora/x0k/implementation/folio/colophon.md".to_string(),
                "corpora/x0k/implementation/folio/colophon.md".to_string(),
            )]
            .into_iter()
            .collect(),
            overlay: vec!["CONTRIBUTING.md".into(), "docs/".into()],
            crates: vec!["x0k-folio".into()],
            sources: BTreeMap::new(),
            proofs: BTreeMap::new(),
        }
    }

    #[test]
    fn overlay_matches_exact_paths_and_dir_prefixes() {
        let p = prov();
        assert!(p.is_overlay("CONTRIBUTING.md"));
        assert!(p.is_overlay("docs/intro.md"));
        assert!(!p.is_overlay("docs-other/x.md"));
        assert!(!p.is_overlay("README.md"));
    }

    #[test]
    fn classification_ladder() {
        let tmp = tempfile::tempdir().unwrap();
        let reference = tmp.path();
        std::fs::create_dir_all(reference.join("x0k-folio/src")).unwrap();
        std::fs::write(reference.join("x0k-folio/Cargo.toml"), "[package]\n").unwrap();
        let p = prov();
        let gen = Some("// @generated by x0k-tangle from corpora/x0k/implementation/folio/colophon.md — DO NOT EDIT.\nfn a() {}\n".to_string());
        let hand = Some("fn b() {}\n".to_string());
        let cases: Vec<(&str, &Option<String>, Class, Option<&str>)> = vec![
            ("CONTRIBUTING.md", &hand, Class::ProjectionLocal, None),
            ("corpora/x0k/implementation/folio/colophon.md", &hand, Class::Literate, Some("corpora/x0k/implementation/folio/colophon.md")),
            ("knowledge/implementation/folio/new-chapter.md", &hand, Class::Literate, Some("knowledge/implementation/folio/new-chapter.md")),
            ("knowledge/implementation/folio/colophon.tangle-map.json", &hand, Class::ProjectionOwned, None),
            ("x0k-folio/Cargo.toml", &hand, Class::ProjectionOwned, None),
            ("x0k-folio/src/colophon.rs", &gen, Class::Generated, None),
            ("x0k-folio/src/hand.rs", &hand, Class::Source, Some("x0k-folio/src/hand.rs")),
            ("README.md", &hand, Class::ProjectionOwned, None),
            ("tools/ci", &hand, Class::ProjectionOwned, None),
        ];
        for (path, old, want, target) in cases {
            let (class, got_target, origin) =
                classify(path, old, &hand, reference, &p, &CorpusLayout::default());
            assert_eq!(class, want, "{path}");
            assert_eq!(got_target.as_deref(), target, "{path}");
            if class == Class::Generated {
                let origin = origin.expect("generated names its origin");
                assert_eq!(origin.doc, "corpora/x0k/implementation/folio/colophon.md");
            }
        }
    }

    /// A projection of documents carries hand-written files outside any
    /// crate — a mirror's source — and generated ones a chapter tangles
    /// there. The first is source, routed back through `sources`; the
    /// second is refused naming its chapter; a file neither names is
    /// scaffolding, as it always was.
    #[test]
    fn files_outside_any_crate_are_sources_or_a_chapters_output() {
        let tmp = tempfile::tempdir().unwrap();
        let reference = tmp.path();
        std::fs::create_dir_all(reference.join("chapters")).unwrap();
        std::fs::write(
            reference.join("chapters/gen.tangle-map.json"),
            r#"{"source":"chapters/gen.md","source_hash":"x","pipelines":[{"kind":"identity-tangle","config_hash":"y","outputs":[{"path":"pkg/gen.py","hash":"z"}]}]}"#,
        )
        .unwrap();
        let mut p = prov();
        p.sources.insert("pkg/mod.py".into(), "pkg/mod.py".into());
        p.path_map.insert("chapters/gen.md".into(), "chapters/gen.md".into());
        let hand = Some("def area(w, h):\n    return w * h\n".to_string());
        let gen = Some("# @generated by x0k-tangle (pipeline: identity-tangle) from chapters/gen.md — DO NOT EDIT.\n".to_string());
        let layout = CorpusLayout::default();
        let (class, target, _) = classify("pkg/mod.py", &hand, &hand, reference, &p, &layout);
        assert_eq!((class, target.as_deref()), (Class::Source, Some("pkg/mod.py")));
        let (class, _, origin) = classify("pkg/gen.py", &gen, &gen, reference, &p, &layout);
        assert_eq!(class, Class::Generated);
        assert_eq!(origin.expect("names its chapter").doc, "chapters/gen.md");
        let (class, _, _) = classify("pkg/other.py", &hand, &hand, reference, &p, &layout);
        assert_eq!(class, Class::ProjectionOwned);
    }

    /// A collection that is not ours keeps its publication wherever it
    /// likes: the receiver finds it by the id the clone records, and an id
    /// nothing declares still asks for `--publication`.
    #[test]
    fn an_outside_publication_is_found_by_its_id() {
        let tmp = tempfile::tempdir().unwrap();
        let ws = tmp.path();
        std::fs::create_dir_all(ws.join("pubs")).unwrap();
        std::fs::write(
            ws.join("pubs/field-notes.md"),
            "# Field notes\n\n```turtle folio:document\npublication:field-notes a x0k:Publication ;\n    x0k:status \"proposed\" .\n```\n",
        )
        .unwrap();
        let vocabulary = Vocabulary::shipped();
        let layout = CorpusLayout::default();
        let found = find_publication_doc(ws, &layout, "x0k:publication/field-notes", &vocabulary)
            .expect("found by id");
        assert!(found.ends_with("pubs/field-notes.md"), "{}", found.display());
        let err = find_publication_doc(ws, &layout, "x0k:publication/elsewhere", &vocabulary)
            .expect_err("nothing declares it");
        assert!(format!("{err:#}").contains("pass --publication"), "{err:#}");
    }

    #[test]
    fn the_reference_replays_the_clones_proofs_and_runs_none() {
        let mut p = prov();
        p.proofs = [
            ("x0k:test/x0k-folio/tests/colophon.rs::reads_a_header", "passed"),
            ("x0k:test/x0k-folio/src/lib.rs::a_word_nobody_wrote", "maybe"),
        ]
        .into_iter()
        .map(|(id, o)| (id.to_string(), o.to_string()))
        .collect();
        match reference_proofs(&p) {
            Proofs::Recorded(outcomes) => {
                assert_eq!(
                    outcomes.into_iter().collect::<Vec<_>>(),
                    vec![("x0k:test/x0k-folio/tests/colophon.rs::reads_a_header".to_string(), ProofOutcome::Passed)],
                    "the recorded outcome replays; a word that is no outcome is dropped"
                );
            }
            other => panic!("the reference must replay the record, never run the proofs: {other:?}"),
        }
        // A clone that recorded nothing replays nothing — still no run.
        assert!(matches!(reference_proofs(&prov()), Proofs::Recorded(o) if o.is_empty()));
    }

    #[test]
    fn patch_headers_follow_git_conventions() {
        let modified = unified_patch("d/f.md", Some("a\nb\n"), Some("a\nc\n"));
        assert!(modified.starts_with("--- a/d/f.md\n+++ b/d/f.md\n@@ -1,2 +1,2 @@\n"));
        let added = unified_patch("d/f.md", None, Some("x\n"));
        assert!(added.starts_with("--- /dev/null\n+++ b/d/f.md\n@@ -0,0 +1 @@\n"));
        let deleted = unified_patch("d/f.md", Some("x\n"), None);
        assert!(deleted.starts_with("--- a/d/f.md\n+++ /dev/null\n@@ -1 +0,0 @@\n"));
    }

    #[test]
    fn touched_chunks_names_the_innermost_chunk() {
        let doc = "```turtle folio:document\nimplementation:t\\/d a x0k:Implementation ;\n    folio:tangleCrate \"c\" ;\n    folio:tangleRoot \"src/lib.rs\" .\n```\n\n```rust {#inner}\nfn inner() -> u8 { 1 }\n```\n\n```rust {#root}\nfn outer() {}\n<<inner>>\n```\n";
        let old = "// @generated\nfn outer() {}\nfn inner() -> u8 { 1 }\n";
        let new = "// @generated\nfn outer() {}\nfn inner() -> u8 { 2 }\n";
        assert_eq!(touched_chunks(doc, old, new), vec!["inner".to_string()]);
        let new_outer = "// @generated\nfn outer() { todo!() }\nfn inner() -> u8 { 1 }\n";
        assert_eq!(touched_chunks(doc, old, new_outer), vec!["root".to_string()]);
    }
}
```

### End to end, against a real projection

The unit tests above never build a projection. They hand `classify` a table
of paths and check that the ladder answers correctly — which is worth pinning,
and which says nothing about the two trees the classifier sits between. Every
hard thing here lives in those trees: a clone whose `PROVENANCE.json` names a
commit, a corpus that has moved since, a `@generated` file whose origin has to
be recovered by re-expanding chunks. So `tests/receive_repo.rs` builds both
trees with the real projector and runs Carol's typo through the whole cycle.

<a name="chunk-receive-e2e-doc"></a><sub>[`tests/receive_repo.rs`](../../crates/x0k-tangle/tests/receive_repo.rs) · `#receive-e2e-doc`</sub>

```rust {#receive-e2e-doc file="tests/receive_repo.rs"}
//! End-to-end pins for `receive_repo`, the inward leg of the projection
//! cycle (`x0k:implementation/tangle/receiving`): a synthetic literate
//! workspace with a git history is projected into a "clone" with the real
//! projector; a contributor edits the clone; the receiver classifies and
//! patches.
//!
//! - a literate-doc edit is received against its monorepo path, with the
//!   reference built at the clone's `corpus_rev` so the corpus's own drift
//!   since the projection does not leak into the proposal;
//! - an `@generated` edit is refused and names the doc + chunk;
//! - an overlay path is projection-local; scaffolding is projection-owned;
//! - `--apply` patches the working copy, and refuses when the target is
//!   already dirty;
//! - the receipt records what `--apply` did — landed, or refused and why;
//! - a workspace with no VCS still receives, with `rev_exact: false`;
//! - a Cargo workspace with nested crates, a root manifest and a vocabulary
//!   module is received at the commit the clone records, a hand-written
//!   edit routed to the crate's real directory;
//! - the recorded commit, not the (possibly moved) revision, pins the
//!   reference;
//! - under the organized layout every edit routes back to its corpus
//!   path, and the received patches apply there.
```

`Class` is reached through its module rather than the crate root: the
receiver's classification vocabulary is deliberately not part of the top-level
surface, and the test speaks the same name a consumer would have to.

<a name="chunk-receive-e2e-uses"></a><sub>[`tests/receive_repo.rs`](../../crates/x0k-tangle/tests/receive_repo.rs) · `#receive-e2e-uses`</sub>

```rust {#receive-e2e-uses file="tests/receive_repo.rs"}
use std::path::{Path, PathBuf};
use std::process::Command;

use x0k_tangle::receive::Class;
use x0k_tangle::{
    project_publication_repo, receive_repo, tangle_document, PipelineRegistry, ReceiveOptions,
    RepoProjectOptions,
};
```

The fixture is a miniature of the monorepo — one publishable crate, one
literate document, one publication doc — because the receiver's whole job is
relating paths across two trees, and a fixture with only one of each still has
every relation. The document and publication are written as string constants so
the fixture is legible at the point of use:

<a name="chunk-receive-e2e-paths"></a><sub>[`tests/receive_repo.rs`](../../crates/x0k-tangle/tests/receive_repo.rs) · `#receive-e2e-paths`</sub>

```rust {#receive-e2e-paths file="tests/receive_repo.rs"}
const DOC_REL: &str = "knowledge/implementation/demo/colophon.md";
const PUB_REL: &str = "decisions/publications/demo.md";
```

<a name="chunk-receive-e2e-fixture-docs"></a><sub>[`tests/receive_repo.rs`](../../crates/x0k-tangle/tests/receive_repo.rs) · `#receive-e2e-fixture-docs`</sub>

```rust {#receive-e2e-fixture-docs file="tests/receive_repo.rs"}
const DOC: &str = "# The demo colophon\n\n```turtle folio:document\nimplementation:demo\\/colophon a x0k:Implementation ;\n    x0k:status \"draft\" ;\n    x0k:summary \"The demo crate's one exported function, and where it trims.\" ;\n    folio:tangleCrate \"demo-crate\" ;\n    folio:tangleRoot \"src/lib.rs\" .\n```\n\nThe header parser tolerates keys it does not own. The first line\nis the whole contract.\n\n```rust {#parse-line}\n/// First line of `s`, trimmed.\npub fn parse_line(s: &str) -> &str {\n    s.lines().next().unwrap_or(\"\").trim()\n}\n```\n\n```rust {#root}\npub mod hand;\n\n<<parse-line>>\n```\n";

const PUB: &str = "# Demo\n\n```turtle folio:document\npublication:demo a x0k:Publication ;\n    x0k:status \"proposed\" ;\n    x0k:license \"MIT\" ;\n    x0k:copyright \"Demo Authors\" ;\n    x0k:publishes x0k:software-module\\/demo-crate ;\n    folio:tangleRoot \"README.md\" .\n```\n\n```markdown {#readme}\n# Demo\n\nA demo publication.\n\n<!-- x0k:contents -->\n```\n";
```

`git` runs with identity and signing pinned inline. A test that inherited
the operator's `user.email` or a global `commit.gpgsign = true` would pass on
one machine and hang on another:

<a name="chunk-receive-e2e-git"></a><sub>[`tests/receive_repo.rs`](../../crates/x0k-tangle/tests/receive_repo.rs) · `#receive-e2e-git`</sub>

```rust {#receive-e2e-git file="tests/receive_repo.rs"}
fn git(dir: &Path, args: &[&str]) {
    let status = Command::new("git")
        .current_dir(dir)
        .args([
            "-c",
            "user.name=test",
            "-c",
            "user.email=test@example.com",
            "-c",
            "commit.gpgsign=false",
        ])
        .args(args)
        .status()
        .expect("git runs");
    assert!(status.success(), "git {args:?} failed");
}
```

The workspace is tangled by the real tangler, not by writing a plausible
`lib.rs`. The assertion after the call is the load-bearing part: if the fixture
ever stopped producing an `@generated` file, the refusal test below would keep
passing for the wrong reason — nothing to refuse.

<a name="chunk-receive-e2e-workspace"></a><sub>[`tests/receive_repo.rs`](../../crates/x0k-tangle/tests/receive_repo.rs) · `#receive-e2e-workspace`</sub>

```rust {#receive-e2e-workspace file="tests/receive_repo.rs"}
/// A workspace with one published crate, one literate doc tangled by the
/// real tangler, and a publication doc. `with_git` commits it so the
/// projector records a resolvable `corpus_rev`.
fn workspace(with_git: bool) -> tempfile::TempDir {
    let tmp = tempfile::tempdir().expect("tempdir");
    let ws = tmp.path();
    std::fs::create_dir_all(ws.join("demo-crate/src")).unwrap();
    std::fs::write(
        ws.join("demo-crate/Cargo.toml"),
        "[package]\nname = \"demo-crate\"\nversion = \"0.1.0\"\nedition = \"2021\"\nlicense = \"LicenseRef-Proprietary\"\n\n[package.metadata.x0k]\naccess = \"public\"\n\n[dependencies]\n",
    )
    .unwrap();
    std::fs::write(
        ws.join("demo-crate/src/hand.rs"),
        "/// Hand-written, not tangled.\npub fn hand() -> u8 {\n    1\n}\n",
    )
    .unwrap();
    std::fs::create_dir_all(ws.join(DOC_REL).parent().unwrap()).unwrap();
    std::fs::write(ws.join(DOC_REL), DOC).unwrap();
    std::fs::create_dir_all(ws.join(PUB_REL).parent().unwrap()).unwrap();
    std::fs::write(ws.join(PUB_REL), PUB).unwrap();
    tangle_document(&ws.join(DOC_REL), ws, &PipelineRegistry::default()).expect("tangle");
    assert!(
        std::fs::read_to_string(ws.join("demo-crate/src/lib.rs"))
            .unwrap()
            .starts_with("// @generated"),
        "fixture tangles a generated output"
    );
    std::fs::write(ws.join(".gitignore"), "/target\n").unwrap();
    if with_git {
        git(ws, &["init", "-q"]);
        git(ws, &["add", "-A"]);
        git(ws, &["commit", "-q", "-m", "corpus"]);
    }
    tmp
}
```

The clone is likewise a real projection rather than a hand-built tree, so
the reference the receiver rebuilds is compared against something the projector
actually emits. `git_init` is off: the clone needs a `PROVENANCE.json`, not a
history of its own.

<a name="chunk-receive-e2e-project"></a><sub>[`tests/receive_repo.rs`](../../crates/x0k-tangle/tests/receive_repo.rs) · `#receive-e2e-project`</sub>

```rust {#receive-e2e-project file="tests/receive_repo.rs"}
fn project(ws: &Path, clone: &Path) {
    project_doc(ws, clone, PUB_REL)
}

fn project_doc(ws: &Path, clone: &Path, publication: &str) {
    project_publication_repo(
        &ws.join(publication),
        clone,
        ws,
        &RepoProjectOptions {
            license: None,
            git_init: false,
            allow_dirty: false,
            emit_github: false,
        },
    )
    .expect("projection");
}
```

Each test hands the receiver its own scratch directory. The receiver
materializes the corpus at a past commit to build the reference, and a shared
scratch would let one test read another's materialization:

<a name="chunk-receive-e2e-options"></a><sub>[`tests/receive_repo.rs`](../../crates/x0k-tangle/tests/receive_repo.rs) · `#receive-e2e-options`</sub>

```rust {#receive-e2e-options file="tests/receive_repo.rs"}
fn scratch_opts(scratch: &Path, apply: bool, out: Option<PathBuf>) -> ReceiveOptions {
    ReceiveOptions {
        apply,
        out_dir: out,
        publication: None,
        scratch: Some(scratch.to_path_buf()),
    }
}
```

The first test is the carried example, with one addition that makes it a
real test rather than a demonstration: the corpus gains a paragraph *after* the
projection and *before* the receive. That drift is the trap. A reference built
from the corpus as it stands would diff Carol's clone against a tree she never
saw, and the maintainer's own new paragraph would come back as a deletion in
her patch. Building the reference at the clone's `corpus_rev` is what keeps the
report to one change, and the assertion that the drift text is absent from the
patch is what proves it.

<a name="chunk-receive-e2e-literate-edit"></a><sub>[`tests/receive_repo.rs`](../../crates/x0k-tangle/tests/receive_repo.rs) · `#receive-e2e-literate-edit`</sub>

```rust {#receive-e2e-literate-edit file="tests/receive_repo.rs"}
#[test]
fn literate_edit_is_received_against_the_monorepo_doc_at_the_clones_rev() {
    let ws = workspace(true);
    let clone = tempfile::tempdir().unwrap();
    let scratch = tempfile::tempdir().unwrap();
    let out = tempfile::tempdir().unwrap();
    project(ws.path(), clone.path());
    let prov: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(clone.path().join("PROVENANCE.json")).unwrap())
            .unwrap();
    assert!(
        prov["corpus_rev"].as_str().map(|s| s.len() == 40).unwrap_or(false),
        "fixture records a git sha as corpus_rev: {prov}"
    );

    // The corpus moves on after the projection (a new closing paragraph).
    let doc = ws.path().join(DOC_REL);
    let drifted = std::fs::read_to_string(&doc).unwrap() + "\nA paragraph the maintainer added later.\n";
    std::fs::write(&doc, &drifted).unwrap();
    git(ws.path(), &["commit", "-q", "-am", "drift"]);

    // Carol fixes a typo in the clone.
    let clone_doc = clone.path().join(DOC_REL);
    let fixed = std::fs::read_to_string(&clone_doc)
        .unwrap()
        .replace("tolerates keys it does not own", "tolerates keys it does not own itself");
    std::fs::write(&clone_doc, fixed).unwrap();

    let report = receive_repo(
        clone.path(),
        ws.path(),
        &scratch_opts(scratch.path(), false, Some(out.path().to_path_buf())),
    )
    .expect("receive");
    assert!(report.rev_exact, "reference built at the clone's corpus_rev");
    assert_eq!(report.reference_rev, prov["corpus_rev"].as_str().unwrap());
    assert_eq!(report.changes.len(), 1, "only Carol's change, not the drift: {:#?}", report.changes);
    let c = &report.changes[0];
    assert_eq!(c.class, Class::Literate);
    assert_eq!(c.kind, "modified");
    assert_eq!(c.target.as_deref(), Some(DOC_REL));
    let patch = c.patch.as_deref().unwrap();
    assert!(patch.starts_with(&format!("--- a/{DOC_REL}\n+++ b/{DOC_REL}\n")));
    assert!(patch.contains("+The header parser tolerates keys it does not own itself."));
    assert!(!patch.contains("maintainer added later"), "drift must not appear reversed");
    assert_eq!(report.received(), 1);
    assert_eq!(report.refused(), 0);

    // The patch set on disk: one numbered patch + the receipt.
    let patch_file = c.patch_file.as_deref().expect("patch written");
    assert_eq!(std::fs::read_to_string(out.path().join(patch_file)).unwrap(), patch);
    let receipt: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(out.path().join("receipt.json")).unwrap())
            .unwrap();
    assert_eq!(receipt["schema"], "x0k.receipt/v1");
    assert_eq!(receipt["report"]["changes"][0]["class"], "literate");
    assert_eq!(receipt["report"]["rev_exact"], true);

    // Nothing was applied without --apply.
    assert_eq!(std::fs::read_to_string(&doc).unwrap(), drifted);
}
```

Editing a generated file is the case the design refuses on purpose, and a
bare refusal would be useless to the contributor. The report has to name the
document and the chunk, so the test checks both — `parse-line`, not `root`,
because the origin walk finds the innermost chunk whose expansion changed.

<a name="chunk-receive-e2e-generated-edit"></a><sub>[`tests/receive_repo.rs`](../../crates/x0k-tangle/tests/receive_repo.rs) · `#receive-e2e-generated-edit`</sub>

```rust {#receive-e2e-generated-edit file="tests/receive_repo.rs"}
#[test]
fn generated_edit_is_refused_and_names_doc_and_chunk() {
    let ws = workspace(true);
    let clone = tempfile::tempdir().unwrap();
    let scratch = tempfile::tempdir().unwrap();
    project(ws.path(), clone.path());

    let lib = clone.path().join("demo-crate/src/lib.rs");
    let edited = std::fs::read_to_string(&lib)
        .unwrap()
        .replace("unwrap_or(\"\").trim()", "unwrap_or(\"\").trim_end()");
    std::fs::write(&lib, edited).unwrap();

    let report = receive_repo(clone.path(), ws.path(), &scratch_opts(scratch.path(), false, None))
        .expect("receive");
    assert_eq!(report.changes.len(), 1);
    let c = &report.changes[0];
    assert_eq!(c.class, Class::Generated);
    assert!(c.patch.is_none(), "a refused change carries no patch");
    let origin = c.produced_by.as_ref().expect("origin named");
    assert_eq!(origin.doc, DOC_REL);
    assert_eq!(origin.chunks, vec!["parse-line".to_string()]);
    assert_eq!(report.refused(), 1);
    assert_eq!(report.received(), 0);
}
```

Three of the five classes only exist because a projection is not a clone of
the corpus. `CONTRIBUTING.md` is listed in the clone's overlay and is
projection-local — the maintainer keeps it on the public side and no monorepo
file corresponds to it. `README.md` is projection-owned: it is regenerated from
the publication document on every publish, so an edit to it is reported and
dropped. `PROVENANCE.json` is neither, because it is the receiver's *input*;
reporting it as a change would mean the receiver diffing its own instructions.

<a name="chunk-receive-e2e-overlay"></a><sub>[`tests/receive_repo.rs`](../../crates/x0k-tangle/tests/receive_repo.rs) · `#receive-e2e-overlay`</sub>

```rust {#receive-e2e-overlay file="tests/receive_repo.rs"}
#[test]
fn overlay_is_projection_local_and_scaffolding_is_projection_owned() {
    let ws = workspace(true);
    let clone = tempfile::tempdir().unwrap();
    let scratch = tempfile::tempdir().unwrap();
    project(ws.path(), clone.path());

    // The maintainer preserves CONTRIBUTING.md on the public side.
    let prov_path = clone.path().join("PROVENANCE.json");
    let mut prov: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&prov_path).unwrap()).unwrap();
    prov["overlay"] = serde_json::json!(["CONTRIBUTING.md"]);
    std::fs::write(&prov_path, serde_json::to_string_pretty(&prov).unwrap()).unwrap();
    std::fs::write(clone.path().join("CONTRIBUTING.md"), "Please edit the .md sources.\n").unwrap();
    // A contributor also touched the README and a hand-written file.
    let readme = clone.path().join("README.md");
    std::fs::write(&readme, std::fs::read_to_string(&readme).unwrap() + "\nextra\n").unwrap();
    let hand = clone.path().join("demo-crate/src/hand.rs");
    std::fs::write(&hand, std::fs::read_to_string(&hand).unwrap().replace("1\n", "2\n")).unwrap();

    let report = receive_repo(clone.path(), ws.path(), &scratch_opts(scratch.path(), false, None))
        .expect("receive");
    let class_of = |p: &str| {
        report
            .changes
            .iter()
            .find(|c| c.path == p)
            .unwrap_or_else(|| panic!("{p} not in report: {:#?}", report.changes))
    };
    assert_eq!(class_of("CONTRIBUTING.md").class, Class::ProjectionLocal);
    assert_eq!(class_of("CONTRIBUTING.md").kind, "added");
    assert_eq!(class_of("README.md").class, Class::ProjectionOwned);
    assert!(
        report.changes.iter().all(|c| c.path != "PROVENANCE.json"),
        "PROVENANCE.json is the receiver's input, never a change"
    );
    let hand = class_of("demo-crate/src/hand.rs");
    assert_eq!(hand.class, Class::Source);
    assert_eq!(hand.target.as_deref(), Some("demo-crate/src/hand.rs"));
    assert_eq!(report.received(), 1);
    assert_eq!(report.refused(), 0);
}
```

`--apply` is the only verb here that writes to the operator's tree, and it
writes exactly once. The second call in this test finds its target already
modified and refuses — not because applying twice would fail, but because a
patch that applies cleanly to a dirty file silently discards whatever made it
dirty. The refusal names the path so the operator can see what it would have
overwritten. Applied, never committed: the intent stamp is the operator's to
make.

<a name="chunk-receive-e2e-apply"></a><sub>[`tests/receive_repo.rs`](../../crates/x0k-tangle/tests/receive_repo.rs) · `#receive-e2e-apply`</sub>

```rust {#receive-e2e-apply file="tests/receive_repo.rs"}
#[test]
fn apply_patches_the_working_copy_and_refuses_a_dirty_target() {
    let ws = workspace(true);
    let clone = tempfile::tempdir().unwrap();
    let scratch = tempfile::tempdir().unwrap();
    project(ws.path(), clone.path());

    let clone_doc = clone.path().join(DOC_REL);
    let fixed = std::fs::read_to_string(&clone_doc)
        .unwrap()
        .replace("is the whole contract", "is the entire contract");
    std::fs::write(&clone_doc, fixed).unwrap();

    let report = receive_repo(clone.path(), ws.path(), &scratch_opts(scratch.path(), true, None))
        .expect("receive --apply");
    assert!(report.applied);
    assert_eq!(report.dirty_check, "git");
    let doc = std::fs::read_to_string(ws.path().join(DOC_REL)).unwrap();
    assert!(doc.contains("is the entire contract"), "working copy patched");
    // Applied, not committed: the operator reviews and commits.
    let status = Command::new("git")
        .current_dir(ws.path())
        .args(["status", "--porcelain"])
        .output()
        .unwrap();
    assert!(
        String::from_utf8_lossy(&status.stdout).contains(DOC_REL),
        "the doc is left uncommitted"
    );

    // The same target is now dirty; a second apply must refuse.
    let err = receive_repo(clone.path(), ws.path(), &scratch_opts(scratch.path(), true, None))
        .expect_err("dirty target refuses --apply");
    assert!(err.to_string().contains("refusing --apply"), "{err}");
    assert!(err.to_string().contains(DOC_REL), "{err}");
}
```

The receipt is what a CI check reads instead of the exit code, so it has
to say what the apply did. Both outcomes are pinned on the patch set
written under `--out`: an apply that lands records `applied: true` and no
error; an apply refused over a dirty target records `applied: false` and
the refusal itself, even though the run then exits with it.

<a name="chunk-receive-e2e-receipt"></a><sub>[`tests/receive_repo.rs`](../../crates/x0k-tangle/tests/receive_repo.rs) · `#receive-e2e-receipt`</sub>

```rust {#receive-e2e-receipt file="tests/receive_repo.rs"}
fn read_receipt(out: &Path) -> serde_json::Value {
    serde_json::from_str(&std::fs::read_to_string(out.join("receipt.json")).expect("receipt.json"))
        .expect("receipt parses")
}

#[test]
fn the_receipt_records_what_apply_did() {
    let ws = workspace(true);
    let clone = tempfile::tempdir().unwrap();
    let scratch = tempfile::tempdir().unwrap();
    project(ws.path(), clone.path());
    let clone_doc = clone.path().join(DOC_REL);
    let fixed = std::fs::read_to_string(&clone_doc)
        .unwrap()
        .replace("is the whole contract", "is the entire contract");
    std::fs::write(&clone_doc, fixed).unwrap();

    let landed = tempfile::tempdir().unwrap();
    let opts = scratch_opts(scratch.path(), true, Some(landed.path().to_path_buf()));
    let report = receive_repo(clone.path(), ws.path(), &opts).expect("receive --apply");
    assert!(report.applied);
    let receipt = read_receipt(landed.path());
    assert_eq!(receipt["report"]["applied"], true, "{receipt:#}");
    assert_eq!(receipt["report"]["apply_error"], serde_json::Value::Null, "{receipt:#}");
    assert_eq!(receipt["report"]["dirty_check"], "git", "{receipt:#}");

    // The target is now dirty: the apply refuses, and the receipt says so.
    let refused = tempfile::tempdir().unwrap();
    let opts = scratch_opts(scratch.path(), true, Some(refused.path().to_path_buf()));
    let err = receive_repo(clone.path(), ws.path(), &opts).expect_err("dirty target refuses --apply");
    let receipt = read_receipt(refused.path());
    assert_eq!(receipt["report"]["applied"], false, "{receipt:#}");
    let recorded = receipt["report"]["apply_error"].as_str().expect("the failure is recorded");
    assert!(recorded.contains("refusing --apply") && recorded.contains(DOC_REL), "{recorded}");
    assert_eq!(recorded, format!("{err:#}"), "the receipt carries the error the run exits with");
}
```

Finally, the honest degradation. A workspace with no VCS cannot be
materialized at a commit, so the reference is built from the tree as it stands
and the report says `rev_exact: false` rather than pretending. The receiver
still works; the maintainer just knows the comparison is against now, not
against the projection.

<a name="chunk-receive-e2e-no-vcs"></a><sub>[`tests/receive_repo.rs`](../../crates/x0k-tangle/tests/receive_repo.rs) · `#receive-e2e-no-vcs`</sub>

```rust {#receive-e2e-no-vcs file="tests/receive_repo.rs"}
#[test]
fn without_a_vcs_the_reference_is_the_current_tree_and_says_so() {
    let ws = workspace(false);
    let clone = tempfile::tempdir().unwrap();
    let scratch = tempfile::tempdir().unwrap();
    project(ws.path(), clone.path());
    let hand = clone.path().join("demo-crate/src/hand.rs");
    std::fs::write(&hand, std::fs::read_to_string(&hand).unwrap() + "// note\n").unwrap();

    let report = receive_repo(clone.path(), ws.path(), &scratch_opts(scratch.path(), false, None))
        .expect("receive");
    assert!(!report.rev_exact);
    assert_eq!(report.clone_rev, "");
    assert_eq!(report.changes.len(), 1);
    assert_eq!(report.changes[0].class, Class::Source);
}
```

### At the exact commit, in a tree shaped like ours

Every fixture above keeps its crate at the workspace root with no root
manifest, which is the one layout the monorepo does not have. Ours is a Cargo
workspace: a root `Cargo.toml` listing members under
`substrate/crates/<area>/<name>`, path dependencies between them, members no
publication ships, and the vocabulary under `corpora/x0k/ontology/modules`.
The projector reads all of that through Cargo, so a reference built from
anything less than the tree it read refuses (a published package "is not a
workspace member") or projects a different bundle. The nested fixture is that
shape in miniature: two published crates at different depths, one depending on
the other by path, an unpublished member beside them, a vocabulary module
where ours lives, the root manifest over all of it, and the publication in a
directory of its own, where the receiver has to find it by its id.

<a name="chunk-receive-e2e-nested-workspace"></a><sub>[`tests/receive_repo.rs`](../../crates/x0k-tangle/tests/receive_repo.rs) · `#receive-e2e-nested-workspace`</sub>

```rust {#receive-e2e-nested-workspace file="tests/receive_repo.rs"}
const NESTED_CRATE: &str = "crates/app/demo-crate";
const NESTED_CORE: &str = "crates/core/demo-core";
/// The publication in a directory of its own, as ours are kept.
const NESTED_PUB_REL: &str = "decisions/publications/demo/demo.md";
const MODULE_FIXTURE: &str =
    concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/ontology-modules/core.ttl");

const NESTED_PUB: &str = "# Demo\n\n```turtle folio:document\npublication:demo a x0k:Publication ;\n    x0k:status \"proposed\" ;\n    x0k:license \"MIT\" ;\n    x0k:copyright \"Demo Authors\" ;\n    x0k:publishes x0k:software-module\\/demo-crate,\n        x0k:software-module\\/demo-core,\n        x0k:ontology-module\\/core ;\n    x0k:entryPoint x0k:software-module\\/demo-crate ;\n    folio:tangleRoot \"README.md\" .\n```\n\n```markdown {#readme}\n# Demo\n\nA demo publication.\n\n<!-- x0k:contents -->\n```\n";

/// A Cargo workspace laid out like the monorepo: a root manifest, published
/// crates at nested paths (one path-depending on the other), an unpublished
/// member, the chapter tangled into the nested crate by the real tangler, and
/// the vocabulary module under `corpora/x0k/ontology/modules`. Committed, so
/// the projection records a commit. `organized` selects the publication's
/// organized repository layout (`crates/<name>/`, `implementation/…`).
fn nested_workspace(organized: bool) -> tempfile::TempDir {
    let tmp = tempfile::tempdir().expect("tempdir");
    let ws = tmp.path();
    std::fs::write(
        ws.join("Cargo.toml"),
        format!(
            "[workspace]\nresolver = \"2\"\nmembers = [\n    \"{NESTED_CRATE}\",\n    \"{NESTED_CORE}\",\n    \"tools/helper\",\n]\n"
        ),
    )
    .unwrap();
    let members = [
        (
            NESTED_CRATE,
            "demo-crate",
            "demo-core = { path = \"../../core/demo-core\" }\n",
            "src/hand.rs",
            "/// Hand-written, not tangled.\npub fn hand() -> u8 {\n    demo_core::one()\n}\n",
        ),
        (
            NESTED_CORE,
            "demo-core",
            "",
            "src/lib.rs",
            "/// What the demo crate builds on.\npub fn one() -> u8 {\n    1\n}\n",
        ),
        ("tools/helper", "helper", "", "src/main.rs", "fn main() {}\n"),
    ];
    for (dir, name, deps, file, body) in members {
        std::fs::create_dir_all(ws.join(dir).join("src")).unwrap();
        std::fs::write(
            ws.join(dir).join("Cargo.toml"),
            format!(
                "[package]\nname = \"{name}\"\nversion = \"0.1.0\"\nedition = \"2021\"\nlicense = \"LicenseRef-Proprietary\"\n\n[package.metadata.x0k]\naccess = \"public\"\n\n[dependencies]\n{deps}"
            ),
        )
        .unwrap();
        std::fs::write(ws.join(dir).join(file), body).unwrap();
    }
    let doc = DOC.replace(
        "folio:tangleCrate \"demo-crate\"",
        &format!("folio:tangleCrate \"{NESTED_CRATE}\""),
    );
    assert_ne!(doc, DOC, "the chapter names the nested crate");
    std::fs::create_dir_all(ws.join(DOC_REL).parent().unwrap()).unwrap();
    std::fs::write(ws.join(DOC_REL), doc).unwrap();
    std::fs::create_dir_all(ws.join(NESTED_PUB_REL).parent().unwrap()).unwrap();
    let publication = if organized {
        NESTED_PUB.replace(
            "    folio:tangleRoot",
            "    x0k:repositoryLayout \"organized\" ;\n    folio:tangleRoot",
        )
    } else {
        NESTED_PUB.to_string()
    };
    std::fs::write(ws.join(NESTED_PUB_REL), publication).unwrap();
    let modules = ws.join("corpora/x0k/ontology/modules");
    std::fs::create_dir_all(&modules).unwrap();
    std::fs::copy(MODULE_FIXTURE, modules.join("core.ttl")).unwrap();
    tangle_document(&ws.join(DOC_REL), ws, &PipelineRegistry::default()).expect("tangle");
    assert!(
        std::fs::read_to_string(ws.join(NESTED_CRATE).join("src/lib.rs"))
            .unwrap()
            .starts_with("// @generated"),
        "fixture tangles a generated output into the nested crate"
    );
    std::fs::write(ws.join(".gitignore"), "/target\n").unwrap();
    git(ws, &["init", "-q"]);
    git(ws, &["add", "-A"]);
    git(ws, &["commit", "-q", "-m", "corpus"]);
    tmp
}
```

The test is the carried example again, at full width. The corpus drifts after
the projection, in the chapter and in a hand-written file; Carol fixes a typo in
the chapter, fixes the hand-written file, and edits the generated one. Exactly
her three changes come back: the chapter's patch against its corpus path, the
hand-written fix against the crate's *real* directory (the clone holds it at
`demo-crate/`, the corpus at `crates/app/demo-crate/`), and the generated edit
refused with its document and chunk. Any path the reference failed to carry —
the module, the dependency's crate — would surface as a fourth change, so the
count is part of the claim. The receipt records the commit the reference was
built at.

<a name="chunk-receive-e2e-nested"></a><sub>[`tests/receive_repo.rs`](../../crates/x0k-tangle/tests/receive_repo.rs) · `#receive-e2e-nested`</sub>

```rust {#receive-e2e-nested file="tests/receive_repo.rs"}
#[test]
fn nested_crates_under_a_root_manifest_are_received_at_the_exact_commit() {
    let ws = nested_workspace(false);
    let clone = tempfile::tempdir().unwrap();
    let scratch = tempfile::tempdir().unwrap();
    let out = tempfile::tempdir().unwrap();
    project_doc(ws.path(), clone.path(), NESTED_PUB_REL);
    assert!(clone.path().join("demo-crate/src/hand.rs").is_file(), "vendored flat by name");
    assert!(clone.path().join("demo-core/src/lib.rs").is_file(), "the dependency ships");
    assert!(clone.path().join("ontology/modules/core.ttl").is_file(), "the module ships");
    let prov: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(clone.path().join("PROVENANCE.json")).unwrap())
            .unwrap();
    let commit = prov["corpus_commit"].as_str().expect("provenance records the commit").to_string();
    assert_eq!(commit.len(), 40, "a git sha: {prov}");

    // The corpus moves on after the projection.
    let doc = ws.path().join(DOC_REL);
    std::fs::write(
        &doc,
        std::fs::read_to_string(&doc).unwrap() + "\nA paragraph the maintainer added later.\n",
    )
    .unwrap();
    let hand_src = ws.path().join(NESTED_CRATE).join("src/hand.rs");
    std::fs::write(
        &hand_src,
        std::fs::read_to_string(&hand_src).unwrap() + "// maintainer note, added later\n",
    )
    .unwrap();
    git(ws.path(), &["commit", "-q", "-am", "drift"]);

    // Carol's three edits.
    let edit = |rel: &str, from: &str, to: &str| {
        let path = clone.path().join(rel);
        let text = std::fs::read_to_string(&path).unwrap();
        assert!(text.contains(from), "{rel} holds `{from}`");
        std::fs::write(&path, text.replace(from, to)).unwrap();
    };
    edit(DOC_REL, "is the whole contract", "is the entire contract");
    edit("demo-crate/src/hand.rs", "demo_core::one()\n", "demo_core::one() + 1\n");
    edit("demo-crate/src/lib.rs", "unwrap_or(\"\").trim()", "unwrap_or(\"\").trim_end()");

    let report = receive_repo(
        clone.path(),
        ws.path(),
        &scratch_opts(scratch.path(), false, Some(out.path().to_path_buf())),
    )
    .expect("receive");
    assert!(report.rev_exact, "reference built at the clone's commit");
    assert_eq!(report.changes.len(), 3, "only Carol's three: {:#?}", report.changes);
    let change = |p: &str| {
        report
            .changes
            .iter()
            .find(|c| c.path == p)
            .unwrap_or_else(|| panic!("{p} not in report: {:#?}", report.changes))
    };

    let literate = change(DOC_REL);
    assert_eq!(literate.class, Class::Literate);
    assert_eq!(literate.target.as_deref(), Some(DOC_REL));
    let patch = literate.patch.as_deref().unwrap();
    assert!(patch.contains("+is the entire contract."), "{patch}");
    assert!(!patch.contains("maintainer added later"), "drift must not appear reversed");

    let source = change("demo-crate/src/hand.rs");
    assert_eq!(source.class, Class::Source);
    let real = format!("{NESTED_CRATE}/src/hand.rs");
    assert_eq!(source.target.as_deref(), Some(real.as_str()), "routed to the crate's real path");
    let patch = source.patch.as_deref().unwrap();
    assert!(patch.starts_with(&format!("--- a/{real}\n+++ b/{real}\n")), "{patch}");
    assert!(!patch.contains("maintainer note"), "drift must not appear reversed");

    let generated = change("demo-crate/src/lib.rs");
    assert_eq!(generated.class, Class::Generated);
    let origin = generated.produced_by.as_ref().expect("origin named");
    assert_eq!(origin.doc, DOC_REL);
    assert_eq!(origin.chunks, vec!["parse-line".to_string()]);

    assert_eq!(report.received(), 2);
    assert_eq!(report.refused(), 1);
    let receipt: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(out.path().join("receipt.json")).unwrap())
            .unwrap();
    assert_eq!(receipt["report"]["reference_commit"], commit.as_str());
}
```

A jj workspace records a *change id* as `corpus_rev`, and a change id names
whatever that change holds now: amend it after projecting and the id resolves
to a tree the contributor never saw. The projector records the commit beside it
for exactly this reason, and the receiver must build from the commit. Git has no
mutable revision of that kind, so the test stands one in: `corpus_rev` is
rewritten to `HEAD`, which moves when the corpus commits its drift. Built from
the revision, the reference would carry the drift and Carol's patch would
delete it; built from the commit, it does not.

<a name="chunk-receive-e2e-commit-pins"></a><sub>[`tests/receive_repo.rs`](../../crates/x0k-tangle/tests/receive_repo.rs) · `#receive-e2e-commit-pins`</sub>

```rust {#receive-e2e-commit-pins file="tests/receive_repo.rs"}
#[test]
fn the_recorded_commit_pins_the_reference_when_the_revision_has_moved() {
    let ws = workspace(true);
    let clone = tempfile::tempdir().unwrap();
    let scratch = tempfile::tempdir().unwrap();
    project(ws.path(), clone.path());
    let prov_path = clone.path().join("PROVENANCE.json");
    let mut prov: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&prov_path).unwrap()).unwrap();
    assert_eq!(prov["corpus_commit"], prov["corpus_rev"], "git records one sha twice");
    prov["corpus_rev"] = serde_json::json!("HEAD");
    std::fs::write(&prov_path, serde_json::to_string_pretty(&prov).unwrap()).unwrap();

    let doc = ws.path().join(DOC_REL);
    std::fs::write(
        &doc,
        std::fs::read_to_string(&doc).unwrap() + "\nA paragraph the maintainer added later.\n",
    )
    .unwrap();
    git(ws.path(), &["commit", "-q", "-am", "drift"]);

    let clone_doc = clone.path().join(DOC_REL);
    let fixed = std::fs::read_to_string(&clone_doc)
        .unwrap()
        .replace("is the whole contract", "is the entire contract");
    std::fs::write(&clone_doc, fixed).unwrap();

    let report = receive_repo(clone.path(), ws.path(), &scratch_opts(scratch.path(), false, None))
        .expect("receive");
    assert!(report.rev_exact);
    assert_eq!(report.changes.len(), 1, "{:#?}", report.changes);
    let patch = report.changes[0].patch.as_deref().unwrap();
    assert!(patch.contains("+is the entire contract."), "{patch}");
    assert!(!patch.contains("maintainer added later"), "built from the commit, not from HEAD");
}
```

Our own publication goes one step further: it selects the *organized*
repository layout, which moves every crate under `crates/<name>/`, every
chapter under `implementation/`, and re-tangles the projection so a generated
header names the chapter's projected path. Nothing in the clone then sits
where the corpus keeps it, so every route back goes through the provenance:
`path_map` (keyed by the projected path) for the chapter, the outermost
manifest-holding directory and the corpus's package roots for the crate, and
`path_map` again to name the generated file's document at its corpus path.
The test applies what it receives, so the patches are proven against the
corpus files and not only by their headers.

<a name="chunk-receive-e2e-organized"></a><sub>[`tests/receive_repo.rs`](../../crates/x0k-tangle/tests/receive_repo.rs) · `#receive-e2e-organized`</sub>

```rust {#receive-e2e-organized file="tests/receive_repo.rs"}
#[test]
fn the_organized_layout_routes_every_edit_back_to_its_corpus_path() {
    let ws = nested_workspace(true);
    let clone = tempfile::tempdir().unwrap();
    let scratch = tempfile::tempdir().unwrap();
    project_doc(ws.path(), clone.path(), NESTED_PUB_REL);
    let chapter = "implementation/demo/colophon.md";
    assert!(clone.path().join(chapter).is_file(), "the chapter is organized");
    assert!(clone.path().join("crates/demo-crate/src/hand.rs").is_file(), "the crate is organized");

    let edit = |rel: &str, from: &str, to: &str| {
        let path = clone.path().join(rel);
        let text = std::fs::read_to_string(&path).unwrap();
        assert!(text.contains(from), "{rel} holds `{from}`");
        std::fs::write(&path, text.replace(from, to)).unwrap();
    };
    edit(chapter, "is the whole contract", "is the entire contract");
    edit("crates/demo-crate/src/hand.rs", "demo_core::one()\n", "demo_core::one() + 1\n");
    edit("crates/demo-crate/src/lib.rs", "unwrap_or(\"\").trim()", "unwrap_or(\"\").trim_end()");

    let report = receive_repo(clone.path(), ws.path(), &scratch_opts(scratch.path(), true, None))
        .expect("receive --apply");
    assert!(report.rev_exact);
    assert_eq!(report.changes.len(), 3, "only Carol's three: {:#?}", report.changes);
    let change = |p: &str| {
        report
            .changes
            .iter()
            .find(|c| c.path == p)
            .unwrap_or_else(|| panic!("{p} not in report: {:#?}", report.changes))
    };
    let literate = change(chapter);
    assert_eq!(literate.class, Class::Literate);
    assert_eq!(literate.target.as_deref(), Some(DOC_REL), "routed through path_map");
    let source = change("crates/demo-crate/src/hand.rs");
    assert_eq!(source.class, Class::Source);
    let real = format!("{NESTED_CRATE}/src/hand.rs");
    assert_eq!(source.target.as_deref(), Some(real.as_str()));
    let generated = change("crates/demo-crate/src/lib.rs");
    assert_eq!(generated.class, Class::Generated, "a generated edit is refused, not scaffolding");
    let origin = generated.produced_by.as_ref().expect("origin named");
    assert_eq!(origin.doc, DOC_REL, "named at its corpus path");
    assert_eq!(origin.chunks, vec!["parse-line".to_string()]);

    assert!(report.applied);
    let doc = std::fs::read_to_string(ws.path().join(DOC_REL)).unwrap();
    assert!(doc.contains("is the entire contract"), "the chapter patch applied to the corpus doc");
    assert!(
        doc.contains(&format!("folio:tangleCrate \"{NESTED_CRATE}\"")),
        "the organized relocation did not leak back: {doc}"
    );
    let hand = std::fs::read_to_string(ws.path().join(&real)).unwrap();
    assert!(hand.contains("demo_core::one() + 1"), "the source patch applied at the crate's real path");
}
```

<a name="chunk-receive-e2e-root"></a><sub>[`tests/receive_repo.rs`](../../crates/x0k-tangle/tests/receive_repo.rs) · `#receive-e2e-root` · assembles [receive-e2e-doc](#chunk-receive-e2e-doc) · [receive-e2e-uses](#chunk-receive-e2e-uses) · [receive-e2e-paths](#chunk-receive-e2e-paths) · [receive-e2e-fixture-docs](#chunk-receive-e2e-fixture-docs) · [receive-e2e-git](#chunk-receive-e2e-git) · [receive-e2e-workspace](#chunk-receive-e2e-workspace) · [receive-e2e-project](#chunk-receive-e2e-project) · [receive-e2e-options](#chunk-receive-e2e-options) · [receive-e2e-literate-edit](#chunk-receive-e2e-literate-edit) · [receive-e2e-generated-edit](#chunk-receive-e2e-generated-edit) · [receive-e2e-overlay](#chunk-receive-e2e-overlay) · [receive-e2e-apply](#chunk-receive-e2e-apply) · [receive-e2e-receipt](#chunk-receive-e2e-receipt) · [receive-e2e-no-vcs](#chunk-receive-e2e-no-vcs) · [receive-e2e-nested-workspace](#chunk-receive-e2e-nested-workspace) · [receive-e2e-nested](#chunk-receive-e2e-nested) · [receive-e2e-commit-pins](#chunk-receive-e2e-commit-pins) · [receive-e2e-organized](#chunk-receive-e2e-organized)</sub>

```rust {#receive-e2e-root file="tests/receive_repo.rs"}
<<receive-e2e-doc>>

<<receive-e2e-uses>>

<<receive-e2e-paths>>

<<receive-e2e-fixture-docs>>

<<receive-e2e-git>>

<<receive-e2e-workspace>>

<<receive-e2e-project>>

<<receive-e2e-options>>

<<receive-e2e-literate-edit>>

<<receive-e2e-generated-edit>>

<<receive-e2e-overlay>>

<<receive-e2e-apply>>

<<receive-e2e-receipt>>

<<receive-e2e-no-vcs>>

<<receive-e2e-nested-workspace>>

<<receive-e2e-nested>>

<<receive-e2e-commit-pins>>

<<receive-e2e-organized>>
```

## Composing the module

<a name="chunk-root"></a><sub>[`src/receive.rs`](../../crates/x0k-tangle/src/receive.rs) · `#root` · assembles [module-doc](#chunk-module-doc) · [classification](#chunk-classification) · [origin](#chunk-origin) · [change](#chunk-change) · [options-and-report](#chunk-options-and-report) · [receive-repo](#chunk-receive-repo) · [provenance](#chunk-provenance) · [find-publication-doc](#chunk-find-publication-doc) · [reference-proofs](#chunk-reference-proofs) · [reference](#chunk-reference) · [materialize-corpus](#chunk-materialize-corpus) · [resolve-commit](#chunk-resolve-commit) · [diff-and-classify](#chunk-diff-and-classify) · [classify](#chunk-classify) · [generated-origin](#chunk-generated-origin) · [touched-chunks](#chunk-touched-chunks) · [unified-patch](#chunk-unified-patch) · [write-patch-set](#chunk-write-patch-set) · [write-receipt](#chunk-write-receipt) · [apply-patch-set](#chunk-apply-patch-set) · [tests](#chunk-tests)</sub>

```rust {#root}
<<module-doc>>

<<classification>>

<<origin>>

<<change>>

<<options-and-report>>

<<receive-repo>>

<<provenance>>

<<find-publication-doc>>

<<reference-proofs>>

<<reference>>

<<materialize-corpus>>

<<resolve-commit>>

<<diff-and-classify>>

<<classify>>

<<generated-origin>>

<<touched-chunks>>

<<unified-patch>>

<<write-patch-set>>

<<write-receipt>>

<<apply-patch-set>>

<<tests>>
```

What this module does not decide is the harder half of the design's
open question: whether the maintainer re-authors a received change by
hand or an accepted proposal is carried mechanically. The mechanism
here is deliberately the mechanical floor — a patch that applies, or a
refusal that names its chunk — and it stops at the working copy. A
received literate edit is not yet a received *change*: the doc has to
be re-tangled, the projection has to match byte-for-byte, and the
commit has to carry an intent. The receiver hands the operator a
reviewable patch set and an exit status; the graph's own proposal
surface (`in-prose-authoring`, `prose-provenance-and-underwriting`) is
where the review is meant to happen, and this module is the adapter
that gets an outsider's edit to its threshold.
