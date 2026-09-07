---
x0k:
  format: folio/v1
  id: x0k:implementation/folio/document-source
  type: implementation
  status: draft
  summary: Prepare a bounded document collection as typed facts using its selected vocabulary.
  tangle:
    crate: crates/x0k-folio-cli
    root: src/source.rs
  edges:
    implements:
    - x0k:design/domains-of-your-own
    cites:
    - x0k:architecture/folio-backends
    - x0k:implementation/folio/document-vocabulary
    motivated_by:
    - x0k:intent/c5ccd003-77d6-4b0d-8824-649f6221c259
---
<a name="chunk-export-source"></a><sub>[`src/lib.rs`](../../crates/x0k-folio-cli/src/lib.rs) · `#export-source`</sub>

```rust {#export-source file="src/lib.rs"}
pub mod source;
```

# Documents become a queryable collection

A Paper definition in one document gives a paper instance in another its
meaning. We first prepare the selected vocabulary, then project each Folio
document against it. Plain Markdown contributes neither definitions nor facts.
The adapter holds no database, host registry, or private x0k type.

Preparation is a bounded filesystem snapshot. Definition errors refuse that
snapshot; document errors remain attached to individual paths. An ingestion
engine can retain each invalid document's last good contribution. A changed
file refuses the prepared projection: callers prepare again before reconciling.

## The snapshot contract

The source owns sorted paths, per-path results, and a vocabulary fingerprint.
A default collection admits at most 100,000 Markdown files, 16 MiB per file,
and 512 MiB of source bytes. Symlinks are not followed. These limits bound
Markdown input; projection and parser allocation add overhead. The caller
owns loading and bounding the supplied ontology model.

<a name="chunk-source-contract"></a><sub>[`src/source.rs`](../../crates/x0k-folio-cli/src/source.rs) · `#source-contract`</sub>

```rust {#source-contract}
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::io::Read;
use anyhow::{anyhow, bail, Context, Result};
use serde_norway::Value;
use x0k_fact_projection::{FactEntry, FactValue};
use x0k_folio::colophon::{is_colophon, parse_envelope_in};
use x0k_folio::document_vocabulary::{
    self as vocabulary, DeclaredInstance, DocumentSource as VocabularySource,
};
use x0k_folio_ingest::lifecycle::{DocumentProjection, DocumentSource};
use x0k_ontology::concept_facts::{camel_to_kebab, OntologyModel, OntologyValue, RDF_TYPE};

pub const MAX_FILES: usize = 100_000;
pub const MAX_FILE_BYTES: u64 = 16 * 1024 * 1024;
pub const MAX_TOTAL_BYTES: usize = 512 * 1024 * 1024;
pub const BASE_VOCABULARY_SOURCE: &str = ".folio-base-vocabulary";

#[derive(Debug, Clone, serde::Serialize)]
pub struct SourceDiagnostic {
    pub path: PathBuf,
    pub error: Option<String>,
    pub non_folio: bool,
}

struct Prepared {
    raw_hash: String,
    projection: std::result::Result<Option<DocumentProjection>, String>,
}

pub struct FolioSource {
    root: PathBuf,
    fingerprint: String,
    prepared: BTreeMap<PathBuf, Prepared>,
    diagnostics: Vec<SourceDiagnostic>,
    base_bytes: Option<Vec<u8>>,
}
```

## Definitions precede envelope interpretation

Reading all source bytes once makes collection order irrelevant. Explicit
Turtle inside a Folio envelope contributes to the selected model even when
that envelope's type needs a class from another document. The shared format
library owns Turtle parsing and YAML declaration syntax.

<a name="chunk-prepare-source"></a><sub>[`src/source.rs`](../../crates/x0k-folio-cli/src/source.rs) · `#prepare-source`</sub>

