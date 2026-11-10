# Pilote BIMedit

Le serveur MCP (`bimedit --mcp`) et le canal TCP local utilisent les mêmes commandes.
Une requête est une ligne UTF-8 `@jeton commande argument`, sa réponse est un objet
JSON avec `ok`.

## Instances et jeton

- Chaque viewer écoute sur `127.0.0.1`, port 47811 s'il est libre, sinon un port choisi
  par le système, et s'inscrit dans `%LOCALAPPDATA%\BIMedit\instances\<pid>.json`
  (`pid`, `port`, `jeton`, `fichier` ouvert, `partage` collaboratif). L'inscription
  disparaît à la fermeture ; le pont efface celles des viewers qui ne répondent plus.
- Le jeton (32 caractères aléatoires, nouveau à chaque lancement) est exigé : une ligne
  sans `@jeton` ou avec un autre jeton est refusée.
- `viewer_instances` (MCP) : `lister` les viewers ouverts, lancés par l'utilisateur ou
  par le pont ; `choisir` (`pid`) celui que les autres outils pilotent ; `nouvelle` en
  lance un et le choisit. Une seule instance ouverte est pilotée sans choix ; aucune :
  le pont en lance une ; plusieurs sans choix : les outils sont refusés.
- Plusieurs instances sur un même projet : l'associer au même `.bimpartage` (`collaborer`
  dans l'une, `ouvrir` du `.bimpartage` dans l'autre). Chacune garde sa copie locale et
  `collaboration_sync` échange les transactions.
- `etat` rend `instance` (`pid`, `port`).
- Mode arrière-plan : `BIMEDIT_ARRIERE_PLAN=1` crée la fenêtre sans l'activer, hors de
  l'écran et hors de la barre des tâches. Elle reste rendue et pilotable (saisies egui,
  captures). Les scripts `tools/test*.py` l'activent par défaut ; `BIMEDIT_ARRIERE_PLAN=0`
  pour voir la fenêtre. Passé au pont MCP (bloc `env` de sa configuration), il s'applique
  aux viewers que le pont lance.
- Mode isolé (tests, CI) : `BIMEDIT_PORT` fixe le port, sans inscription ; le jeton n'est
  exigé que si `BIMEDIT_JETON` est défini (le pont lancé avec les mêmes variables le
  transmet). Les chemins simples occupent tout l'argument
(espaces acceptés) ; les commandes structurées reçoivent du JSON sur une seule ligne.
Les coordonnées BIM sont en mm, indépendamment de l'unité d'affichage ; caméra et
zone de coupe sont en m. Les angles de caméra et de pente sont en degrés.

## Découverte et état

- `aide [pilote|edition|modele|commandes]` : cette documentation ou les contrats de
  l'édition et du modèle, aussi disponibles dans `viewer_aide` (argument `sujet`).
- `etat` : chargement, enregistrement, sélection, caméra, collaboration, entrées en
  attente et rectangle de la vue (points logiques de la fenêtre).
- `erreur_effacer` : ferme le message d'erreur courant.
- `projet_natif` : objet `projet` complet et `empreinte` de cet état.
- `documents` ; `document <id>` : lister et activer les documents ouverts, projets
  (`"id":"3"`) et familles en édition (`"id":"famille:2"`), avec `genre`, `nom`,
  `actif`, `modifie`. `documents` rend aussi `actif` et le `registre` des changements
  de document actif (`rang`, `document`, `precedent` ; 100 derniers), qu'ils viennent
  de l'interface ou du pilote. Chaque réponse objet du pilote porte `document_actif`
  (`id`, `genre`, `nom`, `modifie`) : vérifier qu'il s'agit du bon document avant
  d'enchaîner. Dans une famille, `annuler` / `retablir` et `historique_parcourir`
  agissent sur la famille.
- `fermer_vue [autres]` : fermer la vue active (et son projet si elle est la dernière), ou garder la dernière vue de chaque document. Le dernier onglet modifié exige un enregistrement préalable par le pilote.

## Fichiers et sorties sans boîte de dialogue

- `nouveau {"chemin":"C:/travail/projet.modele.json","nom":"Projet"}` : crée et
  ouvre un nouveau document, conserve les précédents ; refuse une destination existante.
- `ouvrir <chemin>` : IFC, RVT/RFA/RTE, JSON natif ou `.bimpartage` ; attend le chargement.
- `fermer [abandonner]` : ferme le document ; modifications non enregistrées refusées
  sauf abandon explicite. Refuse une opération en cours.
- `importer {"source":"C:/source.ifc","sortie":"C:/projet.modele.json"}` : lance
  la conversion dans un nouveau document natif. Consulter `etat.chargement` jusqu'à
  false, puis `erreur_chargement`. Refuse une destination existante.
