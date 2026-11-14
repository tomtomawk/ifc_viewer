# BIMedit — revue de factorisation

Date : 11 novembre 2026. Revue du code local, incluant les évolutions de l'éditeur de familles de cette session.

## Conclusion

La priorité est de réunir **l'état des documents, le catalogue des outils et le cycle d'édition des familles**. Le projet possède déjà plusieurs bonnes abstractions : commandes métier sérialisables, historique générique, primitives de dessin, accrochage commun et séparation entre modèle et rendu. Il faut étendre ces points d'appui plutôt que remplacer l'architecture entière.

La séparation en fichiers est déjà importante, mais beaucoup de modules UI restent des extensions de `ViewerApp` et partagent son état global. Déplacer des méthodes dans de nouveaux fichiers ne suffira donc pas : les responsabilités et leurs données doivent être regroupées.

## Périmètre et méthode

- Inventaire de tous les fichiers Rust sous `crates/app/src`, puis lectures ciblées des documents, familles, outils, commandes, géométrie, rendu et collaboration.
- Vérification des appels et types par recherche textuelle ; pas de mesure de performance à l'exécution ni de calcul automatisé d'un taux de duplication.
- Les numéros de lignes ci-dessous sont des repères de cette version ; les noms de fonctions constituent les références durables.
- Les conclusions sont une revue structurelle, pas une certification de chaque module. Les différences nécessaires entre IFC, Revit, projet natif et familles ne sont pas assimilées à de la duplication inutile.

Inventaire relevé pendant la revue, commentaires et tests compris : **170 fichiers Rust, 81 585 lignes**. Ces nombres sont un instantané, pas une mesure de qualité.

| Ensemble | Fichiers | Lignes |
|---|---:|---:|
| Interface `ui` | 61 | 31 303 |
| Modèle `modele` | 58 | 24 681 |
| Formats | 24 | 13 581 |
| Rendu | 8 | 5 587 |
| Dessin | 3 | 2 456 |
| Pilote | 6 | 1 399 |
| Noyau | 5 | 1 256 |
| Configuration | 4 | 652 |
| `main.rs` | 1 | 670 |

Les gros fichiers de tests ne sont pas prioritaires uniquement à cause de leur taille. `gpu.rs` contient aussi des shaders : ses 2 580 lignes ne représentent pas toutes du code de contrôle Rust.

## Constats prioritaires

### P1 — Une seule gestion des documents et de leur état actif

**Preuves :** `crates/app/src/ui/mod.rs`, `DocumentOuvert` vers la ligne 421 et `ViewerApp` vers 496 ; `crates/app/src/ui/documents.rs`, `stocker_document` vers 482 et `changer_document` vers 532.

`DocumentOuvert` contient projet, historique, collaboration, scène et vues. Une partie de ces mêmes données est portée séparément dans `ViewerApp` pour le document actif. Le changement de document les transfère à la main avec de nombreux `take` et réinitialisations.

**Risque :** chaque nouvel état d'outil ou de vue doit être ajouté aux bons chemins de stockage, restauration et fermeture. Un oubli peut conserver un état du document précédent. Il s'agit d'un risque architectural observé, pas de l'affirmation qu'une fuite particulière a été reproduite.

**Proposition :** conserver tous les documents dans un registre, avec un seul identifiant actif. Introduire un accès explicite au document actif et séparer l'état global de l'application de l'état de document et de l'état de vue. Migrer d'abord la sélection et les vues, puis historique et collaboration ; ne pas changer simultanément le format de sauvegarde.

**Validation :** ouvrir deux projets et deux familles, alterner, annuler, fermer et réouvrir ; vérifier sélection, vue, état modifié et projet destinataire du chargement.

### P1 — Achever l'unification du cycle de vie des familles

**Preuves :** `crates/app/src/ui/editeur_famille/mod.rs`, `EditeurFamille` ; `crates/app/src/ui/familles_planes.rs:39`, `EditeurPlan` ; les champs `famille`, `familles_inactives`, `familles_planes`, `famille_active` et `famille_plane_active` dans `ViewerApp`.

Le choix de catégorie est commun visuellement, mais les documents 2D et 3D conservent des états et des parcours différents. L'historique des familles modélisées utilise `Historique<FichierFamille>` ; `EditeurPlan` compare son contenu à une référence et ne porte pas cette même pile.

