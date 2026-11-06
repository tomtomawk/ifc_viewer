//! Clic droit dans la vue hors commande (répéter, annuler, sélection, visibilité,
//! verrou, démolir) et poignée de déplacement au milieu du mur sélectionné.
use super::*;
use crate::modele::edition::commandes::Commande;
use crate::ui::ruban::Outil;

/// Action choisie dans le menu du clic droit.
#[derive(Clone)]
pub(super) enum ActionMenu {
    Repeter(Outil),
    Annuler,
    Retablir,
    Identiques { dans_la_vue: bool },
    Garder(&'static str),
    Verrou(bool),
    Masquer,
    MasquerVue,
    Isoler,
    Afficher,
    Demolir,
    Inverser,
    Supprimer,
}

/// Catégorie d'un élément natif, pour le filtre de sélection.
pub(super) fn categorie(p: &crate::modele::Projet, id: &str) -> &'static str {
    if p.murs.iter().any(|x| x.id == id) || p.assemblage(id).is_some() { "Murs" }
    else if p.dalles.iter().any(|x| x.id == id) { "Dalles" }
    else if p.ouvertures.iter().any(|x| x.id == id) { "Ouvertures" }
    else if p.barres.iter().any(|x| x.id == id) { "Structures" }
    else if p.escaliers.iter().any(|x| x.id == id) || p.garde_corps.iter().any(|x| x.id == id) { "Escaliers et garde-corps" }
    else if p.terrains.iter().any(|x| x.id == id) || p.limites_propriete.iter().any(|x| x.id == id) { "Site" }
    else { "Autres" }
}

impl ViewerApp {
    /// Menu du clic droit dans la vue, hors commande.
    pub(super) fn menu_contextuel(&mut self, resp: &egui::Response) {
        let selection = self.selection_bim();
        let (categories, verrouilles, demolis) = match &self.natif {
            Some((p, _)) => {
                let mut c: BTreeMap<&'static str, usize> = BTreeMap::new();
                for id in &selection {
                    *c.entry(categorie(p, id)).or_default() += 1;
                }
                (c, selection.iter().filter(|id| p.verrouille(id)).count(), selection.iter().filter(|id| p.demolitions.contains_key(*id)).count())
            }
            None => (BTreeMap::new(), 0, 0),
        };
        let mut action = None;
        resp.context_menu(|ui| {
            if let Some(o) = self.derniere_commande {
                if ui.button(format!("Répéter : {} (Entrée)", o.nom())).clicked() { action = Some(ActionMenu::Repeter(o)); }
            }
            if ui.button("Annuler (Ctrl+Z)").clicked() { action = Some(ActionMenu::Annuler); }
            if ui.button("Rétablir (Ctrl+Maj+Z)").clicked() { action = Some(ActionMenu::Retablir); }
            if selection.is_empty() {
                ui.separator();
                if ui.button("Afficher les éléments masqués").clicked() { action = Some(ActionMenu::Afficher); }
            } else {
                ui.separator();
                if selection.len() == 1 {
                    if ui.button("Sélectionner les identiques dans la vue").clicked() { action = Some(ActionMenu::Identiques { dans_la_vue: true }); }
                    if ui.button("Sélectionner les identiques dans le projet").clicked() { action = Some(ActionMenu::Identiques { dans_la_vue: false }); }
                }
                if categories.len() > 1 {
                    ui.menu_button("Filtrer la sélection", |ui| {
                        for (c, n) in &categories {
                            if ui.button(format!("Garder seulement : {c} ({n})")).clicked() { action = Some(ActionMenu::Garder(c)); }
                        }
                    });
                }
                ui.separator();
                if verrouilles < selection.len() && ui.button("Verrouiller").clicked() { action = Some(ActionMenu::Verrou(true)); }
                if verrouilles > 0 && ui.button("Déverrouiller").clicked() { action = Some(ActionMenu::Verrou(false)); }
                if ui.button("Masquer temporairement (H)").clicked() { action = Some(ActionMenu::Masquer); }
                if ui.button("Masquer dans la vue").clicked() { action = Some(ActionMenu::MasquerVue); }
                if ui.button("Isoler (I)").clicked() { action = Some(ActionMenu::Isoler); }
                ui.separator();
                if ui.button(if demolis == selection.len() { "Annuler la démolition" } else { "Démolir" }).clicked() { action = Some(ActionMenu::Demolir); }
                if selection.len() == 1 && ui.button("Inverser").clicked() { action = Some(ActionMenu::Inverser); }
                if ui.button("Supprimer (Suppr)").clicked() { action = Some(ActionMenu::Supprimer); }
            }
        });
        if let Some(a) = action {
            self.action_menu(a);
        }
    }

