//! Tests déplacés de `modele/documentation/tests.rs` : ils appellent `echanges::depuis_ifc` (et `moteur2d`, `commandes`).
use commandes::Execution;
use moteur2d::{dessin_vue, pdf, pdf_jeu, svg_feuille, vue_placee};
use modele::documentation::*;
use echanges::depuis_ifc;
use glam::DVec3;
use noyau::trace2d::{Forme, Role};

#[test]
fn dessin_vue_applique_le_contexte_propre_a_la_vue() {
    use modele::contextes::FiltresContextes;
    let (mut p, _) = depuis_ifc::convertir(std::path::Path::new(concat!(env!("CARGO_MANIFEST_DIR"), "/../../samples/test_ifc4_murs.ifc")), &|_| {}).unwrap();
    p.phases = vec!["Existant".into(), "Projet".into()];
    for mur in &mut p.murs { mur.phase = Some("Projet".into()); }
    let vue = p.ajouter_vue_plan(&p.niveaux[0].id.clone(), 1200.0).unwrap();
    let dessin = dessin_vue(&p, &vue).unwrap();
    let ids: std::collections::HashSet<_> = p.murs.iter().map(|m| modele::identifiants::id_natif("mur", &m.id)).collect();
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
    use modele::gabarits_vues::GabaritVue;
    let (mut p, _) = depuis_ifc::convertir(std::path::Path::new(concat!(env!("CARGO_MANIFEST_DIR"), "/../../samples/test_ifc4_murs.ifc")), &|_| {}).unwrap();
    p.phases = vec!["Existant".into(), "Projet".into()];
    for mur in &mut p.murs { mur.phase = Some("Projet".into()); }
    let vue = p.ajouter_vue_plan(&p.niveaux[0].id.clone(), 1200.0).unwrap();
    let ids: std::collections::HashSet<_> = p.murs.iter().map(|m| modele::identifiants::id_natif("mur", &m.id)).collect();
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
fn vue_placee_suit_echelle_vue_meme_avec_ancienne_valeur() {
    let (mut projet, _) = depuis_ifc::convertir(std::path::Path::new(concat!(env!("CARGO_MANIFEST_DIR"), "/../../samples/test_ifc4_murs.ifc")), &|_| {}).unwrap();
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
    let (mut p, _) = depuis_ifc::convertir(std::path::Path::new(concat!(env!("CARGO_MANIFEST_DIR"), "/../../samples/test_ifc4_murs.ifc")), &|_| {}).unwrap();
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
    let (mut p, _) = depuis_ifc::convertir(std::path::Path::new(concat!(env!("CARGO_MANIFEST_DIR"), "/../../samples/test_ifc4_murs.ifc")), &|_| {}).unwrap();
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
        depuis_ifc::convertir(std::path::Path::new(concat!(env!("CARGO_MANIFEST_DIR"), "/../../samples/test_ifc4_murs.ifc")), &|_| {})
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
fn polyligne_fermee_remplie() {
    let (mut p, _) = depuis_ifc::convertir(std::path::Path::new(concat!(env!("CARGO_MANIFEST_DIR"), "/../../samples/test_ifc4_murs.ifc")), &|_| {}).unwrap();
    let niveau = p.niveaux[0].id.clone();
    let vue = p.ajouter_vue_plan(&niveau, 1200.0).unwrap();
    let contour = vec![[0.0, 0.0], [2000.0, 0.0], [2000.0, 1000.0], [0.0, 1000.0]];
    let annotation = |remplissage, fermee| Annotation { id: "zone".into(), vue: vue.clone(), hauteur: 2.5, forme: AnnotationForme::Polyligne { points: contour.clone(), fermee, courbures: Vec::new(), remplissage, traits: Vec::new() } };
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
        depuis_ifc::convertir(std::path::Path::new(concat!(env!("CARGO_MANIFEST_DIR"), "/../../samples/test_ifc4_murs.ifc")), &|_| {})
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
    let (q, _) = q.executer(commandes::Commande::Supprimer { ids: vec![choisie.id.clone()] }).unwrap();
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
