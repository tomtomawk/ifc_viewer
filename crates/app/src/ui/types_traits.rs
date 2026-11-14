//! Types de trait du dessin 2D : choix du type de chaque segment d'une polyligne, et
//! fenêtre de gestion des types du projet (nom, épaisseur, tirets en mm du papier).
use super::*;
use super::communes::melange;
use glam::DVec2;
use crate::modele::Projet;
use crate::modele::documentation::{AnnotationForme, TypeTrait};

/// Aperçu d'un type de trait dans `r` : épaisseur et tirets à l'échelle de l'écran
/// (1 mm du papier ≈ 4 points).
pub(super) fn apercu_trait(ui: &egui::Ui, r: egui::Rect, t: &TypeTrait) {
    let couleur = ui.visuals().text_color();
    let trait_ = egui::Stroke::new((t.epaisseur as f32 * 4.0).clamp(0.6, 6.0), couleur);
    let (a, b) = (DVec2::new(r.left() as f64, r.center().y as f64), DVec2::new(r.right() as f64, r.center().y as f64));
    let mut phase = 0.0;
    for [p, q] in t.decouper(a, b, 4.0, &mut phase) {
        ui.painter().line_segment([egui::pos2(p.x as f32, p.y as f32), egui::pos2(q.x as f32, q.y as f32)], trait_);
    }
}

/// Liste de choix d'un type de trait (id ; vide : continu fin). `actuel` None : varié.
/// Retourne l'id choisi.
pub(in crate::ui) fn choix_type_trait(ui: &mut egui::Ui, salt: impl std::hash::Hash + std::fmt::Debug, actuel: Option<&str>, types: &[TypeTrait]) -> Option<String> {
    let standard = TypeTrait::standard();
    let courant = actuel.map(|id| types.iter().find(|t| t.id == id).unwrap_or(&standard));
    let mut choix = None;
    ui.horizontal(|ui| {
        let (r, _) = ui.allocate_exact_size(egui::vec2(36.0, 16.0), Sense::hover());
        if let Some(t) = courant {
            apercu_trait(ui, r, t);
        }
        egui::ComboBox::from_id_salt(salt).width(150.0).truncate().selected_text(courant.map_or("Varié".to_string(), |t| t.nom.clone())).show_ui(ui, |ui| {
            ui.set_min_width(220.0);
            // le continu fin standard (segment sans type) n'est proposé qu'à défaut de types
            let liste: Vec<&TypeTrait> = if types.is_empty() { vec![&standard] } else { types.iter().collect() };
            for t in liste {
                let (r, rep) = ui.allocate_exact_size(egui::vec2(ui.available_width(), 22.0), Sense::click());
                let actif = courant.is_some_and(|c| c.id == t.id);
                if actif || rep.hovered() {
                    ui.painter().rect_filled(r, 3.0, if actif { ui.visuals().selection.bg_fill } else { ui.visuals().widgets.hovered.bg_fill });
                }
                apercu_trait(ui, egui::Rect::from_min_size(r.min + egui::vec2(6.0, 3.0), egui::vec2(44.0, 16.0)), t);
                ui.painter().text(r.min + egui::vec2(58.0, 11.0), egui::Align2::LEFT_CENTER, &t.nom, egui::FontId::proportional(12.0), ui.visuals().text_color());
                ui.painter().text(r.right_center() - egui::vec2(6.0, 0.0), egui::Align2::RIGHT_CENTER, format!("{:.2} mm", t.epaisseur), egui::FontId::proportional(11.0), ui.visuals().weak_text_color());
                if rep.clicked() {
                    choix = Some(t.id.clone());
                    ui.close();
                }
            }
        });
    });
    choix
}

/// Nombre de segments d'une polyligne.
pub(super) fn segments_polyligne(points: &[[f64; 2]], fermee: bool) -> usize {
    points.len().saturating_sub(1) + usize::from(fermee && points.len() > 2)
}

const FENETRE: &str = "fenetre_types_traits";

impl ViewerApp {
    /// Message de confirmation en bas de la vue (quelques secondes).
    pub(in crate::ui) fn confirmer(&mut self, texte: String) {
        self.confirmation = Some((texte, std::time::Instant::now()));
    }

