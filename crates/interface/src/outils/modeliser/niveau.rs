//! Outil Niveau dans une coupe ou une élévation native : la ligne du futur niveau suit le
//! curseur (accrochages de la vue, cote arrondie au centimètre) ; un clic crée le niveau à
//! cette altitude (une transaction) et l'outil reste actif. Dans un plan, l'ajout se fait
//! par le bouton de Propriétés.
use crate::*;
use crate::zones::ruban::Outil;

/// Nom du prochain niveau : « RDC » s'il n'y en a aucun, sinon le premier « R+k » libre.
pub(crate) fn nom_niveau_suivant(p: &modele::Projet) -> String {
    if p.niveaux.is_empty() {
        return "RDC".into();
    }
    (1..).map(|k| format!("R+{k}")).find(|n| !p.niveaux.iter().any(|x| &x.nom == n)).unwrap_or_default()
}

impl Application {
    /// Outil Niveau actif dans une coupe ou une élévation native.
    pub(crate) fn niveau_vue_actif(&self) -> bool {
        self.onglet_ruban == OngletRuban::Modeliser && self.loading.is_none()
            && self.ruban_edition.outil() == Some(Outil::Niveau)
            && self.vue_native_id().zip(self.natif.as_ref()).is_some_and(|(id, (p, _))| {
                p.vues.iter().any(|v| v.id == id && matches!(v.genre, modele::GenreVue::Coupe { .. }))
            })
    }

    /// Altitude (mm, arrondie au centimètre) sous le curseur, accrochée aux géométries.
    fn altitude_niveau_visee(&mut self, ui: &egui::Ui, rect: egui::Rect, q: egui::Pos2) -> Option<f64> {
        let (maj, temps) = ui.input(|i| (i.modifiers.shift, i.time));
        let c = self.depuis_ecran(rect, q)?;
        let (point, accroche) = self.point_dessin_depuis(rect, c, maj, temps, None);
        self.dessin2d.point = Some((point, accroche));
        // repère de la coupe : second axe vertical, origine à l'altitude 0 du projet
        Some((point.y / 10.0).round() * 10.0)
    }

    /// Survol : consigne, ligne horizontale du futur niveau et sa cote ; clic : création.
    pub(crate) fn outil_niveau_vue(&mut self, ui: &egui::Ui, rect: egui::Rect, resp: &egui::Response) {
        let nom = self.natif.as_ref().map(|(p, _)| nom_niveau_suivant(p)).unwrap_or_default();
        if !self.message_visible() {
            crate::socle::messages::consigne_vue(ui, rect, self.dessin2d.bas_barre, &format!("Niveau « {nom} » : cliquer à l'altitude voulue · Échap ou clic droit : quitter"));
        }
        if resp.clicked_by(PointerButton::Secondary) {
            self.ruban_edition.annuler();
            return;
        }
        let Some(q) = resp.hover_pos().or(resp.interact_pointer_pos()).filter(|q| rect.contains(*q)) else { return };
        let Some(altitude) = self.altitude_niveau_visee(ui, rect, q) else { return };
        let couleur = Color32::from_rgb(55, 130, 200);
        let painter = ui.painter().with_clip_rect(rect);
        if let Some((point, accroche)) = self.dessin2d.point
            && let Some(e) = self.vers_ecran(rect, point)
        {
            crate::saisie::moteur_accrochage::peindre_marqueur(&painter, e, accroche);
            let y = self.vers_ecran(rect, glam::DVec2::new(point.x, altitude)).map_or(e.y, |p| p.y);
            painter.add(egui::Shape::dashed_line(&[egui::pos2(rect.left(), y), egui::pos2(rect.right(), y)], egui::Stroke::new(1.5, couleur), 10.0, 5.0));
            painter.text(egui::pos2(rect.right() - 12.0, y - 4.0), egui::Align2::RIGHT_BOTTOM, format!("{nom} · {}", modele::unite::unite().afficher(altitude)), egui::FontId::proportional(13.0), couleur);
        }
        if resp.clicked() {
            match self.executer_bim(commandes::Commande::Niveau { nom: nom.clone(), altitude }) {
                Ok(_) => self.status = format!("Niveau « {nom} » créé à {} — Ctrl+Z pour annuler.", modele::unite::unite().afficher(altitude)),
                Err(e) => self.error = Some(e),
            }
        }
    }
}
