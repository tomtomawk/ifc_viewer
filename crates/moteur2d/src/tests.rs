
fn rect(x0: f64, y0: f64, x1: f64, y1: f64) -> Vec<[[f64; 2]; 3]> {
    vec![[[x0, y0], [x1, y0], [x1, y1]], [[x0, y0], [x1, y1], [x0, y1]]]
}

#[test]
fn isolant_rectangle_motif_dans_la_couche_et_le_long_de_l_axe() {
    let t = rect(0.0, 0.0, 6000.0, 100.0);
    let s = super::ligne_isolation(&t);
    assert!(!s.is_empty());
    for [a, b] in &s {
        for p in [a, b] {
            assert!(p[1] > -1e-6 && p[1] < 100.0 + 1e-6, "hors couche {p:?}");
        }
    }
    let (xmin, xmax) = s.iter().flatten().fold((f64::MAX, f64::MIN), |(m, n), p| (m.min(p[0]), n.max(p[0])));
    assert!(xmin < 100.0 && xmax > 5900.0, "motif incomplet {xmin} {xmax}");
}

#[test]
fn isolant_mur_droit_avec_onglets_reste_dans_la_couche() {
    // Coupe d'une couche droite de 145 mm, avec deux onglets et des
    // sommets intermédiaires sur les faces issus du maillage.
    let a = [62.1, 8321.7];
    let b = [0.0, 8259.6];
    let c = [0.0, 3051.3];
    let d = [0.0, -855.0];
    let e = [82.8, -937.9];
    let f = [145.0, -1000.0];
    let g = [145.0, 4374.1];
    let h = [145.0, 8404.6];
    let t = [[a, b, c], [c, d, e], [e, f, g], [g, h, a], [a, c, e], [e, g, a]];
    let s = super::ligne_isolation(&t);
    assert!(!s.is_empty());
    assert!(s.iter().flatten().all(|p| (-1e-6..=145.0 + 1e-6).contains(&p[0])));
    assert!(s.iter().flatten().any(|p| p[1] < -850.0));
    assert!(s.iter().flatten().any(|p| p[1] > 8250.0));
}

#[test]
fn isolant_en_l_garde_l_epaisseur_de_la_couche() {
    // couche de 100 retournée en tableau sur 400 : le motif reste à l'échelle de 100.
    let l: Vec<[f32; 2]> = vec![[0.0, 0.0], [3000.0, 0.0], [3000.0, 100.0], [100.0, 100.0], [100.0, 500.0], [0.0, 500.0]];
    let t: Vec<[[f64; 2]; 3]> = noyau::texte::trianguler(vec![l]).iter().map(|t| t.map(|p| [p[0] as f64, p[1] as f64])).collect();
    assert_eq!(super::composantes(&t).len(), 1);
    let s = super::ligne_isolation(&t);
    assert!(!s.is_empty());
    let ymax = s.iter().flatten().map(|p| p[1]).fold(f64::MIN, f64::max);
    assert!(ymax <= 100.0 + 1e-6, "motif dans le retour : {ymax}");
}

#[test]
fn isolant_morceaux_disjoints_traites_separement() {
    let mut t = rect(0.0, 0.0, 2000.0, 100.0);
    t.extend(rect(3000.0, 0.0, 5000.0, 100.0));
    let s = super::ligne_isolation(&t);
    assert!(s.iter().any(|[a, _]| a[0] < 2000.0) && s.iter().any(|[a, _]| a[0] > 3000.0));
    assert!(s.iter().all(|[a, b]| !(a[0] > 2000.0 + 1e-6 && a[0] < 3000.0 - 1e-6) || !(b[0] > 2000.0 && b[0] < 3000.0)));
}

#[test]
fn isolant_suit_une_couche_courbe() {
    // couronne de rayons 5000 à 5100 sur 90°, en 48 facettes
    let mut t = Vec::new();
    let p = |r: f64, a: f64| [r * a.cos(), r * a.sin()];
    for k in 0..48 {
        let (a0, a1) = (k as f64 / 48.0 * std::f64::consts::FRAC_PI_2, (k + 1) as f64 / 48.0 * std::f64::consts::FRAC_PI_2);
        t.push([p(5000.0, a0), p(5100.0, a0), p(5100.0, a1)]);
        t.push([p(5000.0, a0), p(5100.0, a1), p(5000.0, a1)]);
    }
    let s = super::ligne_isolation(&t);
    assert!(s.len() > 100);
    for p in s.iter().flatten() {
        let r = p[0].hypot(p[1]);
        assert!(r > 4999.0 && r < 5101.0, "hors couche {r}");
    }
    let angles: Vec<f64> = s.iter().flatten().map(|p| p[1].atan2(p[0])).collect();
    assert!(angles.iter().any(|a| *a < 0.1) && angles.iter().any(|a| *a > 1.47), "motif incomplet : {:?}", angles.iter().fold((f64::INFINITY, f64::NEG_INFINITY), |(a,b), &x| (a.min(x), b.max(x))));
}

