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
        p.importes.push(ElementImporte { id: id.into(), nom: id.into(), categorie: "Autres".into(), niveau: None, maillage: Maillage::depuis_triangles(&tris), raison: String::new(), origine, donnees: Default::default() });
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

/// N° 20 : la lame d'air du mur à ossature du gabarit garde son épaisseur (les couches
/// voisines restent écartées de 25 mm) mais n'a ni géométrie ni bande de coupe.
#[test]
fn lame_air_vide_mais_epaisse() {
    use modele::creation::ParametresMur;
    let mut p = Projet::depuis_gabarit("Air", &modele::gabarit::Gabarit::defaut());
    let t = p.types_murs.iter().find(|t| t.nom == "Mur ossature bois 145").unwrap().clone();
    assert!(t.couches.iter().any(|c| c.fonction == modele::FonctionCouche::LameAir));
    p.ajouter_mur([0.0, 0.0], [4000.0, 0.0], &ParametresMur { type_mur: Some(t.id.clone()), ..Default::default() }).unwrap();
    let s = depuis_natif(&p, std::path::Path::new("t.modele.json")).unwrap();
    let couches = p.murs.iter().filter(|m| m.assemblage.is_some()).collect::<Vec<_>>();
    assert_eq!(couches.len(), t.couches.len());
    for m in &couches {
        let air = p.type_mur(&m.type_mur).unwrap().couches[0].fonction == modele::FonctionCouche::LameAir;
        let id = id_natif("mur", &m.id);
        assert_eq!(s.object_of.contains_key(&id), !air, "{} : géométrie {}", m.nom, if air { "en trop" } else { "manquante" });
        assert_eq!(s.couches_coupe.contains_key(&id), !air);
    }
    // les couches voisines restent à 25 mm l'une de l'autre : bardage (22) puis air (25)
    let ys: Vec<f64> = s.objects.iter().filter(|o| couches.iter().any(|m| s.object_of.get(&id_natif("mur", &m.id)).is_some_and(|&k| std::ptr::eq(&s.objects[k as usize], *o))))
        .flat_map(|o| [o.min.y as f64 + s.origin.y, o.max.y as f64 + s.origin.y]).collect();
    let mut faces: Vec<i64> = ys.iter().map(|y| (y * 1000.0).round() as i64).collect();
    faces.sort();
    faces.dedup();
    let total: f64 = t.couches.iter().map(|c| c.epaisseur).sum();
    // boîtes englobantes en f32 : 1 mm de tolérance
    assert!((faces.last().unwrap() - faces.first().unwrap() - total.round() as i64).abs() <= 1, "épaisseur totale conservée : {faces:?}");
    assert!(faces.windows(2).any(|w| w[1] - w[0] == 25), "vide de 25 mm entre bardage et panneau : {faces:?}");
}
