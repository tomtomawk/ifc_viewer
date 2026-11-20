//! Tests déplacés de `modele/documentation/tests.rs` : ils appellent du code situé au-dessus
//! de la crate `modele` (commandes, scène, formats, échanges, collaboration ou dessin).
use commandes::Execution;
use moteur2d::{pdf_jeu, svg_feuille, vue_placee};
use modele::documentation::*;
use noyau::trace2d::{Forme, Primitive, Role};

#[test]
fn dessin_sur_feuille_suit_le_fond_la_plage_haute_et_le_style() {
    use modele::creation::ParametresMur;
    let mut p = Projet::nouveau("Réglages du plan");
    let bas = p.ajouter_mur([0.0, 0.0], [4000.0, 0.0], &ParametresMur::default()).unwrap();
    let niveau = p.niveaux[0].id.clone();
    let haut = p.ajouter_mur([0.0, 1500.0], [4000.0, 1500.0], &ParametresMur { decalage: 1700.0, hauteur: 600.0, ..Default::default() }).unwrap();
    let id = p.ajouter_vue_plan(&niveau, 1200.0).unwrap();
    let fond = p.dupliquer_vue(&id).unwrap();
    let i = p.vues.iter().position(|v| v.id == id).unwrap();
    p.vues[i].fond_plan = Some(fond);
    let placee = VuePlacee { vue: id.clone(), position: [50.0, 80.0], echelle: 100.0 };
    let haut = modele::identifiants::id_natif("mur", &haut);
    let bas = modele::identifiants::id_natif("mur", &bas);
    let (dessin, _) = vue_placee(&p, &placee).unwrap();
    let couleur = |dessin: &[Primitive]| dessin.iter().find(|pr| pr.element == Some(bas) && pr.role == Role::Poche && pr.style.couleur != [165, 165, 165, 255]).unwrap().style.couleur;
    let poche_graphique = couleur(&dessin);
    assert!(dessin.iter().any(|pr| pr.style.couleur == [165, 165, 165, 255]));
    assert!(dessin.iter().any(|pr| pr.element == Some(haut) && pr.role == Role::Vu));

    p.vues[i].fond_plan = None;
    p.vues[i].limite_haute_decalage = 1500.0;
    p.rendus_vues.insert(id, modele::soleil::RenduVue { style: Some("realiste".into()), ..Default::default() });
    let (dessin, _) = vue_placee(&p, &placee).unwrap();
    assert!(!dessin.iter().any(|pr| pr.style.couleur == [165, 165, 165, 255] || pr.element == Some(haut)));
    assert_ne!(couleur(&dessin), poche_graphique);
}

#[test]
fn plan_prolonge_en_pointilles_un_escalier_qui_traverse_la_coupe() {
    let mut p = Projet::nouveau("Escalier en plan");
    p.niveaux.push(modele::Niveau {
        id: "rdc".into(), nom: "RDC".into(), altitude: 0.0, origine: None,
    });
    p.escaliers.push(modele::Escalier {
        id: "e".into(), nom: "Escalier".into(), niveau: "rdc".into(), niveau_haut: None,
        decalage_bas: 0.0, decalage_haut: 0.0, hauteur: 2800.0,
        trace: vec![[0.0, 0.0], [6000.0, 0.0]], largeur: 1000.0,
        hauteur_marche: 175.0, paillasse: 150.0, sous_face: Default::default(), balance: None,
    });
    let id = p.ajouter_vue_plan("rdc", 1000.0).unwrap();
    let escalier = modele::identifiants::id_natif("escalier", "e");
    let voir = |p: &Projet| {
        let dessin = moteur2d::dessin_vue(p, &id).unwrap();
        let mut pointilles = Vec::new();
        let mut trait_bas = false;
        for pr in dessin.primitives.into_iter().filter(|pr| pr.element == Some(escalier) && pr.role == Role::Vu) {
            let Forme::Segments(segments) = pr.forme else { continue };
            for s in segments {
                if s.iter().all(|q| (q.z - 0.9995).abs() < 1e-6) {
                    assert!(s[0].distance(s[1]) <= 0.003 * dessin.echelle + 1e-6);
                    pointilles.push(s);
                } else if s.iter().any(|q| q.z < 0.99) {
                    trait_bas = true;
                }
            }
        }
        (pointilles, trait_bas)
    };
    p.vues[0].limite_haute_decalage = 2200.0;
    let (hauts, trait_bas) = voir(&p);
    assert!(trait_bas && !hauts.is_empty());
    let bout = |s: &[[glam::DVec3; 2]]| s.iter().flatten().map(|q| q.x).fold(f64::NEG_INFINITY, f64::max);
    p.vues[0].limite_haute_decalage = 1600.0;
    let (hauts_courts, _) = voir(&p);
    assert!(!hauts_courts.is_empty() && bout(&hauts_courts) < bout(&hauts));
    p.vues[0].elements_masques.push("e".into());
    assert_eq!(voir(&p), (Vec::new(), false));
    p.vues[0].elements_masques.clear();
    p.vues[0].limite_haute_decalage = 1000.0;
    let (hauts, trait_bas) = voir(&p);
    assert!(hauts.is_empty() && trait_bas);
}

