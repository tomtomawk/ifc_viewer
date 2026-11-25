//! Profil d'un mur (élévation modifiée) : contours fermés dessinés dans le plan vertical
//! du mur, qui remplacent le rectangle base-haut.
//!
//! Un profil appartient à un mur indépendant ou à un assemblage (toutes ses couches),
//! clé de `Projet::profils_murs`. Ses points sont `[s, z]` (mm) : `s` le long de l'axe du
//! mur de référence (le mur, ou la première couche de l'assemblage) depuis son début,
//! `z` au-dessus de sa base. Le premier contour est l'extérieur ; les suivants sont des
//! vides à l'intérieur. Murs droits seulement ; un mur à profil n'est pas rattaché aux
//! toitures (le profil prime).
use crate::volumes::solides::{prisme, Tri};
use crate::{Axe, Mur, Projet};
use glam::{DVec2, DVec3};
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct ProfilMur {
    /// Contour extérieur puis vides, `[s, z]` en mm.
    pub contours: Vec<Vec<[f64; 2]>>,
}

type Booleen = fn(&[Tri], &[Tri], noyau::csg::Op, f64) -> Option<Vec<Tri>>;

impl Projet {
    /// Clé du profil d'un mur : son assemblage, ou lui-même s'il est indépendant.
    pub fn cle_profil_mur<'a>(&self, m: &'a Mur) -> Option<&'a str> {
        let cle = m.assemblage.as_deref().unwrap_or(&m.id);
        self.profils_murs.contains_key(cle).then_some(cle)
    }

    /// Mur dont l'axe porte le repère du profil `cle` (mur ou assemblage).
    pub fn reference_profil(&self, cle: &str) -> Option<&Mur> {
        let id = self.assemblage(cle).map_or(cle, |a| a.couches.first().map_or(cle, |c| c.as_str()));
        self.murs.iter().find(|m| m.id == id && (m.assemblage.is_none() || m.assemblage.as_deref() == Some(cle)))
    }

    /// Repère du profil `cle` : origine et direction en plan (début de l'axe de
    /// référence), altitude de la base.
    pub fn repere_profil(&self, cle: &str) -> Result<(DVec2, DVec2, f64), String> {
        let m = self.reference_profil(cle).ok_or("Mur du profil introuvable.")?;
        let Axe::Droit { debut, fin } = m.axe else { return Err("Seul un mur droit peut recevoir un profil.".into()) };
        let (a, b) = (DVec2::from_array(debut), DVec2::from_array(fin));
        let u = (b - a).try_normalize().ok_or("Mur de longueur nulle.")?;
        let (bas, _) = self.bas_haut(m).ok_or("Contrainte de hauteur non valide.")?;
        Ok((a, u, bas))
    }

    /// Rectangle de départ du profil d'un mur ou d'un assemblage : étendue de ses couches
    /// le long de l'axe, de la base au haut.
    pub fn profil_rectangle(&self, cle: &str) -> Result<ProfilMur, String> {
        let (o, u, base) = self.repere_profil(cle)?;
        let murs: Vec<&Mur> = match self.assemblage(cle) {
            Some(a) => a.couches.iter().filter_map(|c| self.murs.iter().find(|m| &m.id == c)).collect(),
            None => self.murs.iter().filter(|m| m.id == cle).collect(),
        };
        let (mut s0, mut s1, mut haut) = (f64::INFINITY, f64::NEG_INFINITY, f64::NEG_INFINITY);
        for m in murs {
            let Axe::Droit { debut, fin } = m.axe else { return Err("Seul un mur droit peut recevoir un profil.".into()) };
            for q in [debut, fin] {
                let s = (DVec2::from_array(q) - o).dot(u);
                (s0, s1) = (s0.min(s), s1.max(s));
            }
            haut = haut.max(self.bas_haut(m).ok_or("Contrainte de hauteur non valide.")?.1);
        }
        let h = haut - base;
        if !(s1 > s0 && h > 0.0 && h.is_finite()) { return Err("Dimensions du mur non valides.".into()); }
        Ok(ProfilMur { contours: vec![vec![[s0, 0.0], [s1, 0.0], [s1, h], [s0, h]]] })
    }

    /// Garde chaque profil à sa place quand la commande a changé la ligne du mur sans le
    /// déplacer en bloc : un mur allongé, raccourci ou retourné sur sa propre ligne garde
    /// son profil dans le projet ; un mur déplacé ou tourné l'emporte avec lui.
    pub fn recaler_profils_murs(&mut self, avant: &Projet) {
        let cles: Vec<String> = self.profils_murs.keys().cloned().collect();
        for cle in cles {
            let (Ok((a0, ua, _)), Ok((b0, ub, _))) = (avant.repere_profil(&cle), self.repere_profil(&cle)) else { continue };
            let fins = |p: &Projet| p.reference_profil(&cle).and_then(|m| match m.axe { Axe::Droit { fin, .. } => Some(DVec2::from_array(fin)), _ => None });
            let (Some(a1), Some(b1)) = (fins(avant), fins(self)) else { continue };
            let translation = (b0 - a0).distance(b1 - a1) < 1e-6;
            // même ligne, ou ligne voisine parallèle (couche de référence changée par un
            // retournement) dont l'étendue recouvre l'ancienne
            let ecart = ua.perp_dot(b0 - a0).abs();
            let epaisseur = self.assemblage(&cle).and_then(|a| self.type_mur(&a.type_mur)).map_or(0.0, |t| t.epaisseur());
            let (s0, s1) = ((b0 - a0).dot(ua), (b1 - a0).dot(ua));
            let recouvre = s0.max(s1) > 0.0 && s0.min(s1) < (a1 - a0).length();
            let meme_ligne = ua.perp_dot(ub).abs() < 1e-9 && ecart <= epaisseur + 1e-6 && recouvre;
            if translation || !meme_ligne { continue; }
            let Some(profil) = self.profils_murs.get_mut(&cle) else { continue };
            for c in &mut profil.contours {
                for q in c.iter_mut() {
                    q[0] = (a0 + ua * q[0] - b0).dot(ub);
                }
            }
        }
    }
}

