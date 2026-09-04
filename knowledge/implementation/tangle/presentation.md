---
x0k:
  format: folio/v1
  id: x0k:implementation/tangle/presentation
  type: implementation
  status: draft
  summary: "Wrapping the woven pages in a canvas shell instead of forking the weaver: the pages move under `pages/` as the fallback a screen reader and a crawler still get, and the shell reads static JSON with no daemon in the loop."
  concerns: [tangle, publication, presentation, render-vello, fallback, theme]
  tangle:
    crate: x0k-tangle
    root: src/presentation.rs
  edges:
    implements:
      - x0k:design/author-and-publish-the-same-surface
    cites:
      - x0k:implementation/tangle/region-weave
      - x0k:implementation/tangle/region-project
      - x0k:implementation/tangle/atlas
      - x0k:implementation/folio/colophon
---

# The shell wraps the weave; it does not fork it

A woven region is already a publication: one standalone HTML page per
member, cross-linked, with a site nav and an `atlas.json`. It is also
plain. The publication the design asks for lands the reader on a
render-vello canvas — the atlas as a navigable time×idea plane, a scripted
narrative trail, a deep-doc portal — and still has to work for a screen
reader, a search crawler, and a browser without WebGPU. This module
resolves that by wrapping the weaver's output rather than changing it: the
woven pages move under `pages/` as the fallback, a canvas shell takes the
root `index.html`, and the data the shell needs is bundled as static JSON it
`fetch`es with no daemon in the loop.

