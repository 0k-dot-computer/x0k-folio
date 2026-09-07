---
x0k:
  format: folio/v1
  id: x0k:implementation/tangle/instance-rendering
  type: implementation
  status: draft
  summary: Instance identity, fields and source declarations rendered with the selected vocabulary on HTML and forge surfaces.
  tangle:
    crate: crates/x0k-tangle
    root: src/instance_rendering.rs
  edges:
    cites:
    - x0k:implementation/folio/document-vocabulary
    - x0k:implementation/tangle/weave
    - x0k:implementation/tangle/region-gfm
---
# Instances on a reading surface

A concept need not have a dedicated renderer to remain useful. The fallback
shows its heading-derived title, resolved identity, concept and authored fields.
The declaration stays available through a source link. Affordances keep a class
hook for their existing presentation and evidence; a custom Paper uses the
generic card.

Definitions are resolved across the collection before any member is rendered;
a definition conflict fails the shared vocabulary closed. Instance errors are
scoped to their source document. Duplicate canonical identities diagnose every
owner, and a cross-document range error belongs to the source of that edge.
Healthy documents retain resolved cards and links.

The collection is resolved before any member is rendered, so a definition in
one selected document can describe an instance in another. Callers may pass
their selected base model. Single-document entry points retain the supplied
base vocabulary by default. Resolution errors remain visible beside the
original YAML; an unresolved declaration is never silently discarded.

HTML uses the existing weave and publication theme variables. Forge output
adds one reversible caption above the original fence. These are surface
renderings, not a new theme or plugin registry.

<a name="chunk-root"></a><sub>[`src/instance_rendering.rs`](../../crates/x0k-tangle/src/instance_rendering.rs) · `#root`</sub>

