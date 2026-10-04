# The collection a publication is drawn from

```turtle folio:document
implementation:tangle\/collection a x0k:Implementation ;
    x0k:status "draft" ;
    x0k:summary "What every verb that reads a publication shares: where a collection keeps each kind of document, the vocabulary its headers are read in, and finding a document by the id its own header declares, wherever it is filed." ;
    x0k:concerns "tangle", "publication", "collection", "vocabulary" ;
    x0k:cites architecture:monorepo-layout,
        implementation:folio\/colophon ;
    x0k:implements design:publish-a-region-as-a-repository ;
    folio:tangleCrate "crates/x0k-tangle" ;
    folio:tangleRoot "src/collection.rs" .
```

A publication names documents by id and lives in a collection: a folder of
Markdown, laid out however its authors lay it out, whose headers are written
in a vocabulary. Every verb that reads one — projecting it as a repository,
publishing that repository, receiving a contribution back from it — needs the
same three answers before it does anything of its own: where each kind of
document is likely to be, which vocabulary to read a header in, and which
file declares a given id. This chapter is those three answers, in one module
the verbs share, so two verbs cannot disagree about what a collection holds.

Nothing here assumes the collection is ours. The layout is read from a file
the collection may carry and falls back to built-in guesses that cost nothing
to try; the vocabulary is the shipped one plus whatever module directories the
caller names; and a document the guesses do not place is found by the id it
declares, anywhere in the collection.

<a name="chunk-module-doc"></a><sub>[`src/collection.rs`](../../crates/x0k-tangle/src/collection.rs) · `#module-doc`</sub>

```rust {#module-doc}
//! The collection a publication is drawn from: where each kind of document
//! lives ([`CorpusLayout`]), the vocabulary a header is read in
//! ([`Vocabulary`]), and finding a document by the id its header declares
//! ([`documents_declaring`]). Shared by every verb that reads a publication.
```

<a name="chunk-uses"></a><sub>[`src/collection.rs`](../../crates/x0k-tangle/src/collection.rs) · `#uses`</sub>

```rust {#uses}
use anyhow::{anyhow, Result};
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use x0k_folio::colophon::{
    parse_envelope, parse_envelope_in, predeclared_prefixes, shipped_prefixes, Colophon, FolioError,
    HEADER_MARKER,
};
use x0k_folio::{EntityId, EntityIdError};
use x0k_ontology::concept_facts::OntologyModel;
```

## The corpus layout

A verb that reads a publication walks four kinds of place: the directory a
class's decisions sit in, the concept pages, the manuscripts, and — for the
repository projector — the literate set's root. Written as
literals, those four facts are structure hiding in strings: the code that
joins `x0k:design/foo` onto `decisions/design/` is *deciding* the layout
every time it runs, and a corpus organized any other way has no way to say
so. `x0k:architecture/monorepo-layout` §5 answers this generally — every
kind of identity resolves through exactly one table, and a move is an edit
to that table — and for a folio document that table already exists: the
class registry, `config/projection-classes.toml`, whose `path_template`
rows are what the daemon projects a document to.

So `CorpusLayout` is the registry read as a set of roots. Nothing is
duplicated: a class's directory *is* its `path_template` with the
`{slug}.md` leaf removed, which is why `design` needs no row of its own
here. What the registry cannot say is where the two document kinds it does
not project live — a literate chapter, which is file-canonical and found by
the id its own header declares rather than by joining an id onto a
directory, and a manuscript — plus the umbrella the class directories hang
from, which a named document's search needs when its class has no row at
all. Those three are a `[corpus]` table beside the class rows, in the same
file, because a second file would be a second table for one identity kind
and that is exactly what §5 forbids.

<a name="chunk-corpus-layout"></a><sub>[`src/collection.rs`](../../crates/x0k-tangle/src/collection.rs) · `#corpus-layout`</sub>

