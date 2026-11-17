//! Liens IFC : maquettes d'autres intervenants (structure, fluides, existant) affichées
//! dans le projet en lecture seule, en 3D, en plan et en coupe. Le fichier lié n'est
//! pas copié dans le projet : seul son chemin et sa position sont enregistrés.
use glam::DVec3;
use serde::{Deserialize, Serialize};

fn vrai() -> bool {
    true
}

/// Fichier IFC lié au projet.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct LienIfc {
    pub id: String,
    pub nom: String,
    /// Chemin absolu du fichier IFC (les feuilles et exports reconstruisent la scène sans
    /// connaître le dossier du projet).
    pub chemin: String,
    /// Translation (mm, repère du projet), appliquée après la rotation.
    #[serde(default)]
    pub decalage: [f64; 3],
    /// Rotation autour de l'axe Z passant par l'origine du fichier lié (degrés, sens
    /// trigonométrique).
    #[serde(default)]
    pub rotation: f64,
    #[serde(default = "vrai")]
    pub visible: bool,
}

impl LienIfc {
    pub fn verifier(&self) -> Result<(), String> {
        if self.chemin.trim().is_empty() {
            return Err(format!("Lien « {} » : chemin du fichier vide.", self.nom));
        }
        if !self.decalage.iter().all(|v| v.is_finite()) || !self.rotation.is_finite() {
            return Err(format!("Lien « {} » : décalage ou rotation non valide.", self.nom));
        }
        Ok(())
    }

    /// Point du fichier lié (mètres, repère du fichier) dans le repère du projet (mètres).
    pub fn transformer(&self, p: DVec3) -> DVec3 {
        let (s, c) = self.rotation.to_radians().sin_cos();
        DVec3::new(p.x * c - p.y * s, p.x * s + p.y * c, p.z) + DVec3::from(self.decalage) * 0.001
    }

    /// Direction (normale) du fichier lié dans le repère du projet.
    pub fn tourner(&self, d: DVec3) -> DVec3 {
        let (s, c) = self.rotation.to_radians().sin_cos();
        DVec3::new(d.x * c - d.y * s, d.x * s + d.y * c, d.z)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn lien() -> LienIfc {
        LienIfc { id: "l1".into(), nom: "Structure".into(), chemin: "/tmp/structure.ifc".into(), decalage: [1000.0, 0.0, -500.0], rotation: 90.0, visible: true }
    }

    #[test]
    fn transformation_rotation_puis_decalage() {
        let p = lien().transformer(DVec3::new(2.0, 0.0, 3.0));
        assert!((p - DVec3::new(1.0, 2.0, 2.5)).length() < 1e-9, "{p}");
    }

    /// Écriture, relecture, valeurs par défaut, rejet des champs inconnus et des
    /// valeurs non valides.
    #[test]
    fn schema_json() {
        let l = lien();
        let json = serde_json::to_string(&l).unwrap();
        assert_eq!(serde_json::from_str::<LienIfc>(&json).unwrap(), l);
        let minimal: LienIfc = serde_json::from_str(r#"{"id":"a","nom":"A","chemin":"/a.ifc"}"#).unwrap();
        assert!(minimal.visible && minimal.decalage == [0.0; 3] && minimal.rotation == 0.0);
        assert!(serde_json::from_str::<LienIfc>(r#"{"id":"a","nom":"A","chemin":"/a.ifc","echelle":2}"#).is_err());
        assert!(LienIfc { chemin: " ".into(), ..lien() }.verifier().is_err());
        assert!(LienIfc { rotation: f64::NAN, ..lien() }.verifier().is_err());
        assert!(lien().verifier().is_ok());
    }
}
