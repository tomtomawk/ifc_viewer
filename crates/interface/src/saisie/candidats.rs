//! Candidats à la sélection sous le curseur, dans une vue du projet (cycle Tab de
//! `saisie/cycle.rs`). Pas au pixel près : un objet se vise aussi à `RAYON_OBJETS` pixels
//! (rayons lancés autour du curseur), un trait à `visee::TOLERANCE`. En plan, un élément
//! horizontal (dalle, terrain) passe après un mur ou un poteau proche : cliquer près d'un
//! mur au-dessus d'une dalle prend le mur, Tab donne la dalle.
use crate::*;

/// Distance (pixels) à laquelle un objet de la scène est encore visé.
pub(crate) const RAYON_OBJETS: f32 = 6.0;
/// Retard (pixels) d'un élément horizontal en plan : un mur à moins de cette distance passe avant.
const RETARD_HORIZONTAL: f32 = 7.0;

/// Ce qui est sélectionné : un objet de la scène, ou un élément natif sans objet
/// (annotation, pièce, niveau, repère de coupe, quadrillage, limite).
#[derive(Clone, Debug, PartialEq)]
pub(crate) enum Cible {
    Objet(Id),
    Natif(String),
}

/// Candidat du cycle : cible, nom affiché, traits à surligner (éléments 2D) et point
/// visé (objets).
#[derive(Clone, Debug)]
pub(crate) struct Candidat {
    pub cible: Cible,
    pub libelle: String,
    pub segments: Vec<[egui::Pos2; 2]>,
    pub point: Option<glam::DVec3>,
}

impl Application {
    /// Contexte du calcul des candidats : caméra, modèle, visibilité, vue.
    pub(crate) fn contexte_cycle(&self, rect: egui::Rect) -> u64 {
        let vp = self.camera.view_proj(rect.width() / rect.height().max(1.0));
        let mut v: Vec<u64> = vp.to_cols_array().iter().map(|x| x.to_bits() as u64).collect();
        v.extend([self.bim.revision, self.vue_active.map_or(u64::MAX, |x| x as u64), self.visible.iter().filter(|x| **x).count() as u64, u64::from(self.options.selection_faces_vues)]);
        v.extend([rect.min.x, rect.min.y, rect.max.x, rect.max.y].map(|x| x.to_bits() as u64));
        crate::saisie::cycle::empreinte(&v)
    }

    fn libelle_natif(&self, id: &str) -> String {
        let Some((p, _)) = &self.natif else { return String::new() };
        if let Some(a) = p.annotations.iter().find(|a| a.id == id) {
            use modele::documentation::AnnotationForme as F;
            return match a.forme {
                F::Texte { .. } => "Texte",
                F::Ligne { .. } | F::Polyligne { .. } | F::Arc { .. } => "Polyligne",
                F::Spline { .. } => "Spline",
                F::Cercle { .. } => "Cercle",
                F::Zone { .. } => "Zone remplie",
                F::Etiquette { .. } => "Étiquette",
                _ => "Cote",
            }.into();
        }
        p.niveaux.iter().find(|x| x.id == id).map(|x| format!("Niveau : {}", x.nom))
            .or_else(|| p.pieces.iter().find(|x| x.id == id).map(|x| format!("Pièce : {}", x.nom)))
            .or_else(|| p.vues.iter().find(|x| x.id == id).map(|x| x.nom.clone()))
            .or_else(|| p.quadrillages.iter().find(|x| x.id == id).map(|x| format!("Quadrillage {}", x.nom)))
            .or_else(|| p.limites_propriete.iter().find(|x| x.id == id).map(|x| format!("Limite : {}", x.nom)))
            .unwrap_or_default()
    }

    fn libelle_objet(&self, id: Id) -> String {
        let Some(s) = &self.scene else { return String::new() };
        // élément natif : sa sorte en français ; sinon la classe IFC
        let sorte = self.natif.as_ref().and_then(|(p, _)| crate::socle::document_bim::elements_natifs(p).find(|(g, c)| modele::identifiants::id_natif(g, c) == id).map(|(g, _)| g)).map(|g| match g {
            "mur" => "Mur", "assemblage" => "Mur composé", "dalle" => "Élément horizontal", "terrain" => "Terrain",
            "limite_propriete" => "Limite", "barre" => "Poteau ou poutre", "composant" => "Composant", "arbre" => "Arbre",
            "escalier" => "Escalier", "garde_corps" => "Garde-corps", "ouverture" => "Ouverture", "niveau" => "Niveau", _ => "Élément",
        }.to_string());
        let classe = sorte.unwrap_or_else(|| pretty_type(s.model.ty(id)).trim_start_matches("Ifc").to_string());
        let nom = s.model.name(id);
        if nom.is_empty() { classe } else { format!("{classe} : {nom}") }
    }

