//! Aperçus, terrain, souris et dessin à l'écran.
use super::*;

impl ViewerApp {
    /// Aperçu du terrain en cours, mis à jour à chaque glissé : surface teintée selon la
    /// cote (bas : vert, haut : sable), courbes de niveau à un pas rond, contour fin.
    pub(in crate::ui) fn apercu_terrain(&self, painter: &egui::Painter, rect: egui::Rect) {
        let Some(contour) = self.dessin2d.dalle.first() else { return };
        let ngf = self.natif.as_ref().map_or(0.0, |(p, _)| p.ngf());
        let bleu = Color32::from_rgb(0, 120, 215);
        let Some(Ok(tris)) = self.terrain_courant().map(|t| t.triangles()) else {
            let pts: Vec<DVec2> = contour.iter().map(|p| DVec2::from_array(*p)).collect();
            self.apercu_surface(painter, rect, std::slice::from_ref(&pts));
            return;
        };
        let (bas, haut) = tris.iter().flatten().fold((f64::MAX, f64::MIN), |(a, b), p| (a.min(p.z), b.max(p.z)));
        let etendue = (haut - bas).max(1e-6);
        let teinte = |z: f64| {
            let t = ((z - bas) / etendue) as f32;
            let (a, b, u) = if t < 0.5 { ([62.0, 138.0, 112.0], [150.0, 182.0, 96.0], t * 2.0) } else { ([150.0, 182.0, 96.0], [200.0, 158.0, 104.0], t * 2.0 - 1.0) };
            let c: [f32; 3] = std::array::from_fn(|k| a[k] + (b[k] - a[k]) * u);
            Color32::from_rgba_unmultiplied(c[0] as u8, c[1] as u8, c[2] as u8, 130)
        };
        let ecran = |p: DVec3| self.ecran_3d(rect, DVec3::new(p.x, p.y, p.z - ngf));
        let mut maillage = egui::Mesh::default();
        for t in &tris {
            let Some(e) = t.iter().map(|p| ecran(*p)).collect::<Option<Vec<_>>>() else { continue };
            let k = maillage.vertices.len() as u32;
            for (p, q) in t.iter().zip(e) {
                maillage.colored_vertex(q, teinte(p.z));
            }
            maillage.add_triangle(k, k + 1, k + 2);
        }
        painter.add(egui::Shape::mesh(maillage));
        // courbes de niveau : une dizaine au plus, au pas rond le plus fin
        if haut - bas > 1.0 {
            let pas = [50.0, 100.0, 200.0, 250.0, 500.0, 1000.0, 2000.0, 5000.0, 10_000.0].into_iter().find(|p| etendue / p <= 12.0).unwrap_or(20_000.0);
            let trait_ = egui::Stroke::new(1.0, Color32::from_white_alpha(150));
            for t in &tris {
                let mut z = (t.iter().map(|p| p.z).fold(f64::MAX, f64::min) / pas).ceil() * pas;
                let zmax = t.iter().map(|p| p.z).fold(f64::MIN, f64::max);
                while z < zmax {
                    let coupe: Vec<DVec3> = (0..3).filter_map(|i| {
                        let (a, b) = (t[i], t[(i + 1) % 3]);
                        ((a.z - z) * (b.z - z) < 0.0).then(|| a + (b - a) * ((z - a.z) / (b.z - a.z)))
                    }).collect();
                    if let [a, b] = coupe[..] && let (Some(a), Some(b)) = (ecran(a), ecran(b)) {
                        painter.line_segment([a, b], trait_);
                    }
                    z += pas;
                }
            }
        }
        let bord: Vec<egui::Pos2> = contour.iter().zip(&self.dessin2d.terrain_cotes).filter_map(|(p, z)| ecran(DVec3::new(p[0], p[1], *z))).collect();
        painter.add(egui::Shape::closed_line(bord, egui::Stroke::new(1.2, bleu.gamma_multiply(0.8))));
    }

    /// Poignées du terrain en cours à l'écran : sommets, points cotés, milieux des côtés.
    pub(super) fn poignees_terrain(&self, rect: egui::Rect) -> Vec<(PoigneeTerrain, egui::Pos2)> {
        let Some(contour) = self.dessin2d.dalle.first() else { return Vec::new() };
        let n = contour.len();
        let ecran = |p: DVec2| self.vers_ecran(rect, p);
        let sommets = contour.iter().enumerate().filter_map(|(i, p)| Some((PoigneeTerrain::Sommet(i), ecran(DVec2::from_array(*p))?)));
        let points = self.dessin2d.terrain_points.iter().enumerate().filter_map(|(i, p)| Some((PoigneeTerrain::Point(i), ecran(DVec2::new(p[0], p[1]))?)));
        let milieux = (0..n).filter_map(|i| Some((PoigneeTerrain::Milieu(i), ecran((DVec2::from_array(contour[i]) + DVec2::from_array(contour[(i + 1) % n])) * 0.5)?)));
        sommets.chain(points).chain(milieux).collect()
    }

