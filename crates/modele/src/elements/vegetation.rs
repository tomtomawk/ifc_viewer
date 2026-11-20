//! Végétation par familles (catégorie `Vegetation`) : géométrie graphique éditable
//! (révolutions du houppier et du tronc, cercle en plan) et géométrie de rendu du style
//! Réaliste désignée par le paramètre texte `Rendu` de la famille (`arbre:feuillu`…), tirée
//! d'un générateur intégré et ajustée à l'encombrement de la géométrie graphique.
//!
//! Les trois familles ci-dessous sont les types par défaut du code : un projet sans type
//! de végétation les propose, et un type n'entre dans le projet qu'au premier placement.
use crate::elements::generateur_vegetal::{remplacement_vegetal, Essence, Remplacement, MATERIAU_ECORCE, MATERIAU_FEUILLAGE};
use crate::elements::composant::TypeComposant;
use crate::famille::modelee::*;
use glam::DVec3;

/// Références de rendu connues : identifiant (valeur du paramètre `Rendu`) et nom.
pub const REFERENCES: [(&str, &str); 3] = [("arbre:feuillu", "Arbre feuillu"), ("arbre:conifere", "Arbre conifère"), ("arbre:fastigie", "Arbre fastigié")];

/// Essence d'une référence de rendu `arbre:…`.
pub fn essence_de_reference(reference: &str) -> Option<Essence> {
    match reference.trim() {
        "arbre:feuillu" => Some(Essence::Feuillu),
        "arbre:conifere" => Some(Essence::Conifere),
        "arbre:fastigie" => Some(Essence::Fastigie),
        _ => None,
    }
}

/// Référence de rendu d'une essence.
pub fn reference_essence(e: Essence) -> &'static str {
    match e {
        Essence::Feuillu => REFERENCES[0].0,
        Essence::Conifere => REFERENCES[1].0,
        Essence::Fastigie => REFERENCES[2].0,
    }
}

/// Géométrie de rendu d'une référence connue, ajustée à l'encombrement (`min`, `max`,
/// mm) de la géométrie graphique : pied au centre du dessous, hauteur et diamètre de la
/// boîte. `graine` : même géométrie à chaque calcul pour une même occurrence. None :
/// référence inconnue (la géométrie graphique est alors rendue).
pub fn rendu_reference(reference: &str, min: DVec3, max: DVec3, graine: &str) -> Option<Remplacement> {
    let essence = essence_de_reference(reference)?;
    let pied = DVec3::new((min.x + max.x) * 0.5, (min.y + max.y) * 0.5, min.z);
    let d = (max.x - min.x).max(max.y - min.y);
    Some(remplacement_vegetal(essence, pied, max.z - min.z, d, graine))
}

/// Proportions d'une essence : hauteur du fût (fraction de la hauteur) et forme du
/// houppier (vrai : ellipse ; faux : cône).
fn forme(e: Essence) -> (f64, bool) {
    match e {
        Essence::Feuillu => (0.35, true),
        Essence::Conifere => (0.12, false),
        Essence::Fastigie => (0.1, true),
    }
}