```rust {#root}
use std::collections::BTreeMap;
use x0k_folio::document_vocabulary::{load_definitions, collect_instances, validate_relationships, DocumentSource};
use x0k_ontology::concept_facts::OntologyModel;

/// A presentation reads the selected collection, never a global custom-term registry.
pub struct InstancePresentation {
    instances: BTreeMap<(String, String), InstanceView>,
    diagnostic: Option<String>,
    document_diagnostics: BTreeMap<String, String>,
}

pub struct InstanceView {
    pub identity: String,
    pub concept: String,
    pub title: String,
    pub source_document: String,
    pub source_start: Option<usize>,
    pub source_end: Option<usize>,
    pub occurrence: usize,
    pub fields: Vec<(String, String)>,
    pub diagnostic: Option<String>,
}

impl InstancePresentation {
    pub fn collect(documents: &[(&str, &str)], base: &OntologyModel) -> Self {
        let sources: Vec<_> = documents.iter().map(|(id, content)| DocumentSource {
            id,
            body: x0k_folio::colophon::split_frontmatter(content).map(|(_, body)| body).unwrap_or(content),
        }).collect();
        let vocabulary = match load_definitions(&sources, base) {
            Ok(vocabulary) => vocabulary,
            Err(error) => return Self {
                instances: BTreeMap::new(), diagnostic: Some(error.to_string()),
                document_diagnostics: BTreeMap::new(),
            },
        };
        let mut document_diagnostics = BTreeMap::new();
        let mut candidates = Vec::new();
        for source in &sources {
            match collect_instances(std::slice::from_ref(source), &vocabulary.model) {
                Ok(instances) => candidates.extend(instances),
                Err(error) => { document_diagnostics.insert(source.id.to_string(), error.to_string()); }
            }
        }
        // Canonical identity owns the collision, even when authored prefixes differ.
        // Every owning document receives the same diagnostic; no first-writer winner.
        let mut owners: BTreeMap<String, Vec<String>> = BTreeMap::new();
        for source in &sources {
            // Include extractable identities from invalid documents too: a local
            // error must not hide their collision with another document.
            for entity in x0k_folio::inline_entity::extract_from_markdown_in(source.body, &vocabulary.model)
                .into_iter().flatten() {
                owners.entry(vocabulary.model.expand(&entity.uri.to_string()))
                    .or_default().push(source.id.to_string());
            }
        }
        for (identity, documents) in owners.iter().filter(|(_, documents)| documents.len() > 1) {
            let message = format!("duplicate instance {identity} in {}", documents.join(", "));
            for document in documents {
                document_diagnostics.insert(document.clone(), message.clone());
            }
        }
        candidates.retain(|instance| !document_diagnostics.contains_key(&instance.source.document));
        // Cross-document range checks can fail only after the combined instance
        // set is known. Remove all sources named by each failure, then revalidate.
        // Definitions/model failures without a source remain globally fail-closed.
        while let Err(error) = validate_relationships(&vocabulary.model, &candidates) {
            if error.sources.is_empty() {
                return Self { instances: BTreeMap::new(), diagnostic: Some(error.to_string()), document_diagnostics };
            }
            let before = candidates.len();
            for source in &error.sources {
                document_diagnostics.insert(source.document.clone(), error.to_string());
            }
            candidates.retain(|instance| !document_diagnostics.contains_key(&instance.source.document));
            if candidates.len() == before {
                return Self { instances: BTreeMap::new(), diagnostic: Some(error.to_string()), document_diagnostics };
            }
        }
        let instances = candidates.into_iter().map(|instance| {
            let authored = instance.entity.uri.to_string();
            let view = InstanceView {
                identity: instance.iri, concept: instance.concept,
                title: instance.entity.title, source_document: instance.source.document.clone(),
                source_start: Some(instance.source.bytes.start), source_end: Some(instance.source.bytes.end), occurrence: 0,
                fields: fields(&instance.entity.yaml), diagnostic: None,
            };
            ((instance.source.document, authored), view)
        }).collect();
        Self { instances, diagnostic: None, document_diagnostics }
    }

    pub fn same_document(document: &str, content: &str) -> Self {
        Self::collect(&[(document, content)], &OntologyModel::shipped())
    }

    pub fn view(&self, document: &str, info: &str, code: &str) -> Option<InstanceView> {
        let tokens: Vec<_> = info.split_ascii_whitespace().collect();
        if tokens.len() != 2 || !tokens[0].eq_ignore_ascii_case("yaml") || !tokens[1].contains(':')
            || tokens[1] == "x0k:params" {
            return None;
        }
        let mapping = serde_norway::from_str::<serde_norway::Mapping>(code).ok();
        let authored = mapping.as_ref().and_then(|m|
            m.get(serde_norway::Value::String("id".into())).and_then(|v| v.as_str()));
        if let Some(view) = authored.and_then(|id| self.instances.get(&(document.to_string(), id.to_string()))) {
            return Some(InstanceView {
                identity: view.identity.clone(), concept: view.concept.clone(), title: view.title.clone(),
                source_document: view.source_document.clone(), source_start: view.source_start,
                source_end: view.source_end, occurrence: 0, fields: view.fields.clone(), diagnostic: None,
            });
        }
        Some(InstanceView {
            identity: authored.unwrap_or("(missing identity)").to_string(), concept: tokens[1].to_string(),
            title: "Unresolved instance".to_string(), source_document: document.to_string(),
            source_start: None, source_end: None, occurrence: 0,
            fields: mapping.as_ref().map(fields).unwrap_or_default(),
            diagnostic: Some(self.diagnostic.clone().or_else(|| self.document_diagnostics.get(document).cloned()).unwrap_or_else(||
                "This declaration's concept or identity is not resolved in the selected vocabulary.".to_string())),
        })
    }
}

fn fields(mapping: &serde_norway::Mapping) -> Vec<(String, String)> {
    let mut fields: Vec<_> = mapping.iter().filter_map(|(key, value)| {
        let key = key.as_str()?;
        if key == "id" { return None; }
        let value = value.as_str().map(str::to_string)
            .unwrap_or_else(|| serde_norway::to_string(value).unwrap_or_default().trim().to_string());
        Some((key.to_string(), value))
    }).collect();
    fields.sort();
    fields
}

pub const INSTANCE_CAPTION_MARK: &str = "<a name=\"folio-instance-";
pub const STYLESHEET: &str = r#"
.folio-instance { margin: 1.4rem 0; padding: 1rem; border: 1px solid var(--border); border-radius: 6px; }
.folio-instance-affordance { border-left: 3px solid var(--accent); }
.folio-instance header { display: flex; gap: .7rem; align-items: baseline; flex-wrap: wrap; }
.folio-instance .instance-kind, .folio-instance .instance-source { color: var(--text-quiet); font-size: .85rem; }
.folio-instance .instance-identity { overflow-wrap: anywhere; }
.folio-instance dl { display: grid; grid-template-columns: minmax(5rem, 1fr) 3fr; gap: .4rem 1rem; }
.folio-instance dt { color: var(--text-quiet); }
.folio-instance dd { margin: 0; overflow-wrap: anywhere; white-space: pre-wrap; }
.folio-instance details { margin-top: .7rem; }
.folio-instance .instance-diagnostic { border-left: 2px solid var(--accent); padding-left: .7rem; }
"#;

fn escape(value: &str) -> String {
    value.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;").replace('"', "&quot;")
}

fn anchor(view: &InstanceView) -> String {
    let identity: String = view.identity.as_bytes().iter().map(|byte| format!("{byte:02x}")).collect();
    format!("{identity}-{}", view.occurrence)
}

fn kind(view: &InstanceView) -> &str {
    view.concept.rsplit(['#', ':', '/']).next().unwrap_or(&view.concept)
}

pub fn html(view: &InstanceView, code: &str) -> String {
    let anchor = anchor(view);
    let source_range = match (view.source_start, view.source_end) {
        (Some(start), Some(end)) => format!(" data-source-start=\"{start}\" data-source-end=\"{end}\""),
        _ => String::new(),
    };
    let class = if view.concept == "https://0k.computer/ontology#Affordance" {
        "folio-instance folio-instance-affordance"
    } else { "folio-instance" };
    let mut html = format!("<section class=\"{class}\" id=\"folio-instance-{anchor}\" data-instance-iri=\"{}\" data-concept-iri=\"{}\" data-source-document=\"{}\"{source_range}>\
        <header><span class=\"instance-kind\">{}</span><strong>{}</strong></header>\
        <p class=\"instance-identity\"><code>{}</code></p><dl>",
        escape(&view.identity), escape(&view.concept), escape(&view.source_document), escape(kind(view)),
        escape(&view.title), escape(&view.identity));
    for (key, value) in &view.fields {
        html.push_str(&format!("<dt>{}</dt><dd>{}</dd>", escape(key), escape(value)));
    }
    html.push_str("</dl>");
    if let Some(diagnostic) = &view.diagnostic {
        html.push_str(&format!("<p class=\"instance-diagnostic\">{}</p>", escape(diagnostic)));
    }
    html.push_str(&format!("<a class=\"instance-source\" href=\"#folio-source-{anchor}\">Source declaration</a>\
        <details id=\"folio-source-{anchor}\"><summary>Declaration</summary><pre><code class=\"language-yaml\">{}</code></pre></details></section>\n",
        escape(code)));
    html
}

/// One removable caption; the original fence remains byte-for-byte below it.
pub fn gfm(view: &InstanceView) -> String {
    let anchor = anchor(view);
    let diagnostic = view.diagnostic.as_ref().map(|d| format!(" · {}", escape(&d.replace('\n', " ")))).unwrap_or_default();
    format!("{INSTANCE_CAPTION_MARK}{anchor}\"></a><sub data-instance-iri=\"{}\" data-concept-iri=\"{}\" data-source-document=\"{}\"><strong>{}</strong> · {} · <code>{}</code> · <a href=\"#folio-source-{anchor}\">source declaration</a>{diagnostic}</sub><a name=\"folio-source-{anchor}\"></a>\n\n",
        escape(&view.identity), escape(&view.concept), escape(&view.source_document),
        escape(kind(view)), escape(&view.title), escape(&view.identity))
}
```

