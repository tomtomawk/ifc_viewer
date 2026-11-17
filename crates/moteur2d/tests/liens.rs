//! Un lien IFC visible entre dans les dessins des feuilles (plans et coupes) ; masqué,
//! la feuille redevient celle du projet seul.
fn maison() -> modele::Projet {
    modele::Projet::lire_json(include_str!("../../../samples/maison_native.modele.json")).unwrap()
}

#[test]
fn feuille_avec_lien() {
    let p = maison();
    let f = p.feuilles.first().expect("feuille de l'exemple").clone();
    let seul = moteur2d::svg_feuille(&p, &f).unwrap();
    let mut q = p.clone();
    q.liens.push(modele::liens::LienIfc {
        id: "lien-1".into(),
        nom: "Essai".into(),
        chemin: concat!(env!("CARGO_MANIFEST_DIR"), "/../../samples/test_ifc4.ifc").into(),
        decalage: [0.0; 3],
        rotation: 0.0,
        visible: true,
    });
    let avec = moteur2d::svg_feuille(&q, &f).unwrap();
    assert!(avec.len() > seul.len() + 500, "le lien ajoute des tracés : {} -> {}", seul.len(), avec.len());
    q.liens[0].visible = false;
    assert_eq!(moteur2d::svg_feuille(&q, &f).unwrap().len(), seul.len());
}
