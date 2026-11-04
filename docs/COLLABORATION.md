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

## Fonctionnalité raccordée

Le ruban **Début → Collaborer** enregistre un projet partagé sur le serveur et
prépare automatiquement la copie locale. **Sortie → Synchro** publie et reçoit les
modifications. **Sortie → Gérer** regroupe réservations, libération, reprise explicite,
instantanés et récupération des textes supprimés. La barre inférieure de collaboration
et l'ancienne barre d'état sont supprimées ; l'état partagé reste visible dans le
ruban et les erreurs sont présentées dans une fenêtre dédiée. Les propriétaires sont les UUID de session (l'auteur figure également
dans les transactions). Une synchronisation est une demande explicite exécutée sur
un thread de travail ; elle n'est pas lancée à chaque frappe. La contention initiale
est réessayée pendant deux secondes avec de nouvelles acquisitions. Une erreur après
acquisition interrompt l'opération : aucune écriture ne reprend avec un handle invalidé.

Les mutations d'interface et du pilote passent par appliquer_projet, après recalage
des menuiseries et validation. La mémorisation persistante des vues passe par la même
session transactionnelle. Les imports de familles et leur propagation sont raccordés.
`--editer` reconnaît une copie associée et utilise cette même frontière. Un projet
standalone conserve son historique antérieur ; en collaboration Ctrl+Z/Ctrl+Maj+Z
produisent des compensations ciblées, y compris pour une action déjà acceptée.
Échap pendant une saisie reste un abandon de saisie sans transaction.

Les textes seuls changent la projection et la révision des caches de dessin, sans
reconstruction 3D ni préparation de tampons GPU. Une réception attend la fin de la
saisie active. Les piles UndoManager Yrs et le GUID du document sont sauvegardés dans
la copie locale : l'annulation des opérations locales survit au redémarrage et ne
supprime pas le texte du collègue. Ces piles personnelles ne sont pas partagées.

Un conflit retient toute la transaction. Le dialogue montre les trois valeurs et la
transaction complète. L'utilisateur peut conserver l'état accepté ou choisir la
proposition locale comme nouvelle transaction avec de nouvelles préconditions. La
proposition antérieure est archivée localement avant résolution. Une compensation
explicite peut restaurer une annotation supprimée ; une mise à jour Yrs seule ne peut
jamais le faire. Les textes supprimés restent consultables et copiables dans **Sortie → Gérer**.

Les références graphiques du gabarit (plumes, motifs, matériaux nécessaires) sont
figées à l'association. Les fichiers natifs passent au schéma 1. La migration 0→1
préserve les identités d'éléments, attribue celles du projet et des objets nommés,
et est stable indépendamment de l'indentation du JSON. Les cotes et étiquettes avec
un hôte inexistant sont désormais refusées : laisser une référence orpheline n'est
pas une politique de résolution collaborative.

## Démarrer deux postes

1. Ouvrir ou créer le projet natif, puis cliquer sur **Début → Collaborer**.
2. Dans le dialogue d'enregistrement, choisir le serveur et un nom, par exemple
   `\\NAS\BIMedit\Maison.bimpartage`.
3. BIMedit écrit l'état complet, ressources et CRDT compris, dans le dossier voisin
   `Maison.collaboration`, puis publie le fichier `.bimpartage` en dernier. Il bascule
   automatiquement la session sur une copie locale. Le fichier d'origine est conservé.
4. Dans l'autre instance, utiliser **Ouvrir** et sélectionner le même fichier
   `Maison.bimpartage`. Aucune copie manuelle ni association de dossier n'est nécessaire.
5. Travailler puis utiliser **Sortie → Synchro**. Après la publication d'un collègue,
   cliquer également sur Synchro pour recevoir ses modifications.

Le fichier `.bimpartage` est un point d'entrée identifiable par son extension et son
format `bimedit-projet-partage`. Il contient l'identité du projet et le nom relatif
**d'un dossier voisin**, pas un JSON de modèle écrasé à chaque sauvegarde. Les ouvertures
lisent l'état accepté actuel du registre et de ses instantanés. Déplacer ou sauvegarder
le fichier partagé et son dossier ensemble ; conserver une seule autorité active.

Les copies sont gérées dans `%LOCALAPPDATA%\BIMedit\Collaboration` sous Windows.
Chaque copie a sa propre session et son verrou ; deux instances sur le même ordinateur
ouvrant le même point d'entrée reçoivent des copies distinctes si la première est occupée.
Une copie disponible est réutilisée à l'ouverture suivante, avec ses propositions et son
historique. La réouverture reçoit les nouveautés et réapplique les propositions locales,
**sans les publier** : cette publication attend un clic sur Synchro.

