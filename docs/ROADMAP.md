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
| T1 | Tests de fichiers malformés, reste : balayage aléatoire des RVT (aucun fichier RVT d'exemple dans le dépôt) ; index des catégories et mémoïsation des formules de nomenclature (23/11/2026) à vérifier sur une maquette RVT (les tests Revit sont sautés sans les fichiers d'essai) | P3 | `crates/formats/src/rvt/` |
| G1 | Reste du gabarit pendant l'édition : les textures livrées (`gabarit:textures/…`, `maillage::textures_gabarit`) restent embarquées et désignées par les matériaux ; à copier dans le dossier du projet (ou une bibliothèque) avant de cesser d'embarquer le gabarit | P3 | `crates/maillage/src/textures_gabarit.rs` |

## 2. Réactivité

| Id | Tâche | Priorité | Détail |
|---|---|---|---|
| A8 | Réactivité (amélioration de confort, pas un défaut) : délai d'affichage après une modification sur un gros projet ; maison < 10 ms. Mesures du 24/11/2026 (`bimedit --mesurer-scene`, immeuble de 1 920 murs, 4 cœurs) : la scène est partiellement reconstruite après une modification (178 à 268 ms après déplacement d'un mur) : les découpes des murs dont les opérandes n'ont pas changé sont reprises (`csg_memoire`), comme la géométrie des liens IFC ; le reste est recalculé (jonctions, arêtes jointes, assemblage, envoi au GPU). Solides des murs 53 à 118 ms, dont 111 à 229 booléens recalculés (une découpe changée invalide la suite de la chaîne de son mur, séquentielle, dominée par un mur de façade de 4 290 triangles) ; arêtes jointes 70 ms (groupes par matériau, tous invalidés) ; assemblage 28 ms. Essayé et écarté le 24/11/2026 : différence BSP limitée aux triangles proches de l'emprise de l'autre solide (solides complets 180 → 79 ms, volumes identiques sur 2 079 murs) — un mur sur 1 920 perd une face de 13 × 200 mm (faces coplanaires résolues autrement), visible en arêtes. Pistes, par rapport gain / risque : (2) coupe du plan affiché par objet, reprise pour les objets inchangés (106 à 116 ms) ; (4) jonctions entre murs prismatiques calculées en plan (différence de polygones 2D par tranche de hauteur) au lieu de booléens 3D en chaîne (55 à 120 ms), percements gardés en 3D ; (5) assemblage et envoi GPU limités aux objets changés (28 ms + 11 à 18 ms) | P2 | `crates/maillage/src/lib.rs`, `crates/modele/src/volumes/solides.rs`, `crates/noyau/src/csg.rs`, `crates/interface/src/saisie/saisie_plan.rs` |
| A5b | Arrière-plan, reste : le chargement abandonné s'arrête à sa prochaine étape annoncée, pas au milieu d'une étape (géométrie IFC de tous les produits, construction de la scène native) ; le dessin d'une vue exportée en SVG (`dessin_vue`) reste calculé sur le fil de l'interface (il dépend de l'état de la vue), seule l'écriture passe en arrière-plan ; l'état « en cours » et le bouton Annuler d'un export n'ont pas été vus à l'écran (exports lancés par un sélecteur de fichier que le pilote ne pilote pas) | P3 | `crates/interface/src/socle/taches.rs` |
| P1 | Plan ou coupe après une édition, reste : le dessin de la vue est encore réuni et envoyé au GPU en entier (sommets de toutes les primitives, environ 30 ms en build debug sur l'immeuble) ; le cache de coupe ne sert pas à l'export ni aux feuilles (`dessin_vue`, `coupe` sans cache) ; envoi GPU de la scène complète : 11 à 18 ms | P3 | `crates/moteur2d/src/coupe.rs`, `crates/interface/src/documenter/coupe.rs` |

## 3. Échanges

| Id | Tâche | Priorité | Détail |
|---|---|---|---|
| A6d | Export IFC : données absentes du modèle — coordonnées E/N et système de coordonnées (`IfcMapConversion`), `IsExternal`, classements feu et acoustique, phases et démolitions (décidé le 17/11/2026 : propriété `Status` des `Pset_*Common`, NEW / EXISTING / DEMOLISH / TEMPORARY, d'après la phase et `Projet::demolitions`, plus un jeu `BIMedit_Phase` pour le nom de la phase) (décision §3) | P2 | `docs/EDITION.md` « Sortie → IFC4 » |
| A6e | Jeux de propriétés au-delà des `Pset_*Common` : en rester là pour le moment (décision du 17/11/2026) | P3 | — |
| A7 | Import IFC : convertir en éléments natifs ce qui arrive encore en maillage, dans l'ordre : 1. toitures (`IfcRoof` : contour et pentes) ; 2. sols et plafonds (`IfcCovering`) ; 3. escaliers et garde-corps (`IfcStair`, `IfcRailing`) ; 4. poteaux et poutres non rectangulaires, inclinés ou à items mappés, avec leurs jeux de propriétés, matériau et couleur (`Barre` n'en porte pas). Conversion retenue si le volume natif reproduit la géométrie lue à 5 % près (`ECART_MAX`), sinon maillage avec la raison. `PredefinedType` des sources IFC2X3 et IFC4X3 non repris (énumérations différentes d'IFC4) | P2 | `crates/echanges/src/depuis_ifc.rs`, `crates/echanges/src/barres_ifc.rs`, `crates/echanges/src/export_ifc/importes.rs` |
| X5 | Revit comme l'IFC. Import, déjà converti en natif : niveaux, murs droits et courbes (couches), portes et fenêtres hébergées, sols et plafonds prismatiques, phases, variantes, sous-projets, plans, coupes, élévations, feuilles, nomenclatures. Reste à convertir (aujourd'hui maillages ou comptés) : toits, sols en pente, escaliers, garde-corps, murs-rideaux, poteaux et poutres, topographie (terrain), autres familles (composants), pièces, quadrillages, cotes et textes des vues ; vues 3D et plans de plafond ignorés ; orientation d'un plan pivoté perdue. Ouverture seule d'un `.rvt` (visionneuse et métadonnées) à retirer ; **lien RVT** dans un projet natif, comme le lien IFC (L1), à créer | P2 | `crates/echanges/src/depuis_rvt.rs`, `docs/MODELE.md` « Conversion depuis Revit », `crates/interface/src/socle/documents.rs`, `crates/maillage/src/liens.rs` |
| X1 | Revit : surfaces `HermiteSurf`, `ConeSurf`, `OffsetSurf` et courbes `GEllipse` non décodées ; contenu des nomenclatures et paramètres calculés non lus ; nomenclatures posées sur les feuilles non lues | P3 | `README.md` « Limites connues », `docs/EDITION.md` |
| X3 | Réimport IFC : comparaison d'une réimportation avec l'import précédent et mise à jour des éléments ; décalage de l'import dans le projet ouvert non proposé à l'écran (commande seule) | P2 | `docs/EDITION.md` (Début → Importer) |
| X2 | IFC : alignements IFC4X3, textures ; `IfcSectionedSolid` autres que `IfcSectionedSolidHorizontal` ; faces B-spline découpées (bornes intérieures au domaine) facettisées par leurs arêtes ; paramètres de début et de fin d'un balayage ignorés hors directrice `IfcPolyline` ; surface de référence non plane prise pour un plan horizontal | P3 | `README.md` « Limites connues » |
| L1 | Liens IFC, suite : propriétés IFC d'un élément lié dans Propriétés ; chemins relatifs au projet (un dossier de projet déplacé ou ouvert sur un autre poste perd ses liens : « Changer de fichier ») ; relecture automatique d'un fichier lié modifié ; coût de chaque reconstruction de la scène (géométrie liée recopiée et placée à chaque commande, à mesurer sur une grosse maquette liée) ; plusieurs liens d'un même fichier ; position par points de repère communs ; retrait du reste du parcours « visionneuse IFC » (navigation IFC des fichiers IFC ; le chargement d'un IFC seul est retiré) | P2 | `crates/maillage/src/liens.rs`, `crates/interface/src/outils/modeliser/liens.rs` |
| A12 | Textes des PDF, SVG et DXF en contours : non sélectionnables | P2 | `crates/moteur2d/` |
| X4 | Feuilles Revit non exportées ; DWG à ouvrir dans AutoCAD et DWG TrueView (écrit en 2004 par `acadrust`, vérifié par relecture seulement) ; arcs des arêtes de maillage et des vues Revit exportés en segments | P3 | `README.md` « Limites connues », `crates/moteur2d/src/dxf.rs` |

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
| S4 | Types de niveau et d'axe de quadrillage : tête choisie par type (aujourd'hui une famille de tête pour tout le projet, `symboles.tete_niveau` et `tete_quadrillage`) et type de trait ; le sélecteur d'un niveau reste « Générique ». Les repères de coupe et d'élévation se choisissent déjà par type de vue | P3 | `docs/MODELE.md` (`familles_symboles`, `symboles`, `types_vues[].repere`) |
| O1 | Étiquette d'annotation : une ouverture (porte, fenêtre) n'est pas reconnue et s'affiche « Référence perdue » (`documentation::primitives`, seuls murs, dalles, barres et importés) ; l'exemple `maison_exemple` repère ses menuiseries par des textes | P2 | `crates/modele/src/documentation/mod.rs` |
| M10 | Décaler et Scinder des lignes d'annotation à la souris | P2 | §2 |
| M22 | Outil Profil de mur (comme Balayage et Profil en creux de Revit) : type (profil, matériau, relief ou creux, décalage, retrait aux extrémités, côté), occurrence sur un mur (côté, hauteur depuis la base ou le haut, début et fin), balayage le long du nu, creux soustrait du mur ; sélecteur de type et palette communs, Propriétés du type ; export `IfcBuildingElementProxy` | P2 | `crates/modele/src/famille/modelee/balayage.rs`, `crates/interface/src/zones/palette_creation.rs` |
| M23 | Profil d'un mur, suites : Scinder un mur à profil (partage du profil entre les deux parties, aujourd'hui refusé) ; côtés en arc dans le profil ; profil d'un mur courbe ; un mur tourné de 180° autour de son propre milieu se confond avec un mur retourné (profil gardé en place, non tourné) | P3 | `crates/modele/src/murs/profil.rs` |
| M11 | Compléments de fonctions faites : trait de démolition dans les plans imprimés ; Joindre par clic sur la cible ; Coller aligné sur plusieurs niveaux d'un coup ; aperçu glissant de Changer d'hôte ; propriétés grisées d'un élément verrouillé | P2 | §3, §5 |
| M21 | Options manquantes des fonctions faites : Aligner (cadenas) ; Déplacer (Déconnecter, déplacement en Z) ; Réseau (Grouper, lié à M1) ; Symétrie (axe pris sur un élément) ; Échelle refusée sur murs, menuiseries et structures | P2 | §2 |
| M12 | Outils Symbole et Légende : boutons présents, désactivés | P2 | `crates/interface/src/zones/ruban/affichage.rs:100-103` |
| M14 | Créer à l'identique : menuiseries à géométrie importée et pentes différentes par arête | P3 | `docs/EDITION.md` |
| M15 | Toitures concaves à pentes différentes par arête (aujourd'hui refusées) | P3 | `docs/EDITION.md` |
| M16 | Poutres inclinées ; sections circulaires de poteau | P2 | `docs/EDITION.md` |
| M17 | Feuilles natives en onglets de dessin éditables | P2 | `docs/EDITION.md` |
| M19 | Palette de commandes ; aide contextuelle | P3 | — |
| U11 | Réglages retirés car jamais lus (U3, 20/11/2026), à réaliser s'ils sont voulus : Texte (ligne de repère avec flèche, alignement gauche / centre / droite), Étiquette (contenu : type, repère, matériau), Cote (mode orthogonal, décalage papier), Zone (motif, pas), Quadrillage en arc, Déplacer contraint horizontal / vertical (Maj contraint déjà aux axes) | P3 | `zones/ruban/mod.rs` (`champs`) ; question au §9 |
| U7 | Revue des parcours : Déplacer, Copier, Tourner, Symétrie, Réseau, Échelle exigent une sélection préalable ; pas d'étape « Sélectionner, Entrée pour valider » dans la commande | P2 | `outils/modifier/transformer.rs`, `docs/MODIFICATION.md` §1 |

## 5. Familles

Objectif : une famille a un document, un éditeur et un format communs quelle que soit sa catégorie ; elle peut porter de la géométrie 3D et des éléments 2D dans ses vues. Cartouches et étiquettes n'utilisent dans le projet que le 2D de la vue en plan ; un profil y prend sa section (contour fermé désigné). Les outils sont ceux du projet (un contrôleur commun, un adaptateur par contexte : projection, unités, données). Les anciennes familles seront refaites, sans migration (`docs/AGENTS.md`). Fonctionnement réalisé : `crates/interface/src/familles/AGENTS.md`, `docs/EDITION.md`, `docs/MODELE.md`.

| Id | Tâche | Priorité | Détail |
|---|---|---|---|
| F3 | Suites des plans tournés et du plan de dessin actif : éléments 2D et cotes saisis en vue 3D sur le plan actif (seuls les profils des formes le sont) ; poignées et transformations des formes posées sur un plan (`support`) ; « Plan par 3 points » quand aucune droite d'appui existante n'est dans le plan (enchaîner deux rotations automatiquement) et depuis une vue plane ; cotes et EQ entre plans tournés parallèles ; à essayer à l'écran (binaire release) : outil « Plan par 3 points », choix d'une face, fermeture d'un profil par clic droit | P2 | `crates/modele/src/famille/modelee/orientation.rs`, `crates/interface/src/familles/editeur_famille/plan_actif.rs` |
| F10 | Outils communs projet / familles, reste à relever (état non vérifié le 20/11/2026) : texte et libellés (placement, poignées, contenu, hauteur, champs dynamiques et aperçu ; ouvrir au projet les fonctions propres aux familles) ; inventaire des lignes, cercles, splines, rectangles et cotations avant de retirer les variantes, sans confondre gestes graphiques et règles métier | P2 | `crates/interface/src/familles/editeur_famille/`, `crates/interface/src/outils/` |
| F6 | Formes guidées, reste : chemin tracé hors du plan de la vue (D), raccordement sur chemin fermé, outil « Tracer le chemin » non essayé à l'écran | P3 | `docs/EDITION.md`, `docs/MODELE.md` |
| F11 | Maillages de famille : import d'un maillage (OBJ, IFC) dans l'éditeur de familles ; écriture compacte des maillages enregistrés | P3 | `crates/modele/src/famille/modelee/maillage.rs` |
| F13 | Motifs de modèle, reste : ancrage par défaut à la face avant le premier placement explicite ; motifs de surface dans les exports SVG/PDF/DWG ; lecture du genre des .pat à l'import (`;%TYPE=MODEL`) | P2 | `crates/moteur3d/src/scene.wgsl`, `crates/maillage/src/materiaux.rs`, `crates/moteur2d/` |
| F8 | Escaliers et garde-corps paramétriques par famille | P3 | `docs/EDITION.md` |

## 6. Structure du code

Description de la dette : `docs/ARCHITECTURE.md` §5.

| Id | Tâche | Priorité | Détail |
|---|---|---|---|
| I12 | Saisie des tracés en double : le projet (`outils/dessiner/dessin_plan/saisie.rs`, saisie clavier dans une chaîne) et l'éditeur de familles (`familles/editeur_famille/canevas.rs` avec `saisie::saisie_trace::SaisieTrace`, plus `chemin_trace.rs` à part) partagent la géométrie du tracé (`moteur2d::dao::Trace`) et le bandeau, mais chacun lit ses touches et ses clics (Entrée, Échap, Espace, F, Tab, valeurs tapées, clic droit) : une règle comme « Échap = Entrée » doit être écrite deux ou trois fois. Extraire une seule saisie de tracé (événements → actions sur `Trace`) utilisée par les deux éditeurs | P2 | `crates/interface/src/saisie/`, `crates/interface/src/outils/dessiner/dessin_plan/saisie.rs`, `crates/interface/src/familles/editeur_famille/canevas.rs` |
| I2 | Catalogue unique des commandes avec schéma JSON généré (`schemars`) et documentation `docs/commandes.md` régénérée et vérifiée par un test : un seul contrat pour interface, pilote et MCP ; base de la palette de commandes (M19). Idée ConstructCraft | P2 | `crates/commandes`, `crates/pilote` |
| I3 | Infobulles du ruban : raccourci, raison d'indisponibilité d'un outil grisé, commande correspondante ; test que la disponibilité suit l'état. Idée ConstructCraft | P2 | `crates/interface/src/zones/ruban/` |
| I4 | Tests d'interface sans affichage (egui_kittest : arbre d'accessibilité, comparaisons d'images à la demande) pour remplacer une partie des essais Xvfb manuels | P2 | `crates/interface` |
| I6 | Budget des booléens (nombre de polygones et de découpes ; au-delà, échec propre au lieu d'un blocage) et `catch_unwind` autour des triangulations | P2 | `crates/noyau/src/csg*.rs`, `noyau::trace2d` |
| A10 | Registre des documents : `Application` (environ 150 champs) transfère 19 champs à la main à chaque changement de document | P2 | `crates/interface/src/lib.rs`, `socle/documents.rs:497-613` |
| R1 | Catalogue unique des outils : cartouches et contrôle complet par le pilote restent à unifier | P2 | audit du 11/11, lot 1 : `git show 9121974:docs/historique/AUDIT_FACTORIZATION_2026-11-11.md` |
| R5 | Contrôleur d'esquisse partagé entre canevas orthogonal et oblique | P2 | idem, lot 5 |
| R7 | Primitives de maillage, passes GPU (`gpu.rs`, un seul `impl Renderer`), dépendances collaboratives | P3 | idem, lot 7 |
| D1 | 44 fichiers au-delà de 600 lignes : à découper quand on y touche | P3 | `docs/ARCHITECTURE.md` §5 |
| D2 | `eframe` dans `config` (adaptateurs d'interface dans une crate de configuration) : à valider | P3 | `docs/ARCHITECTURE.md` §5 |

## 7. Évolutions étudiées, non commencées

| Id | Tâche | Priorité | Détail |
|---|---|---|---|
| E1b | Styles d'objets, reste : à l'écran, lignes vues tracées à un pixel sans tirets (GPU : `moteur3d/src/scene.wgsl`, `LARGEUR_TRAIT`) ; poché de coupe des ouvertures et des composants par leur matériau (couleur seule aujourd'hui) ; éléments importés et liens IFC sans style de catégorie ; tirets de projection repris à chaque arête (pas de phase continue le long d'un contour) ; bibliothèque partagée entre projets si elle reste voulue | P3 | `crates/maillage/src/styles.rs`, `crates/moteur2d/src/vue.rs`, `crates/moteur3d/` |
| E2 | Vérification multiplateforme : essai manuel sur chaque système, cache macOS (demande n° 4) | P2 | §10 |
| E3 | Version navigateur en lecture seule (demande n° 3) | P3 | §10 |

## 8. Collaboration

| Id | Tâche | Priorité | Détail |
|---|---|---|---|
| C2 | Commande de compactage du registre partagé | P3 | `docs/COLLABORATION.md` |

## 9. Décisions en attente

| Sujet | Question |
|---|---|
| U11 | Parmi les réglages retirés le 20/11/2026 parce qu'ils n'agissaient pas (alignement et flèche du texte, contenu de l'étiquette, cote orthogonale et décalage papier, motif de zone, quadrillage en arc, contrainte de Déplacer), lesquels faut-il réaliser ? |
| U7 | Transformations sans sélection préalable : ajouter l'étape « Sélectionner, Entrée pour valider » dans la commande (MODIFICATION.md §1), ou garder la règle actuelle (sélection d'abord) ? |
| Texte | Parcours du texte livré le 20/11/2026 : Entrée valide et Maj+Entrée passe à la ligne ; l'outil reste actif après chaque texte. Convient-il, ou préférer Entrée = nouvelle ligne et Ctrl+Entrée = valider (usage Revit) ? |

## 10. Études des évolutions

Demandes n° 4 et 3 de la feuille de route du 16 novembre 2026 (tâches E2, E3 du §7 ; l'étude de E1, n° 15, est dans l'historique : `git show d8a8bc6:docs/ROADMAP.md`) : architecture ou plan, sans réalisation. Les relevés de code datent du 16 novembre 2026 (commit `35e1c42`).

### 10.2 E2 — Compatibilité multiplateforme (n° 4)

#### État relevé

- **Seule cible installée et vérifiée** : `x86_64-pc-windows-msvc`. Je n'ai lancé aucune compilation ni aucun test Linux, macOS ou ARM64 : les cibles ne sont pas installées sur ce poste.
- **Code propre à Windows**, toujours avec un repli pour les autres systèmes :

| Emplacement | Usage Windows | Repli ailleurs |
|---|---|---|
| `collaboration/src/partage.rs` | `MoveFileExW` (publication sans remplacer) | `hard_link` puis `remove_file` |
| `collaboration/src/session.rs`, `stockage.rs` | `share_mode(0)` (verrou exclusif) | `File::try_lock` |
| `collaboration/src/cli.rs` | mémoire du processus | (mesure absente) |
| `collaboration/src/partage.rs` | cache dans `LOCALAPPDATA` | `XDG_CACHE_HOME`, puis `~/.cache` |
| `pilote/src/mcp.rs` | processus détaché | `process_group(0)` |
| `interface/src/socle/fenetre.rs` | build de Windows (cadre de fenêtre) | faux |
| `app/build.rs` | icône (`winresource`) | rien |

- **Dépendances** : `eframe` est compilé avec `wgpu`, `wayland` et `x11`, et `rfd` existe sur les trois systèmes. Aucune bibliothèque C ne doit être installée à part.

#### Risques

- **macOS** : le cache irait dans `~/.cache` au lieu de `~/Library/Caches` (fonctionnel, mais non conventionnel). Il faut aussi signer et notariser l'application pour la distribuer.
- **Linux** : `rfd` passe par le portail XDG ou GTK, absent sur certains environnements minimaux. Sous Wayland, la décoration de fenêtre personnalisée (`cadre_natif`) reste à vérifier.
- **ARM64** : aucun code `unsafe` dépendant de l'architecture n'a été relevé hors des appels Windows. Le risque principal est la disponibilité des pilotes `wgpu` (Vulkan ou Metal).
- **Verrous de fichiers** sur partage réseau (SMB, NFS) : leur sémantique diffère entre systèmes. La collaboration doit être testée sur chaque système avec un dépôt réseau réel.

#### Plan de vérification

1. Ajouter une intégration continue avec une matrice `windows-latest`, `ubuntu-latest` et `macos-latest` (ARM64), qui lance `cargo check --workspace --tests` puis les tests ciblés `noyau`, `modele`, `commandes`, `moteur2d` et `maillage`. Elle demande un dépôt distant : à décider.
2. Faire un essai manuel de l'interface sur chaque système : ouvrir, tracer, exporter un PDF, collaborer à deux postes.
3. Placer le cache dans `~/Library/Caches/BIMedit` sous macOS, avec un `cfg(target_os = "macos")` dans `racine_cache`.

### 10.3 E3 — Version navigateur, WebAssembly (n° 3)

#### Faisabilité

`eframe` et `wgpu` fonctionnent dans un navigateur (WebGPU, ou WebGL2 en repli), et le modèle (`noyau`, `modele`, `commandes`, `moteur2d`, `maillage`) est du Rust pur. Le portage reste un chantier lourd : l'application repose sur le système de fichiers, les fils d'exécution et les processus.

#### Obstacles relevés (nombre de fichiers par crate)

| Usage | Crates concernées | Remède |
|---|---|---|
| `std::time::Instant` (panique sur `wasm32-unknown-unknown`) | noyau 1, modele 1, maillage 1, moteur2d 1, formats 2, echanges 1, collaboration 4, interface 6, pilote 1, app 2 | crate `web-time`, même API |
| `std::fs` | noyau 1, modele 4, config 1, formats 2, echanges 3, moteur3d 1, collaboration 4, interface 8, pilote 3, app 2 | couche d'accès aux fichiers : lecture et écriture par téléversement et téléchargement ; stockage OPFS ou IndexedDB |
| `rayon` (parallélisme) | 13 crates, dont modele 13, interface 23 et formats 5 | séquentiel dans le navigateur, ou `wasm-bindgen-rayon` (exige l'isolation d'origine : en-têtes COOP et COEP) |
| `std::thread` | collaboration 4, interface 3, pilote 2, echanges 1, app 2 | tâches asynchrones (`wasm-bindgen-futures`) |
| `std::process`, `std::net` | pilote (MCP, instances), collaboration, interface, echanges, app | hors périmètre web : pilote et CLI restent sur poste |
| `rfd` (dialogues) | interface 13 | API asynchrone de `rfd` (navigateur) : à adapter appel par appel |
| verrous de fichiers (collaboration) | collaboration | serveur de synchronisation (`yrs` est déjà la base des transactions) |

#### Périmètre proposé pour une première version web

- **Lecture seule d'abord** : ouvrir un `.modele.json` ou un IFC téléversé, naviguer en 3D, consulter les plans et les coupes, exporter en SVG ou PDF (téléchargement). Crates : `noyau`, `modele`, `scene`, `maillage`, `moteur2d`, `moteur3d`, une partie d'`interface`, `formats` pour l'IFC.
- **Hors périmètre** : import Revit (taille des fichiers, mémoire), pilote MCP, collaboration par fichiers partagés.

#### Performances

- La reconstruction de scène et les booléens (`noyau::csg`) tournent aujourd'hui en parallèle. Sans fils d'exécution, il faut s'attendre à un facteur 4 à 8 sur ce poste : c'est une estimation à mesurer, pas une mesure.
- La mémoire d'un onglet est limitée à 4 Go en `wasm32`.

#### Étapes

1. Remplacer `std::time::Instant` par `web-time` dans tout le dépôt : changement mécanique, sans effet sur le poste.
2. Créer une couche `config::fichiers` (lire, écrire, choisir un fichier) avec une implémentation poste et une implémentation navigateur.
3. Rendre `rayon` facultatif (fonctionnalité `parallele` activée par défaut).
4. Compiler `modele` et `moteur2d` pour `wasm32-unknown-unknown` en intégration continue.
5. Écrire une application web minimale (`crates/web`) : visionneuse en lecture seule.
