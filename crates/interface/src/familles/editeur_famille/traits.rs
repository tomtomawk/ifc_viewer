//! Types de trait des polylignes de famille, comme dans le projet : type de création
//! (outils Polyligne et Rectangle), sélecteur des polylignes sélectionnées (« Types
//! variés » s'ils diffèrent), type de chaque segment ; pendant « Modifier », le segment
//! cliqué dans la vue et son seul type.
use super::*;
use crate::outils::annoter::types_traits::{choix_type_trait, segments_polyligne, selecteur_type_trait};
use glam::DVec2;
use modele::documentation::AnnotationForme;

/// Consigne de l'outil « Modifier ».
pub(super) const INVITE: &str = "Modifier : glisser un sommet ou un milieu d'arête, clic droit sur un sommet pour le retirer, cliquer un segment pour choisir son type de trait ; Échap pour terminer.";
/// Raison d'indisponibilité de « Modifier ».
pub(super) const SANS_TRACE: &str = "Sélectionner un seul tracé de la vue : polyligne, arc, cercle, spline ou profil d'une forme.";

/// Type de trait de chaque segment d'une polyligne (vide : continu fin).
fn traits_segments(forme: &AnnotationForme) -> Vec<String> {
    match forme {
        AnnotationForme::Polyligne { points, fermee, traits, .. } => (0..segments_polyligne(points, *fermee)).map(|k| traits.get(k).cloned().unwrap_or_default()).collect(),
        _ => Vec::new(),
    }
}

/// Donne le type `id` à un segment (`segment`) ou à tous ceux d'une polyligne.
fn definir_trait(forme: &mut AnnotationForme, segment: Option<usize>, id: &str) {
    let AnnotationForme::Polyligne { points, fermee, traits, .. } = forme else { return };
    let n = segments_polyligne(points, *fermee);
    traits.resize(n, String::new());
    match segment {
        Some(k) if k < n => traits[k] = id.to_string(),
        Some(_) => {}
        None => traits.iter_mut().for_each(|t| *t = id.to_string()),
    }
    // tout en continu fin : rien à enregistrer
    if traits.iter().all(String::is_empty) {
        traits.clear();
    }
}

impl EditeurFamille {
    /// Sélectionne le dernier élément 2D créé ; une polyligne prend le type de trait de
    /// création.
    pub(super) fn element_2d_cree(&mut self) {
        let Some(k) = self.famille.elements_2d.len().checked_sub(1) else { return };
        if !self.trait_creation.is_empty() {
            let id = self.trait_creation.clone();
            definir_trait(&mut self.famille.elements_2d[k].forme, None, &id);
        }
        self.selection = SelectionFamille::Element2D(k);
    }

    /// « Modifier » s'applique : un seul élément sélectionné, tracé de la vue ouverte
    /// (élément 2D dessiné, ou profil d'une forme non posée sur un plan).
    pub(super) fn trace_modifiable(&self) -> bool {
        let vue = self.vue.esquisse();
        self.multiple.is_empty()
            && match self.selection {
                SelectionFamille::Element2D(k) => self.famille.elements_2d.get(k).is_some_and(|e| e.vue == vue && e.forme.forme_2d().is_some()),
                SelectionFamille::Extrusion(k) => self.famille.extrusions.get(k).is_some_and(|e| e.vue == vue && e.support.is_none()),
                _ => false,
            }
    }

    /// Segments à l'écran de la polyligne modifiée (points développés des arcs).
    fn segments_ecran(&self, cadre: &Cadre, r: &Resolution) -> Vec<Vec<egui::Pos2>> {
        if self.outil != OutilFamille::ModifierTrace {
            return Vec::new();
        }
        let SelectionFamille::Element2D(k) = self.selection else { return Vec::new() };
        let Some(e) = self.famille.elements_2d.get(k).filter(|e| e.vue == self.vue.esquisse()) else { return Vec::new() };
        let AnnotationForme::Polyligne { points, fermee, courbures, .. } = self.famille.element_resolu(r, e).forme else { return Vec::new() };
        let pts: Vec<DVec2> = points.iter().map(|p| DVec2::from_array(*p)).collect();
        noyau::forme2d::segments_polyligne(&pts, fermee, &courbures)
            .into_iter()
            .map(|(a, b, c)| noyau::forme2d::points_segment(a, b, c).into_iter().map(|q| cadre.ecran(q.x, q.y)).collect())
            .collect()
    }
}

/// Clic dans la vue pendant « Modifier » : choisit le segment visé de la polyligne (ou
/// aucun) ; la sélection ne change pas. Faux hors de « Modifier ».
pub(super) fn clic_segment(ed: &mut EditeurFamille, cadre: &Cadre, r: &Resolution, q: Option<egui::Pos2>) -> bool {
    if ed.outil != OutilFamille::ModifierTrace {
        return false;
    }
    let Some(q) = q else { return true };
    let segments = ed.segments_ecran(cadre, r);
    ed.segment_trace = segments
        .iter()
        .enumerate()
        .map(|(i, s)| (i, s.windows(2).map(|w| panneau::distance_segment(q, w[0], w[1])).fold(f32::MAX, f32::min)))
        .filter(|(_, d)| *d < 6.0)
        .min_by(|a, b| a.1.total_cmp(&b.1))
        .map(|(i, _)| i);
    true
}

