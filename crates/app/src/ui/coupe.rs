//! Plan de coupe, dessin et export de la vue, boîte de sélection 3D, plage de vue.
use super::*;

impl ViewerApp {
    pub(super) fn niveaux(&self) -> Vec<(Id, String, f32)> {
        self.liste_niveaux.clone()
    }

    /// Plan de la coupe horizontale du mode dessin (n, d ; repère de la scène, partie
    /// gardée sous la cote de coupe), None hors mode dessin ou sans coupe.
    pub(super) fn plan_coupe(&self) -> Option<Vec4> {
        if !self.coupe {
            return None;
        }
        let s = self.scene.as_ref()?;
        // coupe native : plan vertical de la vue ouverte, ramené au repère de la scène
        if let Some(p) = self.vue_active.and_then(|i| s.navigateur.as_ref()?.vues.get(i)?.plan_coupe) {
            let n = p.truncate();
            return Some(Vec4::new(n.x as f32, n.y as f32, n.z as f32, (p.w + n.dot(s.origin)) as f32));
        }
        let base = self.coupe_niveau.and_then(|n| self.liste_niveaux.iter().find(|x| x.0 == n)).map_or(s.min.z, |x| x.2);
        Some(Vec4::new(0.0, 0.0, -1.0, base + self.coupe_hauteur))
    }

    /// Poché des parties coupées : celui de la plage de vue en Graphique, la couleur des
    /// matériaux en Réaliste.
    pub(super) fn poche_style(&self) -> Poche {
        if self.style == StyleVisuel::Realiste { Poche::Materiau } else { self.poche }
    }

    /// Trait atténué d'un autre plan, projeté derrière la coupe courante.
    pub(super) fn primitives_fond_plan(&self)->Vec<crate::noyau::trace2d::Primitive> {
        let (Some(s), Some((projet, _)), Some(id), Some(plan)) = (&self.scene, &self.natif, self.vue_native_id(), self.plan_coupe()) else { return Vec::new() };
        let Some(vue) = projet.vues.iter().find(|v| v.id == id) else { return Vec::new() };
        crate::modele::documentation::fond_plan(projet, s, vue, plan, self.echelle)
    }

    /// Éléments du niveau courant entièrement au-dessus de la coupe et sous la limite haute,
    /// en pointillés.
    pub(super) fn primitives_au_dessus_plan(&self)->Vec<crate::noyau::trace2d::Primitive> {
        let Some(plan) = self.plan_coupe() else { return Vec::new(); };
        crate::modele::documentation::pointilles_plage_haute(self.aretes_au_dessus_plan(), self.echelle, plan.w as f64)
    }

    /// Objet de la plage haute dont une arête passe à moins de 6 px du clic (vue en plan).
    pub(super) fn objet_plage_haute(&self,rect:egui::Rect,q:egui::Pos2)->Option<u32> {
        if !self.vue_en_plan() {return None;}
        let s=self.scene.as_ref()?;
        let vp=self.camera.view_proj(rect.width()/rect.height().max(1.0));
        let ecran=|p:glam::DVec3|{let c=vp*p.as_vec3().extend(1.0);(c.w>0.0).then(||egui::pos2(rect.left()+(c.x/c.w+1.0)*0.5*rect.width(),rect.top()+(1.0-c.y/c.w)*0.5*rect.height()))};
        let mut meilleur:Option<(f32,u32)>=None;
        for p in self.aretes_au_dessus_plan() {
            let (Some(e),crate::noyau::trace2d::Forme::Segments(segments))=(p.element,&p.forme) else{continue};
            let Some(&obj)=s.object_of.get(&e) else{continue};
            for seg in segments {
                let (Some(a),Some(b))=(ecran(seg[0]),ecran(seg[1])) else{continue};
                let d=b-a;let t=((q-a).dot(d)/d.length_sq().max(1e-6)).clamp(0.0,1.0);
                let distance=(a+d*t).distance(q);
                if distance<=6.0 && meilleur.is_none_or(|m|distance<m.0) {meilleur=Some((distance,obj));}
            }
        }
        meilleur.map(|m|m.1)
    }

