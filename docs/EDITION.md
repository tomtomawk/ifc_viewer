# Éditeur BIM natif

Le document `.modele.json` reste la source de vérité. Les ajouts de cette itération
utilisent le format v0. Aucun fichier RVT n'est écrit. Pendant l'alpha, le genre
d'ouverture libre est `libre` ; l'ancien nom `baie` n'est plus accepté.

## Parcours disponible

- **Familles → Profils** : ouvrir un profil du projet, en créer un ou charger un
  fichier de profil. L'éditeur permet de modifier les paramètres et les formules
  des coordonnées des sommets, avec un aperçu du contour fermé. Enregistrer dans
  le projet, créer une copie ou exporter un fichier `.profil.json`. Le gabarit
  embarqué fournit un rectangle, une cornière, un profilé T et un profilé I.
  Ces familles de profils sont éditables indépendamment ; leur utilisation pour
  créer des éléments par balayage ou définir une section de poutre reste à ajouter.

- **Début → Phases et variantes**, ou le lien sous les champs de contexte dans
  Propriétés, ouvre le gestionnaire des phases, sous-projets et variantes. Il permet
  d'ajouter et renommer les entrées, de réordonner les phases et de renommer un jeu
  de variantes. Une suppression réaffecte les éléments au choix indiqué ; une
  variante peut être réaffectée au modèle principal. L'ordre ne peut pas placer une
  démolition avant la création de l'élément. Les filtres de la rubrique **Affichage
  temporaire** permettent d'inspecter une phase, un sous-projet ou une variante ; ils
  s'appliquent aux vues pendant la session et ne modifient pas les affectations.
  Sans élément sélectionné, les propriétés de chaque vue proposent aussi **Phase
  affichée** et **Variante affichée** : ces choix sont enregistrés avec la vue et
  repris sur les feuilles. Les filtres temporaires du gestionnaire s'y ajoutent.

- **Gabarit de vue** : dans les propriétés de la vue, le sélecteur attribue un
  gabarit existant et **+** en crée un à partir de la vue. Le menu **…** permet
  de modifier ou supprimer les gabarits du projet, même sans affectation. Un
  gabarit partagé pilote l'échelle, le poché de coupe, la phase et la variante ;
  ses modifications se répercutent immédiatement sur toutes les vues liées ;
  leurs dessins sur les feuilles reprennent notamment le poché et les filtres.
  L'échelle d'une vue placée sur une feuille reste réglable sur la feuille.
  Le cadrage, l'orientation et les masquages
  restent propres à chaque vue. Retirer ou supprimer le gabarit laisse aux vues
  leurs derniers réglages ; ceux-ci redeviennent modifiables individuellement.

- Dans un **plan natif**, sélectionner des éléments puis **Modifier → Masquer**
  (ou H) les masque uniquement dans cette vue ; **Tout afficher** les rétablit.
  Les masquages sont enregistrés avec la vue et repris sur les feuilles.
- **Cadrer** dans un plan natif permet de tracer une polyligne fermée, comme pour
  une dalle : seule la partie intérieure reste visible, sur le plan et sur les
  feuilles. **Retirer le cadrage** rétablit la vue entière ; F ajuste toujours le
  zoom. La **zone de coupe** est une boîte disponible dans la vue 3D, réglable par
  ses poignées ou dans les propriétés ; elle ne recadre pas les vues 2D.

- **Modifier → Modifier tracé** permet de sélectionner un trait et de le supprimer
  avec Suppr. Le contour s'ouvre : cliquer une extrémité, tracer le raccord puis
  cliquer l'autre extrémité pour le refermer. Une dalle ne peut être validée que si
  tous ses contours sont fermés. Les accrochages et la saisie près du curseur restent
  disponibles pendant l'édition.

- **Début → Importer** convertit un IFC/RVT/RFA/RTE dans un nouveau document natif,
  conserve les documents ouverts et écrit le rapport `.rapport.md` à côté du JSON.
  Les murs courbes, ouvertures et maillages importés sont maintenant affichables.
- **Saisie validée** : un nom ou une valeur tapé dans un champ (propriétés, niveaux,
  matériaux, feuilles…) ou un nombre glissé n'est appliqué au modèle qu'à la fin de la
  saisie (Entrée, clic ailleurs, fin du glisser) : la scène est alors reconstruite et
  la modification forme une seule étape d'historique. Échap abandonne la saisie et
  rétablit l'ancienne valeur (sans quitter l'outil en cours).