#[test]
fn isolant_courbe_aux_unites_de_la_scene() {
    let p = |r: f64, a: f64| [r * a.cos(), r * a.sin()];
    let mut triangles = Vec::new();
    for k in 0..48 {
        let a = k as f64 / 48.0 * std::f64::consts::FRAC_PI_2;
        let b = (k + 1) as f64 / 48.0 * std::f64::consts::FRAC_PI_2;
        triangles.push([p(5.0, a), p(5.1, a), p(5.1, b)]);
        triangles.push([p(5.0, a), p(5.1, b), p(5.0, b)]);
    }
    let s = super::ligne_isolation(&triangles);
    assert!(s.len() > 100, "{} segments", s.len());
    assert!(s.iter().flatten().all(|p| (4.999..5.101).contains(&p[0].hypot(p[1]))));
    assert!(s.iter().flatten().any(|p| p[1].atan2(p[0]) > 1.47));
}

#[test]
fn isolant_suit_le_mur_courbe_dans_la_coupe_native() {
    use modele::{Axe, Couche, FonctionCouche, Projet, TypeCompose};
    use modele::creation::ParametresMur;
    // familles de matériaux du gabarit (isolation hachurée), copiées à la création
    let mut projet = Projet::nouveau("Mur courbe");
    projet.familles_materiaux = modele::gabarit::Gabarit::defaut().materiaux;
    projet.types_murs.push(TypeCompose { sorte: None, id: "isolant".into(), nom: "Isolant".into(), couches: vec![Couche { materiau: Some("Laine minérale".into()), epaisseur: 100.0, fonction: FonctionCouche::Isolation, ..Default::default() }], categorie: None, couches_detaillees: true, origine: None });
    let options = ParametresMur { type_mur: Some("isolant".into()), ..Default::default() };
    projet.ajouter_mur_axe(Axe::Arc { debut: [17000.0, -8000.0], fin: [12000.0, -3000.0], centre: [12000.0, -8000.0], direct: true }, &options).unwrap();
    projet.completer_materiaux();
    let scene = maillage::depuis_natif(&projet, std::path::Path::new("test.modele.json")).unwrap();
    let plan = Vec3::NEG_Z.extend((1.0 - scene.origin.z) as f32);
    let primitives = super::coupe(&scene, &vec![true; scene.objects.len()], TOUT, plan, 100.0, Poche::Hachure);
    let segments: Vec<_> = primitives.iter().filter(|p| p.role == Role::Hachure).flat_map(|p| match &p.forme { Forme::Segments(s) => s.clone(), _ => Vec::new() }).collect();
    assert!(segments.len() > 100, "{} segments", segments.len());
    let rayon = |p: DVec3| (p.x + scene.origin.x - 12.0).hypot(p.y + scene.origin.y + 8.0);
    assert!(segments.iter().flatten().all(|&p| (4.94..5.06).contains(&rayon(p))), "le motif sort de la couche");
    let angles: Vec<_> = segments.iter().flatten().map(|p| (p.y + scene.origin.y + 8.0).atan2(p.x + scene.origin.x - 12.0)).collect();
    assert!(angles.iter().any(|a| *a < 0.1) && angles.iter().any(|a| *a > 1.47), "motif incomplet : {:?}", angles.iter().fold((f64::INFINITY, f64::NEG_INFINITY), |(a,b), &x| (a.min(x), b.max(x))));
}