```rust {#prepare-source}
impl FolioSource {
    pub fn prepare(root: &Path, base: OntologyModel) -> Result<Self> {
        Self::prepare_with_provenance(root, base, "selected vocabulary")
    }

    pub fn prepare_with_provenance(root: &Path, base: OntologyModel, origin: &str) -> Result<Self> {
        let root = root.canonicalize().context("open Folio collection")?;
        let base_path = root.join(BASE_VOCABULARY_SOURCE);
        if base_path.symlink_metadata().is_ok() {
            bail!("reserved vocabulary source collides with {}", base_path.display());
        }
        let mut contents = BTreeMap::new();
        let mut total = 0usize;
        for entry in walkdir::WalkDir::new(&root).follow_links(false) {
            let entry = entry.context("discover Folio collection")?;
            if !entry.file_type().is_file() || !is_markdown(entry.path()) { continue; }
            if contents.len() >= MAX_FILES { bail!("collection exceeds 100000 Markdown files"); }
            let size = entry.metadata()?.len();
            if size > MAX_FILE_BYTES { bail!("{} exceeds 16 MiB", entry.path().display()); }
            let bytes = read_markdown(entry.path())?;
            total += bytes.len();
            if total > MAX_TOTAL_BYTES { bail!("collection exceeds 512 MiB"); }
            contents.insert(entry.path().to_path_buf(), bytes);
        }
        let text: Vec<_> = contents.iter().filter_map(|(path, bytes)| {
            std::str::from_utf8(bytes).ok().filter(|body| claims_folio(body))
                .map(|body| (path.to_string_lossy().to_string(), body))
        }).collect();
        let documents: Vec<_> = text.iter().map(|(id, body)| VocabularySource { id, body }).collect();
        let vocabulary = vocabulary::load_definitions(&documents, &base)
            .context("prepare collection vocabulary")?;
        let fingerprint = vocabulary_fingerprint(&vocabulary.model);
        let mut source = Self { root, fingerprint, prepared: BTreeMap::new(), diagnostics: Vec::new(), base_bytes: None };
        let mut instances = Vec::new();
        for (path, bytes) in &contents {
            let result = prepare_document(path, bytes, &vocabulary, &mut instances);
            let projection = result.map_err(|error| format!("{error:#}"));
            source.diagnostics.push(SourceDiagnostic {
                path: path.clone(),
                non_folio: matches!(projection, Ok(None)),
                error: projection.as_ref().err().cloned(),
            });
            source.prepared.insert(path.clone(), Prepared {
                raw_hash: blake3::hash(bytes).to_hex().to_string(), projection,
            });
        }
        // Reject every owner of a duplicate identity, without choosing a winner.
        let mut owners: BTreeMap<&str, std::collections::BTreeSet<&str>> = BTreeMap::new();
        for instance in &instances {
            owners.entry(&instance.iri).or_default().insert(&instance.source.document);
        }
        let mut conflicts: BTreeMap<PathBuf, Vec<String>> = BTreeMap::new();
        for (identity, paths) in owners.iter().filter(|(_, paths)| paths.len() > 1) {
            let error = format!("duplicate instance {identity} in {}",
                paths.iter().copied().collect::<Vec<_>>().join(" and "));
            for path in paths {
                conflicts.entry(PathBuf::from(path)).or_default().push(error.clone());
            }
        }
        for diagnostic in &mut source.diagnostics {
            if let Some(errors) = conflicts.get(&diagnostic.path) {
                let error = errors.join("; ");
                diagnostic.error = Some(error.clone());
                source.prepared.get_mut(&diagnostic.path).unwrap().projection = Err(error);
            }
        }
        instances.retain(|instance| !conflicts.contains_key(Path::new(&instance.source.document)));
        // Known cross-document ranges still require a consistent collection model.
        vocabulary::validate_relationships(&vocabulary.model, &instances)?;
        if !base.facts().is_empty() {
            let (bytes, projection) = base_projection(&source.root, &base, origin)?;
            source.prepared.insert(base_path, Prepared {
                raw_hash: blake3::hash(&bytes).to_hex().to_string(),
                projection: Ok(Some(projection)),
            });
            source.base_bytes = Some(bytes);
        }
        Ok(source)
    }

    pub fn vocabulary_source_count(&self) -> usize { usize::from(self.base_bytes.is_some()) }
    pub fn is_vocabulary_source(&self, path: &Path) -> bool {
        path == self.root.join(BASE_VOCABULARY_SOURCE)
    }
    pub fn root(&self) -> &Path { &self.root }
    pub fn fingerprint(&self) -> &str { &self.fingerprint }
    pub fn diagnostics(&self) -> &[SourceDiagnostic] { &self.diagnostics }
}
```

## The selected base is a source of its own

A supplied vocabulary contributes through a reserved virtual source path,
`.folio-base-vocabulary`, beneath the corpus root. No file is written there;
an existing filesystem entry refuses preparation. The source's read hook returns
a deterministic snapshot instead. The normal ingestion engine owns its updates,
acknowledgements and deletion when the selected base becomes empty. This
virtual source participates only in collection reconciliation; individual
filesystem events accept Markdown paths, so they cannot delete a virtual
contribution merely because it has no file.

The synthetic identity is stable across model revisions. Its provenance records
the caller's selection, such as a module directory or the shipped vocabulary.
Document-carried blocks retain their own document and byte-span provenance.
An empty base creates no synthetic source and contributes no bundled ontology.

<a name="chunk-project-base"></a><sub>[`src/source.rs`](../../crates/x0k-folio-cli/src/source.rs) · `#project-base`</sub>

```rust {#project-base}
fn ontology_fact(fact: &x0k_ontology::concept_facts::OntologyFact) -> FactEntry {
    let value = match &fact.value {
        OntologyValue::Text(value) => FactValue::Text(value.clone()),
        OntologyValue::Entity(value) => FactValue::EntityRef(value.clone()),
    };
    FactEntry::new(&fact.entity, &fact.predicate, value)
}

fn base_projection(root: &Path, base: &OntologyModel, origin: &str) -> Result<(Vec<u8>, DocumentProjection)> {
    let path = root.join(BASE_VOCABULARY_SOURCE);
    let identity = blake3::hash(root.to_string_lossy().as_bytes()).to_hex().to_string();
    let uri = format!("urn:folio:base-vocabulary:{identity}");
    let ordered: std::collections::BTreeSet<_> = base.facts().iter().collect();
    let records: Vec<_> = ordered.iter().map(|fact| match &fact.value {
        OntologyValue::Text(value) => (&fact.entity, &fact.predicate, "text", value),
        OntologyValue::Entity(value) => (&fact.entity, &fact.predicate, "entity", value),
    }).collect();
    let bytes = serde_json::to_vec(&(origin, records))?;
    let mut batches: BTreeMap<String, Vec<FactEntry>> = BTreeMap::new();
    for fact in ordered {
        batches.entry(fact.entity.clone()).or_default().push(ontology_fact(fact));
    }
    for (entity, facts) in &mut batches {
        facts.push(FactEntry::new(entity, base.expand("x0k:folio/sourceDocument"), FactValue::EntityRef(uri.clone())));
    }
    batches.entry(uri.clone()).or_default().extend([
        FactEntry::new(&uri, base.expand("x0k:folio/sourceKind"), FactValue::Text("selected-vocabulary".into())),
        FactEntry::new(&uri, base.expand("x0k:folio/sourceOrigin"), FactValue::Text(origin.into())),
        FactEntry::new(&uri, base.expand("x0k:folio/sourcePath"), FactValue::Text(path.to_string_lossy().into())),
    ]);
    Ok((bytes, DocumentProjection { uri, content_hash: String::new(), batches: batches.into_iter().collect() }))
}
```

