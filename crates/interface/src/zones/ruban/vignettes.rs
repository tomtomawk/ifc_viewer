//! Vignettes des sélecteurs simples (`selecteur::simple`) : terrain, arbre, composant,
//! pièce, poutre. Dessins schématiques à la couleur du trait, dans la zone donnée.
use eframe::egui::{self, Color32, Stroke};

/// Couleur d'un matériau du projet (gris s'il est absent ou non donné).
pub(crate) fn couleur_materiau(p: &modele::Projet, nom: Option<&str>) -> Color32 {
    nom.and_then(|n| p.materiau(n)).map_or(Color32::from_gray(150), |m| Color32::from_rgb(m.couleur[0], m.couleur[1], m.couleur[2]))
}

/// Coupe d'un terrain : bandes des couches (haut en premier), dessus ondulé.
pub(crate) fn terrain(pe: &egui::Painter, r: egui::Rect, couches: &[Color32], c: Color32) {
    let n = couches.len().max(1) as f32;
    let haut = r.top() + r.height() * 0.25;
    for (k, couleur) in couches.iter().enumerate() {
        let y0 = haut + (r.bottom() - haut) * k as f32 / n;
        let y1 = haut + (r.bottom() - haut) * (k + 1) as f32 / n;
        pe.rect_filled(egui::Rect::from_min_max(egui::pos2(r.left(), y0), egui::pos2(r.right(), y1)), 0.0, *couleur);
    }
    let points: Vec<egui::Pos2> = (0..=8).map(|i| {
        let x = r.left() + r.width() * i as f32 / 8.0;
        egui::pos2(x, haut - (i as f32 * 0.9).sin() * r.height() * 0.12)
    }).collect();
    pe.add(egui::Shape::line(points, Stroke::new(1.2, c)));
    pe.rect_stroke(egui::Rect::from_min_max(egui::pos2(r.left(), haut), r.max), 0.0, Stroke::new(0.8, c), egui::StrokeKind::Inside);
}

/// Arbre en élévation : houppier et tronc.
pub(crate) fn arbre(pe: &egui::Painter, r: egui::Rect, c: Color32) {
    let centre = egui::pos2(r.center().x, r.top() + r.height() * 0.4);
    let rayon = r.height().min(r.width()) * 0.38;
    pe.circle(centre, rayon, Color32::from_rgba_unmultiplied(90, 150, 80, 120), Stroke::new(1.0, c));
    pe.line_segment([egui::pos2(centre.x, centre.y + rayon), egui::pos2(centre.x, r.bottom())], Stroke::new(1.5, c));
}

/// Composant en plan : rectangle aux proportions du type.
pub(crate) fn composant(pe: &egui::Painter, r: egui::Rect, largeur: f64, profondeur: f64, c: Color32) {
    pe.rect_stroke(proportions(r, largeur, profondeur), 1.0, Stroke::new(1.2, c), egui::StrokeKind::Inside);
}

/// Étiquette de pièce : cadre et deux lignes de texte.
pub(crate) fn piece(pe: &egui::Painter, r: egui::Rect, c: Color32) {
    let cadre = r.shrink2(egui::vec2(r.width() * 0.12, r.height() * 0.15));
    pe.rect_stroke(cadre, 2.0, Stroke::new(1.0, c), egui::StrokeKind::Inside);
    for (y, l) in [(0.38, 0.6), (0.65, 0.35)] {
        let y = cadre.top() + cadre.height() * y;
        pe.line_segment([egui::pos2(cadre.center().x - cadre.width() * l / 2.0, y), egui::pos2(cadre.center().x + cadre.width() * l / 2.0, y)], Stroke::new(1.5, c));
    }
}

/// Section de poutre aux proportions largeur × hauteur, hachurée.
pub(crate) fn poutre(pe: &egui::Painter, r: egui::Rect, largeur: f64, hauteur: f64, c: Color32) {
    let s = proportions(r, largeur, hauteur);
    pe.rect_filled(s, 0.0, c.gamma_multiply(0.25));
    pe.rect_stroke(s, 0.0, Stroke::new(1.2, c), egui::StrokeKind::Inside);
}

/// Rectangle centré dans `r` aux proportions `l` × `h`.
fn proportions(r: egui::Rect, l: f64, h: f64) -> egui::Rect {
    let (l, h) = (l.max(1.0) as f32, h.max(1.0) as f32);
    let k = (r.width() * 0.9 / l).min(r.height() * 0.9 / h);
    egui::Rect::from_center_size(r.center(), egui::vec2((l * k).max(3.0), (h * k).max(3.0)))
}
