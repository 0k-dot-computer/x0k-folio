---
x0k:
  format: folio/v1
  id: x0k:design/icon-profile#show-an-icon-on-any-surface
  type: design
  status: proposed
  edges:
    transcludes:
      - x0k:design/icon-profile
---

### Show an icon on any surface

Draw the same icon for different surfaces, using the colors and file format each needs.

A surface — a projected repository, a woven page, a plate, the shell, a
favicon — asks for the form it consumes and receives the entity's declared
icon bound to its palette, from one reader, with nothing drawn by hand.

<a name="folio-instance-68747470733a2f2f306b2e636f6d70757465722f6f6e746f6c6f6779236166666f7264616e63652f73686f775f616e5f69636f6e5f6f6e5f615f73757266616365-1"></a><sub data-instance-iri="https://0k.computer/ontology#affordance/show_an_icon_on_a_surface" data-concept-iri="https://0k.computer/ontology#Affordance" data-source-document="corpora/x0k/decisions/design/presentation/icon-profile/show-an-icon-on-any-surface.md"><strong>Affordance</strong> · Show an icon on any surface · <code>https://0k.computer/ontology#affordance/show_an_icon_on_a_surface</code> · <a href="#folio-source-68747470733a2f2f306b2e636f6d70757465722f6f6e746f6c6f6779236166666f7264616e63652f73686f775f616e5f69636f6e5f6f6e5f615f73757266616365-1">source declaration</a></sub><a name="folio-source-68747470733a2f2f306b2e636f6d70757465722f6f6e746f6c6f6779236166666f7264616e63652f73686f775f616e5f69636f6e5f6f6e5f615f73757266616365-1"></a>

```yaml x0k:affordance
id: x0k:affordance/show_an_icon_on_a_surface
status: wip
actors: [human, ai_agent]
edges:
  enabledBy:
    - x0k:software-module/x0k-icon
    - x0k:software-module/x0k-tangle
    - x0k:software-module/x0k-ui-draw
  requires:
    - x0k:affordance/declare_an_icon
```

<picture><source media="(prefers-color-scheme: dark)" srcset="../../../../../../affordances/actor-dark.svg"><img alt="Actor" src="../../../../../../affordances/actor-light.svg" height="20"></picture> <picture><source media="(prefers-color-scheme: dark)" srcset="../../../../../../affordances/proven-dark.svg"><img alt="proven" src="../../../../../../affordances/proven-light.svg" height="16"></picture> *proven* · for a person, an agent · reachable through `cli` `x0k-tangle icon`, `sdk` `files`

*realized in* [Binding roles](../../../../implementation/icon/bind.md) · [Writing an icon out](../../../../implementation/icon/emit.md) · [The faces behind `check`, `affordances` and `icon`](../../../../implementation/tangle/cli-faces.md) · [x0k-tangle: the crate and its CLI](../../../../implementation/tangle/crate.md)

*proven by* each test below, as its chapter tangles it and as it ran at projection.

<details><summary><code>the_publications_palette_block_reads_as_written</code> · <picture><source media="(prefers-color-scheme: dark)" srcset="../../../../../../affordances/passed-dark.svg"><img alt="passed" src="../../../../../../affordances/passed-light.svg" height="16"></picture> passed · <a href="../../../../implementation/icon/bind.md#chunk-tests">#tests</a> in Binding roles</summary>

```rust
#[test]
fn the_publications_palette_block_reads_as_written() {
    let palette: Palette = serde_norway::from_str(PALETTE).unwrap();
    assert_eq!(palette.light.ink, "#111111");
    assert_eq!(palette.scheme(Scheme::Dark).get(Role::Paper), "#1e293b");
}
```

</details>

<details><summary><code>a_fifth_role_or_a_missing_scheme_does_not_read</code> · <picture><source media="(prefers-color-scheme: dark)" srcset="../../../../../../affordances/passed-dark.svg"><img alt="passed" src="../../../../../../affordances/passed-light.svg" height="16"></picture> passed · <a href="../../../../implementation/icon/bind.md#chunk-tests">#tests</a> in Binding roles</summary>

