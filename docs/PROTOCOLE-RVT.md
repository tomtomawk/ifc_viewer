# Protocole d'essais : rétro-ingénierie du format Revit

Complète [LISEZMOI-RVT.md](LISEZMOI-RVT.md) (état du décodage). Principe : produire
nous-mêmes des fichiers Revit qui ne diffèrent que d'**une** opération connue, puis
comparer les octets. Revit est piloté par RiveTT ; la comparaison se fait avec
`tools/rvt-research/rvtdiff.py`.

## 0. Avancement (20 octobre 2026, soir)

Session unique dans Revit 2026.5 (build 20260731), pilotée par RiveTT sans
intervention : fichiers dans `C:\Users\theba\Desktop\Revit\Essais-format\2026\`.

| Campagne | État | Résultat principal |
|---|---|---|
| C0 bruit de fond | faite | réenregistrer sans modifier recopie la partition à l'identique, membres KO compris ; seuls compteurs, chemins et horodatages changent |
| C1 fichier minimal | faite | `V0.rvt` : 475 Ko, 2 421 éléments, un niveau ; `ElemTable` entièrement lisible |
| C2 série élémentaire | faite (E01–E10, porte remplacée par une ouverture) | niveau localisé et lu champ par champ (LISEZMOI § 7) |
| C3 2026 / 2027 | faite pour le schéma, sans rouvrir Revit | 98 classes ajoutées, 31 retirées, 55 versions modifiées |
| C4 gabarits | faite | un `.rte` a la même structure qu'un `.rvt` : seuls l'extension, le chemin et les compteurs diffèrent (aucun drapeau dans `BasicFileInfo`) ; un projet créé depuis ce gabarit garde une partition identique octet pour octet, avec un nouveau GUID de document |
| C5 décompression | **sans objet** : blocage résolu par C7 | — |
| C6 corpus naturel | en partie (règle validée sur 7 fichiers, 2016 à 2027) | — |
| C7 croissance de `ElemTable` (ajoutée) | faite | seuil net à 64 Ko compressés → règle des blocs intercalés |
| C11 autres éléments et projets réels (ajoutée le 21/10) | faite | niveaux, types de murs, dalles, instances, pièces extraits (`elements.py`) et identiques à Revit sur `Projet1.rvt` (2026) et `Saint-cast-Le-Guildo.rvt` (2025) ; lecteur étendu aux formats 2022 et 2025 (ids 32 bits jusqu'en 2022) |
| C8 lecteur d'objets | faite | lecteur générique : tous les enregistrements lus sur les fichiers 2016, 2026 et 2027 essayés, sauf 6 données d'add-ins sur 587 082 ; murs droits et courbes extraits, identiques à Revit |
| C14 familles, vues et 2D, écriture (21/10, soir) | faite | famille chargée = document embarqué ; formes des `.rfa` affichées ; vues et éléments 2D lus ; code correcteur décodé ; fichiers écrits par nos outils ouverts par Revit (LISEZMOI § 4 bis, § 7, § 11) |

Le blocage de décompression est levé : le flux est fait de tranches de 64 896
octets de données séparées par des blocs de 353 octets à retirer (LISEZMOI § 4).
La suite du protocole porte donc sur la **lecture des objets** (§ 5).

## 1. Pré-tests du 20 octobre 2026 (sans Revit)

Faits sur des fichiers déjà présents sur le poste. Ils orientent le protocole.

| Test | Résultat | Conséquence |
|---|---|---|
| Lecture OLE par l'API Windows (`StgOpenStorage`) comparée à `cfb.py` (4 flux de `Projet1.rvt`) | octets identiques | le lecteur OLE est hors de cause |
| Membres KO dans un gabarit au **format 2016** (`GABARIT JBA.rte`) | 55 KO sur 362 dans `Partitions/0`, `Formats/Latest` KO | le phénomène n'est pas propre à 2026/2027 |
| Même gabarit 2016 : `Global/ElemTable`, `Global/Latest` | **OK** (138 597 et 64 949 octets) | une table des éléments complète est disponible pour l'étude |
| Fenêtre deflate partagée entre membres consécutifs (`zdict` = sortie du membre précédent) | 0 membre corrigé sur 112 KO | hypothèse écartée |
| Séquence de 2 octets échappée à l'écriture (motif absent des données compressées) | aucun motif absent sur 8,2 Mo | hypothèse écartée |
| Taux de KO selon la taille compressée (1 099 membres, 2 fichiers) | 0 % < 1 Ko, 5 % à 2–4 Ko, 15 % à 8–16 Ko, 33 % à 16–32 Ko, 100 % > 64 Ko | interprété le matin comme un événement aléatoire ; c'était en fait la probabilité qu'un membre chevauche un bloc intercalé (§ 0) |
| Deux sauvegardes successives (`Saint-cast-Le-Guildo.0006` → `.0007`) | 27 flux identiques, une partition **ajoutée** (`Partitions/302`), compteurs incrémentés de 1 dans `Contents`, `History`, `DocumentIncrementTable`, `BasicFileInfo` | `Save` est **incrémental** ; `SaveAs` au contraire réécrit tout dans une seule partition (vérifié ensuite, C7) |

## 2. Règles communes

1. **Une variable par essai.** Chaque fichier diffère de sa référence par une seule
   opération.
2. **Valeurs-sentinelles.** Toute valeur saisie est unique et repérable en binaire :
   textes `ZZ_NIV_A`, `ZZ_MUR_B`… ; longueurs non rondes (12 345,6 mm, pas 5 000).
   `rvtdiff.py --cherche` les cherche en UTF-16, ASCII, double brut et double en
   pieds (unité interne Revit, 1 pied = 304,8 mm).
3. **Étoile et chaîne.** Série en étoile : chaque essai part de la même référence
   (`SaveAs` vers un nouveau fichier). Série en chaîne : `Save` successifs sur le
   même fichier, pour observer l'incrémental.
4. **Témoin RiveTT.** Après chaque opération, relever par RiveTT les ElementId créés
   ou modifiés, le nombre d'éléments, la version et le build. C'est la vérité
   terrain pour `ElemTable` et les partitions.
5. **Copies uniquement.** Aucun fichier de projet réel n'est modifié. Dossier de
   travail : `C:\Users\theba\Desktop\Revit\Essais-format\<campagne>\`.
6. **Fiche par essai** (une ligne dans `journal.csv` de la campagne) : id, version et
   build Revit, opération, script RiveTT, ElementId, fichier, taille, SHA-256,
   nombre de membres OK/KO, flux modifiés, sentinelles trouvées (flux, membre, offset,
   OK/KO).

## 3. Contraintes d'outillage

| Contrainte | Conséquence | Qui |
|---|---|---|
| RiveTT démarre en **lecture seule** à chaque session Revit ; seul un clic sur *Écriture* (onglet Compléments) lève le verrou | regrouper toutes les écritures d'une version dans **une seule session** ; un clic humain par session | Thomas |
| RiveTT se connecte à « la » session Revit active (tube nommé local) | jamais 2026 et 2027 ouverts en même temps | Claude vérifie l'absence de processus `Revit` avant de lancer l'autre version |
| Lancement de Revit | `Start-Process "C:\Program Files\Autodesk\Revit 2027\Revit.exe"`, puis `ping_revit` jusqu'à réponse | Claude |
| Fermeture propre | `close_document` puis sortie de Revit ; la sortie par script (`PostCommand(ExitRevit)`) est **à vérifier**. Pas d'arrêt forcé du processus tant qu'un document est ouvert | Claude, Thomas en secours |
| Revit 2026 installé : build 20260731 ; RiveTT exige 2026.5+ | vérifié le 20/10 : RiveTT 0.6.3 connecté, écriture autorisée | — |
| Avertissements Revit dans un document ouvert par script | une boîte de dialogue bloquerait la session en l'absence de l'utilisateur : `FailureHandlingOptions.SetForcedModalHandling(false)` dans chaque transaction | Claude |
| Ouverture en arrière-plan | `app.OpenDocumentFile` puis `SaveAs` et `Close(false)` : le document actif de l'interface n'est pas touché (série C2 : 10 fichiers en 4 s) | Claude |
| Sauvegarde impossible dans une transaction ouverte | scripts de série en `transactionMode: none`, une transaction par opération | Claude |
| Revit ne réenregistre pas vers une version antérieure | la comparaison 2026/2027 se fait en rejouant le **même script** dans chaque version, pas en convertissant | — |

Session type, par version :

```
Claude : vérifie qu'aucun Revit ne tourne → lance Revit 202X → ping_revit
Thomas : clique Écriture
Claude : get_server_capabilities (writesAllowed, build) → script de campagne
         → contrôle RiveTT de chaque fichier → close_document → sortie de Revit
