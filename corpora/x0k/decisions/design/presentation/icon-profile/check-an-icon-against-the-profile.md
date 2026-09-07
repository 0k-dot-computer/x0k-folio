---
x0k:
  format: folio/v1
  id: x0k:design/icon-profile#check-an-icon-against-the-profile
  type: design
  status: proposed
  edges:
    transcludes:
      - x0k:design/icon-profile
---

### Check an icon against the profile

Check an icon's geometry and strokes against the shared drawing rules.

An author, or the composing loop, hands a drawing to the checker and is
told it is accepted or which rule it broke and where — never handed back
a silently altered drawing.

<a name="folio-instance-68747470733a2f2f306b2e636f6d70757465722f6f6e746f6c6f6779236166666f7264616e63652f636865636b5f616e5f69636f6e5f616761696e73745f7468655f70726f66696c65-1"></a><sub data-instance-iri="https://0k.computer/ontology#affordance/check_an_icon_against_the_profile" data-concept-iri="https://0k.computer/ontology#Affordance" data-source-document="corpora/x0k/decisions/design/presentation/icon-profile/check-an-icon-against-the-profile.md"><strong>Affordance</strong> · Check an icon against the profile · <code>https://0k.computer/ontology#affordance/check_an_icon_against_the_profile</code> · <a href="#folio-source-68747470733a2f2f306b2e636f6d70757465722f6f6e746f6c6f6779236166666f7264616e63652f636865636b5f616e5f69636f6e5f616761696e73745f7468655f70726f66696c65-1">source declaration</a></sub><a name="folio-source-68747470733a2f2f306b2e636f6d70757465722f6f6e746f6c6f6779236166666f7264616e63652f636865636b5f616e5f69636f6e5f616761696e73745f7468655f70726f66696c65-1"></a>

```yaml x0k:affordance
id: x0k:affordance/check_an_icon_against_the_profile
status: wip
actors: [human, ai_agent]
edges:
  enabledBy:
    - x0k:software-module/x0k-icon
  requires:
    - x0k:affordance/declare_an_icon
```

<picture><source media="(prefers-color-scheme: dark)" srcset="../../../../../../affordances/actor-dark.svg"><img alt="Actor" src="../../../../../../affordances/actor-light.svg" height="20"></picture> <picture><source media="(prefers-color-scheme: dark)" srcset="../../../../../../affordances/proven-dark.svg"><img alt="proven" src="../../../../../../affordances/proven-light.svg" height="16"></picture> *proven* · for a person, an agent · reachable through `cli` `x0k-tangle icon`, `sdk` `check`

*realized in* [x0k-icon: the crate](../../../../implementation/icon/crate.md) · [The checker](../../../../implementation/icon/validate.md) · [The faces behind `check`, `affordances` and `icon`](../../../../implementation/tangle/cli-faces.md) · [x0k-tangle: the crate and its CLI](../../../../implementation/tangle/crate.md)

*proven by* each test below, as its chapter tangles it and as it ran at projection.

<details><summary><code>the_tangle_icon_is_accepted</code> · <picture><source media="(prefers-color-scheme: dark)" srcset="../../../../../../affordances/passed-dark.svg"><img alt="passed" src="../../../../../../affordances/passed-light.svg" height="16"></picture> passed · <a href="../../../../implementation/icon/validate.md#chunk-tests">#tests</a> in The checker</summary>

```rust
#[test]
fn the_tangle_icon_is_accepted() {
    let accepted = validate(parse(fixtures::TANGLE).unwrap()).unwrap();
    assert_eq!(accepted.grid(), Grid::Sixteen);
    assert_eq!(accepted.command_count(), 16);
}
```

</details>

<details><summary><code>a_literal_colour_is_refused_by_rule_5</code> · <picture><source media="(prefers-color-scheme: dark)" srcset="../../../../../../affordances/passed-dark.svg"><img alt="passed" src="../../../../../../affordances/passed-light.svg" height="16"></picture> passed · <a href="../../../../implementation/icon/validate.md#chunk-tests">#tests</a> in The checker</summary>

