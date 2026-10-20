# Format Revit (.rvt) : état du décodage

Notes de rétro-ingénierie du 19–20 octobre 2026. Elles décrivent ce qui a été
**observé** sur des fichiers réels, pas une spécification : rien n'est documenté
par Autodesk. Les points non vérifiés sont signalés comme tels. Méthode d'essai :
[PROTOCOLE-RVT.md](PROTOCOLE-RVT.md).

## En bref

| Partie | État |
|---|---|
| Conteneur OLE (CFB) | lu entièrement, validé par l'API Windows |
| Métadonnées (version, projet, références, vignette) | décodées, intégrées au viewer (`src/rvt.rs`) |
| Décompression | **résolue** (§ 4) : blocs de 353 octets intercalés tous les 64 896 octets ; 100 % des membres gzip décodés avec CRC exact (formats 2016, 2026, 2027) |
| Schéma de classes `Formats/Latest` | **lu en entier** : 4 690 classes et 12 558 champs (2026), 4 757 et 13 080 (2027) ; identifiants de classe établis |
| Table des éléments `Global/ElemTable` | structure établie (en-tête, enregistrements de 40 octets) ; un champ non identifié |
| Partitions `Partitions/*` | **cadrage complet** : 3 sections, un enregistrement par élément ; inventaire identique à Revit (§ 7) |
| Champs des objets | **lus en entier** par un lecteur générique piloté par le schéma : 100 % des enregistrements sur les fichiers 2016, 2022, 2025, 2026 et 2027 essayés, hors données d'add-ins (§ 7) |
| Murs | type, niveaux, ligne (droite ou arc), hauteur et décalages extraits ; identiques à Revit sur 75 murs de deux projets et deux essais (§ 7) |
| Autres éléments | niveaux, types de murs (épaisseur), dalles, instances de familles (type, hôte, position, repère), pièces : identiques à Revit sur deux projets (§ 7) |
| Géométrie calculée (`GElement`) | B-rep des murs lu et maillé (faces planes et cylindriques, trous) ; volume, aire et faces identiques à Revit sur 59 murs et 3 essais (§ 7) |
| Viewer Rust | **murs affichés** à l'ouverture d'un `.rvt`, sans export IFC (`src/rvt/`, § 8) ; mêmes sommets et volumes que `geometrie.py` |

Le viewer n'affiche que les murs. Pour les autres éléments : exporter en IFC depuis
Revit (testé avec RiveTT, IFC4 RV et IFC2x3 CV2, tous deux lus par le viewer).

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
| `Kerreutv9.rvt` | projet d'agence | 2022 | 138,2 Mo |
| `Essais-format/2026/*.rvt` | essais produits par RiveTT (protocole) | 2026 | 0,5 à 1,6 Mo |

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
- `Partitions/N` : 12 octets d'en-tête de partition (`09 00 00 00 00 00 00 00`
  puis un u32), puis des membres gzip, chacun précédé d'un en-tête de 32 octets
  (8 × u32, notés h0 à h7). Relevé **après retrait des blocs intercalés** (§ 4)
  sur les 7 088 membres de `Essai projet 2027 astra.rvt`, sens **non établi** :
  - h4 = longueur du membre gzip + 8 dans 83 % des cas ; les autres membres sont
    suivis de données hors gzip (non étudiées) ;
  - les 16 bits hauts de h0 reprennent les 16 bits bas du h4 précédent dans les mêmes 83 % ;
  - h2 = 4 (90 %), 5, 6 ou 7 ; h6 = 0x65, 0x66 ou 0x67 ;
  - h5 est inférieur à la taille décompressée dans 97 % des cas.

  Les membres décompressés font généralement un peu moins de 128 Ko (jusqu'à 131 088 octets).
- Chaque flux gzip est suivi, après son trailer, de quelques dizaines d'octets à
  forte entropie (35 à 704 observés), sans rôle identifié.

## 4. Décompression : les blocs intercalés (résolu le 20/10/2026)

### Règle

Chaque flux OLE compressé (`Formats/Latest`, `Global/*`, `Partitions/*`) est
découpé en tranches de **65 249 octets : 64 896 octets de données, puis un bloc
de 353 octets** à retirer. Le bloc commence par `00` suivi d'un octet multiple de
0x10 (`00 00` à `00 F0`) ; les 351 octets suivants sont à forte entropie et
diffèrent d'un bloc à l'autre (rôle inconnu). Une fois ces blocs retirés, les
membres gzip sont standard.

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
sont très probablement le contenu des familles chargées (98 000 `CategoryElem`,
100 000 `GStyleElem`) ; non vérifié.

**Validation** contre Revit 2026 sur `Projet1.rvt` (comptes RiveTT / outil
`objets.py`) : murs 59 / 59 `SWall`, niveaux 3 / 3 `Level`, pièces 39 / 39
`RoomElem`, instances de familles 102 / 102 `FamilyInstance`, vues en plan
11 / 11 `DBViewPlan`.

Classes utiles relevées : `SWall` (murs), `Level`, `RoomElem` et `LevelRoomPlan`
(pièces), `FamilyInstance`, `DBViewPlan`, `CurveElem` (lignes), `SketchPlane`.

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
| pièce | `RoomElem` sans `m_areaSchemeId` ni `m_zoneSchemeId` | `ROOM_NAME` (−1 006 900), `ROOM_NUMBER` (−1 006 901), `ROOM_DEPARTMENT` (−1 006 907), niveau `m_assocLevelId`, point `m_point` ; pièce non placée : `m_bIsLocationless` = 1 |
| surface (plan de surfaces) | `RoomElem` avec `m_areaSchemeId` | mêmes paramètres |
| zone | `RoomElem` avec `m_zoneSchemeId` | idem |

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

**Limites.** Surfaces décodées : `Plane`, `CylSurf`. Non décodées : `ConeSurf`,
`HermiteSurf`, `OffsetSurf`, `RuledSurf`, `SurfRev`, `TempImportNurbsSurf`,
`SharedSurface`. Dans `Projet1.rvt`, 397 des 398 `GElement` (murs, pièces, dalles,
garde-corps…) se lisent sans erreur ; l'échec est un `SurfRev` (profil
`m_pProfileCurve`) d'un `FamilySymbol`. Les `GElement` des instances de familles
ne contiennent pas de solide mais un `GInstance` (`m_instanceInfo`,
`m_oEmbeddedSymbolGRep`…) qui renvoie à la géométrie du type : non décodé.
Dalles et pièces : maillées, pas encore comparées à Revit. Triangulation en
O(n³) par face : suffisante pour des murs, à revoir pour de grandes faces.

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