Claude : rvtdiff.py sur toute la série, remplit journal.csv
```

## 4. Campagnes

Ordre recommandé : C0 → C1 → C2 → C3, puis C4 et C5 selon les résultats. C6 ne
demande pas Revit et peut se faire à tout moment.

### C0 — Bruit de fond et déterminisme (fait en 2026)

Question : qu'est-ce qui change quand le modèle ne change pas ? La compression
est-elle déterministe ?

| Essai | Opération |
|---|---|
| C0-1 | ouvrir V0, `SaveAs` → A |
| C0-2 | même session, `SaveAs` → B (aucune modification) |
| C0-3 | fermer, rouvrir A, `SaveAs` → C |
| C0-4 | `SaveAs` avec `Compact = true` → D |

Attendu : liste des champs qui varient sans modification (compteurs, GUID de
session, horodatages, vignette). **Point décisif** : un membre KO qui se retrouve
octet pour octet identique dans A, B et C signifie que le phénomène KO est une
fonction déterministe du contenu, pas un artefact d'écriture.

### C1 — Fichier minimal (fait en 2026)

Créer le projet le plus pauvre possible :
`app.NewProjectDocument(UnitSystem.Metric)` (sans gabarit), sauvé tel quel → V0.
Idem pour une famille à partir du gabarit de famille le plus simple disponible.

Mesures : taille, liste des flux, nombre de membres et de KO, contenu de
`ElemTable` (nombre d'enregistrements comparé au nombre d'éléments donné par
RiveTT). Objectif : un fichier de l'ordre du mégaoctet dont tous les membres sont
OK ou presque. S'il reste des KO, on tient les plus petits membres KO connus, avec
un contenu presque entièrement prévisible.

### C2 — Série élémentaire (faite en 2026, depuis V0)

Une opération par fichier, en étoile depuis V0. Valeurs-sentinelles imposées :

| Essai | Opération | Sentinelles |
|---|---|---|
| E01 | renommer le niveau existant | `ZZ_NIV_A` |
| E02 | changer l'altitude de ce niveau | 3 333,3 mm |
| E03 | créer un niveau | `ZZ_NIV_B`, 6 789,1 mm |
| E04 | créer un mur droit (type par défaut) | (0 ; 0) → (12 345,6 ; 0) mm, hauteur 2 718,2 mm |
| E05 | E04 puis supprimer le mur | — |
| E06 | paramètre projet « Nom du projet » | `ZZ_PROJET` |
| E07 | dupliquer un type de mur | `ZZ_TYPE_MUR` |
| E08 | créer un matériau | `ZZ_MAT`, couleur RVB (17, 34, 51) |
| E09 | mur E04 + porte (famille chargée) | position 4 321,0 mm |
| E10 | pièce fermée par 4 murs | `ZZ_PIECE` |

Puis une **chaîne** E04 → E09 → E10 avec `Save` sur le même fichier, pour relier
chaque opération à la partition ajoutée.

Exploitation, dans l'ordre :
1. flux modifiés et partition ajoutée (`rvtdiff.py V0 E0x`) ;
2. position des sentinelles ;
3. enregistrement ajouté ou modifié dans `ElemTable`, rapproché de l'ElementId
   donné par RiveTT ;
4. décodage de l'objet autour de la sentinelle avec le schéma `Formats/Latest`
   (classe `Level`, `Wall`…), en commençant par le niveau (E01–E03) : nom et altitude
   sont connus, la structure est la plus simple.

### C3 — Revit 2026 contre 2027

Rejouer **le même script** C1 + C2 dans Revit 2026 (Revit 2027 fermé avant). Un
script RiveTT réutilisable (`reusable: true`) garantit l'identité des opérations.

Comparer : liste des flux, versions des classes dans `Formats/Latest` (champ
`version` de chaque classe), champs ajoutés ou retirés, structure de `ElemTable`,
taux de KO. Attendu : une table des différences de schéma entre les deux formats.

### C4 — Gabarits (.rte)

| Essai | Opération |
|---|---|
| C4-1 | V0 enregistré en `.rte` (`SaveAs` avec l'extension `.rte` : **à vérifier** par l'API) |
| C4-2 | `NewProjectDocument` à partir de ce `.rte`, sauvé en `.rvt` |
| C4-3 | E04 enregistré en `.rte` |

Question : mêmes flux, même découpage `Latest`/`ElemTable`/`Partitions` ? Quels
octets distinguent un gabarit d'un projet (en-tête `BasicFileInfo`, drapeau dans
`Global/Latest`…) ?

### C5 — Décompression (sans objet)

Résolue par C7 (§ 0). Les essais d'injection prévus ici restent utiles pour
**l'écriture** d'un RVT : vérifier si Revit contrôle le CRC gzip et le contenu des
blocs de 353 octets (remplacer un bloc par des zéros sur une copie, puis l'ouvrir).

### C6 — Corpus naturel (sans Revit)

Le poste contient des séries de sauvegardes (`.0001`, `.0002`… : Kerreutv8,
Kerreutv9, Saint-Cast, Colombes, Guidel) et des fichiers de formats 2016 à 2027.
- `rvtdiff.py` sur chaque paire consécutive : statistiques des partitions ajoutées ;
- taux de KO par format (2016 → 2027) pour situer l'apparition du phénomène ;
- `Global/ElemTable` du gabarit 2016 (décodable entièrement) : structure des
  enregistrements de 40 octets.

### C7 — Croissance de `ElemTable` (ajoutée le 20/10)

`V0` + 30 paliers de 500 lignes de modèle (`SaveAs` après chaque palier,
`C7_01` à `C7_30`). Le contenu de `ElemTable` étant prévisible, le premier
enregistrement faux désigne le premier octet compressé faux. Résultat : passage
OK → KO entre 64 737 et 66 866 octets compressés, divergence à l'octet 64 895 du
flux, bloc de 353 octets identifié.

## 5. Suite réadaptée : lire les objets

La décompression ne bloque plus. L'objectif devient de lire un élément complet,
puis tous les éléments d'un membre.

### C8 — Lecteur d'objets (faite le 20/10)

Lecteur générique `tools/rvt-research/champs.py`, piloté par le schéma. Trois
corrections ont suffi pour passer de la lecture d'un niveau à 100 % des
enregistrements :
- tailles des codes 0x02 (`char`, 1 octet) et 0x03 (`short`, 2 octets), établies
  par les classes `std::pair< T1, T2 >` du schéma ;
- corps des pointeurs lus en largeur d'abord, trouvés par une recherche avec
  retour arrière sur l'enregistrement du niveau ;
- champs marqués non enregistrés (bit 0x02 du u16 `ex`, schéma 2027).

Essai ajouté : E11, mur courbe (`ArcWall`, arc de rayon 4 321,0 mm). Validation
des murs contre Revit : script RiveTT réutilisable
`rvt-murs-reference` (extrémités, type, niveaux, paramètres) comparé à
`champs.py --murs`.

### C11 — Autres éléments, projets réels (faite le 21/10)

Méthode : `elements.py --plat id` aplatit un objet décodé en `chemin = valeur` ;
on y repère les champs utiles d'un exemple, puis on compare l'extraction à une
référence exportée par RiveTT (scripts réutilisables `rvt-elements-reference`,
`rvt-reference-complete` : ouverture en arrière-plan, fermeture sans
enregistrer). Les écarts du projet réel ont révélé quatre règles : hauteur des
murs contraints, groupes non placés, pièces non placées et surfaces portées par
`RoomElem`, position des instances hébergées (LISEZMOI § 7).

### C12 — Toits, escaliers, grilles, matériaux ; portage Rust (faite le 21/10)

- Repérage des classes par le schéma seul : inventaire des objets courants par
  chaîne d'héritage (`ProfileRoof < RoofBase`, `StairsElement`, `StairsRun <
  StairsRunAndLanding`, `Grid < DatumPlane`, `MaterialElem`, `Family`,
  `FamilySymbol`), puis `elements.plat` sur un exemple de chaque.
- Corpus utilisé (formats 2025 à 2027) : `Projet1.rvt`, `Kerreut 2025.rvt`,
  `Langan.rvt` (toits), `Saint-Malo…rvt` et `Phase2_Lecture.rvt` (escaliers
  hélicoïdaux, gaines). Aucune grille dans le corpus : essai **E12_grilles.rvt**
  créé depuis V0 par le script RiveTT `rvt-essai-e12-grilles` (grille A droite
  (0 ; 0) → (12 345,6 ; 0), B2 oblique (1 000 ; −2 000) → (4 000 ; 7 000), Courbe
  en arc de centre (5 000 ; 3 000) et rayon 4 000 mm, de 0 à 90°).
- Références Revit : script RiveTT réutilisable `rvt-reference-elements`
  (ids BuiltInParameter, matériaux, sols, plafonds, toits, escaliers, grilles,
  pièces, catégories et exemples d'instances, volumes des solides) ;
  `rvt-reference-gaine` pour les solides vides (`InstanceVoidCutUtils`).
- Surfaces réglées et splines d'Hermite : paramétrage choisi par l'écart des arêtes
  partagées avec des faces déjà décodées (variantes comparées, LISEZMOI § 7).
- Résultats reportés dans LISEZMOI § 7 et § 8 ; valeurs de référence reprises dans
  les tests de `src/rvt/maquette.rs`.

### C13 — Géométrie des familles (faite le 21/10)

- Structure du `GInstance` lue sur une porte de `Projet1.rvt`, puis règles des
  `GFilter` établies en cherchant le sous-ensemble de solides du type dont la somme
  égale le volume donné par Revit (porte 257 051 : 0,145 647 contre 0,145 65 m³ ;
  fenêtre 257 071 : objets invisibles, bits 0xE0 de `GInfo.m_flags`).
- Références Revit : scripts RiveTT `rvt-reference-familles` (réutilisable : volume
  des solides directs et des instances, boîte, `GetTotalTransform`, miroir, pour
  toutes les instances de modèle), `rvt-reference-solides` (volume solide par
  solide), `rvt-reference-garde-corps`, `rvt-reference-cotes-portes` (paramètres
  d'instance et de type).
- Convention du repère tranchée par `GetTotalTransform` (214 / 214) : la comparaison
  des boîtes ne suffisait pas, celle de Revit comptant aussi les lignes 2D.
- Paramétrage de `SurfRev` choisi par l'écart des arêtes (nul contre 204 mm).
- Facettisation des faces courbes comparée sur l'arbre de Kerreut (−51 % au départ) :
  découpe des arêtes au milieu (80 000 à 200 000 triangles, mauvaise convergence),
  puis grille intérieure et bascules de Delaunay retenue (−0,36 %, 4 300 triangles).
- Cotes des portes : la largeur et la hauteur annoncées sont celles du panneau
  (paramètres de type `FAMILY_WIDTH_PARAM`, `FAMILY_HEIGHT_PARAM`), relevées dans
  `FamilySymbol.m_pParams` (remarque de l'utilisateur, vérifiée contre l'API).
- Capture d'écran du viewer (fenêtre OpenGL) impossible depuis PowerShell
  (`CopyFromScreen`, `PrintWindow` : fenêtre blanche, y compris avec un IFC) :
  l'affichage reste à vérifier à l'écran.

### C14 — Familles, vues et 2D, écriture (faite le 21/10, soir)

Session Revit 2026 ouverte par l'utilisateur sur `FEN_2PF.rfa`, écriture autorisée ;
fichiers dans `Essais-format\2026\C14\`.

1. **Famille seule** : inventaire de `FEN_2PF.rfa` (19 extrusions, 7 combinaisons,
   un mur hôte) ; référence Revit `rvt-reference-formes-famille` (forme par forme :
   pleine ou vide, sous-catégorie, visibilité, membres des combinaisons, volume).
2. **Projet vide → famille chargée → placée** (`rvt-essais-c14-famille`) : F00 à F04
   en une session, chaque étape enregistrée à part ; comparaison des enregistrements
   (id, classe, Adler-32) entre fichiers successifs. Résultats : LISEZMOI § 7,
   « Familles chargées dans un projet ».
3. **Vues et 2D** : essai E13 créé depuis F02 (`rvt-essai-e13-vues2d`) avec des
   sentinelles (ligne (1 000 ; −1 000) → (3 456,7 ; −1 000), arc de rayon 765,4 mm,
   texte `ZZ_TEXTE_PLAN`, vue de dessin `ZZ_DESSIN`, coupe `ZZ_COUPE`, feuille
   `ZZ_FEUILLE`) ; référence `rvt-reference-vues-2d` (vues et éléments propres à une
   vue, avec ou sans détail) sur E13, `FEN_2PF.rfa` et `Projet1.rvt`.
4. **Écriture** : W0 (conteneur réécrit) puis W1 (CRC à zéro, refusé) ont montré que
   le bloc de 353 octets est contrôlé ; décodage du code correcteur (boîte noire,
   puis lecture du code machine de `Utility.dll` pour l'interopérabilité),
   réimplémenté dans `crcbloc.py` et vérifié par réencodage identique ; W2b, W4, W5,
   W6 ouverts par `rvt-essai-ouverture` (intercepte les boîtes de dialogue) et
   relus par l'API ; `rvt-reference-types-famille` régénère chaque type de la
   famille et en donne le volume et la boîte.

5. **Hachures** (même session, à la demande de l'utilisateur) : motifs, types de
   régions et motifs des matériaux décodés par le schéma, puis comparés en entier aux
   références `rvt-reference-hachures` et `rvt-reference-motifs-materiaux` (résultat
   gardé par RiveTT dans un fichier, comparaison automatique) sur E13 et Kerreut.

Précautions : documents ouverts en arrière-plan et fermés sans enregistrer ; le
fichier de l'utilisateur ouvert dans l'interface n'est jamais modifié ; les essais
qui pouvaient échouer (W1) passés à la fin, un par appel.

### C15 — Topographie (faite le 21/10, soir)

Recherche des classes de topographie dans le schéma (`SiteSurface`, `Toposolid`,
`ToposolidType`, `BuildingPad`, `SiteContourElem`, `FacetedTopology*`), puis de leurs
instances dans tout le corpus (42 fichiers, comptage par le lecteur Rust) :
toposolides dans Saint-Malo et son ancienne version, deux `SiteSurface` dans
`LOT2.rvt`, membres d'un groupe non placé. Pour la `SiteSurface`, essai E15 créé
dans Revit 2026 (surface de 42 points et sous-région, script
`rvt-essai-e15-surface-topo`), référence `rvt-reference-toposurfaces`. Référence
Revit par le script RiveTT `rvt-reference-toposolides` (document ouvert en
arrière-plan, fermé sans enregistrer), comparée automatiquement au décodage
(`geometrie.py`, puis le lecteur Rust). Travail fait sur la branche `topographie`
(worktree séparé), en parallèle d'une autre session.

### C16 — Paramètres et identifiants (faite le 21/10, nuit)

Références RiveTT (Revit 2026, documents ouverts en arrière-plan et fermés sans
enregistrer) : `rvt-table-parametres-integres` (3 740 valeurs de `BuiltInParameter`,
libellé, stockage), `rvt-reference-parametres-projet` (Kerreut 2025 et Projet1 :
46 298 valeurs de paramètres modifiables avec leur définition),
`rvt-reference-parametres-famille` (`FamilyManager` de `FEN_2PF.rfa`, 35 paramètres,
6 types), `rvt-reference-groupes-uniqueid` (libellés des groupes et des unités,
UniqueId et IfcGUID). Décodage en Python (`parametres.py`), correspondances
paramètre → champ apprises sur les références et validées en croisé
(`parametres_apprendre.py`), règle de l'UniqueId (`uniqueid.py`), portage Rust
comparé à la référence par `ifc_viewer --parametres` et un script de dépouillement.
Contrôle visuel par captures du viewer piloté par son canal de commande.

### C9 — Variantes ciblées (Revit, prochaine session)

Quand le lecteur bute sur un champ, produire deux fichiers qui ne diffèrent que
par ce champ. Exemples : niveau avec et sans vue associée ; mur droit puis mur
courbe (même longueur) ; mur sans puis avec contrainte supérieure.

### C10 — Revit 2027 (prochaine session, après redémarrage)

Rejouer les scripts `rvt-essais-c0-c1` et `rvt-essais-c2-etoile` (enregistrés par
RiveTT) dans Revit 2027, Revit 2026 fermé. Comparer les enregistrements d'objets,
pas seulement le schéma.

## 6. Critères d'arrêt et livrables

| Campagne | Réussie si |
|---|---|
| C0 | champs variables identifiés ; déterminisme des KO tranché |
| C1 | fichier minimal < 5 Mo avec la liste exhaustive de ses flux |
| C2 | pour E01–E04 : flux, partition et enregistrement `ElemTable` identifiés ; niveau décodé (nom + altitude) |
| C3 | table des différences de schéma 2026/2027 |
| C4 | octets distinctifs gabarit/projet identifiés |
| C5 | atteint par C7 : tous les membres décodés avec CRC exact |
| C8 | atteint : niveau et mur lus en entier ; 100 % des enregistrements sur 5 fichiers |
| C12 | atteint : toits, escaliers, grilles et matériaux identiques à Revit ; volumes des sols, toits, escaliers et pièces identiques à 10⁻⁵ m³ |
| C13 | atteint : 373 instances sur 448 de volume identique à Revit, 404 à moins de 1 % ; garde-corps 16 / 16 |
| C15 | atteint : surface topographique et sous-région d'E15 identiques à Revit (points, sommets, triangles, aire, boîte) ; 40 toposolides de Saint-Malo lus et maillés, nombre de faces, type, niveau, décalage et boîte identiques pour les 40 ; volume et aire identiques à 10⁻⁵ près pour 37, à 4·10⁻⁴ au plus pour les 3 autres (faces cylindriques facettées : bordures courbes) ; total 45 414,805 m³ contre 45 414,807 m³ |
| C14 | atteint : 11 formes de `FEN_2PF.rfa` sur 13 de volume identique à Revit, le coffre à −0,35 %, la combinaison évidée sans solide comme dans Revit ; motifs, types de régions et motifs des matériaux identiques à Revit ; vues et éléments 2D identiques sur E13, la famille et `Projet1.rvt` ; flux réencodés à l'identique ; quatre fichiers modifiés acceptés par Revit |

Après chaque campagne : résultats reportés dans `LISEZMOI-RVT.md` (faits observés,
hypothèses écartées), scripts RiveTT conservés, fichiers d'essai hors dépôt.

## 7. Outils

| Outil | Rôle |
|---|---|
| `tools/rvt-research/rvtdiff.py` | différence entre deux fichiers : flux, membres, premier octet différent, sentinelles |
| `tools/rvt-research/extract.py` | extraction et contrôle OK/KO |
| `tools/rvt-research/schema.py` | schéma de classes |
| `tools/rvt-research/inflate_ref.py` | décodeur deflate instrumentable |
| `tools/rvt-research/crcbloc.py` | code correcteur : réencodage et contrôle des flux |
| `tools/rvt-research/ecriture.py` | réécriture d'un fichier (champ, texte, essais) |
| `tools/rvt-research/vues.py` | vues et éléments 2D |

```bash
python -I tools/rvt-research/rvtdiff.py V0.rvt E04.rvt --cherche ZZ_MUR_B 12345.6 2718.2
```