```rust
#[test]
fn a_literal_colour_is_refused_by_rule_5() {
    let svg = fixtures::TANGLE.replace(r#"stroke="ink" stroke-width="1.5""#, r##"stroke="#111111" stroke-width="1.5""##);
    let defects = validate(parse(&svg).unwrap()).unwrap_err();
    assert_eq!(defects.len(), 1);
    assert_eq!(defects[0].rule, Rule::LiteralPaint);
    assert_eq!(defects[0].element, ElementRef { ordinal: 4, tag: "rect".to_string() });
    assert_eq!(
        defects[0].to_string(),
        "rule 5 (a literal paint): rect #4 — stroke=\"#111111\"; a paint is ink, line, paper, accent or none"
    );
}
```

</details>

<details><summary><code>a_corner_past_the_safe_area_is_refused_by_rule_11</code> · <picture><source media="(prefers-color-scheme: dark)" srcset="../../../../../../affordances/passed-dark.svg"><img alt="passed" src="../../../../../../affordances/passed-light.svg" height="16"></picture> passed · <a href="../../../../implementation/icon/validate.md#chunk-tests">#tests</a> in The checker</summary>

```rust
#[test]
fn a_corner_past_the_safe_area_is_refused_by_rule_11() {
    let svg = fixtures::TANGLE.replace(r#"x="6.5" y="9.5" width="7""#, r#"x="8.5" y="9.5" width="7""#);
    let defects = validate(parse(&svg).unwrap()).unwrap_err();
    assert_eq!(defects.len(), 1);
    assert_eq!(defects[0].rule, Rule::OutsideTheSafeArea);
    assert_eq!(defects[0].element.ordinal, 4);
    assert!(defects[0].detail.starts_with("far corner at (15.5 13.5)"), "{}", defects[0].detail);
}
```

</details>

<details><summary><code>a_page_over_budget_is_refused_by_rule_12</code> · <picture><source media="(prefers-color-scheme: dark)" srcset="../../../../../../affordances/passed-dark.svg"><img alt="passed" src="../../../../../../affordances/passed-light.svg" height="16"></picture> passed · <a href="../../../../implementation/icon/validate.md#chunk-tests">#tests</a> in The checker</summary>

```rust
#[test]
fn a_page_over_budget_is_refused_by_rule_12() {
    let svg = fixtures::TANGLE.replace(
        r#"d="M4.5 6.5 H9 M4.5 8.5 H7""#,
        r#"d="M4.5 6.5 H9 M4.5 8.5 H7 M4.5 10.5 H7 M4.5 11 H7 M4.5 12 H7 M4.5 13 H7 M4.5 14 H7""#,
    );
    let defects = validate(parse(&svg).unwrap()).unwrap_err();
    assert_eq!(defects.len(), 1);
    assert_eq!(defects[0].rule, Rule::OverBudget);
    assert_eq!(defects[0].element.ordinal, 0);
    assert_eq!(defects[0].detail, "26 drawing commands; the budget is 24");
}
```

</details>

<details><summary><code>every_broken_rule_is_reported_in_one_pass</code> · <picture><source media="(prefers-color-scheme: dark)" srcset="../../../../../../affordances/passed-dark.svg"><img alt="passed" src="../../../../../../affordances/passed-light.svg" height="16"></picture> passed · <a href="../../../../implementation/icon/validate.md#chunk-tests">#tests</a> in The checker</summary>

```rust
#[test]
fn every_broken_rule_is_reported_in_one_pass() {
    let svg = fixtures::TANGLE
        .replace(r#"stroke="ink" stroke-width="1.5""#, r##"stroke="#111111" stroke-width="1.5""##)
        .replace(r#"x="6.5" y="9.5" width="7""#, r#"x="8.5" y="9.5" width="7""#)
        .replace(
            r#"d="M4.5 6.5 H9 M4.5 8.5 H7""#,
            r#"d="M4.5 6.5 H9 M4.5 8.5 H7 M4.5 10.5 H7 M4.5 11 H7 M4.5 12 H7 M4.5 13 H7 M4.5 14 H7""#,
        );
    assert_eq!(refusals(&svg), vec![Rule::LiteralPaint, Rule::OutsideTheSafeArea, Rule::OverBudget]);
}
```

</details>

