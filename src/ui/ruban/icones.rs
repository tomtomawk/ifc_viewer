//! Boutons, bandes de couches et icônes vectorielles des outils.
use super::*;

pub(super) fn bouton_outil(
    ui: &mut egui::Ui,
    r: egui::Rect,
    outil: Outil,
    grand: bool,
    serre: bool,
    actif: bool,
) -> egui::Response {
    let resp = ui.interact(r, ui.scope_id().with(("outil", outil)), Sense::click());
    if actif || resp.hovered() {
        ui.painter().rect_filled(
            r.shrink(1.0),
            4.0,
            if actif {
                ui.visuals().selection.bg_fill
            } else {
                ui.visuals().widgets.hovered.weak_bg_fill
            },
        );
    }
    let couleur = if actif {
        ui.visuals().selection.stroke.color
    } else {
        ui.visuals().text_color()
    };
    let (centre, taille, texte, ancrage) = if grand {
        (
            egui::pos2(r.center().x, r.top() + 24.0),
            32.0,
            egui::pos2(r.center().x, r.bottom() - 12.0),
            egui::Align2::CENTER_CENTER,
        )
    } else if serre {
        (
            egui::pos2(r.left() + 9.0, r.center().y),
            14.0,
            egui::pos2(r.left() + 19.0, r.center().y),
            egui::Align2::LEFT_CENTER,
        )
    } else {
        (
            egui::pos2(r.left() + 12.0, r.center().y),
            17.0,
            egui::pos2(r.left() + 27.0, r.center().y),
            egui::Align2::LEFT_CENTER,
        )
    };
    icone(
        ui.painter(),
        egui::Rect::from_center_size(centre, egui::vec2(taille, taille)),
        outil,
        couleur,
    );
    ui.painter().text(
        texte,
        ancrage,
        outil.nom(),
        egui::FontId::proportional(if serre && !grand { 11.0 } else { 12.0 }),
        couleur,
    );
    resp.widget_info(|| {
        egui::WidgetInfo::selected(egui::Role::Button, ui.is_enabled(), actif, outil.nom())
    });
    resp.on_hover_text(match outil {
        Outil::ModifierContour => "Modifier l'élément sélectionné : tracé d'un contour, d'une polyligne, d'un terrain, d'un garde-corps ou d'une limite de propriété ; famille d'une ouverture, d'un composant ou d'une étiquette (MT)",
        Outil::CopierNiveau => "Copier la sélection sur un niveau en conservant la position en plan",
        Outil::Jonctions => "Priorité des jonctions entre murs",
        Outil::AttacherToiture => "Attacher un ou plusieurs murs à la sous-face d'une toiture : sélectionner les murs puis cliquer la toiture",
        Outil::CreerIdentique => "Créer à l'identique (CS)",
        Outil::Deplacer => "Déplacer (MV) : point de base, puis destination ou distance tapée",
        Outil::Copier => "Copier (CO) : point de base, puis destination ; Multiple pour enchaîner",
        Outil::Tourner => "Tourner (RO) : direction de départ, puis d'arrivée ou angle tapé ; clic sur le centre pour le déplacer",
        Outil::Symetrie => "Symétrie (SY) : deux points de l'axe ; Copier coché par défaut",
        Outil::Reseau => "Réseau (RE) : linéaire (base puis 2ᵉ élément) ou polaire (centre, angle entre éléments)",
        Outil::Echelle => "Échelle (EC) : point de base, longueur de référence, nouvelle longueur ou facteur tapé ; annotations, contours, terrains",
        Outil::Aligner => "Aligner (AL) : référence, puis éléments à aligner",
        Outil::Decaler => "Décaler (DE) : clic sur le mur, puis côté et distance ; Copier coché par défaut",
        Outil::Scinder => "Scinder (SC) : clic sur le mur ou la poutre au point de coupure ; écart réglable",
        Outil::Demolir => "Démolir (DM) : clic sur l'élément, démoli dans la dernière phase ; clic à nouveau pour annuler",
        Outil::Reproduire => "Reproduire les propriétés (RP) : pipette sur la source, pinceau sur les cibles",
        Outil::ChangerHote => "Changer d'hôte (CH) : clic sur l'ouverture, puis sur le nouveau mur",
        Outil::Inverser => "Inverser (NV) : sens d'un mur, côté d'une ouverture (Maj : paumelles)",
        Outil::Supprimer => "Supprimer (Suppr)",
        Outil::Cadrer => "Définir le contour de cadrage du plan ; F ajuste le zoom",
        Outil::Isoler => "Isoler la sélection (I) ; Échap dans la vue : tout réafficher",
        Outil::Masquer => "Masquer temporairement la sélection (H) ; perdu à la fermeture de la vue",
        Outil::MasquerVue => "Masquer la sélection dans cette vue (MQ) ; enregistré avec la vue",
        Outil::Afficher => "Afficher les éléments masqués (AF) : en magenta, clic pour réafficher ; Échap pour sortir",
        Outil::Verrouiller => "Verrouiller la sélection (CD) ; déjà verrouillée : la déverrouiller. Un élément verrouillé n'est ni déplacé, ni modifié, ni supprimé",
        Outil::Deverrouiller => "Déverrouiller la sélection (DC) ; ou clic sur un cadenas dans la vue",
        Outil::Feuilles => "Feuilles : mise en page des vues, cartouche, export SVG/PDF",
        Outil::Nomenclatures => "Nomenclatures : quantités par catégorie et par type, export CSV",
        _ => outil.nom(),
    })
}