**Proposition :** un `DocumentFamille` avec identité, catégorie, état modifié, historique, sélection, vues et document destinataire communs ; le contenu spécialisé reste un enum. Le rendu d'un profil et celui d'une porte peuvent rester différents. Partager ouverture, activation, fermeture, sauvegarde, chargement et historique avant de fusionner les outils de dessin.

**Évolution :** le sélecteur du document de famille commun conserve le contenu lorsqu’une famille non encore chargée change de catégorie ; une famille déjà chargée garde sa catégorie. Les anciens éditeurs spécialisés n’ont pas encore le même parcours. Ne pas réinitialiser silencieusement la famille.

**Validation :** mêmes scénarios de fermeture avec modifications et d'annulation pour porte, composant, profil, cartouche et étiquette ; absence de doublons d'onglets ; projet destinataire inchangé.

### P1 — Un catalogue unique des outils et de leurs capacités

**Preuves :** `OutilFamille` et `outil_disponible` dans `crates/app/src/ui/editeur_famille/mod.rs` ; listes de boutons et dispatch dans `panneau.rs` ; `formes_famille_planes` dans `crates/app/src/ui/ruban/barre_commandes.rs:68` ; ruban spécifique du cartouche dans `crates/app/src/ui/cartouche.rs`.

Les clés, noms, aides, icônes, règles d'activation et traitements sont répartis entre plusieurs listes. L'ajout d'une forme nécessite de modifier ces listes séparément, ce que les nouvelles révolutions et formes guidées rendent particulièrement visible.

**Proposition :** un descripteur par commande : identifiant typé, libellé, groupe, icône et capacités requises. Une fonction unique calcule une disponibilité accompagnée d'une raison ; le ruban et le contrôleur consultent tous deux ce résultat. Décrire séparément la catégorie et le contexte actif : famille plane, construction oblique, dessin en cours, sélection.

**Validation :** matrice catégorie × outil × plan ; un outil grisé ne doit pas être activable par le pilote ou un raccourci. Tester également la désactivation d'un outil déjà actif lors d'un changement de contexte.

### P1 — Nommer et typer les opérations de génération de solides

**Preuves :** `crates/app/src/modele/famille/modelee.rs:215`, `Extrusion`, avec `revolution: Option<Revolution>` et `guidage: Option<FormeGuidee>` ; la collection reste nommée `extrusions`, y compris pour les raccordements.

Ce choix préserve la compatibilité immédiate, mais autorise structurellement des combinaisons incompatibles que `valider` doit refuser. Le nom du type ne représente plus ce qu'il contient.

**Proposition :** une `FormeProfil` contenant les attributs communs — nom, profil, repère, matériau, visibilité, plein/vide — et une `OperationForme` exclusive : extrusion, révolution, extrusion par chemin, raccordement, raccordement par chemin. Définir un schéma JSON explicite pour cette opération et ses données, puis documenter le changement de format. Conformément à la politique de développement, les anciennes familles seront refaites : aucun lecteur de migration n’est nécessaire.

**Validation :** aller-retour de chaque opération avec le schéma courant ; conservation des paramètres, couleurs et vides ; rejet des états incompatibles à la désérialisation.

### P1 — Un moteur de saisie d'esquisse indépendant de la projection

**Preuves :** `crates/app/src/ui/editeur_famille/canevas.rs`, `canevas_construction` et `canevas` ; `crates/app/src/ui/cartouche.rs` ; `crates/app/src/ui/dessin_plan` ; `crates/app/src/dessin/dao.rs` ; `crates/app/src/ui/accrochage.rs`.

Le canevas oblique et le canevas orthogonal gèrent encore séparément clics, fermeture du profil, sélection et dessin temporaire. Le canevas oblique dispose de moins d'accrochages et de poignées que le canevas principal. Les cartouches exploitent déjà `Trace` du module DAO.

**Amélioration effectuée dans cette session :** `crates/app/src/ui/editeur_famille/formes.rs::terminer_forme` centralise la validation et la création des formes pour les deux canevas. Une erreur de validation conserve désormais le tracé à corriger.

**Suite proposée :** un état d'esquisse et des événements communs (point, terminer, annuler, sélectionner) ; des adaptateurs fournissent projection, points d'accrochage et résultat métier. Conserver un contrôleur de cartouche spécialisé pour textes et libellés, mais réutiliser le tracé et la sélection de géométrie.