The fingerprint covers sorted vocabulary facts with length-delimited fields;
it is independent of file ordering. The projection version participates so a
changed mapping cannot silently reuse checkpoints from the old mapping.

<a name="chunk-stamp-vocabulary"></a><sub>[`src/source.rs`](../../crates/x0k-folio-cli/src/source.rs) · `#stamp-vocabulary`</sub>

```rust {#stamp-vocabulary}
fn vocabulary_fingerprint(model: &OntologyModel) -> String {
    let mut hash = blake3::Hasher::new();
    hash.update(b"folio-document-source-v2");
    let ordered: std::collections::BTreeSet<_> = model.facts().iter().collect();
    for fact in ordered {
        let (tag, value) = match &fact.value {
            OntologyValue::Text(value) => (b't', value),
            OntologyValue::Entity(value) => (b'e', value),
        };
        hash.update(&[tag]);
        for field in [&fact.entity, &fact.predicate, value] {
            hash.update(&(field.len() as u64).to_le_bytes());
            hash.update(field.as_bytes());
        }
    }
    hash.finalize().to_hex().to_string()
}

fn claims_folio(content: &str) -> bool {
    if is_colophon(content) { return true; }
    let normalized = content.replace("\r\n", "\n");
    let Some(front) = normalized.strip_prefix("---\n") else { return false; };
    let front = front.split("\n---").next().unwrap_or(front);
    front.lines().any(|line| line.trim() == "x0k:")
        && front.lines().any(|line| {
            line.trim().strip_prefix("format:").is_some_and(|value| value.contains("folio/v1"))
        })
}

fn read_markdown(path: &Path) -> Result<Vec<u8>> {
    let mut bytes = Vec::new();
    std::fs::File::open(path)?.take(MAX_FILE_BYTES + 1).read_to_end(&mut bytes)?;
    if bytes.len() as u64 > MAX_FILE_BYTES { bail!("{} exceeds 16 MiB", path.display()); }
    Ok(bytes)
}

fn is_markdown(path: &Path) -> bool {
    path.extension().is_some_and(|ext| ext.eq_ignore_ascii_case("md"))
}
```

## A projection belongs to one prepared revision

Even an unchanged document must be reconsidered after its vocabulary changes.
The engine's revision hook combines the source bytes and model fingerprint.
Ordinary edits remain local. The same checkpoint tracks vocabulary changes,
so stale facts are retracted through normal source recovery.

<a name="chunk-serve-source"></a><sub>[`src/source.rs`](../../crates/x0k-folio-cli/src/source.rs) · `#serve-source`</sub>

```rust {#serve-source}
impl DocumentSource for FolioSource {
    fn discover(&self, root: &Path) -> Result<Vec<PathBuf>> {
        if root.canonicalize()? != self.root { bail!("prepared source belongs to a different root"); }
        Ok(self.prepared.keys().cloned().collect())
    }
    fn accepts(&self, path: &Path) -> bool { is_markdown(path) }
    fn read(&self, path: &Path) -> Result<Vec<u8>> {
        if path == self.root.join(BASE_VOCABULARY_SOURCE) {
            return self.base_bytes.clone().ok_or_else(|| anyhow!("no selected base vocabulary"));
        }
        read_markdown(path)
    }
    fn revision_hash(&self, bytes: &[u8]) -> String {
        let mut hash = blake3::Hasher::new();
        hash.update(self.fingerprint.as_bytes());
        hash.update(bytes);
        hash.finalize().to_hex().to_string()
    }
    fn project(&self, path: &Path, bytes: &[u8], hash: &str) -> Result<Option<DocumentProjection>> {
        let prepared = self.prepared.get(path).ok_or_else(|| anyhow!("source was not prepared: {}", path.display()))?;
        if prepared.raw_hash != blake3::hash(bytes).to_hex().as_str() {
            bail!("source changed after preparation: {}", path.display());
        }
        let mut projection = prepared.projection.clone().map_err(|error| anyhow!(error))?;
        if let Some(projection) = &mut projection { projection.content_hash = hash.to_string(); }
        Ok(projection)
    }
}
```

## Envelopes, definitions, and instances share typed facts

Envelope metadata uses the existing Folio predicate names. Definitions keep
the original RDF predicates and values. Instance edges resolve against declared
object properties; unqualified legacy keys resolve only to declared x0k terms.
References remain entity values, never strings with an encoding prefix.
HTTP(S) and URN references preserve their absolute IRI spelling; an unknown
compact namespace still fails validation.

