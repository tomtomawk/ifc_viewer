use super::*;
use std::path::PathBuf;

#[test]
fn couches_projet1_lecture_et_conversion() {
    let Some(p) = essai("fichiers", "Projet1.rvt") else { return };
    let m = lire(&p, &|_| {}).unwrap();
    let mut multicouches: Vec<_> = m.types.iter().filter(|(_, t)| t.couches.as_ref().is_some_and(|c| c.len() > 1)).collect();
    multicouches.sort_by_key(|(id, _)| **id);
    assert!(!multicouches.is_empty(), "aucun type multicouche lu");
    let reference = m.types[&29321].couches.as_ref().unwrap();
    assert_eq!(reference.iter().map(|c| c.materiau).collect::<Vec<_>>(),vec![874,872,873]);
    for (c, epaisseur) in reference.iter().zip([20.,200.,100.]) { assert!((c.epaisseur-epaisseur).abs() < 1e-8); }
    for (_, t) in &multicouches {
        let total: f64 = t.couches.as_ref().unwrap().iter().map(|c| c.epaisseur).sum();
        assert!((total-t.epaisseur.unwrap()).abs() < 1e-6);
    }
    let mut c = modele::Conversion::default();
    c.projet = modele::Projet::nouveau("Test couches");
    c.projet.sources.push(modele::source("s1", &p, "RVT"));
    crate::echanges::depuis_rvt::convertir_maquette(&m, &mut c);
    let detaillees: Vec<_> = c.projet.types_murs.iter().filter(|t| t.couches_detaillees && t.couches.len() > 1).collect();
    assert!(!detaillees.is_empty(), "couches perdues pendant la conversion");
    let relu = modele::Projet::lire_json(&c.projet.json().unwrap()).unwrap();
    for t in detaillees {
        let q = relu.type_mur(&t.id).unwrap();
        assert_eq!(q.couches.len(),t.couches.len());
        for (a,b) in q.couches.iter().zip(&t.couches) { assert_eq!(a.materiau,b.materiau); assert_eq!(a.epaisseur,b.epaisseur); }
    }
    let s = scene(&m, "Test couches", "Projet1.rvt".into(), "2026");
    eprintln!("Murs avec position des couches vérifiée : {}", s.couches_coupe.len());
    assert!(!s.couches_coupe.is_empty(), "aucun mur avec orientation de couches vérifiable");
    let mur = m.murs.iter().find(|w| w.type_id == 29321).unwrap();
    let (&id, _) = s.model.file.entities.iter().find(|(_,e)| e.is("IFCWALL") && e.arg(7).as_str() == Some(&mur.id.to_string())).unwrap();
    let groupe = s.model.property_groups(id).into_iter().find(|g| g.name.starts_with("Composition")).unwrap();
    assert_eq!(groupe.props.len(),3);
    let noms = s.model.material_names(s.model.material_of[&id]);
    assert_eq!(noms.len(),3);
    assert!(noms[0].contains(&m.materiaux[&874].nom));
}

/// Fichier d'essai hors dépôt, dans le dossier du décodage (`essais\` : essais créés
/// par les campagnes ; `fichiers\` : projets et familles de référence ; voir son
/// SOURCES.md) ; dossier remplaçable par la variable d'environnement RVT_ESSAIS.
/// Absent : le test est sauté.
fn essai(dossier: &str, nom: &str) -> Option<PathBuf> {
    let base = std::env::var("RVT_ESSAIS").map(PathBuf::from).unwrap_or_else(|_| PathBuf::from(r"C:\Users\theba\Projets code\Decodage RVT"));
    let p = base.join(dossier).join(nom);
    if p.exists() {
        Some(p)
    } else {
        eprintln!("fichier d'essai absent, test sauté : {}", p.display());
        None
    }
}

fn proche(v: f64, att: f64, tol: f64, quoi: &str) {
    assert!((v - att).abs() <= tol, "{quoi} : {v} au lieu de {att}");
}

/// Essai E04 : mur de 12 345,6 × 200 × 2 718,2 mm, valeurs validées contre Revit 2026.
#[test]
fn e04_volume_et_boite() {
    let Some(p) = essai(r"essais\2026", "E04_mur.rvt") else { return };
    let m = lire(&p, &|_| {}).unwrap();
    assert_eq!(m.murs.len(), 1);
    let mur = &m.murs[0];
    assert_eq!((mur.id, mur.classe.as_str(), mur.niveau, mur.type_id), (2797, "SWall", 1959, 1990));
    assert!((mur.hauteur - 2718.2).abs() < 0.05, "hauteur {}", mur.hauteur);
    let g = mur.geometrie.as_ref().unwrap();
    let c = g.controle();
    assert_eq!((g.sommets.len(), g.faces.len(), c.triangles, c.non_appariees), (8, 6, 12, 0));
    assert!((c.volume_m3 - 6.711562).abs() < 1e-6, "volume {}", c.volume_m3);
    assert!((c.aire_m2 - 73.1411).abs() < 1e-4, "aire {}", c.aire_m2);
    for (v, att) in [(c.min, [0.0, -100.0, 0.0]), (c.max, [12_345.6, 100.0, 2_718.2])] {
        for k in 0..3 {
            assert!((v[k] - att[k]).abs() < 1e-3, "boîte {v:?} au lieu de {att:?}");
        }
    }
    let t = &m.types[&1990];
    assert!((t.epaisseur.unwrap() - 200.0).abs() < 1e-6);
}

/// Projet1.rvt (format 2026) : 59 murs validés contre Revit 2026 (volume, aire et
/// faces) ; sols, escaliers, pièces, instances et matériaux comparés à l'API de
/// Revit 2026 (script RiveTT rvt-reference-elements, 21/10/2026).
#[test]
fn projet1() {
    let Some(p) = essai("fichiers", "Projet1.rvt") else { return };
    let m = lire(&p, &|_| {}).unwrap();
    assert_eq!(m.murs.len(), 59);
    assert_eq!(m.niveaux.len(), 3);
    let (mut vol, mut tris) = (0.0, 0);
    for mur in &m.murs {
        let c = mur.geometrie.as_ref().unwrap_or_else(|e| panic!("mur {} : {e}", mur.id)).controle();
        assert_eq!(c.non_appariees, 0, "mur {} non fermé", mur.id);
        vol += c.volume_m3;
        tris += c.triangles;
    }
    assert!((vol - 257.974221).abs() < 1e-5, "volume total {vol}");
    assert_eq!(tris, 3768);

    // Matériaux : nom, couleur, transparence (Revit : Verre 92-99-177, 70 %).
    let verre = &m.materiaux[&33];
    assert_eq!((verre.nom.as_str(), verre.couleur), ("Verre", [92, 99, 177]));
    proche(verre.transparence * 100.0, 70.0, 1e-4, "transparence");
    assert_eq!(m.materiaux[&31].couleur, [142, 140, 122]);
    assert_eq!(m.materiaux.len(), 18);

    // Sols : type, niveau, décalage, volume du solide.
    let att = [(256950, 1910, -50.0, 1.8), (257366, 1910, 0.0, 1.875), (257376, 1910, 0.0, 2.25), (257386, 2049, 0.0, 1.875), (257396, 2049, 0.0, 2.25), (257406, 2049, 0.0, 60.0)];
    assert_eq!(m.dalles.len(), att.len());
    for (d, (id, niv, dec, v)) in m.dalles.iter().zip(att) {
        assert_eq!((d.id, d.niveau, m.types[&d.type_id].nom.as_str()), (id, niv, "Dalle en béton - 250 mm"));
        proche(d.decalage, dec, 1e-6, "décalage");
        proche(d.geometrie.as_ref().unwrap().controle().volume_m3, v, 1e-5, "volume de dalle");
    }

    // Escaliers : niveaux, contremarches, giron, volées, volume.
    assert_eq!(m.escaliers.len(), 2);
    for (e, (id, bas, haut, volee)) in m.escaliers.iter().zip([(257153, 608, 1910, 257154), (257309, 1910, 2049, 257310)]) {
        assert_eq!((e.id, e.niveau_bas, e.niveau_haut, e.contremarches, e.parties.as_slice()), (id, bas, haut, 16, &[volee][..]));
        assert_eq!(m.types[&e.type_id].nom, "Monobloc");
        proche(e.hauteur_contremarche, 187.5, 1e-6, "contremarche");
        proche(e.giron, 230.0, 1e-6, "giron");
        proche(e.hauteur, 3000.0, 1e-6, "hauteur");
        proche(e.geometrie.as_ref().unwrap().controle().volume_m3, 0.807594, 1e-5, "volume d'escalier");
    }

    // Pièces : 39, nom, numéro et volume (Revit : 22,468454 m³ pour « Local vélos »).
    let pieces: Vec<&Piece> = m.pieces.iter().filter(|p| p.genre == "piece").collect();
    assert_eq!(pieces.len(), 39);
    let velos = pieces.iter().find(|p| p.id == 257034).unwrap();
    assert_eq!((velos.nom.as_str(), velos.numero.as_str()), ("Local vélos", "14"));
    assert!(pieces.iter().all(|p| p.point.is_some()));
    proche(velos.geometrie.as_ref().unwrap().controle().volume_m3, 22.468454, 1e-5, "volume de pièce");

    // Instances : catégories, famille et type, hôte, point, repère.
    let par_cat = |c: &str| m.instances.iter().filter(|i| m.categorie(i).0 == c).count();
    assert_eq!((par_cat("Portes"), par_cat("Fenêtres"), par_cat("Barreaux")), (43, 26, 33));
    let porte = m.instances.iter().find(|i| i.id == 257051).unwrap();
    assert_eq!(m.nom_type_instance(porte), "Int. Simple : PP (0.83m x 2.04m)");
    // cotes du panneau : paramètres de type Largeur et Hauteur (Revit : 830 × 2 040)
    let t = &m.types[&porte.type_id];
    assert_eq!((t.largeur.map(f64::round), t.hauteur.map(f64::round)), (Some(830.0), Some(2040.0)));
    assert_eq!((porte.hote, porte.niveau, porte.repere.as_str()), (257007, 608, "P4"));
    assert!(porte.point.distance(DVec3::new(1500.0, 0.0, 0.0)) < 1e-3, "point {:?}", porte.point);
    let fenetre = m.instances.iter().find(|i| i.id == 257071).unwrap();
    assert_eq!(fenetre.repere, "F1");
    assert!(fenetre.point.distance(DVec3::new(2200.0, 0.0, 1500.0)) < 1e-3, "point {:?}", fenetre.point);

    // Géométrie des familles (Revit, détail fin) : porte 0,14565 m³, fenêtre
    // 0,022169 m³ (ouverture invisible écartée), 102 instances 4,453885 m³.
    let vol = |i: &Instance| i.geometrie.as_ref().unwrap_or_else(|e| panic!("instance {} : {e}", i.id)).controle().volume_m3;
    proche(vol(porte), 0.14565, 1e-5, "volume de porte");
    proche(vol(fenetre), 0.022169, 1e-5, "volume de fenêtre");
    let total: f64 = m.instances.iter().map(vol).sum();
    proche(total, 4.453885, 1e-3, "volume des instances");

    // Garde-corps : 11, volumes identiques à Revit (main courante et barreaux).
    assert_eq!(m.garde_corps.len(), 11);
    let gc = m.garde_corps.iter().find(|g| g.id == 256986).unwrap();
    assert_eq!(m.types[&gc.type_id].nom, "Type Bateau");
    proche(gc.geometrie.as_ref().unwrap().controle().volume_m3, 0.098736, 1e-5, "volume de garde-corps");
    let total: f64 = m.garde_corps.iter().map(|g| g.geometrie.as_ref().unwrap().controle().volume_m3).sum();
    proche(total, 0.821724, 1e-5, "volume des garde-corps");

    let s = scene(&m, "Projet1", "Projet1.rvt".into(), "2026");
    assert!(s.objects.len() > 59);
}