    /// Exécute une action du menu du clic droit (aussi appelée par le pilote).
    pub(super) fn action_menu(&mut self, a: ActionMenu) {
        let selection = self.selection_bim();
        let resultat = match a {
            ActionMenu::Repeter(o) => { self.relancer_commande(o); Ok(()) }
            ActionMenu::Annuler | ActionMenu::Retablir => { self.parcourir_historique(matches!(a, ActionMenu::Retablir)); Ok(()) }
            ActionMenu::Identiques { dans_la_vue } => {
                let Some((p, _)) = &self.natif else { return };
                let Some(id) = selection.first() else { return };
                let mut ids = p.identiques(id);
                if dans_la_vue {
                    let visibles: HashSet<String> = self.scene.as_ref().map(|s| s.objects.iter().enumerate()
                        .filter(|(i, _)| self.visible.get(*i).copied().unwrap_or(false))
                        .filter_map(|(_, o)| self.cle_native(self.cible_clic(o.id))).collect()).unwrap_or_default();
                    ids.retain(|x| visibles.contains(x));
                }
                self.status = format!("{} élément(s) identique(s) sélectionné(s).", ids.len());
                self.bim.selection = ids;
                self.selection_multiple.clear();
                self.selected = None;
                Ok(())
            }
            ActionMenu::Garder(c) => {
                let Some((p, _)) = &self.natif else { return };
                let ids: Vec<String> = selection.into_iter().filter(|id| categorie(p, id) == c).collect();
                self.status = format!("Sélection filtrée : {} {c}.", ids.len());
                self.bim.selection = ids;
                self.selection_multiple.clear();
                self.selected = None;
                Ok(())
            }
            ActionMenu::Verrou(v) => { self.verrouiller_selection(v); Ok(()) }
            ActionMenu::Masquer => { self.visibilite_selection(Outil::Masquer); Ok(()) }
            ActionMenu::MasquerVue => { self.visibilite_selection(Outil::MasquerVue); Ok(()) }
            ActionMenu::Isoler => { self.visibilite_selection(Outil::Isoler); Ok(()) }
            ActionMenu::Afficher => { self.basculer_afficher_masques(); Ok(()) }
            ActionMenu::Demolir => {
                let Some((p, _)) = &self.natif else { return };
                let tous = selection.iter().all(|id| p.demolitions.contains_key(id));
                let phase = if tous { None } else { p.phases.last().cloned() };
                self.executer_bim(Commande::Demolir { ids: selection, phase }).map(|_| ())
            }
            ActionMenu::Inverser => {
                let Some(id) = selection.into_iter().next() else { return };
                let ouverture = self.natif.as_ref().is_some_and(|(p, _)| p.ouvertures.iter().any(|o| o.id == id));
                let c = if ouverture { Commande::InverserOuverture { id, paumelles: false } } else { Commande::InverserMur { id } };
                self.executer_bim(c).map(|_| ())
            }
            ActionMenu::Supprimer => { self.demander_suppression(); Ok(()) }
        };
        if let Err(e) = resultat {
            self.error = Some(e);
        }
    }