    /// Terrain en cours : survol des poignées, glissé d'un sommet ou d'un point (un milieu
    /// insère un sommet à la cote moyenne), clic droit : retirer, clic : point coté si
    /// « Placer des points cotés ». Un glissé qui rend le terrain impossible est repris.
    pub(in crate::ui) fn souris_terrain(&mut self, ui: &egui::Ui, rect: egui::Rect, resp: &egui::Response) {
        let poignees = self.poignees_terrain(rect);
        let proche = |q: egui::Pos2| poignees.iter().filter(|(_, e)| e.distance(q) < 10.0).min_by(|a, b| a.1.distance(q).total_cmp(&b.1.distance(q))).map(|(p, _)| *p);
        if self.dessin2d.terrain_glisse.is_none() {
            self.dessin2d.terrain_survol = resp.hover_pos().and_then(proche);
        }
        let d = &mut self.dessin2d;
        if resp.drag_started_by(PointerButton::Primary) && let Some(p) = ui.input(|i| i.pointer.press_origin()).and_then(proche) {
            let avant = (d.dalle[0].clone(), d.terrain_cotes.clone(), d.terrain_points.clone());
            let p = match p {
                PoigneeTerrain::Milieu(i) => {
                    let n = d.dalle[0].len();
                    let (a, b) = (DVec2::from_array(d.dalle[0][i]), DVec2::from_array(d.dalle[0][(i + 1) % n]));
                    d.dalle[0].insert(i + 1, ((a + b) * 0.5).to_array());
                    let z = (d.terrain_cotes[i] + d.terrain_cotes[(i + 1) % n]) * 0.5;
                    d.terrain_cotes.insert(i + 1, z);
                    PoigneeTerrain::Sommet(i + 1)
                }
                p => p,
            };
            d.terrain_glisse = Some((p, avant.0, avant.1, avant.2));
            d.terrain_survol = Some(p);
        }
        if let Some((p, ..)) = self.dessin2d.terrain_glisse
            && let Some(q) = resp.interact_pointer_pos().and_then(|q| self.depuis_ecran(rect, q))
        {
            let d = &mut self.dessin2d;
            match p {
                PoigneeTerrain::Sommet(i) => if let Some(s) = d.dalle[0].get_mut(i) { *s = q.to_array(); },
                PoigneeTerrain::Point(i) => if let Some(s) = d.terrain_points.get_mut(i) { s[0] = q.x; s[1] = q.y; },
                PoigneeTerrain::Milieu(_) => {}
            }
        }
        if resp.drag_stopped() && let Some((_, contour, cotes, points)) = self.dessin2d.terrain_glisse.take() {
            match self.terrain_courant().map(|t| t.triangles()) {
                Some(Err(e)) => {
                    let d = &mut self.dessin2d;
                    (d.dalle[0], d.terrain_cotes, d.terrain_points) = (contour, cotes, points);
                    d.message = Some(e);
                }
                _ => { self.dessin2d.message = None; self.dessin2d.information = None; },
            }
        }
        // clic droit sur une poignée : retirer le point ou le sommet (trois au moins)
        if resp.secondary_clicked() && let Some(p) = resp.interact_pointer_pos().and_then(proche) {
            let d = &mut self.dessin2d;
            match p {
                PoigneeTerrain::Point(i) => { d.terrain_points.remove(i); }
                PoigneeTerrain::Sommet(i) if d.dalle[0].len() > 3 => {
                    let avant = (d.dalle[0].clone(), d.terrain_cotes.clone());
                    d.dalle[0].remove(i);
                    d.terrain_cotes.remove(i);
                    if let Some(Err(e)) = self.terrain_courant().map(|t| t.triangles()) {
                        (self.dessin2d.dalle[0], self.dessin2d.terrain_cotes) = avant;
                        self.dessin2d.message = Some(e);
                    }
                }
                PoigneeTerrain::Sommet(_) => self.dessin2d.message = Some("Le contour garde au moins trois sommets.".into()),
                PoigneeTerrain::Milieu(_) => {}
            }
            self.dessin2d.terrain_survol = None;
        }
        if self.dessin2d.terrain_ajouter_point && resp.clicked() && self.dessin2d.terrain_survol.is_none()
            && let Some(p) = resp.interact_pointer_pos().and_then(|q| self.depuis_ecran(rect, q))
        {
            self.dessin2d.terrain_points.push([p.x, p.y, self.dessin2d.terrain_altitude]);
            match self.terrain_courant().map(|t| t.triangles()) {
                Some(Err(e)) => { self.dessin2d.terrain_points.pop(); self.dessin2d.message = Some(e); }
                _ => { self.dessin2d.message = None; self.dessin2d.information = None; },
            }
        }
        if self.dessin2d.terrain_survol.is_some() {
            ui.ctx().set_cursor_icon(if self.dessin2d.terrain_glisse.is_some() { egui::CursorIcon::Grabbing } else { egui::CursorIcon::Grab });
        }
    }

