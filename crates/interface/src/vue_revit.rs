//! Vues Revit, nomenclatures, fenêtre des éléments non pris en charge.
use super::*;

impl ViewerApp {
    /// Fiche d'un fichier Revit : vignette, version, informations projet, références.
    pub(super) fn revit_view(&mut self, ui: &mut egui::Ui) {
        if self.rvt_preview.is_none() {
            if let Some(img) = self.rvt.as_ref().and_then(|i| i.preview_png.as_deref()).and_then(decode_png) {
                self.rvt_preview = Some(ui.ctx().load_texture("rvt-preview", img, egui::TextureOptions::LINEAR));
            }
        }
        let Some(info) = &self.rvt else { return };
        let preview = self.rvt_preview.clone();
        let mut close = false;
        egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
            ui.set_max_width(980.0);
            ui.horizontal(|ui| {
                ui.heading(info.title.as_deref().unwrap_or(&info.file_name));
                ui.with_layout(egui::Layout::right_to_left(Align::Center), |ui| {
                    if ui.button("Fermer").clicked() {
                        close = true;
                    }
                });
            });
            let mut sub = format!("{} Revit {} · {} · {:.1} Mo", info.kind, info.format().unwrap_or("?"), info.file_name, info.file_size as f64 / 1e6);
            if let Some(c) = &info.category {
                sub += &format!(" · catégorie {c}");
            }
            ui.label(RichText::new(sub).color(Color32::from_gray(90)));
            ui.add_space(6.0);
            egui::Frame::new()
                .fill(Color32::from_rgb(255, 246, 214))
                .stroke(egui::Stroke::new(1.0, Color32::from_rgb(222, 196, 120)))
                .inner_margin(8.0)
                .corner_radius(4.0)
                .show(ui, |ui| match &self.rvt_geom {
                    Ok(summary) => {
                        ui.label(format!(
                            "Vue 3D : {summary}, avec la géométrie calculée par Revit (murs, sols, toits, familles, \
                             escaliers…), vues et éléments 2D, paramètres. Le contenu des nomenclatures et les valeurs \
                             calculées (surfaces, volumes) ne sont pas encore lus."
                        ));
                    }
                    Err(e) => {
                        ui.label(format!(
                            "Métadonnées seules : pas de murs affichables ({e}). Pour la 3D, exporter le modèle en IFC \
                             depuis Revit (Fichier › Exporter › IFC), puis ouvrir le fichier .ifc."
                        ));
                    }
                });
            ui.add_space(8.0);

            ui.horizontal_top(|ui| {
                if let Some(tex) = &preview {
                    let size = tex.size_vec2();
                    let scale = (256.0 / size.x.max(size.y)).clamp(1.0, 4.0);
                    egui::Frame::new().stroke(egui::Stroke::new(1.0, Color32::from_gray(200))).show(ui, |ui| {
                        ui.image((tex.id(), size * scale));
                    });
                    ui.add_space(12.0);
                }
                egui::Grid::new("rvt-main").num_columns(2).striped(true).show(ui, |ui| {
                    for key in ["Format", "Build", "Worksharing", "Central Model Path", "Last Save Path", "Locale when saved", "Unique Document GUID"] {
                        if let Some(v) = info.basic_value(key).filter(|v| !v.is_empty()) {
                            ui.label(RichText::new(rvt::basic_label(key)).strong());
                            ui.add(egui::Label::new(v).wrap());
                            ui.end_row();
                        }
                    }
                    if let Some(u) = &info.updated {
                        ui.label(RichText::new("Mise à jour").strong());
                        ui.label(u);
                        ui.end_row();
                    }
                });
            });
            ui.add_space(8.0);

            let param_grid = |ui: &mut egui::Ui, id: (&str, usize), params: &[rvt::Param]| {
                egui::Grid::new(id).num_columns(2).striped(true).min_col_width(220.0).show(ui, |ui| {
                    for p in params {
                        let hint = [p.kind.as_str(), p.origin.as_str()].iter().filter(|s| !s.is_empty()).copied().collect::<Vec<_>>().join(" · ");
                        let r = ui.label(&p.name);
                        if !hint.is_empty() {
                            r.on_hover_text(hint);
                        }
                        let v = match &p.units {
                            Some(u) => format!("{} {u}", p.value),
                            None => p.value.clone(),
                        };
                        ui.add(egui::Label::new(v).wrap());
                        ui.end_row();
                    }
                });
            };

            if !info.groups.is_empty() {
                let title = if info.kind == "Famille" { "Paramètres de famille" } else { "Informations sur le projet" };
                egui::CollapsingHeader::new(RichText::new(title).strong()).default_open(true).show(ui, |ui| {
                    for (i, g) in info.groups.iter().enumerate() {
                        ui.label(RichText::new(&g.title).color(Color32::from_gray(90)));
                        param_grid(ui, ("rvt-group", i), &g.params);
                        ui.add_space(4.0);
                    }
                });
            }
            if !info.family_types.is_empty() {
                egui::CollapsingHeader::new(RichText::new(format!("Types de famille ({})", info.family_types.len())).strong())
                    .default_open(true)
                    .show(ui, |ui| {
                        for (i, t) in info.family_types.iter().enumerate() {
                            egui::CollapsingHeader::new(&t.title).id_salt(("rvt-type", i)).default_open(info.family_types.len() <= 3).show(ui, |ui| {
                                param_grid(ui, ("rvt-type-grid", i), &t.params);
                            });
                        }
                    });
            }
            if !info.external_refs.is_empty() {
                egui::CollapsingHeader::new(RichText::new(format!("Références externes ({})", info.external_refs.len())).strong())
                    .default_open(true)
                    .show(ui, |ui| {
                        egui::Grid::new("rvt-refs").num_columns(3).striped(true).show(ui, |ui| {
                            for r in &info.external_refs {
                                ui.label(&r.kind);
                                let path = if r.path.is_empty() { "—" } else { r.path.as_str() };
                                let lbl = ui.add(egui::Label::new(path).wrap());
                                if !r.absolute_path.is_empty() {
                                    lbl.on_hover_text(&r.absolute_path);
                                }
                                let state = load_state_fr(&r.load_state);
                                let color = if r.load_state == "Loaded" { Color32::from_rgb(40, 130, 60) } else { Color32::from_rgb(180, 90, 30) };
                                ui.colored_label(color, state);
                                ui.end_row();
                            }
                        });
                    });
            }
            egui::CollapsingHeader::new(RichText::new("Informations de fichier (BasicFileInfo)").strong()).show(ui, |ui| {
                egui::Grid::new("rvt-basic").num_columns(2).striped(true).show(ui, |ui| {
                    for (k, v) in &info.basic {
                        ui.label(rvt::basic_label(k)).on_hover_text(k);
                        ui.add(egui::Label::new(v).wrap());
                        ui.end_row();
                    }
                });
            });
            egui::CollapsingHeader::new(RichText::new(format!("Structure du conteneur ({} flux)", info.streams.iter().filter(|s| !s.1).count())).strong())
                .show(ui, |ui| {
                    egui::Grid::new("rvt-streams").num_columns(2).striped(true).show(ui, |ui| {
                        for (path, is_dir, size) in &info.streams {
                            ui.label(RichText::new(path).monospace());
                            ui.label(if *is_dir { "dossier".to_string() } else { format!("{size} o") });
                            ui.end_row();
                        }
                    });
                });
            for w in &info.warnings {
                ui.colored_label(Color32::from_rgb(180, 90, 30), format!("Avertissement : {w}"));
            }
        });
        if close {
            if self.rvt_geom.is_ok() {
                // Retour à la vue 3D des murs.
                self.rvt_sheet = false;
            } else {
                self.fermer();
            }
        }
    }

    /// Tableau d'une nomenclature Revit, recomposé depuis les éléments lus ; les colonnes
    /// que Revit calcule (formules, pièces d'une porte…) sont signalées.
    pub(super) fn vue_nomenclature(&mut self, ui: &mut egui::Ui) {
        let Some(i) = self.nomenclature_ouverte else { return };
        let Some(v) = self.scene.as_ref().and_then(|s| s.navigateur.as_ref()).and_then(|n| n.vues.get(i)) else { return };
        let Some((titres, lignes, nc, tris)) = &v.tableau else { return };
        ui.vertical(|ui| {
            ui.heading(&v.nom);
            if !nc.is_empty() {
                let noms: Vec<&str> = nc.iter().filter_map(|&k| titres.get(k).map(String::as_str)).collect();
                ui.label(RichText::new(format!("Non calculées (calcul de Revit) : {}", noms.join(", "))).weak());
            }
            ui.label(RichText::new(format!("{} lignes{}", lignes.len(), if tris.is_empty() { String::new() } else { format!(" ; tri : {tris}") })).weak());
            egui::ScrollArea::both().auto_shrink([false, false]).show(ui, |ui| {
                egui::Grid::new("table_nomenclature").striped(true).show(ui, |ui| {
                    for t in titres {
                        ui.label(RichText::new(t).strong());
                    }
                    ui.end_row();
                    for l in lignes {
                        for c in l {
                            ui.label(c);
                        }
                        ui.end_row();
                    }
                });
            });
        });
    }

    pub(super) fn unsupported_window(&mut self, ctx: &egui::Context) {
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

}
