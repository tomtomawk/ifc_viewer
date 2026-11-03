# Collaboration hybride — protocole 1

## État initial et invariants

Base inspectée : f55c88d. Aucun AGENTS.md applicable. Les créations natives utilisent
des compteurs, les commandes ne couvrent pas toutes les mutations, l'historique et
la récupération sérialisent Projet. Les maillages importés et recalés sont des
données sources. Les couches d'assemblage sont des murs distincts. La propagation
des commandes existantes est conservée : on transporte leur résultat, jamais une
commande à rejouer avec les gabarits du destinataire.

Un poste conserve séparément base acceptée, propositions persistantes et projection
de travail. Une transaction est atomique : protocole, projet, UUID, session/auteur,
révision, lectures et empreintes des groupes, changements avant/après, ressources,
réservations et générations. Les groupes géométriques restent indivisibles ; seuls
les champs expressément autorisés peuvent fusionner. Le conflit conserve les trois
états. L'identité ne dépend pas de l'empreinte de version.

## Texte et métier

Yrs : un document par annotation textuelle. La chaîne dans Projet est une projection.
Position, existence, vue et ancrage appartiennent aux transactions métier. Les états
Yrs complets, y compris suppressions, sont persistés. Les mises à jour reçues avant
création sont conservées en attente ; après suppression elles sont archivées sans
résurrection. La création transporte l'état initial dans la même transaction.
L'annulation textuelle utilise les origines locales de Yrs, séparément de la
compensation métier.

## Acceptation et récupération

NAS : contrôle permanent, paquets et ressources immuables par SHA-256, instantanés,
dépôts temporaires. Les caches et queues sont locaux. Déposer n'accepte rien.
Sous Windows, ouverture lecture/écriture du contrôle avec share_mode(0), sans
autorisation de remplacement/suppression. Toutes les décisions et réservations
utilisent ce même handle. Aucune expiration de réservation ; reprise explicite et
génération croissante. Le contrôle est un journal encadré et vérifié ; seule sa
queue incomplète peut être tronquée sous protection. Corruption acceptée : arrêt.
Toute erreur de handle abandonne l'opération ; la tentative suivante relit tout.
Un UUID déjà accepté donne la même décision après perte de l'accusé.

Les ressources sont publiées et vérifiées avant la décision. Un instantané est
référencé par le registre après écriture complète et conserve métier et CRDT.
Pas de purge automatique ; maintenance et rotation explicites, postes déconnectés
à prendre en compte. Les listes significatives gardent leur ordre, les dictionnaires
sont triés. Le chargement partiel peut s'appuyer sur les clés d'objets et empreintes.

## Garanties à qualifier

Exige SMB avec exclusion serveur, refus du remplacement du contrôle ouvert,
flush durable, lectures cohérentes après réouverture et publication atomique dans
un même volume. Désactiver les fichiers hors connexion Windows pour ce dossier.
Pas de promesse NFS ou SMB/NFS mixte. Les essais sur disque local ne qualifient
ni une reconnexion SMB, ni les caches ou la durabilité du NAS réel.

Références API vérifiées :
- https://learn.microsoft.com/en-us/windows/win32/api/fileapi/nf-fileapi-createfilea
- https://docs.rs/yrs/0.28.0/yrs/

La convergence informatique, la conservation des propositions et la validation de
cohérence BIM sont trois garanties distinctes.
