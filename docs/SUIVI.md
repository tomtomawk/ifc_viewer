# Suivi : fait et reste à faire

**Source unique de l'état du projet.** Ce qui est fait et ce qui reste à faire s'écrit ici, et nulle part ailleurs. Les autres documents décrivent le fonctionnement (comment ça marche, comment ça doit marcher) ; ils ne portent ni « fait », ni « à faire », ni avancement.

Règles de tenue :

1. Une tâche terminée quitte « Reste à faire » et entre en tête du « Journal » avec la date, le commit et la vérification faite.
2. Une tâche nouvelle ou découverte entre dans « Reste à faire » avec un identifiant, une priorité et un renvoi vers le document qui la décrit.
3. Une ligne du journal dit ce qui a été vérifié et comment. Ce qui n'a pas été vérifié est écrit tel quel (« non essayé à l'écran »).
4. Le suivi se met à jour dans le même commit que le code.
5. Les journaux détaillés d'anciens chantiers sont dans `docs/historique/` : archives, à ne pas compléter.

Priorités : **P0** défaut présent à corriger ; **P1** frein d'usage ; **P2** évolution planifiée ; **P3** plus tard.

Dernière mise à jour : 17 novembre 2026.

## 1. État de référence

| Contrôle | Résultat | Date |
|---|---|---|
| `cargo check --workspace --tests` | réussi, 0 avertissement (Linux x64) | 17/11/2026 |
| `cargo test --workspace` (debug) | 424 réussis, 0 échec, 2 ignorés (423 relevés avant la correction du test `suivre_trait`, relancé seul ensuite avec sa crate) | 17/11/2026 |
| `cargo test --workspace --release` | non relancé depuis le lot 12 (référence : 395 réussis, 2 ignorés) | 16/11/2026 |
| Chargement des 13 `samples/**/*.modele.json` (`bimedit --mesurer-scene <fichier> 0`) | 13 sur 13 | 17/11/2026 |
| Interface graphique à l'écran | non essayée depuis le lot 11 (pas d'affichage) | — |
| Collaboration sur NAS à deux machines | non faite (tâche C1) | — |

Mesures de reconstruction de la scène (build release, `BIMEDIT_CHRONO=1`, `bimedit --mesurer-scene <fichier> 3`, conteneur Linux partagé, 17/11/2026) :

| Étape | `ilot_demo` (68 murs, 202 ouvertures, 33 dalles) | `immeuble_essai_murs` (1 920 murs, 512 ouvertures) |
|---|---|---|
| Scène complète | 271 à 479 ms | 474 à 518 ms |
| Commande Déplacer un mur | 1 ms | 21 à 109 ms |
| Scène après déplacement | 230 à 310 ms | 229 à 346 ms |
| dont solides des murs | 2 à 5 ms | 75 à 157 ms |
| dont arêtes | 145 à 190 ms | 64 à 82 ms |
| dont assemblage | 45 à 125 ms | 26 à 48 ms |

## 2. Reste à faire

### 2.1 Fiabilité et robustesse

| Id | Tâche | Priorité | Détail |
|---|---|---|---|
| A1 | CI sans passe de tests en debug ni clippy : un échec limité au mode debug n'y apparaît pas | P1 | `.github/workflows/ci.yml` |
| A3 | Lecteurs IFC et RVT non bornés face à un fichier malformé : récursion du lecteur STEP, boucle sur paramètre de courbe, B-spline, révolution, décompression RVT, arc RVT | P0 | `crates/ifc_base/src/step.rs:333-346`, `crates/formats/src/ifc/geom.rs:863-870`, `:936-976`, `:1153-1158`, `crates/formats/src/rvt/objets.rs:66`, `rvt/geometrie.rs:515` |
| A2b | `.mcp.json` lance `target/release/bimedit.exe` : le serveur MCP ne démarre pas hors Windows | P1 | décision à prendre, voir §3 |
| A4 | Messages : une erreur est effacée par l'édition suivante, pas de journal, statut visible seulement pendant un chargement, échec de la copie de récupération non montré, options illisibles non signalées | P1 | `crates/interface/src/socle/messages.rs`, `saisie/saisie_plan.rs:770`, `crates/echanges/src/enregistrement.rs:159`, `crates/config/src/options/mod.rs:91-99` |
| A11 | Pilote : taille de message non bornée, jeton sous le dossier temporaire commun hors Windows, inscription d'instance dont l'échec est ignoré | P2 | `crates/pilote/src/lib.rs:60-71`, `instances.rs:40`, `:57-63` |
| T1 | Tests de fichiers malformés (IFC, RVT, `.modele.json`) ; `scene` sans test, `ifc_base` 2 tests | P2 | avec A3 |
| T2 | Les 20 scripts `tools/test-*.py` (clics simulés) ne tournent pas en CI | P2 | `tools/` |

