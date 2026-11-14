use super::*;
use glam::DVec3;
use modele::{ElementImporte, Maillage, Origine};

/// Topographie importée de Revit : élément géographique, que les plans d'étage gardent
/// (la plage de vue décide ensuite de ce qui est vu).
#[test]
fn topographie_revit_importee_est_geographique() {
    let mut p = Projet::nouveau("T");
    let tris = [[DVec3::ZERO, DVec3::new(1000.0, 0.0, 0.0), DVec3::new(0.0, 1000.0, 500.0)]];
    for (id, classe) in [("a", "Toposolid"), ("b", "SiteSurface"), ("c", "Wall")] {
        let origine = Some(Origine { source: "rvt".into(), id: id.into(), guid: None, classe: classe.into() });
        p.importes.push(ElementImporte { id: id.into(), nom: id.into(), categorie: "Autres".into(), niveau: None, maillage: Maillage::depuis_triangles(&tris), raison: String::new(), origine });
    }
    let s = depuis_natif(&p, std::path::Path::new("t.modele.json")).unwrap();
    let classe = |id: &str| &*s.objects[s.object_of[&id_natif("importe", id)] as usize].class;
    assert_eq!(classe("a"), "IFCGEOGRAPHICELEMENT");
    assert_eq!(classe("b"), "IFCGEOGRAPHICELEMENT");
    assert_eq!(classe("c"), "IFCBUILDINGELEMENTPROXY");
}

/// Terrain recoupant un mur : en plan, le clic sur le mur le sélectionne (le terrain
/// n'est pris que faute d'autre objet) et le terrain porte ses traits (périmètre et
/// courbes de niveau).
#[test]
fn terrain_sous_le_mur_et_ses_traits() {
    use modele::{creation::ParametresMur, elements::site::Terrain};
    let mut p = Projet::nouveau("T");
    p.ajouter_mur([0.0, 0.0], [5000.0, 0.0], &ParametresMur { epaisseur: 200.0, ..Default::default() }).unwrap();
    let z = |x: f64| 200.0 + x / 10.0;
    p.terrains.push(Terrain { id: "t".into(), nom: "Topo".into(), contour: vec![[-5000.0, -5000.0, z(-5000.0)], [10000.0, -5000.0, z(10000.0)], [10000.0, 5000.0, z(10000.0)], [-5000.0, 5000.0, z(-5000.0)]], points: vec![], type_terrain: None });
    let s = depuis_natif(&p, std::path::Path::new("t.modele.json")).unwrap();
    let visible = vec![true; s.objects.len()];
    let plan = glam::Vec4::new(0.0, 0.0, -1.0, 1.2 - s.origin.z as f32);
    let haut = glam::Vec3::new(2.5 - s.origin.x as f32, -s.origin.y as f32, 10.0);
    let (i, _) = s.pick(haut, glam::Vec3::NEG_Z, &visible, Some(plan), None).unwrap();
    assert_eq!(&*s.objects[i as usize].class, "IFCWALL");
    // hors du mur, le terrain reste sélectionnable
    let loin = glam::Vec3::new(-2.0 - s.origin.x as f32, 3.0 - s.origin.y as f32, 10.0);
    let (i, _) = s.pick(loin, glam::Vec3::NEG_Z, &visible, Some(plan), None).unwrap();
    assert_eq!(&*s.objects[i as usize].class, "IFCGEOGRAPHICELEMENT");
    let topo = s.objects.iter().find(|o| &*o.class == "IFCGEOGRAPHICELEMENT").unwrap();
    assert!(topo.aretes.count > 8, "{}", topo.aretes.count);
}
