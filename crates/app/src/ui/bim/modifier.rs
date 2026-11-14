//! Options de modification, jonctions, annotations natives, export et documentation.
use super::*;

impl ViewerApp {
    /// Barre d'options de Modifier, en haut de la vue : réglages de l'outil actif
    /// (Déplacer, Copier, Tourner, Aligner, Ajuster) et commandes propres à la sélection
    /// (jonctions, mise à jour depuis le type, trémie). Propriétés ne garde que les
    /// informations de la sélection.
    pub(in crate::ui) fn options_modifier(&mut self, ui: &mut egui::Ui, selection_seule: bool) {
        use crate::modele::edition::commandes::Commande;
        use crate::ui::ruban::Outil;
        let outil = self.ruban_edition.outil();
        let ids = self.selection_bim();
        let Some((p, _)) = &self.natif else { return };
        let escalier = (ids.len() == 1).then(|| p.escaliers.iter().find(|e| e.id == ids[0])).flatten().map(|e| e.id.clone());
        let mut garde_corps: Option<Vec<usize>> = None;
        // couche d'assemblage : la sortir ou prendre l'assemblage ; mur indépendant : le
        // rattacher à un assemblage du même niveau
        let mur = (ids.len() == 1).then(|| p.murs.iter().find(|m| m.id == ids[0])).flatten().cloned();
        let assemblages: Vec<(String, String)> = mur.as_ref().filter(|m| m.assemblage.is_none()).map(|m| {
            p.assemblages.iter().filter(|a| p.couches_assemblage(&a.id).first().is_some_and(|c| c.niveau == m.niveau)).map(|a| (a.id.clone(), a.nom.clone())).collect()
        }).unwrap_or_default();
        let mut dissocier = false;
        let mut associer: Option<String> = None;
        let mut selection = None;
        let mut commande = None;
        let assemblage_seul = ids.len() == 1 && p.assemblage(&ids[0]).is_some();
        if selection_seule && escalier.is_none() && mur.is_none() && !assemblage_seul {
            return;
        }
        // dans Propriétés (sélection seule), aligné en haut : un alignement centré prenait
        // toute la hauteur du panneau et laissait un grand blanc
        ui.with_layout(egui::Layout::left_to_right(if selection_seule { egui::Align::Min } else { egui::Align::Center }).with_main_wrap(selection_seule), |ui| {
            if selection_seule && escalier.is_some() {
                ui.label("Garde-corps");
                for (nom, cotes) in [("Deux côtés", vec![0, 1]), ("Gauche", vec![0]), ("Droite", vec![1])] {
                    if ui.button(nom).on_hover_text("Garde-corps qui suivent les marches (gauche / droite du sens de montée), réglages de l'outil Garde-corps").clicked() {
                        garde_corps = Some(cotes);
                    }
                }
            }
            if let Some(m) = mur.as_ref().filter(|_| selection_seule) {
                if let Some(a) = &m.assemblage {
                    if ui.button("Dissocier").on_hover_text("La couche devient un mur indépendant (elle garde sa place et ses réglages)").clicked() {
                        dissocier = true;
                    }
                    if ui.button("Assemblage").on_hover_text("Sélectionner l'assemblage entier").clicked() {
                        selection = Some(crate::rendu::scene::id_natif("assemblage", a));
                    }
                } else if !assemblages.is_empty() {
                    egui::ComboBox::from_id_salt("associer_mur").width(170.0).selected_text("Associer à un assemblage…").show_ui(ui, |ui| {
                        for (id, nom) in &assemblages {
                            if ui.selectable_label(false, nom).clicked() {
                                associer = Some(id.clone());
                            }
                        }
                    });
                }
            }
            match outil.filter(|_| !selection_seule) {
                Some(o @ (Outil::Deplacer | Outil::Copier)) => {
                    if o == Outil::Deplacer {
                        ui.checkbox(&mut self.transfo.copie, "Copier").on_hover_text("Déplacer une copie ; l'original reste en place");
                    } else {
                        ui.checkbox(&mut self.transfo.multiple, "Multiple").on_hover_text("Copies successives depuis le même point de base ; Échap pour terminer");
                    }
                    let mut appliquer = false;
                    ui.collapsing("Saisie chiffrée", |ui| {
                        for (nom, v) in ["X", "Y", "Z"].into_iter().zip(&mut self.bim.delta) {
                            ui.horizontal(|ui| {
                                ui.label(nom);
                                ui.add(egui::DragValue::new(v).speed(10.0).dimensions());
                            });
                        }
                        appliquer = ui.add_enabled(!ids.is_empty(), egui::Button::new("Appliquer")).clicked();
                    });
                    if appliquer
                    {
                        commande = Some(if outil == Some(Outil::Copier) || self.transfo.copie {
                            Commande::Copier {
                                ids: ids.clone(),
                                delta: self.bim.delta,
                            }
                        } else {
                            Commande::Deplacer {
                                ids: ids.clone(),
                                delta: self.bim.delta,
                            }
                        });
                    }
                }
                Some(Outil::Tourner) => {
                    ui.checkbox(&mut self.transfo.copie, "Copier").on_hover_text("Tourner une copie ; l'original reste en place");
                    ui.label(if self.transfo.centre.is_some() { "Centre : placé" } else { "Centre : milieu de la sélection" });
                    if ui.add_enabled(!self.transfo.placer_centre, egui::Button::new("Placer le centre")).on_hover_text("Le prochain clic place le centre de rotation ; ou cliquer sur le centre dans la vue").clicked() {
                        self.transfo.placer_centre = true;
                    }
                    if self.transfo.centre.is_some() && ui.button("Centre de la sélection").clicked() {
                        self.transfo.centre = None;
                    }
                }
                Some(Outil::Symetrie) => {
                    ui.checkbox(&mut self.transfo.copie, "Copier").on_hover_text("Symétrie d'une copie ; décocher pour retourner l'original");
                }
                Some(Outil::Reseau) => {
                    ui.selectable_value(&mut self.transfo.polaire, false, "Linéaire");
                    ui.selectable_value(&mut self.transfo.polaire, true, "Polaire");
                    let mut n = self.transfo.nombre_reseau();
                    ui.label("Nombre");
                    if ui.add(egui::DragValue::new(&mut n).range(2..=500)).on_hover_text("Éléments au total, original compris").changed() {
                        self.transfo.nombre = n;
                    }
                }
                Some(Outil::Echelle) => {
                    ui.label("Annotations, contours, terrains : l'épaisseur des murs vient de leur type.");
                }
                Some(Outil::Decaler) => {
                    ui.checkbox(&mut self.transfo.copie, "Copier").on_hover_text("Créer un mur décalé ; décocher pour déplacer le mur");
                }
                Some(Outil::Scinder) => {
                    ui.label("Écart");
                    ui.add(egui::DragValue::new(&mut self.transfo.ecart).range(0.0..=10_000.0).speed(5.0).dimensions());
                }
                Some(Outil::Aligner) => {
                    match self.transfo.reference.as_deref() {
                        Some(r) => {
                            let nom = p.murs.iter().find(|m| m.id == r).map(|m| m.nom.as_str()).or_else(|| p.assemblage(r).map(|a| a.nom.as_str())).unwrap_or("mur");
                            ui.label(format!("Référence : axe de {nom}"));
                            if ui.button("Changer la référence").clicked() {
                                self.transfo.reinitialiser();
                            }
                        }
                        None => { ui.label("Cliquez la référence dans la vue."); }
                    }
                }
                Some(Outil::Ajuster) => {
                    if ui.checkbox(&mut self.ruban_edition.prolonger_angle,"Prolonger en angle")
                        .on_hover_text("Les deux éléments sont raccordés sur leurs lignes de justification").changed() {
                        self.ajustement=Some(Default::default());
                    }

                }
                Some(Outil::AttacherToiture) => {
                    let murs = &self.bim.murs_a_attacher;
                    ui.label(if murs.is_empty() { "Cliquer un mur, puis la toiture dans la vue.".to_string() } else { format!("{} mur(s) : cliquer la toiture dans la vue.", murs.len()) });
                    if ui.add_enabled(!murs.is_empty(), egui::Button::new("Détacher de la toiture")).clicked() {
                        commande = Some(Commande::Attacher { ids: murs.clone(), toiture: None });
                    }
                }
                _ => {}
            }
        });
        if let Some(c) = commande {
            if let Err(e) = self.executer_bim(c) {
                self.error = Some(e);
            }
        }
        if let Some(m) = mur.filter(|_| dissocier || associer.is_some()) {
            let mut p = self.natif.as_ref().unwrap().0.clone();
            let r = match &associer {
                Some(a) => p.associer(&m.id, a),
                None => p.dissocier(&m.id).map(|_| ()),
            };
            if let Err(e) = r.and_then(|()| self.appliquer_projet(p)) {
                self.error = Some(e);
            }
        }
        if let Some(sel) = selection {
            self.select(Some(sel), true);
        }
        if let (Some(id), Some(cotes)) = (escalier, garde_corps) {
            if let Err(e) = self.garde_corps_escalier(&id, &cotes) {
                self.error = Some(e);
            }
        }
    }