### 2.2 Réactivité

| Id | Tâche | Priorité | Détail |
|---|---|---|---|
| A8 | Arêtes et assemblage de la scène refaits en entier à chaque commande (mémoire des booléens des murs déjà en place) ; second `verifier()` peut-être redondant | P1 | mesures au §1 ; `crates/maillage/src/aretes.rs`, `lib.rs`, `crates/noyau/src/csg_memoire.rs` |
| A5 | Enregistrement natif, export IFC, export PDF et jeu de feuilles sur le fil de l'interface, sans progression ni annulation ; un chargement ne s'annule pas | P1 | `crates/interface/src/socle/documents.rs:352`, `:665`, `outils/modifier/options_modifier.rs:265`, `:281`, `documenter/feuilles.rs:142` |
| A9 | Historique : instantané JSON complet par commande, plafond 100 entrées ou 64 Mo | P2 | `crates/commandes/src/socle/historique.rs:192-195` |
| P1 | Plan ou coupe recalculé en entier après chaque édition ; envoi GPU complet | P2 | à mesurer dans l'interface (tâche A8, point 4) |

### 2.3 Échanges

| Id | Tâche | Priorité | Détail |
|---|---|---|---|
| A6 | Export IFC4 : ajouter `IfcSite`, types, jeux de propriétés, quantités, couleurs, `IfcMaterialLayerSetUsage`, horodatage de l'en-tête | P1 | `crates/echanges/src/export_ifc.rs` |
| A7 | Import IFC : un élément importé perd propriétés, matériau et couleur ; poteaux, poutres, toitures, escaliers restent des maillages ; `IfcSpace` ignoré ; `.ifczip` refusé | P1 | `crates/echanges/src/depuis_ifc.rs`, `crates/modele/src/lib.rs` (`ElementImporte`) |
| X1 | Revit : surfaces `HermiteSurf`, `ConeSurf`, `OffsetSurf` et courbes `GEllipse` non décodées ; contenu des nomenclatures et paramètres calculés non lus ; nomenclatures posées sur les feuilles non lues | P3 | `README.md` « Limites connues », `docs/EDITION.md` |
| X2 | IFC : `IfcSurfaceCurveSweptAreaSolid`, `IfcFixedReferenceSweptAreaSolid`, `IfcSectionedSolid`, alignements IFC4X3, textures ; NURBS des `IfcAdvancedBrep` facettisées grossièrement | P3 | `README.md` « Limites connues » |
| X3 | Fusion d'import et comparaison de réimportations | P3 | `docs/EDITION.md` |
| A12 | Textes des PDF, SVG et DXF en contours : non sélectionnables | P2 | `crates/moteur2d/` |
| X4 | Courbes exportées en polylignes (pas d'arcs) ; feuilles Revit non exportées ; DWG non écrit | P3 | `README.md` « Limites connues », `crates/moteur2d/src/dxf.rs` |

### 2.4 Modéliser et modifier

Comportement attendu de chaque fonction : `docs/MODIFICATION.md`.

| Id | Tâche | Priorité | Détail |
|---|---|---|---|
| M1 | Grouper / dissocier | P2 | `docs/MODIFICATION.md` §5 |
| M2 | Remplacer le graphisme dans la vue (plume, couleur, demi-teinte) | P2 | §5 |
| M3 | Cadenas des cotes et alignements | P2 | §5 |
| M4 | Peindre un matériau sur une face | P3 | §5 |
| M5 | Diviser une face | P3 | §5 |
| M6 | Supprimer avec rapport des dépendants | P1 | §4 |
| M7 | Ajuster plusieurs éléments | P2 | §2 |
| M8 | Menu du clic droit pendant une commande (Valider / Annuler) | P2 | §1 |
| M9 | Poignées des dalles et des poutres | P1 | §1 |
| M10 | Décaler et Scinder des lignes d'annotation à la souris | P2 | §2 |
| M11 | Compléments de fonctions faites : trait de démolition dans les plans imprimés ; Joindre par clic sur la cible ; Coller aligné sur plusieurs niveaux d'un coup ; aperçu glissant de Changer d'hôte ; propriétés grisées d'un élément verrouillé | P2 | §3, §5 |
| M21 | Options manquantes des fonctions faites : Aligner (choix de face, cadenas) ; Déplacer (Déconnecter, déplacement en Z) ; Réseau (Grouper, lié à M1) ; Symétrie (axe pris sur un élément) ; Échelle refusée sur murs, menuiseries et structures | P2 | §2 |
| M12 | Outils Symbole et Légende : boutons présents, désactivés | P2 | `crates/interface/src/zones/ruban/affichage.rs:100-103` |
| M13 | Niveau de détail d'une vue : réglage sans effet | P2 | `crates/interface/src/zones/barre_haute.rs:581` |
| M14 | Créer à l'identique : menuiseries à géométrie importée et pentes différentes par arête | P3 | `docs/EDITION.md` |
| M15 | Toitures concaves à pentes différentes par arête (aujourd'hui refusées) | P3 | `docs/EDITION.md` |
| M16 | Poutres inclinées ; sections circulaires de poteau | P2 | `docs/EDITION.md` |
| M17 | Feuilles natives en onglets de dessin éditables | P2 | `docs/EDITION.md` |
| M18 | Éléments au-dessus de la coupe en pointillés ; épaisseurs de trait par catégorie (voir E1) | P2 | `README.md` « Limites connues » |
| M19 | Palette de commandes ; aide contextuelle | P3 | — |
| M20 | Demandes n° 11, 18 et 26 de la feuille de route du 16/11/2026 : aucun commit ne les cite, état inconnu | ? | à préciser, voir §3 |

### 2.5 Familles

Objectif et lots : `docs/PLAN_FUSION_FAMILLES.md`. Avancement détaillé archivé : `docs/historique/FAMILLES_AVANCEMENT_2026-11.md`.

| Id | Tâche | Priorité | Détail |
|---|---|---|---|
| F1 | Réunir `Symbole` et éléments 2D (ancrage facultatif aux plans, une seule liste) : changement de format à valider | P1 | archive, « deux représentations à réunir » |
| F2 | Onglet Modifier des familles : Réseau et Aligner ; accrochage du placement du texte dans le projet | P2 | plan, lot 2 |
| F3 | Remplacer le canevas de famille par celui des vues de projet (navigation, fond, bandeau, échelle de vue du texte) | P2 | plan, « Navigation des vues de famille » |
| F4 | Lot 6 : basculer cartouches, étiquettes et profils sur l'éditeur commun, puis retirer `familles_planes.rs`, l'éditeur de `cartouche.rs`, `profils.rs` et les formats spécialisés | P2 | plan, lot 6 |
| F5 | Lot 7 : suite de tests complète, 38 tests Revit, recette visuelle | P1 | plan, lot 7 |
| F6 | Formes guidées : chemins courbes, chemins fermés, profils de nombres de sommets différents, dessin graphique du chemin | P3 | archive `AUDIT_FACTORIZATION_2026-11-11.md` |
| F7 | Suppression et détachement d'un plan de construction utilisé | P2 | idem, lot 4 |
| F8 | Configurateur de familles ; escaliers et garde-corps paramétriques par famille | P3 | `docs/EDITION.md` |

### 2.6 Structure du code

Description de la dette : `docs/ARCHITECTURE.md` §5.

| Id | Tâche | Priorité | Détail |
|---|---|---|---|
| A10 | Registre des documents : `ViewerApp` (environ 150 champs) transfère 19 champs à la main à chaque changement de document | P2 | `crates/interface/src/lib.rs`, `socle/documents.rs:497-613` |
| R1 | Catalogue unique des outils : cartouches et contrôle complet par le pilote restent à unifier | P2 | archive de l'audit du 11/11, lot 1 |
| R5 | Contrôleur d'esquisse partagé entre canevas orthogonal et oblique | P2 | idem, lot 5 |
| R7 | Primitives de maillage, passes GPU (`gpu.rs`, un seul `impl Renderer`), dépendances collaboratives | P3 | idem, lot 7 |
| D1 | 44 fichiers au-delà de 600 lignes : à découper quand on y touche | P3 | `docs/ARCHITECTURE.md` §5 |
| D2 | `eframe` dans `config` (adaptateurs d'interface dans une crate de configuration) : à valider | P3 | `docs/ARCHITECTURE.md` §5 |

### 2.7 Évolutions étudiées, non commencées

| Id | Tâche | Priorité | Détail |
|---|---|---|---|
| E1 | Styles d'objets globaux (demande n° 15) | P2 | `docs/ETUDES_P2.md` |
| E2 | Vérification multiplateforme : essai manuel sur chaque système, cache macOS (demande n° 4) | P2 | `docs/ETUDES_P2.md` |
| E3 | Version navigateur en lecture seule (demande n° 3) | P3 | `docs/ETUDES_P2.md` |

### 2.8 Collaboration

| Id | Tâche | Priorité | Détail |
|---|---|---|---|
| C1 | Qualification NAS à deux machines (9 étapes) | P1 | `docs/COLLABORATION.md`, « Qualification NAS à deux machines » |
| C2 | Commande de compactage du registre partagé | P3 | `docs/COLLABORATION.md` |

## 3. Décisions en attente

| Sujet | Question |
|---|---|
| Ordre | Fiabilité (A3) avant l'IFC (A6, A7), ou l'inverse ? |
| A2b | Forme portable de `.mcp.json`, le poste de référence étant sous Windows |
| A6 | Jeux de propriétés attendus par les partenaires ; représentation des phases et démolitions dans l'IFC |
| A7 | Convertir les `IfcSpace` en pièces natives ? |
| F1 | Valider le changement de format `Symbole` / éléments 2D |
| E1 | Emplacement partagé de la bibliothèque de styles, liste des catégories, priorité matériau / catégorie |
| M20 | Contenu et état des demandes n° 11, 18 et 26 |
| Style | Adopter `rustfmt` sur tout le dépôt, ou garder le style actuel ? |

## 4. Journal de ce qui est fait

Le plus récent en premier. Les numéros « n° » sont ceux de la feuille de route du 16/11/2026.

| Date | Fait | Commit | Vérification |
|---|---|---|---|
| 17/11/2026 | Test `suivre_trait::tests::ctrl_seul_et_espace` corrigé : la sortie d'egui est vidée de ses textures avant d'être jetée (première partie de A1) | ce commit | `cargo test -p interface --lib` : 60 réussis en debug et en release |
| 17/11/2026 | Échantillons : 11 fichiers `samples/**/*.modele.json` remis au schéma courant (opération d'extrusion explicite, remplissage de symbole, catégorie de famille, format `famille-ouverture 3`) | `ce3a404` | 13 fichiers sur 13 chargés par `bimedit --mesurer-scene` ; non ouverts à l'écran |
| 17/11/2026 | Documentation : ce suivi unique ; états retirés de `MODIFICATION.md`, `PLAN_FUSION_FAMILLES.md`, `ARCHITECTURE.md` ; audit du 11/11 archivé ; `README.md` et `EDITION.md` corrigés (escalier, garde-corps, DXF, section de poutre) | `ce3a404` | relecture |
| 16/11/2026 | Audit des fonctions à améliorer (tâches A1 à A12) | — | compilation, tests debug, mesures |
| 16/11/2026 | Études : styles globaux, multiplateforme, WebAssembly (n° 15, 4, 3) | `63468c0` | étude seule |
| 16/11/2026 | Escaliers balancés (n° 5) | `35e1c42` | — |
| 16/11/2026 | Collaboration : politique de fusion des types de poutres | `18bc7e4` | — |
| 16/11/2026 | Terrains multicouches adaptatifs (n° 14) | `820efbd` | — |
| 16/11/2026 | Poutres : types, profils, justification, arase ; outil au tracé commun avec aperçu (n° 6 à 10) | `03785a1`, `f4875b2` | — |
| 16/11/2026 | Poignées de la zone de coupe 3D dans le bon sens (n° 21) | `17784f6` | — |
| 16/11/2026 | Export multiformat d'un jeu de feuilles, dont DXF (n° 13) | `9014a7f` | — |
| 16/11/2026 | Export PDF de la feuille affichée, cartouche compris (n° 12) | `f36d371` | — |
| 16/11/2026 | Aimantation des vues aux cadres des feuilles (n° 1) | `e087ffe` | — |
| 16/11/2026 | Distance tapée depuis un point accroché (n° 2) | `cc1f127` | — |
| 16/11/2026 | Raccourcis Ctrl et Espace en mode Suivre un trait (n° 24) | `a17dfdb` | test ajouté, en échec en debug jusqu'au 17/11 |
| 16/11/2026 | Axes et faces survolés deviennent des droites de guidage (n° 23) | `66e01ea` | — |
| 16/11/2026 | Accrochages sur les faces des murs en arc (n° 25) | `c735379` | — |
| 16/11/2026 | Lame d'air réellement vide (n° 20) | `d74ccd5` | — |
| 16/11/2026 | Hachures d'isolant complètes dans les murs à ossature (n° 19) | `a55d8f2` | — |
| 16/11/2026 | Un seul sélecteur de type, créations successives et ouvertures (n° 22) | `e800da0`, `5b823ef` | — |
| 16/11/2026 | Bandeau de dessin commun (n° 17) | `735288b` | — |
| 16/11/2026 | Messages centralisés sous le bandeau de dessin (n° 16) | `bb43ce8` | — |
| 16/11/2026 | Crash au changement de rayon d'un mur en arc corrigé (n° 27) | `82b84ab` | — |
| 16/11/2026 | Découpage en workspace de 15 crates (lots 0 à 12), garde-fou des dépendances, `AGENTS.md` par crate | `e72f601` à `5bafadc` | 395 tests réussis, 2 ignorés (release) ; journal : `docs/historique/ARCHITECTURE_CHANTIER_2026-11.md` |
| 16/11/2026 | Intégration continue sur 5 cibles, validation de l'IFC exporté par IfcOpenShell | `.github/workflows/ci.yml` | — |
| 13/11/2026 | Familles : Tourner, Symétrie, Échelle étendus aux révolutions et formes guidées ; aperçu ; raccourcis MV, CO, RO, SY, EC | — | tests ciblés ; symétrie et échelle des révolutions non essayées à l'écran |
| 12/11/2026 | Familles : ruban compact, onglet Modifier, texte du projet au parcours de la famille | — | 16 tests ciblés, instance de test |
| 12/11/2026 | Factorisation : `FormeProfil` à opération exclusive (lot 3) ; plans de construction par identifiant (lot 4, partiel) ; shaders WGSL extraits ; registre des politiques collaboratives | — | validation reportée à l'époque |
| 12/11/2026 | Familles : lots 1, 3, 4, 5 du plan de fusion (modèle unique, éditeur commun, cartouches et étiquettes, profils) | — | détail dans l'archive |
| 11/11/2026 | Recalcul partiel des murs (mémoire des booléens), chronos par étape, `--mesurer-scene` | `b26c3f8` | 800 murs et 512 ouvertures : modification de 3 100 ms à 310 ms (mesure du commit) |
| 11/11/2026 | Revue de factorisation | — | archive `docs/historique/AUDIT_FACTORIZATION_2026-11-11.md` |
| avant le 16/11/2026 | Modifier : Aligner, Déplacer, Copier, Tourner, Symétrie, Réseau, Échelle, Décaler, Scinder, Ajuster, Verrouiller, Déverrouiller, Masquer, Afficher, Isoler, Démolir, Sélectionner les identiques, Filtre de sélection, Reproduire les propriétés, Changer d'hôte, Inverser, menu du clic droit hors commande, poignée du mur | — | `tools/test-transformer.py`, `test-visibilite.py`, `test-elements.py`, `test-verrou.py` |
| avant le 16/11/2026 | Collaboration par partage de fichiers : transactions, réservations, texte Yrs, conflits, instantanés, récupération | — | tests Rust et `tools/test_partage_ui.py` sur NTFS local ; NAS non qualifié (C1) |