    /// Souris dans la vue avec un outil de dessin 2D : point accroché sous le curseur,
    /// clic gauche : point, clic droit : terminer (comme Entrée).
    pub(in crate::ui) fn souris_dessin(&mut self, ui: &egui::Ui, rect: egui::Rect, resp: &egui::Response) {
        if self.usage_dessin().is_some_and(|(u,_)|u==Usage::Mur) && self.ruban_edition.selection_mur {
            self.dessin2d.point=None;
            self.dessin2d.ligne_selection=resp.hover_pos().and_then(|q|self.trait_mur_vise(rect,q));
            if resp.clicked() {
                if let Some(f)=self.dessin2d.ligne_selection.clone() {
                    if let d2::Forme2D::Cercle{centre,rayon}=f {
                        self.ruban_edition.selection_mur=false;
                        self.ruban_edition.forme_mur=OutilDessin::Cercle;
                        let mut t=Trace::new(OutilDessin::Cercle);t.perimetre=true;t.points=vec![centre,centre+DVec2::X*rayon];
                        self.dessin2d.trace=Some(t);
                    } else {self.enregistrer_formes(vec![f]);}
                } else {{ self.dessin2d.message = None; self.dessin2d.information = None; }; self.dessin2d.information = Some("Sélectionnez un trait du dessin dans la vue.".into());}
            }
            return;
        }
        self.dessin2d.ligne_selection=None;
        if !self.dessin2d.dalle.is_empty() && self.usage_dessin().is_some_and(|(u, _)| u == Usage::Terrain) {
            self.souris_terrain(ui, rect, resp);
            self.dessin2d.point = None;
            return;
        }
        if self.dessin2d.edition_contour.as_ref().is_some_and(|e| !e.dessin) {
            let curseur = resp.hover_pos().or(resp.interact_pointer_pos());
            self.dessin2d.curseur = curseur.and_then(|q| self.depuis_ecran(rect, q));
            let temps = ui.input(|i| i.time);
            let maj = ui.input(|i| i.modifiers.shift);
            self.dessin2d.point = self.dessin2d.curseur.map(|c| self.point_dessin(rect, c, maj, temps));
            if self.dessin2d.survol_accroche.is_some() || self.dessin2d.survol_segment.is_some() { ui.ctx().request_repaint_after(std::time::Duration::from_millis(100)); }
            let q = if resp.drag_started_by(PointerButton::Primary) { ui.input(|i| i.pointer.press_origin()).or(resp.interact_pointer_pos()) } else { resp.interact_pointer_pos().or(resp.hover_pos()) };
            let mut sommet: Option<(f32, usize, usize)> = None;
            let mut arete: Option<(f32, usize, usize, egui::Pos2)> = None;
            if let Some(q) = q {
                for (ci, c) in self.dessin2d.dalle.iter().enumerate() {
                    let fermee = self.dessin2d.edition_contour.as_ref().unwrap().fermee(ci);
                    for i in 0..c.len() {
                        if let Some(a) = self.vers_ecran(rect, DVec2::from_array(c[i])) {
                            let distance = q.distance(a);
                            if distance < 10.0 && sommet.is_none_or(|s| distance < s.0) { sommet = Some((distance, ci, i)); }
                            if !fermee && i + 1 == c.len() { continue; }
                            let Some(b) = self.vers_ecran(rect, DVec2::from_array(c[(i + 1) % c.len()])) else { continue };
                            let v = b - a;
                            let t = if v.length_sq() > 0.0 { ((q - a).dot(v) / v.length_sq()).clamp(0.0, 1.0) } else { 0.0 };
                            let p = a + v * t;
                            let distance = q.distance(p);
                            if distance < 8.0 && arete.is_none_or(|s| distance < s.0) { arete = Some((distance, ci, i, p)); }
                        }
                    }
                }
            }
            if resp.drag_started_by(PointerButton::Primary) {
                if let Some((_, c, i)) = sommet {
                    let e = self.dessin2d.edition_contour.as_mut().unwrap();
                    e.selection = Some(c);
                    e.segment = None;
                    e.glisse = Some(GlisseContour::Sommet(c, i));
                } else if let (Some((_, c, i, _)), Some(q)) = (arete, q) {
                    if let Some(origine) = self.depuis_ecran(rect, q) {
                        let a = DVec2::from_array(self.dessin2d.dalle[c][i]);
                        let b = DVec2::from_array(self.dessin2d.dalle[c][(i + 1) % self.dessin2d.dalle[c].len()]);
                        let e = self.dessin2d.edition_contour.as_mut().unwrap();
                        e.selection = Some(c);
                        e.segment = Some((c, i));
                        e.glisse = Some(GlisseContour::Segment { contour: c, segment: i, origine, a, b });
                    }
                }
            }
            let glisse = self.dessin2d.edition_contour.as_ref().and_then(|e| e.glisse);
            let curseur = resp.interact_pointer_pos().or(resp.hover_pos());
            if let (Some(glisse), Some(q)) = (glisse, curseur) {
                if let Some(p) = self.depuis_ecran(rect, q) {
                    match glisse {
                        GlisseContour::Sommet(c, i) => {
                            let actuel = DVec2::from_array(self.dessin2d.dalle[c][i]);
                            let p = self.dessin2d.point.map(|x| x.0).filter(|q| q.distance(actuel) > 1e-6).unwrap_or(p);
                            self.dessin2d.dalle[c][i] = [(p.x * 10.0).round() / 10.0, (p.y * 10.0).round() / 10.0];
                        }
                        GlisseContour::Segment { contour, segment, origine, a, b } => {
                            let delta = p - origine;
                            let suivant = (segment + 1) % self.dessin2d.dalle[contour].len();
                            self.dessin2d.dalle[contour][segment] = (a + delta).to_array();
                            self.dessin2d.dalle[contour][suivant] = (b + delta).to_array();
                        }
                    }
                }
            }
            if resp.drag_stopped() { self.dessin2d.edition_contour.as_mut().unwrap().glisse = None; }
            if resp.clicked_by(PointerButton::Secondary) {
                if let Some((_, c, i)) = sommet {
                    let fermee = self.dessin2d.edition_contour.as_ref().unwrap().fermee(c);
                    if self.dessin2d.dalle[c].len() > if fermee { 3 } else { 2 } { self.dessin2d.dalle[c].remove(i); self.dessin2d.edition_contour.as_mut().unwrap().segment = None; }
                    else { self.dessin2d.message = Some("Le tracé ne peut pas comporter moins de sommets.".into()); }
                }
            } else if resp.double_clicked() {
                if let Some((_, c, i, q)) = arete.filter(|_| sommet.is_none()) {
                    if let Some(p) = self.depuis_ecran(rect, q) { self.dessin2d.dalle[c].insert(i + 1, p.to_array()); }
                }
            } else if resp.clicked() {
                if let Some((_, c, i)) = sommet {
                    let e = self.dessin2d.edition_contour.as_ref().unwrap();
                    if !e.fermee(c) && (i == 0 || i + 1 == self.dessin2d.dalle[c].len()) {
                        let p = DVec2::from_array(self.dessin2d.dalle[c][i]);
                        let e = self.dessin2d.edition_contour.as_mut().unwrap();
                        e.raccord = Some((c, i == 0));
                        e.dessin = true;
                        self.dessin2d.trace.as_mut().unwrap().points = vec![p];
                        { self.dessin2d.message = None; self.dessin2d.information = None; };
                        return;
                    }
                }
                let e = self.dessin2d.edition_contour.as_mut().unwrap();
                e.selection = sommet.map(|s| s.1).or(arete.map(|a| a.1));
                e.segment = if sommet.is_none() { arete.map(|a| (a.1, a.2)) } else { None };
            }
            return;
        }
        let maj = ui.input(|i| i.modifiers.shift);
        let survol = resp.hover_pos().or(resp.interact_pointer_pos());
        self.dessin2d.curseur = survol.and_then(|q| self.depuis_ecran(rect, q));
        let temps = ui.input(|i| i.time);
        self.dessin2d.point = self.dessin2d.curseur.map(|c| self.point_dessin(rect, c, maj, temps));
        if self.dessin2d.survol_accroche.is_some() || self.dessin2d.survol_segment.is_some() { ui.ctx().request_repaint_after(std::time::Duration::from_millis(100)); }
        // escalier : la longueur de la volée est magnétisée au pas de marche (giron)
        if self.usage_dessin().is_some_and(|(u, _)| u == Usage::Escalier) && self.dessin2d.saisie.is_empty() {
            let giron = self.giron_vise();
            let precedent = self.dessin2d.trace.as_ref().and_then(Trace::precedent);
            if let (Some((p, a)), Some(prec)) = (self.dessin2d.point, precedent) && matches!(a, Accroche::Libre | Accroche::Polaire(_) | Accroche::Ortho) {
                let v = p - prec;
                if v.length() > 1e-6 && giron > 1.0 {
                    let k = (v.length() / giron).round().max(1.0);
                    self.dessin2d.point = Some((prec + v.normalize() * k * giron, a));
                }
            }
        }
        let angle = self.angle_axes_plan();
        if let Some(t) = self.dessin2d.trace.as_mut() {
            t.angle = angle;
        }
        if resp.clicked() {
            if let Some((p, _)) = self.dessin2d.point && self.ajouter_point_raccord(p) { return; }
            if self.reprendre_contour_ouvert(rect, resp.interact_pointer_pos()) { return; }
            if let (Some((p, _)), Some(t)) = (self.dessin2d.point, self.dessin2d.trace.as_mut()) {
                self.dessin2d.saisie.clear();
                self.dessin2d.champ_angle = false;
                { self.dessin2d.message = None; self.dessin2d.information = None; };
                let formes = t.ajouter(p);
                self.enregistrer_formes(formes);
            }
        }
        if resp.clicked_by(PointerButton::Secondary) {
            self.valider_saisie_dessin();
        }
        if resp.hovered() {
            ui.ctx().set_cursor_icon(egui::CursorIcon::Crosshair);
        }
    }