- **Modéliser → Niveau** liste les niveaux (nom et altitude modifiables) ;
  « Ajouter un niveau » crée « Niveau n » au-dessus du plus haut, à la hauteur du
  dernier étage (3 m s'il n'y a qu'un niveau). **Quadrillage** crée des axes
  droits, visibles dans les plans natifs. Les dimensions du modèle sont saisies en
  centimètres par défaut ; Début → Options permet de choisir mm, cm ou m. Le JSON,
  les commandes CLI/MCP et les tailles sur papier restent en millimètres.
- **Modéliser → Poteau / Poutre** crée des structures rectangulaires en saisissant
  leurs dimensions et leur niveau. Placez un poteau par un clic dans la vue ;
  pour une poutre, cliquez son départ puis son arrivée. Le placement utilise les
  accrochages et Maj contraint la poutre aux axes du plan ; Échap ou clic droit
  annule un départ. La saisie par coordonnées reste disponible. Les poteaux sont
  verticaux et orientables ; les poutres sont horizontales et définies par leur axe.
  Après sélection, **Modifier → Propriétés structurelles** permet de les modifier ;
  les commandes de déplacement, rotation, copie, suppression et l'historique leur
  sont applicables. Leurs quantités et classes `IfcColumn` / `IfcBeam` sont exportées.
- **Modéliser → Terrain** : choisir le type (épaisseur, matériau), tracer un périmètre
  fermé et le fermer par clic droit, F ou en cliquant le premier sommet. Les cotes sont
  **NGF**, dans l'unité du projet, et se règlent à côté de chaque sommet (S1, S2…) et de
  chaque point coté. Glisser un sommet ou un point le déplace ; glisser le milieu d'un
  côté insère un sommet ; clic droit retire un point (ou un sommet, trois au moins).
  « Placer des points cotés » ajoute un point à chaque clic, à la cote indiquée. L'aperçu
  se teinte selon la cote, avec ses courbes de niveau. Entrée ou Valider crée le terrain :
  un solide de l'épaisseur du type, triangulé (Delaunay contraint par le contour), à
  l'apparence lissée et sans arête. **Modifier → Modifier tracé** reprend un terrain
  sélectionné dans le même outil. Propriétés : type (Dupliquer pour en créer un),
  épaisseur et matériau du type, cotes extrêmes, surface en plan.
- **Origine et cote NGF** : **Début → Coordonnées**, ou un clic sur l'origine (trait
  invisible d'une vue), règle X, Y et la cote NGF de l'origine du projet (niveau 0).
  Le terrain suit ses cotes NGF : changer celle de l'origine remonte ou descend tout le
  projet par rapport à lui (`editer {"action":"coordonnees_origine","coordonnees":[x,y,z]}`).
- **Modéliser → Limite de propriété** : tracer un ou plusieurs contours fermés par
  polygone, rectangle ou cercle, comme pour les éléments horizontaux, puis valider.
  Chaque contour crée une limite indépendante sur le plan du niveau courant. Le terrain est
  exporté en `IfcGeographicElement` et la limite en courbe `IfcAnnotation`. Terrains et
  limites sont créés dans la phase **Existant**, présente dans tout nouveau projet (et
  ajoutée au besoin).
- **Types de murs** : le sélecteur de l'outil Mur est organisé comme celui des éléments
  horizontaux, avec trois onglets — Mur, Mur composé, Cloison. La catégorie est un
  paramètre du type (`categorie` : `mur`, `compose`, `cloison`), choisi dans l'éditeur de
  type ; absent, il suit le nombre de couches (une : mur, plusieurs : mur composé). Les
  types du gabarit non encore dans le projet figurent dans leur onglet.
- **Quitter une commande** : une barre en haut à droite de la vue porte les boutons
  Valider (✔, Entrée) et Annuler (✖, Échap). Valider termine le tracé en cours et ferme
  l'outil ; Annuler abandonne tout et le ferme.
- **Murs** : créés par défaut sur la face extérieure, en chaîne (le mur suivant part de
  la fin du précédent). Espace inverse le sens du mur en cours de tracé (extérieur et
  intérieur échangés).
- **Éléments horizontaux** : un polygone se ferme par F, clic sur le premier sommet ou
  clic droit ; Entrée ne le ferme plus, elle valide la commande. Un nouveau tracé
  continue le même élément : un contour extérieur ajoute une partie, un contour
  intérieur à un autre est une trémie. L'élément est créé à la validation.
  Pour les éléments horizontaux et les limites de propriété, **Sélectionner** dans le
  bandeau, l'onglet **Modifier** ou Échap interrompt le tracé et permet de choisir un
  sommet ou un segment. **Déplacer** utilise un point de départ et un point d'arrivée ;
  **Décaler** déplace le segment sélectionné perpendiculairement, jusqu'au clic, en
  gardant ses extrémités reliées aux segments voisins. Un contour ouvert se reprend en
  double-cliquant une extrémité ; tous les contours doivent être fermés pour valider.
  La sélection et les commandes Déplacer / Décaler sont également disponibles pour le
  terrain, le cadrage de vue, la ligne d'escalier, le garde-corps et la polyligne
  d'annotation en cours de tracé. Pour le terrain, les altitudes restent associées aux
  sommets déplacés. Pour les lignes d'escalier et de garde-corps, le nombre de sommets
  est conservé pendant cette modification.
- **Ouverture** place une porte, une fenêtre ou une ouverture libre par clic sur un mur ou par
  abscisse depuis son départ. Une ouverture d'assemblage perce toutes ses couches. Une
  menuiserie importée conserve son maillage et suit son hôte ; ses dimensions sont
  verrouillées tant qu'elle n'est pas remplacée explicitement par un cadre natif.
- **Sélection** : sélectionner un élément (sans outil en cours) ouvre l'onglet
  Modifier ; tout désélectionner (clic dans le vide, Échap) rend l'onglet d'avant.
  Le panneau Propriétés commence par le sélecteur de type de l'élément : mur composé
  (couches reconstruites d'après le type choisi ; une couche sélectionnée montre le
  type de son mur), mur seul (types à une couche), élément horizontal, porte ou
  fenêtre (dimensions du type).
- **Modifier** propose déplacement, copie, rotation, alignement des axes et
  ajustement de l'extrémité d'un mur sur l'axe d'un autre. Ctrl+clic constitue une
  sélection multiple, surlignée dans la vue. On peut aussi
  tracer un rectangle en maintenant le bouton gauche et en glissant depuis le vide
  (un clic bref désélectionne seulement) : de
  gauche à droite (bleu), seuls les éléments entièrement inclus sont retenus ; de
  droite à gauche (vert), tous les éléments touchés sont retenus. Ctrl ajoute à la
  sélection ; Échap ou clic droit annule le rectangle. Les objets visibles à travers
  d'autres sont inclus, tandis que les éléments masqués et les parties découpées
  sont exclus. Le glissement sur un objet conserve la navigation 3D. Copier un assemblage recrée les
  identifiants de ses couches et ouvertures. Les repères de coupe/élévation et les
  quadrillages se sélectionnent aussi en plan, ainsi que les annotations dans leur
  vue ; leurs propriétés sont accessibles dans Modifier. Les cotes liées gardent
  leurs références : leur décalage est éditable dans leurs propriétés. Supprimer (icône de
  Modifier, touche Suppr ou Retour arrière hors tracé) agit tout
  de suite, couches et ouvertures hébergées comprises ; Ctrl+Z rétablit. Un niveau
  encore utilisé ne peut pas être supprimé.
- Le panneau Propriétés ne montre que les informations de la sélection (type,
  paramètres, couches, sommets d'un élément horizontal). Les commandes sont ailleurs :
  Cadrer, Isoler et Masquer dans le groupe Visibilité de Modifier (touches F, I, H) ;
  dans la barre d'options en haut de la vue, les réglages de Déplacer, Copier,
  Tourner, Aligner et Ajuster, et les commandes propres à la sélection : dissocier
  une couche ou prendre son assemblage, associer un mur à un assemblage, jonctions
  et haut des murs (attache à une toiture, priorité d'une couche aux croisements),
  mise à jour depuis le type, trémie ou contour rectangulaire d'un élément
  horizontal. Modifier un contour réinitialise ses pentes par arête.
- **Raccords des murs droits** : l'onglet se forme quand les extrémités des lignes
  de contrôle se touchent, y compris avec une implantation sur une face et un angle
  aigu. Une tolérance de 0,01 mm absorbe seulement l'arrondi des coordonnées ;
  l'épaisseur ne sert plus de distance d'activation. Une arrivée en T demande le
  contact de l'extrémité avec la ligne de contrôle du mur rejoint. Des points séparés
  n'ajoutent aucun volume de raccord. La fusion des volumes réellement superposés
  reste active et les axes enregistrés ne sont pas déplacés.
- **Propriétés des murs** : le sélecteur de type est commun à la création et à
  la sélection, avec les mêmes catégories, aperçu et types du gabarit. Le nom est
  affiché parmi les propriétés, sous les contraintes de hauteur. La longueur suit
  la ligne d'implantation ; le volume est calculé sur le solide réel, raccords,
  intersections et ouvertures compris.
- **Trajectoires des murs** : la barre propose Sélection, Ligne, Rectangle, Arc
  et Cercle. Sélection reprend un trait existant de la vue en plan avec un aperçu ;
  les arcs restent courbes et le dessin d'origine est conservé. Un cercle sélectionné
  demande ensuite les deux limites du périmètre. Le mode Cercle utilise quatre clics :
  centre, rayon, début et fin du périmètre (sens antihoraire ; recliquer le début pour
  un cercle complet). Le rayon est un guide pointillé, sans volume de mur.
  Les murs courbes sélectionnés montrent leur ligne d'implantation courbe et trois
  poignées : début, point de passage et fin. Les glisser modifie la trajectoire avec
  accrochages, conserve les couches et se rétablit avec Ctrl+Z ; Échap annule le glissé.
- **Thème de l'interface** : le bouton ◐ près de Réduire choisit Clair, Sombre ou
  Système. Le choix est enregistré sur le poste ; Système suit le thème du système.
- **Plage de vue en plan** : les propriétés distinguent Plan de coupe, Limite
  basse et Limite haute (niveau et décalage). Entre le plan de coupe et la limite
  haute, les objets non coupés du niveau de la vue sont représentés en pointillés,
  notamment les poutres et plafonds. Les objets rattachés à un autre niveau sont
  exclus, même si la limite haute utilise ce niveau comme référence.
- **Barre de vue** : le cube ouvre les styles visuels et l'icône soleil donne accès
  séparément aux ombres du soleil et à l'ombre ambiante. En plan, « Cadre » propose
  Rectangle (par défaut) ou Ligne pour tracer le contour. « Fond » permet de choisir
  une autre vue en plan du projet : ses traits et annotations apparaissent en gris
  derrière la vue courante et sont enregistrés avec celle-ci.
- **Volets latéraux** : glisser directement le titre de Propriétés ou de
  l'Arborescence pour détacher le volet dans une fenêtre native et le déplacer sur
  un second écran. Aucun bouton n'est nécessaire. Relâcher près d'un bord intérieur
  de la fenêtre principale le réancre de ce côté. Une fois détaché, son titre permet
  de le déplacer ; un double-clic sur le titre du contenu ou la fermeture de sa
  fenêtre le réintègre. Un clic droit sur le titre offre les autres emplacements.
  Le contenu reste lié au document et à la sélection courants, y compris dans
  l'éditeur de familles. « Paramètres → Général → Réintégrer les volets latéraux »
  retrouve les deux volets si un écran est débranché.
- **Ajuster / prolonger (`AJ`)** : cliquez un mur droit pour choisir son axe
  comme limite, puis cliquez près de l'extrémité d'un autre mur à modifier.
  La limite apparaît en orange et le résultat au survol en bleu, avant validation.
  La même limite reste active pour les murs suivants. Le mode « Prolonger seulement »
  interdit le raccourcissement. Échap ou clic droit efface d'abord la limite,
  puis termine l'outil ; Ctrl+Z annule une modification. Les assemblages droits
  sont traités avec toutes leurs couches. La case **Prolonger en angle** de la
  barre d'options ajuste les deux murs ensemble : cliquer près de chacune des deux
  extrémités à raccorder à leur intersection. Un cercle orange repère la première ;
  l'aperçu bleu montre les deux murs. Un seul Ctrl+Z annule tout le raccord.
  Les deux murs doivent être au même niveau. Les axes parallèles et les déplacements
  qui inverseraient le mur sont refusés. Le formulaire textuel d'Ajuster est supprimé.
- **Navigation des vues de menuiserie** : molette pour zoomer vers le curseur,
  bouton central maintenu pour déplacer la vue, double-clic central, bouton Cadrer (ou raccourci F)
  pour retrouver la famille. Plan, Coupe et Élévation conservent chacun leur
  zoom et leur déplacement ; ces actions ne modifient pas la famille.
- **Éditeur de familles d'ouvertures (à la manière de Revit)** : portes et fenêtres
  sont le même objet ; la catégorie (Porte / Fenêtre) de la famille range son type
  dans l'onglet voulu du sélecteur. Accès : « Éditer la famille… » (barre d'options
  d'une ouverture sélectionnée) ou, au placement, « Éditer la famille… » /
  « Nouvelle famille… » sous le sélecteur de type (gabarit Porte ou Fenêtre selon
  l'onglet), ou depuis la rubrique Familles de l'arborescence du projet (Portes,
  Fenêtres : clic pour ouvrir ; « Nouvelle porte… », « Nouvelle fenêtre… »,
  « Charger une famille… » pour un fichier `.famille.json`). L'éditeur s'ouvre dans
  un onglet « Famille … » (« * » : modifications non chargées dans le projet ;
  plusieurs familles s'ouvrent chacune dans son onglet, rouvrir un type déjà ouvert
  active son onglet) : le
  panneau Propriétés montre la famille (type, paramètres, contraintes, plans,
  volumes, extrusions, symboles) et ses commandes (« Charger dans le projet »,
  « Nouveau type », « Enregistrer sous… » un fichier de famille, « Ouvrir… ») ;
  l'arborescence montre ses vues (clic : ouvrir), paramètres, plans, volumes,
  extrusions, symboles (clic : sélectionner) et les familles du projet. Dans
  l'onglet, Ctrl+S charge la famille dans le projet ; Ctrl+Z / Ctrl+Maj+Z et les
  flèches Annuler / Rétablir agissent sur la famille seule (sa propre pile, un
  glissement ou une saisie font une étape) ; un autre onglet ramène aux
  vues sans perdre la famille ; fermer l'onglet avec des modifications demande de
  les charger, de les abandonner ou d'annuler. La famille vit dans un mur générique :
  - plans de référence tracés par deux points (outil « Plan de référence » : le
    plan passe par le premier point, vertical ou horizontal selon la direction
    dominante du tracé), sur trois axes (largeur, épaisseur, hauteur ; « Centre » et
    « Bas » fixes, « Extérieur » et « Intérieur » suivent les faces du mur hôte) ;
  - paramètres à formules (`Largeur - 2 * Dormant`), de portée **Type** (valeur de
    la famille), **Occurrence** (chaque ouverture placée la règle dans Propriétés,
    « Défaut » reprend la formule) ou **Partagé** (valeur commune du projet, table
    « Paramètres partagés du projet » ; « Depuis le projet… » réutilise un paramètre
    partagé existant ; `Largeur`, `Hauteur` : cotes du type, `Epaisseur` : mur) ;
    chaque paramètre est une **longueur**, un **oui/non** (case à cocher ; « Visible
    si » masque un volume ou une extrusion, un vide masqué ne perce plus) ou un
    **matériau** du projet (« Matériau par » donne sa couleur au volume) ; les
    paramètres d'occurrence oui/non et matériau se règlent aussi par ouverture ;
  - contraintes de distance (cote libellée par un paramètre ou une formule ; à la
    pose, le plan visé se colore en bleu, en rouge s'il n'est pas parallèle, et une
    cote provisoire le relie au plan déjà choisi) et
    d'égalité (EQ), résolues axe par axe par priorité décroissante (0 à 9 ; à
    égalité, la plus ancienne) : une contrainte incompatible est écartée et signalée
    en rouge ; un plan non contraint garde sa position dessinée ;
  - volumes rectangulaires (glisser) et extrusions de profils quelconques (cliquer
    les sommets, recliquer le premier ou clic droit pour fermer), pleins
    (menuiserie, vitrage, panneau, appui) ou vides ; chaque sommet ou face se
    verrouille sur le plan le plus proche, avec un décalage au-delà de 7 px ;
  - symboles 2D (ligne brisée, arc par trois points) en plan ou en élévation,
    affichés dans les vues du projet et sur les feuilles : en plan, ceux des
    ouvertures du niveau ; en coupe ou élévation, ceux des ouvertures vues de face.
  Vues élévation intérieure, plan et coupe, ouvertes depuis l'arborescence. Comme
  dans le projet, la sélection est le mode par défaut : Échap abandonne la saisie
  en cours, puis revient à la sélection, puis vide la sélection ; recliquer l'outil
  actif du ruban y revient aussi.
  Comme dans le projet : Ctrl+clic ajoute ou retire un élément ; glisser hors
  d'un élément trace une fenêtre (gauche → droite : éléments entièrement dedans ;
  droite → gauche : éléments touchés ; Ctrl : ajouter) ; Suppr supprime toute la
  sélection (les plans encore utilisés restent). Les cotes ont le dessin des cotes
  du projet (attaches, traits obliques) ; glisser une cote ou son texte déplace sa
  ligne, enregistrée dans la famille. L'accrochage est celui du projet : croisements
  de plans (croix), sommets (carré), milieux (triangle) ; Maj : orthogonal depuis le
  point précédent, angles ronds de 15° ; la molette zoome du même pas que les vues.
  En sélection, l'élément choisi porte des **poignées** : sommets d'un
  profil ou d'un symbole (glisser : réaccroché au plan le plus proche ; clic droit :
  retirer), milieux d'arêtes (glisser : insérer un sommet), milieux des faces d'un
  volume (glisser : reverrouiller la face). Un clic dans un objet le choisit avant
  les plans qui le traversent (les plans se choisissent hors des objets).
  Les valeurs d'essai font fléchir la famille ; « Enregistrer le type » met à jour
  le type et ses ouvertures (et les paramètres partagés du projet), « Nouveau type »
  l'affecte à l'ouverture (ou au prochain placement). Dans la maquette, les solides
  forment la menuiserie et les vides percent le mur à la place de la baie
  rectangulaire. Une ouverture placée se **retourne** (intérieur ↔ extérieur) ou
  s'**inverse** (main gauche ↔ droite) dans Propriétés. Gabarits : Fenêtre (baie,
  dormant, vitrage, appui en pente esquissé en coupe, triangle d'ouverture) et Porte
  (baie, dormant, vantail, débattement en plan, triangle en élévation). Limites :
  murs droits, profils polygonaux (arcs discrétisés dans les symboles seulement).
- **Nomenclatures** : la ligne « Paramètres » ajoute une colonne par paramètre des
  familles (type, occurrence, partagés) ; pour une ouverture, la valeur tient compte
  de ses valeurs d'occurrence et des paramètres partagés du projet (longueurs en mm,
  oui/non, nom du matériau) ; ces colonnes distinguent les groupes.
