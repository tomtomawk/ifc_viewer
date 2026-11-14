//! Tests déplacés de `modele/elements/terre_plein.rs` (`mod tests_scene`) : ils appellent du code situé au-dessus
//! de la crate `modele` (commandes, scène, formats, échanges, collaboration ou dessin).
use modele::elements::site::Terrain;
use modele::{Dalle, Projet, SorteDalle};

/// Copie de l'aide `tests::projet` de `terre_plein.rs` (terre-plein de 10 × 10 m sur un terrain plat).
fn projet(talus: Option<f64>) -> (Projet, Dalle) {
    let mut p = Projet::nouveau("TP");
    p.niveaux.push(modele::Niveau { id: "n0".into(), nom: "RDC".into(), altitude: 0.0, origine: None });
    p.types_dalles.push(modele::TypeCompose { id: "tp".into(), nom: "Grave".into(), couches: vec![modele::Couche { epaisseur: 200.0, ..Default::default() }], categorie: Default::default(), couches_detaillees: true, origine: None });
    // terrain plat à -1 m, 60 × 60 m
    p.terrains.push(Terrain { id: "t".into(), nom: "TN".into(), contour: vec![[-30_000., -30_000., -1000.], [30_000., -30_000., -1000.], [30_000., 30_000., -1000.], [-30_000., 30_000., -1000.]], points: vec![], type_terrain: None });
    let mut d = Dalle { variante: None, phase: None, sous_projet: None, id: "d".into(), nom: "Terre-plein".into(), type_dalle: "tp".into(), niveau: "n0".into(), sorte: SorteDalle::TerrePlein, decalage: 200.0, contours: vec![vec![[0., 0.], [10_000., 0.], [10_000., 10_000.], [0., 10_000.]]], pentes: vec![], talus_dessus: false, remblais: Vec::new(), type_remblai: None, origine: None };
    if let Some(t) = talus {
        for k in 0..4 { d.definir_pente(0, k, Some(t)); }
    }
    p.dalles.push(d.clone());
    (p, d)
}

#[test]
fn scene_terre_plein() {
    let (mut p, _) = projet(Some(45.0));
    p.types_dalles[0].couches[0].materiau = Some("Grave non traitée".into());
    p.completer_materiaux();
    let s = crate::rendu::natif::depuis_natif(&p, std::path::Path::new("tp.json"));
    assert!(s.is_ok(), "{:?}", s.err());
}
