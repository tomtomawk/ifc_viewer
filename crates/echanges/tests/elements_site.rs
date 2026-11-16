//! Tests déplacés de `modele/elements/site.rs` (`mod tests`) : ils appellent du code situé au-dessus
//! de la crate `modele` (commandes, scène, formats, échanges, collaboration ou dessin).
use commandes::Execution;
use modele::elements::site::*;

#[test]
fn creation_site_sauvegarde_export_et_suppression() {
    use {commandes::Commande, modele::Projet};
    let p = Projet::nouveau("Site");
    let terrain = Terrain { id: String::new(), nom: "Terrain".into(), contour: vec![[0.,0.,0.], [1000.,0.,100.], [1000.,1000.,200.], [0.,1000.,0.]], points: vec![[500.,500.,450.]], type_terrain: None };
    let (p, t) = p.executer(Commande::Terrain { terrain }).unwrap();
    let limite = LimitePropriete { id: String::new(), nom: "Parcelle".into(), altitude: 0., contour: vec![[0.,0.], [1000.,0.], [1000.,1000.], [0.,1000.]] };
    let (p, l) = p.executer(Commande::LimitePropriete { limite }).unwrap();
    let json = p.json().unwrap();
    assert!(json.contains("limites_propriete") && json.contains("terrains"));
    let ifc = echanges::export_ifc::exporter(&p).unwrap();
    assert!(ifc.contains("IFCGEOGRAPHICELEMENT(") && ifc.contains("IFCPOLYLINE("));
    let fichier = ifc_base::step::StepFile::parse(ifc.as_bytes()).unwrap();
    let ifc_modele = ifc_base::semantique::IfcModel::new(fichier);
    let terrain_ifc = ifc_modele.products.iter().copied().find(|&id| ifc_modele.ty(id) == "IFCGEOGRAPHICELEMENT").unwrap();
    assert!(formats::ifc::geom::product_geometry(&ifc_modele, &formats::ifc::geom::GeomStats::default(), terrain_ifc).is_some());
    let scene = maillage::depuis_natif(&p, std::path::Path::new("site.json")).unwrap();
    assert!(scene.objects.iter().any(|o| o.id == modele::identifiants::id_natif("terrain", &t[0])));
    assert!(scene.objects.iter().any(|o| o.id == modele::identifiants::id_natif("limite_propriete", &l[0])));
    let (copie, ids) = p.executer(Commande::Copier { ids: vec![t[0].clone(), l[0].clone()], delta: [3000., 0., 0.] }).unwrap();
    assert_eq!(copie.terrains.len(), 2);
    assert_eq!(copie.limites_propriete.len(), 2);
    assert_eq!(ids.len(), 2);
    assert!(copie.terrains.iter().any(|x| x.contour[0][0] == 3000.));
    let (p, _) = p.executer(Commande::Supprimer { ids: vec![t[0].clone(), l[0].clone()] }).unwrap();
    assert!(p.terrains.is_empty() && p.limites_propriete.is_empty());
}