<details><summary><code>each_rule_has_a_drawing_that_breaks_only_it</code> · <picture><source media="(prefers-color-scheme: dark)" srcset="../../../../../../affordances/passed-dark.svg"><img alt="passed" src="../../../../../../affordances/passed-light.svg" height="16"></picture> passed · <a href="../../../../implementation/icon/validate.md#chunk-tests">#tests</a> in The checker</summary>

```rust
#[test]
fn each_rule_has_a_drawing_that_breaks_only_it() {
    let cases: Vec<(String, Rule)> = vec![
        (format!("<g viewBox=\"0 0 16 16\">{RING}</g>"), Rule::NotOneRoot),
        (format!("<svg viewBox=\"0 0 16 16\" width=\"16\">{RING}</svg>"), Rule::NotOneRoot),
        (format!("<svg viewBox=\"0 0 20 20\">{RING}</svg>"), Rule::OffGridViewBox),
        (wrap(&format!("{RING}<text x=\"2\" y=\"2\">a</text>")), Rule::UnknownElement),
        (wrap(&format!("{RING}<!-- a comment -->")), Rule::UnknownElement),
        (wrap(&RING.replace("fill=\"none\"", "fill=\"none\" id=\"ring\"")), Rule::UnknownAttribute),
        (wrap(&RING.replace("stroke=\"ink\"", "stroke=\"currentColor\"")), Rule::LiteralPaint),
        (wrap(&RING.replace("stroke=\"ink\"", "stroke=\"paper\"")), Rule::PaperAsStroke),
        (wrap(&RING.replace("stroke-width=\"1.5\"", "stroke-width=\"2\"")), Rule::OffRegisterStroke),
        (wrap(&RING.replace("stroke-width=\"1.5\"", "stroke-width=\"1\" stroke-dasharray=\"2 2\"")), Rule::OffProfileDash),
        (wrap(r#"<path d="M4 4 l8 8" fill="none" stroke="ink" stroke-width="1"/>"#), Rule::RelativeOrUnknownPathCommand),
        (wrap(&RING.replace("r=\"6\"", "r=\"5.25\"")), Rule::OffTheHalfUnit),
        (wrap(&RING.replace("cx=\"8\"", "cx=\"9.5\"")), Rule::OutsideTheSafeArea),
        (wrap(&RING.repeat(9)), Rule::OverBudget),
        (wrap(&format!("<g transform=\"rotate(45)\">{RING}</g>")), Rule::TransformNotATranslation),
        (wrap(&RING.replace("<circle", "<circle transform=\"translate(1 1)\"")), Rule::TransformNotATranslation),
        (wrap(&RING.replace("stroke=\"ink\"", "stroke=\"none\"")), Rule::NothingPainted),
    ];
    for (svg, rule) in cases {
        assert_eq!(refusals(&svg), vec![rule], "{svg}");
    }
}
```

</details>

<details><summary><code>a_translation_moves_the_safe_area_check_with_its_subtree</code> · <picture><source media="(prefers-color-scheme: dark)" srcset="../../../../../../affordances/passed-dark.svg"><img alt="passed" src="../../../../../../affordances/passed-light.svg" height="16"></picture> passed · <a href="../../../../implementation/icon/validate.md#chunk-tests">#tests</a> in The checker</summary>

```rust
#[test]
fn a_translation_moves_the_safe_area_check_with_its_subtree() {
    let inside = wrap(&format!("<g transform=\"translate(1 1)\">{}</g>", RING.replace("r=\"6\"", "r=\"5\"").replace("cx=\"8\" cy=\"8\"", "cx=\"7\" cy=\"7\"")));
    assert_eq!(refusals(&inside), vec![]);
    let outside = wrap(&format!("<g transform=\"translate(2 2)\">{RING}</g>"));
    assert_eq!(refusals(&outside), vec![Rule::OutsideTheSafeArea, Rule::OutsideTheSafeArea]);
}
```

</details>

<details><summary><code>a_paint_inherited_from_a_group_counts_as_painted</code> · <picture><source media="(prefers-color-scheme: dark)" srcset="../../../../../../affordances/passed-dark.svg"><img alt="passed" src="../../../../../../affordances/passed-light.svg" height="16"></picture> passed · <a href="../../../../implementation/icon/validate.md#chunk-tests">#tests</a> in The checker</summary>

