# Plan de fusion des familles

## Objectif

Une famille possède un document, un éditeur et un format communs, quelle que soit sa catégorie. Elle peut contenir de la géométrie 3D et des éléments 2D dans ses différentes vues. Pour les catégories cartouche et étiquette, seul le contenu 2D de la vue en plan est utilisé pour le rendu dans le projet. Pour un profil, la vue en plan fournit la section exploitable par les éléments qui utilisent ce profil. Les données des autres vues et les formes 3D restent enregistrées dans la famille.

Le logiciel n'étant pas en production, les anciennes familles seront refaites : aucune migration des anciens formats n'est prévue. Les changements de schéma doivent être documentés conformément à `docs/AGENTS.md`.

## Outils communs au projet et aux familles

Centraliser chaque outil et son parcours graphique pour tous les contextes : catalogue, disponibilité, bandeau, saisie, aperçu, accrochage, sélection, poignées, propriétés et validation. Un même contrôleur produit une opération appliquée par un adaptateur au document actif ; le contexte fournit la projection, les unités, les données et les possibilités réellement disponibles. Les améliorations sont partagées dans les deux sens entre le projet, les familles et les documents.

- **Polyligne** : remplacer les parcours concurrents par celui des vues de projet, enrichi au besoin par les autres ; fermeture et remplissage restent des propriétés de la polyligne.
- **Texte et libellés** : comparer et réunir le placement graphique, la sélection et le déplacement avec poignées, la modification du contenu, de la position et de la hauteur, ainsi que les champs dynamiques et leur aperçu. Permettre au projet de bénéficier des fonctions pertinentes actuellement réservées aux familles ; chaque contexte fournit ses propres champs et unités.
- **Tous les autres outils** : inventorier et reprendre leurs fonctions utiles avant de supprimer les variantes, notamment lignes, arcs, cercles, splines, rectangles et cotations. Ne pas confondre les mêmes gestes graphiques avec les règles métier propres à chaque destination.

## Navigation des vues de famille

Aligner le fonctionnement des vues de famille sur celui des vues de projet :

- Corriger le rendu de la vue 3D et vérifier les formes visibles, le cadrage et les interactions sans projet ouvert.
- Ouvrir et identifier les vues dans les onglets avec leur nom, comme dans le projet, tout en conservant leur association au document de famille.
- Retirer le nom de vue et le bouton « Cadrer » du haut du canevas ; utiliser les mêmes commandes et la même disposition que dans le projet.
- Cadrer par double-clic sur la molette dans les vues planes et 3D, et aligner zoom, déplacement et rotation sur les gestes du projet.
- Réserver le mur hôte aux catégories hébergées par un mur ; vérifier son affichage dans toutes les vues concernées.

## Lots proposés

1. **Modèle de famille unique.** Introduire une catégorie explicite et un document commun regroupant paramètres, vues, formes 3D, éléments 2D et propriétés nécessaires à chaque catégorie. Définir un format JSON unique et valider les données avant le chargement. Les anciens modèles spécialisés restent en place temporairement pendant la construction du nouveau parcours, puis sont retirés.
2. **Dessin et annotations 2D.** Reprendre le parcours de polyligne des vues de projet, y compris le bandeau de dessin, la saisie, l'accrochage, la fermeture, la sélection, les poignées et les propriétés. Une polyligne fermée porte les propriétés de remplissage ; aucun outil distinct « région » ou « zone fermée » n'est nécessaire. Étendre les éléments par vue aux arcs, cercles, splines, rectangles, textes, libellés et cotes graphiques. Distinguer les cotes graphiques des contraintes qui pilotent la famille.
   Réutiliser autant que possible le canevas et la configuration des vues de projet dans l'éditeur de famille : navigation, fond, curseur, repères, affichage, bandeau et outils communs. Prévoir des adaptateurs de contexte pour la géométrie et les opérations propres aux familles, sans dépendre d'un projet ouvert.
3. **Éditeur commun et catégories.** Utiliser le même document, les mêmes vues et le même registre pour toutes les catégories. Rendre disponibles les outils appropriés sans supprimer la géométrie 3D ou les autres vues. Reprendre les propriétés de cartouche et l'édition paramétrique des profils.
4. **Cartouches et étiquettes.** Adapter le chargement et le rendu pour exploiter les seuls éléments 2D de la vue en plan. Conserver le repère et les unités papier, les dimensions du cartouche, les champs dynamiques, leur aperçu, les affectations aux feuilles et aux pièces, et l'export. Remplacer l'actuelle « face imprimée », liée à l'élévation, par le plan dans le nouveau modèle.
5. **Profils.** Définir comment désigner un contour fermé de la vue en plan comme section ; conserver paramètres, formules, validation et dépendances des éléments qui utilisent le profil. Ne pas assimiler les autres éléments 2D du plan au contour de section.
6. **Chargement et nettoyage.** Conserver le parcours autonome et l'association explicite au projet destinataire, puis retirer les éditeurs, structures et formats devenus redondants. Vérifier les occurrences et les mises à jour après rechargement, ainsi que l'ouverture, la fermeture et l'historique.
7. **Validation ultérieure.** Compiler à chaque lot ; lorsque la campagne de tests sera demandée, vérifier les outils et leurs poignées, les JSON courants, les paramètres, la résolution des champs, les exports, les profils et la règle de rendu du plan. Compléter par une recette visuelle avec plusieurs projets et familles.

