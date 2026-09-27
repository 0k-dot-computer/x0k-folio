---
x0k:
  format: folio/v1
  id: x0k:implementation/tangle/publishing
  type: implementation
  status: draft
  summary: The stages between a repository-shaped artifact and a public one, arranged so everything reversible runs by default and the irreversible acts — `cargo publish`, a push to a public remote — sit behind one explicit flag.
  concerns:
  - tangle
  - publication
  - publishing
  - crates-io
  - git
  tangle:
    crate: crates/x0k-tangle
    root: src/publish_repo.rs
  edges:
    implements:
    - x0k:design/publish-a-region-as-a-repository
    cites:
    - x0k:implementation/folio/colophon
---
# Publishing a projected repository

Projection (`region_repo`) makes a repository-shaped *artifact*;
publishing makes it *public*. The two are different acts with different
blast radii: a projection into a scratch directory is free to repeat,
while `cargo publish` to crates.io and a `git push` to a public remote
are outward-facing and effectively irreversible. This module is the
pipeline between them, built so that everything reversible runs by
default and everything irreversible sits behind one explicit flag.

Given a publication doc and an output directory, `publish_repo` runs
four stages:

1. **Project** — `project_publication_repo`, guards on (never
   `allow_dirty`): the leak, closure, and publish-exclusion checks are
   the disclosure boundary and a publish pipeline has no business
   bypassing them. The projection goes *into* the output directory,
   not beside it: an output directory that already holds a projection
   keeps its `.git`, has its overlay paths preserved, and receives the
   run as one new commit on top of the existing history (none when
   nothing changed), so repeated publishes of the same publication
   append to one continuous public history rather than each minting a
   fresh root — the base an outside contribution needs.
2. **Prove** — build and test the projection standalone, with a private
   target dir inside the output (the repo's own `.gitignore` already
   covers it). What ships is what compiled, not what the monorepo
   compiled.
3. **Rehearse** — ask the registry index which version of each crate
   it already serves (see *Asking the index* below), then one
   `cargo publish --dry-run --workspace` with every already-published
   crate `--exclude`d: cargo packages and verifies the rest in a single
   run, ordering them itself and satisfying each crate's in-bundle
   dependency from the workspace rather than from the registry. This is
   the only rehearsal that can pass before the first release, and it is
   also exactly the invocation stage 4 performs for real, which is the
   point of a rehearsal. The dependency order is still computed and
   reported (see below) so the operator can see what cargo will do.
   When the index already serves every publishable crate's version
   there is nothing to rehearse, and the report says so.
4. **Publish** — the same `cargo publish --workspace --exclude …`
   without `--dry-run` (skipped when nothing is pending), and the
   `git push` to the remote the publication doc
   names. **Operator-only:** both run solely under `really: true`; the
   default invocation reports what *would* happen and stops. The remote comes from the doc's
   `publishedOn:` edge (`x0k:surface/<name>`) resolved through
   `config/x0k-tangle.toml`'s `[publish.remotes]` table — the doc names
   the surface, the config owns the URL, and a missing entry is a
   reported gap, not an error.

Placement note: this is build-time tooling that operates on the
workspace and spawns `cargo`/`git` — world-touching orchestration around
the substrate's edges, not pure logic a cell could host. Per the
residency test, a module in the plain `x0k-tangle` crate (beside the
projector it drives) is the right home.

<a name="chunk-module-doc"></a><sub>[`src/publish_repo.rs`](../../crates/x0k-tangle/src/publish_repo.rs) · `#module-doc`</sub>

```rust {#module-doc}
//! Publish pipeline for a projected repository — project, prove,
//! rehearse, and (operator-only, behind `really`) publish.
//!
//! Wraps [`crate::region_repo`]: projects the publication with guards on,
//! builds + tests the projection standalone with a private target dir,
//! asks the registry index which crate versions it already serves, runs
//! one `cargo publish --dry-run --workspace` excluding those, and —
//! only under `really: true` — runs the real `cargo publish` and pushes
//! the projected git history to the remote the publication doc's
//! `publishedOn:` edge names (resolved via `[publish.remotes]` in
//! `config/x0k-tangle.toml`). The default run stops after the rehearsal
//! and reports; nothing outward-facing happens without the flag.

use anyhow::{anyhow, bail, Context, Result};
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use crate::region_repo::{project_publication_repo, RepoProjectOptions, RepoProjectReport};
use x0k_folio::colophon::parse_envelope;
```

## Options and report

<a name="chunk-options-and-report"></a><sub>[`src/publish_repo.rs`](../../crates/x0k-tangle/src/publish_repo.rs) · `#options-and-report`</sub>

