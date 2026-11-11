//! Paramètres personnels : brouillon explicite, validation puis sauvegarde.
use super::*;
use crate::config::options::{
    Options, Unite,
    raccourcis::{self, COMMANDES, Raccourci},
};

pub(super) struct Parametres {
    pub categorie: String,
    pub brouillon: Options,
    filtre: String,
    erreur: Option<String>,
}
impl ViewerApp {
    /// Applique et enregistre le mode d'apparence (bouton de la barre haute, commande).
    pub(super) fn changer_theme(&mut self, ctx: &egui::Context, theme: crate::config::options::ThemeInterface) {
        self.options.theme_interface = theme;
        if let Some(p) = &mut self.parametres {
            p.brouillon.theme_interface = theme;
        }
        theme.appliquer(ctx);
        if let Err(e) = self.options.sauver() {
            self.error = Some(e);
        }
    }
    pub(super) fn ouvrir_parametres(&mut self) {
        if self.parametres.is_none() {
            self.parametres = Some(Parametres {
                categorie: "general".into(),
                brouillon: self.options.clone(),
                filtre: String::new(),
                erreur: None,
            });
        }
        self.raccourci_en_attente = None;
    }
    pub(super) fn appliquer_options(&mut self, o: Options) -> Result<(), String> {
        o.verifier()?;
        let g = match &o.gabarit {
            Some(p) => crate::config::gabarit::Gabarit::charger(p)?,
            None => crate::config::gabarit::Gabarit::defaut(),
        };
        o.sauver()?;
        crate::config::gabarit::definir(g);
        crate::config::options::definir_unite(o.unite);
        self.options = o;
        self.coupe_dirty = true;
        self.bim.revision = self.bim.revision.wrapping_add(1);
        self.raccourci_en_attente = None;
        Ok(())
    }
    pub(super) fn fenetre_parametres(&mut self, ctx: &egui::Context) {
        let Some(mut p) = self.parametres.take() else {
            return;
        };
        let mut ouvert = true;
        let mut appliquer = false;
        let mut annuler = false;
        egui::Window::new("Paramètres").id(egui::Id::unique("parametres_application"))
            .default_pos(ctx.input(|i|i.viewport_rect().center())-egui::vec2(425.0,300.0))
            .open(&mut ouvert).default_size([850.0,600.0]).min_size([620.0,420.0]).resizable(true)
            .show(ctx,|ui| {
                ui.set_min_size(egui::vec2(580.0,370.0));
                ui.horizontal(|ui| {
                    ui.heading("Paramètres de BIMedit");
                    ui.label(RichText::new("Enregistrés sur ce poste").weak());
                });
                ui.separator();
                let hauteur=(ui.available_height()-90.0).clamp(240.0,460.0);
                ui.allocate_ui_with_layout(egui::vec2(ui.available_width(),hauteur),egui::Layout::left_to_right(egui::Align::TOP),|ui| {
                    ui.vertical(|ui| {
                        ui.set_width(155.0);
                        for (id,nom) in [("general","Général"),("dessin","Dessin et accrochages"),("raccourcis","Raccourcis clavier"),("graphisme","Graphisme"),("gabarit","Gabarit")] {
                            ui.selectable_value(&mut p.categorie,id.to_string(),nom);
                        }
                    });
                    ui.separator();
                    let largeur=ui.available_width().max(500.0);
                    egui::ScrollArea::both().id_salt("contenu_parametres").max_height(hauteur).min_scrolled_width(0.0).auto_shrink([false,false]).show(ui,|ui| {
                        ui.with_layout(egui::Layout::top_down(egui::Align::Min),|ui|{
                        ui.set_width(largeur);
                        match p.categorie.as_str() {
                            "general"=>{
                                ui.heading("Général");ui.add_space(10.0);
                                if ui.button("Réintégrer les volets latéraux").clicked(){self.retablir_volets();}
                                ui.label("Unités de longueur du modèle");
                                for (unite,nom) in [(Unite::Millimetre,"Millimètres"),(Unite::Centimetre,"Centimètres"),(Unite::Metre,"Mètres")] {ui.radio_value(&mut p.brouillon.unite,unite,nom);}
                                ui.add_space(12.0);
                                ui.checkbox(&mut p.brouillon.navigation_ifc,"Afficher la navigation IFC");
                                ui.label(RichText::new("Arborescence spatiale, classes et attributs IFC dans les projets natifs et Revit.").small().weak());
                            }
                            "dessin"=>{
                                ui.heading("Dessin et accrochages");ui.add_space(10.0);
                                ui.horizontal(|ui|{ui.label("Délai d'activation du repérage");ui.add(egui::DragValue::new(&mut p.brouillon.delai_accrochage).range(0.1..=5.0).speed(0.05).fixed_decimals(2).suffix(" s"));});
                                ui.label("Survoler une extrémité, un milieu ou un centre pour mémoriser le point de référence.");
                                ui.label("Les accrochages aux objets restent immédiats. Le délai concerne l'activation des guides de repérage.");
                                ui.add_space(12.0);
                                ui.label("Le déplacement des extrémités des murs utilise les mêmes accrochages que le tracé.");
                                ui.label("Maj : contraindre aux axes du plan. Échap : abandonner le déplacement.");
                                if ui.button("Rétablir 0,8 seconde").clicked(){p.brouillon.delai_accrochage=0.8;}
                            }
                            "raccourcis"=>{
                                ui.heading("Raccourcis clavier");
                                ui.label("Une touche (F), une combinaison (Ctrl+S) ou une séquence de 2 à 3 lettres (MU). Vider un champ désactive la commande.");
                                ui.add(egui::TextEdit::singleline(&mut p.filtre).hint_text("Rechercher une commande"));
                                ui.add_space(8.0);
                                egui::Grid::new("raccourcis_personnels").num_columns(4).striped(true).spacing([12.0,7.0]).show(ui,|ui| {
                                    ui.strong("Catégorie");ui.strong("Commande");ui.strong("Raccourci");ui.label("");ui.end_row();
                                    for c in COMMANDES.iter().filter(|c|format!("{} {}",c.nom,c.categorie).to_lowercase().contains(&p.filtre.to_lowercase())) {
                                        ui.label(c.categorie);ui.label(c.nom);
                                        let texte=p.brouillon.raccourcis.entry(c.id.into()).or_insert_with(||c.defaut.into());
                                        ui.add_sized([145.0,24.0],egui::TextEdit::singleline(texte).desired_width(145.0).min_size(egui::vec2(145.0,24.0)).id_salt(c.id));
                                        if ui.small_button("Défaut").on_hover_text(format!("Par défaut : {}",c.defaut)).clicked(){*texte=c.defaut.into();}
                                        ui.end_row();
                                    }
                                });
                                ui.add_space(10.0);
                                if ui.button("Rétablir tous les raccourcis").clicked(){p.brouillon.raccourcis.clear();}
                                ui.label(RichText::new("Entrée et Échap restent les touches de validation et d'annulation des outils. Les commandes de tracé et la navigation au clavier gardent leur contexte.").small().weak());
                            }
                            "graphisme"=>{
                                ui.heading("Graphisme du projet");ui.add_space(10.0);
                                ui.label("Les épaisseurs des plumes sont enregistrées dans le projet ouvert.");
                                if ui.add_enabled(self.natif.is_some(),egui::Button::new("Modifier les 12 épaisseurs de traits…")).clicked(){self.editeur_plumes=self.natif.as_ref().map(|(p,_)|p.epaisseurs_plumes());}
                                ui.label(RichText::new("Les styles visuels, ombres et échelles restent réglables dans la barre de la vue.").small().weak());
                            }
                            "gabarit"=>{
                                ui.heading("Gabarit de départ");ui.add_space(10.0);
                                ui.label(p.brouillon.gabarit.as_ref().map_or("Gabarit embarqué".into(),|c|c.display().to_string()));
                                if ui.button("Choisir un gabarit…").clicked(){
                                    if let Some(c)=rfd::FileDialog::new().set_title("Gabarit").add_filter("Gabarit",&["json"]).pick_file(){
                                        match crate::config::gabarit::Gabarit::charger(&c){Ok(_)=>{p.brouillon.gabarit=Some(c);p.erreur=None;},Err(e)=>p.erreur=Some(e)}
                                    }
                                }
                                if ui.button("Utiliser le gabarit par défaut").clicked(){p.brouillon.gabarit=None;}
                                if ui.button("Enregistrer une copie du gabarit…").clicked(){
                                    if let Some(c)=rfd::FileDialog::new().set_title("Copie du gabarit").add_filter("Gabarit",&["json"]).set_file_name("agence.gabarit.json").save_file(){
                                        let contenu=match &p.brouillon.gabarit {Some(src)=>std::fs::read(src).map_err(|e|e.to_string()),None=>Ok(crate::config::gabarit::texte_defaut().as_bytes().to_vec())};
                                        if let Err(e)=contenu.and_then(|t|crate::modele::echanges::enregistrement::ecrire_atomique(&c,&t)){p.erreur=Some(e);}
                                    }
                                }
                            }
                            _=>{}
                        }
                        });
                    });
                });
                ui.separator();
                if let Err(e)=p.brouillon.verifier(){ui.colored_label(Color32::from_rgb(180,45,40),e);}
                if let Some(e)=&p.erreur {ui.colored_label(Color32::from_rgb(180,45,40),e);}
                ui.horizontal(|ui| {
                    appliquer=ui.add_enabled(p.brouillon.verifier().is_ok(),egui::Button::new("Enregistrer les paramètres")).clicked();
                    annuler=ui.button("Annuler").clicked();
                });
            });
        if appliquer {
            match self.appliquer_options(p.brouillon.clone()) {
                Ok(()) => {
                    self.status = "Paramètres enregistrés.".into();
                    return;
                }
                Err(e) => p.erreur = Some(e),
            }
        }
        if ouvert && !annuler {
            self.parametres = Some(p);
        }
    }

