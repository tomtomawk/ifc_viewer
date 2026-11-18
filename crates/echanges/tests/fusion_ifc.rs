//! Fusion d'un IFC converti dans un projet natif ouvert (`Commande::Fusionner`).
use commandes::*;
use echanges::depuis_ifc;
use modele::*;
use std::collections::HashSet;

fn importe() -> Projet {
    depuis_ifc::convertir(std::path::Path::new(concat!(env!("CARGO_MANIFEST_DIR"), "/../../samples/test_ifc4_murs.ifc")), &|_| {}).unwrap().0
}
fn maison() -> Projet {
    Projet::lire_json(include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/../../samples/maison_native.modele.json"))).unwrap()
}
fn commande(p: Projet, decalage: [f64; 3]) -> Commande {
    serde_json::from_value(serde_json::json!({"action": "fusionner", "projet": serde_json::to_value(&p).unwrap(), "decalage": decalage})).unwrap()
}

#[test]
fn fusion_additionne_rapproche_et_renouvelle() {
    let base = maison();
    let imp = importe();
    let (p, ids) = base.executer(commande(imp.clone(), [0.0; 3])).unwrap();
    p.verifier().unwrap();
    assert_eq!(p.murs.len(), base.murs.len() + imp.murs.len());
    assert_eq!(p.dalles.len(), base.dalles.len() + imp.dalles.len());
    assert_eq!(p.ouvertures.len(), base.ouvertures.len() + imp.ouvertures.len());
    assert_eq!(p.barres.len(), base.barres.len() + imp.barres.len());
    assert_eq!(p.importes.len(), base.importes.len() + imp.importes.len());
    assert_eq!(p.pieces.len(), base.pieces.len() + imp.pieces.len());
    assert!(!ids.is_empty());
    // niveaux rapprochés : ceux de même altitude ne sont pas ajoutés
    let rapproches = imp.niveaux.iter().filter(|n| base.niveaux.iter().any(|b| (b.altitude - n.altitude).abs() <= 10.0 || b.nom == n.nom)).count();
    assert!(rapproches > 0, "l'échantillon doit avoir un niveau à 0");
    assert_eq!(p.niveaux.len(), base.niveaux.len() + imp.niveaux.len() - rapproches);
    let niveau0 = &base.niveaux.iter().find(|n| n.altitude == 0.0).unwrap().id;
    let alt0 = imp.niveaux.iter().find(|n| n.altitude.abs() <= 10.0).map(|n| n.id.clone());
    if let Some(a) = alt0 {
        let murs0 = imp.murs.iter().filter(|m| m.niveau == a).count();
        assert_eq!(p.murs.iter().filter(|m| &m.niveau == niveau0).count(), base.murs.iter().filter(|m| &m.niveau == niveau0).count() + murs0);
    }
    // identifiants uniques et aucun ancien identifiant importé repris
    let tous: Vec<String> = p.niveaux.iter().map(|x| x.id.clone()).chain(p.murs.iter().map(|x| x.id.clone())).chain(p.dalles.iter().map(|x| x.id.clone()))
        .chain(p.ouvertures.iter().map(|x| x.id.clone())).chain(p.importes.iter().map(|x| x.id.clone())).chain(p.barres.iter().map(|x| x.id.clone()))
        .chain(p.assemblages.iter().map(|x| x.id.clone())).chain(p.types_murs.iter().map(|x| x.id.clone())).chain(p.types_dalles.iter().map(|x| x.id.clone())).collect();
    assert_eq!(tous.len(), tous.iter().collect::<HashSet<_>>().len());
    let anciens: HashSet<String> = imp.identifiants();
    for id in &ids { assert!(!anciens.contains(id), "{id}"); }
    // références : hôtes des ouvertures, types des murs
    let connus = p.identifiants();
    for o in &p.ouvertures { assert!(connus.contains(&o.mur), "hôte absent : {}", o.mur); }
    for m in &p.murs { assert!(p.type_mur(&m.type_mur).is_some()); assert!(p.niveaux.iter().any(|n| n.id == m.niveau)); }
    // une seule étape d'annulation
    let mut h = historique::Historique::default();
    h.pousser(&base, &p).unwrap();
    assert_eq!(h.libelles(false).len(), 1);
    let retour: Projet = h.cible(&p, false).unwrap();
    assert_eq!(retour.murs.len(), base.murs.len());
    // seconde fusion : types identiques réutilisés, aucun nouveau type
    let (q, _) = p.executer(commande(imp.clone(), [20000.0, 0.0, 0.0])).unwrap();
    assert_eq!(q.types_murs.len(), p.types_murs.len());
    assert_eq!(q.niveaux.len(), p.niveaux.len());
    assert_eq!(q.murs.len(), p.murs.len() + imp.murs.len());
}

#[test]
fn fusion_types_homonymes_renommes() {
    let base = maison();
    let mut imp = importe();
    if let Some(t) = imp.types_murs.first_mut() { t.nom = base.types_murs[0].nom.clone(); } else { return; }
    let (p, _) = base.executer(commande(imp, [0.0; 3])).unwrap();
    assert!(p.types_murs.iter().any(|t| t.nom == format!("{} (import)", base.types_murs[0].nom)));
}

#[test]
fn fusion_refusee_si_import_invalide() {
    let base = maison();
    let mut imp = importe();
    let mut mur = imp.murs.first().cloned().expect("mur dans l'échantillon");
    mur.type_mur = "type-absent".into();
    imp.murs.push(mur);
    let e = base.executer(commande(imp, [0.0; 3])).unwrap_err();
    assert!(e.contains("non valide"), "{e}");
}
