//! Navigateur de projet, matériaux, arborescence IFC.
use super::*;

impl ViewerApp {
    /// Matériaux : pastille de couleur, nom, nombre d'éléments ; clic : isoler et cadrer
    /// les éléments qui portent le matériau.
    pub(super) fn materiaux_ui(&mut self, ui: &mut egui::Ui) {
        let q = self.filter.trim().to_lowercase();
        let mut isoler: Option<Vec<Id>> = None;
        let natif = self.natif.is_some();
        let mut editer: Option<String> = None;
        let mut n = 0;
        for m in self.liste_materiaux.iter().filter(|m| q.is_empty() || m.nom.to_lowercase().contains(&q)) {
            n += 1;
            ui.horizontal(|ui| {
                let (r, _) = ui.allocate_exact_size(egui::vec2(14.0, 14.0), Sense::hover());
                let [cr, cg, cb] = m.couleur;
                ui.painter().rect_filled(r, 2.0, Color32::from_rgb(cr, cg, cb));
                ui.painter().rect_stroke(r, 2.0, egui::Stroke::new(1.0, Color32::from_gray(120)), egui::StrokeKind::Inside);
                let texte = format!("{} ({})", m.nom, m.elements.len());
                if natif && ui.small_button("…").on_hover_text("Graphismes du matériau : surface et coupe").clicked() {
                    editer = Some(m.nom.clone());
                }
                let resp = ui.selectable_label(false, if m.elements.is_empty() { RichText::new(texte).weak() } else { RichText::new(texte) });
                let resp = resp.on_hover_text(format!("Transparence {:.0} %\nClic : isoler les éléments qui le portent", m.transparence * 100.0));
                if resp.clicked() && !m.elements.is_empty() {
                    isoler = Some(m.elements.clone());
                }
            });
        }
        if n == 0 {
            ui.label(RichText::new("Aucun matériau").weak());
        }
        if editer.is_some() {
            self.materiau_edite = editer;
        }
        if let (Some(ids), Some(s)) = (isoler, &self.scene) {
            let objs: Vec<u32> = ids.iter().filter_map(|id| s.object_of.get(id).copied()).collect();
            self.isolate(objs.clone());
            self.frame_objects(&objs);
        }
    }

