//! Espaces IFC convertis en pièces natives (`samples/test_ifc4_espaces.ifc`) : séjour
//! rectangulaire et bureau en L fermés par des murs ; placard d'emprise trop petite et
//! terrasse hors des murs restés éléments importés.
use echanges::{depuis_ifc, export_ifc};
use ifc_base::semantique::IfcModel;
use ifc_base::step::StepFile;
use modele::{Piece, Projet};
use std::path::Path;

const ECHANTILLON: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../samples/test_ifc4_espaces.ifc");

fn importer() -> Projet {
    depuis_ifc::convertir(Path::new(ECHANTILLON), &|_| {}).unwrap().0
}

fn piece<'a>(p: &'a Projet, numero: &str) -> &'a Piece {
    p.pieces.iter().find(|x| x.numero == numero).unwrap_or_else(|| panic!("pièce {numero} absente : {:?}", p.importes.iter().map(|i| (&i.nom, &i.raison)).collect::<Vec<_>>()))
}

#[test]
fn espaces_convertis_en_pieces() {
    let p = importer();
    p.verifier().unwrap();
    assert_eq!(p.murs.len(), 7, "{:?}", p.importes.iter().map(|i| &i.raison).collect::<Vec<_>>());
    assert_eq!(p.pieces.len(), 2);
    let sejour = piece(&p, "101");
    assert_eq!(sejour.nom, "Séjour");
    assert_eq!(sejour.niveau, p.niveaux[0].id);
    assert!((sejour.aire() / 1e6 - 22.04).abs() < 22.04 * 0.05, "{}", sejour.aire());
    let bureau = piece(&p, "102");
    assert_eq!(bureau.nom, "Bureau");
    assert!((bureau.aire() / 1e6 - 24.64).abs() < 24.64 * 0.05, "{}", bureau.aire());
    // point de placement dans le L (le centre de la boîte, (7000, 3000), est dans les murs)
    let [x, y] = bureau.point;
    assert!((4100.0..9900.0).contains(&x) && (100.0..5900.0).contains(&y) && !(x > 6900.0 && y > 2900.0), "{:?}", bureau.point);
    // données et origine de l'espace
    let jeu = sejour.donnees.proprietes.iter().find(|j| j.nom == "Pset_SpaceCommon").unwrap();
    assert!(jeu.proprietes.iter().any(|x| x.nom == "Reference" && x.valeur == "SEJ"));
    assert!(sejour.donnees.proprietes.iter().any(|j| j.nom == "Qto_SpaceBaseQuantities" && j.quantites));
    let o = sejour.origine.as_ref().unwrap();
    assert_eq!((o.classe.as_str(), o.guid.as_deref()), ("IFCSPACE", Some("1EspacesEssai000000008")));
    // repli : placard (aire du contour 7,84 m² contre 4 m²) et terrasse (hors des murs)
    assert_eq!(p.importes.len(), 2);
    let placard = p.importes.iter().find(|i| i.nom.starts_with("103")).unwrap();
    assert!(placard.raison.contains("écart") && placard.raison.contains("emprise"), "{}", placard.raison);
    assert!(placard.donnees.couleur.is_some_and(|c| c[3] < 255), "transparent");
    assert!(placard.donnees.proprietes.iter().any(|j| j.nom == "Pset_SpaceCommon"));
    let terrasse = p.importes.iter().find(|i| i.nom.starts_with("104")).unwrap();
    assert!(terrasse.raison.contains("aucun contour"), "{}", terrasse.raison);
    assert!(!p.ignores.keys().any(|k| k.contains("Space")), "{:?}", p.ignores);
}

/// Écriture et relecture du projet natif : données et origine de la pièce gardées ; une
/// pièce sans données ne les écrit pas ; données invalides refusées.
#[test]
fn donnees_de_piece_ecrites_et_relues() {
    let p = importer();
    let json = p.json().unwrap();
    let relu = Projet::lire_json(&json).unwrap();
    let (a, b) = (piece(&p, "101"), piece(&relu, "101"));
    assert_eq!(a.donnees, b.donnees);
    assert_eq!(a.origine.as_ref().and_then(|o| o.guid.clone()), b.origine.as_ref().and_then(|o| o.guid.clone()));
    let mut sans = relu.clone();
    sans.importes.clear();
    for x in &mut sans.pieces {
        x.donnees = Default::default();
        x.origine = None;
    }
    let json = sans.json().unwrap();
    assert!(!json.contains("Pset_SpaceCommon") && !json.contains("IFCSPACE"));
    let mut faux = relu;
    faux.pieces[0].donnees.proprietes[0].nom.clear();
    assert!(faux.verifier().is_err());
}

/// IFC -> natif -> IFC : chaque pièce redevient un IfcSpace de même GUID, avec
/// `Pset_SpaceCommon` et un seul `Qto_SpaceBaseQuantities` (recalculé).
#[test]
fn aller_retour_ifc() {
    let p = importer();
    let texte = export_ifc::exporter(&p).unwrap();
    let m = IfcModel::new(StepFile::parse(texte.as_bytes()).unwrap());
    let espaces: Vec<_> = m.products.iter().copied().filter(|&id| m.ty(id) == "IFCSPACE").collect();
    let sejour = espaces.iter().copied().find(|&id| m.global_id(id) == "1EspacesEssai000000008").expect("GUID du séjour gardé");
    let jeux = m.jeux(sejour);
    let pset = jeux.iter().find(|j| j.nom == "Pset_SpaceCommon").expect("Pset_SpaceCommon réécrit");
    assert!(pset.proprietes.iter().any(|x| x.nom == "Reference" && x.valeur == "SEJ"));
    assert_eq!(jeux.iter().filter(|j| j.nom == "Qto_SpaceBaseQuantities").count(), 1);
    let net = jeux.iter().flat_map(|j| &j.proprietes).find(|x| x.nom == "NetFloorArea").and_then(|x| x.nombre).unwrap();
    assert!((net - 22.04).abs() < 22.04 * 0.05, "{net}");
    // relu par l'import : les deux pièces reviennent
    let dossier = std::env::temp_dir().join(format!("bimedit-espaces-{}", std::process::id()));
    std::fs::create_dir_all(&dossier).unwrap();
    let f = dossier.join("espaces.ifc");
    std::fs::write(&f, &texte).unwrap();
    let relu = depuis_ifc::convertir(&f, &|_| {}).unwrap().0;
    let _ = std::fs::remove_dir_all(&dossier);
    assert_eq!(piece(&relu, "102").nom, "Bureau");
    assert!(piece(&relu, "101").donnees.proprietes.iter().any(|j| j.nom == "Pset_SpaceCommon"));
}