- **Choix du type au placement** : l'outil Ouverture affiche en tête de Propriétés
  des onglets Porte / Fenêtre / Libre et le type à placer (générique : cotes du
  ruban) ; un type impose ses cotes, son nom et sa famille ; l'outil garde le
  dernier type choisi.
- **Escalier** (Modéliser) : la ligne de foulée se trace avec le moteur du dessin 2D
  dans le plan ouvert (départ, un sommet par palier, arrivée ; Entrée ou clic droit
  pour finir) : droit, quart tournant ou demi-tournant selon le tracé, palier carré de
  la largeur à chaque sommet intermédiaire. Il monte du niveau du plan au niveau
  immédiatement au-dessus (sinon 2,80 m) ; le nombre de contremarches suit la
  hauteur de marche visée du ruban, un palier compte pour une marche, le giron
  partage la longueur des volées. Propriétés : niveaux de départ et d'arrivée (ou
  hauteur libre), décalage, largeur, hauteur de marche visée, paillasse ; calcul
  affiché (contremarches, giron, paliers, Blondel 2h + g, signalé hors confort :
  60 à 65 cm, h ≤ 20 cm, g ≥ 22 cm). En plan : ligne de foulée, rond au départ,
  flèche à l'arrivée. Le ruban choisit les garde-corps créés avec l'escalier (deux
  côtés, gauche, droite, aucun).
