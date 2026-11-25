# Crate `modele`

Rôle : modèle natif du projet (source de vérité d'un projet) : données, schéma JSON (`Projet::json`, `Projet::lire_json`), validation (`Projet::verifier`), géométrie paramétrique (murs, éléments, volumes, familles) et données de documentation.

Dépendances internes autorisées : `noyau` seul. Externes : `earcutr`, `geo-straight-skeleton`, `glam`, `rayon`, `serde`, `serde_json`. Interdit : toute autre crate du workspace, egui, wgpu.

## Où travailler

| Fichier / dossier (`src/`) | Contenu |
|---|---|
| `lib.rs` | `Projet` et types du modèle, `FORMAT`, `VERSION`, lecture/écriture JSON |
| `importes.rs` | `DonneesImportees` (à plat dans `ElementImporte`, champ `donnees` de `Piece`) : propriétés, matériau et couleur lus dans la source, vérification |
| `placement_surface.rs` | repère du motif de modèle et des textures sur une face plane |
| `creation.rs` | `ParametresMur`, `ParametresDalle`, `Implantation`, création des murs, dalles et vues |
| `murs/`, `elements/` | assemblage et jonctions des murs ; profil d'élévation des murs (`murs/profil.rs` : `ProfilMur`, volume, recalage) ; profilés de mur en relief ou en creux (`elements/modenature.rs` : `Modenature`, `TypeModenature`, balayage le long d'une face, creux retirés des murs) ; escaliers, garde-corps, pièces, site, terre-pleins, composants ; végétation (`vegetation.rs` : familles d'arbres par défaut, références `Rendu` ; `generateur_vegetal.rs` : essences et géométrie de rendu générée ; `dessin_vegetal.rs` : houppier, tronc et symbole de plan des familles d'arbres) |
| `volumes/` | `solides`, `geometrie` : volumes des éléments (consommés par `maillage`) ; `attache_toiture` : volume des murs sous plusieurs toitures et hors de leurs emprises, référence des toitures au-dessus à l’égout |
| `famille/` | familles modélisées (`modelee` ; `modelee/catalogue.rs` : familles livrées, dont `ouverture_libre` et `par_defaut(genre)`), `profil`, `edition` (`type_ouverture_par_defaut`) ; `Role` des pièces de menuiserie ; `modelee/orientation.rs` (`PlanTourne`, `Repere`, `SupportForme` : plans de référence tournés autour d'une droite d'appui, enchaînables, cote angulaire, plan par trois points, support des formes) ; `modelee/visibilite.rs` (`NiveauDetail`, `TypeVue`, `Affichage`, `VisibiliteElement` : visibilité des éléments par niveau de détail et type de vue), `modelee/guidage.rs` (schéma des formes guidées), `balayage.rs` (chemin, arcs, onglets), `raccord.rs` (correspondance des profils, capuchons) |
| `documentation/` | données des vues, feuilles, cotes, nomenclatures ; `habillage_feuille.rs` : cartouches posés (`CartouchePlace`) et annotations des feuilles (`AnnotationFeuille`) ; `symboles.rs` : symboles d'annotation tirés de familles (têtes de niveau et de quadrillage, repères de coupe et d'élévation : `FamilleSymbole`, `SymbolesVues`, `primitives_symbole`) |
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
