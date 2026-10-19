//! Interface egui : arborescence, classes, propriétés et vue 3D.

use crate::camera::{Camera, MAX_PITCH};
use crate::model::pretty_type;
use crate::render::{FrameParams, Renderer};
use crate::scene::{self, Scene};
use crate::step::Id;
use eframe::egui::{self, Align, Color32, PointerButton, RichText, Sense};
use eframe::glow;
use glam::{Vec2, Vec3};
use std::collections::{BTreeMap, HashSet};
use std::path::PathBuf;
use std::sync::mpsc::{Receiver, channel};
use std::sync::{Arc, Mutex};

enum LoadMsg {
    Progress(String),
    Done(Result<Box<Scene>, String>),
}

/// Classes masquées à l'ouverture.
const HIDDEN_BY_DEFAULT: &[&str] = &["IFCSPACE", "IFCOPENINGELEMENT", "IFCOPENINGSTANDARDCASE", "IFCVIRTUALELEMENT", "IFCANNOTATION"];

const SPATIAL: &[&str] = &[
    "IFCPROJECT", "IFCSITE", "IFCBUILDING", "IFCBUILDINGSTOREY", "IFCSPACE", "IFCFACILITY", "IFCFACILITYPART",
    "IFCBRIDGE", "IFCBRIDGEPART", "IFCROAD", "IFCROADPART", "IFCRAILWAY", "IFCRAILWAYPART", "IFCMARINEFACILITY",
    "IFCSPATIALZONE", "IFCEXTERNALSPATIALELEMENT",
];

#[derive(PartialEq, Clone, Copy)]
enum LeftTab {
    Tree,
    Classes,
}

pub struct ViewerApp {
    gl: Option<Arc<glow::Context>>,
    renderer: Arc<Mutex<Option<Renderer>>>,
    scene: Option<Scene>,
    loading: Option<Receiver<LoadMsg>>,
    status: String,
    error: Option<String>,
    camera: Camera,
    selected: Option<Id>,
    reveal: Option<Id>,
    hidden_classes: HashSet<String>,
    hidden_objects: HashSet<u32>,
    hidden_storeys: HashSet<Id>,
    isolated: Option<HashSet<u32>>,
    visible: Vec<bool>,
    visibility_dirty: bool,
    left_tab: LeftTab,
    filter: String,
    show_unsupported: bool,
}

impl ViewerApp {
    pub fn new(cc: &eframe::CreationContext<'_>, path: Option<PathBuf>) -> Self {
        let mut error = None;
        let renderer = match cc.gl.as_ref().map(|gl| Renderer::new(gl)) {
            Some(Ok(r)) => Some(r),
            Some(Err(e)) => {
                error = Some(e);
                None
            }
            None => {
                error = Some("Contexte OpenGL indisponible".into());
                None
            }
        };
        cc.egui_ctx.set_visuals(egui::Visuals::light());
        let mut app = ViewerApp {
            gl: cc.gl.clone(),
            renderer: Arc::new(Mutex::new(renderer)),
            scene: None,
            loading: None,
            status: "Ouvrir un fichier IFC (Ctrl+O) ou le glisser dans la fenêtre.".into(),
            error,
            camera: Camera::default(),
            selected: None,
            reveal: None,
            hidden_classes: HashSet::new(),
            hidden_objects: HashSet::new(),
            hidden_storeys: HashSet::new(),
            isolated: None,
            visible: Vec::new(),
            visibility_dirty: false,
            left_tab: LeftTab::Tree,
            filter: String::new(),
            show_unsupported: false,
        };
        if let Some(p) = path {
            app.open(p, &cc.egui_ctx);
        }
        app
    }

    fn open(&mut self, path: PathBuf, ctx: &egui::Context) {
        let (tx, rx) = channel();
        let ctx = ctx.clone();
        self.loading = Some(rx);
        self.error = None;
        self.status = format!("Chargement de {}…", path.display());
        std::thread::Builder::new()
            .stack_size(64 << 20)
            .spawn(move || {
                let progress = |s: String| {
                    let _ = tx.send(LoadMsg::Progress(s));
                    ctx.request_repaint();
                };
                let res = scene::load(&path, &progress).map(Box::new);
                let _ = tx.send(LoadMsg::Done(res));
                ctx.request_repaint();
            })
            .expect("thread de chargement");
    }

