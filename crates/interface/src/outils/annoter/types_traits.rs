//! Types de trait du dessin 2D : choix du type de chaque segment d'une polyligne, et
//! fenêtre de gestion des types du projet (nom, épaisseur, tirets en mm du papier).
use crate::*;
use crate::zones::communes::melange;
use glam::DVec2;
use modele::Projet;
use modele::documentation::{AnnotationForme, TypeTrait};

/// Aperçu d'un type de trait dans `r` : épaisseur et tirets à l'échelle de l'écran
/// (1 mm du papier ≈ 4 points).
pub(crate) fn apercu_trait(ui: &egui::Ui, r: egui::Rect, t: &TypeTrait) {
    apercu_trait_peintre(ui.painter(), r, ui.visuals().text_color(), t);
}

fn apercu_trait_peintre(p: &egui::Painter, r: egui::Rect, couleur: egui::Color32, t: &TypeTrait) {
    let trait_ = egui::Stroke::new((t.epaisseur as f32 * 4.0).clamp(0.6, 6.0), couleur);
    let (a, b) = (DVec2::new(r.left() as f64, r.center().y as f64), DVec2::new(r.right() as f64, r.center().y as f64));
    let mut phase = 0.0;
    for [p0, q] in t.decouper(a, b, 4.0, &mut phase) {
        p.line_segment([egui::pos2(p0.x as f32, p0.y as f32), egui::pos2(q.x as f32, q.y as f32)], trait_);
    }
}

/// Sélecteur standard d'un type de trait : en-tête (aperçu, nom, épaisseur) et liste, comme les
/// sélecteurs de type des autres objets. `actuel` : id du type (vide : continu fin) ; None :
/// types variés. `objet` : ce dont c'est le trait (« Polyligne », « Polyligne · trait 3 »).
/// Retourne l'id choisi.
pub(crate) fn selecteur_type_trait(ui: &mut egui::Ui, actuel: Option<&str>, projet: &Projet, objet: &str) -> Option<String> {
    use crate::zones::ruban::selecteur;
    let standard = TypeTrait::standard();
    let types: Vec<&TypeTrait> = if projet.types_traits.is_empty() { vec![&standard] } else { projet.types_traits.iter().collect() };
    let courant = actuel.map(|id| types.iter().copied().find(|t| t.id == id).unwrap_or(&standard));
    let titre = format!("{objet} · {}", courant.map_or("Types variés", |t| t.nom.as_str()));
    let detail = courant.map(|t| format!("{:.2} mm", t.epaisseur));
    let vignette = |p: &egui::Painter, r: egui::Rect, c: egui::Color32| {
        if let Some(t) = courant {
            apercu_trait_peintre(p, r.shrink2(egui::vec2(2.0, 6.0)), c, t);
        }
    };
    let bouton = selecteur::entete(ui, &titre, detail.as_deref(), "Type de trait (épaisseur et tirets en mm du papier)", Some(&vignette));
    let mut choix = None;
    let mut gerer = false;
    let largeur = selecteur::largeur_liste(ui, types.iter().map(|t| t.nom.as_str()), 90.0, 0, 0.0);
    selecteur::liste(&bouton, largeur, selecteur::HAUTEUR_CADRE + types.len() as f32 * selecteur::HAUTEUR_LIGNE, |ui| {
        for t in &types {
            let l = selecteur::Ligne { nom: &t.nom, courant: courant.is_some_and(|c| c.id == t.id), etiquette: None, sous_titre: None, detail: Some(format!("{:.2} mm", t.epaisseur)), actions: &[] };
            let mut v = |ui: &mut egui::Ui, r: egui::Rect| apercu_trait(ui, egui::Rect::from_center_size(r.center(), egui::vec2(r.width().min(40.0), 16.0)), t);
            if selecteur::ligne(ui, &l, Some(&mut v)).choisi {
                choix = Some(t.id.clone());
                ui.close();
            }
        }
        gerer = selecteur::ligne_ajout(ui, "Gérer les types de trait…");
    });
    if gerer {
        ouvrir_fenetre_types(ui.ctx(), projet);
    }
    choix
}

