# Format Revit (.rvt) : état du décodage

Notes de rétro-ingénierie du 19–21 octobre 2026 (campagne C14 du 21 au soir :
familles, vues et 2D, écriture). Elles décrivent ce qui a été
**observé** sur des fichiers réels, pas une spécification : rien n'est documenté
par Autodesk. Les points non vérifiés sont signalés comme tels. Méthode d'essai :
[PROTOCOLE-RVT.md](PROTOCOLE-RVT.md).

## En bref

| Partie | État |
|---|---|
| Conteneur OLE (CFB) | lu entièrement, validé par l'API Windows |
| Métadonnées (version, projet, références, vignette) | décodées, intégrées au viewer (`src/rvt.rs`) |
| Décompression | **résolue** (§ 4) : blocs de 353 octets intercalés tous les 64 896 octets ; 100 % des membres gzip décodés avec CRC exact (formats 2016, 2026, 2027) |
| Code correcteur (blocs de 353 octets, fin des flux) | **décodé** (§ 4 bis) : CRC courts entrelacés, réencodage identique octet pour octet de tous les flux essayés (2026, 2027) |
| Schéma de classes `Formats/Latest` | **lu en entier** : 4 690 classes et 12 558 champs (2026), 4 757 et 13 080 (2027) ; identifiants de classe établis |
| Table des éléments `Global/ElemTable` | structure établie (en-tête, enregistrements de 40 octets) ; un champ non identifié |
| Partitions `Partitions/*` | **cadrage complet** : 3 sections, un enregistrement par élément ; inventaire identique à Revit (§ 7) |
| Champs des objets | **lus en entier** par un lecteur générique piloté par le schéma : 100 % des enregistrements sur les fichiers 2016, 2022, 2025, 2026 et 2027 essayés, hors données d'add-ins (§ 7) |
| Murs | type, niveaux, ligne (droite ou arc), hauteur et décalages extraits ; identiques à Revit sur 75 murs de deux projets et deux essais (§ 7) |
| Autres éléments | niveaux, types (nom, épaisseur), familles (nom, catégorie), matériaux (nom, couleur, transparence), sols et plafonds, toposolides et surfaces topographiques (terrain), toits, escaliers (niveaux, contremarches, giron, volées), grilles, instances de familles (type, hôte, position, repère), pièces : identiques à Revit sur trois projets et un essai (§ 7) |
| Géométrie calculée (`GElement`) | B-rep lu et maillé pour murs, sols, plafonds, toits, escaliers, garde-corps, pièces et **familles** (faces planes, cylindriques, réglées et de révolution, trous ; vides écartés) ; volumes identiques à Revit pour 59 murs, 21 sols sur 22, 10 toits, 4 escaliers, 16 garde-corps et 59 pièces (§ 7) |
| Familles (`GInstance`) | géométrie du type placée par le repère de l'instance, filtrée comme une vue 3D en détail fin ; 373 instances sur 448 de volume identique à Revit, 404 à moins de 1 % (§ 7, « Familles ») |
| Documents de famille (`.rfa`) | extrusions, raccords, révolutions, balayages et combinaisons lus et affichés ; volumes identiques à Revit (§ 7, « Documents de famille ») |
| Familles chargées dans un projet | document embarqué dans la partition, ids renumérotés, désigné par `Family.m_oFamDoc` (§ 7) |
| Vues et éléments 2D | vues (genre, échelle, niveau, repère, cadrage), traits, arcs, textes, cotes, régions remplies, fenêtres de vue : identiques à Revit sur trois fichiers (§ 7, « Vues et éléments 2D ») |
| Feuilles | fenêtres de vue placées et découpées comme dans Revit (centre et boîte de l'API exacts quand le contour est le cadrage), cartouches et symboles placés par leur repère (156 instances de vue sur 156 identiques à `GetTransform`), étiquettes remplies par les paramètres de la feuille, du projet et de la révision actuelle, titres des fenêtres de vue (§ 7, « Feuilles ») |
| Hachures | motifs de remplissage (grilles, tirets), types de régions remplies et motifs des matériaux identiques à Revit (219 motifs et 80 matériaux de Kerreut) ; régions tracées unies ou hachurées dans le viewer ; **motifs de surface des matériaux dessinés sur les faces 3D**, calés face par face comme dans Revit (§ 7, « Hachures ») |
| **Écriture** | fichier modifié par nos outils (texte, couleur de matériau, largeur d'un type) **ouvert par Revit 2026**, valeurs relues par l'API (§ 11) |
| Viewer Rust | à l'ouverture d'un `.rvt` ou d'un `.rfa`, sans export IFC (`src/rvt/`, § 8) : **murs, sols, plafonds, toits, escaliers, garde-corps, portes, fenêtres, mobilier et autres familles, pièces, formes de famille affichés**, colorés par matériau ; vues et éléments 2D (masqués par défaut), textes tracés en glyphes ; **feuilles affichées avec cartouche et vues placées** ; grilles dans l'arborescence avec leurs propriétés |

Non décodés : surfaces `HermiteSurf` et courbes `GEllipse` (solides écartés,
comptés) ; parmi les éléments 2D, étiquettes de pièces, de portes et de matériaux,
cotes de niveau, niveaux et repères de vue dans les vues ; filtres de vue et
remplacements graphiques. Les 64 octets qui suivent chaque document d'une partition
(§ 7) ne sont pas recalculés : Revit ne les contrôle pas à l'ouverture.

## Fichiers étudiés

Tous ouvrent normalement dans Revit (vérifié pour `Essai projet 2027 astra.rvt`,
ouvert dans Revit 2027 via RiveTT).

| Fichier | Type | Format | Taille |
|---|---|---|---|
| `Essai projet 2027 astra.rvt` | projet | 2027 | 111,6 Mo |
| `club_GABARIT 2027.rte` | gabarit | 2027 | 111,5 Mo |
| `OUVERTURE_FAMILLE_TEST.rfa` | famille | 2027 | 1,2 Mo |
| `CAR_A4_Entête projet.rfa` | famille | 2027 | 1,2 Mo |
| `club_GABARIT 2026.rte` | gabarit | 2026 | 152,1 Mo |
| `Saint-Malo_avenue aristide briand_46.rvt` | projet | 2026 | 356,5 Mo |
| `Projet1.rvt` | projet | 2026 | 8,8 Mo |
| `GABARIT JBA.rte` | gabarit | 2016 | 4,1 Mo |
| `Saint-cast-Le-Guildo.rvt` | projet d'agence | 2025 | 52,0 Mo |
| `Kerreut 2025.rvt`, `Langan.rvt`, `BIBLIO.rvt`, `Guidel.rvt` | projets d'agence (`Documents\Thomas\ARCHI PERSO`) | 2025 | — |
| `Kerreutv9.rvt`, `Colombes.rvt` | projets d'agence | 2022 | 138,2 Mo (Kerreutv9) |
| `Phase2_Lecture.rvt` | projet de Saint-Malo enregistré en 2027 (`Recettes\`) | 2027 | — |
| `Essais-format/2026/*.rvt` | essais produits par RiveTT (protocole), dont E12 (grilles), E15 (surface topographique) | 2026 | 0,5 à 1,6 Mo |
| `FEN_2PF.rfa`, `Projet vide.rvt` | famille de fenêtre de l'agence (6 types) et projet issu du gabarit, fournis le 21/10 | 2026 | 0,7 et 1,6 Mo |
| `Essais-format/2026/C14/*` | campagne C14 : F00 à F04 (projet vide, famille chargée, mur, fenêtre placée une puis deux fois), E13 (vues et 2D), W0 à W6 (écriture) | 2026 | 0,7 à 2,1 Mo |
| `Saint-cast-Le-Guildo.rvt` (`ARCHI PERSO\SAINT-CAST-LE-GUILDO - Rue des roseraies`) | projet d'agence, 15 fenêtres de vue, 67 instances de vue (campagne C16) | 2025 | 52,0 Mo |

## 1. Conteneur OLE

Format Microsoft Compound File Binary, version 4 : secteurs de 4 096 octets,
mini-secteurs de 64 octets, seuil du mini-flux 4 096 octets. Pour le fichier de
111,6 Mo : 27 secteurs FAT, tous référencés dans l'en-tête (pas de secteur DIFAT
supplémentaire). Les chaînes de secteurs des flux étudiés sont contiguës.

Flux de `Essai projet 2027 astra.rvt` :

| Flux | Taille | Contenu |
|---|---:|---|
| `BasicFileInfo` | 2 109 | métadonnées, texte UTF-16 |
| `ProjectInformation` | 1 387 | ZIP contenant un XML Atom (projets et gabarits) |
| `TransmissionData` | 2 536 | XML UTF-16 des références externes |
| `RevitPreview4.0` | 1 000 | objet sérialisé contenant une vignette PNG |
| `Contents` | 212 | gzip, 240 octets ; contient « GLOBAL » et un GUID (non analysé) |
| `Formats/Latest` | 190 117 | gzip : schéma des classes |
| `Global/Latest` | 227 732 | gzip, 2 Mo : données globales du document |
| `Global/ElemTable` | 185 512 | gzip, 1,3 Mo : table des éléments |
| `Global/History` | 42 220 | gzip : historique |
| `Global/PartitionTable` | 137 | gzip : partitions (contient « Workset1 » et un GUID) |
| `Global/DocumentIncrementTable` | 179 | gzip : incréments (contient le nom de session Windows) |
| `Global/ContentDocuments` | 7 222 680 | gzip, 40,7 Mo décompressés (non analysé) |
| `Partitions/0` | 103 593 406 | 7 086 membres gzip : données des éléments (noms de types, matériaux…) |

Variantes :
- un **gabarit** (`.rte`) a exactement la même structure qu'un projet : `SaveAs`
  d'un projet en `.rte` ne change que le chemin et les compteurs, sans drapeau
  dans `BasicFileInfo` (essai C4) ;
- les **familles** n'ont pas `ProjectInformation` mais un flux `PartAtom` (XML UTF-8 en clair) ;
- les gros modèles ont plusieurs partitions numérotées : `Partitions/105` à `187`
  (Saint-Malo, 81 flux), `Partitions/167` à `228` (gabarit 2026).

## 2. Métadonnées (décodées)

Implémentation : `src/rvt.rs` ; commande `ifc_viewer --stats fichier.rvt`.

**`BasicFileInfo`** : en-tête binaire (chaînes UTF-16 préfixées par leur longueur :
version, build, chemin, GUID, langue…), puis un bloc texte UTF-16 de lignes
`Clé: valeur` : `Worksharing`, `Format: 2027`, `Build: 20260716_1515(x64)`,
`Last Save Path`, `Locale when saved: FRA`, `Unique Document GUID`, etc.
Particularités :
- le bloc n'est pas toujours aligné sur 2 octets : il faut essayer les deux parités ;
- une fin de ligne est écrite en octets simples `0D 0A` et se lit comme le caractère U+0A0D.

**`ProjectInformation`** : archive ZIP à une entrée (nommée d'après un fichier
temporaire `…\Temp\…\Revit<GUID>.project.xml`), compressée en deflate. Elle
contient un XML Atom (`urn:schemas-autodesk-com:partatom`). L'en-tête déclare
UTF-8, mais les accents sont en Windows-1252 : décodage UTF-8, sinon CP1252.
Structure utile :

```xml
<entry><title>…</title><updated>…</updated>
  <A:features><A:feature><A:title>Project Information</A:title>
    <A:group><A:title>Autre</A:title>
      <Nom_du_projet displayName="Nom du projet" type="system" typeOfParameter="Texte">…</Nom_du_projet>
```

**`PartAtom`** (familles) : même XML Atom, plus `<category><term>` (catégorie de la
famille) et `<A:family><A:part>` (un élément par type de famille, avec ses
paramètres et `units="mm"`).

**`TransmissionData`** : u32 (longueur en caractères) puis XML UTF-16.
`<ExternalFileReference>` donne le type (`Keynote Table`, `CAD Link`,
`Revit Link`…), les chemins et l'état de chargement.

**`RevitPreview4.0`** : objet sérialisé (noms de champs `FilePreview`,
`m_pAImage`, `m_width`, `m_height`), puis u32 de longueur et un PNG complet de
128 × 128 px, à l'octet 235 dans tous les fichiers testés.

## 3. Compression

- `Global/*` : 8 octets de préfixe (entier, ex. 0, 1 ou 5), puis un membre gzip
  standard (`1F 8B 08 00 00000000 00 0B`). `Formats/Latest` n'a pas de préfixe.
- `Partitions/N` : 8 octets d'en-tête de partition (`09 00 00 00 00 00 00 00`), puis
  des documents et leurs membres gzip. Cadrage établi le 21/10/2026 (campagne C14),
  après retrait des blocs intercalés (§ 4) :

  ```
  partition = u32 9, u32 0, document*, ContentMarker de clôture
  document  = ContentMarker, membre+, membre vide, 3F 0F u32 64 [64 octets],
              u32 34 "Data generated by Autodesk® Revit®" (UTF-16)
  ContentMarker = u16 0x3A3, pointeur de ContentKey (u32 0 : document principal ;
              sinon u32 FFFFFFFF, u16 0x3A2), u32 nombre d'éléments (−1 pour la
              clôture), puis, pour un document embarqué, le corps du pointeur : le
              GUID du document (16 octets, collé à l'en-tête du premier membre)
  membre    = 28 0F, u32 h2, u32 n, u32 g + 8, u32 somme des corps, u32 section,
              u32 0, gzip (g octets), 21 0F, u32 g + 8
  ```

  Les ids de classe 0x3A3 et 0x3A2 sont ceux du schéma 2026 : `ContentMarker` vaut
  0x391 et `ContentKey` 0x390 dans un fichier 2025 (Kerreut) ; le lecteur les prend
  dans `Formats/Latest`. Le document d'un enregistrement est celui du dernier marqueur
  qui précède son membre (`objets::sections_documents`, vérifié sur Kerreut : 262
  documents embarqués, dont celui de chaque famille de cartouche).

  n est le nombre d'enregistrements du membre (un membre commence toujours sur un
  début d'enregistrement), la somme des corps celle de leurs longueurs (la taille
  décompressée moins 16 octets par enregistrement en 0x65, 20 en 0x66 et 0x67), la
  section 0x65, 0x66 ou 0x67 (§ 7) ; h2 vaut 4 (90 %), 5, 6 ou 7 (sens non établi,
  recopié tel quel). Le lien `21 0F` répète g + 8 avant l'en-tête suivant : ancienne
  lecture « les 16 bits hauts de h0 reprennent le h4 précédent ». Le membre vide (n,
  g et somme nuls) clôt un document. Les 64 octets qui suivent ne sont égaux ni au
  SHA-512, ni au SHA3-512, ni au BLAKE2b des données ou des membres du document ;
  Revit ne les vérifie pas à l'ouverture (essais W2b, W4).

  Les membres décompressés font généralement un peu moins de 128 Ko (jusqu'à 131 088 octets).
- Chaque flux gzip est suivi, après son trailer, de quelques dizaines d'octets à
  forte entropie (35 à 704 observés) : zéros de remplissage et CRC du dernier
  morceau (§ 4 bis).
- Le premier u32 des enregistrements 0x66 et 0x67 de `Partitions/*` est l'**Adler-32
  du corps** de l'enregistrement (400 / 400 vérifiés, puis tous ceux qu'écrit
  `ecriture.py`) ; `Adler32::checkAndRepair` de `Utility.dll` s'en sert pour réparer
  un enregistrement endommagé.

## 4. Décompression : les blocs intercalés (résolu le 20/10/2026)

### Règle

Chaque flux OLE compressé (`Formats/Latest`, `Global/*`, `Partitions/*`) est
découpé en tranches de **65 249 octets : 64 896 octets de données, puis un bloc
de 353 octets** à retirer. Le bloc commence par `00` suivi d'un octet multiple de
0x10 (`00 00` à `00 F0`) ; les 351 octets suivants sont à forte entropie et
diffèrent d'un bloc à l'autre : c'est un code correcteur, décrit au § 4 bis. Une
fois ces blocs retirés, les membres gzip sont standard.

```
flux = [64 896 données][353 bloc][64 896 données][353 bloc]…[reste des données]
```

Implémentation : `unpad()` dans `tools/rvt-research/extract.py`.

Validation (membres gzip décodés avec CRC32 et ISIZE exacts, avant → après) :

| Fichier | Format | Avant | Après |
|---|---|---|---|
| `Essai projet 2027 astra.rvt` | 2027 | 5 551 OK, 1 543 KO | 7 096 OK, 0 KO |
| `Saint-Malo_avenue aristide briand_46.rvt` | 2026 | 6 856 OK, 4 149 KO | 11 005 OK, 0 KO |
| `Projet1.rvt` | 2026 | 601 OK, 122 KO | 723 OK, 0 KO |
| `GABARIT JBA.rte` | 2016 | 319 OK, 57 KO | 376 OK, 0 KO |
| `OUVERTURE_FAMILLE_TEST.rfa` | 2027 | 26 OK, 13 KO | 39 OK, 0 KO |
| `CAR_A4_Entête projet.rfa` | 2027 | 30 OK, 12 KO | 42 OK, 0 KO |
| projet minimal `V0.rvt` (essai C1) | 2026 | 21 OK, 4 KO | 25 OK, 0 KO |

### Comment la règle a été trouvée

1. **Seuil net** (essai C7) : un projet vierge enrichi par paliers de 500 lignes
   de modèle. `Global/ElemTable` reste OK jusqu'à 64 737 octets compressés et
   passe KO à 66 866. Le taux de KO « croissant avec la taille » n'était qu'un
   effet de position.
2. **Contenu attendu connu** : les enregistrements de `ElemTable` sont réguliers ;
   le premier enregistrement faux (15 073) désigne le premier symbole deflate
   faux, à l'octet **64 895** du flux. Dans `Formats/Latest`, sans rapport, la
   divergence tombe au même octet, et les deux flux contiennent `00 F0` à 64 896.
3. **Motif périodique** : la table compressée a une période de 17 octets ; le motif
   s'interrompt à 64 896 et reprend à 65 249. Retirer ces 353 octets rétablit le
   CRC. Dans `Formats/Latest`, le second bloc est à 130 145 = 64 896 + 65 249.

Ce qui expliquait les symptômes : un décodeur standard lit le bloc intercalé comme
des données deflate, se resynchronise quelques symboles plus loin (d'où une fin
« exactement au trailer »), mais avec un décalage qui fausse toutes les copies
suivantes (sortie trop longue, texte brouillé).

### Hypothèses écartées en chemin

| Hypothèse | Test | Résultat |
|---|---|---|
| Inversion d'un bit (corruption disque) | les 85 000 inversions possibles sur un membre de 10 Ko | aucune ne rétablit le CRC |
| Deflate64 (fenêtre 64 Ko, code 285 étendu) | décodeur `inflate_ref.py` modifié | pire : 549 347 octets |
| Bits supplémentaires en ordre inverse | 4 combinaisons longueur/distance sur 3 membres | aucune ne rétablit le CRC |
| Table des longueurs modifiée pour un seul code | calcul sur le nombre d'usages de chaque code | aucune solution entière |
| Lecteur OLE fautif | flux relus par l'API Windows `StgOpenStorage` | octets identiques à `cfb.py` |
| Fenêtre deflate partagée entre membres consécutifs | `zdict` = sortie du membre précédent, 112 membres KO | aucun corrigé |
| Séquence de 2 octets échappée à l'écriture | recherche d'un motif absent sur 8,2 Mo compressés | aucun motif absent |
| zlib modifié | chaînes de version dans les DLL de Revit 2026 | zlib 1.3.1 et 1.3.2 standard (`Utility.dll`…) |

## 4 bis. Code correcteur (décodé le 21/10/2026)

Les blocs de 353 octets et la fin des flux compressés sont les CRC d'une couche
d'écriture de Revit (classes `CRCFile` / `CRCIO` de `Utility.dll`, avec
`setPreambleSize`, `startCheckingCRC`). Revit les **contrôle** : un fichier dont ces
octets sont mis à zéro est refusé (`CorruptModelException`, `CArchiveException 105`,
essai W1) ; une erreur de quelques bits est, elle, corrigée en silence.

**Méthode.** Mise en évidence en boîte noire (tailles quantifiées ; écart linéaire :
deux paires de flux d'écart de données identique ont le même écart de fin, 10 cas
sur 10), puis lecture du code machine de `Utility.dll` (Revit 2026,
`dumpbin /DISASM` de Visual Studio 2022) **à des fins d'interopérabilité** : seules
les fonctions de `CRCIO` et les initialiseurs des codes ont été lus ; l'algorithme est
réécrit dans `tools/rvt-research/crcbloc.py`, aucun code n'est repris.

**Codes.** Huit CRC réfléchis (valeur initiale 0, sans inversion finale), choisis
selon la longueur D du morceau de données :

| Largeur w | Polynôme réfléchi | Longueur L d'un mot (bits) | Pas k | Données D (octets) |
|---:|---:|---:|---:|---|
| 11 | 0x500 | 2 047 | 2 | plus de 5 081 (morceau plein : 64 896) |
| 9 | 0x110 | 511 | 2 | 1 275 à 5 081 |
| 7 | 0x60 | 127 | 2 | 549 à 1 274 |
| 6 | 0x30 | 63 | 2 | 257 à 548 |
| 5 | 0x14 | 31 | 2 | 106 à 256 |
| 4 | 0x0C | 15 | 2 | 41 à 105 |
| 3 | 0x5 | 7 | 2 | 8 à 40 |
| 2 | 0x3 | 3 | 4 | 7 et moins |

Les seuils sont calculés par Revit au chargement à partir des rapports w / L (même
calcul dans `crcbloc._seuils`).

**Disposition d'un mot** (bits rangés poids faible d'abord dans chaque octet) :
- n registres CRC, n ≥ 65. Avec b la largeur du champ de remplissage (bits
  nécessaires pour compter ⌈max(L·k, 65) / 8⌉ octets) et B = 8·D + b : si
  B ≤ (L − w) × 65, mot de ⌈B / 65⌉ + w bits par registre et n = 65 ; sinon mots de
  L bits et n = ⌈B / (L − w)⌉, arrondi pour que n − 65 soit multiple de k ;
- zone de données de (longueur − w) × n bits : les D octets, des zéros, puis le
  champ de b bits qui donne le nombre d'octets de zéros ;
- le bit i de la zone entre dans le registre i mod n ; le bit j du registre r est
  rangé au bit (zone + r + j·n). Le mot fait ⌈longueur × n / 8⌉ octets.

Morceau plein : CRC-11, n = 255, 255 × 2 047 bits = 521 985 bits = **65 249 octets
= 64 896 + 353** ; 3 bits de zéros et le champ de 9 bits (valeur 0) suivent les
données, d'où les 12 bits nuls `00 x0` en tête de chaque bloc.

**Validation.** `crcbloc.py` réencode à l'identique, octet pour octet, tous les flux
compressés de `FEN_2PF.rfa`, `V0.rvt`, `F01_charge.rvt`, `OUVERTURE_FAMILLE_TEST.rfa`
(2027), `CAR_A4_Entête projet.rfa` (2027) et `Projet1.rvt` (62 flux). Un bit modifié
dans les données change 1 octet de données et 6 octets du bloc (les 11 bits d'un
registre, espacés de 255 bits).

## 5. Schéma de classes (`Formats/Latest`)

Grammaire déduite (entiers en little-endian) :

```
fichier      = u16 (0) , { classe , u16 }
classe       = u16 longueur , nom ASCII , corps
corps        = réf_classe (parent) , u32 version , u32 nb_champs , champ* ,
               u32 nb_guid , GUID[16 octets]*
réf_classe   = u16 id   — si id & 0x8000 : u16 (0) + définition en ligne
               (u16 longueur, nom, corps) de la classe id & 0x7FFF
champ        = u32 longueur , nom ASCII , type
type         = u8 code , u8 sous-code , u16 ,
               [u32 nombre  si sous-code & 0xF0 = 0x10] ,
               [u32, u8, type, (réf_classe si l'élément est une classe)  si code = 0x0D] ,
               [réf_classe  si code = 0x0E et sous-code & 0x0F = 0]
```

Codes de type. Les classes `std::pair< T1, T2 >` du schéma donnent le type C++
de leurs deux champs, ce qui fixe la table sans ambiguïté :

| Code | Type C++ | Taille | Exemple |
|---|---|---|---|
| 0x01 | `bool` | 1 | `Element.m_locked` |
| 0x02 | `char` | 1 | `GeomStepList.m_flags`, `Dimension.m_flags` |
| 0x03 | `short` | 2 | `LinePattern.m_pixelPattern` |
| 0x04 | `int`, `unsigned int` | 4 | `GeomStepList.m_idCounter` |
| 0x05 | `unsigned long` | 4 | `ATime.m_msb`, couleurs |
| 0x06 | `float` | 4 | `APropertyFloat.m_value` |
| 0x07 | `double` | 8 | `Plane.m_origin` (`double[3]`) |
| 0x08 | `AString` (sous-code 0x60) | u32 + 2 × n | `DatumPlane.m_text` |
| 0x09 | GUID | 16 | `GUIDvalue.m_guid` |
| 0x0A | référence de classe | 2 | `ClassDefinitionRef.m_ref` (un seul usage) |
| 0x0B | `int64_t` | 8 | `Identifier.m_id64` |
| 0x0D | tableau de tableaux | — | `Envelope.m_corners` = double[2][2], `Trf.m_3x3` |
| 0x0E | classe, pointeur ou référence | — | `Element.m_id` (ElementId) |

Sous-codes : 0x10 tableau fixe (nombre dans le schéma), 0x50 conteneur (nombre
lu dans les données), 0x51 conteneur de pointeurs, 0x11 et 0x13 tableaux fixes de
pointeurs, 0x53 et 0x54 (un à trois usages). Pour 0x0E, le quartet bas donne la
nature : 0 classe incorporée, 1 et 2 pointeur possédé, 3 référence faible (§ 7).

**Champs non enregistrés** (schéma 2027) : le u16 qui suit le sous-code (`ex`)
vaut 0 pour presque tous les champs. 340 champs ont le bit 0x02 (valeurs 0x02,
0x12, 0x22) : `m_pTempExclusionMgr`, `m_oaNonSavedDrawFilters`,
`GeomTable.m_cacheIsCurrent`… Ces champs **n'apparaissent pas dans les données**.
Un seul champ a la valeur 0x04 (`VarExpr.m_refCt`) et il est bien enregistré. Le
schéma 2026 n'a pas ces marques (un seul `ex` non nul) : ses champs transitoires
n'y figurent sans doute pas.

**Identifiant de classe** : rang d'apparition de la définition dans le flux, en
comptant aussi les classes définies en ligne, **plus 12**. Exemples (2026) :
`A3PartyAImage` = 12, `Element` = 37 (0x25), `DatumPlane` = 1 193,
`Level` = 2 535 (0x9E7), `Room` = 3 774. Les identifiants changent d'une version à
l'autre (`DBView` a pour parent 37 en 2026, 38 en 2027).

**Fin du flux** : une classe de nom vide (`u16 0`) ; la dernière classe réelle est
`testclass`.

Exemples décodés :
- `ElementId` → `m_id : Identifier` → `m_id64 : i64` (ids de classe 0x14 et 0x15) ;
- `ADocument` (version 2663) : `m_elemTable`, `m_pPartitionTable`, `m_pHistory`…,
  suivi de 1 568 GUID ;
- `Dimension` (parent `Element`, id 0x26, version 27) : 17 champs puis 2 GUID ;
  elle est définie en ligne comme parent de `DimWithSegRefs` (version 4, 6 champs),
  elle-même parent de `LinearConstraint` ;
- `Trf` : `m_3x3 : double[3][3]`, `m_or : double[3]` (matrice de transformation).

Résultat, schéma lu en entier :

| Format | Classes de premier niveau | Définies en ligne | Champs |
|---|---:|---:|---:|
| 2016 (`GABARIT JBA.rte`) | 2 923 | 869 | 10 327 |
| 2026 | 3 604 | 1 086 | 12 558 |
| 2027 | 3 647 | 1 110 | 13 080 |

`Formats/Latest` est **identique octet pour octet** entre deux fichiers de même
version (deux projets 2026 enregistrés à des dates différentes) : c'est une
constante de version.

De 2026 à 2027 : 98 classes ajoutées (`AdvancedGeolocation`, `BasicFileInfo`…),
31 retirées (`AnalyticalSupportData`, `FaceSculpt*`…), 55 changent de version, par
exemple `ADocument` v2662 → v2663 (champs `m_bOpenedWithIRDB` et
`m_massingDispType` ajoutés).

## 6. Table des éléments (`Global/ElemTable`)

En-tête : u16 (1 174 en 2016, 1 481 en 2026, 1 510 en 2027 : version du format de
table ?), u32 nombre d'enregistrements (2 426 pour un projet vierge, 33 235 pour
`Essai projet 2027 astra.rvt`), puis un enregistrement de 40 octets par élément du
document (28 octets jusqu'au format 2022, où les ids sont sur 4 octets), triés par id
croissant. Les ids ne sont pas contigus (jusqu'à 10 568 507
dans le projet 2027). Le flux se termine par 24 octets non étudiés.

| Octets | Contenu |
|---|---|
| 0–7 | ElementId (i64) |
| 8–19 | trois u32 : numéro de la sauvegarde où l'élément a été modifié en dernier (égaux dans tous les cas observés) |
| 20–27 | ElementId répété |
| 28–35 | −1 le plus souvent ; un autre id pour certains éléments (sens non établi) |
| 36–39 | 0 |

Établi par différence : renommer le niveau 1 959 (essai E01) ne change que
l'enregistrement 1 959, dont les trois u32 passent de 0 à 1 ; dans la série C7,
les lignes créées avant la sauvegarde n portent n.

## 7. Données des éléments (`Partitions/*`)

### Partitions et sauvegardes

- `SaveAs` réécrit tout le modèle dans **une seule** partition, numérotée d'après
  le compteur de sauvegardes (`Partitions/0` à la création, `/1` au premier
  réenregistrement…). Un réenregistrement sans modification recopie la partition
  à l'identique, compression comprise.
- `Save` sur un fichier existant **ajoute** une partition contenant les éléments
  modifiés et garde les anciennes (sauvegardes successives de Saint-Cast : une
  partition ajoutée, 27 flux inchangés ; `Projet1.rvt` : 8 partitions, non
  partagé). Pour lire l'état courant, un élément d'une partition plus récente
  remplace celui d'une partition plus ancienne.

### Cadrage d'une partition

Le champ h6 de l'en-tête de chaque membre (§ 3) donne sa **section** ; les
membres d'une section se concatènent (un enregistrement de plus de 128 Ko déborde
sur le membre suivant). Les trois sections contiennent un enregistrement par
élément, dans le même ordre :

| h6 | Contenu | Cadrage d'un enregistrement |
|---|---|---|
| 0x65 | un `ElementHeader` par élément | `id, u32 n, n octets, u32 n` |
| 0x66 | l'objet de l'élément | `id, u32 (non identifié), u32 n, n octets, u32 n` |
| 0x67 | un complément : `SerializedDummy` (classe sans champ) le plus souvent, la géométrie calculée (`GElement`, voir « Géométrie calculée ») pour les éléments qui en ont une (398 dans `Projet1.rvt` : murs, instances, pièces, dalles…) | comme 0x66 |

L'id est un i64 depuis le format 2024 (ElementId sur 64 bits), un i32 avant
(vérifié sur 2016 et 2022). Le critère fiable est le schéma : la classe `Identifier`
a un champ `m_id64` (`int64_t`) depuis 2024, `m_id` (`int`) avant. Le premier u32
de l'en-tête de partition ne suffit pas : il vaut 9 en 2022 comme en 2026.

Les n octets commencent par l'id de classe (u16), puis les champs. Le cadrage
tombe exactement sur la fin de chaque section dans tous les fichiers essayés
(projets 2026 et 2027 jusqu'à 356 Mo, 69 partitions pour Saint-Malo).

Les partitions contiennent **plus d'objets que `ElemTable`** : 587 083 contre
33 235 dans le projet 2027 (tous les ids de `ElemTable` y sont). Les objets en plus
sont le **contenu des familles chargées**, vérifié le 21/10/2026 (campagne C14,
ci-dessous).

### Familles chargées dans un projet (campagne C14)

Série en chaîne depuis `Projet vide.rvt` (script RiveTT `rvt-essais-c14-famille`) :
F00 réenregistré, F01 `FEN_2PF.rfa` chargée, F02 un mur, F03 la fenêtre placée à
2 345,6 mm (allège 912,3 mm), F04 une seconde fenêtre.

- **Chargement** (F00 → F01) : 37 éléments du projet ajoutés (`Family` 257 028, six
  `FamilySymbol` et leurs `FamSymSurrogate`, sous-catégories, matériaux, motifs).
  Le document de la famille est recopié **en entier** dans la partition du projet,
  comme document embarqué (`ContentMarker` avec clé, cadrage ci-dessus), après ceux
  des 33 familles déjà chargées : 1 131 objets (contre 2 804 dans le `.rfa` : les
  styles inutilisés sont retirés), **renumérotés** dans l'espace d'ids du projet
  (257 275 à 258 405). `Family.m_oFamDoc.m_contentDocGUID` désigne le document
  embarqué et `m_oFamDoc.m_big2SmallMap2` fait la correspondance d'ids ;
  `Global/ContentDocuments` décrit chaque document embarqué (`ContentMarker`,
  `ContentRec` : auteur, historique) et grossit de 88 Ko décompressés. La famille
  imbriquée de la fenêtre (marque de ligne de coupe, 224 objets) devient un document
  embarqué de plus.
- **Avant placement**, le type n'a pas de géométrie : `FamilySymbol` 258 431 de
  1,7 Ko, complément 0x67 `SerializedDummy`.
- **Placement** (F02 → F03) : l'instance (`FamilyInstance` 258 460, `GElement` de
  300 octets : un `GInstance`), le type activé (258 431 passe à 40 Ko et reçoit un
  `GElement` de 133 Ko) et un type propre à l'instance (258 461, coupé par le mur).
  Le mur n'est découpé qu'à la transaction suivante : son solide vaut encore
  3,564 m³ dans F03 et 2,968 736 m³ (deux baies) dans F04, comme dans l'API.
- Contrôle : instance 0,110 297 m³ lue contre 0,110 441 m³ dans Revit (écart du seul
  coffre de volet roulant, face cylindrique concave à 24 facettes).

### Documents de famille (`.rfa`)

Le viewer n'affichait pour `FEN_2PF.rfa` que le mur hôte du gabarit : la géométrie
d'une famille est faite de **formes**, absentes du lecteur. Classes et règles,
établies sur `FEN_2PF.rfa` et validées par le script RiveTT
`rvt-reference-formes-famille` (`get_Geometry` en détail fin) :

| Objet | Classe | Données |
|---|---|---|
| forme | dérivées de `GenSweep` : `ExtrusionElem`, `BlendElem`, `RevolutionElem`, `SweepElem`, `SweptBlendElem`, `FormElem`, `FreeFormElement` | `m_cutting` (1 : vide), sous-catégorie `m_categoryId`, matériau `m_materialId` (−1 : par catégorie), visibilité `m_famElemVisibility.m_flags` ; solide dans son `GElement` |
| combinaison | `FamilyGeomCombination` (→ `GeomCombination`) | `m_svMap` : forme pleine (`first`) et vides (`second.m_set`) ; le `GElement` porte le **solide résultant** |
| sous-catégorie | `CategoryElem` | nom `m_pCategory` → `Category.m_name` ; matériau par `GStyleElem.m_pGStyle` → `GStyle.m_materialElemId`, déjà appliqué aux faces du `GElement` |
| famille imbriquée | `Family` avec `m_famId` ≠ −1 | soit son contenu est rangé en ligne dans le document (`Element.m_famId` = la famille imbriquée : têtes de coupe et de niveau), soit c'est un document embarqué (`m_oFamDoc`) |

Comme dans Revit : une forme membre d'une combinaison n'a pas de géométrie propre (0
solide), un vide non plus ; la combinaison entièrement évidée 9 900 072 n'a aucun
solide. Formes retenues : 6 extrusions pleines hors combinaison et 7 combinaisons ;
volumes identiques à 10⁻⁶ m³ près sauf le coffre (0,040 471 contre 0,040 615 m³) ;
total 0,117 767 contre 0,117 91 m³. Les formes ne sont lues que dans un document de
famille (flux `PartAtom`) : dans un projet, celles d'une famille in situ sont déjà
dans la géométrie de son instance. Bits de `m_famElemVisibility` : 0x10 et 0x20
avant/arrière et gauche/droite (ordre non tranché), 0x08 dessus/dessous, 0x2000 à
0x8000 niveaux de détail ; non utilisés (toutes les formes sont visibles en 3D dans
l'éditeur de familles).

Types de la famille : `Family.m_pFamilyTypes` → `m_pairs` (nom, `params.m_params` :
`m_paramId` et `m_value` réel, `m_int`, `m_elemId`, `m_str` ou formule
`m_oExpression`). `Largeur` = `DOOR_WIDTH` (−1 001 301), `Hauteur` (−1 001 300).

### Vues et éléments 2D

Outil `vues.py`, portage `src/rvt/vues.rs`. Validé contre l'API de Revit 2026
(script RiveTT `rvt-reference-vues-2d`) sur l'essai **E13** (créé depuis F02 par
`rvt-essai-e13-vues2d` : ligne et arc de détail, texte `ZZ_TEXTE_PLAN`, cote,
région remplie, vue de dessin `ZZ_DESSIN`, coupe `ZZ_COUPE`, feuille `ZZ_FEUILLE`
avec une fenêtre de vue), sur `FEN_2PF.rfa` et `Projet1.rvt`.

| Objet | Classe | Données |
|---|---|---|
| vue | dérivées de `DBView` | nom `m_viewName`, échelle 1 / `m_scale`, origine `m_origin`, repère `m_horzDir` (droite), `m_vertDir` (haut), `m_viewDir` (vers l'observateur), niveau d'un plan `m_genElemId`, gabarit `m_isTemplate` |
| genre | `DBViewPlan.m_planViewType` 1 plan d'étage, 2 plan de plafond ; `DBViewSection.m_sectionViewType` 0 coupe, 1 ou 2 élévation ; `DBView3d` ; `DBViewAnonDrafting` vue de dessin ; **`DBViewDrafting` feuille** (nom historique) ; `DBViewSchedule` nomenclature | — |
| cadrage | marqueur de vue, dérivées de `Viewer` (`Section`, `Viewer3d`…) | `m_dbViewId`, `m_boundedSpace` : origine `m_orig`, axes `m_basis`, bornes `m_boundOffset[i]` = [max, min] sur chaque axe, actives si `m_boundActive[i]` = [1, 1] ; cadrage d'annotation `m_bAnnotationCropActive`, décalages `m_pAnnotationCropOffsets` (voir « Feuilles ») |
| élément 2D | tout `Element` avec `m_ownerDBViewId` ≠ −1 | coordonnées du modèle (plans, coupes : dans le plan de la vue) ou de la vue (vue de dessin ; feuille : mm du papier) |
| trait | `CurveElem` | `m_pCurveDriver` → `m_pCrv` (`GLine`, `GArc`) ; le `GElement` porte la même courbe |
| texte | `TextNote` | point `m_orig`, `m_baseVec`, `m_upVec`, largeur `m_lineWidth` (papier) ; texte `m_arrTextFragments[].m_text` ou RTF `m_oSavedRtfData.m_arrRtfStream` (familles) |
| cote | `LinearDimString`, `RadialDim`… | ligne `m_pDimLine` (`GLine`), segments `m_ArrSegInfo` (valeur `m_values[0].m_value`, milieu `m_origin` sur la ligne, textes `m_values[0].m_oTextFields` : `m_valueOverride`, `m_prefix`, `m_suffix`), points des éléments cotés `m_refPnts`, éléments cotés `m_witnessRefs[].m_pWitnessRef.m_geomRef.m_elemId`, type `m_styleSymbolId` (voir « Cotes et visibilité ») |
| région remplie | `FilledRegion` | esquisse `m_sketchId` (`m_absorbedCurves`), face plane ouverte dans son `GElement` |
| fenêtre de vue | `Viewport` | vue `m_dbViewId`, feuille `m_ownerDBViewId`, `m_offset` (position sur la feuille de l'origine de la vue), `m_scale`, titre `m_labelOrigin`, `m_labelLength`, type `m_attrId` : voir « Feuilles » |

Résultats : E13 identique à Revit (19 vues ; ligne (1 000 ; −1 000) → (3 456,7 ;
−1 000) ; arc de centre (4 000 ; −2 000) et rayon 765,4 ; texte à (1 234,5 ;
−2 345,6) ; cote de 2 456,7 mm ; contour de la région ; les deux cotes temporaires
de la région, `OST_WeakDims`, 2 000 et 1 000 mm, lues mais absentes de la vue comme
les 4 lignes de son esquisse, donc non tracées (« Cotes et visibilité ») ; cadrage de
la coupe 4 000 × 3 000 × 1 000 mm). `FEN_2PF.rfa` : 8 vues, 26 cotes (valeurs et lignes) et 4 textes comme
Revit, une fois écartés les éléments des familles imbriquées. `Projet1.rvt` :
31 vues, 4 cotes, 4 fenêtres de vue comme Revit (39 étiquettes de pièces et 3
chemins d'escalier comptés, non tracés). Saint-Malo : 188 vues, 1 595 éléments 2D ;
Kerreut : 106 vues, 1 699 éléments 2D (1 215 et 1 228 une fois écartées les lignes
d'esquisse, absentes des vues, le 22/10).

**Validation** contre Revit 2026 sur `Projet1.rvt` (comptes RiveTT / outil
`objets.py`) : murs 59 / 59 `SWall`, niveaux 3 / 3 `Level`, pièces 39 / 39
`RoomElem`, instances de familles 102 / 102 `FamilyInstance`, vues en plan
11 / 11 `DBViewPlan`.

Classes utiles relevées : `SWall` (murs), `Level`, `RoomElem` et `LevelRoomPlan`
(pièces), `FamilyInstance`, `DBViewPlan`, `CurveElem` (lignes), `SketchPlane`.

### Feuilles (campagne C16, 22/10/2026)

Outil `feuilles.py`, portage `src/rvt/feuilles.rs` (lecture dans `vues.rs` et
`maquette.rs`). Validé contre l'API de Revit 2026 par les scripts RiveTT
`rvt-reference-feuilles` (feuilles, cartouches, fenêtres de vue : `GetBoxCenter`,
`GetBoxOutline`, `GetLabelOutline`, `LabelOffset`, `View.Outline`, `CropBox`,
`GetTransform` des instances de vue), `rvt-compare-feuilles` (prédictions de
`feuilles.py --args` comparées par Revit, écarts renvoyés), `rvt-etendue-vues` (éléments
qui sortent du contour d'une vue) et `rvt-export-feuilles-png` (feuille exportée en
image par Revit, référence visuelle des captures du viewer), sur `E13_vues2d.rvt`,
`Projet1.rvt`, `Kerreut 2025.rvt` et `Saint-cast-Le-Guildo.rvt`.

**Fenêtre de vue** (`Viewport`) :

- `m_offset` est la position sur la feuille de l'**origine de la vue** (`m_origin`) :
  un point p du modèle se place en `m_offset` + `m_scale` · ((p − origine)·droite,
  (p − origine)·haut). Contrôle : centre de la boîte de l'API = `m_offset` + centre de
  `View.Outline` (contour de la vue en mm du papier, relatif à son origine) pour les 46
  fenêtres de vue des quatre fichiers.
- Contour de la vue : son **cadrage** quand il est actif (`m_boundActive` = [1, 1] sur
  les deux premiers axes), les huit coins de `m_boundedSpace` projetés ; vue non cadrée
  (E13, 4 vues de Saint-Cast) : étendue de son contenu, calculée par Revit, non
  reproduite. Boîte de l'API (`GetBoxOutline`) : contour élargi de 0,01 pied
  (3,048 mm) de chaque côté.
- Le contour de Revit déborde du cadrage quand des éléments en sortent : notes de texte
  et autres annotations (cadrage d'annotation inactif), plans de référence et niveaux,
  coupés à 25,598 mm (1 po + 1/128 po) au-delà du cadrage, ou à leur extrémité 2D quand
  elle est réglée dans la vue.
- Titre : `m_labelOrigin` (sur la feuille) = coin inférieur gauche de la boîte +
  `LabelOffset` de l'API ; `m_labelLength` = longueur de la ligne de titre (par défaut
  la largeur du contour). Type `m_attrId` (`ViewportAttributes`) : `m_showLabel`,
  famille de titre `m_familyTagId`, ligne d'extension `m_showExtensionLine`.
- Profondeur : plan d'étage, entre `m_bottomClipElev` et `m_cutPlaneElev` (cotes
  absolues) ; coupe et élévation, de la borne min (plan de la vue) à la borne max de
  l'axe 2 si son premier drapeau `m_boundActive[2][0]` vaut 1 (limite de vue).
- Cadrage d'annotation : `Viewer.m_bAnnotationCropActive` (8 / 8 vues de Kerreut comme
  le paramètre `VIEWER_ANNOTATION_CROP_ACTIVE`), décalages gauche, droite, haut, bas.

| Fichier | Fenêtres de vue | Centre exact | Boîte exacte | Écarts restants (mm) |
|---|---|---|---|---|
| E13 | 1 | — | — | vue non cadrée (contour = contenu) |
| `Projet1.rvt` | 4 | 4 | 3 | élévation : 25,598 à gauche et à droite (niveaux) |
| `Kerreut 2025.rvt` | 26 | 10 | 9 | 7 vues : 25,598 sur 1 à 4 côtés (plans de référence, niveaux) ; 10 vues : textes, cotes, symboles ou niveaux réglés hors cadrage, 0,14 à 57,1 sur 1 à 3 côtés ; centre décalé de 23,6 au plus |
| `Saint-cast-Le-Guildo.rvt` | 15 | 4 sur 11 cadrées | 4 | 2 vues : 25,598 sur 3 côtés ; 5 vues : annotations hors cadrage, 4,6 à 24,3 sur un côté ; 4 vues non cadrées |

**Instances propres à une vue** (`FamilyInstance` avec `m_ownerDBViewId` : cartouches,
symboles d'annotation, composants de détail) : le repère `m_pInstanceInfo.m_Trf`
(origine, axes en colonnes, échelle de la vue comprise pour une annotation) est
**identique à `GetTransform`** pour 89 / 89 instances de Kerreut (66 sur les
feuilles, 23 dans les vues) et 67 / 67 de Saint-Cast. C'est le `LocationPoint` (lu
dans `m_instOrigin`) qui s'en écarte, pas le placement : le cartouche réduit de Kerreut a
son point en (0 ; 0) mais son repère en (−1 190,793 ; 0,058) mm, sa géométrie étant
dessinée vers x = 1 000 mm dans la famille.

Dessin d'une instance de vue :

- traits : courbes (`GLine`, `GArc`…) du `GElement` de son type (`FamilySymbol`),
  régénéré pour ce type (cadre A3H : −420 … 0 × 0 … 297 mm, comme la boîte de l'API),
  filtré comme les familles 3D (les plans de référence, sous `GConditionDir` non nié,
  sont écartés) ;
- textes, étiquettes et régions remplies : éléments du **document embarqué** de la
  famille (`Family.m_oFamDoc.m_contentDocGUID`), hors `ElemTable` : `TextNote`,
  `TagNote` (étiquette), `FilledRegion` (avec son `GElement` 0x67 ; `m_bMasking` : région
  de masquage, blanche), leurs types (`TagNoteAttributes`, `FilledRegionAttributes`),
  motifs, styles (`GStyleElem`, couleurs), polices (`FontElem.m_pFont` : nom et taille,
  propriétaire `m_ownerId` = type de texte) et paramètres de famille
  (`ParamElemFamily`) ; placés par le repère de l'instance ;
- visibilité d'un texte commandée par un paramètre de la famille :
  `m_cellList.m_cells[].m_paramDrivenData[]` de propriété `IS_VISIBLE_PARAM`
  (−1 006 205) → `m_famParamId` (id du document embarqué) → paramètre du projet de même
  nom (`m_caption`) → valeur du type ou de l'instance (« Numero » = 0 : numéro de détail
  masqué, comme dans Revit).

**Étiquette** (`TagNote`) : `m_paramDataFormattingArr[]` (paramètre `m_paramId`,
`m_prefix`, `m_suffix`, `m_sampleText`, `m_bLeadingLineBreak`). Valeur cherchée sur la
feuille (paramètres appris : `m_sheetNumber`, `m_viewName`, `m_issueDate`,
`m_drawnBy`…), puis l'instance du cartouche, puis les informations du projet
(`ProjectInfo` : client −1 006 319, statut −1 006 320…) ; calculées : échelle de la
feuille (« 1 : 50 », « Comme indiqué » si les vues placées en ont plusieurs), révision
actuelle (description, date, numéro, remise à / par : révision de plus grand
`m_sequenceNumber` parmi `m_explicitlyPresentRevisionIds` de la feuille,
`ProjectRevision.m_description`, `m_date`, `m_numberStr`). Titre de vue : nom de la vue,
ou son « Titre sur la feuille » (`m_viewDescription`) s'il est rempli, échelle,
numéro de détail (`Viewport.m_detailNumber`). Les 7 étiquettes du cartouche de
« PRO 05 » de Kerreut donnent les textes de l'export de Revit (« Nicolas et Isabelle
MUSSET », « PRO 05 », « Plan RDC », « 1 : 50 », « DOSSIER PRE-PRO », « 23/03/2024 »,
« PCMI »).

**Mise en page des textes** (mesurée sur l'export de la feuille par Revit) :
alignement de `TextElement.m_flags` (valeurs de `TextAlignFlags` : 0x40, 0x80, 0x100 à
gauche, au centre, à droite ; 0x200, 0x400, 0x800 en haut, au milieu, en bas), point
d'ancrage `m_orig` à cette place du bloc de lignes ; taille de la police = hauteur des
capitales ; première ligne de base à l'ascendante de la police sous le haut du bloc
(écart mesuré < 0,1 mm), interligne 1,7 × la taille ; lignes coupées aux espaces à la
largeur `m_lineWidth` ; couleur de la sous-catégorie du type
(`m_lineAndTextAttr.m_categoryId` → `GStyleElem`), gras du RTF (`\b`).

**Viewer** : une feuille s'ouvre avec son cadre, ses cartouches et symboles (traits de
0,18 mm, régions, textes en glyphes), puis chaque fenêtre de vue rendue dans sa propre
passe : objets de la vue placée (modèle, éléments de détail, annotations) transformés
par la matrice affine modèle → feuille, découpés au rectangle du cadrage (ciseaux) et
en profondeur (plans de découpe `gl_ClipDistance`) ; les annotations ne sont découpées
qu'au cadrage d'annotation actif, comme dans Revit ; titres et cartouche par-dessus.
Captures du viewer et exports de Revit concordent à moins de 1,5 mm près pour la place
des vues, des cartouches et des titres (PRO 05, PRO 08, PCMI3, PCMI5.1 de Kerreut,
ZZ_FEUILLE d'E13). Le contenu des vues reste le rendu ombré du viewer (pas de lignes
cachées : faces ombrées, dalles et terrain coupés en gris là où Revit montre des faces
blanches cernées), sans les éléments 2D non décodés (étiquettes de pièces, de portes
et de matériaux, niveaux, cotes de niveau).

### Cotes et visibilité dans une vue (C16, 22/10/2026)

Les cotes tracées d'abord comme de simples rubans bleus ne ressemblaient pas à celles
de Revit et certaines n'y existaient pas. Règles établies contre l'API (scripts
`rvt-visibilite-elements-vue`, `rvt-categories-masquees`) et les exports des feuilles :

- **Type de cote** (`DimensionStyle`, `m_styleSymbolId` de la cote) : format des
  valeurs `m_oLinFormatOptions` (unité `m_unitTypeId`, précision `m_accuracy`) ou, si
  `m_bUseDefault` = 1, celui du projet (`UnitsElem.m_units.m_formatOptionsMap`, entrée
  `autodesk.spec.aec:length`) ; police (`FontElem` dont le propriétaire est le type),
  gras et sous-catégorie (couleur) dans `m_pLineAndTextAttr` ; lignes d'attache de
  longueur fixe si `m_dimWitnCntrlExtBelow` (`DIM_WITNS_LINE_CNTRL`) vaut 1, sinon
  jusqu'à l'élément ; paramètres `DIM_WITNS_LINE_EXTENSION_BELOW` (longueur fixe),
  `WITNS_LINE_EXTENSION`, `WITNS_LINE_GAP_TO_ELT`, `DIM_LINE_EXTENSION`,
  `TEXT_DIST_TO_LINE` (mm du papier). Kerreut : centimètres à l'unité (« 565 »), E13 :
  mètres au centimètre du projet (« 2.46 »), comme dans Revit.
- **Tracé** : ligne de cote entre les points extrêmes ; ligne d'attache perpendiculaire
  à chaque point de référence (de la ligne vers l'élément, moins l'écart, ou de
  longueur fixe), prolongée au-delà de la ligne ; marque oblique à 45° ; valeur de
  chaque segment centrée sur son milieu, au-dessus de la ligne dans le sens de lecture
  (vers la droite de la vue, ou vers son haut pour une cote verticale) ; valeur
  remplacée (`m_valueOverride` : « GARAGE », « MAISON » sur la coupe « PC A ») ou
  encadrée de son préfixe et de son suffixe.
- **Visibilité** : Revit ne montre une cote que si **chaque élément coté est dans la
  vue** (collecteur de la vue dans l'API : la cote « 613 » de « RDC 50e » cote un mur
  hors cadrage, absent de la vue, et la cote elle-même en est absente). Le viewer écarte
  une cote dont un élément coté est une ligne d'esquisse, est masqué dans la vue, est
  d'une catégorie masquée ou a sa boîte hors du cadrage.
- **Lignes d'esquisse** : une esquisse (`VarSketch`, dérivée de `Sketch`, contour
  d'une région remplie…) liste ses lignes dans `m_elemIdsPairSet.m_data[].m_elementId`
  (et son propriétaire dans `m_userId`) ; ces `CurveElem` (catégorie « <Esquisse> »)
  ne sont pas dans la vue et ne sont plus tracées (801 dans Kerreut). Les cotes des
  façades de PCMI5.1, toutes accrochées à de telles lignes, disparaissent comme dans
  l'export de Revit.
- **Éléments masqués** dans la vue : `m_oHiddenElementsViewSettings.m_hiddenElements`
  (3 dans « PC Est » : deux sols et une ligne de séparation de pièces) ; **catégories
  masquées** : `m_oaDrawFilters[0].m_categoryIds` (celles d'un gabarit de vue y sont
  recopiées) contient toutes les catégories que `View.GetCategoryHidden` déclare
  masquées (10 / 10, 9 / 9 et 8 / 8 sur trois vues de Kerreut). La liste contient en
  plus 37 à 50 ids que l'API n'a pas signalés (catégories absentes de
  `Settings.Categories` ou non masquées : non tranché) ; le viewer ne s'en sert que pour
  les catégories qu'il sait attribuer (murs, sols, plafonds, toits, escaliers,
  garde-corps, pièces, niveaux, quadrillages, catégorie de la famille d'une instance).
  Les objets du modèle masqués ou de catégorie masquée sont retirés des fenêtres de vue
  d'une feuille.

### Champs d'un objet

Les champs suivent l'ordre du schéma, **classe de base d'abord** (`Element`, puis
`DatumPlane`, puis `Level`), sans les champs non enregistrés (§ 5). Encodage :

| Type | Encodage |
|---|---|
| types simples | little-endian, sans alignement (tailles au § 5) |
| chaîne | u32 nombre de caractères, puis UTF-16LE |
| tableau fixe | les éléments à la suite |
| conteneur | u32 nombre d'éléments, puis les éléments |
| classe incorporée | ses champs à la suite |
| pointeur possédé (sous-codes 1, 2) | u32 : 0 = nul ; sinon numéro d'objet (ou 0xFFFFFFFF), puis u16 id de classe ; **corps écrit plus loin** |
| référence faible (sous-code 3) | u32 numéro d'objet (1 = le document, 2 = l'élément lui-même) |

**Corps des pointeurs** : après les champs de l'objet racine, l'enregistrement
contient les corps de tous les objets pointés, **dans l'ordre où les pointeurs ont
été rencontrés** (file unique, largeur d'abord) : d'abord ceux de la racine, puis
ceux trouvés dans ces corps, et ainsi de suite. Exemple, le niveau de l'essai E03
(738 octets) : racine `Level`, puis `ParamValueSetElementId`, `GeomStepList`,
`GeomTable`, `Face`, `Plane` (pointeurs de la racine), puis `DatumPlaneGeomStep`
(pointé par `GeomStepList`) et le `Plane` de la face.

**Validation** : un enregistrement est bien lu si la lecture s'arrête exactement à
sa longueur. Lecteur `tools/rvt-research/champs.py` (`--couverture`) :

| Fichier | Format | Section 0x66 lue en entier |
|---|---|---|
| `E10_piece.rvt` (essai) | 2026 | 2 432 / 2 432 |
| `Projet1.rvt` | 2026 | 54 826 / 54 826 (268 classes) |
| `Saint-cast-Le-Guildo.rvt` | 2025 | 153 618 / 153 624 (6 données d'add-ins) |
| `Kerreutv9.rvt` | 2022 | 383 174 / 383 181 (7 données d'add-ins) |
| `GABARIT JBA.rte` | 2016 | 33 663 / 33 663 (235 classes) |
| `CAR_A4_Entête projet.rfa` | 2027 | 2 871 / 2 871 (75 classes) |
| `Essai projet 2027 astra.rvt` | 2027 | 587 076 / 587 082 (300 classes sur 302) |

Les sections 0x65 et 0x67 des familles 2027 se lisent aussi en entier.

Seule exception relevée : les données d'add-ins (*Extensible Storage* : 5
`DataStorage` et 1 `AppearanceAssetElem` dans le projet 2027). Le pointeur
`ESEntity.m_blob` y est suivi du GUID du schéma de l'add-in et d'un contenu propre
à ce schéma, pas d'un id de classe ; ce format n'est pas décodé.

### Murs (`SWall`, `ArcWall`)

Un mur droit est un objet `SWall`, un mur courbe un `ArcWall` ; les deux dérivent
de `VWall`, comme `Railing` (garde-corps), qu'il faut donc exclure. Champs utiles,
lus dans l'objet du mur et ses corps :

| Donnée | Emplacement |
|---|---|
| type de mur | `VWall.m_WallAttributesId` |
| niveau de base | `Element.m_assocLevelId` |
| niveau haut | `VWall.m_upToLevelId` (−1 : hauteur non contrainte) |
| hauteur, décalages | `Element.m_pParamValueSetDouble` : paramètres `WALL_USER_HEIGHT_PARAM` (−1 001 105), `WALL_BASE_OFFSET` (−1 001 108), `WALL_TOP_OFFSET` (−1 001 109), en pieds |
| ligne de positionnement | `VWall.m_pCurveDriver` → `VWallDriver.m_pCrv` → `GLine` : `m_origin`, `m_dirVec`, `m_endParams` [t0, t1] ; extrémités = origine + t × direction, en pieds, Z compris |
| arc (mur courbe) | `GArc` : `m_center`, `m_radius`, `m_xVec`, `m_yVec`, `m_endParams` en radians ; point(t) = centre + rayon × (cos t · xVec + sin t · yVec) |
| faces de référence | `VWall.m_pRefFaces` → 4 `Face` portant chacune un `Plane` (enveloppe longueur × hauteur) |

Essai E04 : type 1 990 (« Mur 1 »), niveau 1 959, hauteur 2 718,2 mm,
(0 ; 0 ; 0) → (12 345,6 ; 0 ; 0) mm, valeurs saisies retrouvées à 0,1 mm.

**Validation contre Revit** (`Projet1.rvt`, 59 murs, extraction `champs.py --murs`
contre `LocationCurve`, `WallType`, `LevelId` et paramètres lus par RiveTT) :
type, niveau de base, niveau haut, hauteur, décalages et extrémités X, Y, Z
identiques pour les **59 murs** (tolérance 0,05 mm). Mur courbe (essai E11, arc
de centre (5 000 ; 5 000) mm, rayon 4 321,0 mm, de 0 à 90°) : centre, rayon et
extrémités (9 321 ; 5 000) → (5 000 ; 9 321) identiques à Revit.

**Projet réel** (`Saint-cast-Le-Guildo.rvt`, format 2025, ouvert dans Revit 2026) :
les **16 murs** concordent (type, niveaux, hauteur, décalages, extrémités), après
deux corrections :
- *hauteur d'un mur contraint en haut* : le paramètre enregistré
  (`WALL_USER_HEIGHT_PARAM`) garde la hauteur libre d'avant la contrainte (4 000 mm
  pour 4 murs) ; Revit annonce la hauteur calculée, altitude du niveau haut +
  décalage haut − altitude du niveau bas − décalage bas (2 700 mm) ;
- *groupes non placés* : les éléments dont `Element.m_unplacedOwnerId` désigne un
  type de groupe (`ElementGroupType`) appartiennent à un groupe sans occurrence ;
  les collecteurs de Revit les ignorent (deux niveaux en trop à Saint-Cast).

### Autres éléments

Outil `elements.py` (une lecture du fichier, puis un extracteur par sorte
d'élément). Emplacements établis :

| Élément | Classe | Données |
|---|---|---|
| niveau | `Level` | nom `DatumPlane.m_text` ; altitude = Z de `m_pSurface` → `Plane.m_origin` |
| type de mur | dérivées de `WallType` (`BasicWallType`…) | nom `Symbol.m_symbolInfo` → `SymbolInfo.m_name` ; épaisseur = somme des `CompoundStructureLayer.m_layerWidth` de `HostObjAttr.m_pCompoundStructure` (absente pour murs-rideaux et murs empilés) |
| dalle | `Floor` | type `m_floorAttributesId`, niveau `m_assocLevelId`, décalage `FLOOR_HEIGHTABOVELEVEL_PARAM` (−1 001 951), cotes `m_bottom` / `m_top` |
| instance de famille | `FamilyInstance` | type `m_masterSymbolId` (le `Symbol` de l'API ; `m_pInstanceInfo.m_symbolId` est autre chose), hôte `m_hostId`, niveau `m_assocLevelId`, direction `m_RefDir`, repère `ALL_MODEL_MARK` (−1 001 203) ou, pour une porte, `FamInstSpec.m_doorNumber` |
| pièce | `RoomElem` sans `m_areaSchemeId` ni `m_zoneSchemeId` | `ROOM_NAME` (−1 006 900), `ROOM_NUMBER` (−1 006 901), `ROOM_DEPARTMENT` (−1 006 907), niveau `m_assocLevelId`, point `m_point` (x, y seulement, en pieds ; z = altitude du niveau) ; pièce non placée : `m_bIsLocationless` = 1 |
| surface (plan de surfaces) | `RoomElem` avec `m_areaSchemeId` | mêmes paramètres |
| zone | `RoomElem` avec `m_zoneSchemeId` | idem |
| type (tout type) | dérivées de `Symbol` | nom `m_symbolInfo` → `SymbolInfo.m_name` (types de sols `FloorAttributes`, de toits `RoofAttributes`, d'escaliers `StairsType`, de familles `FamilySymbol`…) ; type de famille : famille `FamilySymbol.m_familyId`, paramètres de type `FamilySymbol.m_pParams` → `m_params` (`m_paramId`, `m_value`) |
| cotes d'une porte, d'une fenêtre | `FamilySymbol.m_pParams` | `Largeur` `FAMILY_WIDTH_PARAM` (−1 001 301) et `Hauteur` `FAMILY_HEIGHT_PARAM` (−1 001 300) du type : les **cotes du panneau**, pas l'encombrement de la famille (huisserie comprise) ; « PP (1.03m x 2.04m) » de Kerreut : 1 030 × 2 040 mm comme Revit. La table `Family.m_pFamilyTypes` n'est pas fiable pour cela (type absent, valeurs anciennes) |
| garde-corps | dérivées de `BaseRailing` | type `m_stairsRailingAttrId`, niveau `m_assocLevelId` ; main courante et barreaux dans son `GElement` (`GInstance` des types) |
| famille | `Family` | nom `m_name`, catégorie `m_categoryId` (BuiltInCategory : portes −2 000 023, fenêtres −2 000 014, mobilier −2 000 080…) |
| matériau | `MaterialElem` | `m_pMaterial` → `Material` : nom `m_name`, couleur `m_color` (COLORREF, rouge dans l'octet faible : 0x7A8C8E = 142-140-122), transparence `m_transparency` (0 à 1 ; Revit affiche ×100) |
| plafond | `Ceiling` | type `m_ceilingAttributesId`, niveau `m_assocLevelId`, décalage `CEILING_HEIGHTABOVELEVEL_PARAM` (−1 002 300) |
| toit | dérivées de `RoofBase` (`ProfileRoof` = toit par tracé, `FootPrintRoof` de l'API) | type `m_roofAttributesId`, niveau `m_assocLevelId`, décalage `ROOF_LEVEL_OFFSET_PARAM` (−1 001 701) ; contour `ProfileRoof.m_curveLoops`, faces de référence inclinées `RoofBase.m_pRefLayerFaces` |
| escalier | `StairsElement` | type `m_typeId`, niveaux `m_baseLevelId` / `m_topLevelId` et décalages `m_baseLevelOffset` / `m_topLevelOffset`, `m_actualNumberOfRisers`, `m_actualRiserHeight`, `m_actualTreadDepth` ; volées et paliers : listes `m_runAndLandings` des cellules (`Element.m_cellList`) ; `m_stairsHeight` n'est pas la hauteur réelle (3 048 mm pour 3 000 dans `Projet1.rvt`) |
| volée, palier | dérivées de `StairsRunAndLanding` (`StairsRun`, `StairsLanding`) | escalier `m_stairsId` (souvent −1 : le lien est dans la liste de l'escalier) ; géométrie dans leur `GElement` |
| grille | `Grid` (→ `DatumPlane`) | nom `m_text` ; plan vertical `m_pSurface` (`Plane` pour une grille droite, `CylSurf` pour une grille courbe) ; trace = point(u, 0) pour u entre les bornes de `Surface.m_Envelope` |
| toposolide (terrain, 2024+) | `Toposolid` (→ `CeilingAndFloor`, **pas** `Floor`) | type `m_TypeId` (`ToposolidType`, nom par `m_symbolInfo`), niveau `m_assocLevelId` (−1 pour un toposolide sans niveau : contexte importé, subdivision), décalage `TOPOSOLID_HEIGHTABOVELEVEL_PARAM` (−1 155 255) ; toposolide hôte d'une subdivision `m_hostTopoId` ; géométrie : B-rep du `GElement` comme un sol |
| surface topographique (avant 2024) | `SiteSurface` (→ `Element`) | sous-région : `m_hostId` (−1 pour une surface) ; sous-régions d'une surface : `m_siteAreas` ; points saisis : `m_fixedVerts` du premier `TriangulationDriverMemento` de `m_drivers[0].m_pMementos` (`m_loc` jusqu'en 2022, `m_locXYZ.m_xyz` en 2024 et suivants) ; géométrie : `GPolyMesh` du `GElement` |

Les ids de paramètres (BuiltInParameter) et de catégories (BuiltInCategory) sont
ceux de l'API publique de Revit, relevés dans Revit 2026 par un script RiveTT ; les
noms de catégories sont ceux de l'interface française.

**Position d'une instance** (la `LocationPoint` de l'API) :
- hébergée par un mur : point de la ligne du mur au paramètre `m_hostParam`
  (espace de paramètres de la `GLine` ou du `GArc`), Z = altitude du niveau +
  `m_elevation` (allège). `m_instOrigin` donne la même valeur dans les fichiers
  2026, mais vaut 0 pour ces instances dans le fichier 2025 ;
- autre instance de modèle : `m_instOrigin` ;
- élément 2D propre à une vue (`Element.m_ownerDBViewId` ≠ −1 : composants de
  détail, symboles) : non résolu ; ni `m_instOrigin` ni `m_Trf.m_or` ne donnent
  toujours la valeur de Revit (44 cas sur 67 concordent à Saint-Cast).

**Validation contre Revit** (références exportées par les scripts RiveTT
`rvt-elements-reference` et `rvt-reference-complete`) :

| Donnée | `Projet1.rvt` (2026) | `Saint-cast-Le-Guildo.rvt` (2025) |
|---|---|---|
| niveaux (nom, altitude) | 3 / 3 | 2 / 2 |
| types de murs (nom, épaisseur) | 15 / 15 | 14 / 14 |
| dalles (type, niveau, décalage) | 6 / 6 | aucune dans le projet |
| instances de modèle (type, hôte, niveau, position, repère) | 102 / 102 | 19 / 19 |
| pièces (nom, numéro, service, niveau, position) | 39 / 39 | 24 / 24, dont 17 non placées |

**Seconde campagne, portage Rust** (21/10/2026, script RiveTT
`rvt-reference-elements` : documents ouverts en arrière-plan dans Revit 2026 et
fermés sans enregistrer ; le fichier 2025 est mis à niveau à l'ouverture). Valeurs
lues par `src/rvt/maquette.rs` :

| Donnée | `Projet1.rvt` (2026) | `Kerreut 2025.rvt` (2025) | `Langan.rvt` (2025) | E12 (2026) |
|---|---|---|---|---|
| matériaux (nom, couleur, transparence) | 18 / 18 | 80 / 80 en nombre | 80 en nombre | — |
| sols et plafonds (type, niveau, décalage, volume) | 6 / 6 | 16 / 16 (un volume à 7·10⁻⁶ près : boucle à 0,01 mm près) | aucun | — |
| toits (niveau, décalage, volume) | aucun | 10 / 10 | 2 en nombre | — |
| escaliers (niveaux, contremarches, hauteur, giron, volées, volume) | 2 / 2 | 2 / 2 (marches du type `StairsTriserSymbol`) | aucun | — |
| garde-corps (type, volume) | 11 / 11 | 5 / 5 | aucun | — |
| pièces (volume du `GElement`) | 39 / 39 | 19 / 19 | 1 / 1 | — |
| instances (hôte, point, repère) | 12 / 12 vérifiées | 6 / 6 vérifiées | 2 / 2 | — |
| catégories des instances (portes, fenêtres, barreaux) | 43, 26, 33 comme Revit | — | — | — |
| grilles (nom, extrémités, centre et rayon) | aucune | aucune | aucune | 3 / 3 |

Volumes identiques à 10⁻⁵ m³ près. Les éléments non lus le sont avec une erreur
explicite ; aucune valeur fausse n'a été relevée.

### Géométrie calculée (`GElement`, section 0x67)

L'enregistrement 0x67 d'un élément qui a une géométrie est un `GElement` : le
solide calculé par Revit (jonctions et ouvertures comprises), en pieds, dans les
coordonnées du projet. `champs.py` le lit en entier (mur de E04 : 6 884 octets,
73 corps) ; `geometrie.py` en tire faces, contours et maillage.

**Arbre.** `GElement` dérive de `GRep` → `GGroup` → `GNode` : `GRep.m_bBox`
(boîte englobante), `GRep.m_elementId`, `GGroup.m_subNodes` (sous-nœuds). Chaque
nœud porte `GNode.m_GInfo.m_categoryId` : l'id d'un `GStyleElem` (sous-catégorie),
ou −1 (catégorie du parent). Pour un mur :

| Nœud | Contenu | Catégorie |
|---|---|---|
| racine `GElement` | boîte englobante, sous-nœuds | 119 → `GStyleElem` de `OST_Walls` (−2 000 011) |
| `/0` à `/3` (`GGroup`) | les 4 plans de référence du mur (E04 : deux à l'axe, un sur chaque nu), chacun une `Geometry` à une face (E04) ou des `GLine` / `GArc` (E11) | 799 → `GStyleElem` de `OST_WallRefPlanes` (−2 000 896, nom vérifié par l'API Revit) |
| `/4` (`Geometry`) | le solide | celle de la racine |

**B-rep.** `Geometry` dérive de `GBRep` : `GBRep.m_pFaces` (faces),
`Geometry.m_pEdges` (arêtes). Faces, boucles et arêtes se désignent par références
faibles (numéros d'objet des pointeurs, § « Champs d'un objet »).

| Objet | Champs utiles |
|---|---|
| `Face` (→ `GFace`) | `Face.m_pSurf` (surface), `GFace.m_pFirstLoop` (première boucle), `GFace.m_renderStyleId` (matériau ; 91 = `MaterialElem` « Mur par défaut » dans E04), `GFace.m_faceFlags_v9` |
| `EdgeLoop` (→ `GEdgeLoop`) | `m_nextLoop` (boucle suivante, pointeur possédé), `m_pFace`, `m_next` / `m_prev` (première / dernière arête), `m_Envelope` (boîte en (u, v)) |
| `Edge` (→ `GEdge`) | `m_pFace[2]` (les deux faces ; 0 s'il n'y en a qu'une), `m_next[2]` / `m_prev[2]` (arête suivante / précédente dans la boucle de chaque face, ou la boucle elle-même), `m_firstAndLastEdgePnts[2]`, `m_interiorEdgePnts`, `m_flags` |
| `EdgePnt` | `uv[2]` : le point en (u, v) sur chacune des deux faces de l'arête |
| `Plane` (→ `Surface`) | `m_origin`, `m_xVec`, `m_yVec` ; point(u, v) = origine + u·x + v·y |
| `CylSurf` (→ `Surface`) | `m_center`, `m_xVec`, `m_yVec`, `m_zVec`, `m_radius` ; point(u, v) = centre + r·(cos u·x + sin u·y) + v·z |
| `RuledSurf` (→ `Surface`) | `m_pProfileCurve1`, `m_pProfileCurve2` (courbes de profil ; absente : `m_Point1` / `m_Point2`), enveloppe [0, 1] × [0, 1] ; point(u, v) = C1(u) + v·(C2(u) − C1(u)), Cᵢ(u) = point de la courbe au paramètre t0 + u·(t1 − t0) de ses `m_endParams` |
| `GHermiteSpline` (→ `GCurve`) | `m_NodeArray` : nœuds `m_iParametr` (abscisse curviligne), `m_iPoint`, `m_iTangent` (unitaire) ; entre deux nœuds, Hermite cubique de pas h = écart des paramètres, tangentes multipliées par h |
| `SurfRev` (→ `Surface`) | `m_center`, `m_xVec`, `m_yVec`, `m_zVec`, `m_pProfileCurve` (courbe dans le repère local) ; point(u, v) = profil(v) tourné de l'angle u autour de z : (a cos u − b sin u)·x + (a sin u + b cos u)·y + c·z, (a, b, c) = profil au paramètre propre v. Écart des arêtes nul sur une poignée de porte de `Projet1.rvt` (204 mm avec u et v inversés) |

**Règles établies** (E04, E09, E11, `Projet1.rvt`) :

1. *Parcours d'une boucle* : `EdgeLoop.m_next` donne la première arête. Sur une
   arête, le côté s est celui où `m_pFace[s]` est la face et `m_prev[s]` l'élément
   précédent (arête, ou la boucle pour la première) ; on passe à `m_next[s]`
   jusqu'au retour à la boucle.
2. *Points d'une arête* : pas de courbe 3D. Les points sont
   `m_firstAndLastEdgePnts[0]`, les `m_interiorEdgePnts`, puis
   `m_firstAndLastEdgePnts[1]`, chacun en (u, v) sur les deux faces (`uv[s]`) ;
   les deux calculs 3D coïncident (écart ≤ 3·10⁻¹¹ mm). Les points intérieurs sont
   la tessellation de Revit : arcs de E11 à 30 et 31 points, autant que
   `Edge.Tessellate()` ; une arête droite n'en a pas.
3. *Sens* : le côté s parcourt l'arête du premier au dernier point si
   s = `m_flags & 1`, en sens inverse sinon (observé : `m_flags` 6 ou 7 pour une
   arête droite, 14 pour un arc). Vérifié par la continuité de toutes les boucles
   (fin d'une arête = début de la suivante).
4. *Orientation* : ainsi parcourue, une boucle extérieure tourne dans le sens
   direct autour de la normale sortante du solide, un trou en sens inverse (volumes
   positifs, maillages fermés et orientés). Le bit 0x02 de `GFace.m_faceFlags_v9`
   vaut 1 exactement sur les faces où Revit répond
   `OrientationMatchesSurfaceOrientation = false` (normale sortante opposée à
   x × y du plan, ou face concave d'un cylindre) : 3 faces de l'ouverture de E09,
   face intérieure de E11.
5. *Cas particuliers* : une face peut avoir plusieurs boucles extérieures (E09 :
   face inférieure coupée en deux par une ouverture partant du sol) ; des faces
   sans boucle subsistent (E09 : faces du volume de l'ouverture confondues avec
   celles du mur), on les ignore.
6. *Murs joints* : `Geometry.m_pEdges` contient aussi des arêtes hors boucle
   (`m_next` et `m_prev` nuls, parfois à une seule face : 70 sur 98 pour le mur
   257 010 de `Projet1.rvt`). Seules comptent les arêtes atteintes par les
   boucles ; une `Geometry` est un solide si toutes ont deux faces, une feuille
   sinon (plans de référence).
7. *Vides* : un solide fermé de volume négatif est un vide de famille
   (réservation). Gaine `EQS_Gaine` de Saint-Malo (instance 10 107 759) : boîte de
   6 faces, −5,6156 m³ ; l'API de Revit donne le même volume négatif et
   `InstanceVoidCutUtils.IsVoidInstanceCuttingElement` = vrai. Ces solides ne sont
   pas affichés (19 dans Saint-Malo). Le bit 0x04 de `m_faceFlags_v9` est présent
   sur presque toutes les faces et ne les distingue pas.
8. *Surfaces réglées* : paramétrage vérifié par l'écart des arêtes partagées avec
   des faces planes, nul pour un sol de `Kerreut 2025.rvt` (10 102 578, volume
   1,100 868 m³ comme Revit) ; l'autre ordre (u entre les courbes) donne 7 m
   d'écart. Sur une volée hélicoïdale de Saint-Malo (profils `GHermiteSpline`),
   l'écart avec les faces cylindriques est de 0,06 mm au plus, aux points
   intérieurs seulement : la spline de Revit approche l'hélice exacte. Les sommets
   ne sont alors pas fusionnés (maillage visuellement fermé, 4 arêtes non
   appariées).
9. *Continuité des boucles* : la fin d'une arête et le début de la suivante
   coïncident à 3·10⁻¹¹ mm pour les murs, mais seulement à 0,007–0,017 mm dans
   certaines familles (douche de Saint-Malo) ou dalles et pièces (Kerreut). Le
   contrôle se fait à la tolérance de sommet de Revit, 0,0005 pi (0,15 mm) : il
   débloque 1 sol et 1 pièce de Kerreut, 4 pièces de Saint-Malo, 211 solides de
   familles.
10. *Faces courbes* : la triangulation du seul contour aplatit les faces doublement
   courbes (demi-sphère de la couronne d'un arbre : −51 % de volume) et, avec le
   critère « plus courte diagonale », les cylindres fins (pieds de chaise). Les
   oreilles sont choisies d'abord par la flèche de la diagonale sur la surface (nulle
   sur un plan et le long des génératrices), puis par la longueur ; une face non
   plane reçoit ensuite une grille de points intérieurs (pas réglé pour une flèche
   de 2 mm, 64 points par axe au plus) insérée avec bascules de Delaunay, les arêtes
   du contour restant fixes. Arbre : −0,36 % au lieu de −51 %. Les décisions de la
   triangulation se font sur des (u, v) arrondis à 10⁻⁹ pi : des coins de trous
   alignés à 10⁻¹⁴ près faisaient échouer le pontage des trous (fenêtre de
   `BIBLIO.rvt`).

**Maillage** (`geometrie.py`) : par face, triangulation dans le plan (u, v) par
oreilles, trous reliés au contour par un pont, plus courte diagonale 3D d'abord
(sur un cylindre, chaque triangle couvre un pas de tessellation) ; sommets 3D par
la surface, en mm, fusionnés à 10⁻⁶ mm.

**Validation contre Revit 2026** (solides lus par l'API via RiveTT, documents
ouverts en arrière-plan et fermés sans enregistrer, le 21/10/2026) :

| Fichier | Lu dans le RVT | Revit |
|---|---|---|
| E04 (mur droit) | 8 sommets, 6 faces, 12 triangles ; boîte (0 ; −100 ; 0) → (12 345,6 ; 100 ; 2 718,2) mm, soit 12 345,6 × 200,0 × 2 718,2 ; 6,711 562 m³ ; 73,1411 m² | mêmes 8 sommets, 6 faces, 12 arêtes ; 6,711 562 m³ ; 73,141 14 m² |
| E09 (ouverture 913,5 × 2 100 mm depuis le sol) | 16 sommets, 9 faces utiles, 6,327 892 m³, 70,1444 m² | mêmes 16 sommets, 9 faces, 6,327 892 m³, 70,144 44 m² |
| E11 (mur courbe) | 6 faces dont 2 `CylSurf`, 122 sommets, 3,689 441 m³ | 3,689 91 m³, valeur exacte du cylindre : l'écart (−0,013 %) est celui du prisme polygonal de la tessellation stockée, recalculé à l'identique |
| `Projet1.rvt` | 59 murs, 850 faces planes, dont 22 faces à trous (48 trous) | volume, aire et nombre de faces identiques pour les **59 murs** (écarts < 10⁻⁶ m³ et 10⁻⁶ m²) |

Épaisseur du type « Mur 1 » de E04, lue dans le fichier : `BasicWallType` 1 990 →
`CompoundStructure.m_layers[0].m_layerWidth` = 0,656 17 pi = 200,0 mm, égale à
l'épaisseur du solide. Sans référence Revit, les 39 murs de
`Essai projet 2027 astra.rvt` donnent aussi des maillages fermés.

**Limites.** Surfaces décodées : `Plane`, `CylSurf`, `RuledSurf` (profils `GLine`,
`GArc`, `GHermiteSpline`), `SurfRev`. Non décodées : `ConeSurf`, `HermiteSurf` (lits
de Kerreut), `OffsetSurf`, `TempImportNurbsSurf`, `SharedSurface`, et la courbe
`GEllipse` ; un solide qui en porte est écarté et compté (`Maillage::echecs`), le
reste de l'élément est affiché. Les arêtes restent celles de Revit : sur des tubes
de 1 cm (chaises de Kerreut), leur échantillonnage grossier fait perdre 10 à 30 %
du volume de ces pièces (−4 % sur la chaise). Un mur-rideau n'a pas de solide
propre : ses panneaux et meneaux sont des instances, affichées. Triangulation en
O(n³) par face, à revoir pour de grandes faces.

### Familles (`GInstance`)

Le `GElement` d'une instance de famille (et de certains autres éléments : volées
d'escalier assemblé, garde-corps) contient un `GInstance` au lieu d'un solide :

| Champ | Rôle |
|---|---|
| `m_instanceInfo` (pointeur, `InstanceInfo` → `InstInfoBase`) | `m_Trf` : repère de l'instance ; `m_symbolId` : type dont le `GElement` (section 0x67 du `Symbol`) porte la géométrie |
| `m_oEmbeddedSymbolGRep` | géométrie incorporée à l'instance, à la place de celle du type |
| `m_bHasScale` | toujours 0 dans le corpus |

- *Repère* : p' = `m_or` + R·p, la colonne j de `m_3x3` (composantes `m_3x3[i][j]`)
  étant l'axe j ; 214 instances tournées de Kerreut sur 214 conformes à
  `GetTotalTransform` de l'API, origine identique pour les 346. Un repère miroir
  (déterminant négatif) inverse le sens des triangles.
- *Type* : le `m_symbolId` désigne souvent un type propre à l'instance (porte coupée
  par son mur : 257 052 pour l'instance 257 051 de `Projet1.rvt`, type maître
  104 714), un `FamilySymbol` ou un autre `Symbol` (`StairsTriserSymbol` : marches
  d'un escalier assemblé). Les `GInstance` s'imbriquent (familles imbriquées).
- *Filtres de vue* : l'arbre du type mêle les représentations de toutes les vues
  dans des `GFilter` à conditions (`m_oConditions`). Pour retrouver
  `get_Geometry(Options { DetailLevel = Fine })`, sans vue : `GConditionDir` et
  `GConditionCut` sont vraies si niées (`m_negateDirCondition` = 1 : vue non
  parallèle à l'axe, élément non coupé), `GConditionInt` de `m_param` 3 compare le
  niveau de détail (`m_comp` 0 : différent, 3 : égal ; `m_value` 1 grossier, 3 fin),
  toutes les conditions d'un filtre devant être vraies. Les autres classes
  (`GConditionViewType`, `GConditionVModeType`…) sont tenues pour vraies ;
  `GConditionInt` de `m_param` 1 (sens inconnu, 40 nœuds dans `Projet1.rvt`) donne le
  même volume vraie ou fausse sur la porte 257 050, seul cas essayé.
- *Objets invisibles* : un nœud dont `GInfo.m_flags` porte les bits 0xE0 est écarté
  (`Options.IncludeNonVisibleObjects` = faux) : volume d'ouverture d'une fenêtre,
  sous-catégorie masquée. Fenêtre 257 071 de `Projet1.rvt` : 0,031 44 m³ sans cette
  règle, 0,022 169 m³ avec, comme Revit.
- *Vides* : solide fermé de volume négatif écarté (§ « Règles établies », 7).

Exemple, porte 257 051 de `Projet1.rvt` : 10 solides dans le type, dont 7 retenus
(huisserie, battant détaillé `GConditionInt` ≠ 1 plutôt que simplifié = 1,
quincaillerie = 3, représentations « non coupé » plutôt que « coupé ») ;
0,145 647 m³ contre 0,145 65 m³ dans Revit.

**Validation contre Revit 2026** (script RiveTT `rvt-reference-familles` : volume des
solides de `get_Geometry`, détail fin, boîte et repère de chaque instance) :

| | `Projet1.rvt` (2026) | `Kerreut 2025.rvt` (2025) |
|---|---|---|
| instances | 102 | 346 |
| volume identique (10⁻⁴ près) | 99 | 274 |
| à moins de 1 % (faces de révolution, tubes) | 3 | 28 |
| à moins de 5 % (chaises : tubes de 1 cm ; porte de garage : `GEllipse`) | — | 33 |
| plus de 5 % (lits : `HermiteSurf` écartées ; receveur de douche : faces sans boucle) | — | 7 |
| sans solide, comme dans Revit | — | 4 |
| volume total | 4,453 347 m³ (Revit 4,453 885) | 53,632 m³ (Revit 54,487) |
| boîte englobante | identique ou contenue dans celle de Revit pour toutes (celle de Revit compte aussi les lignes 2D) | idem |

Saint-Malo : 2 585 instances sur 3 116 maillées ; les autres sont des symboles
(cercles et gabarits PMR), vides (gaines, décaissés), panneaux vides ou plantes
sans volume. Garde-corps : 11 / 11 et 5 / 5 de volume identique à Revit, 96 / 118
maillés à Saint-Malo.

### Topographie (toposolides et surfaces topographiques)

Seuls des **toposolides** sont présents dans le corpus : 40 dans Saint-Malo (terrain
« Contexte 300 », subdivisions « Contexte 5 » et « Enrobé 5 », bordures, terre,
béton) et 22 dans son ancienne version `Appartement_T2_MCP_Test.rvt` (22 / 22
maillés), aucun dans les autres projets. Les anciennes surfaces topographiques
(`SiteSurface`, `TopographySurface` de l'API) n'existent dans le corpus que dans
`LOT2.rvt` (2022) : deux, membres d'un type de groupe jamais placé
(`m_unplacedOwnerId` = un `ElementGroupType`) ; Revit 2026 n'en renvoie aucune, le
lecteur les écarte comme tout élément non placé. Le toposolide dérivant de
`CeilingAndFloor` et non de `Floor`, il n'était pas lu ; sa géométrie (`GElement`) se
maille par le chemin B-rep existant, y compris le terrain de 1 133 faces (faces planes
et cylindriques).

Validation contre l'API de Revit 2026 (script RiveTT `rvt-reference-toposolides` :
volume et aire des solides de `get_Geometry`, nombre de faces, boîte, type, niveau,
paramètres) : nombre de faces, type, niveau, décalage et boîte identiques pour les 40 ; volume et aire identiques à 10⁻⁵ près pour 37, à 4·10⁻⁴ au plus pour les 3 autres (faces cylindriques facettées : bordures courbes) ; total 45 414,805 m³ contre 45 414,807 m³. Terrain 10 126 070 : 44 893,383 6 m³ lus contre
44 893,385 4 m³ (écart 4·10⁻⁸), 1 133 faces dans les deux cas ; toposolide 11 226 899
« Terre20 » : niveau 512 913 et décalage −100 mm comme Revit. Les points d'édition de
forme (`SlabShapeEditor`, 117 sur le terrain) ne sont pas lus : le solide calculé
suffit à l'affichage.

Viewer : classe `IFCGEOGRAPHICELEMENT`, faces colorées par matériau, motifs de surface
compris ; `--stats` donne le bilan « Toposolides ». Saint-Malo : 40 / 40 maillés,
45 414,8 m³.

**Surface topographique** (`SiteSurface`). L'enregistrement 0x66 porte la
triangulation dans `m_drivers` : `TriangulationDriver.m_pMementos`, une suite d'états
`TriangulationDriverMemento` (sommets fixes `m_fixedVerts`, arêtes imposées, contour
`m_boundary`, maillage `m_pMesh`). Le premier état donne les points saisis
(`GetPoints` de l'API) ; une surface qui porte une sous-région en a un second, dont les
sommets ajoutent les intersections avec le contour de la sous-région. `m_manager`
(`ContourManager`) garde en cache les courbes de niveau (polylignes par altitude,
non lues).

La géométrie (0x67) est un `GElement` à un `GFilter` et un `GPolyMesh` : maillage à
facettes, matériau `m_materialID` (le paramètre `MATERIAL_ID_PARAM` de l'API),
topologie `m_pFacetedTopology` de classe `FacetedTopologyN`. D'après le schéma, les
bits de N donnent le format : 1 indices i32 (i16 sinon), 2 points f32 décalés de
`m_offset`, 4 points f64 (f32 sinon), 8 visibilité des arêtes, 16 et 32 coordonnées
(u, v) ; les variantes « t » (`FacetedTopology0t`…) n'ont aucun champ enregistré, non
rencontrées. Facettes triangulaires (`m_facetsArr`, 3 indices) ; une surface et sa
sous-région ne se recouvrent pas : le maillage de la surface est évidé au contour de
la sous-région.

Validation sur l'essai **E15** (`Essais-format/2026/E15_surface_topo.rvt`, créé depuis
V0 par le script RiveTT `rvt-essai-e15-surface-topo` : surface de 42 points en
relief, matériau 90, sous-région rectangulaire de 12 × 10 m), comparé au script
`rvt-reference-toposurfaces` (`GetPoints`, maillage de `get_Geometry`, boîte,
paramètres) :

| Élément | Points | Sommets | Triangles | Aire | Boîte |
|---|---|---|---|---|---|
| surface 2797 | 42 / 42, écart nul | 58 / 58 | 84 / 84 | 829,698725 m² dans les deux cas | identique |
| sous-région 2840 (hôte 2797) | 24 / 24, écart nul | 24 / 24 | 26 / 26 | 120,73165 m² dans les deux cas | identique |

Viewer : classe `IFCGEOGRAPHICELEMENT` (« Surface topographique » ou « Sous-région »,
propriétés surface hôte et nombre de points), couleur du matériau ; `--stats` donne le
bilan « Surfaces topographiques ».

### Hachures

Où elles sont : une région remplie (`FilledRegion`) désigne son type par `m_attrId` ;
le type (`FilledRegionAttributes`, dérivé de `Symbol`) donne le motif de premier plan
`m_fillPatternElemId`, le motif d'arrière-plan `m_backgroundPatternElemId` et sa
couleur `m_backgroundPatternColor` ; la couleur de premier plan est celle de la
sous-catégorie propre au type (`m_lineAndTextAttr.m_categoryId` → `GStyleElem` de
type 1 → `GStyle.m_color`, avec `m_penNumber`). Les matériaux portent quatre motifs
et leurs couleurs : `Material.m_cutPatternId`, `m_cutBackgroundPatternId`,
`m_surfacePatternId`, `m_surfaceBackgroundPatternId` (+ `…Color`).

Un motif est un `FillPatternElem` → `FillPattern` : nom `m_name`, cible `m_fpTarget`
(0 dessin, mesures sur le papier ; 1 modèle), et des grilles `m_gridsArr` →
`FillGrid` : angle `m_angle` (radians), origine `m_origin` (u, v), `m_deltas` =
(glissement le long des traits d'une ligne à la suivante, **décalage** entre deux
traits), tirets `m_segs` (longueurs alternées trait / blanc ; vide : trait continu).
Sans grille : motif uni (« Uni », `<Remplissage de solide>` dans l'API).

Validation contre l'API de Revit 2026 (scripts RiveTT `rvt-reference-hachures` :
`FillPattern.GetFillGrids()`, `FilledRegionType` ; `rvt-reference-motifs-materiaux` :
`Material.CutForegroundPatternId`…) :

| Fichier | Motifs | Types de régions | Matériaux | Régions |
|---|---|---|---|---|
| E13 (2026) | 78 / 78 | 1 / 1 | 9 / 9 | 1 unie, comme Revit |
| `Kerreut 2025.rvt` | 219 / 219 | 10 / 10 | 80 / 80 | 20 unies, 70 hachurées, 3 sans type (id −1 dans l'API), comme Revit |

Viewer : une région est tracée avec son arrière-plan uni s'il y en a un, puis son
premier plan uni (face colorée) ou hachuré : chaque grille donne des traits parallèles
découpés sur la face (triangles), tirets compris, à l'échelle de la vue pour un motif
de dessin, en rubans de 0,13 mm sur le papier, à la couleur de premier plan. Kerreut :
6 745 traits pour 70 régions ; Saint-Malo : 14 344 traits pour 31 régions.

**Motifs de surface sur les faces 3D.** Le calage d'un motif est stocké face par face
dans le `GElement` : `GFace.m_pGFilling` → `GFilling` : motif `m_patternId` (celui de
surface du matériau ; −1 sur certaines faces), couleur `m_fillColor` (celle du motif
de surface du matériau), et `m_placer` → `FillPatternPlacer` : origine `m_origin` et
direction `m_dir` du motif dans l'espace (u, v) de la surface, `m_scale`,
`m_isMirrored`, `m_placedDraft`. Pour une face plane (`Plane` : origine, `m_xVec`,
`m_yVec`), le point (x, y) du motif est en origine du plan + (o + e·(x·d + y·d⊥))
dans le repère (`m_xVec`, `m_yVec`), d⊥ changé de signe si le motif est en miroir ;
dans une famille, ce repère suit celui de l'instance. Contrôle sur Kerreut : sur les
murs en « Blocs 200x400 » (motif de modèle 819), les joints de la grille à 0° sont
horizontaux, ceux de la grille à 90° verticaux, aucun trait oblique, et les joints
horizontaux tombent sur les multiples de 200 mm comptés depuis l'origine du projet,
y compris pour des murs décalés de +100 ou −500 mm (calage par défaut de Revit pour un
motif de modèle). Le calage n'a pas été comparé à une vue de Revit.

Viewer : seuls les **motifs de modèle** sont dessinés (un motif de dessin est à taille
d'écran constante dans Revit ; les faces concernées sont comptées : 59 à Kerreut), et
seulement sur les faces planes. Les traits sont tracés en **lignes d'un pixel**
(`GL_LINES`, ajoutées au moteur de rendu : `ProductGeom::lignes`, plage `lines` des
objets de scène), décalés de 1 mm vers l'extérieur de la face, à la couleur du motif ;
ils forment un objet `IFCSURFACEFEATURE` « Motifs de surface » rattaché à l'élément
(case « Motifs » de la barre d'outils), avec un plafond de 1,5 million de traits par
scène. Kerreut : 21 éléments, 6 849 traits ; Saint-Malo : 82 éléments, 24 175 traits
(scène construite en 0,5 s) ; `Projet1.rvt` : 3 éléments, 28 721 traits.

### Contenu repéré

`Global/Latest` contient le nom des niveaux (`ZZ_NIV_A` y apparaît après
renommage) et des définitions JSON d'unités (`autodesk.unit.unit:…`). Les
partitions contiennent les noms de types, matériaux, familles et chemins de
bibliothèque en UTF-16.

## 8. Outils (`tools/rvt-research/`)

Python 3 sans dépendance externe :

| Script | Rôle |
|---|---|
| `cfb.py` | lecteur OLE (liste et lit les flux) |
| `extract.py` | extrait tous les flux, retire les blocs intercalés, décompresse les membres gzip, rapporte OK/KO (`--brut` : sans retrait) |
| `inflate_ref.py` | décodeur deflate de référence instrumentable (symboles, variantes) |
| `schema.py` | analyse `Formats/Latest` décompressé (en entier), export JSON |
| `rvtdiff.py` | différence entre deux fichiers (flux, membres, sentinelles) |
| `objets.py` | inventaire des éléments par classe, lu dans les partitions (`--liste`, `--classe NOM`, `--tout`) |
| `champs.py` | lecture des champs d'un élément (`fichier id`), couverture du lecteur (`--couverture`), extraction des murs (`--murs`) |
| `elements.py` | niveaux, types de murs, dalles, instances, pièces, surfaces (`fichier niveaux pieces…`) ; objet aplati en `chemin = valeur` (`--plat id`) |
| `geometrie.py` | géométrie calculée d'un élément (`fichier id`) : solides B-rep, sommets, faces et triangles en mm, contrôle (boîte, volume, aire, fermeture, écart des arêtes), export OBJ (`--obj`), plans de référence (`--tout`) |
| `familles.py` | géométrie d'une instance de famille (`fichier id…`) : volume, triangles, boîte en mm ; `--trace id` : solides retenus ou écartés (filtre, invisible, non décodé) et pourquoi |
| `vues.py` | vues et éléments 2D (`fichier [--vues] [--elements] [--json f]`) |
| `feuilles.py` | feuilles : fenêtres de vue (centre, boîte prédits), cartouches et instances de vue (repère, traits avec `--traits`), `--json`, `--args` (prédictions pour `rvt-compare-feuilles`) |
| `crcbloc.py` | code correcteur des flux (§ 4 bis) : `encoder_flux`, `decoder_flux` ; en ligne de commande, réencode chaque flux d'un fichier et compare |
| `ecriture.py` | réécriture d'un fichier (§ 11) : conteneur OLE, `--champ ID CHEMIN VALEUR`, `--texte ID ANCIEN NOUVEAU`, essais `--blocs-zero`, `--signature` |

```bash
python -I tools/rvt-research/objets.py "fichier.rvt" --classe SWall
python -I tools/rvt-research/champs.py "fichier.rvt" --murs
python -I tools/rvt-research/champs.py "fichier.rvt" 2797
python -I tools/rvt-research/elements.py "fichier.rvt" niveaux pieces
python -I tools/rvt-research/elements.py "fichier.rvt" --plat 2797
python -I tools/rvt-research/geometrie.py "fichier.rvt" 2797 --obj mur.obj
python -I tools/rvt-research/familles.py "fichier.rvt" --trace 257051
python -I tools/rvt-research/extract.py "fichier.rvt" sortie/
python -I tools/rvt-research/schema.py sortie/Formats_Latest.bin schema.json
python -I tools/rvt-research/inflate_ref.py "fichier.rvt" Formats/Latest
python -I tools/rvt-research/vues.py "fichier.rvt" --json vues.json
python -I tools/rvt-research/feuilles.py "fichier.rvt" --traits --args %LOCALAPPDATA%/RiveTT/scripts/feuilles.json
python -I tools/rvt-research/crcbloc.py "fichier.rfa"
python -I tools/rvt-research/ecriture.py "source.rfa" "sortie.rfa" --champ 2555139 .m_pMaterial.m_color 0x0000FF
```

**Portage Rust** (viewer, 21/10/2026) : `src/rvt/objets.rs` (`extract.unpad`,
`objets.sections`, `records`, `elem_table`), `schema.rs` (`ClassTable`), `champs.rs`
(`Lecteur`), `geometrie.rs` (`maillage`, `controle`, surfaces réglées et de
révolution, splines d'Hermite, densification des faces courbes, familles :
`Bibliotheque` calcule une fois la géométrie de chaque type et la partage entre
ses instances), `maquette.rs` (`champs.murs`, `elements.*`, plus toits, escaliers,
garde-corps, grilles, familles et matériaux, puis modèle IFC synthétique pour
l'arborescence et les propriétés). Les niveaux sont lus d'abord, puis chaque objet
est lu et réduit aussitôt à ses champs utiles, en parallèle : garder tous les
enregistrements lus portait le pic mémoire de Saint-Malo à 6,2 Go, ramené à 2,1 Go.
Géométrie de tous les éléments en parallèle.

Ajouts du 21/10 au soir : `vues.rs` (vues et éléments 2D), formes des documents de
famille dans `maquette.rs` (`Forme`), `--stats` qui liste formes, vues et éléments
2D.

Ajouts du 22/10 (feuilles, § 7 « Feuilles ») : `feuilles.rs` (valeurs des étiquettes,
mise en page des textes, dessin des instances de vue et des titres, passage modèle →
feuille de chaque fenêtre de vue), `objets::sections_documents` (document de chaque
membre), lecture des documents de famille embarqués (textes, étiquettes, régions,
polices) dans `maquette.rs`, `Bibliotheque::dessins` (traits et faces ouvertes des
types 2D) dans `geometrie.rs`, `src/texte.rs` (glyphes de la police du type, lue dans
les polices de Windows par skrifa, triangulés par earcut ; repli Ubuntu d'egui) et le
rendu en passes de `render.rs` et `app.rs` (fenêtres de vue d'une feuille).

**Interface façon Revit** (22/10) : propriétés à gauche, vue au centre, arborescence
du projet à droite. Onglet « Projet » (fichiers Revit) : *Vues (tout)* par rubrique
(plans d'étage, plans de plafond, vues 3D, élévations, coupes, vues de dessin,
légendes) et type de vue (`DBView.m_dbViewTypeId` → nom du `DBViewType`, par exemple
« Élévation 1 »), *Nomenclatures/Quantités*, *Feuilles* (numéro `m_sheetNumber`, vues
placées par leurs fenêtres de vue), *Familles* par catégorie puis famille et types
(familles chargées, puis familles système d'après la classe du type : `BasicWallType`
→ Murs / Mur de base…), *Groupes* (`ElementGroupType`), *Liens Revit*
(`TransmissionData`). Clic : propriétés ; double-clic sur une vue : elle s'ouvre
(caméra orthographique orientée par `m_viewDir` et `m_vertDir` ; plan : son niveau
seul ; coupe, élévation : tout le modèle ; vue de dessin, feuille : ses éléments 2D),
avec son nom en onglet et son échelle dans la barre d'état ; double-clic sur un type :
ses instances isolées. Onglets « Modèle » (arborescence spatiale) et « Classes »
inchangés ; champ « Rechercher » commun.

Dans la scène : murs `IFCWALL`, sols `IFCSLAB`, plafonds `IFCCOVERING`, toits
`IFCROOF`, escaliers `IFCSTAIR` (volées et paliers réunis), garde-corps
`IFCRAILING`, pièces placées `IFCSPACE` (masquées par défaut, comme dans un IFC),
instances selon leur catégorie (`IFCDOOR`, `IFCWINDOW`, `IFCFURNITURE`…,
`IFCBUILDINGELEMENTPROXY` sinon) avec leur géométrie de famille, grilles `IFCGRID`
(propriétés seulement), formes d'un `.rfa` dans la classe de la catégorie de la
famille (`IFCWINDOW` pour une fenêtre), une vue par nœud `IFCANNOTATION` (dans son
niveau pour un plan) avec ses éléments 2D en enfants, tracés dans le plan de la vue
(traits et cotes en rubans de 0,25 mm sur le papier, cadre des textes, régions) et
masqués par défaut (case « Annotations » ; affichés d'office pour une famille 2D
comme un cartouche). Portes et fenêtres : « Largeur (panneau) » et « Hauteur
(panneau) », paramètres de type. Couleur de chaque face : celle de son matériau,
transparence comprise (plafonnée à 90 %) ; 99 % des triangles des familles ont un
matériau connu, les autres (« par catégorie ») prennent la couleur de la classe.
Les instances propres à une vue (cartouches, symboles, composants de détail) sont
dessinées dans leur vue ; notes de texte en glyphes. Double-clic sur une feuille : son
cadre et ses cartouches, ses vues placées découpées à leur cadrage et ses titres
(§ 7, « Feuilles »).

| Fichier | Format | Maillés par le viewer (`--stats`) |
|---|---|---|
| E04, E09, E11 | 2026 | murs 1 / 1 chacun ; sommets, volume, aire, faces et triangles identiques à `geometrie.py` |
| `Projet1.rvt` | 2026 | murs 59 / 59 (257,974 221 m³, 3 768 triangles), sols 6 / 6, escaliers 2 / 2, garde-corps 11 / 11, pièces 39 / 39, instances 102 / 102 |
| `Kerreut 2025.rvt` | 2025 | murs 97 / 108, sols et plafonds 16 / 16, toits 10 / 10, escaliers 2 / 2, garde-corps 5 / 5, pièces 19 / 19, instances 342 / 346 |
| `Langan.rvt`, `BIBLIO.rvt`, `Guidel.rvt` | 2025 | murs 16 / 16, 20 / 21, 5 / 5 ; toits 2 / 2, —, 1 / 1 ; instances 2 / 2, 65 / 73, — |
| `Saint-cast-Le-Guildo.rvt` | 2025 | murs 16 / 16 (fichier absent du poste le 21/10, non relancé) |
| `GABARIT JBA.rte` | 2016 | murs 12 / 12 (non relancé le 21/10) |
| `Colombes.rvt` | 2022 | murs 41 / 41, sols 3 / 3, toits 4 / 4, escaliers 2 / 2, garde-corps 1 / 3, pièces 9 / 10, instances 49 / 49 |
| `Essai projet 2027 astra.rvt` | 2027 | murs 39 / 39, sols 4 / 4, escaliers 1 / 1, instances 12 / 12, garde-corps 0 / 4 |
| `Saint-Malo_avenue aristide briand_46.rvt` et sa version 2027 `Phase2_Lecture.rvt` | 2026, 2027 | résultats identiques : murs 2 219 / 2 235 (16 murs-rideaux sans solide propre), sols et plafonds 92 / 92, toits 10 / 10, escaliers 14 / 14, garde-corps 96 / 118, pièces 137 / 137, instances 2 585 / 3 116 ; lecture complète en 4 s |
| `Kerreutv9.rvt` | 2022 | comme `Kerreut 2025.rvt` (même maquette) |

Tests (`cargo test`) : cotes de Kerreut (visibilité, lignes d'esquisse, catégories
et éléments masqués, format, valeurs remplacées), format des longueurs (sans fichier),
feuilles de Kerreut (centres et boîtes des fenêtres de vue,
repère du cartouche, traits des types, étiquettes et leurs valeurs, titres de vue :
« Titre sur la feuille », visibilité du numéro, cadrage d'annotation), de `Projet1.rvt`
et d'E13 (boîtes, titre), glyphes des textes (sans fichier), E04 (volume, boîte), `Projet1.rvt` (59 murs, matériaux, sols,
escaliers, garde-corps, pièces, instances et leur géométrie, cotes d'une porte),
`Kerreut 2025.rvt` (toits, escalier assemblé, familles, arbre aux surfaces de
révolution, cotes d'une porte), E12 (grilles), `FEN_2PF.rfa` (formes, vues, cotes et
textes), C14 F04 (fenêtres placées), E13 (vues et 2D, objets de la scène), Kerreut
(hachures ; motifs de surface : orientation et trame des joints), Saint-Malo
(toposolides), E15 (surface topographique et sous-région), découpage des
hachures sur un carré et texte RTF (sans fichier), sautés si les fichiers sont absents.
`--stats` donne aussi le bilan de la scène (objets, triangles, traits des motifs).
Test de l'arborescence du projet : vues de `FEN_2PF.rfa` (genre, type, nom),
feuilles numérotées de `Projet1.rvt` et leurs vues placées, nomenclatures. Dossiers : `RVT_ESSAIS` (défaut `C:\Users\theba\Desktop\Revit`) et
`RVT_CORPUS` (défaut `C:\Users\theba\Documents\Thomas\ARCHI PERSO`).

## 9. Pistes pour la suite

1. **Autres éléments** : faits et portés en Rust pour niveaux, types, familles,
   matériaux, sols, plafonds, toits, escaliers, garde-corps, grilles, instances et
   pièces (§ 7, § 8). Restent la position des éléments 2D de vue, les surfaces
   (validées seulement en nombre) et les paramètres en général (seuls quelques
   BuiltInParameter précis sont lus ; les paramètres de type sont dans
   `FamilySymbol.m_pParams`, les noms des paramètres de famille restent à trouver).
2. **Géométrie** : surfaces `HermiteSurf`, `ConeSurf`, `OffsetSurf` et courbe
   `GEllipse` ; faces sans boucle (receveur de douche de Kerreut : maillage ouvert) ;
   22 garde-corps de Saint-Malo sans solide.
3. **Objets hors `ElemTable`** : confirmer qu'il s'agit du contenu des familles
   (`Global/ContentDocuments`, 40 Mo décompressés dans le projet 2027).
4. **Inconnus restants** : `ElementHeader`, h2 des membres, les 64 octets de fin de
   document (non contrôlés à l'ouverture). Résolus le 21/10 : u32 des enregistrements
   (Adler-32), bloc de 353 octets et fin des flux (code correcteur, § 4 bis).
5. **Écriture** (§ 11) : porter `crcbloc.py` et `ecriture.py` en Rust ; écrire des
   éléments nouveaux (pas seulement modifier des champs existants), ce qui demande
   un sérialiseur complet (inverse du lecteur) et la mise à jour de `ElemTable`.
6. **2D** : faits le 22/10 : position des fenêtres de vue, cartouches, symboles et
   composants de détail, étiquettes, tracé des textes et des cotes, visibilité dans
   une vue (§ 7, « Feuilles », « Cotes et visibilité »). Restent les étiquettes de
   pièces, de portes et de matériaux, les niveaux, repères de vue et cotes de niveau
   dans les vues, le contour des vues non cadrées et les débords d'annotations (boîte
   de l'API), les filtres de vue et le rendu en lignes cachées.

## 10. Comparaison avec rvt-rs (licence Apache-2.0)

[rvt-rs](https://github.com/DrunkOnJava/rvt-rs) (Griffin Long, crate `rvt` 0.6.0,
Apache-2.0) est un lecteur Rust de RVT développé indépendamment. Il a été **consulté
partiellement, comme référence de comparaison seulement**, le 21/10/2026 :

- consulté : `src/compression.rs`, pour confirmer le bloc de 353 octets tous les
  64 896 octets (même règle que § 4, trouvée ici de notre côté le 20/10), et la
  liste des modules d'éléments, pour savoir quelles catégories il couvre ;
- **aucun code, aucune table ni aucune donnée de rvt-rs n'est repris** dans ce
  dépôt. Les catégories ajoutées le 21/10 (toits, escaliers, grilles, matériaux)
  ont été décodées ici à partir du schéma `Formats/Latest` (noms de classes et de
  champs) et validées contre l'API de Revit par RiveTT ; les ids de paramètres et de
  catégories viennent de l'API publique de Revit ;
- pas d'export IFC ni de versions antérieures à 2024 tirés de rvt-rs.

Campagne C16 (feuilles, 22/10/2026) : **rvt-rs non consulté** ; tout vient du schéma,
des fichiers et de l'API de Revit (RiveTT).

Différence de méthode : rvt-rs extrait la plupart des éléments par motifs d'octets
dans les partitions et recalcule la géométrie depuis les axes et esquisses ; ce
dépôt lit tous les objets par le schéma et reprend le solide calculé par Revit
(`GElement`). Si du code de rvt-rs devait être repris un jour, il faudrait
conserver sa licence et son avis `NOTICE` pour les fichiers concernés.

## 11. Écriture (campagne C14, 21/10/2026)

Chaîne d'écriture de `tools/rvt-research/ecriture.py` : décoder le flux
(`crcbloc.decoder_flux`), modifier un enregistrement 0x66 (valeur repérée par le
lecteur, qui note la position de chaque champ simple), recalculer son Adler-32,
recompresser son membre gzip, mettre à jour g + 8 et la somme des corps de son
en-tête et le lien `21 0F` suivant, réencoder le flux (`crcbloc.encoder_flux`) et
réécrire le conteneur OLE (même arborescence, nouveaux secteurs).

Essais depuis `FEN_2PF.rfa`, ouverts dans Revit 2026 en arrière-plan par le script
RiveTT `rvt-essai-ouverture` (intercepteur de boîtes de dialogue, fermeture sans
enregistrer) :

| Essai | Modification | Revit |
|---|---|---|
| W0 | conteneur OLE réécrit, flux identiques | ouvert ; 2 341 éléments, 19 formes, 6 types |
| W1 | CRC des flux mis à zéro | **refusé** : `CorruptModelException` (`CArchiveException 105`) après 15 s de tentative de réparation |
| W2b | un bit des 64 octets de fin de document inversé, CRC recalculés | ouvert |
| W4 | texte « Extérieur » remplacé par `ZZ_ECRIT_A` | ouvert ; `TextNote.Text` = `ZZ_ECRIT_A` |
| W5 | couleur du matériau `ARC_MAT_BETON` : 0xB4B4B4 → 0x0000FF | ouvert ; `Material.Color` = 255, 0, 0 |
| W6 | largeur du type `120x250ht` : 1 200 → 1 400 mm (`m_pFamilyTypes`) | ouvert ; `Largeur` = 1 400 mm ; type activé et régénéré : boîte −720 → 720 mm, 0,145 39 m³ (0,130 054 avant) |

Aucune boîte de dialogue, aucun avertissement. Les modifications de longueur
(texte plus court de 2 octets dans W4) sont acceptées.