#[test]
fn coupe_sur_feuille_reste_dans_les_limites_de_la_vue() {
    use modele::creation::ParametresMur;
    let mut p = Projet::nouveau("Coupe cadrée");
    let mur = p.ajouter_mur([-2000.0, 0.0], [6000.0, 0.0], &ParametresMur::default()).unwrap();
    let id = p.ajouter_vue_coupe([0.0, 0.0], [4000.0, 0.0], 1000.0, false, "", 100.0).unwrap();
    let placee = VuePlacee { vue: id, position: [25.0, 50.0], echelle: 100.0 };
    let (primitives, _) = vue_placee(&p, &placee).unwrap();
    let mut points = Vec::new();
    for pr in primitives.into_iter().filter(|pr| pr.element == Some(modele::identifiants::id_natif("mur", &mur))) {
        match pr.forme {
            Forme::Segments(s) => points.extend(s.into_iter().flatten()),
            Forme::Triangles(t) => points.extend(t.into_iter().flatten()),
            _ => {}
        }
    }
    let (gauche, droite) = points.iter().map(|p| p.x * 1000.0).fold((f64::INFINITY, f64::NEG_INFINITY), |(a, b), x| (a.min(x), b.max(x)));
    assert!((gauche - 25.0).abs() < 0.01 && (droite - 65.0).abs() < 0.01, "{gauche}..{droite}");
}

#[test]
fn formes_du_cartouche_persistent_et_sont_dessinees_sur_feuille() {
    let mut p = Projet::nouveau("Agence");
    let f = Feuille { id: "f".into(), numero: "A01".into(), nom: "Plan".into(), jeu: JEU_DEFAUT.into(),
        largeur: 420.0, hauteur: 297.0, cartouche: String::new(), vues: Vec::new(), nomenclatures: Vec::new() };
    let mut famille = FamilleCartouche::default();
    famille.nom = "Cartouche test".into();
    let formes = [AnnotationForme::Cercle { centre: [10.0, 10.0], rayon: 4.0, type_trait: String::new() },
        AnnotationForme::Zone { contour: vec![[20.0, 10.0], [25.0, 10.0], [25.0, 15.0]] },
        AnnotationForme::Cote { debut: PointCote::Fixe { point: [20.0, 10.0] }, fin: PointCote::Fixe { point: [25.0, 10.0] }, decalage: 10.0 }];
    use modele::famille::modelee::{Element2DFamille, VueEsquisse};
    famille.document_famille.famille.elements_2d.extend(formes.iter().cloned().map(|forme| Element2DFamille::fixe(VueEsquisse::Plan, forme, 2.5)));
    p.feuilles.push(f.clone());
    p.cartouches_feuilles.insert(f.id.clone(), famille.nom.clone());
    p.familles_cartouches.push(famille.clone());
    let relu = Projet::lire_json(&p.json().unwrap()).unwrap();
    assert_eq!(relu.familles_cartouches[0].document_famille, famille.document_famille);
    let dessin = cadre_feuille(&relu, &f);
    assert!(dessin.iter().any(|pr| matches!(&pr.forme, Forme::Polyligne { points, .. } if points.len() > 40)));
    assert!(dessin.iter().any(|pr| matches!(pr.forme, Forme::Surface(_))));
    assert!(svg_feuille(&relu, &f).unwrap().contains("#d7d7d2"));
}