/// Liste de choix d'un type de trait (id ; vide : continu fin). `actuel` None : varié.
/// Retourne l'id choisi.
pub(crate) fn choix_type_trait(ui: &mut egui::Ui, salt: impl std::hash::Hash + std::fmt::Debug, actuel: Option<&str>, types: &[TypeTrait]) -> Option<String> {
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
pub(crate) fn segments_polyligne(points: &[[f64; 2]], fermee: bool) -> usize {
    points.len().saturating_sub(1) + usize::from(fermee && points.len() > 2)
}

/// Type de trait de chaque segment d'une polyligne (vide : continu fin) ; projet et familles.
pub(crate) fn traits_segments(forme: &AnnotationForme) -> Vec<String> {
    match forme {
        AnnotationForme::Polyligne { points, fermee, traits, .. } => (0..segments_polyligne(points, *fermee)).map(|k| traits.get(k).cloned().unwrap_or_default()).collect(),
        // spline, cercle : un seul type pour toute la courbe
        AnnotationForme::Spline { type_trait, .. } | AnnotationForme::Cercle { type_trait, .. } => vec![type_trait.clone()],
        _ => Vec::new(),
    }
}

/// Forme qui porte un type de trait : polyligne, spline, cercle.
pub(crate) fn a_un_trait(forme: &AnnotationForme) -> bool {
    matches!(forme, AnnotationForme::Polyligne { .. } | AnnotationForme::Spline { .. } | AnnotationForme::Cercle { .. })
}

/// Donne le type `id` à un segment (`segment`) ou à tous ceux d'une polyligne, ou à une
/// spline ou un cercle entier ; projet et familles. Tout en continu fin : rien d'enregistré.
pub(crate) fn definir_trait(forme: &mut AnnotationForme, segment: Option<usize>, id: &str) {
    if let AnnotationForme::Spline { type_trait, .. } | AnnotationForme::Cercle { type_trait, .. } = forme {
        if segment.is_none_or(|k| k == 0) { *type_trait = id.to_string(); }
        return;
    }
    let AnnotationForme::Polyligne { points, fermee, traits, .. } = forme else { return };
    let n = segments_polyligne(points, *fermee);
    traits.resize(n, String::new());
    match segment {
        Some(k) if k < n => traits[k] = id.to_string(),
        Some(_) => {}
        None => traits.iter_mut().for_each(|t| *t = id.to_string()),
    }
    if traits.iter().all(String::is_empty) {
        traits.clear();
    }
}

const FENETRE: &str = "fenetre_types_traits";

/// Polyligne vue par l'éditeur de types de trait : id, sommets, fermée, types par
/// segment, courbures.
type PolyligneTraits<'a> = (&'a str, Vec<[f64; 2]>, bool, Vec<String>, Vec<f64>);