```rust {#corpus-layout}
/// Where each kind of corpus document lives. Read from the class registry
/// (`x0k:architecture/monorepo-layout` §5: one resolver table per identity
/// kind); the built-in values are the corpus as it stands, so a caller with
/// no registry to read resolves exactly as before.
#[derive(Debug, Clone)]
pub struct CorpusLayout {
    implementation: PathBuf,
    decisions: PathBuf,
    manuscripts: PathBuf,
    /// Class name → its directory, from that class's `path_template`.
    class_dirs: BTreeMap<String, PathBuf>,
    /// Whether a registry declared these roots — a `[corpus]` table was
    /// read — rather than the built-in values standing in for one.
    declared: bool,
}

/// The registry, relative to the workspace root. `config/` is a build-and-run
/// root and does not move with the corpus, so this one path stays a literal.
const CLASS_REGISTRY: &str = "config/projection-classes.toml";
```

The built-in values carry the corpus as it stands today, and they are what a
caller gets when the registry is unreadable. That is a deliberate fallback
rather than a refusal: the projector is also run against a *materialized
corpus* — a `git archive` of an older revision, which may predate any row it
would look for — and against test fixtures that are three files in a temp
directory. Refusing there would trade a working projection for a diagnostic
nobody wanted.

<a name="chunk-corpus-layout-current"></a><sub>[`src/collection.rs`](../../crates/x0k-tangle/src/collection.rs) · `#corpus-layout-current`</sub>

```rust {#corpus-layout-current}
impl Default for CorpusLayout {
    fn default() -> CorpusLayout {
        CorpusLayout {
            implementation: PathBuf::from("knowledge/implementation"),
            decisions: PathBuf::from("decisions"),
            manuscripts: PathBuf::from("manuscripts"),
            class_dirs: [
                ("wiki", "knowledge/wiki"),
                ("design", "decisions/design"),
                ("architecture", "decisions/architecture"),
                ("commitment", "decisions/commitments"),
                ("publication", "decisions/publications"),
            ]
            .into_iter()
            .map(|(c, d)| (c.to_string(), PathBuf::from(d)))
            .collect(),
            declared: false,
        }
    }
}
```

Reading is total: every step that could fail leaves the built-in value in
place. A `path_template` whose slug is not in the file name (a class whose
documents are directories, should one ever exist) and a `path_template_fixed`
singleton both yield no directory and are skipped rather than guessed at.

<a name="chunk-corpus-layout-read"></a><sub>[`src/collection.rs`](../../crates/x0k-tangle/src/collection.rs) · `#corpus-layout-read`</sub>

```rust {#corpus-layout-read}
impl CorpusLayout {
    /// The layout `workspace` declares, falling back per row to the built-in
    /// corpus. Cheap enough to call once per projection; never per member.
    pub fn read(workspace: &Path) -> CorpusLayout {
        let mut layout = CorpusLayout::default();
        let Ok(text) = std::fs::read_to_string(workspace.join(CLASS_REGISTRY)) else {
            return layout;
        };
        let Ok(doc) = text.parse::<toml_edit::DocumentMut>() else {
            return layout;
        };
        if let Some(corpus) = doc.get("corpus").and_then(|i| i.as_table_like()) {
            let root = |k: &str| corpus.get(k).and_then(|i| i.as_str()).map(PathBuf::from);
            layout.implementation = root("implementation").unwrap_or(layout.implementation);
            layout.decisions = root("decisions").unwrap_or(layout.decisions);
            layout.manuscripts = root("manuscripts").unwrap_or(layout.manuscripts);
            layout.declared = true;
        }
        if let Some(classes) = doc.get("classes").and_then(|i| i.as_table_like()) {
            for (name, entry) in classes.iter() {
                let dir = entry
                    .as_table_like()
                    .and_then(|t| t.get("path_template"))
                    .and_then(|i| i.as_str())
                    .and_then(template_dir);
                if let Some(dir) = dir {
                    layout.class_dirs.insert(name.to_string(), dir);
                }
            }
        }
        layout
    }
}

/// The directory a `path_template` resolves into: everything before the
/// `{slug}` leaf. `None` when the template has no such leaf.
fn template_dir(template: &str) -> Option<PathBuf> {
    let (dir, leaf) = template.rsplit_once('/')?;
    leaf.contains("{slug}").then(|| PathBuf::from(dir))
}
```

