# Fonctions de modification : modèle de fonctionnement

Onglet **Modifier** du ruban. Ce document fixe le comportement attendu de chaque
fonction. L'état de l'existant est relevé dans `crates/app/src/modele/edition/commandes/mod.rs`
(enum `Commande`) et dans `crates/app/src/ui/ruban/mod.rs` (`groupes(Menu::Modifier)`).

## 1. Règles communes

Toutes les fonctions suivent le même cycle :

| Étape | Règle |
|---|---|
| 1. Sélection | Avant la commande (présélection) ou après (la commande demande « Sélectionner les éléments, Entrée pour valider »). Ctrl ajoute, Maj retire, fenêtre gauche→droite = entièrement inclus, droite→gauche = touchés. |
| 2. Filtrage | Les éléments **verrouillés** sont retirés de la sélection active avec un message (« 2 éléments verrouillés ignorés »). Les éléments non pris en charge par la commande sont signalés de la même façon. |
| 3. Saisie | Clics dans la vue, avec accrochage. À chaque étape, la **barre de commande** affiche la consigne et les champs numériques (distance, angle, nombre, facteur). Taper un nombre remplit le champ actif ; Tab passe au champ suivant. |
| 4. Aperçu | Aperçu fantôme permanent, sans modifier le projet. |
| 5. Validation | Clic final ou Entrée → **une seule transaction** (`Projet::executer`), donc un seul Ctrl+Z. |
| 6. Sortie | Échap : revient à l'étape précédente, puis quitte la commande. La sélection est conservée. Clic droit : menu Valider / Annuler / Répéter. |
| 7. Répétition | Entrée ou Espace sur une vue vide relance la dernière commande. |

Options communes de la barre d'options :
- **Copier** (case) : la transformation porte sur une copie, l'original reste.
- **Contraindre** (Maj maintenue) : axes du plan, angles multiples de 15°.
- **Multiple** : la commande reste active après validation (copies successives).
- **Conserver les jonctions** : les murs joints suivent (déplacer, décaler) ou sont détachés (copie, symétrie).

Comportement BIM :
- Une ouverture suit son hôte. Déplacée seule, elle glisse dans l'hôte (abscisse, allège) et ne le quitte pas.
- Un mur composé (assemblage) se transforme d'un bloc, sauf si une couche seule est sélectionnée.
- Les annotations liées (cotes, étiquettes) suivent leurs références ; une cote dont une référence disparaît est supprimée avec un message.
- En collaboration, la commande réserve les objets touchés avant de publier.

### Exigence : tout se fait graphiquement

Chaque fonction s'exécute **entièrement à la souris, dans la vue** (plan, coupe, 3D) :
la saisie clavier accélère mais n'est jamais obligatoire. Aucune boîte de dialogue
bloquante. Concrètement :

- **Consigne au curseur** : l'étape en cours s'affiche près du curseur (« Point de départ », « Côté à conserver »…), en plus de la barre de commande.
- **Survol avant clic** : l'élément ou la référence visés sont surlignés avant le clic (référence en bleu, élément traité en orange). L'utilisateur voit toujours ce qu'il va toucher.
- **Accrochage visible** : symbole d'accrochage (extrémité, milieu, intersection, perpendiculaire, axe, face) et lignes de rappel.
- **Cotes temporaires** : distance, angle, nombre ou facteur affichés pendant le mouvement. Un clic sur une cote temporaire permet de la saisir.
- **Aperçu fantôme** du résultat final, copies et jonctions comprises.
- **Poignées** sur la sélection, quand aucune commande n'est active : extrémités (prolonger), milieu (déplacer), centre de rotation (déplaçable), flèches d'inversion. Tirer une poignée équivaut à la commande correspondante.
- **Les mêmes fonctions dans les trois vues** : en 3D, la saisie se fait sur le plan de travail du niveau de l'élément.