**Validation :** mêmes séquences de clics dans plan orthogonal, plan oblique et cartouche ; Échap, fermeture au premier point, clic droit, reprise de tracé, annulation et poignées.

### P2 — Des plans de construction référencés, pas copiés

**Preuves :** `PlanConstruction` dans `modelee.rs:180`, liste `plans_construction` dans la famille et copie optionnelle du repère dans chaque forme. Le sélecteur de `panneau.rs` transforme un plan de référence résolu en repère de construction.

Les coordonnées sont conservées correctement, mais une copie de repère n'exprime pas une dépendance paramétrique. Déplacer plus tard le plan de référence ne signifie pas automatiquement déplacer les formes créées sur sa copie.

**Proposition :** un identifiant stable de plan et un résolveur de repères. Distinguer un plan indépendant d'un plan dérivé d'une référence paramétrique. Lors de la suppression d'un plan utilisé, refuser ou demander explicitement de détacher ses formes en conservant leur position.

**Validation :** déplacement et renommage du plan, changement de paramètre, suppression, enregistrement, rechargement, puis comparaison de la position des solides.

### P2 — Factoriser les primitives géométriques avec un contrat explicite

**Preuves :** `crates/app/src/modele/volumes/solides.rs:85` (`prisme`), `crates/app/src/noyau/trace2d.rs:255` (`trianguler`), `crates/app/src/noyau/texte.rs:195`, `crates/app/src/modele/elements/terre_plein.rs`, et les modules `modelee/revolution.rs` et `modelee/guidage.rs`.

Plusieurs adaptateurs préparent des coordonnées pour Earcut, orientent les triangles et traitent les erreurs. Le chemin `trace2d` passe par la triangulation de texte en `f32`, alors que d'autres calculs utilisent `f64`. Les besoins ne sont pas tous identiques : contours avec trous, texte, terrain et solides fermés demandent des contrats distincts.

**Proposition :** placer les primitives géométriques pures dans le noyau : triangulation plane en `f64`, orientation, contrôle de dégénérescence, transformation de repère et assemblage de sections. Garder des adaptateurs explicites pour trous et texte. Éviter une fonction universelle qui masquerait les conversions d'unités ou de précision.

**Amélioration effectuée :** `modelee/tests_maillage.rs` fournit un contrôle commun de fermeture et d'orientation des arêtes pour révolution et guidage.

**Validation :** contours concaves, sens inversé, trous lorsque supportés, très petites dimensions, grands décalages, volumes signés et maillages fermés. Ne pas confondre fermeture topologique et absence d'auto-intersection.

### P2 — Étendre le point d'entrée métier commun aux familles

**Preuves :** `crates/app/src/modele/edition/commandes/mod.rs:13`, enum `Commande` ; `crates/app/src/ui/pilotage.rs`, `crates/app/src/ui/commandes_pilote.rs`, `crates/app/src/ui/transformer.rs` ; mutations directes de la famille dans `panneau.rs` et `canevas.rs`.

Les commandes métier sérialisables sont une base solide pour aligner interface et pilote. Les familles gardent beaucoup de modifications directes de vecteurs, pendant que le pilote possède son propre dispatch textuel. Il serait excessif d'affirmer que toute la logique métier est dupliquée ; le problème est l'absence d'un parcours commun complet pour les nouvelles opérations de famille.

**Proposition :** des commandes de famille validées au niveau modèle, consommées par l'UI et le pilote. Conserver les états transitoires d'un glissement dans l'UI, puis émettre une transaction à sa fin. Le pilote doit pouvoir créer une forme guidée sans reproduire une séquence de clics.

**Validation :** même résultat sérialisé pour une commande venant de l'UI ou du pilote, mêmes erreurs, même entrée d'historique et même résultat après annulation.

### P2 — Partager le résultat de résolution et de génération pendant une image

**Preuves :** appels à `resoudre` dans les panneaux et canevas des familles ; `guidage.triangles` utilisé pour le diagnostic de Propriétés et pour `FamilleModelee::solides`. Le générateur guidé revalide notamment des sections intermédiaires.

**Risque :** refaire les mêmes calculs pour les propriétés, le dessin et le diagnostic. Aucun ralentissement chiffré n'a été mesuré dans cette revue.

