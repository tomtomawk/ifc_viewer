# Crate `interface`

Rôle : toute l'interface egui de BIMedit (`ViewerApp` : état, boucle, ruban, panneaux, vues 2D et 3D, outils de dessin, feuilles, éditeur de familles, collaboration, paramètres) et la fenêtre native (`fenetre.rs`).

Dépendances internes autorisées : toutes les crates précédentes (réel : `noyau`, `modele`, `ifc_base`, `scene`, `formats`, `maillage`, `commandes`, `moteur2d`, `moteur3d`, `echanges`, `collaboration`, `pilote`, `config`). Interdit : `app`.

## Où travailler

Rangement à plat (70 fichiers, 34 308 lignes) : le rangement interne est la partie 11b du lot 11 (docs/ARCHITECTURE.md §4 et §5.18). Ne pas réorganiser avant elle.

| Fichier (`src/`) | Contenu |
|---|---|
| `lib.rs` | `ViewerApp`, boucle egui, déclaration des modules |
| `fenetre.rs` | `lancer` (fenêtre eframe), `arriere_plan`, `cadre_natif` |
| `commandes_pilote.rs`, `pilotage.rs` | exécution des commandes reçues de `pilote` |
| `ruban/`, `dessin_plan/`, `editeur_famille/`, `bim/` | sous-modules par zone |
| `assets/icone.png` | icône embarquée (`include_bytes!`) |

Le module local `collaboration` masque la crate du même nom : écrire `::collaboration::…` pour la crate. `pub(super)` garde son sens tant que les fichiers ne bougent pas. 15 fichiers dépassent 600 lignes (§5.18 c) : ne pas les agrandir.

## Vérifier

```
cargo test -p interface --release
grep -rn "crate::" crates/interface/src   # uniquement des modules internes
```

L'interface graphique ne se lance pas sans affichage ; `BIMEDIT_ARRIERE_PLAN=1` crée la fenêtre hors écran.