```rust
#[test]
fn a_fifth_role_or_a_missing_scheme_does_not_read() {
    let fifth = PALETTE.replace(r##"accent: "#b88e44" }"##, r##"accent: "#b88e44", signal: "#c00" }"##);
    assert!(serde_norway::from_str::<Palette>(&fifth).is_err());
    let light_only = PALETTE.lines().take(2).collect::<Vec<_>>().join("\n");
    assert!(serde_norway::from_str::<Palette>(&light_only).is_err());
}
```

</details>

<details><summary><code>the_tangle_icon_binds_to_the_folio_palette</code> · <picture><source media="(prefers-color-scheme: dark)" srcset="../../../../../../affordances/passed-dark.svg"><img alt="passed" src="../../../../../../affordances/passed-light.svg" height="16"></picture> passed · <a href="../../../../implementation/icon/bind.md#chunk-tests">#tests</a> in Binding roles</summary>

```rust
#[test]
fn the_tangle_icon_binds_to_the_folio_palette() {
    let palette: Palette = serde_norway::from_str(PALETTE).unwrap();
    let icon = validate(parse(fixtures::TANGLE).unwrap()).unwrap();
    let light = bind(&icon, &palette.light);
    let dark = bind(&icon, &palette.dark);
    assert_eq!(light.elements[0].stroke.as_deref(), Some("#b88e44"));
    assert_eq!(dark.elements[0].stroke.as_deref(), Some("#96b4dc"));
    assert_eq!(light.elements[3].fill.as_deref(), Some("#fffff8"));
    assert_eq!(light.elements[3].stroke.as_deref(), Some("#111111"));
    assert_eq!(light.elements[0].fill.as_deref(), Some("none"));
}
```

</details>

<details><summary><code>the_css_binding_writes_variables_and_the_name_binding_writes_roles</code> · <picture><source media="(prefers-color-scheme: dark)" srcset="../../../../../../affordances/passed-dark.svg"><img alt="passed" src="../../../../../../affordances/passed-light.svg" height="16"></picture> passed · <a href="../../../../implementation/icon/bind.md#chunk-tests">#tests</a> in Binding roles</summary>

```rust
#[test]
fn the_css_binding_writes_variables_and_the_name_binding_writes_roles() {
    let icon = validate(parse(fixtures::TANGLE).unwrap()).unwrap();
    let css = bind(&icon, &RoleBinding::css_variables());
    assert_eq!(css.elements[3].stroke.as_deref(), Some("var(--icon-ink)"));
    let names = bind(&icon, &RoleBinding::names());
    assert_eq!(names.elements[3].fill.as_deref(), Some("paper"));
}
```

</details>

<details><summary><code>the_tangle_icon_normalizes_to_the_designs_bytes</code> · <picture><source media="(prefers-color-scheme: dark)" srcset="../../../../../../affordances/passed-dark.svg"><img alt="passed" src="../../../../../../affordances/passed-light.svg" height="16"></picture> passed · <a href="../../../../implementation/icon/emit.md#chunk-tests">#tests</a> in Writing an icon out</summary>

```rust
#[test]
fn the_tangle_icon_normalizes_to_the_designs_bytes() {
    let icon = tangle();
    let text = normalized(&icon);
    assert_eq!(text, fixtures::TANGLE);
    let again = validate(parse(&text).unwrap()).unwrap();
    assert_eq!(normalized(&again), text);
}
```

</details>

<details><summary><code>a_terse_declaration_normalizes_to_the_canonical_form</code> · <picture><source media="(prefers-color-scheme: dark)" srcset="../../../../../../affordances/passed-dark.svg"><img alt="passed" src="../../../../../../affordances/passed-light.svg" height="16"></picture> passed · <a href="../../../../implementation/icon/emit.md#chunk-tests">#tests</a> in Writing an icon out</summary>

```rust
#[test]
fn a_terse_declaration_normalizes_to_the_canonical_form() {
    let terse = r#"<svg viewBox="0 0 16 16"><path stroke-width="1.50" stroke="ink" d="M2,2 4,4 6 2Z" fill="none"/></svg>"#;
    let icon = validate(parse(terse).unwrap()).unwrap();
    assert_eq!(
        normalized(&icon),
        "<svg viewBox=\"0 0 16 16\">\n  <path d=\"M2 2 L4 4 L6 2 Z\" fill=\"none\" stroke=\"ink\" stroke-width=\"1.5\"/>\n</svg>\n"
    );
}
```