The carried example is a two-member region, Alpha (entry) and Beta. After
the shell is applied, the artifact holds `index.html` (the canvas shell,
titled from Alpha's H1), `boot.js`, `atlas.json`, `members.json`,
`narrative.json`, and under `pages/` the woven `index.html` and
`wiki-b.html` — each now wearing the publication theme, and each still
linking to the other by bare filename because the whole set moved together.

## Purity

Everything here mutates a `RegionWeaveOutput` in memory and reads nothing
from disk. The render-vello wasm is a prebuilt binary the I/O layer
(`x0k:implementation/tangle/region-project`) copies; the narrative trail
arrives as bytes the I/O layer read from a sidecar, or is stubbed. That
split is what lets the shell be unit-tested by weaving two strings.

```rust {#module-doc}
//! Publication shell: turn a woven region into a **self-booting publication
//! artifact**.
//!
//! [`region_weave`](crate::region_weave) emits the semantic substrate: one
//! standalone `.html` page per member plus the `atlas.json` time×thread layout.
//! This module wraps that output (it does NOT fork the weaver) into the rich,
//! offline, render-vello presentation:
//!
//! - the artifact root `index.html` becomes the **render-vello canvas shell**
//!   (a stripped `CanvasHost`; see [`SHELL_INDEX`] / [`BOOT_JS`]),
//! - the woven semantic pages move under `pages/` as the **a11y/SEO/no-WebGPU
//!   fallback** (the shell redirects there when WebGPU is absent or boot fails),
//! - region data is bundled as static JSON the boot shell `fetch`es with zero
//!   daemon coupling: `atlas.json` (already emitted by the weaver),
//!   `members.json` (per-member title/summary/body for the deep-doc portal), and
//!   `narrative.json` (the scripted camera trail — sourced from a repo sidecar).
//!
//! The render-vello wasm itself is bundled by the I/O layer
//! ([`crate::region_project`]) because it is a prebuilt binary copy, not pure
//! data. Everything in THIS module is pure and unit-testable.

use crate::region_weave::{ArtifactFile, RegionInput, RegionWeaveOutput};
use std::path::{Path, PathBuf};
```

## The bundled shell

The shell and boot script are compiled in from `templates/publication/`.
The filenames are constants because the boot script, the fallback redirect,
and the I/O layer all agree on them.

Worth saying plainly, because the two template files are the largest
non-Rust things in the crate and a reader will open them: `boot.js` does
`import('./wasm/x0k_ui_render_vello.js')`, and that wasm is a *prebuilt
binary the I/O layer copies in* ([`region-project.md`](region-project.md)
§ "The render-vello bundle"), behind the `motifs` feature and from a
directory an environment variable names. Neither the wasm nor the crate
that builds it is in this repository, and `weave-region` — the only verb
that produces an artifact these templates go into — needs a corpus
checkout. So what ships here is the shell's *source*, readable and
tested for what it promises structurally; a canvas artifact is not
something this repository can produce on its own, and the fallback
`pages/` weave, which needs none of that, is what a reader can build.

```rust {#shell-assets}
/// The render-vello canvas shell (root `index.html`). Boots [`BOOT_JS`], detects
/// missing WebGPU and falls back to `pages/index.html`.
///
/// A template, not a finished file: it carries `__C0K_PUB_TITLE__` and
/// `__C0K_PUB_STATS__` holes that [`apply_publication_shell`] substitutes.
pub const SHELL_INDEX: &str = include_str!("../templates/publication/index.html");

/// The boot shell — a stripped `CanvasHost` that renders the bundled atlas +
/// narrative over render-vello, offline.
///
/// It imports `./wasm/x0k_ui_render_vello.js`, a prebuilt wasm-pack bundle
/// that [`crate::region_project`] copies in under the `motifs` feature. That
/// binary is not part of this crate, so this script is inert without it.
pub const BOOT_JS: &str = include_str!("../templates/publication/boot.js");

/// Subtree the woven semantic fallback pages live under.
pub const FALLBACK_DIR: &str = "pages";
/// The boot shell filename (root of the artifact).
pub const SHELL_FILE: &str = "index.html";
/// Bundled boot script filename.
pub const BOOT_FILE: &str = "boot.js";
/// Bundled per-member metadata (title/summary/body) for the deep-doc portal.
pub const MEMBERS_FILE: &str = "members.json";
/// Bundled scripted-narrative trail.
pub const NARRATIVE_FILE: &str = "narrative.json";
```

## Applying the shell

Four steps, in the order the artifact grows. The thread map is built first,
from the atlas, so the fallback pages can wear the same per-thread accent as
the canvas; a node in several lanes wears its primary (first) lane. Then
every woven page moves under `pages/` and gets the theme injected — and the
motif loader too, but only when the page carries an actual `data-media-ref`
mount point, since the shared stylesheet mentions `.media-embed` on every
page and matching the class name would pull in a loader the I/O layer never
bundles for embed-less regions. `members.json` and `narrative.json` follow,
then the shell itself with the publication's title and a member/facet count
substituted for the template's placeholders. The entry point becomes the
shell.

```rust {#apply-publication-shell}
/// Wrap a woven [`RegionWeaveOutput`] into the self-booting presentation layout:
/// move the woven pages under `pages/`, emit `members.json`, bundle the
/// `narrative.json` (sidecar bytes if provided, else a minimal stub), and add
/// the canvas shell + boot script at the root. `atlas.json` is left where the
/// weaver placed it (artifact root).
///
/// Pure: mutates `out` in place, reads nothing from disk. The render-vello wasm
/// copy is the I/O layer's job (see [`crate::region_project`]).
pub fn apply_publication_shell(
    out: &mut RegionWeaveOutput,
    input: &RegionInput,
    narrative_json: Option<Vec<u8>>,
) {
    // Map each woven page's artifact path → its lineage thread, so the woven
    // fallback pages can wear the same per-thread accent as the canvas atlas.
    // Built BEFORE the mutable file loop (reads `out.atlas`, an immutable
    // borrow that ends before `iter_mut`).
    let uri_to_path = crate::region_weave::build_uri_to_path(input);
    let mut path_to_thread: std::collections::HashMap<PathBuf, String> =
        std::collections::HashMap::new();
    for n in &out.atlas.nodes {
        if let Some(p) = uri_to_path.get(&n.uri) {
            // A node may belong to several lanes; the woven page wears the
            // primary (lowest-row) lane's accent — its first `threads` entry.
            if let Some(primary) = n.threads.first() {
                path_to_thread.insert(p.clone(), primary.clone());
            }
        }
    }

    // 1. Move every woven `.html` page under `pages/`. The pages link to each
    //    other (and to the woven entry) by bare filename; relocating the whole
    //    set together preserves those relative links unchanged at runtime.
    //    Each page also gets the publication **theme** injected (the atlas-essay
    //    register — serif body, mono technical, generous measure, calm light
    //    chrome, per-thread accent) so the no-WebGPU/SEO fallback reads as the
    //    same crafted publication as the canvas, NOT the system-wide dark weave.
    //    The theme is injected here (scoped to this artifact) rather than by
    //    editing the global weave STYLESHEET, which is shared by all of officina.
    //    Pages that carry a `x0k:media` mount point also get the motif loader
    //    script injected, so embedded motifs run live in the served fallback
    //    page (the loader resolves its assets relative to `../wasm/host.js`;
    //    the wasm + `motifs.json` manifest are bundled by the I/O layer).
    for f in out.files.iter_mut() {
        if is_html(&f.rel_path) {
            let thread = path_to_thread.get(&f.rel_path).map(String::as_str);
            f.rel_path = PathBuf::from(FALLBACK_DIR).join(&f.rel_path);
            if let Ok(html) = std::str::from_utf8(&f.bytes) {
                let mut themed = inject_publication_theme(html, thread);
                // Match the actual mount-point attribute, not the bare class
                // name — every woven page carries a `.media-embed` CSS rule in
                // the shared stylesheet, which must not pull in a loader the
                // I/O layer never bundles for embed-less regions.
                if themed.contains("data-media-ref") {
                    themed = inject_motif_loader(&themed);
                }
                f.bytes = themed.into_bytes();
            }
        }
    }

    // 2. members.json — per-member title/summary/body for the deep-doc portal.
    out.files.push(ArtifactFile {
        rel_path: PathBuf::from(MEMBERS_FILE),
        bytes: build_members_json(input),
    });

    // 3. narrative.json — the scripted camera trail (sidecar bytes, else stub).
    let narrative = narrative_json.unwrap_or_else(stub_narrative_json);
    out.files.push(ArtifactFile {
        rel_path: PathBuf::from(NARRATIVE_FILE),
        bytes: narrative,
    });

    // 4. boot.js + the canvas shell at the root. The shell template carries
    //    `__C0K_PUB_TITLE__` / `__C0K_PUB_STATS__` placeholders; the title is
    //    the entry member's H1 (the same source the woven entry page titles
    //    itself from), so the shell names THIS publication, not a baked-in one.
    let title = input
        .members
        .iter()
        .find(|m| m.uri == input.entry_point_uri)
        .and_then(|m| first_h1(&m.content))
        .unwrap_or_else(|| input.entry_point_uri.clone());
    let plural = |n: usize| if n == 1 { "" } else { "s" };
    let (members_n, facets_n) = (input.members.len(), out.atlas.threads.len());
    let stats = format!(
        "{} member{} · {} facet{}",
        members_n,
        plural(members_n),
        facets_n,
        plural(facets_n)
    );
    let shell = SHELL_INDEX
        .replace("__C0K_PUB_TITLE__", &escape_html_text(&title))
        .replace("__C0K_PUB_STATS__", &stats);
    out.files.push(ArtifactFile {
        rel_path: PathBuf::from(BOOT_FILE),
        bytes: BOOT_JS.as_bytes().to_vec(),
    });
    out.files.push(ArtifactFile {
        rel_path: PathBuf::from(SHELL_FILE),
        bytes: shell.into_bytes(),
    });

    // The reader now lands on the canvas shell; the woven entry is the fallback.
    out.entry_rel_path = PathBuf::from(SHELL_FILE);
}
```

```rust {#escape-html-text}
/// Minimal text-node escaping for values substituted into the shell template.
fn escape_html_text(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}
```

## Injections into a woven page

The loader tag points one level up, because pages live under `pages/` and
the loader resolves its other assets relative to itself. Both injections
are idempotent so a page shelled twice is unchanged.

```rust {#motif-loader}
/// The `<script>` that boots the motif loader on a fallback page. Pages live
/// one level deep (`pages/`), so it points at `../wasm/host.js`; the loader
/// resolves every other asset relative to itself.
const MOTIF_LOADER_TAG: &str = "<script type=\"module\" src=\"../wasm/host.js\"></script>\n";

/// Inject the motif-loader script before `</body>` (else append). Idempotent:
/// skips pages that already reference the loader.
fn inject_motif_loader(html: &str) -> String {
    if html.contains("../wasm/host.js") {
        return html.to_string();
    }
    if let Some(idx) = html.rfind("</body>") {
        let mut out = String::with_capacity(html.len() + MOTIF_LOADER_TAG.len());
        out.push_str(&html[..idx]);
        out.push_str(MOTIF_LOADER_TAG);
        out.push_str(&html[idx..]);
        out
    } else {
        format!("{html}{MOTIF_LOADER_TAG}")
    }
}
```

The palette mirrors `boot.js`'s `THREAD_COLORS` by hand; the two are kept
in step by reading, not by a shared source.

```rust {#thread-hex}
/// The eight-thread hue palette (mirrors `boot.js` `THREAD_COLORS`). The woven
/// fallback page wears its member's thread hue as a top rule + accent so the
/// semantic substrate reads in the same color system as the canvas atlas.
fn thread_hex(thread: Option<&str>) -> &'static str {
    match thread {
        // eight idea-lanes (gold → terracotta → teal → steel-blue → indigo →
        // deep-green → rose)
        Some("founding-vision") => "#c89537",
        Some("malleability") => "#c0562f",
        Some("data-ownership") => "#2c8c78",
        Some("networking") => "#3d6ea8",
        Some("authority-identity") => "#5b4b9e",
        Some("privacy") => "#3f7d57",
        Some("durable-execution") => "#a8446f",
        Some("sovereign-ai") => "#8e3d9e",
        // bands (distinct from the lanes)
        Some("synthesis") => "#5d5470",
        Some("capstone") => "#b03a52",
        Some("frontier") => "#c98a2e",
        _ => "#8a5a2b", // neutral warm accent (the index page / unmapped)
    }
}
```

The theme is injected as a scoped `<style>` before `</head>` rather than by
editing the global weave stylesheet, which every officina page shares. Only
the tokens and the few elements the global sheet sets by literal color are
restated; everything else inherits through the CSS variables.

```rust {#publication-theme}
/// Inject the publication theme (atlas-essay register) as a scoped `<style>`
/// just before `</head>`, so it overrides the global weave STYLESHEET for this
/// artifact only. Idempotent: skips a page already carrying the marker.
fn inject_publication_theme(html: &str, thread: Option<&str>) -> String {
    if html.contains("data-x0k-publication-theme") {
        return html.to_string();
    }
    let accent = thread_hex(thread);
    // The override: light parchment ground, dark ink, serif body at a generous
    // measure, mono for code, calm chrome, and a per-thread accent rule. Only
    // the tokens + the few elements the global sheet sets by literal color are
    // restated; everything else inherits via the CSS vars.
    let style = format!(
        "<style data-x0k-publication-theme>\n\
         :root {{\n\
         --bg:#f7f3ea; --bg-surface:#efe9db; --bg-chunk:#f1ebdb;\n\
         --text:#2b2620; --text-quiet:#6b6256;\n\
         --accent:{accent}; --accent-dim:#b7ad95; --border:#d9cfb8;\n\
         --code-bg:#efe7d6; --chunk-header-bg:#ece4d2; --media-bg:#efe9db; --link:{accent};\n\
         --tok-keyword:#7a55a0; --tok-string:#2c7a4f; --tok-number:#b3681f;\n\
         --tok-comment:#8a8275; --tok-type:#2c6f8c; --tok-function:#7a4a2a;\n\
         --tok-property:#3d6ea8; --tok-operator:#5a5247; --tok-punctuation:#8a8275;\n\
         --font-serif:\"EB Garamond\",\"Palatino Linotype\",Palatino,Georgia,serif;\n\
         }}\n\
         body {{ max-width:42em; font-size:18px; line-height:1.7;\n\
           border-top:4px solid {accent}; padding-top:1.5em; }}\n\
         h1 {{ color:#1f1b16; letter-spacing:-0.01em; }}\n\
         h2 {{ color:#2b2620; border-bottom:1px solid var(--border); }}\n\
         h3 {{ color:#3a342b; }}\n\
         a {{ color:{accent}; }}\n\
         code, pre {{ font-family:var(--font-mono); }}\n\
         /* Calm chrome: the region nav reads as a quiet rule, not a panel. */\n\
         .region-nav {{ background:transparent; border-bottom:1px solid var(--border); }}\n\
         blockquote {{ border-left:3px solid {accent}; color:var(--text-quiet); }}\n\
         </style>\n"
    );
    if let Some(idx) = html.rfind("</head>") {
        let mut out = String::with_capacity(html.len() + style.len());
        out.push_str(&html[..idx]);
        out.push_str(&style);
        out.push_str(&html[idx..]);
        out
    } else {
        format!("{style}{html}")
    }
}
```

```rust {#fallback-paths}
/// The woven semantic entry page's path after the shell is applied
/// (`pages/index.html`) — the no-WebGPU fallback target.
pub fn fallback_entry_path() -> PathBuf {
    PathBuf::from(FALLBACK_DIR).join("index.html")
}

fn is_html(p: &Path) -> bool {
    p.extension().map(|e| e == "html").unwrap_or(false)
}
```

## Bundled data

`members.json` gives the deep-doc portal a title, summary, and prose excerpt
per member without shipping the full pages twice. The stub narrative has
zero stations, which the boot shell renders as the atlas with the launcher
inert.

```rust {#members-json}
/// Build `members.json`: `{ "members": { "<uri>": {title, summary, body} } }`.
/// `title` is the member's first H1, `summary` its frontmatter `summary:` field,
/// `body` a cleaned prose excerpt of its body (the deep-doc portal text).
pub fn build_members_json(input: &RegionInput) -> Vec<u8> {
    use serde_json::{Map, Value};
    let mut members = Map::new();
    for m in &input.members {
        let (front, body) = split_frontmatter(&m.content);
        let title = first_h1(&m.content).unwrap_or_else(|| m.uri.clone());
        let summary = frontmatter_field(front, "summary").unwrap_or_default();
        let excerpt = body_excerpt(body);
        let mut obj = Map::new();
        obj.insert("title".into(), Value::String(title));
        obj.insert("summary".into(), Value::String(summary));
        obj.insert("body".into(), Value::String(excerpt));
        members.insert(m.uri.clone(), Value::Object(obj));
    }
    let mut root = Map::new();
    root.insert("members".into(), Value::Object(members));
    serde_json::to_vec_pretty(&Value::Object(root)).expect("members.json serializes")
}
```

```rust {#stub-narrative}
/// A minimal valid `narrative.json` for publications without a sidecar trail:
/// zero stations (the boot shell renders the atlas with the launcher inert).
fn stub_narrative_json() -> Vec<u8> {
    let v = serde_json::json!({
        "title": "",
        "thesis": "",
        "stations": [],
    });
    serde_json::to_vec_pretty(&v).expect("stub narrative serializes")
}
```

## A third frontmatter splitter

`split_frontmatter` here is the third near-duplicate of one operation in
the published crates, beside `x0k_folio::colophon::split_frontmatter` and
`x0k_folio::transclusion::split_body`. `first_h1` likewise has copies in
the region weaver and the atlas builder. They are kept as they are: each is
a dozen lines, each tolerates slightly different input (this one accepts a
BOM and CRLF envelopes), and the duplication is named rather than resolved
so the byte-for-byte projection of this module stays honest.

```rust {#split-frontmatter}
/// Split a folio/v1 document into `(frontmatter, body)`. If the leading
/// `---`-delimited envelope is absent, frontmatter is empty and the whole input
/// is the body.
fn split_frontmatter(content: &str) -> (&str, &str) {
    let trimmed = content.trim_start_matches('\u{feff}');
    if let Some(rest) = trimmed.strip_prefix("---\n") {
        if let Some(end) = rest.find("\n---\n") {
            let front = &rest[..end];
            let body = &rest[end + "\n---\n".len()..];
            return (front, body);
        }
        if let Some(end) = rest.find("\n---\r\n") {
            let front = &rest[..end];
            let body = &rest[end + "\n---\r\n".len()..];
            return (front, body);
        }
    }
    ("", content)
}
```

```rust {#frontmatter-field}
/// Pull a top-level scalar `key: value` from a frontmatter block (cheap line
/// scan; only matches keys indented two spaces under the `x0k:` envelope, which
/// is where wiki `summary:` lives). Strips surrounding quotes.
fn frontmatter_field(front: &str, key: &str) -> Option<String> {
    let needle = format!("  {key}:");
    for line in front.lines() {
        if let Some(rest) = line.strip_prefix(&needle) {
            let v = rest.trim();
            let v = v.trim_matches(|c| c == '"' || c == '\'');
            if !v.is_empty() {
                return Some(v.to_string());
            }
        }
    }
    None
}
```

```rust {#first-h1}
/// Extract the first markdown `# ` heading from a doc (cheap line scan).
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

## The prose excerpt

The portal shows real prose, not a summary of one. The excerpt drops
headings (the title and summary already carry the framing), fenced code,
and blank-line noise, joins lines within a paragraph, and caps at
`BODY_EXCERPT_MAX` characters, cutting back to a paragraph or sentence
boundary when one exists under the cap.

```rust {#body-excerpt}
/// Maximum body-excerpt length (chars) bundled per member for the deep-doc
/// portal. Keeps the artifact lean while showing real prose; the full page is a
/// click away in the `pages/` fallback.
const BODY_EXCERPT_MAX: usize = 1800;

/// Clean a member body into a prose excerpt for the deep-doc portal: drop ATX
/// headings, fenced code blocks, and blank-line noise; keep prose paragraphs;
/// cap at [`BODY_EXCERPT_MAX`] chars at a paragraph boundary when possible.
fn body_excerpt(body: &str) -> String {
    let mut out = String::new();
    let mut in_fence = false;
    let mut pending_blank = false;
    for line in body.lines() {
        let t = line.trim_end();
        let ts = t.trim_start();
        if ts.starts_with("```") {
            in_fence = !in_fence;
            continue;
        }
        if in_fence {
            continue;
        }
        if ts.starts_with('#') {
            continue; // headings — the title/summary already carry the framing
        }
        if ts.is_empty() {
            pending_blank = !out.is_empty();
            continue;
        }
        if pending_blank {
            out.push_str("\n\n");
            pending_blank = false;
        } else if !out.is_empty() {
            out.push(' ');
        }
        out.push_str(ts);
        if out.len() >= BODY_EXCERPT_MAX {
            break;
        }
    }
    if out.len() > BODY_EXCERPT_MAX {
        // Trim back to the last paragraph/sentence boundary under the cap.
        let cut = out[..BODY_EXCERPT_MAX]
            .rfind("\n\n")
            .or_else(|| out[..BODY_EXCERPT_MAX].rfind(". "))
            .map(|i| i + 1)
            .unwrap_or(BODY_EXCERPT_MAX);
        out.truncate(cut);
        out.push('…');
    }
    out.trim().to_string()
}
```

## Tests

`shell_moves_pages_and_adds_boot_assets` weaves the carried example and
checks the artifact shape: the root `index.html` is the shell (it mentions
the canvas and links the fallback), the woven entry now lives at
`pages/index.html` and still carries the region nav, and the entry point
moved.

The one after it exists because the shell is the only file here that is a
*template* — bytes with `__C0K_PUB_TITLE__` and `__C0K_PUB_STATS__` holes in
them, shipped verbatim in the crate and substituted at the last step of
[`apply_publication_shell`]. Nothing else in the module has a failure mode
that leaves a placeholder in a reader's browser tab, so the test asserts the
narrow thing: no `__C0K_PUB_` token survives anywhere in the emitted shell,
the entry member's title is what replaced the title hole, and a title
carrying markup is escaped on the way in rather than reopening the template
as an injection point.

```rust {#tests}
#[cfg(test)]
mod tests {
    use super::*;
    use crate::region_weave::{weave_region, RegionMember};

    fn member(uri: &str, src: &str, content: &str) -> RegionMember {
        RegionMember {
            uri: uri.to_string(),
            content: content.to_string(),
            source_path: PathBuf::from(src),
        }
    }

    fn wiki_doc(id: &str, title: &str, summary: &str, body: &str) -> String {
        format!(
            "---\nx0k:\n  format: folio/v1\n  type: wiki\n  id: {id}\n  summary: {summary}\n---\n\n# {title}\n\n{body}\n"
        )
    }

    #[test]
    fn split_frontmatter_extracts_body() {
        let c = "---\nx0k:\n  type: wiki\n---\n\n# Title\n\nBody text.\n";
        let (front, body) = split_frontmatter(c);
        assert!(front.contains("type: wiki"));
        assert!(body.contains("# Title"));
        assert!(body.contains("Body text."));
    }

    #[test]
    fn frontmatter_summary_field_parsed() {
        let front = "x0k:\n  format: folio/v1\n  summary: A short summary\n  type: wiki";
        assert_eq!(
            frontmatter_field(front, "summary").as_deref(),
            Some("A short summary")
        );
    }

    #[test]
    fn body_excerpt_strips_headings_and_fences() {
        let body = "## Section\n\nReal prose paragraph one.\n\n```rust\nlet x = 1;\n```\n\nParagraph two.\n";
        let ex = body_excerpt(body);
        assert!(ex.contains("Real prose paragraph one."), "got: {ex}");
        assert!(ex.contains("Paragraph two."), "got: {ex}");
        assert!(!ex.contains("let x"), "code fence leaked: {ex}");
        assert!(!ex.contains("Section"), "heading leaked: {ex}");
    }

    #[test]
    fn members_json_has_title_summary_body_per_member() {
        let input = RegionInput {
            entry_point_uri: "x0k:wiki/a".to_string(),
            members: vec![
                member(
                    "x0k:wiki/a",
                    "knowledge/wiki/a.md",
                    &wiki_doc("x0k:wiki/a", "Alpha", "Summary A", "Prose about alpha."),
                ),
                member(
                    "x0k:wiki/b",
                    "knowledge/wiki/b.md",
                    &wiki_doc("x0k:wiki/b", "Beta", "Summary B", "Prose about beta."),
                ),
            ],
        };
        let bytes = build_members_json(&input);
        let v: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        let a = &v["members"]["x0k:wiki/a"];
        assert_eq!(a["title"], "Alpha");
        assert_eq!(a["summary"], "Summary A");
        assert!(a["body"].as_str().unwrap().contains("Prose about alpha."));
        assert!(v["members"]["x0k:wiki/b"]["title"] == "Beta");
    }

    #[test]
    fn shell_moves_pages_and_adds_boot_assets() {
        let input = RegionInput {
            entry_point_uri: "x0k:wiki/a".to_string(),
            members: vec![
                member(
                    "x0k:wiki/a",
                    "knowledge/wiki/a.md",
                    &wiki_doc("x0k:wiki/a", "Alpha", "Summary A", "Body alpha."),
                ),
                member(
                    "x0k:wiki/b",
                    "knowledge/wiki/b.md",
                    &wiki_doc("x0k:wiki/b", "Beta", "Summary B", "Body beta."),
                ),
            ],
        };
        let mut out = weave_region(&input).unwrap();
        apply_publication_shell(&mut out, &input, None);

        let names: std::collections::BTreeSet<String> = out
            .files
            .iter()
            .map(|f| f.rel_path.to_string_lossy().to_string())
            .collect();

        // Canvas shell + boot + bundled data at the root.
        assert!(names.contains("index.html"), "shell missing: {names:?}");
        assert!(names.contains("boot.js"));
        assert!(names.contains("members.json"));
        assert!(names.contains("narrative.json"));
        assert!(names.contains("atlas.json"));
        // Woven pages moved under pages/.
        assert!(
            names.contains("pages/index.html"),
            "fallback entry: {names:?}"
        );
        assert!(names.iter().any(|n| n == "pages/wiki-b.html"));
        // The root index.html is the SHELL (canvas), not a woven doc.
        let shell = out
            .files
            .iter()
            .find(|f| f.rel_path == Path::new("index.html"))
            .unwrap();
        let shell_html = std::str::from_utf8(&shell.bytes).unwrap();
        assert!(
            shell_html.contains("vello-canvas"),
            "root index isn't the shell"
        );
        assert!(shell_html.contains("pages/index.html"), "no fallback link");
        // The fallback page is the woven document (carries the site nav).
        let fb = out
            .files
            .iter()
            .find(|f| f.rel_path == fallback_entry_path())
            .unwrap();
        let fb_html = std::str::from_utf8(&fb.bytes).unwrap();
        assert!(
            fb_html.contains("region-nav"),
            "fallback isn't the woven page"
        );
        assert_eq!(out.entry_rel_path, PathBuf::from("index.html"));
    }

    #[test]
    fn shell_template_placeholders_are_all_substituted() {
        let input = RegionInput {
            entry_point_uri: "x0k:wiki/a".to_string(),
            members: vec![member(
                "x0k:wiki/a",
                "knowledge/wiki/a.md",
                // A title with markup in it: the substitution escapes, so the
                // template cannot be reopened as an injection point.
                &wiki_doc("x0k:wiki/a", "Alpha & <Omega>", "Summary A", "Body."),
            )],
        };
        let mut out = weave_region(&input).unwrap();
        apply_publication_shell(&mut out, &input, None);

        let shell = out
            .files
            .iter()
            .find(|f| f.rel_path == Path::new(SHELL_FILE))
            .expect("the shell is emitted");
        let html = std::str::from_utf8(&shell.bytes).expect("the shell is UTF-8");
        assert!(
            !html.contains("__C0K_PUB_"),
            "an unsubstituted placeholder shipped in the shell"
        );
        assert!(
            html.contains("Alpha &amp; &lt;Omega&gt;"),
            "the entry member's title is not in the shell, escaped: {}",
            &html[..html.len().min(400)]
        );
        assert!(
            !html.contains("<Omega>"),
            "the title was substituted unescaped"
        );
        assert!(html.contains("1 member · "), "the stats hole is unfilled");
        // The template is `include_str!`d, so this also pins that the shipped
        // asset still HAS the holes: a template with none would pass the
        // negative assertions above for the wrong reason.
        assert!(SHELL_INDEX.contains("__C0K_PUB_TITLE__"));
        assert!(SHELL_INDEX.contains("__C0K_PUB_STATS__"));
    }
}
```

## The file

```rust {#root}
<<module-doc>>

<<shell-assets>>

<<apply-publication-shell>>

<<escape-html-text>>

<<motif-loader>>

<<thread-hex>>

<<publication-theme>>

<<fallback-paths>>

<<members-json>>

<<stub-narrative>>

<<split-frontmatter>>

<<frontmatter-field>>

<<first-h1>>

<<body-excerpt>>

<<tests>>
```

The fallback is not a degraded copy; it is the weave itself, the same pages
a reader without a canvas would have gotten before the shell existed. The
canvas is the addition, and everything it needs is derived from the same
members — which is the design's one-substrate claim, held at the level of
the artifact's file list.
