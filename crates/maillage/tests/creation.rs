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
    let scene = maillage::depuis_natif(&relu, std::path::Path::new("test.json")).unwrap();
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
    let t = TypeCompose { sorte: None, id: String::new(), nom: "Dalle BA 230 + chape".into(), couches: vec![c("Chape ciment", 50.0), c("Béton armé", 230.0)], categorie: None, couches_detaillees: true, origine: None };
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
    let s = maillage::depuis_natif(&relu, std::path::Path::new("dalle.modele.json")).unwrap();
    assert_eq!(s.objects.len(), 2);
    let volume: f64 = s.objects.iter().map(|o| s.vertices[o.opaque.start as usize..(o.opaque.start + o.opaque.count) as usize].as_chunks::<3>().0.iter().map(|t| {
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
        TypeCompose { sorte: None, id: String::new(), nom: t.nom.clone(), couches: t.couches.clone(), categorie: None, couches_detaillees: true, origine: None }
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
    let s = maillage::depuis_natif(&relu, std::path::Path::new("toit.modele.json")).unwrap();
    assert_eq!(s.objects.len(), 2);
    let nav = s.navigateur.unwrap();
    assert!(nav.familles.iter().any(|f| f.categorie == "Plafonds") && nav.familles.iter().any(|f| f.categorie == "Toits"));
    // Le dessus de la toiture part de l’égout à 2,80 m. Le plafond part de sa sous-face.
    let o = &s.objects[1];
    let sous_face_egout = 2.8 - e / 1000.0 / 35f64.to_radians().cos();
    assert!((o.min.z as f64 + s.origin.z - sous_face_egout).abs() < 1e-3, "{}", o.min.z as f64 + s.origin.z);
    let faitage = 2.8 + 3.0 * 35f64.to_radians().tan();
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
    let s = maillage::depuis_natif(&relu, std::path::Path::new("vues.modele.json")).unwrap();
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
    let s = maillage::depuis_natif(&relu, std::path::Path::new("essai.modele.json")).unwrap();
    assert_eq!(s.objects.len(), 2);
    // Le nombre de triangles dépend de la tessellation ; le volume fermé
    // ci-dessous est l’invariant géométrique utile après changement des identités.
    assert!(s.triangles >= 24);
    let volume: f64 = s.objects.iter().map(|o| s.vertices[o.opaque.start as usize..(o.opaque.start+o.opaque.count) as usize].as_chunks::<3>().0.iter().map(|t| {
        let p = [0,1,2].map(|k| glam::Vec3::from(t[k].pos).as_dvec3());
        p[0].dot(p[1].cross(p[2])) / 6.0
    }).sum::<f64>()).sum();
    assert!((volume - 6.72).abs() < 1e-5, "Volume du maillage : {volume}");
}

/// Genre d'un motif : relu du JSON (absent : de dessin) ; un motif de surface de modèle
/// est marqué dans la table du rendu (mm réels).
#[test]
fn motif_de_modele_marque_pour_le_rendu() {
    let g = modele::gabarit::Gabarit::defaut();
    let mut p = Projet::depuis_gabarit("Motifs", &g);
    let t = g.surfaces(SorteDalle::Dalle)[0];
    let type_dalle = TypeCompose { sorte: None, id: String::new(), nom: t.nom.clone(), couches: t.couches.clone(), categorie: None, couches_detaillees: true, origine: None };
    let o = ParametresDalle { sorte: SorteDalle::Dalle, type_dalle, pente: None, niveau: None, decalage: 0.0, phase: None, sous_projet: None, variante: None };
    p.ajouter_dalle(&[[0., 0.], [3000., 0.], [3000., 3000.], [0., 3000.]], &o).unwrap();
    let ligne = LigneMotif { angle: 0.0, pas: 300.0, glissement: 0.0, origine: [0.0, 0.0], tirets: Vec::new() };
    p.motifs.push(DefinitionMotif { id: "carrelage".into(), nom: "Carrelage".into(), genre: GenreMotif::Modele, lignes: vec![ligne], tuile: None });
    assert!(!p.materiaux.is_empty());
    for m in &mut p.materiaux {
        m.surface_motif = Some(MotifCoupe::hachures("Carrelage"));
    }
    let relu = Projet::lire_json(&p.json().unwrap()).unwrap();
    assert_eq!(relu.motifs.iter().find(|d| d.nom == "Carrelage").unwrap().genre, GenreMotif::Modele);
    let sans_genre: DefinitionMotif = serde_json::from_str(r#"{"nom": "Ancien", "lignes": []}"#).unwrap();
    assert_eq!(sans_genre.genre, GenreMotif::Dessin);
    let scene = maillage::depuis_natif(&relu, std::path::Path::new("test.json")).unwrap();
    assert!(scene.table_motifs.iter().any(|t| t[2] == 1.0), "{:?}", scene.table_motifs);

    let tuile = modele::motif_tuile::TuileMotif {
        cote: 600.0,
        divisions: 24,
        segments: (0..24).map(|y| [[0, y], [24, y]]).collect(),
    };
    let carrelage = p.motifs.iter_mut().find(|d| d.nom == "Carrelage").unwrap();
    carrelage.lignes = tuile.lignes();
    carrelage.tuile = Some(tuile.clone());
    let scene = maillage::depuis_natif(&p, std::path::Path::new("test.json")).unwrap();
    let rang = scene.definitions_surface.iter().position(|d| d.nom == "Carrelage").unwrap();
    assert_eq!(scene.definitions_surface[rang].tuile.as_ref(), Some(&tuile));
    assert_eq!(scene.table_motifs[rang][1..], [0.0, 1.0, 600.0]);
}

/// E1 : un élément sans matériau propre prend celui de sa catégorie (couleur et poché de
/// coupe) ; sans matériau de catégorie, couleur et poché génériques.
#[test]
fn materiau_par_defaut_de_la_categorie() {
    let mut p = Projet::nouveau("Catégories");
    p.niveaux.push(Niveau { id: "n".into(), nom: "RDC".into(), altitude: 0.0, origine: None });
    p.types_poteaux.push(modele::elements::poutre::TypeSection { id: "p200".into(), nom: "Poteau 200 × 200".into(), profil: None, largeur: 200.0, hauteur: 200.0, materiau: None });
    let poteau = Barre { id: "p".into(), nom: "Poteau".into(), genre: GenreBarre::Poteau, niveau: "n".into(), debut: [0.0, 0.0, 0.0], fin: [0.0, 0.0, 2500.0], type_barre: "p200".into(), rotation: 0.0, pose: Default::default(), origine: None };
    p.barres.push(poteau);
    let couleur = |p: &Projet| {
        let s = maillage::depuis_natif(p, std::path::Path::new("c.bem")).unwrap();
        let o = s.objects.iter().find(|o| o.id == modele::identifiants::id_natif("barre", "p")).unwrap();
        (s.vertices[o.opaque.start as usize].color, s.couches_coupe.contains_key(&o.id))
    };
    let (avant, coupe) = couleur(&p);
    assert!(!coupe, "sans matériau : poché générique");
    p.styles_categories.insert(CategorieMateriau::Poteaux, StyleCategorie { materiau: Some("Brique".into()), ..Default::default() });
    p.completer_materiaux();
    p.verifier().unwrap();
    let brique = p.materiau("Brique").unwrap().couleur;
    let (apres, coupe) = couleur(&p);
    assert!(coupe, "matériau de la catégorie : bande de coupe");
    assert_ne!(avant, apres);
    assert!((0..3).all(|i| apres[i].abs_diff(brique[i]) <= 3), "{apres:?} / {brique:?}");
}

/// Styles d'objets : plume de coupe de la catégorie sur la bande du poteau, plume et
/// tirets de projection dans la scène.
#[test]
fn plumes_et_trait_de_la_categorie_dans_la_scene() {
    let mut p = Projet::nouveau("Plumes");
    p.niveaux.push(Niveau { id: "n".into(), nom: "RDC".into(), altitude: 0.0, origine: None });
    p.types_poteaux.push(modele::elements::poutre::TypeSection { id: "p200".into(), nom: "Poteau 200 × 200".into(), profil: None, largeur: 200.0, hauteur: 200.0, materiau: None });
    p.barres.push(Barre { id: "p".into(), nom: "Poteau".into(), genre: GenreBarre::Poteau, niveau: "n".into(), debut: [0.0, 0.0, 0.0], fin: [0.0, 0.0, 2500.0], type_barre: "p200".into(), rotation: 0.0, pose: Default::default(), origine: None });
    p.types_traits.push(documentation::TypeTrait { id: "tirets".into(), nom: "Tirets".into(), epaisseur: 0.25, tirets: vec![3.0, 1.5], invisible: false });
    p.styles_categories.insert(CategorieMateriau::Poteaux, StyleCategorie { materiau: Some("Béton".into()), plume_coupe: Some(8), plume_projection: Some(3), type_trait_projection: Some("tirets".into()), ..Default::default() });
    p.completer_materiaux();
    p.verifier().unwrap();
    let s = maillage::depuis_natif(&p, std::path::Path::new("p.bem")).unwrap();
    let id = modele::identifiants::id_natif("barre", "p");
    let plumes = p.epaisseurs_plumes();
    assert_eq!(s.couches_coupe[&id][0].trait_mm, plumes[7]);
    assert_eq!(s.traits_coupe.get(&id).map(|t| t.epaisseur), Some(plumes[7]));
    assert_eq!(s.traits_vus.get(&id), Some(&scene::TraitVu { epaisseur: plumes[2], tirets: vec![3.0, 1.5] }));
}
