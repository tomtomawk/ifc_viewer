//! Matériaux à couches : `IfcMaterialLayerSet` par type de mur ou de dalle,
//! `IfcMaterialLayerSetUsage` des occurrences, et axe des murs (ligne de référence).
use super::step::{reel, refs, texte, Step};
use modele::*;
use std::collections::HashMap;

/// Jeux de couches des types (par identifiant de type) et usages des types de murs.
pub(super) struct Couches {
    jeux: HashMap<String, usize>,
    usages_murs: HashMap<String, usize>,
}

impl Couches {
    /// Un `IfcMaterial` par nom de matériau, un `IfcMaterialLayerSet` par type de mur et
    /// de dalle (couches dans l'ordre du type : de l'extérieur vers l'intérieur pour un
    /// mur, du dessus vers le dessous pour une dalle).
    pub(super) fn new(w: &mut Step, p: &Projet) -> Self {
        let mut materiaux: HashMap<&str, usize> = HashMap::new();
        let mut jeux = HashMap::new();
        for t in p.types_murs.iter().chain(&p.types_dalles) {
            let mut cs = Vec::new();
            for c in &t.couches {
                let nom = c.materiau.as_deref().unwrap_or("Matériau");
                let mat = *materiaux.entry(nom).or_insert_with(|| w.add(format!("IFCMATERIAL({},$,$)", texte(nom))));
                cs.push(w.add(format!("IFCMATERIALLAYER(#{mat},{},$,$,$,$,$)", reel(c.epaisseur))));
            }
            jeux.insert(t.id.clone(), w.add(format!("IFCMATERIALLAYERSET({},{},$)", refs(&cs), texte(&t.nom))));
        }
        Couches { jeux, usages_murs: HashMap::new() }
    }

    pub(super) fn jeu(&self, type_id: &str) -> usize {
        self.jeux[type_id]
    }

    /// Usage du jeu d'un type de mur, partagé par ses murs. L'axe du mur (représentation
    /// Axis) est le milieu de l'épaisseur ; la première couche (extérieure) est à gauche
    /// de l'axe parcouru du début vers la fin, soit +Y local : la base du jeu est à +e/2 et
    /// les couches s'empilent vers -Y (NEGATIVE).
    pub(super) fn usage_mur(&mut self, w: &mut Step, t: &TypeCompose) -> usize {
        if let Some(&u) = self.usages_murs.get(&t.id) {
            return u;
        }
        let u = w.add(format!("IFCMATERIALLAYERSETUSAGE(#{},.AXIS2.,.NEGATIVE.,{},$)", self.jeux[&t.id], reel(t.epaisseur() / 2.0)));
        self.usages_murs.insert(t.id.clone(), u);
        u
    }

    /// Matériau d'une dalle, sol ou plafond sans pente : usage du jeu, base au-dessus de
    /// la première couche (cote `haut`, mm, plan XY du placement à l'origine), couches
    /// empilées vers le bas (NEGATIVE). Toiture ou dalle en pente : le jeu seul (pas de
    /// plan de référence unique).
    pub(super) fn materiau_dalle(&self, w: &mut Step, d: &Dalle, classe: &str, haut: f64) -> usize {
        let plane = d.pentes.iter().flatten().all(Option::is_none);
        let jeu = self.jeux[&d.type_dalle];
        if classe == "IFCROOF" || !plane {
            return jeu;
        }
        w.add(format!("IFCMATERIALLAYERSETUSAGE(#{jeu},.AXIS3.,.NEGATIVE.,{},$)", reel(haut)))
    }
}

/// Représentation « Axis » d'un mur : son axe dans le plan (mm), arc facetté.
pub(super) fn axe_mur(w: &mut Step, contexte_axe: usize, axe: &Axe) -> usize {
    let points: Vec<[f64; 2]> = match *axe {
        Axe::Droit { debut, fin } => vec![debut, fin],
        Axe::Arc { debut, fin, centre, direct } => {
            let r = (debut[0] - centre[0]).hypot(debut[1] - centre[1]);
            let a0 = (debut[1] - centre[1]).atan2(debut[0] - centre[0]);
            let a1 = (fin[1] - centre[1]).atan2(fin[0] - centre[0]);
            let tour = std::f64::consts::TAU;
            let balayage = if direct { (a1 - a0).rem_euclid(tour) } else { -(a0 - a1).rem_euclid(tour) };
            let n = ((balayage.abs() / (tour / 64.0)).ceil() as usize).clamp(2, 64);
            (0..=n).map(|i| a0 + balayage * i as f64 / n as f64).map(|a| [centre[0] + r * a.cos(), centre[1] + r * a.sin()]).collect()
        }
    };
    let ids: Vec<usize> = points.iter().map(|q| w.add(format!("IFCCARTESIANPOINT(({},{}))", reel(q[0]), reel(q[1])))).collect();
    let ligne = w.add(format!("IFCPOLYLINE({})", refs(&ids)));
    w.add(format!("IFCSHAPEREPRESENTATION(#{contexte_axe},'Axis','Curve2D',(#{ligne}))"))
}