    /// Tracé par-dessus la vue : réticule, aperçu élastique, repérage polaire, marque et
    /// nom de l'accrochage, cote dynamique, ligne de commande.
    pub(in crate::ui) fn dessiner_dessin2d(&self, ui: &egui::Ui, rect: egui::Rect) {
        if self.ruban_edition.selection_mur && self.usage_dessin().is_some_and(|(u,_)|u==Usage::Mur) {
            if let Some(f)=&self.dessin2d.ligne_selection {
                let painter=ui.painter().with_clip_rect(rect);
                let lignes=f.polylignes();
                self.apercu_volume_murs(&painter,rect,&lignes);
                for l in lignes {
                    let pts=l.iter().filter_map(|p|self.vers_ecran(rect,*p)).collect();
                    painter.add(egui::Shape::line(pts,egui::Stroke::new(2.5,Color32::from_rgb(0,120,215))));
                }
            }
            return;
        }
        let Some(trace) = &self.dessin2d.trace else { return };
        let painter = ui.painter().with_clip_rect(rect);
        let bleu = Color32::from_rgb(0, 120, 215);
        let vert = Color32::from_rgb(20, 160, 60);
        if let Some(centre) = trace.precedent()
            && let Some(rayon) = super::saisie::distance_saisie(&self.dessin2d.saisie, crate::config::options::unite().facteur())
        {
            let points: Vec<_> = (0..=128).filter_map(|i| {
                let angle = std::f64::consts::TAU * i as f64 / 128.0;
                self.vers_ecran(rect, centre + DVec2::from_angle(angle) * rayon)
            }).collect();
            if points.len() == 129 { painter.add(egui::Shape::dashed_line(&points, egui::Stroke::new(1.0, vert), 6.0, 4.0)); }
        }
        for c in &self.dessin2d.ouverts {
            let pts: Vec<egui::Pos2> = c.iter().filter_map(|q| self.vers_ecran(rect, *q)).collect();
            painter.add(egui::Shape::line(pts, egui::Stroke::new(1.5, bleu)));
            for q in [c.first(), c.last()].into_iter().flatten().filter_map(|q| self.vers_ecran(rect, *q)) {
                painter.rect_stroke(egui::Rect::from_center_size(q, egui::vec2(10.0, 10.0)), 0.0, egui::Stroke::new(2.0, Color32::from_rgb(200, 50, 45)), egui::StrokeKind::Middle);
            }
        }
        for &(origine, direction) in &self.dessin2d.guides {
            if let (Some(a), Some(b)) = (self.vers_ecran(rect, origine), self.vers_ecran(rect, origine + direction * 1000.0)) {
                let d = (b - a).normalized();
                painter.add(egui::Shape::dashed_line(&[a - d * 4000.0, a + d * 4000.0], egui::Stroke::new(1.0, vert), 6.0, 4.0));
            }
        }
        if let Some(p) = self.dessin2d.reference.and_then(|p| self.vers_ecran(rect, p)) {
            painter.circle_stroke(p, 5.0, egui::Stroke::new(1.5, vert));
        }
        if let Some((p, accroche)) = self.dessin2d.point {
            if let Some(e) = self.vers_ecran(rect, p) {
                // réticule
                if !self.selection_traces_active() {
                    let gris = Color32::from_rgba_unmultiplied(60, 60, 60, 160);
                    painter.line_segment([e - egui::vec2(28.0, 0.0), e + egui::vec2(28.0, 0.0)], egui::Stroke::new(1.0, gris));
                    painter.line_segment([e - egui::vec2(0.0, 28.0), e + egui::vec2(0.0, 28.0)], egui::Stroke::new(1.0, gris));
                }
                // aperçu : ligne tracée, et volume pour un mur
                let apercu = trace.apercu(p);
                match self.usage_dessin().map(|(u, _)| u) {
                    Some(Usage::Mur) if trace.outil!=OutilDessin::Arc || trace.points.len()>=2 => self.apercu_volume_murs(&painter, rect, &apercu),
                    Some(Usage::Surface | Usage::LimitePropriete | Usage::Cadrage) => self.apercu_surface(&painter, rect, &apercu),
                    Some(Usage::Escalier) => self.apercu_escalier(&painter, rect, &apercu),
                    _ => {}
                }
                if trace.perimetre && trace.outil==OutilDessin::Cercle {
                    if let Some(c)=trace.points.first().and_then(|c|self.vers_ecran(rect,*c)) {
                        painter.add(egui::Shape::dashed_line(&[c,e],egui::Stroke::new(1.0,bleu.gamma_multiply(0.6)),5.0,4.0));
                    }
                }
                for l in apercu {
                    let pts: Vec<egui::Pos2> = l.iter().filter_map(|q| self.vers_ecran(rect, *q)).collect();
                    painter.add(egui::Shape::line(pts, egui::Stroke::new(1.5, bleu)));
                }
                // repérage polaire / ortho : ligne de construction en pointillés
                if self.dessin2d.guides.is_empty() && let (Some(prec), Accroche::Polaire(_) | Accroche::Ortho) = (trace.precedent(), accroche) {
                    if let Some(a) = self.vers_ecran(rect, prec) {
                        let dir = (e - a).normalized();
                        painter.add(egui::Shape::dashed_line(&[a - dir * 4000.0, a + dir * 4000.0], egui::Stroke::new(1.0, vert), 6.0, 4.0));
                    }
                }
                // marque d'accrochage
                let m = 6.0;
                let trait_ = egui::Stroke::new(2.0, vert);
                match accroche {
                    Accroche::Extremite => {
                        painter.rect_stroke(egui::Rect::from_center_size(e, egui::vec2(2.0 * m, 2.0 * m)), 0.0, trait_, egui::StrokeKind::Middle);
                    }
                    Accroche::Milieu => {
                        painter.add(egui::Shape::closed_line(vec![e + egui::vec2(0.0, -m), e + egui::vec2(m, m), e + egui::vec2(-m, m)], trait_));
                    }
                    Accroche::Centre => {
                        painter.circle_stroke(e, m, trait_);
                    }
                    Accroche::Quadrant => {
                        painter.add(egui::Shape::closed_line(vec![e + egui::vec2(0.0, -m), e + egui::vec2(m, 0.0), e + egui::vec2(0.0, m), e + egui::vec2(-m, 0.0)], trait_));
                    }
                    Accroche::Intersection => {
                        painter.line_segment([e + egui::vec2(-m, -m), e + egui::vec2(m, m)], trait_);
                        painter.line_segment([e + egui::vec2(-m, m), e + egui::vec2(m, -m)], trait_);
                    }
                    Accroche::Perpendiculaire => {
                        painter.add(egui::Shape::line(vec![e + egui::vec2(-m, -m), e + egui::vec2(-m, m), e + egui::vec2(m, m)], trait_));
                        painter.line_segment([e + egui::vec2(-m, 0.0), e + egui::vec2(0.0, 0.0)], trait_);
                    }
                    _ => {}
                }
                // étiquettes : accrochage, longueur et angle depuis le point précédent
                let mut infos = Vec::new();
                let saisie = &self.dessin2d.saisie;
                let coordonnees = saisie.starts_with(['@', '#']) || saisie.eq_ignore_ascii_case("f");
                if coordonnees { infos.push(format!("{saisie}▏")); }
                if accroche != Accroche::Libre {
                    infos.push(accroche.libelle());
                }
                if let Some(prec) = trace.precedent() {
                    let v = p - prec;
                    let u = crate::config::options::unite();
                    let decimales = u.facteur().log10().round().max(0.0) as usize;
                    let (distance, angle) = saisie.split_once(['<', '>']).map_or((saisie.as_str(), None), |(d, a)| (d, Some(a.trim_end_matches('>'))));
                    let distance = if !coordonnees && !distance.is_empty() { format!("{distance}{}", if !self.dessin2d.champ_angle { "▏" } else { "" }) } else { format!("{:.*}", decimales, v.length() / u.facteur()) };
                    let angle = if !coordonnees && angle.is_some() { format!("{}{}", angle.unwrap(), if self.dessin2d.champ_angle { "▏" } else { "" }) } else { format!("{:.1}", v.y.atan2(v.x).to_degrees().rem_euclid(360.0)) };
                    infos.push(format!("{distance}{} · {angle}°", u.suffixe()));
                } else if !saisie.is_empty() && !coordonnees {
                    infos.push(format!("{saisie}▏"));
                }
                if !infos.is_empty() {
                    let texte = painter.layout_no_wrap(infos.join("   "), egui::FontId::proportional(12.0), Color32::WHITE);
                    let cadre = egui::Rect::from_min_size(e + egui::vec2(16.0, 14.0), texte.size() + egui::vec2(10.0, 4.0));
                    painter.rect_filled(cadre, 3.0, Color32::from_rgba_unmultiplied(40, 40, 40, 220));
                    painter.galley(cadre.min + egui::vec2(5.0, 2.0), texte, Color32::WHITE);
                }
            }
        }
        // contours déjà fermés de l'élément horizontal en cours (parties et trémies)
        if self.dessin2d.edition_contour.is_some() || self.usage_dessin().is_some_and(|(u, _)| matches!(u, Usage::Surface | Usage::LimitePropriete | Usage::Cadrage)) {
            for (i, c) in self.dessin2d.dalle.iter().enumerate() {
                let pts: Vec<DVec2> = c.iter().map(|p| DVec2::from_array(*p)).collect();
                if let Some(edit) = &self.dessin2d.edition_contour {
                    let couleur = if edit.selection == Some(i) { Color32::from_rgb(65, 135, 190) } else { bleu };
                    let mut ecran: Vec<_> = pts.iter().filter_map(|p| self.vers_ecran(rect, *p)).collect();
                    if edit.fermee(i) && let Some(p) = ecran.first().copied() { ecran.push(p); }
                    painter.add(egui::Shape::line(ecran.clone(), egui::Stroke::new(2.0, couleur)));
                    if let Some((c, k)) = edit.segment && c == i && k + 1 < ecran.len() {
                        painter.line_segment([ecran[k], ecran[k + 1]], egui::Stroke::new(4.0, Color32::from_rgb(230, 110, 20)));
                    }
                    if !edit.dessin { for p in ecran.iter().take(c.len()) { painter.circle_filled(*p, 4.0, couleur); } }
                } else {
                    self.apercu_surface(&painter, rect, std::slice::from_ref(&pts));
                    if let Some(ecran) = pts.iter().map(|&p| self.vers_ecran(rect, p)).collect::<Option<Vec<_>>>() {
                        painter.add(egui::Shape::closed_line(ecran, egui::Stroke::new(1.5, bleu)));
                    }
                }
            }
        }
        // terrain en cours : surface colorée selon la cote, contour fin, poignées ; les
        // cotes sont à côté de chaque point (cotes_terrain)
        if self.usage_dessin().is_some_and(|(u, _)| u == Usage::Terrain) {
            self.apercu_terrain(&painter, rect);
            for (p, q) in self.poignees_terrain(rect) {
                let survol = self.dessin2d.terrain_survol == Some(p);
                match p {
                    PoigneeTerrain::Sommet(_) | PoigneeTerrain::Point(_) => {
                        let c = if matches!(p, PoigneeTerrain::Point(_)) { vert } else { bleu };
                        painter.circle_filled(q, if survol { 7.0 } else { 5.0 }, if survol { c } else { Color32::WHITE });
                        painter.circle_stroke(q, if survol { 7.0 } else { 5.0 }, egui::Stroke::new(2.0, c));
                    }
                    // milieu : insérer un sommet en le glissant
                    PoigneeTerrain::Milieu(_) => {
                        let r = if survol { 5.0 } else { 3.5 };
                        painter.circle_filled(q, r, Color32::from_white_alpha(if survol { 255 } else { 200 }));
                        painter.circle_stroke(q, r, egui::Stroke::new(1.2, bleu.gamma_multiply(if survol { 1.0 } else { 0.6 })));
                        if survol {
                            painter.line_segment([q - egui::vec2(2.5, 0.0), q + egui::vec2(2.5, 0.0)], egui::Stroke::new(1.2, bleu));
                            painter.line_segment([q - egui::vec2(0.0, 2.5), q + egui::vec2(0.0, 2.5)], egui::Stroke::new(1.2, bleu));
                        }
                    }
                }
            }
        }
        // points déjà donnés
        for q in &trace.points {
            if let Some(e) = self.vers_ecran(rect, *q) {
                painter.circle_filled(e, 3.0, bleu);
            }
        }
    }

