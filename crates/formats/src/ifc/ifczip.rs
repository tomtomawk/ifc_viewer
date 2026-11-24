//! Fichiers IFC-ZIP (`.ifczip`) : archive zip contenant un fichier `.ifc`. Lecture du
//! répertoire central, entrée stockée ou compressée (deflate) ; ZIP64 et chiffrement
//! refusés. Les tailles lues dans l'archive sont bornées (fichier malformé).
use miniz_oxide::inflate::{decompress_to_vec_with_limit, TINFLStatus};

/// Taille décompressée maximale d'un IFC lu dans une archive.
const MAX: usize = 8 << 30;

/// Contenu IFC du fichier lu : tel quel, ou extrait de l'archive s'il commence par la
/// signature zip (`PK`).
pub fn contenu_ifc(data: Vec<u8>) -> Result<Vec<u8>, String> {
    if !data.starts_with(b"PK") {
        return Ok(data);
    }
    extraire(&data).map_err(|e| format!("Fichier IFC-ZIP : {e}"))
}

fn u16_a(b: &[u8], i: usize) -> Result<usize, String> {
    b.get(i..i + 2).map(|x| u16::from_le_bytes([x[0], x[1]]) as usize).ok_or_else(|| "archive tronquée".into())
}

fn u32_a(b: &[u8], i: usize) -> Result<usize, String> {
    b.get(i..i + 4).map(|x| u32::from_le_bytes([x[0], x[1], x[2], x[3]]) as usize).ok_or_else(|| "archive tronquée".into())
}

