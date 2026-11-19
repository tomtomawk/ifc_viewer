//! Coupes des abouts en plan à partir des lignes d'implantation des poutres.
use super::{Barre, GenreBarre, Projet};
use glam::DVec2;

const TOLERANCE: f64 = 0.5;

/// Déplacements longitudinaux des bords droit et gauche de l'about (mm).
pub(super) fn about(
    projet: &Projet,
    barre: &Barre,
    fin: bool,
    largeur: f64,
    hauteur: f64,
) -> [f64; 2] {
    let xy = |p: [f64; 3]| DVec2::new(p[0], p[1]);
    let (a, b) = (xy(barre.debut), xy(barre.fin));
    let longueur = a.distance(b);
    let u = (b - a) / longueur;
    let noeud = if fin { b } else { a };
    let bas = barre.debut[2] + barre.pose.arase.bas(hauteur);
    let decalage = barre.pose.justification.decalage(largeur);
    let variante = |id: &str| projet.contextes.get(id).and_then(|c| c.variante.as_ref());
    let mut raccord = None;
    for voisine in &projet.barres {
        if voisine.id == barre.id
            || voisine.genre != GenreBarre::Poutre
            || voisine.niveau != barre.niveau
            || variante(&voisine.id) != variante(&barre.id)
        {
            continue;
        }
        let (c, d) = (xy(voisine.debut), xy(voisine.fin));
        let Some(v) = (d - c).try_normalize() else {
            continue;
        };
        let det = u.perp_dot(v);
        if det.abs() < 1e-8 || (noeud - c).perp_dot(v).abs() > TOLERANCE {
            continue;
        }
        let portee = c.distance(d);
        let s = (noeud - c).dot(v);
        if s < -TOLERANCE || s > portee + TOLERANCE {
            continue;
        }
        let type_ = voisine
            .pose
            .type_poutre
            .as_deref()
            .and_then(|id| projet.type_poutre(id));
        let (l, h) = type_.map_or((voisine.largeur, voisine.hauteur), |t| {
            (t.largeur, t.hauteur)
        });
        let bas_voisin = voisine.debut[2] + voisine.pose.arase.bas(h);
        if bas.max(bas_voisin) >= (bas + hauteur).min(bas_voisin + h) {
            continue;
        }
        let sur_debut = noeud.distance(c) <= TOLERANCE;
        let sur_fin = noeud.distance(d) <= TOLERANCE;
        let centre = voisine.pose.justification.decalage(l);
        let mut coupes = [0.0; 2];
        for (coupe, cote) in coupes.iter_mut().zip([-0.5, 0.5]) {
            let bord = noeud + u.perp() * (decalage + cote * largeur);
            let cote_voisin = if sur_debut || sur_fin {
                // Onglet : intersecter les faces homologues en suivant le virage.
                if fin == sur_fin { -cote } else { cote }
            } else {
                // En T, la poutre traversante garde ses faces ; couper sur la face proche.
                0.5 * (if fin { -u } else { u }).dot(v.perp()).signum()
            };
            let face = c + v.perp() * (centre + cote_voisin * l);
            *coupe = (face - bord).perp_dot(v) / det;
            // Une coupe ne doit pas dépasser l'autre about, même si lui aussi est raccordé.
            if coupe.abs() >= longueur * 0.5 {
                return [0.0; 2];
            }
            if sur_debut || sur_fin {
                let distance_voisine =
                    (bord + u * *coupe - face).dot(v) - if sur_fin { portee } else { 0.0 };
                if distance_voisine.abs() >= portee * 0.5 {
                    return [0.0; 2];
                }
            }
        }
        if raccord.replace(coupes).is_some() {
            return [0.0; 2];
        }
    }
    raccord.unwrap_or([0.0; 2])
}
