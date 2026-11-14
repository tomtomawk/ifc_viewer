# Architecture : découpage en workspace Cargo

État au lot 2 : le dépôt est un workspace (`Cargo.toml` racine, `resolver = "3"`, `members = ["crates/*"]`). `noyau` (lot 1) et `modele` (lot 2) sont extraits en `crates/noyau` et `crates/modele` ; le reste du code est encore dans une seule crate, `crates/app` (package et binaire `bimedit`). Les dépendances sont déclarées une fois dans `[workspace.dependencies]` (racine) et reprises par `dep.workspace = true`. `Cargo.lock` et `target/` restent à la racine.

Restent à la racine : `docs/`, `samples/`, `gabarits/`, `tools/`, `.github/`, `.mcp.json`, `README.md`, `AGENTS.md`. Les ressources embarquées (`include_str!`) pointent vers ces dossiers par des chemins relatifs depuis `crates/<crate>/src/...` (le gabarit par défaut est embarqué par `crates/modele/src/gabarit.rs`) ; les tests lisent `samples/` via `concat!(env!("CARGO_MANIFEST_DIR"), "/../../samples/...")` (le répertoire courant d'un test est celui de sa crate ; même profondeur pour `crates/app` et `crates/modele`).

## 1. Arborescence cible

```
crates/
  noyau/          géométrie, CSG, texte, primitives 2D ; aucune dépendance interne
  modele/         données, schéma JSON, validation : murs, elements, famille, documentation
  scene/          scène triangulée neutre, identifiants, picking (contient Vec<modele::Apparence>)
  formats/        ifc, rvt
  maillage/       modèle -> scène : volumes, jonctions, depuis_natif
  commandes/      transactions, historique, verrous ; modeliser/ modifier/ dessiner/ annoter/ documenter/
  moteur2d/       coupe, arêtes, poché, motifs, SVG/PDF
  moteur3d/       wgpu, shaders, caméra, ciel, sélection à l'écran
  echanges/       import IFC/RVT, export IFC, enregistrement
  collaboration/
  pilote/         MCP, CLI
  config/
  interface/      egui ; UNE crate (`impl ViewerApp` réparti sur 48 fichiers) ;
                  socle/ zones/ saisie/ outils/{modeliser,modifier,dessiner,annoter}/ documents/ familles/ projet/
  app/            binaire `bimedit`
```

Correspondance avec les dossiers actuels (après lot 2) : `crates/noyau`, `crates/modele` : extraits ; dans `crates/app/src/` : `rendu/scene/mod.rs` -> scene ; `formats/` -> formats ; `rendu/scene/natif.rs` -> maillage ; `edition/` -> commandes ; `dessin/` (dont `feuilles.rs` et `plans.rs`, rendu de vue et de feuille parqué au lot 2) -> moteur2d ; `rendu/` (hors `scene/`) -> moteur3d ; `echanges/` -> echanges ; `collaboration/` et `collaboration.rs` -> collaboration ; `pilote/`, `config/` ; `ui/` -> interface ; `main.rs` -> app ; `modele_tests/` : tests parqués, à répartir (§5.8 d).

## 2. Graphe de dépendances autorisées

Ordre cible, hypothèse de départ (flèche = « dépend de ») :

```
noyau <- modele <- scene <- formats <- maillage <- commandes <- moteur2d <- moteur3d
      <- echanges <- collaboration <- pilote <- config <- interface <- app
```

| Crate | Dépendances internes autorisées |
|---|---|
| noyau | aucune |
| modele | noyau |
| scene | noyau, modele |
| formats | noyau, scene |
| maillage | noyau, modele, scene, formats |
| commandes | noyau, modele, scene, maillage |
| moteur2d | noyau, modele, scene, commandes |
| moteur3d | noyau, modele, scene, moteur2d |
| echanges | noyau, modele, scene, formats, maillage |
| collaboration | modele, commandes, echanges |
| pilote | collaboration et en dessous |
| config | tout ce qui précède, hors interface |
| interface | tout ce qui précède |
| app | interface (et config, pilote pour le démarrage) |

Règle générale : une crate n'importe que des crates situées avant elle dans l'ordre. Aucun cycle. Les dépendances externes (wgpu, egui, etc.) suivent : `wgpu` uniquement dans moteur3d ; `egui`/`eframe` seulement à partir de moteur3d (voir §5.4). L'ordre final est confirmé ; les coupes qu'il impose sont planifiées au §4 (écarts au §5.5). `volumes` reste dans `modele`, donc `maillage` ne contient que `scene/natif.rs`.

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
| 3 | scene | `load`, `assemble`, `assemble_avec` (constructeurs IFC) vont dans `formats` ; plus de `formats`/`dessin` dans `scene`. §5.5 point 1 | **Fable** | à faire |
| 4 | formats | extraction de `ifc/`, `rvt/` + `load`/`assemble` ; tests croisés montés | Sonnet | à faire |
| 5 | maillage | `scene/natif.rs` seul (`depuis_natif`, `depuis_natif_verifie` ; `id_natif` et `numero_vue` sont dans `modele::identifiants` depuis le lot 2) ; les tests de `modele_tests`/`dessin` qui l'appellent y montent | **Fable** | à faire |
| 6 | commandes | `crates/app/src/edition` (commandes, historique, verrous, gestion des contextes ; traits d'extension de `Projet`, §5.8 b) ; décider où vont `concurrence` et `objets` (§5.5 point 5) ; rangement `modeliser/ modifier/ dessiner/ annoter/ documenter/` | **Fable** | à faire |
| 7 | moteur2d | `dessin/` dont le rendu de vue et de feuille parqué (`dessin/feuilles.rs`, `dessin/plans.rs`) | **Fable** | à faire |
| 8 | moteur3d | `rendu/` hors scène | Sonnet | à faire |
| 9 | echanges + collaboration | `crates/app/src/echanges`, `crates/app/src/collaboration*` | Sonnet | à faire |
| 10 | pilote | `pilote/` | Sonnet | à faire |
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