/// Aire signée d'un contour.
fn aire(c: &[[f64; 2]]) -> f64 {
    (0..c.len()).map(|i| { let (a, b) = (c[i], c[(i + 1) % c.len()]); a[0] * b[1] - b[0] * a[1] }).sum::<f64>() / 2.0
}

fn dedans(c: &[[f64; 2]], q: [f64; 2]) -> bool {
    let mut d = false;
    for i in 0..c.len() {
        let (a, b) = (c[i], c[(i + 1) % c.len()]);
        if (a[1] > q[1]) != (b[1] > q[1]) && q[0] < a[0] + (q[1] - a[1]) * (b[0] - a[0]) / (b[1] - a[1]) { d = !d; }
    }
    d
}

/// Contrôle d'un profil : contours simples, vides à l'intérieur du contour extérieur.
pub fn valider_profil(profil: &ProfilMur) -> Result<(), String> {
    let (exterieur, vides) = profil.contours.split_first().ok_or("Profil de mur sans contour.")?;
    for c in &profil.contours {
        if c.iter().flatten().any(|v| !v.is_finite()) { return Err("Profil de mur : coordonnées non finies.".into()); }
        crate::famille::profil::contour_simple(c).map_err(|e| format!("Profil de mur : {e}"))?;
    }
    for v in vides {
        if !v.iter().all(|q| dedans(exterieur, *q)) { return Err("Profil de mur : un vide sort du contour extérieur.".into()); }
    }
    Ok(())
}

/// Contrôle des profils du projet (clés, murs droits, contours).
pub fn verifier(p: &Projet) -> Result<(), String> {
    for (cle, profil) in &p.profils_murs {
        let mur_seul = p.murs.iter().any(|m| &m.id == cle && m.assemblage.is_none());
        if !mur_seul && p.assemblage(cle).is_none() { return Err(format!("Profil d'un mur absent : {cle}.")); }
        p.repere_profil(cle)?;
        valider_profil(profil)?;
    }
    Ok(())
}

/// Volume d'un mur à profil : le prisme de son contour en plan (jonctions comprises),
/// coupé par le profil extrudé à travers le mur, moins les vides du profil.
pub(crate) fn volume(p: &Projet, cle: &str, contour: &[[f64; 2]], booleen: Booleen) -> Result<Vec<Tri>, String> {
    let profil = &p.profils_murs[cle];
    let (o, u, base) = p.repere_profil(cle)?;
    let n = u.perp();
    let (exterieur, vides) = profil.contours.split_first().ok_or("Profil de mur sans contour.")?;
    let (zmin, zmax) = exterieur.iter().fold((f64::INFINITY, f64::NEG_INFINITY), |(a, b), q| (a.min(q[1]), b.max(q[1])));
    // demi-largeur d'extrusion : au-delà de toute la largeur du contour en plan
    let l = contour.iter().map(|q| (DVec2::from_array(*q) - o).dot(n).abs()).fold(0.0, f64::max) + 10.0;
    let extruder = |c: &[[f64; 2]]| -> Vec<Tri> {
        let c: Vec<[f64; 2]> = if aire(c) < 0.0 { c.iter().rev().copied().collect() } else { c.to_vec() };
        prisme(&c, -l, l)
            .into_iter()
            .map(|t| {
                // (s, z, w) -> plan o + u s + n w, altitude base + z ; repère indirect :
                // l'ordre des sommets est inversé pour garder les normales vers l'extérieur
                let f = |v: DVec3| { let q = o + u * v.x + n * v.z; DVec3::new(q.x, q.y, base + v.y) };
                [f(t[0]), f(t[2]), f(t[1])]
            })
            .collect()
    };
    let mur = prisme(contour, base + zmin - 1.0, base + zmax + 1.0);
    let mut tris = booleen(&mur, &extruder(exterieur), noyau::csg::Op::Intersection, 1e-5).ok_or("Profil du mur trop complexe.")?;
    for v in vides {
        if tris.is_empty() { break; }
        tris = booleen(&tris, &extruder(v), noyau::csg::Op::Difference, 1e-5).ok_or("Profil du mur trop complexe.")?;
    }
    Ok(tris)
}

#[cfg(test)]
#[path = "tests_profil.rs"]
mod tests;