    /// Priorité de jonction, ouverte depuis le bouton Jonctions du ruban (Modifier).
    pub(in crate::ui) fn menu_jonctions(&mut self, ui: &mut egui::Ui) {
        use crate::modele::edition::commandes::Commande;
        let ids = self.selection_bim();
        let mut commande = None;
        ui.horizontal(|ui| {
            ui.label("Priorité de jonction");
            ui.add(egui::DragValue::new(&mut self.bim.priorite));
        });
        if ui.add_enabled(ids.len() == 1, egui::Button::new("Appliquer à la couche")).clicked() {
            commande = Some(Commande::PrioriteJonction { id: ids[0].clone(), priorite: self.bim.priorite });
        }
        ui.label("La priorité la plus élevée conserve son volume au croisement.");
        if let Some(c) = commande {
            if let Err(e) = self.executer_bim(c) {
                self.error = Some(e);
            }
            ui.close();
        }
    }

    /// Barre d'options affichée : Modifier, projet natif, outil à réglages ou sélection
    /// avec des commandes propres.
    pub(in crate::ui) fn a_options_modifier(&self) -> bool {
        use crate::ui::ruban::Outil;
        if self.natif.is_none() { return false; }
        if self.onglet_ruban != OngletRuban::Modifier {
            return false;
        }
        // la barre n'apparaît que pendant un outil ; les commandes de la sélection sont
        // dans le ruban (Jonctions, Copier sur niveaux) et dans Propriétés
        matches!(self.ruban_edition.outil(), Some(Outil::Deplacer | Outil::Copier | Outil::Tourner | Outil::Symetrie | Outil::Reseau | Outil::Echelle | Outil::Aligner | Outil::Decaler | Outil::Scinder | Outil::Demolir | Outil::Reproduire | Outil::ChangerHote | Outil::Inverser | Outil::Ajuster | Outil::AttacherToiture))
    }