#[test]
fn famille_cartouche_partagee_est_enregistree_et_utilisee() {
    let f: Feuille = serde_json::from_str(r#"{"id":"f","numero":"A01","nom":"Plan","largeur":420,"hauteur":297,"cartouche":"Agence","vues":[]}"#).unwrap();
    let mut p = Projet::nouveau("Projet");
    let mut g = f.clone();
    g.id = "g".into();
    p.feuilles = vec![f.clone(), g.clone()];
    use modele::famille::modelee::{CategorieFamille, Element2DFamille, FamilleModelee, FichierFamille, VueEsquisse, FORMAT_FAMILLE};
    let mut document = FamilleModelee::vide(CategorieFamille::Cartouche);
    document.elements_2d.push(Element2DFamille::fixe(VueEsquisse::Plan, AnnotationForme::Ligne { debut: [0.0, 0.0], fin: [80.0, 25.0] }, 2.5));
    document.elements_2d.push(Element2DFamille::fixe(VueEsquisse::Plan, AnnotationForme::Texte { point: [2.0, 2.0], texte: "{projet} - {numero}".into() }, 3.0));
    let famille = FamilleCartouche { document_famille: Box::new(FichierFamille { format: FORMAT_FAMILLE.into(), nom: "Agence".into(), largeur: 80.0, hauteur: 25.0, profondeur: 300.0, famille: document }), id: noyau::uuid::uuid(), nom: "Agence".into(), largeur: 80.0, hauteur: 25.0 };
    p.familles_cartouches.push(famille);
    p.cartouches_feuilles.insert(f.id.clone(), "Agence".into());
    p.cartouches_feuilles.insert(g.id.clone(), "Agence".into());
    let relu = Projet::lire_json(&p.json().unwrap()).unwrap();
    assert_eq!(relu.familles_cartouches.len(), 1);
    assert_eq!(relu.cartouches_feuilles.len(), 2);
    assert_eq!(cadre_feuille(&relu, &f).len(), 2); // uniquement le trait et le texte du cartouche
    assert!(svg_feuille(&relu, &f).unwrap().contains("420mm"));
    assert!(pdf_jeu(&relu, JEU_DEFAUT).unwrap().starts_with(b"%PDF-"));
}

/// Polyligne à types de trait par segment : épaisseur du type, tirets à l'échelle de la
/// vue, segment sans type en continu fin ; type absent refusé.
#[test]
fn polyligne_types_de_trait_par_segment() {
    let mut p = Projet::depuis_gabarit("Traits", &modele::gabarit::Gabarit::defaut());
    assert_eq!(p.types_traits, types_traits_standard());
    let vue = p.ajouter_vue_plan(&p.niveaux[0].id.clone(), 100.0).unwrap();
    let echelle = p.vues.iter().find(|v| v.id == vue).unwrap().echelle;
    // trois segments de 10 m : fort, tirets, sans type
    let points = vec![[0.0, 0.0], [10_000.0, 0.0], [10_000.0, 10_000.0], [0.0, 10_000.0]];
    let traits = vec!["trait-continu-fort".to_string(), "trait-tirets".into()];
    p.annotations.push(Annotation { id: "pl".into(), vue: vue.clone(), hauteur: 2.5, forme: AnnotationForme::Polyligne { points, fermee: false, courbures: Vec::new(), remplissage: None, traits } });
    p.verifier().unwrap();
    let v = p.vues.iter().find(|v| v.id == vue).unwrap();
    let prims = annotations(&p, v);
    assert_eq!(prims.len(), 3, "un trait par type");
    // traits rattachés à la polyligne : elle se sélectionne au clic
    assert!(prims.iter().all(|x| x.element == Some(modele::identifiants::id_natif("annotation", "pl"))));
    assert!((prims[0].style.epaisseur - 0.50).abs() < 1e-9);
    let Forme::Segments(s) = &prims[1].forme else { panic!("tirets en segments") };
    // tirets de 3 mm papier, blancs de 1,5 : période 4,5 × échelle sur 10 m
    let periode = 4.5 * echelle;
    assert_eq!(s.len(), (10_000.0 / periode).ceil() as usize);
    let tire = s[0][0].distance(s[0][1]) * 1000.0;
    assert!((tire - 3.0 * echelle).abs() < 1e-6, "{tire}");
    assert!((prims[2].style.epaisseur - 0.18).abs() < 1e-9 && matches!(prims[2].forme, Forme::Polyligne { .. }));
    // type inconnu : refusé
    if let AnnotationForme::Polyligne { traits, .. } = &mut p.annotations[0].forme {
        traits[1] = "inconnu".into();
    }
    assert!(p.verifier().is_err());
}

/// Polyligne à segments d'arc : courbures écrites et relues, arcs développés au rendu
/// (trait et remplissage), sens inversé par une symétrie, données invalides refusées.
#[test]
fn polyligne_segments_d_arc() {
    use glam::DVec2;
    let mut p = Projet::depuis_gabarit("Arcs", &modele::gabarit::Gabarit::defaut());
    let vue = p.ajouter_vue_plan(&p.niveaux[0].id.clone(), 100.0).unwrap();
    // demi-disque : diamètre de 2 m, puis demi-cercle de retour (segment de fermeture)
    let points = vec![[-1000.0, 0.0], [1000.0, 0.0]];
    let forme = |points: Vec<[f64; 2]>, fermee, courbures| AnnotationForme::Polyligne { points, fermee, courbures, remplissage: None, traits: Vec::new() };
    p.annotations.push(Annotation { id: "arc".into(), vue: vue.clone(), hauteur: 2.5, forme: forme(points.clone(), false, vec![1.0]) });
    p.verifier().unwrap();
    // écriture : le champ n'apparaît que s'il y a un arc ; relecture à l'identique
    let json = p.json().unwrap();
    assert!(json.contains("\"courbures\""));
    let relu = Projet::lire_json(&json).unwrap();
    assert_eq!(relu.annotations[0].forme, p.annotations[0].forme);
    let droite = forme(points.clone(), false, Vec::new());
    assert!(!serde_json::to_string(&droite).unwrap().contains("courbures"));
    // rendu : l'arc passe par le bas (sens trigonométrique), à 1 m du centre
    let v = p.vues.iter().find(|v| v.id == vue).unwrap();
    let prims = annotations(&p, v);
    let [Primitive { forme: Forme::Polyligne { points: trace, fermee: false }, .. }] = prims.as_slice() else { panic!("un seul trait attendu") };
    let (o, u, h) = repere(&p, v);
    let plan: Vec<DVec2> = trace.iter().map(|q| { let d = *q * 1000.0 - o; DVec2::new(d.dot(u), d.dot(h)) }).collect();
    assert!(plan.len() > 30 && plan.iter().all(|q| (q.length() - 1000.0).abs() < 1e-6 && q.y < 1e-6));
    assert!(plan.iter().any(|q| q.distance(DVec2::new(0.0, -1000.0)) < 1e-6));
    // contour développé : celui du remplissage et de la forme 2D
    let contour = p.annotations[0].forme.contour_developpe().unwrap();
    assert_eq!(contour.len(), plan.len());
    let forme_2d = p.annotations[0].forme.forme_2d().unwrap();
    assert!((forme_2d.longueur() - std::f64::consts::PI * 1000.0).abs() < 1e-6);
    assert_eq!(AnnotationForme::depuis_forme_2d(&forme_2d), p.annotations[0].forme);
    // symétrie d'axe x : l'arc passe par le haut, sa courbure change de signe
    let (q, _) = p.executer(commandes::Commande::Symetrie { ids: vec!["arc".into()], a: [0.0, 0.0], b: [1.0, 0.0] }).unwrap();
    assert!(matches!(&q.annotations[0].forme, AnnotationForme::Polyligne { courbures, .. } if courbures == &vec![-1.0]));
    // refus : nombre de courbures, valeur non finie, arc entre deux sommets confondus
    for (points, fermee, courbures) in [
        (points.clone(), false, vec![1.0, 0.5]),
        (points.clone(), false, vec![f64::NAN]),
        (vec![[0.0, 0.0], [0.0, 0.0], [5.0, 0.0]], false, vec![1.0, 0.0]),
        (vec![[0.0, 0.0], [10.0, 0.0], [10.0, 10.0]], true, vec![0.0, 1.0]),
    ] {
        p.annotations[0].forme = forme(points, fermee, courbures);
        assert!(p.verifier().is_err());
    }
    // refus à la lecture d'un fichier : une courbure de trop
    // (dernière occurrence : celle de l'annotation, après les familles du gabarit)
    let k = json.rfind("\"courbures\":[").unwrap() + "\"courbures\":[".len();
    let invalide = format!("{}0.5,{}", &json[..k], &json[k..]);
    assert!(invalide != json && Projet::lire_json(&invalide).and_then(|q| q.verifier()).is_err());
}

/// Deux exports de la même feuille donnent le même SVG (comparaisons de fichiers, tests).
#[test]
fn svg_feuille_deterministe() {
    let p = Projet::lire_json(include_str!("../../../samples/maison_native.modele.json")).unwrap();
    let f = &p.feuilles[0];
    let premier = svg_feuille(&p, f).unwrap();
    for _ in 0..4 {
        assert!(svg_feuille(&p, f).unwrap() == premier, "deux exports de la feuille diffèrent");
    }
    // pas de trait réduit à un point (arête verticale vue en plan)
    let nuls = premier.split('M').filter(|m| m.split_once('L').is_some_and(|(a, b)| a.trim() == b.split(['M', '"']).next().unwrap_or("").trim())).count();
    assert_eq!(nuls, 0, "segments de longueur nulle dans le SVG");
}

/// DWG d'une feuille de l'exemple : relu, mêmes entités (lignes, arcs, cercles, aplats)
/// que son DXF, dans les mêmes calques.
#[test]
fn dwg_feuille_relue() {
    let p = modele::Projet::lire_json(&std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/../../samples/maison_native.modele.json")).unwrap()).unwrap();
    let f = p.feuilles.first().expect("une feuille dans l'exemple");
    let dxf = moteur2d::dxf_feuille(&p, f).unwrap();
    let dwg = moteur2d::dwg_feuille(&p, f).unwrap();
    let compte = |doc: &acadrust::CadDocument| {
        let mut c = std::collections::BTreeMap::new();
        for e in doc.entities() {
            *c.entry((format!("{e:?}").split('(').next().unwrap_or("").to_string(), e.common().layer.clone())).or_insert(0usize) += 1;
        }
        c
    };
    let depuis_dxf = acadrust::DxfReader::from_reader(std::io::Cursor::new(dxf.into_bytes())).unwrap().read().unwrap();
    let relu = acadrust::DwgReader::from_stream(std::io::Cursor::new(dwg)).read().unwrap();
    let (a, b) = (compte(&depuis_dxf), compte(&relu));
    assert!(a.len() > 1, "{a:?}");
    assert_eq!(a, b);
}
