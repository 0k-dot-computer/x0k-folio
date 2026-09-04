---
x0k:
  format: folio/v1
  id: x0k:implementation/tangle/receiving
  type: implementation
  status: draft
  summary: "The door the world comes back through: a contributor's clone read as patches against the corpus files it was projected from, because a contribution is a proposal against the graph and never a merge into the projection."
  concerns: [tangle, publication, contribution, receive, git, provenance]
  tangle:
    crate: x0k-tangle
    root: src/receive.rs
  edges:
    implements:
      - x0k:design/publish-a-region-as-a-repository
      - x0k:affordance/receive_contribution_as_proposal
    cites:
      - x0k:implementation/tangle/publishing
      - x0k:implementation/folio/colophon
---
# Receiving a contribution from a projected repository

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
`knowledge/implementation/folio/colophon.md` ("the envelope parser
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

use anyhow::{anyhow, bail, Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use crate::parser::parse_document;
use crate::region_repo::{project_publication_repo, RepoProjectOptions};
use crate::resolve::expand_chunk;
use x0k_folio::colophon::parse_envelope;
```

## Contract

Every changed path in the clone lands in exactly one of five classes.
The first two are *receivable* — a patch exists and can be applied; the
other three are not, and the report says why in a form a contributor
can act on:

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

```rust {#options-and-report}
/// Options for a receive-repo run.
#[derive(Debug, Clone, Default)]
pub struct ReceiveOptions {
    /// Patch the workspace working copy (never commits). Refused when
    /// any target path is already dirty.
    pub apply: bool,
    /// Where to write the patch set + `receipt.json`.
    pub out_dir: Option<PathBuf>,
    /// The publication doc; default is a scan of
    /// `decisions/publications/` for the clone's `publication_uri`.
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
    /// `reference_rev` is `clone_rev`. When false, the diff includes
    /// the corpus's own drift since the clone was projected, reversed.
    pub rev_exact: bool,
    pub changes: Vec<ReceivedChange>,
    /// Which VCS answered the dirty check before `--apply`
    /// (`jj` | `git` | `none`).
    pub dirty_check: String,
    pub applied: bool,
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

```rust {#receive-repo}
/// Receive the changes in `clone` (a projected repository) as patches
/// against `workspace`. Reports every change; applies the receivable
/// ones only under `opts.apply`.
pub fn receive_repo(clone: &Path, workspace: &Path, opts: &ReceiveOptions) -> Result<ReceiveReport> {
    let clone = clone
        .canonicalize()
        .with_context(|| format!("clone dir {}", clone.display()))?;
    let workspace = workspace
        .canonicalize()
        .with_context(|| format!("workspace {}", workspace.display()))?;
    let prov = read_provenance(&clone)?;
    let pub_doc = match &opts.publication {
        Some(p) => p.clone(),
        None => find_publication_doc(&workspace, &prov.publication_uri)?,
    };
    let reference = build_reference(&workspace, &pub_doc, &prov, &clone, opts.scratch.as_deref())?;
    let mut report = ReceiveReport {
        publication_uri: prov.publication_uri.clone(),
        clone_rev: prov.corpus_rev.clone(),
        reference_rev: reference.rev.clone(),
        rev_exact: reference.exact,
        changes: diff_and_classify(&clone, &reference.dir, &prov)?,
        dirty_check: "none".to_string(),
        applied: false,
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
    if opts.apply {
        apply_patch_set(&workspace, &patch_dir, &mut report)?;
    }
    Ok(report)
}
```

## Provenance: what the clone says about itself

`PROVENANCE.json` is the seam the projector left for exactly this
purpose (schema `x0k.provenance/v1`). We read three fields and tolerate
a fourth: `overlay` is a list the maintainer may add on the public
side — paths, or directory prefixes ending in `/`, that the projector
does not own and the receiver does not carry back. It is the design's
"deliberate divergence" made explicit: a `CONTRIBUTING.md` written for
the forge's audience, a forge-specific funding file.

```rust {#provenance}
/// The fields of `PROVENANCE.json` the receiver reads.
#[derive(Debug, Clone, Deserialize)]
pub struct Provenance {
    pub publication_uri: String,
    #[serde(default)]
    pub corpus_rev: String,
    /// Monorepo doc path → projected path.
    #[serde(default)]
    pub path_map: BTreeMap<String, String>,
    /// Projected paths (or `dir/` prefixes) preserved on the public side.
    #[serde(default)]
    pub overlay: Vec<String>,
}

impl Provenance {
    /// Projected path → monorepo path, the direction receiving needs.
    fn canonical_for(&self, projected: &str) -> Option<&str> {
        self.path_map
            .iter()
            .find(|(_, p)| p.as_str() == projected)
            .map(|(c, _)| c.as_str())
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
disclosed region). Publications are few and live in one directory, so
a scan is the resolver:

```rust {#find-publication-doc}
/// Find the publication doc whose `id:` is `uri` under
/// `decisions/publications/`.
fn find_publication_doc(workspace: &Path, uri: &str) -> Result<PathBuf> {
    let dir = workspace.join("decisions/publications");
    let entries = std::fs::read_dir(&dir)
        .with_context(|| format!("listing {}", dir.display()))?;
    for entry in entries.flatten() {
        let path = entry.path();
        if path.extension().map(|e| e != "md").unwrap_or(true) {
            continue;
        }
        let Ok(text) = std::fs::read_to_string(&path) else { continue };
        if let Ok((env, _)) = parse_envelope(&text) {
            if env.id == uri {
                return Ok(path);
            }
        }
    }
    bail!("no publication with id `{uri}` under {} (pass --publication)", dir.display())
}
```

## The reference: the projection the contributor started from

The diff has to be taken against *the projection Carol cloned*, not
against the clone's own git history (a contributor may squash, rebase,
or have cloned a tarball) and not against the monorepo directly (the
projector rewrites manifests, so a raw comparison would drown the
signal). The reference is therefore a fresh run of the projector into a
temp dir. The question is which corpus to run it over.

The honest answer is the corpus at `corpus_rev`. That is cheaper than
it sounds: the corpus is a jj repo colocated with a git store, and
`git archive` can materialize just the directories the projector reads
— the published crates, `knowledge/implementation`, the publication
docs, and the vocabulary — at any commit in well under a second, with
no checkout. When that works, the reference is exact. When it cannot
(no VCS, a revision the store no longer resolves, a workspace that is
not git-backed), we project the *current* tree and the report says so:
`rev_exact: false`, with both revisions named. The consequence of a
skewed reference is stated, not hidden — the diff then contains the
corpus's own edits since the projection, reversed, and the maintainer
reads it as "rebase the contribution" rather than as Carol's intent.

One seam the exact path cannot close on its own: in a jj workspace the
projector records the *change id* of `@`, and a change id names a
mutable change — jj resolves it to whatever that change contains now.
A projection taken from an undescribed working copy therefore pins a
reference that moves with the operator's uncommitted edits; after
`--apply`, the reference already contains the applied fix and a rerun
reports nothing to receive. That is consistent, not wrong, but a
commit id in provenance would be a fixed point; recording one is the
projector's call.

```rust {#reference}
/// The reference projection: a temp dir holding a fresh projection of
/// the publication, and which corpus revision it came from.
struct Reference {
    dir: PathBuf,
    rev: String,
    exact: bool,
    _tmp: tempfile::TempDir,
}

fn build_reference(
    workspace: &Path,
    pub_doc: &Path,
    prov: &Provenance,
    clone: &Path,
    scratch: Option<&Path>,
) -> Result<Reference> {
    let tmp = match scratch {
        Some(s) => tempfile::Builder::new().prefix("x0k-receive-").tempdir_in(s)?,
        None => tempfile::Builder::new().prefix("x0k-receive-").tempdir()?,
    };
    let crates = published_crates(pub_doc)?;
    let (source_root, source_doc, exact) =
        match materialize_corpus_at(workspace, &prov.corpus_rev, &crates, tmp.path()) {
            Some(root) => {
                let rel = pub_doc.strip_prefix(workspace).unwrap_or(pub_doc);
                let doc = root.join(rel);
                let doc = if doc.is_file() { doc } else { pub_doc.to_path_buf() };
                (root, doc, true)
            }
            None => (workspace.to_path_buf(), pub_doc.to_path_buf(), false),
        };
    let rev = if exact {
        prov.corpus_rev.clone()
    } else {
        current_rev(workspace)
    };
    tracing::info!(exact, rev = %rev, "tangle.receive.reference");
    let dir = tmp.path().join("reference");
    let opts = RepoProjectOptions {
        license: None,
        git_init: false,
        // The reference is private scratch, not a disclosure; a guard
        // that fails on today's tree must not block receiving.
        allow_dirty: true,
        emit_github: clone.join(".github/workflows").is_dir(),
    };
    project_publication_repo(&source_doc, &dir, &source_root, &opts)
        .context("projecting the reference")?;
    Ok(Reference { dir, rev, exact, _tmp: tmp })
}
```

The crate membership comes from the publication doc's `publishes`
edges; it decides which crate directories to archive. Reading it here
rather than through the projector keeps the archive narrow:

```rust {#published-crates}
/// Crate names from the publication's `publishes` edges.
fn published_crates(pub_doc: &Path) -> Result<Vec<String>> {
    let text = std::fs::read_to_string(pub_doc)
        .with_context(|| format!("reading {}", pub_doc.display()))?;
    let (env, _) = parse_envelope(&text).map_err(|e| anyhow!("parsing publication: {e:?}"))?;
    Ok(env
        .edges
        .get("publishes")
        .into_iter()
        .flatten()
        .filter_map(|u| u.strip_prefix("x0k:software-module/"))
        .map(|s| s.to_string())
        .collect())
}
```

Materializing the corpus at a revision is two subprocesses: resolve
the revision to a git commit, then `git archive … | tar -x`. The
revision in provenance is whatever `current_corpus_rev` recorded — a
jj change id in the monorepo, a git sha in a plain git checkout — so we
try jj first (`--ignore-working-copy`, so the lookup neither snapshots
nor locks the operator's working copy) and fall back to git. Any
failure anywhere returns `None`; the caller degrades to the current
tree rather than erroring, because a skewed reference with an honest
label is more useful than no report.

```rust {#materialize-corpus}
/// Materialize the projector's inputs at `rev` under `<scratch>/corpus`.
/// `None` when the revision cannot be resolved or archived.
fn materialize_corpus_at(
    workspace: &Path,
    rev: &str,
    crates: &[String],
    scratch: &Path,
) -> Option<PathBuf> {
    if rev.is_empty() {
        return None;
    }
    let (git_dir, commit) = resolve_commit(workspace, rev)?;
    let root = scratch.join("corpus");
    std::fs::create_dir_all(&root).ok()?;
    let mut paths: Vec<String> = vec![
        "knowledge/implementation".into(),
        "decisions/publications".into(),
        "config".into(),
        "ontology/modules".into(),
    ];
    paths.extend(crates.iter().cloned());
    // git archive refuses a pathspec that matches nothing; keep only
    // paths present at the commit.
    let present: Vec<String> = paths
        .into_iter()
        .filter(|p| git_path_exists(&git_dir, &commit, p))
        .collect();
    if present.is_empty() {
        return None;
    }
    let mut archive = std::process::Command::new("git")
        .arg("--git-dir")
        .arg(&git_dir)
        .args(["archive", "--format=tar", &commit, "--"])
        .args(&present)
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
    Some(root)
}

fn git_path_exists(git_dir: &Path, commit: &str, path: &str) -> bool {
    std::process::Command::new("git")
        .arg("--git-dir")
        .arg(git_dir)
        .args(["cat-file", "-e", &format!("{commit}:{path}")])
        .stderr(std::process::Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}
```

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
the reference tree in hand:

```rust {#diff-and-classify}
fn diff_and_classify(clone: &Path, reference: &Path, prov: &Provenance) -> Result<Vec<ReceivedChange>> {
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
        let (class, target, produced_by) = classify(path, &old, &new, reference, prov);
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

The classifier is a short ladder, ordered so the more specific claim
wins. Overlay first — a maintainer's declaration outranks every
inference. Then the literate docs: a path in `path_map` maps back
through it; a *new* `.md` under `knowledge/implementation/` (Carol
contributing a literate document for a module that had none — the
on-ramp the publication doc invites) is receivable at the same path.
Sidecars and crate manifests are projector output. Anything left under
a published crate is source, unless its first line carries the
`@generated` marker — the reference copy's first line when it exists,
the clone's when the file is new — in which case it is refused with
its origin. Everything else is scaffolding.

```rust {#classify}
/// `(class, monorepo target, generated origin)` for one changed path.
fn classify(
    path: &str,
    old: &Option<String>,
    new: &Option<String>,
    reference: &Path,
    prov: &Provenance,
) -> (Class, Option<String>, Option<GeneratedOrigin>) {
    if prov.is_overlay(path) {
        return (Class::ProjectionLocal, None, None);
    }
    if let Some(canonical) = prov.canonical_for(path) {
        return (Class::Literate, Some(canonical.to_string()), None);
    }
    if path.starts_with("knowledge/implementation/") && path.ends_with(".md") {
        return (Class::Literate, Some(path.to_string()), None);
    }
    if path.ends_with(".tangle-map.json") {
        return (Class::ProjectionOwned, None, None);
    }
    let in_crate = path.split('/').count() >= 2 && reference.join(path.split('/').next().unwrap()).join("Cargo.toml").is_file();
    if !in_crate {
        return (Class::ProjectionOwned, None, None);
    }
    let crate_rel = &path[path.find('/').unwrap() + 1..];
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
        let origin = generated_origin(reference, path, old.as_deref(), new.as_deref());
        return (Class::Generated, None, Some(origin));
    }
    (Class::Source, Some(path.to_string()), None)
}
```

## Naming the chunk

A refusal that only says "generated, go away" is the fork-tending
experience the design exists to prevent. The doc is cheap to name: the
projection carries every literate doc's `<stem>.tangle-map.json`
sidecar, and the sidecar's `outputs[].path` names the tangled files.
The chunk is a little more work and is done best-effort: we parse the
doc, expand every named chunk, and for each line the contributor
touched on the reference side (a deleted line, or for an insertion the
nearest reference line above it, deleted or kept) pick the *innermost*
chunk whose expansion contains it
— innermost meaning smallest, since an outline chunk's expansion
contains everything beneath it. A line that appears in no chunk (the
generated header) contributes nothing; the doc alone is still named.

```rust {#generated-origin}
/// The doc (and, best-effort, the chunks) that produce `output` in the
/// reference projection.
fn generated_origin(reference: &Path, output: &str, old: Option<&str>, new: Option<&str>) -> GeneratedOrigin {
    let doc = doc_for_output(reference, output)
        .or_else(|| doc_from_header(old.or(new)?))
        .unwrap_or_else(|| "(unknown — no sidecar names this output)".to_string());
    let chunks = match (old, new, std::fs::read_to_string(reference.join(&doc))) {
        (Some(old), Some(new), Ok(text)) => touched_chunks(&text, old, new),
        _ => Vec::new(),
    };
    GeneratedOrigin { doc, chunks }
}

/// Walk the reference's sidecars for one whose outputs name `output`.
fn doc_for_output(reference: &Path, output: &str) -> Option<String> {
    let root = reference.join("knowledge/implementation");
    for entry in walkdir::WalkDir::new(&root).into_iter().filter_map(|e| e.ok()) {
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
    std::fs::write(
        dir.join("receipt.json"),
        serde_json::to_string_pretty(&serde_json::json!({
            "schema": "x0k.receipt/v1",
            "report": report,
        }))?,
    )?;
    Ok(())
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

```rust {#tests}
#[cfg(test)]
mod tests {
    use super::*;

    fn prov() -> Provenance {
        Provenance {
            publication_uri: "x0k:publication/demo".into(),
            corpus_rev: String::new(),
            path_map: [(
                "knowledge/implementation/folio/colophon.md".to_string(),
                "knowledge/implementation/folio/colophon.md".to_string(),
            )]
            .into_iter()
            .collect(),
            overlay: vec!["CONTRIBUTING.md".into(), "docs/".into()],
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
        let gen = Some("// @generated by x0k-tangle from knowledge/implementation/folio/colophon.md — DO NOT EDIT.\nfn a() {}\n".to_string());
        let hand = Some("fn b() {}\n".to_string());
        let cases: Vec<(&str, &Option<String>, Class, Option<&str>)> = vec![
            ("CONTRIBUTING.md", &hand, Class::ProjectionLocal, None),
            ("knowledge/implementation/folio/colophon.md", &hand, Class::Literate, Some("knowledge/implementation/folio/colophon.md")),
            ("knowledge/implementation/folio/new-chapter.md", &hand, Class::Literate, Some("knowledge/implementation/folio/new-chapter.md")),
            ("knowledge/implementation/folio/colophon.tangle-map.json", &hand, Class::ProjectionOwned, None),
            ("x0k-folio/Cargo.toml", &hand, Class::ProjectionOwned, None),
            ("x0k-folio/src/colophon.rs", &gen, Class::Generated, None),
            ("x0k-folio/src/hand.rs", &hand, Class::Source, Some("x0k-folio/src/hand.rs")),
            ("README.md", &hand, Class::ProjectionOwned, None),
            ("tools/ci", &hand, Class::ProjectionOwned, None),
        ];
        for (path, old, want, target) in cases {
            let (class, got_target, origin) = classify(path, old, &hand, reference, &p);
            assert_eq!(class, want, "{path}");
            assert_eq!(got_target.as_deref(), target, "{path}");
            if class == Class::Generated {
                let origin = origin.expect("generated names its origin");
                assert_eq!(origin.doc, "knowledge/implementation/folio/colophon.md");
            }
        }
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
        let doc = "---\nx0k:\n  format: folio/v1\n  id: x0k:implementation/t/d\n  type: implementation\n  tangle:\n    crate: c\n    root: src/lib.rs\n---\n\n```rust {#inner}\nfn inner() -> u8 { 1 }\n```\n\n```rust {#root}\nfn outer() {}\n<<inner>>\n```\n";
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
//! - a workspace with no VCS still receives, with `rev_exact: false`.
```

`Class` is reached through its module rather than the crate root: the
receiver's classification vocabulary is deliberately not part of the top-level
surface, and the test speaks the same name a consumer would have to.

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

```rust {#receive-e2e-paths file="tests/receive_repo.rs"}
const DOC_REL: &str = "knowledge/implementation/demo/colophon.md";
const PUB_REL: &str = "decisions/publications/demo.md";
```

```rust {#receive-e2e-fixture-docs file="tests/receive_repo.rs"}
const DOC: &str = "---\nx0k:\n  format: folio/v1\n  id: x0k:implementation/demo/colophon\n  type: implementation\n  status: draft\n  summary: The demo crate's one exported function, and where it trims.\n  tangle:\n    crate: demo-crate\n    root: src/lib.rs\n---\n# The demo colophon\n\nThe envelope parser tolerates keys it does not own. The first line\nis the whole contract.\n\n```rust {#parse-line}\n/// First line of `s`, trimmed.\npub fn parse_line(s: &str) -> &str {\n    s.lines().next().unwrap_or(\"\").trim()\n}\n```\n\n```rust {#root}\npub mod hand;\n\n<<parse-line>>\n```\n";

const PUB: &str = "---\nx0k:\n  format: folio/v1\n  type: publication\n  id: x0k:publication/demo\n  status: proposed\n  license: MIT\n  copyright: Demo Authors\n  edges:\n    publishes:\n      - x0k:software-module/demo-crate\n  tangle:\n    root: README.md\n---\n# Demo\n\n```markdown {#readme}\n# Demo\n\nA demo publication.\n\n<!-- x0k:contents -->\n```\n";
```

`git` runs with identity and signing pinned inline. A test that inherited
the operator's `user.email` or a global `commit.gpgsign = true` would pass on
one machine and hang on another:

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

```rust {#receive-e2e-project file="tests/receive_repo.rs"}
fn project(ws: &Path, clone: &Path) {
    project_publication_repo(
        &ws.join(PUB_REL),
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
    assert!(patch.contains("+The envelope parser tolerates keys it does not own itself."));
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

Finally, the honest degradation. A workspace with no VCS cannot be
materialized at a commit, so the reference is built from the tree as it stands
and the report says `rev_exact: false` rather than pretending. The receiver
still works; the maintainer just knows the comparison is against now, not
against the projection.

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

<<receive-e2e-no-vcs>>
```

## Composing the module

```rust {#root}
<<module-doc>>

<<classification>>

<<origin>>

<<change>>

<<options-and-report>>

<<receive-repo>>

<<provenance>>

<<find-publication-doc>>

<<reference>>

<<published-crates>>

<<materialize-corpus>>

<<resolve-commit>>

<<diff-and-classify>>

<<classify>>

<<generated-origin>>

<<touched-chunks>>

<<unified-patch>>

<<write-patch-set>>

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