    pub(super) fn clavier_raccourcis(&mut self, ctx: &egui::Context) {
        if self.parametres.is_some() {
            self.raccourci_en_attente = None;
            return;
        }
        let focus = ctx.egui_wants_keyboard_input();
        let tab_vue = self.tab_dans_vue(ctx) && !self.mode_famille() && self.couche_suivante().is_some();
        if !focus && self.parametres.is_none() && self.ruban_edition.outil().is_none()
            && !self.mode_famille() && !self.vol.actif && self.dessin2d.edition_contour.is_none()
            && self.poignee_axe.is_none() && self.dessin2d.arc_glisse.is_none()
            && self.fenetre_selection.is_none() && self.loading.is_none()
            && ctx.input_mut(|i|i.consume_key(egui::Modifiers::NONE,egui::Key::Space)) {
            let ids=self.selection_bim();
            if ids.len()==1 && self.natif.as_ref().is_some_and(|(p,_)|p.trajectoire_justification(&ids[0]).is_some()) {
                match self.executer_bim(crate::modele::edition::commandes::Commande::InverserMur{id:ids[0].clone()}) {
                    Ok(_)=>self.status="Mur inversé — Ctrl+Z pour annuler.".into(),
                    Err(e)=>self.error=Some(e),
                }
            }
        }

        let trace = if self.mode_famille() {
            self.cartouche_actif().is_some_and(|c| c.trace_en_cours())
        } else {
            self.trace_en_cours()
                || self.dessin2d.edition_contour.is_some()
                || self.poignee_axe.is_some()
                || self.coupe_depart.is_some()
                || self.bim.depart_structure.is_some()
                || !self.transfo.points.is_empty()
        };
        let modeliser = self.natif.is_some()
            && !self.mode_famille()
            && !trace
            && !self.vol.actif
            && self.loading.is_none();
        let catalogue: Vec<_> = COMMANDES
            .iter()
            .filter_map(|c| {
                raccourcis::analyser(self.options.raccourci(c))
                    .ok()
                    .flatten()
                    .map(|r| (c, r))
            })
            .collect();
        let mut actions = Vec::new();
        let mut consommer = Vec::new();
        ctx.input(|i|{
            for (index,e) in i.events.iter().enumerate() {
                if let egui::Event::Key{key,pressed:true,repeat,modifiers,..}=e {
                    // touche maintenue : seules les commandes du soleil se répètent (U, I)
                    if let Some((c,_))=catalogue.iter().find(|(c,r)| matches!(r,Raccourci::Touche{key:k,modifiers:m} if key==k && modifiers.matches_exact(*m))&&(!focus||c.categorie=="Fichiers")&&(!*repeat||c.id.starts_with("soleil_"))) {
                        if (c.id != "selection_suivante" || tab_vue)
                            && (c.categorie=="Fichiers"||(!focus&&!trace&&!self.vol.actif&&(self.scene.is_some()||c.id=="annuler"||c.id=="retablir"))) {
                            actions.push(c.id);consommer.push(index);
                        }
                    }
                }
            }
        });
        if modeliser && !focus {
            let (temps, textes) = ctx.input(|i| {
                (
                    i.time,
                    if i.modifiers.ctrl || i.modifiers.alt || i.modifiers.command {
                        Vec::new()
                    } else {
                        i.events
                            .iter()
                            .filter_map(|e| {
                                if let egui::Event::Text(t) = e {
                                    Some(t.clone())
                                } else {
                                    None
                                }
                            })
                            .collect::<Vec<_>>()
                    },
                )
            });
            let (mut sequence, mut dernier) = self
                .raccourci_en_attente
                .take()
                .filter(|(_, t)| temps - *t <= 2.0)
                .unwrap_or((String::new(), temps));
            for c in textes
                .iter()
                .flat_map(|s| s.chars())
                .filter(char::is_ascii_alphabetic)
                .map(|c| c.to_ascii_uppercase())
            {
                sequence.push(c);
                dernier = temps;
                if let Some((commande, _)) = catalogue
                    .iter()
                    .find(|(_, r)| matches!(r,Raccourci::Sequence(s) if s==&sequence))
                {
                    actions.push(commande.id);
                    sequence.clear();
                } else if !catalogue
                    .iter()
                    .any(|(_, r)| matches!(r,Raccourci::Sequence(s) if s.starts_with(&sequence)))
                {
                    sequence = c.to_string();
                    if !catalogue.iter().any(
                        |(_, r)| matches!(r,Raccourci::Sequence(s) if s.starts_with(&sequence)),
                    ) {
                        sequence.clear();
                    }
                }
            }
            self.raccourci_en_attente = (!sequence.is_empty()).then_some((sequence, dernier));
        } else {
            self.raccourci_en_attente = None;
        }
        ctx.input_mut(|i| {
            let mut n = 0;
            i.events.retain(|_| {
                let garder = !consommer.contains(&n);
                n += 1;
                garder
            });
        });
        for action in actions {
            self.executer_raccourci(ctx, action);
        }
    }