#[test]
fn ossature_coupee_en_plan_montants_et_travees() {
    use modele::{Axe, Couche, FonctionCouche, Materiau, Projet, TypeCompose};
    use modele::creation::ParametresMur;
    use modele::murs::ossature::Ossature;
    let mut projet = Projet::nouveau("Ossature");
    let couche = Couche { materiau: Some("Laine".into()), epaisseur: 145.0, fonction: FonctionCouche::Structure, ossature: Some(Ossature { materiau: Some("Sapin".into()), ..Default::default() }), ..Default::default() };
    projet.types_murs.push(TypeCompose { sorte: None, id: "mob".into(), nom: "MOB".into(), couches: vec![couche], categorie: None, couches_detaillees: true, origine: None });
    projet.materiaux.push(Materiau { nom: "Sapin".into(), couleur: [200, 150, 90], ..Default::default() });
    projet.materiaux.push(Materiau { nom: "Laine".into(), couleur: [250, 230, 120], motif: modele::MotifCoupe::Isolation, ..Default::default() });
    let options = ParametresMur { type_mur: Some("mob".into()), ..Default::default() };
    projet.ajouter_mur([0.0, 0.0], [3000.0, 0.0], &options).unwrap();
    projet.ajouter_mur_axe(Axe::Arc { debut: [17000.0, -8000.0], fin: [12000.0, -3000.0], centre: [12000.0, -8000.0], direct: true }, &options).unwrap();
    let scene = maillage::depuis_natif(&projet, std::path::Path::new("test.modele.json")).unwrap();
    let plan = Vec3::NEG_Z.extend((1.0 - scene.origin.z) as f32);
    let primitives = super::coupe(&scene, &vec![true; scene.objects.len()], TOUT, plan, 100.0, Poche::Materiau);
    let aire = |couleur: [u8; 4], droit: bool| -> f64 {
        primitives.iter().filter(|p| p.role == Role::Poche && p.style.couleur == couleur).flat_map(|p| match &p.forme { Forme::Triangles(t) => t.clone(), _ => Vec::new() })
            .filter(|t| ((t[0].y + scene.origin.y).abs() < 0.5) == droit)
            .map(|t| (t[1] - t[0]).cross(t[2] - t[0]).length() / 2.0).sum()
    };
    // mur droit de 3 m : 6 montants de 45 × 145, le reste en remplissage
    let (bois, laine) = (aire([200, 150, 90, 255], true), aire([250, 230, 120, 255], true));
    assert!((bois - 6.0 * 0.045 * 0.145).abs() < 1e-6, "{bois}");
    assert!((bois + laine - 3.0 * 0.145).abs() < 1e-6, "{laine}");
    // la ligne d'isolant de chaque travée reste dans l'épaisseur du mur droit
    let hachures = super::coupe(&scene, &vec![true; scene.objects.len()], TOUT, plan, 100.0, Poche::Hachure);
    let isolant: Vec<_> = hachures.iter().filter(|p| p.role == Role::Hachure).flat_map(|p| match &p.forme { Forme::Segments(s) => s.clone(), _ => Vec::new() })
        .filter(|s| (s[0].y + scene.origin.y).abs() < 0.5).collect();
    assert!(isolant.len() > 100);
    assert!(isolant.iter().flatten().all(|q| (q.y + scene.origin.y).abs() < 0.08 && (-0.001..3.001).contains(&(q.x + scene.origin.x))), "ligne d'isolant hors du mur : {:?}", isolant.iter().flatten().filter(|q| !((q.y + scene.origin.y).abs() < 0.08 && (-0.001..3.001).contains(&(q.x + scene.origin.x)))).map(|q| (((q.x + scene.origin.x) * 1e4).round() / 1e4, ((q.y + scene.origin.y) * 1e4).round() / 1e4)).take(12).collect::<Vec<_>>());
    // mur courbe (quart de cercle de 5 m de rayon) : 14 montants, sans doublon en face
    let (bois, laine) = (aire([200, 150, 90, 255], false), aire([250, 230, 120, 255], false));
    let total = std::f64::consts::FRAC_PI_2 * 5.0 * 0.145;
    assert!((bois + laine - total).abs() < 1e-3, "{bois} + {laine} / {total}");
    assert!((bois - 14.0 * 0.045 * 0.145).abs() < 2e-3, "{bois}");
}

#[test]
fn fenetre_coupee_piece_par_piece() {
    use modele::{Ouverture, Projet};
    let g = modele::gabarit::Gabarit::defaut();
    let mut p = Projet::depuis_gabarit("OF", &g);
    let tm = p.types_murs.iter().find(|t| t.nom.starts_with("Mur BA 200 + doublage")).unwrap().id.clone();
    let mur = p.ajouter_mur([0.0, 0.0], [5000.0, 0.0], &modele::creation::ParametresMur { type_mur: Some(tm), ..Default::default() }).unwrap();
    for (k, t) in p.types_ouvertures.clone().iter().filter(|t| t.genre == modele::GenreOuverture::Fenetre).enumerate() {
        p.ouvertures = vec![Ouverture { id: format!("o{k}"), nom: "F".into(), genre: t.genre, mur: mur.clone(), type_ouverture: Some(t.id.clone()), abscisse: 2500.0, largeur: t.largeur, hauteur: t.hauteur, allege: 100.0, representation: None, valeurs: Default::default(), materiaux: Default::default(), retournee: false, inversee: false, origine: None }];
        p.verifier().unwrap();
        let scene = maillage::depuis_natif(&p, std::path::Path::new("test.modele.json")).unwrap();
        let id = modele::identifiants::id_natif("ouverture", &format!("o{k}"));
        // coupe verticale dans un vantail, coupe horizontale à mi-hauteur
        for plan in [Vec3::NEG_X.extend((2.3 - scene.origin.x) as f32), Vec3::NEG_Z.extend((0.8 - scene.origin.z) as f32)] {
            let prims: Vec<_> = super::coupe(&scene, &vec![true; scene.objects.len()], TOUT, plan, 20.0, Poche::Hachure).into_iter().filter(|p| p.element == Some(id)).collect();
            // chaque pièce a son contour fermé
            assert!(prims.iter().all(|p| !matches!(p.forme, Forme::Polyligne { fermee: false, .. })), "{} : contour ouvert", t.nom);
            let pieces = prims.iter().filter(|p| p.role == Role::Poche).count();
            assert!(pieces >= 4, "{} : {pieces} pièces", t.nom);
            // vitrage : blanc, trait fin, sans hachure ; menuiserie hachurée
            let vitrages = prims.iter().filter(|p| p.role == Role::Poche && p.style.couleur == [255, 255, 255, 255]).count();
            assert!(vitrages >= 1 && vitrages < pieces, "{} : {vitrages} vitrages", t.nom);
            assert_eq!(prims.iter().filter(|p| p.role == Role::Coupe && p.style.epaisseur == TRAIT_FIN_MM).count(), vitrages);
            assert_eq!(prims.iter().filter(|p| p.role == Role::Hachure).count(), pieces - vitrages, "{}", t.nom);
        }
    }
}

