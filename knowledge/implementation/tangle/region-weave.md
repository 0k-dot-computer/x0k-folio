---
x0k:
  format: folio/v1
  id: x0k:implementation/tangle/region-weave
  type: implementation
  status: draft
  summary: Region weaving as pure post-processing over the single-document weaver — cross-document links, the site nav, the URI-to-file map — computed without reading or writing a file, which is what makes every rule testable with strings.
  concerns: [tangle, publication, weave, region]
  tangle:
    crate: x0k-tangle
    root: src/region_weave.rs
  edges:
    implements:
      - x0k:design/author-and-publish-the-same-surface
    cites:
      - x0k:implementation/tangle/region-project
      - x0k:implementation/tangle/atlas
      - x0k:implementation/tangle/presentation
      - x0k:implementation/folio/transclusion
    presupposes:
      - x0k:wiki/literate-programming
---

# Region weave: many documents, one artifact, no I/O

The single-document weaver turns one literate doc into one standalone HTML
page. A publication is many documents that link to each other, and a reader
who lands on one needs to reach the rest. This chapter is the layer between
those two facts: it takes a **region** — an ordered set of members plus an
entry point — weaves each member with the existing weaver, and then rewrites
the woven pages so the links between them resolve inside the artifact.

The central idea is that region weaving is *pure post-processing*. The
single-doc weaver is wrapped, never forked, and everything region-shaped —
cross-document links, the site nav, the URI-to-file map, the atlas — is
computed over its output and its input. The function reads no files and
writes none; the caller hands it content and receives bytes. That is what
makes every rule in this chapter unit-testable with strings.

```rust {#module-doc}
//! Region projection — the pure weave of a publication region.
//!
//! A *publication region* is an ordered set of decision docs (the membership
//! resolved from the publication doc's `publishes:` edge by
//! [`crate::region_project::parse_publication_region`]) plus an
//! entry-point member the reader lands on. This module projects that region
//! into a **self-contained, navigable, multi-page web artifact** by WRAPPING
//! (never forking) `crate::weave::weave_html` — the single-doc weaver that
//! already emits standalone, styled, syntax-highlighted HTML.
//!
//! The single-doc weaver gives us, per page:
//! - a complete `<!DOCTYPE html>` document with an inlined `<style>` block,
//! - intra-doc chunk anchors (`<a class="chunk-ref" href="#chunk-NAME">`),
//! - `x0k:media` embed mount points (`.media-embed[data-chunk]` /
//!   `[data-media-ref]`) with a static `.media-label` fallback,
//! - ordinary markdown links passed through verbatim.
//!
//! What it does *not* know about is the region: cross-doc links, a site nav
//! that ties the pages together, and which artifact file a member URI maps to.
//! Those are this module's job, applied as pure post-processing on the woven
//! HTML — no change to `weave.rs` is needed (and an anchor-token test guards
//! that the two tokens we splice against still exist in its output).
//!
//! # Purity
//!
//! [`weave_region`] mirrors `pipeline.rs`: it is pure. It takes a [`RegionInput`]
//! (member URIs paired with their on-disk source paths, read by the caller),
//! returns the artifact as in-memory [`ArtifactFile`]s, and never touches the
//! filesystem itself. The CLI / MCP layer does all I/O — reads the sources into
//! `RegionInput`, writes the returned files, copies motif wasm. This keeps the
//! projection unit-testable and decoupled from the corpus's file layout.
```

```rust {#uses}
use crate::parser::parse_document;
use crate::weave::weave_html;
use anyhow::{Context, Result};
use std::collections::{BTreeSet, HashMap};
use std::path::PathBuf;
```

## The carried example

A region with two members: the entry `x0k:design/foo` and a sibling
`x0k:wiki/bar`. The entry's body says `see [[bar]]` and links to
`x0k:wiki/bar#section`. Weaving yields `index.html` and `bar.html`; the
wikilink becomes a markdown link to `x0k:wiki/bar` before the weave, and the
weave's `href="x0k:wiki/bar#section"` becomes `href="bar.html#section"`
afterwards. Both pages carry the same two-item nav, with the current page
marked. Every mechanism below is one step of that transformation.

## Input and output

A member is a URI, the path its content came from, and the content itself.
The path is carried so that links authored as relative `.md` paths can be
resolved too, but the weaver never opens it.

```rust {#region-member}
/// One member of a region as handed to [`weave_region`]: its entity URI and the
/// pre-read source markdown. The caller (CLI/MCP) reads the file; the projection
/// stays decoupled from path resolution and the corpus's file layout.
#[derive(Debug, Clone)]
pub struct RegionMember {
    /// The member's entity URI, exactly as authored (`x0k:design/foo`).
    pub uri: String,
    /// Full source markdown of the member's decision doc (frontmatter + body).
    pub content: String,
    /// The member's workspace-relative source path, e.g.
    /// `decisions/design/foo.md`. Used as a second link-match key so an
    /// author who linked a member by its relative `.md` path (rather than its
    /// URI) still resolves intra-region.
    pub source_path: PathBuf,
}

/// The input to [`weave_region`]: ordered members + which one is the entry.
///
/// Members are in authored order (the order the site nav lists them, and the
/// order the resolver preserved from `publishes:`).
#[derive(Debug, Clone)]
pub struct RegionInput {
    /// Region members, in authored order.
    pub members: Vec<RegionMember>,
    /// The `uri` of the member the reader lands on (mapped to `index.html`).
    pub entry_point_uri: String,
}
```

The output is in-memory files plus the diagnostic lists a caller reports on:
links that looked cross-document but matched no member, media refs that have
no live substrate in a static artifact, and transclusions that degraded to a
link.

```rust {#artifact-file}
/// One file in the projected artifact.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ArtifactFile {
    /// Path relative to the artifact root, e.g. `index.html` or
    /// `architecture-web-first.html`.
    pub rel_path: PathBuf,
    /// File bytes.
    pub bytes: Vec<u8>,
}

/// A cross-doc link the projection could NOT resolve to a region member —
/// recorded (not rewritten) so the operator sees what points outside the
/// region.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnresolvedLink {
    /// The member page the link was found on (its `rel_path`).
    pub on_page: PathBuf,
    /// The href as authored.
    pub href: String,
}
```

```rust {#region-weave-output}
/// The product of projecting a region.
#[derive(Debug, Clone)]
pub struct RegionWeaveOutput {
    /// Every file in the artifact (one `.html` per member; `wasm/` etc. added
    /// by the I/O layer).
    pub files: Vec<ArtifactFile>,
    /// The entry page's path within the artifact (`index.html`).
    pub entry_rel_path: PathBuf,
    /// Union of all `x0k:media` refs across members (deduped, sorted) — the
    /// motif harvest the I/O layer content-addresses and bundles.
    pub media_refs: Vec<String>,
    /// Links that pointed outside the region (kept verbatim in the HTML).
    pub unresolved_links: Vec<String>,
    /// `x0k:media` refs that have no live substrate in a static artifact and
    /// therefore show their static `.media-label` fallback. The weaver records
    /// the full media-ref set here (motif resolution happens in the I/O layer,
    /// which downgrades this to only the genuinely-dangling refs).
    pub degraded_embeds: Vec<String>,
    /// The time×thread atlas computed from the region (also emitted as the
    /// `atlas.json` `ArtifactFile`). render-vello consumes it via the
    /// explicit-position `set_graph` path. See [`crate::atlas`].
    pub atlas: crate::atlas::Atlas,
    /// Transclusion references that degraded to a link rather than inlining
    /// (cycle / unresolved / missing-section / depth). Each is
    /// `"<spine-uri>: <warning-debug>"`. Empty when every transclusion
    /// resolved cleanly.
    pub transclusion_warnings: Vec<String>,
}
```

## The weave

The pipeline builds the URI-to-path map first, so link rewriting on any page
can resolve every sibling, then weaves in two passes: the first collects
titles for the nav, the second rewrites links and injects the nav now that the
list is complete.

```rust {#weave-region}
/// Project an ordered region into a self-contained multi-page web artifact.
///
/// Pure: reads nothing, writes nothing. The URI→artifact-path map is built
/// FIRST (entry → `index.html`, every other member → a deduped slug + `.html`)
/// so per-page link rewriting can resolve sibling members. Each member is then
/// woven via [`weave_html`], its cross-doc links rewritten to artifact-relative
/// paths, and a site nav + scoped `<style>` spliced in.
pub fn weave_region(input: &RegionInput) -> Result<RegionWeaveOutput> {
    if input.members.is_empty() {
        anyhow::bail!("region has no members");
    }

    <<weave-path-maps>>

    <<weave-collections>>

    <<weave-first-pass>>

    <<weave-second-pass>>

    <<weave-atlas>>

    <<weave-output>>
}
```

```rust {#weave-path-maps}
// 1. URI → artifact path map. Entry first so it claims `index.html`;
//    `slug` is deduped against already-claimed paths so two members whose
//    titles/identifiers slug-collide still get distinct files.
let uri_to_path = build_uri_to_path(input);

// A secondary index: the member's source `.md` path string → the same
// artifact path, so links authored as relative `.md` paths resolve too.
let mut path_to_artifact: HashMap<String, PathBuf> = HashMap::new();
for m in &input.members {
    if let Some(art) = uri_to_path.get(&m.uri) {
        path_to_artifact.insert(normalize_md_path(&m.source_path), art.clone());
    }
}

// Titles, in member order, for the nav (woven below; default to the URI).
let mut nav_entries: Vec<NavEntry> = Vec::new();

// Wikilink resolution table: for every in-region member whose URI is a
// `x0k:wiki/<slug>`, map `slug -> display title`. Built FIRST (before any
// weave) so a `[[slug]]` in one member's body can resolve to a sibling.
// Display title = the member's first H1 (cheap scan), else the slug. An
// out-of-region `[[slug]]` (not in this map) is stripped to plain text.
let wiki_titles = build_wiki_title_map(input);
```

```rust {#weave-collections}
// 2. Weave each member, rewrite its links, inject nav + style.
let mut files: Vec<ArtifactFile> = Vec::new();
let mut unresolved_links: Vec<UnresolvedLink> = Vec::new();
let mut media_refs: BTreeSet<String> = BTreeSet::new();
let mut transclusion_warnings: Vec<String> = Vec::new();

// Transclusion source: every member, keyed by URI. A spine member's
// `transcludes:` / inline `x0k:transclude` references resolve against
// the region's own membership (intra-region targets only).
let transclude_source = RegionDocSource::new(input);
```

