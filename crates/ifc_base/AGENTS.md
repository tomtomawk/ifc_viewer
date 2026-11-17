# Crate `ifc_base`

Rôle : socle IFC : parseur STEP (`step`) et couche sémantique (`semantique` : produits, niveaux, matériaux, noms de types), sans géométrie ni scène.

Dépendances internes autorisées : `noyau` seul. Externe : `rayon`. Interdit : `modele`, `scene`, `formats` et toute crate après, egui, wgpu.

## Où travailler

| Fichier (`src/`) | Contenu |
|---|---|
| `step.rs` | `StepFile`, `Entity`, `Value`, `IdMap` : lecture STEP |
| `semantique.rs` | `IfcModel`, `pretty_type` et lecture des relations IFC ; unités (`length_scale`, `area_scale`, `volume_scale`, `angle_scale`) |
| `proprietes.rs` | `IfcModel::jeux` (propriétés et quantités structurées : valeur, unité SI de base) et `property_groups` (affichage) |

## Quand modifier ici

Lecture d'une entité ou d'une relation IFC indépendante de la géométrie. La conversion géométrique et `load`/`assemble` sont dans `formats::ifc` ; l'import vers le modèle natif dans `echanges::depuis_ifc`.

## Pièges

- `scene::Scene` contient un `IfcModel` et un `IdMap` : tout changement de leur forme se répercute sur `scene`, `formats`, `maillage`.
- `step.rs` : l'imbrication d'une valeur est bornée (`PROFONDEUR_MAX`) ; une instance au-delà est écartée.
- Pas de ré-export : `formats::ifc` ne ré-exporte plus `step` ni `semantique`.

## Vérifier

```
cargo test -p ifc_base --release
```
