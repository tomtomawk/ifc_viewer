//! Outil Quadrillage : une ligne de quadrillage se trace en deux clics dans une vue en plan
//! (accrochages du tracé, Maj : orthogonal) ; le repère suivant est proposé (A → B,
//! 1 → 2) et l'outil reste actif.
use crate::*;
use crate::zones::ruban::Outil;

/// Repère proposé après `nom` : nombre final augmenté, lettre suivante (Z → AA), sinon
/// `nom` suivi de « 2 ».
pub(crate) fn repere_suivant(nom: &str) -> String {
    let chiffres = nom.len() - nom.trim_end_matches(|c: char| c.is_ascii_digit()).len();
    if chiffres > 0 {
        let (tete, n) = nom.split_at(nom.len() - chiffres);
        return format!("{tete}{}", n.parse::<u64>().unwrap_or(0) + 1);
    }
    if !nom.is_empty() && nom.chars().all(|c| c.is_ascii_uppercase()) {
        // base 26 sans zéro : A..Z, AA..AZ, BA…
        let mut lettres: Vec<u8> = nom.bytes().collect();
        for k in (0..lettres.len()).rev() {
            if lettres[k] < b'Z' {
                lettres[k] += 1;
                return String::from_utf8(lettres).unwrap_or_default();
            }
            lettres[k] = b'A';
        }
        lettres.insert(0, b'A');
        return String::from_utf8(lettres).unwrap_or_default();
    }
    format!("{nom} 2")
}

impl Application {
    /// Outil Quadrillage actif dans une vue en plan du projet natif.
    pub(crate) fn quadrillage_actif(&self) -> bool {
        self.onglet_ruban == OngletRuban::Modeliser && self.natif.is_some() && self.loading.is_none()
            && self.ruban_edition.outil() == Some(Outil::Quadrillage) && self.vue_en_plan()
    }

    /// Repère de la prochaine ligne : celui du réglage, ou le suivant libre s'il est pris.
    fn repere_libre(&self) -> String {
        let mut nom = self.ruban_edition.texte_option(Outil::Quadrillage, 0);
        if let Some((p, _)) = &self.natif {
            while p.quadrillages.iter().any(|x| x.nom == nom) {
                nom = repere_suivant(&nom);
            }
        }
        nom
    }

    /// Clic de l'outil : départ, puis arrivée qui crée la ligne (une transaction).
    pub(crate) fn clic_quadrillage(&mut self, rect: egui::Rect, q: egui::Pos2, maj: bool) -> Result<(), String> {
        let altitude = self.altitude_vue_native();
        let (point, _) = self.point_tracage(q, rect, maj, self.bim.depart_quadrillage, altitude).ok_or("Quadrillage : tracer dans une vue en plan.")?;
        let Some(debut) = self.bim.depart_quadrillage else {
            self.bim.depart_quadrillage = Some(point);
            return Ok(());
        };
        if (point[0] - debut[0]).hypot(point[1] - debut[1]) < 1.0 {
            return Err("Quadrillage : l'arrivée doit différer du départ.".into());
        }
        let nom = self.repere_libre();
        self.executer_bim(commandes::Commande::Quadrillage { nom: nom.clone(), debut, fin: point })?;
        self.bim.depart_quadrillage = None;
        self.ruban_edition.definir_texte_option(Outil::Quadrillage, 0, &repere_suivant(&nom));
        let suivant = self.repere_libre();
        self.status = format!("Quadrillage « {nom} » créé — repère suivant « {suivant} » ; Ctrl+Z pour annuler.");
        Ok(())
    }

    /// Consigne, point accroché et aperçu de la ligne en cours.
    pub(crate) fn apercu_quadrillage(&self, ui: &egui::Ui, rect: egui::Rect, survol: Option<egui::Pos2>) {
        let nom = self.repere_libre();
        let consigne = match self.bim.depart_quadrillage {
            None => format!("Quadrillage « {nom} » : cliquer le départ de la ligne · Échap ou clic droit : quitter"),
            Some(_) => format!("Quadrillage « {nom} » : cliquer l'arrivée · Maj : orthogonal · Échap ou clic droit : reprendre le départ"),
        };
        if !self.message_visible() {
            crate::socle::messages::consigne_vue(ui, rect, self.dessin2d.bas_barre, &consigne);
        }
        let altitude = self.altitude_vue_native();
        let maj = ui.input(|i| i.modifiers.shift);
        let Some((p, accroche)) = survol.and_then(|q| self.point_tracage(q, rect, maj, self.bim.depart_quadrillage, altitude)) else { return };
        let painter = ui.painter().with_clip_rect(rect);
        let couleur = Color32::from_rgb(55, 130, 200);
        if let Some(q) = self.ecran_plan(rect, p, altitude) {
            crate::saisie::accrochage::marqueur(&painter, q, accroche);
        }
        if let Some(a) = self.bim.depart_quadrillage
            && let (Some(x), Some(y)) = (self.ecran_plan(rect, a, altitude), self.ecran_plan(rect, p, altitude))
        {
            painter.add(egui::Shape::dashed_line(&[x, y], egui::Stroke::new(1.5, couleur), 10.0, 4.0));
            painter.circle_stroke(y, 9.0, egui::Stroke::new(1.5, couleur));
            painter.text(y, egui::Align2::CENTER_CENTER, &nom, egui::FontId::proportional(11.0), couleur);
            painter.text(x.lerp(y, 0.5) + egui::vec2(0.0, -14.0), egui::Align2::CENTER_CENTER, modele::unite::unite().afficher((p[0] - a[0]).hypot(p[1] - a[1])), egui::FontId::proportional(13.0), couleur);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::repere_suivant;

    #[test]
    fn reperes_suivants() {
        assert_eq!(repere_suivant("A"), "B");
        assert_eq!(repere_suivant("Z"), "AA");
        assert_eq!(repere_suivant("AZ"), "BA");
        assert_eq!(repere_suivant("1"), "2");
        assert_eq!(repere_suivant("A9"), "A10");
        assert_eq!(repere_suivant("Axe"), "Axe 2");
    }
}
