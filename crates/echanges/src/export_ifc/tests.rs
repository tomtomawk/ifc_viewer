//! Tests de l'export IFC4 : relecture, hiérarchie spatiale, types, propriétés, quantités.
use super::*;
use crate::depuis_ifc;
#[test]
fn export_ifc4_relecture_geometrie_et_identifiants() {
    let (p, _) =
        depuis_ifc::convertir(std::path::Path::new(concat!(env!("CARGO_MANIFEST_DIR"), "/../../samples/test_ifc4_murs.ifc")), &|_| {})
            .unwrap();
    let s = exporter(&p).unwrap();
    // données identiques d'un export à l'autre (l'en-tête porte l'heure)
    let donnees = |s: &str| s[s.find("DATA;").unwrap()..].to_string();
    assert_eq!(donnees(&s), donnees(&exporter(&p).unwrap()));
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

#[test]
fn entete_nom_et_horodatage() {
    let s = exporter_fichier(&Projet::nouveau("Vide"), "maison.ifc").unwrap();
    let ligne = s.lines().find(|l| l.starts_with("FILE_NAME(")).unwrap();
    let h = &ligne["FILE_NAME('maison.ifc','".len()..][..19];
    assert!(ligne.starts_with("FILE_NAME('maison.ifc','20"), "{ligne}");
    let b = h.as_bytes();
    assert!(b[4] == b'-' && b[7] == b'-' && b[10] == b'T' && b[13] == b':' && b[16] == b':', "{h}");
    assert!(exporter(&Projet::nouveau("Vide")).unwrap().contains("FILE_NAME('modele.ifc','20"));
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

#[test]
fn un_style_par_materiau_reutilise() {
    let (p, _, m) = maison();
    // Béton (murs, dalles) et Tuiles (toiture) : un style chacun
    let styles: Vec<_> = du_type(&m, "IFCSURFACESTYLE").map(|id| m.get(id).unwrap().arg(0).as_str().unwrap().to_string()).collect();
    assert_eq!(styles.len(), 2);
    for nom in ["Béton", "Tuiles"] {
        assert!(styles.iter().any(|s| s == nom), "{nom}");
    }
    // couleur du matériau sur les faces des murs, lue par ifc_base
    let beton = p.materiau("Béton").unwrap().couleur.map(|c| c as f32 / 255.0);
    let corps = |id: noyau::Id| {
        let pds = m.get(m.get(id).unwrap().arg(6).as_ref().unwrap()).unwrap();
        let rep = pds.arg(2).refs().map(|r| m.get(r).unwrap()).find(|r| r.arg(1).as_str() == Some("Body")).unwrap();
        rep.arg(3).refs().collect::<Vec<_>>()
    };
    for id in du_type(&m, "IFCWALL") {
        for f in corps(id) {
            let c = m.item_color[&f];
            assert!((0..3).all(|i| (c[i] - beton[i]).abs() < 1e-5) && c[3] == 1.0);
        }
    }
    assert_eq!(du_type(&m, "IFCSTYLEDITEM").count(), du_type(&m, "IFCTRIANGULATEDFACESET").filter(|f| m.item_color.contains_key(f)).count());
}

#[test]
fn usage_des_couches_coherent_avec_l_axe_du_mur() {
    let json = std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/../../samples/maison_native.modele.json")).unwrap();
    let mut p = Projet::lire_json(&json).unwrap();
    // mur à deux couches : enduit extérieur 20 mm puis béton 200 mm
    let t = &mut p.types_murs[0];
    let mut enduit = t.couches[0].clone();
    enduit.materiau = Some("Enduit".into());
    enduit.epaisseur = 20.0;
    t.couches.insert(0, enduit);
    let s = exporter(&p).unwrap();
    assert!(s.contains(".AXIS2.,.NEGATIVE.,110.000000,$)"));
    // dalles sans pente : usage AXIS3 au-dessus de la première couche
    let d = p.dalles.iter().find(|d| d.sorte == SorteDalle::Dalle && d.pentes.iter().flatten().all(Option::is_none)).unwrap();
    let dessus = p.niveau(&d.niveau).unwrap().altitude + d.decalage;
    assert!(s.contains(&format!(".AXIS3.,.NEGATIVE.,{dessus:.6},$)")));
    // relecture par l'import : chaque mur redevient un assemblage dont la couche
    // extérieure est à gauche de l'axe d'origine, à 100 mm du milieu
    let chemin = std::env::temp_dir().join(format!("bimedit-usage-{}.ifc", std::process::id()));
    std::fs::write(&chemin, &s).unwrap();
    let (relu, _) = depuis_ifc::convertir(&chemin, &|_| {}).unwrap();
    let _ = std::fs::remove_file(&chemin);
    assert_eq!(relu.assemblages.len(), p.murs.len());
    for m in &p.murs {
        let Axe::Droit { debut, fin } = m.axe else { continue };
        let a = relu.assemblages.iter().find(|a| a.nom == m.nom).unwrap_or_else(|| panic!("{} non relu", m.nom));
        let ext = relu.couches_assemblage(&a.id)[0];
        let Axe::Droit { debut: e0, fin: e1 } = ext.axe else { panic!("axe") };
        let (dx, dy) = (fin[0] - debut[0], fin[1] - debut[1]);
        let l = dx.hypot(dy);
        let gauche = [-dy / l, dx / l];
        let milieu = [(e0[0] + e1[0]) / 2.0 - (debut[0] + fin[0]) / 2.0, (e0[1] + e1[1]) / 2.0 - (debut[1] + fin[1]) / 2.0];
        assert!((milieu[0] * gauche[0] + milieu[1] * gauche[1] - 100.0).abs() < 1.0, "{} : couche extérieure à {:?}", m.nom, milieu);
    }
}

/// Quantités brutes (valeur écrite, unités du fichier) du jeu `nom` d'un produit.
fn quantites(m: &ifc_base::semantique::IfcModel, id: noyau::Id, nom: &str) -> std::collections::BTreeMap<String, f64> {
    let jeu = m.property_defs[&id].iter().map(|&d| m.get(d).unwrap()).find(|e| e.is("IFCELEMENTQUANTITY") && e.arg(2).as_str() == Some(nom)).unwrap_or_else(|| panic!("{nom} absent"));
    jeu.arg(5).refs().map(|q| m.get(q).unwrap()).map(|q| (q.arg(0).as_str().unwrap().to_string(), q.arg(3).as_f64().unwrap())).collect()
}

/// Lignes de la nomenclature par élément (colonnes choisies), indexées par nom d'élément.
fn nomenclature(p: &Projet, categorie: &str, colonnes: Vec<documentation::Colonne>) -> HashMap<String, Vec<String>> {
    let n = documentation::Nomenclature { id: "q".into(), nom: "Q".into(), categorie: categorie.into(), regrouper: false, colonnes, parametres: Vec::new() };
    let lignes = documentation::tableau(p, &n).unwrap();
    let par_nom: HashMap<_, _> = lignes[1..].iter().map(|l| (l[0].clone(), l[1..].to_vec())).collect();
    assert_eq!(par_nom.len(), lignes.len() - 1, "noms d'éléments en double");
    par_nom
}

#[test]
fn quantites_egales_a_la_nomenclature() {
    use documentation::Colonne::*;
    let (p, s, m) = maison();
    assert!(s.contains("IFCSIUNIT(*,.AREAUNIT.,$,.SQUARE_METRE.)") && s.contains("IFCSIUNIT(*,.VOLUMEUNIT.,$,.CUBIC_METRE.)"));
    // mêmes arrondis que la nomenclature : mm entiers, m² à 2 décimales, m³ à 3
    let (mm, m2, m3) = (|v: f64| format!("{v:.0}"), |v: f64| format!("{v:.2}"), |v: f64| format!("{v:.3}"));
    let murs = nomenclature(&p, "Murs", vec![Element, Longueur, Hauteur, Epaisseur, Surface, Volume]);
    assert_eq!(du_type(&m, "IFCWALL").count(), murs.len());
    for id in du_type(&m, "IFCWALL") {
        let q = quantites(&m, id, "Qto_WallBaseQuantities");
        let attendu = &murs[&m.name(id)];
        assert_eq!(vec![mm(q["Length"]), mm(q["Height"]), mm(q["Width"]), m2(q["GrossSideArea"]), m3(q["NetVolume"])], *attendu);
    }
    let dalles = nomenclature(&p, "", vec![Element, Epaisseur, Surface, Volume]);
    for id in du_type(&m, "IFCSLAB") {
        let q = quantites(&m, id, "Qto_SlabBaseQuantities");
        assert_eq!(vec![mm(q["Width"]), m2(q["NetArea"]), m3(q["NetVolume"])], dalles[&m.name(id)]);
    }
    let toitures: Vec<_> = du_type(&m, "IFCROOF").collect();
    assert!(!toitures.is_empty());
    for id in toitures {
        assert_eq!(m2(quantites(&m, id, "Qto_RoofBaseQuantities")["ProjectedArea"]), dalles[&m.name(id)][1]);
    }
    let baies = nomenclature(&p, "", vec![Element, Hauteur, Largeur, Surface]);
    for (classe, jeu) in [("IFCDOOR", "Qto_DoorBaseQuantities"), ("IFCWINDOW", "Qto_WindowBaseQuantities")] {
        for id in du_type(&m, classe) {
            let q = quantites(&m, id, jeu);
            assert_eq!(vec![mm(q["Height"]), mm(q["Width"]), m2(q["Area"])], baies[&m.name(id)]);
        }
    }
}
