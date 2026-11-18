//! Fond des vues : seule source de la couleur de fond, pour le rendu du projet, celui
//! des familles et les captures du pilote.
use crate::gpu::StyleVisuel;
use eframe::egui;

/// Fond d'une vue : blanc en style graphique (papier), gris clair sinon.
pub fn fond_vue(style: StyleVisuel) -> [f32; 3] {
    if style.papier() { [1.0, 1.0, 1.0] } else { [0.925, 0.933, 0.945] }
}

/// Même fond en couleur egui (zones peintes avant la première image du rendu).
pub fn couleur_fond_vue(style: StyleVisuel) -> egui::Color32 {
    let [r, v, b] = fond_vue(style).map(|c| (c * 255.0).round() as u8);
    egui::Color32::from_rgb(r, v, b)
}

#[cfg(test)]
mod tests {
    #[test]
    fn fond_blanc_en_style_graphique() {
        assert_eq!(super::fond_vue(crate::gpu::StyleVisuel::Graphique), [1.0; 3]);
        assert_eq!(super::couleur_fond_vue(crate::gpu::StyleVisuel::Graphique), super::egui::Color32::WHITE);
    }
}
