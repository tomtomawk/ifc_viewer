# Éditeur BIM natif

Le document `.modele.json` reste la source de vérité. Les ajouts de cette itération
sont compatibles avec le format v0 : les collections absentes d'un ancien fichier
sont initialisées à vide. Aucun fichier RVT n'est écrit.

## Parcours disponible

- **Début → Importer** convertit un IFC/RVT/RFA/RTE dans un nouveau document natif,
  conserve les documents ouverts et écrit le rapport `.rapport.md` à côté du JSON.
  Les murs courbes, ouvertures et maillages importés sont maintenant affichables.
- **Modéliser → Niveau** crée et modifie les niveaux. **Quadrillage** crée des axes
  droits, visibles dans les plans natifs. Les coordonnées de saisie sont en mm.
- **Ouverture** place une porte, une fenêtre ou une baie par clic sur un mur ou par
  abscisse depuis son départ. Une baie d'assemblage perce toutes ses couches. Une
  menuiserie importée conserve son maillage et suit son hôte ; ses dimensions sont
  verrouillées tant qu'elle n'est pas remplacée explicitement par un cadre natif.
- **Modifier** propose déplacement, copie, rotation, alignement des axes et
  ajustement de l'extrémité d'un mur sur l'axe d'un autre. Ctrl+clic ou les cases de
  la liste constituent une sélection multiple, surlignée dans la vue. Copier un assemblage recrée les
  identifiants de ses couches et ouvertures. Supprimer demande confirmation des
  dépendances ; un niveau encore utilisé ne peut pas être supprimé.
- Le panneau Modifier permet aussi d'éditer les sommets d'un élément horizontal,
  d'y ajouter une trémie, de préparer/appliquer une composition depuis son type,
  d'attacher des murs à une toiture et de régler la priorité d'une couche aux
  croisements. Modifier un contour réinitialise ses pentes par arête.
- **Annuler / Rétablir**, Ctrl+Z et Ctrl+Maj+Z, ont un historique propre à chaque
  document. Un glissement est regroupé en une seule modification. L'historique
  retient jusqu'à 100 instantanés, avec une cible de 64 Mio (au moins le dernier
  instantané est conservé, même s'il dépasse cette taille).
- **Enregistrer / Enregistrer sous** valident le modèle et remplacent le fichier
  atomiquement. Chaque modification appliquée écrit une copie `.recuperation.json`.
  À la réouverture, l'application propose de la restaurer. Enregistrer ou fermer
  explicitement sans enregistrer supprime cette copie de récupération.
- **Documenter** crée textes, lignes, zones rectangulaires, cotes et étiquettes
  dans la vue native ouverte. Avec un mur sélectionné, une cote référence ses
  extrémités ; sinon ses points sont fixes. Une référence supprimée est signalée
  en rouge, sans supprimer silencieusement la cote.
- **Feuilles / Nomenclatures**, dans Documenter ou Sortie, permet d'assembler des
  vues, régler format papier/cartouche/échelles, prévisualiser une feuille, exporter
  SVG/PDF, définir un filtre de catégorie et regrouper les quantités par type.
  Les feuilles et nomenclatures sont aussi accessibles depuis l'arborescence.
- **Sortie → IFC4** exporte niveaux, classes architecturales, compositions et
  relations d'ouverture avec une géométrie tessellée. Les GUID IFC valides de la
  source sont préservés ; les autres sont générés de manière stable.

L'exemple autonome `samples/maison_native.modele.json` contient deux niveaux, huit
murs, des ouvertures, deux planchers, une toiture, des murs attachés, des annotations,
une feuille A3 et une nomenclature. Les fichiers `samples/test_ifc4*.ifc` couvrent
également les conversions et les murs courbes.

## Géométrie et limites explicites

- Les angles droits/en L conservent les onglets existants. Aux croisements et aux
  raccords courbes, les volumes sont répartis par priorité : structure (30),
  isolation (20), finition (10), autre (0), puis identifiant pour départager. Une
  priorité explicite par mur/couche remplace cette valeur. Il ne s'agit pas d'un
  solveur de contraintes équivalent à Revit.
- Les toitures concaves à **pente uniforme sur toutes les arêtes** utilisent un
  squelette droit, y compris les noues. Les combinaisons concaves de pentes
  différentes ou d'arêtes sans pente sont refusées avec un message ; elles doivent
  être décomposées en plusieurs toitures. Les contours convexes gardent le calcul
  par arête existant.
- L'attachement requiert que le mur soit entièrement dans l'emprise de la toiture
  (prévoir un débord). Détacher retrouve la contrainte haute enregistrée. Supprimer
  la toiture détache ses murs sans les supprimer.