#[test]
fn isolant_tres_mince_et_long_n_explose_pas() {
    let s = super::ligne_isolation(&rect(0.0, 0.0, 30000.0, 1.0));
    assert!(s.is_empty());
}
use super::*;
use noyau::aretes::aretes_jointes;

/// Deux pavés du même matériau bout à bout : la jonction ne trace aucune arête ; en
/// équerre, l'angle extérieur reste.
#[test]
fn jonction_sans_arete_meme_materiau() {
    // boîte en 12 triangles orientés vers l'extérieur (comme l'aide de test de
    // `modele::volumes::geometrie`)
    let pave = |mn: DVec3, mx: DVec3| -> Vec<[DVec3; 3]> {
        let p = |i: usize| DVec3::new([mn.x, mx.x][i & 1], [mn.y, mx.y][(i >> 1) & 1], [mn.z, mx.z][i >> 2]);
        let faces = [[0, 2, 3, 1], [4, 5, 7, 6], [0, 1, 5, 4], [2, 6, 7, 3], [0, 4, 6, 2], [1, 3, 7, 5]];
        faces.iter().flat_map(|f| [[p(f[0]), p(f[1]), p(f[2])], [p(f[0]), p(f[2]), p(f[3])]]).collect()
    };
    let mut tris: Vec<([DVec3; 3], usize)> = pave(DVec3::ZERO, DVec3::new(1.0, 0.2, 1.0)).into_iter().map(|t| (t, 0)).collect();
    tris.extend(pave(DVec3::new(1.0, 0.0, 0.0), DVec3::new(2.0, 0.2, 1.0)).into_iter().map(|t| (t, 1)));
    let (vives, _) = aretes_jointes(&tris);
    // aucune arête dans le plan x = 1
    assert!(vives.iter().all(|(s, _)| !(s[0].x - 1.0).abs().lt(&1e-9) || !(s[1].x - 1.0).abs().lt(&1e-9)), "{vives:?}");
    // 12 arêtes d'un pavé 2 × 0,2 × 1 (les 4 longues coupées en deux, une par pavé)
    assert_eq!(vives.len(), 16);
    // sans jonction (calcul ordinaire) : le contour du plan x = 1 reste
    let seuls = aretes(&tris.iter().map(|t| t.0).collect::<Vec<_>>());
    assert!(seuls.vives.iter().any(|s| (s[0].x - 1.0).abs() < 1e-9 && (s[1].x - 1.0).abs() < 1e-9));
}

/// Cube unité en 12 triangles : 12 arêtes, aucune diagonale.
fn cube() -> Vec<[DVec3; 3]> {
    let v = |x: f64, y: f64, z: f64| DVec3::new(x, y, z);
    let faces = [
        [v(0., 0., 0.), v(0., 1., 0.), v(1., 1., 0.), v(1., 0., 0.)],
        [v(0., 0., 1.), v(1., 0., 1.), v(1., 1., 1.), v(0., 1., 1.)],
        [v(0., 0., 0.), v(1., 0., 0.), v(1., 0., 1.), v(0., 0., 1.)],
        [v(1., 1., 0.), v(0., 1., 0.), v(0., 1., 1.), v(1., 1., 1.)],
        [v(0., 1., 0.), v(0., 0., 0.), v(0., 0., 1.), v(0., 1., 1.)],
        [v(1., 0., 0.), v(1., 1., 0.), v(1., 1., 1.), v(1., 0., 1.)],
    ];
    faces.iter().flat_map(|f| [[f[0], f[1], f[2]], [f[0], f[2], f[3]]]).collect()
}

fn longueur(a: &[[DVec3; 2]]) -> f64 {
    a.iter().map(|s| s[0].distance(s[1])).sum()
}

fn scene_section(tris: Vec<[DVec3; 3]>) -> Scene {
    use scene::{Range, SceneObject};
    let mut s = maillage::depuis_natif(&modele::Projet::nouveau("Test"), std::path::Path::new("test.modele.json")).unwrap();
    s.vertices = tris.into_iter().flatten().map(|p| Vertex { pos: p.as_vec3().to_array(), normal: [0, 0, 127, 0], color: [140, 160, 180, 255], obj: 0 }).collect();
    s.couleurs_rendu = vec![[110, 130, 150, 255]; s.vertices.len()];
    s.objects.push(SceneObject { id: 1, class: "IFCWALL".into(), storey: None, opaque: Range { start: 0, count: s.vertices.len() as u32 }, transparent: Range { start: 0, count: 0 }, lines: Range { start: 0, count: 0 }, aretes: Range { start: 0, count: 0 }, min: Vec3::ZERO, max: Vec3::ONE });
    s.object_of.insert(1, 0);
    s
}