    /// Arêtes vues des éléments de la plage haute, sans pointillés.
    pub(super) fn aretes_au_dessus_plan(&self)->Vec<crate::noyau::trace2d::Primitive> {
        let (Some(s),Some((visible,limite)))=(&self.scene,self.masque_plage_haute()) else{return Vec::new()};
        let f=self.camera.forward().as_dvec3().normalize();
        let droite=f.cross(self.camera.up().as_dvec3()).normalize();
        let haut=droite.cross(f);
        dessin::vu(s,&visible,droite,haut,-f,Some(Vec4::new(0.,0.,-1.,limite)),1024)
    }

    /// Plage haute de la vue en plan native ouverte : objets du niveau entièrement au-dessus
    /// de la coupe et sous la limite haute, et altitude de cette limite (repère de la scène).
    pub(super) fn masque_plage_haute(&self)->Option<(Vec<bool>,f32)> {
        let (Some(s),Some((p,_)),Some(id),Some(plan))=(&self.scene,&self.natif,self.vue_native_id(),self.plan_coupe()) else{return None};
        let v=p.vues.iter().find(|v|v.id==id)?;
        let crate::modele::GenreVue::Plan{niveau,..}=&v.genre else{return None};
        let coupe = plan.w;
        let limite = crate::modele::documentation::limite_haute_plan(p, s, v, coupe)?;
        let niveau_id=scene::id_natif("niveau",niveau);
        let exclus = self.ids_masques_contextes();
        // éléments masqués dans la vue : identifiants de scène calculés une fois
        let masques: HashSet<Id> = if v.elements_masques.is_empty() { HashSet::new() } else {
            super::bim::elements_natifs(p).filter(|(_, cle)| v.elements_masques.contains(*cle)).map(|(genre, cle)| scene::id_natif(genre, cle)).collect()
        };
        let visible=s.objects.iter().enumerate().map(|(i,o)| {
            let masque_vue=masques.contains(&o.id) || s.model.parent.get(&o.id).is_some_and(|id|masques.contains(id));
            o.storey==Some(niveau_id) && !exclus.contains(&o.id) && o.min.z>coupe+1e-5 && o.min.z<limite
                && !masque_vue && !self.hidden_classes.contains(&*o.class)
                && !self.hidden_classes.contains(&*self.cle_objet[i])
                && !self.hidden_objects.contains(&(i as u32))
                && !self.hidden_storeys.contains(&niveau_id)
                && self.isolated.as_ref().is_none_or(|iso|iso.contains(&(i as u32)))
        }).collect();
        Some((visible,limite))
    }

    /// Recalcule le tracé de la coupe (poché, hachures, contours) s'il le faut.
    pub(super) fn update_coupe(&mut self) {
        if !self.coupe_dirty {
            return;
        }
        self.coupe_dirty = false;
        let mut t_etape = std::time::Instant::now();
        // élévation : vue depuis sa ligne, rien n'est coupé (pas de poché)
        let elevation = self.vue_active.and_then(|i| self.scene.as_ref()?.navigateur.as_ref()?.vues.get(i)).is_some_and(|v| v.genre == "Élévation" && v.plan_coupe.is_some());
        let (mut tris, mut traits) = match (&self.scene, self.plan_coupe().filter(|_| !elevation)) {
            (Some(s), Some(p)) => {
                let prims = dessin::coupe(s, &self.visible, p, self.echelle, self.poche_style());
                dessin::sommets(s, &prims, p.truncate().as_dvec3().normalize(), self.echelle, self.traits_fins)
            }
            _ => Default::default(),
        };
        crate::noyau::chrono::chrono_lent("coupe : poché et contours", &mut t_etape);
        if let Some(s)=&self.scene {
            let mut prims=self.primitives_fond_plan();
            prims.extend(self.primitives_au_dessus_plan());
            let (t,l)=dessin::sommets(s,&prims,glam::DVec3::Z,self.echelle,true);
            tris.extend(t);traits.extend(l);
        }
        crate::noyau::chrono::chrono_lent("coupe : fond de plan et éléments au-dessus", &mut t_etape);
        if let (Some(s), Some((a, b))) = (&self.scene, self.boite_appliquee()) {
            let poche = (self.style == StyleVisuel::Graphique).then_some(self.poche);
            let (t, l) = dessin::sommets_boite(s, &self.visible, a, b, self.echelle, poche, self.style == StyleVisuel::Realiste, self.traits_fins);
            tris.extend(t);
            traits.extend(l);
        }
        if let Some(s) = &self.scene {
            let prims=self.annotations_natives();
            let (t,l)=dessin::sommets(s,&prims,-self.camera.forward().as_dvec3(),self.echelle,self.traits_fins);
            tris.extend(t); traits.extend(l);
        }
        crate::noyau::chrono::chrono_lent("coupe : boîte et annotations", &mut t_etape);
        if let Some(r) = self.renderer.as_mut() {
            r.upload_coupe(&tris, &traits);
        }
        crate::noyau::chrono::chrono_lent("coupe : envoi au GPU", &mut t_etape);
    }