    /// Navigateur de projet de Revit : vues par genre et type, nomenclatures, feuilles et
    /// vues qu'elles portent, familles par catégorie, groupes, liens. Clic : propriétés ;
    /// double-clic : ouvrir la vue (ou isoler les instances d'un type).
    pub(super) fn navigateur_ui(&mut self, ui: &mut egui::Ui, section: LeftTab) {
        // projet natif : jeux de feuilles et leurs feuilles, nomenclatures ; vues
        // glissables vers une feuille
        // jeux : (nom, feuilles : (id, titre, vues placées : (numéro de vue, nom, échelle)))
        type FeuilleNav = (String, String, Vec<(i64, String, f64)>, Vec<(String, String)>);
        let (jeux_natifs, nomenclatures_natives, glissables): (Vec<(String, Vec<FeuilleNav>)>, Vec<(String, String)>, std::collections::HashMap<i64, String>) = match &self.natif {
            Some((p, _)) => (
                crate::modele::documentation::jeux(p).into_iter().map(|j| {
                    let fs = p.feuilles.iter().filter(|f| f.jeu == j).map(|f| {
                        let vues = f.vues.iter().map(|v| (crate::rendu::scene::numero_vue(&v.vue), p.vues.iter().find(|x| x.id == v.vue).map_or("Vue absente".to_string(), |x| x.nom.clone()), v.echelle)).collect();
                        let tables = f.nomenclatures.iter().map(|n| (n.nomenclature.clone(), p.nomenclatures.iter().find(|x| x.id == n.nomenclature).map_or("Nomenclature absente".to_string(), |x| x.nom.clone()))).collect();
                        (f.id.clone(), format!("{} - {}", f.numero, f.nom), vues, tables)
                    }).collect();
                    (j, fs)
                }).collect(),
                p.nomenclatures.iter().map(|n| (n.id.clone(), n.nom.clone())).collect(),
                p.vues.iter().map(|v| (crate::rendu::scene::numero_vue(&v.id), v.id.clone())).collect(),
            ),
            None => (Vec::new(), Vec::new(), std::collections::HashMap::new()),
        };
        let mut ouvrir_jeu: Option<(String, Option<String>)> = None;
        let mut creer_jeu = false;
        let mut dupliquer_jeu: Option<String> = None;
        let mut ajouter_feuille_jeu: Option<String> = None;
        let mut exporter_jeu_nav: Option<String> = None;
        let mut exporter_feuille_nav: Option<(&str, String)> = None;
        let mut editer_cartouche_nav: Option<String> = None;
        let mut regler_jeu: Option<(String, Option<String>)> = None;
        // feuille glissée dans l'arborescence : (feuille, jeu, feuille devant laquelle)
        let mut deplacer: Option<(String, String, Option<String>)> = None;
        let mut ouvrir_nomenclature: Option<String> = None;
        let Some(s) = self.scene.as_ref() else { return };
        let Some(nav) = s.navigateur.as_ref() else { return };
        let q = self.filter.trim().to_lowercase();
        let garde = |t: &str| q.is_empty() || t.to_lowercase().contains(&q);
        let ouvert = !q.is_empty();
        let (selection, active) = (self.selected, self.vue_active);
        let mut action = None;
        let feuille = |ui: &mut egui::Ui, label: String, entite: Option<Id>, vue: Option<usize>, action: &mut Option<ActionNav>| {
            let actif = vue.is_some() && vue == active;
            let texte = if actif { RichText::new(label).strong() } else { RichText::new(label) };
            // vue du projet natif : glissable sur une feuille d'un jeu
            let glissable = vue.and_then(|i| nav.vues.get(i)).filter(|v| matches!(v.genre.as_str(), "Plan d'étage" | "Coupe" | "Élévation")).and_then(|v| glissables.get(&v.revit_id));
            let mut bouton = egui::Button::selectable(entite.is_some() && entite == selection, texte);
            if glissable.is_some() {
                bouton = bouton.sense(Sense::click_and_drag());
            }
            let r = ui.add(bouton);
            if let Some(id) = glissable {
                r.dnd_set_drag_payload(feuilles::VueGlissee(id.clone()));
            }
            let r = if glissable.is_some() { r.on_hover_text("Double-clic : ouvrir · glisser sur une feuille d'un jeu pour la placer") } else { r };
            if let (Some(i), true) = (vue, glissable.is_some()) {
                r.context_menu(|ui| {
                    if ui.button("Supprimer la vue").clicked() {
                        *action = Some(ActionNav::SupprimerVue(i));
                        ui.close();
                    }
                });
            }
            if r.double_clicked() {
                *action = vue.map(ActionNav::Activer).or_else(|| entite.map(ActionNav::IsolerType));
            } else if r.clicked() {
                if let Some(e) = entite {
                    *action = Some(ActionNav::Selection(e));
                }
            }
        };
        let titre_vue = |v: &crate::rendu::scene::NavVue| if v.nom.is_empty() { format!("{} [{}]", v.genre, v.revit_id) } else { v.nom.clone() };
        // section : repliable sous « Tout », seule et dépliée sous son onglet
        let rubrique_nav = |ui: &mut egui::Ui, onglet: LeftTab, titre: String, ouverte: bool, contenu: &mut dyn FnMut(&mut egui::Ui)| {
            if section == LeftTab::Tout {
                egui::CollapsingHeader::new(RichText::new(titre).strong()).id_salt(("nav", onglet.nom())).default_open(ouverte).open(ouvert.then_some(true)).show(ui, |ui| contenu(ui));
            } else if section == onglet {
                contenu(ui);
            }
        };
        // Vues (tout) : rubriques dans l'ordre de Revit, puis type de vue
        let mut rubriques: BTreeMap<(u8, &str, String), Vec<usize>> = BTreeMap::new();
        for (i, v) in nav.vues.iter().enumerate() {
            if let Some((ordre, nom)) = rubrique(&v.genre) {
                if garde(&titre_vue(v)) {
                    rubriques.entry((ordre, nom, v.type_vue.clone())).or_default().push(i);
                }
            }
        }
        rubrique_nav(ui, LeftTab::Vues, "Vues (tout)".into(), true, &mut |ui| {
            for ((_, nom, type_vue), vues) in &rubriques {
                let mut vues = vues.clone();
                vues.sort_by_key(|&i| titre_vue(&nav.vues[i]).to_lowercase());
                // « Élévations (Élévation 1) » : type entre parenthèses s'il diffère du genre
                let genre = &nav.vues[vues[0]].genre;
                let titre = if type_vue.is_empty() || type_vue.eq_ignore_ascii_case(genre) { nom.to_string() } else { format!("{nom} ({type_vue})") };
                egui::CollapsingHeader::new(titre.clone()).id_salt(("nav-rub", titre)).default_open(true).open(ouvert.then_some(true)).show(ui, |ui| {
                    for i in vues {
                        feuille(ui, titre_vue(&nav.vues[i]), nav.vues[i].entite, Some(i), &mut action);
                    }
                });
            }
        });
        let mut nomenclatures: Vec<usize> = (0..nav.vues.len()).filter(|&i| nav.vues[i].genre == "Nomenclature" && garde(&nav.vues[i].nom)).collect();
        nomenclatures.sort_by_key(|&i| nav.vues[i].nom.to_lowercase());
        rubrique_nav(ui, LeftTab::Nomenclatures, format!("Nomenclatures/Quantités (tout) ({})", nomenclatures.len() + nomenclatures_natives.len()), false, &mut |ui| {
            if nomenclatures.is_empty() && nomenclatures_natives.is_empty() {
                ui.label(RichText::new("Aucune nomenclature").weak());
            }
            for (id, nom) in &nomenclatures_natives {
                if !garde(nom) {
                    continue;
                }
                let r = ui.add(egui::Button::selectable(false, nom.as_str()).sense(Sense::click_and_drag())).on_hover_text("Clic : ouvrir la nomenclature · glisser sur une feuille d'un jeu pour y poser le tableau");
                r.dnd_set_drag_payload(feuilles::NomenclatureGlissee(id.clone()));
                if r.clicked() {
                    ouvrir_nomenclature = Some(id.clone());
                }
            }
            for &i in &nomenclatures {
                feuille(ui, nav.vues[i].nom.clone(), nav.vues[i].entite, Some(i), &mut action);
            }
        });
        let mut feuilles: Vec<usize> = (0..nav.vues.len()).filter(|&i| nav.vues[i].genre == "Feuille").collect();
        feuilles.sort_by_key(|&i| (nav.vues[i].numero.clone(), nav.vues[i].nom.clone()));
        let indice: std::collections::HashMap<i64, usize> = nav.vues.iter().enumerate().map(|(i, v)| (v.revit_id, i)).collect();
        let jeu_ouvert = active.and_then(|i| nav.vues.get(i)).filter(|v| v.genre == "Jeu de feuilles").map(|v| v.nom.clone());
        let n_natives: usize = jeux_natifs.iter().map(|(_, fs)| fs.len()).sum();
        rubrique_nav(ui, LeftTab::Feuilles, format!("Feuilles (tout) ({})", feuilles.len() + n_natives), false, &mut |ui| {
            if self.natif.is_some() && ui.small_button("+ Nouveau jeu").on_hover_text("Créer un jeu de feuilles").clicked() {
                creer_jeu = true;
            }
            if feuilles.is_empty() && jeux_natifs.is_empty() {
                ui.label(RichText::new("Aucune feuille").weak());
            }
            // jeux du projet natif : clic sur le jeu : l'ouvrir ; sur une feuille : l'ouvrir
            // à cette feuille
            for (jeu, fs) in &jeux_natifs {
                let id = ui.make_persistent_id(("nav-jeu", jeu));
                egui::collapsing_header::CollapsingState::load_with_default_open(ui.ctx(), id, true)
                    .show_header(ui, |ui| {
                        let texte = RichText::new(format!("{jeu} ({})", fs.len()));
                        let texte = if jeu_ouvert.as_deref() == Some(jeu.as_str()) { texte.strong() } else { texte };
                        let r = ui.selectable_label(false, texte).on_hover_text("Ouvrir le jeu : toutes ses feuilles à la suite ; y déposer une feuille la met à la fin du jeu");
                        if r.clicked() {
                            ouvrir_jeu = Some((jeu.clone(), None));
                        }
                        r.context_menu(|ui| {
                            if ui.button("Propriétés du jeu").clicked() {
                                regler_jeu = Some((jeu.clone(), None));
                                ui.close();
                            }
                            if ui.button("Ajouter une feuille").clicked() {
                                ajouter_feuille_jeu = Some(jeu.clone());
                                ui.close();
                            }
                            if ui.button("Exporter le jeu en PDF…").clicked() {
                                exporter_jeu_nav = Some(jeu.clone());
                                ui.close();
                            }
                        });
                        let (rect, copie) = ui.allocate_exact_size(egui::vec2(18.0, 18.0), Sense::click());
                        let c = ui.visuals().text_color();
                        let p = ui.painter();
                        p.rect_stroke(egui::Rect::from_min_size(rect.min + egui::vec2(4.0, 2.0), egui::vec2(9.0, 11.0)), 1.0, egui::Stroke::new(1.0, c), egui::StrokeKind::Inside);
                        p.rect_stroke(egui::Rect::from_min_size(rect.min + egui::vec2(7.0, 5.0), egui::vec2(9.0, 11.0)), 1.0, egui::Stroke::new(1.0, c), egui::StrokeKind::Inside);
                        if copie.on_hover_text("Dupliquer le jeu et ses feuilles").clicked() { dupliquer_jeu = Some(jeu.clone()); }
                        // feuille déposée sur le jeu : à la fin du jeu
                        if r.dnd_hover_payload::<feuilles::FeuilleGlissee>().is_some() {
                            ui.painter().rect_stroke(r.rect, 3.0, egui::Stroke::new(1.5, ui.visuals().selection.stroke.color), egui::StrokeKind::Inside);
                        }
                        if let Some(g) = r.dnd_hover_payload::<feuilles::FeuilleGlissee>().and_then(|_| r.dnd_release_payload::<feuilles::FeuilleGlissee>()) {
                            deplacer = Some((g.0.clone(), jeu.clone(), None));
                        }
                    })
                    .body(|ui| {
                        // recherche : un jeu trouvé montre toutes ses feuilles ; une feuille,
                        // si son titre ou une de ses vues placées correspond
                        let jeu_trouve = garde(jeu);
                        for (k, (fid, titre, vues, tables)) in fs.iter().enumerate() {
                            if !jeu_trouve && !garde(titre) && !vues.iter().any(|v| garde(&v.1)) {
                                continue;
                            }
                            // feuille : dépliable sur ses vues placées (comme dans Revit)
                            let id_feuille = ui.make_persistent_id(("nav-feuille-native", fid));
                            let etat = egui::collapsing_header::CollapsingState::load_with_default_open(ui.ctx(), id_feuille, true);
                            let (_, entete, _) = etat.show_header(ui, |ui| ui.add(egui::Button::selectable(false, titre.as_str()).sense(Sense::click_and_drag())).on_hover_text("Clic : afficher la feuille · glisser : la déplacer dans le jeu ou vers un autre jeu"))
                                .body(|ui| {
                                    if vues.is_empty() && tables.is_empty() {
                                        ui.label(RichText::new("Aucune vue placée").weak().small());
                                    }
                                    for (id, nom) in tables {
                                        if ui.selectable_label(false, format!("{nom} (nomenclature)")).on_hover_text("Clic : ouvrir la nomenclature").clicked() {
                                            ouvrir_nomenclature = Some(id.clone());
                                        }
                                    }
                                    for (n, nom, echelle) in vues {
                                        let i = indice.get(n).copied();
                                        let r = ui.selectable_label(i.is_some() && i == active, format!("{nom} · 1:{echelle}")).on_hover_text("Clic : ouvrir la vue");
                                        if r.clicked() {
                                            action = i.map(ActionNav::Activer);
                                        }
                                    }
                                });
                            let r = entete.inner;
                            r.context_menu(|ui| {
                                if ui.button("Propriétés de la feuille").clicked() {
                                    regler_jeu = Some((jeu.clone(), Some(fid.clone())));
                                    ui.close();
                                }
                                if ui.button("Modifier la famille de cartouche…").clicked() {
                                    editer_cartouche_nav = Some(fid.clone());
                                    ui.close();
                                }
                                if ui.button("Exporter en PDF…").clicked() {
                                    exporter_feuille_nav = Some(("pdf", fid.clone()));
                                    ui.close();
                                }
                                if ui.button("Exporter en SVG…").clicked() {
                                    exporter_feuille_nav = Some(("svg", fid.clone()));
                                    ui.close();
                                }
                            });
                            r.dnd_set_drag_payload(feuilles::FeuilleGlissee(fid.clone()));
                            if r.clicked() {
                                ouvrir_jeu = Some((jeu.clone(), Some(fid.clone())));
                            }
                            // dépôt : au-dessus ou au-dessous de cette feuille selon le curseur
                            let dessous = |q: Option<egui::Pos2>| q.is_some_and(|q| q.y > r.rect.center().y);
                            if r.dnd_hover_payload::<feuilles::FeuilleGlissee>().is_some() {
                                let y = if dessous(ui.input(|i| i.pointer.hover_pos())) { r.rect.bottom() } else { r.rect.top() };
                                ui.painter().line_segment([egui::pos2(r.rect.left(), y), egui::pos2(r.rect.right(), y)], egui::Stroke::new(2.5, ui.visuals().selection.stroke.color));
                            }
                            if let Some(g) = r.dnd_hover_payload::<feuilles::FeuilleGlissee>().and_then(|_| r.dnd_release_payload::<feuilles::FeuilleGlissee>()) {
                                let avant = if dessous(ui.input(|i| i.pointer.hover_pos())) { fs.get(k + 1).map(|x| x.0.clone()) } else { Some(fid.clone()) };
                                deplacer = Some((g.0.clone(), jeu.clone(), avant));
                            }
                        }
                    });
            }
            for &i in &feuilles {
                let f = &nav.vues[i];
                let titre = if f.numero.is_empty() { titre_vue(f) } else { format!("{} - {}", f.numero, f.nom) };
                let placees: Vec<usize> = f.placees.iter().filter_map(|v| indice.get(v).copied()).collect();
                if !garde(&titre) && !placees.iter().any(|&j| garde(&nav.vues[j].nom)) {
                    continue;
                }
                if placees.is_empty() {
                    feuille(ui, titre, f.entite, Some(i), &mut action);
                } else {
                    let id = ui.make_persistent_id(("nav-feuille", f.revit_id));
                    egui::collapsing_header::CollapsingState::load_with_default_open(ui.ctx(), id, ouvert)
                        .show_header(ui, |ui| feuille(ui, titre, f.entite, Some(i), &mut action))
                        .body(|ui| {
                            for j in placees {
                                feuille(ui, titre_vue(&nav.vues[j]), nav.vues[j].entite, Some(j), &mut action);
                            }
                        });
                }
            }
        });
        // Familles : catégorie, famille, types
        let mut par_cat: BTreeMap<&str, Vec<&crate::rendu::scene::NavFamille>> = BTreeMap::new();
        for f in &nav.familles {
            if garde(&f.nom) || garde(&f.categorie) || f.types.iter().any(|t| garde(&t.0)) {
                par_cat.entry(f.categorie.as_str()).or_default().push(f);
            }
        }
        // projet natif : portes et fenêtres (familles modélisées), à ouvrir dans l'éditeur
        let familles_natives = if self.natif.is_some() { Some(self.familles_projet()) } else { None };
        let cartouches_natifs = self.natif.as_ref().map(|(p, _)| p.familles_cartouches.iter().map(|f| {
            let feuille = p.cartouches_feuilles.iter().find(|(_, nom)| *nom == &f.nom).map(|(id, _)| id.clone()).or_else(|| p.feuilles.first().map(|f| f.id.clone()));
            (f.clone(), feuille)
        }).collect::<Vec<_>>()).unwrap_or_default();
        let mut ouvrir_famille: Option<(Option<String>, crate::modele::GenreOuverture)> = None;
        let mut charger_famille = false;
        rubrique_nav(ui, LeftTab::Familles, "Familles".into(), false, &mut |ui| {
            if let Some(familles) = &familles_natives {
                egui::CollapsingHeader::new("Cartouches").id_salt("nav-familles-cartouches").show(ui, |ui| {
                    ui.label("Cartouche standard");
                    for (famille, feuille) in &cartouches_natifs {
                        if ui.selectable_label(false, &famille.nom).on_hover_text("Modifier la famille de cartouche").clicked() {
                            if let Some(feuille) = feuille {
                                ui.ctx().data_mut(|d| d.insert_temp(egui::Id::unique("editeur_cartouche"), (famille.clone(), feuille.clone(), None::<[f64; 2]>, famille.nom.clone())));
                            }
                        }
                    }
                });
                use crate::modele::GenreOuverture as G;
                for (genre, titre) in [(G::Porte, "Portes"), (G::Fenetre, "Fenêtres")] {
                    let liste: Vec<_> = familles.iter().filter(|f| f.2 == genre && garde(&f.1)).collect();
                    egui::CollapsingHeader::new(format!("{titre} ({})", liste.len())).id_salt(("nav-ouvertures", titre)).open(ouvert.then_some(true)).show(ui, |ui| {
                        for (id, nom, g) in liste {
                            if ui.selectable_label(false, nom).on_hover_text("Clic : ouvrir la famille dans l'éditeur").clicked() {
                                ouvrir_famille = Some((Some(id.clone()), *g));
                            }
                        }
                    });
                }
                ui.horizontal_wrapped(|ui| {
                    if ui.small_button("Nouvelle porte…").clicked() {
                        ouvrir_famille = Some((None, G::Porte));
                    }
                    if ui.small_button("Nouvelle fenêtre…").clicked() {
                        ouvrir_famille = Some((None, G::Fenetre));
                    }
                    if ui.small_button("Charger une famille…").on_hover_text("Fichier .famille.json : un nouveau type dans le projet").clicked() {
                        charger_famille = true;
                    }
                });
            }
            for (&cat, fams) in &par_cat {
                egui::CollapsingHeader::new(cat).id_salt(("nav-cat", cat)).open(ouvert.then_some(true)).show(ui, |ui| {
                    for f in fams.iter() {
                        egui::CollapsingHeader::new(&f.nom).id_salt(("nav-fam", cat, &f.nom)).open(ouvert.then_some(true)).show(ui, |ui| {
                            for (t, e) in &f.types {
                                let label = if e.is_some() { t.clone() } else { format!("{t} (non utilisé)") };
                                feuille(ui, label, *e, None, &mut action);
                            }
                        });
                    }
                });
            }
        });
        let groupes: Vec<&String> = nav.groupes.iter().filter(|g| garde(g)).collect();
        rubrique_nav(ui, LeftTab::Groupes, format!("Groupes ({})", groupes.len()), false, &mut |ui| {
            if groupes.is_empty() {
                ui.label(RichText::new("Aucun groupe").weak());
            }
            for g in &groupes {
                ui.label(g.as_str());
            }
        });
        let liens: Vec<String> = self.rvt.iter().flat_map(|r| r.external_refs.iter()).filter(|e| e.kind.contains("Revit")).map(|e| e.path.clone()).collect();
        rubrique_nav(ui, LeftTab::Liens, format!("Liens Revit ({})", liens.len()), false, &mut |ui| {
            for l in &liens {
                ui.label(l.as_str());
            }
            if liens.is_empty() {
                ui.label(RichText::new("Aucun lien").weak());
            }
        });
        if section == LeftTab::Tout && !self.liste_materiaux.is_empty() {
            egui::CollapsingHeader::new(RichText::new(format!("Matériaux ({})", self.liste_materiaux.len())).strong()).id_salt(("nav", "materiaux")).open(ouvert.then_some(true)).show(ui, |ui| self.materiaux_ui(ui));
        }
        if let Some((jeu, f)) = ouvrir_jeu {
            self.ouvrir_jeu(&jeu, f);
        }
        if creer_jeu { self.creer_jeu_arborescence(None); }
        if let Some(jeu) = dupliquer_jeu { self.creer_jeu_arborescence(Some(&jeu)); }
        if let Some(jeu) = ajouter_feuille_jeu {
            if let Some((p, _)) = &self.natif {
                let f = Self::nouvelle_feuille(p, &jeu);
                if let Err(e) = self.executer_bim(crate::modele::edition::commandes::Commande::Feuille { feuille: f }) { self.error = Some(e); }
            }
        }
        if let Some(jeu) = exporter_jeu_nav { self.exporter_jeu_dialogue(&jeu); }
        if let Some((extension, id)) = exporter_feuille_nav { self.exporter_feuille(extension, &id); }
        if let Some(id) = editer_cartouche_nav { self.editer_cartouche_feuille(ui.ctx(), &id); }
        if let Some((jeu, feuille)) = regler_jeu {
            self.ouvrir_jeu(&jeu, feuille);
        }
        if let Some((t, genre)) = ouvrir_famille {
            self.ouvrir_editeur_famille_type(t, genre);
        }
        if charger_famille {
            if let Some(c) = rfd::FileDialog::new().add_filter("Famille", &["json"]).pick_file() {
                if let Err(e) = self.charger_famille_fichier(&c) {
                    self.error = Some(e);
                }
            }
        }
        if let Some((id, jeu, avant)) = deplacer {
            self.deplacer_feuille(&id, &jeu, avant.as_deref());
        }
        if let Some(id) = ouvrir_nomenclature {
            self.ouvrir_nomenclature(&id);
        }
        match action {
            Some(ActionNav::Selection(e)) => self.select(Some(e), false),
            Some(ActionNav::Activer(i)) => self.activer_vue(i),
            Some(ActionNav::SupprimerVue(i)) => self.supprimer_vue_native(i),
            Some(ActionNav::IsolerType(t)) => {
                if let Some(s) = &self.scene {
                    let objs: Vec<u32> = s.objects.iter().enumerate().filter(|(_, o)| s.model.type_of.get(&o.id) == Some(&t)).map(|(i, _)| i as u32).collect();
                    self.select(Some(t), false);
                    self.isolate(objs.clone());
                    self.frame_objects(&objs);
                }
            }
            None => {}
        }
    }

    pub(super) fn tree_roots(&self) -> Vec<Id> {
        let s = self.scene.as_ref().unwrap();
        let mut roots: Vec<Id> = s.model.project.into_iter().collect();
        // Produits hors arborescence spatiale.
        let orphans = s.model.products.iter().any(|p| !s.model.parent.contains_key(p));
        if orphans {
            roots.push(0);
        }
        roots
    }

    pub(super) fn tree_node(&mut self, ui: &mut egui::Ui, id: Id, depth: u32) {
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

    pub(super) fn class_group(&mut self, ui: &mut egui::Ui, parent: Id, ty: &str, items: Vec<Id>, depth: u32) {
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

    pub(super) fn is_ancestor(&self, anc: Id, mut id: Id) -> bool {
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

    pub(super) fn class_list(&mut self, ui: &mut egui::Ui) {
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

    pub(super) fn filtered_list(&mut self, ui: &mut egui::Ui) {
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

}