The five accessors are the whole surface. `class_dir` is the old table's
job done by lookup; `decision_search_dirs` is the wider net a *named*
document gets, where the publication wrote a class the registry may not
know and the plural of a class name is as good a guess as the singular.
Ordering matters and duplicates do not: the registry's own answer is tried
first, and a class whose registry row already is `decisions/<class>s` must
not have that directory walked twice, or one document would answer as two.

The fifth says whether the corpus is one a registry *declares*. The
built-in values are a guess that costs nothing to try — a member found
there is checked against its own header like any other — but they are not
a statement about the collection, and a step that would *rename* a
corpus's own directories (the repository projector's organized layout,
which moves our decisions root to `decisions/`) must not take a guess for
one. `declared_corpus_root` is the corpus the `[corpus]` table names, the
directory its implementation root hangs from, and nothing when no table
was read: an outside collection's paths are then taken as they are.

<a name="chunk-corpus-layout-accessors"></a><sub>[`src/collection.rs`](../../crates/x0k-tangle/src/collection.rs) · `#corpus-layout-accessors`</sub>

```rust {#corpus-layout-accessors}
impl CorpusLayout {
    /// Root of the literate set — the directory a publication's chapters are
    /// discovered under.
    pub fn implementation_root(&self) -> &Path {
        &self.implementation
    }

    /// The corpus a registry's `[corpus]` table declares — the directory the
    /// implementation root hangs from — or `None` when no registry declared
    /// one and the built-in values are standing in.
    pub fn declared_corpus_root(&self) -> Option<&Path> {
        if !self.declared {
            return None;
        }
        self.implementation.parent()
    }

    /// Root the class directories hang from, for messages and for the
    /// fallbacks below.
    pub fn decisions_root(&self) -> &Path {
        &self.decisions
    }

    /// The directory documents of `class` live in.
    pub fn class_dir(&self, class: &str) -> PathBuf {
        if class == "manuscript" {
            return self.manuscripts.clone();
        }
        match self.class_dirs.get(class) {
            Some(dir) => dir.clone(),
            None => self.decisions.join(class),
        }
    }

    /// Every directory a named document of `class` may be under, in the order
    /// to try them: what the registry says, then the class name and its
    /// plural under the decisions root, deduplicated.
    pub fn decision_search_dirs(&self, class: &str) -> Vec<PathBuf> {
        let mut dirs: Vec<PathBuf> = Vec::new();
        for dir in [
            self.class_dir(class),
            self.decisions.join(class),
            self.decisions.join(format!("{class}s")),
        ] {
            if !dirs.contains(&dir) {
                dirs.push(dir);
            }
        }
        dirs
    }
}
```

## The vocabulary a publication is read in

A publication is a folio document, and a folio document is written in the
terms of a vocabulary. The publishing verbs used to read every header against
one vocabulary only — the modules this build compiled — so a collection
whose documents are typed in its own module (`acme:report\/q3 a
acme:Report`) could not be published at all: its members' headers do not
parse without the module that declares `acme:`, and a publication naming
one does not parse either. `check` already answers this for reading a
collection, with `--vocabulary <dir>`; the publishing verbs take the same flag,
repeatable, and read the publication and its members against the compiled
modules plus every directory named.

`Vocabulary` is that reading, built once per run and handed to every step
that parses a header. Naming no directory is not "the shipped model, read
the new way": it is the call every corpus verb made before it took a
vocabulary, `parse_envelope`, so a corpus that names nothing projects byte
for byte as it did. Only a named directory switches to
`parse_envelope_in`, which predeclares the named modules' prefixes and
admits a class they declare as a genus. A member id is licensed by every
namespace a loaded module declares — the compiled set's included, since
those are loaded modules too.

<a name="chunk-vocabulary"></a><sub>[`src/collection.rs`](../../crates/x0k-tangle/src/collection.rs) · `#vocabulary`</sub>