/// Composition d'un type en bande horizontale : une case par couche, largeur
/// proportionnelle à l'épaisseur, couleur du matériau ; extérieur à gauche.
pub(super) fn bande_couches_brouillon(ui: &mut egui::Ui, projet: &crate::modele::Projet, materiaux: &[crate::modele::Materiau], couches: &[crate::modele::Couche], hauteur: f32) {
    let (r, resp) = ui.allocate_exact_size(egui::vec2(ui.available_width(), hauteur), Sense::hover());
    if let Some(c) = peindre_bande_couches(ui.painter(), r, projet, materiaux, couches, resp.hover_pos()) {
        resp.on_hover_text(texte_couche(c));
    }
}

/// Plan des couches dans `r`, de l'extérieur (à gauche) vers l'intérieur, chaque couche
/// à la largeur de son épaisseur et à la couleur de son matériau. Rend la couche survolée.
pub(super) fn peindre_bande_couches<'a>(p: &egui::Painter, r: egui::Rect, projet: &crate::modele::Projet, materiaux: &[crate::modele::Materiau], couches: &'a [crate::modele::Couche], survol: Option<egui::Pos2>) -> Option<&'a crate::modele::Couche> {
    let total: f64 = couches.iter().map(|c| c.epaisseur.max(0.0)).sum();
    if total <= 0.0 {
        return None;
    }
    let mut x = r.left();
    let mut vise = None;
    for c in couches {
        let l = (c.epaisseur.max(0.0) / total) as f32 * r.width();
        let case = egui::Rect::from_min_size(egui::pos2(x, r.top()), egui::vec2(l, r.height()));
        let m = c.materiau.as_ref().and_then(|n| materiaux.iter().find(|m| &m.nom == n)).cloned()
            .unwrap_or_else(|| crate::modele::murs::assemblage::materiau_par_defaut(c.materiau.as_deref().unwrap_or("Mur générique")));
        peindre_materiau_coupe(p, case, projet, &m, true);
        // ossature : un montant en coupe au milieu de la couche
        if let Some(o) = c.ossature.as_ref().filter(|_| l > 3.0) {
            let montant = egui::Rect::from_center_size(case.center(), egui::vec2(l, (case.height() * 0.4).max(4.0)));
            let bois = materiaux.iter().find(|m| m.nom == o.materiau()).cloned().unwrap_or_else(|| crate::modele::murs::assemblage::materiau_par_defaut(o.materiau()));
            peindre_materiau_coupe(p, montant, projet, &bois, true);
            p.rect_stroke(montant, 0.0, egui::Stroke::new(0.6, Color32::from_gray(60)), egui::StrokeKind::Inside);
        }
        p.rect_stroke(case, 0.0, egui::Stroke::new(0.6, Color32::from_gray(90)), egui::StrokeKind::Inside);
        if survol.is_some_and(|q| case.contains(q)) {
            vise = Some(c);
        }
        x += l;
    }
    vise
}

/// Motifs de coupe du matériau, limités à la vignette de la couche.
pub(super) fn peindre_materiau_coupe(p: &egui::Painter, r: egui::Rect, projet: &crate::modele::Projet, m: &crate::modele::Materiau, vertical: bool) {
    use crate::modele::MotifCoupe;
    if !r.is_positive() { return; }
    let (fond, couleur) = m.fond_coupe();
    let [r0, g0, b0] = m.couleur;
    p.rect_filled(r, 0.0, Color32::from_rgb(r0, g0, b0));
    let dessiner = |motif: &MotifCoupe, couleur: [u8; 3]| {
        if let Some(definition) = projet.definition_motif(motif) {
            super::super::aides::apercu_motif_colore(p, r, &definition, 3.0, Color32::from_rgb(couleur[0], couleur[1], couleur[2]));
        }
    };
    if fond != MotifCoupe::Uni { dessiner(&fond, couleur); }
    let encre = m.encre_coupe();
    match &m.motif {
        MotifCoupe::Uni => { p.rect_filled(r, 0.0, Color32::from_rgb(encre[0], encre[1], encre[2])); }
        MotifCoupe::Vide => {}
        MotifCoupe::Isolation => {
            let longueur = if vertical { r.height() } else { r.width() };
            let amplitude = (if vertical { r.width() } else { r.height() } * 0.38).min(5.0);
            let points: Vec<_> = (0..=((longueur / 2.0).ceil() as usize).min(512)).map(|i| {
                let v = (i as f32 * 2.0).min(longueur);
                let oscillation = (v * std::f32::consts::PI / 6.0).sin() * amplitude;
                if vertical { egui::pos2(r.center().x + oscillation, r.top() + v) } else { egui::pos2(r.left() + v, r.center().y + oscillation) }
            }).collect();
            if points.len() >= 2 { p.with_clip_rect(r).add(egui::Shape::line(points, egui::Stroke::new(1.0, Color32::from_rgb(encre[0], encre[1], encre[2])))); }
        }
        motif => dessiner(motif, encre),
    }
}

