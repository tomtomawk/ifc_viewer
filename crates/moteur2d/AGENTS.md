# Crate `moteur2d`

Rôle : dessin 2D tiré de la scène triangulée : coupe par un plan (poché, hachures, contours), arêtes vues et cachées, motifs de coupe, outils de DAO (tracé, accrochage, saisie), rendu des vues et des feuilles en SVG et PDF.

Dépendances internes autorisées : `noyau`, `modele`, `scene`, `maillage` (`commandes` en dev-dependency seulement). Interdit : `egui`, `eframe`, `wgpu`, et toute crate située après (`moteur3d`, `echanges`, `collaboration`, `pilote`, `config`, `interface`). Couleurs en `[u8; 4]`, épaisseurs et constantes de trait dans `noyau::aretes`.

## Où travailler

| Fichier (`src/`) | Contenu |
|---|---|
| `lib.rs` | `Poche`, `TRAIT_FIN_MM`, `PAS_HACHURE_MM`, `decouper`, `section` ; ré-exports |
| `coupe.rs` | `coupe`, `coupe_avec_cache` et `CacheCoupe` (poché, hachures, contours ; coupe de chaque objet reprise tant que son empreinte ne change pas ; fusion des matériaux) |
| `vue.rs` | `vu` (arêtes vues et cachées), `sommets`, `sommets_boite` (sommets GPU des coupes), `cadrer_primitives` |
| `motifs.rs` | motif d'isolant : en plan d'un mur droit, le long de l'axe connu de la couche (`ligne_isolation_axe`, `CoucheCoupe::axe_isolation`) ; ailleurs (coupes verticales, éléments importés), d'après la forme de la région coupée (`ligne_isolation`), mur courbe d'après son centre |
| `dao.rs` | `Trace`, `OutilDessin`, `accrocher`, `lire_saisie`… |
| `feuilles.rs` | `dessin_vue`, `vue_placee`, `svg_feuille`, `pdf`, `pdf_jeu` |
| `dxf.rs` | `dxf` : export DXF R12 (LINE, ARC, CIRCLE, SOLID) |
| `dwg.rs` | `dwg` : DWG 2004 du DXF (relu et réécrit par `acadrust`, calques déclarés) |
| `plans.rs` | `fond_plan`, `limite_haute_plan`, `pointilles_plage_haute` |

Les points d'entrée sont ré-exportés à la racine (`moteur2d::coupe`, `moteur2d::svg_feuille`…) ; `dao` est un module public (`moteur2d::dao::Trace`). Tests dans le même fichier ou dans `tests.rs` (tests de `lib.rs`, `coupe.rs`, `vue.rs`). Un test qui a besoin d'`echanges` (ex. `depuis_ifc`) va dans la crate concernée, pas ici. Fichiers de 600 lignes au plus pour tout nouveau découpage ; `dao.rs` (795 lignes) ne doit pas grandir.

`coupe`, `vu`, `sommets_boite` et `aretes_plage_haute` reçoivent l'`Affichage` de la vue (niveau de détail et type : éléments des familles montrés) ; `feuilles::affichage_vue` le tire d'une vue native.

## Quand modifier ici

Coupe, poché, hachures, arêtes vues et cachées, sortie SVG/PDF des vues et feuilles, accrochage et tracé de DAO. Les écrans (vue en plan, feuille) sont dans `interface/src/documenter/` et `outils/dessiner/`.

## Vérifier

```
cargo test -p moteur2d --release
cargo tree -p moteur2d | grep -Ei "egui|wgpu|eframe"   # doit rester vide
```