    /// Types de trait des polylignes sélectionnées : un choix pour tous leurs segments,
    /// puis, pour une seule polyligne, un choix par segment. Retourne le projet modifié.
    pub(super) fn traits_polylignes(&self, ui: &mut egui::Ui, projet: &Projet, ids: &[String]) -> Option<Projet> {
        let polylignes: Vec<(&str, Vec<[f64; 2]>, bool, Vec<String>, Vec<f64>)> = projet
            .annotations
            .iter()
            .filter(|a| ids.contains(&a.id))
            .filter_map(|a| match &a.forme {
                AnnotationForme::Polyligne { points, fermee, traits, courbures, .. } => Some((a.id.as_str(), points.clone(), *fermee, traits.clone(), courbures.clone())),
                _ => None,
            })
            .collect();
        if polylignes.is_empty() {
            return None;
        }
        // type de chaque segment (vide : continu fin)
        let tous: Vec<String> = polylignes.iter().flat_map(|(_, pts, f, t, _)| (0..segments_polyligne(pts, *f)).map(move |k| t.get(k).cloned().unwrap_or_default())).collect();
        let (premier, mixte) = melange(tous.iter().cloned());
        let mut changement: Option<(Option<usize>, String)> = None;
        egui::Grid::new("traits_polylignes").num_columns(2).striped(true).spacing([10.0, 4.0]).show(ui, |ui| {
            ui.label("Type de trait").on_hover_text("Type de trait de tous les segments (épaisseur et tirets)");
            if let Some(id) = choix_type_trait(ui, "trait_tous", if mixte { None } else { premier.as_deref() }, &projet.types_traits) {
                changement = Some((None, id));
            }
            ui.end_row();
        });
        if let [(_, pts, fermee, traits, courbures)] = polylignes.as_slice() {
            let n = segments_polyligne(pts, *fermee);
            egui::CollapsingHeader::new(format!("Segments ({n})")).id_salt("segments_polyligne").default_open(n <= 12).show(ui, |ui| {
                egui::Grid::new("traits_segments").num_columns(2).striped(true).spacing([10.0, 3.0]).show(ui, |ui| {
                    for k in 0..n {
                        let (a, b) = (DVec2::from_array(pts[k]), DVec2::from_array(pts[(k + 1) % pts.len()]));
                        // segment d'arc : longueur développée
                        let arc = crate::dessin::dao::arc_courbure(a, b, courbures.get(k).copied().unwrap_or(0.0));
                        let longueur = arc.map_or(a.distance(b), |(_, r, _, s)| r * s.abs());
                        ui.label(format!("{} · {}{}", k + 1, crate::config::options::unite().afficher(longueur.round()), if arc.is_some() { " · arc" } else { "" }))
                            .on_hover_text("Segment, depuis le premier point de la polyligne ; longueur");
                        if let Some(id) = choix_type_trait(ui, ("trait_segment", k), Some(traits.get(k).map_or("", |s| s.as_str())), &projet.types_traits) {
                            changement = Some((Some(k), id));
                        }
                        ui.end_row();
                    }
                });
            });
        }
        if ui.button("Types de trait…").on_hover_text("Créer ou modifier les types de trait du projet").clicked() {
            ouvrir_fenetre_types(ui.ctx(), projet);
        }
        let (segment, id) = changement?;
        let mut q = projet.clone();
        for a in q.annotations.iter_mut().filter(|a| ids.contains(&a.id)) {
            if let AnnotationForme::Polyligne { points, fermee, traits, .. } = &mut a.forme {
                let n = segments_polyligne(points, *fermee);
                traits.resize(n, String::new());
                match segment {
                    Some(k) if k < n => traits[k] = id.clone(),
                    Some(_) => {}
                    None => traits.iter_mut().for_each(|t| *t = id.clone()),
                }
                // tout en continu fin : rien à enregistrer
                if traits.iter().all(String::is_empty) {
                    traits.clear();
                }
            }
        }
        Some(q)
    }