/// Infobulle d'une couche : matériau, fonction, épaisseur.
pub(super) fn texte_couche(c: &crate::modele::Couche) -> String {
    format!("{} · {} · {}", c.materiau.as_deref().unwrap_or("Sans matériau"), c.fonction.libelle(), crate::config::options::unite().afficher(c.epaisseur))
}

/// Petit bouton à pictogramme dessiné (la police n'a pas ✕ ▾).
pub(super) fn petit_bouton(ui: &mut egui::Ui, actif: bool, g: Glyphe) -> egui::Response {
    let (r, resp) = ui.allocate_exact_size(egui::vec2(18.0, 18.0), if actif { Sense::click() } else { Sense::hover() });
    let v = ui.visuals();
    if actif && resp.hovered() {
        ui.painter().rect_filled(r, 3.0, v.widgets.hovered.bg_fill);
    }
    let c = if actif { v.text_color() } else { v.weak_text_color().gamma_multiply(0.5) };
    let m = r.center();
    let p = ui.painter();
    match g {
        Glyphe::Liste => triangle_bas(p, m, c),
        Glyphe::Croix => {
            let t = egui::Stroke::new(1.3, c);
            p.line_segment([m + egui::vec2(-3.5, -3.5), m + egui::vec2(3.5, 3.5)], t);
            p.line_segment([m + egui::vec2(-3.5, 3.5), m + egui::vec2(3.5, -3.5)], t);
        }
    }
    resp
}

/// Flèche de liste déroulante (la police n'a pas toujours ▾).
pub(super) fn triangle_bas(p: &egui::Painter, c: egui::Pos2, couleur: Color32) {
    let (l, h) = (4.0, 3.0);
    p.add(egui::Shape::convex_polygon(
        vec![egui::pos2(c.x - l, c.y - h * 0.5), egui::pos2(c.x + l, c.y - h * 0.5), egui::pos2(c.x, c.y + h)],
        couleur,
        egui::Stroke::NONE,
    ));
}