impl Application {
    /// Types de trait des polylignes sélectionnées : un choix pour tous leurs segments,
    /// puis, pour une seule polyligne, un choix par segment. Retourne le projet modifié.
    fn polylignes_de<'a>(projet: &'a Projet, ids: &[String]) -> Vec<PolyligneTraits<'a>> {
        projet
            .annotations
            .iter()
            .filter(|a| ids.contains(&a.id))
            .filter_map(|a| match &a.forme {
                AnnotationForme::Polyligne { points, fermee, traits, courbures, .. } => Some((a.id.as_str(), points.clone(), *fermee, traits.clone(), courbures.clone())),
                _ => None,
            })
            .collect()
    }

    /// Projet dont les segments choisis (`segment` : un seul, sinon tous) des polylignes de `ids`
    /// ont le type de trait `id`.
    fn avec_trait(projet: &Projet, ids: &[String], segment: Option<usize>, id: &str) -> Projet {
        let mut q = projet.clone();
        q.annotations.iter_mut().filter(|a| ids.contains(&a.id)).for_each(|a| definir_trait(&mut a.forme, segment, id));
        q
    }

    /// Emplacement « sélecteur » d'une sélection de dessins 2D : type de trait de tous les
    /// segments des polylignes, des splines et des cercles (« Types variés » s'ils
    /// diffèrent). `None` : aucun de ces tracés.
    /// `Some(projet)` si un type est choisi.
    pub(crate) fn selecteur_traits_polylignes(&self, ui: &mut egui::Ui, projet: &Projet, ids: &[String]) -> Option<Option<Projet>> {
        let traces: Vec<&AnnotationForme> = projet.annotations.iter().filter(|a| ids.contains(&a.id) && a_un_trait(&a.forme)).map(|a| &a.forme).collect();
        if traces.is_empty() {
            return None;
        }
        let tous: Vec<String> = traces.iter().flat_map(|f| traits_segments(f)).collect();
        let (premier, mixte) = melange(tous.iter().cloned());
        let nom = |f: &AnnotationForme| match f { AnnotationForme::Spline { .. } => "Spline", AnnotationForme::Cercle { .. } => "Cercle", _ => "Polyligne" };
        let objet = match traces.as_slice() {
            [f] => nom(f).to_string(),
            [f, reste @ ..] if reste.iter().all(|g| nom(g) == nom(f)) => format!("{}s ({})", nom(f), traces.len()),
            _ => format!("Tracés ({})", traces.len()),
        };
        let choix = selecteur_type_trait(ui, if mixte { None } else { premier.as_deref() }, projet, &objet);
        Some(choix.map(|id| Self::avec_trait(projet, ids, None, &id)))
    }

    /// Emplacement « schéma » : type de trait de chaque segment d'une polyligne seule.
    pub(crate) fn segments_polylignes(&self, ui: &mut egui::Ui, projet: &Projet, ids: &[String]) -> Option<Projet> {
        let polylignes = Self::polylignes_de(projet, ids);
        let [(_, pts, fermee, traits, courbures)] = polylignes.as_slice() else { return None };
        let mut changement: Option<(usize, String)> = None;
        let n = segments_polyligne(pts, *fermee);
        egui::CollapsingHeader::new(format!("Segments ({n})")).id_salt("segments_polyligne").default_open(n <= 12).show(ui, |ui| {
            egui::Grid::new("traits_segments").num_columns(2).striped(true).spacing([10.0, 3.0]).show(ui, |ui| {
                for k in 0..n {
                    let (a, b) = (DVec2::from_array(pts[k]), DVec2::from_array(pts[(k + 1) % pts.len()]));
                    // segment d'arc : longueur développée
                    let arc = noyau::forme2d::arc_courbure(a, b, courbures.get(k).copied().unwrap_or(0.0));
                    let longueur = arc.map_or(a.distance(b), |(_, r, _, s)| r * s.abs());
                    ui.label(format!("{} · {}{}", k + 1, modele::unite::unite().afficher(longueur.round()), if arc.is_some() { " · arc" } else { "" }))
                        .on_hover_text("Segment, depuis le premier point de la polyligne ; longueur");
                    if let Some(id) = choix_type_trait(ui, ("trait_segment", k), Some(traits.get(k).map_or("", |s| s.as_str())), &projet.types_traits) {
                        changement = Some((k, id));
                    }
                    ui.end_row();
                }
            });
        });
        let (k, id) = changement?;
        Some(Self::avec_trait(projet, ids, Some(k), &id))
    }

    /// Fenêtre des types de trait du projet : nom, épaisseur, tirets, aperçu.
    pub(crate) fn fenetre_types_traits(&mut self, ctx: &egui::Context) {
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
                    let nom = (1..).map(|k| format!("Trait {k}")).find(|n| !types.iter().any(|t| &t.nom == n)).unwrap_or_default();
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
                t.id = format!("trait-{}", noyau::uuid::uuid());
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
        self.informer("Types de trait enregistrés");
        Ok(())
    }
}

/// Ouvre la fenêtre des types de trait sur ceux du projet.
pub(crate) fn ouvrir_fenetre_types(ctx: &egui::Context, projet: &Projet) {
    let types = projet.types_traits.clone();
    let tirets: Vec<String> = types.iter().map(|t| t.tirets.iter().map(|x| format!("{x}")).collect::<Vec<_>>().join(" ")).collect();
    ctx.data_mut(|d| d.insert_temp(egui::Id::unique(FENETRE), (types, tirets, None::<String>)));
}