    fn poll_loading(&mut self) {
        let Some(rx) = &self.loading else { return };
        while let Ok(msg) = rx.try_recv() {
            match msg {
                LoadMsg::Progress(s) => self.status = s,
                LoadMsg::Done(Ok(scene)) => {
                    self.loading = None;
                    self.install_scene(*scene);
                    return;
                }
                LoadMsg::Done(Err(e)) => {
                    self.loading = None;
                    self.status = "Échec du chargement.".into();
                    self.error = Some(e);
                    return;
                }
            }
        }
    }

    fn install_scene(&mut self, scene: Scene) {
        if let (Some(gl), Some(r)) = (&self.gl, self.renderer.lock().unwrap().as_mut()) {
            r.upload_vertices(gl, &scene.vertices);
        }
        self.status = format!(
            "{} — {} — {} entités, {} objets 3D, {} triangles — {:.2} s",
            scene.file_name,
            if scene.model.file.schema.is_empty() { "schéma ?" } else { &scene.model.file.schema },
            scene.model.file.entities.len(),
            scene.objects.len(),
            scene.triangles,
            scene.load_ms as f64 / 1000.0,
        );
        self.camera = Camera::default();
        self.camera.scene_radius = (scene.max - scene.min).length() * 0.5;
        self.camera.fit(scene.min, scene.max);
        self.selected = None;
        self.hidden_objects.clear();
        self.hidden_storeys.clear();
        self.isolated = None;
        self.hidden_classes = HIDDEN_BY_DEFAULT.iter().map(|s| s.to_string()).collect();
        self.visible = vec![true; scene.objects.len()];
        self.scene = Some(scene);
        self.visibility_dirty = true;
    }

    fn update_visibility(&mut self) {
        if !self.visibility_dirty {
            return;
        }
        self.visibility_dirty = false;
        let Some(scene) = &self.scene else { return };
        let mut opaque = Vec::new();
        let mut transp = Vec::new();
        for (i, o) in scene.objects.iter().enumerate() {
            let vis = !self.hidden_classes.contains(&*o.class)
                && !self.hidden_objects.contains(&(i as u32))
                && o.storey.is_none_or(|s| !self.hidden_storeys.contains(&s))
                && self.isolated.as_ref().is_none_or(|iso| iso.contains(&(i as u32)));
            self.visible[i] = vis;
            if vis {
                opaque.extend(o.opaque.start..o.opaque.start + o.opaque.count);
                transp.extend(o.transparent.start..o.transparent.start + o.transparent.count);
            }
        }
        if let (Some(gl), Some(r)) = (&self.gl, self.renderer.lock().unwrap().as_mut()) {
            r.upload_indices(gl, &opaque, &transp);
        }
    }

    fn select(&mut self, id: Option<Id>, reveal: bool) {
        self.selected = id;
        if reveal {
            self.reveal = id;
            if id.is_some() {
                self.left_tab = LeftTab::Tree;
                self.filter.clear();
            }
        }
    }

    fn selected_object(&self) -> Option<u32> {
        let s = self.scene.as_ref()?;
        s.object_of.get(&self.selected?).copied()
    }

    /// Objets 3D d'un nœud et de ses descendants.
    fn objects_under(&self, id: Id) -> Vec<u32> {
        let Some(s) = &self.scene else { return vec![] };
        let mut out = Vec::new();
        let mut stack = vec![id];
        let mut seen = HashSet::new();
        while let Some(n) = stack.pop() {
            if !seen.insert(n) {
                continue;
            }
            if let Some(&o) = s.object_of.get(&n) {
                out.push(o);
            }
            if let Some(ch) = s.model.children.get(&n) {
                stack.extend(ch.iter().copied());
            }
        }
        out
    }

    fn frame_objects(&mut self, objs: &[u32]) {
        let Some(s) = &self.scene else { return };
        let mut mn = Vec3::splat(f32::MAX);
        let mut mx = Vec3::splat(f32::MIN);
        for &o in objs {
            let o = &s.objects[o as usize];
            mn = mn.min(o.min);
            mx = mx.max(o.max);
        }
        if mn.x <= mx.x {
            self.camera.fit(mn, mx);
        }
    }

