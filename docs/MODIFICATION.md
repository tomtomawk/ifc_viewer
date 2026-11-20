# Fonctions de modification : modèle de fonctionnement

Onglet **Modifier** du ruban. Ce document fixe le comportement attendu de chaque
fonction ; l'état est dans [ROADMAP.md](ROADMAP.md) (à faire) et [SUIVI.md](SUIVI.md) (fait). Les commandes existantes se lisent dans `crates/commandes/src/socle/commande.rs`
(enum `Commande`) et dans `crates/interface/src/zones/ruban/mod.rs` (`groupes(Menu::Modifier)`).

## 1. Règles communes

Toutes les fonctions suivent le même cycle :

| Étape | Règle |
|---|---|
| 1. Sélection | Avant la commande (présélection) ou après (la commande demande « Sélectionner les éléments, Entrée pour valider »). Ctrl ajoute, Maj retire, fenêtre gauche→droite = entièrement inclus, droite→gauche = touchés. |
| 2. Filtrage | Les éléments **verrouillés** sont retirés de la sélection active avec un message (« 2 éléments verrouillés ignorés »). Les éléments non pris en charge par la commande sont signalés de la même façon. |
| 3. Saisie | Clics dans la vue, avec accrochage. À chaque étape, la **barre de commande** affiche la consigne et les champs numériques (distance, angle, nombre, facteur). Taper un nombre remplit le champ actif ; Tab passe au champ suivant. |
| 4. Aperçu | Aperçu fantôme permanent, sans modifier le projet. |
| 5. Validation | Clic final ou Entrée → **une seule transaction** (`Projet::executer`), donc un seul Ctrl+Z. |
| 6. Sortie | Échap : revient à l'étape précédente, puis quitte la commande. La sélection est conservée. Clic droit : comme Échap (placements de poteau, composant, arbre, pièce, texte ; Attacher au toit retire le dernier mur choisi) ; menu Valider / Annuler / Répéter à venir (M8). |
| 7. Répétition | Entrée ou Espace sur une vue vide relance la dernière commande. |

Options communes de la barre d'options :
- **Copier** (case) : la transformation porte sur une copie, l'original reste.
- **Contraindre** (Maj maintenue) : axes du plan, angles multiples de 15°.
- **Multiple** : la commande reste active après validation (copies successives).
- **Conserver les jonctions** : les murs joints suivent (déplacer, décaler) ou sont détachés (copie, symétrie).

Supprimer (`Commande::Supprimer`, `crates/commandes/src/modifier/mod.rs` et `supprimer.rs`) :
- Agit tout de suite, sans avertissement ni choix ; une seule étape d'annulation (Ctrl+Z).
- Partent avec l'élément ses dépendants structurels (`dependances`) : couches d'un assemblage, ouvertures d'un mur, garde-corps d'un escalier.
- Les annotations qui s'appuient sur un élément supprimé sont conservées et détachées : une cote associée (`PointCote::Mur` ou `Arete`) a ses points supprimés figés en points fixes à leur position (l'autre côté suit encore son élément) ; une cote angulaire entre arêtes est figée (sans arêtes) ; une étiquette devient un texte libre portant le nom affiché.
- Une annotation sélectionnée part normalement ; les annotations d'une vue supprimée partent avec la vue.
- Message d'état : « N élément(s) supprimé(s), M annotation(s) détachée(s) — Ctrl+Z pour annuler ». Le pilote (`editer {"action":"supprimer",...}`) donne le même résultat.