Une entrée connue et sa copie locale restent ouvrables hors connexion. Le point d'entrée
est également mis en cache localement. Sans copie existante, une première ouverture exige
le serveur. Un dépôt disparu ou un contrôle absent n'est jamais réinitialisé par l'ouverture
ou la synchronisation d'un `.bimpartage`. Une entrée incompatible, un autre projet dans le
dossier ou une révision centrale inférieure à celle connue sont diagnostiqués.

Les réservations sont dans **Sortie → Gérer** : réserver avant une édition sensible,
synchroniser les modifications, puis libérer la sélection. Elles n'expirent pas.
« Enregistré localement — en attente » ne signifie jamais « publié ».

Si les pilotes locaux sont utilisés sur le même ordinateur, attribuer BIMEDIT_PORT=47811
et BIMEDIT_PORT=47812 aux instances. Ils restent sans rôle dans le protocole NAS.
Une copie est verrouillée jusqu'à la fin d'une synchronisation, même après fermeture
du document. Enregistrer sous est désactivé pour une copie collaborative.

L'environnement `BIMEDIT_CACHE_COLLABORATION` permet de définir une racine locale
spécifique pour les essais. Ne pas supprimer ce cache si des propositions sont en attente.
Le code utilise le mode synchronisé de Yrs pour transférer la session préparée par le
thread de chargement à l'interface, sans relâcher son verrou local entre les deux.

Les commandes suivantes restent disponibles pour l'administration et les essais du
protocole sur des copies locales explicitement désignées ; le parcours normal passe
par Collaborer et Ouvrir.

Équivalent console (utiliser `cargo run --release --` comme préfixe sous PowerShell,
ou Start-Process -Wait, car le binaire Windows release est un exécutable graphique) :

```text
--collaboration associer C:\PosteA\maison.json \\NAS\BIMedit\Projet
--editer C:\PosteA\maison.json C:\PosteA\commande.json
--collaboration synchroniser C:\PosteA\maison.json
--collaboration etat C:\PosteA\maison.json
--collaboration reserver C:\PosteA\maison.json murs/IDENTIFIANT
--collaboration reprendre C:\PosteA\maison.json murs/IDENTIFIANT
--collaboration liberer C:\PosteA\maison.json murs/IDENTIFIANT
--collaboration instantane C:\PosteA\maison.json
```

Les noms de groupes exacts sont les clés collection/identifiant des transactions.
La reprise révoque la génération précédente ; elle ne republie pas automatiquement
la proposition de l'ancien détenteur.

## Fichiers locaux et récupération

À côté de `maison.json`, conserver ensemble :

- `maison.collaboration.json` : base, queue, conflits et historique personnel ;
- `maison.collaboration.ressources-locales/` : ressources par empreinte ;
- `maison.collaboration.prepare-UUID.json` : contenu exact préparé avant publication ;
- `maison.collaboration.conflit-UUID.json` : propositions archivées explicitement ;
- `maison.collaboration.lock` : fichier permanent de protection de cette copie.

Le JSON natif sauvegardé est une projection exportable. Il ne suffit pas à reprendre
la synchronisation textuelle ; les fichiers ci-dessus portent les états Yrs et les
propositions encore locales. Le stockage lourd des sidecars, paquets et instantanés
est externalisé ; le JSON natif autonome conserve ses données sources intégrées.

Après arrêt brutal, rouvrir la même copie. Le sidecar restaure l'overlay même si le
JSON natif date d'avant les dernières modifications. Demander une synchronisation :
le registre est relu sous exclusion ; une queue terminale incomplète est tronquée,
les paquets orphelins ne sont pas acceptés, les UUID connus sont comparés au contenu
préparé. Une acceptation sans accusé ne crée donc pas une deuxième opération.

Pour extraire sans NAS une projection ou un texte supprimé :

```text
--collaboration recuperer C:\PosteA\maison.json C:\Recuperation\maison.json
--collaboration texte-recuperer C:\PosteA\maison.json ANNOTATION C:\Recuperation\texte.txt
```

En cas de corruption d'un enregistrement complet, d'une empreinte ou d'une ressource
requise : arrêter les publications et conserver tous les fichiers pour diagnostic.
Ne pas supprimer controle, le remplacer ou sauter arbitrairement un enregistrement.
Une erreur de repère d'instantané est également traitée en arrêt, pas en succès dégradé.

Sauvegarde cohérente explicite, sous autorité unique :

```text
--collaboration sauvegarde \\NAS\BIMedit\Projet D:\Sauvegardes\Projet-neuf
--collaboration auditer D:\Sauvegardes\Projet-neuf
```