    pub(in crate::ui) fn vue_native_id(&self) -> Option<String> {
        let p = &self.natif.as_ref()?.0;
        let num = self
            .scene
            .as_ref()?
            .navigateur
            .as_ref()?
            .vues
            .get(self.vue_active?)?
            .revit_id;
        p.vues
            .iter()
            .find(|v| scene::numero_vue(&v.id) == num)
            .map(|v| v.id.clone())
    }
    pub(in crate::ui) fn annotations_natives(&self) -> Vec<noyau::trace2d::Primitive> {
        let Some(id) = self.vue_native_id() else {
            return Vec::new();
        };
        let p = &self.natif.as_ref().unwrap().0;
        let Some(v) = p.vues.iter().find(|v| v.id == id) else {
            return Vec::new();
        };
        let origin = self.scene.as_ref().map_or(glam::DVec3::ZERO, |s| s.origin);
        let selection: HashSet<u64> = self
            .bim
            .selection
            .iter()
            .map(|id| scene::id_natif("annotation", id))
            .collect();
        crate::modele::documentation::annotations(p, v)
            .into_iter()
            .filter(|a| !a.element.is_some_and(|id| v.elements_masques.iter().any(|cle| scene::id_natif("annotation", cle) == id)))
            .filter(|a| !self.objet_en_edition().is_some_and(|(id, annotation)| annotation && a.element == Some(scene::id_natif("annotation", id))))
            .map(|mut a| {
                if a.element.is_some_and(|id| selection.contains(&id)) {
                    a.style.couleur = [230, 120, 20, 255];
                }
                a.translater(-origin)
            })
            .collect()
    }
    pub(in crate::ui) fn export_ifc_dialogue(&mut self) {
        let Some((p, _)) = &self.natif else { return };
        let Some(path) = rfd::FileDialog::new()
            .set_file_name("projet.ifc")
            .add_filter("IFC4", &["ifc"])
            .save_file()
        else {
            return;
        };
        match crate::modele::echanges::export_ifc::exporter(p)
            .and_then(|s| crate::modele::echanges::enregistrement::ecrire_atomique(&path, s.as_bytes()))
        {
            Ok(()) => self.status = format!("IFC4 exporté : {}", path.display()),
            Err(e) => self.error = Some(e),
        }
    }
    pub(in crate::ui) fn export_pdf_dialogue(&mut self) {
        let Some(path) = rfd::FileDialog::new()
            .set_file_name("dessin.pdf")
            .add_filter("PDF", &["pdf"])
            .save_file()
        else {
            return;
        };
        match self
            .dessin_vue()
            .and_then(|d| crate::modele::documentation::pdf(&d.svg("Dessin")))
            .and_then(|s| crate::modele::echanges::enregistrement::ecrire_atomique(&path, &s))
        {
            Ok(()) => self.status = format!("PDF exporté : {}", path.display()),
            Err(e) => self.error = Some(e),
        }
    }
    /// Fenêtre des nomenclatures du projet : créer, régler (nom, catégorie, regrouper),
    /// tableau des quantités, export CSV.
    pub(in crate::ui) fn fenetre_documentation(&mut self, ctx: &egui::Context) {
        use crate::modele::{edition::commandes::Commande, documentation as d};
        if !self.bim.documentation {
            return;
        }
        let Some((p, _)) = &self.natif else { return };
        let mut ouvert = true;
        let mut commande = None;
        let mut export = None;
        egui::Window::new("Nomenclatures").id(egui::Id::unique("fenetre_nomenclatures")).open(&mut ouvert).default_width(640.0).show(ctx,|ui|{
            if ui.button("Nouvelle nomenclature").clicked() {commande=Some(Commande::Nomenclature {nomenclature:d::Nomenclature {id:String::new(),nom:"Quantités".into(),categorie:String::new(),regrouper:true,colonnes:Vec::new(),parametres:Vec::new()}});self.bim.nomenclature=p.nomenclatures.len();}
            ui.separator();
            if p.nomenclatures.is_empty() { ui.label(RichText::new("Aucune nomenclature : « Nouvelle nomenclature » pour commencer.").weak()); }
            if !p.nomenclatures.is_empty() {
                self.bim.nomenclature=self.bim.nomenclature.min(p.nomenclatures.len()-1);
                egui::ComboBox::from_id_salt("nomenclature_projet").selected_text(&p.nomenclatures[self.bim.nomenclature].nom).show_ui(ui,|ui|{for (k,n) in p.nomenclatures.iter().enumerate(){ui.selectable_value(&mut self.bim.nomenclature,k,&n.nom);}});
                let mut n=p.nomenclatures[self.bim.nomenclature].clone();
                ui.horizontal(|ui|{ui.label("Nom");ui.text_edit_singleline(&mut n.nom);ui.label("Catégorie");ui.text_edit_singleline(&mut n.categorie);});
                ui.checkbox(&mut n.regrouper,"Regrouper par type");
                // colonnes : sans choix, catégorie, type ou élément, nombre, volume
                ui.horizontal_wrapped(|ui|{
                    ui.label("Colonnes");
                    let defaut=[d::Colonne::Categorie, if n.regrouper {d::Colonne::Type} else {d::Colonne::Element}, d::Colonne::Nombre, d::Colonne::Volume];
                    let actuelles: Vec<d::Colonne> = if n.colonnes.is_empty() { defaut.to_vec() } else { n.colonnes.clone() };
                    for c in d::Colonne::TOUTES {
                        let mut on = actuelles.contains(&c);
                        if ui.toggle_value(&mut on, c.titre()).changed() {
                            let mut v: Vec<d::Colonne> = d::Colonne::TOUTES.into_iter().filter(|x| if *x == c { on } else { actuelles.contains(x) }).collect();
                            if v.is_empty() { v.push(d::Colonne::Type); }
                            n.colonnes = v;
                        }
                    }
                });
                // paramètres des familles (type, occurrence) et partagés du projet
                let mut noms: std::collections::BTreeSet<String> = p.types_ouvertures.iter().filter_map(|t| t.modele.as_ref()).flat_map(|f| f.parametres.iter().map(|x| x.nom.clone())).collect();
                noms.extend(p.parametres_partages.keys().cloned());
                noms.extend(n.parametres.iter().cloned());
                if !noms.is_empty() {
                    ui.horizontal_wrapped(|ui|{
                        ui.label("Paramètres").on_hover_text("Valeurs des familles d'ouvertures : type, occurrence (par ouverture) et partagés (projet)");
                        for nom in &noms {
                            let mut on = n.parametres.contains(nom);
                            if ui.toggle_value(&mut on, nom).changed() {
                                if on { n.parametres.push(nom.clone()); } else { n.parametres.retain(|x| x != nom); }
                            }
                        }
                    });
                }
                if serde_json::to_string(&n).ok()!=serde_json::to_string(&p.nomenclatures[self.bim.nomenclature]).ok() {commande=Some(Commande::Nomenclature {nomenclature:n.clone()});}
                let cle=serde_json::to_string(&n).unwrap_or_default();
                if self.bim.tableau_cache.as_ref().is_none_or(|(r,c,_)|*r!=self.bim.revision || *c!=cle) {self.bim.tableau_cache=Some((self.bim.revision,cle,d::tableau(p,&n)));}
                match &self.bim.tableau_cache.as_ref().unwrap().2 {
                    Ok(lignes)=>{egui::ScrollArea::both().max_height(230.0).show(ui,|ui|{egui::Grid::new("quantites_projet").striped(true).show(ui,|ui|{for l in lignes {for c in l {ui.label(c);}ui.end_row();}});});}
                    Err(e)=>{ui.colored_label(Color32::DARK_RED,e);}
                }
                if ui.button("Exporter CSV…").clicked(){export=Some(self.bim.nomenclature);}
            }
        });
        self.bim.documentation = ouvert;
        if let Some(c) = commande {
            if let Err(e) = self.executer_bim(c) {
                self.error = Some(e);
            }
        }
        if let Some(k) = export {
            let Some(path) = rfd::FileDialog::new().set_file_name("nomenclature.csv").add_filter("csv", &["csv"]).save_file() else {
                return;
            };
            let p = &self.natif.as_ref().unwrap().0;
            let contenu = d::tableau(p, &p.nomenclatures[k]).map(|l| d::csv(&l).into_bytes());
            match contenu.and_then(|b| crate::modele::echanges::enregistrement::ecrire_atomique(&path, &b)) {
                Ok(()) => self.status = format!("Export : {}", path.display()),
                Err(e) => self.error = Some(e),
            }
        }
    }
}