<a name="chunk-project-document"></a><sub>[`src/source.rs`](../../crates/x0k-folio-cli/src/source.rs) · `#project-document`</sub>

```rust {#project-document}
fn prepare_document(
    path: &Path, bytes: &[u8], vocabulary: &vocabulary::DocumentVocabulary,
    all_instances: &mut Vec<DeclaredInstance>,
) -> Result<Option<DocumentProjection>> {
    let content = std::str::from_utf8(bytes).context("Markdown is not UTF-8")?;
    if !claims_folio(content) { return Ok(None); }
    let model = &vocabulary.model;
    let (envelope, _) = parse_envelope_in(model, content)?;
    let identity = x0k_folio::entity_id::EntityId::parse_in(model, &envelope.id)?;
    let uri = model.expand(&identity.to_string());
    let path_id = path.to_string_lossy();
    let documents = [VocabularySource { id: &path_id, body: content }];
    let instances = vocabulary::collect_instances(&documents, model)?;
    let mut facts = Vec::new();
    let mut add = |key: &str, value: FactValue| {
        facts.push(FactEntry::new(&uri, model.expand(key), value));
    };
    add("x0k:folio/docType", FactValue::Text(envelope.doc_type.as_str().into()));
    add("x0k:folio/bodyFormat", FactValue::Text(envelope.body_format.clone()));
    if let Some(status) = envelope.status { add("x0k:folio/status", FactValue::Text(status.as_str().into())); }
    if let Some(summary) = envelope.summary { add("x0k:summary", FactValue::Text(summary)); }
    for concern in envelope.concerns { add("x0k:folio/concern", FactValue::Text(concern)); }
    add("x0k:folio/originalId", FactValue::Text(envelope.id.clone()));
    add("x0k:folio/sourcePath", FactValue::Text(path_id.to_string()));
    for (key, targets) in &envelope.edges {
        let predicate = vocabulary::resolve_property(model, key)
            .ok_or_else(|| anyhow!("unknown envelope object property {key}"))?;
        for target in targets {
            let target = vocabulary::resolve_reference(model, target).map_err(|error| anyhow!(error))?;
            facts.push(FactEntry::new(&uri, &predicate, FactValue::EntityRef(target)));
        }
    }
    if let Some(class) = model.classes().into_iter().find(|class| {
        class.uri.split_once(':').is_some_and(|(prefix, local)|
            model.expand(&format!("{prefix}:")) == model.expand(&format!("{}:", identity.scheme))
                && camel_to_kebab(local) == identity.class)
    }) {
        facts.push(FactEntry::new(&uri, RDF_TYPE, FactValue::EntityRef(model.expand(&class.uri))));
    }
    for block in vocabulary.definition_blocks.iter().filter(|block| block.source.document == path_id) {
        let mut entities = std::collections::BTreeSet::new();
        for fact in &block.facts {
            facts.push(ontology_fact(fact));
            entities.insert(fact.entity.clone());
        }
        for entity in entities {
            source_facts(&mut facts, &entity, &uri, block.source.bytes.clone(), model);
        }
    }
    for instance in &instances { project_instance(instance, &uri, model, &mut facts)?; }
    all_instances.extend(instances);
    let mut batches: BTreeMap<String, Vec<FactEntry>> = BTreeMap::new();
    for fact in facts { batches.entry(fact.entity.clone()).or_default().push(fact); }
    Ok(Some(DocumentProjection { uri, content_hash: String::new(), batches: batches.into_iter().collect() }))
}
```

## Scalar fields keep their type

A boolean stays a boolean and an integer stays an integer. Lists contribute
multiple values. Nested mappings use JSON record bytes; null has no asserted
value. Scalar strings stay text unless authored as graph-edge references. Scalar
fields are preserved rather than checked against a datatype schema; the
shared document vocabulary does not admit datatype-property definitions.
Unqualified field keys use their instance namespace and the existing camelCase
spelling convention; qualified keys keep the author's namespace.

<a name="chunk-project-instance"></a><sub>[`src/source.rs`](../../crates/x0k-folio-cli/src/source.rs) · `#project-instance`</sub>

