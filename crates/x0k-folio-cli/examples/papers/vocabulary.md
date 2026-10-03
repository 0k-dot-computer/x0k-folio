# Papers and citations

```turtle folio:document
wiki:paper-vocabulary a x0k:Wiki ;
    x0k:summary "A small vocabulary for papers and their citations." .
```

A paper can cite another paper, and says whether it was reviewed and how many
pages it has. The vocabulary belongs to this collection.

```turtle folio:graph
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
