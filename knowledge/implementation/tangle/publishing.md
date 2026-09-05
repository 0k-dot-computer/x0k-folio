---
x0k:
  format: folio/v1
  id: x0k:implementation/tangle/publishing
  type: implementation
  status: draft
  summary: The stages between a repository-shaped artifact and a public one, arranged so everything reversible runs by default and the irreversible acts — `cargo publish`, a push to a public remote — sit behind one explicit flag.
  concerns: [tangle, publication, publishing, crates-io, git]
  tangle:
    crate: x0k-tangle
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
3. **Rehearse** — one `cargo publish --dry-run --workspace`: cargo
   packages and verifies every crate in the bundle in a single run,
   ordering them itself and satisfying each crate's in-bundle
   dependency from the workspace rather than from the registry. This is
   the only rehearsal that can pass before the first release, and it is
   also exactly the invocation stage 4 performs for real, which is the
   point of a rehearsal. The dependency order is still computed and
   reported (see below) so the operator can see what cargo will do.
4. **Publish** — the same `cargo publish --workspace` without
   `--dry-run`, and the `git push` to the remote the publication doc
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

<a name="chunk-module-doc"></a><sub>[`src/publish_repo.rs`](../../../x0k-tangle/src/publish_repo.rs) · `#module-doc`</sub>

```rust {#module-doc}
//! Publish pipeline for a projected repository — project, prove,
//! rehearse, and (operator-only, behind `really`) publish.
//!
//! Wraps [`crate::region_repo`]: projects the publication with guards on,
//! builds + tests the projection standalone with a private target dir,
//! runs `cargo publish --dry-run` per crate in dependency order, and —
//! only under `really: true` — runs the real `cargo publish` and pushes
//! the projected git history to the remote the publication doc's
//! `publishedOn:` edge names (resolved via `[publish.remotes]` in
//! `config/x0k-tangle.toml`). The default run stops after the rehearsal
//! and reports; nothing outward-facing happens without the flag.

use anyhow::{anyhow, bail, Context, Result};
use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use crate::region_repo::{project_publication_repo, RepoProjectOptions, RepoProjectReport};
use x0k_folio::colophon::parse_envelope;
```

## Options and report

<a name="chunk-options-and-report"></a><sub>[`src/publish_repo.rs`](../../../x0k-tangle/src/publish_repo.rs) · `#options-and-report`</sub>

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
    /// The one dry-run rehearsal over the whole bundle. `None` when the
    /// projection did not build or test green and the rehearsal never ran.
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

<a name="chunk-publish-repo"></a><sub>[`src/publish_repo.rs`](../../../x0k-tangle/src/publish_repo.rs) · `#publish-repo` · assembles [really-publish](#chunk-really-publish)</sub>

```rust {#publish-repo}
/// Run the publish pipeline for the publication doc at `region_doc`,
/// projecting into `output_dir` against `workspace`.
pub fn publish_repo(
    region_doc: &Path,
    output_dir: &Path,
    workspace: &Path,
    opts: &PublishRepoOptions,
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

    let mut report = PublishRepoReport {
        publish_order: publish_order(output_dir, &projection.crates)?,
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

    // Rehearse: ONE dry run over the whole bundle. Not per crate — see
    // `PublishRehearsal` for why a per-crate gate cannot pass a first
    // release. This is the same invocation the real publish uses.
    let (ok, tail) = cargo_in(
        output_dir,
        &target_dir,
        &["publish", "--dry-run", "--allow-dirty", "--workspace"],
    )?;
    report.rehearsal = Some(PublishRehearsal {
        ok,
        output_tail: tail,
    });

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
forge-specific beyond a URL.

<a name="chunk-really-publish"></a><sub>[`src/publish_repo.rs`](../../../x0k-tangle/src/publish_repo.rs) · `#really-publish`</sub>

```rust {#really-publish}
if !report.rehearsal.as_ref().is_some_and(|r| r.ok) {
    bail!("refusing --really publish: the dry-run rehearsal did not pass");
}
let (ok, tail) = cargo_in(output_dir, &target_dir, &["publish", "--workspace"])?;
if !ok {
    bail!("cargo publish --workspace failed:\n{tail}");
}
report.published = true;
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

<a name="chunk-publish-order"></a><sub>[`src/publish_repo.rs`](../../../x0k-tangle/src/publish_repo.rs) · `#publish-order`</sub>

```rust {#publish-order}
/// Topological publish order (dependencies first) over the projected
/// crates, from their vendored manifests' path deps.
fn publish_order(output_dir: &Path, crates: &[String]) -> Result<Vec<String>> {
    let set: BTreeSet<&str> = crates.iter().map(|s| s.as_str()).collect();
    // crate → its in-bundle deps.
    let mut deps: BTreeMap<&str, BTreeSet<&str>> = BTreeMap::new();
    for name in crates {
        let manifest_path = output_dir.join(name).join("Cargo.toml");
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

## Running cargo, resolving the remote

`cargo_in` runs one cargo invocation in the projection with the private
target dir, capturing output and returning success plus a bounded tail
— the caller's report stays readable while the full output remains on
the process's stderr for anyone watching the run live.

<a name="chunk-cargo-in"></a><sub>[`src/publish_repo.rs`](../../../x0k-tangle/src/publish_repo.rs) · `#cargo-in`</sub>

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
github = "git@github.com:0k-dot-computer/x0k-folio.git"
```

The split keeps the publication doc forge-light: the doc commits to *a
surface*, the config owns the concrete URL, and moving forges is a
config edit that touches no decision document.

<a name="chunk-resolve-remote"></a><sub>[`src/publish_repo.rs`](../../../x0k-tangle/src/publish_repo.rs) · `#resolve-remote`</sub>

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

The unit-testable pieces are the order computation and the remote
resolution; the pipeline's cargo stages are exercised by the
publication e2e (`tests/integration/tests/publication_publishing_e2e.rs`),
which owns a real projected workspace.

<a name="chunk-tests"></a><sub>[`src/publish_repo.rs`](../../../x0k-tangle/src/publish_repo.rs) · `#tests`</sub>

```rust {#tests}
#[cfg(test)]
mod tests {
    use super::*;

    fn write_crate(dir: &Path, name: &str, deps: &[&str]) {
        let crate_dir = dir.join(name);
        std::fs::create_dir_all(crate_dir.join("src")).unwrap();
        let mut manifest = format!(
            "[package]\nname = \"{name}\"\nversion = \"0.1.0\"\nedition = \"2021\"\n\n[dependencies]\n"
        );
        for d in deps {
            manifest.push_str(&format!("{d} = {{ path = \"../{d}\" }}\n"));
        }
        std::fs::write(crate_dir.join("Cargo.toml"), manifest).unwrap();
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

<a name="chunk-root"></a><sub>[`src/publish_repo.rs`](../../../x0k-tangle/src/publish_repo.rs) · `#root` · assembles [module-doc](#chunk-module-doc) · [options-and-report](#chunk-options-and-report) · [publish-repo](#chunk-publish-repo) · [publish-order](#chunk-publish-order) · [cargo-in](#chunk-cargo-in) · [resolve-remote](#chunk-resolve-remote) · [tests](#chunk-tests)</sub>

```rust {#root}
<<module-doc>>

<<options-and-report>>

<<publish-repo>>

<<publish-order>>

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