**Proposition :** un résultat résolu par révision de famille et valeurs d'essai, contenant paramètres résolus, diagnostics et maillages. Les panneaux lisent ce résultat ; la mutation invalide sa révision. Mesurer avant d'introduire un cache global ou une synchronisation complexe.

**Validation :** même géométrie et mêmes diagnostics avec ou sans cache ; invalidation après déplacement d'un point, formule, changement de type, annulation et rechargement.

### P2 — Distinguer ressources GPU, passes et shaders

**Preuve :** `crates/app/src/rendu/gpu.rs` mêle chaînes WGSL, uniformes, ressources et orchestration ; `COMMUN` débute vers la ligne 18. Le schéma d'uniformes doit rester cohérent entre Rust et WGSL.

**Proposition :** extraire d'abord les shaders dans des fichiers WGSL inclus à la compilation ; séparer ensuite ressources partagées, passes et orchestration. Préserver un propriétaire clair des ressources wgpu et des mises à jour. La division en fichiers n'apporte pas, à elle seule, de gain de performance.

**Validation :** compilation des shaders, disposition des uniformes et images de référence pour les styles principaux. Cette revue n'a pas effectué de comparaison visuelle GPU.

### P2 — Formaliser les métadonnées des collections du projet

**Preuves :** `crates/app/src/modele/mod.rs`, `Projet` ; `crates/app/src/modele/collaboration.rs:42`, `objets`, et vers 92, `collection_identifiee`. La collaboration reconstruit des collections et dépendances à partir du modèle et des identifiants.

**Proposition :** documenter dans un registre les collections, leur identité, leurs références et leur stratégie de comparaison. Commencer par tester l'exhaustivité du registre vis-à-vis du projet sérialisé. Ne pas remplacer les règles spécifiques de concurrence par une simple réflexion JSON générique.

**Validation :** l'ajout d'une collection doit provoquer un échec de test si sa politique collaborative manque ; tester aller-retour et dépendances. Les nombreux tests de collaboration existants sont à conserver.

## Ordre conseillé des travaux

| Lot | Livrable | Critère de fin |
|---|---|---|
| 1 | Catalogue des commandes et capacités | Un seul lieu décrit une forme et les catégories compatibles |
| 2 | Document de famille et historique communs | Même activation, fermeture et annulation en 2D et 3D |
| 3 | Enum exclusive des opérations et JSON explicite | Chaque opération est décrite et relue ; états incompatibles impossibles |
| 4 | Plans identifiés et résolus | Une forme suit son plan sans duplication de coordonnées |
| 5 | Contrôleur d'esquisse partagé | Parité de sélection, accrochage et Échap dans les deux canevas |
| 6 | Registre des documents de l'application | Fin des transferts manuels de champs du document actif |
| 7 | Primitives de maillage, rendu et collections | Contrats explicites et tests de non-régression dédiés |

Chaque lot doit rester relisible et testable séparément. Éviter de combiner changement du format JSON, refonte de l’historique et changement de moteur de rendu dans un même chantier.

## Avancement de la factorisation — 12 novembre 2026

- Lot 1, partiel : le catalogue de l'éditeur de famille modélisée alimente son ruban et les formes grisées des familles planes. La disponibilité tient compte de la vue et du plan oblique. Les outils spécifiques des cartouches et le contrôle complet par pilote restent à unifier.
- Lot 2, partiel : profils, cartouches et étiquettes utilisent la même pile générique d'instantanés que les familles modélisées ; les gestes sont regroupés et Annuler/Rétablir sont accessibles par la barre et les raccourcis. Le parcours commun crée ou ouvre désormais les familles de fichier en mode autonome, même si un projet est ouvert ; elles restent ouvertes à la fermeture du projet. Une famille issue d’un projet reste liée à ce projet et le chargement y associe une famille autonome. Les éditeurs 2D et 3D partagent `DocumentFamille` pour identité, rattachement, chemin et historique ; leurs contenus et états d’interaction restent spécialisés. Le registre et les accès aux éditeurs restent distincts. Le chargement d'une famille plane dans le projet réinitialise sa pile afin de ne pas restaurer une ancienne identité de destination.
- Lot 7, partiel : les shaders WGSL sont extraits et validés ; la triangulation plane et l'adaptateur de texte partagent une primitive en double précision ; un registre conserve les politiques collaboratives existantes et un test vérifie la couverture des collections de `Projet`.
- Lot 3, implémenté, validation reportée : `FormeProfil` porte une opération exclusive sérialisée ; le gabarit livré a été mis à jour. Les anciens fichiers ne sont pas migrés. La collection conserve le nom `extrusions` dans le JSON.
- Lot 4, partiel : les formes désignent les plans de construction par identifiant. Les plans issus d'une référence sont résolus selon la position courante ; le renommage et la suppression de la référence sont pris en compte. La gestion complète de suppression et de détachement des plans de construction reste à concevoir et à vérifier.
- Lot 5, partiel : Échap et la fermeture d’un profil sont communs aux deux canevas. Les profils sur plan oblique disposent de poignées de déplacement, d’insertion et de suppression, et s’accrochent à l’origine et aux sommets du plan. Le contrôleur, les règles d’accrochage et la sélection ne sont pas encore entièrement unifiés entre les deux canevas.
- Lot 6, partiel : les sélections multiples et BIM sont conservées par document lors du changement. Le registre intégral et la suppression des transferts manuels restent à réaliser.
- La campagne de tests et la recette visuelle des derniers changements sont reportées à la demande de l'utilisateur. Les primitives de maillage, les passes GPU et les dépendances collaboratives ne sont pas encore entièrement factorisées.

