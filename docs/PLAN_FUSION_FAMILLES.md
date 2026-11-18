# Plan de fusion des familles

## Objectif

Une famille possède un document, un éditeur et un format communs, quelle que soit sa catégorie. Elle peut contenir de la géométrie 3D et des éléments 2D dans ses différentes vues. Pour les catégories cartouche et étiquette, seul le contenu 2D de la vue en plan est utilisé pour le rendu dans le projet. Pour un profil, la vue en plan fournit la section exploitable par les éléments qui utilisent ce profil. Les données des autres vues et les formes 3D restent enregistrées dans la famille.

Le logiciel n'étant pas en production, les anciennes familles seront refaites : aucune migration des anciens formats n'est prévue. Les changements de schéma doivent être documentés conformément à `docs/AGENTS.md`.

## Outils communs au projet et aux familles

Centraliser chaque outil et son parcours graphique pour tous les contextes : catalogue, disponibilité, bandeau, saisie, aperçu, accrochage, sélection, poignées, propriétés et validation. Un même contrôleur produit une opération appliquée par un adaptateur au document actif ; le contexte fournit la projection, les unités, les données et les possibilités réellement disponibles. Les améliorations sont partagées dans les deux sens entre le projet, les familles et les documents.

- **Polyligne** : remplacer les parcours concurrents par celui des vues de projet, enrichi au besoin par les autres ; fermeture et remplissage restent des propriétés de la polyligne.
- **Arc** : dans les vues de projet, l'arc est un segment de la polyligne (Alt+clic sur sa fin, puis point de passage ou rayon ; `courbures` par segment) et l'outil Arc est retiré. Le moteur commun (`crates/moteur2d/src/dao.rs`, `Trace::arcs_permis`) le permet déjà ; l'éditeur de familles garde pour l'instant son outil Arc 2D. Les éléments 2D de famille (liste unique, ancrage facultatif aux plans) portent déjà `courbures` ; régions et sections utilisent le contour développé. Reste : activer les arcs dans la polyligne de famille, puis retirer l'outil Arc 2D.
- **Texte et libellés** : comparer et réunir le placement graphique, la sélection et le déplacement avec poignées, la modification du contenu, de la position et de la hauteur, ainsi que les champs dynamiques et leur aperçu. Permettre au projet de bénéficier des fonctions pertinentes actuellement réservées aux familles ; chaque contexte fournit ses propres champs et unités.
- **Tous les autres outils** : inventorier et reprendre leurs fonctions utiles avant de supprimer les variantes, notamment lignes, arcs, cercles, splines, rectangles et cotations. Ne pas confondre les mêmes gestes graphiques avec les règles métier propres à chaque destination.

## Navigation des vues de famille

Aligner le fonctionnement des vues de famille sur celui des vues de projet :

- Corriger le rendu de la vue 3D et vérifier les formes visibles, le cadrage et les interactions sans projet ouvert.
- Ouvrir et identifier les vues dans les onglets avec leur nom, comme dans le projet, tout en conservant leur association au document de famille.
- Retirer le nom de vue et le bouton « Cadrer » du haut du canevas ; utiliser les mêmes commandes et la même disposition que dans le projet.
- Cadrer par double-clic sur la molette dans les vues planes et 3D, et aligner zoom, déplacement et rotation sur les gestes du projet.
- Réserver le mur hôte aux catégories hébergées par un mur ; vérifier son affichage dans toutes les vues concernées.
- Un seul moteur d'accrochage et de repérage (`interface/src/saisie/moteur_accrochage.rs`) pour le projet, les familles et les plans de construction : chaque contexte fournit une `SourceAccrochage` (tracés et droites d'accrochage dans son repère 2D : murs et éléments de la vue ; plans de référence, contours, éléments 2D de la famille) ; types de points, suivi polaire, guides, marqueurs, délai des options et saisie de distance sont communs.
- Les vues de famille (plan, élévation, coupe, 3D) passent par le canevas des vues de projet : la famille est convertie en scène (`editeur_famille/rendu.rs` : solides en faces et arêtes comme les éléments du projet ; éléments 2D, cotes de mesure et mur générique en primitives maillées par `moteur2d::sommets` comme les annotations) et rendue par le `Renderer` du projet avec ses paramètres d'affichage (`frame_params` : fond `moteur3d::fond::fond_vue`, style visuel, traits, éclairage). Chaque vue plane a une `Camera` orthographique orientée sur la vue ; gestes (panoramique, molette de `socle::projection::zoom_molette`, `Camera::fit`) et conversion monde → écran (`socle::projection::ecran`, aussi utilisée par `ecran_plan` du projet) sont communs. Ne restent dessinés par egui que les éléments propres à l'éditeur, en superposition comme les poignées et accrochages du projet : plans de référence, contraintes et égalités, contours des vides, poignées, aperçus d'outil, dessin face à un plan de construction.

