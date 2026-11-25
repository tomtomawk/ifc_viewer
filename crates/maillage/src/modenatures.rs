//! Profilés de mur dans la scène : un relief est un objet à part, du matériau de son
//! type ; un creux, déjà retiré du mur, garde un volume invisible sélectionnable dans
//! les vues 3D (ni coupé ni vu dans les plans et les coupes).
use crate::materiaux::{objet_materiau, Tables};
use formats::ifc::geom::{Part, ProductGeom};
use ifc_base::semantique::IfcModel;
use modele::identifiants::id_natif;
use modele::CategorieMateriau as Cat;
use modele::Projet;

pub(crate) fn ajouter(projet: &Projet, model: &mut IfcModel, geoms: &mut Vec<ProductGeom>, t: &mut Tables) -> Result<(), String> {
    for m in &projet.modenatures {
        let ty = projet.type_modenature(&m.type_modenature).ok_or("Type de profilé absent.")?;
        let id = id_natif("modenature", &m.id);
        let niveau = projet.assemblage(&m.mur).and_then(|a| a.couches.first()).map_or(m.mur.as_str(), String::as_str);
        if let Some(mur) = projet.murs.iter().find(|x| x.id == niveau) {
            let parent = id_natif("niveau", &mur.niveau);
            model.parent.insert(id, parent);
            model.children.entry(parent).or_default().push(id);
        }
        let tris: Vec<_> = modele::elements::modenature::solide(projet, m)?.into_iter().map(|t| t.map(|v| v * 0.001)).collect();
        if ty.creux {
            t.materiaux_parts.push(vec![None]);
            let masque = modele::famille::modelee::VisibiliteElement { plan: false, coupe: false, ..Default::default() }.masque();
            geoms.push(ProductGeom { visibilite: vec![masque], id, parts: vec![Part { color: [0.5, 0.5, 0.5, 0.0], tris }], lignes: Vec::new(), rendu: Vec::new(), motifs: Vec::new() });
        } else {
            let materiau = projet.materiau_resolu(ty.materiau.as_deref(), Cat::Murs);
            geoms.push(objet_materiau(projet, id, tris, materiau.as_ref(), [0.80, 0.78, 0.74, 1.0], projet.trait_coupe(None, Cat::Murs), t));
        }
    }
    Ok(())
}
