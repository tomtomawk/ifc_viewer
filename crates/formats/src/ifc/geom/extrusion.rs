//! Extrusion rectangulaire simple d'un produit : sa représentation « Body » est un seul
//! `IfcExtrudedAreaSolid` d'un `IfcRectangleProfileDef`, sans item mappé ni ouverture.
//! Sert à reconnaître les poteaux et poutres convertibles en éléments paramétriques.
use super::{GeomStats, Geom};
use ifc_base::semantique::IfcModel;
use ifc_base::step::Id;
use glam::{DMat4, DVec3};

/// Section rectangulaire extrudée, en mètres dans le repère du projet.
#[derive(Debug, Clone, Copy)]
pub struct ExtrusionRectangulaire {
    /// Centre de la section au départ de l'extrusion.
    pub depart: DVec3,
    /// Vecteur d'extrusion (direction × profondeur).
    pub extrusion: DVec3,
    /// Directions unitaires des côtés `XDim` et `YDim` de la section.
    pub axe_x: DVec3,
    pub axe_y: DVec3,
    /// `XDim`, `YDim`.
    pub x: f64,
    pub y: f64,
}

/// Extrusion rectangulaire du produit `id`, ou None si sa forme est autre.
pub fn rectangulaire(m: &IfcModel, id: Id) -> Option<ExtrusionRectangulaire> {
    if m.openings.get(&id).is_some_and(|o| !o.is_empty()) {
        return None;
    }
    let stats = GeomStats::default();
    let g = Geom { m, stats: &stats, eps: 1e-6 / m.length_scale };
    let e = m.get(id)?;
    let items = g.body_items(e.arg(6).as_ref()?);
    let [item] = items.as_slice() else { return None };
    let s = g.get(*item).filter(|s| s.is("IFCEXTRUDEDAREASOLID"))?;
    let profil = g.get(s.arg(0).as_ref()?).filter(|p| p.is("IFCRECTANGLEPROFILEDEF"))?;
    let (x, y, profondeur) = (profil.arg(3).as_f64()?, profil.arg(4).as_f64()?, s.arg(3).as_f64()?);
    if !(x > 0.0 && y > 0.0 && profondeur > 0.0) {
        return None;
    }
    let direction = g.direction(s.arg(2).as_ref()).unwrap_or(DVec3::Z);
    let monde = DMat4::from_scale(DVec3::splat(m.length_scale)) * g.placement(e.arg(5).as_ref(), 0) * g.axis2(s.arg(1).as_ref());
    let section = monde * g.axis2(profil.arg(2).as_ref());
    let l = m.length_scale;
    Some(ExtrusionRectangulaire {
        depart: section.transform_point3(DVec3::ZERO),
        extrusion: monde.transform_vector3(direction * profondeur),
        axe_x: section.transform_vector3(DVec3::X).try_normalize()?,
        axe_y: section.transform_vector3(DVec3::Y).try_normalize()?,
        x: x * l,
        y: y * l,
    })
}