    /// Poignée au milieu du mur sélectionné (hors commande) : la glisser déplace le mur ;
    /// les flèches d'inversion à côté retournent son sens. Rend vrai si la souris est prise.
    pub(super) fn poignee_milieu(&mut self, ui: &egui::Ui, rect: egui::Rect, resp: &egui::Response, axe: &Option<(String, [f64; 2], [f64; 2], f64, f64)>) -> bool {
        let Some((id, a, b, z, _)) = axe else {
            self.glisse_milieu = None;
            return false;
        };
        let m = [(a[0] + b[0]) / 2.0, (a[1] + b[1]) / 2.0];
        let Some(me) = self.ecran_plan(rect, m, *z) else { return false };
        let (Some(ea), Some(eb)) = (self.ecran_plan(rect, *a, *z), self.ecran_plan(rect, *b, *z)) else { return false };
        let n = egui::vec2(-(eb.y - ea.y), eb.x - ea.x).normalized();
        let fleches = me + n * 22.0;
        let painter = ui.painter().with_clip_rect(rect);
        let bleu = Color32::from_rgb(40, 120, 210);
        let survol = resp.hover_pos();
        let pres = |q: egui::Pos2, r: f32| survol.is_some_and(|h| h.distance(q) <= r);
        // carré de déplacement
        let c = if pres(me, 8.0) || self.glisse_milieu.is_some() { Color32::from_rgb(230, 120, 20) } else { bleu };
        painter.rect_filled(egui::Rect::from_center_size(me, egui::vec2(9.0, 9.0)), 1.0, c);
        // flèches d'inversion
        let c2 = if pres(fleches, 9.0) { Color32::from_rgb(230, 120, 20) } else { bleu };
        let u = (eb - ea).normalized() * 7.0;
        for (s, k) in [(1.0f32, -1.0f32), (-1.0, 1.0)] {
            let base = fleches + n * 3.5 * k;
            painter.line_segment([base - u * s, base + u * s], egui::Stroke::new(1.6, c2));
            painter.line_segment([base + u * s, base + u * s * 0.4 + n * 3.0 * k], egui::Stroke::new(1.6, c2));
        }
        if resp.clicked() && resp.interact_pointer_pos().is_some_and(|q| q.distance(fleches) <= 9.0) {
            match self.executer_bim(Commande::InverserMur { id: id.clone() }) {
                Ok(_) => self.status = "Mur inversé — Ctrl+Z pour annuler.".into(),
                Err(e) => self.error = Some(e),
            }
            return true;
        }
        if resp.drag_started_by(PointerButton::Primary) && ui.input(|i| i.pointer.press_origin()).is_some_and(|q| q.distance(me) <= 8.0) {
            self.glisse_milieu = Some((id.clone(), m, m));
        }
        if let Some(depart) = self.glisse_milieu.as_ref().map(|g| g.1) {
            if let Some(q) = resp.interact_pointer_pos().or(resp.hover_pos()) {
                let shift = ui.input(|i| i.modifiers.shift);
                let p = self.point_tracage(q, rect, shift, Some(depart), *z).map(|(p, _)| p);
                if let (Some(p), Some(g)) = (p, self.glisse_milieu.as_mut()) {
                    g.2 = p;
                }
            }
            let courant = self.glisse_milieu.as_ref().map_or(depart, |g| g.2);
            let d = [courant[0] - depart[0], courant[1] - depart[1]];
            if let (Some(x), Some(y)) = (self.ecran_plan(rect, [a[0] + d[0], a[1] + d[1]], *z), self.ecran_plan(rect, [b[0] + d[0], b[1] + d[1]], *z)) {
                painter.line_segment([x, y], egui::Stroke::new(2.5, bleu.gamma_multiply(0.7)));
            }
            if ui.input(|i| i.key_pressed(egui::Key::Escape)) || resp.clicked_by(PointerButton::Secondary) {
                self.glisse_milieu = None;
                return true;
            }
            if resp.drag_stopped() {
                let id = id.clone();
                self.glisse_milieu = None;
                if d[0].hypot(d[1]) > 0.5 {
                    match self.executer_bim(Commande::Deplacer { ids: vec![id], delta: [d[0], d[1], 0.0] }) {
                        Ok(_) => self.status = "Mur déplacé — Ctrl+Z pour annuler.".into(),
                        Err(e) => self.error = Some(e),
                    }
                }
            }
            return true;
        }
        false
    }
}