    /// Dessin vectoriel de la vue (orthographique) : coupe, arêtes vues sans les parties
    /// cachées, éléments 2D de la vue Revit ouverte ; repère de la scène, axes de la caméra.
    pub(super) fn dessin_vue(&self) -> Result<trace2d::Dessin, String> {
        let s = self.scene.as_ref().ok_or("aucun fichier ouvert")?;
        if !self.camera.ortho {
            return Err("vue orthographique requise (bouton Ortho)".into());
        }
        let vue = self.vue_active.and_then(|i| s.navigateur.as_ref()?.vues.get(i));
        if vue.is_some_and(|v| v.genre == "Feuille") {
            return Err("export d'une feuille non pris en charge".into());
        }
        let f = self.camera.forward().as_dvec3().normalize();
        let droite = f.cross(self.camera.up().as_dvec3()).normalize();
        let haut = droite.cross(f);
        let mut primitives = self.primitives_fond_plan();
        primitives.extend(self.primitives_au_dessus_plan());
        let coupe = self.plan_coupe();
        if let Some(p) = coupe {
            primitives.extend(dessin::coupe(s, &self.visible, p, self.echelle, self.poche_style()));
        }
        primitives.extend(dessin::vu(s, &self.visible, droite, haut, -f, coupe, 4096));
        primitives = dessin::cadrer_primitives(primitives, &self.cadrage_vue(), -f);
        // éléments 2D de la vue Revit ouverte, s'ils sont affichés (repère du fichier →
        // repère de la scène)
        if let Some(v) = vue {
            for p in v.primitives.iter().filter(|p| p.element.and_then(|e| s.object_of.get(&e)).is_some_and(|&o| self.visible[o as usize])) {
                primitives.push(p.translater(-s.origin));
            }
        }
        primitives.extend(self.annotations_natives());
        if let Some(vue) = self.vue_native_id().and_then(|id| self.natif.as_ref()?.0.vues.iter().find(|v| v.id == id)) && !vue.contour_cadrage.is_empty() {
            primitives = crate::modele::elements::cadrage::decouper(primitives.into_iter().map(|p| p.translater(s.origin)).collect(), &vue.contour_cadrage)?
                .into_iter().map(|p| p.translater(-s.origin)).collect();
        }
        Ok(trace2d::Dessin { droite, haut, echelle: self.echelle, primitives })
    }

    /// Export SVG du dessin de la vue ; nombre de primitives écrites.
    pub(super) fn exporter_svg(&self, chemin: &std::path::Path) -> Result<usize, String> {
        let d = self.dessin_vue()?;
        let s = self.scene.as_ref().ok_or("aucun fichier ouvert")?;
        let vue = self.vue_active.and_then(|i| s.navigateur.as_ref()?.vues.get(i)).map(|v| format!(" — {} : {}", v.genre, v.nom)).unwrap_or_default();
        let titre = format!("{}{vue} — 1:{}", s.file_name, d.echelle);
        std::fs::write(chemin, d.svg(&titre)).map_err(|e| format!("écriture impossible : {e}"))?;
        Ok(d.primitives.len())
    }