- **Garde-corps** (Modéliser) : tracé en plan (hauteur, décalage, remplissage
  barreaudage, panneau plein ou vitrage) ; main courante, lisse basse, montants aux
  sommets et tous les 1,5 m au plus, barreaux tous les 110 mm. Sur un escalier
  sélectionné, la barre d'options ajoute des garde-corps hébergés (deux côtés,
  gauche, droite) dont la base suit les marches ; ils suivent l'escalier copié et
  partent avec lui. Escaliers et garde-corps se déplacent, se copient, se tournent,
  s'exportent en IFC (IfcStair, IfcRailing) et entrent dans les nomenclatures.
- **Annuler / Rétablir**, Ctrl+Z et Ctrl+Maj+Z, ont un historique propre à chaque
  document. Les flèches après Sortie annulent ou rétablissent une étape ; leurs
  flèches donnent accès à la liste pour remonter ou rétablir plusieurs étapes.
  Les intitulés décrivent les éléments modifiés lorsqu'ils peuvent être identifiés.
  Un glissement est regroupé en une seule modification. L'historique
  retient jusqu'à 100 instantanés, avec une cible de 64 Mio (au moins le dernier
  instantané est conservé, même s'il dépasse cette taille).
- **Enregistrer / Enregistrer sous** valident le modèle et remplacent le fichier
  atomiquement. Chaque modification appliquée écrit une copie `.recuperation.json`.
  À la réouverture, l'application propose de la restaurer. Enregistrer ou fermer
  explicitement sans enregistrer supprime cette copie de récupération.
