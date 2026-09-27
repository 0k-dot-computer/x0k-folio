---
x0k:
  format: folio/v1
  id: x0k:wiki/paper-vocabulary
  type: wiki
  summary: A small vocabulary for papers and their citations.
---
# Papers and citations

A paper can cite another paper, and says whether it was reviewed and how many
pages it has. The vocabulary belongs to this collection.

```turtle folio:ontology
@prefix owl: <http://www.w3.org/2002/07/owl#> .
@prefix rdfs: <http://www.w3.org/2000/01/rdf-schema#> .
@prefix xsd: <http://www.w3.org/2001/XMLSchema#> .
@prefix vann: <http://purl.org/vocab/vann/> .
@prefix paper: <https://example.org/papers#> .

<https://example.org/paper-vocabulary> a owl:Ontology ;
    vann:preferredNamespacePrefix "paper" ;
    vann:preferredNamespaceUri "https://example.org/papers#" .
paper:Paper a owl:Class ;
    rdfs:label "Paper" ;
    rdfs:isDefinedBy <https://example.org/paper-vocabulary> .
paper:cites a owl:ObjectProperty ;
    rdfs:domain paper:Paper ;
    rdfs:range paper:Paper ;
    rdfs:isDefinedBy <https://example.org/paper-vocabulary> .
paper:reviewed a owl:DatatypeProperty ;
    rdfs:domain paper:Paper ;
    rdfs:range xsd:boolean ;
    rdfs:isDefinedBy <https://example.org/paper-vocabulary> .
paper:pages a owl:DatatypeProperty ;
    rdfs:domain paper:Paper ;
    rdfs:range xsd:integer ;
    rdfs:isDefinedBy <https://example.org/paper-vocabulary> .
```
