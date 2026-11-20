//! Panneaux ancrables : contenu, en-tête, déplacement et ancrage.
use crate::*;

impl Application {
    /// Contenu d'un panneau déplaçable.
    pub(crate) fn panneau(&mut self, ui: &mut egui::Ui, p: Panneau) {
        // Le contenu garde une largeur lisible sans imposer cette largeur au volet.
        // Le défilement permet de retrouver les champs même dans un volet très étroit.
        if self.ancrages[p as usize]!=Ancrage::Externe {
            self.volets[p as usize].taille=Some(ui.available_size());
        }
        let largeur = ui.available_width().max(280.0);
        egui::ScrollArea::both()
            .id_salt(("contenu_panneau", p.nom()))
            .min_scrolled_width(0.0)
            .min_scrolled_height(0.0)
            .auto_shrink([false, false])
            .show(ui, |ui| {
                ui.set_width(largeur);
                self.contenu_panneau(ui, p);
            });
    }

    pub(crate) fn contenu_panneau(&mut self, ui: &mut egui::Ui, p: Panneau) {
        if p == Panneau::Proprietes && self.mode_famille() { self.categorie_famille_ui(ui); }
        // onglet Famille : les panneaux montrent la famille
        if self.famille_active && self.famille.is_some() {
            match p {
                Panneau::Proprietes => self.proprietes_famille(ui),
                Panneau::Arborescence => self.arborescence_famille(ui),
            }
            return;
        }
        match p {
            Panneau::Proprietes => self.volet_proprietes(ui),
            Panneau::Arborescence => self.volet_arborescence(ui),
        }
    }

    /// Poignée de déplacement et menu pour détacher ou réancrer le volet.
    pub(crate) fn entete_panneau(&mut self, ui: &mut egui::Ui, p: Panneau, titre: &str) {
        let i = p as usize;
        ui.horizontal(|ui| {
            let (rect, _) = ui.allocate_exact_size(egui::vec2(ui.available_width(), 20.0), Sense::hover());
            let resp = ui.interact(rect, egui::Id::unique(("entete_panneau", p.nom())), Sense::click_and_drag());
            let encre = ui.visuals().strong_text_color();
            ui.painter_at(rect).text(rect.left_center(), egui::Align2::LEFT_CENTER, titre, egui::FontId::proportional(15.0), encre);
            if resp.drag_started_by(PointerButton::Primary)
                && let Some(q)=resp.interact_pointer_pos() {
                    let appui=ui.input(|i|i.pointer.press_origin()).unwrap_or(q);
                    let decalage=appui-rect.min;
                    let origine=ui.ctx().input(|i|i.viewport().inner_rect.map_or(egui::Pos2::ZERO,|r|r.min));
                    self.volets[i].position=Some(origine+(q-decalage).to_vec2());
                    self.deplacement_panneau=Some((p,decalage,ui.ctx().viewport_id()));
                    self.ancrer(p,Ancrage::Externe);
                }
            self.volets[i].entete=Some(rect);
            self.menu_volet(&resp,p);
            if self.ancrages[i]==Ancrage::Externe && resp.double_clicked() {
                self.ancrer(p,if p==Panneau::Proprietes {Ancrage::Gauche}else{Ancrage::Droite});
            }
            let survol = if resp.dragged() { egui::CursorIcon::Grabbing } else { egui::CursorIcon::Grab };
            resp.on_hover_cursor(survol).on_hover_text(if self.ancrages[i]==Ancrage::Externe {"Glisser pour déplacer ; double-clic pour réintégrer le volet"} else {"Glisser le titre pour détacher et déplacer le volet sur un autre écran"});
        });
    }

    pub(crate) fn cible_ancrage(&self, q: egui::Pos2) -> Option<Ancrage> {
        crate::socle::volets::cible_volet(self.zone_panneaux,q)
    }

    /// Le suivi reste actif pendant le passage d'un panneau ancré à une fenêtre flottante.
    pub(crate) fn deplacer_panneau(&mut self, ctx: &egui::Context) {
        let Some((p,decalage,source))=self.deplacement_panneau else{return};
        // Lire le viewport où le geste a commencé, aussi bien ancré que déjà flottant.
        if let Some((position,local,enfonce))=crate::socle::volets::suivre_pointeur(ctx,source,decalage) {
            self.volets[p as usize].position=Some(position);
            self.volets[p as usize].cible=self.cible_ancrage(local);
            if !enfonce {
                if let Some(a)=self.volets[p as usize].cible {self.ancrer(p,a);}
                self.deplacement_panneau=None;
                self.volets[p as usize].cible=None;
            }
        }
        ctx.request_repaint();
    }

    pub(crate) fn apercu_ancrage(&self, ctx: &egui::Context) {
        let Some((p, _, _)) = self.deplacement_panneau else { return };
        let Some(cible) = self.volets[p as usize].cible else { return };
        let zone = self.zone_panneaux;
        let largeur = self.largeurs_panneaux[p as usize].min(zone.width() * 0.45);
        let x = if cible == Ancrage::Gauche { zone.left() } else { zone.right() - largeur };
        let rect = egui::Rect::from_min_size(egui::pos2(x, zone.top()), egui::vec2(largeur, zone.height()));
        let peintre = ctx.layer_painter(egui::LayerId::new(egui::Order::Tooltip, egui::Id::unique("apercu_ancrage")));
        peintre.rect_filled(rect, 4.0, Color32::from_rgba_unmultiplied(90, 140, 195, 45));
        peintre.rect_stroke(rect, 4.0, egui::Stroke::new(1.5, Color32::from_rgb(90, 140, 195)), egui::StrokeKind::Inside);
    }

    /// Ancre un panneau, ou le rend flottant au-dessus de la vue (à sa dernière place
    /// flottante, ramenée dans la vue).
    pub(crate) fn ancrer(&mut self, p: Panneau, a: Ancrage) {
        let i = p as usize;
        if a == Ancrage::Flottant && self.ancrages[i] != a && self.pos_imposee[i].is_none() && self.rect_vue.is_positive() {
            let v = self.rect_vue;
            let q = self.pos_flottants[i];
            self.pos_imposee[i] = Some(egui::pos2(q.x.clamp(v.left(), (v.right() - 200.0).max(v.left())), q.y.clamp(v.top(), (v.bottom() - 200.0).max(v.top()))));
        }
        self.ancrages[i] = a;
        if a != Ancrage::Flottant {
            self.pos_imposee[i] = None;
        }
    }

}
