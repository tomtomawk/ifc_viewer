# Format Revit (.rvt 2027) : état du décodage

Notes de rétro-ingénierie du 19–20 octobre 2026. Elles décrivent ce qui a été
**observé** sur des fichiers réels, pas une spécification : rien n'est documenté
par Autodesk. Les points non vérifiés sont signalés comme tels.

## En bref

| Partie | État |
|---|---|
| Conteneur OLE (CFB) | lu entièrement |
| Métadonnées (version, projet, références, vignette) | décodées, intégrées au viewer (`src/rvt.rs`) |
| Schéma de classes `Formats/Latest` | grammaire trouvée ; 26 % du flux analysé (1 106 classes, 3 023 champs) |
| Table des éléments `Global/ElemTable` | structure partielle (enregistrements de 40 octets) |
| Données des éléments `Partitions/*` | non décodées |
| Décompression | **bloquante** : 22 % des membres gzip ne se décodent pas correctement avec un inflate standard |

Pour obtenir la géométrie aujourd'hui : exporter en IFC depuis Revit (testé avec
RiveTT, IFC4 RV et IFC2x3 CV2, tous deux lus par le viewer).

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
| `Global/ContentDocuments` | 7 222 680 | gzip, non décodé (voir § 4) |
| `Partitions/0` | 103 593 406 | 7 086 membres gzip : données des éléments (noms de types, matériaux…) |

Variantes :
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
- `Partitions/N` : 12 octets d'en-tête de partition (`09 00 00 00 00 00 00 00 B7 03 00 00`
  pour `Partitions/0`), puis des membres gzip, chacun précédé d'un en-tête de
  32 octets (8 × u32). Relevé sur les 7 086 membres de `Partitions/0`
  (interprétation **non confirmée**) :
  - champ 3 = 4 dans 90 % des cas ;
  - champ 5 = longueur du membre gzip + 8 dans 63 % des cas. Les autres écarts
    (−345, −180, −533 octets…) indiquent des données hors gzip entre certains
    membres (non étudiées) ;
  - les 16 bits hauts du champ 1 reprennent le champ 5 du membre précédent dans 83 % des cas ;
  - champ 7 = 0x65, 0x66 ou 0x67 (101 à 103) ; champ 8 = 0 (une exception).

  Les membres décompressés font jusqu'à environ 128 Ko.

## 4. Le blocage : les membres gzip « KO »

Un membre KO se décompresse sans erreur, ou avec une erreur tardive, mais son
contenu ne correspond pas au CRC32 et à la taille (ISIZE) de son trailer gzip.

`Essai projet 2027 astra.rvt` (relevé par `tools/rvt-research/extract.py`) :

| Flux | Résultat |
|---|---|
| `Formats/Latest` | KO : 516 043 octets décodés pour ISIZE 513 948 |
| `Global/Latest` | KO : 2 044 648 pour 2 035 518 |
| `Global/ElemTable` | KO : 1 334 825 pour 1 329 430 |
| `Global/ContentDocuments` | erreur zlib « invalid stored block lengths » |
| `Global/History`, `PartitionTable`, `DocumentIncrementTable`, `Contents` | OK |
| `Partitions/0` | 5 547 OK, 1 539 KO (22 %) sur 7 086 membres |

Le phénomène touche **tous** les fichiers testés, 2026 comme 2027 : par exemple
878 KO sur 2 645 membres dans `Partitions/196` du gabarit 2026, 213 KO sur 226
dans `Partitions/170` de Saint-Malo, 11 KO sur 31 dans une famille de 1,2 Mo.

### Ce qui est établi

- **Ce n'est pas une corruption du fichier** : le phénomène est systématique sur
  six fichiers, et Revit les ouvre.
- **Ce n'est pas une erreur de lecture OLE** : les chaînes de secteurs sont
  contiguës, et les membres OK se décodent avec CRC exact.
- **Le flux de bits reste synchronisé** : le décodeur atteint la fin du flux deflate
  exactement à l'endroit du trailer gzip, et le trailer est plausible. Seules les
  **valeurs** décodées divergent, pas la structure.
- **La sortie est correcte jusqu'à un point, puis devient du texte brouillé**
  (fragments recopiés au mauvais endroit) : `Formats/Latest` à partir d'environ
  136 Ko (0x215B0), `Global/Latest` vers 1,5 Mo (0x178000), `Global/ElemTable`
  à l'enregistrement 2 978.
- **La sortie est toujours trop longue** (décodé > ISIZE). Certains membres
  provoquent « invalid distance too far back » ou « invalid code lengths set ».

### Hypothèses écartées

| Hypothèse | Test | Résultat |
|---|---|---|
| Inversion d'un bit (corruption disque) | les 85 000 inversions possibles sur un membre de 10 Ko | aucune ne rétablit le CRC |
| Deflate64 (fenêtre 64 Ko, code 285 étendu) | décodeur `inflate_ref.py` modifié | pire : 549 347 octets |
| Bits supplémentaires en ordre inverse | 4 combinaisons longueur/distance sur 3 membres | aucune ne rétablit le CRC |
| Table des longueurs modifiée pour un seul code | calcul sur le nombre d'usages de chaque code | aucune solution entière |

