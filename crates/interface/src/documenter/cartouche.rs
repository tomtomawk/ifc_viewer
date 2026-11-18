//! Peinture d'un rendu 2D de famille (cartouche, étiquette, profil) dans un cadre de vue.
use crate::*;
use crate::familles::editeur_famille::navigation::Cadre;

pub(crate) fn peindre(painter: &egui::Painter, cadre: &Cadre, dessin: crate::documenter::feuilles::Rendu, selection: Option<Color32>) {
    let ecran = |q: egui::Pos2| cadre.ecran(q.x as f64, q.y as f64);
    let mut mesh = egui::Mesh::default();
    for (t, c) in dessin.tris {
        let i = mesh.vertices.len() as u32;
        for q in t { mesh.colored_vertex(ecran(q), selection.unwrap_or(c)); }
        mesh.add_triangle(i, i + 1, i + 2);
    }
    if !mesh.is_empty() { painter.add(egui::Shape::mesh(mesh)); }
    for (t, c, e) in dessin.traits {
        painter.line_segment(t.map(ecran), egui::Stroke::new((e * cadre.echelle as f32).max(1.0), selection.unwrap_or(c)));
    }
}
