//! Test de `modele/famille/profil.rs` qui relit le projet par `collaboration::projet` (aller-retour des objets partagés).
use modele::famille::modelee::FichierFamille;

#[test]
fn profils_du_gabarit_varient_et_survivent_au_rechargement() {
    let g = modele::gabarit::Gabarit::defaut();
    assert_eq!(g.profils.len(), 4);
    let mut p = modele::Projet::depuis_gabarit("Profils", &g);
    p.verifier().unwrap();
    for profil in &mut p.profils {
        let avant = profil.points().unwrap();
        profil.document_famille.famille.parametres[0].formule = "250".into();
        assert_ne!(avant, profil.points().unwrap());
    }
    let relu = modele::Projet::lire_json(&p.json().unwrap()).unwrap();
    assert_eq!(p.profils, relu.profils);
    let fichier = &p.profils[0].document_famille;
    assert_eq!(&FichierFamille::lire(&fichier.json().unwrap()).unwrap(), &**fichier);
    let partage = collaboration::projet(&commandes::objets::objets(&p).unwrap()).unwrap();
    assert_eq!(p.profils, partage.profils);
}
