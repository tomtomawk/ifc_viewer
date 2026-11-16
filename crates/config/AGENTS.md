# Crate `config`

Rôle : options utilisateur (thème, unité d'affichage, navigation IFC, délai d'accrochage, gabarit, raccourcis) enregistrées dans le dossier de configuration de l'utilisateur ; table des raccourcis par commande.

Dépendances internes autorisées : `noyau`, `modele`, `echanges` (réel : les trois). Interdit : `interface`, `pilote`, `moteur3d`, `commandes`, `collaboration`. `eframe` n'est là que pour `egui::{DragValue, Context, ThemePreference, Key, Modifiers}` : ne pas y ajouter d'autre usage d'egui.

## Où travailler

| Fichier (`src/`) | Contenu |
|---|---|
| `lib.rs` | déclare `options` |
| `options/mod.rs` | `Options` (charger, enregistrer), `ThemeInterface`, trait `Dimensions` |
| `options/raccourcis.rs` | `COMMANDES`, `Raccourci` |

L'unité d'affichage (`Unite`, `unite()`, `definir_unite()`) est dans `modele::unite`, pas ici.

## Quand modifier ici

Une option utilisateur ou un raccourci clavier enregistré. Le panneau des paramètres est dans `interface/src/socle/parametres.rs`.

## Vérifier

```
cargo test -p config --release
cargo tree -p config   # aucune crate interne hors noyau, modele, echanges
```
