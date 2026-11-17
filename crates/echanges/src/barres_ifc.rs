//! Poteaux (`IfcColumn`) et poutres (`IfcBeam`) simples vers `modele::Barre` : section
//! rectangulaire extrudée (`IfcExtrudedAreaSolid` d'un `IfcRectangleProfileDef`),
//! verticalement pour un poteau, horizontalement pour une poutre dont la section n'est
//! pas tournée autour de son axe. Sinon : la raison, et l'élément reste importé.
use formats::ifc::geom::extrusion::{rectangulaire, ExtrusionRectangulaire};
use glam::DVec3;
use ifc_base::semantique::IfcModel;
use ifc_base::step::Id;
use modele::{Barre, GenreBarre, Origine};

pub(crate) const POTEAUX: &[&str] = &["IFCCOLUMN", "IFCCOLUMNSTANDARDCASE"];
pub(crate) const POUTRES: &[&str] = &["IFCBEAM", "IFCBEAMSTANDARDCASE"];

/// Tolérance angulaire (sinus) : vertical, horizontal, section non tournée.
const ANGLE: f64 = 1e-6;

/// Barre du produit `id` (genre selon sa classe) posée sur le niveau `niveau` d'altitude
/// `altitude` (mm) ; `Err` : raison de le garder importé.
pub(crate) fn barre(m: &IfcModel, id: Id, ident: String, nom: String, niveau: String, altitude: f64, origine: Option<Origine>) -> Result<Barre, &'static str> {
    let genre = if POTEAUX.contains(&m.ty(id)) { GenreBarre::Poteau } else { GenreBarre::Poutre };
    let e = rectangulaire(m, id).ok_or("pas une section rectangulaire extrudée (IfcExtrudedAreaSolid d'un IfcRectangleProfileDef)")?;
    let mm = |v: DVec3| v * 1000.0;
    let (a, f) = (mm(e.depart), mm(e.depart + e.extrusion));
    let d = e.extrusion.normalize();
    let mut b = Barre { id: ident, nom, genre, niveau, debut: [0.0; 3], fin: [0.0; 3], largeur: 0.0, hauteur: 0.0, rotation: 0.0, pose: Default::default(), origine };
    match genre {
        GenreBarre::Poteau => {
            if d.z.abs() < 1.0 - ANGLE {
                return Err("poteau non vertical");
            }
            let (bas, haut) = if a.z <= f.z { (a, f) } else { (f, a) };
            b.debut = [bas.x, bas.y, bas.z - altitude];
            b.fin = [bas.x, bas.y, haut.z - altitude];
            (b.largeur, b.hauteur) = (e.x * 1000.0, e.y * 1000.0);
            b.rotation = e.axe_x.y.atan2(e.axe_x.x).to_degrees();
        }
        GenreBarre::Poutre => {
            if d.z.abs() > ANGLE {
                return Err("poutre inclinée");
            }
            // largeur : côté horizontal de la section ; hauteur : côté vertical
            let (largeur, hauteur) = section_poutre(&e).ok_or("section de poutre tournée autour de son axe")?;
            b.debut = [a.x, a.y, a.z - altitude];
            b.fin = [f.x, f.y, f.z - altitude];
            (b.largeur, b.hauteur) = (largeur * 1000.0, hauteur * 1000.0);
        }
    }
    Ok(b)
}

/// (côté horizontal, côté vertical) d'une section de poutre droite, en m.
fn section_poutre(e: &ExtrusionRectangulaire) -> Option<(f64, f64)> {
    if e.axe_y.z.abs() > 1.0 - ANGLE {
        Some((e.x, e.y))
    } else if e.axe_x.z.abs() > 1.0 - ANGLE {
        Some((e.y, e.x))
    } else {
        None
    }
}