/// Kerreut 2025.rvt (format 2025, corpus de l'agence, hors dépôt) : toits par tracé
/// comparés à l'API de Revit 2026 (fichier mis à niveau à l'ouverture).
#[test]
fn kerreut_2025_toits() {
    let Some(p) = essai("fichiers", "Kerreut 2025.rvt") else { return };
    let m = lire(&p, &|_| {}).unwrap();
    let att = [
        (9793689, 512913, 0.0, 7.483604),
        (9814775, 512913, 0.0, 6.997656),
        (9815700, 512913, -500.0, 1.095285),
        (9899736, 512913, -100.0, 0.329248),
        (9911706, 512914, 0.0, 9.267006),
        (9912297, 608, 2765.69, 7.62112),
        (9955293, 512913, -357.22, 1.918785),
        (10089207, 608, 3100.0, 2.403819),
        (10235462, 512914, 700.88, 3.064929),
        (10235825, 512914, 880.88, 8.257448),
    ];
    assert_eq!(m.toits.len(), att.len());
    for (t, (id, niv, dec, v)) in m.toits.iter().zip(att) {
        assert_eq!((t.id, t.niveau, t.classe.as_str()), (id, niv, "ProfileRoof"));
        proche(t.decalage, dec, 0.01, "décalage de toit");
        proche(t.geometrie.as_ref().unwrap().controle().volume_m3, v, 1e-5, "volume de toit");
    }
    assert_eq!(m.types[&m.toits[0].type_id].nom, "ARC_Toit_Couverture(2)_Chevron(6)");
    // porte « PP (1.03m x 2.04m) » : Largeur 1 030, Hauteur 2 040 dans Revit (absente de
    // la table de types de la famille, présente dans FamilySymbol.m_pParams)
    let porte = m.instances.iter().find(|i| i.id == 9911769).unwrap();
    let t = &m.types[&porte.type_id];
    assert_eq!((t.nom.as_str(), t.largeur.map(f64::round), t.hauteur.map(f64::round)), ("PP (1.03m x 2.04m)", Some(1030.0), Some(2040.0)));
    let e = m.escaliers.iter().find(|e| e.id == 9911889).unwrap();
    assert_eq!((e.niveau_bas, e.niveau_haut, e.contremarches, e.parties.as_slice()), (512913, 512914, 15, &[9911890][..]));
    proche(e.hauteur_contremarche, 180.0, 1e-6, "contremarche");
    proche(e.giron, 240.0, 1e-6, "giron");
    proche(e.hauteur, 2700.0, 1e-6, "hauteur");
    // escalier assemblé : marches dans le type `StairsTriserSymbol` (GInstance)
    proche(e.geometrie.as_ref().unwrap().controle().volume_m3, 0.195419, 1e-5, "volume d'escalier");

    // Familles : 342 instances sur 346 maillées (les 4 autres n'ont pas de solide dans
    // Revit) ; arbre aux surfaces de révolution, 6,566407 m³ dans Revit (−51 % sans
    // points intérieurs).
    assert_eq!(m.instances.iter().filter(|i| i.geometrie.is_ok()).count(), 342);
    let arbre = m.instances.iter().find(|i| i.id == 9692487).unwrap();
    let v = arbre.geometrie.as_ref().unwrap().controle().volume_m3;
    assert!((v - 6.566407).abs() / 6.566407 < 0.01, "volume de l'arbre {v}");
}

/// Essai E12 : trois grilles créées dans Revit 2026 (A droite, B2 oblique, Courbe en
/// arc de rayon 4 000 mm).
#[test]
fn e12_grilles() {
    let Some(p) = essai(r"essais\2026", "E12_grilles.rvt") else { return };
    let m = lire(&p, &|_| {}).unwrap();
    assert_eq!(m.grilles.len(), 3);
    let g = |nom: &str| m.grilles.iter().find(|g| g.nom == nom).unwrap();
    let pt = |x, y| DVec3::new(x, y, 0.0);
    for (nom, id, debut, fin) in [("A", 2797, pt(0.0, 0.0), pt(12_345.6, 0.0)), ("B2", 2798, pt(1000.0, -2000.0), pt(4000.0, 7000.0)), ("Courbe", 2799, pt(9000.0, 3000.0), pt(5000.0, 7000.0))] {
        let x = g(nom);
        assert_eq!(x.id, id);
        assert!(x.debut.distance(debut) < 1e-6 && x.fin.distance(fin) < 1e-6, "{nom} : {:?} → {:?}", x.debut, x.fin);
    }
    let (c, r) = g("Courbe").arc.unwrap();
    assert!(c.distance(pt(5000.0, 3000.0)) < 1e-6 && (r - 4000.0).abs() < 1e-6);
    assert!(g("A").arc.is_none());
}

/// Famille de fenêtre `FEN_2PF.rfa` : formes affichées comme dans Revit 2026 (pleines
/// hors combinaison, combinaisons ; membres et vides écartés), volumes de
/// `get_Geometry` relevés par le script RiveTT `rvt-reference-formes-famille`.
#[test]
fn fenetre_rfa_formes() {
    let Some(p) = essai("fichiers", "FEN_2PF.rfa") else { return };
    let m = lire(&p, &|_| {}).unwrap();
    let ids: BTreeSet<i64> = m.formes.iter().map(|f| f.id).collect();
    let attendus = [9900043, 9900072, 9900099, 9900103, 9900142, 9900181, 9900224, 9900236, 9900237, 9900258, 9900428, 9900429, 9900436];
    assert_eq!(ids, attendus.into_iter().collect());
    let vol = |id: i64| m.formes.iter().find(|f| f.id == id).unwrap().geometrie.as_ref().map(|g| g.controle().volume_m3);
    for (id, v) in [(9900043, 0.0094), (9900099, 0.01264), (9900103, 0.00264), (9900224, 0.006528), (9900236, 0.004373), (9900428, 0.00788), (9900258, 0.00972)] {
        proche(vol(id).unwrap(), v, 2e-6, &format!("volume de la forme {id}"));
    }
    // combinaison entièrement évidée : aucun solide, comme dans Revit
    assert!(vol(9900072).is_err());
    // coffre de volet roulant (face cylindrique concave à 24 facettes) : 0,040615 m³ dans Revit
    assert!((vol(9900142).unwrap() - 0.040615).abs() / 0.040615 < 0.005);
    let total: f64 = m.formes.iter().filter_map(|f| f.geometrie.as_ref().ok()).map(|g| g.controle().volume_m3).sum();
    assert!((total - 0.11791).abs() / 0.11791 < 0.002, "volume total {total}");
    assert_eq!(m.categorie_famille(m.formes[0].famille).1, "IFCWINDOW");
    assert_eq!(m.nom_categorie(9880506), "VR");
    assert_eq!(m.murs.len(), 1);
}

/// Essai E13 : vues et éléments 2D créés dans Revit 2026 (script RiveTT
/// `rvt-essai-e13-vues2d`), valeurs relevées par `rvt-reference-vues-2d`.
#[test]
fn e13_vues_2d() {
    let Some(p) = essai(r"essais\2026\C14", "E13_vues2d.rvt") else { return };
    let m = lire(&p, &|_| {}).unwrap();
    assert_eq!(m.vues.len(), 19);
    let vue = |id: i64| m.vues.iter().find(|v| v.id == id).unwrap();
    let coupe = vue(258483);
    assert_eq!((coupe.nom.as_str(), coupe.genre, coupe.echelle), ("ZZ_COUPE", "Coupe", 100.0));
    assert!(coupe.origine.distance(DVec3::new(3500.0, -2000.0, -150.0)) < 1e-6);
    assert!(coupe.direction.distance(DVec3::X) < 1e-9);
    let (_, _, l) = coupe.cadrage.unwrap();
    assert!((l[0] - 4000.0).abs() < 1e-6 && (l[1] - 3000.0).abs() < 1e-6 && (l[2] - 1000.0).abs() < 1e-6);
    assert_eq!((vue(609).genre, vue(609).niveau), ("Plan d'étage", 608));
    assert_eq!((vue(258471).genre, vue(258471).echelle), ("Vue de dessin", 10.0));
    assert_eq!(vue(258491).genre, "Feuille");
    let el = |id: i64| &m.elements_2d.iter().find(|e| e.id == id).unwrap().contenu;
    let pt = |x, y| DVec3::new(x, y, 0.0);
    match el(258460) {
        Contenu::Trait(p) => assert!(p[0].distance(pt(1000.0, -1000.0)) < 1e-6 && p[1].distance(pt(3456.7, -1000.0)) < 1e-6),
        _ => panic!("ligne"),
    }
    match el(258461) {
        Contenu::Trait(p) => assert!(p[0].distance(pt(4765.4, -2000.0)) < 1e-6 && p.last().unwrap().distance(pt(3234.6, -2000.0)) < 1e-6 && p.len() > 30),
        _ => panic!("arc"),
    }
    match el(258462) {
        Contenu::Texte { texte, point, .. } => assert!(texte == "ZZ_TEXTE_PLAN" && point.distance(pt(1234.5, -2345.6)) < 1e-6),
        _ => panic!("texte"),
    }
    match el(258463) {
        Contenu::Cote { segments, origine, .. } => assert!(segments.len() == 1 && (segments[0].0 - 2456.7).abs() < 1e-6 && origine.distance(pt(1000.0, -1500.0)) < 1e-6),
        _ => panic!("cote"),
    }
    match el(258496) {
        Contenu::FenetreDeVue { vue, echelle, .. } => assert!(*vue == 609 && (*echelle - 0.01).abs() < 1e-12),
        _ => panic!("fenêtre de vue"),
    }
    // région remplie de 2 000 × 1 000 mm : face plane de son GElement
    let r = m.elements_2d.iter().find(|e| e.id == 258466).unwrap();
    let g = r.geometrie.as_ref().unwrap();
    let aire: f64 = g.faces.iter().flat_map(|f| &f.triangles).map(|t| (g.sommets[t[1]] - g.sommets[t[0]]).cross(g.sommets[t[2]] - g.sommets[t[0]]).length() / 2.0).sum();
    proche(aire, 2_000_000.0, 1e-3, "aire de la région");
    let mut classes: Vec<&str> = m.elements_2d.iter().map(|e| e.classe.as_str()).collect();
    classes.sort_unstable();
    classes.dedup_by(|a, b| a == b);
    // les 4 lignes de l'esquisse de la région ne sont pas dans la vue (collecteur de la
    // vue dans l'API, script `rvt-elements-propres-e13`)
    assert_eq!(m.elements_2d.len(), 10, "{classes:?}");
    assert!([258467, 258468, 258469, 258470].iter().all(|id| !m.elements_2d.iter().any(|e| e.id == *id)));
    // scène : un objet tracé par élément 2D visible (sauf la fenêtre de vue), dans le plan
    // de la vue, plus le titre de la fenêtre de vue sur la feuille ; les deux cotes
    // automatiques de l'esquisse, absentes de la vue, ne sont pas tracées
    let s = scene(&m, "E13", "E13_vues2d.rvt".into(), "2026");
    let annot: Vec<_> = s.objects.iter().filter(|o| &*o.class == "IFCANNOTATION").collect();
    assert_eq!(annot.len(), 8, "objets 2D tracés");
    assert!(annot.iter().all(|o| o.opaque.count + o.transparent.count > 0));
}

