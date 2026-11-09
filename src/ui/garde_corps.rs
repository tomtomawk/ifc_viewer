//! Sélecteur de type et élévation paramétrique des garde-corps.
use super::*;
use crate::config::options::{Dimensions, unite};
use crate::modele::elements::garde_corps::{BarreauExtremite, BarreauMotif, Lisse, Section};
use crate::modele::{Projet, Remplissage, TypeGardeCorps, identifiant};
use crate::ui::ruban::Outil;
use glam::DVec2;

pub(super) struct EditeurGardeCorps {
    brouillon: TypeGardeCorps,
    occurrence: Option<String>,
    creation: bool,
    longueur: f64,
    erreur: Option<String>,
}

impl ViewerApp {
    pub(super) fn standard_garde_corps(&self) -> TypeGardeCorps {
        let remplissage = match self
            .ruban_edition
            .texte_option(Outil::GardeCorps, 2)
            .as_str()
        {
            "Panneau plein" => Remplissage::Plein,
            "Vitrage" => Remplissage::Vitre,
            _ => Remplissage::Barreaudage,
        };
        TypeGardeCorps::standard(
            self.ruban_edition.nombre_option(Outil::GardeCorps, 0),
            remplissage,
        )
    }

    pub(super) fn selecteur_garde_corps(&mut self, ui: &mut egui::Ui, ids: &[String]) -> bool {
        let Some((p, _)) = &self.natif else {
            return false;
        };
        let creation = ids.is_empty()
            && self.onglet_ruban == OngletRuban::Modeliser
            && self.ruban_edition.outil() == Some(Outil::GardeCorps);
        let occurrence = match ids {
            [id] => p.garde_corps.iter().find(|g| &g.id == id),
            _ => None,
        };
        if !creation && occurrence.is_none() {
            return false;
        }
        let id = occurrence.map(|g| g.id.clone());
        let choisi = if creation {
            self.ruban_edition.type_garde_corps.clone()
        } else {
            occurrence.and_then(|g| g.type_garde_corps.clone())
        };
        let mut choix = choisi.clone();
        let standard = occurrence.map_or_else(
            || self.standard_garde_corps(),
            |g| TypeGardeCorps::standard(g.hauteur, g.remplissage),
        );
        let type_actuel = choisi
            .as_ref()
            .and_then(|id| p.types_garde_corps.iter().find(|t| &t.id == id));
        ui.heading("Garde-corps");
        egui::ComboBox::from_id_salt("type_garde_corps")
            .width((ui.available_width() - 8.0).max(1.0))
            .truncate()
            .selected_text(type_actuel.map_or("Standard", |t| &t.nom))
            .show_ui(ui, |ui| {
                ui.selectable_value(&mut choix, None, "Standard");
                for t in &p.types_garde_corps {
                    ui.selectable_value(&mut choix, Some(t.id.clone()), &t.nom);
                }
            });
        let t = type_actuel.unwrap_or(&standard);
        elevation(ui, p, t, 1600.0, 65.0, false);
        ui.weak(format!(
            "{} · {}",
            t.remplissage.nom(),
            unite().afficher(t.hauteur)
        ));
        let nouveau = ui.button("Nouveau type…").clicked();
        if choix != choisi {
            if creation {
                self.ruban_edition.type_garde_corps = choix;
            } else if let Some(g) = occurrence {
                let mut q = p.clone();
                let dest = q.garde_corps.iter_mut().find(|x| x.id == g.id).unwrap();
                dest.type_garde_corps = choix.clone();
                if let Some(t) = choix
                    .as_ref()
                    .and_then(|id| p.types_garde_corps.iter().find(|t| &t.id == id))
                {
                    dest.hauteur = t.hauteur;
                    dest.remplissage = t.remplissage;
                }
                if let Err(e) = self.appliquer_projet(q) {
                    self.error = Some(e);
                }
            }
        }
        if nouveau {
            self.ouvrir_type_garde_corps(id.as_deref(), choisi.as_deref(), true);
        }
        true
    }