    /// Fenêtre des types de trait du projet : nom, épaisseur, tirets, aperçu.
    pub(super) fn fenetre_types_traits(&mut self, ctx: &egui::Context) {
        let id = egui::Id::unique(FENETRE);
        type Etat = (Vec<TypeTrait>, Vec<String>, Option<String>);
        let Some((mut types, mut tirets, mut erreur)) = ctx.data(|d| d.get_temp::<Etat>(id)) else { return };
        let (mut ouvert, mut enregistrer) = (true, false);
        egui::Window::new("Types de trait").open(&mut ouvert).default_width(560.0).resizable(true).show(ctx, |ui| {
            ui.label(RichText::new("Épaisseur et tirets en mm du papier : même aspect à toutes les échelles. Tirets : longueurs trait, blanc, trait, blanc… (vide : continu).").small().weak());
            let mut supprimer = None;
            egui::Grid::new("grille_types_traits").num_columns(5).striped(true).spacing([8.0, 4.0]).show(ui, |ui| {
                for t in ["Nom", "Épaisseur", "Tirets", "Aperçu", ""] {
                    ui.label(RichText::new(t).small().weak());
                }
                ui.end_row();
                for (k, (t, texte)) in types.iter_mut().zip(tirets.iter_mut()).enumerate() {
                    ui.add_sized([150.0, 20.0], egui::TextEdit::singleline(&mut t.nom));
                    ui.add(egui::DragValue::new(&mut t.epaisseur).speed(0.01).range(0.05..=5.0).max_decimals(2).suffix(" mm"));
                    if ui.add_sized([110.0, 20.0], egui::TextEdit::singleline(texte).hint_text("continu")).changed() {
                        t.tirets = texte.split([' ', ';']).filter(|x| !x.is_empty()).filter_map(|x| x.replace(',', ".").parse().ok()).collect();
                    }
                    let (r, _) = ui.allocate_exact_size(egui::vec2(90.0, 18.0), Sense::hover());
                    apercu_trait(ui, r, t);
                    if ui.small_button("Supprimer").on_hover_text("Supprimer le type : ses segments passent en continu fin").clicked() {
                        supprimer = Some(k);
                    }
                    ui.end_row();
                }
            });
            if let Some(k) = supprimer {
                types.remove(k);
                tirets.remove(k);
            }
            ui.horizontal(|ui| {
                if ui.button("+ Nouveau type").clicked() {
                    let nom = (1..).map(|k| format!("Trait {k}")).find(|n| !types.iter().any(|t| &t.nom == n)).unwrap();
                    types.push(TypeTrait { id: String::new(), nom, epaisseur: 0.25, tirets: Vec::new() });
                    tirets.push(String::new());
                }
            });
            if let Some(e) = &erreur {
                ui.colored_label(ui.visuals().error_fg_color, e);
            }
            ui.separator();
            if ui.button("Enregistrer").clicked() {
                enregistrer = true;
            }
        });
        if enregistrer {
            match self.enregistrer_types_traits(types.clone()) {
                Ok(()) => {
                    ctx.data_mut(|d| d.remove::<Etat>(id));
                    return;
                }
                Err(e) => erreur = Some(e),
            }
        }
        if ouvert {
            ctx.data_mut(|d| d.insert_temp(id, (types, tirets, erreur)));
        } else {
            ctx.data_mut(|d| d.remove::<Etat>(id));
        }
    }

    /// Remplace les types de trait du projet ; les segments d'un type supprimé passent
    /// en continu fin.
    fn enregistrer_types_traits(&mut self, mut types: Vec<TypeTrait>) -> Result<(), String> {
        let (p, _) = self.natif.as_ref().ok_or("Aucun projet natif.")?;
        let mut noms = std::collections::HashSet::new();
        for t in &mut types {
            t.nom = t.nom.trim().to_string();
            t.valider()?;
            if !noms.insert(t.nom.clone()) {
                return Err(format!("Deux types s'appellent « {} ».", t.nom));
            }
            if t.id.is_empty() {
                t.id = format!("trait-{}", crate::modele::collaboration::uuid());
            }
        }
        let mut q = p.clone();
        for a in &mut q.annotations {
            if let AnnotationForme::Polyligne { traits, .. } = &mut a.forme {
                traits.iter_mut().filter(|t| !t.is_empty() && !types.iter().any(|x| &x.id == *t)).for_each(|t| t.clear());
                if traits.iter().all(String::is_empty) {
                    traits.clear();
                }
            }
        }
        q.types_traits = types;
        self.appliquer_projet(q)?;
        self.status = "Types de trait enregistrés.".into();
        self.confirmer("Types de trait enregistrés".into());
        Ok(())
    }
}

/// Ouvre la fenêtre des types de trait sur ceux du projet.
pub(super) fn ouvrir_fenetre_types(ctx: &egui::Context, projet: &Projet) {
    let types = projet.types_traits.clone();
    let tirets: Vec<String> = types.iter().map(|t| t.tirets.iter().map(|x| format!("{x}")).collect::<Vec<_>>().join(" ")).collect();
    ctx.data_mut(|d| d.insert_temp(egui::Id::unique(FENETRE), (types, tirets, None::<String>)));
}