/// Vues et éléments 2D de `FEN_2PF.rfa` : 8 vues, 26 cotes et 4 textes comme dans
/// Revit (ceux des familles imbriquées écartés), textes lus dans leur RTF.
#[test]
fn fenetre_rfa_vues() {
    let Some(p) = essai("fichiers", "FEN_2PF.rfa") else { return };
    let m = lire(&p, &|_| {}).unwrap();
    assert_eq!(m.vues.len(), 8);
    let cotes: HashMap<i64, Vec<f64>> = m
        .elements_2d
        .iter()
        .filter_map(|e| match &e.contenu {
            Contenu::Cote { segments, .. } => Some((e.id, segments.iter().map(|s| s.0).collect())),
            _ => None,
        })
        .collect();
    assert_eq!(cotes.len(), 26);
    for (id, v) in [(9899425, vec![1200.0]), (9899435, vec![2040.0]), (9899422, vec![600.0, 600.0]), (9900193, vec![1020.0, 1020.0])] {
        assert!(cotes[&id].iter().zip(&v).all(|(a, b)| (a - b).abs() < 1e-6) && cotes[&id].len() == v.len(), "cote {id} : {:?}", cotes[&id]);
    }
    let textes: Vec<&str> = m.elements_2d.iter().filter_map(|e| if let Contenu::Texte { texte, .. } = &e.contenu { Some(texte.as_str()) } else { None }).collect();
    assert_eq!(textes, ["Extérieur", "Intérieur", "Extérieur", "Extérieur"]);
}

/// Hachures de `Kerreut 2025.rvt` : 219 motifs et types de régions identiques à Revit
/// (script RiveTT `rvt-reference-hachures`) ; 93 régions : 20 unies, 70 hachurées et
/// 3 sans type, comme dans l'API ; motifs de coupe et de surface des matériaux
/// (`rvt-reference-motifs-materiaux`).
#[test]
fn kerreut_hachures() {
    let Some(p) = essai("fichiers", "Kerreut 2025.rvt") else { return };
    let m = lire(&p, &|_| {}).unwrap();
    assert_eq!(m.motifs.len(), 219);
    let (mut unies, mut hachurees, mut sans_type, mut traits) = (0, 0, 0, 0);
    for e in &m.elements_2d {
        let Contenu::Region { type_id } = &e.contenu else { continue };
        let Some(t) = m.types_regions.get(type_id) else {
            sans_type += 1;
            continue;
        };
        match m.motifs.get(&t.avant) {
            Some(x) if !x.grilles.is_empty() => {
                let g = e.geometrie.as_ref().unwrap();
                let v = m.vues.iter().find(|v| v.id == e.vue).unwrap();
                let face: Vec<[DVec3; 3]> = g.faces.iter().flat_map(|f| f.triangles.iter().map(|t| t.map(|i| g.sommets[i]))).collect();
                traits += vues::hachures(&face, x, v.echelle, v.droite, v.haut, 20_000).len();
                hachurees += 1;
            }
            _ => unies += 1,
        }
    }
    assert_eq!((unies, hachurees, sans_type), (20, 70, 3));
    assert!(traits > 1000, "{traits} traits");
    // motif 11 « Diagonale haut » : 45°, 3,175 mm (1/8 po) entre les traits, sur le papier
    let d = &m.motifs[&11];
    assert_eq!(d.nom, "Diagonale haut");
    assert!(!d.modele && d.grilles.len() == 1 && (d.grilles[0].angle.to_degrees() - 45.0).abs() < 1e-9 && (d.grilles[0].decalage - 3.175).abs() < 1e-9);
    // ARC_MAT_PIERRE : coupe 3 245 428, surface 963 733 (couleur 3C3C3C)
    let pierre = &m.materiaux[&9241118];
    assert_eq!((pierre.motifs[0].0, pierre.motifs[2].0, pierre.motifs[2].1), (3245428, 963733, [0x3c, 0x3c, 0x3c]));
}

/// Motifs de surface de `Kerreut 2025.rvt` : faces planes calées par leur
/// `FillPatternPlacer` ; sur un mur en « Blocs 200x400 » (motif 819), les joints de
/// la grille à 0° sont horizontaux et espacés de 200 mm.
#[test]
fn kerreut_motifs_surface() {
    let Some(p) = essai("fichiers", "Kerreut 2025.rvt") else { return };
    let m = lire(&p, &|_| {}).unwrap();
    let blocs = &m.motifs[&819];
    assert!(blocs.modele && blocs.grilles.len() == 2);
    let (mut faces, mut horizontaux, mut verticaux, mut autres) = (0, 0, 0, 0);
    let mut hauteurs: Vec<f64> = Vec::new();
    for g in m.murs.iter().filter_map(|x| x.geometrie.as_ref().ok()) {
        for f in g.faces.iter().filter(|f| f.motif.is_some_and(|c| c.motif == 819)) {
            let tris: Vec<[DVec3; 3]> = f.triangles.iter().map(|t| t.map(|i| g.sommets[i])).collect();
            let n = (tris[0][1] - tris[0][0]).cross(tris[0][2] - tris[0][0]).normalize();
            if n.z.abs() > 1e-6 {
                continue; // face non verticale
            }
            faces += 1;
            for [a, b] in vues::motif_sur_face(&tris, &f.motif.unwrap(), blocs, 0.0, 100_000) {
                let d = (b - a).normalize();
                if d.z.abs() < 1e-6 {
                    horizontaux += 1;
                    hauteurs.push(a.z * 1000.0);
                } else if d.z.abs() > 1.0 - 1e-6 {
                    verticaux += 1;
                } else {
                    autres += 1;
                }
            }
        }
    }
    assert!(faces > 0 && horizontaux > 0 && verticaux > 0 && autres == 0, "{faces} faces, {horizontaux} / {verticaux} / {autres}");
    // joints horizontaux : altitudes à un multiple de 200 mm près d'un même décalage
    let r0 = hauteurs[0].rem_euclid(200.0);
    let hors = hauteurs.iter().filter(|h| ((*h - r0).rem_euclid(200.0) + 1e-3) % 200.0 > 2e-3).count();
    assert!(hors * 10 < hauteurs.len(), "{hors} joints hors de la trame sur {}", hauteurs.len());
}

/// Arborescence du projet comme dans Revit : `FEN_2PF.rfa` (plans, plafond, vue 3D,
/// élévations « Élévation 1 », familles) et `Projet1.rvt` (feuilles numérotées avec
/// leurs vues placées, nomenclatures).
#[test]
fn navigateur_projet() {
    if let Some(p) = essai("fichiers", "FEN_2PF.rfa") {
        let m = lire(&p, &|_| {}).unwrap();
        let nav = scene(&m, "FEN_2PF", "FEN_2PF.rfa".into(), "2026").navigateur.unwrap();
        let mut vues: Vec<(String, String, String)> = nav.vues.iter().map(|v| (v.genre.clone(), v.type_vue.clone(), v.nom.clone())).collect();
        vues.sort();
        let attendu = [
            ("Plan d'étage", "Plan d'étage", "Niveau de réf."),
            ("Plan de plafond", "Plan de plafond", "Niveau de réf."),
            ("Vue 3D", "Vue 3D", "Vue 1"),
            ("Élévation", "Elévation 1", "Droite"),
            ("Élévation", "Elévation 1", "Extérieur"),
            ("Élévation", "Elévation 1", "Gauche"),
            ("Élévation", "Elévation 1", "Intérieur"),
        ];
        assert_eq!(vues, attendu.map(|(a, b, c)| (a.to_string(), b.to_string(), c.to_string())));
        assert!(nav.vues.iter().all(|v| v.entite.is_some()));
        assert!(nav.familles.iter().any(|f| f.categorie == "Murs" && f.nom == "Mur de base"));
    }
    let Some(p) = essai("fichiers", "Projet1.rvt") else { return };
    let m = lire(&p, &|_| {}).unwrap();
    let nav = scene(&m, "Projet1", "Projet1.rvt".into(), "2026").navigateur.unwrap();
    let f = nav.vues.iter().find(|v| v.numero == "A101").unwrap();
    assert_eq!((f.genre.as_str(), f.nom.as_str()), ("Feuille", "RDC - Plan et cotations"));
    let placee = nav.vues.iter().find(|v| v.revit_id == f.placees[0]).unwrap();
    assert_eq!(placee.nom, "RDC - Présentation");
    assert_eq!(nav.vues.iter().filter(|v| v.genre == "Nomenclature").count(), 8);
    assert_eq!(nav.vues.iter().filter(|v| v.genre == "Feuille").count(), 5);
    let portes = nav.familles.iter().filter(|f| f.categorie == "Portes").count();
    assert!(portes > 0);
}

/// Essai C14 F04 : la fenêtre chargée dans le projet vide et placée deux fois dans un
/// mur ; volume de chaque instance 0,110441 m³ dans Revit (coffre : −0,35 %).
#[test]
fn c14_fenetres_placees() {
    let Some(p) = essai(r"essais\2026\C14", "F04_deux.rvt") else { return };
    let m = lire(&p, &|_| {}).unwrap();
    assert!(m.formes.is_empty(), "les formes des familles chargées ne sont pas des éléments du projet");
    assert_eq!(m.instances.len(), 2);
    for i in &m.instances {
        let v = i.geometrie.as_ref().unwrap().controle().volume_m3;
        assert!((v - 0.110441).abs() / 0.110441 < 0.002, "instance {} : {v}", i.id);
        assert_eq!(m.categorie(i).1, "IFCWINDOW");
    }
    let i = m.instances.iter().find(|i| i.id == 258460).unwrap();
    assert!(i.point.distance(DVec3::new(2345.6, 0.0, 912.3)) < 1e-6, "{:?}", i.point);
    proche(m.murs[0].geometrie.as_ref().unwrap().controle().volume_m3, 2.968736, 1e-6, "volume du mur percé");
}

/// Toposolides de Saint-Malo : 40 terrains, bordures et revêtements, volumes de
/// `get_Geometry` relevés par le script RiveTT `rvt-reference-toposolides`.
#[test]
fn saint_malo_toposolides() {
    let Some(p) = essai("fichiers", "Saint-Malo_avenue aristide briand_46.rvt") else { return };
    let m = lire(&p, &|_| {}).unwrap();
    assert_eq!(m.toposolides.len(), 40);
    assert!(m.toposolides.iter().all(|t| t.geometrie.is_ok()));
    let t = |id: i64| m.toposolides.iter().find(|t| t.id == id).unwrap();
    let vol = |id: i64| t(id).geometrie.as_ref().unwrap().controle().volume_m3;
    // terrain « Contexte 300 » (1 133 faces) : 44 893,385 424 m³ dans Revit
    assert!((vol(10126070) - 44893.385424).abs() / 44893.385424 < 1e-6, "{}", vol(10126070));
    assert_eq!(t(10126070).geometrie.as_ref().unwrap().faces.len(), 1133);
    for (id, v) in [(10477748, 20.05992), (11226899, 3.011512), (11020967, 121.657792), (11167178, 0.799032), (11013412, 0.025601)] {
        assert!((vol(id) - v).abs() / v < 1e-4, "toposolide {id} : {} au lieu de {v}", vol(id));
    }
    // niveau et décalage (TOPOSOLID_HEIGHTABOVELEVEL_PARAM) : −100 mm sur le niveau 512 913
    assert_eq!((t(11226899).niveau, t(11226899).type_id), (512913, 10049174));
    proche(t(11226899).decalage, -100.0, 1e-6, "décalage");
    assert_eq!(m.types[&10049174].nom, "Terre20");
}