- `enregistrer [chemin.modele.json]` : enregistrer/enregistrer sous ; un RVT démarre
  une conversion suivie dans `etat.enregistrement`. Une copie collaborative garde son chemin.
- `exporter <chemin.svg>` ; `exporter_pdf <chemin.pdf>` : dessin de la vue active.
- `exporter_ifc <chemin.ifc>` : projet natif en IFC4.
- `exporter_jeu <chemin.pdf> [jeu]` : jeu de feuilles PDF (syntaxe historique sans espaces dans le chemin).
- `export {"format":"jeu_pdf","chemin":"C:/Projet partagé/Plans.pdf","jeu":"Plans"}` :
  export structuré acceptant les espaces ; formats svg, pdf, ifc, csv (id requis),
  jeu_pdf, feuille_svg et feuille_pdf (id de la feuille requis pour les deux derniers).
- `nomenclature {"id":"…"}` : définition et tableau calculé.
- `exporter_csv {"id":"…","chemin":"C:/quantites.csv"}` : export de ce tableau.
- `familles` ; `famille_editer <id de type>` ; `famille_ouvrir <chemin>` ;
  `famille_charger <chemin>` ; `famille_enregistrer <chemin>` : éditeur/fichiers de familles.
- `famille_nouvelle porte|fenetre` ; `famille_document` : ouvrir un gabarit et lire
  le fichier de l'éditeur (`fichier`, `type`, `modifie`).
- `famille_modifier <objet fichier complet>` : modifier et valider la famille dans
  l'éditeur (plans, contraintes, géométrie, formules, matériaux, dimensions…).
- `famille_charger_projet [nouveau]` : charger les modifications dans le type, ou
  créer un nouveau type ; propagation des occurrences et historique du projet.
- `famille_fermer [abandonner]` : fermeture sans dialogue, abandon explicite si modifiée.

## Transactions et propriétés

`editer <JSON>` exécute les commandes métier documentées dans `docs/EDITION.md` :
terrain, limite_propriete, coordonnees_origine, barre, escalier, garde_corps,
modifier_vue, modifier_quadrillage, niveau, quadrillage, annoter, feuille,
nomenclature, modifier_niveau, ouverture, deplacer, copier, tourner, aligner,
ajuster, actualiser_type, attacher, priorite_jonction, supprimer, contour, tremie,
piece, composant, type_composant.
Les champs et types sont ceux de `src/modele/edition/commandes/mod.rs`.

Pour les autres propriétés (types, matériaux, plumes, motifs, phases, variantes,
sous-projets, contextes, murs, dalles, familles paramétriques, cartouches, vues,
cadrages, nomenclatures, paramètres partagés…), utiliser `modifier_projet` :

```json
{"empreinte":"empreinte obtenue par projet_natif","operations":[
  {"op":"replace","path":"/nom","value":"Projet renommé"},
  {"op":"replace","path":"/niveaux/0/altitude","value":120}
]}
```

Opérations JSON Patch : `add`, `replace`, `remove`, `test`, `copy`, `move`.
Chemins JSON Pointer (`~1` pour `/`, `~0` pour `~`), indices de tableaux depuis 0,
`/-` pour ajouter en fin. Le parent doit exister ; créer une collection absente
avec `add /materiaux []` avant d'y ajouter un objet. Utiliser des identifiants neufs
pour les créations ; relire le projet après la transaction. L'empreinte évite
d'écraser une modification concurrente ; un état périmé exige une nouvelle lecture.
Identité, format et version du projet sont immuables. Le document est validé et
la scène préparée avant le commit ; historique et collaboration suivent les mêmes
chemins que les modifications manuelles. Une erreur ne modifie pas le document.
Les références/hébergements doivent rester cohérents : préférer les commandes métier
pour déplacer, copier, supprimer et régler les ouvertures.

