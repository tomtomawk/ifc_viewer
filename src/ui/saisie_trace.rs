//! Saisie clavier partagée des points pour les tracés DAO.
use eframe::egui;
use glam::DVec2;

pub(super) fn ajouter_caractere_saisie(saisie: &mut String, angle: &mut bool, ch: char) {
    if matches!(ch, '@' | '#') && (saisie.is_empty() || saisie == "<") {
        saisie.clear();
        saisie.push(ch);
        *angle = false;
    } else if matches!(ch, '<' | '>') && !saisie.starts_with(['@', '#']) {
        if let Some(i) = saisie.find(['<', '>']) { saisie.replace_range(i..=i, &ch.to_string()); }
        else { saisie.push(ch); }
        *angle = true;
    } else if *angle && !saisie.starts_with(['@', '#']) {
        if !saisie.contains(['<', '>']) { saisie.push('<'); }
        saisie.push(ch);
    } else if let Some(i) = saisie.find(['<', '>']) {
        saisie.insert(i, ch);
    } else {
        saisie.push(ch);
    }
}

pub(super) fn effacer_caractere_saisie(saisie: &mut String, angle: bool) {
    if angle || !saisie.contains(['<', '>']) { saisie.pop(); }
    else if let Some(i) = saisie.find(['<', '>']) && i > 0 { saisie.remove(i - 1); }
}

/// Point provisoire issu de la saisie, sans effacer le texte en cours.
/// Un angle seul projette le curseur sur son guide, comme dans le projet.
pub(super) fn apercu_point(texte: &str, precedent: Option<DVec2>, curseur: DVec2, facteur: f64) -> Option<(DVec2, Option<f64>)> {
    if texte.is_empty() { return None; }
    if let Some((distance, angle)) = texte.split_once(['<', '>']) {
        let depart = precedent?;
        let degres = angle.trim_end_matches('>').replace(',', ".").parse::<f64>().ok()?;
        if !degres.is_finite() { return None; }
        let direction = DVec2::from_angle(degres.to_radians());
        let longueur = distance.replace(',', ".").parse::<f64>().ok().map(|d| d * facteur)
            .filter(|d| d.is_finite() && *d > 0.0)
            .unwrap_or_else(|| (curseur - depart).dot(direction).max(0.0));
        return Some((depart + direction * longueur, Some(degres)));
    }
    let direction = precedent.map(|p| curseur - p);
    let point = crate::dessin::dao::lire_saisie(texte, precedent, direction, facteur).ok()?;
    point.is_finite().then_some((point, None))
}

#[derive(Default)]
pub(super) struct SaisieTrace {
    pub texte: String,
    angle: bool,
}

impl SaisieTrace {
    pub fn vider(&mut self) { self.texte.clear(); self.angle = false; }

    /// Lit uniquement les caractères de dessin ; Entrée valide un point saisi.
    pub fn clavier(&mut self, ctx: &egui::Context, en_cours: bool) -> bool {
        if ctx.egui_wants_keyboard_input() { return false; }
        let mut valider = false;
        let mut tab_utilise = false;
        ctx.input_mut(|i| i.events.retain(|event| match event {
            egui::Event::Text(t) if t.chars().all(|c| c.is_ascii_digit() || "@#,;.-<>".contains(c)) => {
                for c in t.chars() { ajouter_caractere_saisie(&mut self.texte, &mut self.angle, c); }
                false
            }
            egui::Event::Key { key: egui::Key::Backspace, pressed: true, .. } if !self.texte.is_empty() => {
                effacer_caractere_saisie(&mut self.texte, self.angle); false
            }
            egui::Event::Key { key: egui::Key::Tab, pressed: true, modifiers, .. } if en_cours && modifiers.is_none() && !self.texte.starts_with(['@', '#']) => {
                tab_utilise = true;
                self.angle = !self.angle;
                if self.angle && !self.texte.contains(['<', '>']) { self.texte.push('<'); }
                false
            }
            egui::Event::Key { key: egui::Key::Enter | egui::Key::Space, pressed: true, .. } if !self.texte.is_empty() => { valider = true; false }
            egui::Event::Text(t) if t == " " && !self.texte.is_empty() => { valider = true; false }
            _ => true,
        }));
        if tab_utilise { ctx.memory_mut(|m| m.move_focus(egui::FocusDirection::None)); }
        valider
    }

    pub fn point(&mut self, precedent: Option<DVec2>, curseur: Option<DVec2>, facteur: f64) -> Result<DVec2, String> {
        let direction = precedent.zip(curseur).map(|(p, c)| c - p);
        let point = crate::dessin::dao::lire_saisie(&self.texte, precedent, direction, facteur)?;
        if !point.is_finite() { return Err("Les coordonnées doivent être finies.".into()); }
        self.vider();
        Ok(point)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn apercu_applique_distance_angle_et_coordonnees_sans_mutation() {
        let p = DVec2::new(10.0, 20.0);
        let (q, angle) = apercu_point("2<90", Some(p), DVec2::new(50.0, 70.0), 10.0).unwrap();
        assert!((q - DVec2::new(10.0, 40.0)).length() < 1e-9);
        assert_eq!(angle, Some(90.0));
        let (q, _) = apercu_point("<90", Some(p), DVec2::new(50.0, 70.0), 10.0).unwrap();
        assert!((q - DVec2::new(10.0, 70.0)).length() < 1e-9);
        assert_eq!(apercu_point("@2;3", None, DVec2::ZERO, 10.0).unwrap().0, DVec2::new(20.0, 30.0));
        assert!(apercu_point("2<", Some(p), DVec2::ZERO, 10.0).is_none());
    }

    #[test]
    fn coordonnees_utilisent_le_meme_interpreteur_et_conservent_les_erreurs() {
        let mut saisie = SaisieTrace { texte: "#2,5;1".into(), ..Default::default() };
        assert_eq!(saisie.point(Some(DVec2::new(10.0, 20.0)), None, 10.0).unwrap(), DVec2::new(35.0, 30.0));
        assert!(saisie.texte.is_empty());
        saisie.texte = "@oops".into();
        assert!(saisie.point(None, None, 1.0).is_err());
        assert_eq!(saisie.texte, "@oops");
        saisie.texte = "@inf;0".into();
        assert!(saisie.point(None, None, 1.0).is_err());
    }
}
