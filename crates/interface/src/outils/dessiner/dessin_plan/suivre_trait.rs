//! Raccourcis clavier du mode « Suivre un trait » de l'outil Mur (la polyligne n'en a pas).
use eframe::egui;

/// Raccourcis du mode « Suivre un trait » : (Ctrl seul pressé puis relâché, Espace).
/// `controle_seul` mémorise d'une image à l'autre qu'aucune autre touche n'a accompagné Ctrl.
pub(super) fn raccourcis(ctx: &egui::Context, controle_seul: &mut bool) -> (bool, bool) {
    let (mut justifier, mut inverser) = (false, false);
    ctx.input_mut(|i| i.events.retain(|e| match e {
        egui::Event::Key { key: egui::Key::ControlLeft | egui::Key::ControlRight, pressed: true, repeat: false, .. } => { *controle_seul = true; true }
        egui::Event::Key { key: egui::Key::ControlLeft | egui::Key::ControlRight, pressed: false, .. } => {
            justifier |= *controle_seul;
            *controle_seul = false;
            true
        }
        egui::Event::Key { pressed: true, modifiers, .. } if modifiers.ctrl => { *controle_seul = false; true }
        egui::Event::Text(t) if t == " " => { inverser = true; false }
        egui::Event::Key { key: egui::Key::Space, .. } => false,
        _ => true,
    }));
    (justifier, inverser)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn image(ctx: &egui::Context, events: Vec<egui::Event>, controle: &mut bool) -> (bool, bool) {
        let mut r = (false, false);
        let mut sortie = ctx.run_ui(egui::RawInput { events, ..Default::default() }, |ui| r = raccourcis(ui.ctx(), controle));
        // sans rendu, les textures ne sont pas appliquées : epaint refuse de les jeter en debug
        sortie.textures_delta.clear();
        r
    }

    fn touche(key: egui::Key, pressed: bool, ctrl: bool) -> egui::Event {
        egui::Event::Key { key, physical_key: None, pressed, repeat: false, modifiers: egui::Modifiers { ctrl, command: ctrl, ..Default::default() } }
    }

    /// N° 24 : Ctrl seul change l'implantation, Ctrl+autre touche non ; Espace inverse.
    #[test]
    fn ctrl_seul_et_espace() {
        let ctx = egui::Context::default();
        let mut controle = false;
        assert_eq!(image(&ctx, vec![touche(egui::Key::ControlLeft, true, true)], &mut controle), (false, false));
        assert_eq!(image(&ctx, vec![touche(egui::Key::ControlLeft, false, false)], &mut controle), (true, false));
        assert_eq!(image(&ctx, vec![touche(egui::Key::ControlLeft, true, true), touche(egui::Key::Z, true, true), touche(egui::Key::ControlLeft, false, false)], &mut controle), (false, false));
        assert_eq!(image(&ctx, vec![egui::Event::Text(" ".into())], &mut controle), (false, true));
    }
}
