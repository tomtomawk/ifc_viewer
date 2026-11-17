//! Tests de l'export IFC4 : relecture, hiérarchie spatiale, types, propriétés, quantités.
use super::*;
use crate::depuis_ifc;
#[test]
fn export_ifc4_relecture_geometrie_et_identifiants() {
    let (p, _) =
        depuis_ifc::convertir(std::path::Path::new(concat!(env!("CARGO_MANIFEST_DIR"), "/../../samples/test_ifc4_murs.ifc")), &|_| {})
            .unwrap();
    let s = exporter(&p).unwrap();
    assert_eq!(s, exporter(&p).unwrap());
    let m = ifc_base::semantique::IfcModel::new(ifc_base::step::StepFile::parse(s.as_bytes()).unwrap());
    assert_eq!(m.file.schema, "IFC4");
    assert!(!m.products.is_empty());
    let stats = formats::ifc::geom::GeomStats::default();
    let count = m
        .products
        .iter()
        .filter(|&&id| formats::ifc::geom::product_geometry(&m, &stats, id).is_some())
        .count();
    assert!(count >= p.murs.len() + p.dalles.len() + p.importes.len());
}

/// Projet natif de démonstration et son export relu par `ifc_base`.
fn maison() -> (Projet, String, ifc_base::semantique::IfcModel) {
    let json = std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/../../samples/maison_native.modele.json")).unwrap();
    let p = Projet::lire_json(&json).unwrap();
    let s = exporter(&p).unwrap();
    let m = ifc_base::semantique::IfcModel::new(ifc_base::step::StepFile::parse(s.as_bytes()).expect("relecture STEP"));
    (p, s, m)
}

fn du_type<'a>(m: &'a ifc_base::semantique::IfcModel, ty: &'a str) -> impl Iterator<Item = noyau::Id> + 'a {
    let mut ids: Vec<_> = m.file.entities.iter().filter(|(_, e)| e.is(ty)).map(|(&id, _)| id).collect();
    ids.sort_unstable();
    ids.into_iter()
}

#[test]
fn hierarchie_projet_site_batiment_niveaux() {
    let (p, _, m) = maison();
    let sites: Vec<_> = du_type(&m, "IFCSITE").collect();
    assert_eq!(sites.len(), 1);
    let bat = du_type(&m, "IFCBUILDING").next().unwrap();
    assert_eq!(m.parent.get(&sites[0]).copied(), m.project);
    assert_eq!(m.parent.get(&bat), Some(&sites[0]));
    let niveaux: Vec<_> = du_type(&m, "IFCBUILDINGSTOREY").collect();
    assert_eq!(niveaux.len(), p.niveaux.len());
    assert!(niveaux.iter().all(|n| m.parent.get(n) == Some(&bat)));
    // les murs sont dans un niveau
    assert!(du_type(&m, "IFCWALL").all(|w| m.parent.get(&w).is_some_and(|n| niveaux.contains(n))));
}

#[test]
fn terrain_et_limite_dans_le_site() {
    use modele::elements::site::{LimitePropriete, Terrain};
    let mut p = Projet::nouveau("Site");
    p.terrains.push(Terrain { id: "t".into(), nom: "Terrain".into(), contour: vec![[0., 0., 0.], [1000., 0., 100.], [1000., 1000., 200.], [0., 1000., 0.]], points: vec![], type_terrain: None });
    p.limites_propriete.push(LimitePropriete { id: "l".into(), nom: "Parcelle".into(), altitude: 0., contour: vec![[0., 0.], [1000., 0.], [1000., 1000.]] });
    p.lieu = Some(modele::soleil::Lieu { nom: "Rennes".into(), latitude: 48.1173, longitude: -1.6778, fuseau: 1.0, heure_ete: true });
    p.coordonnees_origine = [0.0, 0.0, 35_000.0];
    p.orientation_nord = std::f64::consts::FRAC_PI_2;
    let s = exporter(&p).unwrap();
    assert!(s.contains(".ELEMENT.,(48,7,2,280000),(-1,-40,-40,-80000),35000.000000,$,$)"));
    assert!(s.contains("IFCDIRECTION((-1.000000,0.000000))"));
    let m = ifc_base::semantique::IfcModel::new(ifc_base::step::StepFile::parse(s.as_bytes()).unwrap());
    let site = du_type(&m, "IFCSITE").next().unwrap();
    assert_eq!(m.parent.get(&du_type(&m, "IFCGEOGRAPHICELEMENT").next().unwrap()), Some(&site));
    assert_eq!(m.parent.get(&du_type(&m, "IFCANNOTATION").next().unwrap()), Some(&site));
}