Comportement BIM :
- Une ouverture suit son hôte. Déplacée seule, elle glisse dans l'hôte (abscisse, allège) et ne le quitte pas.
- Un mur composé (assemblage) se transforme d'un bloc, sauf si une couche seule est sélectionnée.
- Les annotations liées (cotes, étiquettes) suivent leurs références. Quand une référence est supprimée, l'annotation est conservée et détachée (voir Supprimer).
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
- **Poignées** sur la sélection, quand aucune commande n'est active : extrémités (prolonger), milieu (déplacer), centre de rotation (déplaçable), flèches d'inversion. Tirer une poignée équivaut à la commande correspondante. Murs droits et poutres (sélection d'un seul élément) : ronds aux extrémités de la ligne (implantation du mur, ligne tracée de la poutre) — glisser allonge, raccourcit ou pivote autour de l'autre extrémité, accrochages de la vue, Maj pour les axes, longueur tapée pendant le glissement ou dans la cote affichée ensuite ; carré au milieu — glisser déplace (`Deplacer`) ; flèches d'inversion pour les murs seulement. Une poutre garde la hauteur de ses extrémités et s'enregistre par `Commande::Barre` ; un glissement = une étape d'annulation (`interface/src/outils/modifier/poignees.rs`, `Barre::avec_extremite`).
- **Les mêmes fonctions dans les trois vues** : en 3D, la saisie se fait sur le plan de travail du niveau de l'élément.

Contrôle de chaque fonction : un script `tools/test-*.py` la joue par le pilote (clics simulés), puis vérifie l'aperçu, la transaction, l'annulation et Échap.

## 2. Fonctions de modification

| # | Fonction | Raccourci | Sélection | Saisie | Options |
|---|---|---|---|---|---|
| 1 | **Aligner** | AL | aucune préalable | 1. segment de référence en plan ou face de référence en 3D ; 2. segment ou face de l’élément à déplacer, clic par clic | Échap ou clic droit pour changer de référence |
| 2 | **Déplacer** | MV | éléments | point de départ → point d'arrivée, ou distance saisie | Copier ; Contraindre ; Déconnecter (détacher les jonctions) ; Déplacer en Z |
| 3 | **Décaler** | DE | murs, lignes, contours | côté par la souris + distance, ou clic de passage | Copier (par défaut : oui) ; Mesure : axe / face |
| 4 | **Tourner** | RO | éléments | centre (sélection par défaut, déplaçable) → direction de départ → angle, ou angle saisi | Copier ; Pas angulaire |
| 5 | **Copier** | CO | éléments | point de départ → point(s) d'arrivée | Multiple ; Sur un niveau (= `CopierSurNiveau`) |
| 6 | **Réseau** | RE | éléments | linéaire : 2 points + nombre ; polaire : centre + angle + nombre | Linéaire / polaire ; Dernier = distance totale ou pas ; Grouper |
| 7 | **Échelle** | EC | éléments 2D, contours, terrains, annotations | point de base → facteur (saisi ou 2 longueurs) | Graphique / numérique |
| 8 | **Symétrie** | SY | éléments | axe : élément existant ou 2 points | Copier (par défaut : oui) ; Symétriser les familles (sens d'ouverture des portes) |
| 9 | **Ajuster / prolonger** | AJ · TR (en angle) | aucune préalable | 1. référence ; 2. élément, côté à conserver | Un seul / plusieurs éléments vers une même référence ; En angle (deux éléments à l'intersection) |
| 10 | **Scinder** | SC | aucune préalable | clic sur l'élément au point de coupure | Écart (scinder avec espace) ; Supprimer le segment intérieur (2 clics) |
| 11 | **Supprimer** | Suppr | éléments | aucune | Agit tout de suite, sans avertissement ni choix ; dépendants structurels supprimés avec l'élément, annotations liées détachées (voir ci-dessous) |

**Aligner en plan** suit les segments du dessin, notamment les arêtes visibles, les contours de coupe, les quadrillages et les traits d’annotation. **Aligner en 3D** utilise le plan du triangle touché. Une rotation autour de l’axe vertical et une translation amènent le segment ou la face sur le support de référence. Deux faces d’inclinaisons différentes demandent une bascule hors du plan horizontal et sont refusées. Un élément IFC lié peut servir de référence. Un clic déplace l’élément du projet qui porte la géométrie visée en une seule opération annulable.

## 3. Visibilité

Deux portées distinctes, à ne pas confondre :

| Portée | Effet | Enregistré | Indicateur |
|---|---|---|---|
| **Temporaire** | masque ou isole pour travailler | non (perdu à la fermeture de la vue) | cadre coloré autour de la vue + bouton « Réinitialiser » |
| **Dans la vue** | masque l'élément dans cette vue seulement | oui (`Vue::elements_masques`) | visible avec « Afficher les éléments masqués » |

| # | Fonction | Raccourci | Effet |
|---|---|---|---|
| 12 | **Verrouiller** | CD | l'élément ne peut être ni déplacé, ni modifié, ni supprimé ; il reste sélectionnable et copiable ; cadenas affiché sur l'élément dans la vue |
| 13 | **Déverrouiller** | DC | lève le verrou ; ou clic sur le cadenas dans la vue |
| 14 | **Masquer** | H (temporaire), MQ (dans la vue) | deux commandes : temporaire (perdu à la fermeture de la vue, cadre turquoise avec une croix en haut pour en sortir) ou enregistré avec la vue ; ruban Modifier, groupe « Verrou et visibilité » |
| 15 | **Afficher** | AF | mode « Afficher les éléments masqués » : masqués en magenta, clic sur un élément pour le réafficher (temporaire ou de la vue), Échap pour sortir |
| — | **Isoler** | I | temporaire |

Règle : le verrouillage s'applique au **modèle** (partagé en collaboration). Le masquage s'applique à la **vue** ou à la **session**.

## 4. Interaction graphique par fonction

| Fonction | Clics | Ce que l'écran montre |
|---|---|---|
| Aligner | référence → élément(s) | référence en bleu, segment ou face visé en orange |
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
| Supprimer | sélection → Suppr | aucun : suppression immédiate, message d'état |
| Verrouiller | sélection → bouton | cadenas sur chaque élément verrouillé ; un clic sur le cadenas déverrouille |
| Masquer | sélection → H / HV | cadre coloré de vue temporaire |
| Afficher | mode afficher → clic | masqués en magenta, clic pour les réafficher |

## 5. Fonctions ajoutées (oubliées dans la liste initiale)

| Priorité | Fonction | Interaction graphique |
|---|---|---|
| 1 | Démolir (DM) | clic sur l'élément → démoli dans la dernière phase, teinte rouge ; nouveau clic : annuler |
| 1 | Sélectionner tous les identiques | clic droit sur l'élément → « dans la vue / dans le projet » |
| 1 | Filtre de sélection | clic droit → Filtrer la sélection → « Garder seulement : catégorie (n) » |
| 1 | Reproduire les propriétés (RP) | pipette sur la source → pinceau sur les cibles (murs simples, dalles, ouvertures, poteaux et poutres) |
| 1 | Remplacer le type | sélection → sélecteur de type du panneau Propriétés |
| 1 | Joindre / détacher, attacher haut / bas | bouton Jonctions du ruban Modifier |
| 1 | Coller aligné sur niveaux | Ctrl+C puis Ctrl+V → niveau de destination |
| 2 | Grouper / dissocier | sélection → bouton ; groupe encadré au survol |
| 2 | Changer d'hôte (CH) | clic sur l'ouverture (ou sur la baie) → clic sur le nouveau mur à l'emplacement voulu |
| 2 | Inverser (NV) | outil : clic sur un mur (sens) ou une ouverture (côté, Maj : paumelles) ; flèches d'inversion à côté du mur sélectionné |
| 2 | Remplacer le graphisme dans la vue | sélection → plume, couleur, demi-teinte dans la barre d'options |
| 2 | Cadenas des cotes et alignements | cadenas cliquable sur la cote |
| 3 | Peindre un matériau | matériau choisi → clic sur la face |
| 3 | Diviser une face | tracer le contour sur la face |

## 6. État

Reste à faire : [ROADMAP.md](ROADMAP.md) §4 ; fait : [SUIVI.md](SUIVI.md) §3. Tests graphiques : `tools/test-transformer.py`, `tools/test-visibilite.py`, `tools/test-elements.py`, `tools/test-verrou.py`.
