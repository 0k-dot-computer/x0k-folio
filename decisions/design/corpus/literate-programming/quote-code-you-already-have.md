```turtle folio:document
design:literate-programming%23quote-code-you-already-have a x0k:Design ;
    x0k:status "proposed" ;
    x0k:transcludes design:literate-programming .
```

### Quote code you already have

Quote a function or type from a source file, and be told when the quote no longer matches it.

I explain code that lives in an ordinary source file and stays there. A code
block in my document names the file and the symbol, and the toolchain fills it
with the whole item the symbol names, read by the grammar of its language. The
quote cannot go stale quietly: once the source changes, the check fails until
the block is filled again. The document needs no header for this, so any
Markdown page can quote code.

<a name="folio-instance-68747470733a2f2f306b2e636f6d70757465722f6f6e746f6c6f6779236166666f7264616e63652f71756f74655f636f64655f66726f6d5f6974735f736f75726365-1"></a><sub data-instance-iri="https://0k.computer/ontology#affordance/quote_code_from_its_source" data-concept-iri="https://0k.computer/ontology#Affordance" data-source-document="corpora/x0k/decisions/design/corpus/literate-programming/quote-code-you-already-have.md"><strong>Affordance</strong> · Quote code you already have · <code>https://0k.computer/ontology#affordance/quote_code_from_its_source</code> · <a href="#folio-source-68747470733a2f2f306b2e636f6d70757465722f6f6e746f6c6f6779236166666f7264616e63652f71756f74655f636f64655f66726f6d5f6974735f736f75726365-1">source declaration</a></sub><a name="folio-source-68747470733a2f2f306b2e636f6d70757465722f6f6e746f6c6f6779236166666f7264616e63652f71756f74655f636f64655f66726f6d5f6974735f736f75726365-1"></a>

```turtle folio:graph
affordance:quote_code_from_its_source a x0k:Affordance ;
    x0k:claimedFor actor:human,
        actor:ai_agent ;
    x0k:enabledBy software-module:x0k-tangle .
```

<picture><source media="(prefers-color-scheme: dark)" srcset="../../../../assets/icons/actor-dark.svg"><img alt="Actor" src="../../../../assets/icons/actor-light.svg" height="20"></picture> <picture><source media="(prefers-color-scheme: dark)" srcset="../../../../assets/icons/declared-dark.svg"><img alt="declared" src="../../../../assets/icons/declared-light.svg" height="16"></picture> *declared* · for a person, an agent · reachable through `cli` `x0k-tangle sync`

*realized in* [x0k-tangle: the crate and its CLI](../../../../implementation/tangle/crate.md)


Its mark: a source file on the right, one line of it drawn heavy, and the
same line inside a block laid over the document on the left.

```svg x0k:icon
<svg viewBox="0 0 16 16">
  <path d="M1.5 1.5 H7 V14.5 H1.5 Z" fill="none" stroke="line" stroke-width="1"/>
  <path d="M3 4 H5.5 M3 12.5 H5.5" fill="none" stroke="line" stroke-width="1"/>
  <path d="M9 1.5 H12.5 L14.5 3.5 V14.5 H9 Z" fill="none" stroke="line" stroke-width="1"/>
  <path d="M10.5 8 H13" fill="none" stroke="ink" stroke-width="1.5"/>
  <rect x="2.5" y="6" width="6" height="4" rx="0.5" fill="paper" stroke="ink" stroke-width="1.5"/>
  <path d="M4 8 H7" fill="none" stroke="ink" stroke-width="1"/>
</svg>
```
