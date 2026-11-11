//! Ruban et panneau de propriétés de l'éditeur de famille.
use super::*;

impl ViewerApp {
    /// Ruban de l'éditeur de familles : outils de création (onglet Créer), chargement dans
    /// le projet et fichiers (onglet Sortie).
    pub(in crate::ui) fn ruban_famille(&mut self, ui: &mut egui::Ui, sortie: bool) {
        use crate::ui::ruban::{bandeau, Commande as C, Icone as I};
        let Some(ed) = self.famille.as_ref() else { return };
        if sortie {
            let placement = ed.ouverture.is_none();
            let nouveau = ed.type_id.is_none();
            let charger = vec![
                C::new("charger", "Charger dans le projet", I::Charger, if nouveau { "Crée le type avec cette famille dans le projet (Ctrl+S)" } else { "Met à jour le type et toutes ses ouvertures (Ctrl+S)" }, self.natif.is_some()),
                C::new("nouveau_type", "Nouveau type", I::Nouveau, if placement { "Crée un type avec cette famille ; il sert au prochain placement" } else { "Crée un type avec cette famille et l'affecte à l'ouverture" }, self.natif.is_some()),
            ];
            let fichier = vec![
                C::new("enregistrer_sous", "Enregistrer sous", I::EnregistrerSous, "Fichier de famille .famille.json, à charger dans un autre projet", true),
                C::new("ouvrir", "Ouvrir", I::Ouvrir, "Ouvrir un fichier de famille dans l'éditeur", true),
            ];
            let mut action = None;
            for (cle, r) in bandeau(ui, &[("Charger", charger), ("Fichier", fichier)]) {
                if !r.clicked() {
                    continue;
                }
                action = match cle {
                    "charger" => Some(ActionFamille::Charger(nouveau)),
                    "nouveau_type" => Some(ActionFamille::Charger(true)),
                    "enregistrer_sous" => Some(ActionFamille::EnregistrerSous),
                    "ouvrir" => Some(ActionFamille::Ouvrir),
                    _ => None,
                };
            }
            if action.is_some() {
                self.action_famille = action;
            }
            return;
        }
        let outil = ed.outil;
        let outils = |liste: &[(OutilFamille, &'static str, &'static str, &'static str)]| -> Vec<C> {
            liste.iter().map(|&(o, cle, nom, aide)| {
                let actif = !ed.vue_3d && ed.outil_disponible(o);
                let aide = if actif { aide } else if ed.vue_3d { "Ouvrir une vue plane pour dessiner." } else { "Cet outil n'est pas disponible sur le plan de construction actif. Revenir au plan de la vue pour l'utiliser." };
                C { enfonce: actif && outil == o, ..C::new(cle, nom, I::Famille(cle), aide, actif) }
            }).collect()
        };
        let references = outils(&[
            (OutilFamille::PlanConstruction, "plan_construction", "Plan oblique", "Deux points dans la vue définissent un plan perpendiculaire à cette vue"),
            (OutilFamille::PlanReference, "plan_ref", "Plan de référence", "Cliquer deux points : le plan suit la direction dominante du tracé (vertical ou horizontal)"),
            (OutilFamille::Cote, "cote", "Cote", "Cliquer deux plans parallèles (le plan visé se colore), puis la position de la ligne de cote ; libeller ensuite la cote par un paramètre"),
            (OutilFamille::Egalite, "egalite", "Égalité", "Cliquer trois plans parallèles : le second reste au milieu des deux autres"),
        ]);
        let mesures = outils(&[
            (OutilFamille::CoteAngle, "cote_angle", "Angle", "Cote angulaire : sommet, un point sur chaque branche, puis position de l'arc (points accrochés aux plans)"),
            (OutilFamille::CoteDiametre, "cote_diametre", "Diamètre", "Cote de diamètre : centre, puis point du cercle"),
            (OutilFamille::Altimetrie, "altimetrie", "Altimétrie", "Altitude depuis le bas de la baie (élévation ou coupe) : point coté, puis position du texte"),
        ]);
        let formes = outils(&[
            (OutilFamille::ExtrusionChemin, "extrusion_chemin", "Extrusion par chemin", "Dessiner le profil fermé, puis définir les points du chemin dans Propriétés"),
            (OutilFamille::Raccordement, "raccordement", "Raccordement", "Dessiner le profil initial, puis régler le profil d’arrivée et la distance dans Propriétés"),
            (OutilFamille::RaccordementChemin, "raccordement_chemin", "Raccordement par chemin", "Dessiner le profil initial, puis définir le chemin et le profil d’arrivée dans Propriétés"),
            (OutilFamille::Revolution, "revolution", "Révolution", "Deux points pour l’axe, puis un profil fermé ; angle réglable dans Propriétés"),
            (OutilFamille::RevolutionVide, "revolution_vide", "Révolution vide", "Volume de découpe obtenu par rotation d’un profil autour de son axe"),
            (OutilFamille::Volume, "volume", "Volume", "Extrusion pleine : glisser un rectangle ; ses faces se verrouillent sur les plans les plus proches"),
            (OutilFamille::Vide, "vide", "Vide", "Extrusion vide : perce le mur hôte une fois la famille chargée"),
            (OutilFamille::Extrusion, "extrusion", "Extrusion", "Profil quelconque : cliquer les sommets (accrochés aux plans), recliquer le premier ou clic droit pour fermer"),
            (OutilFamille::ExtrusionVide, "extrusion_vide", "Extrusion vide", "Profil quelconque qui perce le mur hôte"),
        ]);
        let symboles = outils(&[
            (OutilFamille::Ligne, "ligne", "Ligne 2D", "Symbole 2D (plan ou élévation) affiché dans les vues du projet : clic droit pour finir"),
            (OutilFamille::Arc, "arc", "Arc 2D", "Symbole 2D en arc : départ, point de passage, fin"),
            (OutilFamille::Region, "region", "Région 2D", "Contour fermé rempli (plein ou hachuré) : cliquer les sommets, recliquer le premier ou clic droit pour fermer"),
        ]);
        let reponses = bandeau(ui, &[("Références", references), ("Cotes", mesures), ("Formes", formes), ("Symboles 2D", symboles)]);
        let Some(ed) = self.famille.as_mut() else { return };
        for (cle, r) in reponses {
            if !r.clicked() {
                continue;
            }
            let o = match cle {
                "plan_construction" => { ed.construction = None; OutilFamille::PlanConstruction },
                "plan_ref" => OutilFamille::PlanReference,
                "cote" => OutilFamille::Cote,
                "egalite" => OutilFamille::Egalite,
                "volume" => OutilFamille::Volume,
                "vide" => OutilFamille::Vide,
                "extrusion_chemin" => OutilFamille::ExtrusionChemin,
                "raccordement" => OutilFamille::Raccordement,
                "raccordement_chemin" => OutilFamille::RaccordementChemin,
                "revolution" => OutilFamille::Revolution,
                "revolution_vide" => OutilFamille::RevolutionVide,
                "extrusion" => OutilFamille::Extrusion,
                "extrusion_vide" => OutilFamille::ExtrusionVide,
                "ligne" => OutilFamille::Ligne,
                "arc" => OutilFamille::Arc,
                "region" => OutilFamille::Region,
                "cote_angle" => OutilFamille::CoteAngle,
                "cote_diametre" => OutilFamille::CoteDiametre,
                "altimetrie" => OutilFamille::Altimetrie,
                _ => continue,
            };
            // recliquer l'outil actif revient à la sélection
            ed.choisir_outil(if ed.outil == o { OutilFamille::Selection } else { o });
        }
    }
}

pub(super) fn panneau(ui: &mut egui::Ui, ed: &mut EditeurFamille) {
    match ed.selection {
        SelectionFamille::Extrusion(k) if k < ed.famille.extrusions.len() => {
            extrusion_ui(ui, &mut ed.famille.extrusions[k], &ed.famille.plans, &ed.famille.parametres, k);
            let e = &ed.famille.extrusions[k];
            if let Some(revolution) = &e.revolution {
                let r = ed.resoudre();
                let contour: Vec<_> = e.profil.iter().map(|p| { let (h,v)=ed.famille.point_esquisse(&r,p); [h,v] }).collect();
                if let Err(erreur) = revolution.valider(&contour) { ui.colored_label(ui.visuals().error_fg_color, erreur); }
            }
            if let Some(guidage) = &e.guidage {
                let r = ed.resoudre();
                let contour: Vec<_> = e.profil.iter().map(|p| { let (h,v)=ed.famille.point_esquisse(&r,p); [h,v] }).collect();
                let erreur = guidage.valider(&contour).err().or_else(|| guidage.triangles(&contour).is_empty().then(|| "Forme dégénérée : revoir le chemin ou les profils.".into()));
                if let Some(erreur) = erreur { ui.colored_label(ui.visuals().error_fg_color, erreur); }
                if guidage.profil_fin().is_some() && ui.button("Reprendre le profil initial à l'arrivée").clicked() {
                    use crate::modele::famille::modelee::FormeGuidee;
                    if let Some(FormeGuidee::Raccordement { profil_fin, .. } | FormeGuidee::RaccordementChemin { profil_fin, .. }) = &mut ed.famille.extrusions[k].guidage { *profil_fin=contour; }
                }
            }
            return;
        }
        SelectionFamille::Volume(k) if k < ed.famille.volumes.len() => {
            volume_ui(ui, &mut ed.famille.volumes[k], &ed.famille.plans, &ed.famille.parametres, k);
            return;
        }
        _ => {}
    }
    egui::CollapsingHeader::new("Plan de construction").default_open(true).show(ui, |ui| {
        let avant = ed.construction;
        let mut reference = None;
        egui::ComboBox::from_id_salt("plan_construction_actif").selected_text(ed.construction.and_then(|k| ed.famille.plans_construction.get(k)).map_or("Plan de la vue", |p| p.nom.as_str())).show_ui(ui, |ui| {
            ui.selectable_value(&mut ed.construction, None, "Plan de la vue");
            for (k, plan) in ed.famille.plans_construction.iter().enumerate() {
                ui.selectable_value(&mut ed.construction, Some(k), &plan.nom);
            }
            ui.separator();
            for plan in &ed.famille.plans {
                if ui.button(format!("Référence : {}", plan.nom)).clicked() { reference = Some(plan.clone()); }
            }
        });
        if let Some(p) = reference {
            let vue = match p.axe { AxePlan::X => VueEsquisse::Coupe, AxePlan::Y => VueEsquisse::Elevation, AxePlan::Z => VueEsquisse::Plan };
            let r = ed.resoudre();
            let plan = PlanConstruction { nom: format!("Référence : {}", p.nom), origine: vue.point(0.0,0.0,r.positions.get(&p.nom).copied().unwrap_or(p.position)).to_array(), horizontal: vue.point(1.0,0.0,0.0).to_array(), vertical: vue.point(0.0,1.0,0.0).to_array() };
            let k = ed.famille.plans_construction.iter().position(|p| p == &plan).unwrap_or_else(|| {
                let mut plan = plan;
                let base = plan.nom.clone();
                plan.nom = (1..).map(|n| if n == 1 { base.clone() } else { format!("{base} {n}") }).find(|nom| ed.famille.plans_construction.iter().all(|p| &p.nom != nom)).unwrap();
                ed.famille.plans_construction.push(plan);
                ed.famille.plans_construction.len()-1
            });
            ed.construction = Some(k);
        }
        if avant != ed.construction { ed.choisir_outil(OutilFamille::Selection); ed.selectionner(Vec::new()); }
        if ed.construction.is_some() { ui.label("Esquisse en coordonnées locales du plan ; profondeur suivant sa normale."); }
    });
    let r = ed.resoudre();
    let u = crate::config::options::unite();
    egui::Grid::new("famille_type").num_columns(2).spacing([10.0, 5.0]).show(ui, |ui| {
        ui.label("Type");
        ui.text_edit_singleline(&mut ed.nom);
        ui.end_row();
        let composant = ed.famille.genre == GenreOuverture::Libre;
        ui.label(LARGEUR).on_hover_text(if composant { "Paramètre de type : largeur du composant (entre Gauche et Droite)" } else { "Paramètre de type : largeur de la baie" });
        ui.add(egui::DragValue::new(&mut ed.largeur).speed(10.0).range(10.0..=20_000.0).dimensions());
        ui.end_row();
        ui.label(HAUTEUR).on_hover_text(if composant { "Paramètre de type : hauteur du composant" } else { "Paramètre de type : hauteur de la baie" });
        ui.add(egui::DragValue::new(&mut ed.hauteur).speed(10.0).range(10.0..=20_000.0).dimensions());
        ui.end_row();
        ui.label(if composant { "Profondeur" } else { EPAISSEUR }).on_hover_text(if composant { "Paramètre de type : profondeur du composant (entre Intérieur et Extérieur ; Epaisseur dans les formules)" } else { "Épaisseur du mur hôte : essai seulement, la maquette donne celle du mur" });
        ui.add(egui::DragValue::new(&mut ed.epaisseur).speed(10.0).range(10.0..=20_000.0).dimensions());
        ui.end_row();
        ui.label("Couleur");
        ui.color_edit_button_srgb(&mut ed.famille.couleur);
        ui.end_row();
    });

    egui::CollapsingHeader::new(RichText::new("Paramètres").strong()).default_open(true).show(ui, |ui| {
        let mut supprimer = None;
        let mut partager = None;
        let materiaux_projet = &ed.materiaux_projet;
        egui::Grid::new("famille_parametres").num_columns(6).spacing([5.0, 4.0]).show(ui, |ui| {
            for (k, p) in ed.famille.parametres.iter_mut().enumerate() {
                ui.add_sized([88.0, 20.0], egui::TextEdit::singleline(&mut p.nom));
                // valeur selon la nature : formule, case oui/non, matériau du projet
                match p.genre {
                    _ if p.portee == Portee::Partage && ed.partages.contains_key(&p.nom) => {
                        ui.label(RichText::new("projet").weak()).on_hover_text("Valeur du paramètre partagé du projet (section ci-dessous) ; la formule sert à défaut");
                    }
                    GenreParametre::OuiNon => {
                        let mut oui = evaluer_oui(&p.formule);
                        if ui.checkbox(&mut oui, "").on_hover_text("Valeur par défaut (une formule reste possible : 0 = non)").changed() {
                            p.formule = if oui { "1".into() } else { "0".into() };
                        }
                    }
                    GenreParametre::Materiau => {
                        let texte = if p.formule.trim().is_empty() { "(rôle)".to_string() } else { p.formule.clone() };
                        egui::ComboBox::from_id_salt(("valeur_materiau", k)).width(80.0).selected_text(texte).show_ui(ui, |ui| {
                            ui.selectable_value(&mut p.formule, String::new(), "(couleur du rôle)");
                            for (n, _) in materiaux_projet {
                                ui.selectable_value(&mut p.formule, n.clone(), n);
                            }
                        });
                    }
                    GenreParametre::Longueur => {
                        let aide = if p.portee == Portee::Occurrence { "Valeur par défaut ; chaque ouverture placée peut la changer dans Propriétés" } else { "Formule : nombres (mm), paramètres, + - * / ( )" };
                        ui.add_sized([80.0, 20.0], egui::TextEdit::singleline(&mut p.formule)).on_hover_text(aide);
                    }
                }
                egui::ComboBox::from_id_salt(("genre_parametre", k)).width(70.0).selected_text(p.genre.nom()).show_ui(ui, |ui| {
                    for g in GenreParametre::TOUS {
                        if ui.selectable_value(&mut p.genre, g, g.nom()).changed() {
                            p.formule = match g {
                                GenreParametre::Longueur => "100".into(),
                                GenreParametre::OuiNon => "1".into(),
                                GenreParametre::Materiau => String::new(),
                            };
                        }
                    }
                }).response.on_hover_text("Longueur (mm), oui/non (visibilité des volumes) ou matériau (couleur des volumes)");
                egui::ComboBox::from_id_salt(("portee", k)).width(74.0).selected_text(p.portee.nom()).show_ui(ui, |ui| {
                    for x in Portee::TOUTES.into_iter().filter(|x| *x != Portee::Partage || p.genre != GenreParametre::Materiau) {
                        ui.selectable_value(&mut p.portee, x, x.nom());
                    }
                }).response.on_hover_text("Type : valeur de la famille · Occurrence : propre à chaque ouverture · Partagé : valeur du projet (longueurs et oui/non)");
                match (p.genre, r.valeurs.get(&p.nom), r.materiaux.get(&p.nom)) {
                    (GenreParametre::Materiau, _, Some(m)) => ui.label(RichText::new(if m.is_empty() { "—" } else { m.as_str() }).weak()),
                    (GenreParametre::OuiNon, Some(v), _) => ui.label(RichText::new(if v.abs() > 1e-9 { "oui" } else { "non" }).weak()),
                    (_, Some(v), _) => ui.label(RichText::new(u.afficher(*v)).weak()),
                    _ => ui.colored_label(Color32::from_rgb(200, 40, 40), "?"),
                };
                if ui.small_button("−").on_hover_text("Supprimer le paramètre").clicked() {
                    supprimer = Some(k);
                }
                ui.end_row();
                if p.portee == Portee::Partage && !ed.partages.contains_key(&p.nom) {
                    partager = Some(k);
                }
            }
        });
        // un paramètre déclaré partagé rejoint la table du projet avec sa valeur actuelle
        if let Some(k) = partager {
            let p = &ed.famille.parametres[k];
            if let Some(v) = r.valeurs.get(&p.nom) {
                ed.partages.insert(p.nom.clone(), *v);
            }
        }
        if let Some(k) = supprimer {
            ed.famille.parametres.remove(k);
        }
        ui.horizontal(|ui| {
            if ui.button("Ajouter un paramètre").clicked() {
                let nom = (1..).map(|k| format!("Parametre{k}")).find(|n| ed.famille.parametres.iter().all(|p| &p.nom != n)).unwrap();
                ed.famille.parametres.push(Parametre::nouveau(&nom, "100"));
            }
            let disponibles: Vec<(String, f64)> = ed.partages.iter().filter(|(n, _)| ed.famille.parametres.iter().all(|p| &p.nom != *n)).map(|(n, v)| (n.clone(), *v)).collect();
            ui.add_enabled_ui(!disponibles.is_empty(), |ui| {
                ui.menu_button("Depuis le projet…", |ui| {
                    for (n, v) in &disponibles {
                        if ui.button(format!("{n} ({})", u.afficher(*v))).clicked() {
                            ed.famille.parametres.push(Parametre { portee: Portee::Partage, ..Parametre::nouveau(n, &format!("{v}")) });
                            ui.close();
                        }
                    }
                })
                .response
                .on_hover_text("Réutiliser un paramètre partagé du projet");
            });
        });
    });

    egui::CollapsingHeader::new(RichText::new("Paramètres partagés du projet").strong()).default_open(false).show(ui, |ui| {
        ui.label(RichText::new("Communs à toutes les familles qui les déclarent « Partagé » ; écrits dans le projet à l'enregistrement.").small().weak());
        let mut retirer = None;
        egui::Grid::new("famille_partages").num_columns(3).spacing([6.0, 4.0]).show(ui, |ui| {
            for (n, v) in ed.partages.iter_mut() {
                ui.label(n);
                ui.add(egui::DragValue::new(v).speed(5.0).dimensions());
                if ui.small_button("−").on_hover_text("Retirer du projet (les familles reprennent leur formule)").clicked() {
                    retirer = Some(n.clone());
                }
                ui.end_row();
            }
        });
        if let Some(n) = retirer {
            ed.partages.remove(&n);
        }
    });

    egui::CollapsingHeader::new(RichText::new("Contraintes").strong()).default_open(true).show(ui, |ui| {
        let mut supprimer = None;
        let noms_parametres: Vec<String> = [LARGEUR, HAUTEUR, EPAISSEUR].into_iter().map(String::from).chain(ed.famille.parametres.iter().map(|p| p.nom.clone())).collect();
        egui::Grid::new("famille_contraintes").num_columns(5).spacing([6.0, 4.0]).show(ui, |ui| {
            for (k, c) in ed.famille.contraintes.iter_mut().enumerate() {
                let conflit = r.conflits.contains(&k);
                let choisie = ed.selection == SelectionFamille::Contrainte(k);
                let texte = match c {
                    Contrainte::Distance { a, b, .. } => format!("{a} – {b}"),
                    Contrainte::Egalite { a, m, b, .. } => format!("EQ {a} | {m} | {b}"),
                };
                let couleur = if conflit { Color32::from_rgb(200, 40, 40) } else { ui.visuals().text_color() };
                if ui.selectable_label(choisie, RichText::new(texte).color(couleur)).on_hover_text(if conflit { "Écartée : en conflit avec une contrainte de priorité supérieure" } else { "Satisfaite" }).clicked() {
                    ed.selection = SelectionFamille::Contrainte(k);
                }
                match c {
                    Contrainte::Distance { valeur, .. } => {
                        ui.horizontal(|ui| {
                            ui.add_sized([80.0, 20.0], egui::TextEdit::singleline(valeur)).on_hover_text("Valeur (mm) ou formule : libeller la cote par un paramètre pour la piloter");
                            egui::ComboBox::from_id_salt(("libelle", k)).width(18.0).selected_text("").show_ui(ui, |ui| {
                                for n in &noms_parametres {
                                    if ui.selectable_label(valeur == n, n).clicked() {
                                        *valeur = n.clone();
                                    }
                                }
                            });
                        });
                    }
                    Contrainte::Egalite { .. } => {
                        ui.label("EQ");
                    }
                }
                ui.add(egui::DragValue::new(c.priorite_mut()).range(0..=9).prefix("P ")).on_hover_text("Priorité : en cas de conflit, la plus haute l'emporte (à égalité, la plus ancienne)");
                ui.label(if conflit { RichText::new("conflit").color(Color32::from_rgb(200, 40, 40)) } else { RichText::new("ok").color(Color32::from_rgb(30, 140, 60)) });
                if ui.small_button("−").on_hover_text("Supprimer la contrainte").clicked() {
                    supprimer = Some(k);
                }
                ui.end_row();
            }
        });
        if let Some(k) = supprimer {
            ed.erreur = ed.supprimer(&SelectionFamille::Contrainte(k)).err();
        }
    });

    egui::CollapsingHeader::new(RichText::new("Plans de référence").strong()).default_open(false).show(ui, |ui| {
        let mut renommer = None;
        let mut supprimer = None;
        egui::Grid::new("famille_plans").num_columns(5).spacing([6.0, 4.0]).show(ui, |ui| {
            for (k, p) in ed.famille.plans.iter_mut().enumerate() {
                let id = egui::Id::unique(("nom_plan", k));
                let mut texte = ui.data_mut(|d| d.get_temp::<String>(id)).unwrap_or_else(|| p.nom.clone());
                let rep = ui.add_sized([110.0, 20.0], egui::TextEdit::singleline(&mut texte));
                if rep.changed() {
                    ui.data_mut(|d| d.insert_temp(id, texte.clone()));
                }
                if rep.lost_focus() {
                    ui.data_mut(|d| d.remove::<String>(id));
                    if texte != p.nom {
                        renommer = Some((p.nom.clone(), texte));
                    }
                }
                ui.label(RichText::new(p.axe.nom()).weak());
                ui.label(u.afficher(r.positions.get(&p.nom).copied().unwrap_or(p.position)));
                if p.hote.is_some() {
                    ui.label(RichText::new("hôte").weak());
                } else {
                    ui.checkbox(&mut p.fixe, "fixe").on_hover_text("Épinglé à sa position dessinée");
                }
                if p.hote.is_none() && ui.small_button("−").on_hover_text("Supprimer le plan (s'il n'est pas utilisé)").clicked() {
                    supprimer = Some(k);
                }
                ui.end_row();
            }
        });
        if let Some((a, n)) = renommer {
            ed.erreur = ed.renommer_plan(&a, &n).err();
        }
        if let Some(k) = supprimer {
            let nom = ed.famille.plans[k].nom.clone();
            ed.erreur = ed.supprimer(&SelectionFamille::Plan(nom)).err();
        }
        // limites des retours des couches du mur hôte
        let retour = |nom: &str| ed.famille.plans.iter().any(|p| p.axe == AxePlan::Y && p.nom.to_lowercase().replace('é', "e") == nom);
        let etat = |ok: bool| if ok { "présent" } else { "absent" };
        ui.label(RichText::new(format!(
            "Retours des couches : nommer un plan d'épaisseur « Retour extérieur » (limite des couches côté extérieur, {}) et « Retour intérieur » ({}). Sans ces plans, pas de retour.",
            etat(retour("retour exterieur")), etat(retour("retour interieur"))
        )).small().weak());
    });

    egui::CollapsingHeader::new(RichText::new("Volumes").strong()).default_open(true).show(ui, |ui| {
        let mut supprimer = None;
        for (k, v) in ed.famille.volumes.iter().enumerate() {
            ui.horizontal(|ui| {
                let texte = if v.vide { format!("{} (vide)", v.nom) } else { format!("{} · {}", v.nom, v.materiau.nom()) };
                if ui.selectable_label(ed.selection == SelectionFamille::Volume(k), texte).clicked() {
                    ed.selection = SelectionFamille::Volume(k);
                }
                if ui.small_button("−").on_hover_text("Supprimer le volume").clicked() {
                    supprimer = Some(k);
                }
            });
        }
        if let Some(k) = supprimer {
            ed.erreur = ed.supprimer(&SelectionFamille::Volume(k)).err();
        }
        if let SelectionFamille::Volume(k) = ed.selection.clone() {
            if let Some(v) = ed.famille.volumes.get_mut(k) {
                ui.separator();
                volume_ui(ui, v, &ed.famille.plans, &ed.famille.parametres, k);
            }
        }
    });

    egui::CollapsingHeader::new(RichText::new("Formes à profil").strong()).default_open(true).show(ui, |ui| {
        let mut supprimer = None;
        for (k, e) in ed.famille.extrusions.iter().enumerate() {
            ui.horizontal(|ui| {
                let texte = format!("{} · {} sommets{}", e.nom, e.profil.len(), if e.vide { " (vide)" } else { "" });
                if ui.selectable_label(ed.selection == SelectionFamille::Extrusion(k), texte).clicked() {
                    ed.selection = SelectionFamille::Extrusion(k);
                }
                if ui.small_button("−").on_hover_text("Supprimer l'extrusion").clicked() {
                    supprimer = Some(k);
                }
            });
        }
        if let Some(k) = supprimer {
            ed.erreur = ed.supprimer(&SelectionFamille::Extrusion(k)).err();
        }
        if let SelectionFamille::Extrusion(k) = ed.selection.clone() {
            if let Some(e) = ed.famille.extrusions.get_mut(k) {
                ui.separator();
                extrusion_ui(ui, e, &ed.famille.plans, &ed.famille.parametres, k);
            }
        }
    });

    egui::CollapsingHeader::new(RichText::new("Symboles 2D").strong()).default_open(true).show(ui, |ui| {
        let mut supprimer = None;
        let mut changer_remplissage = None;
        for (k, s) in ed.famille.symboles.iter().enumerate() {
            ui.horizontal(|ui| {
                let vue = match s.vue {
                    VueEsquisse::Plan => "plan",
                    VueEsquisse::Elevation => "élévation",
                    VueEsquisse::Coupe => "coupe (éditeur seulement)",
                };
                let genre = if s.remplissage.is_some() { "Région" } else if s.arc { "Arc" } else { "Ligne" };
                let texte = format!("{genre} {} · {vue}", k + 1);
                if ui.selectable_label(ed.selection == SelectionFamille::Symbole(k), texte).clicked() {
                    ed.selection = SelectionFamille::Symbole(k);
                }
                if let Some(m) = s.remplissage {
                    let mut choix = m;
                    egui::ComboBox::from_id_salt(("remplissage", k)).width(80.0).selected_text(choix.nom()).show_ui(ui, |ui| {
                        for x in [Remplissage::Plein, Remplissage::Hachure] { ui.selectable_value(&mut choix, x, x.nom()); }
                    });
                    if choix != m { changer_remplissage = Some((k, choix)); }
                }
                if ui.small_button("−").on_hover_text("Supprimer le symbole").clicked() {
                    supprimer = Some(k);
                }
            });
        }
        if let Some((k, m)) = changer_remplissage {
            ed.famille.symboles[k].remplissage = Some(m);
        }
        if let Some(k) = supprimer {
            ed.erreur = ed.supprimer(&SelectionFamille::Symbole(k)).err();
        }
        ui.label(RichText::new("Les symboles de plan et d'élévation s'affichent dans les vues du projet et sur les feuilles.").small().weak());
        let mut supprimer_cote = None;
        for (k, c) in ed.famille.cotes_mesure.iter().enumerate() {
            ui.horizontal(|ui| {
                let texte = format!("{} {} · {}", c.genre.nom(), k + 1, match c.vue { VueEsquisse::Plan => "plan", VueEsquisse::Elevation => "élévation", VueEsquisse::Coupe => "coupe" });
                if ui.selectable_label(ed.selection == SelectionFamille::CoteMesure(k), texte).clicked() {
                    ed.selection = SelectionFamille::CoteMesure(k);
                }
                if ui.small_button("−").on_hover_text("Supprimer la cote").clicked() {
                    supprimer_cote = Some(k);
                }
            });
        }
        if let Some(k) = supprimer_cote {
            ed.erreur = ed.supprimer(&SelectionFamille::CoteMesure(k)).err();
        }
    });
}

/// Valeur oui/non d'une formule simple (« 0 » : non ; illisible : oui).
pub(super) fn evaluer_oui(formule: &str) -> bool {
    crate::modele::famille::modelee::evaluer(formule, &BTreeMap::new()).map_or(true, |v| v.abs() > 1e-9)
}

/// Remplit un polygone quelconque (triangulé).
pub(super) fn remplir(painter: &egui::Painter, pts: &[egui::Pos2], couleur: Color32) {
    let mut m = egui::Mesh::default();
    for t in crate::noyau::texte::trianguler(vec![pts.iter().map(|p| [p.x, p.y]).collect()]) {
        for p in t {
            m.colored_vertex(egui::pos2(p[0], p[1]), couleur);
        }
    }
    m.indices = (0..m.vertices.len() as u32).collect();
    painter.add(egui::Shape::mesh(m));
}

pub(super) fn distance_segment(q: egui::Pos2, a: egui::Pos2, b: egui::Pos2) -> f32 {
    let ab = b - a;
    let t = if ab.length_sq() > 0.0 { ((q - a).dot(ab) / ab.length_sq()).clamp(0.0, 1.0) } else { 0.0 };
    q.distance(a + ab * t)
}

pub(super) fn dans_polygone(q: egui::Pos2, p: &[egui::Pos2]) -> bool {
    let mut dedans = false;
    for i in 0..p.len() {
        let (a, b) = (p[i], p[(i + 1) % p.len()]);
        if (a.y > q.y) != (b.y > q.y) && q.x < a.x + (q.y - a.y) / (b.y - a.y) * (b.x - a.x) {
            dedans = !dedans;
        }
    }
    dedans
}

/// Lignes de grille : visibilité commandée par un paramètre oui/non, couleur par un
/// paramètre matériau (pas pour un vide).
pub(super) fn pilotage_ui(ui: &mut egui::Ui, visible: &mut Option<String>, materiau: &mut Option<String>, vide: bool, parametres: &[Parametre], id: (&str, usize)) {
    let liste = |ui: &mut egui::Ui, choix: &mut Option<String>, genre: GenreParametre, aucun: &str, sel: (&str, &str, usize)| {
        egui::ComboBox::from_id_salt(sel).width(130.0).selected_text(choix.clone().unwrap_or_else(|| aucun.into())).show_ui(ui, |ui| {
            ui.selectable_value(choix, None, aucun);
            for p in parametres.iter().filter(|p| p.genre == genre) {
                ui.selectable_value(choix, Some(p.nom.clone()), &p.nom);
            }
        });
    };
    ui.label("Visible si").on_hover_text("Paramètre oui/non : non masque le volume (un vide masqué ne perce plus)");
    liste(ui, visible, GenreParametre::OuiNon, "toujours", ("visible", id.0, id.1));
    ui.end_row();
    if !vide {
        ui.label("Matériau par").on_hover_text("Paramètre matériau : couleur du matériau du projet");
        liste(ui, materiau, GenreParametre::Materiau, "(rôle)", ("materiau_parametre", id.0, id.1));
        ui.end_row();
    }
}

/// Propriétés de l'extrusion choisie : nom, plein ou vide, matériau, profondeur.
pub(super) fn extrusion_ui(ui: &mut egui::Ui, e: &mut Extrusion, plans: &[PlanRef], parametres: &[Parametre], k: usize) {
    egui::Grid::new(("extrusion", k)).num_columns(2).spacing([8.0, 4.0]).show(ui, |ui| {
        ui.label("Nom");
        ui.text_edit_singleline(&mut e.nom);
        ui.end_row();
        ui.label("Genre");
        ui.horizontal(|ui| {
            ui.selectable_value(&mut e.vide, false, "Plein");
            ui.selectable_value(&mut e.vide, true, "Vide").on_hover_text("Perce le mur hôte");
        });
        ui.end_row();
        if !e.vide {
            ui.label("Matériau");
            egui::ComboBox::from_id_salt(("materiau_extrusion", k)).selected_text(e.materiau.nom()).show_ui(ui, |ui| {
                for m in Materiau::TOUS {
                    ui.selectable_value(&mut e.materiau, m, m.nom());
                }
            });
            ui.end_row();
        }
        pilotage_ui(ui, &mut e.visible, &mut e.materiau_parametre, e.vide, parametres, ("extrusion", k));
        if let Some(revolution) = &mut e.revolution {
            ui.label("Angle de départ");
            ui.add(egui::DragValue::new(&mut revolution.debut).speed(1.0).suffix("°"));
            ui.end_row();
            ui.label("Angle de révolution");
            ui.add(egui::DragValue::new(&mut revolution.angle).range(0.1..=360.0).speed(1.0).suffix("°"));
            ui.end_row();
            for (i, point) in revolution.axe.iter_mut().enumerate() {
                ui.label(format!("Axe · point {}", i+1));
                ui.horizontal(|ui| {
                    ui.label("H"); ui.add(egui::DragValue::new(&mut point[0]).speed(1.0).dimensions());
                    ui.label("V"); ui.add(egui::DragValue::new(&mut point[1]).speed(1.0).dimensions());
                });
                ui.end_row();
            }
            return;
        }
        if e.guidage.is_some() { return; }
        let axe = e.vue.axes().2;
        let [d0, d1] = &mut e.profondeur;
        for (cote, face) in [("début", d0), ("fin", d1)] {
            ui.label(format!("{} {cote}", if e.construction.is_some() { "Profondeur" } else { axe.nom() }));
            ui.horizontal(|ui| {
                egui::ComboBox::from_id_salt(("profondeur", k, cote)).width(120.0).selected_text(face.plan.as_str()).show_ui(ui, |ui| {
                    if e.construction.is_some() { ui.selectable_value(&mut face.plan, String::new(), "Origine du plan"); }
                    for p in plans.iter().filter(|p| e.construction.is_none() && p.axe == axe) {
                        ui.selectable_value(&mut face.plan, p.nom.clone(), &p.nom);
                    }
                });
                ui.add(egui::DragValue::new(&mut face.decalage).speed(1.0).dimensions());
            });
            ui.end_row();
        }
    });
    if let Some(guidage) = &mut e.guidage { formes::guidage_ui(ui, guidage); }
    ui.label(RichText::new(format!("Profil esquissé en {} : sommets verrouillés sur les plans.", match e.vue { VueEsquisse::Elevation => "élévation", VueEsquisse::Plan => "plan", VueEsquisse::Coupe => "coupe" })).small().weak());
}

/// Propriétés du volume choisi : nom, plein ou vide, matériau, faces verrouillées.
pub(super) fn volume_ui(ui: &mut egui::Ui, v: &mut Volume, plans: &[PlanRef], parametres: &[Parametre], k: usize) {
    egui::Grid::new(("volume", k)).num_columns(2).spacing([8.0, 4.0]).show(ui, |ui| {
        ui.label("Nom");
        ui.text_edit_singleline(&mut v.nom);
        ui.end_row();
        ui.label("Genre");
        ui.horizontal(|ui| {
            ui.selectable_value(&mut v.vide, false, "Plein");
            ui.selectable_value(&mut v.vide, true, "Vide").on_hover_text("Perce le mur hôte");
        });
        ui.end_row();
        if !v.vide {
            ui.label("Matériau");
            egui::ComboBox::from_id_salt(("materiau", k)).selected_text(v.materiau.nom()).show_ui(ui, |ui| {
                for m in Materiau::TOUS {
                    ui.selectable_value(&mut v.materiau, m, m.nom());
                }
            });
            ui.end_row();
        }
        pilotage_ui(ui, &mut v.visible, &mut v.materiau_parametre, v.vide, parametres, ("volume", k));
    });
    ui.label(RichText::new("Faces verrouillées (plan + décalage)").weak());
    egui::Grid::new(("faces", k)).num_columns(3).spacing([6.0, 4.0]).show(ui, |ui| {
        for axe in AxePlan::TOUS {
            let i = axe as usize;
            for (cote, face) in [("min", &mut v.min[i]), ("max", &mut v.max[i])] {
                ui.label(format!("{} {cote}", axe.nom()));
                egui::ComboBox::from_id_salt(("face", k, i, cote)).width(130.0).selected_text(face.plan.as_str()).show_ui(ui, |ui| {
                    for p in plans.iter().filter(|p| p.axe == axe) {
                        ui.selectable_value(&mut face.plan, p.nom.clone(), &p.nom);
                    }
                });
                ui.add(egui::DragValue::new(&mut face.decalage).speed(1.0).dimensions());
                ui.end_row();
            }
        }
    });
}
