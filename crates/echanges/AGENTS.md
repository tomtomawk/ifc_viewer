# Crate `echanges`

Rôle : entrées/sorties du modèle natif : import IFC et Revit vers `Projet`, export IFC, enregistrement natif atomique et récupération, rapport d'import.

Dépendances internes autorisées : `noyau`, `modele`, `ifc_base`, `formats` (dev seulement : `commandes`, `maillage`, `moteur2d`). Interdit : `moteur2d`/`moteur3d` en production, `config`, `collaboration`, `pilote`, `interface`, egui, wgpu, eframe.

## Où travailler

| Fichier (`src/`) | Contenu |
|---|---|
| `depuis_ifc.rs` | `convertir` : IFC -> `Projet` |
| `depuis_rvt.rs` | `convertir`, `convertir_maquette` : Revit -> `Projet` (575 lignes : ne pas agrandir) |
| `export_ifc.rs` | `exporter` : `Projet` -> IFC |
| `enregistrement.rs` | `enregistrer_natif`, `ecrire_atomique`, récupération, `SourceRvt`, `Bilan` |
| `rapport.rs` | `markdown` |

`tests/` : tests d'intégration dont le projet de départ vient de `depuis_ifc` (commandes, documentation, site, escalier, volumes) ; `couches_rvt.rs` lit un fichier hors dépôt (`RVT_ESSAIS`) et est sauté s'il est absent. Les `samples/` se lisent par `concat!(env!("CARGO_MANIFEST_DIR"), "/../../samples/...")`.

## Quand modifier ici

Import IFC ou Revit vers le modèle natif, export IFC, enregistrement atomique, rapport d'import. La lecture bas niveau des formats est dans `formats` et `ifc_base`.

## Vérifier

```
cargo test -p echanges --release
cargo tree -p echanges   # aucune crate interne hors noyau, modele, ifc_base, formats (+ commandes, maillage, moteur2d en dev)
```
