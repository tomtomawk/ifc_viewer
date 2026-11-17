# Crate `modele`

Rôle : modèle natif du projet (source de vérité d'un projet) : données, schéma JSON (`Projet::json`, `Projet::lire_json`), validation (`Projet::verifier`), géométrie paramétrique (murs, éléments, volumes, familles) et données de documentation.

Dépendances internes autorisées : `noyau` seul. Externes : `earcutr`, `geo-straight-skeleton`, `glam`, `rayon`, `serde`, `serde_json`. Interdit : toute autre crate du workspace, egui, wgpu.

## Où travailler

| Fichier / dossier (`src/`) | Contenu |
|---|---|
| `lib.rs` | `Projet` et types du modèle, `FORMAT`, `VERSION`, lecture/écriture JSON |
| `creation.rs` | `ParametresMur`, `ParametresDalle`, `Implantation`, création des murs, dalles et vues |
| `murs/`, `elements/` | assemblage et jonctions des murs ; escaliers, garde-corps, pièces, site, terre-pleins, composants |
| `volumes/` | `solides`, `geometrie` : volumes des éléments (consommés par `maillage`) |
| `famille/` | familles paramétriques (`modelee`, `profil`, `edition`) |
| `documentation/` | données des vues, feuilles, cotes, nomenclatures |
| `liens.rs` | `LienIfc` : fichier IFC lié (chemin, décalage, rotation, visibilité) et sa transformation vers le projet |
| `gabarit.rs`, `gabarits_vues.rs`, `contextes.rs`, `unite.rs`, `identifiants.rs`, `validation.rs`, `soleil.rs` | gabarit par défaut (`gabarits/defaut.gabarit.json`), vues, filtres de contexte, unité d'affichage, `id_natif`, vérification, soleil |

## Quand modifier ici

Un champ ou une règle du fichier de projet, une géométrie d'élément, une validation. Un changement de schéma impose de mettre à jour `docs/MODELE.md` et de suivre `docs/AGENTS.md` (pas de migration). Les commandes d'édition sont dans `commandes`, pas ici.

## Pièges

- Les méthodes de `Projet` qui exécutent des commandes sont des traits d'extension de `commandes` (`Execution`, `Verrous`, `Contextes` dans `socle/`, `Modification` dans `modifier/composees.rs`).
- `include_str!` du gabarit : chemin relatif `../../../gabarits/` depuis `src/`.
- `documentation/mod.rs` (1 200 lignes), `famille/modelee.rs`, `lib.rs`, `creation.rs`, `elements/terre_plein.rs`, `volumes/solides.rs` dépassent 600 lignes : ne pas les agrandir.

## Vérifier

```
cargo test -p modele --release
cargo tree -p modele | grep -Ei "egui|wgpu|eframe"   # doit rester vide
```