La destination doit être neuve. La sauvegarde copie le registre par son handle et
vérifie les empreintes de toutes les ressources, paquets et instantanés copiés. Pour
restaurer, arrêter tous les postes, préserver le dépôt endommagé, remettre **l'ensemble**
de la sauvegarde au chemin partagé initial, puis rouvrir et synchroniser les copies.
Ne jamais reconstruire le dépôt central depuis un simple JSON exporté : cela perdrait
les tombstones, les décisions et les identités CRDT. Une copie connaissant une révision
acceptée refuse de réinitialiser silencieusement un registre central devenu vide.

## Fusion, performances et croissance

L'index dérivé observe les références (y compris les clés des cartes), utilisateurs
et versions de leurs collections. Il couvre aussi les références nommées aux matériaux,
motifs et cartouches. La précondition spatiale est volontairement globale pour une
transaction géométrique : deux géométries éloignées peuvent donc être refusées.
Seuls `nom` et `type_vue` d'une vue sont déclarés indépendants entre eux. Les ajouts
en fin d'une liste significative peuvent fusionner ; les réordonnancements concurrents
restent conflictuels. Aucun champ BIM ne reçoit un arbitrage silencieux par date.

Une édition géométrique conserve la reconstruction de scène existante. Le format
indexe les clés touchées pour permettre une invalidation plus fine ultérieure ; le
chargement partiel n'est pas implémenté. Les comparaisons utilisent encore une
projection complète en mémoire. Les petits essais ci-dessous ne garantissent pas
la latence sur une grande maquette.

Le repère d'instantané permet de charger l'état puis le suffixe de journal, en
vérifiant l'empreinte du préfixe. La lecture du préfixe reste linéaire en octets,
et la table des UUID acceptés, les générations, les tombstones et les états Yrs
continuent à croître. Les mises à jour brutes préservent aussi les dépendances Yrs
arrivées dans le désordre. Aucun nettoyage concurrent ni purge des postes absents.

Maintenance : produire des instantanés et des sauvegardes, surveiller la taille du
registre et des historiques personnels. Une rotation physique exige une fenêtre
d'arrêt de tous les postes, un état complet conservant UUID/réservations/CRDT et un
chemin d'autorité unique après bascule. Le logiciel ne propose pas encore de commande
de compactage du registre : ne pas remplacer le contrôle à la main sur un dépôt actif.

## Vérification et mesures

Commandes reproductibles :

```powershell
cargo test --release
cargo build --release
./tools/qualifier_collaboration.ps1
python tools/test_collaboration_ui.py
python tools/test_partage_ui.py
cargo run --release -- --collaboration mesures samples/maison_native.modele.json rapport.json
```

Les tests Rust couvrent les créations sans collision, fusions et conflits, suppression
d'hôte pendant ajout d'ouverture, assemblages et familles, générations révoquées,
processus en course, arrêts forcés à huit frontières de dépôt/registre, doublons,
texte reçu avant création ou après suppression, Unicode, annulation locale Yrs et
restauration de sa pile, réponse perdue, reconnexion et compensation après intervention
du collègue, instantané concurrent, ressources manquantes/corrompues et protocoles.
Les tests enfants sont de vrais processus, pas des mocks de système de fichiers.

Le script PowerShell 7 vérifie deux copies, deux processus de publication, convergence,
instantané et audit d'une sauvegarde restaurable. Le script Python lance deux vraies
applications wgpu avec des pilotes locaux distincts et coédite une annotation.

Rapports conservés : [mesures locales](collaboration-mesures-locales.json) et
[mesures de deux instances graphiques](collaboration-mesures-ui.json).
Sur l'échantillon maison (2 332 sommets), en release sur disque local : comparaison
0,42 ms ; validation et commit local 14,11 ms ; reconstruction 2,64 ms ; mémoire
résidente du processus de mesure 13,6 Mo ; ouverture du journal 6,33 ms ; ouverture
par instantané 5,70 ms. Initialisation et publication instrumentées : 7 429 octets
écrits, 6 950 lus, 5 écritures logiques, 3 lectures, exclusion 30,88 ms.

Dans les deux applications : latence aller-retour pilote pour une édition 11–33 ms,
comparaison/validation locale 4,4–4,9 ms, reconstruction 1,91–2,00 ms et préparation
des tampons GPU côté CPU 0,049–0,084 ms. Pour les éditions textuelles : géométrie et
préparation GPU nulles ; aller-retour pilote 5–21 ms. Ces durées incluent parfois la
planification des images ; ce ne sont ni une mesure clavier-à-photon ni un timer GPU.
Les compteurs de fichiers portent sur les opérations logiques instrumentées, pas
sur les métadonnées, les appels système complets ou les paquets réseau SMB.