- `selection_native ["id1","id2"]` : sélection multiple avec identifiants natifs.
- `annuler` ; `retablir` ; `historique` : états et libellés des piles locales.
- `historique_parcourir {"action":"annuler","nombre":2}` : plusieurs étapes ;
  en collaboration, une transaction à la fois (les piles locales n'y sont pas utilisées).

## Outils graphiques et saisie

`outil <nom>` active tous les outils opérationnels du ruban :
niveau, quadrillage, mur, terrain, limite_propriete, dalle, sol, plafond, toiture,
poteau, poutre, escalier, garde_corps, ouverture, composant, piece, arbre, deplacer, copier,
tourner, aligner,
ajuster, modifier_trace, creer_identique, supprimer, cadrer, isoler, masquer, texte,
cote, etiquette, ligne, polyligne, spline, cercle, arc, zone, vue_plan, coupe,
vue_coupe, feuilles, nomenclatures. `outil aucun` annule l'outil.
Les outils encore désactivés dans l'interface restent indisponibles.

`saisie <tableau JSON>` produit les mêmes entrées egui que l'utilisateur :

```json
[
  {"action":"clic","x":600,"y":400,"bouton":"gauche"},
  {"action":"texte","texte":"@2000,0"},
  {"action":"touche","touche":"Enter"}
]
```

Actions : `clic` (bouton optionnel gauche/droit/milieu, maj/ctrl optionnels),
`pointer` (x,y), `bouton` (x,y,bouton,appuye,maj/ctrl optionnels) pour les glissements,
`defiler` (x,y,dx,dy), `texte` (texte), `touche` (touche,maj/ctrl/alt optionnels).
Touches : Enter, Escape, Delete, Backspace, Tab, ArrowLeft/Right/Up/Down, A…Z, etc.
Les positions sont des points logiques absolus dans la fenêtre, pas des pixels PNG
physiques ni des coordonnées BIM. `etat.rectangle_vue` permet de situer la vue.
Chaque entrée est traitée sur une image distincte. Attendre
`etat.saisies_en_attente == 0` avant une nouvelle opération ou une capture.
Ces entrées permettent de régler les paramètres d'outil, tracer, valider/annuler,
modifier une famille et utiliser les dialogues de l'interface ; les dialogues
natifs du système ne sont pas pilotés par egui (utiliser les commandes de fichiers).

Raccourcis existants :
- `mur x0 y0 x1 y1 [hauteur epaisseur]` ; `type_mur nom|materiau:epaisseur[:fonction]|…`.
- `dalle x0 y0 x1 y1 x2 y2 …` ; `surface type <nom>` ; `surface pente oui|non` ;
  `surface <parametre> <valeur>` ; `pente <id|dernier> <arete|toutes> <degres|aucune>`.
- `modifier_trace <id>` ; `vue_plan <nom ou id niveau> [hauteur coupe]` ;
  `coupe_vue x0 y0 x1 y1 [profondeur] [elevation]`.

## Collaboration

- `collaborer <chemin.bimpartage>` : création asynchrone d'un partage.
- `collaboration_associer <dossier central>` : association administrative.
- `collaboration_sync` ; `collaboration_instantane` : demande de synchronisation.
- `collaboration_etat` : copie, baux, transactions en attente, textes et conflits.
- `collaboration_reserver ["id natif",…]` ; `collaboration_liberer […]` ;
  `collaboration_reprendre […]` : réservation/libération/reprise explicite.
  Sans argument, utilise la sélection native. Consulter `collaboration_etat` :
  le succès de la commande signifie que la synchronisation est demandée.
- `collaboration_resoudre {"transaction":"…","choix":"locale"}` : nouvelle
  transaction validée ; `choix:"acceptee"` archive la proposition et garde l'état accepté.

## Vue et interface

- `ruban debut|modeliser|modifier|documenter|sortie|famille_creer|famille_sortie`.
- `options` lit ; `options {"navigation_ifc":true,"unite":"centimetre"}` modifie
  et mémorise (unités : millimetre, centimetre, metre).
- `gabarit` lit ; `gabarit <chemin.gabarit.json>` charge ; `gabarit defaut` rétablit.
- `gabarit_exporter <chemin.gabarit.json>` enregistre une copie du gabarit actif.
- `vues` ; `vue <indice|nom>` ; `feuilles` ; `selection_feuille <JSON>` ; `proprietes_feuille`.
- `camera dessus|face|iso|cadrer|ortho|perspective|deux_points` (deux_points : perspective à verticales droites, enregistrée avec la vue 3D) ou `camera lacet tangage [distance]` ; `zoom facteur`.
- `zone on|off|ajuster` ou `zone xmin ymin zmin xmax ymax zmax`.
- `glisser dx dy [gauche|droit|milieu] [maj]` ; `objet x y` (0..1 dans la vue).
- `selectionner <id STEP>` ; `isoler <id>` ; `cadrer <id>` ; `proprietes [id]`.
- `afficher <CLASSES>` ; `masquer <CLASSES>` ; `tout_afficher` ; `classes` ; `elements [CLASSE|*] [texte]`.
- `categories` ; `visibilite oui|non` ; `visibilite <categorie> oui|non`.
- `panneau proprietes|arborescence gauche|droite|flottant|externe|fermer` ; `fiche oui|non`.
- `onglet tout|vues|feuilles|nomenclatures|familles|materiaux|groupes|liens|modele|classes`.
- `style filaire|lignes_cachees|ombre|couleurs|realiste|rendu` ; `dessin oui|non` ; `traits_fins oui|non`.
- `vue_camera_maj` : enregistre le point de vue courant dans la vue caméra active (« Mettre à jour la vue ») ; `vue_camera_retablir` : y revient.
  Naviguer dans une vue caméra ne modifie plus son point de vue enregistré.
- `pieton oui|non|avancer <m>` : vue piéton (œil à 1,60 m au-dessus du sol, marche qui suit le sol et s'arrête aux murs) ;
  `vue_camera [nom]` : enregistre le point de vue courant comme vue 3D du projet (genre `camera` : `oeil`, `cible` en mm, `champ` en degrés).
- `soleil [jour mois heure]` : lit, ou règle, la date (sans année) et l'heure légale (heures décimales) du soleil de la vue ;
  rend `azimut` et `hauteur` (degrés), calculés au lieu du projet (`projet.lieu`, Rennes par défaut).
- `ciel [JSON]` : lit ou modifie les réglages du ciel physique de la vue (`ambiance` Clair|Voile|Nuageux|Couvert|Brumeux,
  `nuages`, `brume`, `intensite_soleil`, `intensite_ciel`, `taille_soleil`, `sol`, `exposition_auto`, `exposition` en EV).
  Style, ombres, soleil et ciel sont enregistrés par vue dans `projet.rendus_vues`.
- `ombres portees|ambiante oui|non` ; `coupe non` ou `coupe hauteur [niveau]` ; `niveaux` ;
  `poche hachure|plein|vide` ; `echelle denominateur`.
- `capture vue|fenetre [LxH] <chemin.png>` ; `taille largeur hauteur` ; `reduire` ; `restaurer`.
- `quitter [abandonner]` : sortie sans dialogue ; refuse des documents modifiés
  sauf abandon explicite, et refuse les opérations en cours.

## Paramètres et accrochages des murs

`options` accepte aussi `delai_accrochage` (secondes, 0.8 par défaut) et
`raccourcis` (carte identifiant de commande → touche/séquence). Exemple :
`options {"delai_accrochage":0.8,"raccourcis":{"mur":"MW"}}`.
`raccourcis` liste les commandes, catégories, valeurs par défaut et valeurs actives.
Les doublons, préfixes ambigus et touches inconnues sont refusés avant toute écriture.

`parametres ouvrir|fermer|etat|general|dessin|raccourcis|graphisme|gabarit` pilote
l'ouverture et les catégories du panneau. `axe_selection` décrit la ligne
d'implantation du mur sélectionné, les positions écran de ses poignées, son point
déplacé et l'état des accrochages. La saisie `bouton`/`pointer` permet de glisser
une poignée, de contrôler la référence acquise et de relâcher ou annuler.

`outil ajuster` active le parcours graphique : un clic choisit l'axe limite,
le survol d'une extrémité prépare l'aperçu et un second clic valide.
`ajuster_etat` expose en lecture seule la référence, le survol, l'aperçu et
l'éventuel motif de refus, pour contrôler la saisie graphique sans modifier le projet.

`ecran <x> <y>` donne la position écran d'un point du plan (mm) à l'altitude du niveau
de la vue. `composant_outil [JSON]` lit ou règle le composant à placer
(`type_composant`, `rotation`, `decalage`) et liste les types ; Espace (`touche Space`)
tourne l'aperçu de 90°. L'outil `piece` crée une pièce au clic dans un espace fermé
par des murs (refus si le contour n'est pas fermé).

`ajuster_angle oui|non` active la case du raccord en angle ; la transaction métier
`ajuster_angle` reçoit `id`, `reference`, `fin` et `fin_reference` et modifie les deux
murs atomiquement. `volets_etat` décrit les fenêtres natives et leurs rectangles ;
`volets_retablir` réancre les deux volets. Fermer un volet externe conserve le projet.
`famille_navigation etat|elevation|plan|coupe|cadrer` décrit ou choisit la vue de
travail et son cadrage. La molette (`saisie defiler`) et le glissement au bouton
`milieu` utilisent la navigation réelle de l'éditeur, sans toucher au modèle.

`mur_trace ligne|rectangle|arc|cercle|selection` choisit le mode graphique de création
et active l'outil Mur. Sans argument, il décrit les points acquis, l'invite et les
poignées écran d'un mur courbe sélectionné. Le cercle de mur attend centre, rayon,
début puis fin de périmètre ; les annotations circulaires restent en deux clics.
`editer {"action":"modifier_trajectoire_mur","id":"…","points":[[x0,y0],[xm,ym],[x1,y1]]}`
modifie atomiquement la ligne d'implantation d'un mur courbe et toutes ses couches.