/// Famille d'un arbre (catégorie Végétation) : Largeur = diamètre du houppier, Hauteur =
/// hauteur totale. Houppier et tronc en révolution autour de l'axe vertical, matériaux
/// « Feuillage » et « Écorce » par paramètres, cercle du houppier en plan, référence de
/// rendu de l'essence.
pub fn famille_vegetal(e: Essence) -> FamilleModelee {
    use AxePlan::*;
    let (fut, ellipse) = forme(e);
    let plan = |nom: &str, axe, fixe| PlanRef { nom: nom.into(), axe, position: 0.0, fixe, hote: None };
    let dist = |a: &str, b: &str, v: String| Contrainte::Distance { a: a.into(), b: b.into(), valeur: v, priorite: 9, ligne: None };
    let face = |plan: &str| Face { plan: plan.into(), decalage: 0.0 };
    let pt = |h: &str, v: &str| PointEsquisse { h: face(h), v: face(v) };
    // hauteurs relatives dans le houppier (du fût au sommet)
    let z = |t: f64| format!("{HAUTEUR}*{:.4}", fut + (1.0 - fut) * t);
    let mut f = FamilleModelee::vide(CategorieFamille::Vegetation);
    f.plans = vec![
        plan("Centre", X, true), plan("Gauche", X, false), plan("Droite", X, false),
        plan("R50", X, false), plan("R87", X, false), plan("Tronc", X, false),
        plan("Milieu", Y, true),
        PlanRef { nom: "Extérieur".into(), axe: Y, position: 0.0, fixe: false, hote: Some(Hote::Exterieur) },
        PlanRef { nom: "Intérieur".into(), axe: Y, position: 0.0, fixe: false, hote: Some(Hote::Interieur) },
        plan("Bas", Z, true), plan("Haut", Z, false), plan("Fut", Z, false),
        plan("Z07", Z, false), plan("Z25", Z, false), plan("Z50", Z, false), plan("Z75", Z, false), plan("Z93", Z, false),
    ];
    f.contraintes = vec![
        Contrainte::Egalite { a: "Gauche".into(), m: "Centre".into(), b: "Droite".into(), priorite: 9, ligne: None },
        dist("Gauche", "Droite", LARGEUR.into()),
        dist("Centre", "R50", format!("{LARGEUR}*0.25")),
        dist("Centre", "R87", format!("{LARGEUR}*0.433")),
        dist("Centre", "Tronc", format!("{LARGEUR}*0.035")),
        dist("Bas", "Haut", HAUTEUR.into()),
        dist("Bas", "Fut", z(0.0)),
        dist("Bas", "Z07", z(0.067)), dist("Bas", "Z25", z(0.25)), dist("Bas", "Z50", z(0.5)),
        dist("Bas", "Z75", z(0.75)), dist("Bas", "Z93", z(0.933)),
    ];
    f.parametres = vec![
        Parametre { nom: PARAMETRE_RENDU.into(), formule: reference_essence(e).into(), portee: Portee::Type, genre: GenreParametre::Texte },
        Parametre { nom: MATERIAU_FEUILLAGE.into(), formule: MATERIAU_FEUILLAGE.into(), portee: Portee::Type, genre: GenreParametre::Materiau },
        Parametre { nom: MATERIAU_ECORCE.into(), formule: MATERIAU_ECORCE.into(), portee: Portee::Type, genre: GenreParametre::Materiau },
    ];
    // demi-profils (côté droit de l'axe), plan d'élévation : h = X, v = Z
    let houppier = if ellipse {
        vec![pt("Centre", "Haut"), pt("R50", "Z93"), pt("R87", "Z75"), pt("Droite", "Z50"), pt("R87", "Z25"), pt("R50", "Z07"), pt("Centre", "Fut")]
    } else {
        vec![pt("Centre", "Haut"), pt("Droite", "Fut"), pt("Centre", "Fut")]
    };
    let tronc = vec![pt("Centre", "Bas"), pt("Tronc", "Bas"), pt("Tronc", "Z50"), pt("Centre", "Z50")];
    // en plan, le cercle du houppier remplace la vue des solides
    let mut vis = VisibiliteElement::default();
    vis.plan = false;
    let forme_rev = |nom: &str, profil, materiau: &str| FormeProfil {
        operation: OperationForme::Revolution { revolution: Revolution { axe: [[0.0, 0.0], [0.0, 100.0]], debut: 0.0, angle: 360.0 } },
        support: None,
        nom: nom.into(),
        vide: false,
        materiau: Materiau::Menuiserie,
        vue: VueEsquisse::Elevation,
        profil,
        profondeur: [face("Intérieur"), face("Extérieur")],
        visible: None,
        visibilite: vis.clone(),
        materiau_parametre: Some(materiau.into()),
    };
    f.extrusions = vec![forme_rev("Houppier", houppier, MATERIAU_FEUILLAGE), forme_rev("Tronc", tronc, MATERIAU_ECORCE)];
    // cercle du houppier : quatre quarts d'arc ancrés aux plans (courbure tan(90°/4))
    let mut cercle = Element2DFamille::polyligne(VueEsquisse::Plan, vec![pt("Droite", "Milieu"), pt("Centre", "Extérieur"), pt("Gauche", "Milieu"), pt("Centre", "Intérieur")], true);
    if let crate::documentation::AnnotationForme::Polyligne { courbures, .. } = &mut cercle.forme {
        *courbures = vec![std::f64::consts::FRAC_PI_8.tan(); 4];
    }
    f.elements_2d = vec![cercle];
    f.couleur = e.couleur();
    f
}

/// Type par défaut (code) d'une essence : dimensions usuelles, famille `famille_vegetal`.
pub fn type_vegetal(e: Essence) -> TypeComposant {
    let (h, d) = e.dimensions();
    let id = format!("type-vegetal-{}", reference_essence(e).trim_start_matches("arbre:"));
    TypeComposant { id, nom: format!("Arbre {}", e.nom().to_lowercase()), largeur: d, profondeur: d, hauteur: h, modele: famille_vegetal(e) }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Familles valides, solides de révolution non vides à la taille du type, rendu de la
    /// référence ajusté à l'encombrement, références inconnues refusées.
    #[test]
    fn familles_vegetales() {
        for e in Essence::TOUTES {
            let t = type_vegetal(e);
            t.modele.valider(t.largeur, t.hauteur, t.profondeur).unwrap();
            assert_eq!(t.modele.reference_rendu(), Some(reference_essence(e)));
            let r = t.modele.resoudre(t.largeur, t.hauteur, t.profondeur);
            assert!(r.erreurs.is_empty(), "{:?}", r.erreurs);
            let tris: Vec<[DVec3; 3]> = t.modele.solides(&r).into_iter().flat_map(|s| s.tris).collect();
            assert!(!tris.is_empty());
            let (min, max) = tris.iter().flatten().fold((DVec3::splat(f64::MAX), DVec3::splat(f64::MIN)), |(a, b), p| (a.min(*p), b.max(*p)));
            assert!((max.z - min.z - t.hauteur).abs() < 1.0, "{e:?} hauteur {}", max.z - min.z);
            assert!((max.x - min.x - t.largeur).abs() < t.largeur * 0.02, "{e:?} houppier {}", max.x - min.x);
            let rendu = rendu_reference(t.modele.reference_rendu().unwrap(), min, max, "a").unwrap();
            assert!(!rendu.feuillage.is_empty() && !rendu.bois.is_empty());
            // même géométrie pour la même graine
            assert_eq!(rendu_reference(reference_essence(e), min, max, "a").unwrap().feuillage, rendu.feuillage);
        }
        assert!(rendu_reference("modele:inconnu", DVec3::ZERO, DVec3::ONE, "a").is_none());
        let mut f = famille_vegetal(Essence::Feuillu);
        f.parametres[0].portee = Portee::Occurrence;
        assert!(f.valider(7000.0, 10000.0, 7000.0).is_err(), "texte d’occurrence refusé");
    }
}
