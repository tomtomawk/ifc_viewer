//! Flux compressés et cadrage des partitions (LISEZMOI-RVT.md du dossier Decodage RVT,
//! § 3, 4, 6 et 7).
//!
//! Chaque flux compressé est une suite de tranches de 65 249 octets : 64 896 octets
//! de données puis un bloc intercalé de 353 octets à retirer avant décompression.
//! Une partition est ensuite une suite de membres gzip, chacun précédé d'un en-tête
//! de 32 octets (8 × u32) dont le 7ᵉ (h6) donne la section :
//! - 0x65 : un `ElementHeader` par élément, `[id][u32 n][n octets][u32 n]` ;
//! - 0x66 : l'objet de l'élément, `[id][u32 ?][u32 n][n octets][u32 n]` ;
//! - 0x67 : un complément (géométrie `GElement`…), même cadrage que 0x66.

use rayon::prelude::*;
use std::collections::{BTreeMap, HashMap};

const DATA: usize = 64_896;
const PAD: usize = 353;
/// En-tête gzip fixe des membres écrits par Revit (deflate, sans nom ni date).
const GZIP: [u8; 10] = [0x1F, 0x8B, 0x08, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x0B];

pub fn u16_at(b: &[u8], o: usize) -> Option<u16> {
    Some(u16::from_le_bytes(b.get(o..o + 2)?.try_into().ok()?))
}

pub fn u32_at(b: &[u8], o: usize) -> Option<u32> {
    Some(u32::from_le_bytes(b.get(o..o + 4)?.try_into().ok()?))
}

fn i64_at(b: &[u8], o: usize) -> Option<i64> {
    Some(i64::from_le_bytes(b.get(o..o + 8)?.try_into().ok()?))
}

fn i32_at(b: &[u8], o: usize) -> Option<i32> {
    Some(i32::from_le_bytes(b.get(o..o + 4)?.try_into().ok()?))
}

/// Retire les blocs intercalés de 353 octets.
pub fn unpad(b: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(b.len());
    for tranche in b.chunks(DATA + PAD) {
        out.extend_from_slice(&tranche[..tranche.len().min(DATA)]);
    }
    out
}

/// Positions des en-têtes gzip (sans chevauchement, comme `re.finditer`).
fn gzip_heads(b: &[u8]) -> Vec<usize> {
    let mut out = Vec::new();
    let mut i = 0;
    while i + GZIP.len() <= b.len() {
        if b[i] == GZIP[0] && b[i..i + GZIP.len()] == GZIP {
            out.push(i);
            i += GZIP.len();
        } else {
            i += 1;
        }
    }
    out
}

/// Décompresse un membre gzip (en-tête fixe de 10 octets puis deflate brut). Les
/// octets qui suivent la fin du flux deflate (trailer, données hors gzip) sont ignorés.
pub fn inflate(b: &[u8]) -> Result<Vec<u8>, String> {
    if !b.starts_with(&GZIP) {
        return Err("membre gzip attendu".into());
    }
    miniz_oxide::inflate::decompress_to_vec(&b[GZIP.len()..]).map_err(|e| format!("décompression : {:?}", e.status))
}

/// Flux `Global/*` : 8 octets de préfixe, puis un membre gzip.
pub fn inflate_global(raw: &[u8]) -> Result<Vec<u8>, String> {
    let b = unpad(raw);
    inflate(b.get(8..).ok_or("flux Global tronqué")?)
}

/// Document d'une partition auquel appartient un membre : `None` pour le document
/// principal, sinon le GUID du document embarqué (famille chargée).
pub type Document = Option<[u8; 16]>;

/// {h6 : données décompressées concaténées} d'un flux de partition. Les membres sont
/// décompressés en parallèle ; un enregistrement peut déborder sur le membre suivant
/// de la même section, d'où la concaténation dans l'ordre. Pour chaque section aussi,
/// le début (position dans les données concaténées) de chacun de ses membres et le
/// document auquel il appartient.
/// `marqueurs` : ids de classe de `ContentMarker` et `ContentKey` (propres à chaque
/// version du schéma). Un document commence par un `ContentMarker` collé à l'en-tête
/// de son premier membre : u16 classe, u32 0 et u32 nombre d'éléments pour le document
/// principal ; u16 classe, u32 FFFFFFFF, u16 `ContentKey`, u32 nombre d'éléments puis
/// le GUID (corps du pointeur, écrit après les champs) pour un document embarqué.
pub fn sections_documents(raw: &[u8], marqueurs: Option<(u16, u16)>) -> Result<(BTreeMap<u32, Vec<u8>>, BTreeMap<u32, Vec<(usize, Document)>>), String> {
    let b = unpad(raw);
    let heads = gzip_heads(&b);
    // document de chaque membre : le dernier marqueur rencontré
    let mut docs: Vec<Document> = Vec::with_capacity(heads.len());
    let mut doc: Document = None;
    for &h in &heads {
        if let (Some((cm, ck)), Some(o)) = (marqueurs, h.checked_sub(54)) {
            if u16_at(&b, o) == Some(cm) && u32_at(&b, o + 2) == Some(u32::MAX) && u16_at(&b, o + 6) == Some(ck) {
                doc = b.get(h - 42..h - 26).and_then(|g| g.try_into().ok());
            } else if u16_at(&b, h - 36) == Some(cm) && u32_at(&b, h - 34) == Some(0) {
                doc = None;
            }
        }
        docs.push(doc);
    }
    let membres: Vec<Result<(u32, Vec<u8>), String>> = (0..heads.len())
        .into_par_iter()
        .map(|i| {
            let h = heads[i];
            let end = if i + 1 < heads.len() { heads[i + 1].saturating_sub(32) } else { b.len() };
            let h6 = h.checked_sub(32).and_then(|o| u32_at(&b, o + 24)).ok_or("en-tête de membre absent")?;
            Ok((h6, inflate(b.get(h..end.max(h)).unwrap_or_default())?))
        })
        .collect();
    let mut out: BTreeMap<u32, Vec<u8>> = BTreeMap::new();
    let mut debuts: BTreeMap<u32, Vec<(usize, Document)>> = BTreeMap::new();
    for (m, doc) in membres.into_iter().zip(docs) {
        let (h6, d) = m?;
        let s = out.entry(h6).or_default();
        debuts.entry(h6).or_default().push((s.len(), doc));
        s.extend(d);
    }
    Ok((out, debuts))
}