    pub(super) fn ouvrir_type_garde_corps(
        &mut self,
        occurrence: Option<&str>,
        type_id: Option<&str>,
        nouveau: bool,
    ) {
        let Some((p, _)) = &self.natif else { return };
        let garde = occurrence.and_then(|id| p.garde_corps.iter().find(|g| g.id == id));
        let id = type_id.or_else(|| garde.and_then(|g| g.type_garde_corps.as_deref()));
        let mut brouillon = id
            .and_then(|id| p.types_garde_corps.iter().find(|t| t.id == id))
            .cloned()
            .unwrap_or_else(|| {
                garde.map_or_else(
                    || self.standard_garde_corps(),
                    |g| TypeGardeCorps::standard(g.hauteur, g.remplissage),
                )
            });
        if nouveau || brouillon.id.is_empty() {
            brouillon.id.clear();
            let nom = brouillon.nom.clone();
            brouillon.nom = (1..)
                .map(|n| format!("{nom} {n}"))
                .find(|nom| p.types_garde_corps.iter().all(|t| &t.nom != nom))
                .unwrap();
        }
        let longueur = garde
            .and_then(|g| {
                g.trace
                    .windows(2)
                    .map(|w| DVec2::from_array(w[0]).distance(DVec2::from_array(w[1])))
                    .find(|l| *l > 1e-6)
            })
            .unwrap_or(1600.0);
        self.editeur_garde_corps = Some(EditeurGardeCorps {
            brouillon,
            occurrence: occurrence.map(str::to_owned),
            longueur,
            erreur: None,
            creation: occurrence.is_none() && self.ruban_edition.outil() == Some(Outil::GardeCorps),
        });
    }

    pub(super) fn fenetre_garde_corps(&mut self, ctx: &egui::Context) {
        let (Some(mut ed), Some((p, _))) = (self.editeur_garde_corps.take(), &self.natif) else {
            return;
        };
        let (mut ouvert, mut enregistrer, mut annuler) = (true, false, false);
        let t = &mut ed.brouillon;
        egui::Window::new(if t.id.is_empty() {
            "Nouveau type de garde-corps"
        } else {
            "Modifier le type de garde-corps"
        })
        .id(egui::Id::unique("editeur_garde_corps"))
        .open(&mut ouvert)
        .collapsible(false)
        .default_width(600.0)
        .default_height(640.0)
        .anchor(egui::Align2::CENTER_CENTER, egui::Vec2::ZERO)
        .vscroll(true)
        .show(ctx, |ui| {
            ui.horizontal_wrapped(|ui| {
                ui.label("Nom");
                ui.add(egui::TextEdit::singleline(&mut t.nom).desired_width(240.0));
                egui::ComboBox::from_id_salt("remplissage")
                    .selected_text(t.remplissage.nom())
                    .show_ui(ui, |ui| {
                        for r in Remplissage::TOUS {
                            ui.selectable_value(&mut t.remplissage, r, r.nom());
                        }
                    });
            });
            ui.separator();
            horizontaux(ui, p, t);
            ui.separator();
            ui.horizontal_wrapped(|ui| {
                ui.heading("Élévation");
                ui.label("Longueur du segment");
                ui.add(cote(&mut ed.longueur).range(50.0..=100_000.0));
            });
            elevation(ui, p, t, ed.longueur, 192.0, true);
            ui.separator();
            verticaux(ui, p, t);
            if let Some(e) = &ed.erreur {
                ui.colored_label(ui.visuals().error_fg_color, e);
            }
            ui.horizontal(|ui| {
                enregistrer = ui.button("Enregistrer le type").clicked();
                annuler = ui.button("Annuler").clicked();
            });
        });
        if enregistrer {
            let resultat = (|| {
                t.valider(p)?;
                if p.types_garde_corps
                    .iter()
                    .any(|x| x.id != t.id && x.nom.eq_ignore_ascii_case(t.nom.trim()))
                {
                    return Err("Ce nom de type existe déjà.".into());
                }
                let mut q = p.clone();
                let mut type_enregistre = t.clone();
                type_enregistre.nom = t.nom.trim().to_owned();
                if t.id.is_empty() {
                    type_enregistre.id = (0..)
                        .map(|n| identifiant("type-garde-corps", &format!("{}:{n}", t.nom)))
                        .find(|id| q.types_garde_corps.iter().all(|x| &x.id != id))
                        .unwrap();
                    q.types_garde_corps.push(type_enregistre.clone());
                } else {
                    let dest = q
                        .types_garde_corps
                        .iter_mut()
                        .find(|x| x.id == t.id)
                        .ok_or("Ce type de garde-corps a été supprimé.")?;
                    *dest = type_enregistre.clone();
                }
                if let Some(id) = &ed.occurrence {
                    let garde = q
                        .garde_corps
                        .iter_mut()
                        .find(|g| &g.id == id)
                        .ok_or("Ce garde-corps a été supprimé.")?;
                    garde.type_garde_corps = Some(type_enregistre.id.clone());
                }
                for garde in &mut q.garde_corps {
                    if garde.type_garde_corps.as_ref() == Some(&type_enregistre.id) {
                        garde.hauteur = t.hauteur;
                        garde.remplissage = t.remplissage;
                    }
                }
                Ok((q, type_enregistre.id))
            })()
            .and_then(|(q, id)| {
                self.appliquer_projet(q)?;
                if ed.creation {
                    self.ruban_edition.type_garde_corps = Some(id);
                }
                Ok(())
            });
            match resultat {
                Ok(()) => return,
                Err(e) => ed.erreur = Some(e),
            }
        }
        if ouvert && !annuler {
            self.editeur_garde_corps = Some(ed);
        }
    }
}