```rust {#vocabulary}
/// The vocabulary a publishing verb reads a publication and its members against:
/// the modules this build compiled, plus every module directory the caller
/// names (`--vocabulary`, repeatable; each is loaded as `check` loads one).
/// Naming none is the shipped reading exactly — the same parser call and the
/// same prefixes every publishing verb used before it took a vocabulary.
#[derive(Debug, Clone)]
pub struct Vocabulary {
    model: OntologyModel,
    /// Whether any directory was named. Only then is a header read *in* the
    /// model, a class it declares admitted as a genus.
    extended: bool,
    /// The directories named, in the order given — where a projection
    /// finds the collection's own modules to ship.
    dirs: Vec<PathBuf>,
}

impl Default for Vocabulary {
    fn default() -> Vocabulary {
        Vocabulary::shipped()
    }
}

impl Vocabulary {
    /// The modules this build compiled, and nothing else.
    pub fn shipped() -> Vocabulary {
        Vocabulary { model: OntologyModel::shipped(), extended: false, dirs: Vec::new() }
    }

    /// The compiled modules plus every module directory in `dirs`. No
    /// directory is [`Vocabulary::shipped`].
    pub fn load(dirs: &[PathBuf]) -> Result<Vocabulary> {
        if dirs.is_empty() {
            return Ok(Vocabulary::shipped());
        }
        let mut facts = OntologyModel::shipped().facts().to_vec();
        for dir in dirs {
            let named = OntologyModel::load(dir)
                .map_err(|e| anyhow!("loading a vocabulary from {}: {e}", dir.display()))?;
            facts.extend_from_slice(named.facts());
        }
        Ok(Vocabulary { model: OntologyModel::new(facts), extended: true, dirs: dirs.to_vec() })
    }

    /// The module directories named, in the order given; empty for
    /// [`Vocabulary::shipped`].
    pub fn dirs(&self) -> &[PathBuf] {
        &self.dirs
    }

    /// Read a document's header in this vocabulary.
    pub fn read(&self, content: &str) -> Result<(Colophon, String), FolioError> {
        if self.extended {
            parse_envelope_in(&self.model, content)
        } else {
            parse_envelope(content)
        }
    }

    /// The prefixes a header read in this vocabulary may use undeclared.
    pub fn prefixes(&self) -> Vec<(String, String)> {
        if self.extended {
            predeclared_prefixes(&self.model)
        } else {
            shipped_prefixes().to_vec()
        }
    }

    /// Parse a member id, licensed by every namespace a loaded module
    /// declares.
    pub fn parse_id(&self, id: &str) -> Result<EntityId, EntityIdError> {
        EntityId::parse_in(&self.model, id)
    }
}
```

## Finding a member by its id

A member names a document by id, and the id is the identity: a file moves
between directories, is renamed, sits in a layout this projector was never
told about, and its header still says what it is. So a member is found
the way a literate chapter is found — by the id its own header declares.
The layout's directories are tried first, because they are where our own
corpus keeps things and a member found there is found without walking
anything; but they are a first guess, never the answer that refuses. A
member the layout does not place is looked for across the whole
collection, and only a member *nothing in the collection declares* is
missing.

