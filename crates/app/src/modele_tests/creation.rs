//! Tests déplacés de `modele/creation.rs` (`mod tests`) : ils appellent du code situé au-dessus
//! de la crate `modele` (commandes, scène, formats, échanges, collaboration ou dessin).
use modele::*;
use modele::creation::*;

#[test]
fn mur_contraintes_phases_et_sous_projets_persistes() {
    let mut p = Projet::nouveau("Contraintes");
    p.niveaux = vec![
        Niveau { id: "bas".into(), nom: "RDC".into(), altitude: 1000., origine: None },
        Niveau { id: "haut".into(), nom: "Étage".into(), altitude: 4200., origine: None },
    ];
    p.phases = vec!["Existant".into(), "Projet".into()];
    p.sous_projets = vec!["Structure".into(), "Enveloppe".into()];
    let mut options = ParametresMur { niveau: Some("bas".into()), niveau_haut: Some("haut".into()),
        decalage: 150., decalage_haut: -200., phase: Some("Projet".into()), sous_projet: Some("Enveloppe".into()), ..Default::default() };
    assert_eq!(options.bornes(&p).unwrap(), (1150., 4000.));
    p.ajouter_mur([0.,0.], [1000.,0.], &options).unwrap();
    let relu = Projet::lire_json(&p.json().unwrap()).unwrap();
    let mur = &relu.murs[0];
    assert_eq!(relu.bas_haut(mur), Some((1150.,4000.)));
    assert_eq!(mur.haut, Haut::Niveau { niveau: "haut".into(), decalage: -200. });
    assert_eq!(mur.phase.as_deref(), Some("Projet"));
    assert_eq!(mur.sous_projet.as_deref(), Some("Enveloppe"));
    assert_eq!(relu.sous_projets, p.sous_projets);
    let scene = crate::rendu::scene::depuis_natif(&relu, std::path::Path::new("test.json")).unwrap();
    assert!(((scene.max.z-scene.min.z) as f64 - 2.85).abs() < 1e-5);
    let avant = p.json().unwrap();
    for (niveau, decalage) in [("absent", 0.), ("bas", 0.), ("haut", -4000.), ("haut", f64::INFINITY)] {
        options.niveau_haut = Some(niveau.into()); options.decalage_haut = decalage;
        assert!(p.ajouter_mur([0.,0.], [1000.,0.], &options).is_err());
        assert_eq!(p.json().unwrap(), avant);
    }
    options.niveau_haut = None; options.decalage_haut = 100.;
    assert_eq!(options.bornes(&p).unwrap(), (1150.,4050.));
    options.phase = Some("Absente".into());
    assert!(p.ajouter_mur([0.,0.], [1000.,0.], &options).is_err());
    assert_eq!(p.json().unwrap(), avant);
}

#[test]
fn dalle_creation_contour_et_relecture() {
    let mut p = Projet::nouveau("Dalle");
    let c = |m: &str, e: f64| Couche { materiau: Some(m.into()), epaisseur: e, ..Default::default() };
    let t = TypeCompose { id: String::new(), nom: "Dalle BA 230 + chape".into(), couches: vec![c("Chape ciment", 50.0), c("Béton armé", 230.0)], categorie: None, couches_detaillees: true, origine: None };
    let o = ParametresDalle { sorte: SorteDalle::Dalle, type_dalle: t, pente: None, niveau: None, decalage: 0.0, phase: None, sous_projet: None, variante: None };
    // contour en sens horaire, point de fermeture répété : réorienté et nettoyé
    let id = p.ajouter_dalle(&[[0.0, 0.0], [0.0, 4000.0], [5000.0, 4000.0], [5000.0, 0.0], [0.0, 0.0]], &o).unwrap();
    let d = p.dalles.iter().find(|d| d.id == id).unwrap();
    assert_eq!(d.contours[0].len(), 4);
    assert!((p.volume_dalle(d).unwrap() - 5000.0 * 4000.0 * 280.0).abs() < 1.0);
    // même type : pas de doublon ; contours invalides refusés sans rien changer
    p.ajouter_dalle(&[[6000.0, 0.0], [8000.0, 0.0], [8000.0, 2000.0]], &o).unwrap();
    assert_eq!(p.types_dalles.len(), 1);
    let avant = p.json().unwrap();
    for mauvais in [vec![[0.0, 0.0], [1000.0, 0.0]], vec![[0.0, 0.0], [1000.0, 1000.0], [1000.0, 0.0], [0.0, 1000.0]], vec![[0.0, 0.0], [10.0, 0.0], [10.0, 10.0]]] {
        assert!(p.ajouter_dalle(&mauvais, &o).is_err());
        assert_eq!(p.json().unwrap(), avant);
    }
    let relu = Projet::lire_json(&avant).unwrap();
    assert_eq!(relu.dalles.len(), 2);
    let s = crate::rendu::scene::depuis_natif(&relu, std::path::Path::new("dalle.modele.json")).unwrap();
    assert_eq!(s.objects.len(), 2);
    let volume: f64 = s.objects.iter().map(|o| s.vertices[o.opaque.start as usize..(o.opaque.start + o.opaque.count) as usize].chunks_exact(3).map(|t| {
        let p = [0, 1, 2].map(|k| glam::Vec3::from(t[k].pos).as_dvec3());
        p[0].dot(p[1].cross(p[2])) / 6.0
    }).sum::<f64>()).sum();
    assert!((volume - (20.0 + 2.0) * 0.28).abs() < 1e-4, "Volume du maillage : {volume}");
}