    /// Candidats sous le point écran `q`, du premier au dernier.
    pub(crate) fn candidats_selection(&self, rect: egui::Rect, q: egui::Pos2) -> Vec<Candidat> {
        if !self.point_dans_cadrage(rect, q) {
            return Vec::new();
        }
        // (rang, profondeur, candidat)
        let mut liste: Vec<(f32, f32, Candidat)> = Vec::new();
        let natif = |app: &Self, rang: f32, id: String, segments: Vec<[egui::Pos2; 2]>| (rang, 0.0, Candidat { libelle: app.libelle_natif(&id), cible: Cible::Natif(id), segments, point: None });
        if let Some(s) = &self.scene {
            // arête de la plage haute (au-dessus de la coupe) : prioritaire
            if let Some(obj) = self.objet_plage_haute(rect, q) {
                let id = s.objects[obj as usize].id;
                liste.push((-1.0, 0.0, Candidat { cible: Cible::Objet(self.cible_clic(id)), libelle: self.libelle_objet(self.cible_clic(id)), segments: Vec::new(), point: None }));
            }
            // objets : rayon au curseur, puis huit rayons autour
            let aspect = rect.width() / rect.height().max(1.0);
            let plan = self.vue_en_plan();
            let mut decalages = vec![egui::Vec2::ZERO];
            decalages.extend((0..8).map(|k| egui::Vec2::angled(k as f32 * std::f32::consts::FRAC_PI_4) * RAYON_OBJETS));
            let mut objets: std::collections::HashMap<u32, (f32, f32, glam::Vec3)> = Default::default();
            for d in decalages {
                let n = crate::socle::projection::ndc(rect, q + d);
                let (o, dir) = self.camera.ray(n.x, n.y, aspect);
                for (obj, t) in s.pick_tous(o, dir, &self.visible, self.plan_coupe(), self.boite_appliquee()) {
                    let classe = &*s.objects[obj as usize].class;
                    let horizontal = plan && matches!(classe, "IFCSLAB" | "IFCCOVERING" | "IFCROOF" | "IFCGEOGRAPHICELEMENT");
                    let rang = d.length() + if horizontal { RETARD_HORIZONTAL } else { 0.0 };
                    let e = objets.entry(obj).or_insert((f32::MAX, t, o + dir * t));
                    if rang < e.0 || (rang == e.0 && t < e.1) {
                        *e = (rang, t, o + dir * t);
                    }
                }
            }
            // option « Sélectionner par les faces vues » décochée : un objet n'est retenu que
            // coupé, ou près d'une de ses arêtes vues
            let par_faces = self.options.selection_faces_vues;
            for (obj, (rang, t, point)) in objets {
                if !par_faces && !self.objet_coupe(obj) && !self.arete_vue_proche(rect, q, obj) {
                    continue;
                }
                let id = s.objects[obj as usize].id;
                let point = Some(point.as_dvec3() + s.origin);
                // couche d'un mur composé : le mur composé d'abord, la couche ensuite (Tab)
                let cible = self.cible_clic(id);
                liste.push((rang, t, Candidat { cible: Cible::Objet(cible), libelle: self.libelle_objet(cible), segments: Vec::new(), point }));
                if cible != id {
                    liste.push((rang + 0.01, t, Candidat { cible: Cible::Objet(id), libelle: self.libelle_objet(id), segments: Vec::new(), point }));
                }
            }
        }
        // traits 2D : un peu avant un objet à la même distance (dessinés par-dessus)
        for (d, id, segments) in self.elements_2d_proches(rect, q) {
            liste.push(natif(self, d * 0.5, id, segments));
        }
        if let Some(id) = self.limite_propriete_proche(rect, q) {
            liste.push(natif(self, 1.0, id, Vec::new()));
        }
        if self.vue_en_plan() && let Some(id) = self.repere_coupe_proche(rect, q, self.altitude_vue_native()) {
            liste.push(natif(self, 1.0, id, Vec::new()));
        }
        liste.sort_by(|a, b| a.0.total_cmp(&b.0).then(a.1.total_cmp(&b.1)));
        let mut vus = Vec::new();
        liste.into_iter().filter(|(_, _, c)| !vus.contains(&c.cible) && { vus.push(c.cible.clone()); true }).map(|(_, _, c)| c).collect()
    }