Chaque lot doit rester relisible. Le passage d'un lot au suivant ne doit pas masquer les fonctionnalités encore prises en charge par les anciens éditeurs.

## Avancement

- Le modèle commun porte la catégorie, les volumes, les vues et des annotations 2D partagées avec le projet. Le changement de catégorie conserve son contenu tant que la famille n'est pas chargée.
- Le chargement d'un cartouche ou d'une étiquette conserve le document complet ; le rendu est reconstruit depuis les seuls éléments 2D de son plan plutôt que depuis une copie du dessin. Les profils chargés conservent leur document et utilisent une section choisie parmi les contours fermés de la vue en plan, ou le seul contour lorsqu’il est unique. Les familles documentaires par défaut et les quatre profils embarqués possèdent un document commun ; les trois familles d'ouverture du gabarit portent la catégorie et les opérations du schéma courant.
- Les événements de tracé et les principaux boutons du bandeau de polyligne sont partagés par les vues de projet, le cartouche et la famille. La famille utilise également le moteur commun pour les arcs, cercles, splines et rectangles. L’accrochage utilise le moteur DAO commun et tient compte des formes 2D de la vue de famille. Les tracés de famille et de cartouche partagent l’interprétation des coordonnées absolues, relatives, polaires et des distances avec le projet ; l’aperçu des valeurs et le guide d’angle utilisent le même calcul que dans le projet. Les guides par survol et leurs intersections utilisent désormais le même repérage dans le projet, les familles modélisées et les cartouches spécialisés.
- Le texte possède des propriétés communes et peut être placé et manipulé dans l'éditeur de famille. Les formes 2D utilisent le tracé DAO commun et les icônes du projet. Les textes et remplissages des éléments 2D sont rendus dans les vues de projet ; les champs sont résolus pour les feuilles et pièces. Les parcours complets de placement et d'édition restent à unifier.
- Les anciens états, registres et formats d'éditeurs plans restent présents pour les parcours encore spécialisés. Leur suppression et la fusion complète des sélections, poignées, propriétés et du canevas restent à réaliser.
- `cargo check` et 342 tests automatisés passent (deux ignorés) ; 38 tests de lecture de maquettes Revit, trop longs lors de cette campagne, restent à exécuter. Les parcours pilotés couvrent aussi les familles, les composants, les pièces et les exports. Une recette visuelle complète reste à réaliser.

- Le mur hôte générique des vues planes est limité aux portes et fenêtres.

- Les familles modélisées ouvrent un onglet par vue, avec activation et fermeture indépendantes ; le dernier onglet ferme le document selon les règles habituelles. Le bandeau supérieur a été retiré. Le double-clic molette cadre aussi la 3D, dont le zoom vise le curseur. La vue 3D a été vérifiée sur les gabarits de fenêtre et de composant dans une version reconstruite ; l’alignement complet des gestes et les anciens éditeurs plans restent à reprendre.

- La saisie des points par clavier est disponible pour les polylignes, arcs, cercles, splines et rectangles des familles modélisées et des cartouches spécialisés. Les unités sont celles du modèle, ou le millimètre papier pour les cartouches et étiquettes. Les positions et déplacements des poignées d’annotations sont partagés entre projet, familles modélisées et cartouches, avec une poignée de rayon pour les cercles. Dans le projet, la forme modifiée est prévisualisée et enregistrée en une opération au relâchement. Les annotations 2D sélectionnées peuvent également être déplacées ensemble dans le projet, avec aperçu et une seule opération d’historique. Les sélections mixtes du projet peuvent être déplacées depuis une annotation, avec une commande atomique ; les autres parcours d’édition restent à réunir.

- La sélection par fenêtre des familles modélisées inclut les formes 2D et les textes. Les raccourcis de sélection et de suppression fonctionnent aussi dans une famille autonome ; l’unification du parcours complet de sélection et d’édition avec le projet reste ouverte.

- Les annotations 2D d’une famille modélisée peuvent être déplacées par glissement, individuellement ou en groupe lorsqu’elles sont toutes dans la vue active. Les propriétés utilisent les unités du modèle ou celles du papier selon la catégorie ; l’aperçu des champs utilise la feuille ou la pièce associée lorsqu’elle existe. Les déplacements mixtes conservent les références en modifiant les décalages des volumes, symboles et extrusions simples ; les plans, contraintes et formes guidées sont refusés ensemble.

- Un profil peut désigner sa section parmi plusieurs polylignes fermées en plan ; un contour unique reste choisi automatiquement. La référence est enregistrée et contrôlée au chargement. Les autres contours restent dans la famille.

- Dans le projet, les annotations 2D d’une même vue peuvent être déplacées ensemble par glissement ; l’aperçu suit le geste et une seule commande permet l’annulation. Le chargement direct d’un fichier de famille alimente le registre correspondant à sa catégorie. Les fichiers spécialisés contenant un document commun s’ouvrent dans l’éditeur commun sans être écrasés.

- Le fichier de famille courant (`famille-ouverture 3`) conserve la profondeur du composant ou l’épaisseur d’essai de l’hôte ; cette valeur participe à l’historique et au chargement. Le format précédent est incompatible et aucune migration n’est ajoutée.
