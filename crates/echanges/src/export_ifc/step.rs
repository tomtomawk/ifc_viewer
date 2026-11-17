//! Écrivain STEP de l'export IFC : lignes numérotées, textes, références, GUID
//! déterministes, faces tessellées et en-tête.
use modele::{Maillage, Origine};
use std::fmt::Write;

pub(super) type Tri = [glam::DVec3; 3];

/// Entités du fichier, numérotées à partir de 1 dans l'ordre d'ajout.
pub(super) struct Step {
    pub(super) lignes: Vec<String>,
}

impl Step {
    pub(super) fn add(&mut self, s: String) -> usize {
        self.lignes.push(s);
        self.lignes.len()
    }

    /// Faces triangulées (mm) ; None si aucun triangle.
    pub(super) fn faces(&mut self, tris: &[Tri]) -> Option<usize> {
        let m = Maillage::depuis_triangles(tris);
        if m.triangles.is_empty() {
            return None;
        }
        let points = m.sommets.iter().map(|p| format!("({:.6},{:.6},{:.6})", p[0], p[1], p[2])).collect::<Vec<_>>().join(",");
        let pts = self.add(format!("IFCCARTESIANPOINTLIST3D(({points}))"));
        let faces = m.triangles.iter().map(|t| format!("({},{},{})", t[0] + 1, t[1] + 1, t[2] + 1)).collect::<Vec<_>>().join(",");
        Some(self.add(format!("IFCTRIANGULATEDFACESET(#{pts},$,$,({faces}),$)")))
    }

    /// Forme d'un produit : corps tessellé fait des `faces`, précédé des autres
    /// représentations (axe) ; 0 s'il n'y a rien.
    pub(super) fn forme(&mut self, contexte: usize, faces: &[usize], autres: &[usize]) -> usize {
        let mut reps = autres.to_vec();
        if !faces.is_empty() {
            reps.push(self.add(format!("IFCSHAPEREPRESENTATION(#{contexte},'Body','Tessellation',{})", refs(faces))));
        }
        if reps.is_empty() {
            return 0;
        }
        self.add(format!("IFCPRODUCTDEFINITIONSHAPE($,$,{})", refs(&reps)))
    }

    /// Forme d'un seul jeu de faces ; 0 si vide.
    pub(super) fn mesh(&mut self, tris: &[Tri], contexte: usize) -> usize {
        let faces: Vec<usize> = self.faces(tris).into_iter().collect();
        self.forme(contexte, &faces, &[])
    }

    /// Fichier complet : en-tête puis entités.
    pub(super) fn fichier(&self, nom: &str, horodatage: &str) -> String {
        let mut out = format!(
            "ISO-10303-21;\nHEADER;\nFILE_DESCRIPTION(('ViewDefinition [ReferenceView_V1.2]'),'2;1');\nFILE_NAME({},{},(''),(''),'bimedit','bimedit','');\nFILE_SCHEMA(('IFC4'));\nENDSEC;\nDATA;\n",
            texte(nom),
            texte(horodatage)
        );
        for (i, l) in self.lignes.iter().enumerate() {
            let _ = writeln!(out, "#{}={l};", i + 1);
        }
        out.push_str("ENDSEC;\nEND-ISO-10303-21;\n");
        out
    }
}

/// Chaîne STEP : apostrophes doublées, non-ASCII en `\X2\`.
pub(super) fn texte(s: &str) -> String {
    let mut out = String::from("'");
    for c in s.chars() {
        if c == '\'' {
            out.push_str("''");
        } else if c == '\\' || !c.is_ascii() || c.is_control() {
            out.push_str("\\X2\\");
            for n in c.encode_utf16(&mut [0; 2]).iter() {
                let _ = write!(out, "{n:04X}");
            }
            out.push_str("\\X0\\");
        } else {
            out.push(c);
        }
    }
    out.push('\'');
    out
}

pub(super) fn ref_(i: usize) -> String {
    if i == 0 { "$".into() } else { format!("#{i}") }
}

pub(super) fn refs(ids: &[usize]) -> String {
    format!("({})", ids.iter().map(|i| format!("#{i}")).collect::<Vec<_>>().join(","))
}

/// Réel STEP (toujours avec un point décimal).
pub(super) fn reel(v: f64) -> String {
    format!("{v:.6}")
}

/// GlobalId : celui de l'origine s'il est valide, sinon dérivé de la clé (FNV-1a 128).
pub(super) fn guid(cle: &str, origine: Option<&Origine>) -> String {
    let alphabet = b"0123456789ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz_$";
    if let Some(g) = origine
        .and_then(|o| o.guid.as_ref())
        .filter(|g| g.len() == 22 && g.bytes().all(|b| alphabet.contains(&b)) && g.as_bytes()[0] <= b'3')
    {
        return g.clone();
    }
    let mut h = 0x6c62272e07bb014262b821756295c58du128;
    for b in cle.bytes() {
        h = (h ^ (b as u128)).wrapping_mul(0x1000000000000000000013b);
    }
    let mut s = [b'0'; 22];
    for i in (0..22).rev() {
        s[i] = alphabet[(h & 63) as usize];
        h >>= 6;
    }
    String::from_utf8(s.to_vec()).unwrap()
}
