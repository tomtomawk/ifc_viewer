//! Tests de `formats` qui appellent du code d'`app` (`echanges`) et de `modele` : ils
//! restent dans `app` tant que `echanges` n'est pas extraite (lots 5 à 9).
use formats::rvt::maquette::{lire, scene};
use std::path::PathBuf;

/// Fichier d'essai hors dépôt (même convention que `formats::rvt::maquette::tests`).
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
