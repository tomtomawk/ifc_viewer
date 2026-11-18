# Crate `formats`

Rôle : lecture des formats externes vers la scène : IFC (géométrie des produits, assemblage de la `Scene`) et Revit (`.rvt/.rfa/.rte` : OLE, maquette, vues, nomenclatures, paramètres).

Dépendances internes autorisées : `noyau`, `ifc_base`, `scene` (exactement ces trois). Externes : `earcutr`, `glam`, `miniz_oxide`, `png`, `rayon`, `serde_json`. Interdit : `modele` en direct, `maillage` et toute crate après, egui, wgpu.

## Où travailler

| Fichier (`src/`) | Contenu |
|---|---|
| `ifc/geom.rs` | `ProductGeom`, conversion géométrique des produits IFC |
| `ifc/geom/balayage.rs` | balayages le long d'une directrice (`IfcSurfaceCurveSweptAreaSolid`, `IfcFixedReferenceSweptAreaSolid`, `IfcSectionedSolidHorizontal`) |
| `ifc/geom/nurbs.rs` | faces B-spline des `IfcAdvancedBrep` échantillonnées ; `ifc/geom/essais.rs` : fichier IFC de test construit en chaîne |
| `ifc/geom/extrusion.rs` | `rectangulaire` : section rectangulaire extrudée d'un produit (poteaux, poutres convertibles) |
| `ifc/bornes.rs` | bornes des valeurs lues dans le fichier (arcs, B-splines, révolutions, items mappés) : toute taille ou durée tirée du fichier passe par là |
| `ifc/assemblage.rs` | `load`, `assemble`, `assemble_avec` : fichier IFC -> `Scene` ; masques de visibilité des parties (`ProductGeom::visibilite`) reportés sur les sommets, arêtes calculées par masque |
| `rvt/mod.rs` | `load`, `is_revit_path`, lecture OLE et métadonnées (`RvtInfo`) |
| `rvt/maquette/` | `lecture.rs` (`lire` -> `Maquette`), `vers_scene.rs` (`load_scene`), `tests.rs` |
| `rvt/{geometrie,vues,feuilles,nomenclatures,parametres,formules,coordonnees,champs,annexes,medias,objets,plans,schema,zones}.rs` | une partie du décodage Revit par fichier |

## Quand modifier ici

Géométrie d'un produit IFC ou décodage d'une donnée Revit. La conversion vers le modèle natif (`Projet`) est dans `echanges`, pas ici ; un test qui en a besoin va dans `echanges`.

## Pièges

- Fichiers de plus de 600 lignes à ne pas agrandir : `ifc/geom.rs`, `rvt/maquette/{lecture,mod,vers_scene,tests}.rs`, `rvt/geometrie.rs`, `rvt/mod.rs`, `rvt/vues.rs`, `rvt/parametres.rs`.
- `crates/echanges/tests/couches_rvt.rs` lit un fichier Revit hors dépôt (`RVT_ESSAIS`) : sauté s'il est absent.
- Fichier malformé : jamais de boucle, d'allocation ni de récursion dont la taille vient du fichier sans borne (`ifc/bornes.rs`, `rvt/objets.rs::inflate`).
- Pas de ré-export : `ifc_base::step` et `semantique` s'importent depuis `ifc_base`.

## Vérifier

```
cargo test -p formats --release
cargo run --release -p bimedit -- --stats samples/test_ifc4.ifc
```
