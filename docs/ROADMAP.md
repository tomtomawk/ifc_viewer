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
| A1b | Résorber les avertissements clippy restants (131 au 20/11/2026 après correction automatique : 49 types complexes, 11 champs assignés après `Default::default()`, 9 fonctions à plus de 7 arguments, 8 comparaisons niées sur des flottants, 8 éléments après un module de test…), puis rendre clippy bloquant dans la CI | P2 | `.github/workflows/ci.yml`, tâche `debug` |
| T1 | Tests de fichiers malformés : fait pour les cas d'A3 ; restent le balayage aléatoire (fuzzing) d'IFC et de RVT, les fichiers `rvt/{vues,feuilles,nomenclatures,parametres,annexes,zones}.rs` non relus, et `Projet::verifier()` sur un `.modele.json` malformé ; `scene` sans test | P2 | `crates/formats/src/ifc/bornes.rs` |
| T2 | Les 18 scripts `tools/test*.py` (clics simulés) ne tournent pas en CI. Sous Linux (Xvfb, 20/11/2026) : 17 réussissent avec la fenêtre visible (`BIMEDIT_ARRIERE_PLAN=0 xvfb-run -a python3 tools/<script>.py --exe target/debug/bimedit`) ; en mode arrière-plan (fenêtre hors écran, défaut des scripts), les clics de `test-pilote` ne tracent pas de mur sous Xvfb (non vu sous Windows). `test_partage_ui` dépasse le délai : la capture de la première instance, recouverte par la seconde sur le même affichage, ne répond pas (hypothèse non vérifiée ; essayer deux affichages Xvfb ou Windows) | P2 | `tools/` |
| G1 | Retirer les reprises du gabarit pendant l'édition (règle de `docs/AGENTS.md` : le gabarit n'amorce que la création d'un projet et ne sera plus embarqué). Relevé du 20/11/2026 (types d'éléments horizontaux et de terrain retirés depuis) : types de murs du gabarit proposés et copiés au choix (`zones/ruban/murs.rs:106`), types de coupe et d'élévation (`zones/ruban/mod.rs:337`, `zones/fiche_selection.rs:124`), motifs et plumes de repli (`modele/src/lib.rs` : `epaisseurs_plumes`, `definition_motif`, `noms_motifs`, `figer_references`), couleur d'un matériau manquant (`modele/src/murs/assemblage.rs:9`, `materiau_par_defaut`), profil par défaut (`familles/selection_famille.rs:81`), motifs de base (`zones/graphismes.rs:180`). Chaque cas : donnée portée par le projet dès sa création, sinon défaut du code | P2 | `crates/modele/src/gabarit.rs` |

## 2. Réactivité

| Id | Tâche | Priorité | Détail |
|---|---|---|---|
| A8 | Réactivité, reste (mesures du 17/11/2026, immeuble de 1 920 murs) : solides des murs percés 110 à 200 ms après déplacement d'un mur (booléens des façades de 4 290 triangles autour du mur déplacé) ; arêtes jointes 72 à 87 ms (grands groupes de matériaux, tous invalidés par un mur) ; coupe du plan affiché 106 à 116 ms (tâche P1) ; second `verifier()` (14 ms) peut-être redondant | P1 | `crates/maillage/src/aretes.rs`, `modele::volumes::solides`, `crates/interface/src/documenter/coupe.rs` |
| A5b | Arrière-plan, reste : le chargement ne s'arrête pas vraiment (fermer le document abandonne son résultat, le fil finit son calcul) ; l'export SVG d'une coupe (`documenter/coupe.rs`) et l'export SVG de la vue restent sur le fil de l'interface ; l'état « en cours » et le bouton Annuler n'ont pas été vus à l'écran (enregistrement trop rapide sur les échantillons ; exports lancés par un sélecteur de fichier que le pilote ne pilote pas) | P2 | `crates/interface/src/socle/taches.rs` |
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
| U11 | Réglages retirés car jamais lus (U3, 20/11/2026), à réaliser s'ils sont voulus : Texte (ligne de repère avec flèche, alignement gauche / centre / droite), Étiquette (contenu : type, repère, matériau), Cote (mode orthogonal, décalage papier), Zone (motif, pas), Quadrillage en arc, Déplacer contraint horizontal / vertical (Maj contraint déjà aux axes) | P3 | `zones/ruban/mod.rs` (`champs`) ; question au §9 |
| U7 | Revue des parcours : Déplacer, Copier, Tourner, Symétrie, Réseau, Échelle exigent une sélection préalable ; pas d'étape « Sélectionner, Entrée pour valider » dans la commande | P2 | `outils/modifier/transformer.rs`, `docs/MODIFICATION.md` §1 |