fn aire_poche(v: &[Vertex]) -> f64 {
    v.as_chunks::<3>().0.iter().filter(|t| t[0].color == [75, 75, 75, 255]).map(|t| {
        let p = [0, 1, 2].map(|k| Vec3::from(t[k].pos).as_dvec3());
        (p[1] - p[0]).cross(p[2] - p[0]).length() * 0.5
    }).sum()
}

#[test]
fn section_box_six_faces_pleines() {
    let s = scene_section(cube());
    let a = Vec3::splat(0.2);
    let b = Vec3::splat(0.8);
    let (v, _) = sommets_boite(&s, &[true], TOUT, a, b, 100.0, Some(Poche::Plein), false, false);
    assert!((aire_poche(&v) - 6.0 * 0.6 * 0.6).abs() < 1e-5);
    assert!(v.iter().all(|v| { let p = Vec3::from(v.pos); p.cmpge(a - Vec3::splat(1e-6)).all() && p.cmple(b + Vec3::splat(1e-6)).all() }));
}

#[test]
fn section_box_conserve_le_vide_interieur() {
    let mut tris = cube();
    tris.extend(cube().into_iter().map(|t| t.map(|p| p * 0.5 + DVec3::splat(0.25))));
    let s = scene_section(tris);
    let (v, _) = sommets_boite(&s, &[true], TOUT, Vec3::splat(-1.0), Vec3::new(2.0, 2.0, 0.5), 100.0, Some(Poche::Plein), false, false);
    assert!((aire_poche(&v) - 0.75).abs() < 1e-6, "aire {}", aire_poche(&v));
}

#[test]
fn section_box_couches_materiaux_et_cavite() {
    let mut tris = cube();
    tris.extend(cube().into_iter().map(|t| t.map(|p| p * 0.5 + DVec3::splat(0.25))));
    let mut s = scene_section(tris);
    let bande = |a: f32, b: f32, couleur| scene::CoucheCoupe {
        plans: vec![Vec3::X.extend(-a),Vec3::NEG_X.extend(b)], couleur, fond: [255,255,255,255], motif_fond: None, motif: None, couleur_motif: [70,70,70,255], hachure_defaut: true, isolation: false, centre_isolation: None, trait_mm: TRAIT_FORT_MM,
    };
    s.couches_coupe.insert(1,vec![bande(0.0,0.2,[190,120,120,255]),bande(0.2,1.0,[120,190,120,255])]);
    let p = coupe(&s,&[true], TOUT,Vec3::NEG_Z.extend(0.5),100.0,Poche::Hachure);
    let surfaces: Vec<_> = p.iter().filter(|p| p.role == Role::Poche).collect();
    assert_eq!(surfaces.len(),2);
    let aire = |p: &Primitive| p.mailler(DVec3::NEG_Z,100.0).triangles.iter().map(|t| (t[1]-t[0]).cross(t[2]-t[0]).length()*0.5).sum::<f64>();
    assert!((aire(surfaces[0])-0.2).abs()<1e-6);
    assert!((aire(surfaces[1])-0.55).abs()<1e-6);
    assert!(p.iter().any(|p| p.role == Role::Hachure));
    let (v,_) = sommets_boite(&s,&[true], TOUT,Vec3::splat(-1.0),Vec3::new(2.,2.,0.5),100.0,None,false, false);
    assert!(v.iter().any(|v| v.color == [190,120,120,255]));
    assert!(v.iter().any(|v| v.color == [120,190,120,255]));
}

#[test]
fn section_box_hachures_bornees() {
    let s = scene_section(cube());
    let (a, b) = (Vec3::splat(0.2), Vec3::splat(0.8));
    let (_, traits) = sommets_boite(&s, &[true], TOUT, a, b, 100.0, Some(Poche::Hachure), false, false);
    assert!(!traits.is_empty());
    assert!(traits.iter().all(|v| { let p = Vec3::from(v.pos); p.cmpge(a - Vec3::splat(1e-6)).all() && p.cmple(b + Vec3::splat(1e-6)).all() }));
}

#[test]
fn section_box_sans_coupe_ou_objet_masque() {
    let s = scene_section(cube());
    for (visible, a, b) in [(true, Vec3::splat(-1.0), Vec3::splat(2.0)), (false, Vec3::splat(0.2), Vec3::splat(0.8))] {
        let (v, t) = sommets_boite(&s, &[visible], TOUT, a, b, 100.0, Some(Poche::Plein), false, false);
        assert!(v.is_empty() && t.is_empty());
    }
}

#[test]
fn section_box_couleur_matiere_et_modification() {
    let s = scene_section(cube());
    for rendu in [false, true] {
        let (v, _) = sommets_boite(&s, &[true], TOUT, Vec3::splat(0.2), Vec3::splat(0.8), 100.0, None, rendu, false);
        let couleur = if rendu { [110, 130, 150, 255] } else { [140, 160, 180, 255] };
        assert!(v.iter().any(|v| v.color == couleur));
        assert!(v.iter().all(|v| v.obj == 0));
    }
    let (v, _) = sommets_boite(&s, &[true], TOUT, Vec3::splat(0.3), Vec3::splat(0.7), 100.0, Some(Poche::Plein), false, false);
    assert!((aire_poche(&v) - 6.0 * 0.4 * 0.4).abs() < 1e-5);
}