    /// Instructions de la commande active : texte bleu juste sous la barre de commande.
    pub(in crate::ui) fn dessiner_instructions(&self, ui: &egui::Ui, rect: egui::Rect) {
        use crate::ui::ruban::Outil;
        let painter = ui.painter().with_clip_rect(rect);
        let edition = self.dessin2d.edition_contour.as_ref();
        let usage = self.usage_dessin();
        let outil = self.ruban_edition.outil();
        let consigne_transfo = self.transfo.outil.map(|o| self.transfo.consigne(o));
        let modification = if self.orientation_nord_en_cours { Some("Viser un trait puis cliquer près de l'extrémité indiquant le nord · Échap : annuler") } else { match outil {
            Some(Outil::Deplacer | Outil::Copier | Outil::Tourner | Outil::Symetrie | Outil::Reseau | Outil::Echelle | Outil::Aligner | Outil::Decaler | Outil::Scinder | Outil::Demolir | Outil::Reproduire | Outil::ChangerHote | Outil::Inverser) if consigne_transfo.is_some() => consigne_transfo.as_deref(),
            Some(Outil::Deplacer | Outil::Copier | Outil::Tourner | Outil::Symetrie | Outil::Reseau | Outil::Echelle) => Some("Sélectionner d'abord les éléments à transformer"),
            _ => None,
        }};
        if edition.is_none() && usage.is_none() && modification.is_none() { return; }
        let u = crate::config::options::unite();
        let nom = if self.orientation_nord_en_cours { "NORD DU PROJET".into() } else { match usage {
            Some((Usage::Mur, o)) => format!("MUR · {}", nom_outil(o)),
            Some((Usage::Escalier, _)) => format!("ESCALIER · LIGNE = {}", ["AXE", "BORD GAUCHE", "BORD DROIT"][self.dessin2d.justification_escalier as usize % 3]),
            Some((Usage::GardeCorps, _)) => "GARDE-CORPS".into(),
            Some((Usage::LimitePropriete, o)) => format!("LIMITE DE PROPRIÉTÉ · {}", if o == OutilDessin::Polyligne { "POLYGONE" } else { nom_outil(o) }),
            Some((Usage::Cadrage, _)) => "CADRAGE DE LA VUE".into(),
            Some((Usage::Surface, o)) => format!(
                "{} · {}",
                self.ruban_edition.dalle_active().map_or("SURFACE".into(), |d| d.sorte.nom().to_uppercase()),
                if o == OutilDessin::Polyligne { "POLYGONE" } else { nom_outil(o) }
            ),
            _ => self.dessin2d.trace.as_ref().map_or("MODIFICATION".into(), |t| nom_outil(t.outil).into()),
        }};
        let invite = if let Some(e) = edition.filter(|_| self.selection_traces_active()) {
            if e.segment.is_some() { "Trait sélectionné : Suppr pour le retirer ; glisser un sommet pour le déplacer" }
            else { "Sélectionner un trait ou glisser un sommet ; cliquer un sommet ouvert pour prolonger" }
        } else if usage.is_some_and(|(u, _)| u == Usage::Terrain) && !self.dessin2d.dalle.is_empty() {
            if self.dessin2d.terrain_ajouter_point { "Cliquer dans le terrain pour placer un point coté · glisser un point pour le déplacer · clic droit : retirer" }
            else { "Glisser un sommet ou un point · glisser un milieu de côté pour ajouter un sommet · clic droit : retirer · cotes NGF à côté de chaque point" }
        } else if let Some(m) = modification { m }
        else if self.ruban_edition.selection_mur && usage.is_some_and(|(u,_)|u==Usage::Mur) {"Sélectionner un trait existant dans la vue"} else { self.dessin2d.trace.as_ref().map_or("", |t| t.invite()) };
        let contexte = match usage.map(|(u, _)| u) {
            Some(Usage::Mur) => "Ctrl : justification · Espace : inverser le sens · ",
            Some(Usage::Escalier) => "Ctrl ou Espace : justification de la ligne · Longueur magnétisée au giron · Départ, un sommet par palier, arrivée · Marches calculées selon le niveau supérieur · ",
            Some(Usage::GardeCorps) => "Cliquer les sommets · Sur un escalier, sélectionner Garde-corps dans les options · ",
            Some(Usage::Cadrage) => "Tracer un contour fermé, puis valider · ",
            Some(Usage::Terrain) if self.dessin2d.dalle.is_empty() => "Tracer le périmètre fermé du terrain · ",
            _ => "",
        };
        let aide = if usage.is_some() { format!("{contexte}@x;y · #dx;dy · distance<angle ou distance>angle · Tab : changer de champ · distance ({}) · Maj : ortho", u.suffixe().trim()) } else { String::new() };
        let police = egui::FontId::monospace(13.0);
        let largeur = (rect.width() - 20.0).max(1.0);
        let bleu = Color32::from_rgb(55, 130, 200);
        let mut ligne = egui::text::LayoutJob::default();
        ligne.wrap.max_width = largeur;
        let format = |color| egui::TextFormat { font_id: police.clone(), color, ..Default::default() };
        ligne.append(&format!("{nom} — {invite}"), 0.0, format(bleu));
        if let Some(message) = self.dessin2d.message.as_ref().or(self.dessin2d.information.as_ref()) {
            let couleur = if self.dessin2d.message.is_some() { Color32::from_rgb(200, 50, 45) } else { bleu };
            ligne.append(&format!(" : {message}"), 0.0, format(couleur));
        }
        let g1 = painter.layout_job(ligne);
        let g2 = painter.layout(aide, egui::FontId::proportional(11.0), bleu.gamma_multiply(0.85), largeur);
        let h1 = g1.size().y;
        let haut = rect.top() + self.dessin2d.bas_barre.max(4.0) + 8.0;
        let zone = egui::Rect::from_min_size(egui::pos2(rect.left() + 8.0, haut), egui::vec2(g1.size().x.max(g2.size().x) + 8.0, h1 + g2.size().y + 6.0));
        painter.rect_filled(zone, 3.0, Color32::from_white_alpha(185));
        painter.galley(zone.min + egui::vec2(4.0, 2.0), g1, bleu);
        painter.galley(zone.min + egui::vec2(4.0, 4.0 + h1), g2, bleu);
    }