    /// Cadrage de la coupe ou de l'élévation native ouverte (profondeur, côtés), repère
    /// de la scène.
    pub(super) fn cadrage_vue(&self) -> Vec<Vec4> {
        let Some(s) = &self.scene else { return Vec::new() };
        let Some(v) = self.vue_active.and_then(|i| s.navigateur.as_ref()?.vues.get(i)) else { return Vec::new() };
        v.cadrage.iter().map(|p| {
            let n = p.truncate();
            Vec4::new(n.x as f32, n.y as f32, n.z as f32, (p.w + n.dot(s.origin)) as f32)
        }).collect()
    }

    pub(super) fn contour_cadrage_vue(&self) -> Vec<glam::Vec2> {
        if self.ruban_edition.outil() == Some(crate::ui::ruban::Outil::Cadrer) { return Vec::new(); }
        let Some(id) = self.vue_native_id() else { return Vec::new() };
        let (Some((p, _)), Some(s)) = (&self.natif, &self.scene) else { return Vec::new() };
        p.vues.iter().find(|v| v.id == id).map(|v| v.contour_cadrage.iter().map(|p| glam::Vec2::new((p[0] * 0.001 - s.origin.x) as f32, (p[1] * 0.001 - s.origin.y) as f32)).collect()).unwrap_or_default()
    }

    pub(super) fn point_dans_cadrage(&self, rect: egui::Rect, q: egui::Pos2) -> bool {
        let Some(id) = self.vue_native_id() else { return true };
        let Some((p, _)) = &self.natif else { return true };
        let Some(vue) = p.vues.iter().find(|v| v.id == id) else { return true };
        if vue.contour_cadrage.is_empty() || self.ruban_edition.outil() == Some(crate::ui::ruban::Outil::Cadrer) { return true; }
        let z = self.altitude_vue_native();
        let points: Option<Vec<[f64; 2]>> = vue.contour_cadrage.iter().map(|p| self.ecran_plan(rect, *p, z).map(|p| [p.x as f64, p.y as f64])).collect();
        points.is_none_or(|points| crate::modele::volumes::geometrie::dedans([q.x as f64, q.y as f64], &points))
    }

    /// Zone de coupe appliquée au rendu et à la sélection : active, vue 3D.
    pub(super) fn boite_appliquee(&self) -> Option<(Vec3, Vec3)> {
        if self.boite_active && self.vue_3d() { self.boite } else { None }
    }

    /// Active la zone de coupe ; la première fois, elle entoure tout le modèle.
    pub(super) fn activer_boite(&mut self, active: bool) {
        self.boite_active = active;
        self.coupe_dirty = true;
        if active
            && self.boite.is_none()
            && let Some(s) = &self.scene
        {
            let marge = ((s.max - s.min).length() * 0.02).max(0.1);
            self.boite = Some((s.min - Vec3::splat(marge), s.max + Vec3::splat(marge)));
        }
    }

    /// Zone de coupe dans les propriétés : bornes en coordonnées du fichier (m).
    pub(super) fn proprietes_boite(&mut self, ui: &mut egui::Ui) {
        let (Some((mut a, mut b)), Some(s)) = (self.boite, &self.scene) else { return };
        // coordonnées du fichier en f64 : origine éventuellement lointaine (géoréférencement)
        let o = s.origin;
        egui::CollapsingHeader::new(RichText::new("Zone de coupe").strong()).id_salt("zone_de_coupe").default_open(true).show(ui, |ui| {
            egui::Grid::new("grille_zone_de_coupe").num_columns(3).spacing([10.0, 4.0]).show(ui, |ui| {
                ui.label("");
                ui.label("Min");
                ui.label("Max");
                ui.end_row();
                for (k, nom) in ["X", "Y", "Z"].iter().enumerate() {
                    ui.label(*nom);
                    let (mut lo, mut hi) = (a[k] as f64 + o[k], b[k] as f64 + o[k]);
                    ui.add(egui::DragValue::new(&mut lo).speed(0.05).max_decimals(3).dimensions_metres());
                    ui.add(egui::DragValue::new(&mut hi).speed(0.05).max_decimals(3).dimensions_metres());
                    a[k] = (lo - o[k]) as f32;
                    b[k] = (hi - o[k]) as f32;
                    ui.end_row();
                }
            });
            if ui.button("Ajuster au modèle").clicked() {
                self.boite = None;
                self.activer_boite(true);
                (a, b) = self.boite.unwrap_or((a, b));
            }
        });
        self.regler_boite(a, b);
    }

