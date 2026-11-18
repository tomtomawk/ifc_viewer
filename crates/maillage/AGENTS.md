# Crate `maillage`

Rôle : adaptation du modèle natif vers la scène triangulée : `depuis_natif` et `depuis_natif_verifie` (`Projet` -> `scene::Scene`), arêtes, matériaux et navigateur de la scène native.

Dépendances internes autorisées : `noyau`, `modele`, `ifc_base`, `scene`, `formats` (exactement ces cinq). Externes : `glam`, `rayon`. Interdit : `commandes` et toute crate après (en dev-dependency seulement pour les tests de ses utilisateurs), egui, wgpu.

## Où travailler

| Fichier (`src/`) | Contenu |
|---|---|
| `lib.rs` | `depuis_natif`, `depuis_natif_verifie` : objets de scène, `IfcModel` et `StepFile` minimaux (schéma « Modèle natif v0 »), traits |
| `aretes.rs` | arêtes vives, silhouettes, terrain (`aretes_jointes`, `aretes_silhouette`) |
| `materiaux.rs` | `rendu_materiau`, `coupe_materiau`, `motif_surface`, `opacite` |
| `navigateur.rs` | `navigateur` : arborescence du projet pour la scène |
| `liens.rs` | liens IFC : `charger` (lecture et cache par chemin, date, taille), placement dans la scène, rattachement aux niveaux ; arêtes calculées une fois par fichier |

`tests/` : `creation.rs`, `elements_terre_plein.rs` ; `src/tests.rs` pour les tests unitaires.

## Quand modifier ici

Ce que l'on voit d'un élément du modèle dans la scène (couleurs, matériaux, arêtes, objets). Les parties des familles (ouvertures, composants) portent leur masque de visibilité (`ProductGeom::visibilite`) ; leurs arêtes sont alors calculées par masque à l'assemblage, pas jointes aux autres éléments. Les volumes eux-mêmes restent dans `modele::volumes`.

## Pièges

- `id_natif` et `numero_vue` sont dans `modele::identifiants`, pas ici.
- `maillage` est en dev-dependency de `commandes`, `moteur3d`, `echanges` : une dépendance de production inverse créerait un cycle.

## Vérifier

```
cargo test -p maillage --release
```
