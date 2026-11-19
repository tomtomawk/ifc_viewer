# Crate `collaboration`

Rôle : transactions de valeurs persistantes partagées entre postes, indépendantes de l'interface et du transport : dépôt (journal, instantanés, verrou fichier), session locale, concurrence et réservations, fichiers `.bimpartage`, texte collaboratif (`yrs`), CLI `--collaboration`.

Dépendances internes autorisées : `noyau`, `modele`, `maillage`, `commandes`, `echanges` (exactement celles-ci). Interdit : `scene`, `moteur2d`, `moteur3d`, `config`, `pilote`, `interface`, egui, wgpu, eframe. Aucun ré-export de `commandes::objets` : appeler `commandes::objets::{objets, carte_objet}`.

## Où travailler

| Fichier (`src/`) | Contenu |
|---|---|
| `lib.rs` | `PROTOCOLE`, `projet`, `Transaction`, validation et fusion ; libellés de session (`libelle_etat`, `libelle_echec`, `echec`) |
| `session.rs` | `Session` (599 lignes : ne pas agrandir) |
| `stockage.rs` | `Depot` (626 lignes : ne pas agrandir) |
| `concurrence.rs`, `partage.rs`, `texte.rs`, `cli.rs` | décomposition et empreintes, `.bimpartage`, texte `yrs`, CLI |
| `tests.rs` | 37 tests ; `course_entre_deux_processus…` et `crash_processus…` relancent le binaire de test par `--exact "tests::enfant_…"` : garder ce chemin si le module change de nom |

`tests/` : 2 tests d'intégration (aller-retour par `collaboration::projet`). Les `include_str!` de `samples/` et de `modele/src/lib.rs` sont relatifs à `src/`.

## Quand modifier ici

Protocole de transaction, journal, verrou fichier, fusion de projets, texte collaboratif, `.bimpartage`. L'écran de collaboration est dans `interface/src/projet/collaboration.rs` ; la commande en ligne `--collaboration` est ici (`cli.rs`). Voir `docs/COLLABORATION.md`.

## Vérifier

```
cargo test -p collaboration --release
cargo tree -p collaboration   # aucune crate interne hors noyau, modele, maillage, commandes, echanges
```
