# Études P2 — styles d'objets globaux, multiplateforme, WebAssembly

Feuille de route du 16 novembre 2026, demandes n° 15, 4 et 3. Ces trois demandes sont des évolutions planifiées : ce document en fixe l'architecture ou le plan, sans les réaliser. Les relevés de code datent du 16 novembre 2026 (commit `35e1c42`).

## N° 15 — Styles d'objets globaux

### Besoin

Une bibliothèque de styles indépendante des projets, par catégorie d'objets : couleur, épaisseur de trait, motifs, transparence. Le besoin est de prévoir l'architecture tôt, et de préciser les surcharges locales et la synchronisation.

### Existant

- Les styles sont aujourd'hui portés par chaque projet :
  - `materiaux[]` : couleur, motifs de coupe et de surface, apparence ;
  - `plumes` : épaisseurs ;
  - `motifs[]` : définitions de hachures ;
  - `types_traits` : types de trait.
- Le gabarit (`gabarits/defaut.gabarit.json`, `modele::gabarit`) amorce un nouveau projet, puis n'est plus relu. Chaque projet garde sa copie.

### Architecture proposée

1. **Fichier de bibliothèque**, hors projet : `styles.bimstyles.json`, dans le dossier de configuration de l'utilisateur (crate `config`) ou à un chemin partagé de l'agence. Contenu :
   - `version` ;
   - `categories[]` : `cle` (catégorie IFC ou native : `murs`, `dalles`, `ouvertures`, `annotations/cotes`…), `couleur`, `plume` (1 à 12), `motif_coupe`, `motif_surface`, `transparence` (0 à 1), `type_trait`, chaque champ facultatif ;
   - `motifs[]` et `types_traits[]` référencés par nom.
2. **Résolution d'un style**, de la plus faible à la plus forte priorité :
   1. valeur par défaut du code ;
   2. bibliothèque globale (catégorie) ;
   3. surcharge du projet (catégorie) ;
   4. surcharge de vue (gabarit de vue, visibilité / graphismes) ;
   5. matériau de l'élément pour la couleur et les motifs de coupe.

   Une fonction pure `modele::styles::resoudre(bibliotheque, projet, vue, categorie) -> StyleResolu` sert le maillage (`maillage`) et le dessin (`moteur2d`) : un seul point de calcul.
3. **Surcharges locales** : `projet.styles_surcharges[]`, avec la même structure que les catégories de la bibliothèque. Un champ absent hérite de la bibliothèque. Propriétés montre la provenance de chaque valeur (« bibliothèque », « projet », « vue ») et propose de rétablir la valeur héritée.
4. **Synchronisation** :
   - le projet mémorise `styles_source` : chemin et empreinte SHA-256 de la bibliothèque lue (`noyau::hash` existe) ;
   - à l'ouverture, une empreinte différente déclenche un message (n° 16) « Bibliothèque de styles modifiée » avec deux choix : appliquer, ou garder la copie du projet ;
   - le projet garde un instantané des valeurs résolues de la bibliothèque. Il s'affiche donc à l'identique sur un poste sans accès à la bibliothèque, et un export reste reproductible.
5. **Collaboration** : `styles_surcharges` devient une collection identifiée (`commandes::socle::objets::COLLECTIONS`). La bibliothèque elle-même n'entre pas dans les transactions partagées : chaque poste la lit, et l'instantané du projet fait foi.

### Points à décider avant réalisation