#[test]
fn aretes_cube() {
    let a = aretes(&cube());
    assert!((longueur(&a.vives) - 12.0).abs() < 1e-6, "{}", longueur(&a.vives));
    assert!(a.douces.is_empty());
}

/// Face du dessus découpée avec une jonction en T (comme après une CSG) : la découpe
/// n'est pas une arête.
#[test]
fn jonction_en_t() {
    let mut t = cube();
    t.retain(|t| !t.iter().all(|p| p.z == 1.0));
    let v = |x: f64, y: f64| DVec3::new(x, y, 1.0);
    // moitié gauche en deux triangles, moitié droite en trois (sommet en T à (0.5, 0.5))
    t.extend([[v(0., 0.), v(0.5, 0.), v(0.5, 1.)], [v(0., 0.), v(0.5, 1.), v(0., 1.)]]);
    t.extend([[v(0.5, 0.), v(1., 0.), v(0.5, 0.5)], [v(1., 0.), v(1., 1.), v(0.5, 0.5)], [v(0.5, 0.5), v(1., 1.), v(0.5, 1.)]]);
    let a = aretes(&t).vives;
    assert!((longueur(&a) - 12.0).abs() < 1e-6, "{}", longueur(&a));
}

/// Cylindre à 32 facettes : seuls les deux cercles sont des arêtes vives ; les 32
/// génératrices sont des arêtes douces (silhouettes).
#[test]
fn cylindre_sans_generatrices() {
    let k = 32;
    let p = |i: usize, z: f64| {
        let a = i as f64 / k as f64 * std::f64::consts::TAU;
        DVec3::new(a.cos(), a.sin(), z)
    };
    let mut t = Vec::new();
    for i in 0..k {
        let j = (i + 1) % k;
        t.push([p(i, 0.), p(j, 0.), p(j, 1.)]);
        t.push([p(i, 0.), p(j, 1.), p(i, 1.)]);
        t.push([DVec3::ZERO, p(j, 0.), p(i, 0.)]);
        t.push([DVec3::Z, p(i, 1.), p(j, 1.)]);
    }
    let a = aretes(&t);
    let perimetre = 2.0 * k as f64 * 2.0 * (std::f64::consts::PI / k as f64).sin();
    assert!((longueur(&a.vives) - perimetre).abs() < 1e-6, "{} {}", longueur(&a.vives), perimetre);
    assert_eq!(a.douces.len(), k);
    assert!(a.douces.iter().all(|(s, n1, n2)| (s[0].distance(s[1]) - 1.0).abs() < 1e-9 && n1.dot(*n2) < 0.99));
}

/// Polygone découpé : un triangle à cheval garde un quadrilatère.
#[test]
fn decoupe_triangle() {
    let t = [DVec3::ZERO, DVec3::new(2.0, 0.0, 0.0), DVec3::new(0.0, 0.0, 2.0)];
    let q = decouper(&t, DVec3::NEG_Z, 1.0);
    assert_eq!(q.len(), 4);
    assert!(q.iter().all(|p| p.z <= 1.0 + 1e-12));
}

#[test]
fn coupe_carree() {
    let segs: Vec<[[f64; 2]; 2]> = vec![[[0., 0.], [1., 0.]], [[1., 1.], [1., 0.]], [[1., 1.], [0., 1.]], [[0., 1.], [0., 0.]], [[3., 0.], [4., 0.]]];
    let (b, o) = chainer(&segs);
    assert_eq!(b.len(), 1);
    assert_eq!(b[0].len(), 4);
    assert_eq!(o.len(), 1);
    let s = section([DVec3::ZERO, DVec3::new(1., 0., 2.), DVec3::new(0., 1., 2.)], DVec3::NEG_Z, 1.0).unwrap();
    assert!((s[0].z - 1.0).abs() < 1e-12 && (s[1].z - 1.0).abs() < 1e-12);
}