</details>

<details><summary><code>the_stem_is_the_entity_ids_last_segment_in_file_form</code> · <picture><source media="(prefers-color-scheme: dark)" srcset="../../../../../../affordances/passed-dark.svg"><img alt="passed" src="../../../../../../affordances/passed-light.svg" height="16"></picture> passed · <a href="../../../../implementation/icon/emit.md#chunk-tests">#tests</a> in Writing an icon out</summary>

```rust
#[test]
fn the_stem_is_the_entity_ids_last_segment_in_file_form() {
    assert_eq!(
        stem_of("x0k:affordance/read_an_affordance_out_of_a_document"),
        "read-an-affordance-out-of-a-document"
    );
    assert_eq!(stem_of("x0k:class/AIAgent"), "aiagent");
}
```

</details>

<details><summary><code>the_per_scheme_files_differ_only_in_their_colours</code> · <picture><source media="(prefers-color-scheme: dark)" srcset="../../../../../../affordances/passed-dark.svg"><img alt="passed" src="../../../../../../affordances/passed-light.svg" height="16"></picture> passed · <a href="../../../../implementation/icon/emit.md#chunk-tests">#tests</a> in Writing an icon out</summary>

```rust
#[test]
fn the_per_scheme_files_differ_only_in_their_colours() {
    let label = Label::for_entity("x0k:affordance/project_source_code_out_of_a_document", "Tangle — project source code out of a document");
    let out = files(&tangle(), &folio_palette(), &label);
    assert_eq!(out[0].0, "project-source-code-out-of-a-document-light.svg");
    assert_eq!(out[1].0, "project-source-code-out-of-a-document-dark.svg");
    let light = &out[0].1;
    assert!(light.starts_with("<svg xmlns=\"http://www.w3.org/2000/svg\" viewBox=\"0 0 16 16\" role=\"img\" aria-label=\"Tangle — project source code out of a document\" stroke-linecap=\"round\" stroke-linejoin=\"round\">\n"), "{light}");
    assert!(light.contains("stroke=\"#b88e44\""));
    assert!(!light.contains("stroke-linecap=\"round\"/>"), "caps and joins live on the root only");
    let recoloured = light.replace("#b88e44", "#96b4dc").replace("#111111", "#e2e8f0").replace("#fffff8", "#1e293b");
    assert_eq!(recoloured, out[1].1);
}
```

</details>

<details><summary><code>inline_svg_writes_the_roles_as_variables</code> · <picture><source media="(prefers-color-scheme: dark)" srcset="../../../../../../affordances/passed-dark.svg"><img alt="passed" src="../../../../../../affordances/passed-light.svg" height="16"></picture> passed · <a href="../../../../implementation/icon/emit.md#chunk-tests">#tests</a> in Writing an icon out</summary>

```rust
#[test]
fn inline_svg_writes_the_roles_as_variables() {
    let label = Label::for_entity("x0k:affordance/project_source_code_out_of_a_document", "Tangle");
    let text = inline_svg(&tangle(), &label);
    assert!(text.contains("fill=\"var(--icon-paper)\" stroke=\"var(--icon-ink)\""));
    assert!(!text.contains("\"ink\""));
}
```

</details>

<details><summary><code>the_sprite_holds_one_symbol_per_icon_under_its_stem</code> · <picture><source media="(prefers-color-scheme: dark)" srcset="../../../../../../affordances/passed-dark.svg"><img alt="passed" src="../../../../../../affordances/passed-light.svg" height="16"></picture> passed · <a href="../../../../implementation/icon/emit.md#chunk-tests">#tests</a> in Writing an icon out</summary>