## A declaration on both surfaces

These tests use one Paper and one Affordance, with the Paper definition in another selected document.

<a name="chunk-tests"></a><sub>[`src/instance_rendering.rs`](../../crates/x0k-tangle/src/instance_rendering.rs) · `#tests`</sub>

```rust {#tests}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parser::parse_document;
    use crate::region_gfm::{weave_chapter_with_instances, unweave_chapter, ChapterLinks};
    use crate::region_weave::{RegionInput, RegionMember, weave_region_with_vocabulary};
    use crate::weave::weave_html_with_instances;
    use std::path::PathBuf;

    fn fixture() -> (String, String) {
        let fence = char::from(96).to_string().repeat(3);
        let definitions = format!("# Vocabulary\n\n{fence}turtle folio:ontology\n\
            @prefix owl: <http://www.w3.org/2002/07/owl#> .\n\
            @prefix rdfs: <http://www.w3.org/2000/01/rdf-schema#> .\n\
            @prefix vann: <http://purl.org/vocab/vann/> .\n\
            @prefix paper: <https://example.test/paper#> .\n\
            <https://example.test/vocabulary> a owl:Ontology ;\n\
              vann:preferredNamespacePrefix \"paper\" ;\n\
              vann:preferredNamespaceUri \"https://example.test/paper#\" .\n\
            paper:Paper a owl:Class ; rdfs:isDefinedBy <https://example.test/vocabulary> .\n\
            {fence}\n");
        let instances = format!("---\nx0k:\n  format: folio/v1\n  id: x0k:design/example\n  type: design\n---\n\
            # Examples\n\n## A paper\nA short paper description.\n\n\
            {fence}yaml paper:paper\nid: paper:paper/one\nauthor: \"A <reader>\"\n{fence}\n\n\
            ## Read the paper\nRead this document.\n\n{fence}yaml x0k:affordance\nid: x0k:affordance/read-paper\nactors: [human]\n{fence}\n");
        (definitions, instances)
    }

    #[test]
    fn the_same_paper_and_affordance_render_on_html_and_reversible_forge_surfaces() {
        let (definitions, source) = fixture();
        let source=format!("{source}\n```svg x0k:icon\n<svg viewBox=\"0 0 16 16\"/>\n```\n");

        let context = InstancePresentation::collect(&[("vocab.md", &definitions), ("example.md", &source)],
            &OntologyModel::shipped());
        let html = weave_html_with_instances(&source, &parse_document(&source).unwrap(), "example.md", &context).unwrap().html;
        let links = ChapterLinks { uri_to_rel: &BTreeMap::new(), affordances: &BTreeMap::new() };
        let woven = weave_chapter_with_instances(&source, "example.md", None, &links, &context).unwrap();
        for text in [&html, &woven] {
            assert!(text.contains("https://example.test/paper#paper/one"), "{text}");
            assert!(text.contains("https://example.test/paper#Paper"), "{text}");
            assert!(text.contains("https://0k.computer/ontology#Affordance"), "{text}");
            assert!(text.contains("folio-source-"), "{text}");
            assert!(text.contains("data-source-document=\"example.md\""));
            assert!(!text.contains("Unresolved instance"), "{text}");
        }
        assert!(html.contains("folio-instance-affordance"));
        assert!(html.contains("A &lt;reader&gt;"));
        assert!(html.contains("data-source-start="));
        assert!(html.contains("var(--border)"), "cards use existing theme variables");
        assert_eq!(unweave_chapter(&woven), source);
    }

    #[test]
    fn region_wide_definitions_render_an_instance_in_another_member() {
        let (definitions, source) = fixture();
        let input = RegionInput {
            members: vec![
                RegionMember { uri: "x0k:design/example".into(), content: source, source_path: PathBuf::from("example.md") },
                RegionMember { uri: "x0k:design/vocabulary".into(), content: definitions, source_path: PathBuf::from("vocab.md") },
            ],
            entry_point_uri: "x0k:design/example".into(),
        };
        let mut output = weave_region_with_vocabulary(&input, &OntologyModel::shipped()).unwrap();
        crate::presentation::apply_publication_shell(&mut output, &input, None);
        let page = output.files.iter().find(|file| file.rel_path == std::path::Path::new("pages/index.html")).unwrap();
        let html = std::str::from_utf8(&page.bytes).unwrap();
        assert!(html.contains("https://example.test/paper#paper/one"), "{html}");
        assert!(html.contains("folio-instance-affordance"));
        assert!(!html.contains("Unresolved instance"));
        assert!(html.contains("Source declaration"));
    }

    fn paper(id: &str, extra: &str) -> String {
        format!("# Paper\n\n~~~yaml paper:paper\nid: paper:paper/{id}\n{extra}~~~\n")
    }

    #[test]
    fn malformed_sibling_does_not_degrade_healthy_html_or_woven_output() {
        let (definitions, _) = fixture();
        let healthy = paper("healthy", "");
        let bad = "# Broken\n\n~~~yaml paper:unknown\nid: paper:unknown/broken\n~~~\n";
        let context = InstancePresentation::collect(
            &[("vocab.md", &definitions), ("healthy.md", &healthy), ("bad.md", bad)],
            &OntologyModel::shipped());
        let view = context.view("healthy.md", "yaml paper:paper", "id: paper:paper/healthy\n").unwrap();
        assert_eq!(view.identity, "https://example.test/paper#paper/healthy");
        assert!(view.diagnostic.is_none());
        assert!(!html(&view, "id: paper:paper/healthy").contains("instance-diagnostic"));
        assert!(!gfm(&view).contains("Unresolved"));
        let bad = context.view("bad.md", "yaml paper:unknown", "id: paper:unknown/broken\n").unwrap();
        assert!(bad.diagnostic.as_ref().unwrap().contains("bad.md"));
        assert!(html(&bad, "id: paper:unknown/broken").contains("instance-diagnostic"));
    }

    #[test]
    fn every_duplicate_owner_is_diagnosed_without_harming_a_healthy_sibling() {
        let (definitions, _) = fixture();
        let duplicate = paper("shared", "");
        let healthy = paper("healthy", "");
        let context = InstancePresentation::collect(
            &[("vocab.md", &definitions), ("a.md", &duplicate), ("b.md", &duplicate),
              ("c.md", &duplicate), ("healthy.md", &healthy)], &OntologyModel::shipped());
        for document in ["a.md", "b.md", "c.md"] {
            let view = context.view(document, "yaml paper:paper", "id: paper:paper/shared\n").unwrap();
            let error = view.diagnostic.unwrap();
            assert!(error.contains("duplicate instance https://example.test/paper#paper/shared"), "{error}");
            for owner in ["a.md", "b.md", "c.md"] { assert!(error.contains(owner), "{error}"); }
        }
        assert!(context.view("healthy.md", "yaml paper:paper", "id: paper:paper/healthy\n").unwrap().diagnostic.is_none());
    }

    #[test]
    fn duplicate_inside_one_document_still_invalidates_another_owner() {
        let (definitions, _) = fixture();
        let one = paper("shared", "");
        let two = format!("{one}\\n{one}");
        let context = InstancePresentation::collect(
            &[("vocab.md", &definitions), ("one.md", &one), ("two.md", &two)],
            &OntologyModel::shipped());
        for document in ["one.md", "two.md"] {
            let error = context.view(document, "yaml paper:paper", "id: paper:paper/shared\\n").unwrap().diagnostic.unwrap();
            assert!(error.contains("one.md") && error.contains("two.md"), "{error}");
        }
    }

    #[test]
    fn bad_cross_document_range_diagnoses_its_source_only() {
        let (definitions, _) = fixture();
        let definitions = definitions.replace(
            "paper:Paper a owl:Class ; rdfs:isDefinedBy <https://example.test/vocabulary> .",
            "paper:Paper a owl:Class ; rdfs:isDefinedBy <https://example.test/vocabulary> .\n\
             paper:Other a owl:Class ; rdfs:isDefinedBy <https://example.test/vocabulary> .\n\
             paper:cites a owl:ObjectProperty ; rdfs:isDefinedBy <https://example.test/vocabulary> ; rdfs:domain paper:Paper ; rdfs:range paper:Paper .");
        let bad = paper("bad", "edges:\n  paper:cites: [paper:other/target]\n");
        let target = "# Target\n\n~~~yaml paper:other\nid: paper:other/target\n~~~\n";
        let healthy = paper("healthy", "");
        let context = InstancePresentation::collect(
            &[("vocab.md", &definitions), ("bad.md", &bad), ("target.md", target),
              ("healthy.md", &healthy)], &OntologyModel::shipped());
        let bad = context.view("bad.md", "yaml paper:paper", "id: paper:paper/bad\n").unwrap();
        let error = bad.diagnostic.unwrap();
        assert!(error.contains("bad.md") && error.contains("expected"), "{error}");
        for (document, class, id) in [("target.md", "paper:other", "paper:other/target"),
            ("healthy.md", "paper:paper", "paper:paper/healthy")] {
            assert!(context.view(document, &format!("yaml {class}"), &format!("id: {id}\n")).unwrap().diagnostic.is_none());
        }
    }

    #[test]
    fn invalid_shared_definitions_fail_closed_for_all_instances() {
        let healthy = paper("healthy", "");
        let context = InstancePresentation::collect(
            &[("vocab.md", "~~~turtle folio:ontology\nnot valid turtle\n~~~\n"), ("healthy.md", &healthy)],
            &OntologyModel::shipped());
        assert!(context.view("healthy.md", "yaml paper:paper", "id: paper:paper/healthy\n").unwrap().diagnostic.is_some());
    }

    #[test]
    fn unresolved_concepts_keep_identity_source_and_explicit_diagnostic() {
        let (_, source) = fixture();
        let context = InstancePresentation::collect(&[("example.md", &source)], &OntologyModel::new([]));
        let html = weave_html_with_instances(&source, &parse_document(&source).unwrap(), "example.md", &context).unwrap().html;
        assert!(html.contains("Unresolved instance"));
        assert!(html.contains("instance-diagnostic"));
        assert!(html.contains("paper:paper/one"));
        assert!(html.contains("A &lt;reader&gt;"));
        assert!(html.contains("Source declaration"));
    }
}
```
