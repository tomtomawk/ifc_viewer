//! Messages de l'application, affichés en un seul endroit : en haut de la vue, sous le
//! bandeau de dessin. Information et avertissement disparaissent seuls ; une erreur
//! reste jusqu'à sa fermeture (×) ou au message suivant.
use crate::*;

/// Durée d'affichage d'une information ou d'un avertissement.
const DUREE: f32 = 3.0;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum Niveau {
    Information,
    /// Premier usage prévu : modification automatique du type de terrain (n° 14).
    #[allow(dead_code)]
    Avertissement,
    Erreur,
}

impl Niveau {
    fn couleur(self) -> Color32 {
        match self {
            Niveau::Information => Color32::from_rgb(40, 100, 160),
            Niveau::Avertissement => Color32::from_rgb(190, 110, 0),
            Niveau::Erreur => Color32::from_rgb(185, 35, 35),
        }
    }
}

/// Message temporaire (information ou avertissement) et son heure d'apparition.
pub(crate) type MessageTemporaire = Option<(Niveau, String, std::time::Instant)>;

/// Position commune des consignes et des messages : sous le bandeau de dessin.
pub(crate) fn origine_consigne(rect: egui::Rect, bas_barre: f32) -> egui::Pos2 {
    rect.left_top() + egui::vec2(8.0, bas_barre.max(4.0) + 8.0)
}

/// Consigne d'outil (texte d'aide permanent), dans le style des messages d'information.
pub(crate) fn consigne_vue(ui: &egui::Ui, rect: egui::Rect, bas_barre: f32, texte: &str) {
    let painter = ui.painter().with_clip_rect(rect);
    let couleur = Niveau::Information.couleur();
    let galley = painter.layout(texte.into(), egui::FontId::proportional(13.0), couleur, (rect.width() - 24.0).max(1.0));
    let cadre = egui::Rect::from_min_size(origine_consigne(rect, bas_barre), galley.size() + egui::vec2(8.0, 6.0));
    painter.rect_filled(cadre, 3.0, Color32::from_gray(250));
    painter.galley(cadre.min + egui::vec2(4.0, 3.0), galley, couleur);
}

/// Message au premier plan (visible au-dessus des éditeurs flottants), avec un bouton ×
/// pour une erreur. `alpha` estompe la fin d'un message temporaire. Retourne `true` si
/// l'utilisateur l'a fermé.
fn afficher(ui: &egui::Ui, rect: egui::Rect, bas_barre: f32, niveau: Niveau, texte: &str, alpha: f32) -> bool {
    let couleur = niveau.couleur().gamma_multiply(alpha);
    let fermable = niveau == Niveau::Erreur;
    let marge_bouton = if fermable { 22.0 } else { 0.0 };
    let origine = origine_consigne(rect, bas_barre);
    egui::Area::new(ui.scope_id().with("message_vue"))
        .order(egui::Order::Foreground)
        .fixed_pos(origine)
        .constrain_to(rect)
        .interactable(fermable)
        .show(ui.ctx(), |ui| {
            let galley = ui.painter().layout(texte.into(), egui::FontId::proportional(13.0), couleur, (rect.width() - 24.0 - marge_bouton).max(1.0));
            let taille = galley.size() + egui::vec2(8.0 + marge_bouton, 6.0);
            let (cadre, _) = ui.allocate_exact_size(egui::vec2(taille.x, taille.y.max(22.0)), Sense::hover());
            ui.painter().rect(cadre, 3.0, Color32::from_gray(250).gamma_multiply(alpha), egui::Stroke::new(1.0, couleur.gamma_multiply(0.5)), egui::StrokeKind::Inside);
            ui.painter().galley(cadre.min + egui::vec2(4.0, 3.0), galley, couleur);
            if !fermable { return false; }
            let fermer = egui::Rect::from_min_size(cadre.right_top() - egui::vec2(22.0, 0.0), egui::vec2(22.0, 22.0));
            ui.put(fermer, egui::Button::new(RichText::new("×").size(16.0).color(couleur)).frame(false))
                .on_hover_text("Effacer le message").clicked()
        }).inner
}

/// Erreur propre à un éditeur (cartouche…), au même endroit que les autres messages.
pub(crate) fn erreur_vue(ui: &egui::Ui, rect: egui::Rect, bas_barre: f32, texte: &str) -> bool {
    afficher(ui, rect, bas_barre, Niveau::Erreur, texte, 1.0)
}

impl ViewerApp {
    /// Information temporaire (confirmation d'une action). Un avertissement se pose de
    /// même avec `Niveau::Avertissement`.
    pub(crate) fn informer(&mut self, texte: impl Into<String>) {
        self.signaler(Niveau::Information, texte);
    }

    pub(crate) fn signaler(&mut self, niveau: Niveau, texte: impl Into<String>) {
        match niveau {
            Niveau::Erreur => self.error = Some(texte.into()),
            _ => self.message = Some((niveau, texte.into(), std::time::Instant::now())),
        }
    }

    /// Un message occupe l'emplacement des consignes.
    pub(crate) fn message_visible(&self) -> bool {
        self.error.is_some() || self.dessin2d.message.is_some() || self.message.as_ref().is_some_and(|(_, _, debut)| debut.elapsed().as_secs_f32() < DUREE)
    }

    /// Affiche le message courant : l'erreur en priorité, sinon le message temporaire.
    pub(crate) fn afficher_messages(&mut self, ui: &egui::Ui, rect: egui::Rect, bas_barre: f32) {
        if let Some(texte) = self.error.clone() {
            if afficher(ui, rect, bas_barre, Niveau::Erreur, &texte, 1.0) { self.error = None; }
            return;
        }
        // Erreur de l'outil de dessin en cours : effacée par l'outil ou par ×.
        if let Some(texte) = self.dessin2d.message.clone() {
            if afficher(ui, rect, bas_barre, Niveau::Erreur, &texte, 1.0) { self.dessin2d.message = None; }
            return;
        }
        let Some((niveau, texte, debut)) = &self.message else { return };
        let age = debut.elapsed().as_secs_f32();
        if age >= DUREE {
            self.message = None;
            return;
        }
        afficher(ui, rect, bas_barre, *niveau, texte, ((DUREE - age) / 0.5).min(1.0));
        ui.ctx().request_repaint();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn niveaux_distincts() {
        let couleurs = [Niveau::Information, Niveau::Avertissement, Niveau::Erreur].map(Niveau::couleur);
        assert!(couleurs[0] != couleurs[1] && couleurs[1] != couleurs[2] && couleurs[0] != couleurs[2]);
        let r = egui::Rect::from_min_size(egui::pos2(10.0, 20.0), egui::vec2(100.0, 100.0));
        assert_eq!(origine_consigne(r, 0.0), egui::pos2(18.0, 32.0));
    }
}
