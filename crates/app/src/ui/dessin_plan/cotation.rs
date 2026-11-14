//! Cotation graphique des vues natives (Annoter), selon la méthode commune
//! (`documentation::GenreCote`) : points recueillis par le tracé du dessin 2D
//! (accrochages, repérage polaire, saisie au clavier), aperçu de la cote sous le curseur.
use super::*;
use modele::documentation::{self as doc, GenreCote};
use crate::ui::ruban::Outil;

/// Hauteur papier des textes de cote (mm).
const HAUTEUR_TEXTE: f64 = 2.5;

impl ViewerApp {
    /// Genre de cote de l'outil actif (Annoter, vue native ouverte).
    pub(in crate::ui) fn outil_cotation(&self) -> Option<GenreCote> {
        if self.onglet_ruban != OngletRuban::Annoter || self.vue_native_id().is_none() { return None; }
        self.ruban_edition.outil()?.genre_cote()
    }

    /// Consigne de l'étape en cours.
    pub(in crate::ui) fn invite_cotation(&self) -> Option<&'static str> {
        if let Some(invite) = self.invite_aretes() { return Some(invite); }
        Some(self.outil_cotation()?.invite(self.dessin2d.trace.as_ref().map_or(0, |t| t.points.len())))
    }

    fn vue_cotation(&self) -> Option<(&modele::Projet, &modele::Vue)> {
        let id = self.vue_native_id()?;
        let p = &self.natif.as_ref()?.0;
        Some((p, p.vues.iter().find(|v| v.id == id)?))
    }

    fn forme_cotation(&self, genre: GenreCote, pts: &[DVec2]) -> Option<AnnotationForme> {
        let relatif = self.ruban_edition.texte_option(Outil::Altimetrie, 0) == "Niveau sous le point";
        let decimales = if self.ruban_edition.texte_option(Outil::Altimetrie, 1) == "0.000 m" { 3 } else { 2 };
        genre.forme(pts, relatif, decimales)
    }

    /// Crée la cote dès que ses points sont donnés (souris ou clavier).
    pub(in crate::ui) fn verifier_cotation(&mut self) {
        let Some(genre) = self.outil_cotation() else { return };
        let Some(t) = self.dessin2d.trace.as_mut() else { return };
        if t.points.len() < genre.points() { return; }
        let pts: Vec<DVec2> = t.points.drain(..).take(genre.points()).collect();
        let Some(vue) = self.vue_native_id() else { return };
        let Some(forme) = self.forme_cotation(genre, &pts) else {
            self.dessin2d.message = Some("Cote impossible : points confondus ou alignés.".into());
            self.dessin2d.aretes_cote.clear();
            return;
        };
        // sélection d'arêtes : la cote s'appuie sur les éléments et les suit
        let forme = self.lier_aretes(forme);
        match self.executer_bim(Commande::Annoter { annotation: Annotation { id: String::new(), vue, hauteur: HAUTEUR_TEXTE, forme } }) {
            Ok(_) => self.confirmer("Cote créée".into()),
            Err(e) => self.error = Some(e),
        }
        self.bim.selection.clear();
    }

    /// Aperçu de la cote en cours, le curseur donnant le point suivant.
    pub(in crate::ui) fn apercu_cotation(&self, painter: &egui::Painter, rect: egui::Rect, curseur: DVec2) {
        let (Some(genre), Some(t), Some((p, v))) = (self.outil_cotation(), &self.dessin2d.trace, self.vue_cotation()) else { return };
        let bleu = Color32::from_rgb(0, 120, 215);
        let segment = |a: DVec2, b: DVec2| if let (Some(x), Some(y)) = (self.vers_ecran(rect, a), self.vers_ecran(rect, b)) { painter.line_segment([x, y], egui::Stroke::new(1.5, bleu)); };
        let ecrire = |q: DVec2, s: &str| if let Some(e) = self.vers_ecran(rect, q) { painter.text(e, egui::Align2::CENTER_BOTTOM, s, egui::FontId::proportional(13.0), bleu); };
        let Some(pts) = genre.points_apercu(&t.points, curseur) else {
            // étapes intermédiaires : élastique depuis le point précédent
            let mut pts = t.points.clone();
            pts.push(curseur);
            for w in pts.windows(2) { segment(w[0], w[1]); }
            return;
        };
        let Some(forme) = self.forme_cotation(genre, &pts) else { return };
        let taille = HAUTEUR_TEXTE * v.echelle;
        let unite = crate::config::options::unite();
        let longueur = |x: f64| unite.afficher(x);
        let altitude = |q: [f64; 2], relatif: bool| doc::altitude_vue(p, v, q, relatif);
        let style = doc::StyleCote { oblique: taille * 0.6, taille, longueur: &longueur, altitude: &altitude };
        if let Some((traits, textes)) = doc::geometrie(&forme, None, &style) {
            for [a, b] in traits { segment(a, b); }
            for (q, _, s) in textes { ecrire(q, &s); }
        }
    }
}