/// Essai E15 : surface topographique de 42 points et sous-région rectangulaire
/// créées dans Revit 2026 (script RiveTT `rvt-essai-e15-surface-topo`) ; points,
/// sommets, triangles, aire et boîte du maillage de `get_Geometry` relevés par
/// `rvt-reference-toposurfaces`.
#[test]
fn e15_surface_topo() {
    let Some(p) = essai(r"essais\2026", "E15_surface_topo.rvt") else { return };
    let m = lire(&p, &|_| {}).unwrap();
    assert_eq!(m.surfaces_topo.len(), 2);
    for (id, hote, points, sommets, triangles, aire, min, max) in [
        (2797, -1, 42, 58, 84, 829.698725, [0.0, 0.0, -3.308721], [120.406824, 84.645669, 6.806726]),
        (2840, 2797, 24, 24, 26, 120.73165, [26.246719, 19.685039, 1.491963], [65.616798, 52.493439, 5.053267]),
    ] {
        let s = m.surfaces_topo.iter().find(|s| s.id == id).unwrap();
        assert_eq!((s.hote, s.points), (hote, points), "{id}");
        let g = s.geometrie.as_ref().unwrap();
        let c = g.controle();
        assert_eq!((g.sommets.len(), c.triangles), (sommets, triangles), "{id}");
        proche(c.aire_m2, aire, 1e-6, "aire");
        for k in 0..3 {
            proche(c.min[k], min[k] * FT, 0.01, "boîte min");
            proche(c.max[k], max[k] * FT, 0.01, "boîte max");
        }
    }
}

/// Feuilles de Kerreut : fenêtres de vue, cartouches et étiquettes, comparés à l'API
/// de Revit 2026 (scripts RiveTT `rvt-reference-feuilles`, `rvt-compare-feuilles`) :
/// `GetBoxCenter`, `GetBoxOutline`, `GetTransform` des cartouches.
#[test]
fn kerreut_feuilles() {
    let Some(p) = essai("fichiers", "Kerreut 2025.rvt") else { return };
    let m = lire(&p, &|_| {}).unwrap();
    let vue = |id: i64| m.vues.iter().find(|v| v.id == id).unwrap();
    let fen = |id: i64| match &m.elements_2d.iter().find(|e| e.id == id).unwrap().contenu {
        Contenu::FenetreDeVue { vue: v, decalage, echelle, .. } => (*v, *decalage, *echelle),
        _ => panic!("{id} n'est pas une fenêtre de vue"),
    };
    // centre et boîte (mm) : décalage + contour du cadrage, élargi de 0,01 pied
    for (id, centre, boite) in [
        (9985483, [-134.509, 231.005], [-266.514, 168.778, -2.504, 293.231]),
        (10258761, [-310.344, 212.743], [-415.665, 138.341, -205.024, 287.146]),
        (10226514, [-354.368, 194.746], [-414.919, 165.809, -293.816, 223.682]),
    ] {
        let (v, d, e) = fen(id);
        let f = feuilles::fenetre(d, e, vue(v));
        let c = f.cadre.unwrap().map(|x| x * 1000.0);
        proche((c[0] + c[2]) / 2.0, centre[0], 0.002, "centre x");
        proche((c[1] + c[3]) / 2.0, centre[1], 0.002, "centre y");
        let marge = 0.01 * FT;
        for (k, b) in boite.iter().enumerate() {
            proche(c[k] + if k < 2 { -marge } else { marge }, *b, 0.002, "boîte");
        }
        // l'origine de la vue tombe sur le décalage
        let o = f.vers_feuille.transform_point3(vue(v).origine / 1000.0) * 1000.0;
        proche(o.x, d.x, 1e-6, "origine x");
        proche(o.y, d.y, 1e-6, "origine y");
    }
    // cartouche réduit de PRO 05 : repère, traits du type, étiquettes remplies
    let i = m.instances_vue.iter().find(|i| i.id == 10168802).unwrap();
    assert_eq!((i.vue, i.type_id), (10168795, 9791176));
    proche(i.origine.x, -1190.793, 0.001, "origine du cartouche");
    assert_eq!(m.dessins[&9791176].traits.len(), 23);
    assert_eq!(m.dessins[&8966185].traits.len(), 7);
    let doc = m.familles[&m.types[&9791176].famille].document.unwrap();
    assert_eq!(m.textes_familles[&doc].iter().filter(|e| matches!(e.contenu, Contenu::Etiquette { .. })).count(), 7);
    assert_eq!(feuilles::police(&m, 9791121).0, "Arial");
    proche(feuilles::police(&m, 9791121).1, 2.6, 1e-9, "taille de police");
    let f = vue(10168795);
    let val = |p: i64| feuilles::valeur_cartouche(&m, f, 10168802, p).unwrap_or_default();
    assert_eq!(val(-1006319), "Nicolas et Isabelle MUSSET");
    assert_eq!(val(-1007401), "PRO 05");
    assert_eq!(val(-1007400), "Plan RDC");
    assert_eq!(val(-1007402), "1 : 50");
    assert_eq!(val(-1007414), "DOSSIER PRE-PRO");
    assert_eq!(val(-1007415), "23/03/2024");
    assert_eq!(val(-1006320), "PCMI");
    // 66 instances sur les feuilles, toutes dessinées
    let feuilles: std::collections::HashSet<i64> = m.vues.iter().filter(|v| v.genre == "Feuille").map(|v| v.id).collect();
    let sur_feuilles: Vec<&InstanceVue> = m.instances_vue.iter().filter(|i| feuilles.contains(&i.vue)).collect();
    assert_eq!(sur_feuilles.len(), 66);
    assert!(sur_feuilles.iter().all(|i| m.dessins.contains_key(&i.type_id)));
}

/// Feuilles de `Projet1.rvt` (sans cartouche) et d'E13 : boîtes des fenêtres de vue
/// comparées à `GetBoxOutline` de Revit 2026 (`rvt-compare-feuilles`) ; titre de la
/// fenêtre de vue d'E13 (« Niveau 0 », « 1 : 100 », numéro de détail).
#[test]
fn projet1_e13_feuilles() {
    let marge = 0.01 * FT;
    if let Some(p) = essai("fichiers", "Projet1.rvt") {
        let m = lire(&p, &|_| {}).unwrap();
        // plans cadrés : boîte exacte ; élévation : centre exact, boîte de Revit élargie
        // de 25,598 mm à gauche et à droite par les niveaux qui sortent du cadrage
        for (id, boite) in [(258041, [-118.048, -78.048, 118.048, 78.048]), (258042, [-118.048, -85.548, 118.048, 85.548]), (258010, [-273.118, -158.048, 273.118, 158.048])] {
            let e = m.elements_2d.iter().find(|e| e.id == id).unwrap();
            let Contenu::FenetreDeVue { vue, decalage, echelle, .. } = &e.contenu else { panic!() };
            let v = m.vues.iter().find(|v| v.id == *vue).unwrap();
            let c = feuilles::fenetre(*decalage, *echelle, v).cadre.unwrap().map(|x| x * 1000.0);
            for (k, b) in boite.iter().enumerate() {
                proche(c[k] + if k < 2 { -marge } else { marge }, *b, 0.002, "boîte");
            }
        }
    }
    let Some(p) = essai(r"essais\2026\C14", "E13_vues2d.rvt") else { return };
    let m = lire(&p, &|_| {}).unwrap();
    let e = m.elements_2d.iter().find(|e| e.id == 258496).unwrap();
    let Contenu::FenetreDeVue { vue, decalage, echelle, titre, longueur_titre, type_id } = &e.contenu else { panic!() };
    let v = m.vues.iter().find(|v| v.id == *vue).unwrap();
    // vue non cadrée : pas de découpe ; l'origine de la vue tombe sur le décalage
    let f = feuilles::fenetre(*decalage, *echelle, v);
    assert!(f.cadre.is_none());
    proche(decalage.x, 370.0, 1e-9, "décalage");
    // titre : m_labelOrigin = coin de la boîte + LabelOffset de l'API (370 ; 252,774)
    proche(titre.x, 370.0, 1e-3, "titre x");
    proche(titre.y, 252.774, 1e-3, "titre y");
    proche(*longueur_titre, 60.0, 1e-6, "ligne de titre");
    assert!(m.types_fenetres[type_id].titre);
    assert_eq!(feuilles::valeur_titre(&m, e, v, -1_005_203).as_deref(), Some("Niveau 0"));
    assert_eq!(feuilles::valeur_titre(&m, e, v, -1_005_204).as_deref(), Some("1 : 100"));
    let feuille = m.vues.iter().find(|x| x.id == 258491).unwrap();
    let titres: Vec<_> = feuilles::objets_vue(&m, feuille).into_iter().filter(|o| o.genre == "Titre de vue").collect();
    assert_eq!(titres.len(), 1);
    assert!(titres[0].parts.len() >= 3, "régions, traits et textes du titre");
}

/// Titres des fenêtres de vue de Kerreut (feuille PCMI3) : « Titre sur la feuille »
/// à la place du nom, numéro de détail masqué par le paramètre « Numero » du type,
/// comme dans l'export de la feuille par Revit.
#[test]
fn kerreut_titres_de_vue() {
    let Some(p) = essai("fichiers", "Kerreut 2025.rvt") else { return };
    let m = lire(&p, &|_| {}).unwrap();
    let e = m.elements_2d.iter().find(|e| e.id == 9985364).unwrap();
    let v = m.vues.iter().find(|v| v.id == 9985266).unwrap();
    assert_eq!((v.nom.as_str(), v.titre_feuille.as_str()), ("PC A", "COUPE AA"));
    assert_eq!(feuilles::valeur_titre(&m, e, v, -1_005_203).as_deref(), Some("COUPE AA"));
    let t = &m.types_fenetres[&9347557];
    assert!(t.titre && !t.ligne && t.famille_titre == 9347548);
    // étiquettes du titre : nom et échelle visibles, numéro de détail masqué
    let doc = m.familles[&m.types[&9347548].famille].document.unwrap();
    let etiquettes: Vec<&Element2D> = m.textes_familles[&doc].iter().filter(|x| matches!(x.contenu, Contenu::Etiquette { .. })).collect();
    assert_eq!(etiquettes.len(), 3);
    let numero = etiquettes.iter().find(|x| x.id == 9347483).unwrap();
    assert_eq!(m.params_embarques[&numero.visible_si], "Numero");
    // cadrage d'annotation actif sur A 50e, inactif sur PC A (paramètre de l'API)
    assert!(m.vues.iter().find(|v| v.id == 10168711).unwrap().cadre.unwrap().annotation_active);
    assert!(!v.cadre.unwrap().annotation_active);
}

/// Cotes de Kerreut comme dans Revit 2026 : visibles seulement si chaque élément coté
/// est dans la vue (collecteur de la vue, script `rvt-visibilite-elements-vue`),
/// lignes d'esquisse écartées, catégories masquées (`GetCategoryHidden`, script
/// `rvt-categories-masquees`), valeurs au format du type (centimètres) ou remplacées.
#[test]
fn kerreut_cotes() {
    let Some(p) = essai("fichiers", "Kerreut 2025.rvt") else { return };
    let m = lire(&p, &|_| {}).unwrap();
    let boites = feuilles::boites(&m);
    let vue = |id: i64| m.vues.iter().find(|v| v.id == id).unwrap();
    let el = |id: i64| m.elements_2d.iter().find(|e| e.id == id).unwrap();
    let visible = |e: i64, v: i64| feuilles::visible_dans_vue(&m, el(e), vue(v), &boites);
    // « RDC 50e » : 10168246 visible ; 10168254 cote un mur hors cadrage
    assert!(visible(10168246, 10168217));
    assert!(!visible(10168254, 10168217));
    // « PC Est » : cotes accrochées à des lignes d'esquisse, absentes de la vue
    assert!(m.lignes_esquisse.contains(&9985379));
    assert!(!m.elements_2d.iter().any(|e| e.id == 9985379));
    assert!(!visible(10085862, 9985368) && !visible(10104460, 9985368));
    // catégories masquées : les 10 de l'API (niveaux compris) sont dans la liste lue
    let api = [-2009645, -2008186, -2008185, -2003400, -2001115, -2001113, -2000535, -2000269, -2000240, -2000200];
    assert!(api.iter().all(|c| vue(9985368).categories_masquees.contains(c)));
    assert_eq!(vue(9985368).masques, vec![9722834, 9916200, 9988805]);
    // type de cote « Cotes_2.25 mm_Noir_M 1 » : centimètres à l'unité, gras
    let t = &m.types_cotes[&9706670];
    assert_eq!(t.format.as_ref().map(|f| (f.0.as_str(), f.1)), Some(("autodesk.unit.unit:centimeters-1.0.1", 1.0)));
    assert!(t.gras && !t.attache_fixe);
    proche(t.decalage_texte, 1.5, 1e-9, "décalage du texte");
    let Contenu::Cote { segments, textes, .. } = &el(10168246).contenu else { panic!() };
    assert_eq!(vues::texte_longueur(segments[0].0, t.format.as_ref()), "321");
    // valeurs remplacées de la coupe « PC A » : « GARAGE », « MAISON »
    let Contenu::Cote { textes: tx, .. } = &el(10020238).contenu else { panic!() };
    assert_eq!(tx.iter().map(|t| t.remplacement.as_str()).collect::<Vec<_>>(), ["GARAGE", "MAISON"]);
    assert!(textes.iter().all(|t| t.remplacement.is_empty()));
}