    fn fit_all(&mut self) {
        let Some(s) = &self.scene else { return };
        let objs: Vec<u32> = (0..s.objects.len() as u32).filter(|&i| self.visible[i as usize]).collect();
        if objs.is_empty() {
            let (a, b) = (s.min, s.max);
            self.camera.fit(a, b);
        } else {
            self.frame_objects(&objs);
        }
    }

    fn open_dialog(&mut self, ctx: &egui::Context) {
        if let Some(p) = rfd::FileDialog::new().add_filter("IFC", &["ifc", "IFC"]).pick_file() {
            self.open(p, ctx);
        }
    }

    // ------------------------------------------------------------------ UI

    fn top_bar(&mut self, ui: &mut egui::Ui) {
        let ctx = ui.ctx().clone();
        ui.horizontal(|ui| {
            if ui.button("Ouvrir…").on_hover_text("Ctrl+O").clicked() {
                self.open_dialog(&ctx);
            }
            ui.separator();
            let has = self.scene.is_some();
            ui.add_enabled_ui(has, |ui| {
                if ui.button("Cadrer").on_hover_text("F").clicked() {
                    self.fit_all();
                }
                if ui.button("Dessus").clicked() {
                    self.camera.pitch = MAX_PITCH;
                    self.camera.yaw = -std::f32::consts::FRAC_PI_2;
                }
                if ui.button("Face").clicked() {
                    self.camera.pitch = 0.0;
                    self.camera.yaw = -std::f32::consts::FRAC_PI_2;
                }
                if ui.button("Iso").clicked() {
                    self.camera.pitch = 0.6;
                    self.camera.yaw = -2.3;
                }
                ui.toggle_value(&mut self.camera.ortho, "Ortho");
                ui.separator();
                for (label, classes) in [
                    ("Espaces", &["IFCSPACE"][..]),
                    ("Ouvertures", &["IFCOPENINGELEMENT", "IFCOPENINGSTANDARDCASE"][..]),
                ] {
                    let mut shown = !self.hidden_classes.contains(classes[0]);
                    if ui.checkbox(&mut shown, label).changed() {
                        for c in classes {
                            if shown {
                                self.hidden_classes.remove(*c);
                            } else {
                                self.hidden_classes.insert(c.to_string());
                            }
                        }
                        self.visibility_dirty = true;
                    }
                }
                ui.separator();
                if ui.button("Tout afficher").clicked() {
                    self.hidden_objects.clear();
                    self.hidden_storeys.clear();
                    self.isolated = None;
                    self.visibility_dirty = true;
                }
            });
            if self.loading.is_some() {
                ui.separator();
                ui.spinner();
            }
        });
    }