Contrôles exécutés : suite release complète (205 tests applicatifs et 4 tests du
pilote réussis ; 1 test préexistant ignoré), puis 23 tests collaboratifs ciblés après
les derniers renforcements de préconditions ; build release ; statistiques IFC ;
exports IFC, PDF et CSV ; scripts de qualification locale et de deux instances UI.
Un contrôle supplémentaire des 105 tests du modèle a passé après le remplacement
des identités dérivées des couches natives. La validation indépendante avec
IfcOpenShell 0.9.0.post1 a produit 15 géométries et 0 erreur.
Les tests de conversion dont les données externes sont absentes conservent les
conditions d'exécution préexistantes du dépôt.

## Qualification NAS à deux machines (reste à réaliser)

Utiliser un dossier de qualification dédié et enregistrer modèle NAS, firmware,
version SMB, options de cache, fichiers hors connexion désactivés et antivirus.
Vérifier les droits permettant création, écriture, flush et renommage dans le volume.

1. Exécuter le script PowerShell avec `-DepotParent \\NAS\Qualification` et conserver
   son résultat. Cela teste un client SMB ; cela ne remplace pas les étapes suivantes.
2. Sur A, créer le projet partagé avec Début → Collaborer ; sur B, ouvrir son .bimpartage.
   Synchroniser, créer simultanément, puis publier les deux et recevoir après barrière.
3. Réserver le même groupe simultanément depuis A et B : un seul propriétaire. Avec
   le handle de contrôle ouvert sur A, tenter depuis B une autre ouverture exclusive,
   la suppression et le renommage de controle : les trois doivent être refusés.
4. Reprendre explicitement depuis B, puis publier la proposition déjà préparée de A :
   génération ancienne refusée, proposition locale conservée. Aucun délai d'attente
   ne doit libérer à lui seul une réservation.
5. Rejouer les cas d'hôte/ouverture, propagation et coédition textuelle des tests.
   Tester Échap et une réception pendant une saisie active ; la saisie doit rester intacte.
6. Couper le réseau de A pendant dépôt de ressource, publication du paquet, écriture
   du registre et juste après acceptation. Reconnecter et redemander une sync : nouvelle
   acquisition et relecture, résultat entier ou refus conservé, jamais deux acceptations.
7. Arrêter brutalement chaque client, puis le NAS selon sa procédure de test autorisée.
   Contrôler la durabilité annoncée pour sync_all/SMB FLUSH et la queue récupérée.
8. Publier pendant un instantané, restaurer une sauvegarde sur un autre partage de
   qualification, reprendre les postes déconnectés et vérifier contenu métier, textes,
   tombstones, UUID et générations. Altérer une copie de paquet/ressource : arrêt diagnostiqué.
9. Mesurer sur une vraie maquette, séparément latences locales, comparaison, géométrie,
   préparation GPU, mémoire, octets/opérations avec trace système, exclusion centrale,
   ouverture et rattrapage. Conserver les journaux des deux machines et du NAS.

La réussite locale fournit des preuves sur le code et NTFS. La disponibilité des
handles, les caches, la reconnexion et la durabilité physique du partage réel restent
à qualifier. Aucun résultat fourni ici ne qualifie NFS ou un accès mixte SMB/NFS.

## Vérification du parcours automatique

`tools/test_partage_ui.py` lance deux vraies applications sur le même fichier partagé,
avec une racine de cache commune. Il vérifie la création du dossier voisin, deux copies
locales distinctes, la publication et la réception, le redémarrage avec une proposition
non publiée et sa publication uniquement à la demande. Il capture les rubans Début et
Sortie et vérifie que le fichier source et le point d'entrée n'ont pas été écrasés.
Les tests Rust vérifient également la réutilisation d'une copie, le mode hors connexion,
le refus d'une entrée incompatible ou d'une traversée de chemin et l'absence de recréation
d'un contrôle central disparu. Une création interrompue avant publication du point
d'entrée peut laisser un dossier de dépôt à diagnostiquer ; elle n'expose pas un projet
partagé partiellement écrit comme une création réussie.

Validation finale du parcours : `cargo test --release` — 211 tests applicatifs et
4 tests du pilote réussis, 1 test préexistant ignoré ; build release sans avertissement.
`tools/test_partage_ui.py` a passé avec deux applications graphiques, réouverture
du cache contenant une proposition non publiée et convergence après Synchro.
