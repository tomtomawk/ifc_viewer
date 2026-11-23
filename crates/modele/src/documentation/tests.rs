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
    let standard = FamilleCartouche { largeur: 125.0, ..Default::default() };
    let autre = FamilleCartouche { nom: "Agence".into(), largeur: 80.0, ..Default::default() };
    p.familles_cartouches.extend([standard, autre]);
    p.cartouches_feuilles.insert("personnalisee".into(), "Agence".into());
    let relu = Projet::lire_json(&p.json().unwrap()).unwrap();
    assert_eq!(famille_cartouche(&relu, "premiere").unwrap().largeur, 125.0);
    assert_eq!(famille_cartouche(&relu, "seconde").unwrap().largeur, 125.0);
    assert_eq!(famille_cartouche(&relu, "personnalisee").unwrap().largeur, 80.0);
}

#[test]
fn nomenclature_parametres_de_famille() {
    use crate::famille::modelee::FamilleModelee;
    let mut p = Projet::nouveau("Familles");
    p.niveaux.push(Niveau { id: "n".into(), nom: "RDC".into(), altitude: 0.0, origine: None });
    p.types_murs.push(TypeCompose { sorte: None, id: "t".into(), nom: "Mur".into(), couches: vec![Couche { epaisseur: 200.0, ..Default::default() }], categorie: None, couches_detaillees: true, origine: None });
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
        crate::Niveau { id: "n0".into(), nom: "RDC".into(), altitude: 0.0, origine: None },
        crate::Niveau { id: "n1".into(), nom: "R+1".into(), altitude: 3000.0, origine: None },
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

/// Spline et cercle portent un type de trait : écrit seulement s'il est donné, rendu en
/// tirets, refusé s'il n'existe pas dans le projet.
#[test]
fn type_de_trait_des_splines_et_cercles() {
    use noyau::trace2d::Forme;
    let mut p = Projet::nouveau("Traits");
    p.types_traits = types_traits_standard();
    let tirets = p.types_traits.iter().find(|t| !t.tirets.is_empty()).unwrap().id.clone();
    let cercle = AnnotationForme::Cercle { centre: [0.0, 0.0], rayon: 1000.0, type_trait: tirets.clone() };
    let spline = AnnotationForme::Spline { points: vec![[0.0, 0.0], [1000.0, 500.0], [2000.0, 0.0]], fermee: false, type_trait: String::new() };
    // continu : une polyligne ; tirets : des segments
    let pt = |q: DVec2| q.extend(0.0);
    assert!(matches!(primitives_dessin(&p, &cercle, 100.0, &pt)[0].forme, Forme::Segments(_)));
    assert!(matches!(primitives_dessin(&p, &spline, 100.0, &pt)[0].forme, Forme::Polyligne { .. }));
    // écriture : champ omis quand il est vide, relu tel quel
    let json = serde_json::to_string(&spline).unwrap();
    assert!(!json.contains("type_trait"));
    let relu: AnnotationForme = serde_json::from_str(&serde_json::to_string(&cercle).unwrap()).unwrap();
    assert_eq!(relu, cercle);
    // validation : type absent refusé
    p.niveaux.push(crate::Niveau { id: "rdc".into(), nom: "RDC".into(), altitude: 0.0, origine: None });
    let vue = p.ajouter_vue_plan("rdc", 1200.0).unwrap();
    p.annotations.push(Annotation { id: "c".into(), vue, hauteur: 2.5, forme: cercle });
    p.verifier().unwrap();
    if let AnnotationForme::Cercle { type_trait, .. } = &mut p.annotations[0].forme { *type_trait = "absent".into(); }
    assert!(p.verifier().is_err());
}

#[test]
fn cartouche_deplace_supprime_et_annotations_de_feuille() {
    let mut p = Projet::nouveau("Projet");
    let mut f: Feuille = serde_json::from_str(r#"{"id":"f","numero":"A01","nom":"Plan","largeur":420,"hauteur":297,"cartouche":"","vues":[]}"#).unwrap();
    // place par défaut : en bas à droite, dans la marge
    let [l, h] = taille_cartouche(&p, &f);
    assert_eq!(cadre_cartouche(&p, &f), Some([420.0 - MARGE - l, MARGE, 420.0 - MARGE, MARGE + h]));
    // déplacé : coin bas gauche choisi ; supprimé : plus de cadre ni de primitives
    f.position_cartouche = Some([15.0, 240.0]);
    assert_eq!(cadre_cartouche(&p, &f), Some([15.0, 240.0, 15.0 + l, 240.0 + h]));
    assert!(!cartouche_feuille(&p, &f).is_empty());
    f.sans_cartouche = true;
    assert_eq!(cadre_cartouche(&p, &f), None);
    assert!(cadre_feuille(&p, &f).is_empty());
    // annotations : texte à champs et rectangle, dessinés, déplacés, repérés
    let texte = AnnotationFeuille { hauteur: 5.0, forme: AnnotationForme::Texte { point: [50.0, 50.0], texte: "{numero} {feuille}".into() } };
    let rect = AnnotationFeuille { hauteur: HAUTEUR_TEXTE_FEUILLE, forme: AnnotationForme::Polyligne { points: vec![[100.0, 100.0], [200.0, 100.0], [200.0, 150.0], [100.0, 150.0]], fermee: true, courbures: Vec::new(), remplissage: None, traits: Vec::new() } };
    f.annotations = vec![texte, rect];
    assert_eq!(contenu_texte_feuille(&p, &f, "{numero} {feuille}"), "A01 Plan");
    assert_eq!(cadre_feuille(&p, &f).len(), 2);
    assert_eq!(f.annotations[0].distance(&f, &p, [51.0, 52.0]), 0.0);
    assert!((f.annotations[1].distance(&f, &p, [150.0, 98.0]) - 2.0).abs() < 1e-9);
    f.annotations[1].translater([10.0, -5.0]);
    assert_eq!(f.annotations[1].etendue(&f, &p), Some([110.0, 95.0, 210.0, 145.0]));
    // écriture et relecture ; feuille sans ces champs : cartouche par défaut, rien d'annoté
    p.feuilles.push(f.clone());
    p.verifier().unwrap();
    let relu = Projet::lire_json(&p.json().unwrap()).unwrap();
    assert_eq!(relu.feuilles[0].annotations, f.annotations);
    assert!(relu.feuilles[0].sans_cartouche);
    assert_eq!(relu.feuilles[0].position_cartouche, Some([15.0, 240.0]));
    let json = serde_json::to_string(&Feuille { annotations: Vec::new(), position_cartouche: None, sans_cartouche: false, ..f.clone() }).unwrap();
    assert!(!json.contains("\"annotations\""));
    assert!(!json.contains("sans_cartouche") && !json.contains("position_cartouche"));
    // validation : forme non admise ou point non fini refusés
    p.feuilles[0].annotations.push(AnnotationFeuille { hauteur: 2.5, forme: AnnotationForme::Cote { debut: PointCote::Fixe { point: [0.0, 0.0] }, fin: PointCote::Fixe { point: [10.0, 0.0] }, decalage: 5.0 } });
    assert!(p.verifier().is_err());
    p.feuilles[0].annotations.pop();
    p.feuilles[0].annotations[0].translater([f64::NAN, 0.0]);
    assert!(p.verifier().is_err());
}
