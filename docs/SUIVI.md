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
| `tools/valider_ifc.py --donnees` (IfcOpenShell 0.9.0.post1) | `maison_native` : 15 géométries, 0 erreur ; `immeuble_essai_murs` : 2 947 géométries, 5 erreurs antérieures à A6 (X5) | 17/11/2026 |
| `cargo test --workspace` (debug) | 469 réussis, 0 échec, 4 ignorés (dont 2 mesures lancées à la demande) | 17/11/2026 |
| `cargo test --workspace --release` | non relancé en entier ; `-p echanges --release` : 39 réussis, 0 échec | 17/11/2026 |
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
| A11 | Pilote : taille de message non bornée, jeton sous le dossier temporaire commun hors Windows, inscription d'instance dont l'échec est ignoré | P2 | `crates/pilote/src/lib.rs:60-71`, `instances.rs:40`, `:57-63` |
| T1 | Tests de fichiers malformés : fait pour les cas d'A3 ; restent le balayage aléatoire (fuzzing) d'IFC et de RVT, les fichiers `rvt/{vues,feuilles,nomenclatures,parametres,annexes,zones}.rs` non relus, et `Projet::verifier()` sur un `.modele.json` malformé ; `scene` sans test | P2 | `crates/formats/src/ifc/bornes.rs` |
| A2c | Vérifier sur le poste Windows que le serveur MCP démarre avec `.mcp.json` sans `.exe` | P0 | `.mcp.json` ; revenir à `bimedit.exe` en cas d'échec |
| T2 | Les 20 scripts `tools/test-*.py` (clics simulés) ne tournent pas en CI | P2 | `tools/` |

### 2.2 Réactivité