/// N° 19 : en coupe verticale d'un mur à ossature (lisses horizontales ou montants),
/// sur un niveau surélevé ou non, la ligne d'isolant de chaque travée couvre toute sa
/// hauteur et reste dedans.
#[test]
fn isolant_ossature_couvre_chaque_travee_en_coupe() {
    use modele::{Couche, FonctionCouche, Materiau, Projet, TypeCompose};
    use modele::creation::ParametresMur;
    use modele::murs::ossature::Ossature;
    for (horizontale, altitude) in [(false, 0.0), (true, 0.0), (true, 3000.0)] {
        let mut projet = Projet::depuis_gabarit("Ossature", &modele::gabarit::Gabarit::defaut());
        for n in &mut projet.niveaux { n.altitude += altitude; }
        let couche = Couche { materiau: Some("Laine".into()), epaisseur: 145.0, fonction: FonctionCouche::Structure, ossature: Some(Ossature { materiau: Some("Sapin".into()), horizontale, ..Default::default() }), ..Default::default() };
        projet.types_murs.push(TypeCompose { sorte: None, id: "mob".into(), nom: "MOB".into(), couches: vec![couche], categorie: None, couches_detaillees: true, origine: None });
        projet.materiaux.push(Materiau { nom: "Sapin".into(), couleur: [200, 150, 90], ..Default::default() });
        projet.materiaux.push(Materiau { nom: "Laine".into(), couleur: [250, 230, 120], motif: modele::MotifCoupe::Isolation, ..Default::default() });
        projet.ajouter_mur([0.0, 0.0], [3000.0, 0.0], &ParametresMur { type_mur: Some("mob".into()), ..Default::default() }).unwrap();
        let scene = maillage::depuis_natif(&projet, std::path::Path::new("test.modele.json")).unwrap();
        let plan = Vec3::NEG_X.extend((0.3 - scene.origin.x) as f32);
        let prims = super::coupe(&scene, &vec![true; scene.objects.len()], TOUT, plan, 50.0, Poche::Hachure);
        let etendue = |pts: Vec<DVec3>| pts.iter().map(|q| q.z + scene.origin.z).fold((f64::INFINITY, f64::NEG_INFINITY), |(a, b), z| (a.min(z), b.max(z)));
        // travées de remplissage : surfaces hautes de plus de 10 cm
        let travees: Vec<(f64, f64)> = prims.iter().filter(|p| p.role == Role::Poche).filter_map(|p| match &p.forme { Forme::Triangles(t) => Some(etendue(t.iter().flatten().copied().collect())), _ => None }).filter(|(a, b)| b - a > 0.1).collect();
        let hachures: Vec<(f64, f64)> = prims.iter().filter(|p| p.role == Role::Hachure).filter_map(|p| match &p.forme { Forme::Segments(s) => Some(etendue(s.iter().flatten().copied().collect())), _ => None }).collect();
        assert!(!travees.is_empty());
        for (a, b) in travees {
            assert!(hachures.iter().any(|(h0, h1)| (h0 - a).abs() < 0.02 && (h1 - b).abs() < 0.02), "horizontale={horizontale}, altitude={altitude} : travée [{a:.3}, {b:.3}] mal couverte ; hachures {hachures:?}");
        }
    }
}

/// F9 : pieds de table réservés au niveau de détail élevé et plateau exclu des plans ;
/// la coupe et les arêtes vues suivent l'affichage demandé.
#[test]
fn coupe_et_vu_filtrent_les_elements_de_famille() {
    use modele::elements::composant::{type_table, Composant};
    use modele::famille::modelee::{Affichage, NiveauDetail, TypeVue};
    let mut p = modele::Projet::nouveau("T");
    p.niveaux.push(modele::Niveau { id: "n".into(), nom: "RDC".into(), altitude: 0.0, origine: None });
    let mut t = type_table();
    for pied in &mut t.modele.volumes[1..] {
        pied.visibilite.faible = false;
        pied.visibilite.moyen = false;
    }
    t.modele.volumes[0].visibilite.plan = false;
    p.composants.push(Composant { id: "c".into(), nom: "c".into(), type_composant: t.id.clone(), niveau: "n".into(), position: [0.0; 2], decalage: 0.0, rotation: 0.0 });
    p.types_composants.push(t);
    let s = maillage::depuis_natif(&p, std::path::Path::new("t.modele.json")).unwrap();
    let visible = vec![true; s.objects.len()];
    let a = |detail, vue| Affichage { detail, vue };
    // coupe à 40 cm : seuls les pieds sont coupés
    let plan = Vec3::NEG_Z.extend((0.4 - s.origin.z) as f32);
    let coupees = |x| super::coupe(&s, &visible, x, plan, 50.0, Poche::Hachure).len();
    assert!(coupees(a(NiveauDetail::Eleve, TypeVue::Plan)) > 0);
    assert_eq!(coupees(a(NiveauDetail::Faible, TypeVue::Plan)), 0);
    // vu de dessus : le plateau n'est pas dessiné en plan, il l'est en coupe
    let vues = |x| super::vu(&s, &visible, x, DVec3::X, DVec3::Y, DVec3::Z, None, 256).len();
    assert_eq!(vues(a(NiveauDetail::Faible, TypeVue::Plan)), 0);
    assert!(vues(a(NiveauDetail::Faible, TypeVue::Coupe)) > 0);
    assert!(vues(a(NiveauDetail::Eleve, TypeVue::Plan)) > 0);
}

