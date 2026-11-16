# Crate `interface`

Rôle : toute l'interface egui de BIMedit (`ViewerApp` : état, boucle, ruban, panneaux, vues 2D et 3D, outils, feuilles, éditeur de familles, collaboration, paramètres) et la fenêtre native (`socle/fenetre.rs`).

Dépendances internes autorisées : les 13 crates précédentes (`noyau`, `modele`, `ifc_base`, `scene`, `formats`, `maillage`, `commandes`, `moteur2d`, `moteur3d`, `echanges`, `collaboration`, `pilote`, `config`). Interdit : `app`.

## Où travailler

Rangement par domaine (lot 11b, docs/historique/ARCHITECTURE_CHANTIER_2026-11.md §5.19). Chaque dossier a un `AGENTS.md` court ; ne charger que celui de la partie à modifier.

| Dossier (`src/`) | Contenu |
|---|---|
| `lib.rs` | `ViewerApp`, boucle egui (`update`), déclaration des dossiers |
| `socle/` | documents ouverts, vues, viewport, soleil et caméra, fenêtre native (`fenetre`, seul module public), paramètres, aides, pilotage MCP, volets |
| `zones/` | `ruban/`, barre haute, panneaux, panneau droit, navigateur, propriétés, menu de la vue, vues natives et Revit, graphismes |
| `saisie/` | accrochage, repérage, saisie clavier et en plan, sélection, poignées d'annotation |
| `outils/` | `modeliser/` (`bim.rs`, murs courbes, garde-corps, escaliers), `modifier/` (transformer, ajuster, options de Modifier), `dessiner/dessin_plan/` (DAO 2D), `annoter/` (cotation, textes, types de trait) |
| `documenter/` | feuilles, cartouche, coupe, gabarits de vue |
| `familles/` | `editeur_famille/`, familles planes, document de famille, profils, sélection des familles |
| `projet/` | collaboration, gestion des contextes |
| `../assets/icone.png` | icône embarquée (`include_bytes!("../../assets/icone.png")` depuis `socle/fenetre.rs` et `zones/barre_haute.rs`) |

Règles :
- Chemins absolus `crate::<dossier>::<module>::…` ; pas de `super::` vers un autre dossier. Les fichiers `impl ViewerApp` importent `use crate::*;` (glob de la racine : `ViewerApp`, imports communs de `lib.rs`) ; dans un sous-dossier (`ruban/`, `dessin_plan/`, `editeur_famille/`), `use super::*;` reprend le glob du `mod.rs`.
- Visibilité : `pub(crate)` pour ce qui sert hors du dossier ; `pub(super)` seulement à l'intérieur d'un sous-dossier (`ruban/`, `dessin_plan/`, `editeur_famille/`). Rien de `pub` hors `socle::fenetre`.
- Le module local `projet::collaboration` masque la crate du même nom dans `projet/` : écrire `::collaboration::…` pour la crate.
- 23 fichiers dépassent 600 lignes (§5.19 c) : ne pas les agrandir ; découpage prévu en 11c.
- Les `mod.rs` des dossiers de premier niveau et d'`outils/` ne contiennent que des déclarations de modules. Ceux de `ruban/`, `dessin_plan/` et `editeur_famille/` portent du code (antérieur au lot 11b).

## Quand modifier ici

Un écran, un outil à la souris, le ruban, un panneau, l'éditeur de familles, les feuilles. Ne pas y mettre de logique de modèle ni de géométrie : elle va dans `modele`, `commandes` ou `moteur2d`.

## Vérifier

```
cargo test -p interface --release
grep -rn "crate::" crates/interface/src   # uniquement des modules internes
```

L'interface graphique ne se lance pas sans affichage ; `BIMEDIT_ARRIERE_PLAN=1` crée la fenêtre hors écran.
