//! Outils des tests de géométrie : petit fichier IFC construit à partir de sa section DATA.
use super::{Geom, GeomStats};
use glam::DVec3;
use ifc_base::semantique::IfcModel;
use ifc_base::step::StepFile;
use noyau::csg::Tri;

/// Modèle IFC dont la section DATA est `donnees`.
pub(super) fn geom_de(donnees: &str) -> (IfcModel, GeomStats) {
    let texte = format!("ISO-10303-21;\nHEADER;\nFILE_SCHEMA(('IFC4X3'));\nENDSEC;\nDATA;\n{donnees}\nENDSEC;\nEND-ISO-10303-21;\n");
    (IfcModel::new(StepFile::parse(texte.as_bytes()).unwrap()), GeomStats::default())
}

pub(super) fn geom<'a>(m: &'a IfcModel, stats: &'a GeomStats) -> Geom<'a> {
    Geom { m, stats, eps: 1e-9 }
}

/// Boîte englobante de triangles.
pub(super) fn boite(tris: &[Tri]) -> (DVec3, DVec3) {
    super::bbox(tris)
}