    /// L'objet est coupé par le plan de coupe actif (sa boîte est de part et d'autre).
    pub(crate) fn objet_coupe(&self, obj: u32) -> bool {
        let (Some(s), Some(c)) = (&self.scene, self.plan_coupe()) else { return false };
        let o = &s.objects[obj as usize];
        let n = c.truncate();
        let cotes: Vec<f32> = (0..8).map(|k| {
            let p = glam::Vec3::new([o.min.x, o.max.x][k & 1], [o.min.y, o.max.y][(k >> 1) & 1], [o.min.z, o.max.z][k >> 2]);
            n.dot(p) + c.w
        }).collect();
        cotes.iter().any(|d| *d > 0.0) && cotes.iter().any(|d| *d < 0.0)
    }

    /// Une arête de l'objet (hors partie retirée par la coupe) passe à moins de
    /// `RAYON_OBJETS` pixels du point écran `q`.
    pub(crate) fn arete_vue_proche(&self, rect: egui::Rect, q: egui::Pos2, obj: u32) -> bool {
        let Some(s) = &self.scene else { return false };
        let o = &s.objects[obj as usize];
        let vp = self.camera.view_proj(rect.width() / rect.height().max(1.0));
        let coupe = self.plan_coupe();
        let ecran = |p: glam::Vec3| {
            let c = vp * p.extend(1.0);
            (c.w > 1e-6).then(|| egui::pos2(rect.left() + (c.x / c.w + 1.0) * 0.5 * rect.width(), rect.top() + (1.0 - c.y / c.w) * 0.5 * rect.height()))
        };
        s.primitives_montrees(o.aretes, 2, self.affichage()).any(|v| {
            let (mut a, mut b) = (glam::Vec3::from(v[0].pos), glam::Vec3::from(v[1].pos));
            if let Some(c) = coupe {
                let (sa, sb) = (c.truncate().dot(a) + c.w, c.truncate().dot(b) + c.w);
                if sa < 0.0 && sb < 0.0 { return false; }
                if sa < 0.0 { a = a + (b - a) * (sa / (sa - sb)); } else if sb < 0.0 { b = a + (b - a) * (sa / (sa - sb)); }
            }
            let (Some(a), Some(b)) = (ecran(a), ecran(b)) else { return false };
            let d = b - a;
            let t = if d.length_sq() > 0.0 { ((q - a).dot(d) / d.length_sq()).clamp(0.0, 1.0) } else { 0.0 };
            q.distance(a + d * t) <= RAYON_OBJETS
        })
    }

    /// Sélectionne la cible d'un clic (Ctrl : ajoute ou retire).
    pub(crate) fn selectionner_candidat(&mut self, c: &Candidat, ctrl: bool) {
        self.point_clique = c.point;
        match &c.cible {
            Cible::Natif(cle) => {
                let mut selection = if ctrl { self.selection_bim() } else { Vec::new() };
                if let Some(i) = selection.iter().position(|x| x == cle) { selection.remove(i); } else { selection.push(cle.clone()); }
                // élément avec un objet dans la scène (limite) : surligné comme les autres
                let objet = selection.iter().find_map(|k| self.scene.as_ref()?.objects.iter().find(|o| self.cle_native(o.id).as_deref() == Some(k)).map(|o| o.id));
                self.select(objet, false);
                self.bim.selection = selection;
            }
            Cible::Objet(id) => {
                let id = *id;
                if ctrl && self.natif.is_none() {
                    let mut ids = self.selection_multiple.clone();
                    if ids.is_empty() && let Some(sel) = self.selected { ids.push(sel); }
                    if let Some(pos) = ids.iter().position(|x| *x == id) { ids.remove(pos); } else { ids.push(id); }
                    self.select(ids.first().copied(), true);
                    self.selection_multiple = ids;
                } else if ctrl {
                    let mut selection = self.selection_bim();
                    if let Some(cle) = self.cle_native(id) {
                        if selection.contains(&cle) { selection.retain(|s| s != &cle); } else { selection.push(cle); }
                    }
                    self.select(if selection.is_empty() { None } else { Some(id) }, true);
                    self.bim.selection = selection;
                } else {
                    self.select(Some(id), true);
                }
            }
        }
    }
}