/// Segment choisi pendant « Modifier », surligné.
pub(super) fn peindre_segment(painter: &egui::Painter, ed: &EditeurFamille, cadre: &Cadre, r: &Resolution) {
    let Some(i) = ed.segment_trace else { return };
    if let Some(s) = ed.segments_ecran(cadre, r).get(i) {
        painter.add(egui::Shape::line(s.clone(), egui::Stroke::new(3.0, Color32::from_rgb(30, 120, 230))));
    }
}

/// Propriétés : type de trait de création (Polyligne, Rectangle), ou des polylignes
/// sélectionnées et de leurs segments.
pub(super) fn proprietes_traits(ui: &mut egui::Ui, ed: &mut EditeurFamille, projet: &modele::Projet) {
    if matches!(ed.outil, OutilFamille::Ligne | OutilFamille::Rectangle) {
        if !projet.types_traits.iter().any(|t| t.id == ed.trait_creation) {
            ed.trait_creation.clear();
        }
        if let Some(id) = selecteur_type_trait(ui, Some(&ed.trait_creation), projet, "Polyligne") {
            ed.trait_creation = id;
        }
        ui.separator();
        return;
    }
    if !matches!(ed.outil, OutilFamille::Selection | OutilFamille::ModifierTrace) {
        return;
    }
    let indices: Vec<usize> = ed
        .selectionnes()
        .into_iter()
        .filter_map(|s| match s {
            SelectionFamille::Element2D(k) => Some(k),
            _ => None,
        })
        .filter(|k| matches!(ed.famille.elements_2d.get(*k).map(|e| &e.forme), Some(AnnotationForme::Polyligne { .. })))
        .collect();
    if indices.is_empty() {
        return;
    }
    let segment = ed.segment_trace.filter(|_| ed.outil == OutilFamille::ModifierTrace && indices.len() == 1);
    let tous: Vec<String> = indices.iter().flat_map(|k| traits_segments(&ed.famille.elements_2d[*k].forme)).collect();
    let (actuel, objet) = match segment {
        Some(i) => (Some(tous.get(i).cloned().unwrap_or_default()), format!("Polyligne · trait {}", i + 1)),
        None => {
            let (premier, mixte) = crate::zones::communes::melange(tous.iter().cloned());
            let objet = if indices.len() > 1 { format!("Polylignes ({})", indices.len()) } else { "Polyligne".to_string() };
            ((!mixte).then(|| premier.unwrap_or_default()), objet)
        }
    };
    if let Some(id) = selecteur_type_trait(ui, actuel.as_deref(), projet, &objet) {
        for k in &indices {
            definir_trait(&mut ed.famille.elements_2d[*k].forme, segment, &id);
        }
    }
    // une seule polyligne : type de chaque segment, avec sa longueur
    if let [k] = indices[..] {
        let r = ed.resoudre();
        let resolu = ed.famille.element_resolu(&r, &ed.famille.elements_2d[k]);
        let AnnotationForme::Polyligne { points, fermee, courbures, .. } = &resolu.forme else { return };
        let pts: Vec<DVec2> = points.iter().map(|p| DVec2::from_array(*p)).collect();
        let segments = noyau::forme2d::segments_polyligne(&pts, *fermee, courbures);
        let mut changement = None;
        egui::CollapsingHeader::new(format!("Segments ({})", segments.len())).id_salt(("segments_famille", k)).default_open(segments.len() <= 12).show(ui, |ui| {
            egui::Grid::new(("traits_segments_famille", k)).num_columns(2).striped(true).spacing([10.0, 3.0]).show(ui, |ui| {
                for (i, (a, b, c)) in segments.iter().enumerate() {
                    let arc = noyau::forme2d::arc_courbure(*a, *b, *c);
                    let longueur = arc.map_or(a.distance(*b), |(_, r, _, s)| r * s.abs());
                    let texte = format!("{} · {}{}", i + 1, modele::unite::unite().afficher(longueur.round()), if arc.is_some() { " · arc" } else { "" });
                    let texte = if segment == Some(i) { RichText::new(texte).strong() } else { RichText::new(texte) };
                    ui.label(texte).on_hover_text("Segment, depuis le premier point de la polyligne ; longueur");
                    if let Some(id) = choix_type_trait(ui, ("trait_segment_famille", i), Some(tous.get(i).map_or("", |s| s.as_str())), &projet.types_traits) {
                        changement = Some((i, id));
                    }
                    ui.end_row();
                }
            });
        });
        if let Some((i, id)) = changement {
            definir_trait(&mut ed.famille.elements_2d[k].forme, Some(i), &id);
        }
    }
    ui.separator();
}

#[cfg(test)]
mod tests {
    use super::*;

    fn polyligne(n: usize, fermee: bool) -> AnnotationForme {
        AnnotationForme::Polyligne { points: (0..n).map(|k| [k as f64 * 100.0, (k % 2) as f64 * 50.0]).collect(), fermee, courbures: Vec::new(), remplissage: None, traits: Vec::new() }
    }

    #[test]
    fn trait_d_un_segment_puis_de_tous() {
        let mut f = polyligne(4, true);
        definir_trait(&mut f, Some(2), "trait-tirets");
        assert_eq!(traits_segments(&f), ["", "", "trait-tirets", ""]);
        definir_trait(&mut f, None, "trait-axe");
        assert_eq!(traits_segments(&f), vec!["trait-axe"; 4]);
        // retour au continu fin partout : plus rien d'enregistré
        definir_trait(&mut f, None, "");
        assert!(matches!(&f, AnnotationForme::Polyligne { traits, .. } if traits.is_empty()));
    }
}
