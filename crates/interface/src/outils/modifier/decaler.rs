//! Visée de Décaler pour les murs et les surfaces.
use crate::outils::modifier::transformer::{distance_signee, lire_valeur};
use crate::*;
use modele::Axe;

fn distance_au_cote(distance: f64, imposee: f64) -> f64 {
    if imposee > 0.0 {
        imposee.copysign(distance)
    } else {
        distance
    }
}

impl Application {
    pub(super) fn apercu_decalage_mur(
        &self,
        id: &str,
        d: f64,
        rect: egui::Rect,
        painter: &egui::Painter,
        couleur: Color32,
    ) -> Option<([f64; 2], [f64; 2])> {
        use modele::volumes::solides::repere;
        let (projet, _) = self.natif.as_ref()?;
        let axe = modele::volumes::solides::hote(projet, id).ok()?.0;
        let (axe, z) = match axe {
            Axe::Arc {
                debut,
                centre,
                direct,
                ..
            } => {
                let r = (debut[0] - centre[0]).hypot(debut[1] - centre[1]);
                if r + if direct { -d } else { d } < 10.0 {
                    return None;
                }
                (axe, projet.trajectoire_justification(id)?.1)
            }
            Axe::Droit { .. } => {
                let (a, b, z) = self.axe_mur(id)?;
                (Axe::Droit { debut: a, fin: b }, z)
            }
        };
        let cible = modele::murs::assemblage::decaler_axe(&axe, d);
        let n = if matches!(axe, Axe::Arc { .. }) {
            64
        } else {
            1
        };
        for (ligne, epaisseur) in [(&axe, 2.0), (&cible, 2.5)] {
            let longueur = ligne.longueur();
            let points = (0..=n)
                .filter_map(|i| {
                    let p = repere(ligne, longueur * f64::from(i) / f64::from(n)).0;
                    self.ecran_plan(rect, p.to_array(), z)
                })
                .collect();
            painter.add(egui::Shape::line(
                points,
                egui::Stroke::new(epaisseur, couleur),
            ));
        }
        Some((
            repere(&axe, axe.longueur() * 0.5).0.to_array(),
            repere(&cible, cible.longueur() * 0.5).0.to_array(),
        ))
    }

    /// Distance de décalage (mm) donnée par le curseur `p` pour l'élément `id` : à gauche
    /// du sens d'un mur droit, vers le centre d'un arc trigonométrique, vers l'extérieur
    /// d'un contour de dalle.
    pub(super) fn distance_decalage(&self, id: &str, p: [f64; 2]) -> Option<f64> {
        let (projet, _) = self.natif.as_ref()?;
        if let Some(d) = projet.dalles.iter().find(|d| d.id == id) {
            let c = d.contours.first()?;
            let n = c.len();
            let dist = (0..n)
                .map(|i| {
                    let (a, b) = (c[i], c[(i + 1) % n]);
                    let (ux, uy) = (b[0] - a[0], b[1] - a[1]);
                    let l2 = (ux * ux + uy * uy).max(1e-9);
                    let t = (((p[0] - a[0]) * ux + (p[1] - a[1]) * uy) / l2).clamp(0.0, 1.0);
                    (p[0] - a[0] - ux * t).hypot(p[1] - a[1] - uy * t)
                })
                .fold(f64::MAX, f64::min);
            let mut dedans = false;
            for i in 0..n {
                let (a, b) = (c[i], c[(i + n - 1) % n]);
                if (a[1] > p[1]) != (b[1] > p[1])
                    && p[0] < (b[0] - a[0]) * (p[1] - a[1]) / (b[1] - a[1]) + a[0]
                {
                    dedans = !dedans;
                }
            }
            return Some(if dedans { -dist } else { dist });
        }
        if let Some(Axe::Arc {
            debut,
            centre,
            direct,
            ..
        }) = modele::volumes::solides::hote(projet, id)
            .ok()
            .map(|(axe, ..)| axe)
        {
            let r = (debut[0] - centre[0]).hypot(debut[1] - centre[1]);
            let rp = (p[0] - centre[0]).hypot(p[1] - centre[1]);
            return Some(if direct { r - rp } else { rp - r });
        }
        let (a, b, _) = self.axe_mur(id)?;
        Some(distance_signee(p, a, b))
    }

    pub(super) fn distance_decalage_visee(
        &self,
        id: &str,
        p: [f64; 2],
        brut: Option<[f64; 2]>,
    ) -> Option<f64> {
        let imposee = lire_valeur(&self.transfo.saisie)
            .map(|v| modele::unite::unite().vers_mm(v.abs()))
            .unwrap_or(self.transfo.distance_decalage);
        let distance =
            self.distance_decalage(id, if imposee > 0.0 { brut.unwrap_or(p) } else { p })?;
        Some(distance_au_cote(distance, imposee))
    }

    /// Élément décalable : mur (droit, courbe, composé) ou dalle.
    pub(super) fn decalable(&self, id: &str) -> bool {
        self.natif.as_ref().is_some_and(|(p, _)| {
            p.dalles.iter().any(|d| d.id == id) || modele::volumes::solides::hote(p, id).is_ok()
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn distance_saisie_utilise_le_cote_sans_prendre_la_distance_du_curseur() {
        assert_eq!(distance_au_cote(-30.0, 250.0), -250.0);
        assert_eq!(distance_au_cote(60.0, 250.0), 250.0);
        assert_eq!(distance_au_cote(-30.0, 0.0), -30.0);
    }
}
