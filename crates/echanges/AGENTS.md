# Crate `echanges`

Rôle : entrées/sorties du modèle natif : import IFC et Revit vers `Projet`, export IFC, enregistrement natif atomique et récupération, rapport d'import.

Dépendances internes autorisées : `noyau`, `modele`, `ifc_base`, `formats` (dev seulement : `commandes`, `maillage`, `moteur2d`). Interdit : `moteur2d`/`moteur3d` en production, `config`, `collaboration`, `pilote`, `interface`, egui, wgpu, eframe.

## Où travailler

| Fichier (`src/`) | Contenu |
|---|---|
| `depuis_ifc.rs` | `convertir` : IFC -> `Projet` |
| `barres_ifc.rs` | poteaux et poutres IFC à section rectangulaire extrudée -> `Barre` (sinon la raison de l'import) |
| `espaces_ifc.rs` | `IfcSpace` -> `Piece` (point intérieur de l'emprise, contour détecté sur les murs convertis, contrôle d'aire à 5 %), sinon élément importé transparent |
| `donnees_ifc.rs` | propriétés, quantités, matériaux et couleur d'un élément IFC gardés sur l'élément importé |
| `depuis_rvt.rs` | `convertir`, `convertir_maquette` : Revit -> `Projet` (575 lignes : ne pas agrandir) |
| `export_ifc/mod.rs` | `exporter`, `exporter_fichier` (nom dans l'en-tête) : `Projet` -> IFC4 ; parcours des éléments |
| `export_ifc/step.rs` | écrivain STEP : textes, GUID déterministes (`guid`, ne pas changer), faces tessellées, en-tête horodaté |
| `export_ifc/{site,types,materiaux,styles}.rs` | `IfcSite` et `TrueNorth` ; `IfcWallType`… et `IfcRelDefinesByType` ; jeux de couches, `IfcMaterialLayerSetUsage`, axe des murs ; `IfcSurfaceStyle` par matériau |
| `export_ifc/{proprietes,quantites}.rs` | `Pset_*Common` (seulement les valeurs connues du modèle) ; `Qto_*BaseQuantities` pris aux calculs de `modele` des nomenclatures, aucun calcul propre |
| `export_ifc/importes.rs` | éléments importés : classe IFC d'origine (liste sûre) ou proxy, jeux lus à l'import réécrits, matériaux, couleur ; jeux lus d'une pièce (`jeux_piece`) |
| `export_ifc/tests.rs` | relecture, hiérarchie, types, Pset, quantités = nomenclature, styles, couches relues par l'import |
| `enregistrement.rs` | extensions natives `.bem` et `.bef`, `enregistrer_natif`, `ecrire_atomique`, récupération, `SourceRvt`, `Bilan` |
| `rapport.rs` | `markdown` |

`tests/` : tests d'intégration dont le projet de départ vient de `depuis_ifc` (commandes, documentation, site, escalier, volumes) ; `import_proprietes.rs` : données des éléments importés et aller-retour IFC -> natif -> IFC ; `import_barres.rs` : poteaux et poutres convertis ; `import_espaces.rs` : espaces convertis en pièces, repli, aller-retour IFC ; `copies_guid.rs` : GlobalId uniques après copie d'éléments importés ; `fusion_ifc.rs` : IFC converti fusionné dans `maison_native` (`Commande::Fusionner`) ; `couches_rvt.rs` lit un fichier hors dépôt (`RVT_ESSAIS`) et est sauté s'il est absent. Les `samples/` se lisent par `concat!(env!("CARGO_MANIFEST_DIR"), "/../../samples/...")`.

`examples/essai_isolants.rs` : génère `samples/essai_isolants.bem` (dessin des isolants en plan et en coupe : L, T, baies, mur oblique, courbe, ossature, dalle et toiture isolées, coupes perpendiculaire, oblique et presque parallèle). `examples/maison_exemple.rs` : génère `samples/maison_exemple.bem` depuis le gabarit par défaut (API de création de `modele` et commandes de `commandes`, sans écrire le JSON à la main) ; le relancer après un changement du format ou du gabarit.

## Quand modifier ici

Import IFC ou Revit vers le modèle natif, export IFC, enregistrement atomique, rapport d'import. La lecture bas niveau des formats est dans `formats` et `ifc_base`.

## Vérifier

```
cargo test -p echanges --release
cargo tree -p echanges   # aucune crate interne hors noyau, modele, ifc_base, formats (+ commandes, maillage, moteur2d en dev)
cargo run --release -p bimedit -- --export-ifc samples/maison_native.bem target/maison.ifc
python3 tools/valider_ifc.py --donnees target/maison.ifc   # ifcopenshell==0.9.0.post1, pytest
```