Each member is transcluded, wikilink-rewritten, parsed and woven in that
order, because transclusion arrives as markdown and the weaver must see a
complete folio/v1 file. Media refs are harvested from the source rather than
the woven HTML, so the harvest runs even when the `motifs` feature is off.

```rust {#weave-first-pass}
// First pass: weave + collect titles so the nav (built per-page) is complete.
struct Woven {
    rel_path: PathBuf,
    html: String,
}
let mut woven: Vec<Woven> = Vec::new();
for m in &input.members {
    let rel_path = uri_to_path
        .get(&m.uri)
        .cloned()
        .expect("every member is in uri_to_path");
    // Resolve transclusions FIRST (recursively, cycle-checked): inline
    // the addressed sections of referenced members so the woven page is
    // one composed read. Seeds the cycle-guard with this member's own
    // URI so a self-transclusion degrades to a link. Feeds the woven
    // HTML automatically — transclusion arrives pre-resolved as markdown.
    let resolved = x0k_folio::transclusion::resolve_with_root(
        &m.content,
        &m.uri,
        None,
        &transclude_source,
    );
    for w in &resolved.warnings {
        transclusion_warnings.push(format!("{}: {:?}", m.uri, w));
    }
    // Splice the resolved body back behind the member's frontmatter so
    // the weaver still sees a complete folio/v1 file.
    let transcluded_content = reassemble_with_body(&m.content, &resolved.body);
    // Pre-process `[[slug]]` wikilinks BEFORE weaving: in-region → a
    // markdown link `[title](x0k:wiki/slug)` (later rewritten to the local
    // `.html` by `rewrite_cross_doc_links`); out-of-region → plain text.
    let content = rewrite_wikilinks(&transcluded_content, &wiki_titles);
    let parsed =
        parse_document(&content).with_context(|| format!("parsing member `{}`", m.uri))?;
    let out =
        weave_html(&content, &parsed).with_context(|| format!("weaving member `{}`", m.uri))?;
    // Harvest media refs from the source (the weaver consumes them into
    // mount points; the source still carries the `ref=`). The scan is
    // pure and runs in every build — with `motifs` off, the refs still
    // populate `degraded_embeds` so the standalone build reports what it
    // cannot bundle instead of silently dropping the fences. Only the
    // wasm bundling itself (region_project) is behind `motifs`.
    for r in scan_media_refs(&m.content) {
        media_refs.insert(r);
    }
    let title = out.title.unwrap_or_else(|| m.uri.clone());
    nav_entries.push(NavEntry {
        href: rel_path.clone(),
        title,
    });
    woven.push(Woven {
        rel_path,
        html: out.html,
    });
}
```

```rust {#weave-second-pass}
// Second pass: rewrite links + inject nav now that nav_entries is complete.
for w in &woven {
    let (rewritten, mut unresolved) =
        rewrite_cross_doc_links(&w.html, &uri_to_path, &path_to_artifact, &w.rel_path);
    unresolved_links.append(&mut unresolved);

    let with_nav = inject_site_nav(&rewritten, &nav_entries, &w.rel_path);
    files.push(ArtifactFile {
        rel_path: w.rel_path.clone(),
        bytes: with_nav.into_bytes(),
    });
}
```

The atlas is computed from the same input and emitted as one more file.

```rust {#weave-atlas}
// 3. Compute the time×thread atlas and emit it as `atlas.json`. render-vello
//    consumes the explicit node positions via the `set_graph` path, so the
//    layout is finalized here (no render-vello layout pass needed).
let atlas = crate::atlas::build_atlas(input);
files.push(ArtifactFile {
    rel_path: PathBuf::from(crate::atlas::ATLAS_FILE),
    bytes: crate::atlas::atlas_json(&atlas),
});
```

```rust {#weave-output}
let media_refs: Vec<String> = media_refs.into_iter().collect();
Ok(RegionWeaveOutput {
    files,
    entry_rel_path: PathBuf::from(ENTRY_FILE),
    // Every media ref is "degraded" until the I/O layer resolves which
    // ones bundle as canvas wasm vs stay static-label.
    degraded_embeds: media_refs.clone(),
    media_refs,
    unresolved_links: unresolved_links.into_iter().map(|u| u.href).collect(),
    atlas,
    transclusion_warnings,
})
```

## Transclusion within the region

A member's `transcludes:` references resolve against the region's own
membership: the region is the document source. Out-of-region targets return
`None` and degrade to a link with a warning.

```rust {#region-doc-source}
/// A [`x0k_folio::transclusion::DocSource`] backed by a region's
/// members. Maps a folio URI to that member's **body markdown**
/// (frontmatter stripped), so transclusion references resolve against the
/// region's own membership. Only intra-region targets
/// resolve; an out-of-region reference returns `None` and degrades to a
/// link (recorded as a warning).
struct RegionDocSource {
    by_uri: HashMap<String, String>,
}

impl RegionDocSource {
    fn new(input: &RegionInput) -> Self {
        let mut by_uri = HashMap::new();
        for m in &input.members {
            // Store the FULL file content; `DocSource::body` strips the
            // frontmatter via the shared `split_body` so the inlined
            // markdown matches what the renderer sees.
            by_uri.insert(m.uri.clone(), m.content.clone());
        }
        Self { by_uri }
    }
}

impl x0k_folio::transclusion::DocSource for RegionDocSource {
    fn body(&self, uri: &str) -> Option<String> {
        self.by_uri
            .get(uri)
            .map(|c| x0k_folio::transclusion::split_body(c).1.to_string())
    }
}
```

Media refs are scanned with a pure line scan that mirrors the surface-build
scanner, kept local so the harvest has no feature dependency.

```rust {#scan-media-refs}
/// Scan a document for `x0k:media {ref="…"}` fence refs. Pure line scan,
/// mirroring `x0k_surface_build::scan_media_refs` so the harvest matches
/// what the motif publish step resolves — inlined here so the harvest runs
/// featureless (the `motifs`-severed standalone build still reports its
/// degraded embeds).
fn scan_media_refs(text: &str) -> Vec<String> {
    let mut refs = Vec::new();
    for line in text.lines() {
        let l = line.trim_start();
        if !l.contains("x0k:media") {
            continue;
        }
        // Find ref="..." on the same line.
        if let Some(start) = l.find("ref=") {
            let after = &l[start + 4..];
            let after = after.trim_start_matches(['"', '\'']);
            let end = after.find(['"', '\'', '}', ' ']).unwrap_or(after.len());
            let uri = after[..end].trim().to_string();
            if uri.starts_with("x0k:") {
                refs.push(uri);
            }
        }
    }
    refs
}
```

After transclusion, the resolved body is spliced back behind the member's
own frontmatter.

```rust {#reassemble}
/// Reassemble a folio/v1 file from its original frontmatter and a
/// (transclusion-resolved) body. Preserves the `---`-delimited envelope
/// byte-for-byte; only the body region is replaced. If the input has no
/// frontmatter, returns the resolved body alone.
fn reassemble_with_body(original: &str, new_body: &str) -> String {
    match x0k_folio::transclusion::split_body(original) {
        (Some(yaml), _) => {
            let mut out = String::with_capacity(yaml.len() + new_body.len() + 16);
            out.push_str("---");
            out.push_str(yaml);
            out.push_str("\n---\n");
            out.push_str(new_body);
            if !new_body.ends_with('\n') {
                out.push('\n');
            }
            out
        }
        (None, _) => new_body.to_string(),
    }
}
```

## Wikilinks

`[[slug]]` resolves to whichever in-region member carries that identifier,
whatever its URI class. The title map is built once from every member's first
H1; a slug absent from the map is out-of-region and is stripped to plain text
so no dangling href ships.

```rust {#entry-file}
/// The entry member's artifact filename.
pub const ENTRY_FILE: &str = "index.html";
```

```rust {#wiki-title-map}
/// Build `slug -> (display-title, member-URI)` for every in-region member,
/// keyed by the member's URI identifier (the part after the last `/`). The
/// display title is the member's first markdown H1 (cheap scan of its source),
/// falling back to the slug. This map is the in-region wikilink set: a
/// `[[slug]]` whose `slug` is a key resolves to a markdown link to that
/// member's actual URI; one that is absent is out-of-region.
///
/// Class-agnostic by design: a region's members may span URI classes (e.g.
/// `x0k:genealogy/<slug>` authored chapters alongside `x0k:wiki/<slug>`
/// reference pages), so a `[[slug]]` resolves to whichever member carries that
/// identifier — the emitted href is the member's full URI, which
/// [`rewrite_cross_doc_links`] then maps to its local `.html`. On the rare
/// identifier collision across classes, the first member in authored order wins
/// (the entry point and authored chapters are listed before reference pages).
pub fn build_wiki_title_map(input: &RegionInput) -> HashMap<String, (String, String)> {
    let mut map: HashMap<String, (String, String)> = HashMap::new();
    for m in &input.members {
        // Slug = the URI identifier (after the last `/`), dropping any `@locator`.
        let ident = m.uri.rsplit('/').next().unwrap_or(m.uri.as_str());
        let slug = ident.split('@').next().unwrap_or(ident);
        if slug.is_empty() {
            continue;
        }
        let title = first_h1(&m.content).unwrap_or_else(|| slug.to_string());
        map.entry(slug.to_string())
            .or_insert_with(|| (title, m.uri.clone()));
    }
    map
}
```

`first_h1` and `parse_wikilink_slug` here are the same scans the atlas
carries in its own copies; each module keeps its own rather than sharing,
and this chapter names that rather than hiding it.

