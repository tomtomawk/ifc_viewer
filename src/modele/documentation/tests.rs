use super::*;

#[test]
fn dessin_vue_applique_le_contexte_propre_a_la_vue() {
    use crate::modele::edition::contextes::FiltresContextes;
    let (mut p, _) = depuis_ifc::convertir(std::path::Path::new("samples/test_ifc4_murs.ifc"), &|_| {}).unwrap();
    p.phases = vec!["Existant".into(), "Projet".into()];
    for mur in &mut p.murs { mur.phase = Some("Projet".into()); }
    let vue = p.ajouter_vue_plan(&p.niveaux[0].id.clone(), 1200.0).unwrap();
    let dessin = dessin_vue(&p, &vue).unwrap();
    let ids: std::collections::HashSet<_> = p.murs.iter().map(|m| crate::rendu::scene::id_natif("mur", &m.id)).collect();
    assert!(dessin.primitives.iter().any(|p| p.element.is_some_and(|id| ids.contains(&id))));
    p.contextes_vues.insert(vue.clone(), FiltresContextes { phase: Some("Existant".into()), ..Default::default() });
    let filtre = dessin_vue(&p, &vue).unwrap();
    assert!(!filtre.primitives.iter().any(|p| p.element.is_some_and(|id| ids.contains(&id))));
    let relu = Projet::lire_json(&p.json().unwrap()).unwrap();
    assert_eq!(relu.contextes_vues[&vue].phase.as_deref(), Some("Existant"));
    assert!(!dessin_vue(&relu, &vue).unwrap().primitives.iter().any(|p| p.element.is_some_and(|id| ids.contains(&id))));
}
#[test]
fn dessin_vue_suit_le_gabarit_partage() {
    use crate::modele::edition::gabarits_vues::GabaritVue;
    let (mut p, _) = depuis_ifc::convertir(std::path::Path::new("samples/test_ifc4_murs.ifc"), &|_| {}).unwrap();
    p.phases = vec!["Existant".into(), "Projet".into()];
    for mur in &mut p.murs { mur.phase = Some("Projet".into()); }
    let vue = p.ajouter_vue_plan(&p.niveaux[0].id.clone(), 1200.0).unwrap();
    let ids: std::collections::HashSet<_> = p.murs.iter().map(|m| crate::rendu::scene::id_natif("mur", &m.id)).collect();
    let id = p.enregistrer_gabarit_vue(GabaritVue { id: String::new(), nom: "Plan".into(), echelle: 50.0, poche: "plein".into(), phase: Some("Existant".into()), variante: None }).unwrap();
    p.affecter_gabarit_vue(&vue, Some(&id)).unwrap();
    let relu = Projet::lire_json(&p.json().unwrap()).unwrap();
    assert!(!dessin_vue(&relu, &vue).unwrap().primitives.iter().any(|p| p.element.is_some_and(|id| ids.contains(&id))));
    let mut g = p.gabarit_vue(&vue).unwrap().clone();
    g.phase = Some("Projet".into());
    p.enregistrer_gabarit_vue(g).unwrap();
    assert!(dessin_vue(&p, &vue).unwrap().primitives.iter().any(|p| p.element.is_some_and(|id| ids.contains(&id))));
    assert_eq!(p.vues.iter().find(|v| v.id == vue).unwrap().echelle, 50.0);
}