| Id | Tâche | Priorité | Détail |
|---|---|---|---|
| A8 | Réactivité, reste (mesures du 17/11/2026, immeuble de 1 920 murs) : solides des murs percés 110 à 200 ms après déplacement d'un mur (booléens des façades de 4 290 triangles autour du mur déplacé) ; arêtes jointes 72 à 87 ms (grands groupes de matériaux, tous invalidés par un mur) ; coupe du plan affiché 106 à 116 ms (tâche P1) ; second `verifier()` (14 ms) peut-être redondant | P1 | `crates/maillage/src/aretes.rs`, `modele::volumes::solides`, `crates/interface/src/documenter/coupe.rs` |
| A5b | Arrière-plan, reste : le chargement ne s'arrête pas vraiment (fermer le document abandonne son résultat, le fil finit son calcul) ; l'export SVG d'une coupe (`documenter/coupe.rs`) et l'export SVG de la vue restent sur le fil de l'interface ; l'état « en cours » et le bouton Annuler n'ont pas été vus à l'écran (enregistrement trop rapide sur les échantillons ; exports lancés par un sélecteur de fichier que le pilote ne pilote pas) | P2 | `crates/interface/src/socle/taches.rs` |
| D3 | Sorties non déterministes : `moteur2d::svg_feuille` (donc PDF et DXF d'une feuille) émet les arêtes vues dans un ordre qui change d'un appel à l'autre ; deux exports identiques diffèrent, ce qui gêne les comparaisons et les tests | P2 | constaté par `socle::taches::tests` le 17/11/2026 |
| P1 | Plan ou coupe recalculé en entier après chaque édition : 42 à 88 ms sur l'îlot, 106 à 116 ms sur l'immeuble (poché et contours, boîte et annotations, envoi au GPU) ; envoi GPU de la scène complet : 11 à 18 ms | P2 | `crates/interface/src/documenter/coupe.rs` |

### 2.3 Échanges

| Id | Tâche | Priorité | Détail |
|---|---|---|---|
| A6c | CI : lancer `tools/valider_ifc.py --donnees` (le nom du fichier est passé à l'en-tête depuis le 17/11/2026, fusion de A6) | P2 | `.github/workflows/ci.yml` |
| A6d | Export IFC : données absentes du modèle — coordonnées E/N et système de coordonnées (`IfcMapConversion`), `IsExternal`, classements feu et acoustique, phases et démolitions (décidé le 17/11/2026 : propriété `Status` des `Pset_*Common`, NEW / EXISTING / DEMOLISH / TEMPORARY, d'après la phase et `Projet::demolitions`, plus un jeu `BIMedit_Phase` pour le nom de la phase) (décision §3) | P2 | `docs/EDITION.md` « Sortie → IFC4 » |
| A6e | Jeux de propriétés au-delà des `Pset_*Common` : en rester là pour le moment (décision du 17/11/2026) | P3 | — |
| X5 | Export IFC : sur `immeuble_essai_murs`, 5 couches d'enduit de 15 mm des façades longues (47,8 m, 4 290 triangles, percées de nombreuses baies : « Mur 1 · Enduit », 617, 1541, 1849, 2157) sont lues « géométrie vide » par IfcOpenShell 0.9.0.post1 ; même résultat avec l'export d'avant A6, donc défaut antérieur (maillage percé non fermé ou trop fin pour la tolérance 0,00001 du contexte ?) | P2 | `crates/echanges/src/export_ifc/`, `modele::volumes::solides` ; constaté le 17/11/2026 |
| A7c | Import IFC : convertir les `IfcSpace` en pièces natives (décision du 17/11/2026) : `Piece` avec numéro = `Name`, nom = `LongName`, niveau, point de placement dans l'emprise de l'espace, contour recalculé par `piece::detecter` sur les murs convertis ; si l'aire obtenue s'écarte de plus de 5 % de l'emprise ou de `NetFloorArea`, l'espace reste un élément importé transparent avec ses propriétés ; données facultatives sur `Piece` pour garder `Pset_SpaceCommon` | P1 | `crates/echanges/src/depuis_ifc.rs`, `crates/modele/src/elements/piece.rs` |
| A7 | Import IFC, suite : toitures, escaliers et poteaux ou poutres non rectangulaires, inclinés ou à items mappés restent des maillages ; une barre convertie perd ses jeux de propriétés, son matériau et sa couleur (`Barre` n'en porte pas) ; booléens réécrits en texte et `PredefinedType` perdu à l'export d'un élément importé ; `IfcSpace` ignoré (décision §3) ; `.ifczip` refusé | P1 | `crates/echanges/src/depuis_ifc.rs`, `crates/echanges/src/barres_ifc.rs`, `crates/echanges/src/export_ifc/importes.rs` |
| X1 | Revit : surfaces `HermiteSurf`, `ConeSurf`, `OffsetSurf` et courbes `GEllipse` non décodées ; contenu des nomenclatures et paramètres calculés non lus ; nomenclatures posées sur les feuilles non lues | P3 | `README.md` « Limites connues », `docs/EDITION.md` |
| X2 | IFC : `IfcSurfaceCurveSweptAreaSolid`, `IfcFixedReferenceSweptAreaSolid`, `IfcSectionedSolid`, alignements IFC4X3, textures ; NURBS des `IfcAdvancedBrep` facettisées grossièrement | P3 | `README.md` « Limites connues » |
| X3 | Importer un IFC dans le projet ouvert (fusion des niveaux, types et matériaux, en une transaction) ; aujourd'hui Début > Importer crée un nouveau projet natif. Fusion d'import et comparaison de réimportations | P2 | `docs/EDITION.md` ; décision du 17/11/2026 : lien d'abord, import ensuite |
| L1 | Liens IFC, suite : propriétés IFC d'un élément lié dans Propriétés ; chemins relatifs au projet (un dossier de projet déplacé ou ouvert sur un autre poste perd ses liens : « Changer de fichier ») ; relecture automatique d'un fichier lié modifié ; coût de chaque reconstruction de la scène (géométrie liée recopiée et placée à chaque commande, à mesurer sur une grosse maquette liée) ; plusieurs liens d'un même fichier ; position par points de repère communs ; retrait du parcours « visionneuse IFC » devenu inutile (`LoadMsg::Done`, navigation IFC des fichiers IFC) | P2 | `crates/maillage/src/liens.rs`, `crates/interface/src/outils/modeliser/liens.rs` |
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

### 2.5 Familles

Objectif et lots : `docs/PLAN_FUSION_FAMILLES.md`. Avancement détaillé archivé : `docs/historique/FAMILLES_AVANCEMENT_2026-11.md`.

| Id | Tâche | Priorité | Détail |
|---|---|---|---|
| F1 | Réunir `Symbole` et éléments 2D (ancrage facultatif aux plans, une seule liste) : changement de format accepté le 17/11/2026, à condition de remettre à jour les familles d'exemple (`samples/`) et du gabarit (`gabarits/defaut.gabarit.json`) | P1 | archive, « deux représentations à réunir » |
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
| E1 | Emplacement partagé de la bibliothèque de styles, liste des catégories, priorité matériau / catégorie |
| Style | Adopter `rustfmt` sur tout le dépôt, ou garder le style actuel ? |

## 4. Journal de ce qui est fait

Le plus récent en premier. Les numéros « n° » sont ceux de la feuille de route du 16/11/2026.

| Date | Fait | Commit | Vérification |
|---|---|---|---|
| 17/11/2026 | Demandes n° 11, 18 et 26 de la feuille de route du 16/11/2026 : déclarées déjà faites par l'utilisateur | — | non vérifiable : aucun commit ne les cite |
| 17/11/2026 | A7 (partie) et A6b, import IFC qui garde les données : un élément importé porte ses jeux de propriétés et de quantités (ceux du type complètent ceux de l'objet), ses matériaux et sa couleur (`importes[].proprietes`, `materiau`, `couleur`, facultatifs, vérifiés) ; couleur utilisée dans la 3D ; affichage en lecture seule dans Propriétés ; export dans la classe IFC d'origine (liste sûre de classes à neuf attributs, sinon proxy) avec ses jeux, matériaux et couleur ; poteaux et poutres à section rectangulaire extrudée convertis en barres (`Barre.origine`, GUID gardé) ; aires et volumes lus selon `AREAUNIT`/`VOLUMEUNIT` ou l'unité de la quantité (A6b) ; échantillon `test_ifc4_proprietes.ifc` | `88064b2` à ce commit | 11 tests ajoutés (`modele` 2, `ifc_base` 2, `maillage` 1, `echanges` 6 dont l'aller-retour IFC -> natif -> IFC : propriétés par jeu, classe, données relues) ; `cargo test --workspace` (debug, sans info de débogage) : 466 réussis, 0 échec, 2 ignorés ; `bimedit --modele samples/test_ifc4_proprietes.ifc` puis `--export-ifc`, `valider_ifc.py --donnees` (ifcopenshell 0.9.0.post1) : 6 géométries, 0 erreur ; mêmes contrôles sur `test_ifc4`, `test_ifc4_murs` (6 et 7 géométries, 0 erreur) et `maison_native` (15 géométries, 0 erreur). Panneau Propriétés non essayé à l'écran ; non ouvert dans un autre logiciel BIM ; après fusion : `cargo test --workspace` (debug) 469 réussis, 0 échec, 4 ignorés ; projet importé de `test_ifc4_proprietes.ifc` exporté puis validé (`valider_ifc.py --donnees` : 6 géométries, 0 erreur) ; panneau Propriétés d'un élément importé contrôlé à l'écran sous Xvfb (matériau, couleur, jeux de propriétés et de quantités avec unités) |
| 17/11/2026 | A8, scène plus rapide : arbres et terrains exclus des arêtes jointes (leurs arêtes étaient calculées puis remplacées : 90 % du temps des arêtes sur l'îlot, à cause des feuillages) ; silhouettes des arbres gardées d'une construction à l'autre ; calcul des arêtes d'une droite par balayage (supprime un coût quadratique sur les longues droites, sans gain mesurable sur les échantillons) | ce commit | empreinte des arêtes identique avant et après sur l'îlot, l'immeuble et la maison (`cargo test -p maillage --release --test mesure_aretes -- --ignored --nocapture`) ; `bimedit --mesurer-scene`, îlot, après déplacement d'un mur : arêtes 160–195 → 7–10 ms, scène 270–340 → 80–103 ms ; immeuble inchangé (260–400 ms) ; dans l'interface sous Xvfb (BIMEDIT_CHRONO), aller-retour d'un déplacement par le pilote : îlot 137–224 ms, immeuble 390–495 ms |
| 17/11/2026 | A9, historique plus léger : instantanés compressés (deflate rapide, 1,2 Mo → 216 Ko pour l'immeuble d'essai), état enregistré gardé en empreinte ; 10 niveaux garantis, 100 au plus ; mémoire réglable dans Début > Paramètres (option `historique_mo`, 64 Mo par défaut) ; libellé d'une étape calculé sur le texte brut de la seule collection modifiée, avec un index par identifiant (la recherche était quadratique) | ce commit | 3 tests ajoutés (libellé identique à l'ancien calcul sur des commandes réelles, annuler et rétablir exacts, plancher et plafond de niveaux) ; mesure release sur l'immeuble de 1 920 murs (`cargo test -p commandes --release --test mesure_historique -- --ignored --nocapture`) : libellé 78 → 6 ms, étape d'historique 136 → 14 ms |
| 17/11/2026 | Liens IFC : un IFC ne s'ouvre plus seul ; Modéliser > Lien IFC et dépôt d'un IFC dans la fenêtre le lient au projet natif (lecture en arrière-plan, cache par fichier, transaction annulable) ; géométrie liée placée (décalage, rotation), rattachée aux niveaux, visible en 3D, en plan, en coupe et dans les feuilles ; rubrique Liens (Visible, X/Y/Z, rotation, Recharger, Changer de fichier, Retirer) ; `liens[]` dans le format du projet ; commande `lien` et commande pilote `lier` | ce commit | 9 tests ajoutés (`modele` 2, `commandes` 1, `maillage` 5, `moteur2d` 1) ; interface sous Xvfb : ouverture d'un IFC refusée avec le message, lien de `test_ifc4.ifc` dans la maison (5 éléments), plan du RDC, coupe A et 3D capturés avec la géométrie liée, Ctrl+Z retire le lien. Bouton du ruban et dépôt de fichier non essayés à l'écran (sélecteur de fichier et glisser-déposer non pilotables) |
| 17/11/2026 | A6, export IFC4 (toujours tessellé) : `IfcSite` (lieu, cote NGF, `TrueNorth`), terrains et limites dans le site ; types IFC par type utilisé ; `Pset_*Common` limités aux valeurs connues ; `Qto_*BaseQuantities` pris aux calculs des nomenclatures, unités m² et m³ ; un `IfcSurfaceStyle` par matériau ; `IfcMaterialLayerSetUsage` des murs (axe `Axis`) et des dalles sans pente ; heure UTC et `exporter_fichier` pour l'en-tête ; `export_ifc.rs` découpé en `export_ifc/` ; `tools/valider_ifc.py --donnees`. Reste : A6b à A6d | `a5e6ac5` à ce commit | 10 tests ajoutés ; `cargo test --workspace` (debug, sans info de débogage faute de disque) : 439 réussis, 0 échec, 2 ignorés ; `cargo test -p echanges --release` : 38 réussis ; `valider_ifc.py --donnees` sur `maison.ifc` (ifcopenshell 0.9.0.post1) : 15 géométries, 0 erreur ; `immeuble_essai_murs` : 5 murs d'enduit à géométrie vide pour ifcopenshell, corps inchangé par ce lot ; noms des Pset/Qto vérifiés sur les gabarits IFC4 ADD2 d'ifcopenshell (site buildingSMART bloqué) ; non ouvert dans un autre logiciel BIM |
| 17/11/2026 | A5, enregistrement et exports en arrière-plan : Ctrl+S d'un projet natif, IFC4, PDF (vue, feuille ou jeu), feuille de l'arborescence, export multiformat d'un jeu ; étape, temps écoulé et bouton Annuler dans la barre haute ; copie du projet prise au lancement, document laissé modifié s'il a changé pendant l'enregistrement ; annulation avant chaque écriture, aucun fichier partiel ; un travail à la fois, sortie refusée pendant un travail ; `etat.enregistrement` du pilote couvre ces travaux | ce commit | 3 tests ajoutés (annulation sans toucher au fichier existant, export de jeu identique à l'export direct et annulable, PDF de jeu page par page) ; interface sous Xvfb : Ctrl+S sur l'immeuble de 1 920 murs, fichier écrit avec l'annotation, document non modifié, copie de récupération effacée, statut au journal ; barre haute contrôlée pendant un chargement. Non vu à l'écran : état en cours, Annuler, édition pendant l'enregistrement (A5b) |
| 17/11/2026 | A4, journal des messages : erreurs et statuts relevés à chaque image dans un journal de 200 entrées (fenêtre Journal, Copier, Vider, compteur d'erreurs dans la barre haute ; commande `journal` du pilote) ; erreur affichée jusqu'à × ou l'erreur suivante, information en dessous ; statuts affichés hors chargement ; échec de copie de récupération, options illisibles ou non valides et gabarit illisible signalés ; `Options::enregistrer` retiré au profit de `sauver` | ce commit | 4 tests ajoutés (`interface` 2, `config` 1, `echanges` 1) ; interface lancée sous Xvfb (Vulkan logiciel) et pilotée : erreur d'une dalle trop petite gardée affichée après une annotation réussie, journal de 6 entrées, fenêtre ouverte par le bouton, captures d'écran contrôlées |
| 17/11/2026 | A3, lecteurs bornés : imbrication STEP limitée à 256 ; arcs ramenés à un tour et paramètres non finis refusés ; B-spline validée (degré, multiplicités, nœuds) ; révolution bornée à un tour ; items mappés limités à 50 000 parties et 5 millions de triangles par produit ; décompression RVT plafonnée à 2 Go ; arc RVT limité à 720 segments | ce commit | 5 tests ajoutés (`ifc_base` 3 tests, `formats` 56, réussis en debug et en release) ; 4 fichiers IFC piégés : débordement de pile, épuisement mémoire et deux gels reproduits avec le binaire d'avant, ouverture en moins de 0,1 s après ; `--stats` identique sur `test_ifc4.ifc` et `test_ifc4_murs.ifc` ; cas RVT non essayés sur un vrai fichier |
| 17/11/2026 | A2 : limites du `README.md` et d'`EDITION.md` revérifiées dans le code (entités IFC et surfaces Revit non décodées, seuils CSG, `.ifczip`) ; `.mcp.json` sans `.exe` | ce commit | serveur MCP démarré sous Linux (réponse à `initialize`) ; non essayé sous Windows (A2c) |
| 17/11/2026 | Test `suivre_trait::tests::ctrl_seul_et_espace` corrigé : la sortie d'egui est vidée de ses textures avant d'être jetée (première partie de A1) | `4e20e43` | `cargo test -p interface --lib` : 60 réussis en debug et en release |
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