#[test]
fn plafond_et_toiture_en_pente() {
    let g = modele::gabarit::Gabarit::defaut();
    let mut p = Projet::depuis_gabarit("Toit", &g);
    assert_eq!(p.types_murs.len(), g.types_murs.len());
    assert_eq!(p.niveaux.len(), 1);
    let type_de = |sorte| {
        let t = g.surfaces(sorte)[0];
        TypeCompose { id: String::new(), nom: t.nom.clone(), couches: t.couches.clone(), categorie: None, couches_detaillees: true, origine: None }
    };
    let rect = [[0.0, 0.0], [10_000.0, 0.0], [10_000.0, 6_000.0], [0.0, 6_000.0]];
    let plafond = ParametresDalle { sorte: SorteDalle::Plafond, type_dalle: type_de(SorteDalle::Plafond), pente: None, niveau: None, decalage: 2500.0, phase: None, sous_projet: None, variante: None };
    let toit = ParametresDalle { sorte: SorteDalle::Toiture, type_dalle: type_de(SorteDalle::Toiture), pente: Some(35.0), decalage: 2800.0, ..plafond.clone() };
    p.ajouter_dalle(&rect, &plafond).unwrap();
    let id = p.ajouter_dalle(&rect, &toit).unwrap();
    assert!(p.ajouter_dalle(&rect, &ParametresDalle { pente: Some(90.0), ..toit.clone() }).is_err());
    let d = p.dalles.iter().find(|d| d.id == id).unwrap().clone();
    assert_eq!(d.sorte, SorteDalle::Toiture);
    assert_eq!((0..4).filter_map(|k| d.pente(0, k)).count(), 4);
    // épaisseur normale aux versants : volume = aire × e / cos 35°
    let e = p.type_dalle(&d.type_dalle).unwrap().epaisseur();
    let v = p.volume_dalle(&d).unwrap();
    assert!((v - 60e6 * e / 35f64.to_radians().cos()).abs() / v < 1e-9, "{v}");
    let relu = Projet::lire_json(&p.json().unwrap()).unwrap();
    let s = crate::rendu::scene::depuis_natif(&relu, std::path::Path::new("toit.modele.json")).unwrap();
    assert_eq!(s.objects.len(), 2);
    let nav = s.navigateur.unwrap();
    assert!(nav.familles.iter().any(|f| f.categorie == "Plafonds") && nav.familles.iter().any(|f| f.categorie == "Toits"));
    // toiture en dessous du faîtage, sous-face à l'égout (2,80 m), plafond sous-face à 2,50 m
    let o = &s.objects[1];
    assert!((o.min.z as f64 + s.origin.z - 2.8).abs() < 1e-3, "{}", o.min.z as f64 + s.origin.z);
    let faitage = 2.8 + 3.0 * 35f64.to_radians().tan() + e / 1000.0 / 35f64.to_radians().cos();
    assert!((o.max.z as f64 + s.origin.z - faitage).abs() < 1e-3);
    assert!((s.objects[0].min.z as f64 + s.origin.z - 2.5).abs() < 1e-3);
}

