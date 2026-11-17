//! Import IFC qui garde les données : propriétés, quantités, matériau et couleur des
//! éléments importés (`samples/test_ifc4_proprietes.ifc`).
use echanges::depuis_ifc;
use modele::{ElementImporte, Projet};
use std::path::Path;

const ECHANTILLON: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../samples/test_ifc4_proprietes.ifc");

fn importer() -> Projet {
    depuis_ifc::convertir(Path::new(ECHANTILLON), &|_| {}).unwrap().0
}

fn importe<'a>(p: &'a Projet, nom: &str) -> &'a ElementImporte {
    p.importes.iter().find(|e| e.nom == nom).unwrap_or_else(|| panic!("{nom} non importé"))
}

/// (jeu, propriété) -> (valeur, unité)
fn valeurs(e: &ElementImporte) -> Vec<(String, String, String, Option<String>)> {
    e.donnees.proprietes.iter().flat_map(|j| j.proprietes.iter().map(|p| (j.nom.clone(), p.nom.clone(), p.valeur.clone(), p.unite.clone()))).collect()
}

#[test]
fn proprietes_materiau_et_couleur_gardes() {
    let p = importer();
    p.verifier().unwrap();
    let bloc = importe(&p, "Bloc");
    let s = |x: &str| x.to_string();
    assert_eq!(
        valeurs(bloc),
        vec![
            (s("Pset_Essai"), s("Reference"), s("B-01"), None),
            (s("Pset_Essai"), s("Commentaire"), s("Bloc d'essai"), None),
            (s("Pset_Essai"), s("Masse"), s("412.5"), Some(s("kg"))),
            (s("Pset_Essai"), s("Largeur"), s("1"), Some(s("m"))),
            (s("Pset_Essai"), s("Nombre"), s("3"), None),
            // m² et m³ explicites : valeurs du fichier, pas divisées par 10⁶ et 10⁹
            (s("Qto_Bloc"), s("Height"), s("1"), Some(s("m"))),
            (s("Qto_Bloc"), s("BaseArea"), s("0.5"), Some(s("m²"))),
            (s("Qto_Bloc"), s("NetVolume"), s("0.1667"), Some(s("m³"))),
        ]
    );
    assert!(!bloc.donnees.proprietes[0].quantites && bloc.donnees.proprietes[1].quantites);
    assert_eq!(bloc.donnees.materiau.as_deref(), Some("Acier"));
    // style de l'item (vert) prioritaire sur le matériau sans couleur
    assert_eq!(bloc.donnees.couleur, Some([51, 153, 51, 255]));
    // couleur du matériau (IfcMaterialDefinitionRepresentation)
    let table = importe(&p, "Table");
    assert_eq!(table.donnees.materiau.as_deref(), Some("Chêne"));
    assert_eq!(table.donnees.couleur, Some([153, 102, 51, 255]));
    assert_eq!(table.donnees.nombre_proprietes(), 1);
    // relu tel quel depuis le JSON natif
    let relu = Projet::lire_json(&p.json().unwrap()).unwrap();
    assert_eq!(importe(&relu, "Bloc").donnees, bloc.donnees);
}

/// Sans style ni matériau : pas de couleur (la couleur par défaut de la classe n'est pas
/// une donnée) ; échantillon historique : poteau rouge.
#[test]
fn couleur_seulement_si_lue() {
    let (p, _) = depuis_ifc::convertir(Path::new(concat!(env!("CARGO_MANIFEST_DIR"), "/../../samples/test_ifc4.ifc")), &|_| {}).unwrap();
    assert_eq!(importe(&p, "Tétraèdre").donnees.couleur, None);
    assert_eq!(importe(&p, "Poteau").donnees.couleur, Some([204, 26, 26, 255]));
}

/// Nombre de propriétés par jeu d'un produit IFC (jeux de l'objet seulement).
fn comptes(m: &ifc_base::semantique::IfcModel, id: noyau::Id) -> Vec<(String, usize)> {
    let mut v: Vec<_> = m.jeux(id).into_iter().filter(|j| !j.du_type).map(|j| (j.nom, j.proprietes.len())).collect();
    v.sort();
    v
}

/// IFC -> natif -> IFC : chaque élément importé garde le nombre de propriétés de chacun
/// de ses jeux, sa classe IFC d'origine, son matériau et sa couleur ; réimporté, il
/// retrouve les mêmes données.
#[test]
fn aller_retour_ifc_natif_ifc() {
    use ifc_base::semantique::IfcModel;
    use ifc_base::step::StepFile;
    let source = IfcModel::new(StepFile::parse(&std::fs::read(ECHANTILLON).unwrap()).unwrap());
    let p = importer();
    let texte = echanges::export_ifc::exporter(&p).unwrap();
    let export = IfcModel::new(StepFile::parse(texte.as_bytes()).unwrap());
    let par_guid = |m: &IfcModel, g: &str| m.products.iter().copied().find(|&id| m.global_id(id) == g).unwrap_or_else(|| panic!("{g} absent"));
    assert!(!p.importes.is_empty());
    for e in &p.importes {
        let guid = e.origine.as_ref().and_then(|o| o.guid.clone()).unwrap();
        let (a, b) = (par_guid(&source, &guid), par_guid(&export, &guid));
        assert_eq!(comptes(&export, b), comptes(&source, a), "{}", e.nom);
        let n: usize = comptes(&source, a).iter().map(|x| x.1).sum();
        assert_eq!(e.donnees.nombre_proprietes(), n, "{}", e.nom);
        assert_eq!(export.ty(b), source.ty(a), "{} : classe d'origine", e.nom);
    }
    // réimport de l'export : mêmes données (valeurs, unités, matériau, couleur)
    let chemin = std::env::temp_dir().join(format!("bimedit-proprietes-{}.ifc", std::process::id()));
    std::fs::write(&chemin, &texte).unwrap();
    let (relu, _) = depuis_ifc::convertir(&chemin, &|_| {}).unwrap();
    let _ = std::fs::remove_file(&chemin);
    for e in &p.importes {
        assert_eq!(importe(&relu, &e.nom).donnees, e.donnees, "{}", e.nom);
    }
}
