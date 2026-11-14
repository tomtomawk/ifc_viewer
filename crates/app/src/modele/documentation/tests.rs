use super::*;

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