- Emplacement partagé de la bibliothèque (serveur de l'agence ou dossier synchronisé), et qui a le droit de la modifier.
- Liste fermée des catégories, alignée sur la boîte de visibilité actuelle (`zones/barre_haute.rs`, catégories de modèle et d'annotation).
- Le matériau prime-t-il sur la catégorie pour la couleur en style « Graphique » ? Proposition : oui en coupe, non en vue.

### Ordre de réalisation

1. `modele::styles` : bibliothèque, surcharges, `resoudre`, tests et schéma dans `docs/MODELE.md`.
2. `maillage` et `moteur2d` : lecture du style résolu.
3. Interface : éditeur de bibliothèque, provenance dans Propriétés, message de synchronisation.

## N° 4 — Compatibilité multiplateforme

### État relevé

- **Seule cible installée et vérifiée** : `x86_64-pc-windows-msvc`. Je n'ai lancé aucune compilation ni aucun test Linux, macOS ou ARM64 : les cibles ne sont pas installées sur ce poste.
- **Code propre à Windows**, toujours avec un repli pour les autres systèmes :

| Emplacement | Usage Windows | Repli ailleurs |
|---|---|---|
| `collaboration/src/partage.rs` | `MoveFileExW` (publication sans remplacer) | `hard_link` puis `remove_file` |
| `collaboration/src/session.rs`, `stockage.rs` | `share_mode(0)` (verrou exclusif) | `File::try_lock` |
| `collaboration/src/cli.rs` | mémoire du processus | (mesure absente) |
| `collaboration/src/partage.rs` | cache dans `LOCALAPPDATA` | `XDG_CACHE_HOME`, puis `~/.cache` |
| `pilote/src/mcp.rs` | processus détaché | `process_group(0)` |
| `interface/src/socle/fenetre.rs` | build de Windows (cadre de fenêtre) | faux |
| `app/build.rs` | icône (`winresource`) | rien |

- **Dépendances** : `eframe` est compilé avec `wgpu`, `wayland` et `x11`, et `rfd` existe sur les trois systèmes. Aucune bibliothèque C ne doit être installée à part.

### Risques

- **macOS** : le cache irait dans `~/.cache` au lieu de `~/Library/Caches` (fonctionnel, mais non conventionnel). Il faut aussi signer et notariser l'application pour la distribuer.
- **Linux** : `rfd` passe par le portail XDG ou GTK, absent sur certains environnements minimaux. Sous Wayland, la décoration de fenêtre personnalisée (`cadre_natif`) reste à vérifier.
- **ARM64** : aucun code `unsafe` dépendant de l'architecture n'a été relevé hors des appels Windows. Le risque principal est la disponibilité des pilotes `wgpu` (Vulkan ou Metal).
- **Verrous de fichiers** sur partage réseau (SMB, NFS) : leur sémantique diffère entre systèmes. La collaboration doit être testée sur chaque système avec un dépôt réseau réel.

### Plan de vérification

1. Ajouter une intégration continue avec une matrice `windows-latest`, `ubuntu-latest` et `macos-latest` (ARM64), qui lance `cargo check --workspace --tests` puis les tests ciblés `noyau`, `modele`, `commandes`, `moteur2d` et `maillage`. Elle demande un dépôt distant : à décider.
2. Faire un essai manuel de l'interface sur chaque système : ouvrir, tracer, exporter un PDF, collaborer à deux postes.
3. Placer le cache dans `~/Library/Caches/BIMedit` sous macOS, avec un `cfg(target_os = "macos")` dans `racine_cache`.

## N° 3 — Version navigateur (WebAssembly)

### Faisabilité

`eframe` et `wgpu` fonctionnent dans un navigateur (WebGPU, ou WebGL2 en repli), et le modèle (`noyau`, `modele`, `commandes`, `moteur2d`, `maillage`) est du Rust pur. Le portage reste un chantier lourd : l'application repose sur le système de fichiers, les fils d'exécution et les processus.

### Obstacles relevés (nombre de fichiers par crate)

| Usage | Crates concernées | Remède |
|---|---|---|
| `std::time::Instant` (panique sur `wasm32-unknown-unknown`) | noyau 1, modele 1, maillage 1, moteur2d 1, formats 2, echanges 1, collaboration 4, interface 6, pilote 1, app 2 | crate `web-time`, même API |
| `std::fs` | noyau 1, modele 4, config 1, formats 2, echanges 3, moteur3d 1, collaboration 4, interface 8, pilote 3, app 2 | couche d'accès aux fichiers : lecture et écriture par téléversement et téléchargement ; stockage OPFS ou IndexedDB |
| `rayon` (parallélisme) | 13 crates, dont modele 13, interface 23 et formats 5 | séquentiel dans le navigateur, ou `wasm-bindgen-rayon` (exige l'isolation d'origine : en-têtes COOP et COEP) |
| `std::thread` | collaboration 4, interface 3, pilote 2, echanges 1, app 2 | tâches asynchrones (`wasm-bindgen-futures`) |
| `std::process`, `std::net` | pilote (MCP, instances), collaboration, interface, echanges, app | hors périmètre web : pilote et CLI restent sur poste |
| `rfd` (dialogues) | interface 13 | API asynchrone de `rfd` (navigateur) : à adapter appel par appel |
| verrous de fichiers (collaboration) | collaboration | serveur de synchronisation (`yrs` est déjà la base des transactions) |

### Périmètre proposé pour une première version web

- **Lecture seule d'abord** : ouvrir un `.modele.json` ou un IFC téléversé, naviguer en 3D, consulter les plans et les coupes, exporter en SVG ou PDF (téléchargement). Crates : `noyau`, `modele`, `scene`, `maillage`, `moteur2d`, `moteur3d`, une partie d'`interface`, `formats` pour l'IFC.
- **Hors périmètre** : import Revit (taille des fichiers, mémoire), pilote MCP, collaboration par fichiers partagés.

### Performances

- La reconstruction de scène et les booléens (`noyau::csg`) tournent aujourd'hui en parallèle. Sans fils d'exécution, il faut s'attendre à un facteur 4 à 8 sur ce poste : c'est une estimation à mesurer, pas une mesure.
- La mémoire d'un onglet est limitée à 4 Go en `wasm32`.

### Étapes

1. Remplacer `std::time::Instant` par `web-time` dans tout le dépôt : changement mécanique, sans effet sur le poste.
2. Créer une couche `config::fichiers` (lire, écrire, choisir un fichier) avec une implémentation poste et une implémentation navigateur.
3. Rendre `rayon` facultatif (fonctionnalité `parallele` activée par défaut).
4. Compiler `modele` et `moteur2d` pour `wasm32-unknown-unknown` en intégration continue.
5. Écrire une application web minimale (`crates/web`) : visionneuse en lecture seule.