```rust {#project-instance}
fn project_instance(instance: &DeclaredInstance, document: &str, model: &OntologyModel, facts: &mut Vec<FactEntry>) -> Result<()> {
    facts.push(FactEntry::new(&instance.iri, RDF_TYPE, FactValue::EntityRef(instance.concept.clone())));
    let namespace = model.expand(&format!("{}:", instance.entity.uri.scheme));
    for (key, value) in &instance.entity.yaml {
        let key = key.as_str().ok_or_else(|| anyhow!("instance field key must be a string"))?;
        if matches!(key, "id" | "edges") { continue; }
        let predicate = if key.contains(':') { model.expand(key) } else { format!("{namespace}{}", camel_key(key)) };
        for value in values(value)? { facts.push(FactEntry::new(&instance.iri, &predicate, value)); }
    }
    for (key, value) in [("title", &instance.entity.title), ("description", &instance.entity.description)] {
        if !value.is_empty() && !instance.entity.yaml.contains_key(Value::String(key.into())) {
            facts.push(FactEntry::new(&instance.iri, format!("{namespace}{key}"), FactValue::Text(value.clone())));
        }
    }
    for relation in vocabulary::validate_relationships(model, std::slice::from_ref(instance))? {
        facts.push(FactEntry::new(relation.subject, relation.predicate, FactValue::EntityRef(relation.object)));
    }
    source_facts(facts, &instance.iri, document, instance.source.bytes.clone(), model);
    Ok(())
}

fn camel_key(key: &str) -> String {
    let mut parts = key.split('_');
    let mut result = parts.next().unwrap_or_default().to_string();
    for part in parts {
        let mut chars = part.chars();
        if let Some(first) = chars.next() { result.extend(first.to_uppercase()); result.extend(chars); }
    }
    result
}

fn values(value: &Value) -> Result<Vec<FactValue>> {
    Ok(match value {
        Value::Null => vec![],
        Value::Bool(value) => vec![FactValue::Boolean(*value)],
        Value::Number(value) => vec![if let Some(n) = value.as_i64() { FactValue::SignedInt(n.into()) }
            else if let Some(n) = value.as_u64() { FactValue::UnsignedInt(n.into()) }
            else { FactValue::Float(value.as_f64().ok_or_else(|| anyhow!("unsupported YAML number"))?) }],
        Value::String(value) => vec![FactValue::Text(value.clone())],
        Value::Sequence(sequence) => sequence.iter().map(values).collect::<Result<Vec<_>>>()?.into_iter().flatten().collect(),
        Value::Mapping(_) => vec![FactValue::Record(serde_json::to_vec(value)?)],
        Value::Tagged(_) => bail!("tagged YAML field values are not supported"),
    })
}

fn source_facts(facts: &mut Vec<FactEntry>, entity: &str, document: &str, bytes: std::ops::Range<usize>, model: &OntologyModel) {
    for (key, value) in [
        ("sourceDocument", FactValue::EntityRef(document.to_string())),
        ("sourceStart", FactValue::UnsignedInt(bytes.start as u128)),
        ("sourceEnd", FactValue::UnsignedInt(bytes.end as u128)),
    ] {
        facts.push(FactEntry::new(entity, model.expand(&format!("x0k:folio/{key}")), value));
    }
}
```

## The Paper collection exercises both declaration orders

These tests cross document boundaries, preserve typed fields and references,
and distinguish ordinary Markdown from a malformed Folio document. A changed
definition invalidates the untouched instance document's revision.

<a name="chunk-test-source"></a><sub>[`src/source.rs`](../../crates/x0k-folio-cli/src/source.rs) · `#test-source`</sub>

