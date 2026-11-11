//! Barre de commandes générique (bandeau) et ses icônes.
use super::*;

impl Commande {
    pub fn new(cle: &'static str, nom: &'static str, icone: Icone, aide: &'static str, actif: bool) -> Commande {
        Commande { cle, nom, icone, aide, actif, enfonce: false }
    }
}

/// Bandeau à grandes icônes, de la hauteur des bandeaux d'outils : groupes séparés,
/// nom du groupe en bas. Rend la réponse de chaque commande (clé, réponse).
pub fn bandeau(ui: &mut egui::Ui, groupes: &[(&str, Vec<Commande>)]) -> Vec<(&'static str, egui::Response)> {
    let mut reponses = Vec::new();
    egui::ScrollArea::horizontal()
        .id_salt("ruban_commandes_fixes")
        .auto_shrink([false, true])
        .show(ui, |ui| {
            ui.spacing_mut().item_spacing.x = ESPACE_GROUPES;
            ui.horizontal_top(|ui| {
                reponses = bandeau_contenu(ui, groupes);
            });
        });
    reponses
}

/// Contenu insérable dans une ligne de ruban déjà défilante.
pub(super) fn bandeau_contenu(ui: &mut egui::Ui, groupes: &[(&str, Vec<Commande>)]) -> Vec<(&'static str, egui::Response)> {
    let mut reponses = Vec::new();
    for (indice, (nom, commandes)) in groupes.iter().enumerate() {
        let largeurs: Vec<f32> = commandes.iter().map(|c| (largeur_texte(ui, c.nom, 12.0) + 20.0).max(62.0)).collect();
        let largeur = (largeurs.iter().sum::<f32>() + ESPACE_COLONNES * largeurs.len().saturating_sub(1) as f32).max(largeur_texte(ui, nom, 11.0) + 8.0);
        let (rect, _) = ui.allocate_exact_size(egui::vec2(largeur, HAUTEUR_GROUPE), Sense::hover());
        if indice > 0 {
            let x = rect.left() - ESPACE_GROUPES / 2.0;
            ui.painter().line_segment([egui::pos2(x, rect.top() + 4.0), egui::pos2(x, rect.bottom() - 4.0)], ui.visuals().widgets.noninteractive.bg_stroke);
        }
        let mut x = rect.left();
        for (c, l) in commandes.iter().zip(largeurs) {
            let r = egui::Rect::from_min_size(egui::pos2(x, rect.top()), egui::vec2(l, HAUTEUR_OUTILS));
            let mut sous = ui.new_child(egui::UiBuilder::new().max_rect(r));
            if !c.actif {
                sous.disable();
            }
            let resp = sous.interact(r, sous.scope_id().with(("commande", c.cle)), Sense::click());
            if c.enfonce || (c.actif && resp.hovered()) {
                sous.painter().rect_filled(r.shrink(1.0), 4.0, if c.enfonce { sous.visuals().selection.bg_fill } else { sous.visuals().widgets.hovered.weak_bg_fill });
            }
            let couleur = if !c.actif {
                sous.visuals().weak_text_color().gamma_multiply(0.6)
            } else if c.enfonce {
                sous.visuals().selection.stroke.color
            } else {
                sous.visuals().text_color()
            };
            icone_commande(sous.painter(), egui::Rect::from_center_size(egui::pos2(r.center().x, r.top() + 24.0), egui::vec2(32.0, 32.0)), c.icone, couleur);
            sous.painter().text(egui::pos2(r.center().x, r.bottom() - 12.0), egui::Align2::CENTER_CENTER, c.nom, egui::FontId::proportional(12.0), couleur);
            resp.widget_info(|| egui::WidgetInfo::selected(egui::Role::Button, c.actif, c.enfonce, c.nom));
            let resp = if c.aide.is_empty() { resp } else { resp.on_hover_text(c.aide).on_disabled_hover_text(c.aide) };
            reponses.push((c.cle, resp));
            x += l + ESPACE_COLONNES;
        }
        ui.painter().text(egui::pos2(rect.center().x, rect.bottom() - 8.0), egui::Align2::CENTER_CENTER, *nom, egui::FontId::proportional(11.0), ui.visuals().weak_text_color());
    }
    reponses
}