/// Format des longueurs : unité et précision du type de cote ou du projet.
#[test]
fn format_longueurs() {
    let cm = ("autodesk.unit.unit:centimeters-1.0.1".to_string(), 1.0);
    assert_eq!(vues::texte_longueur(5650.0, Some(&cm)), "565");
    let m = ("autodesk.unit.unit:meters-1.0.0".to_string(), 0.01);
    assert_eq!(vues::texte_longueur(2456.7, Some(&m)), "2.46");
    assert_eq!(vues::texte_longueur(2456.7, None), "2.46");
    let mm = ("autodesk.unit.unit:millimeters-1.0.0".to_string(), 0.5);
    assert_eq!(vues::texte_longueur(12.3, Some(&mm)), "12.5");
}

/// Paramètres de `FEN_2PF.rfa` : valeurs du type courant (`Family.m_familyParams`)
/// contre FamilyManager dans Revit 2026, type courant « 120x204ht 16 » (script RiveTT
/// `rvt-reference-parametres-famille`).
#[test]
fn fenetre_rfa_parametres() {
    let Some(p) = essai("fichiers", "FEN_2PF.rfa") else { return };
    let m = lire(&p, &|_| {}).unwrap();
    assert_eq!(m.famille_document, Some(9_899_388));
    let aff = m.parametres_affiches(9_899_388);
    let val = |nom: &str| aff.iter().find(|a| a.nom == nom).map(|a| (a.groupe, a.valeur.as_str()));
    assert_eq!(val("Distance résa"), Some(("Autre", "110")), "formule if(…, 110 mm, 10 mm), unités de la famille : mm sans symbole");
    assert_eq!(val("Nu pose"), Some(("Cotes", "160")));
    assert_eq!(val("Seuil PMR"), Some(("Cotes", "Oui")));
    assert_eq!(val("Allège supérieure à 12cm (equerre)"), Some(("Autre", "Non")), "formule not(…)");
    assert_eq!(val("Hauteur"), Some(("Cotes", "2040")));
    assert_eq!(val("Largeur"), Some(("Cotes", "1200")));
    assert_eq!(val("Description"), Some(("Données d'identification", "PF + FIXE")));
    assert_eq!(val("Commentaires du type"), Some(("Données d'identification", "M02")));
    let d = &m.definitions[&9_900_475];
    assert_eq!((d.nom.as_str(), d.genre.as_str(), d.occurrence), ("Distance résa", "famille", Some(false)));
    assert_eq!(m.unique_id(9_899_388).as_deref(), Some("79dc52d9-c475-4841-a47e-5c94d4e238e2-00970d7c"));
}

/// Paramètres et IfcGUID de `Kerreut 2025.rvt` contre l'API de Revit 2026 (script
/// RiveTT `rvt-reference-parametres-projet`) : porte coulissante 9911752, dont la
/// largeur est un paramètre d'occurrence de sa famille, son type 8677076 et un
/// paramètre partagé.
#[test]
fn kerreut_parametres() {
    let Some(p) = essai("fichiers", "Kerreut 2025.rvt") else { return };
    let m = lire(&p, &|_| {}).unwrap();
    let val = |id: i64, nom: &str| m.parametres_affiches(id).into_iter().find(|a| a.nom == nom).map(|a| a.valeur);
    assert_eq!(val(9_911_752, "Largeur").as_deref(), Some("1.30"));
    assert_eq!(val(9_911_752, "Identifiant").as_deref(), Some("2275"));
    assert_eq!(val(9_911_752, "Niveau").as_deref(), Some("RDC"));
    assert_eq!(val(9_911_752, "Famille et type").as_deref(), Some("ARC_PTE_Placard coulissante_multi-panneaux: Largeur 1 m mini_largeur panneaux max 90 cm"));
    // le type ne montre pas la largeur, paramètre d'occurrence de sa famille
    assert_eq!(val(8_677_076, "Largeur"), None);
    assert_eq!(val(8_677_076, "Hauteur").as_deref(), Some("2.10"));
    assert_eq!(val(8_677_076, "Identifiant du type").as_deref(), Some("PL03"));
    assert_eq!(val(8_677_076, "Opération").as_deref(), Some("SlidingToRight"));
    assert_eq!(m.guid_ifc(9_911_752).as_deref(), Some("3Sv4yF$LD1uBYRSvD2Wzs7"));
    assert_eq!(m.guid_ifc(8_677_076).as_deref(), Some("2VbQHXdHTBoAiSW3rsPCNH"));
    let d = &m.definitions[&208_407];
    assert_eq!((d.nom.as_str(), d.genre.as_str(), d.guid.as_deref()), ("Adresse du Client", "partagé", Some("f25fdde5-ae9e-422f-87c4-1d9e5fc71836")));
}

/// Coordonnées contre l'API de Revit 2026 (script RiveTT `rvt-reference-coordonnees`,
/// `Decodage RVT/references/coordonnees.json`) : essai E16 (`rvt-essai-e16-coordonnees` :
/// emplacement actif placé, second emplacement, point de base déplacé, lieu), Kerreut
/// (rotation et altitude réelles) et Saint-Malo.
#[test]
fn coordonnees() {
    let pied = |v: f64| v * FT;
    for (fichier, nom, emplacements, position, angle, base, base_partage, lieu) in [
        (
            essai(r"essais\2026", "E16_coordonnees.rvt"),
            "Site par défaut",
            2,
            [1_152_344.379_921_258_4, 22_273_662.549_212_597, 149.862_204_724_409_45],
            0.409_384_429_347_789_9,
            [16.404_199_475_065_617, 22.965_879_265_091_864, 3.280_839_895_013_123_5],
            [1_152_350.287_129_944, 22_273_690.146_934_54, 153.143_044_619_422_57],
            (47.2184, -1.5536, "ZZ_LIEU_NANTES"),
        ),
        (essai("fichiers", "Kerreut 2025.rvt"), "Interne", 1, [0.0, 0.0, 226.049_868_766_404_02], 0.729_962_381_603_939_6, [0.0; 3], [0.0, 0.0, 226.049_868_766_404_02], (48.113_518, -1.669_897, "48.1135177612305,-1.66989731788635")),
        (essai("fichiers", "Saint-Malo_avenue aristide briand_46.rvt"), "Interne", 1, [0.0, 0.0, 25.426_509_186_351_172], -0.031_576_952_957_789_395, [0.0; 3], [0.0, 0.0, 25.426_509_186_351_172], (48.113_518, -1.669_897, "48.1135177612305,-1.66989731788635")),
    ] {
        let Some(p) = fichier else { continue };
        let m = lire(&p, &|_| {}).unwrap();
        let c = &m.coordonnees;
        let e = c.actif().unwrap();
        assert_eq!((e.nom.as_str(), c.nommes().count()), (nom, emplacements), "{}", p.display());
        let o = c.partage(DVec3::ZERO).unwrap();
        for k in 0..3 {
            proche(o[k], pied(position[k]), 1e-3, "origine interne en coordonnées partagées");
            proche(c.point_base.unwrap()[k], pied(base[k]), 1e-6, "point de base");
            proche(c.partage(c.point_base.unwrap()).unwrap()[k], pied(base_partage[k]), 1e-3, "point de base partagé");
        }
        proche(e.angle(), angle, 1e-12, "angle par rapport au nord géographique");
        let l = &c.lieux[&e.lieu];
        proche(l.latitude, lieu.0, 1e-6, "latitude");
        proche(l.longitude, lieu.1, 1e-6, "longitude");
        assert_eq!(l.nom, lieu.2);
    }
}

/// Essai E17 (`rvt-essai-e17-scg-nord`, depuis E16) : système EPSG:2154 et un plan
/// orienté au nord géographique ; valeurs de Revit 2026 (`rvt-reference-scg-nord`).
#[test]
fn e17_systeme_et_nord() {
    let Some(p) = essai(r"essais\2026", "E17_scg_nord.rvt") else { return };
    let m = lire(&p, &|_| {}).unwrap();
    let c = &m.coordonnees;
    let l = &c.lieux[&c.actif().unwrap().lieu];
    assert_eq!((l.systeme.as_str(), l.epsg), ("Lambert93", Some(2154)));
    proche(l.latitude, 0.839_648_926_828_889_7_f64.to_degrees(), 1e-9, "latitude");
    proche(l.longitude, (-0.029_473_898_594_148_437_f64).to_degrees(), 1e-9, "longitude");
    assert_eq!(l.nom, "48.1083397799843,-1.6887299952413");
    let nord = |id: i64| m.vues.iter().find(|v| v.id == id).unwrap().nord_geographique;
    assert!(nord(1979) && !nord(1960) && !nord(1970));
}

/// Phases de Saint-Malo contre l'API de Revit 2026 (`rvt-reference-phases`) : ordre,
/// filtre « Conservé + Projet », élément créé en Existant et démoli en Démolition.
#[test]
fn phases_saint_malo() {
    let Some(p) = essai("fichiers", "Saint-Malo_avenue aristide briand_46.rvt") else { return };
    let m = lire(&p, &|_| {}).unwrap();
    assert_eq!(m.ordre_phases, [3, 354_698, 0]);
    assert_eq!(m.phases[&0], "Projet");
    assert_eq!(m.filtres_phase[&751], ("Conservé + Projet".to_string(), [0, 0, 1, 0, 1, 0, 0]));
    let ph = |k| id_parametre(&m, 10_126_105, k);
    assert_eq!((ph(-1_012_100), ph(-1_012_101)), (Some(3), Some(354_698)));
    let etats: Vec<usize> = m.ordre_phases.iter().map(|&q| scene::etat_de_phase(&m.ordre_phases, 3, 354_698, q)).collect();
    assert_eq!(etats, [4, 3, 1]);
}

/// Plages de vue contre l'API de Revit 2026 (`rvt-reference-plages-vue`) : plan
/// SP/ST_RDC de Kerreut (niveau RDC) et gabarit au niveau associé (−3).
#[test]
fn plages_de_vue_kerreut() {
    let Some(p) = essai("fichiers", "Kerreut 2025.rvt") else { return };
    let m = lire(&p, &|_| {}).unwrap();
    let v = m.vues.iter().find(|v| v.id == 2_519_142).unwrap();
    let attendu = [(608, 7.545_931_758_530_182), (608, 3.937_007_874_015_747_3), (608, 0.0), (608, 0.0)];
    for (lu, (n, d)) in v.plage_vue.unwrap().iter().zip(attendu) {
        assert_eq!(lu.0, n);
        proche(lu.1, d * FT, 1e-6, "décalage");
    }
    assert_eq!(texte_plage(&m, v), "Haut : RDC +2300 mm ; Plan de coupe : RDC +1200 mm ; Bas : RDC +0 mm ; Profondeur de vue : RDC +0 mm");
    let g = m.vues.iter().find(|v| v.id == 6_965_030).unwrap();
    assert!(g.plage_vue.unwrap().iter().all(|x| x.0 == -3));
}