#[test]
fn vues_plan_et_coupe() {
    let mut p = Projet::nouveau("Vues");
    p.ajouter_mur([0.0, 0.0], [5000.0, 0.0], &ParametresMur::default()).unwrap();
    let n = p.niveaux[0].id.clone();
    p.ajouter_vue_plan(&n, 1200.0).unwrap();
    p.ajouter_vue_plan(&n, 1500.0).unwrap();
    p.ajouter_vue_coupe([0.0, -2000.0], [5000.0, -2000.0], 8000.0, false, "Coupe de bâtiment", 100.0).unwrap();
    let e = p.ajouter_vue_coupe([5000.0, 6000.0], [0.0, 6000.0], 20000.0, true, "Élévation extérieure", 100.0).unwrap();
    assert!(p.ajouter_vue_plan("absent", 1200.0).is_err());
    assert!(p.ajouter_vue_coupe([0.0, 0.0], [10.0, 0.0], 8000.0, false, "", 100.0).is_err());
    let noms: Vec<&str> = p.vues.iter().map(|v| v.nom.as_str()).collect();
    assert_eq!(noms, ["Niveau 0", "Niveau 0 (2)", "Coupe 1", "Élévation 1"]);
    assert!(p.renommer_vue(&e, "Coupe 1").is_err());
    p.renommer_vue(&e, "Façade sud").unwrap();
    p.retourner_vue(&e).unwrap();
    assert!(matches!(p.vues[3].genre, GenreVue::Coupe { debut, .. } if debut == [0.0, 6000.0]));
    p.retourner_vue(&e).unwrap();
    let relu = Projet::lire_json(&p.json().unwrap()).unwrap();
    assert_eq!(relu.vues, p.vues);
    let s = crate::rendu::scene::depuis_natif(&relu, std::path::Path::new("vues.modele.json")).unwrap();
    let nav = s.navigateur.as_ref().unwrap();
    let genres: Vec<&str> = nav.vues.iter().map(|v| v.genre.as_str()).collect();
    assert_eq!(genres, ["Vue 3D", "Plan d'étage", "Plan d'étage", "Coupe", "Élévation"]);
    assert_eq!(nav.vues[0].nom, "Vue 3D");
    assert_eq!(nav.vues.iter().filter(|v| v.genre == "Vue 3D").count(), 1);
    // élévation tracée de droite à gauche au nord : elle regarde vers le sud (−y)
    assert!((nav.vues[4].direction - glam::DVec3::Y).length() < 1e-9, "{:?}", nav.vues[4].direction);
    // cadrage : profondeur et deux côtés
    assert_eq!(nav.vues[3].cadrage.len(), 3);
    let (p0, p1) = (glam::DVec3::new(2.5, 0.0, 0.0), glam::DVec3::new(2.5, 7.0, 0.0));
    let garde = |q: glam::DVec3| nav.vues[3].cadrage.iter().all(|pl| pl.truncate().dot(q) + pl.w >= 0.0);
    assert!(garde(p0) && !garde(p1), "au-delà de la profondeur de 8 m");
    p.supprimer_vue(&e).unwrap();
    assert_eq!(p.vues.len(), 3);
    // coupe : on regarde vers la gauche de la ligne (+y), l'observateur est côté −y
    let c = &nav.vues[3];
    assert!((c.direction - glam::DVec3::NEG_Y).length() < 1e-9, "{:?}", c.direction);
    assert!(c.plan_coupe.is_some());
}

#[test]
fn mur_creation_enregistrement_relecture() {
    let mut p = Projet::nouveau("Essai");
    let id = p.ajouter_mur([0.,0.], [5000.,0.], &ParametresMur::default()).unwrap();
    p.ajouter_mur([5000.,0.], [5000.,3000.], &ParametresMur::default()).unwrap();
    let relu = Projet::lire_json(&p.json().unwrap()).unwrap();
    assert_eq!(relu.murs[0].id, id);
    assert_eq!(relu.niveaux.len(), 1);
    assert_eq!(relu.types_murs.len(), 1);
    assert_eq!(relu.murs.len(), 2);
    assert!((relu.volume_mur(&relu.murs[0]).unwrap() - 4_200_000_000.0).abs() < 1.0);
    let s = crate::rendu::scene::depuis_natif(&relu, std::path::Path::new("essai.modele.json")).unwrap();
    assert_eq!(s.objects.len(), 2);
    // Le nombre de triangles dépend de la tessellation ; le volume fermé
    // ci-dessous est l’invariant géométrique utile après changement des identités.
    assert!(s.triangles >= 24);
    let volume: f64 = s.objects.iter().map(|o| s.vertices[o.opaque.start as usize..(o.opaque.start+o.opaque.count) as usize].chunks_exact(3).map(|t| {
        let p = [0,1,2].map(|k| glam::Vec3::from(t[k].pos).as_dvec3());
        p[0].dot(p[1].cross(p[2])) / 6.0
    }).sum::<f64>()).sum();
    assert!((volume - 6.72).abs() < 1e-5, "Volume du maillage : {volume}");
}