````rust {#test-source}
#[cfg(test)]
mod tests {
    use super::*;

    fn envelope(id: &str, body: &str) -> String {
        format!("---\nx0k:\n  format: folio/v1\n  id: {id}\n  type: wiki\n---\n{body}")
    }

    fn definitions() -> &'static str {
        r#"```turtle folio:ontology
@prefix owl: <http://www.w3.org/2002/07/owl#> .
@prefix rdfs: <http://www.w3.org/2000/01/rdf-schema#> .
@prefix vann: <http://purl.org/vocab/vann/> .
@prefix p: <https://example.test/paper#> .
<https://example.test/module/paper> a owl:Ontology ;
    vann:preferredNamespacePrefix "paper" ;
    vann:preferredNamespaceUri "https://example.test/paper#" .
p:Paper a owl:Class ; rdfs:isDefinedBy <https://example.test/module/paper> .
p:cites a owl:ObjectProperty ; rdfs:domain p:Paper ; rdfs:range p:Paper ;
    rdfs:isDefinedBy <https://example.test/module/paper> .
```
"#
    }

    fn paper(name: &str, target: Option<&str>) -> String {
        let edges = target.map(|target| format!("edges:\n  paper:cites: [{target}]\n")).unwrap_or_default();
        format!("## {name}\nA paper.\n\n```yaml paper:paper\nid: paper:paper/{name}\nreviewed: true\npages: 12\nscore: 1.5\n{edges}```\n")
    }

    fn project(source: &FolioSource, path: &Path) -> DocumentProjection {
        let bytes = std::fs::read(path).unwrap();
        source.project(path, &bytes, &source.revision_hash(&bytes)).unwrap().unwrap()
    }

    fn selected_base(label: &str) -> OntologyModel {
        let text = definitions().replace("p:Paper a owl:Class",
            &format!("p:Paper rdfs:label \"{label}\" ; a owl:Class"));
        vocabulary::load_definitions(
            &[VocabularySource { id: "base.ttl", body: &text }], &OntologyModel::new([])).unwrap().model
    }

    async fn query_values(backend: &x0k_folio_dialog::DialogBackend, predicate: &str) -> Vec<FactValue> {
        let request = x0k_folio_dialog::QueryRequest {
            premises: vec![serde_json::json!({
                "assert": {"with": {"value": {"the": predicate, "cardinality": "many"}}},
                "where": {"this": {"?": {"name": "entity"}}, "value": {"?": {"name": "value"}}}
            })],
            select: vec!["value".into()], ..Default::default()
        };
        backend.query(request).await.unwrap().rows.into_iter()
            .map(|row| row["value"].clone()).collect()
    }

    #[tokio::test]
    async fn selected_base_updates_and_removal_use_normal_source_recovery() {
        // A visible Dialog commit can precede its lifecycle acknowledgement.
        // Reconcile unchanged input until the durable checkpoint catches up;
        // an absent acknowledgement or retained tombstone must still fail.
        async fn reconcile_until_acknowledged(
            source: &mut FolioSource,
            root: &Path,
            checkpoint: &Path,
            sinks: &std::sync::Mutex<Vec<x0k_folio_ingest::backend::Backend>>,
            expected: &[(PathBuf, String)],
        ) {
            let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
            loop {
                x0k_folio_ingest::lifecycle::reconcile(
                    source, root, checkpoint, sinks, None).await.unwrap();
                let state = x0k_folio_ingest::checkpoint::load_state(checkpoint).unwrap();
                let complete = state.files.len() == expected.len()
                    && state.recovery.len() == expected.len()
                    && expected.iter().all(|(path, hash)| {
                        let key = path.to_string_lossy();
                        state.files.get(key.as_ref()).is_some_and(|file|
                            file.content_hash == *hash && file.acked_by.contains("dialog"))
                            && state.recovery.get(key.as_ref()).is_some_and(|record|
                                !record.deleted && record.revisions.get("dialog").is_some_and(
                                    |revision| revision.applied_hash.as_ref() == Some(hash)))
                    });
                if complete { return; }
                assert!(std::time::Instant::now() < deadline,
                    "source recovery did not reach the exact acknowledged revisions: {state:?}");
            }
        }

        let temporary = tempfile::tempdir().unwrap();
        let root = temporary.path().join("corpus");
        std::fs::create_dir(&root).unwrap();
        let path = root.join("instance.md");
        std::fs::write(&path, envelope("x0k:wiki/instance", &paper("one", None))).unwrap();
        let backend = x0k_folio_dialog::DialogBackend::open(temporary.path().join("database")).unwrap();
        let sinks = std::sync::Mutex::new(vec![backend.backend("dialog")]);
        let checkpoint = temporary.path().join("checkpoint");
        let mut first = FolioSource::prepare_with_provenance(&root, selected_base("Old"), "directory:base.ttl").unwrap();
        assert_eq!(first.diagnostics().len(), 1);
        assert_eq!(first.vocabulary_source_count(), 1);
        let raw = std::fs::read(&path).unwrap();
        let old_revision = first.revision_hash(&raw);
        let virtual_path = root.join(BASE_VOCABULARY_SOURCE);
        let expected = vec![
            (path.clone(), old_revision.clone()),
            (virtual_path.clone(), first.revision_hash(&first.read(&virtual_path).unwrap())),
        ];
        reconcile_until_acknowledged(&mut first, &root, &checkpoint, &sinks, &expected).await;
        assert_eq!(x0k_folio_ingest::lifecycle::apply_path_change(
            &first, &virtual_path, &checkpoint, &sinks).unwrap(), 0);
        let label = "http://www.w3.org/2000/01/rdf-schema#label";
        assert_eq!(query_values(&backend, label).await, vec![FactValue::Text("Old".into())]);
        let origin = selected_base("Old").expand("x0k:folio/sourceOrigin");
        assert_eq!(query_values(&backend, &origin).await, vec![FactValue::Text("directory:base.ttl".into())]);
        let mut changed = FolioSource::prepare_with_provenance(&root, selected_base("New"), "shipped").unwrap();
        assert_ne!(changed.revision_hash(&raw), old_revision);
        let changed_revision = changed.revision_hash(&raw);
        let expected = vec![
            (path.clone(), changed_revision.clone()),
            (virtual_path.clone(), changed.revision_hash(&changed.read(&virtual_path).unwrap())),
        ];
        reconcile_until_acknowledged(&mut changed, &root, &checkpoint, &sinks, &expected).await;
        assert_eq!(query_values(&backend, label).await, vec![FactValue::Text("New".into())]);
        assert_eq!(query_values(&backend, &origin).await, vec![FactValue::Text("shipped".into())]);
        let mut removed = FolioSource::prepare(&root, OntologyModel::new([])).unwrap();
        assert_eq!(removed.vocabulary_source_count(), 0);
        assert!(removed.diagnostics()[0].error.is_some());
        reconcile_until_acknowledged(
            &mut removed, &root, &checkpoint, &sinks,
            &[(path.clone(), changed_revision)],
        ).await;
        assert!(query_values(&backend, label).await.is_empty());
        assert!(query_values(&backend, &origin).await.is_empty());
        let state = x0k_folio_ingest::checkpoint::load_state(&checkpoint).unwrap();
        assert_eq!(state.files.len(), 1, "invalid instance keeps its last good source");
        assert!(!state.files.keys().any(|key| key.ends_with(BASE_VOCABULARY_SOURCE)));
    }

    #[tokio::test]
    async fn duplicate_owners_keep_last_good_while_a_healthy_sibling_advances() {
        let temporary = tempfile::tempdir().unwrap();
        let root = temporary.path().join("corpus");
        std::fs::create_dir(&root).unwrap();
        let paths: Vec<_> = ["one", "two", "healthy"].iter().map(|name| {
            let path = root.join(format!("{name}.md"));
            std::fs::write(&path, envelope(&format!("x0k:wiki/{name}"), &paper(name, None))).unwrap();
            path
        }).collect();
        let backend = x0k_folio_dialog::DialogBackend::open(temporary.path().join("database")).unwrap();
        let sinks = std::sync::Mutex::new(vec![backend.backend("dialog")]);
        let checkpoint = temporary.path().join("checkpoint");
        let mut first = FolioSource::prepare(&root, selected_base("Paper")).unwrap();
        x0k_folio_ingest::lifecycle::reconcile(&mut first, &root, &checkpoint, &sinks, None).await.unwrap();
        let prior = x0k_folio_ingest::checkpoint::load_state(&checkpoint).unwrap();
        for (index, name) in ["one", "two", "healthy"].iter().enumerate() {
            let body = paper(if index < 2 { "shared" } else { name }, None)
                .replace("pages: 12", if index < 2 { "pages: 99" } else { "pages: 13" });
            std::fs::write(&paths[index], envelope(&format!("x0k:wiki/{name}"), &body)).unwrap();
        }
        let mut changed = FolioSource::prepare(&root, selected_base("Paper")).unwrap();
        assert_eq!(changed.diagnostics().iter().filter(|d| d.error.is_some()).count(), 2);
        for path in &paths[..2] {
            let diagnostic = changed.diagnostics().iter().find(|d| &d.path == path).unwrap();
            let error = diagnostic.error.as_deref().unwrap();
            assert!(error.contains("duplicate instance https://example.test/paper#paper/shared"));
            assert!(error.contains("one.md") && error.contains("two.md"));
            let bytes = std::fs::read(path).unwrap();
            assert!(changed.project(path, &bytes, &changed.revision_hash(&bytes)).is_err());
        }
        x0k_folio_ingest::lifecycle::reconcile(&mut changed, &root, &checkpoint, &sinks, None).await.unwrap();
        let after = x0k_folio_ingest::checkpoint::load_state(&checkpoint).unwrap();
        for path in &paths[..2] {
            let key = path.to_string_lossy().to_string();
            assert_eq!(prior.files[&key].content_hash, after.files[&key].content_hash);
        }
        let pages = query_values(&backend, "https://example.test/paper#pages").await;
        assert!(pages.contains(&FactValue::SignedInt(12)));
        assert!(pages.contains(&FactValue::SignedInt(13)));
        assert!(!pages.contains(&FactValue::SignedInt(99)));
        assert_ne!(prior.files[&paths[2].to_string_lossy().to_string()].content_hash,
            after.files[&paths[2].to_string_lossy().to_string()].content_hash);
    }

    #[test]
    fn virtual_source_collision_is_refused_and_snapshot_bytes_are_canonical() {
        let root = tempfile::tempdir().unwrap();
        let model = selected_base("Paper");
        let reordered = OntologyModel::new(model.facts().iter().rev().cloned());
        let first = FolioSource::prepare_with_provenance(root.path(), model, "shipped").unwrap();
        let second = FolioSource::prepare_with_provenance(root.path(), reordered, "shipped").unwrap();
        let path = root.path().join(BASE_VOCABULARY_SOURCE);
        assert_eq!(first.read(&path).unwrap(), second.read(&path).unwrap());
        assert_eq!(first.fingerprint(), second.fingerprint());
        assert!(!path.exists());
        std::fs::write(&path, "operator file").unwrap();
        let error = FolioSource::prepare(root.path(), OntologyModel::new([])).err().unwrap();
        assert!(error.to_string().contains("reserved vocabulary source collides"));
        assert_eq!(std::fs::read_to_string(path).unwrap(), "operator file");
    }

    #[test]
    fn public_paper_corpus_runs_without_a_base_vocabulary() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("examples/papers");
        let source = FolioSource::prepare(&root, OntologyModel::new([])).unwrap();
        assert_eq!(source.diagnostics().len(), 3);
        assert_eq!(source.vocabulary_source_count(), 0);
        let vocabulary = project(&source, &root.join("vocabulary.md"));
        assert!(vocabulary.batches.iter().flat_map(|(_, facts)| facts).any(|fact|
            fact.predicate == "http://purl.org/vocab/vann/preferredNamespaceUri"
                && fact.value == FactValue::Text("https://example.org/papers#".into())));
        assert!(source.diagnostics().iter().all(|d| d.error.is_none() && !d.non_folio));
        let alpha = project(&source, &root.join("alpha.md"));
        assert!(alpha.batches.iter().flat_map(|(_, facts)| facts).any(|fact|
            fact.predicate == "https://example.org/papers#cites"
                && fact.value == FactValue::EntityRef("https://example.org/papers#paper/beta".into())));
    }

    #[test]
    fn paper_collection_preserves_definitions_references_types_and_source_links() {
        let root = tempfile::tempdir().unwrap();
        let alpha = root.path().join("a.md");
        let beta = root.path().join("b.md");
        std::fs::write(&alpha, envelope("x0k:wiki/a", &paper("alpha", Some("paper:paper/beta")))).unwrap();
        std::fs::write(&beta, envelope("x0k:wiki/b", &format!("{}{}", definitions(), paper("beta", None)))).unwrap();
        let source = FolioSource::prepare(root.path(), OntologyModel::new([])).unwrap();
        assert!(source.diagnostics().iter().all(|d| d.error.is_none() && !d.non_folio));
        let projection = project(&source, &alpha);
        let facts: Vec<_> = projection.batches.iter().flat_map(|(_, facts)| facts).collect();
        assert!(facts.iter().any(|fact| fact.predicate == "https://example.test/paper#cites"
            && fact.value == FactValue::EntityRef("https://example.test/paper#paper/beta".into())));
        for value in [FactValue::Boolean(true), FactValue::SignedInt(12), FactValue::Float(1.5)] {
            assert!(facts.iter().any(|fact| fact.value == value));
        }
        assert!(facts.iter().any(|fact| fact.predicate.ends_with("folio/sourceDocument")
            && fact.value == FactValue::EntityRef(projection.uri.clone())));
        assert!(project(&source, &beta).batches.iter().any(|(entity, _)| entity == "https://example.test/paper#Paper"));
    }

    #[test]
    fn external_envelope_citations_remain_entity_values() {
        let root = tempfile::tempdir().unwrap();
        let path = root.path().join("citation.md");
        std::fs::write(&path, "---\nx0k:\n  format: folio/v1\n  id: x0k:wiki/citation\n  type: wiki\n  edges:\n    cites: [https://example.org/paper]\n---\n# Citation\n").unwrap();
        let source = FolioSource::prepare(root.path(), OntologyModel::shipped()).unwrap();
        let projection = project(&source, &path);
        assert!(projection.batches.iter().flat_map(|(_, facts)| facts).any(|fact|
            fact.predicate.ends_with("#cites")
                && fact.value == FactValue::EntityRef("https://example.org/paper".into())));
    }

    #[test]
    fn prose_does_not_define_and_invalid_documents_remain_visible() {
        let root = tempfile::tempdir().unwrap();
        std::fs::write(root.path().join("README.md"), definitions()).unwrap();
        let invalid = root.path().join("invalid.md");
        std::fs::write(&invalid, "---\nx0k:\n  format: folio/v1\n---\n").unwrap();
        let unclosed = root.path().join("unclosed.md");
        std::fs::write(&unclosed, "---\nx0k:\n  format: folio/v1\n").unwrap();
        let unknown = root.path().join("unknown.md");
        std::fs::write(&unknown, envelope("x0k:wiki/unknown", &paper("one", None))).unwrap();
        let source = FolioSource::prepare(root.path(), OntologyModel::new([])).unwrap();
        assert_eq!(source.diagnostics().iter().filter(|d| d.non_folio).count(), 1);
        assert_eq!(source.diagnostics().iter().filter(|d| d.error.is_some()).count(), 3);
        assert!(source.project(&invalid, &std::fs::read(&invalid).unwrap(), "revision").is_err());
    }

    #[test]
    fn model_changes_revise_unchanged_instances_and_stale_snapshots_refuse_edits() {
        let root = tempfile::tempdir().unwrap();
        let vocabulary = root.path().join("vocabulary.md");
        let instance = root.path().join("instance.md");
        std::fs::write(&vocabulary, envelope("x0k:wiki/vocabulary", definitions())).unwrap();
        std::fs::write(&instance, envelope("x0k:wiki/instance", &paper("one", None))).unwrap();
        let first = FolioSource::prepare(root.path(), OntologyModel::new([])).unwrap();
        let bytes = std::fs::read(&instance).unwrap();
        let changed = definitions().replace("p:Paper a owl:Class", "p:Paper rdfs:label \"Paper\" ; a owl:Class");
        std::fs::write(&vocabulary, envelope("x0k:wiki/vocabulary", &changed)).unwrap();
        let second = FolioSource::prepare(root.path(), OntologyModel::new([])).unwrap();
        assert_ne!(first.revision_hash(&bytes), second.revision_hash(&bytes));
        assert!(first.project(&instance, b"changed", "new").is_err());
        std::fs::remove_file(vocabulary).unwrap();
        let removed = FolioSource::prepare(root.path(), OntologyModel::new([])).unwrap();
        assert!(removed.diagnostics().iter().any(|d| d.error.is_some()));
    }

    #[test]
    fn base_vocabulary_resolves_legacy_edges_without_becoming_required() {
        let model = OntologyModel::shipped();
        let resolved = vocabulary::resolve_property(&model, "motivated_by").unwrap();
        assert_eq!(resolved, model.expand("x0k:motivatedBy"));
        assert_eq!(vocabulary::resolve_property(&model, "motivatedBy"), Some(resolved));
        assert!(vocabulary::resolve_property(&OntologyModel::new([]), "motivated_by").is_none());
    }

    /// Run explicitly against operator-selected files; never embed private corpus fixtures.
    #[test]
    #[ignore]
    fn selected_real_corpus() {
        let root = std::env::var_os("FOLIO_SOURCE_CORPUS").expect("FOLIO_SOURCE_CORPUS");
        let source = FolioSource::prepare(Path::new(&root), OntologyModel::shipped()).unwrap();
        if let Some(diagnostic) = source.diagnostics().iter().find(|d| d.error.is_some()) {
            panic!("{}: {}", diagnostic.path.display(), diagnostic.error.as_ref().unwrap());
        }
        assert!(source.diagnostics().iter().any(|d| !d.non_folio));
    }
}
````

The prepared snapshot is deliberately conservative. Conflicting definitions and
known cross-document range mismatches stop collection preparation. Duplicate
instance identities reject every conflicting document, naming all owners; no
owner wins by traversal order. Healthy documents can still advance. Every rejected
document remains visible in diagnostics and keeps its last good database contribution.
