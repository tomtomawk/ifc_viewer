//! Tests déplacés de `modele/famille/profil.rs` (`mod tests`) : ils appellent du code situé au-dessus
//! de la crate `modele` (commandes, scène, formats, échanges, collaboration ou dessin).
use modele::famille::profil::FichierProfil;

#[test]
fn profils_du_gabarit_varient_et_survivent_au_rechargement() {
    let g = modele::gabarit::Gabarit::defaut();
    assert_eq!(g.profils.len(), 4);
    let mut p = modele::Projet::depuis_gabarit("Profils", &g);
    p.verifier().unwrap();
    for profil in &mut p.profils {
        let avant = profil.points().unwrap();
        profil.document_famille.as_mut().unwrap().famille.parametres[0].formule = "250".into();
        assert_ne!(avant, profil.points().unwrap());
    }
    let relu = modele::Projet::lire_json(&p.json().unwrap()).unwrap();
    assert_eq!(p.profils, relu.profils);
    let fichier = FichierProfil { format: "bimedit.profil".into(), profil: p.profils[0].clone() };
    assert_eq!(FichierProfil::lire(&serde_json::to_string(&fichier).unwrap()).unwrap(), p.profils[0]);
    let partage = crate::collaboration::projet(&crate::collaboration::objets(&p).unwrap()).unwrap();
    assert_eq!(p.profils, partage.profils);
}