```bash
python -I tools/rvt-research/objets.py "fichier.rvt" --classe SWall
python -I tools/rvt-research/champs.py "fichier.rvt" --murs
python -I tools/rvt-research/champs.py "fichier.rvt" 2797
python -I tools/rvt-research/elements.py "fichier.rvt" niveaux pieces
python -I tools/rvt-research/elements.py "fichier.rvt" --plat 2797
python -I tools/rvt-research/geometrie.py "fichier.rvt" 2797 --obj mur.obj
python -I tools/rvt-research/extract.py "fichier.rvt" sortie/
python -I tools/rvt-research/schema.py sortie/Formats_Latest.bin schema.json
python -I tools/rvt-research/inflate_ref.py "fichier.rvt" Formats/Latest
```

**Portage Rust** (viewer, 21/10/2026) : `src/rvt/objets.rs` (`extract.unpad`,
`objets.sections`, `records`, `elem_table`), `schema.rs` (`ClassTable`), `champs.rs`
(`Lecteur`), `geometrie.rs` (`maillage`, `controle`), `maquette.rs` (`champs.murs`,
`elements.niveaux`, `elements.types_murs`, puis modèle IFC synthétique pour
l'arborescence et les propriétés). Seuls les murs (`SWall`, `ArcWall`) sont lus.

| Fichier | Format | Murs maillés par le viewer |
|---|---|---|
| E04, E09, E11 | 2026 | 1 / 1 chacun ; sommets, volume, aire, faces et triangles identiques à `geometrie.py` |
| `Projet1.rvt` | 2026 | 59 / 59, identiques à `geometrie.py` (257,974 221 m³, 3 768 triangles) |
| `Saint-cast-Le-Guildo.rvt` | 2025 | 16 / 16 |
| `GABARIT JBA.rte` | 2016 | 12 / 12 |
| `Essai projet 2027 astra.rvt` | 2027 | 39 / 39 |
| `Saint-Malo_avenue aristide briand_46.rvt` | 2026 | 2 235 / 2 235, lecture en 2,3 s |
| `Kerreutv9.rvt` | 2022 | 95 / 108 : 2 boucles discontinues et 11 `GElement` sans solide fermé, mêmes échecs que `geometrie.py` |

Tests (`cargo test`) : E04 (volume, boîte) et `Projet1.rvt` (59 murs, volume total),
sautés si les fichiers d'essai sont absents.

## 9. Pistes pour la suite

1. **Autres éléments** : faits pour niveaux, types de murs, dalles, instances et
   pièces (§ 7). Restent le contour des dalles et des pièces, les toits, escaliers,
   garde-corps, murs-rideaux, la position des éléments 2D de vue, et les surfaces
   (validées seulement en nombre).
2. **Géométrie calculée** : décodée et validée pour les murs (§ 7). Suite :
   surfaces restantes (`SurfRev`, `RuledSurf`, `HermiteSurf`…), `GInstance` des
   instances de familles (géométrie du type et placement), comparaison des dalles
   et pièces avec Revit. Portage Rust fait pour les murs (§ 8) ; à étendre aux
   dalles, pièces et instances au fur et à mesure de leur validation, puis couleurs
   par matériau (`GFace.m_renderStyleId`, nom et couleur des `MaterialElem` à décoder).
3. **Objets hors `ElemTable`** : confirmer qu'il s'agit du contenu des familles
   (`Global/ContentDocuments`, 40 Mo décompressés dans le projet 2027).
4. **Inconnus restants** : u32 des enregistrements 0x66, `ElementHeader`, octets de
   fin des flux gzip, bloc de 353 octets (rôle inconnu, nécessaire pour écrire).
