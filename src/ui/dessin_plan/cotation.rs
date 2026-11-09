//! Cotation graphique des vues natives (Annoter) : cote linéaire, angulaire, de
//! diamètre et altimétrique, saisies point par point avec les accrochages, le repérage
//! polaire et la saisie au clavier du dessin 2D ; aperçu de la cote sous le curseur.
use super::*;
use crate::modele::documentation::{self as doc, PointCote};
use crate::ui::ruban::Outil;

/// Hauteur papier des textes de cote (mm).
const HAUTEUR_TEXTE: f64 = 2.5;

impl ViewerApp {
    /// Outil de cotation actif et nombre de points qu'il attend.
    pub(in crate::ui) fn outil_cotation(&self) -> Option<(Outil, usize)> {
        if self.onglet_ruban != OngletRuban::Annoter || self.vue_native_id().is_none() { return None; }
        match self.ruban_edition.outil()? {
            Outil::Cote => Some((Outil::Cote, 3)),
            Outil::CoteAngulaire => Some((Outil::CoteAngulaire, 4)),
            Outil::CoteDiametre => Some((Outil::CoteDiametre, 2)),
            Outil::Altimetrie => Some((Outil::Altimetrie, 2)),
            _ => None,
        }
    }

    /// Consigne de l'étape en cours.
    pub(in crate::ui) fn invite_cotation(&self) -> Option<&'static str> {
        let (outil, _) = self.outil_cotation()?;
        let k = self.dessin2d.trace.as_ref().map_or(0, |t| t.points.len());
        Some(match outil {
            Outil::Cote => ["Premier point à coter", "Second point à coter", "Position de la ligne de cote"][k.min(2)],
            Outil::CoteAngulaire => ["Sommet de l'angle", "Point sur la première branche", "Point sur la seconde branche", "Position de l'arc de cote"][k.min(3)],
            Outil::CoteDiametre => ["Centre du cercle (accrochage Centre)", "Point du cercle"][k.min(1)],
            _ => ["Point dont l'altitude est cotée", "Position du texte"][k.min(1)],
        })
    }

    /// Échelle de la vue native ouverte (1/échelle).
    fn echelle_vue_cotation(&self) -> f64 {
        let id = self.vue_native_id();
        self.natif.as_ref().and_then(|(p, _)| p.vues.iter().find(|v| Some(&v.id) == id.as_ref())).map_or(100.0, |v| v.echelle)
    }

    /// Forme de la cote pour les points donnés (dernier point : position).
    fn forme_cotation(&self, outil: Outil, pts: &[DVec2]) -> Option<AnnotationForme> {
        let a = |p: DVec2| p.to_array();
        Some(match (outil, pts) {
            (Outil::Cote, [p, q, pos]) => {
                let d = (*q - *p).try_normalize()?;
                AnnotationForme::Cote { debut: PointCote::Fixe { point: a(*p) }, fin: PointCote::Fixe { point: a(*q) }, decalage: (*pos - *p).dot(d.perp()) }
            }
            (Outil::CoteAngulaire, [s, p, q, pos]) => {
                doc::angle_branches(*s, *p, *q)?;
                AnnotationForme::CoteAngulaire { sommet: a(*s), debut: a(*p), fin: a(*q), rayon: pos.distance(*s).max(1.0) }
            }
            (Outil::CoteDiametre, [c, p]) if c.distance(*p) > 1e-6 => AnnotationForme::CoteDiametre { centre: a(*c), point: a(*p) },
            (Outil::Altimetrie, [p, r]) => AnnotationForme::Altimetrie {
                point: a(*p),
                repere: a(*r),
                relative_niveau: self.ruban_edition.texte_option(Outil::Altimetrie, 0) == "Niveau sous le point",
                decimales: if self.ruban_edition.texte_option(Outil::Altimetrie, 1) == "0.000 m" { 3 } else { 2 },
            },
            _ => return None,
        })
    }

    /// Crée la cote dès que ses points sont donnés (souris ou clavier).
    pub(in crate::ui) fn verifier_cotation(&mut self) {
        let Some((outil, n)) = self.outil_cotation() else { return };
        let Some(t) = self.dessin2d.trace.as_mut() else { return };
        if t.points.len() < n { return; }
        let pts: Vec<DVec2> = t.points.drain(..).take(n).collect();
        let Some(vue) = self.vue_native_id() else { return };
        let Some(forme) = self.forme_cotation(outil, &pts) else {
            self.dessin2d.message = Some("Cote impossible : points confondus ou alignés.".into());
            return;
        };
        match self.executer_bim(Commande::Annoter { annotation: Annotation { id: String::new(), vue, hauteur: HAUTEUR_TEXTE, forme } }) {
            Ok(_) => self.confirmer("Cote créée".into()),
            Err(e) => self.error = Some(e),
        }
        self.bim.selection.clear();
    }

    /// Aperçu de la cote en cours, le curseur donnant le point suivant.
    pub(in crate::ui) fn apercu_cotation(&self, painter: &egui::Painter, rect: egui::Rect, curseur: DVec2) {
        let Some((outil, n)) = self.outil_cotation() else { return };
        let Some(t) = &self.dessin2d.trace else { return };
        let bleu = Color32::from_rgb(0, 120, 215);
        let trait_ = egui::Stroke::new(1.5, bleu);
        let mut pts = t.points.clone();
        pts.push(curseur);
        let taille = HAUTEUR_TEXTE * self.echelle_vue_cotation();
        let (oblique, marge) = (taille * 0.6, taille);
        let segment = |a: DVec2, b: DVec2| if let (Some(x), Some(y)) = (self.vers_ecran(rect, a), self.vers_ecran(rect, b)) { painter.line_segment([x, y], trait_); };
        let ecrire = |p: DVec2, s: &str| if let Some(e) = self.vers_ecran(rect, p) { painter.text(e, egui::Align2::CENTER_BOTTOM, s, egui::FontId::proportional(13.0), bleu); };
        // étapes intermédiaires : élastique depuis le point précédent
        if pts.len() < n && !(outil == Outil::CoteAngulaire && pts.len() == 3) {
            for w in pts.windows(2) { segment(w[0], w[1]); }
            if outil == Outil::Cote && pts.len() == 2 { ecrire((pts[0] + pts[1]) / 2.0, &crate::config::options::unite().afficher(pts[0].distance(pts[1]))); }
            return;
        }
        if outil == Outil::CoteAngulaire && pts.len() == 3 {
            // rayon provisoire : la moitié de la branche la plus courte
            let r = pts[1].distance(pts[0]).min(pts[2].distance(pts[0])) * 0.5;
            pts.push(pts[0] + DVec2::X * r);
        }
        let Some(forme) = self.forme_cotation(outil, &pts[..n]) else { return };
        let g = match &forme {
            AnnotationForme::Cote { debut: PointCote::Fixe { point: a }, fin: PointCote::Fixe { point: b }, decalage } => {
                let g = doc::geometrie_cote(&[DVec2::from_array(*a), DVec2::from_array(*b)], *decalage, oblique);
                for [a, b] in &g.traits { segment(*a, *b); }
                for (p, v) in &g.textes { ecrire(*p, &crate::config::options::unite().afficher(*v)); }
                return;
            }
            AnnotationForme::CoteAngulaire { sommet, debut, fin, rayon } => doc::geometrie_angle((*sommet).into(), (*debut).into(), (*fin).into(), *rayon, oblique, marge),
            AnnotationForme::CoteDiametre { centre, point } => doc::geometrie_diametre((*centre).into(), (*point).into(), oblique, marge),
            AnnotationForme::Altimetrie { point, repere, relative_niveau, decimales } => {
                let z = self.natif.as_ref().zip(self.vue_native_id()).and_then(|((p, _), id)| p.vues.iter().find(|v| v.id == id).map(|v| doc::altitude_vue(p, v, *point, *relative_niveau)));
                Some(doc::geometrie_altimetrie((*point).into(), (*repere).into(), z.unwrap_or(0.0), *decimales, taille))
            }
            _ => None,
        };
        if let Some(g) = g {
            for [a, b] in &g.traits { segment(*a, *b); }
            ecrire(g.texte.0, &g.texte.2);
        }
    }
}
