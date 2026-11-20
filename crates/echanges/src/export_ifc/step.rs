//! Écrivain STEP de l'export IFC : lignes numérotées, textes, références, GUID
//! déterministes, faces tessellées et en-tête.
use modele::{Maillage, Origine};
use std::fmt::Write;

pub(super) type Tri = [glam::DVec3; 3];

/// Hauteur minimale d'un triangle exporté (mm) : 1 µm, dix fois la confusion
/// d'OpenCascade (1e-7 m). Les booléens des murs percés laissent des aiguilles de 0,04 µm.
const HAUTEUR_MIN: f64 = 1e-3;

/// Plus petite hauteur d'un triangle (aire double / plus long côté) ; 0 s'il est réduit à un point.
fn hauteur([a, b, c]: &Tri) -> f64 {
    let long = (*b - *a).length().max((*c - *b).length()).max((*a - *c).length());
    if long == 0.0 { 0.0 } else { (*b - *a).cross(*c - *a).length() / long }
}

/// Entités du fichier, numérotées à partir de 1 dans l'ordre d'ajout.
pub(super) struct Step {
    pub(super) lignes: Vec<String>,
}

impl Step {
    pub(super) fn add(&mut self, s: String) -> usize {
        self.lignes.push(s);
        self.lignes.len()
    }

    /// Faces triangulées (mm) ; None si aucun triangle. Les aiguilles (hauteur sous
    /// `HAUTEUR_MIN`) sont écartées : IfcOpenShell (OpenCascade) rend vide tout le jeu de
    /// faces qui en contient une (X5).
    pub(super) fn faces(&mut self, tris: &[Tri]) -> Option<usize> {
        let nets: Vec<Tri> = tris.iter().filter(|t| hauteur(t) >= HAUTEUR_MIN).copied().collect();
        let m = Maillage::depuis_triangles(&nets);
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
    // alphabet ASCII : un octet par caractère
    s.iter().map(|&b| char::from(b)).collect()
}

/// Horodatage ISO 8601 UTC (`AAAA-MM-JJThh:mm:ss`) de `secondes` depuis 1970.
pub(super) fn horodatage_utc(secondes: u64) -> String {
    let (jours, s) = ((secondes / 86_400) as i64, secondes % 86_400);
    // jours depuis 1970 -> date civile (algorithme de H. Hinnant)
    let z = jours + 719_468;
    let ere = z.div_euclid(146_097);
    let jde = z - ere * 146_097;
    let ade = (jde - jde / 1460 + jde / 36_524 - jde / 146_096) / 365;
    let jda = jde - (365 * ade + ade / 4 - ade / 100);
    let mp = (5 * jda + 2) / 153;
    let jour = jda - (153 * mp + 2) / 5 + 1;
    let mois = if mp < 10 { mp + 3 } else { mp - 9 };
    let annee = ade + ere * 400 + i64::from(mois <= 2);
    format!("{annee:04}-{mois:02}-{jour:02}T{:02}:{:02}:{:02}", s / 3600, s % 3600 / 60, s % 60)
}

/// Horodatage UTC de l'instant présent.
pub(super) fn maintenant() -> String {
    horodatage_utc(std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_secs()))
}

#[cfg(test)]
mod tests {
    use glam::DVec3;

    /// Une aiguille de 0,04 µm est écartée, un triangle ordinaire gardé.
    #[test]
    fn aiguilles_ecartees() {
        let mut w = super::Step { lignes: Vec::new() };
        let aiguille = [DVec3::ZERO, DVec3::new(1000.0, 0.0, 0.0), DVec3::new(500.0, 4e-5, 0.0)];
        assert!(w.faces(&[aiguille]).is_none());
        let t = [DVec3::ZERO, DVec3::new(1000.0, 0.0, 0.0), DVec3::new(0.0, 15.0, 0.0)];
        assert!(w.faces(&[t, aiguille]).is_some());
        assert!(w.lignes.last().unwrap().contains("((1,2,3))"));
    }

    #[test]
    fn horodatage_iso_8601() {
        assert_eq!(super::horodatage_utc(0), "1970-01-01T00:00:00");
        assert_eq!(super::horodatage_utc(951_782_400), "2000-02-29T00:00:00");
        assert_eq!(super::horodatage_utc(1_794_928_215), "2026-11-17T15:10:15");
    }
}