/// Nomenclatures de Projet1 contre le tableau affiché par Revit 2026
/// (`rvt-reference-nomenclatures`) : définition (catégorie, détail, champs, filtre,
/// tri) et contenu de la nomenclature détaillée des portes (43 lignes).
#[test]
fn nomenclatures_projet1() {
    let Some(p) = essai("fichiers", "Projet1.rvt") else { return };
    let m = lire(&p, &|_| {}).unwrap();
    let def = |id: i64| m.vues.iter().find(|v| v.id == id).and_then(|v| v.nomenclature.clone()).unwrap();
    let portes = def(76_991);
    assert_eq!((portes.categorie, portes.detail, portes.champs.len()), (-2_000_023, true, 8));
    let f = &portes.filtres[0];
    assert_eq!((f.champ, f.genre, &f.valeur), (7, 1, &parametres::Brut::Entier(0)));
    assert!(portes.tris[0].entete && portes.tris[0].pied && portes.tris[0].ligne_vide);
    let t = super::super::nomenclatures::calculer(&m, &def(257_959));
    assert_eq!(t.titres, ["Identifiant", "Famille et type", "Niveau", "Largeur", "Hauteur"]);
    assert_eq!(t.lignes.len(), 43);
    assert_eq!(t.lignes[1], ["P4", "Int. Simple: PP (0.83m x 2.04m)", "Niveau 0", "0.83", "2.04"]);
}

/// Liaisons des paramètres de projet et partagés (catégories, occurrence ou type,
/// GUID) et valeurs des paramètres globaux contre l'API de Revit 2026
/// (`rvt-reference-liaisons-parametres`, `references/liaisons_parametres.json`).
#[test]
fn liaisons_et_globaux() {
    let base = std::env::var("RVT_ESSAIS").unwrap_or_else(|_| r"C:\Users\theba\Projets code\Decodage RVT".into());
    let Ok(t) = std::fs::read_to_string(format!(r"{base}\references\liaisons_parametres.json")) else { return };
    let r: serde_json::Value = serde_json::from_str(&t).unwrap();
    for fichier in ["Kerreut 2025.rvt", "Saint-Malo_avenue aristide briand_46.rvt"] {
        let Some(p) = essai("fichiers", fichier) else { continue };
        let m = lire(&p, &|_| {}).unwrap();
        let f = &r[fichier];
        assert_eq!(m.liaisons.len(), f["liaisons"].as_array().unwrap().len(), "{fichier} : liaisons");
        for x in f["liaisons"].as_array().unwrap() {
            let id = x["id"].as_i64().unwrap();
            let (cats, occ) = &m.liaisons[&id];
            let attendues: BTreeSet<i64> = x["categories"].as_array().unwrap().iter().map(|c| c.as_i64().unwrap()).collect();
            assert_eq!(cats.iter().copied().collect::<BTreeSet<i64>>(), attendues, "{fichier} : catégories de {id}");
            assert_eq!(*occ, x["genre"].as_str() == Some("occurrence"), "{fichier} : genre de {id}");
            let d = &m.definitions[&id];
            assert_eq!(d.nom, x["nom"].as_str().unwrap());
            assert_eq!(d.guid.as_deref(), x["guid"].as_str(), "{fichier} : GUID de {id}");
        }
        let globaux = f["globaux"].as_array().unwrap();
        assert_eq!(m.definitions.values().filter(|d| d.genre == "global").count(), globaux.len(), "{fichier} : globaux");
        for x in globaux {
            let d = &m.definitions[&x["id"].as_i64().unwrap()];
            assert_eq!(d.nom, x["nom"].as_str().unwrap());
            let lu = match d.valeur_globale.as_ref() {
                Some(parametres::Brut::Texte(s)) => s.clone(),
                Some(parametres::Brut::Entier(i)) | Some(parametres::Brut::Id(i)) => i.to_string(),
                Some(parametres::Brut::Reel(v)) => format!("{v}"),
                _ => String::new(),
            };
            let attendu = x["valeur"].as_str().unwrap_or("");
            let egal = lu == attendu || attendu.parse::<f64>().ok().zip(lu.parse::<f64>().ok()).is_some_and(|(a, b)| (a - b).abs() < 1e-9 * a.abs().max(1.0));
            assert!(egal, "{fichier} : global {} lu « {lu} », Revit « {attendu} »", d.nom);
        }
    }
}

/// Variantes de `Projet variante.rvt` (fait à la main dans Revit 2026 par Thomas, le
/// 24/10/2026) contre l'API (`rvt-reference-variantes`) : jeu « Variante 1 », deux
/// options (principale « Variante 1 », que Revit nomme « Variante 1  <principal> »,
/// et « Variante 2 »), deux murs dans la principale, trois dans la secondaire ; jeu
/// « test d'une forme ronde » (« Rond de 50 » principale, « Rond de 100 »), deux murs
/// dans chacune.
#[test]
fn variantes_essai() {
    let Some(p) = essai("fichiers", "Projet variante.rvt") else { return };
    let m = lire(&p, &|_| {}).unwrap();
    let jeu = &m.variantes[&257_066];
    assert_eq!((jeu.nom.as_str(), jeu.jeu, jeu.lien), ("Variante 1", true, 257_067));
    assert_eq!((m.variantes[&257_067].nom.as_str(), m.variantes[&257_067].lien), ("Variante 1", 257_066));
    assert_eq!((m.variantes[&257_070].nom.as_str(), m.variantes[&257_070].lien), ("Variante 2", 257_066));
    // second jeu, ajouté à 18 h 13 : « test d'une forme ronde »
    let jeu2 = &m.variantes[&257_717];
    assert_eq!((jeu2.nom.as_str(), jeu2.jeu, jeu2.lien), ("test d'une forme ronde", true, 257_718));
    assert_eq!((m.variantes[&257_718].nom.as_str(), m.variantes[&257_718].lien), ("Rond de 50", 257_717));
    assert_eq!((m.variantes[&257_721].nom.as_str(), m.variantes[&257_721].lien), ("Rond de 100", 257_717));
    for (mur, option, principale) in [
        (257_248, 257_067, true),
        (257_371, 257_067, true),
        (257_450, 257_070, false),
        (257_508, 257_070, false),
        (257_616, 257_070, false),
        (258_274, 257_718, true),
        (258_275, 257_718, true),
        (258_446, 257_721, false),
        (258_447, 257_721, false),
    ] {
        assert_eq!(m.variante_de.get(&mur), Some(&option), "mur {mur}");
        assert_eq!(m.variante(mur).map(|v| v.2), Some(principale), "mur {mur}");
    }
    assert_eq!(m.variante_de.len(), 9);
    assert!(m.groupe_de.is_empty() && m.groupes.is_empty());
    assert_eq!(m.ordre_phases, [3, 0]);
}

/// Groupes, composants de légende, table des notes, cas et natures de charge,
/// gabarits de tableaux électriques, motifs de ligne contre Revit 2026
/// (`rvt-reference-classes-restantes`, `references/classes_restantes.json`).
#[test]
fn objets_annexes() {
    let base = std::env::var("RVT_ESSAIS").unwrap_or_else(|_| r"C:\Users\theba\Projets code\Decodage RVT".into());
    let Ok(t) = std::fs::read_to_string(format!(r"{base}\references\classes_restantes.json")) else { return };
    let r: serde_json::Value = serde_json::from_str(&t).unwrap();
    for fichier in ["Projet vide.rvt", "Kerreut 2025.rvt", "Saint-Malo_avenue aristide briand_46.rvt"] {
        let Some(p) = essai("fichiers", fichier) else { continue };
        let m = lire(&p, &|_| {}).unwrap();
        let f = &r[fichier];
        let ids = |x: &serde_json::Value| -> BTreeSet<i64> { x.as_array().unwrap().iter().map(|v| v.as_i64().unwrap()).collect() };
        // groupes : type, membres (liste du groupe et renvoi de chaque membre), point
        assert_eq!(m.groupes.len(), f["groupes"].as_array().unwrap().len(), "{fichier} : groupes");
        for x in f["groupes"].as_array().unwrap() {
            let id = x["id"].as_i64().unwrap();
            let g = m.groupes.iter().find(|g| g.id == id).unwrap();
            assert_eq!(g.type_id, x["type"].as_i64().unwrap(), "{fichier} groupe {id}");
            assert_eq!(m.types.get(&g.type_id).map(|t| t.nom.as_str()), x["nomType"].as_str(), "{fichier} groupe {id}");
            let attendus = ids(&x["membres"]);
            assert_eq!(g.membres.iter().copied().collect::<BTreeSet<i64>>(), attendus, "{fichier} groupe {id} : membres");
            // renvoi de chaque membre lu vers son groupe
            for e in &attendus {
                if let Some(gr) = m.groupe_de.get(e) {
                    assert_eq!(*gr, id, "{fichier} : groupe du membre {e}");
                }
            }
            let p = DVec3::new(x["point"][0].as_f64().unwrap(), x["point"][1].as_f64().unwrap(), x["point"][2].as_f64().unwrap()) * FT;
            let lu = m.point_groupe(g);
            assert!(lu.distance(p) < 1e-3, "{fichier} groupe {id} : point {lu:?} contre {p:?}");
        }
        // composants de légende placés
        let legendes: Vec<_> = m.legendes.iter().filter(|l| !l.apercu_type).collect();
        assert_eq!(legendes.len(), f["legendes"].as_array().unwrap().len(), "{fichier} : légendes");
        for x in f["legendes"].as_array().unwrap() {
            let l = legendes.iter().find(|l| l.id == x["id"].as_i64().unwrap()).unwrap();
            assert_eq!((l.vue, l.type_id, l.vue_composant), (x["vue"].as_i64().unwrap(), x["composant"].as_i64().unwrap(), x["vueComposant"].as_i64().unwrap()));
            proche(l.longueur, x["longueur"].as_f64().unwrap() * FT, 1e-6, "longueur");
        }
        // table des notes d'identification
        let notes = m.tables_cles.iter().find(|t| t.genre.starts_with("Table des notes")).unwrap();
        assert_eq!(notes.entrees.len(), f["notes"].as_array().unwrap().len(), "{fichier} : notes");
        for x in f["notes"].as_array().unwrap().iter().step_by(37) {
            let e = notes.entrees.iter().find(|e| e.0 == x["cle"].as_str().unwrap()).unwrap();
            assert_eq!((e.1.as_str(), e.2.as_str()), (x["parent"].as_str().unwrap(), x["texte"].as_str().unwrap_or("")));
        }
        // cas et natures de charge, gabarits de tableaux électriques
        let nommes = |genre: &str| m.objets_nommes.iter().filter(|o| o.genre == genre).map(|o| (o.id, o.nom.clone())).collect::<BTreeSet<_>>();
        let attendus = |k: &str| f[k].as_array().unwrap().iter().map(|x| (x["id"].as_i64().unwrap(), x["nom"].as_str().unwrap().to_string())).collect::<BTreeSet<_>>();
        assert_eq!(nommes("Cas de charge"), attendus("cas"), "{fichier} : cas de charge");
        assert_eq!(nommes("Nature de charge"), attendus("natures"), "{fichier} : natures");
        assert_eq!(nommes("Gabarit de tableau électrique"), attendus("tableaux"), "{fichier} : tableaux");
        for x in f["cas"].as_array().unwrap() {
            let o = m.objets_nommes.iter().find(|o| o.id == x["id"].as_i64().unwrap()).unwrap();
            assert_eq!(o.props[0].1, x["numero"].as_i64().unwrap().to_string());
            assert_eq!(o.props[1].1, x["nature"].as_i64().unwrap().to_string());
        }
        // profilés de mur : maillés comme des formes
        let profiles: Vec<_> = m.formes.iter().filter(|x| x.classe == "WallSweep").collect();
        let mailles = profiles.iter().filter(|x| x.geometrie.is_ok()).count();
        eprintln!("{fichier} : profilés de mur {mailles} maillés sur {}", profiles.len());
        assert_eq!(mailles, profiles.len(), "{fichier} : profilés de mur");
        // motifs de ligne : nom et segments
        assert_eq!(m.motifs_ligne.len(), f["motifs"].as_array().unwrap().len(), "{fichier} : motifs de ligne");
        for x in f["motifs"].as_array().unwrap() {
            let mo = &m.motifs_ligne[&x["id"].as_i64().unwrap()];
            assert_eq!(mo.nom, x["nom"].as_str().unwrap());
            let segs = x["segments"].as_array().unwrap();
            assert_eq!(mo.segments.len(), segs.len(), "{}", mo.nom);
            for ((g, l), s) in mo.segments.iter().zip(segs) {
                let genre = match s[0].as_str().unwrap() {
                    "Dash" => 0,
                    "Space" => 1,
                    "Dot" => 2,
                    _ => -1,
                };
                assert_eq!(*g, genre, "{} : genre", mo.nom);
                proche(*l, s[1].as_f64().unwrap() * FT, 1e-6, "segment");
            }
        }
    }
}