## 5. Familles

Objectif : une famille a un document, un éditeur et un format communs quelle que soit sa catégorie ; elle peut porter de la géométrie 3D et des éléments 2D dans ses vues. Cartouches et étiquettes n'utilisent dans le projet que le 2D de la vue en plan ; un profil y prend sa section (contour fermé désigné). Les outils sont ceux du projet (un contrôleur commun, un adaptateur par contexte : projection, unités, données). Les anciennes familles seront refaites, sans migration (`docs/AGENTS.md`). Fonctionnement réalisé : `crates/interface/src/familles/AGENTS.md`, `docs/EDITION.md`, `docs/MODELE.md`.

| Id | Tâche | Priorité | Détail |
|---|---|---|---|
| F3 | Suites des plans tournés et du plan de dessin actif : éléments 2D et cotes saisis en vue 3D sur le plan actif (seuls les profils des formes le sont) ; poignées et transformations des formes posées sur un plan (`support`) ; « Plan par 3 points » quand aucune droite d'appui existante n'est dans le plan (enchaîner deux rotations automatiquement) et depuis une vue plane ; cotes et EQ entre plans tournés parallèles ; à essayer à l'écran (binaire release) : outil « Plan par 3 points », choix d'une face, fermeture d'un profil par clic droit | P2 | `crates/modele/src/famille/modelee/orientation.rs`, `crates/interface/src/familles/editeur_famille/plan_actif.rs` |
| F5 | Validation des familles (ancien lot 7) : outils et leurs poignées, JSON courants, paramètres, résolution des champs, exports, profils, règle de rendu du plan ; 38 tests Revit ; recette visuelle avec plusieurs projets et familles | P1 | `crates/modele/src/famille/`, `crates/interface/src/familles/` |
| F10 | Outils communs projet / familles, reste à relever (état non vérifié le 20/11/2026) : texte et libellés (placement, poignées, contenu, hauteur, champs dynamiques et aperçu ; ouvrir au projet les fonctions propres aux familles) ; inventaire des lignes, cercles, splines, rectangles et cotations avant de retirer les variantes, sans confondre gestes graphiques et règles métier | P2 | `crates/interface/src/familles/editeur_famille/`, `crates/interface/src/outils/` |
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
| R1 | Catalogue unique des outils : cartouches et contrôle complet par le pilote restent à unifier | P2 | audit du 11/11, lot 1 : `git show 9121974:docs/historique/AUDIT_FACTORIZATION_2026-11-11.md` |
| R5 | Contrôleur d'esquisse partagé entre canevas orthogonal et oblique | P2 | idem, lot 5 |
| R7 | Primitives de maillage, passes GPU (`gpu.rs`, un seul `impl Renderer`), dépendances collaboratives | P3 | idem, lot 7 |
| D1 | 44 fichiers au-delà de 600 lignes : à découper quand on y touche | P3 | `docs/ARCHITECTURE.md` §5 |
| D2 | `eframe` dans `config` (adaptateurs d'interface dans une crate de configuration) : à valider | P3 | `docs/ARCHITECTURE.md` §5 |

## 7. Évolutions étudiées, non commencées

| Id | Tâche | Priorité | Détail |
|---|---|---|---|
| E1 | Styles d'objets globaux (demande n° 15) | P2 | §10 |
| E2 | Vérification multiplateforme : essai manuel sur chaque système, cache macOS (demande n° 4) | P2 | §10 |
| E3 | Version navigateur en lecture seule (demande n° 3) | P3 | §10 |

## 8. Collaboration

| Id | Tâche | Priorité | Détail |
|---|---|---|---|
| C1 | Qualification NAS à deux machines (9 étapes) | P1 | `docs/COLLABORATION.md`, « Qualification NAS à deux machines » |
| C2 | Commande de compactage du registre partagé | P3 | `docs/COLLABORATION.md` |

## 9. Décisions en attente

| Sujet | Question |
|---|---|
| E1 | Emplacement partagé de la bibliothèque de styles, liste des catégories, priorité matériau / catégorie |
| U11 | Parmi les réglages retirés le 20/11/2026 parce qu'ils n'agissaient pas (alignement et flèche du texte, contenu de l'étiquette, cote orthogonale et décalage papier, motif de zone, quadrillage en arc, contrainte de Déplacer), lesquels faut-il réaliser ? |
| U7 | Transformations sans sélection préalable : ajouter l'étape « Sélectionner, Entrée pour valider » dans la commande (MODIFICATION.md §1), ou garder la règle actuelle (sélection d'abord) ? |
| Texte | Parcours du texte livré le 20/11/2026 : Entrée valide et Maj+Entrée passe à la ligne ; l'outil reste actif après chaque texte. Convient-il, ou préférer Entrée = nouvelle ligne et Ctrl+Entrée = valider (usage Revit) ? |
| Commits | `AGENTS.md` demande de pousser sur `master` ; la session cloud était configurée pour une branche de travail. Les commits du 20/11/2026 ont été poussés sur `master` selon `AGENTS.md` : à confirmer pour les sessions autonomes suivantes |

## 10. Études des évolutions

Demandes n° 15, 4 et 3 de la feuille de route du 16 novembre 2026 (tâches E1, E2, E3 du §7) : architecture ou plan, sans réalisation. Les relevés de code datent du 16 novembre 2026 (commit `35e1c42`).

### 10.1 E1 — Styles d'objets globaux (n° 15)

#### Besoin

Une bibliothèque de styles indépendante des projets, par catégorie d'objets : couleur, épaisseur de trait, motifs, transparence. Le besoin est de prévoir l'architecture tôt, et de préciser les surcharges locales et la synchronisation.

#### Existant

- Les styles sont aujourd'hui portés par chaque projet :
  - `materiaux[]` : couleur, motifs de coupe et de surface, apparence ;
  - `plumes` : épaisseurs ;
  - `motifs[]` : définitions de hachures ;
  - `types_traits` : types de trait.
- Le gabarit (`gabarits/defaut.gabarit.json`, `modele::gabarit`) amorce un nouveau projet, puis n'est plus relu. Chaque projet garde sa copie.

#### Architecture proposée

1. **Fichier de bibliothèque**, hors projet : `styles.bimstyles.json`, dans le dossier de configuration de l'utilisateur (crate `config`) ou à un chemin partagé de l'agence. Contenu :
   - `version` ;
   - `categories[]` : `cle` (catégorie IFC ou native : `murs`, `dalles`, `ouvertures`, `annotations/cotes`…), `couleur`, `plume` (1 à 12), `motif_coupe`, `motif_surface`, `transparence` (0 à 1), `type_trait`, chaque champ facultatif ;
   - `motifs[]` et `types_traits[]` référencés par nom.
2. **Résolution d'un style**, de la plus faible à la plus forte priorité :
   1. valeur par défaut du code ;
   2. bibliothèque globale (catégorie) ;
   3. surcharge du projet (catégorie) ;
   4. surcharge de vue (gabarit de vue, visibilité / graphismes) ;
   5. matériau de l'élément pour la couleur et les motifs de coupe.

   Une fonction pure `modele::styles::resoudre(bibliotheque, projet, vue, categorie) -> StyleResolu` sert le maillage (`maillage`) et le dessin (`moteur2d`) : un seul point de calcul.
3. **Surcharges locales** : `projet.styles_surcharges[]`, avec la même structure que les catégories de la bibliothèque. Un champ absent hérite de la bibliothèque. Propriétés montre la provenance de chaque valeur (« bibliothèque », « projet », « vue ») et propose de rétablir la valeur héritée.
4. **Synchronisation** :
   - le projet mémorise `styles_source` : chemin et empreinte SHA-256 de la bibliothèque lue (`noyau::hash` existe) ;
   - à l'ouverture, une empreinte différente déclenche un message (n° 16) « Bibliothèque de styles modifiée » avec deux choix : appliquer, ou garder la copie du projet ;
   - le projet garde un instantané des valeurs résolues de la bibliothèque. Il s'affiche donc à l'identique sur un poste sans accès à la bibliothèque, et un export reste reproductible.
5. **Collaboration** : `styles_surcharges` devient une collection identifiée (`commandes::socle::objets::COLLECTIONS`). La bibliothèque elle-même n'entre pas dans les transactions partagées : chaque poste la lit, et l'instantané du projet fait foi.

#### Points à décider avant réalisation (§9)

- Emplacement partagé de la bibliothèque (serveur de l'agence ou dossier synchronisé), et qui a le droit de la modifier.
- Liste fermée des catégories, alignée sur la boîte de visibilité actuelle (`zones/barre_haute.rs`, catégories de modèle et d'annotation).
- Le matériau prime-t-il sur la catégorie pour la couleur en style « Graphique » ? Proposition : oui en coupe, non en vue.

#### Ordre de réalisation

1. `modele::styles` : bibliothèque, surcharges, `resoudre`, tests et schéma dans `docs/MODELE.md`.
2. `maillage` et `moteur2d` : lecture du style résolu.
3. Interface : éditeur de bibliothèque, provenance dans Propriétés, message de synchronisation.

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