```rust {#first-h1}
/// Extract the first markdown `# ` heading from a doc's body (cheap line scan).
/// Skips the frontmatter envelope. Returns `None` if no H1 is present.
fn first_h1(content: &str) -> Option<String> {
    for line in content.lines() {
        let t = line.trim_start();
        if let Some(rest) = t.strip_prefix("# ") {
            return Some(rest.trim().to_string());
        }
    }
    None
}
```

```rust {#rewrite-wikilinks}
/// Replace `[[slug]]` wikilinks in a member's markdown source.
///
/// - In-region (`slug` is a key in `wiki_titles`): becomes a markdown link
///   `[<title>](<member-uri>)` where `<member-uri>` is the resolved member's
///   actual URI (e.g. `x0k:wiki/<slug>` or `x0k:genealogy/<slug>`). The later
///   `rewrite_cross_doc_links` pass maps that href to the member's local
///   `<class>-<id>.html` file.
/// - Out-of-region: becomes plain text `<slug>` (the brackets are stripped) so
///   no dangling href ships.
///
/// The pattern is tight: `[[` + one-or-more `[a-z0-9-]` + `]]`. Anything that
/// doesn't match (e.g. `[[Foo Bar]]`, `[[ ]]`, code-fence `[[x]]`) is left
/// verbatim — we don't attempt to parse code fences here because the wiki slug
/// charset never collides with prose.
pub fn rewrite_wikilinks(content: &str, wiki_titles: &HashMap<String, (String, String)>) -> String {
    let bytes = content.as_bytes();
    let mut out = String::with_capacity(content.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'[' && i + 1 < bytes.len() && bytes[i + 1] == b'[' {
            // Find the closing `]]`.
            if let Some((slug, end)) = parse_wikilink_slug(bytes, i + 2) {
                if let Some((title, uri)) = wiki_titles.get(&slug) {
                    out.push_str(&format!("[{title}]({uri})"));
                } else {
                    // Out-of-region: strip to plain text.
                    out.push_str(&slug);
                }
                i = end;
                continue;
            }
        }
        // Not a wikilink start; copy the byte (UTF-8 safe: only ASCII `[` is
        // special, multi-byte chars are copied verbatim via the char boundary).
        let ch_len = utf8_len(bytes[i]);
        out.push_str(&content[i..i + ch_len]);
        i += ch_len;
    }
    out
}

/// Parse a tight wikilink slug `[a-z0-9-]+` starting at `start`, requiring a
/// closing `]]`. Returns `(slug, index-just-past-the-closing-]])` on success.
fn parse_wikilink_slug(bytes: &[u8], start: usize) -> Option<(String, usize)> {
    let mut j = start;
    while j < bytes.len() {
        let b = bytes[j];
        if b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-' {
            j += 1;
        } else {
            break;
        }
    }
    // Need at least one slug char and a closing `]]`.
    if j == start || j + 1 >= bytes.len() || bytes[j] != b']' || bytes[j + 1] != b']' {
        return None;
    }
    let slug = std::str::from_utf8(&bytes[start..j]).ok()?.to_string();
    Some((slug, j + 2))
}