/// Images, imports CAO et liens Revit contre Revit 2026 (`rvt-reference-images-liens`,
/// `references/images_liens.json`) ; les données de chaque image sont un fichier
/// JPEG ou PNG dont l'en-tête donne la taille en pixels de Revit.
#[test]
fn images_et_liens() {
    let base = std::env::var("RVT_ESSAIS").unwrap_or_else(|_| r"C:\Users\theba\Projets code\Decodage RVT".into());
    let Ok(t) = std::fs::read_to_string(format!(r"{base}\references\images_liens.json")) else { return };
    let r: serde_json::Value = serde_json::from_str(&t).unwrap();
    let v3 = |x: &serde_json::Value| DVec3::new(x[0].as_f64().unwrap(), x[1].as_f64().unwrap(), x[2].as_f64().unwrap());
    // taille en pixels d'après l'en-tête du fichier image
    let pixels = |d: &[u8]| -> Option<(u32, u32)> {
        if d.starts_with(b"\x89PNG") {
            return Some((u32::from_be_bytes(d[16..20].try_into().ok()?), u32::from_be_bytes(d[20..24].try_into().ok()?)));
        }
        let mut i = 2;
        while i + 9 < d.len() {
            let (m, l) = (d[i + 1], u16::from_be_bytes([d[i + 2], d[i + 3]]) as usize);
            if (0xC0..=0xCF).contains(&m) && m != 0xC4 && m != 0xC8 && m != 0xCC {
                return Some((u16::from_be_bytes([d[i + 7], d[i + 8]]) as u32, u16::from_be_bytes([d[i + 5], d[i + 6]]) as u32));
            }
            i += 2 + l;
        }
        None
    };
    for fichier in ["Kerreut 2025.rvt", "Saint-Malo_avenue aristide briand_46.rvt"] {
        let Some(p) = essai("fichiers", fichier) else { continue };
        let m = lire(&p, &|_| {}).unwrap();
        let f = &r[fichier];
        let projet: Vec<&super::super::medias::TypeImage> = m.types_images.iter().filter(|t| t.famille == -1).collect();
        let attendus = f["types_compte"].as_u64().unwrap_or(f["types"].as_array().unwrap().len() as u64) as usize;
        assert_eq!(projet.len(), attendus, "{fichier} : types d'image du projet");
        for x in f["types"].as_array().unwrap() {
            let id = x[0].as_i64().unwrap();
            let t = projet.iter().find(|t| t.id == id).unwrap_or_else(|| panic!("type {id}"));
            assert_eq!((t.nom.as_str(), t.largeur_px, t.hauteur_px, t.chemin.as_deref()), (x[1].as_str().unwrap(), x[2].as_i64().unwrap(), x[3].as_i64().unwrap(), x[4].as_str()), "{fichier} {id}");
            let d = &m.rasters[&t.donnees];
            assert_eq!(pixels(&d.donnees), Some((t.largeur_px as u32, t.hauteur_px as u32)), "{fichier} {id} : en-tête {}", d.format);
        }
        assert_eq!(m.images.len(), f["images"].as_array().unwrap().len(), "{fichier} : images placées");
        for x in f["images"].as_array().unwrap() {
            let id = x[0].as_i64().unwrap();
            let i = m.images.iter().find(|i| i.id == id).unwrap();
            let t = m.types_images.iter().find(|t| t.id == i.type_id).unwrap();
            assert_eq!((i.type_id, i.vue), (x[1].as_i64().unwrap(), x[2].as_i64().unwrap()));
            proche(t.largeur * i.echelle.0, x[3].as_f64().unwrap() * FT, 1e-3, "largeur");
            proche(t.hauteur * i.echelle.1, x[4].as_f64().unwrap() * FT, 1e-3, "hauteur");
            assert!(i.centre.distance(v3(&x[5]) * FT) < 1e-3, "{fichier} {id} : centre {:?}", i.centre / FT);
        }
        assert_eq!(m.imports.len(), f["imports"].as_array().unwrap().len(), "{fichier} : imports");
        for x in f["imports"].as_array().unwrap() {
            let id = x[0].as_i64().unwrap();
            let i = m.imports.iter().find(|i| i.id == id).unwrap();
            let t = m.types_imports.iter().find(|t| t.id == i.type_id).unwrap();
            assert_eq!((i.type_id, t.nom.as_str(), t.lie, i.vue), (x[1].as_i64().unwrap(), x[2].as_str().unwrap(), x[3].as_bool().unwrap(), x[4].as_i64().unwrap()), "{fichier} {id}");
            assert!(i.origine.distance(v3(&x[5]) * FT) < 1e-3 && i.axes.x_axis.distance(v3(&x[6])) < 1e-6, "{fichier} {id} : repère");
            if let Some(c) = x[7].as_str() {
                assert_eq!(t.chemin, c);
            }
        }
        for x in f["liens"].as_array().unwrap() {
            let i = m.liens.iter().find(|i| i.id == x[0].as_i64().unwrap()).unwrap();
            let t = m.types_liens.iter().find(|t| t.id == i.type_id).unwrap();
            assert_eq!((i.type_id, t.nom.as_str(), t.chemin.as_str(), t.relatif), (x[2].as_i64().unwrap(), x[3].as_str().unwrap(), x[4].as_str().unwrap(), x[5].as_str() == Some("Relative")));
            assert!(x[1].as_str().unwrap().starts_with(&format!("{} : {} : ", t.nom, i.nom)), "nom du lien");
            assert!(i.origine.distance(v3(&x[6]) * FT) < 1e-3 && i.axes.x_axis.distance(v3(&x[7])) < 1e-6);
        }
    }
}

/// Plans de référence, plans de construction, apparences de rendu et apparence des
/// matériaux contre Revit 2026 (`rvt-reference-plans-apparences`,
/// `references/plans_apparences.json`).
#[test]
fn plans_et_apparences() {
    let base = std::env::var("RVT_ESSAIS").unwrap_or_else(|_| r"C:\Users\theba\Projets code\Decodage RVT".into());
    let Ok(t) = std::fs::read_to_string(format!(r"{base}\references\plans_apparences.json")) else { return };
    let r: serde_json::Value = serde_json::from_str(&t).unwrap();
    let v3 = |x: &serde_json::Value| DVec3::new(x[0].as_f64().unwrap(), x[1].as_f64().unwrap(), x[2].as_f64().unwrap());
    for fichier in ["FEN_2PF.rfa", "Projet1.rvt", "Kerreut 2025.rvt"] {
        let Some(p) = essai("fichiers", fichier) else { continue };
        let m = lire(&p, &|_| {}).unwrap();
        let f = &r[fichier];
        let (mut n, mut faux) = (0, Vec::new());
        for x in f["plans"].as_array().unwrap() {
            n += 1;
            let id = x["id"].as_i64().unwrap();
            let ok = m.plans_reference.iter().find(|p| p.id == id).is_some_and(|p| {
                p.nom == x["nom"].as_str().unwrap()
                    && p.bulle.distance(v3(&x["bulle"]) * FT) < 1e-3
                    && p.libre.distance(v3(&x["libre"]) * FT) < 1e-3
                    && p.normale.distance(v3(&x["normale"])) < 1e-6
                    && p.reference == x["reference"].as_i64().unwrap()
                    && p.definit_origine == (x["origine"].as_i64() == Some(1))
            });
            if !ok {
                faux.push(id);
            }
        }
        for x in f["esquisses"].as_array().unwrap().iter().filter(|x| !x["origine"].is_null()) {
            n += 1;
            let id = x["id"].as_i64().unwrap();
            let lu = m.plans_construction.iter().find(|p| p.id == id);
            let ok = lu.is_some_and(|p| {
                m.nom_plan_construction(p) == x["nom"].as_str().unwrap() && p.origine.distance(v3(&x["origine"]) * FT) < 1e-3 && p.axes.z_axis.distance(v3(&x["normale"])) < 1e-6
            });
            if !ok {
                faux.push(id);
                if let Some(p) = lu {
                    eprintln!("   esquisse {id} : lu « {} » support {} {:?} {:?} ; Revit « {} » {} {}", m.nom_plan_construction(p), p.support, p.origine / FT, p.axes.z_axis, x["nom"], x["origine"], x["normale"]);
                } else {
                    eprintln!("   esquisse {id} absente");
                }
            }
        }
        for x in f["apparences"].as_array().unwrap() {
            n += 1;
            let id = x["id"].as_i64().unwrap();
            let diffuse: Option<Vec<f64>> = x["diffuse"].as_array().map(|a| a.iter().map(|c| c.as_f64().unwrap()).collect());
            let ok = m.apparences.get(&id).is_some_and(|a| {
                a.nom == x["nom"].as_str().unwrap()
                    && match (&a.diffuse, &diffuse) {
                        (Some(c), Some(d)) => c.iter().zip(d).all(|(u, v)| (u - v).abs() < 1e-6),
                        (None, None) => true,
                        _ => false,
                    }
                    && a.texture.as_deref() == x["texture"].as_str()
            });
            if !ok {
                faux.push(id);
            }
        }
        for x in f["materiaux"].as_array().unwrap() {
            n += 1;
            let id = x["id"].as_i64().unwrap();
            if m.materiaux.get(&id).map(|k| k.apparence) != x["apparence"].as_i64() {
                faux.push(id);
            }
        }
        eprintln!("{fichier} : {} justes sur {n}, faux {:?}", n - faux.len(), &faux[..faux.len().min(12)]);
        assert!(faux.is_empty(), "{fichier} : {} faux", faux.len());
    }
}

/// Flux globaux lus en entier (`Global/Latest`, `PartitionTable`,
/// `DocumentIncrementTable`, `Contents`, `ContentDocuments`) et leurs informations,
/// contre la lecture Python (`Decodage RVT/brouillons/flux/verifier_infos.py`).
#[test]
fn flux_globaux() {
    for (fichier, documents) in [("Projet1.rvt", 101), ("Kerreut 2025.rvt", 262), ("FEN_2PF.rfa", 1)] {
        let Some(p) = essai("fichiers", fichier) else { continue };
        let d = decodage(&p).unwrap();
        assert!(d.non_lus.is_empty() && d.flux.0 == d.flux.1, "{fichier} : {:?}", d.non_lus);
        let i = infos_document(&p).unwrap();
        assert_eq!(i.documents.len(), documents, "{fichier}");
        if fichier == "Projet1.rvt" {
            assert_eq!(i.logiciel, "20260731_1210(x64)");
            assert_eq!(i.sous_projets, ["Workset1"]);
            assert_eq!(i.enregistrements.len(), 8);
            assert_eq!(i.enregistrements[0], ("thebault3LE8A".to_string(), 37_174));
            assert_eq!(i.versions.last().map(String::as_str), Some("Revit 2026 2026 (2026.000) : 20260731_1210(x64)"));
        }
    }
}