### Indice non concluant

Sur 81 membres de deux familles, les membres KO ont plus souvent plusieurs blocs
deflate dynamiques (18 KO sur 23 à 3 blocs ou plus, contre 3 KO sur 42 à 1 bloc).
Mais dans le membre étudié en détail, la divergence commence **dans le premier
bloc** : le nombre de blocs ne suffit donc pas à l'expliquer.

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

Codes de type observés :

| Code | Sens | Exemple |
|---|---|---|
| 0x01 | booléen | `m_corruptDocument` |
| 0x02 | entier 16 bits | `Dimension.m_flags` |
| 0x04 | entier 32 bits | `m_dimVersion` |
| 0x07 | double | `m_lockedValue` ; `double[3]` avec sous-code 0x10 |
| 0x08 | chaîne (sous-code 0x60) | `ADTGridTextLocation.m_text` |
| 0x09 | GUID | `GUIDvalue.m_guid` |
| 0x0B | entier 64 bits | `Identifier.m_id64` |
| 0x0D | tableau de tableaux | `Trf.m_3x3` = double[3][3] |
| 0x0E | classe, référence ou pointeur | `m_ownerFamilyId` (ElementId) |

Sous-codes : 0x10 tableau fixe, 0x50 conteneur, 0x51 conteneur de pointeurs,
0x11 tableau fixe de pointeurs. Les sous-codes 0x01, 0x02 et 0x03 sont des
pointeurs ; leur nuance (possédé, partagé, faible ?) n'est **pas établie**.

Exemples décodés :
- `ElementId` → `m_id : Identifier` → `m_id64 : i64` (ids de classe 0x14 et 0x15) ;
- `ADocument` (version 2663) : `m_elemTable`, `m_pPartitionTable`, `m_pHistory`…,
  suivi de 1 568 GUID ;
- `Dimension` (parent `Element`, id 0x26, version 27) : 17 champs puis 2 GUID ;
  elle est définie en ligne comme parent de `DimWithSegRefs` (version 4, 6 champs),
  elle-même parent de `LinearConstraint` ;
- `Trf` : `m_3x3 : double[3][3]`, `m_or : double[3]` (matrice de transformation).

Résultat : 687 classes de premier niveau et 419 définies en ligne, soit 1 106
classes et 3 023 champs. L'analyse s'arrête à 0x215FA sur 0x7DFCB (26 %),
exactement où la décompression diverge.

## 6. Table des éléments (`Global/ElemTable`)

En-tête de deux u16 (1 510 et 33 235), puis des enregistrements de 40 octets à
partir de l'octet 6, contenant un identifiant croissant (1, 2, 3…), des valeurs
répétées et des blocs `FF FF FF FF FF FF FF FF`. Lecture cohérente jusqu'à
l'enregistrement 2 978, où la décompression diverge. Sens des champs non établi.
Pour comparaison, Revit annonce 8 401 éléments pour ce modèle.

## 7. Contenu visible des partitions

Les membres OK de `Partitions/0` (434 Mo décompressés) contiennent
21 846 chaînes UTF-16 distinctes : noms de types (« Plancher 10cm Dalle,
ép=200 »), matériaux, familles, chemins de bibliothèque (`A:\00_BIBLIOTHEQUE\…`),
fichiers DWG liés. `Global/Latest` contient notamment des définitions JSON
d'unités et de spécifications (`autodesk.unit.unit:…`, `autodesk.spec…`).
La sérialisation binaire des objets n'a pas été étudiée.

## 8. Outils (`tools/rvt-research/`)

Python 3 sans dépendance externe :

| Script | Rôle |
|---|---|
| `cfb.py` | lecteur OLE (liste et lit les flux) |
| `extract.py` | extrait tous les flux, décompresse les membres gzip, rapporte OK/KO |
| `inflate_ref.py` | décodeur deflate de référence instrumentable (symboles, variantes) |
| `schema.py` | analyse `Formats/Latest` décompressé, export JSON |

```bash
python -I tools/rvt-research/extract.py "fichier.rvt" sortie/
python -I tools/rvt-research/schema.py sortie/Formats_Latest.bin schema.json
python -I tools/rvt-research/inflate_ref.py "fichier.rvt" Formats/Latest
```

## 9. Pistes pour la suite

1. **Lever le blocage de décompression**, condition de tout le reste :
   - partir du plus petit membre KO (environ 1,5 Ko compressé dans
     `Essai projet 2027 astra.rvt`) et localiser le premier symbole faux ;
   - obtenir un texte de référence : des valeurs connues côté Revit (noms,
     paramètres lus via RiveTT) qui doivent apparaître dans la sortie, pour
     repérer précisément où elle commence à diverger ;
   - comparer avec une sauvegarde de Revit 2027 d'un modèle minimal (un mur), en
     faisant varier un seul paramètre entre deux enregistrements.
2. **Associer `ElemTable` aux ElementId** : comparer les enregistrements aux
   identifiants exportés par RiveTT (`export_elements_data`).
3. **Décoder un objet simple** (niveau : nom et altitude connus) dans les
   partitions, avec le schéma de classes.