fn cote(valeur: &mut f64) -> egui::DragValue<'_> {
    egui::DragValue::new(valeur).speed(1.0).dimensions()
}

fn profils(ui: &mut egui::Ui, p: &Projet, id: &mut Option<String>, standard: &str) {
    let nom = id
        .as_ref()
        .and_then(|id| p.profils.iter().find(|t| &t.id == id))
        .map_or(standard, |p| &p.nom);
    egui::ComboBox::from_id_salt("profil")
        .width(185.0)
        .truncate()
        .selected_text(nom)
        .show_ui(ui, |ui| {
            ui.selectable_value(id, None, standard);
            for profil in &p.profils {
                ui.selectable_value(id, Some(profil.id.clone()), &profil.nom);
            }
        });
}

fn horizontaux(ui: &mut egui::Ui, p: &Projet, t: &mut TypeGardeCorps) {
    ui.heading("Profils horizontaux");
    let mut retirer = None;
    egui::Grid::new("lisses_garde_corps")
        .spacing([12.0, 4.0])
        .show(ui, |ui| {
            ui.label("Élément");
            ui.label("Profil");
            ui.label("Cote depuis le sol");
            ui.label("");
            ui.end_row();
            ui.label("Main courante");
            ui.push_id("main_courante", |ui| {
                profils(ui, p, &mut t.main_courante, "Carré 50 × 50")
            });
            ui.add(cote(&mut t.hauteur))
                .on_hover_text("Dessus de la main courante");
            ui.label("");
            ui.end_row();
            for (i, l) in t.lisses.iter_mut().enumerate() {
                ui.label(format!("Lisse {}", i + 1));
                ui.push_id(("lisse", i), |ui| {
                    profils(ui, p, &mut l.profil, "Carré 40 × 40")
                });
                ui.add(cote(&mut l.altitude))
                    .on_hover_text("Axe de la lisse");
                if ui.small_button("Retirer").clicked() {
                    retirer = Some(i);
                }
                ui.end_row();
            }
        });
    if let Some(i) = retirer {
        t.lisses.remove(i);
    }
    if ui.small_button("Ajouter une lisse").clicked() {
        t.lisses.push(Lisse {
            altitude: t.hauteur / 2.0,
            profil: None,
        });
    }
}

fn verticaux(ui: &mut egui::Ui, p: &Projet, t: &mut TypeGardeCorps) {
    ui.heading("Profils verticaux");
    ui.horizontal_wrapped(|ui| {
        ui.label("Bas depuis le sol");
        ui.add(cote(&mut t.bas));
        ui.label("Jeu depuis le haut");
        ui.add(cote(&mut t.jeu_haut));
    });
    let mut retirer = None;
    egui::Grid::new("barreaux_garde_corps")
        .spacing([12.0, 4.0])
        .show(ui, |ui| {
            ui.label("Élément");
            ui.label("Profil");
            ui.label("Retrait / entraxe");
            ui.label("");
            ui.end_row();
            for (nom, bord) in [("Départ", &mut t.depart), ("Fin", &mut t.arrivee)] {
                let mut actif = bord.is_some();
                if ui.checkbox(&mut actif, nom).changed() {
                    *bord = actif.then_some(BarreauExtremite {
                        retrait: 0.0,
                        profil: None,
                    });
                }
                if let Some(b) = bord {
                    ui.push_id(nom, |ui| profils(ui, p, &mut b.profil, "Carré 50 × 50"));
                    ui.add(cote(&mut b.retrait))
                        .on_hover_text("Distance de l'extrémité à l'axe du montant");
                } else {
                    ui.label("—");
                    ui.label("—");
                }
                ui.label("");
                ui.end_row();
            }
            if t.remplissage == Remplissage::Barreaudage {
                for (i, b) in t.motif.iter_mut().enumerate() {
                    ui.label(format!("Motif · {}", i + 1));
                    ui.push_id(("motif", i), |ui| {
                        profils(ui, p, &mut b.profil, "Carré 20 × 20")
                    });
                    ui.add(cote(&mut b.pas))
                        .on_hover_text("Distance entre l'axe précédent et celui de ce barreau");
                    if ui.small_button("Retirer").clicked() {
                        retirer = Some(i);
                    }
                    ui.end_row();
                }
            }
        });
    if let Some(i) = retirer {
        t.motif.remove(i);
    }
    if t.remplissage == Remplissage::Barreaudage {
        if ui.small_button("Ajouter un barreau au motif").clicked() {
            t.motif.push(BarreauMotif {
                pas: 110.0,
                profil: None,
            });
        }
        ui.weak("Le motif se répète sur chaque segment. La fin garde son retrait.");
    }
}