- **Dessin 2D** (Documenter) : ligne, polyligne, spline, cercle et arc se tracent
  directement dans la vue native ouverte, à la manière de CADCraft ou d'AutoCAD.
  Clic : point suivant ; clic droit, Entrée ou Espace : terminer ; F puis Entrée :
  fermer la polyligne ou la spline ; Échap : effacer la saisie, puis abandonner le
  tracé en cours, puis quitter l'outil. La ligne enchaîne les segments ; le cercle
  se donne par centre et rayon, l'arc par départ, point de passage et fin ; la spline
  passe par ses points (Catmull-Rom centripète). Accrochages : extrémité, milieu,
  centre, quadrant, intersection, perpendiculaire et point proche, sur les dessins
  de la vue et, en plan, sur les contours et axes des murs, les contours des éléments
  horizontaux du niveau et les quadrillages. Sans accrochage, repérage polaire tous
  les 15° ; Maj force l'orthogonal. Au clavier, la ligne de commande en bas de la vue
  reçoit `x,y` ou `x;y` (absolu), `@dx,dy` (relatif), `d<angle` ou `@d<angle`
  (polaire depuis le point précédent) ou une distance seule (dans la direction du
  curseur), dans l'unité d'affichage. Chaque forme achevée devient une annotation
  de la vue. Le même moteur (`dessin2d.rs`) trace les murs (ligne, rectangle, arc,
  cercle ; voir le README) : un arc donne un mur courbe (`axe` en arc), un cercle
  complet deux demi-cercles ; chaque forme constitue une seule étape d'annulation.
  Les éléments horizontaux (dalle, sol, plafond, toiture) l'utilisent aussi :
  polygone, rectangle ou cercle sur le plan de leur altitude.