    /// Nouvelles bornes de la zone de coupe, épaisseur minimale gardée.
    pub(super) fn regler_boite(&mut self, a: Vec3, b: Vec3) {
        let ep = 0.05;
        let b = b.max(a + Vec3::splat(ep));
        self.boite = Some((a, b));
        self.coupe_dirty = true;
    }

    /// Contour de la zone de coupe sur la vue et ses poignées : une au centre de chaque
    /// face vue par la tranche ou de biais (en plan : les quatre côtés ; en élévation :
    /// dessus, dessous et côtés). Glisser une poignée déplace sa face le long de sa
    /// normale.
    pub(super) fn poignees_boite(&mut self, ui: &mut egui::Ui, rect: egui::Rect) {
        let Some((a, b)) = self.boite else { return };
        if !self.boite_active || !self.vue_3d() {
            return;
        }
        let vp = self.camera.view_proj(rect.width() / rect.height().max(1.0));
        let ecran = |p: Vec3| -> Option<egui::Pos2> {
            let c = vp * p.extend(1.0);
            (c.w > 1e-4).then(|| egui::pos2(rect.left() + (c.x / c.w + 1.0) * 0.5 * rect.width(), rect.top() + (1.0 - c.y / c.w) * 0.5 * rect.height()))
        };
        let coin = |i: usize| Vec3::new(if i & 1 == 0 { a.x } else { b.x }, if i & 2 == 0 { a.y } else { b.y }, if i & 4 == 0 { a.z } else { b.z });
        let couleur = Color32::from_rgb(230, 120, 20);
        let painter = ui.painter_at(rect);
        for i in 0..8 {
            for bit in [1, 2, 4] {
                if i & bit == 0
                    && let (Some(p), Some(q)) = (ecran(coin(i)), ecran(coin(i | bit)))
                {
                    painter.line_segment([p, q], egui::Stroke::new(1.5, couleur));
                }
            }
        }
        let f = self.camera.forward();
        let centre = (a + b) * 0.5;
        let (mut na, mut nb) = (a, b);
        for k in 0..3 {
            for haut in [false, true] {
                let mut n = Vec3::ZERO;
                n[k] = if haut { 1.0 } else { -1.0 };
                if n.dot(f).abs() > 0.9 {
                    continue; // face vue de face : pas de sens à la glisser à l'écran
                }
                let mut c = centre;
                c[k] = if haut { b[k] } else { a[k] };
                let (Some(p), Some(q)) = (ecran(c), ecran(c + n)) else { continue };
                if !rect.contains(p) {
                    continue;
                }
                let r = egui::Rect::from_center_size(p, egui::vec2(14.0, 14.0));
                let resp = ui.interact(r, ui.make_persistent_id(("poignee_boite", k, haut)), Sense::drag());
                let actif = resp.hovered() || resp.dragged();
                painter.circle(p, if actif { 7.0 } else { 5.5 }, if actif { couleur } else { Color32::WHITE }, egui::Stroke::new(1.5, couleur));
                if resp.dragged() {
                    // pixels par mètre le long de la normale, à l'écran
                    let s = q - p;
                    let d = resp.drag_delta().dot(s) / s.length_sq().max(1e-6);
                    if haut {
                        nb[k] += d;
                    } else {
                        na[k] += d;
                    }
                }
                if actif {
                    resp.on_hover_cursor(egui::CursorIcon::Grab);
                }
            }
        }
        if (na, nb) != (a, b) {
            self.regler_boite(na.min(nb - Vec3::splat(0.05)), nb);
        }
    }