```rust {#options-and-report}
/// Options for a publish-repo run.
#[derive(Debug, Clone, Default)]
pub struct PublishRepoOptions {
    /// Explicit SPDX license override, passed through to the projection.
    /// `None` keeps the publication doc's `license:` authoritative.
    pub license: Option<String>,
    /// Emit the `.github/workflows/` thin wrappers in the projection.
    pub emit_github: bool,
    /// Actually run `cargo publish` (no `--dry-run`) and `git push`.
    /// Operator-only; the default run stops after the dry-run rehearsal.
    pub really: bool,
}

/// The bundle's `cargo publish --dry-run --workspace` outcome. One
/// rehearsal, not one per crate: a per-crate dry run of a dependent
/// crate cannot pass before its dependencies are on the registry
/// (cargo verifies the packaged tarball against the live index and the
/// in-bundle `version` deps are not there yet), so a per-crate gate
/// reports red on a bundle that is perfectly publishable. The workspace
/// run resolves the bundle against itself and is the same invocation the
/// real publish uses.
#[derive(Debug, Clone)]
pub struct PublishRehearsal {
    pub ok: bool,
    /// Tail of the cargo output — enough to see the verdict or the
    /// first error without dumping full logs into the caller.
    pub output_tail: String,
}

/// Report of a publish-repo run. Every stage records its outcome; later
/// stages after a failure are skipped and stay at their defaults.
#[derive(Debug)]
pub struct PublishRepoReport {
    pub projection: RepoProjectReport,
    pub build_ok: bool,
    pub test_ok: bool,
    /// Publish order computed from in-bundle path deps (dependencies
    /// before dependents). Reported so the operator can see the order
    /// cargo will publish in; cargo performs the ordering itself.
    pub publish_order: Vec<String>,
    /// Every projected crate in `publish_order`, with its manifest
    /// version and what the registry index said about it. The pending
    /// entries are exactly what the rehearsal and the real publish
    /// hand to cargo.
    pub plan: Vec<PlannedCrate>,
    /// The one dry-run rehearsal over the pending crates. `None` when the
    /// projection did not build or test green and the rehearsal never
    /// ran, or when nothing is pending and there was nothing to rehearse.
    pub rehearsal: Option<PublishRehearsal>,
    /// The remote URL the `publishedOn:` surface resolved to, when
    /// configured.
    pub remote: Option<String>,
    /// The surface URI the publication doc names (e.g.
    /// `x0k:surface/github`), resolved or not.
    pub surface: Option<String>,
    /// Real publishes + push happened (only ever true under `really`).
    pub published: bool,
    pub pushed: bool,
}
```

## The pipeline

The stages run in sequence, each gating the next: a projection that
fails guards never builds, a build that fails never rehearses, and the
rehearsal happens even when `really` is set — a real publish with a
failing dry-run behind it is exactly the accident the rehearsal exists
to prevent.