    /// Volume des murs en cours : faces au sol et en tête, séparations des couches et
    /// arêtes verticales aux extrémités, le long de chaque polyligne de l'aperçu.
    pub(in crate::ui) fn apercu_volume_murs(&self, painter: &egui::Painter, rect: egui::Rect, lignes: &[Vec<DVec2>]) {
        let Some((projet, _)) = &self.natif else { return };
        let options = self.ruban_edition.parametres_mur();
        let Ok((bas, haut)) = options.bornes(projet) else { return };
        let gabarit = options.gabarit(projet);
        let e = gabarit.epaisseur();
        if e <= 0.0 || haut <= bas {
            return;
        }
        // milieu de l'épaisseur : décalé de la ligne tracée selon l'implantation
        let centre = -gabarit.decalage_reference(options.implantation.reference());
        let mut separations = vec![-0.5, 0.5];
        let mut k = 0.5;
        for c in gabarit.couches.iter().take(gabarit.couches.len().saturating_sub(1)) {
            k -= c.epaisseur / e;
            separations.push(k);
        }
        let couleur = Color32::from_rgb(55, 130, 200);
        let ecran = |q: DVec2, z: f64| self.ecran_3d(rect, DVec3::new(q.x, q.y, z));
        for l in lignes {
            for w in l.windows(2) {
                let (a, b) = (w[0], w[1]);
                let Some(n) = (b - a).try_normalize().map(|d| d.perp()) else { continue };
                let n = if options.inverse { -n } else { n };
                let c = |p: DVec2, k: f64| p + n * (centre + e * k);
                let sol: Option<Vec<_>> = [c(a, -0.5), c(b, -0.5), c(b, 0.5), c(a, 0.5)].iter().map(|&q| ecran(q, bas)).collect();
                if let Some(sol) = sol {
                    painter.add(egui::Shape::convex_polygon(sol, couleur.gamma_multiply(0.18), egui::Stroke::NONE));
                }
                for (i, &k) in separations.iter().enumerate() {
                    let trait_ = egui::Stroke::new(if i < 2 { 1.5 } else { 0.8 }, couleur.gamma_multiply(if i < 2 { 1.0 } else { 0.8 }));
                    for z in [bas, haut] {
                        if let (Some(p), Some(q)) = (ecran(c(a, k), z), ecran(c(b, k), z)) {
                            painter.line_segment([p, q], trait_);
                        }
                    }
                }
            }
            // arêtes verticales aux extrémités (pas pour une forme fermée)
            let ferme = l.len() > 2 && l.first().zip(l.last()).is_some_and(|(a, b)| a.distance(*b) < 1e-6);
            if ferme || l.len() < 2 {
                continue;
            }
            for (p, voisin) in [(l[0], l[1]), (l[l.len() - 1], l[l.len() - 2])] {
                let Some(n) = (p - voisin).try_normalize().map(|d| d.perp()) else { continue };
                let n = if options.inverse { -n } else { n };
                for k in [-0.5, 0.5] {
                    let q = p + n * (centre + e * k);
                    if let (Some(x), Some(y)) = (ecran(q, bas), ecran(q, haut)) {
                        painter.line_segment([x, y], egui::Stroke::new(1.0, couleur));
                    }
                }
            }
        }
    }