    /// Plage de vue du dessin (style Lignes cachées), en tête des propriétés comme dans
    /// Revit : coupe horizontale, niveau, hauteur du plan de coupe, poché.
    pub(super) fn plage_de_vue(&mut self, ui: &mut egui::Ui) {
        let niveaux = self.niveaux();
        let nom = |n: Option<Id>| n.and_then(|n| niveaux.iter().find(|x| x.0 == n)).map_or("Bas du modèle".to_string(), |x| x.1.clone());
        let mut change = false;
        // coupe native : plan vertical de la vue, pas de niveau ni de hauteur
        let verticale = self.vue_active.and_then(|i| self.scene.as_ref()?.navigateur.as_ref()?.vues.get(i)?.plan_coupe).is_some();
        egui::CollapsingHeader::new(RichText::new("Plage de vue").strong()).id_salt("plage_de_vue").default_open(true).show(ui, |ui| {
            egui::Grid::new("grille_plage_de_vue").num_columns(2).spacing([10.0, 4.0]).show(ui, |ui| {
                ui.label("Plan de coupe");
                change |= ui.checkbox(&mut self.coupe, if verticale { "ligne de coupe" } else { "" }).on_hover_text("Parties coupées en poché et trait fort, parties au-dessus retirées").changed();
                ui.end_row();
                if !verticale {
                ui.add_enabled_ui(self.coupe, |ui| ui.label("Niveau"));
                ui.add_enabled_ui(self.coupe, |ui| {
                    egui::ComboBox::from_id_salt("coupe_niveau").selected_text(nom(self.coupe_niveau)).show_ui(ui, |ui| {
                        change |= ui.selectable_value(&mut self.coupe_niveau, None, "Bas du modèle").changed();
                        for (id, n, z) in &niveaux {
                            change |= ui.selectable_value(&mut self.coupe_niveau, Some(*id), format!("{n} ({z:+.2})")).changed();
                        }
                    });
                });
                ui.end_row();
                ui.add_enabled_ui(self.coupe, |ui| ui.label("Décalage du plan de coupe"));
                ui.add_enabled_ui(self.coupe, |ui| {
                    change |= ui.add(egui::DragValue::new(&mut self.coupe_hauteur).speed(0.01).range(-50.0..=200.0).dimensions_metres()).on_hover_text("Hauteur du plan de coupe au-dessus du niveau").changed();
                });
                ui.end_row();
                }
                ui.add_enabled_ui(self.coupe, |ui| ui.label("Poché"));
                ui.add_enabled_ui(self.coupe && !self.vue_gabaritee(), |ui| {
                    egui::ComboBox::from_id_salt("poche").selected_text(self.poche.libelle()).show_ui(ui, |ui| {
                        for p in [Poche::Hachure, Poche::Plein, Poche::Vide] {
                            change |= ui.selectable_value(&mut self.poche, p, p.libelle()).changed();
                        }
                    });
                });
                ui.end_row();
            });
        });
        if change {
            self.visibility_dirty = true;
            self.coupe_dirty = true;
            self.memoriser_vue();
        }
    }

    /// Export SVG du dessin de la vue, après choix du fichier.
    pub(super) fn exporter_svg_dialogue(&mut self) {
        {
            let nom = self.scene.as_ref().map(|s| s.file_name.rsplit_once('.').map_or(s.file_name.clone(), |x| x.0.to_string())).unwrap_or_default();
            if let Some(chemin) = rfd::FileDialog::new().add_filter("SVG", &["svg"]).set_file_name(format!("{nom}.svg")).save_file() {
                match self.exporter_svg(&chemin) {
                    Ok(n) => self.status = format!("{} : {n} primitives exportées", chemin.display()),
                    Err(e) => self.error = Some(e),
                }
            }
        }
    }

}