/// UTF-8 byte length from a leading byte.
fn utf8_len(b: u8) -> usize {
    if b < 0x80 {
        1
    } else if b >> 5 == 0b110 {
        2
    } else if b >> 4 == 0b1110 {
        3
    } else {
        4
    }
}
```

## Paths

The entry member claims `index.html`; every other member gets a deduped slug
from its URI (class and identifier both, so `x0k:design/foo` and
`x0k:wiki/foo` do not collide).

```rust {#uri-to-path}
/// Build the URI → artifact-path map. The entry member claims `index.html`;
/// every other member gets a deduped slug derived from its URI identifier (the
/// part after the last `/`), suffixed `.html`. Deterministic and dedup-safe:
/// members in authored order, the entry first.
pub fn build_uri_to_path(input: &RegionInput) -> HashMap<String, PathBuf> {
    let mut map: HashMap<String, PathBuf> = HashMap::new();
    let mut used: BTreeSet<String> = BTreeSet::new();

    // Entry first → index.html.
    map.insert(input.entry_point_uri.clone(), PathBuf::from(ENTRY_FILE));
    used.insert(ENTRY_FILE.to_string());

    for m in &input.members {
        if m.uri == input.entry_point_uri {
            continue;
        }
        if map.contains_key(&m.uri) {
            continue; // duplicate member URI — keep first.
        }
        let base = slug_for_uri(&m.uri);
        let file = dedup_filename(&base, &mut used);
        map.insert(m.uri.clone(), PathBuf::from(file));
    }
    map
}

/// Slugify a member URI into a filename stem. Uses the class + identifier so
/// two members with the same identifier under different classes don't collide
/// (`x0k:design/foo` → `design-foo`). Falls back to slugifying the whole URI.
fn slug_for_uri(uri: &str) -> String {
    // Strip a leading `x0k:` scheme if present.
    let rest = uri.strip_prefix("x0k:").unwrap_or(uri);
    slugify(rest)
}

/// Lowercase, collapse non-alphanumeric runs to a single `-`, trim. Mirrors
/// `weave::slugify`'s discipline (kept local so the pure module has no private
/// dependency on weave internals). Empty → `page`.
fn slugify(text: &str) -> String {
    let mut slug = String::new();
    let mut prev_dash = false;
    for ch in text.chars().flat_map(|c| c.to_lowercase()) {
        if ch.is_ascii_alphanumeric() {
            slug.push(ch);
            prev_dash = false;
        } else if !prev_dash {
            slug.push('-');
            prev_dash = true;
        }
    }
    let trimmed = slug.trim_matches('-');
    if trimmed.is_empty() {
        "page".to_string()
    } else {
        trimmed.to_string()
    }
}

/// Pick a `<base>.html` filename, deduping against `used` (`foo.html`,
/// `foo-2.html`, …). Records the chosen name in `used`.
fn dedup_filename(base: &str, used: &mut BTreeSet<String>) -> String {
    let first = format!("{base}.html");
    if used.insert(first.clone()) {
        return first;
    }
    let mut n = 2;
    loop {
        let candidate = format!("{base}-{n}.html");
        if used.insert(candidate.clone()) {
            return candidate;
        }
        n += 1;
    }
}

/// Normalize a member's source `.md` path to a forward-slash string for
/// link-key matching (so a Windows-authored path and a posix link compare).
fn normalize_md_path(p: &std::path::Path) -> String {
    p.components()
        .map(|c| c.as_os_str().to_string_lossy())
        .collect::<Vec<_>>()
        .join("/")
}
```

## Cross-document links and the nav

Link rewriting matches exactly: an href is either a member URI (with an
optional fragment), a member's source path, or left alone. Anything that
looks cross-document but matches nothing is kept verbatim and recorded.

```rust {#nav-entry}
/// One site-nav item.
#[derive(Debug, Clone, PartialEq, Eq)]
struct NavEntry {
    /// The member page's artifact path.
    href: PathBuf,
    /// Display title (first H1, or the URI).
    title: String,
}
```

```rust {#rewrite-cross-doc-links}
/// Rewrite cross-doc links in one woven page.
///
/// For every `<a href="...">`, if the href EXACT-matches a member URI (or that
/// member's known relative `.md` path), rewrite it to the artifact-relative
/// path of that member's page, preserving any `#fragment`. Intra-doc chunk
/// anchors (`#chunk-...` and any bare `#...`) are left untouched. Any link that
/// looks like a cross-doc reference (a `x0k:` URI or a `.md` path) but matches
/// no member is collected as [`UnresolvedLink`] and left verbatim. Ordinary
/// external links (http(s), mailto, …) pass through silently.
///
/// Matching is EXACT (after splitting off the fragment) — no substring or fuzzy
/// matching, so an unrelated href that merely contains a member URI is never
/// rewritten.
pub fn rewrite_cross_doc_links(
    html: &str,
    uri_to_path: &HashMap<String, PathBuf>,
    path_to_artifact: &HashMap<String, PathBuf>,
    current_page: &std::path::Path,
) -> (String, Vec<UnresolvedLink>) {
    let mut out = String::with_capacity(html.len());
    let mut unresolved = Vec::new();
    let needle = "href=\"";
    let mut rest = html;
    loop {
        let Some(pos) = rest.find(needle) else {
            out.push_str(rest);
            break;
        };
        // Emit everything up to and including `href="`.
        out.push_str(&rest[..pos + needle.len()]);
        let after = &rest[pos + needle.len()..];
        let Some(endq) = after.find('"') else {
            // Malformed; emit the remainder untouched.
            out.push_str(after);
            break;
        };
        let href = &after[..endq];
        let replacement = rewrite_one_href(href, uri_to_path, path_to_artifact, current_page);
        match replacement {
            HrefAction::Keep => out.push_str(href),
            HrefAction::Rewrite(new) => out.push_str(&new),
            HrefAction::Unresolved => {
                out.push_str(href);
                unresolved.push(UnresolvedLink {
                    on_page: current_page.to_path_buf(),
                    href: href.to_string(),
                });
            }
        }
        out.push('"');
        rest = &after[endq + 1..];
    }
    (out, unresolved)
}

enum HrefAction {
    /// Leave the href as-is.
    Keep,
    /// Replace with this artifact-relative href.
    Rewrite(String),
    /// Looks cross-doc but matched no member — keep verbatim, record it.
    Unresolved,
}

fn rewrite_one_href(
    href: &str,
    uri_to_path: &HashMap<String, PathBuf>,
    path_to_artifact: &HashMap<String, PathBuf>,
    current_page: &std::path::Path,
) -> HrefAction {
    // Pure-fragment links (`#chunk-foo`, `#section`) are intra-page — leave them.
    if href.starts_with('#') {
        return HrefAction::Keep;
    }
    // Split off a fragment to preserve it across the rewrite.
    let (target, fragment) = match href.split_once('#') {
        Some((t, f)) => (t, Some(f)),
        None => (href, None),
    };
    if target.is_empty() {
        return HrefAction::Keep;
    }

    // Exact member-URI match.
    if let Some(art) = uri_to_path.get(target) {
        return HrefAction::Rewrite(with_fragment(&relativize(art, current_page), fragment));
    }
    // Exact relative-`.md`-path match.
    let norm = normalize_link_path(target);
    if let Some(art) = path_to_artifact.get(&norm) {
        return HrefAction::Rewrite(with_fragment(&relativize(art, current_page), fragment));
    }

    // Looks cross-doc but unmatched → record. A `x0k:` URI or a `.md` path that
    // isn't a member points outside the region.
    if target.starts_with("x0k:") || target.ends_with(".md") {
        return HrefAction::Unresolved;
    }

    HrefAction::Keep
}

/// Normalize a link target path for matching against member source paths: drop
/// a leading `./`, collapse to forward slashes. (We do NOT resolve `..` against
/// the current page — member paths are workspace-relative and authors reference
/// them as such; there is no relative-resolution pass.)
fn normalize_link_path(target: &str) -> String {
    let t = target.trim_start_matches("./");
    t.replace('\\', "/")
}

/// Reattach a fragment to a rewritten href.
fn with_fragment(base: &str, fragment: Option<&str>) -> String {
    match fragment {
        Some(f) => format!("{base}#{f}"),
        None => base.to_string(),
    }
}

/// Artifact-relative link from `current_page` to `target`. The artifact is flat
/// (every page at the root), so this is just the target's file name.
fn relativize(target: &std::path::Path, _current_page: &std::path::Path) -> String {
    target
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_else(|| target.to_string_lossy().to_string())
}
```

The nav is spliced after the first `<body>` and its scoped style after the
first `</style>`; both tokens are guarded by a test against the single-doc
weaver's output, so a change there fails here rather than silently producing
nav-less pages.

```rust {#site-nav}
/// Inject the site nav after the first `<body>\n` token, and a small scoped
/// `<style>` after the first `</style>` (so the nav is themed but the woven
/// document's own STYLESHEET is untouched). Returns the page unchanged if the
/// expected tokens are absent (defensive — the anchor-token test guards them).
fn inject_site_nav(html: &str, nav_entries: &[NavEntry], current_page: &std::path::Path) -> String {
    let nav_html = render_site_nav(nav_entries, current_page);

    // Splice the scoped style after the FIRST `</style>`.
    let with_style = match html.find("</style>\n") {
        Some(idx) => {
            let cut = idx + "</style>\n".len();
            let mut s = String::with_capacity(html.len() + REGION_NAV_STYLE.len() + nav_html.len());
            s.push_str(&html[..cut]);
            s.push_str("<style>\n");
            s.push_str(REGION_NAV_STYLE);
            s.push_str("</style>\n");
            s.push_str(&html[cut..]);
            s
        }
        None => html.to_string(),
    };

    // Splice the nav after the FIRST `<body>\n`.
    match with_style.find("<body>\n") {
        Some(idx) => {
            let cut = idx + "<body>\n".len();
            let mut s = String::with_capacity(with_style.len() + nav_html.len());
            s.push_str(&with_style[..cut]);
            s.push_str(&nav_html);
            s.push_str(&with_style[cut..]);
            s
        }
        None => with_style,
    }
}

/// Render the static site-nav markup. Members in order; the current page is
/// marked `aria-current="page"` and given the `current` class.
fn render_site_nav(nav_entries: &[NavEntry], current_page: &std::path::Path) -> String {
    let mut s = String::new();
    s.push_str("<nav class=\"region-nav\" aria-label=\"Publication\">\n<ul>\n");
    for e in nav_entries {
        let href = e
            .href
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_else(|| e.href.to_string_lossy().to_string());
        let is_current = e.href == current_page;
        let (cls, aria) = if is_current {
            (" class=\"current\"", " aria-current=\"page\"")
        } else {
            ("", "")
        };
        s.push_str(&format!(
            "<li{cls}><a href=\"{}\"{aria}>{}</a></li>\n",
            escape_html(&href),
            escape_html(&e.title),
        ));
    }
    s.push_str("</ul>\n</nav>\n");
    s
}

/// Minimal local HTML escape (the pure module avoids reaching into weave's
/// private `escape_html`).
fn escape_html(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

/// Scoped styling for the injected nav. Kept small and self-contained; it does
/// not redefine any of weave's STYLESHEET selectors.
const REGION_NAV_STYLE: &str = r#"
.region-nav {
  position: sticky;
  top: 0;
  z-index: 10;
  padding: 0.5rem 1rem;
  border-bottom: 1px solid rgba(255,255,255,0.08);
  background: rgba(15,15,18,0.92);
  backdrop-filter: blur(6px);
}
.region-nav ul {
  list-style: none;
  margin: 0;
  padding: 0;
  display: flex;
  flex-wrap: wrap;
  gap: 0.25rem 1rem;
}
.region-nav li { margin: 0; }
.region-nav a {
  text-decoration: none;
  opacity: 0.72;
  font-size: 0.85rem;
}
.region-nav a:hover { opacity: 1; }
.region-nav li.current a {
  opacity: 1;
  font-weight: 600;
  border-bottom: 2px solid currentColor;
}
"#;
```

## Validation

`validate_artifact` states the artifact's invariants as a check a consumer
can run: the entry exists, every page is a complete standalone document with
no external stylesheet, and every intra-region href resolves to a file in the
set.

```rust {#validate}
/// Assert a [`RegionWeaveOutput`] is a well-formed self-contained artifact.
///
/// Validates the invariants the integration test and any consumer relies on:
/// the entry page exists; every page is a complete standalone HTML document
/// (starts with `<!DOCTYPE html>`, has an inlined `<style>`, and pulls in NO
/// external stylesheet `<link>`); and every intra-region href resolves to a
/// file present in `files`. Returns `Err` with the first violation.
pub fn validate_artifact(out: &RegionWeaveOutput) -> Result<()> {
    let present: BTreeSet<String> = out
        .files
        .iter()
        .map(|f| f.rel_path.to_string_lossy().to_string())
        .collect();

    // Entry must exist.
    let entry = out.entry_rel_path.to_string_lossy().to_string();
    if !present.contains(&entry) {
        anyhow::bail!("entry page `{entry}` is not in the artifact files");
    }

    for f in &out.files {
        let name = f.rel_path.to_string_lossy().to_string();
        // Only validate HTML pages here (wasm/host.js added by the I/O layer).
        if !name.ends_with(".html") {
            continue;
        }
        let html = std::str::from_utf8(&f.bytes)
            .with_context(|| format!("page `{name}` is not valid UTF-8"))?;
        if !html.starts_with("<!DOCTYPE html>") {
            anyhow::bail!("page `{name}` does not start with <!DOCTYPE html>");
        }
        if !html.contains("<style>") {
            anyhow::bail!("page `{name}` has no inlined <style> block");
        }
        if html.contains("<link") && html.contains("stylesheet") {
            anyhow::bail!("page `{name}` references an external stylesheet (not self-contained)");
        }
        // Every intra-region href (a bare `.html` file, no scheme, no `#`-only)
        // must resolve to a file in the artifact. Links are page-relative, so
        // resolve them against the page's own directory (the woven fallback
        // pages live under `pages/` and link each other by bare filename; the
        // root shell links into `pages/`).
        for href in extract_hrefs(html) {
            if href.starts_with('#')
                || href.contains("://")
                || href.starts_with("mailto:")
                || href.starts_with("x0k:")
            {
                continue;
            }
            let file = href.split('#').next().unwrap_or(&href);
            if file.is_empty() {
                continue;
            }
            // Only validate links that target an artifact page (`.html`).
            if file.ends_with(".html") {
                let resolved = resolve_relative(&f.rel_path, file);
                if !present.contains(&resolved) {
                    anyhow::bail!(
                        "page `{name}` links to `{file}` (→ `{resolved}`) which is not in the artifact"
                    );
                }
            }
        }
    }
    Ok(())
}

/// Resolve a page-relative href against the linking page's path, returning the
/// artifact-root-relative path string used as the `present`-set key. Handles a
/// leading `./` and `../` segments. E.g. a page `pages/a.html` linking to
/// `b.html` → `pages/b.html`; the root shell linking to `pages/index.html` →
/// `pages/index.html`.
fn resolve_relative(page: &std::path::Path, href: &str) -> String {
    // The page's directory components (everything but the filename).
    let mut stack: Vec<String> = page
        .parent()
        .map(|p| {
            p.components()
                .map(|c| c.as_os_str().to_string_lossy().to_string())
                .collect()
        })
        .unwrap_or_default();
    for seg in href.replace('\\', "/").split('/') {
        match seg {
            "" | "." => {}
            ".." => {
                stack.pop();
            }
            other => stack.push(other.to_string()),
        }
    }
    stack.join("/")
}

/// Pull every `href="..."` value out of an HTML string (cheap scan, validation
/// only).
fn extract_hrefs(html: &str) -> Vec<String> {
    let mut out = Vec::new();
    let needle = "href=\"";
    let mut rest = html;
    while let Some(pos) = rest.find(needle) {
        let after = &rest[pos + needle.len()..];
        if let Some(endq) = after.find('"') {
            out.push(after[..endq].to_string());
            rest = &after[endq + 1..];
        } else {
            break;
        }
    }
    out
}
```

## Tests

The rules above are each one string-to-string rewrite, so the unit tests are
too: a fixture region built in memory, one rewrite, one assertion.

```rust {#tests}
#[cfg(test)]
mod tests {
    use super::*;

    fn member(uri: &str, src: &str, content: &str) -> RegionMember {
        RegionMember {
            uri: uri.to_string(),
            content: content.to_string(),
            source_path: PathBuf::from(src),
        }
    }

    /// A minimal folio/v1 member body. The weaver only needs the markdown
    /// body; frontmatter is split off by `weave_html`'s `split_body`.
    fn doc(title: &str, body: &str) -> String {
        format!(
            "---\nx0k:\n  format: folio/v1\n  type: design\n  id: x0k:design/x\n---\n\n# {title}\n\n{body}\n"
        )
    }

    #[test]
    fn uri_to_path_entry_is_index_deterministic_and_dedup() {
        let input = RegionInput {
            entry_point_uri: "x0k:design/a".to_string(),
            members: vec![
                member("x0k:design/a", "decisions/design/a.md", &doc("A", "a")),
                member("x0k:design/b", "decisions/design/b.md", &doc("B", "b")),
                // identifier-collision across classes must NOT collide files.
                member(
                    "x0k:architecture/b",
                    "decisions/architecture/b.md",
                    &doc("Arch B", "c"),
                ),
            ],
        };
        let map = build_uri_to_path(&input);
        assert_eq!(
            map.get("x0k:design/a").unwrap(),
            &PathBuf::from("index.html")
        );
        assert_eq!(
            map.get("x0k:design/b").unwrap(),
            &PathBuf::from("design-b.html")
        );
        assert_eq!(
            map.get("x0k:architecture/b").unwrap(),
            &PathBuf::from("architecture-b.html")
        );
        // Deterministic: re-running yields identical mapping.
        let map2 = build_uri_to_path(&input);
        assert_eq!(map.len(), map2.len());
        for (k, v) in &map {
            assert_eq!(map2.get(k), Some(v));
        }
    }

    #[test]
    fn dedup_filename_suffixes_collisions() {
        // Two members that slug to the same base get distinct files.
        let input = RegionInput {
            entry_point_uri: "x0k:design/entry".to_string(),
            members: vec![
                member(
                    "x0k:design/entry",
                    "decisions/design/entry.md",
                    &doc("E", "e"),
                ),
                // both slug to `design-dup` — second must become design-dup-2.
                member("x0k:design/dup", "decisions/design/dup.md", &doc("D1", "d")),
            ],
        };
        let mut input2 = input.clone();
        // Force a real collision: a member whose URI slugs identically.
        input2.members.push(member(
            "x0k:design/dup",
            "decisions/design/dup.md",
            &doc("D2", "d"),
        ));
        let map = build_uri_to_path(&input2);
        // The duplicate URI keeps the first mapping (dedup by URI).
        assert_eq!(
            map.get("x0k:design/dup").unwrap(),
            &PathBuf::from("design-dup.html")
        );
    }

    #[test]
    fn rewrite_member_uri_to_relative_preserves_fragment() {
        let mut uri_to_path = HashMap::new();
        uri_to_path.insert("x0k:design/a".to_string(), PathBuf::from("index.html"));
        uri_to_path.insert("x0k:design/b".to_string(), PathBuf::from("design-b.html"));
        let path_to_artifact = HashMap::new();
        let html = r#"<a href="x0k:design/b#open-questions">B</a>"#;
        let (out, unresolved) = rewrite_cross_doc_links(
            html,
            &uri_to_path,
            &path_to_artifact,
            std::path::Path::new("index.html"),
        );
        assert_eq!(out, r#"<a href="design-b.html#open-questions">B</a>"#);
        assert!(unresolved.is_empty());
    }

    #[test]
    fn rewrite_relative_md_path_to_member() {
        let mut uri_to_path = HashMap::new();
        uri_to_path.insert("x0k:design/b".to_string(), PathBuf::from("design-b.html"));
        let mut path_to_artifact = HashMap::new();
        path_to_artifact.insert(
            "decisions/design/b.md".to_string(),
            PathBuf::from("design-b.html"),
        );
        let html = r#"<a href="decisions/design/b.md">B</a> and <a href="./decisions/design/b.md#x">B2</a>"#;
        let (out, unresolved) = rewrite_cross_doc_links(
            html,
            &uri_to_path,
            &path_to_artifact,
            std::path::Path::new("index.html"),
        );
        assert!(
            out.contains(r#"<a href="design-b.html">B</a>"#),
            "got: {out}"
        );
        assert!(
            out.contains(r#"<a href="design-b.html#x">B2</a>"#),
            "got: {out}"
        );
        assert!(unresolved.is_empty());
    }

    #[test]
    fn rewrite_leaves_chunk_and_section_anchors_untouched() {
        let uri_to_path = HashMap::new();
        let path_to_artifact = HashMap::new();
        let html = r##"<a class="chunk-ref" href="#chunk-foo">x</a><a href="#section-2">y</a>"##;
        let (out, unresolved) = rewrite_cross_doc_links(
            html,
            &uri_to_path,
            &path_to_artifact,
            std::path::Path::new("index.html"),
        );
        assert_eq!(out, html);
        assert!(unresolved.is_empty());
    }

    #[test]
    fn rewrite_collects_out_of_region_links() {
        let uri_to_path = HashMap::new();
        let path_to_artifact = HashMap::new();
        let html = r#"<a href="x0k:design/not-a-member">x</a><a href="other/doc.md">y</a><a href="https://ex.com">z</a>"#;
        let (out, unresolved) = rewrite_cross_doc_links(
            html,
            &uri_to_path,
            &path_to_artifact,
            std::path::Path::new("index.html"),
        );
        // Unmatched cross-doc links are kept verbatim but recorded.
        assert_eq!(out, html);
        let hrefs: Vec<&str> = unresolved.iter().map(|u| u.href.as_str()).collect();
        assert!(hrefs.contains(&"x0k:design/not-a-member"), "got {hrefs:?}");
        assert!(hrefs.contains(&"other/doc.md"), "got {hrefs:?}");
        // External http link is NOT flagged.
        assert!(!hrefs.contains(&"https://ex.com"));
    }

    #[test]
    fn nav_injected_after_body_current_marked_all_listed() {
        let entries = vec![
            NavEntry {
                href: PathBuf::from("index.html"),
                title: "Entry".to_string(),
            },
            NavEntry {
                href: PathBuf::from("design-b.html"),
                title: "Bee".to_string(),
            },
        ];
        let page = "<!DOCTYPE html>\n<html>\n<head>\n<style>\nx</style>\n</head>\n<body>\n<article>hi</article>\n</body>\n</html>\n";
        let out = inject_site_nav(page, &entries, std::path::Path::new("index.html"));
        // The nav element is after <body>, before the article. (Note the
        // scoped `.region-nav` *style* lands in <head> before <body>; we anchor
        // on the `<nav` element, which only appears in the body.)
        let body_idx = out.find("<body>\n").unwrap();
        let nav_idx = out.find("<nav class=\"region-nav\"").unwrap();
        let article_idx = out.find("<article>").unwrap();
        assert!(body_idx < nav_idx && nav_idx < article_idx);
        // All members listed.
        assert!(out.contains(">Entry</a>"));
        assert!(out.contains(">Bee</a>"));
        // Current page marked.
        assert!(out
            .contains(r#"<li class="current"><a href="index.html" aria-current="page">Entry</a>"#));
        // Scoped style injected after </style>.
        assert!(out.contains(".region-nav"));
    }

    /// Anchor-token guard: a `weave.rs` change that drops `<body>\n` or
    /// `</style>\n` from the output trips THIS test, so the nav-injection seam
    /// is protected.
    #[test]
    fn weave_html_output_contains_splice_anchor_tokens() {
        let content = doc("Title", "Some body text.");
        let parsed = parse_document(&content).unwrap();
        let out = weave_html(&content, &parsed).unwrap();
        assert!(
            out.html.contains("<body>\n"),
            "weave_html no longer emits `<body>\\n` — region nav splice broken"
        );
        assert!(
            out.html.contains("</style>\n"),
            "weave_html no longer emits `</style>\\n` — region style splice broken"
        );
    }

    #[test]
    fn wikilink_in_region_becomes_local_link_out_of_region_becomes_text() {
        let mut titles: HashMap<String, (String, String)> = HashMap::new();
        titles.insert(
            "hypercard".to_string(),
            ("HyperCard".to_string(), "x0k:wiki/hypercard".to_string()),
        );
        // A genealogy-class member resolves by the same slug mechanism; the
        // emitted href is the member's actual URI, not a hardcoded wiki class.
        titles.insert(
            "augmenting-intellect-foundations".to_string(),
            (
                "Augmenting Intellect".to_string(),
                "x0k:genealogy/augmenting-intellect-foundations".to_string(),
            ),
        );
        // In-region wiki slug → markdown link to its URI (rewritten to .html later).
        let in_region = rewrite_wikilinks("see [[hypercard]] now", &titles);
        assert_eq!(in_region, "see [HyperCard](x0k:wiki/hypercard) now");
        // In-region genealogy slug → link to the genealogy URI.
        let genealogy = rewrite_wikilinks("see [[augmenting-intellect-foundations]] now", &titles);
        assert_eq!(
            genealogy,
            "see [Augmenting Intellect](x0k:genealogy/augmenting-intellect-foundations) now"
        );
        // Out-of-region slug → plain text, brackets stripped, no href.
        let out_region = rewrite_wikilinks("see [[resonant-computing]] now", &titles);
        assert_eq!(out_region, "see resonant-computing now");
        // Malformed (uppercase / spaces) is left verbatim — tight regex.
        assert_eq!(rewrite_wikilinks("[[Foo Bar]]", &titles), "[[Foo Bar]]");
        assert_eq!(rewrite_wikilinks("[[ ]]", &titles), "[[ ]]");
    }

    #[test]
    fn wikilink_resolves_end_to_end_to_local_html() {
        // An in-region `[[slug]]` becomes a markdown link, then the cross-doc
        // rewrite turns the `x0k:wiki/slug` href into the member's local file.
        let entry = "---\nx0k:\n  format: folio/v1\n  type: wiki\n  id: x0k:wiki/lineage\n  subtype: index\n  summary: x\n  edges:\n    cites:\n      - x0k:wiki/hypercard\n---\n\n# Lineage\n\nSee [[hypercard]].\n".to_string();
        let card = doc("HyperCard", "Body.");
        let input = RegionInput {
            entry_point_uri: "x0k:wiki/lineage".to_string(),
            members: vec![
                member("x0k:wiki/lineage", "knowledge/wiki/lineage.md", &entry),
                member("x0k:wiki/hypercard", "knowledge/wiki/hypercard.md", &card),
            ],
        };
        let out = weave_region(&input).unwrap();
        let index = std::str::from_utf8(
            &out.files
                .iter()
                .find(|f| f.rel_path == std::path::Path::new("index.html"))
                .unwrap()
                .bytes,
        )
        .unwrap();
        // The wikilink resolved to the slugged member file.
        assert!(
            index.contains(r#"href="wiki-hypercard.html""#),
            "in-region wikilink should resolve to local html; got body without it"
        );
        // No raw x0k: wikilink href survived.
        assert!(!index.contains(r#"href="x0k:wiki/hypercard""#));
        assert!(
            out.unresolved_links.is_empty(),
            "got {:?}",
            out.unresolved_links
        );
        validate_artifact(&out).expect("artifact should validate");
    }

    #[test]
    fn wiki_frontmatter_with_extra_fields_weaves() {
        // Wiki frontmatter carries extra fields (subtype, summary, edges.cites).
        // Confirm parse_document + weave_html tolerate them.
        let content = "---\nx0k:\n  format: folio/v1\n  type: wiki\n  id: x0k:wiki/x\n  subtype: node\n  summary: a short summary\n  updated_by: agent\n  created_at: 2026-06-05\n  concerns: [lineage]\n  edges:\n    cites:\n      - x0k:wiki/y\n---\n\n# X Page\n\nProse with a [[y]] link.\n";
        let parsed = parse_document(content).expect("wiki frontmatter should parse");
        let out = weave_html(content, &parsed).expect("wiki page should weave");
        assert!(out.html.starts_with("<!DOCTYPE html>"));
        assert_eq!(out.title.as_deref(), Some("X Page"));
    }

    #[test]
    fn media_harvest_is_deduped_across_members() {
        let body_a = "Intro\n\n```x0k:media {ref=\"x0k:implementation/canvas/viz\"}\n```\n";
        let body_b =
            "More\n\n```x0k:media {ref=\"x0k:implementation/canvas/viz\"}\n```\n\n```x0k:media {ref=\"x0k:implementation/canvas/other\"}\n```\n";
        let input = RegionInput {
            entry_point_uri: "x0k:design/a".to_string(),
            members: vec![
                member("x0k:design/a", "decisions/design/a.md", &doc("A", body_a)),
                member("x0k:design/b", "decisions/design/b.md", &doc("B", body_b)),
            ],
        };
        let out = weave_region(&input).unwrap();
        assert_eq!(
            out.media_refs,
            vec![
                "x0k:implementation/canvas/other".to_string(),
                "x0k:implementation/canvas/viz".to_string(),
            ]
        );
    }

    /// 2-member golden region with bidirectional links: both pages exist, links
    /// resolve both ways, nav is present on both, and the artifact validates.
    #[test]
    fn two_member_golden_region_bidirectional() {
        let body_a = "See [the architecture](x0k:architecture/web).\n";
        let body_b = "Back to [the design](x0k:design/author).\n";
        let input = RegionInput {
            entry_point_uri: "x0k:design/author".to_string(),
            members: vec![
                member(
                    "x0k:design/author",
                    "decisions/design/author.md",
                    &doc("Author", body_a),
                ),
                member(
                    "x0k:architecture/web",
                    "decisions/architecture/web.md",
                    &doc("Web First", body_b),
                ),
            ],
        };
        let out = weave_region(&input).unwrap();

        // Both pages exist (plus the emitted atlas.json).
        assert_eq!(out.files.len(), 3);
        assert!(out
            .files
            .iter()
            .any(|f| f.rel_path == std::path::Path::new(crate::atlas::ATLAS_FILE)));
        let names: BTreeSet<String> = out
            .files
            .iter()
            .map(|f| f.rel_path.to_string_lossy().to_string())
            .collect();
        assert!(names.contains("index.html"));
        assert!(names.contains("architecture-web.html"));
        assert_eq!(out.entry_rel_path, PathBuf::from("index.html"));

        // Links resolve both ways.
        let index = std::str::from_utf8(
            &out.files
                .iter()
                .find(|f| f.rel_path == std::path::Path::new("index.html"))
                .unwrap()
                .bytes,
        )
        .unwrap();
        let arch = std::str::from_utf8(
            &out.files
                .iter()
                .find(|f| f.rel_path == std::path::Path::new("architecture-web.html"))
                .unwrap()
                .bytes,
        )
        .unwrap();
        assert!(
            index.contains(r#"href="architecture-web.html""#),
            "entry should link to architecture page"
        );
        assert!(
            arch.contains(r#"href="index.html""#),
            "architecture page should link back to entry"
        );

        // Nav present on both.
        assert!(index.contains("region-nav"));
        assert!(arch.contains("region-nav"));

        // No unresolved links (both are members).
        assert!(
            out.unresolved_links.is_empty(),
            "got {:?}",
            out.unresolved_links
        );

        // Artifact validates.
        validate_artifact(&out).expect("artifact should validate");
    }
}
```

## The whole artifact, end to end

The unit tests above hold the region layer to its purity: strings in,
strings out, one rule at a time. Two things they cannot reach are exactly the
two the reader meets. The first is the *artifact as a whole* — every page
standalone, every intra-region link resolving, the entry present — which is a
property of the set, not of any rewrite. The second is the I/O leg that
[`region-project.md`](region-project.md) owns: publication doc and member files
on disk, a directory of pages written out. `tests/region_weave.rs` covers both,
against the crate's public surface.

```rust {#region-tests-doc file="tests/region_weave.rs"}
//! Integration tests for region projection — the `region-weave` and
//! `region-project` chapters of the crate's literate source.
//!
//! These exercise the public surface end-to-end: the pure
//! [`x0k_tangle::weave_region`] + [`x0k_tangle::validate_artifact`] invariants,
//! and the I/O orchestration [`x0k_tangle::project_publication`] against a real
//! temp-dir workspace (publication doc + member decision files on disk → a
//! written multi-page artifact).
```

The fixtures are deliberately minimal folio documents built by `format!`
rather than fixture files, because the region layer cares about frontmatter
`id` and links and nothing else. An empty atlas stands in wherever a
hand-built `RegionWeaveOutput` is needed — the validator inspects pages, never
the atlas, so filling one in would be furniture.

```rust {#region-tests-uses file="tests/region_weave.rs"}
use std::fs;
use std::path::PathBuf;

use x0k_tangle::{
    project_publication, validate_artifact, weave_region, ArtifactFile, RegionInput, RegionMember,
};
```

```rust {#region-tests-atlas file="tests/region_weave.rs"}
/// An empty atlas for hand-built `RegionWeaveOutput` fixtures (the validator
/// only inspects HTML pages, not the atlas).
fn empty_atlas() -> x0k_tangle::Atlas {
    x0k_tangle::Atlas {
        nodes: vec![],
        placements: vec![],
        edges: vec![],
        min_year: 0,
        threads: vec![],
        bands: vec![],
        unresolved_years: vec![],
    }
}
```

```rust {#region-tests-fixtures file="tests/region_weave.rs"}
/// A minimal folio/v1 design doc body (frontmatter split off by the weaver).
fn design_doc(id: &str, title: &str, body: &str) -> String {
    format!(
        "---\nx0k:\n  format: folio/v1\n  type: design\n  id: {id}\n  status: proposed\n---\n\n# {title}\n\n{body}\n"
    )
}

fn architecture_doc(id: &str, title: &str, body: &str) -> String {
    format!(
        "---\nx0k:\n  format: folio/v1\n  type: architecture\n  id: {id}\n  status: proposed\n---\n\n# {title}\n\n{body}\n"
    )
}

fn member(uri: &str, src: &str, content: &str) -> RegionMember {
    RegionMember {
        uri: uri.to_string(),
        content: content.to_string(),
        source_path: PathBuf::from(src),
    }
}
```

The first test is the artifact's whole contract in one assertion set: each
page is a complete standalone HTML document, every link between members
resolves to a file that exists, and the entry page is there. This is the test
that would catch a rewrite rule that is individually correct and collectively
wrong.

```rust {#region-tests-self-contained file="tests/region_weave.rs"}
#[test]
fn artifact_is_self_contained_and_links_resolve() {
    let input = RegionInput {
        entry_point_uri: "x0k:design/author".to_string(),
        members: vec![
            member(
                "x0k:design/author",
                "decisions/design/author.md",
                &design_doc(
                    "x0k:design/author",
                    "Author and Publish",
                    "See [the architecture](x0k:architecture/web-first).",
                ),
            ),
            member(
                "x0k:architecture/web-first",
                "decisions/architecture/web-first.md",
                &architecture_doc(
                    "x0k:architecture/web-first",
                    "Web First",
                    "Back to [the design](x0k:design/author).",
                ),
            ),
        ],
    };

    let out = weave_region(&input).expect("weave region");

    // index.html is the entry and is present.
    assert_eq!(out.entry_rel_path, PathBuf::from("index.html"));
    let names: Vec<String> = out
        .files
        .iter()
        .map(|f| f.rel_path.to_string_lossy().to_string())
        .collect();
    assert!(names.contains(&"index.html".to_string()));
    assert!(names.contains(&"architecture-web-first.html".to_string()));

    // Each HTML page: DOCTYPE, inlined <style>, no external stylesheet link.
    // (atlas.json is a sibling artifact, not an HTML page — skip it.)
    for f in out
        .files
        .iter()
        .filter(|f| f.rel_path.extension().map(|e| e == "html").unwrap_or(false))
    {
        let html = std::str::from_utf8(&f.bytes).unwrap();
        assert!(
            html.starts_with("<!DOCTYPE html>"),
            "{} missing DOCTYPE",
            f.rel_path.display()
        );
        assert!(
            html.contains("<style>"),
            "{} missing inlined <style>",
            f.rel_path.display()
        );
        assert!(
            !(html.contains("<link") && html.contains("stylesheet")),
            "{} pulls an external stylesheet",
            f.rel_path.display()
        );
        // Site nav present on every page.
        assert!(
            html.contains("<nav class=\"region-nav\""),
            "{} missing site nav",
            f.rel_path.display()
        );
    }

    // The full validator passes (entry present; every intra-region href maps to
    // a file in the artifact).
    validate_artifact(&out).expect("artifact validates");
}
```

`validate_artifact` earns its keep only by refusing. Its two failure modes
get one test each — a page pointing at a file the artifact does not hold, and a
region whose entry is missing — because a validator that never says no is
indistinguishable from one that does nothing.

```rust {#region-tests-dangling-link file="tests/region_weave.rs"}
#[test]
fn validate_artifact_rejects_dangling_intra_region_link() {
    // Hand-build a bad artifact: index links to a sibling that isn't present.
    let bad = x0k_tangle::RegionWeaveOutput {
        files: vec![ArtifactFile {
            rel_path: PathBuf::from("index.html"),
            bytes: b"<!DOCTYPE html>\n<head><style>x</style></head>\n<body>\n<a href=\"missing.html\">x</a>\n</body>".to_vec(),
        }],
        entry_rel_path: PathBuf::from("index.html"),
        media_refs: vec![],
        unresolved_links: vec![],
        degraded_embeds: vec![],
        atlas: empty_atlas(),
        transclusion_warnings: vec![],
    };
    let err = validate_artifact(&bad).expect_err("dangling link must fail validation");
    assert!(
        err.to_string().contains("missing.html"),
        "unexpected error: {err}"
    );
}
```

```rust {#region-tests-missing-entry file="tests/region_weave.rs"}
#[test]
fn validate_artifact_rejects_missing_entry() {
    let bad = x0k_tangle::RegionWeaveOutput {
        files: vec![ArtifactFile {
            rel_path: PathBuf::from("other.html"),
            bytes: b"<!DOCTYPE html>\n<style>x</style>\n".to_vec(),
        }],
        entry_rel_path: PathBuf::from("index.html"),
        media_refs: vec![],
        unresolved_links: vec![],
        degraded_embeds: vec![],
        atlas: empty_atlas(),
        transclusion_warnings: vec![],
    };
    let err = validate_artifact(&bad).expect_err("missing entry must fail");
    assert!(err.to_string().contains("index.html"), "got: {err}");
}
```

Everything to here runs in memory. The I/O leg is the projector's, and it
is tested the only way it can be honestly tested: publication doc and member
files written to a temp directory, `project_publication` run over them, and the
written tree read back and walked as a reader would.

```rust {#region-tests-project-to-disk file="tests/region_weave.rs"}
#[test]
fn project_publication_writes_navigable_artifact_to_disk() {
    let ws = tempfile::tempdir().unwrap();
    let ws_root = ws.path();

    // Member decision docs.
    let design_dir = ws_root.join("decisions/design");
    let arch_dir = ws_root.join("decisions/architecture");
    let pub_dir = ws_root.join("decisions/publications");
    fs::create_dir_all(&design_dir).unwrap();
    fs::create_dir_all(&arch_dir).unwrap();
    fs::create_dir_all(&pub_dir).unwrap();

    fs::write(
        design_dir.join("author.md"),
        design_doc(
            "x0k:design/author",
            "Author and Publish",
            "See [the architecture](x0k:architecture/web-first) for the substrate.",
        ),
    )
    .unwrap();
    fs::write(
        arch_dir.join("web-first.md"),
        architecture_doc(
            "x0k:architecture/web-first",
            "Web First, Progressively Native",
            "Realizes [the design](x0k:design/author).",
        ),
    )
    .unwrap();

    // The publication doc demarcating the region.
    let publication = "---\n\
        x0k:\n\
        \x20\x20format: folio/v1\n\
        \x20\x20type: publication\n\
        \x20\x20id: x0k:publication/sample-region\n\
        \x20\x20status: proposed\n\
        \x20\x20edges:\n\
        \x20\x20\x20\x20publishes:\n\
        \x20\x20\x20\x20\x20\x20- x0k:design/author\n\
        \x20\x20\x20\x20\x20\x20- x0k:architecture/web-first\n\
        \x20\x20\x20\x20entryPoint:\n\
        \x20\x20\x20\x20\x20\x20- x0k:design/author\n\
        ---\n\n# Sample Region\n\nA demonstration publication.\n";
    let pub_path = pub_dir.join("sample-region.md");
    fs::write(&pub_path, publication).unwrap();

    // Project into an output dir (inside the tempdir; never the repo tree).
    let out_dir = ws_root.join("artifact");
    // no_motifs=true: no motif refs in this sample, and we don't want the test
    // to invoke the cargo-driven motif build.
    let report = project_publication(&pub_path, &out_dir, ws_root, true).expect("project");

    // page_count counts the woven semantic fallback pages (under pages/).
    assert_eq!(report.page_count, 2);
    assert!(
        report.unresolved_links.is_empty(),
        "{:?}",
        report.unresolved_links
    );

    // The render-vello canvas shell is the root index.html (Phase 5).
    let shell = out_dir.join("index.html");
    assert!(shell.is_file(), "canvas shell not written");
    assert_eq!(out_dir.join(&report.entry_rel_path), shell);
    let shell_html = fs::read_to_string(&shell).unwrap();
    assert!(
        shell_html.contains("vello-canvas"),
        "root index isn't the shell"
    );
    assert!(
        shell_html.contains("pages/index.html"),
        "shell missing fallback link"
    );

    // The woven semantic pages are the no-WebGPU fallback under pages/.
    let index = out_dir.join("pages/index.html");
    let arch = out_dir.join("pages/architecture-web-first.html");
    assert!(index.is_file(), "fallback entry not written");
    assert!(arch.is_file(), "member fallback page not written");

    // Links resolve both ways across the woven fallback files (bare filenames,
    // resolved within pages/ at runtime).
    let index_html = fs::read_to_string(&index).unwrap();
    let arch_html = fs::read_to_string(&arch).unwrap();
    assert!(
        index_html.contains("href=\"architecture-web-first.html\""),
        "entry should link to architecture page"
    );
    assert!(
        arch_html.contains("href=\"index.html\""),
        "architecture page should link back to entry"
    );

    // Site nav on both, marking the current page.
    assert!(index_html.contains("<nav class=\"region-nav\""));
    assert!(arch_html.contains("<nav class=\"region-nav\""));
    assert!(index_html.contains("aria-current=\"page\""));

    // Bundled boot assets + region data at the artifact root.
    assert!(out_dir.join("boot.js").is_file(), "boot.js not written");
    assert!(
        out_dir.join("members.json").is_file(),
        "members.json not written"
    );
    assert!(
        out_dir.join("narrative.json").is_file(),
        "narrative.json not written"
    );
    assert!(
        out_dir.join("atlas.json").is_file(),
        "atlas.json not written"
    );
}
```

A publication with one member has no `entryPoint` to declare, and demanding
one would be a rule with a single obvious answer. The default is the sole
member, projected as `index.html`:

```rust {#region-tests-default-entry file="tests/region_weave.rs"}
#[test]
fn single_member_publication_defaults_entry() {
    let ws = tempfile::tempdir().unwrap();
    let ws_root = ws.path();
    let design_dir = ws_root.join("decisions/design");
    let pub_dir = ws_root.join("decisions/publications");
    fs::create_dir_all(&design_dir).unwrap();
    fs::create_dir_all(&pub_dir).unwrap();
    fs::write(
        design_dir.join("solo.md"),
        design_doc("x0k:design/solo", "Solo", "Just one doc."),
    )
    .unwrap();

    let publication = "---\n\
        x0k:\n\
        \x20\x20format: folio/v1\n\
        \x20\x20type: publication\n\
        \x20\x20id: x0k:publication/solo-pub\n\
        \x20\x20status: proposed\n\
        \x20\x20edges:\n\
        \x20\x20\x20\x20publishes:\n\
        \x20\x20\x20\x20\x20\x20- x0k:design/solo\n\
        ---\n\n# Solo Pub\n";
    let pub_path = pub_dir.join("solo-pub.md");
    fs::write(&pub_path, publication).unwrap();

    let out_dir = ws_root.join("artifact");
    let report = project_publication(&pub_path, &out_dir, ws_root, true).expect("project");
    assert_eq!(report.page_count, 1);
    // Canvas shell at the root; the sole woven page is the fallback entry.
    assert!(out_dir.join("index.html").is_file());
    assert!(out_dir.join("pages/index.html").is_file());
}
```

The self-booting artifact is structural acceptance rather than a rule: the
bundle carries the render-vello wasm, the boot shell, the region data, and the
woven semantic pages as the no-WebGPU fallback — and the shell loads nothing
from outside itself, which is what makes the artifact a single file a reader
can open offline. The wasm is located under the default
`<workspace>/ui/0k.computer/wasm/render-vello`, faked here so the test does not
wait on a real wasm build. Bundling *is* the `motifs` feature, so the test is
gated on it; the motifs-severed build reports `wasm_bundled: false` on purpose,
and a test that ran there would be asserting the wrong thing.

```rust {#region-tests-self-booting file="tests/region_weave.rs"}
#[test]
#[cfg_attr(not(feature = "motifs"), ignore = "wasm bundling requires the motifs feature")]
fn self_booting_artifact_bundles_wasm_data_shell_and_fallback() {
    let ws = tempfile::tempdir().unwrap();
    let ws_root = ws.path();
    let wiki_dir = ws_root.join("knowledge/wiki");
    let pub_dir = ws_root.join("decisions/publications");
    fs::create_dir_all(&wiki_dir).unwrap();
    fs::create_dir_all(&pub_dir).unwrap();

    // A fake prebuilt render-vello wasm bundle at the default location.
    let wasm_src = ws_root.join("ui/0k.computer/wasm/render-vello");
    fs::create_dir_all(&wasm_src).unwrap();
    fs::write(wasm_src.join("x0k_ui_render_vello.js"), b"// fake js\n").unwrap();
    fs::write(
        wasm_src.join("x0k_ui_render_vello_bg.wasm"),
        b"\0asm fake wasm bytes",
    )
    .unwrap();

    // Two wiki members with a frontmatter summary + body (deep-doc portal text).
    for (slug, title) in [("alpha", "Alpha Page"), ("beta", "Beta Page")] {
        fs::write(
            wiki_dir.join(format!("{slug}.md")),
            format!(
                "---\nx0k:\n  format: folio/v1\n  type: wiki\n  id: x0k:wiki/{slug}\n  summary: Summary of {title}\n---\n\n# {title}\n\nReal prose body for {title}.\n"
            ),
        )
        .unwrap();
    }

    let publication = "---\n\
        x0k:\n\
        \x20\x20format: folio/v1\n\
        \x20\x20type: publication\n\
        \x20\x20id: x0k:publication/lineage-mini\n\
        \x20\x20status: proposed\n\
        \x20\x20edges:\n\
        \x20\x20\x20\x20publishes:\n\
        \x20\x20\x20\x20\x20\x20- x0k:wiki/alpha\n\
        \x20\x20\x20\x20\x20\x20- x0k:wiki/beta\n\
        \x20\x20\x20\x20entryPoint:\n\
        \x20\x20\x20\x20\x20\x20- x0k:wiki/alpha\n\
        ---\n\n# Lineage Mini\n";
    let pub_path = pub_dir.join("lineage-mini.md");
    fs::write(&pub_path, publication).unwrap();

    // A narrative sidecar beside the publication doc.
    fs::write(
        pub_dir.join("lineage-mini.narrative.json"),
        br#"{"title":"T","thesis":"Th","stations":[{"id":"s1","ordinal":1,"title":"S1","thread":"x","empower":"e","shadow":"sh","highlight":["x0k:wiki/alpha"],"key_edges":[]}]}"#,
    )
    .unwrap();

    let out_dir = ws_root.join("artifact");
    let report = project_publication(&pub_path, &out_dir, ws_root, true).expect("project");

    // Renderer + narrative bundled.
    assert!(report.wasm_bundled, "render-vello wasm should be bundled");
    assert!(report.wasm_bytes > 0);
    assert!(
        report.narrative_bundled,
        "narrative sidecar should be bundled"
    );

    // Every required artifact file is present.
    for rel in [
        "index.html",
        "boot.js",
        "atlas.json",
        "members.json",
        "narrative.json",
        "wasm/x0k_ui_render_vello.js",
        "wasm/x0k_ui_render_vello_bg.wasm",
        "pages/index.html",
        "pages/wiki-beta.html",
    ] {
        assert!(out_dir.join(rel).is_file(), "missing artifact file: {rel}");
    }

    // The bundled members.json carries real per-member body text.
    let members: serde_json::Value =
        serde_json::from_slice(&fs::read(out_dir.join("members.json")).unwrap()).unwrap();
    assert_eq!(members["members"]["x0k:wiki/alpha"]["title"], "Alpha Page");
    assert!(members["members"]["x0k:wiki/alpha"]["body"]
        .as_str()
        .unwrap()
        .contains("Real prose body"));

    // The canvas shell loads ONLY same-origin relative resources (no http(s)
    // <script>/<link> CDN loads). Prose hyperlinks live in the fallback pages,
    // not the shell.
    let shell = fs::read_to_string(out_dir.join("index.html")).unwrap();
    assert!(shell.contains("vello-canvas"));
    assert!(
        !shell.contains("http://"),
        "shell has an external http resource"
    );
    assert!(
        !shell.contains("https://"),
        "shell has an external https resource"
    );
    assert!(
        shell.contains("./boot.js") || shell.contains("\"boot.js\""),
        "shell should load the local boot script"
    );
}
```

### Transclusion across the region

A spine document composes sections out of its neighbours — by an inline
`x0k:transclude` fence or by a `transcludes:` sequence in its frontmatter — and
the region weaver has to resolve those at the same seam where it resolves
links. Both spellings land in one woven page with both sections inlined, and
the sections that were *not* named stay where they were:

```rust {#region-tests-transclude-two-briefs file="tests/region_weave.rs"}
#[test]
fn spine_transcludes_two_briefs_into_one_woven_page() {
    let design_a = "---\nx0k:\n  format: folio/v1\n  type: design\n  id: x0k:design/alpha\n  status: proposed\n---\n\n# Alpha\n\nlead\n\n## Brief\n\nAlpha's brief paragraph.\n\n## Purpose\n\nAlpha purpose (must NOT be inlined).\n";
    let design_b = "---\nx0k:\n  format: folio/v1\n  type: design\n  id: x0k:design/beta\n  status: proposed\n---\n\n# Beta\n\nlead\n\n## Brief\n\nBeta's brief paragraph.\n\n## Purpose\n\nBeta purpose (must NOT be inlined).\n";
    // Spine: inline fence pulls alpha#brief; frontmatter pulls beta#brief.
    let spine = "---\nx0k:\n  format: folio/v1\n  type: wiki\n  id: x0k:wiki/spine\n  status: stable\n  transcludes:\n    - x0k:design/beta#brief\n---\n\n# Spine\n\nConnective prose before.\n\n```x0k:transclude {ref=\"x0k:design/alpha#brief\"}\n```\n\nConnective prose after.\n";

    let input = RegionInput {
        entry_point_uri: "x0k:wiki/spine".to_string(),
        members: vec![
            RegionMember {
                uri: "x0k:wiki/spine".to_string(),
                content: spine.to_string(),
                source_path: PathBuf::from("knowledge/wiki/spine.md"),
            },
            RegionMember {
                uri: "x0k:design/alpha".to_string(),
                content: design_a.to_string(),
                source_path: PathBuf::from("decisions/design/alpha.md"),
            },
            RegionMember {
                uri: "x0k:design/beta".to_string(),
                content: design_b.to_string(),
                source_path: PathBuf::from("decisions/design/beta.md"),
            },
        ],
    };

    let out = weave_region(&input).unwrap();
    let index = std::str::from_utf8(
        &out.files
            .iter()
            .find(|f| f.rel_path == std::path::Path::new("index.html"))
            .unwrap()
            .bytes,
    )
    .unwrap();

    // Both briefs inlined into the spine's index page.
    assert!(
        index.contains("Alpha's brief paragraph."),
        "inline-fence transclusion of alpha#brief not inlined"
    );
    assert!(
        index.contains("Beta's brief paragraph."),
        "frontmatter transclusion of beta#brief not inlined"
    );
    // The spine's own connective prose surrounds them.
    assert!(index.contains("Connective prose before."));
    assert!(index.contains("Connective prose after."));
    // Section extraction stops at the next heading — purpose NOT pulled in.
    assert!(
        !index.contains("Alpha purpose"),
        "section extraction leaked past the #brief heading"
    );
    assert!(!index.contains("Beta purpose"));
    // No raw transclude fence survived into the woven HTML.
    assert!(!index.contains("x0k:transclude"));
    // Clean resolution — no degraded references.
    assert!(
        out.transclusion_warnings.is_empty(),
        "unexpected transclusion warnings: {:?}",
        out.transclusion_warnings
    );

    validate_artifact(&out).expect("woven spine artifact validates");
}
```

Transclusion is only worth having if it stays live. An edit applied to the
source document's section — through the same `replace_section` the viewer's
save path uses — must appear when the spine re-resolves, and the spine's own
bytes must be untouched, because the spine holds a reference and not a copy.
That is the design's edit-through property
(`decisions/design/corpus/transclusion.md` §"Edit-through") proved at the
weave/resolution seam, which is the naga-free analogue of the native
re-render.

```rust {#region-tests-edit-through file="tests/region_weave.rs"}
#[test]
fn edit_through_to_source_is_reflected_when_spine_reresolves() {
    use x0k_folio::transclusion::{replace_section, split_body};

    let source_before = "---\nx0k:\n  format: folio/v1\n  type: design\n  id: x0k:design/source\n  status: proposed\n---\n\n# Source\n\n## Brief\n\noriginal source brief.\n\n## Purpose\n\npurpose stays put.\n";
    let spine = "---\nx0k:\n  format: folio/v1\n  type: wiki\n  id: x0k:wiki/spine\n  status: stable\n---\n\n# Spine\n\nSpine connective prose.\n\n```x0k:transclude {ref=\"x0k:design/source#brief\"}\n```\n";

    let weave = |source_content: &str| {
        let input = RegionInput {
            entry_point_uri: "x0k:wiki/spine".to_string(),
            members: vec![
                RegionMember {
                    uri: "x0k:wiki/spine".to_string(),
                    content: spine.to_string(),
                    source_path: PathBuf::from("knowledge/wiki/spine.md"),
                },
                RegionMember {
                    uri: "x0k:design/source".to_string(),
                    content: source_content.to_string(),
                    source_path: PathBuf::from("decisions/design/source.md"),
                },
            ],
        };
        let out = weave_region(&input).unwrap();
        std::str::from_utf8(
            &out.files
                .iter()
                .find(|f| f.rel_path == std::path::Path::new("index.html"))
                .unwrap()
                .bytes,
        )
        .unwrap()
        .to_string()
    };

    // Before the edit: the spine inlines the original brief.
    let index_before = weave(source_before);
    assert!(index_before.contains("original source brief."));
    assert!(!index_before.contains("EDITED source brief."));

    // Apply the edit-through to the SOURCE body's `#brief` section — the
    // exact transform `DocumentViewer::save_transclusion_edit` performs.
    let (yaml, body) = split_body(source_before);
    let new_body = replace_section(body, "brief", "## Brief\n\nEDITED source brief.\n").unwrap();
    let source_after = format!("---{}\n---\n{new_body}", yaml.unwrap());
    // The source's other section is preserved by the edit.
    assert!(source_after.contains("purpose stays put."));
    assert!(source_after.contains("EDITED source brief."));
    assert!(!source_after.contains("original source brief."));

    // After the edit: re-resolving the spine inlines the EDITED brief; the
    // spine's own connective prose is unchanged (the spine content never
    // mutated — only the source did).
    let index_after = weave(&source_after);
    assert!(
        index_after.contains("EDITED source brief."),
        "spine should re-resolve to the edited source content"
    );
    assert!(!index_after.contains("original source brief."));
    assert!(
        index_after.contains("Spine connective prose."),
        "spine's own prose untouched by the source edit"
    );
    // The edit didn't leak the source's other section into the spine.
    assert!(!index_after.contains("purpose stays put."));
}
```

A spine that transcludes itself is the one input that can turn resolution
into a loop. It degrades to a link with a recorded warning — the reader gets a
page, the author gets told:

```rust {#region-tests-self-transclusion file="tests/region_weave.rs"}
#[test]
fn spine_self_transclusion_degrades_to_link_with_warning() {
    let spine = "---\nx0k:\n  format: folio/v1\n  type: wiki\n  id: x0k:wiki/loop\n  status: stable\n---\n\n# Loop\n\n```x0k:transclude {ref=\"x0k:wiki/loop\"}\n```\n";
    let input = RegionInput {
        entry_point_uri: "x0k:wiki/loop".to_string(),
        members: vec![RegionMember {
            uri: "x0k:wiki/loop".to_string(),
            content: spine.to_string(),
            source_path: PathBuf::from("knowledge/wiki/loop.md"),
        }],
    };
    let out = weave_region(&input).unwrap();
    assert!(
        out.transclusion_warnings
            .iter()
            .any(|w| w.contains("Cycle")),
        "expected a cycle warning, got {:?}",
        out.transclusion_warnings
    );
    validate_artifact(&out).expect("artifact still validates after degrade");
}
```

```rust {#region-tests-root file="tests/region_weave.rs"}
<<region-tests-doc>>

<<region-tests-uses>>

<<region-tests-atlas>>

<<region-tests-fixtures>>

<<region-tests-self-contained>>

<<region-tests-dangling-link>>

<<region-tests-missing-entry>>

<<region-tests-project-to-disk>>

<<region-tests-default-entry>>

<<region-tests-self-booting>>

<<region-tests-transclude-two-briefs>>

<<region-tests-edit-through>>

<<region-tests-self-transclusion>>
```

## Composing the module

```rust {#root}
<<module-doc>>

<<uses>>

<<region-member>>

<<artifact-file>>

<<region-weave-output>>

<<weave-region>>

<<region-doc-source>>

<<scan-media-refs>>

<<reassemble>>

<<entry-file>>

<<wiki-title-map>>

<<first-h1>>

<<rewrite-wikilinks>>

<<uri-to-path>>

<<nav-entry>>

<<rewrite-cross-doc-links>>

<<site-nav>>

<<validate>>

<<tests>>
```

A region is woven the way it is read: page by page, with the links between
pages the only thing the region layer needs to know.