## Lots proposés

1. **Modèle de famille unique.** Introduire une catégorie explicite et un document commun regroupant paramètres, vues, formes 3D, éléments 2D et propriétés nécessaires à chaque catégorie. Définir un format JSON unique et valider les données avant le chargement. Les anciens modèles spécialisés restent en place temporairement pendant la construction du nouveau parcours, puis sont retirés.
2. **Dessin et annotations 2D.** Reprendre le parcours de polyligne des vues de projet, y compris le bandeau de dessin, la saisie, l'accrochage, la fermeture, la sélection, les poignées et les propriétés. Une polyligne fermée porte les propriétés de remplissage ; aucun outil distinct « région » ou « zone fermée » n'est nécessaire. Étendre les éléments par vue aux arcs, cercles, splines, rectangles, textes, libellés et cotes graphiques. Distinguer les cotes graphiques des contraintes qui pilotent la famille.
   L'onglet Modifier de l'éditeur reprend les fonctions du projet (`docs/MODIFICATION.md`) avec le même parcours graphique : Déplacer, Copier, Tourner, Symétrie, Échelle, Réseau (linéaire et polaire) et Aligner, appliqués aux volumes, formes et éléments 2D dans les limites du modèle de famille (boîte alignée sur ses plans, forme liée à sa vue d'esquisse). Un plan de construction utilisé par des formes ne se supprime pas ; il se détache en repère fixe équivalent.
   Réutiliser autant que possible le canevas et la configuration des vues de projet dans l'éditeur de famille : navigation, fond, curseur, repères, affichage, bandeau et outils communs. Prévoir des adaptateurs de contexte pour la géométrie et les opérations propres aux familles, sans dépendre d'un projet ouvert.
3. **Éditeur commun et catégories.** Utiliser le même document, les mêmes vues et le même registre pour toutes les catégories. Rendre disponibles les outils appropriés sans supprimer la géométrie 3D ou les autres vues. Reprendre les propriétés de cartouche et l'édition paramétrique des profils.
4. **Cartouches et étiquettes.** Adapter le chargement et le rendu pour exploiter les seuls éléments 2D de la vue en plan. Conserver le repère et les unités papier, les dimensions du cartouche, les champs dynamiques, leur aperçu, les affectations aux feuilles et aux pièces, et l'export. Remplacer l'actuelle « face imprimée », liée à l'élévation, par le plan dans le nouveau modèle.
5. **Profils.** Définir comment désigner un contour fermé de la vue en plan comme section ; conserver paramètres, formules, validation et dépendances des éléments qui utilisent le profil. Ne pas assimiler les autres éléments 2D du plan au contour de section.
6. **Chargement et nettoyage.** Conserver le parcours autonome et l'association explicite au projet destinataire, puis retirer les éditeurs, structures et formats devenus redondants. Vérifier les occurrences et les mises à jour après rechargement, ainsi que l'ouverture, la fermeture et l'historique.
7. **Validation ultérieure.** Compiler à chaque lot ; lorsque la campagne de tests sera demandée, vérifier les outils et leurs poignées, les JSON courants, les paramètres, la résolution des champs, les exports, les profils et la règle de rendu du plan. Compléter par une recette visuelle avec plusieurs projets et familles.

Chaque lot doit rester relisible. Le passage d'un lot au suivant ne doit pas masquer les fonctionnalités encore prises en charge par les anciens éditeurs.

## État

L'avancement et le reste à faire sont dans [SUIVI.md](SUIVI.md) §2.5. Le relevé détaillé du 11 au 13 novembre 2026 est archivé dans [historique/FAMILLES_AVANCEMENT_2026-11.md](historique/FAMILLES_AVANCEMENT_2026-11.md).