```rust
#[test]
fn the_sprite_holds_one_symbol_per_icon_under_its_stem() {
    let weave = validate(parse(fixtures::WEAVE).unwrap()).unwrap();
    let a = Label::for_entity("x0k:affordance/project_source_code_out_of_a_document", "Tangle");
    let b = Label::for_entity("x0k:affordance/read_a_document_as_the_woven_artifact", "Weave");
    let text = sprite(&[(&a, &tangle()), (&b, &weave)]);
    assert!(text.starts_with("<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"0\" height=\"0\" aria-hidden=\"true\">\n  <defs>\n    <symbol id=\"icon-project-source-code-out-of-a-document\" viewBox=\"0 0 16 16\" stroke-linecap=\"round\" stroke-linejoin=\"round\">\n      <path d=\"M2.5 1.5 H8.5"), "{text}");
    assert!(text.contains("<symbol id=\"icon-read-a-document-as-the-woven-artifact\""));
    assert_eq!(text.matches("<symbol ").count(), 2);
    assert!(text.ends_with("    </symbol>\n  </defs>\n</svg>\n"));
}
```

</details>

<details><summary><code>a_label_is_escaped_where_xml_needs_it</code> · <picture><source media="(prefers-color-scheme: dark)" srcset="../../../../../../affordances/passed-dark.svg"><img alt="passed" src="../../../../../../affordances/passed-light.svg" height="16"></picture> passed · <a href="../../../../implementation/icon/emit.md#chunk-tests">#tests</a> in Writing an icon out</summary>

```rust
#[test]
fn a_label_is_escaped_where_xml_needs_it() {
    let label = Label { stem: "x".into(), title: "Both — a person & \"an agent\"".into() };
    let text = svg(&bind(&tangle(), &RoleBinding::names()), &label);
    assert!(text.contains("aria-label=\"Both — a person &amp; &quot;an agent&quot;\""));
}
```

</details>

<details><summary><code>icon_writes_each_declaration_as_its_light_and_dark_files</code> · <picture><source media="(prefers-color-scheme: dark)" srcset="../../../../../../affordances/passed-dark.svg"><img alt="passed" src="../../../../../../affordances/passed-light.svg" height="16"></picture> passed · <a href="../../../../implementation/tangle/cli-faces.md#chunk-tests-icon-files">#tests-icon-files</a> in The faces behind `check`, `affordances` and `icon`</summary>

```rust
#[test]
fn icon_writes_each_declaration_as_its_light_and_dark_files() {
    let tmp = TempDir::new().unwrap();
    write(tmp.path(), "docs/fixture.md", &design_doc_with_icon(PERSON));
    write(tmp.path(), "publication.md", PUBLICATION);
    let out_dir = tmp.path().join("icons");

    let out = Command::new(env!("CARGO_BIN_EXE_x0k-tangle"))
        .args(["icon", "--out"])
        .arg(&out_dir)
        .arg("--palette")
        .arg(tmp.path().join("publication.md"))
        .arg(tmp.path().join("docs"))
        .output()
        .expect("the x0k-tangle binary runs");
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(out.status.success(), "{stderr}");
    assert!(stderr.contains("2 file(s) written"), "{stderr}");

    let light = fs::read_to_string(out_dir.join("frob-the-widget-light.svg"))
        .expect("named after the affordance declared in the section");
    let dark = fs::read_to_string(out_dir.join("frob-the-widget-dark.svg")).unwrap();
    assert!(light.contains("aria-label=\"Frob the widget\""), "{light}");
    assert!(light.contains("stroke=\"#111111\""), "bound to the light scheme: {light}");
    assert!(dark.contains("stroke=\"#e2e8f0\""), "bound to the dark scheme: {dark}");
    assert!(!light.contains("\"ink\""), "roles are bound, never written: {light}");
}
```

</details>


The same drawing sits on a surface: the diamond carries across, the frame
becomes a screen.

```svg x0k:icon
<svg viewBox="0 0 16 16">
  <rect x="1.5" y="2.5" width="13" height="9" rx="1" fill="none" stroke="line" stroke-width="1"/>
  <path d="M8 4 L10.5 7 L8 10 L5.5 7 Z" fill="none" stroke="ink" stroke-width="1.5"/>
  <path d="M8 11.5 V14 M5.5 14 H10.5" fill="none" stroke="line" stroke-width="1"/>
</svg>
```