- **Documenter** crée aussi textes, zones rectangulaires, cotes et étiquettes
  dans la vue native ouverte. Avec un mur sélectionné, une cote référence ses
  extrémités ; sinon ses points sont fixes. Une référence supprimée est signalée
  en rouge, sans supprimer silencieusement la cote.
- **Feuilles** (Documenter, groupe Documents, ou l'arborescence) ouvre un jeu de
  feuilles dans un onglet : toutes ses feuilles l'une sous l'autre, comme les pages
  d'InDesign, avec cadre et cartouche (projet, nom, cartouche libre, échelles,
  numéro). Glisser un plan, une coupe ou une élévation depuis l'arborescence sur une
  feuille la place, centrée sous le curseur ; glisser une vue placée la déplace ;
  double-clic l'ouvre ; Suppr la retire (ou supprime la feuille choisie). La page en
  pointillés ajoute une feuille. Glisser le titre d'une feuille (au-dessus de la page)
  la remet à un autre rang du jeu (barre bleue d'insertion) ; dans l'arborescence,
  glisser une feuille au-dessus ou au-dessous d'une autre la déplace, la déposer sur
  le nom d'un jeu la met à la fin de ce jeu. Ctrl+Z annule. Molette : zoom ; glisser le fond : défiler. Les
  réglages sont dans Propriétés, sans boîte de dialogue : nom du jeu (Entrée pour
  renommer) lorsque rien n'est sélectionné ; feuille (numéro, nom, format A0 à A4,
  paysage/portrait, dimensions, cartouche) ; vue placée (échelle et position).
  L'arborescence liste tous les jeux, toutes
  leurs feuilles et, sous chaque feuille, ses vues placées (clic : ouvrir la vue) ; clic
  sur une feuille pour l'amener à l'écran. La recherche garde un jeu entier si son nom
  correspond, une feuille si son titre ou une de ses vues correspond. Un fichier
  Revit converti (Début → Importer) apporte ses plans d'étage, coupes, élévations et
  feuilles (voir MODELE.md, conversion depuis Revit).
  L'arborescence propose la création et la duplication de jeux ; ses menus contextuels
  ajoutent une feuille, exportent un jeu ou une feuille et ouvrent l'éditeur de cartouche.
  Le pilote permet de lister les jeux
  (`feuilles`), sélectionner un jeu ou une feuille (`selection_feuille {"jeu":"Plans","feuille":"id"}` ;
  ajouter `"vue":0` pour une vue placée) et lire la sélection (`proprietes_feuille`).
  L'outil MCP `viewer_modifier_feuille` ou `editer` permet de modifier les données
  d'une feuille ; enregistrer ensuite le projet pour conserver la modification.