    fn status_bar(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            if let Some(e) = &self.error {
                ui.colored_label(Color32::from_rgb(200, 40, 40), format!("Erreur : {e}"));
                ui.separator();
            }
            ui.label(&self.status);
            if let Some(s) = &self.scene {
                if !s.unsupported.is_empty() {
                    let n: usize = s.unsupported.iter().map(|u| u.1).sum();
                    ui.separator();
                    if ui.link(format!("{n} entités géométriques ignorées")).clicked() {
                        self.show_unsupported = !self.show_unsupported;
                    }
                }
            }
            ui.with_layout(egui::Layout::right_to_left(Align::Center), |ui| {
                ui.label(RichText::new("Gauche : orbite · Droit/milieu : pan · Molette : zoom · Double-clic : cadrer").weak());
            });
        });
    }

    fn left_panel(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            ui.selectable_value(&mut self.left_tab, LeftTab::Tree, "Arborescence");
            ui.selectable_value(&mut self.left_tab, LeftTab::Classes, "Classes");
        });
        ui.add(egui::TextEdit::singleline(&mut self.filter).hint_text("Filtrer (nom, classe, GlobalId)"));
        ui.separator();
        if self.scene.is_none() {
            ui.label(RichText::new("Aucun modèle chargé").weak());
            return;
        }
        egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
            if !self.filter.trim().is_empty() {
                self.filtered_list(ui);
                return;
            }
            match self.left_tab {
                LeftTab::Tree => {
                    let roots = self.tree_roots();
                    for r in roots {
                        self.tree_node(ui, r, 0);
                    }
                }
                LeftTab::Classes => self.class_list(ui),
            }
        });
        self.reveal = None;
    }

    fn tree_roots(&self) -> Vec<Id> {
        let s = self.scene.as_ref().unwrap();
        let mut roots: Vec<Id> = s.model.project.into_iter().collect();
        // Produits hors arborescence spatiale.
        let orphans = s.model.products.iter().any(|p| !s.model.parent.contains_key(p));
        if orphans {
            roots.push(0);
        }
        roots
    }

    fn tree_node(&mut self, ui: &mut egui::Ui, id: Id, depth: u32) {
        if depth > 32 {
            return;
        }
        let s = self.scene.as_ref().unwrap();
        let model = &s.model;
        let (label, children): (String, Vec<Id>) = if id == 0 {
            let orph: Vec<Id> = model.products.iter().copied().filter(|p| !model.parent.contains_key(p)).collect();
            ("Hors arborescence".into(), orph)
        } else {
            let ty = model.ty(id);
            let name = model.name(id);
            let label = if SPATIAL.contains(&ty) {
                format!("{} {}", pretty_type(ty).trim_start_matches("Ifc"), name)
            } else {
                model.label(id)
            };
            (label, model.children.get(&id).cloned().unwrap_or_default())
        };

        // Enfants spatiaux en nœuds, autres regroupés par classe.
        let mut spatial = Vec::new();
        let mut groups: BTreeMap<String, Vec<Id>> = BTreeMap::new();
        for c in children {
            let ty = model.ty(c);
            if SPATIAL.contains(&ty) {
                spatial.push(c);
            } else {
                groups.entry(ty.to_string()).or_default().push(c);
            }
        }
        let is_storey = model.ty(id) == "IFCBUILDINGSTOREY";
        let reveal_path = self.reveal.is_some_and(|r| self.is_ancestor(id, r));
        let egui_id = ui.make_persistent_id(("node", id));
        let mut state = egui::collapsing_header::CollapsingState::load_with_default_open(ui.ctx(), egui_id, depth < 3);
        if reveal_path {
            state.set_open(true);
        }
        let selected = self.selected == Some(id);
        let leaf = spatial.is_empty() && groups.is_empty();
        let mut clicked = false;
        let mut storey_toggle = None;
        let mut header = |ui: &mut egui::Ui| {
            if is_storey {
                let mut shown = !self.hidden_storeys.contains(&id);
                if ui.checkbox(&mut shown, "").on_hover_text("Afficher le niveau").changed() {
                    storey_toggle = Some(shown);
                }
            }
            let r = ui.selectable_label(selected, label.clone());
            if self.reveal == Some(id) {
                r.scroll_to_me(Some(Align::Center));
            }
            clicked = r.clicked();
        };
        if leaf {
            ui.horizontal(|ui| {
                ui.add_space(18.0);
                header(ui);
            });
        } else {
            state
                .show_header(ui, |ui| header(ui))
                .body(|ui| {
                    for c in spatial {
                        self.tree_node(ui, c, depth + 1);
                    }
                    for (ty, items) in groups {
                        self.class_group(ui, id, &ty, items, depth + 1);
                    }
                });
        }
        if let Some(shown) = storey_toggle {
            if shown {
                self.hidden_storeys.remove(&id);
            } else {
                self.hidden_storeys.insert(id);
            }
            self.visibility_dirty = true;
        }
        if clicked && id != 0 {
            self.select(Some(id), false);
        }
    }

    fn class_group(&mut self, ui: &mut egui::Ui, parent: Id, ty: &str, items: Vec<Id>, depth: u32) {
        let egui_id = ui.make_persistent_id(("group", parent, ty));
        let mut state = egui::collapsing_header::CollapsingState::load_with_default_open(ui.ctx(), egui_id, false);
        if self.reveal.is_some_and(|r| items.iter().any(|&i| i == r || self.is_ancestor(i, r))) {
            state.set_open(true);
        }
        state
            .show_header(ui, |ui| {
                ui.label(RichText::new(format!("{} ({})", pretty_type(ty), items.len())).color(Color32::from_gray(90)));
            })
            .body(|ui| {
                for i in items {
                    self.tree_node(ui, i, depth + 1);
                }
            });
    }

    fn is_ancestor(&self, anc: Id, mut id: Id) -> bool {
        let Some(s) = &self.scene else { return false };
        if anc == 0 {
            // Racine des orphelins
            while let Some(&p) = s.model.parent.get(&id) {
                id = p;
            }
            return s.model.project != Some(id);
        }
        for _ in 0..64 {
            if id == anc {
                return true;
            }
            match s.model.parent.get(&id) {
                Some(&p) => id = p,
                None => return false,
            }
        }
        false
    }

    fn class_list(&mut self, ui: &mut egui::Ui) {
        let s = self.scene.as_ref().unwrap();
        let mut counts: BTreeMap<String, usize> = BTreeMap::new();
        for o in &s.objects {
            *counts.entry(o.class.to_string()).or_default() += 1;
        }
        ui.horizontal(|ui| {
            if ui.small_button("Tout").clicked() {
                self.hidden_classes.clear();
                self.visibility_dirty = true;
            }
            if ui.small_button("Rien").clicked() {
                self.hidden_classes = counts.keys().cloned().collect();
                self.visibility_dirty = true;
            }
        });
        for (cls, n) in counts {
            let mut shown = !self.hidden_classes.contains(&cls);
            if ui.checkbox(&mut shown, format!("{} ({n})", pretty_type(&cls))).changed() {
                if shown {
                    self.hidden_classes.remove(&cls);
                } else {
                    self.hidden_classes.insert(cls);
                }
                self.visibility_dirty = true;
            }
        }
    }

    fn filtered_list(&mut self, ui: &mut egui::Ui) {
        let s = self.scene.as_ref().unwrap();
        let q = self.filter.trim().to_lowercase();
        let mut hits = Vec::new();
        for &p in &s.model.products {
            let name = s.model.name(p);
            let ty = s.model.ty(p);
            let gid = s.model.global_id(p);
            if name.to_lowercase().contains(&q) || ty.to_lowercase().contains(&q) || gid.to_lowercase() == q {
                hits.push((p, format!("{}  ·  {}", s.model.label(p), pretty_type(ty))));
                if hits.len() >= 2000 {
                    break;
                }
            }
        }
        ui.label(RichText::new(format!("{} résultat(s){}", hits.len(), if hits.len() >= 2000 { " (limité)" } else { "" })).weak());
        let mut clicked = None;
        for (p, label) in hits {
            if ui.selectable_label(self.selected == Some(p), label).clicked() {
                clicked = Some(p);
            }
        }
        if let Some(p) = clicked {
            self.select(Some(p), false);
        }
    }

    fn right_panel(&mut self, ui: &mut egui::Ui) {
        ui.heading("Propriétés");
        ui.separator();
        let (Some(s), Some(id)) = (&self.scene, self.selected) else {
            ui.label(RichText::new("Sélectionner un élément dans la vue ou l'arborescence.").weak());
            return;
        };
        let model = &s.model;
        let has_obj = s.object_of.contains_key(&id);
        let under = self.objects_under(id);
        let mut action = None;
        ui.horizontal_wrapped(|ui| {
            if ui.add_enabled(!under.is_empty(), egui::Button::new("Cadrer")).clicked() {
                action = Some("frame");
            }
            if ui.add_enabled(!under.is_empty(), egui::Button::new("Isoler")).on_hover_text("I").clicked() {
                action = Some("isolate");
            }
            if ui.add_enabled(!under.is_empty(), egui::Button::new("Masquer")).on_hover_text("H").clicked() {
                action = Some("hide");
            }
            if ui.button("Copier GlobalId").clicked() {
                ui.ctx().copy_text(model.global_id(id));
            }
        });
        if !has_obj && under.is_empty() {
            ui.label(RichText::new("Pas de géométrie affichable.").weak());
        }
        ui.add_space(4.0);
        let attrs = model.attributes(id);
        let groups = model.property_groups(id);
        egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
            egui::Grid::new("attrs").num_columns(2).striped(true).show(ui, |ui| {
                for (k, v) in attrs {
                    ui.label(RichText::new(k).strong());
                    ui.add(egui::Label::new(v).wrap());
                    ui.end_row();
                }
            });
            for (gi, g) in groups.into_iter().enumerate() {
                egui::CollapsingHeader::new(RichText::new(&g.name).strong())
                    .id_salt(("pset", id, gi))
                    .default_open(true)
                    .show(ui, |ui| {
                        egui::Grid::new(("pgrid", id, gi)).num_columns(2).striped(true).show(ui, |ui| {
                            for (k, v) in g.props {
                                ui.label(k);
                                ui.add(egui::Label::new(v).wrap());
                                ui.end_row();
                            }
                        });
                    });
            }
        });
        match action {
            Some("frame") => self.frame_objects(&under),
            Some("isolate") => self.isolate(under),
            Some("hide") => self.hide(under),
            _ => {}
        }
    }

    fn isolate(&mut self, objs: Vec<u32>) {
        if objs.is_empty() {
            return;
        }
        // Les classes masquées par défaut doivent rester visibles si on les isole.
        if let Some(s) = &self.scene {
            for &o in &objs {
                self.hidden_classes.remove(&*s.objects[o as usize].class);
            }
        }
        self.isolated = Some(objs.into_iter().collect());
        self.visibility_dirty = true;
    }

    fn hide(&mut self, objs: Vec<u32>) {
        self.hidden_objects.extend(objs);
        self.visibility_dirty = true;
    }

    fn unsupported_window(&mut self, ctx: &egui::Context) {
        let Some(s) = &self.scene else { return };
        let mut open = self.show_unsupported;
        egui::Window::new("Entités non prises en charge").open(&mut open).show(ctx, |ui| {
            ui.label("Ces représentations ont été ignorées (aucune géométrie générée) :");
            egui::Grid::new("unsup").striped(true).show(ui, |ui| {
                for (t, n) in &s.unsupported {
                    ui.label(pretty_type(t));
                    ui.label(n.to_string());
                    ui.end_row();
                }
            });
        });
        self.show_unsupported = open;
    }

    fn viewport(&mut self, ui: &mut egui::Ui) {
        let rect = ui.available_rect_before_wrap();
        let resp = ui.allocate_rect(rect, Sense::click_and_drag());
        let aspect = rect.width() / rect.height().max(1.0);
        let ndc = |p: egui::Pos2| {
            Vec2::new((p.x - rect.left()) / rect.width() * 2.0 - 1.0, 1.0 - (p.y - rect.top()) / rect.height() * 2.0)
        };

        if self.scene.is_some() {
            let pick = |app: &Self, p: egui::Pos2| -> Option<(u32, Vec3)> {
                let s = app.scene.as_ref()?;
                let n = ndc(p);
                let (o, d) = app.camera.ray(n.x, n.y, aspect);
                s.pick(o, d, &app.visible).map(|(obj, t)| (obj, o + d * t))
            };

            // Orbite autour du point visé.
            if resp.drag_started_by(PointerButton::Primary) && !ui.input(|i| i.modifiers.shift) {
                if let Some((_, p)) = resp.interact_pointer_pos().and_then(|pp| pick(self, pp)) {
                    self.camera.set_pivot(p);
                }
            }
            let d = resp.drag_delta();
            let shift = ui.input(|i| i.modifiers.shift);
            if resp.dragged_by(PointerButton::Primary) && !shift {
                self.camera.orbit(d.x, d.y);
            } else if resp.dragged_by(PointerButton::Secondary)
                || resp.dragged_by(PointerButton::Middle)
                || (resp.dragged_by(PointerButton::Primary) && shift)
            {
                self.camera.pan(d.x, d.y, rect.height());
            }

            if let Some(hp) = resp.hover_pos() {
                let scroll = ui.input(|i| i.smooth_scroll_delta.y);
                if scroll != 0.0 {
                    let factor = (-scroll * 0.0018).exp();
                    let target = pick(self, hp).map(|(_, p)| p).or_else(|| {
                        // Point sur le plan de visée à la distance de la cible.
                        let n = ndc(hp);
                        let (o, dir) = self.camera.ray(n.x, n.y, aspect);
                        let f = self.camera.forward();
                        let t = (self.camera.target - o).dot(f) / dir.dot(f).max(1e-6);
                        Some(o + dir * t)
                    });
                    self.camera.zoom(factor, if self.camera.ortho { None } else { target });
                }
            }

            if resp.double_clicked() {
                match resp.interact_pointer_pos().and_then(|p| pick(self, p)) {
                    Some((obj, _)) => self.frame_objects(&[obj]),
                    None => self.fit_all(),
                }
            } else if resp.clicked() {
                let hit = resp.interact_pointer_pos().and_then(|p| pick(self, p));
                let id = hit.map(|(obj, _)| self.scene.as_ref().unwrap().objects[obj as usize].id);
                self.select(id, true);
            }

            if resp.hovered() || resp.has_focus() {
                let (f, esc, h, i) = ui.input(|i| {
                    (
                        i.key_pressed(egui::Key::F),
                        i.key_pressed(egui::Key::Escape),
                        i.key_pressed(egui::Key::H),
                        i.key_pressed(egui::Key::I),
                    )
                });
                if f {
                    match self.selected.map(|s| self.objects_under(s)) {
                        Some(o) if !o.is_empty() => self.frame_objects(&o),
                        _ => self.fit_all(),
                    }
                }
                if esc {
                    if self.isolated.is_some() {
                        self.isolated = None;
                        self.visibility_dirty = true;
                    } else {
                        self.select(None, false);
                    }
                }
                if let Some(sel) = self.selected {
                    if h {
                        let o = self.objects_under(sel);
                        self.hide(o);
                    }
                    if i {
                        let o = self.objects_under(sel);
                        self.isolate(o);
                    }
                }
            }
        }

        self.update_visibility();

        let params = FrameParams {
            mvp: self.camera.view_proj(aspect),
            eye: self.camera.eye(),
            view_dir: self.camera.forward(),
            ortho: self.camera.ortho,
            selected: self.selected_object().unwrap_or(u32::MAX),
            background: [0.925, 0.933, 0.945],
        };
        let renderer = self.renderer.clone();
        let callback = egui::PaintCallback {
            rect,
            callback: Arc::new(eframe::egui_glow::CallbackFn::new(move |info, painter| {
                let vp = info.viewport_in_pixels();
                if let Some(r) = renderer.lock().unwrap().as_ref() {
                    r.paint(painter.gl(), &params, [vp.left_px, vp.from_bottom_px, vp.width_px, vp.height_px]);
                }
            })),
        };
        ui.painter().add(callback);

        if self.scene.is_none() {
            let msg = if self.loading.is_some() { self.status.clone() } else { "Glisser un fichier .ifc ici\nou Ctrl+O".to_string() };
            ui.painter().text(rect.center(), egui::Align2::CENTER_CENTER, msg, egui::FontId::proportional(20.0), Color32::from_gray(110));
        } else if self.loading.is_some() {
            ui.painter().text(
                rect.left_top() + egui::vec2(12.0, 12.0),
                egui::Align2::LEFT_TOP,
                &self.status,
                egui::FontId::proportional(15.0),
                Color32::from_gray(60),
            );
        }
    }
}

impl eframe::App for ViewerApp {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        self.poll_loading();

        let dropped: Vec<PathBuf> = ctx.input(|i| i.raw.dropped_files.iter().map(|f| f.path().to_path_buf()).collect());
        if let Some(p) = dropped.into_iter().next() {
            self.open(p, &ctx);
        }
        if ctx.input_mut(|i| i.consume_key(egui::Modifiers::COMMAND, egui::Key::O)) {
            self.open_dialog(&ctx);
        }

        egui::Panel::top("top").show(ui, |ui| self.top_bar(ui));
        egui::Panel::bottom("status").show(ui, |ui| self.status_bar(ui));
        egui::Panel::left("left").resizable(true).default_size(320.0).show(ui, |ui| self.left_panel(ui));
        egui::Panel::right("right").resizable(true).default_size(340.0).show(ui, |ui| self.right_panel(ui));
        egui::CentralPanel::no_frame().show(ui, |ui| self.viewport(ui));
        self.unsupported_window(&ctx);
    }

    fn on_exit(&mut self, gl: Option<&glow::Context>) {
        if let (Some(gl), Some(r)) = (gl, self.renderer.lock().unwrap().as_ref()) {
            r.destroy(gl);
        }
    }
}