## État des fonctions ajoutées et limites concrètes

- Révolution : axe par deux points, angle de départ, angle de rotation, plein/vide ; discrétisation angulaire de 5° maximum.
- Extrusion par chemin : un profil fermé et un chemin polygonal ouvert en coordonnées locales H/V/D.
- Raccordement : deux profils de même nombre de sommets, correspondance par indice, distance entre profils.
- Raccordement par chemin : interpolation des profils selon la longueur parcourue et transport du repère de section le long du chemin.
- L'utilisateur dessine le profil initial ; le chemin et le profil d'arrivée se règlent actuellement numériquement dans Propriétés. Le chemin commence droit et le profil final est initialisé par copie. Les nouvelles formes sont grisées dans les familles planes.
- Les chemins courbes analytiques, chemins fermés, profils de nombres de sommets différents, correspondances automatiques, paramètres/formules du second profil et dessin graphique du chemin restent à développer.
- Le transport des sections aux changements de direction est une approximation polygonale ; ce n'est pas un raccord tangentiel ni un calcul exact de coupe d'onglet. La validation contrôle des dégénérescences et des sections intermédiaires échantillonnées, mais ne prouve pas l'absence de toute auto-intersection du solide.
- Les tests de géométrie ne remplacent pas la recette visuelle de l'éditeur. Aucune vérification visuelle de cette session n'est revendiquée.

## Vérifications et commit

Les tests ciblés couvrent les anciennes familles, les profils, les révolutions, les guidages, les volumes attendus, le sens des faces, les arêtes fermées et plusieurs saisies invalides.

Résultats de la version finale :

- `cargo check` : réussi.
- `cargo test modele::famille` : 20 réussis, 1 ignoré (génération du gabarit).
- `cargo test ui::ruban -- --nocapture` : 10 réussis, dont le contrôle de disposition aux largeurs de 1 000, 1 280 et 1 500 pixels.
- `git diff --check` : réussi.
- La campagne globale de 368 tests n'est **pas validée**. Elle a révélé un débordement des commandes de cartouche, corrigé puis revérifié par les 10 tests du ruban. Plusieurs tests de collaboration ont échoué ; un cas isolé, `partager_ouvrir_deux_copies_et_reutiliser_sans_ecraser`, échoue sur `Accès refusé. (os error 5)` à la création du dépôt. Les lectures de gros fichiers Revit encore en cours ont été interrompues pour libérer l'exécutable et recompiler les tests ciblés après correction. Aucun résultat global positif n'est revendiqué.

Le diagnostic Git a constaté : aucun `index.lock` résiduel ; le groupe `CodexSandboxUsers` n'a que `ReadAndExecute, Synchronize` sur `.git`. Le refus d'écriture vient donc des droits de l'environnement et non d'un fichier verrou oublié. Aucune permission n'a été modifiée pour contourner cette restriction.

La nouvelle tentative `git add` sur la liste explicite des fichiers de cette session échoue encore en créant `.git/index.lock` (`Permission denied`). Aucun commit n'a pu être créé dans cet environnement. Les fichiers d'essai personnels sous `samples` n'ont pas été inclus dans cette tentative.