The walk is the collection, not the disk: a directory a projection was
written into (it holds a `PROVENANCE.json`) carries copies of the
collection's documents under the same ids and is not part of it, and
build state (`target/`, `node_modules/`) and hidden directories (`.git`,
`.jj`, an editor's) are not documents anyone wrote. One walk answers every
id asked about, in path order, so two documents declaring one id are both
reported, in the same order every time. The walk itself is shared: the
repository projector finds a collection's chapters the same way, by the
id each declares wherever it is filed, and two walks that disagreed about
what the collection is would disagree about what it holds.

<a name="chunk-documents-declaring"></a><sub>[`src/collection.rs`](../../crates/x0k-tangle/src/collection.rs) · `#documents-declaring`</sub>

```rust {#documents-declaring}
/// Every Markdown file in the collection under `workspace`, in path order:
/// not under a directory holding `PROVENANCE.json` (a projection), build
/// state (`target/`, `node_modules/`), or a hidden directory.
pub fn collection_documents(workspace: &Path) -> Vec<PathBuf> {
    walkdir::WalkDir::new(workspace)
        .sort_by_file_name()
        .into_iter()
        .filter_entry(|entry| {
            if entry.depth() == 0 || !entry.file_type().is_dir() {
                return true;
            }
            let name = entry.file_name().to_string_lossy();
            !(name.starts_with('.')
                || name == "target"
                || name == "node_modules"
                || entry.path().join("PROVENANCE.json").is_file())
        })
        .filter_map(|e| e.ok())
        .filter(|entry| {
            entry.file_type().is_file() && entry.path().extension().is_some_and(|e| e == "md")
        })
        .map(|entry| entry.into_path())
        .collect()
}

/// Every document in the collection under `workspace` whose header declares
/// one of `ids`, by id, each id's paths in path order. One walk answers
/// every id asked about. A directory holding `PROVENANCE.json` (a
/// projection), build state, and hidden directories are not walked.
pub fn documents_declaring(
    workspace: &Path,
    vocabulary: &Vocabulary,
    ids: &BTreeSet<String>,
) -> BTreeMap<String, Vec<PathBuf>> {
    let mut found: BTreeMap<String, Vec<PathBuf>> = BTreeMap::new();
    if ids.is_empty() {
        return found;
    }
    for path in collection_documents(workspace) {
        let Ok(text) = std::fs::read_to_string(&path) else {
            continue;
        };
        if !text.contains(HEADER_MARKER) {
            continue;
        }
        let Ok((env, _)) = vocabulary.read(&text) else {
            continue;
        };
        if ids.contains(&env.id) {
            found.entry(env.id).or_default().push(path);
        }
    }
    found
}
```

## Tests

The layout's tests pin the built-in guesses, a registry moving every root,
and the search that never walks one directory twice. The id walk is pinned
directly: a projection's copy of a document is not the collection, and two
documents declaring one id are both reported.

<a name="chunk-tests"></a><sub>[`src/collection.rs`](../../crates/x0k-tangle/src/collection.rs) · `#tests`</sub>

```rust {#tests}
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_built_in_layout_is_the_corpus_as_it_stands() {
        // Wiki members live under knowledge/wiki, not decisions/ — the layout
        // invariant the module doc states.
        let l = CorpusLayout::default();
        assert_eq!(l.class_dir("wiki"), Path::new("knowledge").join("wiki"));
        assert_eq!(l.class_dir("design"), Path::new("decisions").join("design"));
        assert_eq!(
            l.class_dir("architecture"),
            Path::new("decisions").join("architecture")
        );
        assert_eq!(l.class_dir("manuscript"), Path::new("manuscripts"));
        // A class the registry does not know still resolves, by its own name.
        assert_eq!(l.class_dir("gizmo"), Path::new("decisions").join("gizmo"));
        assert_eq!(l.implementation_root(), Path::new("knowledge/implementation"));
    }

    #[test]
    fn a_registry_moves_every_root_and_a_missing_one_moves_none() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(dir.path().join("config")).unwrap();
        std::fs::write(
            dir.path().join(CLASS_REGISTRY),
            "[corpus]\n\
             implementation = \"corpora/x0k/implementation\"\n\
             decisions = \"corpora/x0k/decisions\"\n\
             manuscripts = \"corpora/x0k/manuscripts\"\n\
             [classes.wiki]\n\
             path_template = \"corpora/x0k/wiki/{slug}.md\"\n\
             [classes.design]\n\
             path_template = \"corpora/x0k/decisions/design/{slug}.md\"\n\
             [classes.registry]\n\
             path_template_fixed = \".0k/workstreams.toml\"\n",
        )
        .unwrap();
        let moved = CorpusLayout::read(dir.path());
        assert_eq!(moved.implementation_root(), Path::new("corpora/x0k/implementation"));
        assert_eq!(moved.class_dir("wiki"), Path::new("corpora/x0k/wiki"));
        assert_eq!(moved.class_dir("design"), Path::new("corpora/x0k/decisions/design"));
        // No `path_template`, so no directory is guessed; the class falls back
        // to the decisions root, which the `[corpus]` table moved.
        assert_eq!(moved.class_dir("registry"), Path::new("corpora/x0k/decisions/registry"));
        // A class the moved registry never mentions follows the moved root.
        assert_eq!(moved.class_dir("gizmo"), Path::new("corpora/x0k/decisions/gizmo"));

        let bare = tempfile::tempdir().unwrap();
        assert_eq!(
            CorpusLayout::read(bare.path()).class_dir("design"),
            Path::new("decisions/design")
        );
    }

    #[test]
    fn a_named_documents_search_never_walks_one_directory_twice() {
        // `commitment` resolves to `decisions/commitments` through the
        // registry, which is also what the plural fallback produces: two hits
        // for one file would read as an ambiguous id.
        let dirs = CorpusLayout::default().decision_search_dirs("commitment");
        assert_eq!(
            dirs,
            vec![
                PathBuf::from("decisions/commitments"),
                PathBuf::from("decisions/commitment"),
            ]
        );
        assert_eq!(
            CorpusLayout::default().decision_search_dirs("design"),
            vec![PathBuf::from("decisions/design"), PathBuf::from("decisions/designs")]
        );
    }

    #[test]
    fn only_a_registry_declares_a_corpus_root() {
        // The built-in values are a guess, not a declaration: nothing a
        // projection renames hangs off them.
        assert_eq!(CorpusLayout::default().declared_corpus_root(), None);
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(dir.path().join("config")).unwrap();
        std::fs::write(
            dir.path().join(CLASS_REGISTRY),
            "[corpus]\nimplementation = \"corpora/x0k/implementation\"\n",
        )
        .unwrap();
        assert_eq!(
            CorpusLayout::read(dir.path()).declared_corpus_root(),
            Some(Path::new("corpora/x0k"))
        );
    }

    /// A design declaring `id`, with nothing else in it.
    fn design(id: &str) -> String {
        format!("# A design\n\n```turtle folio:document\n{id} a x0k:Design ;\n    x0k:status \"proposed\" .\n```\n\nHere.\n")
    }

    #[test]
    fn an_id_is_found_wherever_it_is_filed_and_never_in_a_projection() {
        let ws = tempfile::tempdir().unwrap();
        let write = |rel: &str, text: &str| {
            let path = ws.path().join(rel);
            std::fs::create_dir_all(path.parent().unwrap()).unwrap();
            std::fs::write(path, text).unwrap();
        };
        write("notes/kept/elsewhere.md", &design("design:present"));
        write("out/notes/kept/elsewhere.md", &design("design:present"));
        write("out/PROVENANCE.json", "{}\n");
        write("target/copy.md", &design("design:present"));
        write(".hidden/copy.md", &design("design:present"));
        let ids: BTreeSet<String> = ["x0k:design/present".to_string()].into();
        let found = documents_declaring(ws.path(), &Vocabulary::shipped(), &ids);
        assert_eq!(
            found.get("x0k:design/present").map(Vec::as_slice),
            Some([ws.path().join("notes/kept/elsewhere.md")].as_slice()),
            "{found:?}"
        );

        write("notes/other.md", &design("design:present"));
        let found = documents_declaring(ws.path(), &Vocabulary::shipped(), &ids);
        assert_eq!(found["x0k:design/present"].len(), 2, "both are reported: {found:?}");
    }
}
```

## Composing the module

<a name="chunk-root"></a><sub>[`src/collection.rs`](../../crates/x0k-tangle/src/collection.rs) · `#root` · assembles [module-doc](#chunk-module-doc) · [uses](#chunk-uses) · [corpus-layout](#chunk-corpus-layout) · [corpus-layout-current](#chunk-corpus-layout-current) · [corpus-layout-read](#chunk-corpus-layout-read) · [corpus-layout-accessors](#chunk-corpus-layout-accessors) · [vocabulary](#chunk-vocabulary) · [documents-declaring](#chunk-documents-declaring) · [tests](#chunk-tests)</sub>

```rust {#root}
<<module-doc>>

<<uses>>

<<corpus-layout>>

<<corpus-layout-current>>

<<corpus-layout-read>>

<<corpus-layout-accessors>>

<<vocabulary>>

<<documents-declaring>>

<<tests>>
```