#[test]
fn un_type_ifc_par_type_utilise() {
    let json = std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/../../samples/maison_native.modele.json")).unwrap();
    let mut p = Projet::lire_json(&json).unwrap();
    // une porte typée, une fenêtre sans type
    p.types_ouvertures.push(TypeOuverture { id: "porte90".into(), nom: "Porte 90".into(), genre: GenreOuverture::Porte, largeur: 900.0, hauteur: 2100.0, famille: None, modele: None, origine: None, famille_nom: None });
    let porte = p.ouvertures.iter_mut().find(|o| o.genre == GenreOuverture::Porte).unwrap();
    porte.type_ouverture = Some("porte90".into());
    let s = exporter(&p).unwrap();
    let m = ifc_base::semantique::IfcModel::new(ifc_base::step::StepFile::parse(s.as_bytes()).unwrap());
    let types_murs: std::collections::BTreeSet<_> = p.murs.iter().map(|m| &m.type_mur).collect();
    assert_eq!(du_type(&m, "IFCWALLTYPE").count(), types_murs.len());
    assert_eq!(du_type(&m, "IFCDOORTYPE").count(), 1);
    assert_eq!(du_type(&m, "IFCWINDOWTYPE").count(), 0);
    let dalles: std::collections::BTreeSet<_> = p.dalles.iter().map(|d| (d.sorte == SorteDalle::Toiture, &d.type_dalle)).collect();
    assert_eq!(du_type(&m, "IFCSLABTYPE").count() + du_type(&m, "IFCROOFTYPE").count() + du_type(&m, "IFCCOVERINGTYPE").count(), dalles.len());
    // chaque mur, dalle et porte est typé, sous la classe de type qui lui correspond
    for (classe, classe_type) in [("IFCWALL", "IFCWALLTYPE"), ("IFCSLAB", "IFCSLABTYPE"), ("IFCROOF", "IFCROOFTYPE"), ("IFCDOOR", "IFCDOORTYPE")] {
        for id in du_type(&m, classe) {
            let t = m.type_of.get(&id).unwrap_or_else(|| panic!("{classe} #{id} sans type"));
            assert_eq!(m.ty(*t), classe_type);
        }
    }
    assert_eq!(m.name(du_type(&m, "IFCDOORTYPE").next().unwrap()), "Porte 90");
    // un seul IfcRelDefinesByType par type
    assert_eq!(du_type(&m, "IFCRELDEFINESBYTYPE").count(), types_murs.len() + dalles.len() + 1);
}

/// Groupe de propriétés ou de quantités `nom` d'un produit : (propriété, valeur affichée).
fn groupe(m: &ifc_base::semantique::IfcModel, id: noyau::Id, nom: &str) -> Option<Vec<(String, String)>> {
    m.property_groups(id).into_iter().find(|g| g.name == nom).map(|g| g.props)
}

#[test]
fn jeux_de_proprietes_communs() {
    let (p, _, m) = maison();
    for id in du_type(&m, "IFCWALL") {
        let mur = p.murs.iter().find(|x| x.nom == m.name(id)).unwrap();
        let props = groupe(&m, id, "Pset_WallCommon").expect("Pset_WallCommon");
        assert_eq!(props, vec![("Reference".to_string(), p.type_mur(&mur.type_mur).unwrap().nom.clone())]);
    }
    for (classe, jeu) in [("IFCSLAB", "Pset_SlabCommon"), ("IFCROOF", "Pset_RoofCommon")] {
        for id in du_type(&m, classe) {
            let d = p.dalles.iter().find(|x| x.nom == m.name(id)).unwrap();
            assert_eq!(groupe(&m, id, jeu).expect(jeu)[0].1, p.type_dalle(&d.type_dalle).unwrap().nom);
        }
    }
    // ouvertures sans type : aucune propriété connue, pas de jeu
    assert!(du_type(&m, "IFCDOOR").all(|id| groupe(&m, id, "Pset_DoorCommon").is_none()));
    assert!(!du_type(&m, "IFCPROPERTYSINGLEVALUE").any(|id| m.get(id).unwrap().arg(0).as_str() == Some("IsExternal")));
}
