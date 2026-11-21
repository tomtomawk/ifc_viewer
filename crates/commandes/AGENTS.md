# Crate `commandes`

Rôle : commandes d'édition sérialisables (`Commande`, JSON `{"action": "..."}`) et leur exécution sur un `modele::Projet` ; historique, verrous, contextes (phases, sous-projets, variantes).

Dépendances internes autorisées : `noyau`, `modele` (`maillage` en dev-dependency seulement). Interdit : `egui`, `eframe`, `wgpu`, et toute crate située après (`moteur2d`, `echanges`, `collaboration`, `pilote`, `config`, `interface`).

## Où travailler

| Métier | Dossier (`src/`) | Contenu |
|---|---|---|
| Socle | `socle/` | `commande.rs` (`enum Commande`), `execution.rs` (`Execution::executer`, répartition par métier), `historique.rs` (piles d'annulation par différences, fusion des gestes, `modifications` : ids touchés par une étape ; tests dans `historique_tests.rs`), `differences.rs` (différence avant/après de deux états JSON, objets appariés par `id`), `report.rs` (sélection multiple, projet et familles : changement de l'élément principal reporté champ par champ sur ses pareils, `PROPRES` jamais reportés), `libelles.rs` (libellés des étapes), `verrous.rs`, `contextes.rs`, `objets.rs` |
| Modéliser | `modeliser/` | site, escaliers, garde-corps, pièces, composants (dont végétaux), barres, ouvertures, contours et trémies de dalle ; `fusion.rs` : fusion d'un projet importé (`Fusionner`) |
| Modifier | `modifier/` | `mod.rs` (déplacer, tourner, aligner, ajuster, supprimer), `supprimer.rs` (annotations détachées), `copier.rs`, `presse_papiers.rs` (instantané de sélection pour le collage entre projets), `composees.rs` (symétrie, échelle, réseau, décaler, scinder, démolir, reproduire), attachement des murs à plusieurs toitures, `transformations.rs`, `scinder_surface.rs` (découpe de surfaces et terrains par polyligne avec `i_overlay`) |
| Annoter | `annoter/` | quadrillages, annotations (cotes, textes, étiquettes, tracés 2D), niveaux ; `cotes.rs` (cotes pilotantes) |
| Documenter | `documenter/` | vues en coupe, feuilles, nomenclatures |

Il n'y a pas de dossier `dessiner/` : un tracé 2D est une annotation (`Commande::Annoter`). La création des murs, dalles et vues est dans `modele::creation`.

## Ajouter une commande

1. Ajouter la variante à `enum Commande` (`socle/commande.rs`) et la documenter dans `docs/EDITION.md`.
2. L'affecter à un métier dans le `match` de `executer_interne` (`socle/execution.rs`) : sans cela, la crate ne compile pas.
3. Écrire le bras dans `executer_<métier>` du dossier du métier (fichier de 600 lignes au plus ; sinon nouveau fichier dans le dossier).
4. Tester dans le même fichier (`#[cfg(test)]`). Un test qui a besoin d'`echanges`, de `maillage` ou du rendu va dans la crate concernée, pas ici.

Chaque fichier d'exécution commence par `use crate::socle::interne::*;` (imports communs). Aucun changement de schéma JSON sans mise à jour de `docs/EDITION.md` (voir `docs/AGENTS.md`).

## Quand modifier ici

Une commande d'édition (variante, exécution, historique, verrous, contextes). Son bouton ou son outil à la souris est dans `interface/src/outils/<métier>/` ; sa description pour le pilote dans `docs/EDITION.md`.

## Vérifier

```
cargo test -p commandes --release
cargo tree -p commandes | grep -Ei "egui|wgpu|eframe"   # doit rester vide
```
