//! Végétation par familles (catégorie `Vegetation`) : géométrie graphique en maillages de
//! famille (houppier en grappe et tronc, symbole de plan par essence :
//! `dessin_vegetal`) mis à l'échelle du type, et géométrie de rendu du style
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

/// Famille d'un arbre (catégorie Végétation) : Largeur = diamètre du houppier, Hauteur =
/// hauteur totale. Trois maillages dans la boîte du type : houppier (matériau
/// « Feuillage ») et tronc (« Écorce »), masqués en plan, et symbole de plan de l'essence ;
/// référence de rendu de l'essence.
pub fn famille_vegetal(e: Essence) -> FamilleModelee {
    use AxePlan::*;
    let plan = |nom: &str, axe, fixe| PlanRef { nom: nom.into(), axe, position: 0.0, fixe, hote: None };
    let dist = |a: &str, b: &str, v: &str| Contrainte::Distance { a: a.into(), b: b.into(), valeur: v.into(), priorite: 9, ligne: None };
    let mut f = FamilleModelee::vide(CategorieFamille::Vegetation);
    f.plans = vec![
        plan("Centre", X, true), plan("Gauche", X, false), plan("Droite", X, false),
        plan("Milieu", Y, true),
        PlanRef { nom: "Extérieur".into(), axe: Y, position: 0.0, fixe: false, hote: Some(Hote::Exterieur) },
        PlanRef { nom: "Intérieur".into(), axe: Y, position: 0.0, fixe: false, hote: Some(Hote::Interieur) },
        plan("Bas", Z, true), plan("Haut", Z, false),
    ];
    f.contraintes = vec![
        Contrainte::Egalite { a: "Gauche".into(), m: "Centre".into(), b: "Droite".into(), priorite: 9, ligne: None },
        dist("Gauche", "Droite", LARGEUR),
        dist("Bas", "Haut", HAUTEUR),
    ];
    f.parametres = vec![
        Parametre { nom: PARAMETRE_RENDU.into(), formule: reference_essence(e).into(), portee: Portee::Type, genre: GenreParametre::Texte },
        Parametre { nom: MATERIAU_FEUILLAGE.into(), formule: MATERIAU_FEUILLAGE.into(), portee: Portee::Type, genre: GenreParametre::Materiau },
        Parametre { nom: MATERIAU_ECORCE.into(), formule: MATERIAU_ECORCE.into(), portee: Portee::Type, genre: GenreParametre::Materiau },
    ];
    // en plan, le symbole remplace la vue des solides
    let sans_plan = VisibiliteElement { plan: false, ..Default::default() };
    // géométrie produite par le code : le fichier ne garde que les références
    use crate::elements::dessin_vegetal::reference;
    let genere = |nom: &str, partie: &str| MaillageFamille { nom: nom.into(), generateur: Some(reference(e, partie)), ..Default::default() };
    f.maillages = vec![
        MaillageFamille { materiau_parametre: Some(MATERIAU_FEUILLAGE.into()), visibilite: sans_plan, ..genere("Houppier", "houppier") },
        MaillageFamille { materiau_parametre: Some(MATERIAU_ECORCE.into()), visibilite: sans_plan, ..genere("Tronc", "tronc") },
        genere("Symbole", "symbole"),
    ];
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

    /// Familles valides, maillages non vides à la taille du type, rendu de la
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
            assert!(max.x - min.x <= t.largeur + 1.0 && max.x - min.x > t.largeur * 0.8, "{e:?} houppier {}", max.x - min.x);
            // symbole de plan : contour, tronc, croix
            assert_eq!(t.modele.traits_maillages_plan(&r, NiveauDetail::Moyen).len(), 4);
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