#[test]
fn dessin_sur_feuille_suit_le_fond_la_plage_haute_et_le_style() {
    use crate::modele::edition::ParametresMur;
    let mut p = Projet::nouveau("Réglages du plan");
    let bas = p.ajouter_mur([0.0, 0.0], [4000.0, 0.0], &ParametresMur::default()).unwrap();
    let niveau = p.niveaux[0].id.clone();
    let haut = p.ajouter_mur([0.0, 1500.0], [4000.0, 1500.0], &ParametresMur { decalage: 1700.0, hauteur: 600.0, ..Default::default() }).unwrap();
    let id = p.ajouter_vue_plan(&niveau, 1200.0).unwrap();
    let fond = p.dupliquer_vue(&id).unwrap();
    let i = p.vues.iter().position(|v| v.id == id).unwrap();
    p.vues[i].fond_plan = Some(fond);
    let placee = VuePlacee { vue: id.clone(), position: [50.0, 80.0], echelle: 100.0 };
    let haut = crate::rendu::scene::id_natif("mur", &haut);
    let bas = crate::rendu::scene::id_natif("mur", &bas);
    let (dessin, _) = vue_placee(&p, &placee).unwrap();
    let couleur = |dessin: &[Primitive]| dessin.iter().find(|pr| pr.element == Some(bas) && pr.role == Role::Poche && pr.style.couleur != [165, 165, 165, 255]).unwrap().style.couleur;
    let poche_graphique = couleur(&dessin);
    assert!(dessin.iter().any(|pr| pr.style.couleur == [165, 165, 165, 255]));
    assert!(dessin.iter().any(|pr| pr.element == Some(haut) && pr.role == Role::Vu));

    p.vues[i].fond_plan = None;
    p.vues[i].limite_haute_decalage = 1500.0;
    p.rendus_vues.insert(id, crate::modele::soleil::RenduVue { style: Some("realiste".into()), ..Default::default() });
    let (dessin, _) = vue_placee(&p, &placee).unwrap();
    assert!(!dessin.iter().any(|pr| pr.style.couleur == [165, 165, 165, 255] || pr.element == Some(haut)));
    assert_ne!(couleur(&dessin), poche_graphique);
}