- Le modèle est reconstruit après chaque opération : la fluidité sur des maquettes
  volumineuses reste à optimiser. La copie de récupération n'est pas un journal
  incrémental ; un calcul qui n'a pas encore abouti n'y figure pas.
- L'export IFC utilise des corps tessellés, pas une restitution des familles Revit
  ni de tous les paramètres métier. Les objets importés deviennent des
  `IfcBuildingElementProxy` portant leur catégorie en `ObjectType`.
- Les textes PDF/SVG sont vectorisés en contours, pas en texte sélectionnable.
  Le PDF d'une feuille respecte son format exact ; l'export d'une vue seule ajoute
  une marge. Imprimer à **100 %**, sans « ajuster à la page ».
- Le cadre de porte/fenêtre natif est une représentation simple. Un configurateur
  de familles, les poteaux/poutres/escaliers/garde-corps paramétriques, les symboles,
  légendes et altimétries, la fusion d'import et la comparaison de réimportations
  restent des itérations suivantes. Les outils non raccordés sont désactivés.
- Les feuilles natives s'ouvrent dans le gestionnaire de documents avec un aperçu ;
  elles ne constituent pas encore des onglets de dessin éditables comme les vues.

## Interfaces et commandes

Les commandes UI et MCP passent par les mêmes transactions. Une erreur de commande,
validation ou reconstruction conserve le modèle précédent.

`projet_natif` renvoie le document JSON et ses identifiants natifs. `editer <JSON>`
utilise ces identifiants, **pas** les identifiants numériques de la scène.

```text
editer {"action":"niveau","nom":"Étage","altitude":3000}
editer {"action":"deplacer","ids":["rdc-m0"],"delta":[1000,0,0]}
editer {"action":"copier","ids":["rdc-m0"],"delta":[0,3000,0]}
editer {"action":"tourner","ids":["rdc-m0"],"centre":[0,0],"angle":90}
editer {"action":"ouverture","mur":"rdc-m0","genre":"porte","abscisse":2000,"largeur":900,"hauteur":2100,"allege":0}
editer {"action":"attacher","ids":["etage-m0"],"toiture":"toiture"}
editer {"action":"supprimer","ids":["porte"]}
annuler
retablir
enregistrer /chemin/projet.modele.json
```

Le type `modele::commandes::Commande` décrit aussi `modifier_niveau`, `quadrillage`,
`aligner`, `ajuster`, `actualiser_type`, `priorite_jonction`, `contour`, `tremie`,
`annoter`, `feuille` et `nomenclature`. Les trois derniers acceptent respectivement
un objet `annotation`, `feuille` ou `nomenclature` ; identifiant vide = création,
identifiant existant = modification.

Sans fenêtre, les mêmes documents peuvent être traités en ligne de commande :

```sh
cargo run -- --editer projet.modele.json commande.json sortie.modele.json
cargo run -- --export-ifc projet.modele.json sortie.ifc
cargo run -- --export-vue projet.modele.json plan-rdc plan.pdf
cargo run -- --export-feuille projet.modele.json A01 feuille.pdf
cargo run -- --quantites projet.modele.json quantites.csv Murs
```

La sortie de dessin accepte `.svg` ou `.pdf`. `--editer` remplace le fichier d'entrée
si la destination est omise. Les commandes renvoient un code non nul en cas d'erreur.

Collections v0 ajoutées : `quadrillages`, `annotations`, `feuilles`, `nomenclatures`.
Tables facultatives : `attaches_toitures` (id du mur/assemblage → id de toiture),
`priorites_jonction` (id du mur/couche → entier). Les annotations appartiennent à une
vue ; les positions et tailles des feuilles sont en mm papier. Aucun historique
Undo/Redo n'est sérialisé dans le projet.

## Vérifications

```sh
cargo test --locked
cargo run -- --export-ifc samples/maison_native.modele.json target/maison.ifc
cargo run -- --export-feuille samples/maison_native.modele.json A01 target/maison.pdf
python tools/valider_ifc.py target/maison.ifc
```

Le dernier contrôle nécessite IfcOpenShell et pytest, seulement pour la validation
indépendante (`python -m pip install ifcopenshell pytest`). Les tests essentiels
utilisent les exemples du dépôt ; les tests du corpus RVT externe continuent à être
sautés lorsque ce corpus n'est pas disponible. Pour un serveur graphique de test
sans anticrénelage, `IFC_VIEWER_MSAA=0` sélectionne directement ce mode.
