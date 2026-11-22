# `interface/src/projet/`

Rôle : projet partagé et contextes.

| Fichier | Contenu |
|---|---|
| `collaboration.rs` | parcours Collaborer / Ouvrir un projet partagé, commandes du ruban Sortie. Ce module local masque la crate `collaboration` : écrire `::collaboration::…` pour la crate |
| `conflits.rs` | libellés des éléments et boutons de résolution communs au bandeau et à la fenêtre Détail, comparaison des champs et sélection des éléments |
| `collaboration_pilote.rs` | réponses du pilote : état résumé, clés à réserver, identifiant de transaction par préfixe (fonctions pures, testées) |
| `gestion_contextes.rs` | phases, sous-projets, variantes |

Dépendances typiques : `socle/documents.rs`, `zones/ruban/`.

Vérifier : `cargo test -p interface --release`.