pub(super) fn icone(p: &egui::Painter, r: egui::Rect, o: Outil, c: Color32) {
    use Outil::*;
    let t = egui::Stroke::new(if r.width() < 20.0 { 1.1 } else { 1.4 }, c);
    let pt = |x: f32, y: f32| egui::pos2(r.left() + x * r.width(), r.top() + y * r.height());
    let seg = |a: (f32, f32), b: (f32, f32)| {
        p.line_segment([pt(a.0, a.1), pt(b.0, b.1)], t);
    };
    let cadre = |x: f32, y: f32, w: f32, h: f32| {
        p.rect_stroke(
            egui::Rect::from_min_max(pt(x, y), pt(x + w, y + h)),
            0.0,
            t,
            egui::StrokeKind::Inside,
        );
    };
    let face = |points: &[(f32, f32)], opacite: f32| {
        p.add(egui::Shape::convex_polygon(
            points.iter().map(|&(x, y)| pt(x, y)).collect(),
            c.gamma_multiply(opacite),
            t,
        ));
    };
    match o {
        Niveau | Altimetrie => {
            seg((0.05, 0.65), (0.95, 0.65));
            seg((0.3, 0.2), (0.3, 0.65));
            seg((0.18, 0.48), (0.3, 0.65));
            seg((0.42, 0.48), (0.3, 0.65));
            if o == Niveau {
                seg((0.05, 0.88), (0.95, 0.88));
            }
        }
        Quadrillage => {
            for n in [0.25, 0.65] {
                seg((n, 0.1), (n, 0.9));
                seg((0.1, n), (0.9, n));
            }
        }
        Mur => {
            // Mur mince en perspective, trait fin ; les couches ne se lisent que sur
            // le chant et le dessus, en trait plus léger.
            let fin = egui::Stroke::new(t.width * 0.6, c);
            let leger = egui::Stroke::new(t.width * 0.45, c.gamma_multiply(0.7));
            let (e, f) = (0.16, -0.10); // épaisseur du mur (profondeur)
            let face = |points: &[(f32, f32)], opacite: f32| {
                p.add(egui::Shape::convex_polygon(
                    points.iter().map(|&(x, y)| pt(x, y)).collect(),
                    c.gamma_multiply(opacite),
                    fin,
                ));
            };
            face(&[(0.10, 0.20), (0.72, 0.40), (0.72, 0.98), (0.10, 0.78)], 0.05);
            face(&[(0.10, 0.20), (0.10 + e, 0.20 + f), (0.72 + e, 0.40 + f), (0.72, 0.40)], 0.10);
            face(&[(0.72, 0.40), (0.72 + e, 0.40 + f), (0.72 + e, 0.98 + f), (0.72, 0.98)], 0.18);
            for couche in [1.0 / 3.0, 2.0 / 3.0] {
                let (dx, dy) = (couche * e, couche * f);
                p.line_segment([pt(0.10 + dx, 0.20 + dy), pt(0.72 + dx, 0.40 + dy)], leger);
                p.line_segment([pt(0.72 + dx, 0.40 + dy), pt(0.72 + dx, 0.98 + dy)], leger);
            }
        }
        Surfaces => {
            // Trois silhouettes espacées : couverture, plafond suspendu et plancher.
            face(
                &[(0.08, 0.22), (0.36, 0.05), (0.86, 0.16), (0.58, 0.32)],
                0.06,
            );
            seg((0.36, 0.05), (0.60, 0.16));
            face(
                &[(0.08, 0.49), (0.49, 0.39), (0.92, 0.50), (0.51, 0.61)],
                0.04,
            );
            seg((0.20, 0.46), (0.20, 0.36));
            seg((0.79, 0.47), (0.79, 0.37));
            face(
                &[(0.08, 0.78), (0.49, 0.68), (0.92, 0.79), (0.51, 0.90)],
                0.10,
            );
            seg((0.08, 0.78), (0.08, 0.86));
            seg((0.08, 0.86), (0.51, 0.98));
            seg((0.51, 0.98), (0.92, 0.87));
            seg((0.92, 0.87), (0.92, 0.79));
            seg((0.51, 0.90), (0.51, 0.98));
        }
        Dalle => {
            // Béton porteur : tranche épaisse, avec hachures dans l'épaisseur.
            face(
                &[(0.06, 0.35), (0.55, 0.15), (0.94, 0.40), (0.45, 0.61)],
                0.04,
            );
            face(
                &[(0.06, 0.35), (0.45, 0.61), (0.45, 0.88), (0.06, 0.62)],
                0.13,
            );
            face(
                &[(0.45, 0.61), (0.94, 0.40), (0.94, 0.67), (0.45, 0.88)],
                0.20,
            );
            for k in [0.0, 0.18, 0.36] {
                seg((0.50 + k, 0.69 - k * 0.42), (0.56 + k, 0.80 - k * 0.42));
            }
        }
        Sol => {
            // Finition mince : carrelage sur le dessus, très faible épaisseur.
            face(
                &[(0.06, 0.44), (0.55, 0.20), (0.94, 0.46), (0.45, 0.70)],
                0.05,
            );
            for k in [0.33, 0.66] {
                seg(
                    (0.06 + 0.49 * k, 0.44 - 0.24 * k),
                    (0.45 + 0.49 * k, 0.70 - 0.24 * k),
                );
                seg(
                    (0.06 + 0.39 * k, 0.44 + 0.26 * k),
                    (0.55 + 0.39 * k, 0.20 + 0.26 * k),
                );
            }
            seg((0.06, 0.50), (0.45, 0.76));
            seg((0.45, 0.76), (0.94, 0.52));
            seg((0.06, 0.44), (0.06, 0.50));
            seg((0.94, 0.46), (0.94, 0.52));
        }
        Toiture => {
            // Deux pans bien distincts autour d'un faîtage, avec débords.
            face(
                &[(0.04, 0.62), (0.32, 0.20), (0.75, 0.08), (0.47, 0.50)],
                0.16,
            );
            face(
                &[(0.32, 0.20), (0.68, 0.68), (0.97, 0.54), (0.75, 0.08)],
                0.04,
            );
            seg((0.32, 0.20), (0.75, 0.08));
            seg((0.04, 0.69), (0.68, 0.80));
            seg((0.68, 0.80), (0.97, 0.65));
            seg((0.68, 0.68), (0.68, 0.80));
            seg((0.19, 0.59), (0.47, 0.17));
        }
        Plafond => {
            // Panneau haut, suspendu à trois tiges ; lecture par le dessous.
            for (x, y) in [(0.13, 0.50), (0.55, 0.30), (0.89, 0.53)] {
                seg((x, y), (x, y - 0.24));
                seg((x - 0.05, y - 0.24), (x + 0.05, y - 0.24));
            }
            face(
                &[(0.06, 0.55), (0.55, 0.34), (0.94, 0.59), (0.45, 0.81)],
                0.13,
            );
            seg((0.06, 0.61), (0.45, 0.87));
            seg((0.45, 0.87), (0.94, 0.65));
            seg((0.25, 0.68), (0.74, 0.47));
        }
        Terrain => {
            face(&[(0.06,0.67),(0.25,0.39),(0.48,0.51),(0.69,0.22),(0.94,0.52),(0.94,0.72),(0.53,0.93)], 0.12);
            seg((0.06,0.67),(0.53,0.93));
            seg((0.25,0.39),(0.55,0.67));
            seg((0.69,0.22),(0.55,0.67));
            seg((0.55,0.67),(0.94,0.52));
        }
        LimitePropriete => {
            seg((0.12,0.31),(0.65,0.10));
            seg((0.65,0.10),(0.90,0.40));
            seg((0.90,0.40),(0.44,0.89));
            seg((0.44,0.89),(0.12,0.31));
            for (x,y) in [(0.12,0.31),(0.65,0.10),(0.90,0.40),(0.44,0.89)] { cadre(x-0.05,y-0.05,0.10,0.10); }
        }
        Poteau => {
            face(
                &[(0.33, 0.22), (0.57, 0.12), (0.75, 0.22), (0.51, 0.33)],
                0.09,
            );
            face(
                &[(0.33, 0.22), (0.51, 0.33), (0.51, 0.86), (0.33, 0.75)],
                0.06,
            );
            face(
                &[(0.51, 0.33), (0.75, 0.22), (0.75, 0.75), (0.51, 0.86)],
                0.17,
            );
            seg((0.16, 0.82), (0.49, 0.98));
            seg((0.49, 0.98), (0.91, 0.79));
        }
        Poutre => {
            // Barre horizontale en perspective, avec sa section visible à gauche.
            face(
                &[(0.08, 0.46), (0.69, 0.18), (0.92, 0.30), (0.31, 0.58)],
                0.09,
            );
            face(
                &[(0.08, 0.46), (0.31, 0.58), (0.31, 0.78), (0.08, 0.66)],
                0.06,
            );
            face(
                &[(0.31, 0.58), (0.92, 0.30), (0.92, 0.50), (0.31, 0.78)],
                0.17,
            );
        }
        Escalier => {
            for k in 0..4 {
                let x = 0.07 + k as f32 * 0.13;
                let y = 0.84 - k as f32 * 0.17;
                seg((x, y), (x, y - 0.16));
                seg((x, y - 0.16), (x + 0.13, y - 0.16));
                seg((x, y - 0.16), (x + 0.35, y - 0.31));
                seg((x + 0.35, y - 0.31), (x + 0.48, y - 0.31));
            }
            seg((0.07, 0.84), (0.55, 0.84));
            seg((0.55, 0.84), (0.94, 0.18));
        }
        GardeCorps => {
            // Main courante en perspective, avec retour et platines distinctes.
            seg((0.08, 0.36), (0.73, 0.12));
            seg((0.73, 0.12), (0.94, 0.25));
            seg((0.08, 0.46), (0.73, 0.22));
            seg((0.73, 0.22), (0.94, 0.35));
            seg((0.10, 0.66), (0.73, 0.43));
            seg((0.73, 0.43), (0.92, 0.55));
            for (x, y) in [(0.12, 0.36), (0.42, 0.25), (0.73, 0.14), (0.91, 0.27)] {
                seg((x, y), (x, y + 0.52));
                seg((x - 0.06, y + 0.54), (x + 0.06, y + 0.50));
            }
        }
        Ouverture => {
            cadre(0.15, 0.08, 0.7, 0.84);
            seg((0.5, 0.08), (0.5, 0.92));
            seg((0.15, 0.5), (0.85, 0.5));
        }
        Piece => {
            cadre(0.1, 0.1, 0.8, 0.8);
            seg((0.1, 0.55), (0.45, 0.55));
            seg((0.62, 0.55), (0.9, 0.55));
            seg((0.45, 0.1), (0.45, 0.55));
        }
        Arbre => {
            // houppier festonné, tronc
            let c = egui::pos2(r.left() + 0.5 * r.width(), r.top() + 0.4 * r.height());
            let n = 40;
            let pts: Vec<egui::Pos2> = (0..n)
                .map(|k| {
                    let a = k as f32 / n as f32 * std::f32::consts::TAU;
                    let rr = r.width() * (0.3 + 0.04 * (a * 6.0).cos().abs());
                    c + egui::vec2(a.cos() * rr, a.sin() * rr)
                })
                .collect();
            p.add(egui::Shape::closed_line(pts, t));
            seg((0.5, 0.62), (0.5, 0.94));
            seg((0.38, 0.94), (0.62, 0.94));
        }
        Composant => {
            for (x, y) in [(0.1, 0.1), (0.55, 0.1), (0.1, 0.55), (0.55, 0.55)] {
                cadre(x, y, 0.32, 0.32);
            }
        }
        Deplacer => {
            for (a, b) in [
                ((0.1, 0.5), (0.9, 0.5)),
                ((0.5, 0.1), (0.5, 0.9)),
                ((0.1, 0.5), (0.27, 0.35)),
                ((0.1, 0.5), (0.27, 0.65)),
                ((0.9, 0.5), (0.73, 0.35)),
                ((0.9, 0.5), (0.73, 0.65)),
                ((0.5, 0.1), (0.35, 0.27)),
                ((0.5, 0.1), (0.65, 0.27)),
                ((0.5, 0.9), (0.35, 0.73)),
                ((0.5, 0.9), (0.65, 0.73)),
            ] {
                seg(a, b);
            }
        }
        Copier => {
            cadre(0.1, 0.1, 0.55, 0.55);
            cadre(0.35, 0.35, 0.55, 0.55);
        }
        CopierNiveau => {
            seg((0.05, 0.92), (0.95, 0.92));
            seg((0.05, 0.50), (0.95, 0.50));
            cadre(0.25, 0.60, 0.5, 0.32);
            cadre(0.25, 0.08, 0.5, 0.32);
            seg((0.5, 0.40), (0.5, 0.60));
        }
        Jonctions => {
            // deux murs en L, onglet à l'angle
            seg((0.1, 0.9), (0.1, 0.1));
            seg((0.1, 0.1), (0.9, 0.1));
            seg((0.38, 0.9), (0.38, 0.38));
            seg((0.38, 0.38), (0.9, 0.38));
            seg((0.1, 0.1), (0.38, 0.38));
        }
        AttacherToiture => {
            seg((0.1, 0.42), (0.5, 0.12));
            seg((0.5, 0.12), (0.9, 0.42));
            seg((0.24, 0.42), (0.24, 0.88));
            seg((0.76, 0.42), (0.76, 0.88));
            seg((0.24, 0.88), (0.76, 0.88));
            seg((0.5, 0.8), (0.5, 0.42));
            seg((0.36, 0.56), (0.5, 0.42));
            seg((0.64, 0.56), (0.5, 0.42));
        }
        ModifierContour => {
            seg((0.12, 0.24), (0.70, 0.12));
            seg((0.70, 0.12), (0.87, 0.66));
            seg((0.87, 0.66), (0.31, 0.88));
            seg((0.31, 0.88), (0.12, 0.24));
            for (x, y) in [(0.12, 0.24), (0.70, 0.12), (0.87, 0.66), (0.31, 0.88)] {
                cadre(x - 0.05, y - 0.05, 0.10, 0.10);
            }
        }
        CreerIdentique => {
            cadre(0.08, 0.12, 0.44, 0.44);
            cadre(0.48, 0.48, 0.44, 0.44);
            seg((0.68, 0.08), (0.68, 0.36));
            seg((0.54, 0.22), (0.82, 0.22));
        }
        Supprimer => {
            seg((0.16, 0.25), (0.84, 0.25));
            seg((0.35, 0.13), (0.65, 0.13));
            seg((0.25, 0.29), (0.30, 0.88));
            seg((0.75, 0.29), (0.70, 0.88));
            seg((0.30, 0.88), (0.70, 0.88));
            seg((0.42, 0.40), (0.42, 0.76));
            seg((0.58, 0.40), (0.58, 0.76));
        }
        Tourner | Arc => {
            let pts = (0..24)
                .map(|k| {
                    let a = 0.2 + k as f32 / 23.0 * 4.4;
                    pt(0.5 + 0.36 * a.cos(), 0.5 + 0.36 * a.sin())
                })
                .collect();
            p.add(egui::Shape::line(pts, t));
            if o == Tourner {
                seg((0.85, 0.57), (0.9, 0.28));
                seg((0.85, 0.57), (0.62, 0.52));
            }
        }
        Symetrie => {
            p.add(egui::Shape::dashed_line(&[pt(0.5, 0.05), pt(0.5, 0.95)], t, 3.0, 2.0));
            p.add(egui::Shape::closed_line(vec![pt(0.1, 0.75), pt(0.4, 0.75), pt(0.4, 0.25)], t));
            p.add(egui::Shape::closed_line(vec![pt(0.9, 0.75), pt(0.6, 0.75), pt(0.6, 0.25)], t));
        }
        Reseau => {
            for k in 0..3 {
                let x = 0.08 + k as f32 * 0.3;
                cadre(x, 0.35, 0.22, 0.3);
            }
        }
        Echelle => {
            cadre(0.1, 0.5, 0.35, 0.35);
            cadre(0.1, 0.15, 0.75, 0.7);
            seg((0.45, 0.5), (0.8, 0.2));
            seg((0.8, 0.2), (0.66, 0.22));
            seg((0.8, 0.2), (0.78, 0.34));
        }
        Decaler => {
            seg((0.1, 0.3), (0.9, 0.3));
            p.add(egui::Shape::dashed_line(&[pt(0.1, 0.75), pt(0.9, 0.75)], t, 3.0, 2.0));
            seg((0.5, 0.35), (0.5, 0.7));
            seg((0.5, 0.7), (0.43, 0.6));
            seg((0.5, 0.7), (0.57, 0.6));
        }
        Demolir => {
            // marteau
            p.rect_filled(egui::Rect::from_min_max(pt(0.15, 0.15), pt(0.7, 0.38)), 1.0, t.color);
            seg((0.45, 0.38), (0.45, 0.95));
        }
        Reproduire => {
            // pinceau
            cadre(0.2, 0.1, 0.6, 0.3);
            seg((0.5, 0.4), (0.5, 0.55));
            p.add(egui::Shape::closed_line(vec![pt(0.38, 0.55), pt(0.62, 0.55), pt(0.58, 0.95), pt(0.42, 0.95)], t));
        }
        ChangerHote => {
            cadre(0.05, 0.6, 0.4, 0.15);
            cadre(0.55, 0.6, 0.4, 0.15);
            cadre(0.15, 0.25, 0.2, 0.3);
            seg((0.38, 0.4), (0.7, 0.4));
            seg((0.7, 0.4), (0.62, 0.33));
            seg((0.7, 0.4), (0.62, 0.47));
        }
        Inverser => {
            seg((0.15, 0.35), (0.85, 0.35));
            seg((0.15, 0.35), (0.27, 0.25));
            seg((0.85, 0.65), (0.15, 0.65));
            seg((0.85, 0.65), (0.73, 0.75));
        }
        Scinder => {
            seg((0.05, 0.5), (0.42, 0.5));
            seg((0.58, 0.5), (0.95, 0.5));
            seg((0.5, 0.15), (0.5, 0.85));
        }
        Aligner => {
            seg((0.15, 0.05), (0.15, 0.95));
            cadre(0.25, 0.15, 0.65, 0.23);
            cadre(0.25, 0.6, 0.4, 0.23);
        }
        Ajuster => {
            seg((0.1, 0.8), (0.9, 0.8));
            seg((0.3, 0.15), (0.3, 0.95));
            seg((0.6, 0.2), (0.9, 0.5));
            seg((0.6, 0.5), (0.9, 0.2));
        }
        Cote => {
            seg((0.15, 0.15), (0.15, 0.85));
            seg((0.85, 0.15), (0.85, 0.85));
            seg((0.05, 0.6), (0.95, 0.6));
            seg((0.06, 0.7), (0.24, 0.5));
            seg((0.76, 0.7), (0.94, 0.5));
        }
        CoteAngulaire => {
            seg((0.12, 0.88), (0.92, 0.88));
            seg((0.12, 0.88), (0.7, 0.12));
            p.add(egui::Shape::line((0..=8).map(|i| { let a = 0.916 * i as f32 / 8.0; pt(0.12 + 0.55 * a.cos(), 0.88 - 0.55 * a.sin()) }).collect(), t));
        }
        CoteDiametre => {
            p.circle_stroke(pt(0.5, 0.5), 0.4 * r.width().min(r.height()), t);
            seg((0.2, 0.8), (0.8, 0.2));
            seg((0.1, 0.8), (0.3, 0.8));
            seg((0.7, 0.2), (0.9, 0.2));
        }
        Texte => {
            seg((0.12, 0.16), (0.88, 0.16));
            seg((0.5, 0.16), (0.5, 0.88));
            seg((0.32, 0.88), (0.68, 0.88));
        }
        Etiquette => {
            p.add(egui::Shape::closed_line(
                vec![
                    pt(0.08, 0.1),
                    pt(0.6, 0.1),
                    pt(0.93, 0.5),
                    pt(0.5, 0.93),
                    pt(0.08, 0.5),
                ],
                t,
            ));
            p.circle_stroke(pt(0.3, 0.3), 2.0, t);
        }
        Ligne => {
            seg((0.15, 0.85), (0.85, 0.15));
            p.circle_filled(pt(0.15, 0.85), r.width() * 0.06, c);
            p.circle_filled(pt(0.85, 0.15), r.width() * 0.06, c);
        }
        Rectangle => cadre(0.08, 0.18, 0.84, 0.64),
        Polyligne => {
            // petite hachure sous le premier creux : une polyligne fermée peut être remplie
            for (a, b) in [((0.194, 0.644), (0.32, 0.77)), ((0.241, 0.541), (0.43, 0.73)), ((0.287, 0.437), (0.54, 0.69))] {
                p.line_segment([pt(a.0, a.1), pt(b.0, b.1)], egui::Stroke::new(t.width * 0.6, t.color));
            }
            p.add(egui::Shape::line(vec![pt(0.1, 0.85), pt(0.35, 0.3), pt(0.65, 0.65), pt(0.9, 0.15)], t));
            for (x, y) in [(0.1, 0.85), (0.35, 0.3), (0.65, 0.65), (0.9, 0.15)] {
                p.circle_filled(pt(x, y), r.width() * 0.05, c);
            }
        }
        Spline => {
            let pts = (0..=24).map(|k| {
                let x = k as f32 / 24.0;
                pt(0.1 + 0.8 * x, 0.5 - 0.32 * (x * std::f32::consts::TAU).sin())
            }).collect();
            p.add(egui::Shape::line(pts, t));
        }
        Cercle => {
            p.circle_stroke(r.center(), r.width() * 0.38, t);
            p.circle_filled(r.center(), r.width() * 0.05, c);
        }
        Zone => {
            cadre(0.08, 0.08, 0.84, 0.84);
            for k in 0..5 {
                let x = 0.1 + k as f32 * 0.17;
                seg((x, 0.85), (0.85, x));
            }
        }
        Symbole => {
            p.circle_stroke(r.center(), r.width() * 0.38, t);
            seg((0.5, 0.25), (0.5, 0.75));
            seg((0.25, 0.5), (0.75, 0.5));
        }
        Feuilles => {
            // feuille et son cartouche
            cadre(0.12, 0.08, 0.76, 0.84);
            cadre(0.5, 0.68, 0.38, 0.24);
            cadre(0.22, 0.18, 0.5, 0.4);
        }
        Nomenclatures => {
            // tableau : en-tête et lignes
            cadre(0.1, 0.12, 0.8, 0.76);
            face(&[(0.1, 0.12), (0.9, 0.12), (0.9, 0.3), (0.1, 0.3)], 0.35);
            for y in [0.48, 0.68] {
                seg((0.1, y), (0.9, y));
            }
            seg((0.42, 0.12), (0.42, 0.88));
        }
        Cadrer => {
            // coins de cadrage autour d'un objet
            for (x, y, dx, dy) in [(0.08, 0.08, 1.0, 1.0), (0.92, 0.08, -1.0, 1.0), (0.08, 0.92, 1.0, -1.0), (0.92, 0.92, -1.0, -1.0)] {
                seg((x, y), (x + 0.22 * dx, y));
                seg((x, y), (x, y + 0.22 * dy));
            }
            cadre(0.32, 0.32, 0.36, 0.36);
        }
        Isoler => {
            // objet plein, voisins en pointillé
            face(&[(0.32, 0.32), (0.68, 0.32), (0.68, 0.68), (0.32, 0.68)], 0.6);
            for (x, y) in [(0.05, 0.42), (0.8, 0.42)] {
                p.add(egui::Shape::dashed_line(&[pt(x, y), pt(x + 0.15, y), pt(x + 0.15, y + 0.16), pt(x, y + 0.16), pt(x, y)], t, 2.0, 2.0));
            }
        }
        MasquerVue => {
            let pts = (0..=16).map(|k| { let a = std::f32::consts::PI * k as f32 / 16.0; pt(0.5 - 0.42 * a.cos(), 0.45 - 0.22 * a.sin()) }).collect();
            p.add(egui::Shape::line(pts, t));
            let pts = (0..=16).map(|k| { let a = std::f32::consts::PI * k as f32 / 16.0; pt(0.5 - 0.42 * a.cos(), 0.45 + 0.22 * a.sin()) }).collect();
            p.add(egui::Shape::line(pts, t));
            seg((0.15, 0.8), (0.85, 0.1));
            cadre(0.1, 0.78, 0.8, 0.17);
        }
        Afficher => {
            let pts = (0..=16).map(|k| { let a = std::f32::consts::PI * k as f32 / 16.0; pt(0.5 - 0.42 * a.cos(), 0.5 - 0.22 * a.sin()) }).collect();
            p.add(egui::Shape::line(pts, t));
            let pts = (0..=16).map(|k| { let a = std::f32::consts::PI * k as f32 / 16.0; pt(0.5 - 0.42 * a.cos(), 0.5 + 0.22 * a.sin()) }).collect();
            p.add(egui::Shape::line(pts, t));
            p.circle_filled(r.center(), r.width() * 0.11, Color32::from_rgb(235, 50, 190));
        }
        Masquer => {
            // œil barré
            let pts = (0..=16).map(|k| { let a = std::f32::consts::PI * k as f32 / 16.0; pt(0.5 - 0.42 * a.cos(), 0.5 - 0.22 * a.sin()) }).collect();
            p.add(egui::Shape::line(pts, t));
            let pts = (0..=16).map(|k| { let a = std::f32::consts::PI * k as f32 / 16.0; pt(0.5 - 0.42 * a.cos(), 0.5 + 0.22 * a.sin()) }).collect();
            p.add(egui::Shape::line(pts, t));
            p.circle_stroke(r.center(), r.width() * 0.1, t);
            seg((0.15, 0.85), (0.85, 0.15));
        }
        Verrouiller | Deverrouiller => {
            let ferme = o == Verrouiller;
            let corps = egui::Rect::from_min_max(pt(0.22, 0.48), pt(0.78, 0.9));
            p.rect_stroke(corps, 2.0, t, egui::StrokeKind::Middle);
            let d = if ferme { 0.0 } else { 0.2 };
            let pts: Vec<egui::Pos2> = (0..=12).map(|k| { let a = std::f32::consts::PI * k as f32 / 12.0; pt(0.5 - 0.18 * a.cos() + d, 0.3 - 0.18 * a.sin()) }).collect();
            let mut arc = vec![pt(0.32 + d, if ferme { 0.48 } else { 0.4 })];
            arc.extend(pts);
            arc.push(pt(0.68 + d, if ferme { 0.48 } else { 0.4 }));
            p.add(egui::Shape::line(arc, t));
            p.circle_filled(pt(0.5, 0.66), r.width() * 0.05, t.color);
        }
        Legende => {
            for y in [0.2, 0.5, 0.8] {
                cadre(0.08, y - 0.07, 0.17, 0.14);
                seg((0.4, y), (0.92, y));
            }
        }
        VuePlan => {
            // plan : contour de pièces vu de dessus
            cadre(0.1, 0.15, 0.8, 0.7);
            seg((0.5, 0.15), (0.5, 0.55));
            seg((0.1, 0.55), (0.35, 0.55));
            seg((0.65, 0.55), (0.9, 0.55));
        }
        VueCoupe => {
            // repère de coupe : trait, têtes rondes et flèches de visée
            seg((0.12, 0.5), (0.88, 0.5));
            for x in [0.12, 0.88] {
                p.circle_stroke(pt(x, 0.5), r.width() * 0.1, t);
                p.add(egui::Shape::convex_polygon(vec![pt(x - 0.1, 0.38), pt(x + 0.1, 0.38), pt(x, 0.2)], c, egui::Stroke::NONE));
            }
        }
    }
}

/// Icône d'action d'une ligne de type : crayon (modifier) ou deux feuilles (dupliquer).
pub(super) fn icone_action_type(p: &egui::Painter, r: egui::Rect, crayon: bool, c: Color32) {
    let t = egui::Stroke::new(1.4, c);
    let pt = |x: f32, y: f32| egui::pos2(r.left() + x * r.width(), r.top() + y * r.height());
    if crayon {
        p.add(egui::Shape::closed_line(vec![pt(0.15, 0.85), pt(0.2, 0.62), pt(0.68, 0.14), pt(0.86, 0.32), pt(0.38, 0.8)], t));
        p.line_segment([pt(0.58, 0.24), pt(0.76, 0.42)], t);
        p.line_segment([pt(0.15, 0.85), pt(0.38, 0.8)], t);
    } else {
        p.rect_stroke(egui::Rect::from_min_max(pt(0.1, 0.3), pt(0.65, 0.9)), 1.0, t, egui::StrokeKind::Middle);
        p.add(egui::Shape::line(vec![pt(0.35, 0.3), pt(0.35, 0.1), pt(0.9, 0.1), pt(0.9, 0.7), pt(0.65, 0.7)], t));
    }
}