/// Les familles planes gardent les commandes de volume visibles, sans les activer.
pub(crate) fn formes_famille_planes() -> Vec<Commande> {
    [("volume", "Volume"), ("vide", "Vide"), ("extrusion", "Extrusion"), ("extrusion_vide", "Extrusion vide"), ("revolution", "Révolution"), ("revolution_vide", "Révolution vide"), ("extrusion_chemin", "Extrusion par chemin"), ("raccordement", "Raccordement"), ("raccordement_chemin", "Raccordement par chemin")]
        .into_iter().map(|(cle, nom)| Commande::new(cle, nom, Icone::Famille(cle),
            "Cette famille est plane : les formes volumiques sont réservées aux portes, fenêtres et composants.", false)).collect()
}

/// Pictogrammes génériques des commandes de fichier.
pub(super) fn icone_commande(p: &egui::Painter, r: egui::Rect, i: Icone, c: Color32) {
    let t = egui::Stroke::new(1.4, c);
    let pt = |x: f32, y: f32| egui::pos2(r.left() + x * r.width(), r.top() + y * r.height());
    let seg = |a: (f32, f32), b: (f32, f32)| {
        p.line_segment([pt(a.0, a.1), pt(b.0, b.1)], t);
    };
    // document à coin replié
    let document = || {
        p.add(egui::Shape::closed_line(vec![pt(0.2, 0.06), pt(0.6, 0.06), pt(0.8, 0.26), pt(0.8, 0.94), pt(0.2, 0.94)], t));
        seg((0.6, 0.06), (0.6, 0.26));
        seg((0.6, 0.26), (0.8, 0.26));
    };
    // disquette
    let disquette = || {
        p.add(egui::Shape::closed_line(vec![pt(0.12, 0.12), pt(0.74, 0.12), pt(0.88, 0.26), pt(0.88, 0.88), pt(0.12, 0.88)], t));
        p.rect_stroke(egui::Rect::from_min_max(pt(0.3, 0.12), pt(0.66, 0.34)), 0.0, t, egui::StrokeKind::Inside);
        p.rect_stroke(egui::Rect::from_min_max(pt(0.26, 0.54), pt(0.74, 0.88)), 0.0, t, egui::StrokeKind::Inside);
    };
    match i {
        Icone::Collaborer => {
            for(x,y)in [(0.25,0.3),(0.75,0.3),(0.5,0.7)]{p.circle_stroke(pt(x,y),r.width()*0.1,t);}
            for(a,b)in [((0.25,0.42),(0.5,0.59)),((0.75,0.42),(0.5,0.59)),((0.38,0.3),(0.62,0.3))]{seg(a,b);}
        }
        Icone::Synchro => {
            seg((0.2,0.35),(0.8,0.35));seg((0.8,0.35),(0.62,0.17));seg((0.8,0.35),(0.62,0.53));
            seg((0.8,0.68),(0.2,0.68));seg((0.2,0.68),(0.38,0.5));seg((0.2,0.68),(0.38,0.86));
        }
        Icone::Nord => {
            p.circle_stroke(r.center(), r.width() * 0.42, t);
            p.add(egui::Shape::convex_polygon(vec![pt(0.5, 0.12), pt(0.37, 0.62), pt(0.5, 0.53), pt(0.63, 0.62)], c, egui::Stroke::NONE));
            seg((0.5, 0.53), (0.5, 0.84));
        }
        Icone::Coordonnees => {
            // repère : Z vers le haut, X et Y en perspective, origine pleine ; repère NGF
            // (trait de niveau) à droite
            let o = (0.38, 0.66);
            seg(o, (0.38, 0.12));
            seg(o, (0.88, 0.66));
            seg(o, (0.12, 0.9));
            p.circle_filled(pt(o.0, o.1), r.width() * 0.06, c);
            p.add(egui::Shape::convex_polygon(vec![pt(0.38, 0.08), pt(0.31, 0.22), pt(0.45, 0.22)], c, egui::Stroke::NONE));
            seg((0.6, 0.3), (0.92, 0.3));
            p.add(egui::Shape::convex_polygon(vec![pt(0.76, 0.3), pt(0.69, 0.18), pt(0.83, 0.18)], c, egui::Stroke::NONE));
        }
        Icone::Lieu => {
            // globe (méridien, parallèles) et repère de position
            let c0 = pt(0.44, 0.56);
            let rg = r.width() * 0.34;
            p.circle_stroke(c0, rg, t);
            p.add(egui::Shape::ellipse_stroke(c0, egui::vec2(rg * 0.45, rg), t));
            seg((0.1, 0.56), (0.78, 0.56));
            seg((0.15, 0.39), (0.73, 0.39));
            seg((0.15, 0.73), (0.73, 0.73));
            p.circle_filled(pt(0.76, 0.22), r.width() * 0.13, c);
            p.add(egui::Shape::convex_polygon(vec![pt(0.66, 0.27), pt(0.86, 0.27), pt(0.76, 0.46)], c, egui::Stroke::NONE));
        }
        Icone::Nouveau => {
            document();
            seg((0.5, 0.48), (0.5, 0.78));
            seg((0.35, 0.63), (0.65, 0.63));
        }
        Icone::Ouvrir => {
            p.add(egui::Shape::closed_line(vec![pt(0.06, 0.2), pt(0.36, 0.2), pt(0.44, 0.3), pt(0.86, 0.3), pt(0.86, 0.84), pt(0.06, 0.84)], t));
            p.add(egui::Shape::line(vec![pt(0.06, 0.84), pt(0.22, 0.46), pt(0.96, 0.46), pt(0.86, 0.84)], t));
        }
        Icone::Fermer => {
            document();
            seg((0.36, 0.48), (0.64, 0.76));
            seg((0.64, 0.48), (0.36, 0.76));
        }
        Icone::Importer => {
            p.add(egui::Shape::line(vec![pt(0.14, 0.5), pt(0.14, 0.9), pt(0.86, 0.9), pt(0.86, 0.5)], t));
            seg((0.5, 0.08), (0.5, 0.66));
            p.add(egui::Shape::line(vec![pt(0.32, 0.48), pt(0.5, 0.66), pt(0.68, 0.48)], t));
        }
        Icone::Fiche => {
            document();
            for y in [0.44, 0.6, 0.76] {
                seg((0.32, y), (0.68, y));
            }
        }
        Icone::Options => {
            let m = r.center();
            p.circle_stroke(m, r.width() * 0.2, t);
            for k in 0..8 {
                let a = k as f32 * std::f32::consts::TAU / 8.0;
                let d = egui::vec2(a.cos(), a.sin());
                p.line_segment([m + d * r.width() * 0.3, m + d * r.width() * 0.44], egui::Stroke::new(3.0, c));
            }
            p.circle_stroke(m, r.width() * 0.32, t);
        }
        Icone::Enregistrer => disquette(),
        Icone::EnregistrerSous => {
            disquette();
            p.add(egui::Shape::convex_polygon(vec![pt(0.62, 1.0), pt(1.0, 0.62), pt(1.06, 0.68), pt(0.68, 1.06)], c, egui::Stroke::NONE));
        }
        Icone::Exporter(format) => {
            document();
            p.text(pt(0.5, 0.64), egui::Align2::CENTER_CENTER, format, egui::FontId::proportional(r.width() * 0.26), c);
        }
        Icone::Charger => {
            document();
            seg((0.02, 0.58), (0.5, 0.58));
            p.add(egui::Shape::line(vec![pt(0.32, 0.42), pt(0.5, 0.58), pt(0.32, 0.74)], t));
        }
        Icone::Famille(cle) => {
            let cadre = |x: f32, y: f32, w: f32, h: f32| {
                p.rect_stroke(egui::Rect::from_min_max(pt(x, y), pt(x + w, y + h)), 0.0, t, egui::StrokeKind::Inside);
            };
            let tirets = |a: (f32, f32), b: (f32, f32)| {
                let n = 6;
                for k in (0..n).step_by(2) {
                    let (u, v) = (k as f32 / n as f32, (k + 1) as f32 / n as f32);
                    seg((a.0 + (b.0 - a.0) * u, a.1 + (b.1 - a.1) * u), (a.0 + (b.0 - a.0) * v, a.1 + (b.1 - a.1) * v));
                }
            };
            let boite_tiretee = |x: f32, y: f32, w: f32, h: f32| {
                tirets((x, y), (x + w, y));
                tirets((x + w, y), (x + w, y + h));
                tirets((x + w, y + h), (x, y + h));
                tirets((x, y + h), (x, y));
            };
            match cle {
                "elevation" => {
                    cadre(0.16, 0.1, 0.68, 0.8);
                    cadre(0.3, 0.24, 0.4, 0.52);
                    seg((0.5, 0.24), (0.5, 0.76));
                }
                "plan" => {
                    cadre(0.08, 0.3, 0.84, 0.4);
                    seg((0.08, 0.5), (0.92, 0.5));
                    seg((0.5, 0.06), (0.5, 0.3));
                    p.add(egui::Shape::convex_polygon(vec![pt(0.5, 0.06), pt(0.42, 0.2), pt(0.58, 0.2)], c, egui::Stroke::NONE));
                }
                "coupe" => {
                    cadre(0.3, 0.1, 0.4, 0.8);
                    for y in [0.3, 0.46, 0.62, 0.78] {
                        seg((0.3, y + 0.1), (0.7, y - 0.1));
                    }
                }
                "plan_ref" => {
                    // plan tracé par deux points
                    tirets((0.04, 0.5), (0.96, 0.5));
                    p.circle_filled(pt(0.22, 0.5), r.width() * 0.07, c);
                    p.circle_filled(pt(0.78, 0.5), r.width() * 0.07, c);
                }
                "cote" => {
                    seg((0.14, 0.2), (0.14, 0.8));
                    seg((0.86, 0.2), (0.86, 0.8));
                    seg((0.14, 0.5), (0.86, 0.5));
                    p.add(egui::Shape::convex_polygon(vec![pt(0.14, 0.5), pt(0.26, 0.42), pt(0.26, 0.58)], c, egui::Stroke::NONE));
                    p.add(egui::Shape::convex_polygon(vec![pt(0.86, 0.5), pt(0.74, 0.42), pt(0.74, 0.58)], c, egui::Stroke::NONE));
                }
                "cote_angle" => {
                    seg((0.12, 0.86), (0.9, 0.86));
                    seg((0.12, 0.86), (0.68, 0.14));
                    p.add(egui::Shape::line((0..=8).map(|i| { let a = 0.91 * i as f32 / 8.0; pt(0.12 + 0.52 * a.cos(), 0.86 - 0.52 * a.sin()) }).collect(), t));
                }
                "cote_diametre" => {
                    p.circle_stroke(pt(0.5, 0.5), r.width() * 0.36, t);
                    seg((0.22, 0.78), (0.78, 0.22));
                }
                "altimetrie" => {
                    seg((0.06, 0.66), (0.94, 0.66));
                    p.add(egui::Shape::closed_line(vec![pt(0.32, 0.66), pt(0.18, 0.44), pt(0.46, 0.44)], t));
                    seg((0.46, 0.44), (0.9, 0.44));
                }
                "egalite" => {
                    for x in [0.12, 0.5, 0.88] {
                        seg((x, 0.14), (x, 0.5));
                    }
                    seg((0.12, 0.32), (0.88, 0.32));
                    p.text(pt(0.5, 0.74), egui::Align2::CENTER_CENTER, "EQ", egui::FontId::proportional(r.width() * 0.3), c);
                }
                "volume" => {
                    p.add(egui::Shape::convex_polygon(vec![pt(0.14, 0.32), pt(0.6, 0.32), pt(0.6, 0.88), pt(0.14, 0.88)], c.gamma_multiply(0.3), t));
                    p.add(egui::Shape::convex_polygon(vec![pt(0.14, 0.32), pt(0.38, 0.1), pt(0.84, 0.1), pt(0.6, 0.32)], c.gamma_multiply(0.15), t));
                    p.add(egui::Shape::convex_polygon(vec![pt(0.6, 0.32), pt(0.84, 0.1), pt(0.84, 0.66), pt(0.6, 0.88)], c.gamma_multiply(0.45), t));
                }
                "vide" => {
                    boite_tiretee(0.14, 0.3, 0.46, 0.58);
                    tirets((0.14, 0.3), (0.38, 0.1));
                    tirets((0.6, 0.3), (0.84, 0.1));
                    tirets((0.38, 0.1), (0.84, 0.1));
                    tirets((0.84, 0.1), (0.84, 0.64));
                    tirets((0.6, 0.88), (0.84, 0.64));
                }
                "extrusion_chemin" | "raccordement" | "raccordement_chemin" => {
                    let raccord = cle != "extrusion_chemin";
                    let haut = if raccord { 0.18 } else { 0.3 };
                    p.rect_stroke(egui::Rect::from_min_max(pt(0.08,0.62),pt(0.38,0.92)),0.0,t,egui::StrokeKind::Inside);
                    p.rect_stroke(egui::Rect::from_min_max(pt(0.62,haut),pt(0.92,0.48)),0.0,t,egui::StrokeKind::Inside);
                    if cle == "raccordement" {
                        seg((0.08,0.62),(0.62,haut)); seg((0.38,0.92),(0.92,0.48));
                    } else {
                        p.add(egui::Shape::line(vec![pt(0.23,0.77),pt(0.46,0.77),pt(0.46,0.33),pt(0.77,0.33)],t));
                    }
                }
                "revolution" | "revolution_vide" => {
                    tirets((0.5,0.08),(0.5,0.94));
                    let courbe: Vec<_> = (0..=24).map(|i| {
                        let angle = i as f32 / 24.0 * std::f32::consts::TAU * 0.85;
                        pt(0.5+0.36*angle.cos(),0.55+0.23*angle.sin())
                    }).collect();
                    if cle == "revolution_vide" { p.add(egui::Shape::dashed_line(&courbe,t,4.0,3.0)); }
                    else { p.add(egui::Shape::line(courbe,t)); }
                    seg((0.68,0.27),(0.71,0.39));
                    seg((0.71,0.39),(0.83,0.36));
                    seg((0.62,0.14),(0.8,0.14));
                    seg((0.8,0.14),(0.8,0.3));
                }
                "extrusion" => {
                    p.add(egui::Shape::convex_polygon(vec![pt(0.12, 0.84), pt(0.12, 0.34), pt(0.44, 0.16), pt(0.76, 0.34), pt(0.76, 0.84)], c.gamma_multiply(0.3), t));
                    seg((0.76, 0.34), (0.9, 0.24));
                    seg((0.76, 0.84), (0.9, 0.74));
                    seg((0.9, 0.24), (0.9, 0.74));
                }
                "extrusion_vide" => {
                    for (a, b) in [((0.12, 0.84), (0.12, 0.34)), ((0.12, 0.34), (0.44, 0.16)), ((0.44, 0.16), (0.76, 0.34)), ((0.76, 0.34), (0.76, 0.84)), ((0.76, 0.84), (0.12, 0.84))] {
                        tirets(a, b);
                    }
                    tirets((0.76, 0.34), (0.9, 0.24));
                    tirets((0.76, 0.84), (0.9, 0.74));
                    tirets((0.9, 0.24), (0.9, 0.74));
                }
                "ligne" => {
                    p.add(egui::Shape::line(vec![pt(0.12, 0.8), pt(0.4, 0.3), pt(0.64, 0.6), pt(0.88, 0.16)], t));
                    for (x, y) in [(0.12, 0.8), (0.4, 0.3), (0.64, 0.6), (0.88, 0.16)] {
                        p.circle_filled(pt(x, y), r.width() * 0.05, c);
                    }
                }
                "region" => {
                    let contour = vec![pt(0.14, 0.82), pt(0.2, 0.22), pt(0.62, 0.14), pt(0.86, 0.5), pt(0.6, 0.86)];
                    p.add(egui::Shape::convex_polygon(contour.clone(), c.gamma_multiply(0.25), t));
                    for k in 0..4 {
                        let x = 0.26 + 0.16 * k as f32;
                        seg((x, 0.78), (x + 0.2, 0.3));
                    }
                }
                "arc" => {
                    let pts: Vec<egui::Pos2> = (0..=16).map(|k| {
                        let a = std::f32::consts::PI * (1.0 + k as f32 / 16.0);
                        pt(0.5 + 0.38 * a.cos(), 0.78 + 0.6 * a.sin())
                    }).collect();
                    p.add(egui::Shape::line(pts, t));
                    for (x, y) in [(0.12, 0.78), (0.5, 0.18), (0.88, 0.78)] {
                        p.circle_filled(pt(x, y), r.width() * 0.05, c);
                    }
                }
                _ => cadre(0.2, 0.2, 0.6, 0.6),
            }
        }
    }
}
