//! Tests déplacés de `modele/gabarits_vues.rs` (`mod tests`) : ils appellent du code situé au-dessus
//! de la crate `modele` (commandes, scène, formats, échanges, collaboration ou dessin).
use crate::modele::*;
use crate::modele::creation::ListeCreation;
use crate::modele::gabarits_vues::*;

#[test]
fn gabarit_partage_actualise_detache_et_se_recharge() {
    let mut p = Projet::depuis_gabarit("Gabarits", &crate::modele::gabarit::Gabarit::defaut());
    let niveau = p.niveaux[0].id.clone();
    let a = p.ajouter_vue_plan(&niveau, 1200.0).unwrap();
    let b = p.ajouter_vue_plan(&niveau, 1300.0).unwrap();
    let c = p.ajouter_vue_plan(&niveau, 1400.0).unwrap();
    let variante = p.ajouter_choix_creation(ListeCreation::Variante, "Option A", "Façade").unwrap();
    let g = GabaritVue { id: String::new(), nom: "Plan projet".into(), echelle: 50.0, poche: "plein".into(), phase: Some("Projet".into()), variante: Some(variante.clone()) };
    let id = p.enregistrer_gabarit_vue(g).unwrap();
    p.affecter_gabarit_vue(&a, Some(&id)).unwrap();
    p.affecter_gabarit_vue(&b, Some(&id)).unwrap();
    p.affecter_gabarit_vue("3d", Some(&id)).unwrap();
    assert_eq!(p.vues.iter().find(|v| v.id == a).unwrap().echelle, 50.0);
    assert_eq!(p.contextes_vues[&b].variante.as_deref(), Some(variante.as_str()));
    assert_eq!(p.contextes_vues["3d"].phase.as_deref(), Some("Projet"));
    assert_eq!(p.echelle_vue_3d, Some(50.0));
    let mut g = p.gabarits_vues[0].clone();
    g.echelle = 200.0; g.poche = "vide".into(); g.phase = Some("Existant".into());
    p.enregistrer_gabarit_vue(g.clone()).unwrap();
    for id_vue in [&a, &b] {
        let vue = p.vues.iter().find(|v| &v.id == id_vue).unwrap();
        assert_eq!(vue.echelle, 200.0);
        assert_eq!(vue.poche_vue.as_deref(), Some("vide"));
        assert_eq!(p.contextes_vues[id_vue].phase.as_deref(), Some("Existant"));
    }
    assert_eq!(p.vues.iter().find(|v| v.id == c).unwrap().echelle, 100.0);
    assert_eq!(p.echelle_vue_3d, Some(200.0));
    p.affecter_gabarit_vue(&a, None).unwrap();
    g.echelle = 75.0;
    p.enregistrer_gabarit_vue(g).unwrap();
    assert_eq!(p.vues.iter().find(|v| v.id == a).unwrap().echelle, 200.0);
    assert_eq!(p.vues.iter().find(|v| v.id == b).unwrap().echelle, 75.0);
    p.verifier().unwrap();
    let relu = Projet::lire_json(&p.json().unwrap()).unwrap();
    assert_eq!(relu.gabarit_vue(&b).unwrap().echelle, 75.0);
    assert!(relu.gabarit_vue(&a).is_none());
    use crate::collaboration;
    let objets = collaboration::objets(&p).unwrap();
    let partage = collaboration::projet(&objets).unwrap();
    assert_eq!(partage.gabarit_vue(&b).unwrap(), p.gabarit_vue(&b).unwrap());
    assert_eq!(partage.contextes_vues[&b], p.contextes_vues[&b]);
    partage.verifier().unwrap();
    p.supprimer_gabarit_vue(&id).unwrap();
    assert!(p.gabarit_vue(&b).is_none());
    assert_eq!(p.vues.iter().find(|v| v.id == b).unwrap().echelle, 75.0);
    assert_eq!(p.echelle_vue_3d, Some(75.0));
    p.verifier().unwrap();
}

#[test]
fn invalides_et_modifications_de_phases_restent_coherentes() {
    let mut p = Projet::depuis_gabarit("Gabarits", &crate::modele::gabarit::Gabarit::defaut());
    let vue = p.ajouter_vue_plan(&p.niveaux[0].id.clone(), 1200.0).unwrap();
    let v = p.ajouter_choix_creation(ListeCreation::Variante, "A", "Jeu").unwrap();
    let id = p.enregistrer_gabarit_vue(GabaritVue { id: String::new(), nom: "Test".into(), echelle: 100.0, poche: "hachure".into(), phase: Some("Projet".into()), variante: Some(v.clone()) }).unwrap();
    p.affecter_gabarit_vue(&vue, Some(&id)).unwrap();
    p.renommer_contexte(ListeCreation::Phase, "Projet", "Travaux", "").unwrap();
    assert_eq!(p.gabarit_vue(&vue).unwrap().phase.as_deref(), Some("Travaux"));
    assert_eq!(p.contextes_vues[&vue].phase.as_deref(), Some("Travaux"));
    p.supprimer_contexte(ListeCreation::Variante, &v, None).unwrap();
    assert_eq!(p.gabarit_vue(&vue).unwrap().variante, None);
    assert_eq!(p.contextes_vues[&vue].variante, None);
    let avant = p.json().unwrap();
    let mut invalide = p.gabarit_vue(&vue).unwrap().clone();
    invalide.echelle = -1.0;
    assert!(p.enregistrer_gabarit_vue(invalide).is_err());
    assert!(p.affecter_gabarit_vue("vue absente", Some(&id)).is_err());
    assert_eq!(p.json().unwrap(), avant);
}