<a name="chunk-publish-repo"></a><sub>[`src/publish_repo.rs`](../../crates/x0k-tangle/src/publish_repo.rs) · `#publish-repo` · assembles [really-publish](#chunk-really-publish)</sub>

```rust {#publish-repo}
/// Run the publish pipeline for the publication doc at `region_doc`,
/// projecting into `output_dir` against `workspace`, asking crates.io's
/// sparse index what is already published.
pub fn publish_repo(
    region_doc: &Path,
    output_dir: &Path,
    workspace: &Path,
    opts: &PublishRepoOptions,
) -> Result<PublishRepoReport> {
    publish_repo_with_index(region_doc, output_dir, workspace, opts, &SparseIndex::default())
}

/// [`publish_repo`] against an explicit registry index.
pub fn publish_repo_with_index(
    region_doc: &Path,
    output_dir: &Path,
    workspace: &Path,
    opts: &PublishRepoOptions,
    index: &dyn RegistryIndex,
) -> Result<PublishRepoReport> {
    // `git_init` here means "commit the projection": a fresh output dir
    // gets a root commit, an existing repo gets the run appended.
    let proj_opts = RepoProjectOptions {
        license: opts.license.clone(),
        git_init: true,
        allow_dirty: false,
        emit_github: opts.emit_github,
    };
    let projection = project_publication_repo(region_doc, output_dir, workspace, &proj_opts)?;

    // Ask the index before the minutes-long build: a run that cannot
    // tell what is already published should say so first.
    let order = publish_order(output_dir, &projection.crates)?;
    let plan = plan_publication(output_dir, &order, index)?;
    let mut report = PublishRepoReport {
        publish_order: order,
        plan,
        projection,
        build_ok: false,
        test_ok: false,
        rehearsal: None,
        remote: None,
        surface: None,
        published: false,
        pushed: false,
    };

    // Prove: standalone build + test, private target dir inside the
    // output (covered by the projected .gitignore).
    let target_dir = output_dir.join("target/publish");
    report.build_ok = cargo_in(output_dir, &target_dir, &["build", "--workspace"])?.0;
    if !report.build_ok {
        return Ok(report);
    }
    report.test_ok = cargo_in(output_dir, &target_dir, &["test", "--workspace"])?.0;
    if !report.test_ok {
        return Ok(report);
    }

    // Rehearse: ONE dry run over the pending crates. Not per crate — see
    // `PublishRehearsal` for why a per-crate gate cannot pass a first
    // release. This is the same invocation the real publish uses; with
    // nothing pending there is no invocation to rehearse.
    if let Some(args) = publish_args(&report.plan, true) {
        let args: Vec<&str> = args.iter().map(String::as_str).collect();
        let (ok, tail) = cargo_in(output_dir, &target_dir, &args)?;
        report.rehearsal = Some(PublishRehearsal {
            ok,
            output_tail: tail,
        });
    }

    // Resolve the remote the publication names, whether or not we push:
    // the report should show where a real publish would land.
    let (surface, remote) = resolve_remote(region_doc, workspace)?;
    report.surface = surface;
    report.remote = remote;

    if opts.really {
        <<really-publish>>
    }

    Ok(report)
}
```

The `--allow-dirty` on the *dry-run* deserves a note so it is not
mistaken for guard-bypassing: it is cargo's own flag about uncommitted
VCS state, needed because the rehearsal writes `Cargo.lock` and the
private target dir into the fresh projection clone. The projector's
disclosure guards are a different mechanism and stay on.

## The irreversible half

Everything in this chunk is outward-facing: a version number burned on
crates.io, history on a public remote. It runs only under `really`, it
refuses to start unless every rehearsal passed, and the push goes to
the configured remote exactly as `git push <url> HEAD:main` — nothing
forge-specific beyond a URL. A run with nothing pending publishes
nothing and still pushes: a documentation-only re-projection is a
real publish of the repository even when no crate version moved.

<a name="chunk-really-publish"></a><sub>[`src/publish_repo.rs`](../../crates/x0k-tangle/src/publish_repo.rs) · `#really-publish`</sub>

```rust {#really-publish}
if let Some(args) = publish_args(&report.plan, false) {
    if !report.rehearsal.as_ref().is_some_and(|r| r.ok) {
        bail!("refusing --really publish: the dry-run rehearsal did not pass");
    }
    let args: Vec<&str> = args.iter().map(String::as_str).collect();
    let (ok, tail) = cargo_in(output_dir, &target_dir, &args)?;
    if !ok {
        bail!("cargo {} failed:\n{tail}", args.join(" "));
    }
    report.published = true;
}
let Some(remote) = report.remote.clone() else {
    bail!(
        "no remote configured for {} — add it under [publish.remotes] in config/x0k-tangle.toml",
        report.surface.as_deref().unwrap_or("(no publishedOn edge)")
    );
};
let status = std::process::Command::new("git")
    .current_dir(output_dir)
    .args(["push", &remote, "HEAD:main"])
    .status()
    .context("running git push")?;
if !status.success() {
    bail!("git push to {remote} failed");
}
report.pushed = true;
```

## Dependency order

Publish order is a topological sort over the projected crates' in-bundle
path deps (Kahn's algorithm, ties broken alphabetically for
determinism). Computing it from the vendored manifests rather than
hard-coding the current four names means a future publication with a
different membership publishes correctly with no code change.

Where a vendored manifest sits is asked of the tree, not assumed. The
projector's relayout put every published crate under `crates/<name>`,
and this verb went on reading `<name>/Cargo.toml` at the projection
root until the 0.1.1 rehearsal (2026-09-23) failed on the first crate
before a single cargo stage ran. Both layouts are accepted, `crates/`
first; a crate found under neither is named with both paths it was
looked for at, since "No such file" for one of them is what hid this.

<a name="chunk-vendored-manifest"></a><sub>[`src/publish_repo.rs`](../../crates/x0k-tangle/src/publish_repo.rs) · `#vendored-manifest`</sub>

```rust {#vendored-manifest}
/// A vendored crate's manifest under `crates/<name>` (the projector's
/// layout) or `<name>` (the layout before the relayout), whichever the
/// tree actually holds.
fn vendored_manifest(output_dir: &Path, krate: &str) -> Result<PathBuf> {
    let candidates = [
        output_dir.join("crates").join(krate).join("Cargo.toml"),
        output_dir.join(krate).join("Cargo.toml"),
    ];
    candidates
        .iter()
        .find(|p| p.is_file())
        .cloned()
        .ok_or_else(|| {
            anyhow!(
                "no vendored manifest for crate {krate}: looked at {} and {}",
                candidates[0].display(),
                candidates[1].display()
            )
        })
}
```

<a name="chunk-publish-order"></a><sub>[`src/publish_repo.rs`](../../crates/x0k-tangle/src/publish_repo.rs) · `#publish-order`</sub>

```rust {#publish-order}
/// Topological publish order (dependencies first) over the projected
/// crates, from their vendored manifests' path deps.
fn publish_order(output_dir: &Path, crates: &[String]) -> Result<Vec<String>> {
    let set: BTreeSet<&str> = crates.iter().map(|s| s.as_str()).collect();
    // crate → its in-bundle deps.
    let mut deps: BTreeMap<&str, BTreeSet<&str>> = BTreeMap::new();
    for name in crates {
        let manifest_path = vendored_manifest(output_dir, name)?;
        let text = std::fs::read_to_string(&manifest_path)
            .with_context(|| format!("reading {}", manifest_path.display()))?;
        let doc = text
            .parse::<toml_edit::DocumentMut>()
            .with_context(|| format!("parsing {}", manifest_path.display()))?;
        let entry = deps.entry(name.as_str()).or_default();
        if let Some(table) = doc.get("dependencies").and_then(|d| d.as_table()) {
            for (_, item) in table.iter() {
                let Some(path) = item
                    .as_table_like()
                    .and_then(|t| t.get("path"))
                    .and_then(|p| p.as_str())
                else {
                    continue;
                };
                let target = Path::new(path)
                    .file_name()
                    .and_then(|s| s.to_str())
                    .unwrap_or(path);
                if let Some(t) = set.get(target) {
                    entry.insert(t);
                }
            }
        }
    }
    let mut order: Vec<String> = Vec::with_capacity(crates.len());
    let mut placed: BTreeSet<&str> = BTreeSet::new();
    while placed.len() < set.len() {
        let ready: Vec<&str> = deps
            .iter()
            .filter(|(n, d)| !placed.contains(*n) && d.iter().all(|x| placed.contains(x)))
            .map(|(n, _)| *n)
            .collect();
        if ready.is_empty() {
            bail!("dependency cycle among published crates: {set:?}");
        }
        for n in ready {
            placed.insert(n);
            order.push(n.to_string());
        }
    }
    Ok(order)
}
```

## Asking the index what is already published

`cargo publish --workspace` uploads every publishable member, and a
member whose manifest version the registry already holds is not
skipped — in a dry run cargo prints `warning: crate x0k-folio@0.1.1
already exists on crates.io index` and exits 0, and the real run stops
on that same check before uploading anything. So a bundle in which
only some crates moved (0.1.1 bumped three of seven) rehearses green
and then cannot be published by the invocation it rehearsed: 0.1.1
went out as three `cargo publish -p` runs typed by hand (reproduced
2026-09-25 on a clone of the 0.1.1 projection: seven "already exists"
warnings, `EXIT=0`). The verb therefore asks the index first and hands
cargo only what it does not serve.

**Which question, asked how.** The question is exact: *does the index
serve `name@version`?* Three ways to ask it were weighed:

- `cargo search <name>` answers a different question — the newest
  version of the best-matching crates, not whether a given version
  exists — and cannot tell `0.1.0` published from `0.1.0` never
  published once `0.1.1` is out.
- `cargo info <name>@<version>` answers it, but prefers a local
  workspace member of the same name to the registry (so it must run
  outside the projection to mean anything) and prints a human layout,
  not a format.
- The **sparse index** — `https://index.crates.io/<prefix>/<name>`,
  one JSON object per published version, 404 for a crate the registry
  has never seen — is the file cargo's own resolver reads, needs no
  auth, and is a format. That is the one used.

It is fetched with `curl`, spawned like `cargo` and `git` already are,
rather than by linking an HTTP client into a crate that is itself
published: the verb is corpus-only orchestration, and a TLS stack in
every `x0k-tangle` build would be paid for by users who never reach
it. The fetch sits behind a one-method trait so the planning below is
tested against a stub, not the network.

A yanked version counts as published: crates.io refuses to re-upload a
yanked version, so publishing it would fail the same way.

<a name="chunk-registry-index"></a><sub>[`src/publish_repo.rs`](../../crates/x0k-tangle/src/publish_repo.rs) · `#registry-index`</sub>

```rust {#registry-index}
/// What a registry index already serves, asked per crate.
pub trait RegistryIndex {
    /// Every version the index lists for `name`, yanked included. Empty
    /// when the index has never seen the crate.
    fn published_versions(&self, name: &str) -> Result<BTreeSet<String>>;
}

/// A cargo sparse index over HTTPS (crates.io's by default), fetched
/// with `curl`.
#[derive(Debug, Clone)]
pub struct SparseIndex {
    pub base: String,
}

impl Default for SparseIndex {
    fn default() -> Self {
        Self {
            base: "https://index.crates.io".to_string(),
        }
    }
}

/// A crate's path in a cargo index: `1/a`, `2/ab`, `3/a/abc`,
/// `ab/cd/abcd…`, lowercased.
fn sparse_index_path(name: &str) -> String {
    let name = name.to_ascii_lowercase();
    match name.len() {
        1 => format!("1/{name}"),
        2 => format!("2/{name}"),
        3 => format!("3/{}/{name}", &name[..1]),
        _ => format!("{}/{}/{name}", &name[..2], &name[2..4]),
    }
}

/// The `vers` of every line of a sparse-index file.
fn parse_index_versions(body: &str) -> Result<BTreeSet<String>> {
    let mut versions = BTreeSet::new();
    for line in body.lines().filter(|l| !l.trim().is_empty()) {
        let entry: serde_json::Value =
            serde_json::from_str(line).context("parsing a sparse-index line")?;
        let vers = entry
            .get("vers")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow!("sparse-index line without `vers`: {line}"))?;
        versions.insert(vers.to_string());
    }
    Ok(versions)
}

impl RegistryIndex for SparseIndex {
    fn published_versions(&self, name: &str) -> Result<BTreeSet<String>> {
        let url = format!("{}/{}", self.base.trim_end_matches('/'), sparse_index_path(name));
        let out = std::process::Command::new("curl")
            .args(["-sS", "--location", "--write-out", "\n%{http_code}", &url])
            .output()
            .with_context(|| format!("running curl {url}"))?;
        if !out.status.success() {
            bail!(
                "fetching {url}: {}",
                String::from_utf8_lossy(&out.stderr).trim()
            );
        }
        let text = String::from_utf8_lossy(&out.stdout);
        let (body, code) = text.rsplit_once('\n').unwrap_or(("", text.as_ref()));
        tracing::debug!(url = %url, status = code, "publish.index.fetched");
        // Cargo reads 404, 410 and 451 as "no such crate"; anything else
        // but 200 means the index could not be asked, and a publish plan
        // is not guessed around that.
        match code.trim() {
            "200" => parse_index_versions(body),
            "404" | "410" | "451" => Ok(BTreeSet::new()),
            other => bail!("index {url} answered HTTP {other}"),
        }
    }
}
```

**What each crate is.** A projected crate is one of three things, read
from its vendored manifest and then from the index:

- **Unpublishable** — `publish = false` (or `publish = []`, or no
  `version`, which cargo treats the same way). Cargo's workspace mode
  already skips these: `cargo publish --workspace` passes over them
  without a word, while naming one with `-p` is a hard error
  (`` `x0k-folio-dialog` cannot be published``; both observed
  2026-09-25). The verb leaves that skip to cargo — it never names an
  unpublishable crate to cargo in any form — and reads the flag only so
  it neither asks the index about the crate nor reports it as pending.
- **Already published** — the index serves the manifest's version.
  Passed to cargo as `--exclude <name>`.
- **Pending** — everything else. Its version goes up.

Using `--workspace --exclude` rather than a list of `-p` flags is what
keeps the `publish = false` rule cargo's: the selection is still
"every publishable member", minus what the index already has.

<a name="chunk-publication-plan"></a><sub>[`src/publish_repo.rs`](../../crates/x0k-tangle/src/publish_repo.rs) · `#publication-plan`</sub>

```rust {#publication-plan}
/// What the registry index said about one projected crate.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CrateDisposition {
    /// `publish = false` (or no version): cargo's workspace mode skips
    /// it, and the index is never asked.
    Unpublishable,
    /// The index already serves this manifest version.
    AlreadyPublished,
    /// The index does not serve this version; the run publishes it.
    Pending,
}

/// One projected crate in publish order.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlannedCrate {
    pub name: String,
    /// The manifest version; empty for a crate with none.
    pub version: String,
    pub disposition: CrateDisposition,
}

/// A `[package]` field, following `field.workspace = true` to the
/// projection root's `[workspace.package]`.
fn package_field(
    doc: &toml_edit::DocumentMut,
    root: Option<&toml_edit::DocumentMut>,
    key: &str,
) -> Option<toml_edit::Item> {
    let item = doc.get("package")?.get(key)?;
    let inherited = item
        .as_table_like()
        .and_then(|t| t.get("workspace"))
        .and_then(|w| w.as_bool())
        == Some(true);
    if inherited {
        root?.get("workspace")?.get("package")?.get(key).cloned()
    } else {
        Some(item.clone())
    }
}

/// Classify every crate in `order` against `index`, keeping the order.
fn plan_publication(
    output_dir: &Path,
    order: &[String],
    index: &dyn RegistryIndex,
) -> Result<Vec<PlannedCrate>> {
    let root = std::fs::read_to_string(output_dir.join("Cargo.toml"))
        .ok()
        .and_then(|t| t.parse::<toml_edit::DocumentMut>().ok());
    let mut plan = Vec::with_capacity(order.len());
    for name in order {
        let manifest_path = vendored_manifest(output_dir, name)?;
        let doc = std::fs::read_to_string(&manifest_path)
            .with_context(|| format!("reading {}", manifest_path.display()))?
            .parse::<toml_edit::DocumentMut>()
            .with_context(|| format!("parsing {}", manifest_path.display()))?;
        let version = package_field(&doc, root.as_ref(), "version")
            .and_then(|v| v.as_str().map(str::to_string));
        let publish = package_field(&doc, root.as_ref(), "publish");
        let refuses = publish.as_ref().is_some_and(|p| {
            p.as_bool() == Some(false) || p.as_array().is_some_and(|a| a.is_empty())
        });
        let disposition = match &version {
            None => CrateDisposition::Unpublishable,
            Some(_) if refuses => CrateDisposition::Unpublishable,
            Some(v) if index.published_versions(name)?.contains(v) => {
                CrateDisposition::AlreadyPublished
            }
            Some(_) => CrateDisposition::Pending,
        };
        tracing::info!(
            krate = %name,
            version = version.as_deref().unwrap_or(""),
            disposition = ?disposition,
            "publish.plan.crate"
        );
        plan.push(PlannedCrate {
            name: name.clone(),
            version: version.unwrap_or_default(),
            disposition,
        });
    }
    Ok(plan)
}

/// The one `cargo publish` invocation for `plan` — `--workspace` minus
/// every already-published crate — or `None` when nothing is pending.
fn publish_args(plan: &[PlannedCrate], dry_run: bool) -> Option<Vec<String>> {
    if !plan
        .iter()
        .any(|c| c.disposition == CrateDisposition::Pending)
    {
        return None;
    }
    let mut args = vec!["publish".to_string()];
    if dry_run {
        args.push("--dry-run".to_string());
        args.push("--allow-dirty".to_string());
    }
    args.push("--workspace".to_string());
    for c in plan
        .iter()
        .filter(|c| c.disposition == CrateDisposition::AlreadyPublished)
    {
        args.push("--exclude".to_string());
        args.push(c.name.clone());
    }
    Some(args)
}
```

## Running cargo, resolving the remote

`cargo_in` runs one cargo invocation in the projection with the private
target dir, capturing output and returning success plus a bounded tail
— the caller's report stays readable while the full output remains on
the process's stderr for anyone watching the run live.

<a name="chunk-cargo-in"></a><sub>[`src/publish_repo.rs`](../../crates/x0k-tangle/src/publish_repo.rs) · `#cargo-in`</sub>

```rust {#cargo-in}
/// Run `cargo <args>` in `dir` with a private CARGO_TARGET_DIR. Returns
/// `(success, tail-of-combined-output)`.
fn cargo_in(dir: &Path, target_dir: &Path, args: &[&str]) -> Result<(bool, String)> {
    let out = std::process::Command::new("cargo")
        .current_dir(dir)
        .env("CARGO_TARGET_DIR", target_dir)
        .args(args)
        .output()
        .with_context(|| format!("running cargo {args:?}"))?;
    let mut combined = String::from_utf8_lossy(&out.stdout).to_string();
    combined.push_str(&String::from_utf8_lossy(&out.stderr));
    let tail: String = combined
        .lines()
        .rev()
        .take(15)
        .collect::<Vec<_>>()
        .into_iter()
        .rev()
        .collect::<Vec<_>>()
        .join("\n");
    Ok((out.status.success(), tail))
}
```

The remote resolution reads the publication doc's `publishedOn:` edge —
a surface URI like `x0k:surface/github` — and looks its short name up in
`config/x0k-tangle.toml`:

```toml
[publish.remotes]
github = "https://github.com/0k-dot-computer/x0k-folio"
```

The split keeps the publication doc forge-light: the doc commits to *a
surface*, the config owns the concrete URL, and moving forges is a
config edit that touches no decision document.

The URL is HTTPS, not SSH. The node that publishes needs push rights,
not a GitHub SSH key: over HTTPS `git push` asks the node's credential
helper, and a node signed in with `gh` (`gh auth setup-git`) answers
with its token. The 0.1.1 release was pushed exactly that way, by hand,
because the configured `git@github.com:` remote needed a key arca does
not hold.

<a name="chunk-resolve-remote"></a><sub>[`src/publish_repo.rs`](../../crates/x0k-tangle/src/publish_repo.rs) · `#resolve-remote`</sub>

```rust {#resolve-remote}
/// Resolve the publication's `publishedOn:` surface to a configured git
/// remote URL. Returns `(surface_uri, url)` — either may be `None` (no
/// edge; no config entry). Missing config is a reported gap, not an
/// error: the dry-run stages are useful without a remote.
fn resolve_remote(region_doc: &Path, workspace: &Path) -> Result<(Option<String>, Option<String>)> {
    let content = std::fs::read_to_string(region_doc)
        .with_context(|| format!("reading {}", region_doc.display()))?;
    let (env, _) = parse_envelope(&content).map_err(|e| anyhow!("parsing publication: {e}"))?;
    let Some(surface) = env
        .edges
        .get("publishedOn")
        .and_then(|v| v.first())
        .cloned()
    else {
        return Ok((None, None));
    };
    let Some(short) = surface.strip_prefix("x0k:surface/") else {
        return Ok((Some(surface), None));
    };
    let config_path = workspace.join("config/x0k-tangle.toml");
    let Ok(text) = std::fs::read_to_string(&config_path) else {
        return Ok((Some(surface), None));
    };
    let doc = text
        .parse::<toml_edit::DocumentMut>()
        .with_context(|| format!("parsing {}", config_path.display()))?;
    let url = doc
        .get("publish")
        .and_then(|p| p.get("remotes"))
        .and_then(|r| r.get(short))
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());
    Ok((Some(surface), url))
}
```

## Tests

The unit-testable pieces are the order computation, the publication
plan (against a stub index), and the remote resolution; the pipeline's
cargo stages are exercised by the publication e2e
(`proof/tests/integration/tests/publication_publishing_e2e.rs`), which
owns a real projected workspace. The manifest-path fix from the 0.1.1
rehearsal stays pinned three ways: the fixtures write the projector's
`crates/<name>` layout, the flat layout has its own test, and a missing
manifest must name both paths it was looked for at.

<a name="chunk-tests"></a><sub>[`src/publish_repo.rs`](../../crates/x0k-tangle/src/publish_repo.rs) · `#tests`</sub>

```rust {#tests}
#[cfg(test)]
mod tests {
    use super::*;

    // The projector's layout: every vendored crate under `crates/`.
    fn write_crate(dir: &Path, name: &str, deps: &[&str]) {
        write_crate_at(&dir.join("crates"), name, deps)
    }

    fn write_crate_at(dir: &Path, name: &str, deps: &[&str]) {
        write_manifest(dir, name, "version = \"0.1.0\"\n", deps)
    }

    /// A vendored crate under `crates/` whose `[package]` carries
    /// `package` (version, publish, …) verbatim.
    fn write_manifest(dir: &Path, name: &str, package: &str, deps: &[&str]) {
        let crate_dir = dir.join(name);
        std::fs::create_dir_all(crate_dir.join("src")).unwrap();
        let mut manifest = format!(
            "[package]\nname = \"{name}\"\n{package}edition = \"2021\"\n\n[dependencies]\n"
        );
        for d in deps {
            manifest.push_str(&format!("{d} = {{ path = \"../{d}\" }}\n"));
        }
        std::fs::write(crate_dir.join("Cargo.toml"), manifest).unwrap();
    }

    /// An index that serves a fixed table and records every crate it
    /// was asked about.
    struct StubIndex {
        served: BTreeMap<String, BTreeSet<String>>,
        asked: std::cell::RefCell<Vec<String>>,
    }

    impl StubIndex {
        fn serving(served: &[(&str, &[&str])]) -> Self {
            Self {
                served: served
                    .iter()
                    .map(|(n, vs)| (n.to_string(), vs.iter().map(|v| v.to_string()).collect()))
                    .collect(),
                asked: Default::default(),
            }
        }
    }

    impl RegistryIndex for StubIndex {
        fn published_versions(&self, name: &str) -> Result<BTreeSet<String>> {
            self.asked.borrow_mut().push(name.to_string());
            Ok(self.served.get(name).cloned().unwrap_or_default())
        }
    }

    #[test]
    fn plan_skips_the_published_orders_the_rest_and_never_attempts_unpublishable() {
        let tmp = tempfile::tempdir().unwrap();
        let crates = tmp.path().join("crates");
        // `base` is on the index at its version; `core` only at an older
        // one; `app` and `extra` not at all; `cli` refuses publication.
        write_manifest(&crates, "base", "version = \"0.1.0\"\n", &[]);
        write_manifest(&crates, "core", "version = \"0.2.0\"\n", &["base"]);
        write_manifest(&crates, "extra", "version = \"0.3.0\"\n", &["base"]);
        write_manifest(&crates, "app", "version = \"0.1.0\"\n", &["core"]);
        write_manifest(
            &crates,
            "cli",
            "version = \"0.1.0\"\npublish = false\n",
            &["app"],
        );
        let index = StubIndex::serving(&[("base", &["0.1.0"]), ("core", &["0.1.0"])]);
        let names: Vec<String> = ["cli", "app", "extra", "core", "base"]
            .iter()
            .map(|s| s.to_string())
            .collect();
        let order = publish_order(tmp.path(), &names).unwrap();
        let plan = plan_publication(tmp.path(), &order, &index).unwrap();

        let of = |n: &str| plan.iter().find(|c| c.name == n).unwrap().disposition;
        assert_eq!(
            of("base"),
            CrateDisposition::AlreadyPublished,
            "`base` 0.1.0 is on the index and must be skipped"
        );
        assert_eq!(of("core"), CrateDisposition::Pending, "an older version on the index is not this one");
        assert_eq!(
            of("cli"),
            CrateDisposition::Unpublishable,
            "publish = false crate `cli` was planned for publication"
        );

        let pending: Vec<&str> = plan
            .iter()
            .filter(|c| c.disposition == CrateDisposition::Pending)
            .map(|c| c.name.as_str())
            .collect();
        assert_eq!(pending, ["core", "extra", "app"], "pending crates, in dependency order");

        let asked = index.asked.borrow();
        assert!(
            !asked.iter().any(|n| n == "cli"),
            "the index was asked about publish = false crate `cli`: {asked:?}"
        );

        for dry_run in [true, false] {
            let args = publish_args(&plan, dry_run).expect("three crates are pending");
            assert!(
                !args.iter().any(|a| a == "cli"),
                "publish = false crate `cli` was named to cargo: {args:?}"
            );
            assert!(args.iter().any(|a| a == "--workspace"), "{args:?}");
            let excluded: Vec<&str> = args
                .windows(2)
                .filter(|w| w[0] == "--exclude")
                .map(|w| w[1].as_str())
                .collect();
            assert_eq!(excluded, ["base"], "only the already-published crate is excluded: {args:?}");
            assert_eq!(args.iter().any(|a| a == "--dry-run"), dry_run, "{args:?}");
        }
    }

    #[test]
    fn nothing_pending_means_no_cargo_invocation() {
        let tmp = tempfile::tempdir().unwrap();
        let crates = tmp.path().join("crates");
        write_manifest(&crates, "base", "version = \"0.1.0\"\n", &[]);
        write_manifest(&crates, "top", "version = \"0.1.1\"\n", &["base"]);
        write_manifest(&crates, "cli", "version = \"0.1.1\"\npublish = false\n", &["top"]);
        let index = StubIndex::serving(&[("base", &["0.1.0"]), ("top", &["0.1.0", "0.1.1"])]);
        let order = publish_order(
            tmp.path(),
            &["base".to_string(), "top".to_string(), "cli".to_string()],
        )
        .unwrap();
        let plan = plan_publication(tmp.path(), &order, &index).unwrap();
        assert!(plan
            .iter()
            .all(|c| c.disposition != CrateDisposition::Pending), "{plan:?}");
        assert_eq!(publish_args(&plan, true), None);
        assert_eq!(publish_args(&plan, false), None);
    }

    #[test]
    fn plan_follows_workspace_inherited_version_and_publish() {
        let tmp = tempfile::tempdir().unwrap();
        std::fs::write(
            tmp.path().join("Cargo.toml"),
            "[workspace]\nmembers = [\"crates/*\"]\n\n[workspace.package]\nversion = \"1.2.3\"\npublish = false\n",
        )
        .unwrap();
        let crates = tmp.path().join("crates");
        write_manifest(&crates, "inherits", "version.workspace = true\n", &[]);
        write_manifest(
            &crates,
            "private",
            "version.workspace = true\npublish.workspace = true\n",
            &[],
        );
        let index = StubIndex::serving(&[("inherits", &["1.2.3"])]);
        let plan = plan_publication(
            tmp.path(),
            &["inherits".to_string(), "private".to_string()],
            &index,
        )
        .unwrap();
        assert_eq!(plan[0].version, "1.2.3");
        assert_eq!(plan[0].disposition, CrateDisposition::AlreadyPublished);
        assert_eq!(plan[1].disposition, CrateDisposition::Unpublishable);
    }

    #[test]
    fn sparse_index_paths_follow_the_cargo_layout() {
        assert_eq!(sparse_index_path("a"), "1/a");
        assert_eq!(sparse_index_path("ab"), "2/ab");
        assert_eq!(sparse_index_path("abc"), "3/a/abc");
        assert_eq!(sparse_index_path("x0k-folio"), "x0/k-/x0k-folio");
        assert_eq!(sparse_index_path("Serde"), "se/rd/serde");
    }

    #[test]
    fn sparse_index_lines_yield_every_version_yanked_included() {
        let body = "{\"name\":\"x\",\"vers\":\"0.1.0\",\"yanked\":false}\n\
                    {\"name\":\"x\",\"vers\":\"0.1.1\",\"yanked\":true}\n";
        let versions = parse_index_versions(body).unwrap();
        assert_eq!(
            versions.into_iter().collect::<Vec<_>>(),
            ["0.1.0".to_string(), "0.1.1".to_string()]
        );
        assert!(parse_index_versions("{\"name\":\"x\"}").is_err());
    }

    #[test]
    fn publish_order_puts_dependencies_first() {
        let tmp = tempfile::tempdir().unwrap();
        write_crate(tmp.path(), "leaf-a", &[]);
        write_crate(tmp.path(), "leaf-b", &[]);
        write_crate(tmp.path(), "mid", &["leaf-a"]);
        write_crate(tmp.path(), "top", &["mid", "leaf-b"]);
        let order = publish_order(
            tmp.path(),
            &[
                "top".to_string(),
                "mid".to_string(),
                "leaf-a".to_string(),
                "leaf-b".to_string(),
            ],
        )
        .unwrap();
        let pos = |n: &str| order.iter().position(|x| x == n).unwrap();
        assert!(pos("leaf-a") < pos("mid"));
        assert!(pos("mid") < pos("top"));
        assert!(pos("leaf-b") < pos("top"));
    }

    #[test]
    fn publish_order_reads_the_flat_layout_too() {
        let tmp = tempfile::tempdir().unwrap();
        write_crate_at(tmp.path(), "leaf", &[]);
        write_crate_at(tmp.path(), "top", &["leaf"]);
        let order = publish_order(tmp.path(), &["top".to_string(), "leaf".to_string()]).unwrap();
        assert_eq!(order, vec!["leaf".to_string(), "top".to_string()]);
    }

    #[test]
    fn publish_order_names_both_paths_for_a_missing_manifest() {
        let tmp = tempfile::tempdir().unwrap();
        let err = publish_order(tmp.path(), &["ghost".to_string()]).expect_err("missing must refuse");
        let msg = format!("{err:#}");
        assert!(msg.contains("crates/ghost/Cargo.toml"), "{msg}");
        assert!(msg.contains("ghost/Cargo.toml"), "{msg}");
    }

    #[test]
    fn publish_order_rejects_cycles() {
        let tmp = tempfile::tempdir().unwrap();
        write_crate(tmp.path(), "a", &["b"]);
        write_crate(tmp.path(), "b", &["a"]);
        let err = publish_order(tmp.path(), &["a".to_string(), "b".to_string()])
            .expect_err("cycle must refuse");
        assert!(err.to_string().contains("cycle"));
    }

    #[test]
    fn resolve_remote_reads_surface_and_config() {
        let tmp = tempfile::tempdir().unwrap();
        let doc_path = tmp.path().join("pub.md");
        std::fs::write(
            &doc_path,
            "---\nx0k:\n  format: folio/v1\n  id: x0k:publication/x\n  type: publication\n  edges:\n    publishedOn:\n      - x0k:surface/github\n---\nbody\n",
        )
        .unwrap();
        // No config: surface resolves, remote does not.
        let (surface, remote) = resolve_remote(&doc_path, tmp.path()).unwrap();
        assert_eq!(surface.as_deref(), Some("x0k:surface/github"));
        assert!(remote.is_none());
        // With config: both resolve.
        std::fs::create_dir_all(tmp.path().join("config")).unwrap();
        std::fs::write(
            tmp.path().join("config/x0k-tangle.toml"),
            "[publish.remotes]\ngithub = \"git@example.com:org/repo.git\"\n",
        )
        .unwrap();
        let (_, remote) = resolve_remote(&doc_path, tmp.path()).unwrap();
        assert_eq!(remote.as_deref(), Some("git@example.com:org/repo.git"));
    }
}
```

## Composing the module

<a name="chunk-root"></a><sub>[`src/publish_repo.rs`](../../crates/x0k-tangle/src/publish_repo.rs) · `#root` · assembles [module-doc](#chunk-module-doc) · [options-and-report](#chunk-options-and-report) · [publish-repo](#chunk-publish-repo) · [vendored-manifest](#chunk-vendored-manifest) · [publish-order](#chunk-publish-order) · [registry-index](#chunk-registry-index) · [publication-plan](#chunk-publication-plan) · [cargo-in](#chunk-cargo-in) · [resolve-remote](#chunk-resolve-remote) · [tests](#chunk-tests)</sub>

```rust {#root}
<<module-doc>>

<<options-and-report>>

<<publish-repo>>

<<vendored-manifest>>

<<publish-order>>

<<registry-index>>

<<publication-plan>>

<<cargo-in>>

<<resolve-remote>>

<<tests>>
```

The candid seam in this design used to be stage 3: the rehearsal ran
`cargo publish --dry-run` once per crate, and a dependent crate's dry
run cannot pass before its dependencies are on crates.io — cargo
verifies the packaged tarball against the live index, and the in-bundle
`version` deps are not there yet. Measured in a real projection,
`cargo package -p x0k-tangle` fails with `no matching package named
'x0k-folio' found`. Dependency order does not rescue it: a dry run
publishes nothing, so the earlier crates never become resolvable. The
gate was therefore *unpassable on a first release* — red on a bundle
that was in fact publishable, which is the worst kind of gate.

The workspace dry run is the honest replacement rather than a
loosening: cargo resolves the bundle against itself, verifies every
tarball, and is the same invocation stage 4 runs for real, so a green
rehearsal now means what it says. The path not taken was
`--no-verify`, which would have made the per-crate gate pass by
skipping the build that is the whole content of the check.