Contrôle de chaque fonction : un script `tools/test-*.py` la joue par le pilote (clics simulés), puis vérifie l'aperçu, la transaction, l'annulation et Échap.

## 2. Fonctions de modification

| # | Fonction | Raccourci | Sélection | Saisie | Options | État actuel |
|---|---|---|---|---|---|---|
| 1 | **Aligner** | AL | aucune préalable | 1. ligne de référence (axe, face, ligne de quadrillage) ; 2. éléments à aligner, clic par clic | Référence : axe / face extérieure / face intérieure / face du noyau ; Verrouiller l'alignement (cadenas) | **graphique** : référence cliquée puis éléments cliqués, survol surligné ; reste : choix de face, cadenas |
| 2 | **Déplacer** | MV | éléments | point de départ → point d'arrivée, ou distance saisie | Copier ; Contraindre ; Déconnecter (détacher les jonctions) ; Déplacer en Z | **graphique** : deux clics ou distance tapée, case Copier ; reste : Déconnecter, Z |
| 3 | **Décaler** | DE | murs, lignes, contours | côté par la souris + distance, ou clic de passage | Copier (par défaut : oui) ; Mesure : axe / face | **graphique** : murs droits, courbes (arc concentrique) et composés, contour de dalle (vers l'extérieur ou l'intérieur), distance au curseur ou tapée, Copier coché ; lignes d'annotation par le pilote |
| 4 | **Tourner** | RO | éléments | centre (sélection par défaut, déplaçable) → direction de départ → angle, ou angle saisi | Copier ; Pas angulaire | **graphique** : centre déplaçable au clic, rapporteur, Maj 15°, angle tapé, case Copier (une seule annulation) |
| 5 | **Copier** | CO | éléments | point de départ → point(s) d'arrivée | Multiple ; Sur un niveau (= `CopierSurNiveau`) | **graphique** : deux clics, Multiple |
| 6 | **Réseau** | RE | éléments | linéaire : 2 points + nombre ; polaire : centre + angle + nombre | Linéaire / polaire ; Dernier = distance totale ou pas ; Grouper | **graphique** : linéaire (base, 2ᵉ élément ou pas tapé) et polaire (centre, angle entre éléments), nombre réglable ; reste : Grouper |
| 7 | **Échelle** | EC | éléments 2D, contours, terrains, annotations | point de base → facteur (saisi ou 2 longueurs) | Graphique / numérique | **graphique** : base, référence, nouvelle longueur ou facteur tapé ; refusée sur murs, menuiseries, structures |
| 8 | **Symétrie** | SY | éléments | axe : élément existant ou 2 points | Copier (par défaut : oui) ; Symétriser les familles (sens d'ouverture des portes) | **graphique** : deux points de l'axe, Copier coché par défaut, face extérieure des murs gardée dehors, paumelles inversées ; reste : axe pris sur un élément |
| 9 | **Ajuster / prolonger** | AJ · TR (en angle) | aucune préalable | 1. référence ; 2. élément, côté à conserver | Un seul / plusieurs éléments vers une même référence ; En angle (deux éléments à l'intersection) | `Ajuster` et `AjusterAngle` existent ; mode « plusieurs » absent |
| 10 | **Scinder** | SC | aucune préalable | clic sur l'élément au point de coupure | Écart (scinder avec espace) ; Supprimer le segment intérieur (2 clics) | **graphique** : murs droits et composés, poutres (hauteur interpolée), écart réglable, ouvertures réparties ; lignes d'annotation par le pilote |
| 11 | **Supprimer** | Suppr | éléments | aucune | Rapport des dépendants supprimés (ouvertures, cotes, étiquettes) avant validation | `Supprimer` existe ; rapport des dépendants absent |

## 3. Visibilité

Deux portées distinctes, à ne pas confondre :

| Portée | Effet | Enregistré | Indicateur |
|---|---|---|---|
| **Temporaire** | masque ou isole pour travailler | non (perdu à la fermeture de la vue) | cadre coloré autour de la vue + bouton « Réinitialiser » |
| **Dans la vue** | masque l'élément dans cette vue seulement | oui (`Vue::elements_masques`) | visible avec « Afficher les éléments masqués » |

| # | Fonction | Raccourci | Effet | État actuel |
|---|---|---|---|---|
| 12 | **Verrouiller** | CD | l'élément ne peut être ni déplacé, ni modifié, ni supprimé ; il reste sélectionnable et copiable ; cadenas affiché sur l'élément dans la vue | **fait** : `Projet::verrous`, `Commande::Verrouiller`, contrôle dans `executer` et à chaque édition (`modele/edition/verrous.rs`), test `tools/test-verrou.py` ; reste : propriétés grisées |
| 13 | **Déverrouiller** | DC | lève le verrou ; ou clic sur le cadenas dans la vue | **fait** |
| 14 | **Masquer** | H (temporaire), MQ (dans la vue) | deux commandes : temporaire (perdu à la fermeture de la vue, cadre turquoise avec une croix en haut pour en sortir) ou enregistré avec la vue ; ruban Modifier, groupe « Verrou et visibilité » | **fait** |
| 15 | **Afficher** | AF | mode « Afficher les éléments masqués » : masqués en magenta, clic sur un élément pour le réafficher (temporaire ou de la vue), Échap pour sortir | **fait** |
| — | **Isoler** | I | temporaire | existe |

Règle : le verrouillage s'applique au **modèle** (partagé en collaboration). Le masquage s'applique à la **vue** ou à la **session**.

## 4. Interaction graphique par fonction

| Fonction | Clics | Ce que l'écran montre |
|---|---|---|
| Aligner | référence → élément(s) | référence en pointillé bleu, face visée surlignée, cadenas cliquable après l'alignement |
| Déplacer | départ → arrivée | élément fantôme attaché au curseur, cote de distance, lignes d'alignement sur les autres éléments |
| Décaler | survol → clic du côté voulu | ligne décalée en fantôme du côté du curseur, cote de décalage |
| Tourner | centre (glissable) → direction → angle | rapporteur, arc d'angle, cote d'angle, fantôme tourné |
| Copier | départ → arrivées successives | fantôme à chaque position, compteur de copies |
| Réseau | départ → deuxième élément ou dernier | toutes les copies en fantôme, nombre modifiable au clic sur l'étiquette, poignée pour étirer le réseau |
| Échelle | base → longueur de référence → nouvelle longueur | fantôme mis à l'échelle, facteur affiché |
| Symétrie | axe (élément ou 2 points) | axe en tirets, fantôme symétrique, portes retournées |
| Ajuster / prolonger | référence → côté à conserver | partie conservée en orange, partie supprimée en gris pointillé, prolongement en fantôme |
| Prolonger en angle | élément 1 → élément 2 | les deux éléments surlignés, angle résultant en fantôme |
| Scinder | point sur l'élément | marque de coupure suivant le curseur, cotes aux deux extrémités |
| Supprimer | sélection → Suppr | dépendants surlignés en rouge avant validation |
| Verrouiller | sélection → bouton | cadenas sur chaque élément verrouillé ; un clic sur le cadenas déverrouille |
| Masquer | sélection → H / HV | cadre coloré de vue temporaire |
| Afficher | mode afficher → clic | masqués en magenta, clic pour les réafficher |

## 5. Fonctions ajoutées (oubliées dans la liste initiale)

| Priorité | Fonction | Interaction graphique | État |
|---|---|---|---|
| 1 | Démolir (DM) | clic sur l'élément → démoli dans la dernière phase, teinte rouge ; nouveau clic : annuler | **fait** (`Projet::demolitions`) ; reste : trait de démolition dans les plans imprimés |
| 1 | Sélectionner tous les identiques | clic droit sur l'élément → « dans la vue / dans le projet » | **fait** |
| 1 | Filtre de sélection | clic droit → Filtrer la sélection → « Garder seulement : catégorie (n) » | **fait** |
| 1 | Reproduire les propriétés (RP) | pipette sur la source → pinceau sur les cibles (murs simples, dalles, ouvertures, poteaux et poutres) | **fait** |
| 1 | Remplacer le type | sélection → sélecteur de type du panneau Propriétés | existait |
| 1 | Joindre / détacher, attacher haut / bas | bouton Jonctions du ruban Modifier | existait ; reste : version clic sur la cible |
| 1 | Coller aligné sur niveaux | Ctrl+C puis Ctrl+V → niveau de destination | existait (un niveau à la fois) ; reste : plusieurs niveaux d'un coup |
| 2 | Grouper / dissocier | sélection → bouton ; groupe encadré au survol | **à faire** |
| 2 | Changer d'hôte (CH) | clic sur l'ouverture (ou sur la baie) → clic sur le nouveau mur à l'emplacement voulu | **fait** ; reste : aperçu glissant |
| 2 | Inverser (NV) | outil : clic sur un mur (sens) ou une ouverture (côté, Maj : paumelles) ; flèches d'inversion à côté du mur sélectionné | **fait** |
| 2 | Remplacer le graphisme dans la vue | sélection → plume, couleur, demi-teinte dans la barre d'options | **à faire** |
| 2 | Cadenas des cotes et alignements | cadenas cliquable sur la cote | **à faire** |
| 3 | Peindre un matériau | matériau choisi → clic sur la face | **à faire** |
| 3 | Diviser une face | tracer le contour sur la face | **à faire** |

## 6. Ordre de mise en œuvre conseillé

1. **Socle commun** : fait pour Déplacer, Copier, Tourner, Aligner (`crates/app/src/ui/transformer.rs`, test `tools/test-transformer.py`) : consigne au curseur, survol, aperçu, cotes temporaires, valeur tapée, Échap d'une étape, clic droit, une transaction. Reste : menu du clic droit, répétition de la dernière commande, poignées hors commande.
2. **Verrouiller / Déverrouiller** : fait. Une sélection mixte ignore les éléments verrouillés ; une sélection entièrement verrouillée est refusée.
3. **Symétrie et Réseau** : faits (`Commande::Symetrie`, `Commande::Reseau`, transformation affine dans `transformations.rs`).
4. **Décaler et Scinder** : faits pour les murs droits et composés (`commandes/modifier.rs`).
5. **Échelle** : faite, réservée aux annotations, contours, terrains et quadrillages.

Test graphique de l'ensemble : `tools/test-transformer.py` (clics et frappes simulés, annulation).

Fait ensuite (`crates/app/src/ui/menu_vue.rs`, `tools/test-visibilite.py`, `tools/test-elements.py`) :
- Visibilité dans le ruban Modifier (groupe « Verrou et visibilité » : Verrou en bascule, Isoler, Masquer, Masquer vue, Afficher) ; Cadre dans la barre de vue. Croix en haut du cadre temporaire pour tout réafficher.
- Accrochages du tracé : murs existants, origine du projet, prolongement et perpendiculaire du dernier trait, prolongement du mur dont on glisse une extrémité ; suivi polaire tous les 45°.
- Clic droit dans la vue hors commande : répéter la dernière commande, annuler, rétablir, identiques, filtre, verrou, masquer, isoler, démolir, inverser, supprimer.
- Entrée sans commande active : relance la dernière commande de modification.
- Poignée carrée au milieu du mur sélectionné (glisser = déplacer) et flèches d'inversion.
- Un clic qui ne touche aucune face (bord du mur en plan, vide d'une baie) vise le mur ou l'ouverture qui contient le point.

Reste à faire : Supprimer avec rapport des dépendants, Ajuster plusieurs éléments, menu du clic droit pendant une commande (Valider / Annuler), poignées des dalles et des poutres, Décaler et Scinder des lignes d'annotation à la souris, fonctions de la section 5 marquées « à faire ».