    fn executer_raccourci(&mut self, ctx: &egui::Context, action: &str) {
        // onglet Famille : les raccourcis du projet (masquer, isoler, copier…) agiraient
        // sur la maquette masquée
        let famille = ["nouveau", "ouvrir", "fermer", "parametres", "theme_suivant", "enregistrer", "annuler", "retablir", "supprimer", "cadrer"];
        if self.mode_famille() && !famille.contains(&action) {
            return;
        }
        if self.famille_plane_active.is_some() {
            match action {
                "enregistrer" => { self.error = if self.famille_plane_active.and_then(|i| self.familles_planes.get(&i)).is_some_and(|e| e.autonome) { self.enregistrer_famille_plane_dialogue(false).err() } else { self.enregistrer_famille_plane(false).err() }; return; }
                "fermer" => { self.demander_fermeture_famille(); return; }
                "supprimer" => { self.supprimer_selection_famille_plane(); return; }
                "cadrer" => { let _ = self.navigation_cartouche("cadrer"); return; }
                "annuler" | "retablir" => return,
                _ => {}
            }
        }
        match action {
            "copier_presse" => self.memoriser_copie_niveau(self.selection_bim()),
            "coller_niveau" => self.ouvrir_coller_niveau(),
            "nouveau" if self.loading.is_none() => self.creer_projet(),
            "ouvrir" => self.open_dialog(ctx),
            "fermer" => self.fermer(),
            "parametres" => self.ouvrir_parametres(),
            "theme_suivant" => self.changer_theme(ctx, self.options.theme_interface.suivant()),
            "enregistrer" => {
                if self.famille_active && self.famille.is_some() {
                    self.action_famille = Some(if self.famille.as_ref().is_some_and(|e| e.autonome) {
                        editeur_famille::ActionFamille::Enregistrer
                    } else { editeur_famille::ActionFamille::Charger(self.famille.as_ref().is_some_and(|e| e.nouvelle())) });
                } else {
                    self.enregistrer_projet(ctx);
                }
            }
            "annuler" => self.parcourir_historique(false),
            "retablir" => self.parcourir_historique(true),
            "selection_tout" if self.rect_vue.is_positive() => self.selectionner_rectangle(
                self.rect_vue,
                FenetreSelection {
                    debut: egui::pos2(-100_000_000.0, -100_000_000.0),
                    fin: egui::pos2(100_000_000.0, 100_000_000.0),
                    ajouter: false,
                },
            ),
            "selection_suivante" => {
                if let Some(id) = self.couche_suivante() {
                    self.select(Some(id), true);
                    ctx.memory_mut(|m| m.move_focus(egui::FocusDirection::None));
                }
            }
            // onglet Famille : Suppr agit sur la famille, jamais sur le projet masqué
            "supprimer" if self.famille_active && self.famille.is_some() => {
                if let Some(ed) = self.famille.as_mut() { ed.supprimer_selection(); }
            }
            "supprimer" if self.natif.is_some() => self.demander_suppression(),
            "cadrer" if self.famille_active && self.famille.is_some() => self.cadrer_famille(),
            "cadrer" => match self.selected.map(|s| self.objects_under(s)) {
                Some(o) if !o.is_empty() => self.frame_objects(&o),
                _ => self.fit_all(),
            },
            "masquer" => {
                if !self.masquer_elements_vue() {
                    if let Some(sel) = self.selected {
                        let o = self.objects_under(sel);
                        self.hide(o);
                    }
                }
            }
            "isoler" => {
                if let Some(sel) = self.selected {
                    let o = self.objects_under(sel);
                    self.isolate(o);
                }
            }
            "visibilite" => self.voir_visibilite = !self.voir_visibilite,
            "soleil_moins" | "soleil_plus" => {
                let pas = if action == "soleil_plus" { 0.25 } else { -0.25 };
                self.soleil_heure = (self.soleil_heure + pas).rem_euclid(24.0).min(23.99);
                let (az, h) = self.position_soleil();
                self.status = format!("Soleil : {:02} h {:02} · azimut {az:.0}°, hauteur {h:.0}°", self.soleil_heure as u32, ((self.soleil_heure.fract()) * 60.0).round() as u32);
                self.visibility_dirty = true;
            }
            "nouveau" | "selection_tout" | "supprimer" => {}
            outil => {
                if let Err(e) = self.activer_outil_interface(outil) {
                    self.error = Some(e);
                }
            }
        }
    }
}