fn elevation(
    ui: &mut egui::Ui,
    p: &Projet,
    t: &TypeGardeCorps,
    longueur: f64,
    hauteur: f32,
    cotes: bool,
) {
    let (rect, _) = ui.allocate_exact_size(
        egui::vec2(ui.available_width(), hauteur),
        egui::Sense::hover(),
    );
    let peintre = ui.painter_at(rect);
    peintre.rect_filled(rect, 4.0, ui.visuals().extreme_bg_color);
    if longueur <= 0.0 || t.hauteur <= 0.0 || !longueur.is_finite() || !t.hauteur.is_finite() {
        return;
    }
    let marge = if cotes { 24.0 } else { 6.0 };
    let zone = rect.shrink2(egui::vec2(marge, if cotes { 18.0 } else { 5.0 }));
    let s = (zone.width() / longueur as f32).min(zone.height() / t.hauteur as f32);
    let origine = egui::pos2(
        zone.center().x - longueur as f32 * s / 2.0,
        zone.center().y + t.hauteur as f32 * s / 2.0,
    );
    let point = |x: f64, z: f64| origine + egui::vec2(x as f32 * s, -z as f32 * s);
    let metal = ui.visuals().text_color();
    let accent = ui.visuals().selection.stroke.color;
    let surface = |x0, x1, z0, z1, couleur| {
        peintre.rect_filled(
            egui::Rect::from_two_pos(point(x0, z0), point(x1, z1)),
            0.0,
            couleur,
        );
    };
    let haut = t.hauteur - t.jeu_haut;
    if t.remplissage != Remplissage::Barreaudage {
        surface(
            0.0,
            longueur,
            t.bas,
            haut,
            if t.remplissage == Remplissage::Vitre {
                Color32::from_rgba_unmultiplied(64, 164, 212, 105)
            } else {
                ui.visuals().widgets.noninteractive.bg_stroke.color
            },
        );
    }
    if let Ok(barreaux) = t.barreaux(longueur) {
        for b in barreaux {
            let (bas, carre) = if b.extremite {
                (0.0, 50.0)
            } else {
                (t.bas, 20.0)
            };
            if let Ok(section) = Section::lire(p, b.profil, carre) {
                let demi = (section.largeur / 2.0).max(0.4 / s as f64);
                surface(
                    b.x - demi,
                    b.x + demi,
                    bas,
                    haut,
                    if b.extremite { accent } else { metal },
                );
            }
        }
    }
    if let Ok(main) = Section::lire(p, t.main_courante.as_deref(), 50.0) {
        surface(0.0, longueur, t.hauteur - main.hauteur, t.hauteur, metal);
    }
    for l in &t.lisses {
        if let Ok(section) = Section::lire(p, l.profil.as_deref(), 40.0) {
            let demi = (section.hauteur / 2.0).max(0.4 / s as f64);
            surface(0.0, longueur, l.altitude - demi, l.altitude + demi, metal);
        }
    }
    if cotes {
        let gris = ui.visuals().weak_text_color();
        let ligne = egui::Stroke::new(1.0, gris);
        let police = egui::FontId::proportional(11.0);
        let a = point(0.0, 0.0) + egui::vec2(0.0, 5.0);
        let b = point(longueur, 0.0) + egui::vec2(0.0, 5.0);
        peintre.line_segment([a, b], ligne);
        for q in [a, b] {
            peintre.line_segment([q - egui::vec2(0.0, 3.0), q + egui::vec2(0.0, 3.0)], ligne);
        }
        peintre.text(
            a.lerp(b, 0.5) + egui::vec2(0.0, 2.0),
            egui::Align2::CENTER_TOP,
            unite().afficher(longueur),
            police.clone(),
            gris,
        );
        peintre.text(
            point(longueur, t.hauteur / 2.0) + egui::vec2(9.0, 0.0),
            egui::Align2::LEFT_CENTER,
            unite().afficher(t.hauteur),
            police,
            gris,
        );
    }
}
