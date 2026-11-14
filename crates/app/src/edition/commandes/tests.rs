use super::*;
use crate::echanges::{depuis_ifc, export_ifc};
use crate::edition::historique;
fn projet() -> Projet {
    depuis_ifc::convertir(std::path::Path::new(concat!(env!("CARGO_MANIFEST_DIR"), "/../../samples/test_ifc4_murs.ifc")), &|_| {})
        .unwrap()
        .0
}
#[test]
fn structures_creation_copie_rotation_suppression_et_validation() {
    let p = projet();
    let n = p.niveaux[0].id.clone();
    let poteau = Barre {
        id: String::new(),
        nom: "Poteau".into(),
        genre: GenreBarre::Poteau,
        niveau: n.clone(),
        debut: [100.0, 200.0, 0.0],
        fin: [100.0, 200.0, 3000.0],
        largeur: 300.0,
        hauteur: 400.0,
        rotation: 0.0,
    };
    let (p, ids) = p.executer(Commande::Barre { barre: poteau }).unwrap();
    let b = &p.barres[0];
    assert!((geometrie::volume(&solides::barre(&p, b).unwrap()) - 360_000_000.0).abs() < 0.1);
    let (p, _) = p
        .executer(Commande::Tourner {
            ids: ids.clone(),
            centre: [0.0, 0.0],
            angle: 90.0,
        })
        .unwrap();
    assert!((p.barres[0].rotation - 90.0).abs() < 1e-6);
    let (copie, copies) = p
        .executer(Commande::Copier {
            ids: ids.clone(),
            delta: [1000.0, 0.0, 0.0],
        })
        .unwrap();
    assert_eq!(copie.barres.len(), 2);
    let (retour, _) = copie.executer(Commande::Supprimer { ids: copies }).unwrap();
    assert_eq!(retour.json().unwrap(), p.json().unwrap());
    assert!(
        p.executer(Commande::Supprimer {
            ids: vec![n.clone()]
        })
        .is_err()
    );
    let poutre = Barre {
        id: String::new(),
        nom: "Poutre".into(),
        genre: GenreBarre::Poutre,
        niveau: n,
        debut: [0.0, 0.0, 2500.0],
        fin: [3000.0, 0.0, 2500.0],
        largeur: 200.0,
        hauteur: 400.0,
        rotation: 0.0,
    };
    let (p, _) = p
        .executer(Commande::Barre {
            barre: poutre.clone(),
        })
        .unwrap();
    assert!(
        (geometrie::volume(&solides::barre(&p, &p.barres[1]).unwrap()) - 240_000_000.0).abs()
            < 0.1
    );
    let mut invalide = poutre;
    invalide.fin[2] += 1.0;
    assert!(p.executer(Commande::Barre { barre: invalide }).is_err());
    let relu = Projet::lire_json(&p.json().unwrap()).unwrap();
    maillage::depuis_natif(&relu, std::path::Path::new("test.modele.json")).unwrap();
    let ifc = export_ifc::exporter(&relu).unwrap();
    assert!(ifc.contains("IFCCOLUMN(") && ifc.contains("IFCBEAM("));
}
#[test]
fn modifier_deplacer_copier_et_supprimer_une_coupe() {
    let mut p = projet();
    let id = p
        .ajouter_vue_coupe([0.0, 0.0], [2000.0, 0.0], 6000.0, false, "Coupe", 100.0)
        .unwrap();
    let (moved, _) = p
        .executer(Commande::Deplacer {
            ids: vec![id.clone()],
            delta: [500.0, 1000.0, 0.0],
        })
        .unwrap();
    assert!(
        matches!(moved.vues.iter().find(|v| v.id == id).unwrap().genre, GenreVue::Coupe { debut, .. } if debut == [500.0,1000.0])
    );
    assert!(
        p.executer(Commande::Deplacer {
            ids: vec![id.clone()],
            delta: [0.0, 0.0, 10.0]
        })
        .is_err()
    );
    let (copie, ids) = moved
        .executer(Commande::Copier {
            ids: vec![id.clone()],
            delta: [0.0, 2000.0, 0.0],
        })
        .unwrap();
    assert!(
        matches!(copie.vues.iter().find(|v| v.id == ids[0]).unwrap().genre, GenreVue::Coupe { debut, .. } if debut == [500.0,3000.0])
    );
    let (retour, _) = copie.executer(Commande::Supprimer { ids }).unwrap();
    assert_eq!(retour.json().unwrap(), moved.json().unwrap());
}
#[test]
fn annotations_et_quadrillages_se_copient_et_se_deplacent() {
    let p = Projet::lire_json(
        &std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/../../samples/maison_native.modele.json")).unwrap(),
    )
    .unwrap();
    let texte = p
        .annotations
        .iter()
        .find(|a| a.id == "texte-sejour")
        .unwrap();
    let ancien = match &texte.forme {
        documentation::AnnotationForme::Texte { point, .. } => *point,
        _ => panic!("texte attendu"),
    };
    let (deplace, _) = p
        .executer(Commande::Deplacer {
            ids: vec![texte.id.clone()],
            delta: [100.0, 200.0, 0.0],
        })
        .unwrap();
    assert!(
        matches!(&deplace.annotations.iter().find(|a| a.id == texte.id).unwrap().forme, documentation::AnnotationForme::Texte { point, .. } if *point == [ancien[0] + 100.0, ancien[1] + 200.0])
    );
    let mut p = p;
    p.contextes.insert(
        "porte".into(),
        ContexteElement {
            phase: Some("Construction".into()),
            sous_projet: Some("Structure".into()),
            variante: None,
        },
    );
    let mut note = p
        .annotations
        .iter()
        .find(|a| a.id == "texte-sejour")
        .unwrap()
        .clone();
    note.id = "note-coupe".into();
    note.vue = "coupe".into();
    p.annotations.push(note);
    let (section, _) = p
        .executer(Commande::Deplacer {
            ids: vec!["note-coupe".into()],
            delta: [100.0, 0.0, 200.0],
        })
        .unwrap();
    assert!(
        matches!(&section.annotations.iter().find(|a| a.id == "note-coupe").unwrap().forme, documentation::AnnotationForme::Texte { point, .. } if *point == [ancien[0]+100.0, ancien[1]+200.0])
    );
    assert!(
        p.executer(Commande::Deplacer {
            ids: vec!["note-coupe".into()],
            delta: [0.0, 100.0, 0.0]
        })
        .is_err()
    );
    let (copie, ids) = p
        .executer(Commande::Copier {
            ids: vec!["coupe".into()],
            delta: [100.0, 0.0, 0.0],
        })
        .unwrap();
    assert!(copie.vues.iter().any(|v| v.id == ids[0]));
    assert!(
        copie
            .annotations
            .iter()
            .any(|a| a.vue == ids[0] && a.id != "note-coupe")
    );
    let (retour, _) = copie.executer(Commande::Supprimer { ids }).unwrap();
    assert_eq!(retour.json().unwrap(), p.json().unwrap());
    let (copie, ids) = p
        .executer(Commande::Copier {
            ids: vec!["rdc-m0".into(), "cote-largeur".into()],
            delta: [10000.0, 0.0, 0.0],
        })
        .unwrap();
    let porte = copie
        .ouvertures
        .iter()
        .find(|o| o.mur == ids[0] && o.genre == GenreOuverture::Porte)
        .unwrap();
    assert_eq!(
        copie
            .contextes
            .get(&porte.id)
            .unwrap()
            .sous_projet
            .as_deref(),
        Some("Structure")
    );
    let cote = copie.annotations.iter().find(|a| a.id == ids[1]).unwrap();
    assert!(
        matches!(&cote.forme, documentation::AnnotationForme::Cote { debut: documentation::PointCote::Mur { id, .. }, .. } if id == &ids[0])
    );
    let (retour, _) = copie.executer(Commande::Supprimer { ids }).unwrap();
    assert_eq!(retour.json().unwrap(), p.json().unwrap());
    let grid = &p.quadrillages[0];
    let (copie, ids) = p
        .executer(Commande::Copier {
            ids: vec![grid.id.clone()],
            delta: [100.0, 0.0, 0.0],
        })
        .unwrap();
    assert!(
        copie
            .quadrillages
            .iter()
            .any(|g| g.id == ids[0] && g.debut[0] == grid.debut[0] + 100.0)
    );
}
#[test]
fn copier_assemblage_et_dependances_puis_supprimer_sans_perte() {
    let p = projet();
    let a = p.assemblages.first().unwrap();
    let avant = p.json().unwrap();
    let (copie, ids) = p
        .executer(Commande::Copier {
            ids: vec![a.id.clone()],
            delta: [7000.0, 0.0, 1000.0],
        })
        .unwrap();
    let id = &ids[0];
    let nouveau = copie.assemblage(id).unwrap();
    assert_eq!(nouveau.couches.len(), a.couches.len());
    assert!(nouveau.couches.iter().all(|id| !a.couches.contains(id)));
    assert_eq!(
        copie.ouvertures.iter().filter(|o| o.mur == *id).count(),
        p.ouvertures.iter().filter(|o| o.mur == a.id).count()
    );
    let (retour, _) = copie.executer(Commande::Supprimer { ids }).unwrap();
    assert_eq!(retour.json().unwrap(), avant);
}
#[test]
fn erreurs_atomiques_et_niveaux_utilises() {
    let p = projet();
    let avant = p.json().unwrap();
    assert!(
        p.executer(Commande::Niveau {
            nom: "".into(),
            altitude: 0.0
        })
        .is_err()
    );
    assert!(
        p.executer(Commande::Supprimer {
            ids: vec![p.niveaux[0].id.clone()]
        })
        .is_err()
    );
    assert!(
        p.executer(Commande::Deplacer {
            ids: vec![p.assemblages[0].id.clone()],
            delta: [f64::NAN, 0.0, 0.0]
        })
        .is_err()
    );
    assert_eq!(p.json().unwrap(), avant);
}
#[test]
fn deplacement_rotation_menuiseries_et_annulation() {
    let p = projet();
    let a = &p.assemblages[0];
    let (q, _) = p
        .executer(Commande::Deplacer {
            ids: vec![a.id.clone()],
            delta: [1000.0, 2000.0, 300.0],
        })
        .unwrap();
    let mut h = historique::Historique::default();
    h.enregistrer(&p).unwrap();
    h.pousser(&p, &q).unwrap();
    let (r, _) = q
        .executer(Commande::Tourner {
            ids: vec![a.id.clone()],
            centre: [1000.0, 2000.0],
            angle: 90.0,
        })
        .unwrap();
    h.pousser(&q, &r).unwrap();
    let retour = h.cible(false).unwrap();
    h.confirmer(&r, false).unwrap();
    assert_eq!(retour.json().unwrap(), q.json().unwrap());
    let retour = h.cible(false).unwrap();
    h.confirmer(&q, false).unwrap();
    assert_eq!(retour.json().unwrap(), p.json().unwrap());
    assert!(!h.modifie(&retour));
    let retabli = h.cible(true).unwrap();
    assert_eq!(retabli.json().unwrap(), q.json().unwrap());
}
#[test]
fn tremie_interieure_et_exterieure() {
    let mut p = Projet::nouveau("Trémie");
    let params = creation::ParametresDalle {
        sorte: SorteDalle::Dalle,
        type_dalle: TypeCompose {
            id: "t".into(),
            nom: "Dalle".into(),
            couches: vec![Couche {
                epaisseur: 200.0,
                ..Default::default()
            }],
            categorie: None, couches_detaillees: true,
            origine: None,
        },
        pente: None,
        niveau: None,
        decalage: 0.0,
        phase: None,
        sous_projet: None,
        variante: None,
    };
    let id = p
        .ajouter_dalle(
            &[[0.0, 0.0], [6000.0, 0.0], [6000.0, 5000.0], [0.0, 5000.0]],
            &params,
        )
        .unwrap();
    let v = p.volume_dalle(&p.dalles[0]).unwrap();
    let contour = vec![
        [1000.0, 1000.0],
        [2000.0, 1000.0],
        [2000.0, 2000.0],
        [1000.0, 2000.0],
    ];
    let (q, _) = p
        .executer(Commande::Tremie {
            id: id.clone(),
            contour: contour.clone(),
        })
        .unwrap();
    assert!((q.volume_dalle(&q.dalles[0]).unwrap() / v - 29.0 / 30.0).abs() < 1e-8);
    assert!(q.executer(Commande::Tremie { id: id.clone(), contour: contour.clone() }).is_err());
    let mut avec_pentes = q.clone();
    avec_pentes.dalles[0].pentes = vec![vec![Some(15.0), Some(20.0), None, None]];
    let mut modifies = avec_pentes.dalles[0].contours.clone();
    modifies[0][0] = [-500.0, 0.0];
    let (modifie, _) = avec_pentes.executer(Commande::Contour { id: id.clone(), contours: modifies.clone() }).unwrap();
    assert_eq!(modifie.dalles[0].pente(0, 0), None);
    assert_eq!(modifie.dalles[0].pente(0, 1), Some(20.0));
    modifies[1] = vec![[5500.0, 1000.0], [6500.0, 1000.0], [6500.0, 2000.0], [5500.0, 2000.0]];
    assert!(avec_pentes.executer(Commande::Contour { id: id.clone(), contours: modifies }).is_err());
    // terre-plein : un sommet déplacé garde les talus et les remblais de ses arêtes ;
    // un sommet ajouté ne garde que ceux des arêtes inchangées
    let mut plein = q.clone();
    plein.dalles[0].sorte = modele::SorteDalle::TerrePlein;
    plein.dalles[0].pentes = vec![vec![Some(45.0), Some(30.0), None, None]];
    plein.dalles[0].remblais = vec![vec![true, false, false, false]];
    let mut deplaces = plein.dalles[0].contours.clone();
    deplaces[0][0] = [-500.0, 0.0];
    let (r, _) = plein.executer(Commande::Contour { id: id.clone(), contours: deplaces.clone() }).unwrap();
    assert_eq!((r.dalles[0].pente(0, 0), r.dalles[0].pente(0, 1), r.dalles[0].remblaye(0, 0)), (Some(45.0), Some(30.0), true));
    let milieu = [(deplaces[0][0][0] + deplaces[0][1][0]) / 2.0, (deplaces[0][0][1] + deplaces[0][1][1]) / 2.0];
    deplaces[0].insert(1, milieu);
    let (r, _) = plein.executer(Commande::Contour { id, contours: deplaces }).unwrap();
    assert_eq!((r.dalles[0].pente(0, 0), r.dalles[0].pente(0, 2), r.dalles[0].remblaye(0, 0)), (None, Some(30.0), false));
}