/// P1 : coupe reprise du cache après le déplacement d'un mur : même résultat qu'une
/// coupe complète, seuls les objets modifiés sont recoupés ; un changement d'échelle
/// vide le cache.
#[test]
fn coupe_avec_cache_identique_et_partielle() {
    use modele::creation::ParametresMur;
    use modele::Projet;
    let mut p = Projet::depuis_gabarit("Cache", &modele::gabarit::Gabarit::defaut());
    for k in 0..6 {
        let y = k as f64 * 3000.0;
        p.ajouter_mur([0.0, y], [8000.0, y], &ParametresMur::default()).unwrap();
    }
    let chemin = std::path::Path::new("cache.modele.json");
    let decrire = |prims: &[Primitive]| { let mut v: Vec<String> = prims.iter().map(|x| format!("{:?}", x.element)).collect(); v.sort(); v };
    let s = maillage::depuis_natif(&p, chemin).unwrap();
    let plan = Vec3::NEG_Z.extend((1.0 - s.origin.z) as f32);
    let vis = vec![true; s.objects.len()];
    let mut cache = super::CacheCoupe::default();
    let a = super::coupe_avec_cache(&s, &vis, TOUT, plan, 100.0, Poche::Hachure, &mut cache);
    assert_eq!(cache.recoupes, s.objects.len() - s.objects.iter().filter(|o| &*o.class == "IFCANNOTATION").count());
    assert_eq!(decrire(&a), decrire(&super::coupe(&s, &vis, TOUT, plan, 100.0, Poche::Hachure)));
    // même scène : rien n'est recoupé
    let b = super::coupe_avec_cache(&s, &vis, TOUT, plan, 100.0, Poche::Hachure, &mut cache);
    assert_eq!(cache.recoupes, 0);
    assert_eq!(a.len(), b.len());
    // un mur déplacé loin des autres : lui seul est recoupé
    let mut q = p.clone();
    if let modele::Axe::Droit { debut, fin } = &mut q.murs[5].axe { debut[1] += 5000.0; fin[1] += 5000.0; }
    let s2 = maillage::depuis_natif(&q, chemin).unwrap();
    // l'emprise a changé, l'origine (arrondie à 100 m) non
    assert_eq!(s2.origin, s.origin);
    let vis = vec![true; s2.objects.len()];
    let c = super::coupe_avec_cache(&s2, &vis, TOUT, plan, 100.0, Poche::Hachure, &mut cache);
    assert!(cache.recoupes >= 1 && cache.recoupes <= 2, "{} recoupés", cache.recoupes);
    assert_eq!(decrire(&c), decrire(&super::coupe(&s2, &vis, TOUT, plan, 100.0, Poche::Hachure)));
    // autre échelle : tout est recoupé
    super::coupe_avec_cache(&s, &vec![true; s.objects.len()], TOUT, plan, 50.0, Poche::Hachure, &mut cache);
    assert!(cache.recoupes >= 6);
}

/// Styles d'objets : lignes vues d'un objet à l'épaisseur de sa catégorie, en tirets
/// mesurés en mm du papier à l'échelle de la vue ; les autres objets restent intacts.
#[test]
fn lignes_vues_stylees_par_objet() {
    let mut s = scene_section(vec![[DVec3::ZERO, DVec3::X, DVec3::Y]]);
    let id = s.objects[0].id;
    let vu = |e: u64| Primitive { forme: Forme::Segments(vec![[DVec3::ZERO, DVec3::new(1.0, 0.0, 0.0)]]), style: noyau::trace2d::Style { couleur: [0, 0, 0, 255], epaisseur: 0.0 }, role: Role::Vu, element: Some(e) };
    s.traits_vus.insert(id, scene::TraitVu { epaisseur: 0.35, tirets: vec![3.0, 2.0] });
    // 1 m au 1:100 = 10 mm papier : tirets de 3 mm (0,3 m), blancs de 2 mm (0,2 m)
    let p = super::styler_traits(vec![vu(id), vu(id + 1)], &s, DVec3::X, DVec3::Y, 100.0);
    assert_eq!(p[0].style.epaisseur, 0.35);
    let Forme::Segments(t) = &p[0].forme else { panic!() };
    let longueurs: Vec<f64> = t.iter().map(|[a, b]| ((b.x - a.x) * 1000.0).round() / 1000.0).collect();
    assert_eq!(longueurs, vec![0.3, 0.3]);
    assert!((t[1][0].x - 0.5).abs() < 1e-9);
    assert_eq!(p[1].style.epaisseur, 0.0);
    assert!(matches!(&p[1].forme, Forme::Segments(v) if v.len() == 1));
    // contour coupé fermé (carré de 1 m) : tirets de coupe, épaisseur de la bande gardée
    s.traits_coupe.insert(id, scene::TraitVu { epaisseur: 0.5, tirets: vec![5.0, 5.0] });
    let carre = vec![DVec3::ZERO, DVec3::X, DVec3::new(1.0, 1.0, 0.0), DVec3::Y];
    let contour = Primitive { forme: Forme::Polyligne { points: carre, fermee: true }, style: noyau::trace2d::Style { couleur: [0, 0, 0, 255], epaisseur: 0.18 }, role: Role::Coupe, element: Some(id) };
    let p = super::styler_traits(vec![contour], &s, DVec3::X, DVec3::Y, 100.0);
    assert_eq!(p[0].style.epaisseur, 0.18);
    // 4 côtés de 1 m au 1:100, tirets de 0,5 m : un tiret par côté
    assert!(matches!(&p[0].forme, Forme::Segments(v) if v.len() == 4), "{:?}", p[0].forme);
}