#[test]
fn coupe_sur_feuille_reste_dans_les_limites_de_la_vue() {
    use crate::modele::edition::ParametresMur;
    let mut p = Projet::nouveau("Coupe cadrée");
    let mur = p.ajouter_mur([-2000.0, 0.0], [6000.0, 0.0], &ParametresMur::default()).unwrap();
    let id = p.ajouter_vue_coupe([0.0, 0.0], [4000.0, 0.0], 1000.0, false, "", 100.0).unwrap();
    let placee = VuePlacee { vue: id, position: [25.0, 50.0], echelle: 100.0 };
    let (primitives, _) = vue_placee(&p, &placee).unwrap();
    let mut points = Vec::new();
    for pr in primitives.into_iter().filter(|pr| pr.element == Some(crate::rendu::scene::id_natif("mur", &mur))) {
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
    famille.formes = vec![
        AnnotationForme::Cercle { centre: [10.0, 10.0], rayon: 4.0 },
        AnnotationForme::Zone { contour: vec![[20.0, 10.0], [25.0, 10.0], [25.0, 15.0]] },
        AnnotationForme::Cote { debut: PointCote::Fixe { point: [20.0, 10.0] }, fin: PointCote::Fixe { point: [25.0, 10.0] }, decalage: 10.0 },
    ];
    p.feuilles.push(f.clone());
    p.cartouches_feuilles.insert(f.id.clone(), famille.nom.clone());
    p.familles_cartouches.push(famille.clone());
    let relu = Projet::lire_json(&p.json().unwrap()).unwrap();
    assert_eq!(relu.familles_cartouches[0].formes, famille.formes);
    let dessin = cadre_feuille(&relu, &f);
    assert!(dessin.iter().any(|pr| matches!(&pr.forme, Forme::Polyligne { points, .. } if points.len() > 40)));
    assert!(dessin.iter().any(|pr| matches!(pr.forme, Forme::Surface(_))));
    assert!(svg_feuille(&relu, &f).unwrap().contains("#d7d7d2"));
}

#[test]
fn vue_placee_suit_echelle_vue_meme_avec_ancienne_valeur() {
    let (mut projet, _) = depuis_ifc::convertir(std::path::Path::new("samples/test_ifc4_murs.ifc"), &|_| {}).unwrap();
    let id = projet.ajouter_vue_plan(&projet.niveaux[0].id.clone(), 1200.0).unwrap();
    let placee = VuePlacee { vue: id.clone(), position: [30.0, 80.0], echelle: 100.0 };
    let (_, avant) = vue_placee(&projet, &placee).unwrap();
    projet.vues.iter_mut().find(|vue| vue.id == id).unwrap().echelle = 200.0;
    let projet = Projet::lire_json(&projet.json().unwrap()).unwrap();
    let placee: VuePlacee = serde_json::from_str(&serde_json::to_string(&placee).unwrap()).unwrap();
    let (_, apres) = vue_placee(&projet, &placee).unwrap();
    assert_eq!(placee.echelle_sur_feuille(&projet), 200.0);
    assert!(avant[2] - avant[0] > 10.0);
    for (i, origine) in [30.0, 80.0, 30.0, 80.0].into_iter().enumerate() {
        assert!((apres[i] - (origine + (avant[i] - origine) / 2.0)).abs() < 1e-6);
    }
}

#[test]
fn vue_placee_respecte_rotation_et_limite_basse() {
    let (mut p, _) = depuis_ifc::convertir(std::path::Path::new("samples/test_ifc4_murs.ifc"), &|_| {}).unwrap();
    let niveau = p.niveaux[0].id.clone();
    let id = p.ajouter_vue_plan(&niveau, 1200.0).unwrap();
    let placee = VuePlacee { vue: id.clone(), position: [0.0, 0.0], echelle: 100.0 };
    let dessin = dessin_vue(&p, &id).unwrap();
    assert!(!dessin.primitives.is_empty());
    let (_, avant) = vue_placee(&p, &placee).unwrap();
    let vue = p.vues.iter_mut().find(|v| v.id == id).unwrap();
    vue.orientation_plan = std::f64::consts::FRAC_PI_2;
    let tourne = dessin_vue(&p, &id).unwrap();
    assert!(tourne.droite.distance(DVec3::Y) < 1e-9);
    let (_, apres) = vue_placee(&p, &placee).unwrap();
    assert!(((avant[2] - avant[0]) - (apres[3] - apres[1])).abs() < 0.1);
    let vue = p.vues.iter_mut().find(|v| v.id == id).unwrap();
    if let GenreVue::Plan { bas_decalage, .. } = &mut vue.genre { *bas_decalage = 1_000_000.0; }
    assert!(dessin_vue(&p, &id).unwrap().primitives.is_empty());
    p.vues.iter_mut().find(|v| v.id == id).unwrap().coupe_active = Some(false);
    assert!(!dessin_vue(&p, &id).unwrap().primitives.is_empty());
    let relu = Projet::lire_json(&p.json().unwrap()).unwrap();
    let vue = relu.vues.iter().find(|v| v.id == id).unwrap();
    assert_eq!(vue.coupe_active, Some(false));
    assert_eq!(vue.orientation_plan, std::f64::consts::FRAC_PI_2);
}
#[test]
fn vue_placee_respecte_masquage_et_cadrage() {
    let (mut p, _) = depuis_ifc::convertir(std::path::Path::new("samples/test_ifc4_murs.ifc"), &|_| {}).unwrap();
    let niveau = p.niveaux[0].id.clone();
    let id = p.ajouter_vue_plan(&niveau, 1200.0).unwrap();
    let tous = dessin_vue(&p, &id).unwrap();
    assert!(!tous.primitives.is_empty());
    let elements: Vec<_> = p.murs.iter().map(|x| x.id.clone()).chain(p.dalles.iter().map(|x| x.id.clone())).chain(p.importes.iter().map(|x| x.id.clone())).chain(p.ouvertures.iter().map(|x| x.id.clone())).collect();
    p.vues.iter_mut().find(|v| v.id == id).unwrap().elements_masques = elements;
    let masque = dessin_vue(&p, &id).unwrap();
    assert!(masque.primitives.len() < tous.primitives.len());
    let masque_recharge = Projet::lire_json(&p.json().unwrap()).unwrap();
    assert_eq!(dessin_vue(&masque_recharge, &id).unwrap().primitives.len(), masque.primitives.len());
    let vue = p.vues.iter_mut().find(|v| v.id == id).unwrap();
    vue.elements_masques.clear();
    vue.contour_cadrage = vec![[0.,0.], [1000.,0.], [1000.,1000.], [0.,1000.]];
    let recadre = dessin_vue(&p, &id).unwrap();
    assert!(!recadre.primitives.is_empty());
    for primitive in recadre.primitives {
        let points: Vec<DVec3> = match primitive.forme {
            Forme::Segments(s) => s.into_iter().flatten().collect(),
            Forme::Triangles(t) => t.into_iter().flatten().collect(),
            _ => panic!("La découpe doit produire des segments ou triangles"),
        };
        assert!(points.iter().all(|p| p.x >= -1e-6 && p.x <= 1.000001 && p.y >= -1e-6 && p.y <= 1.000001));
    }
    let relu = Projet::lire_json(&p.json().unwrap()).unwrap();
    assert_eq!(relu.vues.iter().find(|v| v.id == id).unwrap().contour_cadrage.len(), 4);
}
#[test]
fn documents_compatibles_v0_references_et_pdf() {
    let (mut p, _) =
        depuis_ifc::convertir(std::path::Path::new("samples/test_ifc4_murs.ifc"), &|_| {})
            .unwrap();
    let vue = p
        .ajouter_vue_plan(&p.niveaux[0].id.clone(), 1200.0)
        .unwrap();
    p.annotations.push(Annotation {
        id: "annotation-test".into(),
        vue: vue.clone(),
        hauteur: 2.5,
        forme: AnnotationForme::Cote {
            debut: PointCote::Mur {
                id: p.murs[0].id.clone(),
                fin: false,
            },
            fin: PointCote::Fixe {
                point: [1000.0, 0.0],
            },
            decalage: 500.0,
        },
    });
    let f = Feuille {
        id: "f".into(),
        numero: "A01".into(),
        nom: "Plan".into(),
        jeu: JEU_DEFAUT.into(),
        largeur: 420.0,
        hauteur: 297.0,
        cartouche: "Agence & associés".into(),
        vues: vec![VuePlacee {
            vue,
            position: [30.0, 80.0],
            echelle: 100.0,
        }],
        nomenclatures: Vec::new(),
    };
    p.feuilles.push(f.clone());
    p.verifier().unwrap();
    let relu = Projet::lire_json(&p.json().unwrap()).unwrap();
    assert_eq!(relu.feuilles.len(), 1);
    let svg = svg_feuille(&p, &f).unwrap();
    assert!(svg.contains("width=\"420mm\""));
    assert!(svg.contains("height=\"297mm\""));
    let pdf = pdf(&svg).unwrap();
    assert!(pdf.starts_with(b"%PDF-"));
    // jeu de deux feuilles (A3 et A4 portrait) : un PDF de deux pages à leurs formats
    let mut g = f.clone();
    g.id = "g".into();
    g.numero = "A02".into();
    (g.largeur, g.hauteur) = (210.0, 297.0);
    p.feuilles.push(g);
    let jeu = pdf_jeu(&p, JEU_DEFAUT).unwrap();
    let texte = String::from_utf8_lossy(&jeu);
    assert!(jeu.starts_with(b"%PDF-"));
    assert!(texte.contains("/Count 2"), "pages");
    assert_eq!(texte.matches("/MediaBox").count(), 2);
    assert!(texte.contains("1190.55") && texte.contains("595.27"), "formats A3 et A4 en points");
    p.feuilles.truncate(1);
    p.supprimer_vue(&f.vues[0].vue).unwrap();
    assert!(p.annotations.is_empty());
    assert!(p.feuilles[0].vues.is_empty());
}
#[test]
fn feuilles_par_jeu_et_anciens_fichiers() {
    // feuille enregistrée avant les jeux : jeu par défaut
    let f: Feuille = serde_json::from_str(r#"{"id":"f","numero":"A01","nom":"Plan","largeur":420,"hauteur":297,"cartouche":"","vues":[]}"#).unwrap();
    assert_eq!(f.jeu, JEU_DEFAUT);
    let mut p = Projet::nouveau("J");
    let mut g = f.clone();
    g.id = "g".into();
    g.jeu = "Détails".into();
    let mut h = f.clone();
    h.id = "h".into();
    p.feuilles = vec![f, g, h];
    assert_eq!(jeux(&p), vec![JEU_DEFAUT.to_string(), "Détails".to_string()]);
    // réordonner : h avant f, puis f à la fin du jeu Détails
    deplacer_feuille(&mut p, "h", JEU_DEFAUT, Some("f")).unwrap();
    let ordre = |p: &Projet| p.feuilles.iter().map(|f| format!("{}:{}", f.id, f.jeu)).collect::<Vec<_>>().join(" ");
    assert_eq!(ordre(&p), "h:Plans f:Plans g:Détails");
    deplacer_feuille(&mut p, "f", "Détails", None).unwrap();
    assert_eq!(ordre(&p), "h:Plans g:Détails f:Détails");
    deplacer_feuille(&mut p, "f", "Détails", Some("g")).unwrap();
    assert_eq!(ordre(&p), "h:Plans f:Détails g:Détails");
    assert!(deplacer_feuille(&mut p, "x", "Plans", None).is_err());
    // cadre et cartouche dans la feuille
    let cadre = cadre_feuille(&p, &p.feuilles[0]);
    assert!(cadre.len() > 4);
}

#[test]
fn cartouche_standard_enregistre_est_partage_par_les_feuilles_sans_affectation() {
    let mut p = Projet::nouveau("Projet");
    let mut standard = FamilleCartouche::default();
    standard.largeur = 125.0;
    let mut autre = FamilleCartouche::default();
    autre.nom = "Agence".into();
    autre.largeur = 80.0;
    p.familles_cartouches.extend([standard, autre]);
    p.cartouches_feuilles.insert("personnalisee".into(), "Agence".into());
    let relu = Projet::lire_json(&p.json().unwrap()).unwrap();
    assert_eq!(famille_cartouche(&relu, "premiere").unwrap().largeur, 125.0);
    assert_eq!(famille_cartouche(&relu, "seconde").unwrap().largeur, 125.0);
    assert_eq!(famille_cartouche(&relu, "personnalisee").unwrap().largeur, 80.0);
}

#[test]
fn famille_cartouche_partagee_est_enregistree_et_utilisee() {
    let f: Feuille = serde_json::from_str(r#"{"id":"f","numero":"A01","nom":"Plan","largeur":420,"hauteur":297,"cartouche":"Agence","vues":[]}"#).unwrap();
    let mut p = Projet::nouveau("Projet");
    let mut g = f.clone();
    g.id = "g".into();
    p.feuilles = vec![f.clone(), g.clone()];
    let famille = FamilleCartouche { id: super::collaboration::uuid(), nom: "Agence".into(), largeur: 80.0, hauteur: 25.0, traits: vec![[[0.0, 0.0], [80.0, 25.0]]], textes: vec![TexteCartouche { contenu: "{projet} - {numero}".into(), position: [2.0, 2.0], hauteur: 3.0 }], formes: Vec::new() };
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

#[test]
fn nomenclature_parametres_de_famille() {
    use crate::modele::famille::modelee::FamilleModelee;
    let mut p = Projet::nouveau("Familles");
    p.niveaux.push(Niveau { id: "n".into(), nom: "RDC".into(), altitude: 0.0, origine: None });
    p.types_murs.push(TypeCompose { id: "t".into(), nom: "Mur".into(), couches: vec![Couche { epaisseur: 200.0, ..Default::default() }], categorie: None, couches_detaillees: true, origine: None });
    p.murs.push(Mur { id: "m".into(), nom: "Mur".into(), type_mur: "t".into(), niveau: "n".into(), decalage_bas: 0.0, haut: Haut::Libre { hauteur: 3000.0 }, axe: Axe::Droit { debut: [0.0, 0.0], fin: [8000.0, 0.0] }, assemblage: None, phase: None, sous_projet: None, variante: None, origine: None });
    let mut famille = FamilleModelee::gabarit_fenetre();
    famille.parametres.iter_mut().find(|x| x.nom == "MateriauAppui").unwrap().formule = "Pierre".into();
    p.types_ouvertures.push(TypeOuverture { id: "f".into(), nom: "F1".into(), genre: GenreOuverture::Fenetre, largeur: 1200.0, hauteur: 1350.0, famille: None, modele: Some(famille), origine: None, famille_nom: None });
    for (k, avec) in [(0, true), (1, false), (2, true)] {
        p.ouvertures.push(Ouverture {
            id: format!("o{k}"), nom: format!("F{k}"), genre: GenreOuverture::Fenetre, mur: "m".into(), type_ouverture: Some("f".into()),
            abscisse: 1500.0 + 2500.0 * k as f64, largeur: 1200.0, hauteur: 1350.0, allege: 900.0, representation: None,
            valeurs: if avec { BTreeMap::new() } else { BTreeMap::from([("AvecAppui".to_string(), 0.0)]) },
            materiaux: BTreeMap::new(), retournee: false, inversee: false, origine: None,
        });
    }
    p.verifier().unwrap();
    let n = Nomenclature { id: "n".into(), nom: "Fenêtres".into(), categorie: "Fenêtres".into(), regrouper: true, colonnes: vec![Colonne::Type, Colonne::Nombre], parametres: vec!["AvecAppui".into(), "MateriauAppui".into()] };
    let t = tableau(&p, &n).unwrap();
    assert_eq!(t[0], ["Type", "Nombre", "AvecAppui", "MateriauAppui"]);
    // l'occurrence sans appui forme son propre groupe
    assert!(t.contains(&vec!["F1".to_string(), "2".into(), "oui".into(), "Pierre".into()]), "{t:?}");
    assert!(t.contains(&vec!["F1".to_string(), "1".into(), "non".into(), "Pierre".into()]), "{t:?}");
}

#[test]
fn geometrie_cote_commune_et_chaine() {
    let g = geometrie_cote(&[DVec2::ZERO, DVec2::new(1000.0, 0.0)], 300.0, 100.0);
    // deux attaches, deux obliques, une ligne de cote
    assert_eq!(g.traits.len(), 5);
    assert_eq!(g.traits[0], [DVec2::ZERO, DVec2::new(0.0, 300.0)]);
    assert_eq!(g.traits[4], [DVec2::new(0.0, 300.0), DVec2::new(1000.0, 300.0)]);
    assert_eq!(g.textes, vec![(DVec2::new(500.0, 300.0), 1000.0)]);
    // chaîne EQ : un texte par intervalle
    let eq = geometrie_cote(&[DVec2::ZERO, DVec2::new(0.0, 400.0), DVec2::new(0.0, 1000.0)], 50.0, 10.0);
    assert_eq!(eq.textes.iter().map(|t| t.1).collect::<Vec<_>>(), vec![400.0, 600.0]);
    assert!(eq.textes.iter().all(|t| (t.0.x + 50.0).abs() < 1e-9));
}
#[test]
fn polyligne_fermee_remplie() {
    let (mut p, _) = depuis_ifc::convertir(std::path::Path::new("samples/test_ifc4_murs.ifc"), &|_| {}).unwrap();
    let niveau = p.niveaux[0].id.clone();
    let vue = p.ajouter_vue_plan(&niveau, 1200.0).unwrap();
    let contour = vec![[0.0, 0.0], [2000.0, 0.0], [2000.0, 1000.0], [0.0, 1000.0]];
    let annotation = |remplissage, fermee| Annotation { id: "zone".into(), vue: vue.clone(), hauteur: 2.5, forme: AnnotationForme::Polyligne { points: contour.clone(), fermee, remplissage, traits: Vec::new() } };
    let hachures = |p: &Projet| annotations(p, p.vues.iter().find(|v| v.id == vue).unwrap()).into_iter().filter(|x| x.role == Role::Hachure).collect::<Vec<_>>();
    // motif choisi : hachures de la définition, encre gris foncé
    p.annotations = vec![annotation(Some(RemplissageContour::Motif { motif: MotifCoupe::Croisees }), true)];
    p.verifier().unwrap();
    let h = hachures(&p);
    assert!(matches!(&h[..], [x] if matches!(&x.forme, Forme::Segments(s) if s.len() > 10) && x.style.couleur == [70, 70, 70, 255]));
    // matériau : son motif de coupe ; aplat : triangles
    let nom = p.materiaux[0].nom.clone();
    p.annotations = vec![annotation(Some(RemplissageContour::Materiau { materiau: nom }), true)];
    p.verifier().unwrap();
    assert!(!hachures(&p).is_empty());
    p.annotations = vec![annotation(Some(RemplissageContour::Motif { motif: MotifCoupe::Uni }), true)];
    assert!(matches!(&hachures(&p)[..], [x] if matches!(&x.forme, Forme::Triangles(t) if t.len() == 2)));
    // relu tel quel ; refusé sur une polyligne ouverte ou un matériau absent
    let relu: Annotation = serde_json::from_str(&serde_json::to_string(&p.annotations[0]).unwrap()).unwrap();
    assert!(matches!(relu.forme, AnnotationForme::Polyligne { remplissage: Some(RemplissageContour::Motif { motif: MotifCoupe::Uni }), .. }));
    p.annotations = vec![annotation(Some(RemplissageContour::Motif { motif: MotifCoupe::Uni }), false)];
    assert!(p.verifier().is_err());
    p.annotations = vec![annotation(Some(RemplissageContour::Materiau { materiau: "Inconnu".into() }), true)];
    assert!(p.verifier().is_err());
}

#[test]
fn nomenclature_csv_et_quantites_sans_double_compte() {
    let (p, _) =
        depuis_ifc::convertir(std::path::Path::new("samples/test_ifc4_murs.ifc"), &|_| {})
            .unwrap();
    let n = Nomenclature {
        id: "n".into(),
        nom: "Murs".into(),
        categorie: "Murs".into(),
        regrouper: true,
        colonnes: Vec::new(),
        parametres: Vec::new(),
    };
    // colonnes choisies : type, niveau, nombre, longueur, volume ; regroupées par type
    // et niveau, le nombre total reste celui des murs
    let choisie = Nomenclature { colonnes: vec![Colonne::Type, Colonne::Niveau, Colonne::Nombre, Colonne::Longueur, Colonne::Volume], ..n.clone() };
    let t = tableau(&p, &choisie).unwrap();
    assert_eq!(t[0], ["Type", "Niveau", "Nombre", "Longueur (mm)", "Volume net (m³)"]);
    let total: usize = t.iter().skip(1).map(|r| r[2].parse::<usize>().unwrap()).sum();
    assert_eq!(total, p.murs.len());
    let detail = tableau(&p, &Nomenclature { regrouper: false, ..choisie.clone() }).unwrap();
    assert_eq!(detail.len() - 1, p.murs.len());
    assert_eq!(Colonne::depuis_titre("Famille et type"), Some(Colonne::Type));
    assert_eq!(Colonne::depuis_titre("Commentaires"), None);
    // tableau posé sur une feuille : titre, une ligne par groupe, sous sa position
    let mut q = p.clone();
    q.nomenclatures.push(choisie.clone());
    let posee = NomenclaturePlacee { nomenclature: choisie.id.clone(), position: [20.0, 200.0] };
    let (prims, e) = nomenclature_placee(&q, &posee).unwrap();
    assert!(!prims.is_empty() && e[3] == 200.0 && e[1] < 200.0 - 6.0 * t.len() as f64 && e[2] > 20.0, "{e:?}");
    let f = Feuille { id: "f".into(), numero: "A01".into(), nom: "Quantités".into(), jeu: JEU_DEFAUT.into(), largeur: 420.0, hauteur: 297.0, cartouche: String::new(), vues: Vec::new(), nomenclatures: vec![posee] };
    q.feuilles.push(f.clone());
    q.verifier().unwrap();
    assert!(svg_feuille(&q, &f).unwrap().len() > svg_feuille(&q, &Feuille { nomenclatures: Vec::new(), ..f.clone() }).unwrap().len());
    // nomenclature supprimée : retirée de la feuille
    let (q, _) = q.executer(crate::modele::edition::commandes::Commande::Supprimer { ids: vec![choisie.id.clone()] }).unwrap();
    assert!(q.feuilles[0].nomenclatures.is_empty());
    let t = tableau(&p, &n).unwrap();
    let count: usize = t
        .iter()
        .skip(1)
        .map(|r| r[2].parse::<usize>().unwrap())
        .sum();
    assert_eq!(count, p.murs.len());
    assert_eq!(
        csv(&[vec!["A;B".into(), "Il dit \"oui\"".into()]]),
        "\"A;B\";\"Il dit \"\"oui\"\"\"\r\n"
    );
}

/// Polyligne à types de trait par segment : épaisseur du type, tirets à l'échelle de la
/// vue, segment sans type en continu fin ; type absent refusé.
#[test]
fn polyligne_types_de_trait_par_segment() {
    let mut p = Projet::depuis_gabarit("Traits", &crate::config::gabarit::Gabarit::defaut());
    assert_eq!(p.types_traits, types_traits_standard());
    let vue = p.ajouter_vue_plan(&p.niveaux[0].id.clone(), 100.0).unwrap();
    let echelle = p.vues.iter().find(|v| v.id == vue).unwrap().echelle;
    // trois segments de 10 m : fort, tirets, sans type
    let points = vec![[0.0, 0.0], [10_000.0, 0.0], [10_000.0, 10_000.0], [0.0, 10_000.0]];
    let traits = vec!["trait-continu-fort".to_string(), "trait-tirets".into()];
    p.annotations.push(Annotation { id: "pl".into(), vue: vue.clone(), hauteur: 2.5, forme: AnnotationForme::Polyligne { points, fermee: false, remplissage: None, traits } });
    p.verifier().unwrap();
    let v = p.vues.iter().find(|v| v.id == vue).unwrap();
    let prims = annotations(&p, v);
    assert_eq!(prims.len(), 3, "un trait par type");
    // traits rattachés à la polyligne : elle se sélectionne au clic
    assert!(prims.iter().all(|x| x.element == Some(crate::rendu::scene::id_natif("annotation", "pl"))));
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

#[test]
fn cotes_de_mesure_angle_diametre_altimetrie() {
    use glam::DVec2;
    // angle saillant, quel que soit l'ordre des branches
    let (a, ..) = angle_branches(DVec2::ZERO, DVec2::new(1000.0, 0.0), DVec2::new(0.0, 500.0)).unwrap();
    assert!((a.to_degrees() - 90.0).abs() < 1e-9);
    let (b, ..) = angle_branches(DVec2::ZERO, DVec2::new(0.0, 500.0), DVec2::new(1000.0, 0.0)).unwrap();
    assert!((b - a).abs() < 1e-12);
    assert!(angle_branches(DVec2::ZERO, DVec2::ZERO, DVec2::X).is_none());
    let g = geometrie_angle(DVec2::ZERO, DVec2::new(1000.0, 0.0), DVec2::new(1000.0, 1000.0), 600.0, 50.0, 100.0).unwrap();
    assert_eq!(g.texte.2, "45°");
    // diamètre : ligne d'un bord à l'autre par le centre
    let g = geometrie_diametre(DVec2::new(100.0, 100.0), DVec2::new(400.0, 100.0), 50.0, 100.0).unwrap();
    assert_eq!(g.traits[0], [DVec2::new(-200.0, 100.0), DVec2::new(400.0, 100.0)]);
    assert!(g.texte.2.starts_with("Ø "));
    // altimétrie : signe et zéro
    assert_eq!(texte_altitude(2500.0, 2), "+2.50");
    assert_eq!(texte_altitude(-200.0, 3), "-0.200");
    assert_eq!(texte_altitude(0.4, 2), "±0.00");
}

#[test]
fn altimetrie_en_coupe_relative_au_niveau() {
    let mut p = Projet::nouveau("Altimétrie");
    p.niveaux = vec![
        crate::modele::Niveau { id: "n0".into(), nom: "RDC".into(), altitude: 0.0, origine: None },
        crate::modele::Niveau { id: "n1".into(), nom: "R+1".into(), altitude: 3000.0, origine: None },
    ];
    let id = p.ajouter_vue_coupe([0.0, 0.0], [4000.0, 0.0], 1000.0, false, "", 100.0).unwrap();
    let v = p.vues.iter().find(|v| v.id == id).unwrap().clone();
    assert!((altitude_vue(&p, &v, [500.0, 3500.0], false) - 3500.0).abs() < 1e-9);
    assert!((altitude_vue(&p, &v, [500.0, 3500.0], true) - 500.0).abs() < 1e-9);
    assert!((altitude_vue(&p, &v, [500.0, -200.0], true) + 200.0).abs() < 1e-9);
}

#[test]
fn methode_de_cotation_commune() {
    use glam::DVec2;
    // nombre de points et construction identiques pour tous les éditeurs
    let c = GenreCote::Lineaire.forme(&[DVec2::ZERO, DVec2::new(1000.0, 0.0), DVec2::new(500.0, 300.0)], false, 2).unwrap();
    assert!(matches!(c, AnnotationForme::Cote { decalage, .. } if (decalage - 300.0).abs() < 1e-9));
    assert!(GenreCote::Lineaire.forme(&[DVec2::ZERO, DVec2::ZERO, DVec2::X], false, 2).is_none());
    assert!(GenreCote::Angulaire.points_apercu(&[DVec2::ZERO, DVec2::X * 100.0], DVec2::Y * 100.0).is_some_and(|p| p.len() == 4));
    for g in [GenreCote::Lineaire, GenreCote::Angulaire, GenreCote::Diametre, GenreCote::Altimetrie] {
        assert!(!g.invite(0).is_empty());
    }
    // cartouche : cote angulaire rendue sur le papier
    let p = Projet::nouveau("Cartouche");
    let f = GenreCote::Angulaire.forme(&[DVec2::ZERO, DVec2::new(20.0, 0.0), DVec2::new(0.0, 20.0), DVec2::new(10.0, 0.0)], false, 2).unwrap();
    assert!(!forme_cartouche(&p, &f).is_empty());
    // ancien nom sérialisé de l'éditeur de familles
    assert_eq!(serde_json::from_str::<GenreCote>("\"angle\"").unwrap(), GenreCote::Angulaire);
}