```rust
#[test]
fn a_paint_inherited_from_a_group_counts_as_painted() {
    let svg = wrap(r#"<g stroke="ink" stroke-width="1.5"><circle cx="8" cy="8" r="6"/></g>"#);
    assert_eq!(refusals(&svg), vec![]);
}
```

</details>

<details><summary><code>a_second_icon_on_one_grid_is_refused_by_rule_15</code> · <picture><source media="(prefers-color-scheme: dark)" srcset="../../../../../../affordances/passed-dark.svg"><img alt="passed" src="../../../../../../affordances/passed-light.svg" height="16"></picture> passed · <a href="../../../../implementation/icon/validate.md#chunk-tests">#tests</a> in The checker</summary>

```rust
#[test]
fn a_second_icon_on_one_grid_is_refused_by_rule_15() {
    let a = validate(parse(fixtures::TANGLE).unwrap()).unwrap();
    let b = validate(parse(fixtures::WEAVE).unwrap()).unwrap();
    let defects = one_per_grid(&[&a, &b]);
    assert_eq!(defects.len(), 1);
    assert_eq!(defects[0].rule, Rule::TwoIconsOnOneGrid);
    assert_eq!(one_per_grid(&[&a]), vec![]);
}
```

</details>

<details><summary><code>icon_accepts_a_declaration_in_the_profile</code> · <picture><source media="(prefers-color-scheme: dark)" srcset="../../../../../../affordances/passed-dark.svg"><img alt="passed" src="../../../../../../affordances/passed-light.svg" height="16"></picture> passed · <a href="../../../../implementation/tangle/cli-faces.md#chunk-tests-icon">#tests-icon</a> in The faces behind `check`, `affordances` and `icon`</summary>

```rust
#[test]
fn icon_accepts_a_declaration_in_the_profile() {
    let tmp = TempDir::new().unwrap();
    write(tmp.path(), "docs/fixture.md", &design_doc_with_icon(PERSON));

    let out = run(&["icon"], tmp.path());
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(out.status.success(), "the design's own mark was refused: {stderr}");
    assert!(stderr.contains("1 icon(s) checked, 0 refused"), "{stderr}");
}
```

</details>

<details><summary><code>icon_refuses_a_drawing_by_rule_naming_the_element_and_fails</code> · <picture><source media="(prefers-color-scheme: dark)" srcset="../../../../../../affordances/passed-dark.svg"><img alt="passed" src="../../../../../../affordances/passed-light.svg" height="16"></picture> passed · <a href="../../../../implementation/tangle/cli-faces.md#chunk-tests-icon">#tests-icon</a> in The faces behind `check`, `affordances` and `icon`</summary>

```rust
#[test]
fn icon_refuses_a_drawing_by_rule_naming_the_element_and_fails() {
    let tmp = TempDir::new().unwrap();
    let literal = PERSON.replacen("stroke=\"ink\"", "stroke=\"#111111\"", 1);
    write(tmp.path(), "docs/fixture.md", &design_doc_with_icon(&literal));

    let out = run(&["icon"], tmp.path());
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(!out.status.success(), "a literal paint passed: {stderr}");
    assert!(stderr.contains("rule 5 (a literal paint): circle #1"), "the rule and the element: {stderr}");
    assert!(stderr.contains("fixture.md § Frob the widget"), "and where: {stderr}");
    assert!(stderr.contains("1 icon(s) checked, 1 refused"), "{stderr}");
}
```

</details>


Its mark is a drawing checked against its frame.

```svg x0k:icon
<svg viewBox="0 0 16 16">
  <rect x="1.5" y="1.5" width="10" height="10" rx="1" fill="none" stroke="line" stroke-width="1"/>
  <path d="M6.5 3.5 L9 6.5 L6.5 9 L4 6.5 Z" fill="none" stroke="ink" stroke-width="1.5"/>
  <path d="M8.5 12 L10.5 14 L14.5 9.5" fill="none" stroke="ink" stroke-width="1.5"/>
</svg>
```
