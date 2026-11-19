# Feuille de route : à venir et en attente

**Source unique de ce qui reste à faire.** Tâches à venir, tâches en attente d'une décision, décisions à prendre. Ce qui est fait ou abandonné est dans [SUIVI.md](SUIVI.md). Les autres documents décrivent le fonctionnement ; ils ne portent ni « fait », ni « à faire », ni avancement.

Règles de tenue :

1. Une tâche nouvelle ou découverte entre ici avec un identifiant, une priorité et un renvoi vers le document qui la décrit.
2. Une tâche terminée, ou terminée en partie, quitte cette feuille de route (ou n'y garde que le reste) dans le même commit que le code ; le fait est journalisé dans [SUIVI.md](SUIVI.md) §3.
3. Une tâche abandonnée quitte cette feuille de route et entre dans [SUIVI.md](SUIVI.md) §2 avec la date et la raison.
4. Une tâche suspendue à un choix reste dans sa section ; la question est écrite au §9.

Priorités : **P0** défaut présent à corriger ; **P1** frein d'usage ; **P2** évolution planifiée ; **P3** plus tard.

## 1. Fiabilité et robustesse

| Id | Tâche | Priorité | Détail |
|---|---|---|---|
| A1b | Résorber les avertissements clippy (environ 270 au 17/11/2026, dont 140 `if` imbriqués et 52 types complexes) et 3 erreurs (`approx_constant` dans des tests de `formats`), puis rendre clippy bloquant dans la CI | P2 | `.github/workflows/ci.yml`, tâche `debug` |
| A11 | Pilote : taille de message non bornée, jeton sous le dossier temporaire commun hors Windows, inscription d'instance dont l'échec est ignoré | P2 | `crates/pilote/src/lib.rs:60-71`, `instances.rs:40`, `:57-63` |
| T1 | Tests de fichiers malformés : fait pour les cas d'A3 ; restent le balayage aléatoire (fuzzing) d'IFC et de RVT, les fichiers `rvt/{vues,feuilles,nomenclatures,parametres,annexes,zones}.rs` non relus, et `Projet::verifier()` sur un `.modele.json` malformé ; `scene` sans test | P2 | `crates/formats/src/ifc/bornes.rs` |
| T2 | Les 20 scripts `tools/test-*.py` (clics simulés) ne tournent pas en CI | P2 | `tools/` |

## 2. Réactivité

| Id | Tâche | Priorité | Détail |
|---|---|---|---|
| A8 | Réactivité, reste (mesures du 17/11/2026, immeuble de 1 920 murs) : solides des murs percés 110 à 200 ms après déplacement d'un mur (booléens des façades de 4 290 triangles autour du mur déplacé) ; arêtes jointes 72 à 87 ms (grands groupes de matériaux, tous invalidés par un mur) ; coupe du plan affiché 106 à 116 ms (tâche P1) ; second `verifier()` (14 ms) peut-être redondant | P1 | `crates/maillage/src/aretes.rs`, `modele::volumes::solides`, `crates/interface/src/documenter/coupe.rs` |
| A5b | Arrière-plan, reste : le chargement ne s'arrête pas vraiment (fermer le document abandonne son résultat, le fil finit son calcul) ; l'export SVG d'une coupe (`documenter/coupe.rs`) et l'export SVG de la vue restent sur le fil de l'interface ; l'état « en cours » et le bouton Annuler n'ont pas été vus à l'écran (enregistrement trop rapide sur les échantillons ; exports lancés par un sélecteur de fichier que le pilote ne pilote pas) | P2 | `crates/interface/src/socle/taches.rs` |
| D3 | Sorties non déterministes : `moteur2d::svg_feuille` (donc PDF et DXF d'une feuille) émet les arêtes vues dans un ordre qui change d'un appel à l'autre ; deux exports identiques diffèrent, ce qui gêne les comparaisons et les tests | P2 | constaté par `socle::taches::tests` le 17/11/2026 |
| P1 | Plan ou coupe recalculé en entier après chaque édition : 42 à 88 ms sur l'îlot, 106 à 116 ms sur l'immeuble (poché et contours, boîte et annotations, envoi au GPU) ; envoi GPU de la scène complet : 11 à 18 ms | P2 | `crates/interface/src/documenter/coupe.rs` |

## 3. Échanges

| Id | Tâche | Priorité | Détail |
|---|---|---|---|
| A6d | Export IFC : données absentes du modèle — coordonnées E/N et système de coordonnées (`IfcMapConversion`), `IsExternal`, classements feu et acoustique, phases et démolitions (décidé le 17/11/2026 : propriété `Status` des `Pset_*Common`, NEW / EXISTING / DEMOLISH / TEMPORARY, d'après la phase et `Projet::demolitions`, plus un jeu `BIMedit_Phase` pour le nom de la phase) (décision §3) | P2 | `docs/EDITION.md` « Sortie → IFC4 » |
| A6e | Jeux de propriétés au-delà des `Pset_*Common` : en rester là pour le moment (décision du 17/11/2026) | P3 | — |
| A7 | Import IFC, suite : toitures, escaliers et poteaux ou poutres non rectangulaires, inclinés ou à items mappés restent des maillages ; une barre convertie perd ses jeux de propriétés, son matériau et sa couleur (`Barre` n'en porte pas) ; `PredefinedType` perdu à l'export d'un élément importé ; `.ifczip` refusé | P1 | `crates/echanges/src/depuis_ifc.rs`, `crates/echanges/src/barres_ifc.rs`, `crates/echanges/src/export_ifc/importes.rs` |
| X1 | Revit : surfaces `HermiteSurf`, `ConeSurf`, `OffsetSurf` et courbes `GEllipse` non décodées ; contenu des nomenclatures et paramètres calculés non lus ; nomenclatures posées sur les feuilles non lues | P3 | `README.md` « Limites connues », `docs/EDITION.md` |
| X3 | Réimport IFC : comparaison d'une réimportation avec l'import précédent et mise à jour des éléments ; décalage de l'import dans le projet ouvert non proposé à l'écran (commande seule) | P2 | `docs/EDITION.md` (Début → Importer) |
| X2 | IFC : alignements IFC4X3, textures ; `IfcSectionedSolid` autres que `IfcSectionedSolidHorizontal` ; faces B-spline découpées (bornes intérieures au domaine) facettisées par leurs arêtes ; paramètres de début et de fin d'un balayage ignorés hors directrice `IfcPolyline` ; surface de référence non plane prise pour un plan horizontal | P3 | `README.md` « Limites connues » |
| L1 | Liens IFC, suite : propriétés IFC d'un élément lié dans Propriétés ; chemins relatifs au projet (un dossier de projet déplacé ou ouvert sur un autre poste perd ses liens : « Changer de fichier ») ; relecture automatique d'un fichier lié modifié ; coût de chaque reconstruction de la scène (géométrie liée recopiée et placée à chaque commande, à mesurer sur une grosse maquette liée) ; plusieurs liens d'un même fichier ; position par points de repère communs ; retrait du parcours « visionneuse IFC » devenu inutile (`LoadMsg::Done`, navigation IFC des fichiers IFC) | P2 | `crates/maillage/src/liens.rs`, `crates/interface/src/outils/modeliser/liens.rs` |
| A12 | Textes des PDF, SVG et DXF en contours : non sélectionnables | P2 | `crates/moteur2d/` |
| X4 | Feuilles Revit non exportées ; DWG non écrit (format fermé) ; arcs des arêtes de maillage et des vues Revit exportés en segments | P3 | `README.md` « Limites connues », `crates/moteur2d/src/dxf.rs` |

## 4. Modéliser et modifier

Comportement attendu de chaque fonction : `docs/MODIFICATION.md`.

| Id | Tâche | Priorité | Détail |
|---|---|---|---|
| M1 | Grouper / dissocier | P2 | `docs/MODIFICATION.md` §5 |
| M2 | Remplacer le graphisme dans la vue (plume, couleur, demi-teinte) | P2 | §5 |
| M3 | Cadenas des cotes et alignements | P2 | §5 |
| M4 | Peindre un matériau sur une face | P3 | §5 |
| M5 | Diviser une face | P3 | §5 |
| M7 | Ajuster plusieurs éléments | P2 | §2 |
| M8 | Menu du clic droit pendant une commande (Valider / Annuler) | P2 | §1 |
| S3 | Quadrillages et repères des autres coupes non dessinés ni sélectionnables en coupe et élévation ; repère de coupe sélectionnable seulement en plan | P2 | `modele::documentation::niveaux_et_limites_en_coupe` |
| S4 | Types de niveau (tête, trait) absents du modèle : le sélecteur d'un niveau reste « Générique » | P3 | `docs/MODELE.md` |
| M10 | Décaler et Scinder des lignes d'annotation à la souris | P2 | §2 |
| M11 | Compléments de fonctions faites : trait de démolition dans les plans imprimés ; Joindre par clic sur la cible ; Coller aligné sur plusieurs niveaux d'un coup ; aperçu glissant de Changer d'hôte ; propriétés grisées d'un élément verrouillé | P2 | §3, §5 |
| M21 | Options manquantes des fonctions faites : Aligner (cadenas) ; Déplacer (Déconnecter, déplacement en Z) ; Réseau (Grouper, lié à M1) ; Symétrie (axe pris sur un élément) ; Échelle refusée sur murs, menuiseries et structures | P2 | §2 |
| M12 | Outils Symbole et Légende : boutons présents, désactivés | P2 | `crates/interface/src/zones/ruban/affichage.rs:100-103` |
| M14 | Créer à l'identique : menuiseries à géométrie importée et pentes différentes par arête | P3 | `docs/EDITION.md` |
| M15 | Toitures concaves à pentes différentes par arête (aujourd'hui refusées) | P3 | `docs/EDITION.md` |
| M16 | Poutres inclinées ; sections circulaires de poteau | P2 | `docs/EDITION.md` |
| M17 | Feuilles natives en onglets de dessin éditables | P2 | `docs/EDITION.md` |
| M18 | Épaisseurs de trait par catégorie (voir E1) | P2 | `README.md` « Limites connues » |
| M19 | Palette de commandes ; aide contextuelle | P3 | — |

## 5. Familles

Objectif et lots : `docs/PLAN_FUSION_FAMILLES.md`. Avancement détaillé archivé : `docs/historique/FAMILLES_AVANCEMENT_2026-11.md`.

| Id | Tâche | Priorité | Détail |
|---|---|---|---|
| F3 | Suites des plans tournés et du plan de dessin actif : éléments 2D et cotes saisis en vue 3D sur le plan actif (seuls les profils des formes le sont) ; poignées et transformations des formes posées sur un plan (`support`) ; « Plan par 3 points » quand aucune droite d'appui existante n'est dans le plan (enchaîner deux rotations automatiquement) et depuis une vue plane ; cotes et EQ entre plans tournés parallèles ; à essayer à l'écran (binaire release) : outil « Plan par 3 points », choix d'une face, fermeture d'un profil par clic droit | P2 | `crates/modele/src/famille/modelee/orientation.rs`, `crates/interface/src/familles/editeur_famille/plan_actif.rs` |
| T3 | `tools/test-accrochage.py` : l'étape 4 (prolongement d'un mur par sa poignée) échoue, déjà avec la version `840fcea` (avant le moteur d'accrochage commun) | P2 | `tools/test-accrochage.py` |
| F5 | Lot 7 : suite de tests complète, 38 tests Revit, recette visuelle | P1 | plan, lot 7 |
| F6 | Formes guidées, reste : chemin tracé hors du plan de la vue (D), raccordement sur chemin fermé, outil « Tracer le chemin » non essayé à l'écran | P3 | `docs/EDITION.md`, `docs/MODELE.md` |
| F8 | Escaliers et garde-corps paramétriques par famille | P3 | `docs/EDITION.md` |

## 6. Structure du code

Description de la dette : `docs/ARCHITECTURE.md` §5.

| Id | Tâche | Priorité | Détail |
|---|---|---|---|
| I2 | Catalogue unique des commandes avec schéma JSON généré (`schemars`) et documentation `docs/commandes.md` régénérée et vérifiée par un test : un seul contrat pour interface, pilote et MCP ; base de la palette de commandes (M19). Idée ConstructCraft | P2 | `crates/commandes`, `crates/pilote` |
| I3 | Infobulles du ruban : raccourci, raison d'indisponibilité d'un outil grisé, commande correspondante ; test que la disponibilité suit l'état. Idée ConstructCraft | P2 | `crates/interface/src/zones/ruban/` |
| I4 | Tests d'interface sans affichage (egui_kittest : arbre d'accessibilité, comparaisons d'images à la demande) pour remplacer une partie des essais Xvfb manuels | P2 | `crates/interface` |
| I5 | Clippy : interdire `unwrap`/`expect`/`panic` hors tests (`clippy.toml`), puis clippy bloquant (avec A1b) ; actions de la CI épinglées par SHA | P2 | `.github/workflows/ci.yml` |
| I6 | Budget des booléens (nombre de polygones et de découpes ; au-delà, échec propre au lieu d'un blocage) et `catch_unwind` autour des triangulations | P2 | `crates/noyau/src/csg*.rs`, `noyau::trace2d` |
| A10 | Registre des documents : `Application` (environ 150 champs) transfère 19 champs à la main à chaque changement de document | P2 | `crates/interface/src/lib.rs`, `socle/documents.rs:497-613` |
| R1 | Catalogue unique des outils : cartouches et contrôle complet par le pilote restent à unifier | P2 | archive de l'audit du 11/11, lot 1 |
| R5 | Contrôleur d'esquisse partagé entre canevas orthogonal et oblique | P2 | idem, lot 5 |
| R7 | Primitives de maillage, passes GPU (`gpu.rs`, un seul `impl Renderer`), dépendances collaboratives | P3 | idem, lot 7 |
| D1 | 44 fichiers au-delà de 600 lignes : à découper quand on y touche | P3 | `docs/ARCHITECTURE.md` §5 |
| D2 | `eframe` dans `config` (adaptateurs d'interface dans une crate de configuration) : à valider | P3 | `docs/ARCHITECTURE.md` §5 |

## 7. Évolutions étudiées, non commencées

| Id | Tâche | Priorité | Détail |
|---|---|---|---|
| E1 | Styles d'objets globaux (demande n° 15) | P2 | `docs/ETUDES_P2.md` |
| E2 | Vérification multiplateforme : essai manuel sur chaque système, cache macOS (demande n° 4) | P2 | `docs/ETUDES_P2.md` |
| E3 | Version navigateur en lecture seule (demande n° 3) | P3 | `docs/ETUDES_P2.md` |

## 8. Collaboration

| Id | Tâche | Priorité | Détail |
|---|---|---|---|
| C1 | Qualification NAS à deux machines (9 étapes) | P1 | `docs/COLLABORATION.md`, « Qualification NAS à deux machines » |
| C2 | Commande de compactage du registre partagé | P3 | `docs/COLLABORATION.md` |

## 9. Décisions en attente

| Sujet | Question |
|---|---|
| E1 | Emplacement partagé de la bibliothèque de styles, liste des catégories, priorité matériau / catégorie |