/// Document d'un enregistrement commençant à la position `pos` des données d'une
/// section, d'après les débuts de ses membres (voir [`sections_documents`]).
pub fn document_de(debuts: &[(usize, Document)], pos: usize) -> Document {
    match debuts.partition_point(|d| d.0 <= pos) {
        0 => None,
        k => debuts[k - 1].1,
    }
}

/// Enregistrements `(id, octets)` d'une section. `stamp` : présence du u32 non
/// identifié avant la longueur (sections 0x66 et 0x67) ; `ids8` : ElementId sur
/// 8 octets (formats 2024 et suivants) ou 4 (2016 à 2022).
pub fn records(data: &[u8], stamp: bool, ids8: bool) -> Result<Vec<(i64, &[u8])>, String> {
    let ids = if ids8 { 8 } else { 4 };
    let hdr = ids + if stamp { 8 } else { 4 };
    let mut out = Vec::new();
    let mut o = 0;
    while o + hdr <= data.len() {
        let eid = if ids8 { i64_at(data, o) } else { i32_at(data, o).map(i64::from) }.unwrap();
        let n = u32_at(data, o + hdr - 4).unwrap() as usize;
        let fin = o + hdr + n;
        if u32_at(data, fin).map(|v| v as usize) != Some(n) {
            return Err(format!("cadrage perdu à {o} (id {eid}, longueur {n})"));
        }
        out.push((eid, &data[o + hdr..fin]));
        o = fin + 4;
    }
    Ok(out)
}

/// Éléments de `Global/ElemTable` (ceux du document) : u16, u32 nombre, puis des
/// enregistrements de 40 octets (id i64, formats 2024 et suivants) ou 28 (id i32). Le
/// u32 qui suit l'id est l'index de l'épisode de création dans `Global/History`
/// (vérifié sur les enregistrements de 40 octets) : {id : index}.
pub fn elem_table(raw: &[u8]) -> Result<HashMap<i64, u32>, String> {
    let t = inflate_global(raw)?;
    let n = u32_at(&t, 2).ok_or("ElemTable tronquée")? as usize;
    let size = if n > 0 { (t.len() - 6) / n } else { 40 };
    (0..n)
        .map(|k| {
            let o = 6 + size * k;
            let (id, ids) = if size == 40 { (i64_at(&t, o), 8) } else { (i32_at(&t, o).map(i64::from), 4) };
            id.zip(u32_at(&t, o + ids)).ok_or_else(|| "ElemTable tronquée".to_string())
        })
        .collect()
}

/// GUID des épisodes de `Global/History` (octets Windows), index 0 en premier : le flux
/// se termine par u32 n, n × (GUID, u8), u32 0, la liste étant rangée du dernier
/// épisode au premier.
pub fn episodes(raw: &[u8]) -> Result<Vec<[u8; 16]>, String> {
    let h = inflate_global(raw)?;
    for n in 1..=h.len() / 17 {
        let Some(debut) = h.len().checked_sub(4 + 17 * n + 4) else { break };
        // le compte, puis le même octet après chaque GUID
        if u32_at(&h, debut) == Some(n as u32) && (0..n).all(|k| h[debut + 20 + 17 * k] == h[debut + 20]) {
            let mut l: Vec<[u8; 16]> = (0..n).map(|k| h[debut + 4 + 17 * k..debut + 20 + 17 * k].try_into().unwrap()).collect();
            l.reverse();
            return Ok(l);
        }
    }
    Err("liste des épisodes introuvable dans Global/History".into())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unpad_tranches() {
        let mut b: Vec<u8> = (0..DATA).map(|i| i as u8).collect();
        b.extend([0xEE; PAD]);
        b.extend([1, 2, 3]);
        let u = unpad(&b);
        assert_eq!(u.len(), DATA + 3);
        assert_eq!(&u[DATA..], &[1, 2, 3]);
    }

    #[test]
    fn cadrage() {
        let mut d = Vec::new();
        for (id, corps) in [(7i64, &b"ab"[..]), (9, &b"xyz"[..])] {
            d.extend(id.to_le_bytes());
            d.extend(0u32.to_le_bytes());
            d.extend((corps.len() as u32).to_le_bytes());
            d.extend(corps);
            d.extend((corps.len() as u32).to_le_bytes());
        }
        let r = records(&d, true, true).unwrap();
        assert_eq!(r, vec![(7, &b"ab"[..]), (9, &b"xyz"[..])]);
        d[20] = 9;
        assert!(records(&d, true, true).is_err());
    }
}
