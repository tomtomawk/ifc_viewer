//! Styles d'objets par catégorie (E1) appliqués aux objets de la scène : épaisseur des
//! contours coupés sans bande de matériau (ouvertures, composants, garde-corps) et
//! épaisseur et tirets des lignes vues en projection.
use modele::identifiants::id_natif;
use modele::{CategorieMateriau as Cat, Projet};
use ifc_base::step::IdMap;
use scene::TraitVu;

/// Tables de la scène : contour coupé (mm du papier) et lignes vues, par objet.
pub(crate) fn traits(projet: &Projet) -> (IdMap<f64>, IdMap<TraitVu>) {
    let (mut coupe, mut vus) = (IdMap::default(), IdMap::default());
    let mut ajouter = |id: u64, cat: Cat| {
        if projet.styles_categories.get(&cat).is_some_and(|s| s.plume_coupe.is_some()) {
            coupe.insert(id, projet.trait_coupe(None, cat));
        }
        if let Some((epaisseur, tirets)) = projet.trait_projection(cat) {
            vus.insert(id, TraitVu { epaisseur, tirets });
        }
    };
    for m in &projet.murs { ajouter(id_natif("mur", &m.id), Cat::Murs); }
    for d in &projet.dalles {
        ajouter(id_natif("dalle", &d.id), Cat::de_dalle(d.sorte));
        ajouter(id_natif("remblai", &d.id), Cat::TerrePleins);
    }
    for o in &projet.ouvertures { ajouter(id_natif("ouverture", &o.id), Cat::de_ouverture(o.genre)); }
    for b in &projet.barres { ajouter(id_natif("barre", &b.id), Cat::de_barre(b.genre)); }
    for e in &projet.escaliers { ajouter(id_natif("escalier", &e.id), Cat::Escaliers); }
    for g in &projet.garde_corps { ajouter(id_natif("garde_corps", &g.id), Cat::GardeCorps); }
    for c in &projet.composants { ajouter(id_natif("composant", &c.id), Cat::Composants); }
    for t in &projet.terrains { ajouter(id_natif("terrain", &t.id), Cat::Terrain); }
    (coupe, vus)
}