/// Surface et périmètre des zones de surface contre `Area.Area` et `Area.Perimeter`
/// de Revit 2026 (`rvt-reference-zones-surface`, `references/zones_surface.json`).
#[test]
fn zones_surface() {
    let base = std::env::var("RVT_ESSAIS").unwrap_or_else(|_| r"C:\Users\theba\Projets code\Decodage RVT".into());
    let Ok(t) = std::fs::read_to_string(format!(r"{base}\references\zones_surface.json")) else { return };
    let r: serde_json::Value = serde_json::from_str(&t).unwrap();
    // pièces (et zones) : surface du circuit contre `Room.Area`
    // (`rvt-reference-pieces-unites`)
    let pu: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(format!(r"{base}\references\pieces_unites.json")).unwrap()).unwrap();
    for (dossier, fichier, minimum) in [("fichiers", "Projet1.rvt", 39), ("fichiers", "Kerreut 2025.rvt", 35), ("fichiers", "Saint-Malo_avenue aristide briand_46.rvt", 164), (r"essais\2026", "E10_piece.rvt", 1)] {
        let Some(p) = essai(dossier, fichier) else { continue };
        let m = lire(&p, &|_| {}).unwrap();
        let val = |id: i64| m.parametres.get(&id).and_then(|v| v.iter().find(|(q, _)| *q == -1_006_902)).and_then(|(_, b)| if let parametres::Brut::Reel(x) = b { Some(*x) } else { None });
        let l: Vec<&serde_json::Value> = pu[fichier]["pieces"].as_array().unwrap().iter().filter(|x| x["surface"].as_f64().unwrap_or(0.0) > 0.0).collect();
        let justes = l.iter().filter(|x| val(x["id"].as_i64().unwrap()).is_some_and(|a| (a - x["surface"].as_f64().unwrap()).abs() < 1e-9 * a.max(1.0))).count();
        eprintln!("{fichier} : surfaces des pièces {justes} sur {}", l.len());
        assert!(justes >= minimum, "{fichier} : {justes} sur {}", l.len());
    }
    for (fichier, surfaces, perimetres) in [("Kerreut 2025.rvt", 16, 15), ("Saint-Malo_avenue aristide briand_46.rvt", 27, 20)] {
        let Some(p) = essai("fichiers", fichier) else { continue };
        let m = lire(&p, &|_| {}).unwrap();
        let val = |id: i64, p: i64| m.parametres.get(&id).and_then(|v| v.iter().find(|(q, _)| *q == p)).and_then(|(_, b)| if let parametres::Brut::Reel(x) = b { Some(*x) } else { None });
        let (mut s_justes, mut p_justes, mut n) = (0, 0, 0);
        for z in r[fichier]["zones"].as_array().unwrap().iter().filter(|z| z["place"].as_bool() == Some(true)) {
            n += 1;
            let id = z["id"].as_i64().unwrap();
            let (s, pe) = (z["surface"].as_f64().unwrap(), z["perimetre"].as_f64().unwrap());
            s_justes += val(id, -1_006_902).is_some_and(|a| (a - s).abs() < 1e-9 * s.max(1.0)) as usize;
            p_justes += val(id, -1_006_917).is_some_and(|b| (b - pe).abs() < 1e-6 * pe.max(1.0)) as usize;
        }
        eprintln!("{fichier} : surfaces {s_justes} sur {n}, périmètres {p_justes} sur {n}");
        assert!(s_justes >= surfaces && p_justes >= perimetres, "{fichier} : surfaces {s_justes}, périmètres {p_justes} sur {n}");
    }
}

/// Élévation des niveaux selon la base de leur type, contre Revit 2026
/// (`rvt-essai-e19-base-niveaux`, depuis E16 : point de base relevé de 1 m, niveau
/// d'un type mesuré depuis le point topographique).
#[test]
fn elevations_niveaux_e19() {
    let Some(p) = essai(r"essais\2026", "E19_base_niveaux.rvt") else { return };
    let m = lire(&p, &|_| {}).unwrap();
    let elev = |id: i64| match m.parametres[&id].iter().find(|(p, _)| *p == -1_007_102).map(|(_, b)| b.clone()) {
        Some(parametres::Brut::Reel(x)) => x,
        b => panic!("{b:?}"),
    };
    proche(elev(1959), -3.280_839_895_013_123_5, 1e-9, "base : point de base du projet");
    proche(elev(2805), 159.704_724_409_448_83, 1e-6, "base : point topographique");
}

/// Pièces de départ et d'arrivée des portes et fenêtres contre `FromRoom`/`ToRoom`
/// de Revit 2026 dans la dernière phase (`rvt-reference-pieces-portes`,
/// `Decodage RVT/references/pieces_portes.json`).
#[test]
fn pieces_portes() {
    let base = std::env::var("RVT_ESSAIS").unwrap_or_else(|_| r"C:\Users\theba\Projets code\Decodage RVT".into());
    let Ok(t) = std::fs::read_to_string(format!(r"{base}\references\pieces_portes.json")) else { return };
    let r: serde_json::Value = serde_json::from_str(&t).unwrap();
    for (fichier, minimum) in [("Projet1.rvt", 67), ("Kerreut 2025.rvt", 37), ("Saint-Malo_avenue aristide briand_46.rvt", 515)] {
        let Some(p) = essai("fichiers", fichier) else { continue };
        let m = lire(&p, &|_| {}).unwrap();
        let l = r[fichier]["instances"].as_array().unwrap();
        let justes = l.iter().filter(|x| m.pieces_de_porte(x["id"].as_i64().unwrap()) == (x["de"].as_i64().unwrap(), x["a"].as_i64().unwrap())).count();
        eprintln!("{fichier} : {justes} sur {}", l.len());
        assert!(justes >= minimum, "{fichier} : {justes} sur {}", l.len());
    }
}

/// Essai E18 (`rvt-essai-e18-formules`) : une formule par fonction, évaluée avec les
/// valeurs du type courant, contre la valeur que donne FamilyManager dans Revit 2026
/// (`Decodage RVT/references/formules_e18.json`).
#[test]
fn formules_e18() {
    use super::super::formules::{de_brut, evaluer, Expr, Valeur};
    use super::super::parametres::Stockage;
    let Some(p) = essai(r"essais\2026", "E18_formules.rfa") else { return };
    let m = lire(&p, &|_| {}).unwrap();
    let (fid, f) = m.familles.iter().find(|(_, f)| !f.formules.is_empty()).unwrap();
    let vals = &m.parametres[fid];
    let st = |p: i64| m.definitions.get(&p).map(|d| d.stockage).unwrap_or(Stockage::Aucun);
    let v = |p: i64| vals.iter().find(|(q, _)| *q == p).map(|(_, b)| de_brut(b, st(p)));
    let var = |e: &Expr| match e {
        Expr::Param(q) => v(*q),
        _ => None,
    };
    let calcul = |id: i64| f.formules.iter().find(|(p, _)| *p == id).and_then(|(_, ex)| evaluer(ex, &var));
    let revit = [
        (2942, 0.5), (2943, 0.866_025_403_784_438_7), (2944, 0.577_350_269_189_625_7), (2945, 0.523_598_775_598_298_9), (2946, 1.047_197_551_196_597_9),
        (2947, 0.785_398_163_397_448_3), (2948, 9.974_182_454_814_718), (2949, 0.361_727_836_017_592_84), (2951, 1.516_575_088_810_31), (2952, 2.7),
        (2953, std::f64::consts::PI), (2954, 2.0), (2955, 3.0), (2956, 2.0), (2957, -3.0), (2958, -2.0), (2959, -3.0), (2960, 3.0), (2961, 5.29),
        (2962, -2.3), (2964, -0.4), (2965, 1.15), (2966, 1.0), (2967, 0.0), (2968, 1.0), (2969, 0.0), (2970, 1.0), (2971, 1.0), (2976, 20.0),
    ];
    for (id, attendu) in revit {
        let x = calcul(id).and_then(|x| x.nombre()).unwrap_or(f64::NAN);
        proche(x, attendu, 1e-9, &format!("formule {id}"));
    }
    assert_eq!(calcul(2975), Some(Valeur::Texte("grand".into())));
    assert_eq!(f.formules.len(), 30);
}

/// Champ pourcentage et champ calculé de Saint-Malo contre le tableau de Revit 2026
/// (`rvt-reference-nomenclatures`) : « Part de SP » = « % de SHAB » × 5 000 m², le
/// pourcentage étant la part de la surface de la pièce dans le total général.
#[test]
fn nomenclature_formule_saint_malo() {
    let Some(p) = essai("fichiers", "Saint-Malo_avenue aristide briand_46.rvt") else { return };
    let m = lire(&p, &|_| {}).unwrap();
    let d = m.vues.iter().find(|v| v.id == 9_190_140).and_then(|v| v.nomenclature.clone()).unwrap();
    let t = super::super::nomenclatures::calculer(&m, &d);
    let ligne = |n: &str| t.lignes.iter().find(|l| l[0] == n).cloned().unwrap();
    assert_eq!(ligne("A110"), ["A110", "T1", "", "20.5 m²", "1%", "42.3 m²"]);
    assert_eq!(ligne("A119"), ["A119", "T1", "", "33.4 m²", "1%", "68.9 m²"]);
    assert_eq!(&ligne("Total général")[4], "100%");
}

/// Formules des familles évaluées avec les valeurs de chaque type, contre la valeur
/// calculée que Revit a enregistrée dans le type (une formule entière est arrondie).
#[test]
fn formules_familles() {
    use super::super::formules::{de_brut, evaluer, Expr};
    use super::super::parametres::Stockage;
    for (fichier, minimum) in [("Kerreut 2025.rvt", 161), ("Saint-Malo_avenue aristide briand_46.rvt", 411)] {
        let Some(p) = essai("fichiers", fichier) else { continue };
        let m = lire(&p, &|_| {}).unwrap();
        let st = |p: i64| m.definitions.get(&p).map(|d| d.stockage).or_else(|| parametres::integres().get(&p).map(|i| i.stockage)).unwrap_or(Stockage::Aucun);
        let (mut justes, mut sans, mut faux) = (0, 0, Vec::new());
        for (t, ty) in &m.types {
            let (Some(f), Some(vals)) = (m.familles.get(&ty.famille), m.parametres.get(t)) else { continue };
            let v = |p: i64| vals.iter().find(|(q, _)| *q == p).map(|(_, b)| de_brut(b, st(p)));
            for (p, ex) in &f.formules {
                let Some(attendu) = v(*p).and_then(|x| x.nombre()) else { continue };
                let var = |e: &Expr| match e {
                    Expr::Param(q) => v(*q),
                    _ => None,
                };
                match evaluer(ex, &var).and_then(|x| x.nombre()) {
                    Some(x) => {
                        let x = if st(*p) == Stockage::Entier { x.round() } else { x };
                        if (x - attendu).abs() <= 1e-6 * attendu.abs().max(1.0) {
                            justes += 1
                        } else {
                            faux.push((*t, *p, x, attendu))
                        }
                    }
                    None => sans += 1,
                }
            }
        }
        eprintln!("{fichier} : {justes} justes, {sans} sans valeur");
        assert!(faux.is_empty() && justes >= minimum, "{fichier} : {justes} justes, faux {faux:?}");
    }
}