    /// Surface en cours : remplissage et aire du contour (fermé sur le curseur).
    pub(in crate::ui) fn apercu_surface(&self, painter: &egui::Painter, rect: egui::Rect, lignes: &[Vec<DVec2>]) {
        let couleur = Color32::from_rgb(55, 130, 200);
        for l in lignes {
            let mut pts = l.clone();
            if pts.len() > 2 && pts.first().zip(pts.last()).is_some_and(|(a, b)| a.distance(*b) < 1e-6) {
                pts.pop();
            }
            if pts.len() < 3 {
                continue;
            }
            let Some(ecr) = pts.iter().map(|&q| self.vers_ecran(rect, q)).collect::<Option<Vec<_>>>() else { continue };
            let contour: Vec<[f64; 2]> = pts.iter().map(|p| p.to_array()).collect();
            let aire = crate::modele::volumes::geometrie::aire_pair_impair(std::slice::from_ref(&contour));
            let mut m = egui::Mesh::default();
            for t in crate::noyau::texte::trianguler(vec![ecr.iter().map(|p| [p.x, p.y]).collect()]) {
                for p in t {
                    m.colored_vertex(egui::pos2(p[0], p[1]), couleur.gamma_multiply(0.18));
                }
            }
            m.indices = (0..m.vertices.len() as u32).collect();
            painter.add(egui::Shape::mesh(m));
            let mut ferme = ecr.clone();
            ferme.push(ecr[0]);
            painter.add(egui::Shape::dashed_line(&ferme[ferme.len() - 2..], egui::Stroke::new(1.0, couleur), 5.0, 4.0));
            let c = ecr.iter().fold(egui::Vec2::ZERO, |s, p| s + p.to_vec2()) / ecr.len() as f32;
            let galee = painter.layout_no_wrap(format!("{:.2} m²", aire / 1e6), egui::FontId::proportional(13.0), Color32::from_gray(30));
            let cadre = egui::Rect::from_center_size(c.to_pos2(), galee.size() + egui::vec2(12.0, 6.0));
            painter.rect(cadre, 3.0, Color32::from_rgba_unmultiplied(255, 255, 255, 235), egui::Stroke::new(1.0, couleur), egui::StrokeKind::Inside);
            painter.galley(cadre.min + egui::vec2(6.0, 3.0), galee, Color32::from_gray(30));
        }
    }

}
