//! Tests déplacés de `modele/gabarits_vues.rs` (`mod tests`) : ils appellent du code situé au-dessus
//! de la crate `modele` (commandes, scène, formats, échanges, collaboration ou dessin).
use commandes::Contextes;
use modele::*;
use modele::creation::ListeCreation;
use modele::gabarits_vues::*;

#[test]
fn invalides_et_modifications_de_phases_restent_coherentes() {
    let mut p = Projet::depuis_gabarit("Gabarits", &modele::gabarit::Gabarit::defaut());
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