fn extraire(b: &[u8]) -> Result<Vec<u8>, String> {
    // fin du répertoire central : dans les 22 + 65 535 derniers octets
    let debut = b.len().saturating_sub(22 + 65_535);
    let fin = (debut..b.len().saturating_sub(21)).rev().find(|&i| b[i..i + 4] == [0x50, 0x4b, 0x05, 0x06]).ok_or("répertoire central introuvable")?;
    let (nombre, taille_rc, mut pos) = (u16_a(b, fin + 10)?, u32_a(b, fin + 12)?, u32_a(b, fin + 16)?);
    if nombre == 0xFFFF || pos == 0xFFFF_FFFF || taille_rc == 0xFFFF_FFFF {
        return Err("archive ZIP64 non prise en charge".into());
    }
    let mut noms = Vec::new();
    for _ in 0..nombre {
        if u32_a(b, pos)? != 0x0201_4b50 {
            return Err("entrée du répertoire central non valide".into());
        }
        let (drapeaux, methode) = (u16_a(b, pos + 8)?, u16_a(b, pos + 10)?);
        let (compresse, taille) = (u32_a(b, pos + 20)?, u32_a(b, pos + 24)?);
        let (n, e, c) = (u16_a(b, pos + 28)?, u16_a(b, pos + 30)?, u16_a(b, pos + 32)?);
        let local = u32_a(b, pos + 42)?;
        let nom = String::from_utf8_lossy(b.get(pos + 46..pos + 46 + n).ok_or("archive tronquée")?).into_owned();
        pos += 46 + n + e + c;
        if !nom.to_lowercase().ends_with(".ifc") {
            noms.push(nom);
            continue;
        }
        if drapeaux & 1 != 0 {
            return Err(format!("{nom} : entrée chiffrée"));
        }
        if compresse == 0xFFFF_FFFF || taille == 0xFFFF_FFFF || local == 0xFFFF_FFFF {
            return Err("archive ZIP64 non prise en charge".into());
        }
        if u32_a(b, local)? != 0x0403_4b50 {
            return Err(format!("{nom} : en-tête local non valide"));
        }
        let donnees = local + 30 + u16_a(b, local + 26)? + u16_a(b, local + 28)?;
        let brut = b.get(donnees..donnees.checked_add(compresse).ok_or("archive tronquée")?).ok_or("archive tronquée")?;
        let contenu = match methode {
            0 => brut.to_vec(),
            8 => decompress_to_vec_with_limit(brut, taille.min(MAX)).map_err(|e| match e.status {
                TINFLStatus::HasMoreOutput => format!("{nom} : plus grand que la taille annoncée ou que {} Go", MAX >> 30),
                s => format!("{nom} : flux compressé non valide ({s:?})"),
            })?,
            m => return Err(format!("{nom} : méthode de compression {m} non prise en charge")),
        };
        if contenu.len() != taille {
            return Err(format!("{nom} : taille décompressée différente de celle annoncée"));
        }
        return Ok(contenu);
    }
    Err(if noms.is_empty() { "archive vide".into() } else { format!("aucun fichier .ifc dans l'archive ({})", noms.join(", ")) })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Archive d'une entrée `nom` (méthode 0 ou 8).
    pub(crate) fn archive(nom: &str, contenu: &[u8], methode: u16) -> Vec<u8> {
        let brut = if methode == 8 { miniz_oxide::deflate::compress_to_vec(contenu, 6) } else { contenu.to_vec() };
        let mut z = Vec::new();
        let entete = |z: &mut Vec<u8>, central: bool| {
            z.extend(if central { [0x50, 0x4b, 0x01, 0x02] } else { [0x50, 0x4b, 0x03, 0x04] });
            if central {
                z.extend([20, 0]);
            }
            z.extend([20, 0, 0, 0]);
            z.extend(methode.to_le_bytes());
            z.extend([0; 8]); // heure, date, CRC (non vérifié)
            z.extend((brut.len() as u32).to_le_bytes());
            z.extend((contenu.len() as u32).to_le_bytes());
            z.extend((nom.len() as u16).to_le_bytes());
            z.extend([0, 0]);
            if central {
                z.extend([0; 6]); // commentaire, disque, attributs internes
                z.extend([0; 4]); // attributs externes
                z.extend(0u32.to_le_bytes()); // en-tête local à l'octet 0
            }
            z.extend(nom.as_bytes());
        };
        entete(&mut z, false);
        z.extend(&brut);
        let rc = z.len();
        entete(&mut z, true);
        let taille = z.len() - rc;
        z.extend([0x50, 0x4b, 0x05, 0x06, 0, 0, 0, 0, 1, 0, 1, 0]);
        z.extend((taille as u32).to_le_bytes());
        z.extend((rc as u32).to_le_bytes());
        z.extend([0, 0]);
        z
    }

    #[test]
    fn ifc_extrait_stocke_ou_compresse() {
        let ifc = b"ISO-10303-21;\nHEADER;\nENDSEC;\nDATA;\nENDSEC;\nEND-ISO-10303-21;\n".repeat(20);
        for methode in [0, 8] {
            assert_eq!(contenu_ifc(archive("projet/Maquette.IFC", &ifc, methode)).unwrap(), ifc);
        }
        assert_eq!(contenu_ifc(ifc.clone()).unwrap(), ifc);
    }

    #[test]
    fn archives_non_valides_refusees_avec_message() {
        let ifc = b"ISO-10303-21;".to_vec();
        assert!(contenu_ifc(archive("lisezmoi.txt", &ifc, 0)).unwrap_err().contains("aucun fichier .ifc"));
        let mut tronquee = archive("a.ifc", &ifc, 8);
        tronquee.truncate(40);
        assert!(contenu_ifc(tronquee).is_err());
        assert!(contenu_ifc(b"PK\x03\x04".to_vec()).is_err());
        // taille annoncée plus petite que le contenu : refus borné, pas d'allocation libre
        let mut menteuse = archive("a.ifc", &b"x".repeat(1000), 8);
        let n = menteuse.len();
        let rc = u32::from_le_bytes(menteuse[n - 6..n - 2].try_into().unwrap()) as usize;
        menteuse[rc + 24..rc + 28].copy_from_slice(&10u32.to_le_bytes());
        assert!(contenu_ifc(menteuse).is_err());
    }
}
