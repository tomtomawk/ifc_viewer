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
/// `altitude` (mm), et sa section (largeur, hauteur, mm) ; son type est à fixer par
/// l'appelant (`type_section`). `Err` : raison de le garder importé.
pub(crate) fn barre(m: &IfcModel, id: Id, ident: String, nom: String, niveau: String, altitude: f64, origine: Option<Origine>) -> Result<(Barre, (f64, f64)), &'static str> {
    let genre = if POTEAUX.contains(&m.ty(id)) { GenreBarre::Poteau } else { GenreBarre::Poutre };
    let e = rectangulaire(m, id).ok_or("pas une section rectangulaire extrudée (IfcExtrudedAreaSolid d'un IfcRectangleProfileDef)")?;
    let mm = |v: DVec3| v * 1000.0;
    let (a, f) = (mm(e.depart), mm(e.depart + e.extrusion));
    let d = e.extrusion.normalize();
    let mut b = Barre { id: ident, nom, genre, niveau, debut: [0.0; 3], fin: [0.0; 3], type_barre: String::new(), rotation: 0.0, pose: Default::default(), origine };
    let section;
    match genre {
        GenreBarre::Poteau => {
            if d.z.abs() < 1.0 - ANGLE {
                return Err("poteau non vertical");
            }
            let (bas, haut) = if a.z <= f.z { (a, f) } else { (f, a) };
            b.debut = [bas.x, bas.y, bas.z - altitude];
            b.fin = [bas.x, bas.y, haut.z - altitude];
            section = (e.x * 1000.0, e.y * 1000.0);
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
            section = (largeur * 1000.0, hauteur * 1000.0);
        }
    }
    Ok((b, section))
}

/// Type rectangulaire de la sorte `genre` aux dimensions données (au dixième de mm),
/// créé s'il manque ; rend son identifiant.
pub(crate) fn type_section(p: &mut modele::Projet, genre: GenreBarre, (largeur, hauteur): (f64, f64)) -> String {
    let (l, h) = ((largeur * 10.0).round() / 10.0, (hauteur * 10.0).round() / 10.0);
    let types = match genre { GenreBarre::Poteau => &mut p.types_poteaux, GenreBarre::Poutre => &mut p.types_poutres };
    if let Some(t) = types.iter().find(|t| t.profil.is_none() && t.largeur == l && t.hauteur == h) {
        return t.id.clone();
    }
    let sorte = match genre { GenreBarre::Poteau => "Poteau", GenreBarre::Poutre => "Poutre" };
    let u = modele::unite::unite();
    let id = format!("type-{}-{}", sorte.to_lowercase(), noyau::uuid::uuid());
    types.push(modele::elements::poutre::TypeSection { id: id.clone(), nom: format!("{sorte} {} × {}", u.afficher(l), u.afficher(h)), profil: None, largeur: l, hauteur: h, materiau: None });
    id
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
