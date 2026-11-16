# Architecture : découpage en workspace Cargo

État après le lot 9 (§5.16) : le dépôt est un workspace (`Cargo.toml` racine, `resolver = "3"`, `members = ["crates/*"]`). `noyau` (lot 1), `modele` (lot 2), `ifc_base` et `scene` (lot 3), `formats` (lot 4), `maillage` (lot 5), `commandes` (lot 6), `moteur2d` (lot 7), `moteur3d` (lot 8), `echanges` et `collaboration` (lot 9) sont extraits en `crates/noyau`, `crates/modele`, `crates/ifc_base`, `crates/scene`, `crates/formats`, `crates/maillage`, `crates/commandes`, `crates/moteur2d`, `crates/moteur3d`, `crates/echanges` et `crates/collaboration` ; le reste du code est encore dans une seule crate, `crates/app` (package et binaire `bimedit`). Les dépendances sont déclarées une fois dans `[workspace.dependencies]` (racine) et reprises par `dep.workspace = true`. `Cargo.lock` et `target/` restent à la racine.

Restent à la racine : `docs/`, `samples/`, `gabarits/`, `tools/`, `.github/`, `.mcp.json`, `README.md`, `AGENTS.md`. Les ressources embarquées (`include_str!`) pointent vers ces dossiers par des chemins relatifs depuis `crates/<crate>/src/...` (le gabarit par défaut est embarqué par `crates/modele/src/gabarit.rs`) ; les tests lisent `samples/` via `concat!(env!("CARGO_MANIFEST_DIR"), "/../../samples/...")` (le répertoire courant d'un test est celui de sa crate ; même profondeur pour `crates/app` et `crates/modele`).

## 1. Arborescence cible

```
crates/
  noyau/          géométrie, CSG, texte, primitives 2D ; aucune dépendance interne
  modele/         données, schéma JSON, validation : murs, elements, famille, documentation
  ifc_base/       parseur STEP (`step`) et couche sémantique IFC (`semantique`) ; dépend de noyau seul
  scene/          scène triangulée, navigateur, picking (contient Vec<modele::Apparence>, ifc_base::IfcModel, IdMap)
  formats/        ifc (geom, assemblage : load/assemble), rvt
  maillage/       modèle -> scène : depuis_natif, depuis_natif_verifie (volumes et jonctions restent dans modele)
  commandes/      `Commande` et son exécution ; socle/ (exécution, historique, verrous, contextes, objets)
                  modeliser/ modifier/ annoter/ documenter/ (pas de dessiner/ : §5.13 a)
  moteur2d/       coupe, arêtes vues/cachées, poché, motifs, outils de DAO, rendu de vue et de feuille SVG/PDF
  moteur3d/       wgpu, shaders, caméra, ciel, sélection à l'écran
  echanges/       import IFC/RVT, export IFC, enregistrement, rapport
  collaboration/  transactions partagées : session, stockage, concurrence, partage, texte (yrs), cli
  pilote/         MCP, CLI, registre des instances, saisie (egui::Event) ; mcp.rs ~600 lignes
  config/
  interface/      egui ; UNE crate (`impl ViewerApp` réparti sur 48 fichiers) ;
                  socle/ zones/ saisie/ outils/{modeliser,modifier,dessiner,annoter}/ documents/ familles/ projet/
  app/            binaire `bimedit`
```

Correspondance avec les dossiers actuels (après lot 10) : `crates/noyau`, `crates/modele`, `crates/ifc_base`, `crates/scene`, `crates/formats`, `crates/maillage`, `crates/commandes`, `crates/moteur2d`, `crates/moteur3d`, `crates/echanges`, `crates/collaboration`, `crates/pilote` : extraits ; dans `crates/app/src/` : `config/` ; `ui/` -> interface ; `main.rs` -> app ; `modele_tests/` : 17 tests parqués, accueil commandes ou moteur2d (§5.16 d) ; `formats_tests/` et `commandes_tests/` n'existent plus.

## 2. Graphe de dépendances autorisées

Ordre cible (flèche = « dépend de ») ; `ifc_base` ajoutée au lot 3 (option B du §5.9) :

```
noyau <- modele <- ifc_base <- scene <- formats <- maillage <- commandes <- moteur2d <- moteur3d
      <- echanges <- collaboration <- pilote <- config <- interface <- app
```

| Crate | Dépendances internes autorisées |
|---|---|
| noyau | aucune |
| modele | noyau |
| ifc_base | noyau |
| scene | noyau, modele, ifc_base |
| formats | noyau, ifc_base, scene (réel : exactement ces trois ; `modele` n'est pas une dépendance directe, il n'arrive que par `scene`, §5.11) |
| maillage | noyau, modele, ifc_base, scene, formats (réel : exactement ces cinq, toutes utilisées directement, §5.12) |
| commandes | noyau, modele, scene, maillage (réel : `noyau` et `modele` seuls ; `scene` et `maillage` restent autorisées mais ne sont pas des dépendances, §5.13) |
| moteur2d | noyau, modele, scene, maillage, commandes (réel : `noyau`, `modele`, `scene`, `maillage` ; `commandes` reste autorisée sans être utilisée ; ni `formats` ni `ifc_base` ne sont nécessaires, §5.14) |
| moteur3d | noyau, modele, scene, moteur2d (réel : `noyau`, `modele`, `scene` ; `moteur2d` reste autorisée sans être utilisée ; `maillage` en dev-dependency pour un test ; externes : `eframe` (egui, egui_wgpu, wgpu), `glam`, `image`, `earcutr`, §5.15) |
| echanges | noyau, modele, scene, formats, maillage (réel : `noyau`, `modele`, `ifc_base`, `formats` ; `scene` et `maillage` restent autorisées sans être utilisées ; dev : `commandes`, `maillage`, `moteur2d` pour les tests d'intégration ; externes : `glam`, `rayon`, §5.16) |
| collaboration | noyau, modele, scene, maillage, commandes, echanges (réel : `noyau`, `modele`, `maillage`, `commandes`, `echanges` ; `scene` reste autorisée sans être utilisée ; externes : `yrs`, `serde`, `serde_json`, `glam`, §5.16) |
| pilote | collaboration et en dessous, hors `config` et `ui` (réel : `noyau`, `modele`, `maillage`, `commandes`, `moteur2d`, `echanges`, `collaboration` ; `scene`, `formats`, `ifc_base`, `moteur3d` non utilisées ; externes : `eframe` (egui seul : `egui::Event` dans `saisie.rs`, `egui::Context` dans `lib.rs`), `png`, `serde`, `serde_json`, `serde_ignored`, `uuid`, §5.17) |
| config | tout ce qui précède, hors interface |
| interface | tout ce qui précède |
| app | interface (et config, pilote pour le démarrage) |

`ifc_base` est autorisée pour toute crate située après elle qui manipule `StepFile`, `IfcModel`, `Value`, `Entity` ou `IdMap` (aujourd'hui : formats, maillage, echanges, interface, app ; relevé au §5.10 e) ; les lignes ci-dessus à partir de commandes seront complétées au lot de chaque crate.

Règle générale : une crate n'importe que des crates situées avant elle dans l'ordre. Aucun cycle. Les dépendances externes (wgpu, egui, etc.) suivent : `wgpu` uniquement dans moteur3d ; `egui`/`eframe` seulement à partir de moteur3d (voir §5.4). L'ordre final est confirmé ; les coupes qu'il impose sont planifiées au §4 (écarts au §5.5). `volumes` reste dans `modele`, donc `maillage` ne contient que l'ancien `rendu/natif.rs`.

## 3. Règles

1. Un lot = une crate = une session.
2. Deux commits par lot : (a) coupe des cycles dans la crate courante, sans déplacer de fichier ; (b) `git mv` + `Cargo.toml` + remplacement de `crate::<nom>::` par `<nom>::`.
3. Porte de sortie d'un lot : `cargo build --workspace` et `cargo test --workspace --release` verts, avec le même nombre de tests qu'avant le lot (référence au lot 0 : 390 réussis, 2 ignorés).
4. Aucun changement fonctionnel ni de format (voir `docs/AGENTS.md` : pas de migration ni de compatibilité des formats natifs).
5. `pub(crate)` devient `pub` seulement si la compilation l'exige.
6. Lots séquentiels, jamais en parallèle.
7. Plafond d'environ 600 lignes par fichier pour les nouveaux découpages.
8. Un `AGENTS.md` court par crate (rôle, dépendances autorisées, points d'entrée, commande de test ciblée), écrit au lot 12.

## 4. Avancement

Plan révisé après l'inventaire du §5 (validé par l'utilisateur). L'ordre final des crates est inchangé ; ce qui change, c'est le contenu des lots 1 à 6 : les coupes de cycles se font **avant** chaque extraction, dans la crate courante (commit a), puis le déplacement (commit b). Orchestration : Sonnet 5.5 ; lots marqués « Fable » confiés à un agent Fable 5.2. **Arrêt obligatoire après le lot 2 pour validation des frontières par l'utilisateur.**

Code « parqué » : quand une coupe fait monter du code vers une crate qui n'existe pas encore (ex. rendu de feuille de `documentation` vers moteur2d), il reste dans `crates/app` jusqu'au lot de sa crate d'accueil.

| Lot | Crate | Coupes préalables (commit a) | Modèle | État |
|---|---|---|---|---|
| 0 | workspace | chemins, CI, inventaire | Sonnet | **fait** (`242681c`) |
| 1 | noyau | Descendre dans `noyau` : `Id` (= `u64`, neutre), `uuid`, `hash` (+ `empreinte`, `encoder`, dont `hash` dépend ; de `collaboration`), `Forme2D` + `Geom`, `geom_segment`, `arc_3_points`, `arc_courbure`, `angle_norm`, `segments_polyligne`, `points_segment`, `points_arc`, `points_spline`, `developper` (de `dessin/dao.rs` ; `Geom` et les `points_*` descendent avec `Forme2D` qui en dépend ; les outils de tracé et d'accrochage restent dans `dessin`), `Aretes`/`aretes`/`aretes_jointes`/`chainer`/`Polyligne`/`PAS`/`TRAIT_FORT_MM`/`NOIR` (de `dessin`), `MAX_APPARENCES` (de `rendu::gpu`), `chrono` (de `rendu::scene`). Si un symbole dépend de plus que `noyau`, s'arrêter et le signaler. §5.5 points 5, 6, 7, 8 (partie `Id`) | Sonnet | **fait** (coupes `dee72ad`, `a8bb20b`, `7746a1e`, `9b18b34` ; extraction `06f1578`) |
| 2 | modele | `config::gabarit` sous `modele` ; descendre dans `modele` : `contextes`, `gabarits_vues`, `ParametresMur`, `Implantation`, `contour_valide` (et `Commande` si les références de production l'exigent : à trancher et documenter) ; `Motif`/`GrilleMotif`/`hachures_plan` ; `volumes` et `murs/jonctions` **restent** dans `modele` ; remplacer `scene::chrono` ; monter hors de `modele` le rendu de feuille de `documentation` (`coupe`, `vu`, `cadrer_primitives`, `Poche`, ~l. 1177-1213) et les tests appelant `depuis_natif`, `export_ifc`, `ifc::*`. §5.5 points 2, 3, 4, 6, 9 | **Fable** | **fait** (coupes `8c74f6e`, `ef292b4`, `fe4551a`, `32e14b4`, `f82b294`, `e64d176`, `5dd76f1` ; extraction `501e119` ; décisions au §5.8) |
| 3 | ifc_base + scene | `load`, `assemble`, `assemble_avec` (constructeurs IFC) vont dans `formats` ; plus de `formats`/`dessin` dans `scene`. §5.5 point 1 | **Fable** | **fait** (coupe `a069497` ; extraction de `ifc_base` `b698a58` ; extraction de `scene` `a114732` ; option B du §5.9, détail au §5.10) |
| 4 | formats | extraction de `ifc/` (`geom`, `assemblage`), `rvt/` ; tests croisés montés ; retrait du ré-export `formats::ifc::{step, semantique}` (§5.10 b) | Sonnet | **fait** (coupes `9e2ada7`, `7c1fa60`, `d8b818a` ; extraction `aff8d40` ; détail au §5.11) |
| 5 | maillage | `rendu/natif.rs` seul (`depuis_natif`, `depuis_natif_verifie` ; `id_natif` et `numero_vue` sont dans `modele::identifiants` depuis le lot 2) ; les tests de `modele_tests`/`dessin` qui l'appellent restent parqués dans `app` (§5.12 d) | **Fable** | **fait** (coupe `f5c528d` ; extraction `525e9cb` ; détail au §5.12) |
| 6 | commandes | `crates/app/src/edition` (commandes, historique, verrous, gestion des contextes ; traits d'extension de `Projet`, §5.8 b) ; décider où vont `concurrence` et `objets` (§5.5 point 5) ; rangement `modeliser/ modifier/ dessiner/ annoter/ documenter/` | **Fable** | **fait** (coupes `90be806` ; extraction `8f9e763` ; rangement par métier `0682940` ; détail au §5.13) |
| 7 | moteur2d | `dessin/` dont le rendu de vue et de feuille parqué (`dessin/feuilles.rs`, `dessin/plans.rs`) | **Fable** | **fait** (aucune coupe nécessaire ; extraction `22e2c46` ; découpage de `lib.rs` `6405175` ; détail au §5.14) |
| 8 | moteur3d | `rendu/` hors scène | Sonnet | **fait** (aucune coupe nécessaire ; extraction `d2686d2` ; `gpu.rs` non découpé ; détail au §5.15) |
| 9 | echanges + collaboration | `crates/app/src/echanges`, `crates/app/src/collaboration*` | Sonnet | **fait** (aucune coupe préalable nécessaire ; extraction de `echanges` `e72f601` ; extraction de `collaboration` `35fab15` ; détail au §5.16) |
| 10 | pilote | `pilote/` | Sonnet | **fait** (aucune coupe préalable nécessaire ; extraction `5424089` ; détail au §5.17) |
| 11 | config + interface | rangement interne `socle/ zones/ saisie/ outils/... ` ; `cadre_natif` et `arriere_plan` de `main.rs` | **Fable** | à faire |
| 12 | app + docs | binaire, `AGENTS.md` par crate, README | Sonnet | à faire |

## 5. Inventaire des imports (état du lot 0)

Méthode : script de scan de tous les `.rs` de `crates/app/src` (hors lignes de commentaire `//`) ; chaque occurrence `crate::...` est résolue, accolades `use crate::x::{a, b}` développées, vers le module cible. Les tests sont comptés à part : « dont tests » = fichiers `tests.rs` / `tests_*.rs` ou bloc `#[cfg(test)] mod x { ... }` en ligne. Limites : les références relatives (`super::`, `self::`, `use {edition::...}` dans `modele/mod.rs`) ne sont pas dans les tableaux 5.1 et 5.2 ; celles qui franchissent un sous-dossier de `modele` sont relevées au §5.3. Une référence via un alias de module (`use crate::rendu::scene; scene::id_natif(...)`) compte pour le module, pas pour le symbole ; les comptes par symbole (§5.6) viennent donc d'une recherche par mot et non de ce scan.

### 5.1 Imports `crate::X` par module (hors références internes au module)

Unités : `modele(racine)` = `modele/mod.rs` et `soleil.rs` ; `rendu(autres)` = `rendu/` sans `scene/` ; `rendu/scene/mod` = `scene/mod.rs` ; `rendu/scene/natif` = `scene/natif.rs`.

| Source | Importe (nombre de références ; entre parenthèses, dont tests) |
|---|---|
| `noyau` | `formats` 1 |
| `formats` | `noyau` 12 ; `rendu/scene/mod` 6 ; `modele(racine)` 4 (4) ; `modele/echanges` 1 (1) ; `dessin` 1 |
| `modele(racine)` | `config` 6 |
| `modele/murs` | `modele/volumes` 14 (6) ; `modele(racine)` 12 (5) ; `modele/edition` 6 (6) ; `modele/famille` 6 (3) ; `config` 3 (2) ; `dessin` 1 ; `formats` 1 |
| `modele/elements` | `modele(racine)` 28 (16) ; `modele/edition` 8 (5) ; `noyau` 6 (2) ; `modele/documentation` 6 (3) ; `rendu/scene/natif` 6 (6) ; `modele/volumes` 5 (4) ; `modele/famille` 4 (2) ; `formats` 4 (4) ; `modele/echanges` 2 (2) ; `modele/murs` 2 |
| `modele/volumes` | `modele/murs` 13 (3) ; `noyau` 11 ; `modele/famille` 11 ; `rendu/scene/natif` 5 (1) ; `modele/documentation` 4 ; `dessin` 3 ; `modele(racine)` 1 ; `modele/edition` 1 (1) |
| `modele/famille` | `modele(racine)` 19 (2) ; `modele/documentation` 11 (1) ; `modele/volumes` 4 (1) ; `config` 4 (4) ; `modele/collaboration` 3 (2) |
| `modele/edition` | `modele(racine)` 23 (15) ; `modele/murs` 7 ; `modele/collaboration` 6 ; `rendu/scene/natif` 6 (6) ; `modele/famille` 5 (4) ; `modele/documentation` 3 (2) ; `config` 3 (3) ; `modele/elements` 3 ; `dessin` 1 ; `modele/volumes` 1 |
| `modele/documentation` | `modele/famille` 19 (3) ; `dessin` 18 ; `rendu/scene/natif` 15 (6) ; `noyau` 14 ; `modele/edition` 9 (7) ; `modele(racine)` 7 (3) ; `config` 5 (2) ; `formats` 4 ; `modele/volumes` 2 ; `modele/collaboration` 1 ; `rendu/scene/mod` 1 ; `modele/elements` 1 |
| `modele/echanges` | `modele(racine)` 47 (1) ; `formats` 21 (5) ; `modele/volumes` 8 ; `modele/documentation` 5 ; `modele/collaboration` 1 |
| `modele/collaboration` | `config` 2 (2) ; `rendu/scene/natif` 2 |
| `dessin` | `modele(racine)` 18 (18) ; `noyau` 14 (3) ; `rendu/scene/mod` 7 (4) ; `rendu/scene/natif` 6 (6) ; `formats` 5 ; `modele/edition` 5 (5) ; `modele/murs` 1 (1) ; `config` 1 (1) ; `modele/volumes` 1 (1) |
| `rendu(autres)` | `modele(racine)` 8 (6) ; `rendu/scene/mod` 2 ; `rendu/scene/natif` 1 (1) |
| `rendu/scene/mod` | `formats` 10 ; `dessin` 5 ; `modele(racine)` 2 ; `noyau` 1 |
| `rendu/scene/natif` | `modele(racine)` 26 (3) ; `modele/murs` 15 ; `dessin` 12 ; `modele/elements` 11 (1) ; `formats` 7 ; `modele/volumes` 6 ; `modele/famille` 1 ; `rendu/scene/mod` 1 ; `rendu(autres)` 1 ; `modele/documentation` 1 ; `modele/edition` 1 (1) |
| `pilote` | `modele(racine)` 8 (3) ; `rendu/scene/natif` 3 ; `modele/collaboration` 2 (1) ; `config` 1 (1) |
| `config` | `modele(racine)` 10 ; `modele/famille` 3 ; `formats` 3 ; `modele/documentation` 2 (1) ; `modele/collaboration` 2 ; `modele/elements` 1 ; `modele/echanges` 1 |
| `ui` | `modele(racine)` 283 (28) ; `config` 132 (14) ; `modele/documentation` 98 (3) ; `modele/famille` 82 (1) ; `modele/edition` 63 (6) ; `dessin` 56 (5) ; `modele/echanges` 43 ; `rendu(autres)` 37 ; `modele/elements` 35 (3) ; `modele/collaboration` 28 ; `noyau` 26 (6) ; `formats` 22 (1) ; `modele/murs` 17 (2) ; `modele/volumes` 14 ; `rendu/scene/natif` 6 ; `rendu/scene/mod` 4 ; `pilote` 2 ; `crate::cadre_natif` 2 ; `crate::arriere_plan` 2 |

`crate::cadre_natif` et `crate::arriere_plan` sont deux fonctions de `main.rs`, appelées par `ui/barre_haute.rs` et `ui/volets.rs` : elles devront passer en paramètre ou dans `config`/`interface` au lot de l'interface.

### 5.2 Imports qui violent le sens de l'ordre cible

Correspondance utilisée pour classer : noyau = `noyau` ; modele = `modele(racine)`, `murs`, `elements`, `famille`, `documentation` ; scene = `rendu/scene/mod` ; maillage = `rendu/scene/natif` et `modele/volumes` ; commandes = `modele/edition` ; moteur2d = `dessin` ; moteur3d = `rendu(autres)` ; echanges ; collaboration ; etc. Une violation est un import d'une crate située plus haut dans l'ordre. « prod / test » : références hors tests / dans les tests.

| Source (crate cible) | Importe (crate cible) | prod / test | Symboles |
|---|---|---|---|
| noyau | formats | 1 / 0 | `formats::ifc::step::Id` (`noyau/trace2d.rs`) ; `Id` est `pub type Id = u64` |
| `rendu/scene/mod` (scene) | formats | 10 / 0 | `ifc::step::{Id, IdMap, StepFile}`, `ifc::semantique::IfcModel`, `ifc::geom::{GeomStats, ProductGeom}`, `rvt::vues::Motif` : constructeurs IFC `load`, `assemble`, `assemble_avec` |
| `rendu/scene/mod` (scene) | moteur2d | 5 / 0 | `dessin::{Aretes, aretes, NOIR}` |
| formats | scene (sens autorisé en cible, mais cycle avec scene -> formats) | 6 / 0 | `NavFenetre` (`rvt/feuilles.rs`) ; `Scene`, `Navigateur`, `NavVue`, `NavFamille`, `NavMateriau`, `NavFenetre` (`rvt/maquette/mod.rs` et `vers_scene.rs`, qui construit la `Scene` d'un fichier Revit) |
| formats | moteur2d | 1 / 0 | `dessin::TRAIT_FORT_MM` |
| formats | echanges | 0 / 1 | `depuis_rvt::convertir_maquette` (test) |
| `rendu/scene/natif` (maillage) | moteur2d | 12 / 0 | `dessin::{Aretes, TRAIT_FORT_MM, aretes_jointes}` |
| `rendu/scene/natif` (maillage) | moteur3d | 1 / 0 | `rendu::gpu::MAX_APPARENCES` |
| `rendu/scene/natif` (maillage) | commandes | 0 / 1 | `edition::ParametresMur` (test) |
| `modele/volumes` (idem) | moteur2d | 3 / 0 | `dessin::{aretes, chainer}` (`volumes/geometrie.rs`) |
| `modele/volumes` (si laissé dans modele, §5.5.2) | maillage (`scene/natif.rs`) | 4 / 1 | `rendu::scene::chrono` (chronométrage, `solides.rs`) ; `depuis_natif` (test) |
| `modele/volumes` (idem) | commandes | 0 / 1 | `Commande::Deplacer` (test) |
| modele(racine) | config | 6 / 0 | `config::gabarit::{actif, Gabarit}` |
| `modele/murs` | maillage (`modele/volumes`) | 8 / 6 | `volumes::solides::{mur, hote, repere, prisme, Tri}`, `volumes::geometrie::volume` |
| `modele/murs` | commandes | 0 / 6 | `edition::{ParametresMur, Implantation, commandes::Commande}` (tests) |
| `modele/murs` | config, moteur2d, formats | 1 / 2 ; 1 / 0 ; 1 / 0 | `config::gabarit::{actif, Gabarit::defaut, familles_livrees}` ; `dessin::dao::arc_3_points` ; `formats::ifc::step::format_real` |
| `modele/elements` | commandes | 3 / 5 | `edition::{ParametresMur, contour_valide, commandes::Commande}` |
| `modele/elements` | maillage | 1 / 4 (volumes) ; 0 / 6 (natif) | `volumes::{geometrie::volume, solides::prisme}` ; `depuis_natif`, `id_natif` (tests) |
| `modele/elements` | formats, echanges | 0 / 4 ; 0 / 2 | `ifc::{step, semantique, geom}` ; `export_ifc::exporter` (tests de `site.rs`) |
| `modele/famille` | maillage (`modele/volumes`) | 3 / 1 | `volumes::solides::{prisme, hote}`, `volumes::geometrie::volume` |
| `modele/famille` | collaboration, config | 1 / 2 ; 0 / 4 | `collaboration::{uuid, projet, objets}` ; `config::gabarit::{Gabarit::defaut, familles_livrees}` |
| `modele/documentation` | moteur2d | 18 / 0 | `dessin::dao::{Forme2D, developper, segments_polyligne}`, `dessin::{coupe, vu, cadrer_primitives, Poche}` |
| `modele/documentation` | maillage | 9 / 6 (natif) ; 2 / 0 (volumes) | `id_natif` (8 prod), `depuis_natif`, `numero_vue` ; `volumes::solides::{hote, repere}` |
| `modele/documentation` | scene | 1 / 0 | `Scene` |
| `modele/documentation` | commandes | 2 / 7 | `edition::{ParametresMur, commandes::Commande, contextes, gabarits_vues}` |
| `modele/documentation` | formats, config, collaboration | 4 / 0 ; 3 / 2 ; 1 / 0 | `rvt::vues::{GrilleMotif, Motif, hachures_plan}`, `ifc::step::Id` ; `config::{gabarit, options::unite}` ; `collaboration::uuid` |
| `modele/edition` (commandes) | collaboration | 6 / 0 | `collaboration::{uuid, concurrence, objets}` |
| `modele/edition` | moteur2d, config | 1 / 0 ; 0 / 3 | `dessin::dao::arc_3_points` ; `config::gabarit::Gabarit::defaut` |
| `modele/echanges` | collaboration | 1 / 0 | `collaboration::uuid` |
| `modele/collaboration` | config | 0 / 2 | `Gabarit::defaut` (tests) |
| `dessin` (moteur2d) | config | 0 / 1 | `Gabarit::defaut` (test) |
| `pilote` | config | 0 / 1 | `Gabarit::defaut` (test) |

### 5.3 Références relatives non visibles dans le §5.1

- `modele/mod.rs` (struct `Projet` et ses `impl`) : champs `edition::contextes::FiltresContextes` et `edition::gabarits_vues::GabaritVue` (modele -> commandes), `collaboration::{uuid, hash}` (4 occurrences, modele -> collaboration), `volumes::{geometrie, solides}` (4 occurrences, modele -> volumes). Le bloc `use { echanges::..., edition::{commandes, historique, validation}, ... }` n'est utilisé que par les tests du fichier (`#[allow(unused_imports)]`).
- `modele/murs/assemblage.rs`, `modele/famille/edition.rs` : `collaboration::uuid()` (4 occurrences).
- `modele/collaboration/{cli,partage,stockage}.rs` -> `echanges::enregistrement::{enregistrer_natif, ecrire_atomique}` et `documentation::AnnotationForme` : 14 références hors tests, conformes à l'ordre cible (collaboration après echanges). `modele/collaboration/tests.rs` référence `edition` (11), `documentation` (7), `famille` (1) : conformes.
- `modele/volumes/solides.rs:948` (test) : `edition::ParametresDalle`.

### 5.4 Réponses aux questions de l'inventaire

**(i) Tous les `wgpu::` sont-ils dans `rendu/` ? Oui.** `wgpu::` apparaît dans `rendu/gpu.rs` (279) et `rendu/ciel.rs` (30), nulle part ailleurs ; les shaders (`*.wgsl`) sont dans `rendu/`. Deux nuances : `main.rs` utilise `eframe::egui_wgpu::{WgpuConfiguration, WgpuSetup}` (3 lignes, `limites_gpu()` : limite de buffers) et `ui/mod.rs` / `ui/commandes_pilote.rs` manipulent `cc.wgpu_render_state` et `Renderer::new(rs)` ; `Renderer::new` prend un `egui_wgpu::RenderState`. `egui` est aussi présent hors interface : `rendu/{orientation_plan,selection_rectangle,gpu}.rs`, `config/options/{mod,raccourcis}.rs`, `pilote/saisie.rs` ; il est absent de `noyau`, `formats`, `modele`, `dessin`, `rendu/scene` (une seule mention dans un commentaire de `noyau/texte.rs`). Cela place `egui` en dépendance de moteur3d, config et pilote.

**(ii) `modele/edition` appelle-t-il la génération de scène ? Seulement dans les tests.** Six références, toutes dans des tests : `rendu::scene::depuis_natif` (`edition/mod.rs` lignes 752, 848, 881, 913, 945 et `edition/commandes/tests.rs:72`). Aucune référence hors tests. Les tests doivent donc monter dans la crate `maillage` (ou une crate de tests) lors de la coupe : un test unitaire de `modele` ne peut pas appeler `maillage`, qui dépend de `modele`. Même constat pour `modele/elements` (6 en test), `modele/documentation` (1 hors test), `modele/volumes` (1 test), `dessin` (5 tests), `rendu(autres)` (1 test).

### 5.5 Écarts avec l'ordre proposé (à trancher avant les lots 1 à 12)

1. **scene et formats sont cycliques dans les deux sens.** `rendu/scene/mod.rs` contient les constructeurs IFC (`load`, `assemble`, `assemble_avec`) qui importent `formats::ifc` (10 références hors tests) et `formats::rvt::vues::Motif` ; `formats/rvt/feuilles.rs` et `formats/rvt/maquette/{mod,vers_scene}.rs` (conversion Revit -> `Scene`) importent `Scene`, `Navigateur`, `NavVue`, `NavFamille`, `NavMateriau`, `NavFenetre` (6 références hors tests, sens autorisé en cible). L'ordre « formats après scene » exige de déplacer `load/assemble/assemble_avec` dans `formats` (ils n'utilisent que `Scene`, donc légal) et de remplacer `ifc::step::Id` (`= u64`) et `Motif` par des types de `noyau`/`scene`. Cycle en plus de ceux déjà connus : `scene -> dessin::{Aretes, aretes, NOIR}`.
2. **`modele/volumes` ne peut pas aller dans `maillage` en l'état.** `murs`, `famille`, `elements`, `documentation` appellent `volumes::{solides, geometrie}` en code de production (8 + 3 + 1 + 2 références hors tests) et `volumes` appelle en retour `murs` (13), `famille` (11), `documentation` (4) : `volumes` est un composant du modèle. Option recommandée : `volumes` et les jonctions (`modele/murs/jonctions.rs`) restent dans `modele` ; `maillage` ne reçoit que `rendu/scene/natif.rs` (`depuis_natif`, `id_natif`, `numero_vue`). Il faut alors couper `volumes -> dessin::{aretes, chainer}` (3) et `volumes -> scene::chrono` (4). Alternative (inverser `murs -> volumes`) : changement structurel plus lourd, non recommandé.
3. **`modele` dépend de `commandes` (`modele/edition`).** Hors tests : `Projet` porte `FiltresContextes` et `GabaritVue` (types de `edition`), `elements` et `documentation` utilisent `ParametresMur`, `contour_valide`, `Commande`, `Implantation`. Ces types de données doivent descendre dans `modele` (par ex. `contextes`, `gabarits_vues`, `ParametresMur`, `Implantation`) ; `commandes` ne garde que transactions, historique, verrous, `Commande`. Les références en tests (`Commande`, `ParametresMur`) suivent la même coupe ou montent dans `commandes`.
4. **`modele` dépend de `config`** (`config::gabarit::{actif, Gabarit::defaut, familles_livrees}` : 6 références pour la racine, plus `murs`, `documentation`, `famille`, `edition` en tests ou en production) alors que `config` dépend de `modele` (19 références). Le gabarit par défaut et son chargement doivent passer sous `modele` (ou être injectés) ; `config` garde les options utilisateur et les raccourcis.
5. **`modele` dépend de `collaboration`** pour `collaboration::{uuid, hash}` (références relatives : modele racine 4, `murs` 2, `famille` 2 ; références `crate::` : `edition` 6 dont `uuid` 3, `famille` 1, `documentation` 1, `echanges` 1) : `uuid` et `hash` descendent dans `noyau` ou `modele` ; `concurrence`/`objets` appelés par `edition` demandent soit de coller `commandes` et `collaboration`, soit de déplacer ces deux modules.
6. **`modele/documentation` -> `dessin` (moteur2d)** : 18 références hors tests (`dessin::dao::{Forme2D, developper, segments_polyligne}`, `coupe`, `vu`, `cadrer_primitives`, `Poche`). Le type cité dans la demande est en fait `dessin::dao::Forme2D` (il n'existe pas de `dao::Forme` ; `Forme` désigne `noyau::trace2d::Forme`). `Forme2D` est défini dans `dessin/dao.rs` (19 usages hors tests) ; il doit descendre dans `noyau`. Les fonctions de rendu (`coupe`, `vu`, `cadrer_primitives`, `Poche`) sont appelées depuis `documentation/mod.rs` lignes ~1177-1213 : `documentation` contient du code de rendu de feuille qui doit monter (moteur2d ou `commandes`/`documenter`). Autres usages de `dessin` par `modele` : `murs`, `edition` (`dao::arc_3_points`, fonction géométrique pure, vers `noyau`).
7. **`maillage` (`scene/natif.rs`) -> `dessin`** : 12 références hors tests (`Aretes`, `TRAIT_FORT_MM`, `aretes_jointes`) et `rendu::gpu::MAX_APPARENCES` : `Aretes` et `TRAIT_FORT_MM` descendent dans `scene` ou `noyau` ; `MAX_APPARENCES` descend dans `scene`.
8. **`formats::rvt::vues::{Vue, Cadre}` : pas de dépendance de `modele` en production.** Ils sont utilisés par `modele/echanges` (2 et 1 références ; légal : echanges après formats), par `dessin` (3, `rvt::vues`), `scene` (2 + 2 via `natif`) et `ui` (3) ; `modele/documentation` n'utilise que `GrilleMotif`, `Motif`, `hachures_plan` (4 références, hors tests selon le scan). La coupe se limite donc à `Motif`/`GrilleMotif` et `hachures_plan`.
9. **`formats -> echanges` en tests** (`maquette/mod.rs`, `convertir_maquette`) et **`modele -> formats` / `modele -> echanges` en tests** (`elements/site.rs`) : ces tests doivent monter dans `echanges`. Hors tests : `murs/assemblage.rs:180 -> formats::ifc::step::format_real` (1) ; `documentation -> ifc::step::Id` n'apparaît qu'en test.
10. **`dessin -> scene`** (7 références, dont 4 en tests : `Scene`, `Vertex`, `SceneObject`, `Range`, `CoucheCoupe`) et **`dessin -> commandes`** (5, toutes en tests) sont conformes à l'ordre cible (moteur2d après scene et commandes).

Conclusion : l'ordre `noyau <- modele <- scene <- formats <- maillage <- commandes <- moteur2d <- moteur3d <- echanges <- collaboration <- pilote <- config <- interface <- app` reste cohérent en tant qu'ordre final, mais plusieurs lots exigent des coupes préalables non prévues : §5.5 points 1 (scene/formats), 2 (volumes reste dans modele), 3 (types d'`edition` vers modele), 4 (gabarit/config), 5 (uuid/hash), 6 (Forme2D et rendu de feuille hors de documentation). Les lots 2 (modele) et 3 (scene) sont les plus touchés et devraient absorber le plus de commits de coupe. Les lots 6 à 12 (commandes, moteur2d, moteur3d, echanges, collaboration, pilote, config, interface) ne révèlent aucun écart structurel supplémentaire.

### 5.6 Qui utilise les types signalés

Comptes d'occurrences du symbole (recherche par mot, commentaires exclus) ; p = hors tests, t = tests.

| Symbole | Utilisateurs |
|---|---|
| `scene::id_natif` | définition + `scene/natif` 58p ; `ui` 40p + 5t (12 fichiers) ; `modele/documentation` 8p + 6t (3 fichiers) ; `modele/elements` 3t ; `dessin` 1t ; `scene/mod` 1p (réexport) |
| `scene::depuis_natif` | `ui` 5p (4 fichiers) ; `pilote` 3p ; `modele/collaboration` 2p ; `modele/documentation` 1p ; `modele/edition` 6t ; `modele/elements` 3t ; `dessin` 5t ; `modele/volumes` 1t ; `rendu(autres)` 1t |
| `scene::numero_vue` | `ui` 20p (8 fichiers) ; `modele/documentation` 1p |
| `dessin::dao::Forme2D` | `dessin/dao.rs` 19p + 17t ; `ui` 12p + 26t (7 fichiers) ; `modele/documentation` 4p |
| `rvt::vues::{Vue, Cadre}` | `modele/echanges` (2 + 1 hors tests, 1 en test) |
| `ifc::step::Id` | `noyau/trace2d.rs` 1 ; `scene/mod` 2 ; `dessin` 2 ; `modele/echanges` 1 ; `modele/documentation` 1t ; `ui` 1 ; `formats` 4 |
| `NavFenetre` | `scene/mod` (définition) ; `formats` 4p (2 fichiers : `rvt/feuilles.rs`, `rvt/maquette/vers_scene.rs`) |
| `Navigateur` | `scene/mod`, `scene/natif` ; `formats` 3p (2 fichiers) |
| `NavVue`, `NavFamille`, `NavMateriau` | `scene/mod`, `scene/natif` ; `formats` (1 à 2 occurrences chacun) ; `ui` (2, 1 et 4) |
| `Scene` | `scene/mod`, `scene/natif` ; `dessin` 7p + 1t ; `ui` 14p ; `modele/documentation` 3p ; `formats` 2p + 1t ; `rendu(autres)` 2p |

### 5.7 Après lot 1 (2026-11-14)

Violations restantes concernant `noyau` : **aucune**. `crates/noyau` n'a aucune dépendance interne (`grep -rn "crate::\(modele\|formats\|rendu\|dessin\|ui\|pilote\|config\)" crates/noyau` est vide) ; ses dépendances externes sont `earcutr`, `epaint_default_fonts`, `glam`, `serde`, `serde_json`, `sha2`, `skrifa`, `uuid`. Les points §5.5 5 (`uuid`/`hash`), 6 (`Forme2D`, `arc_3_points`, `developper`, `segments_polyligne`) et 7 (`Aretes`, `TRAIT_FORT_MM`, `aretes_jointes`, `MAX_APPARENCES`) sont levés pour la partie `noyau` ; il reste dans `dessin` `TRAIT_FIN_MM`, `PAS_HACHURE_MM`, `Poche`, `coupe`, `vu`, `cadrer_primitives`, `sommets*`.

Emplacements : `noyau::{Id, uuid::uuid, hash::{hash, empreinte, encoder}, forme2d::*, aretes::*, constantes::MAX_APPARENCES, chrono::{chrono, chrono_lent}}`. `formats::ifc::step` ré-exporte `noyau::Id` (`pub use`). `MAX_APPARENCES` existe aussi en dur dans `rendu/scene.wgsl` (`const MAX_APPARENCES: i32 = 32`) : les deux valeurs doivent rester égales. Trois fonctions de triangulation (`texte::trianguler`, `trace2d::trianguler`, `geometrie::trianguler_contours`) sont passées de `pub(crate)` à `pub`, exigé par la compilation. Les dépendances `epaint_default_fonts`, `skrifa` et `sha2` ne sont plus déclarées dans `crates/app`.

### 5.8 Après lot 2 (2026-11-14)

Tests : 390 réussis, 2 ignorés, comme avant le lot ; répartition `bimedit` 279 (+ 1 ignoré), `modele` 101 (+ 1 ignoré), `noyau` 10. `cargo test -p modele --release` passe seul. `cargo tree -p modele` ne contient ni `egui`, ni `eframe`, ni `wgpu`. Aucun changement fonctionnel ni de schéma JSON : l'export IFC de `samples/maison_native.modele.json` est identique octet pour octet avant et après le lot.

#### (a) Contenu de `crates/modele`

Dépendances : `noyau` ; externes `earcutr`, `geo-straight-skeleton`, `glam`, `rayon`, `serde`, `serde_json`. 40 fichiers, 14 541 lignes. Aucun `crate::` ne sort de la crate.

| Chemin | Contenu | Origine |
|---|---|---|
| `lib.rs` | `Projet` et types du modèle, lecture/écriture JSON | `modele/mod.rs` |
| `soleil.rs` | position du soleil | inchangé |
| `murs/` | `assemblage`, `jonctions`, `ossature`, `retours` | inchangé |
| `elements/` | `arbre`, `cadrage`, `composant`, `escalier`, `garde_corps`, `pente`, `piece`, `site`, `terre_plein` | inchangé |
| `volumes/` | `geometrie`, `solides` | inchangé (reste dans `modele`, §5.5 point 2) |
| `famille/` | `modelee` (+ `catalogue`, `guidage`, `revolution`, `tests_maillage`), `profil`, `edition` | inchangé |
| `documentation/` | données des vues, feuilles, annotations, cotation, nomenclatures ; primitives des annotations | sans le rendu de vue et de feuille |
| `gabarit.rs` | `Gabarit`, `actif`, `definir`, `familles_livrees`, `texte_defaut` | `config/gabarit.rs` |
| `creation.rs` | `Implantation`, `ParametresMur`, `ParametresDalle`, `contour_valide`, `croisement`, `ListeCreation` ; méthodes de `Projet` : `ajouter_mur*`, `ajouter_dalle*`, `ajouter_choix_creation`, vues (`ajouter_vue_plan`, `ajouter_vue_coupe`, `dupliquer_vue`, `renommer_vue`, `retourner_vue`, `supprimer_vue`), `axe_element`, extrémités et lignes ajustables | `modele/edition/mod.rs` |
| `contextes.rs` | `FiltresContextes`, `Projet::elements_masques_contextes` | partie de `modele/edition/contextes.rs` |
| `gabarits_vues.rs` | `GabaritVue` et ses méthodes de `Projet` | `modele/edition/gabarits_vues.rs` |
| `validation.rs` | `Projet::verifier` | `modele/edition/validation.rs` |
| `identifiants.rs` | `id_natif`, `numero_vue` | `rendu/scene/natif.rs` |
| `unite.rs` | `Unite`, `unite()`, `definir_unite()` | `config/options/mod.rs` |

Descendus dans `noyau` au passage : `noyau::motif::{Motif, GrilleMotif, hachures_plan}` (de `formats::rvt::vues`) et `noyau::nombres::format_real` (de `formats::ifc::step`).

#### (b) Décisions de frontière

| # | Symbole | Ancien -> nouveau | Pourquoi | Alternative écartée |
|---|---|---|---|---|
| 1 | Gabarit de projet | `config::gabarit` -> `modele::gabarit` | données pures du modèle (aucune dépendance à `config::options` ni à egui) ; `modele` le lit en production | injection du gabarit en paramètre : signatures de `Projet` à modifier partout |
| 2 | Création paramétrique | `modele::edition` (racine) -> `modele::creation` | `ParametresMur`, `Implantation`, `contour_valide` devaient descendre ; leurs seuls consommateurs sont `ajouter_mur*`, `ajouter_dalle*` et les vues, qui ne dépendent que du modèle (`Projet::depuis_gabarit` appelle `ajouter_vue_plan`, `documentation` appelle `axe_element`) ; les tests de `murs`, `elements`, `documentation` qui posent un mur restent dans `modele` | descendre seulement les types et laisser les méthodes dans `commandes` : types sans usage de production dans `modele`, traits d'extension en plus, et 28 tests de plus à monter |
| 3 | `FiltresContextes` | `edition::contextes` -> `modele::contextes` | champ de `Projet` ; `elements_masques_contextes` appelé par `documentation` | descendre tout `contextes.rs` : la gestion des contextes appelle `executer`, `identifiants`, `phase_element` (commandes) |
| 4 | `GabaritVue` | `edition::gabarits_vues` -> `modele::gabarits_vues` | champ de `Projet` ; le fichier ne dépend que du modèle | aucune |
| 5 | `Projet::verifier` | `edition::validation` -> `modele::validation` | appelé en production par `famille/edition.rs` ; ne dépend que du modèle ; conforme au §1 (« validation » dans `modele`) | monter `famille/edition.rs` : ses autres méthodes servent au modèle |
| 6 | `Commande` | reste dans `edition::commandes` | une seule référence de production dans `modele` : `documentation::piloter_cote` | descendre `Commande` (1 200 lignes, tout `edition` en dépend) |
| 7 | `piloter_cote` | `modele::documentation` -> `edition::commandes::piloter_cote` (`edition/commandes/cotes.rs`) | renvoie une `Commande` ; signature inchangée | type de données intermédiaire dans `modele` : aurait modifié les deux tests |
| 8 | `croisement` | `edition::commandes` -> `modele::creation` | fonction géométrique pure appelée par `ajouter_dalle_contours` | aucune |
| 9 | Méthodes de `Projet` restées dans `edition` | `impl Projet` -> traits d'extension `edition::{Execution, Modification, Verrous, Contextes}` + traits internes (`ExecutionInterne`, `ModificationInterne`, `Transformation`, `VerrousInterne`, `ContextesInterne`) | Rust interdit les méthodes inhérentes sur un type d'une autre crate ; corps inchangés, 38 méthodes, 5 fichiers ; 23 fichiers appelants hors `edition` reçoivent une ligne `use` | fonctions libres : 92 appels à réécrire dans 23 fichiers ; fusion d'`edition` dans `modele` : contraire au plan (`Commande`, `collaboration`) |
| 10 | `Motif`, `GrilleMotif`, `hachures_plan` | `formats::rvt::vues` -> `noyau::motif` | géométrie pure sans dépendance ; utilisés par `documentation`, `dessin`, `scene` | `modele::motif` : `scene` et `formats` en auraient dépendu |
| 11 | `format_real` | `formats::ifc::step` -> `noyau::nombres` | fonction pure, utilisée par `murs/assemblage.rs` et `Unite::afficher` | copie dans `modele` |
| 12 | `Unite`, `unite()`, `definir_unite()` | `config::options` -> `modele::unite` | **non prévu au plan** : `documentation` lit l'unité d'affichage pour le texte des cotes et des nomenclatures (3 références de production) | passer l'unité en paramètre des fonctions de `documentation` : signatures publiques modifiées |
| 13 | `id_natif`, `numero_vue` | `rendu::scene::natif` -> `modele::identifiants` | fonctions de nommage pures, 8 références de production dans `documentation` | monter les primitives d'annotation qui les appellent |
| 14 | Rendu de vue et de feuille | `modele::documentation` -> `crate::dessin` | appelle `depuis_natif`, `coupe`, `vu`, `cadrer_primitives`, `Poche`, `svg2pdf`, `pdf-writer` | voir (c) |
| 15 | `edition`, `echanges`, `collaboration` | `crates/app/src/modele/…` -> `crates/app/src/{edition,echanges,collaboration}` | un module `modele` résiduel dans `app` aurait porté le même nom que la crate | `mod modele` résiduel : chemins ambigus |

Ré-exports conservés (chemin d'origine encore valide) : `config::options::{Unite, unite, definir_unite}` (84 références dans `ui`), `rendu::scene::{id_natif, numero_vue}` (une centaine de références), `formats::ifc::step::format_real` (43 références), `formats::rvt::vues::{Motif, GrilleMotif, hachures_plan}` (utilisés par `formats`). Aucun ré-export `modele::…` à un ancien chemin.

Visibilités élargies, exigées par la compilation : `documentation::{cotes_de, texte}` (privé -> `pub`), `documentation::{pt_papier, trait_papier}` (`pub(super)` -> `pub`), `documentation::remplir_champs` (`pub(crate)` -> `pub`) ; `Projet::ajouter_dalle` n'est plus sous `#[cfg(test)]` (appelé par des tests d'`app`).

#### (c) Code parqué dans `crates/app`

| Fichier | Contenu | Crate d'accueil |
|---|---|---|
| `dessin/feuilles.rs` | `dessin_vue`, `ids_masques_vue`, `vue_placee`, `feuille`, `svg_feuille`, `pdf`, `pdf_pages`, `pdf_jeu` (de `documentation/mod.rs` et `documentation/feuilles.rs`) | moteur2d (lot 7) |
| `dessin/plans.rs` | `fond_plan`, `limite_haute_plan`, `pointilles_plage_haute` (fichier `documentation/plans.rs` déplacé) | moteur2d (lot 7) |
| `edition/commandes/cotes.rs` | `piloter_cote` et ses deux tests | commandes (lot 6) |
| `modele_tests/` | 36 tests, voir (d) | maillage, commandes, moteur2d, echanges, collaboration selon ce qu'ils appellent (lots 5 à 9) |

Signatures inchangées ; les appelants (`ui`, `pilote`) importent `crate::dessin::…`.

#### (d) Tests déplacés

38 tests sortis des modules de `modele`, noms et assertions inchangés (imports adaptés ; `super::FichierProfil` devient `FichierProfil` dans `famille_profil.rs`). Les aides de test nécessaires sont déplacées ou recopiées (`projet`, `rectangle`, `piece`, `boite`, `escalier`, `arbre`, `type_trois_couches`, `projet` de `terre_plein`).

| Origine (`crates/modele/src/…`) | Destination (`crates/app/src/…`) | Tests |
|---|---|---|
| `murs/assemblage.rs` | `modele_tests/murs_assemblage.rs` | `modification_type_actualise_tous_les_murs_sans_detacher_les_couches`, `couche_editee_hors_du_mur_compose_se_detache` |
| `elements/arbre.rs` | `modele_tests/elements_arbre.rs` | `creer_puis_supprimer_un_arbre` |
| `elements/composant.rs` | `modele_tests/elements_composant.rs` | `composant_place_transforme_copie_supprime` |
| `elements/escalier.rs` | `modele_tests/elements_escalier.rs` | `commandes_escalier_et_garde_corps` |
| `elements/piece.rs` | `modele_tests/elements_piece.rs` | `piece_dans_quatre_murs`, `etiquette_famille_de_la_piece`, `piece_ouverte_refusee_et_recalcul`, `piece_mur_compose` |
| `elements/site.rs` | `modele_tests/elements_site.rs` | `creation_site_sauvegarde_export_et_suppression`, `site_en_phase_existant_cotes_ngf_et_epaisseur_du_type` |
| `elements/terre_plein.rs` (`tests_scene`) | `modele_tests/elements_terre_plein.rs` | `scene_terre_plein` |
| `volumes/solides.rs` | `modele_tests/volumes_solides.rs` | `conversion_ifc_rouvre_et_indices_invalides_refuses`, `recalcul_partiel_identique_apres_modification` |
| `famille/profil.rs` | `modele_tests/famille_profil.rs` | `profils_du_gabarit_varient_et_survivent_au_rechargement` |
| `documentation/tests.rs` | `modele_tests/documentation.rs` | `dessin_vue_applique_le_contexte_propre_a_la_vue`, `dessin_vue_suit_le_gabarit_partage`, `dessin_sur_feuille_suit_le_fond_la_plage_haute_et_le_style`, `coupe_sur_feuille_reste_dans_les_limites_de_la_vue`, `formes_du_cartouche_persistent_et_sont_dessinees_sur_feuille`, `vue_placee_suit_echelle_vue_meme_avec_ancienne_valeur`, `vue_placee_respecte_rotation_et_limite_basse`, `vue_placee_respecte_masquage_et_cadrage`, `documents_compatibles_v0_references_et_pdf`, `famille_cartouche_partagee_est_enregistree_et_utilisee`, `polyligne_fermee_remplie`, `nomenclature_csv_et_quantites_sans_double_compte`, `polyligne_types_de_trait_par_segment`, `polyligne_segments_d_arc` |
| `creation.rs` (ancien `edition/mod.rs`) | `modele_tests/creation.rs` | `mur_contraintes_phases_et_sous_projets_persistes`, `dalle_creation_contour_et_relecture`, `plafond_et_toiture_en_pente`, `vues_plan_et_coupe`, `mur_creation_enregistrement_relecture` |
| `gabarits_vues.rs` | `modele_tests/gabarits_vues.rs` | `gabarit_partage_actualise_detache_et_se_recharge`, `invalides_et_modifications_de_phases_restent_coherentes` |
| `documentation/cotation.rs` (`tests_aretes`) | `edition/commandes/cotes.rs` | `cote_entre_axes_suit_et_pilote_le_mur`, `angle_entre_aretes_pilote_la_rotation` |

Le test `dessin::…::jonction_sans_arete_meme_materiau` utilisait l'aide `modele::volumes::geometrie::tests::boite` (module `#[cfg(test)]`, invisible d'une autre crate) : l'aide est recopiée dans le test. `collaboration::tests::toutes_les_collections_du_projet_ont_une_politique` lit maintenant `crates/modele/src/lib.rs`.

#### (e) Violations restantes (lots 3 à 9)

Concernant `modele` : **aucune** (§5.5 points 2, 3, 4, 6 et 9 pour la partie `modele` levés ; point 5 levé pour `modele`, reste `edition`).

| Source (crate cible) | Importe | Références | Lot |
|---|---|---|---|
| `rendu/scene/mod.rs` (scene) | formats | `ifc::{geom, semantique, step}` : constructeurs `load`, `assemble`, `assemble_avec` (§5.5 point 1) | 3 |
| `formats` | scene | `NavFenetre`, `Scene`, `Navigateur`, `NavVue`, `NavFamille` (sens autorisé, cycle tant que le point précédent n'est pas levé) | 3 |
| `formats` | modele, echanges | tests de `rvt/maquette/tests.rs` (`modele::Projet`, `convertir_maquette`) : `formats` n'a pas `modele` dans ses dépendances autorisées | 4 |
| `edition` (commandes) | collaboration | `collaboration::{concurrence, objets}` dans `verrous.rs` (§5.5 point 5) | 6 |
| `edition` (commandes) | echanges | tests : `depuis_ifc`, `export_ifc` (`commandes/tests.rs`) | 6 |
| `dessin` (moteur2d) | formats | `formats::ifc::step::Id` (= `noyau::Id`, 2 références) | 7 |
| `dessin/feuilles.rs` (moteur2d) | maillage | `depuis_natif` : `maillage` n'est pas dans les dépendances autorisées de moteur2d au §2 (il est avant lui dans l'ordre) | 5 ou 7, à valider |
| `collaboration` | maillage | `depuis_natif` (`cli.rs`, 2 références) : non listé au §2 pour collaboration | 9, à valider |
| `modele_tests` | commandes, scene/maillage, formats, echanges, collaboration, moteur2d | tests parqués, à répartir | 5 à 9 |

Points ouverts pour la validation des frontières : (1) décision 2 (création paramétrique dans `modele` plutôt que dans `commandes/modeliser`) ; (2) décision 9 (traits d'extension de `Projet` dans `commandes`) ; (3) décision 12 (unité d'affichage, état global, dans `modele`) ; (4) décision 5 (validation dans `modele`) ; (5) `maillage` à ajouter aux dépendances autorisées de moteur2d et de collaboration au §2 ; (6) emplacement final des tests de `modele_tests`.

### 5.9 Lot 3, état intermédiaire (2026-11-14) : coupe faite, extraction arrêtée

**Obstacle résolu** : l'utilisateur a retenu l'option B du (b), avec le nom `ifc_base` (et non `ifc`, pour ne pas entrer en collision avec le module `formats::ifc`). Le lot est terminé ; état final au §5.10. Le texte ci-dessous est conservé comme trace de la décision.

Tests : 390 réussis, 2 ignorés, répartition inchangée (`bimedit` 279 + 1 ignoré, `modele` 101 + 1 ignoré, `noyau` 10). `crates/scene` n'existe pas. Sorties comparées à `d997e6c` : `--export-ifc` et `--quantites` de `samples/maison_native.modele.json` identiques octet pour octet ; `--stats` de `samples/test_ifc4.ifc`, `samples/test_ifc4_murs.ifc` et de l'IFC exporté identiques hors ligne « Durée » ; `--export-vue` (`plan-rdc`, `coupe`) identiques après tri des lignes et des segments (l'ordre des segments du SVG varie d'une exécution à l'autre avec le même binaire, avant comme après le lot).

#### (a) Coupe faite (`a069497`)

| Symbole | Ancien -> nouveau | Pourquoi |
|---|---|---|
| `load`, `assemble`, `assemble_avec` | `rendu::scene` -> `formats::ifc::scene` (`crates/app/src/formats/ifc/scene.rs`, 246 lignes) | consomment `ProductGeom`, `GeomStats`, `StepFile`, `IfcModel` ; corps inchangés. Appelants : `main.rs`, `ui/documents.rs`, `rendu/scene/natif.rs`, `formats/rvt/maquette/vers_scene.rs` |
| `Id` dans `scene` | `formats::ifc::step::Id` -> `noyau::Id` | même type |

`assemble_avec` n'est pas propre à l'IFC : c'est l'assembleur commun `ProductGeom` -> `Scene`, appelé aussi par `depuis_natif` (maillage) et par la conversion Revit ; `maillage` et `formats` peuvent tous deux dépendre de `formats`, donc l'emplacement est légal. `Motif`, `Aretes`/`aretes`/`NOIR` et `MAX_APPARENCES` étaient déjà pris dans `noyau` par `scene/mod.rs` (lots 1 et 2). `rendu/scene/natif.rs` ne s'appuie plus sur les imports de `mod.rs` pour `StepFile`, `rayon`, `Path`. Aucun test dans `rendu/scene/mod.rs` : aucun test déplacé. Aucune visibilité élargie, aucun ré-export ajouté.

#### (b) Obstacle : `Scene` contient des types de `formats::ifc`

Il reste dans `rendu/scene/mod.rs` deux imports de `formats`, non prévus par le plan (le §5.5 point 1 ne citait que les constructeurs) :

- `Scene::model: IfcModel` (`formats::ifc::semantique`) : modèle sémantique de toute scène, IFC, Revit (`vers_scene.rs`) ou native (`depuis_natif` fabrique un `StepFile` et appelle `IfcModel::new`). 58 accès `scene.model` dans 14 fichiers (`ui` 48, `main.rs` 6, `dessin` 2, tests de `formats/rvt` 3) : `parent`, `ty`, `file`, `name`, `products`, `get`, `type_of`, `property_groups`, `project`, `length_scale`, `label`, `children`, `material_of`, `material_names`, `global_id`.
- `IdMap` (`formats::ifc::step`, `HashMap<Id, V>` à hachage identité) : champs `Scene::object_of` et `Scene::couches_coupe`.

`IfcModel` contient un `StepFile` ; `semantique.rs` (581 lignes) ne dépend que de `step.rs` (552 lignes) et de `noyau` ; `step.rs` ne dépend que de `noyau` et `rayon`. Tant que `Scene` porte `IfcModel`, `scene` ne peut pas précéder `formats` sans décision de structure.

| Option | Contenu | Coût | Effet sur §2 |
|---|---|---|---|
| A | `step.rs` et `semantique.rs` descendent dans `scene` (`scene::step`, `scene::semantique`) ; `formats::ifc` garde `geom.rs` et `scene.rs` | 2 fichiers déplacés (1 133 lignes, 2 tests) ; avec ré-export `formats::ifc::{step, semantique}` : 0 remplacement ; sans : 39 références dans 23 fichiers hors `formats/ifc` + 4 dedans ; `rayon` en dépendance de `scene` | graphe inchangé ; `scene` n'est plus « neutre » : elle porte le parseur STEP et la sémantique IFC |
| B | nouvelle crate `ifc` (ou `step`) = `step.rs` + `semantique.rs`, entre `modele` et `scene` | même déplacement que A dans une crate à part ; mêmes comptes de références | `scene` dépend de `noyau`, `modele`, `ifc` ; `formats` de `noyau`, `ifc`, `scene` ; une crate de plus (15) |
| C | `Scene` perd `model` ; `IdMap` descend dans `noyau` ; le couple (scène, modèle) devient un type de `formats` | 58 accès dans 14 fichiers à réécrire, plus les signatures qui prennent `&Scene` pour lire le modèle ; `Scene { model, .. }` construit à un seul endroit (`assemble_avec`) | graphe inchangé, `scene` neutre ; le type manipulé par `ui`, `dessin`, `rendu` change |
| D | séparer données et parseur : types (`IdMap`, `Value`, `Entity`, `StepFile`, `IfcModel` et ses méthodes) dans `scene`, parseur (`StepFile::parse`, `Parser`, `decode_step_string`, ~370 lignes) dans `formats` | `StepFile::parse` (méthode inhérente) devient une fonction libre : 4 appels hors `formats/ifc` ; découpe de `step.rs` | graphe inchangé ; `scene` porte la sémantique IFC mais pas la lecture de fichier |

Aucune option ne change le comportement ni un format. Recommandation : B si l'on veut garder `scene` sans code IFC de lecture et un graphe lisible (coût identique à A, une crate de plus) ; A si l'on refuse une 15ᵉ crate. C est la seule qui rend `scene` réellement neutre, au prix du changement de type le plus étendu.

#### (c) Reste à faire au lot 3 après décision

Extraction de `crates/scene` (types `Vertex`, `Range`, `SceneObject`, `Scene`, `Navigateur`, `NavVue`, `NavFenetre`, `NavFamille`, `NavMateriau`, `ReperePartage`, `CoucheCoupe`, `Scene::{sol_sous, obstacle, pick}`, `etat_de_phase`) ; `rendu/scene/natif.rs` reste dans `app` (maillage, lot 5) ; `projet_vide` (`#[cfg(test)]`, appelle `depuis_natif`, un seul appelant : test de `dessin/mod.rs`) reste dans `app`.

#### (d) Violations restantes

Celles du §5.8 (e), sauf la première ligne, remplacée par : `rendu/scene/mod.rs` (scene) -> formats : `ifc::semantique::IfcModel`, `ifc::step::IdMap` (champs de `Scene`, voir (b)). Ajout : `rendu/scene/natif.rs` (maillage) -> `formats::ifc::scene::assemble_avec` (sens autorisé).

### 5.10 Après lot 3 (2026-11-14)

Tests : 390 réussis, 2 ignorés, comme avant le lot ; répartition `bimedit` 277 (+ 1 ignoré), `modele` 101 (+ 1 ignoré), `noyau` 10, `ifc_base` 2, `scene` 0. `cargo test -p ifc_base -p scene --release` passe seul. `cargo tree -p ifc_base` et `cargo tree -p scene` ne contiennent ni `egui`, ni `eframe`, ni `wgpu`. Aucun `crate::` ne sort de `crates/ifc_base/src` ni de `crates/scene/src`. Sorties comparées à `f332780` : `--export-ifc` et `--quantites` de `samples/maison_native.modele.json` identiques octet pour octet ; `--stats` de `samples/test_ifc4.ifc`, `samples/test_ifc4_murs.ifc` et de l'IFC exporté identiques hors ligne « Durée » (et nom du fichier exporté). `--export-vue` n'a pas été comparé à ce lot.

#### (a) Contenu de `crates/ifc_base` et de `crates/scene`

`ifc_base` (`b698a58`) : dépendances `noyau` ; externe `rayon`. Elle ne dépend pas de `modele`. 3 fichiers, 1 136 lignes.

| Chemin | Contenu | Origine |
|---|---|---|
| `lib.rs` | `pub mod semantique; pub mod step;` | nouveau |
| `step.rs` (552 l.) | parseur ISO 10303-21 : `IdHasher`, `IdMap`, `Value`, `Entity`, `StepFile` (dont `StepFile::parse`), décodage des chaînes ; ré-exporte `noyau::Id` et `noyau::nombres::format_real` | `crates/app/src/formats/ifc/step.rs`, inchangé |
| `semantique.rs` (581 l.) | `IfcModel` (arborescence spatiale, types, propriétés, matériaux, unités), `pretty_type` | `crates/app/src/formats/ifc/semantique.rs` ; seul changement : `crate::formats::ifc::step::` -> `crate::step::` |

`scene` (`a114732`) : dépendances `noyau`, `modele`, `ifc_base` ; externe `glam`. 1 fichier, 384 lignes.

| Chemin | Contenu | Origine |
|---|---|---|
| `lib.rs` | `Vertex`, `Range`, `SceneObject`, `Navigateur`, `NavMateriau`, `NavVue`, `NavFenetre`, `NavFamille`, `ReperePartage`, `Scene`, `CoucheCoupe`, `Scene::{sol_sous, obstacle, pick}`, `etat_de_phase` (privés : `ray_aabb`, `ray_tri`) | `crates/app/src/rendu/scene/mod.rs`, sans `mod natif`, sans les ré-exports `depuis_natif`/`depuis_natif_verifie`/`id_natif`/`numero_vue`, sans `projet_vide` |

`scene` prend dans `modele` : `Apparence` (`Scene::apparences`) ; dans `ifc_base` : `IfcModel` (`Scene::model`), `IdMap` (`Scene::object_of`, `Scene::couches_coupe`) ; dans `noyau` : `Id`, `trace2d::Primitive`, `motif::Motif`.

#### (b) Décisions et ré-exports conservés

| # | Décision | Pourquoi | Alternative écartée |
|---|---|---|---|
| 1 | Option B : crate `ifc_base` entre `modele` et `scene` | décision de l'utilisateur ; `scene` ne porte pas le parseur STEP | options A, C, D du §5.9 |
| 2 | `ifc_base` ne dépend que de `noyau` | la compilation ne demande rien d'autre ; l'ordre `modele <- ifc_base` est un rang, pas une dépendance | aucune |
| 3 | `rendu/scene/natif.rs` -> `rendu/natif.rs` (module `crate::rendu::natif`) ; le module `rendu::scene` disparaît | `rendu/scene/` ne contenait plus que lui ; un module `rendu::scene` résiduel aurait porté le nom de la crate `scene` | garder `rendu/scene/mod.rs` réduit aux ré-exports : 0 remplacement pour `depuis_natif`/`id_natif`, mais chemin `rendu::scene` ambigu avec la crate |
| 4 | `projet_vide` (`#[cfg(test)]`) -> `rendu/natif.rs` | appelle `depuis_natif` ; un seul appelant (test de `dessin/mod.rs`) | aucune |
| 5 | `scene` en une seule `lib.rs` | 384 lignes, sous le plafond de 600 | découpe en fichiers : sans nécessité |

Ré-exports conservés (dette) :

| Ré-export | Où | Références | À retirer |
|---|---|---|---|
| `pub use ifc_base::{semantique, step};` | `crates/app/src/formats/ifc/mod.rs` | 41 références dans 24 fichiers d'`app` passent encore par `formats::ifc::{step, semantique}` | lot 4 |
| `pub use modele::identifiants::{id_natif, numero_vue};` | `crates/app/src/rendu/natif.rs` (remplace celui de `rendu::scene`) | 75 références `rendu::natif::{id_natif, numero_vue}` | lot 5 (maillage) |
| `step::{Id, format_real}` (ré-exports de `noyau`) | `crates/ifc_base/src/step.rs` | déjà présents avant le lot (§5.7, §5.8) | lot 4 ou plus tard |

Remplacements dans `app` (`a114732`) : `crate::rendu::scene::X` et `scene::X` (via `use crate::rendu::scene::{self, …}`) deviennent `scene::X` pour les types déplacés, `crate::rendu::natif::X` pour `depuis_natif`, `depuis_natif_verifie`, `id_natif`, `numero_vue`, `projet_vide` (103 références `rendu::natif::` dans 30 fichiers). `rendu/natif.rs` importe explicitement `glam::{DVec3, Vec3, Vec4}`, `noyau::Id`, `ifc_base::semantique::IfcModel` et `scene::*` à la place de `use super::*`. Aucune visibilité élargie. Aucun changement de format ni de comportement.

#### (c) Code parqué dans `crates/app`

Liste du §5.8 (c) inchangée, plus :

| Fichier | Contenu | Crate d'accueil |
|---|---|---|
| `rendu/natif.rs` (884 l.) | `depuis_natif`, `depuis_natif_verifie`, `projet_vide` (test), ré-export `id_natif`/`numero_vue`, module `tests` | maillage (lot 5) |
| `formats/ifc/scene.rs` | `load`, `assemble`, `assemble_avec` (§5.9 a) | formats (lot 4) |

#### (d) Tests déplacés

| Origine | Destination | Tests |
|---|---|---|
| `crates/app/src/formats/ifc/step.rs` | `crates/ifc_base/src/step.rs` | `parse_basic`, parti avec le fichier, inchangé |
| `crates/app/src/formats/ifc/semantique.rs` | `crates/ifc_base/src/semantique.rs` | 1 test (`pretty_type`), parti avec le fichier, inchangé |

Aucun test n'a dû remonter dans `app`. `rendu/scene/mod.rs` ne contenait aucun test : `scene` a 0 test.

#### (e) Violations restantes (lots 4 à 9)

Concernant `ifc_base` et `scene` : **aucune**.

| Source (crate cible) | Importe | Références | Lot |
|---|---|---|---|
| `formats` | modele, echanges | tests de `rvt/maquette/tests.rs` (`modele::Projet`, `convertir_maquette`) : `modele` n'est pas dans les dépendances autorisées de formats | 4 |
| `formats/ifc/mod.rs` | — | le module `formats::ifc::scene` porte le nom de la crate `scene` : dans la future crate `formats`, un chemin `scene::X` écrit dans `ifc/mod.rs` désignerait le module (aujourd'hui aucun) | 4, à surveiller |
| `rendu/natif.rs` (maillage) | formats | `formats::ifc::geom::{Part, ProductGeom}`, `formats::ifc::scene::assemble_avec` (sens autorisé) | 5 |
| `edition` (commandes) | collaboration | `collaboration::{concurrence, objets}` dans `verrous.rs` (§5.5 point 5) | 6 |
| `edition` (commandes) | echanges | tests : `depuis_ifc`, `export_ifc` (`commandes/tests.rs`) | 6 |
| `dessin` (moteur2d) | formats | `formats::ifc::step::Id` (= `noyau::Id`, 2 références, via le ré-export) | 4 (retrait du ré-export) ou 7 |
| `config` | formats | `formats::ifc::step::format_real` (= `noyau::nombres::format_real`, 2 références, via le ré-export) | 4 |
| `dessin/feuilles.rs` (moteur2d) | maillage | `depuis_natif` (autorisé au §2 depuis `d997e6c`) | 7 |
| `collaboration` | maillage | `depuis_natif` (`cli.rs`, autorisé au §2 depuis `d997e6c`) | 9 |
| `modele_tests` | commandes, maillage, formats, echanges, collaboration, moteur2d | tests parqués, à répartir | 5 à 9 |

Usage d'`ifc_base` hors `formats` relevé dans `app` (via `formats::ifc::{step, semantique}`) : `rendu/natif.rs` (`Entity`, `StepFile`, `Value`, `IfcModel`), `echanges` (`IfcModel`, `pretty_type`, `Entity`, `Id`, `StepFile`), `ui` (`pretty_type`, `Id`, `Value`, `format_real`), `main.rs` (`pretty_type`, `format_real`), `modele_tests` (`IfcModel`, `StepFile`), `dessin` (`Id`), `config` (`format_real`). `Id` et `format_real` sont dans `noyau` : `dessin` et `config` n'ont pas besoin d'`ifc_base`.

Points ouverts : (1) dépendances `ifc_base` à inscrire au §2 pour maillage, echanges, interface et app (proposé ci-dessus, à valider) ; (2) `formats` -> `modele` pour les tests de `rvt/maquette` (monter les tests ou autoriser la dépendance) ; (3) nom du module `formats::ifc::scene` face à la crate `scene`.

### 5.11 Après lot 4 (2026-11-14)

#### (a) Contenu de `crates/formats`

Package `formats` (lib, 12 537 lignes, 52 tests). Dépendances internes : `noyau`, `ifc_base`, `scene` (`modele` n'est pas déclarée ; elle est transitive via `scene`). Dépendances externes : `earcutr`, `glam`, `miniz_oxide`, `png`, `rayon`, `serde_json`. `cargo tree -p formats` ne contient ni `egui`, ni `wgpu`, ni `eframe`. `grep -rn "crate::" crates/formats/src` : uniquement des chemins internes à `formats`.

| Chemin (`crates/formats/src/`) | Contenu |
|---|---|
| `lib.rs` | `pub mod ifc; pub mod rvt;` (ancien `formats/mod.rs`) |
| `ifc/geom.rs` | conversion STEP -> géométrie (`ProductGeom`, `Part`, `GeomStats`, `Ligne`, `product_geometry`, `class_color`) |
| `ifc/assemblage.rs` | `load`, `assemble`, `assemble_avec` (ancien `ifc/scene.rs`) |
| `rvt/` | lecture Revit : conteneur OLE, `objets`, `schema`, `champs`, `geometrie`, `vues`, `parametres` (+ `donnees/*.tsv`, 4 fichiers, `include_str!` relatifs, déplacés avec les sources), `formules`, `nomenclatures`, `feuilles`, `medias`, `annexes`, `plans`, `zones`, `coordonnees`, `maquette/` (`lecture`, `vers_scene` : `Maquette` -> `Scene`, `tests`) |

Aucun `include_str!` vers `docs/` ou `samples/`. Aucune visibilité élargie (`pub(crate)`/`pub(super)` inchangés, 44 occurrences).

#### (b) Décisions

| Symbole | Ancien -> nouveau | Pourquoi |
|---|---|---|
| `formats::ifc::scene` (module) | -> `formats::ifc::assemblage` | éviter l'ambiguïté avec la crate `scene` (`scene::X`) ; 4 appelants (`main.rs`, `ui/documents.rs`, `rendu/natif.rs`, `rvt/maquette/vers_scene.rs`) |
| `couches_projet1_lecture_et_conversion` | `formats::rvt::maquette::tests` -> `app` (`formats_tests`) | seul code de `formats` appelant `echanges::depuis_rvt::convertir_maquette` et `modele::{Conversion, Projet}` ; `modele` n'est donc pas dépendance de `formats`, même pas en `dev-dependencies` |
| `Motif` dans `rvt/maquette/mod.rs` | importé par `super::vues` -> `noyau::motif::Motif` | le ré-export `pub use` de `vues.rs` est retiré ; `vues.rs` importe `noyau::motif` en privé |

Aucune dépendance de production de `formats` vers `app` n'existait au départ (la seule référence était le test ci-dessus) : aucune conversion vers le modèle natif n'a été montée dans `app`.

#### (c) Ré-exports

Retirés : `pub use ifc_base::{semantique, step};` (`formats/ifc/mod.rs` ; 24 fichiers d'`app` : `formats::ifc::step::X` -> `ifc_base::step::X`, `formats::ifc::semantique::X` -> `ifc_base::semantique::X`) ; `formats::ifc::step::{Id, format_real}` -> `noyau::Id`, `noyau::nombres::format_real` (y compris `dessin`, `config`, `ui`, `main.rs`) ; `pub use noyau::motif::{GrilleMotif, Motif, hachures_plan}` de `rvt/vues.rs` (-> `noyau::motif`).
Conservés : `ifc_base::step::{Id, format_real}` (ré-exports de `noyau`, déjà là aux lots 1 et 2)  ; `pub use vers_scene::*` dans `rvt/maquette/mod.rs` (interne à `formats`).

#### (d) Tests déplacés

| Test | De | Vers |
|---|---|---|
| `couches_projet1_lecture_et_conversion` (nom et assertions inchangés ; le helper `essai` y est recopié) | `crates/app/src/formats/rvt/maquette/tests.rs` | `crates/app/src/formats_tests/mod.rs` (monte dans `echanges` au lot 9) |

Les 52 autres tests de `formats` ont suivi le code (`cargo test -p formats --release` : 52). Les tests de `rvt/maquette/tests.rs` lisent des fichiers hors dépôt (`RVT_ESSAIS`) et se sautent s'ils sont absents : aucun `.rvt` n'est versionné, la lecture Revit n'est donc couverte ici que par les tests unitaires.

#### (e) Violations restantes (lots 5 à 9)

| De | Vers | Détail | Lot |
|---|---|---|---|
| `rendu/natif.rs` (maillage) | formats | `formats::ifc::geom::Ligne`, `formats::ifc::assemblage::assemble_avec` (sens autorisé) | 5 |
| `echanges` | formats | `rvt::{maquette, vues::{Cadre, Contenu, Vue}, feuilles::etendue_cartouche}`, `ifc::geom::{GeomStats, product_geometry}` (sens autorisé) | 9 |
| `formats_tests` | echanges, modele | test ci-dessus : `formats` -> `echanges` dans l'ordre inverse | 9 |
| `modele_tests/elements_site.rs` | formats, echanges | `ifc::geom::{GeomStats, product_geometry}`, `export_ifc` (tests parqués) | 5 à 9 |
| `main.rs`, `ui` | formats | `rvt::{maquette, is_revit_path, vues, …}`, `ifc::assemblage::load` (sens autorisé) | 11 |
| `formats` | `modele` (transitif) | `scene` dépend de `modele` (`Vec<modele::Apparence>`) : `formats` en hérite sans l'utiliser directement | — |

Points ouverts du §5.10 : (2) `formats` -> `modele` : tranché (tests montés, pas de dépendance) ; (3) nom du module : tranché (`assemblage`). Le point (1) (§2 : `ifc_base` pour maillage, echanges, interface, app) reste à valider.

### 5.12 Après lot 5 (2026-11-14)

#### (a) Contenu de `crates/maillage`

Package `maillage` (lib, 900 lignes, 2 tests). Dépendances internes : `noyau`, `modele`, `ifc_base`, `scene`, `formats`. Dépendances externes : `glam`, `rayon`. `cargo tree -p maillage` ne contient ni `egui`, ni `wgpu`, ni `eframe`. `grep -rn "crate::" crates/maillage/src` : aucune occurrence. API publique : `depuis_natif`, `depuis_natif_verifie` (seuls symboles `pub`).

| Chemin (`crates/maillage/src/`) | Contenu |
|---|---|
| `lib.rs` (536 lignes) | `depuis_natif`, `depuis_natif_verifie` (une fonction d'environ 490 lignes, non découpée), `lisser` ; suite de l'ancien `rendu/natif.rs` (`git mv`) |
| `materiaux.rs` | `opacite`, `rendu_materiau`, `motif_surface`, `coupe_materiau` |
| `aretes.rs` | `aretes_jointes`, `aretes_silhouette`, `aretes_terrain` |
| `navigateur.rs` | `navigateur` (arborescence : vues, familles, matériaux) |
| `tests.rs` | `topographie_revit_importee_est_geographique`, `terrain_sous_le_mur_et_ses_traits` |

Usage des dépendances : `ifc_base::{step::{Entity, StepFile, Value}, semantique::IfcModel}` ; `formats::ifc::geom::{Part, ProductGeom, Ligne}`, `formats::ifc::assemblage::assemble_avec` ; `noyau::{Id, chrono, aretes, motif, constantes::MAX_APPARENCES}` ; `scene::*` ; `modele`.

#### (b) Décisions

| Sujet | Décision | Pourquoi |
|---|---|---|
| Coupes de production | aucune | `rendu/natif.rs` n'avait déjà plus aucune référence `crate::` (ni `dessin`, ni `edition`, ni `config`, ni `rendu::gpu`) après les lots 1 à 4 |
| `projet_vide` (`#[cfg(test)]`, alias de `depuis_natif`) | supprimé ; son seul appelant (`dessin/mod.rs`, helper de test `scene_section`) appelle `maillage::depuis_natif` | un symbole `cfg(test)` d'une crate n'est pas visible d'une autre crate ; même corps, aucun changement de comportement |
| Découpage du fichier (884 lignes) | 4 modules + `tests.rs`, déplacement de fonctions entières sans modification | plafond de 600 lignes (§3 règle 7) ; les 8 fonctions déplacées passent de privées à `pub(crate)` (exigé par la compilation) ; aucun `pub` ajouté |
| Commentaire de documentation de `aretes_terrain` | resté au-dessus de `aretes_silhouette` (placement d'origine, erroné) | déplacement mécanique ; à corriger hors chantier |

#### (c) Ré-exports retirés

`pub use modele::identifiants::{id_natif, numero_vue};` de `rendu/natif.rs` : 75 lignes dans 21 fichiers d'`app` (`dessin` : 3, `modele_tests` : 2, `ui` : 16) pointent sur `modele::identifiants::{id_natif, numero_vue}`. `crate::rendu::natif::{depuis_natif, depuis_natif_verifie}` -> `maillage::…` (29 lignes, appel de l'ancien `projet_vide` compris). Aucun ré-export conservé.

#### (d) Tests parqués dans `crates/app`

Les 2 tests de `rendu/natif.rs` ont suivi le code (`cargo test -p maillage --release` : 2 ; `bimedit` : 225 -> 223). Tests d'`app` qui appellent `maillage::depuis_natif`, laissés en place :

| Fichier (`crates/app/src/`) | Tests | Autres dépendances | Accueil |
|---|---|---|---|
| `modele_tests/creation.rs` | `mur_contraintes_phases_et_sous_projets_persistes`, `dalle_creation_contour_et_relecture`, `plafond_et_toiture_en_pente`, `vues_plan_et_coupe`, `mur_creation_enregistrement_relecture` | `modele` seul | maillage (déplaçables dès maintenant, à valider) |
| `modele_tests/elements_terre_plein.rs` | `scene_terre_plein` | `modele` seul | maillage (idem) |
| `modele_tests/elements_site.rs` | `creation_site_sauvegarde_export_et_suppression`, `site_en_phase_existant_cotes_ngf_et_epaisseur_du_type` | `edition::{Execution, commandes}`, `echanges::export_ifc`, `formats` | echanges (lot 9) |
| `modele_tests/volumes_solides.rs` | `conversion_ifc_rouvre_et_indices_invalides_refuses` | `edition`, `echanges::depuis_ifc` | echanges (lot 9) |
| `edition/commandes/tests.rs` | `structures_creation_copie_rotation_suppression_et_validation` | `edition`, `echanges::{depuis_ifc, export_ifc}` (fichier entier) | commandes (lot 6) ou echanges (lot 9) selon le test |
| `dessin/mod.rs` | `isolant_suit_le_mur_courbe_dans_la_coupe_native`, `ossature_coupee_en_plan_montants_et_travees`, `fenetre_coupee_piece_par_piece`, `angle_quelconque_sans_reste_d_onglet`, `isolant_d_un_mur_courbe_reel`, et les tests passant par le helper `scene_section` | `dessin` | moteur2d (lot 7) |
| ~~`rendu/selection_rectangle.rs`~~ | `fenetre_croisement_visibilite_et_coupe` | `rendu` | moteur3d (lot 8) : a suivi le code, voir §5.15 d |

#### (e) Violations restantes (lots 6 à 9)

Concernant `maillage` : **aucune**.

| De | Vers | Détail | Lot |
|---|---|---|---|
| `edition` (commandes) | collaboration | `collaboration::{concurrence, objets}` dans `verrous.rs` (§5.5 point 5) | 6 |
| `edition` (commandes) | echanges | tests : `depuis_ifc`, `export_ifc` (`commandes/tests.rs`) | 6 |
| `dessin/feuilles.rs` (moteur2d) | maillage | `depuis_natif` (production, sens autorisé) | 7 |
| `collaboration` | maillage, commandes, echanges | `depuis_natif` (`cli.rs`), `edition::commandes`, `echanges::enregistrement` (sens autorisé) | 9 |
| `pilote` | maillage, commandes, echanges, collaboration | `depuis_natif` (`cli.rs`) ; sens autorisé | 10 |
| `config` | echanges | `echanges::enregistrement` (1 référence ; sens autorisé) | 11 |
| `ui` | maillage | `depuis_natif`, `depuis_natif_verifie` (`collaboration.rs`, `pilotage.rs`, `bim/mod.rs`, `documents.rs`, `saisie_plan.rs`) ; sens autorisé | 11 |
| `modele_tests`, `formats_tests` | commandes, maillage, formats, echanges, collaboration, moteur2d | tests parqués, à répartir (§5.8 d, §5.11 d, tableau (d) ci-dessus) | 6 à 9 |

Relevé par `grep` des chemins `crate::<module>` entre `edition`, `dessin`, `rendu`, `echanges`, `collaboration`, `pilote`, `config` ; les références de `ui` et de `main.rs` vers les autres modules ne sont pas relevées ici (lot 11). Point ouvert du §5.10 (1) (`ifc_base` au §2 pour echanges, interface, app) : inchangé ; pour `maillage`, la dépendance directe à `ifc_base` est confirmée par le code.

### 5.13 Après lot 6 (2026-11-14)

Tests : 390 réussis, 2 ignorés, comme avant le lot ; répartition `bimedit` 199 (+ 1 ignoré), `commandes` 18, `formats` 52, `modele` 101 (+ 1 ignoré), `noyau` 10, `ifc_base` 2, `maillage` 8 (2 unitaires + 6 dans `tests/`), `scene` 0. `cargo test -p commandes --release` passe seul (18). `cargo tree -p commandes` ne contient ni `egui`, ni `wgpu`, ni `eframe`. Aucun `crate::` ne sort de `crates/commandes/src`.

#### (a) Contenu de `crates/commandes`

Package `commandes` (lib, 17 fichiers, 3 288 lignes). Dépendances internes : `noyau` (`uuid`), `modele`. Externes : `glam`, `serde`, `serde_json`. API publique (racine) : `Commande`, `Execution`, `Modification`, `Verrous`, `Contextes`, `piloter_cote`, `decaler_polygone`, modules `historique`, `verrous`, `contextes`, `objets`. Les modules de métier sont privés : leurs traits (`ExecutionModeliser`, etc.) sont `pub(crate)`.

| Chemin (`crates/commandes/src/`) | Lignes | Contenu |
|---|---|---|
| `lib.rs` | 15 | modules et ré-exports |
| `socle/commande.rs` | 192 | `enum Commande` seul (fichier servi par l'aide « commandes » du pilote) |
| `socle/execution.rs` | 196 | traits `Execution`, `ExecutionInterne` ; `executer` (copie de travail, verrous, validation) ; `executer_interne` : `match` exhaustif qui envoie chaque variante vers son métier ; `Verrouiller` |
| `socle/historique.rs` | 365 | `Historique`, `Instantane` (4 tests) |
| `socle/verrous.rs` | 145 | `Verrous`, `VerrousInterne`, `verifier`, `message` (2 tests) |
| `socle/contextes.rs` | 264 | `Contextes` : phases, sous-projets, variantes (2 tests) |
| `socle/objets.rs` | 119 | `Objets`, `objets`, `COLLECTIONS`, `PolitiqueCollection`, `carte_objet` (descendus de `collaboration`, (b)) |
| `socle/interne.rs` | 12 | imports communs (`use crate::socle::interne::*;`) |
| `modeliser/mod.rs` | 347 | `Terrain`, `LimitePropriete`, `CoordonneesOrigine`, `Escalier`, `GardeCorps`, `Piece`, `TypeComposant`, `Arbre`, `Composant`, `Barre`, `Ouverture`, `ActualiserType`, `Contour`, `Tremie` |
| `modifier/mod.rs` | 173 | `Attacher`, `PrioriteJonction`, `Aligner`, `InverserMur`, `ModifierTrajectoireMur`, `AjusterAngle`, `Ajuster`, `Deplacer`, `Tourner`, `Supprimer` ; renvoi vers `composees` |
| `modifier/copier.rs` | 282 | `Copier`, `CopierSurNiveau`, `remapper_annotation` |
| `modifier/composees.rs` | 557 | ancien `commandes/modifier.rs` : `Modification`, symétrie, échelle, réseau, décaler, scinder, démolir, reproduire, changer d'hôte, inverser une ouverture, `decaler_polygone` (8 tests) |
| `modifier/transformations.rs` | 373 | `Affine`, `Transformation` (inchangé) |
| `annoter/mod.rs` | 87 | `ModifierQuadrillage`, `Quadrillage`, `Annoter`, `Niveau`, `ModifierNiveau` |
| `annoter/cotes.rs` | 90 | `piloter_cote` (2 tests) |
| `documenter/mod.rs` | 62 | `ModifierVue`, `Feuille`, `Nomenclature` |

Pas de dossier `dessiner/` : il n'existe aucune commande de tracé distincte ; lignes, arcs, formes et hachures sont des `Annotation` créées par `Commande::Annoter` (`annoter/`). Les outils de tracé eux-mêmes sont dans `dessin/dao.rs` (moteur2d, lot 7) et dans l'interface. Pas de sous-fichiers par objet dans `modeliser/` (murs, dalles, toits…) : la création des murs, dalles et vues est dans `modele::creation` (§5.8 b 2), pas dans les commandes.

Rangement (commit `0682940`) : les bras du `match` de l'ancien `executer_interne` (860 lignes) sont déplacés tels quels dans une méthode `executer_<métier>` par dossier, terminée par `_ => unreachable!()` ; `socle/execution.rs` garde un `match` exhaustif sans bras générique, donc une variante ajoutée à `Commande` ne compile pas tant qu'elle n'est pas affectée à un métier. Contrôle : toutes les lignes de l'ancien `commandes/mod.rs` se retrouvent dans les nouveaux fichiers, hors déclarations `mod`/`use` et deux changements de visibilité.

#### (b) Décisions de frontière

| # | Sujet | Décision | Pourquoi | Alternative écartée |
|---|---|---|---|---|
| 1 | `collaboration::objets` et `collaboration::concurrence::carte_objet` (appelés par `verrous::verifier`, §5.5 point 5) | descendus dans `commandes::objets` avec ce dont `objets` dépend (`Objets`, `COLLECTIONS`, `PolitiqueCollection`, `politique_collection`, `collection_identifiee`) ; `collaboration` ré-exporte `Objets`, `objets` et `concurrence::carte_objet` sous leurs anciens chemins | ces symboles ne dépendent que de `modele::Projet` et `serde_json`. Le module `concurrence` entier ne descend pas : il dépend de `Changement`, `Transaction`, `Index` (reste de `collaboration`) ; seule `carte_objet` (fonction pure) était appelée | (i) coller `commandes` et `collaboration` : `collaboration` tire `yrs`, le stockage et `echanges` ; (ii) descendre dans `modele` : possible (mêmes dépendances), écarté car la table d'objets ne sert qu'à comparer deux états (verrous, transactions), pas au modèle ; (iii) injecter la comparaison dans `verifier` : change la signature d'`executer` |
| 2 | Ré-export par `collaboration` | conservé (`pub use commandes::objets::{Objets, objets}` ; `pub use commandes::objets::carte_objet` dans `concurrence`) | `Objets` : 58 occurrences dans `collaboration.rs` et les sous-modules via `use super::*` ; `objets` : appelé par `ui`, `main.rs`, `modele_tests` | remplacer tous les chemins : plus de 50 remplacements |
| 3 | Visibilités | `COLLECTIONS` et `PolitiqueCollection` passent `pub` (test `collaboration` sur le registre) ; `Historique::{cible, confirmer}` perdent `#[cfg(test)]` (appelés par un test resté dans `app`) ; `ExecutionInterne`, `ModificationInterne`, `identite` passent `pub(crate)` | exigé par la compilation | modifier le test pour appeler `cible_n`/`confirmer_n` |
| 4 | Chemins publics | tout est à la racine : `commandes::Commande` (et non `commandes::commandes::Commande`) | 50 remplacements dans `app`, chemins stables si le rangement interne change | garder un module façade `commandes::commandes` |
| 5 | Dépendances `scene`, `maillage` | non déclarées | aucun usage : les seuls appels à `maillage::depuis_natif` étaient dans les tests montés dans `app` | — |
| 6 | Aide « commandes » du pilote (`include_str!`) | sert `socle/commande.rs` (192 lignes : l'`enum`) au lieu de l'ancien `commandes/mod.rs` (1 216 lignes : `enum` + exécution) | le sujet documente « le contrat métier Rust sérialisable » ; l'ancien fichier n'existe plus | servir la concaténation des fichiers de métier |

Changements visibles hors code : le texte de l'aide « commandes » du pilote (point 6) et le chemin cité dans `docs/PILOTE.md` (embarqué dans l'aide du pilote), `docs/MODIFICATION.md` et `README.md`. Aucun changement de schéma JSON ni de comportement des commandes.

#### (c) Fichiers proches du plafond

Aucun fichier de `commandes` ne dépasse 600 lignes. `modifier/composees.rs` (557, dont 135 de tests) n'a pas été redécoupé : ses méthodes partagent un seul `impl ModificationInterne for Projet`. Le bras `Copier` (193 lignes) reste d'un bloc dans `modifier/copier.rs`. Hors de `commandes` : `collaboration/tests.rs` (1 385), `collaboration/stockage.rs` (626), `collaboration/session.rs` (604), à traiter au lot 9.

#### (d) Tests déplacés ou parqués

| Tests | Avant | Après | Raison |
|---|---|---|---|
| 11 tests de `historique`, `verrous`, `contextes` (8) et `commandes/modifier.rs`, `cotes.rs` (10) : 18 | `app` (`edition`) | `commandes` | suivent le code |
| `structures_creation_copie_rotation_suppression_et_validation`, `modifier_deplacer_copier_et_supprimer_une_coupe`, `annotations_et_quadrillages_se_copient_et_se_deplacent`, `copier_assemblage_et_dependances_puis_supprimer_sans_perte`, `erreurs_atomiques_et_niveaux_utilises`, `deplacement_rotation_menuiseries_et_annulation`, `tremie_interieure_et_exterieure` (7) | `edition/commandes/tests.rs` | `crates/app/src/commandes_tests/commandes.rs` (parqués ; accueil : echanges, lot 9) | leur projet de départ est lu par `echanges::depuis_ifc::convertir` ; le premier appelle aussi `export_ifc` et `maillage::depuis_natif`. Seuls les `use` changent |
| `mur_contraintes_phases_et_sous_projets_persistes`, `dalle_creation_contour_et_relecture`, `plafond_et_toiture_en_pente`, `vues_plan_et_coupe`, `mur_creation_enregistrement_relecture` (5) ; `scene_terre_plein` (1) | `app/src/modele_tests/{creation, elements_terre_plein}.rs` | `crates/maillage/tests/{creation, elements_terre_plein}.rs` (tests d'intégration, fichiers inchangés) | ne dépendent que de `modele`, `maillage`, `glam` |

Restent parqués dans `crates/app/src/modele_tests` (30 tests) et `formats_tests` (1). Quatre fichiers ne dépendent plus que de `modele` et `commandes` et peuvent rejoindre `crates/commandes/tests/` (non fait, à valider) : `elements_arbre.rs` (1), `elements_composant.rs` (1), `elements_piece.rs` (4), `murs_assemblage.rs` (2). Les autres appellent `echanges`, `collaboration` ou `dessin` (lots 7 et 9).

#### (e) Violations restantes (lots 7 à 9)

Concernant `commandes` : **aucune**. Relevé par `grep` des chemins `crate::<module>` entre `dessin`, `rendu`, `echanges`, `collaboration`, `pilote`, `config` (les références de `ui` et de `main.rs` ne sont pas relevées, lot 11) :

| De | Vers | Détail | Lot |
|---|---|---|---|
| `dessin`, `rendu`, `echanges` | — | aucune référence `crate::` vers un autre de ces modules | 7, 8, 9 |
| `collaboration` | echanges | 11 références (`cli.rs`, `partage.rs`, `stockage.rs`, `tests.rs`) ; sens autorisé | 9 |
| `collaboration` | commandes | `commandes::objets` (ré-exports), `Commande`, `Execution` ; sens autorisé | 9 |
| `pilote` | dessin, echanges, collaboration | `cli.rs` (1, 5, 1) ; sens autorisé | 10 |
| `pilote` | commandes | `include_str!("../../../commandes/src/socle/commande.rs")` (`mod.rs`, `mcp.rs`) : chemin de fichier entre crates, à remplacer par une constante exportée par `commandes` si le chemin gêne | 10 |
| `config` | echanges | `echanges::enregistrement` (1 référence ; sens autorisé) | 11 |
| `commandes_tests`, `modele_tests`, `formats_tests` | commandes, maillage, formats, echanges, collaboration, moteur2d | tests parqués (tableau (d)) | 7 à 9 |

Points ouverts : (1) `scene` et `maillage` restent « autorisées » pour `commandes` au §2 sans être utilisées : à retirer du tableau si l'on veut un graphe strict ; (2) déplacement des 8 tests de `modele_tests` vers `crates/commandes/tests/` ; (3) point ouvert du §5.10 (1) inchangé.

### 5.14 Après lot 7 (2026-11-16)

Tests : 390 réussis, 2 ignorés, comme avant le lot ; répartition `bimedit` 165 (avant : 199 + 1 ignoré), `moteur2d` 34 (+ 1 ignoré, `mesure::sphere_500k`), `commandes` 18, `formats` 52, `modele` 101 (+ 1 ignoré), `noyau` 10, `ifc_base` 2, `maillage` 8, `scene` 0. `cargo test -p moteur2d --release` passe seul (34 + 1 ignoré). `cargo tree -p moteur2d` ne contient ni `egui`, ni `wgpu`, ni `eframe`. `grep -rn "crate::" crates/moteur2d/src` : 2 chemins internes (`crate::coupe`, `crate::Poche`, dans un test). Sorties comparées à `bf0370e` (binaire de référence construit dans un worktree temporaire) : `--export-ifc` et `--quantites` de `samples/maison_native.modele.json` identiques octet pour octet ; `--stats` des deux IFC de `samples/` identiques hors ligne « Durée » ; `--export-vue` (`plan-rdc`, `plan-etage`, `coupe`) et `--export-feuille A01` en SVG identiques après tri des lignes et des sous-chemins `M…` de chaque attribut `d` ; en PDF, le binaire de référence n'est pas déterministe (deux exécutions diffèrent, l'ordre des tracés suit celui du SVG) : les flux décompressés, triés par ligne, sont identiques entre les deux exécutions de référence et la nouvelle.

#### (a) Contenu de `crates/moteur2d`

Package `moteur2d` (lib, 8 fichiers, 2 713 lignes, 35 tests dont 1 ignoré). Dépendances internes : `noyau`, `modele`, `scene`, `maillage`. Externes : `glam`, `rayon`, `svg2pdf`, `pdf-writer` (ces deux dernières retirées des dépendances d'`app`, qui ne les appelle plus).

| Chemin (`crates/moteur2d/src/`) | Lignes | Contenu | Origine |
|---|---|---|---|
| `lib.rs` | 187 | modules et ré-exports ; `Poche`, `TRAIT_FIN_MM`, `PAS_HACHURE_MM` ; `decouper`, `section` (privés) ; tests `mesure` et `test_isolant_arc` | `dessin/mod.rs` |
| `coupe.rs` | 373 | `coupe` ; privés `coupe_par_solides`, `fusionner_materiaux` | `dessin/mod.rs` |
| `vue.rs` | 292 | `sommets`, `sommets_boite`, `vu`, `cadrer_primitives` ; privé `maillage` | `dessin/mod.rs` |
| `tests.rs` | 383 | module `tests` (25 tests, chemins `tests::…` inchangés) | `dessin/mod.rs` |
| `motifs.rs` | 332 | `ligne_isolation` et aides (motif d'isolant) | `dessin/motifs.rs` |
| `dao.rs` | 795 | outils de tracé : `Trace`, `OutilDessin`, `EvenementTrace`, `Accroche`, `accrocher`, `polaire`, `ortho`, `lire_saisie`, `perimetre_cercle`, `milieu_arc`, `courbure_3_points`, `scinder_segment` | `dessin/dao.rs` |
| `feuilles.rs` | 274 | `dessin_vue`, `vue_placee`, `feuille`, `svg_feuille`, `pdf`, `pdf_pages`, `pdf_jeu` ; `ids_masques_vue` (`pub(super)`) | `dessin/feuilles.rs` (parqué au lot 2) |
| `plans.rs` | 77 | `fond_plan`, `limite_haute_plan`, `pointilles_plage_haute` | `dessin/plans.rs` (parqué au lot 2) |

API publique à la racine inchangée (`moteur2d::coupe`, `moteur2d::Poche`, `moteur2d::dao::Trace`, etc. : mêmes noms qu'en `crate::dessin::…`). `feuille` et `pdf_pages` sont `pub` dans un module privé et non ré-exportés, comme avant. Appelants dans `app` : `ui` (14 fichiers), `pilote/cli.rs`, `modele_tests/documentation.rs` ; 48 lignes `crate::dessin::` -> `moteur2d::` (et `use crate::dessin;` / `dessin::X` -> `moteur2d::X` dans `pilote/cli.rs` et `ui/coupe.rs`).

#### (b) Décisions de frontière

| # | Sujet | Décision | Pourquoi |
|---|---|---|---|
| 1 | Coupes préalables (commit a) | aucune, pas de commit a | `dessin` n'avait plus aucune référence `crate::` hors de lui-même (une seule, `crate::dessin::coupe` dans un test, devenue `crate::coupe`) : ni `config`, ni `rendu`, ni `echanges`, ni `collaboration`, ni `ui`, ni `pilote`, ni egui (couleurs déjà en `[u8; 4]`, plumes et constantes dans `noyau::aretes`) |
| 2 | `formats`, `ifc_base` | non déclarées | aucun usage (`Id` vient de `noyau`, `Motif` de `noyau::motif`) |
| 3 | `commandes` | non déclarée (reste autorisée au §2) | aucun usage en production ni dans les tests de `dessin` |
| 4 | `maillage` | dépendance de production | `feuilles.rs` (`dessin_vue`) appelle `depuis_natif` ; les tests de `lib.rs`/`tests.rs` aussi |
| 5 | Visibilités | aucune élargie | la compilation n'a rien exigé ; les fonctions déplacées par le découpage restent privées ou gardent leur visibilité (`decouper`, `section` restent à la racine, visibles des sous-modules) |
| 6 | Découpage de `dessin/mod.rs` (1 226 lignes) | commit séparé `6405175` : `coupe.rs`, `vue.rs`, `tests.rs` | déplacement de fonctions entières (contrôle : toutes les lignes non vides de l'ancien fichier se retrouvent, hors `mod tests {` et son `}`) ; avertissements clippy de la crate identiques avant et après |

Aucun `include_str!` dans `moteur2d`. Aucun changement fonctionnel ni de format.

#### (c) Fichiers au-delà du plafond

`dao.rs` (795 lignes, dont 223 de tests en 3 modules) : non découpé ; fichier déplacé tel quel, la partie de production (572 lignes) est sous le plafond, le dépassement vient des tests. Un découpage `dao/tests.rs` serait un simple déplacement, laissé hors de ce lot (non demandé, fichier non créé au lot). Aucun autre fichier de `moteur2d` ne dépasse 600 lignes.

#### (d) Tests

Les 35 tests de `dessin` (`mod.rs` 25 dont 1 ignoré, `dao.rs` 9, `motifs.rs` 1) ont suivi le code : ils n'appellent que `noyau`, `modele`, `scene`, `maillage` (`depuis_natif`). Aucun test n'a dû rester ou monter dans `app`.

Tests parqués dans `app` qui appellent `moteur2d` : `modele_tests/documentation.rs` (14 tests, §5.8 d). 8 appellent `echanges::depuis_ifc` (accueil : echanges, lot 9) ; 6 n'utilisent que `modele`, `commandes`, `moteur2d`, `noyau`, `glam` et pourraient rejoindre `crates/moteur2d/tests/` (non fait, à valider) : `dessin_sur_feuille_suit_le_fond_la_plage_haute_et_le_style`, `coupe_sur_feuille_reste_dans_les_limites_de_la_vue`, `formes_du_cartouche_persistent_et_sont_dessinees_sur_feuille`, `famille_cartouche_partagee_est_enregistree_et_utilisee`, `polyligne_types_de_trait_par_segment`, `polyligne_segments_d_arc` (cela ferait de `commandes` une dépendance de développement de `moteur2d`).

#### (e) Violations restantes (lots 8 à 11)

Concernant `moteur2d` : **aucune**. Relevé par `grep` des chemins `crate::<module>` entre `rendu`, `echanges`, `collaboration`, `pilote`, `config`, `ui` (hors `ui` et `main.rs` comme sources, lot 11) :

| De | Vers | Détail | Lot |
|---|---|---|---|
| `rendu` | — | aucune référence `crate::` vers `echanges`, `collaboration`, `pilote`, `config`, `ui` | 8 |
| `collaboration` | echanges | 11 références ; sens autorisé | 9 |
| `pilote` | echanges, collaboration | `cli.rs` : 5 et 1 références ; sens autorisé | 10 |
| `config` | echanges | 1 référence (`enregistrement`) ; sens autorisé | 11 |
| `commandes_tests`, `modele_tests`, `formats_tests` | commandes, maillage, formats, echanges, collaboration, moteur2d | tests parqués (§5.13 d, (d) ci-dessus) | 8 à 9 |

Points ouverts : (1) déplacement des 6 tests de `modele_tests/documentation.rs` vers `crates/moteur2d/tests/` ; (2) `commandes` autorisée mais non utilisée par `moteur2d` au §2 (comme `scene`/`maillage` pour `commandes`, §5.13) ; (3) découpage des tests de `dao.rs`.

### 5.15 Après lot 8 (2026-11-16)

Tests : 390 réussis, 2 ignorés, comme avant le lot ; répartition `bimedit` 153 (avant : 165), `moteur3d` 12, `moteur2d` 34 (+ 1 ignoré), `commandes` 18, `formats` 52, `modele` 101 (+ 1 ignoré), `noyau` 10, `ifc_base` 2, `maillage` 8, `scene` 0. `cargo test -p moteur3d --release` passe seul (12). `grep -rn "crate::" crates/moteur3d/src` : 5 chemins, tous internes (`crate::ciel`, `crate::camera`). `cargo tree -p moteur3d` : aucune crate interne autre que `noyau`, `modele`, `scene` (+ `maillage` en dev). Le rendu GPU et l'interface n'ont pas pu être lancés (pas d'affichage) : `.wgsl` inchangés octet pour octet (`git diff -M` : 0 ligne), code GPU uniquement déplacé.

#### (a) Contenu de `crates/moteur3d`

Package `moteur3d` (lib, 4 068 lignes dont 1 171 de WGSL, 12 tests). Dépendances internes : `noyau` (`constantes::MAX_APPARENCES`), `modele` (`Apparence`, `CarteTexture`), `scene` (`Scene`, `Vertex`) ; dev : `maillage`, `modele`. Externes : `eframe` (egui, egui_wgpu, wgpu, naga des tests), `glam`, `image` (textures des apparences), `earcutr` (triangulation du rectangle de sélection) ; `image` et `earcutr` ne sont plus déclarées dans `app`.

| Fichier (`crates/moteur3d/src/`) | Lignes | Contenu | Origine |
|---|---|---|---|
| `lib.rs` | 6 | déclaration des modules (tous `pub`) | `rendu/mod.rs` |
| `gpu.rs` | 1 416 | `Renderer` (`new`, uploads, `paint`, `capture`), `FrameParams`, `StyleVisuel`, `CielRendu`, passes ; 2 tests de validation des shaders (naga) | `rendu/gpu.rs` |
| `ciel.rs` | 453 | ciel physique (tables de transmittance et de diffusion, vue du ciel, `couleur_soleil`) ; 2 tests | `rendu/ciel.rs` |
| `camera.rs` | 260 | `Camera`, `MAX_PITCH` ; 1 test | `rendu/camera.rs` |
| `orientation_plan.rs` | 503 | repère des plans (egui) ; 5 tests | `rendu/orientation_plan.rs` |
| `selection_rectangle.rs` | 259 | sélection par rectangle (egui) ; 2 tests | `rendu/selection_rectangle.rs` |
| `scene.wgsl`, `commun.wgsl`, `ao.wgsl` | 1 008, 51, 112 | shaders, `include_str!` relatifs, inchangés | `rendu/*.wgsl` |

Appelants dans `app` : 10 fichiers de `ui` (`crate::rendu::X` -> `moteur3d::X`, noms de modules inchangés : `gpu`, `camera`, `ciel`, `orientation_plan`, `selection_rectangle`). Aucun ré-export, aucune visibilité élargie.

#### (b) Décisions de frontière

| # | Sujet | Décision | Pourquoi |
|---|---|---|---|
| 1 | Coupes préalables (commit a) | aucune, pas de commit a | `rendu` n'avait aucune référence `crate::` hors de lui-même (ni `config`, `echanges`, `collaboration`, `ui`, `pilote`, `edition`) |
| 2 | `egui`, `wgpu` | via `eframe.workspace = true` | `app` les obtient par `eframe` (`eframe::egui`, `eframe::wgpu`, `eframe::egui_wgpu`) ; aucun `egui`/`wgpu` au `[workspace.dependencies]` ; garder `eframe` évite de changer les versions et les features. Conséquence : moteur3d tire tout `eframe` ; la séparation fine (`egui`, `egui-wgpu`, `wgpu` directs) reste possible plus tard, à versions et features égales |
| 3 | `moteur2d` | non déclarée (reste autorisée au §2) | aucun usage |
| 4 | `maillage` | dev-dependency | seul le test `fenetre_croisement_visibilite_et_coupe` appelle `depuis_natif` |
| 5 | `image`, `earcutr` | déclarées dans moteur3d, retirées d'`app` | seul `rendu` les appelait |
| 6 | `COMMUN` du ciel et `COMMUN` de `gpu.rs` | inchangés | deux constantes homonymes de modules différents |

Commentaire de `scene.wgsl` (« voir `rendu::ciel` ») laissé tel quel pour garder le shader identique ; le module s'appelle désormais `moteur3d::ciel`.

#### (c) Fichiers au-delà du plafond

`gpu.rs` (1 416 lignes) non découpé. `Renderer` est une structure de 66 champs privés avec un seul `impl` ; `new` fait environ 360 lignes (l. 320 à 682) et `rendre` environ 210. Un découpage par modules exigerait des `impl Renderer` répartis et des champs `pub(super)`, ou le découpage de `new` : changements de code, pas de simple déplacement. Seules les définitions de types et constantes (l. 13 à 317, ~300 lignes) se déplaceraient sans risque, ce qui ne ramène pas le fichier sous 600 lignes. `scene.wgsl` (1 008 lignes) : shader, non découpé (concaténation `COMMUN` + `SCENE` à préserver). Autres fichiers : sous 600 lignes (`orientation_plan.rs` 503, dont ~175 de tests).

#### (d) Tests

Les 12 tests de `rendu` ont suivi le code : `gpu.rs` 2, `ciel.rs` 2, `camera.rs` 1, `orientation_plan.rs` 5, `selection_rectangle.rs` 2 dont `fenetre_croisement_visibilite_et_coupe` (parqué depuis le lot 5 ; n'utilise que `modele`, `maillage`, `scene`, `glam`, `egui`). `bimedit` : 165 -> 153. Aucun test n'a dû rester dans `app` pour moteur3d. Les tests parqués des autres lots (`modele_tests`, `formats_tests`, `commandes_tests`, tableau du §5.12 d) sont inchangés.

#### (e) Violations restantes (lots 9 à 11)

Concernant `moteur3d` : **aucune**.

| De | Vers | Détail | Lot |
|---|---|---|---|
| `collaboration` | echanges | 11 références ; sens autorisé | 9 |
| `pilote` | echanges, collaboration | `cli.rs` ; sens autorisé | 10 |
| `config` | echanges | 1 référence (`enregistrement`) ; sens autorisé | 11 |
| `ui` | moteur3d, moteur2d, maillage, commandes, echanges, … | sens autorisé | 11 |
| `modele_tests`, `formats_tests`, `commandes_tests` | commandes, maillage, formats, echanges, collaboration, moteur2d | tests parqués | 9 |

Points ouverts : (1) décider si `egui`/`egui-wgpu`/`wgpu` deviennent des dépendances directes au lieu d'`eframe` (point 2 ci-dessus) ; (2) découpage de `gpu.rs` si le refactor de `Renderer::new` est accepté (changement de code) ; (3) le §5.4 prévoyait 14 tests pour `rendu` ; le décompte réel est 12.

### 5.16 Après lot 9 (2026-11-16)

Tests : 390 réussis, 2 ignorés, comme avant le lot ; répartition `bimedit` 82 (avant : 153), `collaboration` 43, `echanges` 28, `moteur3d` 12, `moteur2d` 34 (+ 1 ignoré), `commandes` 18, `formats` 52, `modele` 101 (+ 1 ignoré), `noyau` 10, `ifc_base` 2, `maillage` 8, `scene` 0. Les 4 lignes « 1 passed » en plus sont les sorties de sous-processus des tests de `collaboration` (`course_entre_deux_processus…`, `crash_processus…`), qui relancent le binaire de test. `cargo test -p echanges --release` (9 unitaires + 19 d'intégration) et `cargo test -p collaboration --release` (41 unitaires + 2 d'intégration) passent seuls. `grep -rn "crate::" crates/echanges/src` : 4 chemins, tous internes (`crate::rapport`, `crate::depuis_rvt`, `crate::depuis_ifc`) ; `crates/collaboration/src` : aucun. `cargo tree` ne contient ni `egui`, ni `wgpu`, ni `eframe` pour les deux crates. Sorties comparées à `01c3d9f` (binaire de référence construit dans un worktree temporaire) : `--export-ifc` et `--quantites` de `samples/maison_native.modele.json` identiques octet pour octet ; `--stats` des deux IFC de `samples/` identiques hors « Durée » ; `--modele` (import IFC -> natif, rapport inclus) puis `--export-ifc`/`--quantites` du natif relu : identiques hors identifiants aléatoires (UUID du natif, GUID IFC) ; `--export-vue` identique après tri des lignes et des sous-chemins (la référence n'est pas déterministe) ; scénario `--collaboration` sans réseau (dépôt partagé = dossier local) : `associer`, `etat`, `reserver`, `editer`, `synchroniser`, `instantane`, `liberer`, `recuperer`, `auditer`, `sauvegarde`, action inconnue : sorties, codes de retour, `recuperer` et liste des fichiers du dépôt identiques (UUID masqués).

#### (a) Contenu des deux crates

`crates/echanges` (lib, 6 fichiers, 1 653 lignes, 9 tests unitaires + 19 d'intégration). Dépendances internes : `noyau` (`uuid`), `modele`, `ifc_base`, `formats`. Externes : `glam`, `rayon` (retirée d'`app`). Dev : `commandes`, `maillage`, `moteur2d`, `serde_json`.

| Fichier (`crates/echanges/src/`) | Lignes | Contenu |
|---|---|---|
| `lib.rs` | 6 | modules (tous `pub`) |
| `depuis_ifc.rs` | 358 | `convertir` : IFC -> `Projet` natif, mesures ; 2 tests |
| `depuis_rvt.rs` | 575 | `convertir`, `convertir_maquette` : Revit -> `Projet` ; 4 tests (fichiers d'essai hors dépôt, sautés si absents) |
| `export_ifc.rs` | 396 | `exporter` : `Projet` -> IFC ; 1 test |
| `enregistrement.rs` | 188 | `enregistrer_natif`, `ecrire_atomique`, récupération, `SourceRvt`, `Bilan` ; 2 tests |
| `rapport.rs` | 130 | `markdown` : rapport d'import |

`crates/collaboration` (lib, 8 fichiers, 4 419 lignes, 41 tests unitaires + 2 d'intégration). Dépendances internes : `noyau` (`hash`, `uuid`), `modele`, `maillage` (`cli.rs` : `depuis_natif`), `commandes` (`objets`, `Commande`, `Execution`), `echanges` (`enregistrement`, 11 appels). Externes : `yrs` (retirée d'`app`), `serde`, `serde_json`, `glam`.

| Fichier (`crates/collaboration/src/`) | Lignes | Contenu |
|---|---|---|
| `lib.rs` | 494 | `PROTOCOLE`, `projet`, `Transaction`, `Changement`, `Bail`, validation et fusion de transactions |
| `session.rs` | 604 | `Session` (copie locale, synchronisation, réservations, annulation) |
| `stockage.rs` | 626 | `Depot` (journal, instantanés, verrou fichier, reprise après crash) |
| `concurrence.rs` | 535 | décomposition des changements, empreintes de voisinage et de zones |
| `partage.rs` | 384 | fichiers `.bimpartage`, création et ouverture d'un projet partagé ; 5 tests |
| `texte.rs` | 170 | `Editeur`, `EtatTexte` (texte collaboratif `yrs`) |
| `cli.rs` | 222 | `--collaboration …` (administration reproductible, mesures) |
| `tests.rs` | 1 384 | 36 tests (dont les 2 qui relancent le binaire de test) |

#### (b) Décisions de frontière

| # | Sujet | Décision | Pourquoi |
|---|---|---|---|
| 1 | Coupes préalables (commit a) | aucune, pas de commit a | `echanges` et `collaboration` n'avaient aucune référence `crate::` vers `config`, `ui`, `pilote`, `moteur2d`, `moteur3d` ; `echanges` ne référençait pas `collaboration` ; `collaboration::uuid`/`hash` étaient déjà dans `noyau` |
| 2 | `commandes` dans `echanges` | dev-dependency seulement | aucun usage en production ; les 7 tests de `commandes` et les tests de `modele` déplacés l'appellent |
| 3 | `maillage`, `moteur2d` dans `echanges` | dev-dependencies | `maillage::depuis_natif` dans des tests ; `moteur2d` pour 8 tests de `documentation` ; `scene` non déclarée |
| 4 | `ifc_base` dans `echanges` | dépendance de production | `depuis_ifc`/`export_ifc` manipulent `StepFile`, `IfcModel` (autorisé, §2) |
| 5 | Ré-exports de transition de `collaboration` | retirés | `pub use commandes::objets::{Objets, objets}` (lib) et `pub use commandes::objets::carte_objet` (`concurrence`) deviennent des `use` privés ; `collaboration::objets` -> `commandes::objets::objets` dans `ui/collaboration.rs`, `ui/pilotage.rs`, `ui/saisie_plan.rs` et les tests ; `concurrence::carte_objet` -> `commandes::objets::carte_objet` (lib). `collaboration::projet` reste (défini dans la crate) |
| 6 | Noms de modules | `echanges::{depuis_ifc,…}` et `collaboration::{session,partage,cli,…}` identiques aux anciens `crate::…` | dans `ui`, un module local `ui::collaboration` masque la crate : les chemins s'y écrivent `::collaboration::…` |
| 7 | Sous-processus des tests | `--exact "collaboration::tests::enfant_…"` -> `"tests::enfant_…"` (3 endroits de `tests.rs`) | le test est désormais à la racine de la crate : chemin de test sans préfixe |
| 8 | `include_str!` / `samples/` | `../../../../samples/` -> `../../../samples/` (`partage.rs`, `tests.rs`) ; `../../../modele/src/lib.rs` -> `../../modele/src/lib.rs` (`tests.rs`) | fichiers un niveau plus haut ; les tests d'`echanges` gardent `CARGO_MANIFEST_DIR/../../samples` (même profondeur) |
| 9 | Dépendances d'`app` | `yrs` et `rayon` retirées | plus aucun appel dans `app` |

Aucun `pub(crate)` élargi. Aucun changement fonctionnel ni de format.

#### (c) Fichiers au-delà du plafond

`collaboration/src/tests.rs` (1 384), `stockage.rs` (626) et `session.rs` (604) : non découpés. `tests.rs` est un fichier de tests déplacé tel quel (36 fonctions, helpers partagés `dossier`, `exemple`). `stockage.rs` (un `impl Depot` et ses aides) et `session.rs` (un `impl Session`) : un découpage répartirait des `impl` et élargirait des visibilités, ce n'est pas un déplacement mécanique. Dans `echanges`, `depuis_rvt.rs` (575, dont ~100 de tests) et les autres fichiers sont sous 600.

#### (d) Tests déplacés et parqués

Déplacés vers `crates/echanges/tests/` (19 tests d'intégration, mêmes noms et assertions ; seuls les `use` changent, `crate::echanges::` -> `echanges::`) :

| Fichier | Tests | Origine |
|---|---|---|
| `commandes.rs` | 7 : `structures_creation_copie_rotation_suppression_et_validation`, `modifier_deplacer_copier_et_supprimer_une_coupe`, `annotations_et_quadrillages_se_copient_et_se_deplacent`, `copier_assemblage_et_dependances_puis_supprimer_sans_perte`, `erreurs_atomiques_et_niveaux_utilises`, `deplacement_rotation_menuiseries_et_annulation`, `tremie_interieure_et_exterieure` | `app/src/commandes_tests/commandes.rs` (dossier supprimé) |
| `couches_rvt.rs` | 1 : `couches_projet1_lecture_et_conversion` (fichier d'essai hors dépôt : sauté s'il est absent) | `app/src/formats_tests/mod.rs` (dossier supprimé) |
| `documentation.rs` | 8 : `dessin_vue_applique_le_contexte_propre_a_la_vue`, `dessin_vue_suit_le_gabarit_partage`, `vue_placee_suit_echelle_vue_meme_avec_ancienne_valeur`, `vue_placee_respecte_rotation_et_limite_basse`, `vue_placee_respecte_masquage_et_cadrage`, `documents_compatibles_v0_references_et_pdf`, `polyligne_fermee_remplie`, `nomenclature_csv_et_quantites_sans_double_compte` | `modele_tests/documentation.rs` |
| `elements_escalier.rs` | 1 : `commandes_escalier_et_garde_corps` | `modele_tests/elements_escalier.rs` (fichier supprimé) |
| `elements_site.rs` | 1 : `creation_site_sauvegarde_export_et_suppression` | `modele_tests/elements_site.rs` |
| `volumes_solides.rs` | 1 : `conversion_ifc_rouvre_et_indices_invalides_refuses` | `modele_tests/volumes_solides.rs` |

Déplacés vers `crates/collaboration/tests/` (2 tests, aucune dev-dependency supplémentaire) : `famille_profil.rs` (`profils_du_gabarit_varient_et_survivent_au_rechargement`, de `modele_tests/famille_profil.rs`, fichier supprimé) et `gabarits_vues.rs` (`gabarit_partage_actualise_detache_et_se_recharge`, de `modele_tests/gabarits_vues.rs` ; la ligne `use collaboration;` interne est retirée). Les 36 tests de l'ancien `collaboration/tests.rs` et les 5 de `partage.rs` ont suivi le code ; aucun ne dépend de `config`, `ui` ou `pilote`.

Dev-dependencies de `echanges` (`commandes`, `maillage`, `moteur2d`, `serde_json`) : acceptées pour ces tests d'intégration ; elles ne sont pas des dépendances de production (`cargo tree -p echanges` sans `--edges dev`).

Parqués dans `crates/app/src/modele_tests/` (17 tests, liste vérifiée par `grep -c "#\[test\]"`) : aucun n'appelle `echanges` ni `collaboration` ; accueil possible : `commandes` (11 : `elements_arbre` 1, `elements_composant` 1, `elements_piece` 4, `murs_assemblage` 2, `elements_site` 1 (`site_en_phase_existant_cotes_ngf_et_epaisseur_du_type`, aussi `maillage`), `gabarits_vues` 1 (`invalides_et_modifications_de_phases_restent_coherentes`), `volumes_solides` 1 (`recalcul_partiel_identique_apres_modification`)) et `moteur2d` (6 de `documentation.rs`, §5.14 d). Non déplacés (à valider) ; `moteur2d` y gagnerait `commandes` en dev-dependency. `formats_tests` et `commandes_tests` n'existent plus.

#### (e) Violations restantes (lots 10 et 11)

Concernant `echanges` et `collaboration` : **aucune** (ni `config`, `ui`, `pilote`, `moteur2d`, `moteur3d`).

| De | Vers | Détail | Lot |
|---|---|---|---|
| `pilote` | echanges, collaboration | `cli.rs` : 5 références à `echanges::`, 1 à `collaboration::cli::executer` ; sens autorisé | 10 |
| `pilote` | commandes | `include_str!("../../../commandes/src/socle/commande.rs")` (`mod.rs`, `mcp.rs`) : chemin de fichier entre crates (§5.13 e) | 10 |
| `config` | echanges | `options/mod.rs` : `echanges::enregistrement::ecrire_atomique` (1) ; sens autorisé | 11 |
| `ui` | echanges, collaboration | 43 lignes `echanges::` (11 fichiers), 6 `::collaboration::` (3 fichiers) ; sens autorisé | 11 |
| `main.rs` | echanges, collaboration | 2 lignes `echanges::` (`--modele`), 1 `collaboration::cli::executer` ; sens autorisé | 11 |
| `modele_tests` | commandes, maillage, moteur2d | 17 tests parqués (d) | 10 à 11 |

Points ouverts : (1) déplacement des 11 tests de `modele_tests` vers `crates/commandes/tests/` et des 6 de `documentation.rs` vers `crates/moteur2d/tests/` ; (2) `scene` autorisée mais inutilisée par `echanges` et `collaboration`, `maillage` par `echanges` (en dev seulement) au §2 ; (3) découpage de `stockage.rs` et `session.rs` ; (4) l'identifiant de test des sous-processus (`tests::enfant_…`) dépend du nom du module de tests : à garder si `tests.rs` est déplacé.

### 5.17 Après lot 10 (2026-11-16)

Tests : 390 réussis, 2 ignorés, comme avant le lot ; répartition `bimedit` 70 (avant : 82), `pilote` 12, `collaboration` 43, `echanges` 28, `moteur3d` 12, `moteur2d` 34 (+ 1 ignoré), `commandes` 18, `formats` 52, `modele` 101 (+ 1 ignoré), `noyau` 10, `ifc_base` 2, `maillage` 8, `scene` 0 (la somme brute des lignes « test result » donne 394 : 4 lignes sont des sorties de sous-processus de `collaboration`). `cargo test -p pilote --release` : 12 réussis. `grep -rn "crate::" crates/pilote/src` : une seule ligne, interne (`crate::instances` dans `mcp.rs`). Sorties comparées à `986dd4e` (binaire de référence construit dans un worktree temporaire) : `--export-ifc` et `--quantites` de `samples/maison_native.modele.json` identiques octet pour octet ; `--stats` de `test_ifc4.ifc` et `test_ifc4_murs.ifc` identiques hors « Durée » ; échange MCP sans interface (`initialize` puis `tools/list` sur stdin de `bimedit --mcp`) : sortie identique octet pour octet.

#### (a) Contenu de `crates/pilote`

Lib, 6 fichiers, 1 410 lignes, 12 tests. Dépendances internes : `noyau`, `modele`, `maillage`, `commandes`, `moteur2d`, `echanges`, `collaboration`. Externes : `eframe`, `png`, `serde`, `serde_json`, `serde_ignored` (retirée d'`app`), `uuid`.

| Fichier (`src/`) | Lignes | Contenu |
|---|---|---|
| `lib.rs` | 146 | écoute TCP (`demarrer`, `Requete`, `PORT`), helpers JSON (`texte`, `objet`, `liste`, `erreur`, `succes`), `png`, `documentation` (aide, `include_str!`) |
| `mcp.rs` | 607 | serveur MCP stdio (`bimedit --mcp`) ; 6 tests |
| `cli.rs` | 94 | `--editer`, `--export-ifc`, `--export-vue`, `--export-feuille`, `--quantites`, `--mesurer-scene` |
| `edition.rs` | 198 | transactions et modifications du projet natif demandées par le pilote ; 3 tests |
| `instances.rs` | 160 | registre des instances, jeton, authentification ; 2 tests |
| `saisie.rs` | 205 | entrées egui validées avant leur mise en file ; 1 test |

#### (b) Décisions de frontière

| # | Sujet | Décision | Pourquoi |
|---|---|---|---|
| 1 | Coupes préalables (commit a) | aucune, pas de commit a | `pilote` n'avait aucune référence `crate::` vers `config` ni `ui` ; le test cité au §5.5 (`Gabarit::defaut`) n'existe plus (`modele::gabarit`) |
| 2 | `eframe` | dépendance de production conservée | `saisie.rs` construit des `egui::Event` (`Event::PointerButton`, `PointerMoved`, `MouseWheel`, `Key`, `Text`), `Modifiers`, `PointerButton`, `Pos2`, `Key::from_name` ; `lib.rs::demarrer` reçoit un `egui::Context` pour `request_repaint`. Un type neutre déplacerait la conversion vers `ui` : changement structurel non fait |
| 3 | `include_str!` de `docs/` | `../../../../docs/` -> `../../../docs/` (`lib.rs`, `mcp.rs`) | crate un niveau plus haut |
| 4 | `include_str!` de `commandes` | `../../../commandes/src/socle/commande.rs` -> `../../commandes/src/socle/commande.rs` ; pas de constante exportée par `commandes` | chemin entre crates conservé, relatif correct ; une constante serait une API nouvelle |
| 5 | `use super::instances` | -> `use crate::instances` (`mcp.rs`) | `mod.rs` est devenu `lib.rs` |
| 6 | Appelants | `crate::pilote::` -> `pilote::` (`main.rs`, `ui/mod.rs`) ; `mod pilote;` retiré ; `commandes_pilote.rs` et `pilotage.rs` restent dans `ui` | modules d'interface ; aucun module de `ui` ne porte le nom `pilote` : pas besoin de `::pilote::` |
| 7 | Dépendances d'`app` | `png` et `serde_ignored` retirées | plus aucun appel dans `app` |

Aucune visibilité élargie. Aucun changement fonctionnel ni de format.

#### (c) Fichiers au-delà du plafond

`mcp.rs` (607) : laissé entier. Les autres fichiers sont sous 600.

#### (d) Tests déplacés et parqués

Les 12 tests suivent le code (mêmes noms et assertions) : `edition.rs` 3, `instances.rs` 2, `mcp.rs` 6, `saisie.rs` 1 ; aucun ne dépend de `config` ni de `ui`. Aucun test de `pilote` ne reste dans `app`. Les 17 tests de `modele_tests` restent parqués (§5.16 d).

#### (e) Violations restantes (lot 11)

Concernant `pilote` : **aucune** (ni `config`, ni `ui`). Egui reste en dépendance de `pilote` (b 2).

| De | Vers | Détail | Lot |
|---|---|---|---|
| `ui` | pilote | `commandes_pilote.rs`, `pilotage.rs`, `mod.rs` : `pilote::{erreur, texte, objet, succes, edition, saisie, instances, demarrer, Requete}` ; sens autorisé | 11 |
| `main.rs` | pilote | `pilote::mcp::servir`, `pilote::cli::executer`, `pilote::instances::retirer`, `pilote::texte` ; sens autorisé | 11 |
| `config` | echanges | `options/mod.rs` : `echanges::enregistrement::ecrire_atomique` (1) ; sens autorisé | 11 |
| `modele_tests` | commandes, maillage, moteur2d | 17 tests parqués (§5.16 d) | 11 |

Point ouvert : l'aide « commandes » du pilote embarque `commandes/src/socle/commande.rs` par chemin de fichier ; renommer ou déplacer ce fichier casse la compilation de `pilote`.