- **Nomenclatures** (Documenter ou l'arborescence) : quantités par catégorie,
  regroupées par type ou détaillées par élément, colonnes au choix (catégorie, type,
  élément, niveau, nombre, longueur, hauteur, largeur, épaisseur, surface, volume net ;
  regroupées, nombre, longueur, surface et volume sont totalisés), export CSV.
  Glisser une nomenclature de l'arborescence sur une feuille y pose son tableau
  (titre, en-tête, lignes ; coin haut gauche sous le curseur), tenu à jour avec le
  modèle, déplacé à la souris, retiré par Suppr ; il est dans les exports PDF/SVG.
  Sous chaque feuille, l'arborescence liste aussi ses nomenclatures. Les
  nomenclatures posées sur les feuilles Revit ne sont pas encore lues à l'import.
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
  de familles, les escaliers/garde-corps paramétriques, les symboles,
  légendes et altimétries, la fusion d'import et la comparaison de réimportations
  restent des itérations suivantes. Les sections circulaires et les poutres inclinées
  ne sont pas encore prises en charge. Les outils non raccordés sont désactivés.
- Les feuilles natives s'ouvrent dans le gestionnaire de documents avec un aperçu ;
  elles ne constituent pas encore des onglets de dessin éditables comme les vues.

## Raccourcis clavier

Sans champ de texte actif, taper deux lettres à la suite : `CS` créer à l'identique,
`MU` mur, `PO` poteau, `PP` poutre, `OU` ouverture, `MV` déplacer, `CO` copier,
`RO` tourner, `AL` aligner, `AJ` ajuster. `Ctrl+A` sélectionne les éléments visibles ;
Suppr demande confirmation de suppression. `CS` reprend les paramètres des murs,
éléments horizontaux à pente uniforme, poteaux, poutres, ouvertures natives simples,
quadrillages, coupes, élévations et annotations. Les menuiseries à géométrie importée
et les pentes différentes par arête ne sont pas encore proposées en création identique.
Les commandes Créer à l'identique et Supprimer sont aussi accessibles par leurs icônes dans l'onglet Modifier.
Le sous-projet de base est `A` et la variante par défaut est `Principale` ; le contexte
de la sélection est affiché dans Propriétés.

## Interfaces et commandes

Les commandes UI et MCP passent par les mêmes transactions. Une erreur de commande,
validation ou reconstruction conserve le modèle précédent.

`projet_natif` renvoie le document JSON et ses identifiants natifs. `editer <JSON>`
utilise ces identifiants, **pas** les identifiants numériques de la scène.

```text
editer {"action":"niveau","nom":"Étage","altitude":3000}
editer {"action":"barre","barre":{"id":"","nom":"Poteau","genre":"poteau","niveau":"rdc","debut":[0,0,0],"fin":[0,0,2800],"largeur":300,"hauteur":300,"rotation":0}}
editer {"action":"barre","barre":{"id":"","nom":"Poutre","genre":"poutre","niveau":"rdc","debut":[0,0,2600],"fin":[3000,0,2600],"largeur":200,"hauteur":400}}
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
vue ; leurs formes : `texte`, `ligne`, `polyligne` (`points`, `fermee`), `spline`
(`points` de passage, `fermee`), `cercle` (`centre`, `rayon`), `arc` (`points` :
départ, passage, fin), `zone`, `cote`, `etiquette`, en mm dans le repère 2D de la vue ; les positions et tailles des feuilles sont en mm papier. Aucun historique
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
sans multiéchantillonnage, `BIMEDIT_MSAA=0` désactive l'anticrénelage de la vue 3D.
