//! Interface du dessin 2D : clics et saisie au clavier, accrochages, repérage polaire
//! (Maj : ortho), aperçu élastique, invite et ligne de commande en bas de la vue, à la
//! manière de CADCraft. Sert aux annotations (Documenter → Dessin 2D, dans le repère de
//! la vue native ouverte) et aux murs (Modéliser → Mur, sur le plan horizontal de leur
//! base : ligne, rectangle, arc ou cercle).
use super::*;
use crate::dessin2d::{self as d2, Accroche, Geom, OutilDessin, Trace};
use crate::modele::commandes::Commande;
use crate::modele::documentation::{Annotation, AnnotationForme};
use glam::{DVec2, DVec3};

/// État du dessin 2D : tracé en cours, saisie au clavier, point sous le curseur (brut
/// et accroché), géométries d'accrochage de la vue (révision, vue).
#[derive(Default)]
pub(super) struct EtatDessin2D {
    pub trace: Option<Trace>,
    forme_cadrage: Option<OutilDessin>,
    ligne_selection: Option<d2::Forme2D>,
    pub arc_glisse: Option<(String,usize,[DVec2;3])>,
    pub saisie: String,
    curseur: Option<DVec2>,
    point: Option<(DVec2, Accroche)>,
    geoms: Option<(u64, String, Vec<Geom>)>,
    survol_accroche: Option<(DVec2, f64)>,
    reference: Option<DVec2>,
    guides: Vec<(DVec2, DVec2)>,
    pub message: Option<String>,
    /// Contours fermés de l'élément horizontal en cours : parties et trémies, créés à la
    /// validation.
    pub dalle: Vec<Vec<[f64; 2]>>,
    /// Terrain en cours : cote NGF de chaque sommet du contour (`dalle[0]`), points cotés
    /// (cote NGF), cote des prochains points, placement de points.
    terrain_cotes: Vec<f64>,
    terrain_points: Vec<[f64; 3]>,
    terrain_altitude: f64,
    terrain_ajouter_point: bool,
    /// Terrain existant repris par Modifier tracé (identifiant, nom), type choisi.
    terrain_modifie: Option<(String, String)>,
    terrain_type: Option<String>,
    /// Poignée sous le curseur, poignée glissée et état d'avant le glissé (repris si le
    /// terrain devient impossible à trianguler).
    terrain_survol: Option<PoigneeTerrain>,
    terrain_glisse: Option<(PoigneeTerrain, Vec<[f64; 2]>, Vec<f64>, Vec<[f64; 3]>)>,
    dernier_usage: Option<Usage>,
    pub edition_contour: Option<EditionContour>,
}

pub(super) struct EditionContour {
    id: String,
    altitude: f64,
    dessin: bool,
    selection: Option<usize>,
    segment: Option<(usize, usize)>,
    glisse: Option<GlisseContour>,
    annotation: Option<Annotation>,
    fermees: Vec<bool>,
    raccord: Option<(usize, bool)>,
}

/// Poignée du terrain en cours : sommet du contour, point coté, milieu d'un côté (le
/// glisser insère un sommet).
#[derive(Clone, Copy, PartialEq, Debug)]
enum PoigneeTerrain {
    Sommet(usize),
    Point(usize),
    Milieu(usize),
}

#[derive(Clone, Copy)]
enum GlisseContour {
    Sommet(usize, usize),
    Segment { contour: usize, segment: usize, origine: DVec2, a: DVec2, b: DVec2 },
}

fn ouvrir_contour(contour: &mut Vec<[f64; 2]>, segment: usize) {
    let debut = (segment + 1) % contour.len();
    contour.rotate_left(debut);
}

fn raccorder_contour(contour: &mut Vec<[f64; 2]>, premier: bool, trace: &[DVec2]) {
    let mut ajouts: Vec<_> = trace.iter().skip(1).map(|p| p.to_array()).collect();
    if premier { ajouts.reverse(); ajouts.extend(contour.iter().copied()); *contour = ajouts; }
    else { contour.extend(ajouts); }
}

#[cfg(test)]
mod tests_raccord {
    use super::*;
    #[test]
    fn suppression_et_raccord_depuis_les_deux_extremites() {
        let original = vec![[0.,0.], [4.,0.], [4.,4.], [0.,4.]];
        let mut ouvert = original.clone();
        ouvrir_contour(&mut ouvert, 1);
        assert_eq!(ouvert, vec![[4.,4.], [0.,4.], [0.,0.], [4.,0.]]);
        let mut depuis_fin = ouvert.clone();
        raccorder_contour(&mut depuis_fin, false, &[DVec2::new(4.,0.), DVec2::new(6.,2.)]);
        assert_eq!(depuis_fin.last(), Some(&[6.,2.]));
        raccorder_contour(&mut ouvert, true, &[DVec2::new(4.,4.), DVec2::new(6.,2.)]);
        assert_eq!(ouvert.first(), Some(&[6.,2.]));
        assert_eq!(ouvert.len(), 5);
    }
}

impl EditionContour {
    fn fermee(&self, i: usize) -> bool { self.fermees.get(i).copied().unwrap_or(true) }
}

/// Ce que produit le tracé.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(super) enum Usage {
    /// Annotations de la vue native ouverte.
    Annotation,
    /// Murs, suivant le type et les réglages de l'outil Mur.
    Mur,
    /// Élément horizontal (dalle, sol, plafond, toiture) : contour fermé.
    Surface,
    /// Escalier : ligne de foulée (palier à chaque sommet intermédiaire).
    Escalier,
    /// Garde-corps : tracé en plan.
    GardeCorps,
    Terrain,
    LimitePropriete,
    Cadrage,
}

/// Contour d'élément horizontal d'une forme fermée (cercle : 48 côtés).
pub(super) fn contour_surface(f: &d2::Forme2D) -> Option<Vec<[f64; 2]>> {
    let pts: Vec<DVec2> = match f {
        d2::Forme2D::Polyligne { points, .. } | d2::Forme2D::Spline { points, .. } if points.len() >= 3 => points.clone(),
        d2::Forme2D::Cercle { centre, rayon } => {
            let mut p = d2::points_arc(*centre, *rayon, 0.0, std::f64::consts::TAU);
            p.pop();
            // 48 côtés au plus, quelle que soit la finesse de l'aperçu
            let pas = (p.len() / 48).max(1);
            p.into_iter().step_by(pas).collect()
        }
        _ => return None,
    };
    Some(pts.into_iter().map(|p| p.to_array()).collect())
}

/// Lignes tracées des murs d'une forme : un mur par segment, un mur courbe par arc, deux
/// demi-cercles pour un cercle.
pub(super) fn axes_murs(f: &d2::Forme2D) -> Vec<crate::modele::Axe> {
    use crate::modele::Axe;
    let droit = |a: DVec2, b: DVec2| Axe::Droit { debut: a.to_array(), fin: b.to_array() };
    match f {
        d2::Forme2D::Ligne([a, b]) => vec![droit(*a, *b)],
        d2::Forme2D::Polyligne { points, fermee } | d2::Forme2D::Spline { points, fermee } => {
            let mut v: Vec<Axe> = points.windows(2).map(|w| droit(w[0], w[1])).collect();
            if *fermee && points.len() >= 3 {
                v.push(droit(points[points.len() - 1], points[0]));
            }
            v
        }
        d2::Forme2D::Arc([a, b, c]) => d2::arc_3_points(*a, *b, *c)
            .map(|(o, _, _, balayage)| Axe::Arc { debut: a.to_array(), fin: c.to_array(), centre: o.to_array(), direct: balayage > 0.0 })
            .into_iter()
            .collect(),
        d2::Forme2D::Cercle { centre, rayon } => {
            let (a, b) = (*centre + DVec2::X * *rayon, *centre - DVec2::X * *rayon);
            let arc = |d: DVec2, f: DVec2| Axe::Arc { debut: d.to_array(), fin: f.to_array(), centre: centre.to_array(), direct: true };
            vec![arc(a, b), arc(b, a)]
        }
    }
}

fn nom_outil(o: OutilDessin) -> &'static str {
    match o {
        OutilDessin::Rectangle => "RECTANGLE",
        OutilDessin::Ligne => "LIGNE",
        OutilDessin::Polyligne => "POLYLIGNE",
        OutilDessin::Spline => "SPLINE",
        OutilDessin::Cercle => "CERCLE",
        OutilDessin::Arc => "ARC",
    }
}

impl ViewerApp {
    pub(super) fn selection_traces_active(&self) -> bool {
        self.dessin2d.edition_contour.as_ref().is_some_and(|e| !e.dessin)
    }
    pub(super) fn objet_en_edition(&self) -> Option<(&str, bool)> {
        self.dessin2d.edition_contour.as_ref().map(|e| (e.id.as_str(), e.annotation.is_some()))
    }
    fn supprimer_trait_edition(&mut self) {
        let Some((c, i)) = self.dessin2d.edition_contour.as_ref().and_then(|e| e.segment) else { return };
        let e = self.dessin2d.edition_contour.as_mut().unwrap();
        if e.fermee(c) {
            ouvrir_contour(&mut self.dessin2d.dalle[c], i);
            e.fermees[c] = false;
        } else {
            if self.dessin2d.dalle[c].len() == 2 && self.dessin2d.dalle.len() == 1 {
                self.dessin2d.message = Some("Le dernier trait ne peut pas être supprimé.".into());
                return;
            }
            let suite = self.dessin2d.dalle[c].split_off(i + 1);
            let gauche = self.dessin2d.dalle[c].len();
            if gauche < 2 { self.dessin2d.dalle.remove(c); e.fermees.remove(c); }
            if suite.len() >= 2 {
                let insertion = if gauche < 2 { c } else { c + 1 };
                self.dessin2d.dalle.insert(insertion, suite);
                e.fermees.insert(insertion, false);
            }
        }
        e.selection = None;
        e.segment = None;
        self.dessin2d.message = Some("Cliquez une extrémité ouverte pour reprendre le tracé.".into());
    }

    fn ajouter_point_raccord(&mut self, p: DVec2) -> bool {
        let Some((c, premier)) = self.dessin2d.edition_contour.as_ref().and_then(|e| e.raccord) else { return false };
        let autre = self.dessin2d.dalle.iter().enumerate().filter(|(i, _)| *i != c && !self.dessin2d.edition_contour.as_ref().unwrap().fermee(*i)).find_map(|(i, t)| {
            if DVec2::from_array(t[0]).distance(p) < 1e-6 { Some((i, true)) }
            else if DVec2::from_array(*t.last().unwrap()).distance(p) < 1e-6 { Some((i, false)) }
            else { None }
        });
        if let Some((autre, debut)) = autre {
            let mut a = self.dessin2d.dalle[c].clone();
            let mut b = self.dessin2d.dalle[autre].clone();
            if premier { a.reverse(); }
            if !debut { b.reverse(); }
            a.extend(self.dessin2d.trace.as_ref().unwrap().points.iter().skip(1).map(|p| p.to_array()));
            a.extend(b);
            self.dessin2d.dalle[c] = a;
            self.dessin2d.dalle.remove(autre);
            self.dessin2d.trace.as_mut().unwrap().points.clear();
            let e = self.dessin2d.edition_contour.as_mut().unwrap();
            e.fermees.remove(autre);
            e.selection = Some(if autre < c { c - 1 } else { c });
            e.raccord = None;
            e.dessin = false;
            self.dessin2d.message = None;
            return true;
        }
        let contour = &mut self.dessin2d.dalle[c];
        let cible = DVec2::from_array(if premier { *contour.last().unwrap() } else { contour[0] });
        let trace = self.dessin2d.trace.as_mut().unwrap();
        if p.distance(cible) < 1e-6 {
            raccorder_contour(contour, premier, &trace.points);
            trace.points.clear();
            let e = self.dessin2d.edition_contour.as_mut().unwrap();
            e.fermees[c] = true;
            e.raccord = None;
            e.dessin = false;
            self.dessin2d.message = None;
        } else if trace.points.last().is_none_or(|q| q.distance(p) > 1e-6) { trace.points.push(p); }
        true
    }
    /// Terrain repris par Modifier tracé, pendant l'outil Terrain.
    pub(super) fn terrain_en_edition(&self) -> Option<&str> {
        self.dessin2d.terrain_modifie.as_ref().filter(|_| self.usage_dessin().is_some_and(|(u, _)| u == Usage::Terrain)).map(|(id, _)| id.as_str())
    }

    fn reinitialiser_terrain(&mut self) {
        let d = &mut self.dessin2d;
        d.terrain_cotes.clear();
        d.terrain_points.clear();
        d.terrain_ajouter_point = false;
        d.terrain_modifie = None;
        d.terrain_survol = None;
        d.terrain_glisse = None;
    }

    /// Terrain du tracé en cours (contour fermé, cotes, points, type).
    fn terrain_courant(&self) -> Option<crate::modele::site::Terrain> {
        let contour = self.dessin2d.dalle.first()?;
        let (id, nom) = self.dessin2d.terrain_modifie.clone().unwrap_or_else(|| (String::new(), "Terrain".into()));
        Some(crate::modele::site::Terrain {
            id,
            nom,
            contour: contour.iter().zip(&self.dessin2d.terrain_cotes).map(|(p, z)| [p[0], p[1], *z]).collect(),
            points: self.dessin2d.terrain_points.clone(),
            type_terrain: self.dessin2d.terrain_type.clone(),
        })
    }

    /// Modifier tracé d'un terrain : l'outil Terrain reprend son contour, ses cotes, ses
    /// points et son type ; la validation remplace le terrain.
    fn commencer_edition_terrain(&mut self, id: &str) {
        let Some(t) = self.natif.as_ref().and_then(|(p, _)| p.terrains.iter().find(|t| t.id == id)).cloned() else { return };
        self.ruban_edition.activer_outil(crate::ruban::Outil::Terrain);
        self.onglet_ruban = OngletRuban::Modeliser;
        self.dessin2d.edition_contour = None;
        self.dessin2d.dalle = vec![t.contour.iter().map(|p| [p[0], p[1]]).collect()];
        self.dessin2d.terrain_cotes = t.contour.iter().map(|p| p[2]).collect();
        self.dessin2d.terrain_points = t.points.clone();
        self.dessin2d.terrain_altitude = t.contour.first().map_or(0.0, |p| p[2]);
        self.dessin2d.terrain_ajouter_point = false;
        self.dessin2d.terrain_type = t.type_terrain.clone();
        self.dessin2d.terrain_modifie = Some((t.id.clone(), t.nom.clone()));
        self.dessin2d.dernier_usage = Some(Usage::Terrain);
        self.dessin2d.trace = Some(Trace::new(OutilDessin::Polyligne));
        self.visibility_dirty = true;
        self.status = "Terrain : glisser les sommets et les points, cotes à côté de chacun · Entrée : valider · Échap : annuler".into();
    }

    pub(super) fn commencer_edition_contour(&mut self) {
        let ids = self.selection_bim();
        let Some((p, _)) = &self.natif else { return };
        if ids.len()==1 && (p.murs.iter().any(|m|m.id==ids[0])||p.assemblages.iter().any(|m|m.id==ids[0])) {
            self.onglet_ruban=OngletRuban::Modifier;
            self.ruban_edition.annuler();
            self.status="Mur : glisser une extrémité de la ligne d'implantation ; accrochages actifs, Maj pour contraindre aux axes.".into();
            return;
        }
        if ids.len() == 1 && p.terrains.iter().any(|t| t.id == ids[0]) {
            let id = ids[0].clone();
            self.commencer_edition_terrain(&id);
            return;
        }
        if ids.len() != 1 { self.status = "Sélectionnez un élément horizontal ou une polyligne.".into(); return; }
        let (altitude, contours, annotation) = if let Some(d) = p.dalles.iter().find(|d| d.id == ids[0]) {
            (p.niveau(&d.niveau).map_or(0.0, |n| n.altitude) + d.decalage, d.contours.clone(), None)
        } else if let Some(a) = p.annotations.iter().find(|a| a.id == ids[0]) {
            let AnnotationForme::Polyligne { points, .. } = &a.forme else { self.status = "Sélectionnez un élément horizontal ou une polyligne.".into(); return; };
            if self.vue_native_id().as_deref() != Some(a.vue.as_str()) { self.status = "Ouvrez la vue de la polyligne pour la modifier.".into(); return; }
            (0.0, vec![points.clone()], Some(a.clone()))
        } else { self.status = "Sélectionnez un élément horizontal ou une polyligne.".into(); return; };
        self.dessin2d.dernier_usage = Some(if annotation.is_some() { Usage::Annotation } else { Usage::Surface });
        let fermees = vec![annotation.as_ref().is_none_or(|a| matches!(&a.forme, AnnotationForme::Polyligne { fermee: true, .. })); contours.len()];
        self.dessin2d.dalle = contours;
        self.dessin2d.geoms = None;
        self.dessin2d.edition_contour = Some(EditionContour { id: ids[0].clone(), altitude, dessin: false, selection: None, segment: None, glisse: None, annotation, fermees, raccord: None });
        self.dessin2d.trace = Some(Trace::new(OutilDessin::Polyligne));
        self.ruban_edition.forme_dalle = OutilDessin::Polyligne;
        self.ruban_edition.annuler();
        self.visibility_dirty = true;
        self.coupe_dirty = true;
    }

    pub(super) fn options_edition_contour(&mut self, ui: &mut egui::Ui) {
        let trace_en_cours = self.trace_en_cours();
        let poly = self.dessin2d.edition_contour.as_ref().is_some_and(|e| e.annotation.is_some());
        let mut supprimer_trait = false;
        ui.horizontal(|ui| {
            let edition = self.dessin2d.edition_contour.as_mut().unwrap();
            if ui.add_enabled(!trace_en_cours, egui::Button::selectable(!edition.dessin, "Modifier les sommets")).on_disabled_hover_text("Terminez ou annulez le tracé en cours.").clicked() { edition.dessin = false; }
            if !poly && ui.selectable_label(edition.dessin, "Ajouter un contour").clicked() { edition.dessin = true; }
            supprimer_trait = ui.add_enabled(edition.segment.is_some() && !trace_en_cours, egui::Button::new("Supprimer le trait")).clicked();
            ui.separator();
            if !poly && ui.add_enabled(edition.selection.is_some() && self.dessin2d.dalle.len() > 1, egui::Button::new("Supprimer le contour sélectionné")).clicked() {
                if let Some(i) = edition.selection.take() { self.dessin2d.dalle.remove(i); edition.fermees.remove(i); }
                edition.segment = None;
            }
        });
        if supprimer_trait { self.supprimer_trait_edition(); }
        if !poly && self.dessin2d.edition_contour.as_ref().is_some_and(|e| e.dessin && e.raccord.is_none()) { self.options_trace(ui); }
    }
    /// Outil de dessin actif : annotation ou mur.
    pub(super) fn outil_dessin(&self) -> Option<OutilDessin> {
        self.usage_dessin().map(|(_, o)| o)
    }

    /// Tracé actif et son usage : dessin 2D (Documenter, vue native ouverte) ou mur
    /// (Modéliser, projet natif chargé).
    pub(super) fn usage_dessin(&self) -> Option<(Usage, OutilDessin)> {
        use crate::ruban::Outil;
        if self.ruban_edition.outil() == Some(Outil::Cadrer) && self.vue_en_plan() && self.vue_native_id().is_some() {
            return Some((Usage::Cadrage, self.dessin2d.forme_cadrage.unwrap_or(OutilDessin::Rectangle)));
        }
        if self.dessin2d.edition_contour.is_some() {
            if self.dessin2d.edition_contour.as_ref().is_some_and(|e| e.annotation.is_some()) { return Some((Usage::Annotation, OutilDessin::Polyligne)); }
            let o = match self.ruban_edition.forme_dalle {
                o @ (OutilDessin::Rectangle | OutilDessin::Cercle) => o,
                _ => OutilDessin::Polyligne,
            };
            return Some((Usage::Surface, o));
        }
        let modeliser = self.onglet_ruban == OngletRuban::Modeliser && self.natif.is_some() && self.scene.is_some() && self.loading.is_none();
        if modeliser && self.ruban_edition.outil() == Some(Outil::Terrain) { return Some((Usage::Terrain, OutilDessin::Polyligne)); }
        if modeliser && self.ruban_edition.outil() == Some(Outil::LimitePropriete) {
            let forme = match self.ruban_edition.forme_dalle {
                f @ (OutilDessin::Rectangle | OutilDessin::Cercle) => f,
                _ => OutilDessin::Polyligne,
            };
            return Some((Usage::LimitePropriete, forme));
        }
        if modeliser && self.ruban_edition.mur_actif() {
            return Some((Usage::Mur, self.ruban_edition.forme_mur));
        }
        if modeliser && self.ruban_edition.dalle_active().is_some() {
            let forme = match self.ruban_edition.forme_dalle {
                f @ (OutilDessin::Rectangle | OutilDessin::Cercle) => f,
                _ => OutilDessin::Polyligne,
            };
            return Some((Usage::Surface, forme));
        }
        if modeliser && self.ruban_edition.outil() == Some(Outil::Escalier) {
            return Some((Usage::Escalier, OutilDessin::Polyligne));
        }
        if modeliser && self.ruban_edition.outil() == Some(Outil::GardeCorps) {
            return Some((Usage::GardeCorps, OutilDessin::Polyligne));
        }
        if self.onglet_ruban != OngletRuban::Annoter || self.vue_native_id().is_none() {
            return None;
        }
        Some((Usage::Annotation, match self.ruban_edition.outil()? {
            Outil::Ligne => OutilDessin::Ligne,
            Outil::Polyligne => OutilDessin::Polyligne,
            Outil::Spline => OutilDessin::Spline,
            Outil::Cercle => OutilDessin::Cercle,
            Outil::Arc => OutilDessin::Arc,
            _ => return None,
        }))
    }

    /// Un tracé a des points en cours (les raccourcis et Suppr attendent).
    pub(super) fn trace_en_cours(&self) -> bool {
        self.dessin2d.trace.as_ref().is_some_and(|t| !t.points.is_empty())
    }

    /// Altitude de la base des murs tracés (mm) : niveau de base et décalage.
    fn altitude_murs(&self) -> f64 {
        let Some((p, _)) = &self.natif else { return 0.0 };
        let o = self.ruban_edition.parametres_mur();
        o.niveau.as_ref().and_then(|id| p.niveau(id)).or_else(|| p.niveaux.first()).map_or(0.0, |n| n.altitude) + o.decalage
    }

    /// Altitude du plan de tracé horizontal (mm) : base du mur, ou niveau et décalage de
    /// l'élément horizontal (dessus d'une dalle ou d'un sol, sous-face d'un plafond).
    fn altitude_trace(&self) -> f64 {
        if let Some(e) = &self.dessin2d.edition_contour { return e.altitude; }
        match self.usage_dessin().map(|(u, _)| u) {
            Some(Usage::Terrain | Usage::LimitePropriete | Usage::Cadrage) => self.niveau_trace().and_then(|n| self.natif.as_ref()?.0.niveau(&n).map(|n| n.altitude)).unwrap_or(0.0),
            Some(Usage::Escalier | Usage::GardeCorps) => {
                let Some((p, _)) = &self.natif else { return 0.0 };
                let decalage = if self.usage_dessin().is_some_and(|(u, _)| u == Usage::GardeCorps) { self.ruban_edition.nombre_option(crate::ruban::Outil::GardeCorps, 1) } else { 0.0 };
                self.niveau_trace().and_then(|n| p.niveau(&n)).map_or(0.0, |n| n.altitude) + decalage
            }
            Some(Usage::Surface) => {
                let (Some((p, _)), Some(d)) = (&self.natif, self.ruban_edition.dalle_active()) else { return 0.0 };
                d.niveau.as_ref().and_then(|id| p.niveau(id)).or_else(|| p.niveaux.first()).map_or(0.0, |n| n.altitude) + d.decalage
            }
            _ => self.altitude_murs(),
        }
    }

    /// Repère du tracé : origine, droite, haut (mm). Annotation : celui de la vue native
    /// ouverte ; mur ou élément horizontal : plan horizontal de son altitude.
    fn repere_dessin(&self) -> Option<(DVec3, DVec3, DVec3)> {
        if let Some((id,_,_))=&self.dessin2d.arc_glisse {
            let (_,z)=self.natif.as_ref()?.0.trajectoire_justification(id)?;
            return Some((DVec3::new(0.,0.,z),DVec3::X,DVec3::Y));
        }
        if self.poignee_axe.is_some() {
            let (_,_,_,z,_)=self.axe_selection()?;
            return Some((DVec3::new(0.0,0.0,z),DVec3::X,DVec3::Y));
        }
        match self.usage_dessin()?.0 {
            Usage::Mur | Usage::Surface | Usage::Escalier | Usage::GardeCorps | Usage::Terrain | Usage::LimitePropriete | Usage::Cadrage => Some((DVec3::new(0.0, 0.0, self.altitude_trace()), DVec3::X, DVec3::Y)),
            Usage::Annotation => {
                let id = self.vue_native_id()?;
                let p = &self.natif.as_ref()?.0;
                let v = p.vues.iter().find(|v| v.id == id)?;
                Some(crate::modele::documentation::repere(p, v))
            }
        }
    }

    /// Point du modèle (mm) à l'écran.
    fn ecran_3d(&self, rect: egui::Rect, p: DVec3) -> Option<egui::Pos2> {
        let s = self.scene.as_ref()?;
        let w = (p * 0.001 - s.origin).as_vec3();
        let c = self.camera.view_proj(rect.width() / rect.height().max(1.0)) * w.extend(1.0);
        (c.w > 0.0).then(|| egui::pos2(rect.left() + (c.x / c.w + 1.0) * 0.5 * rect.width(), rect.top() + (1.0 - c.y / c.w) * 0.5 * rect.height()))
    }

    fn vers_ecran(&self, rect: egui::Rect, q: DVec2) -> Option<egui::Pos2> {
        let (o, u, h) = self.repere_dessin()?;
        self.ecran_3d(rect, o + u * q.x + h * q.y)
    }

    fn depuis_ecran(&self, rect: egui::Rect, q: egui::Pos2) -> Option<DVec2> {
        let (o, u, h) = self.repere_dessin()?;
        let s = self.scene.as_ref()?;
        let x = (q.x - rect.left()) / rect.width() * 2.0 - 1.0;
        let y = 1.0 - (q.y - rect.top()) / rect.height() * 2.0;
        let (r0, dir) = self.camera.ray(x, y, rect.width() / rect.height().max(1.0));
        let (r0, dir) = ((r0.as_dvec3() + s.origin) * 1000.0, dir.as_dvec3());
        let n = u.cross(h);
        let den = n.dot(dir);
        if den.abs() < 1e-12 {
            return None;
        }
        let t = n.dot(o - r0) / den;
        let p = r0 + dir * t - o;
        Some(DVec2::new(p.dot(u), p.dot(h)))
    }

    /// Géométries d'accrochage. Annotation : dessins de la vue et, en plan, murs (contours
    /// et axes), éléments horizontaux et quadrillages de son niveau. Mur : murs qui
    /// traversent le plan de tracé, éléments horizontaux, quadrillages et dessins de la
    /// vue en plan ouverte (calque de tracé).
    fn geoms_dessin(&mut self) -> Vec<Geom> {
        let Some((usage, _)) = self.usage_dessin().or_else(||(self.poignee_axe.is_some() || self.dessin2d.arc_glisse.is_some()).then_some((Usage::Mur,OutilDessin::Ligne))) else { return Vec::new() };
        let vue = self.vue_native_id();
        let altitude = if let Some((id,_,_))=&self.dessin2d.arc_glisse {self.natif.as_ref().and_then(|(p,_)|p.trajectoire_justification(id)).map_or(0.0,|a|a.1)} else if self.poignee_axe.is_some() {self.axe_selection().map_or(0.0,|a|a.3)} else {self.altitude_trace()};
        let exclu=self.poignee_axe.as_ref().map(|a|a.0.as_str()).or_else(||self.dessin2d.arc_glisse.as_ref().map(|a|a.0.as_str()));
        let cle = format!("{usage:?}|{}|{altitude}|{exclu:?}", vue.as_deref().unwrap_or(""));
        if let Some((r, k, g)) = &self.dessin2d.geoms {
            if *r == self.bim.revision && *k == cle {
                return g.clone();
            }
        }
        let Some((p, _)) = &self.natif else { return Vec::new() };
        let plan = vue.as_ref().and_then(|id| p.vues.iter().find(|v| &v.id == id)).and_then(|v| match &v.genre {
            crate::modele::GenreVue::Plan { niveau, .. } => Some(niveau.clone()),
            _ => None,
        });
        let mut g = Vec::new();
        // dessins de la vue : toujours pour une annotation, en plan pour un mur
        if let Some(vue) = vue.as_ref().filter(|_| usage == Usage::Annotation || plan.is_some()) {
            for a in p.annotations.iter().filter(|a| &a.vue == vue && self.dessin2d.edition_contour.as_ref().is_none_or(|e| e.id != a.id)) {
                if let Some(f) = a.forme.forme_2d() {
                    g.extend(f.geoms());
                }
            }
        }
        let modele = usage != Usage::Annotation || plan.is_some();
        if modele {
            let seg = |a: [f64; 2], b: [f64; 2]| Geom::Segment([DVec2::from_array(a), DVec2::from_array(b)]);
            let coupes = crate::modele::jonctions::coupes(p);
            let retenu = |m: &crate::modele::Mur| match usage {
                Usage::Annotation => plan.as_ref() == Some(&m.niveau),
                _ => p.bas_haut(m).is_some_and(|(b, h)| altitude >= b - 0.1 && altitude <= h + 0.1),
            };
            for m in p.murs.iter().filter(|m| retenu(m)&&exclu!=Some(m.id.as_str())&&exclu!=m.assemblage.as_deref()) {
                if let Some(c) = crate::modele::jonctions::contour(p, m, &coupes) {
                    for k in 0..4 {
                        g.push(seg(c[k], c[(k + 1) % 4]));
                    }
                }
                match m.axe {
                    crate::modele::Axe::Droit { debut, fin } => g.push(seg(debut, fin)),
                    crate::modele::Axe::Arc { debut, fin, centre, .. } => {
                        let o = DVec2::from_array(centre);
                        let (a, b) = (DVec2::from_array(debut), DVec2::from_array(fin));
                        let r = a.distance(o);
                        let a0 = (a - o).to_angle();
                        let direct = matches!(m.axe, crate::modele::Axe::Arc { direct: true, .. });
                        let mut balayage = (b - o).to_angle() - a0;
                        balayage = if direct { balayage.rem_euclid(std::f64::consts::TAU) } else { -(-balayage).rem_euclid(std::f64::consts::TAU) };
                        g.push(Geom::Arc { centre: o, rayon: r, depart: a0, balayage });
                    }
                }
            }
            for d in p.dalles.iter().filter(|d| (usage != Usage::Annotation || plan.as_ref() == Some(&d.niveau)) && self.dessin2d.edition_contour.as_ref().is_none_or(|e| e.id != d.id)) {
                for c in &d.contours {
                    for k in 0..c.len() {
                        g.push(seg(c[k], c[(k + 1) % c.len()]));
                    }
                }
            }
            for t in &p.terrains {
                for i in 0..t.contour.len() {
                    let a = t.contour[i]; let b = t.contour[(i + 1) % t.contour.len()];
                    g.push(seg([a[0], a[1]], [b[0], b[1]]));
                }
            }
            for l in &p.limites_propriete {
                for i in 0..l.contour.len() { g.push(seg(l.contour[i], l.contour[(i + 1) % l.contour.len()])); }
            }
            for q in &p.quadrillages {
                g.push(seg(q.debut, q.fin));
            }
        }
        self.dessin2d.geoms = Some((self.bim.revision, cle, g.clone()));
        g
    }

    pub(super) fn point_edition_axe(&mut self,ui:&egui::Ui,rect:egui::Rect,q:egui::Pos2,maj:bool,fixe:[f64;2])->Option<[f64;2]> {
        let c=self.depuis_ecran(rect,q)?;
        let fixe=DVec2::from_array(fixe);
        let mut trace=Trace::new(OutilDessin::Ligne);
        trace.points.push(fixe);
        self.dessin2d.trace=Some(trace);
        let point=self.point_dessin_depuis(rect,c,maj,ui.input(|i|i.time),Some(fixe));
        self.dessin2d.curseur=Some(c);
        self.dessin2d.point=Some(point);
        if self.dessin2d.survol_accroche.is_some() {ui.ctx().request_repaint_after(std::time::Duration::from_millis(50));}
        Some(point.0.to_array())
    }

    pub(super) fn terminer_edition_axe(&mut self) {
        self.dessin2d.trace=None;
        self.dessin2d.point=None;
        self.dessin2d.geoms=None;
        self.dessin2d.guides.clear();
        self.dessin2d.reference=None;
        self.dessin2d.survol_accroche=None;
    }
    pub(super) fn etat_accrochage(&self)->serde_json::Value {
        serde_json::json!({"reference":self.dessin2d.reference.map(|p|p.to_array()),"point":self.dessin2d.point.map(|(p,a)|serde_json::json!({"position":p.to_array(),"accroche":a.libelle()}))})
    }

    /// Point retenu pour le curseur `c` : accrochage, sinon ortho (Maj) ou repérage
    /// polaire (tous les 15°) depuis le point précédent, sinon le point libre.
    fn point_dessin(&mut self, rect: egui::Rect, c: DVec2, maj: bool, temps: f64) -> (DVec2, Accroche) {
        let precedent=self.dessin2d.trace.as_ref().and_then(Trace::precedent);
        self.point_dessin_depuis(rect,c,maj,temps,precedent)
    }

    fn point_dessin_depuis(&mut self, rect:egui::Rect,c:DVec2,maj:bool,temps:f64,precedent:Option<DVec2>)->(DVec2,Accroche) {
        // tolérance : 12 pixels en mm du plan de la vue
        let tol = self.vers_ecran(rect, c).zip(self.vers_ecran(rect, c + DVec2::X * 1000.0)).map_or(200.0, |(a, b)| 12.0 * 1000.0 / (a.distance(b) as f64).max(1e-6));
        let mut geoms = self.geoms_dessin();
        if let Some(t) = &self.dessin2d.trace {
            geoms.extend(t.points.windows(2).map(|w| Geom::Segment([w[0], w[1]])));
            if let [p] = t.points.as_slice() {
                geoms.push(Geom::Segment([*p, *p]));
            }
        }
        if self.dessin2d.edition_contour.is_some() {
            for (ci, contour) in self.dessin2d.dalle.iter().enumerate() {
                let fermee = self.dessin2d.edition_contour.as_ref().unwrap().fermee(ci);
                for i in 0..contour.len().saturating_sub(usize::from(!fermee)) {
                    geoms.push(Geom::Segment([DVec2::from_array(contour[i]), DVec2::from_array(contour[(i + 1) % contour.len()])]));
                }
            }
        }
        self.dessin2d.guides.clear();
        let accroche = (!maj).then(|| d2::accrocher(c, tol, &geoms, precedent)).flatten();
        let fixe = accroche.filter(|(_, a)| !matches!(a, Accroche::Proche | Accroche::Perpendiculaire));
        if let Some((p, _)) = fixe {
            match self.dessin2d.survol_accroche {
                Some((ancien, debut)) if ancien.distance(p) < tol * 0.1 => {
                    if temps - debut >= self.options.delai_accrochage { self.dessin2d.reference = Some(p); }
                }
                _ => self.dessin2d.survol_accroche = Some((p, temps)),
            }
            return (p, accroche.unwrap().1);
        }
        self.dessin2d.survol_accroche = None;
        if maj {
            self.dessin2d.guides.clear();
        } else {
            let th = self.angle_axes_plan();
            let u = DVec2::from_angle(th);
            let v = DVec2::new(-u.y, u.x);
            let mut lignes = Vec::new();
            if let Some(t) = &self.dessin2d.trace {
                if let [.., a, b] = t.points.as_slice() {
                    let d = *b - *a;
                    if d.length_squared() > 1e-12 { lignes.push((*b, d.normalize())); }
                }
                if let Some(p) = precedent { lignes.extend([(p, u), (p, v)]); }
            }
            if let Some(p) = self.dessin2d.reference { lignes.extend([(p, u), (p, v)]); }
            // Une intersection de deux guides est prioritaire sur une projection simple.
            let mut meilleur: Option<(f64, DVec2, Vec<(DVec2, DVec2)>)> = None;
            for i in 0..lignes.len() {
                for j in i + 1..lignes.len() {
                    let (a, d) = lignes[i];
                    let (b, e) = lignes[j];
                    let det = d.perp_dot(e);
                    if det.abs() < 1e-9 { continue; }
                    let p = a + d * ((b - a).perp_dot(e) / det);
                    let distance = p.distance(c);
                    if distance < tol && meilleur.as_ref().is_none_or(|m| distance < m.0) {
                        meilleur = Some((distance, p, vec![lignes[i], lignes[j]]));
                    }
                }
            }
            if let Some((_, p, guides)) = meilleur {
                self.dessin2d.guides = guides;
                return (p, Accroche::Intersection);
            }
            let mut proche = None;
            for (a, d) in lignes {
                let p = a + d * (c - a).dot(d);
                let distance = p.distance(c);
                if distance < tol && proche.as_ref().is_none_or(|(dist, _, _)| distance < *dist) {
                    proche = Some((distance, p, (a, d)));
                }
            }
            if let Some((_, p, guide)) = proche {
                self.dessin2d.guides.push(guide);
                return (p, Accroche::Ortho);
            }
        }
        if let Some(a) = accroche { return a; }
        // repère du plan tourné : ortho et angles ronds suivent l'écran ; longueurs et
        // coordonnées libres au pas de 10 mm pour un mur, 1 mm pour un dessin
        let th = self.angle_axes_plan();
        let pas = if self.usage_dessin().is_some_and(|(u, _)| u != Usage::Annotation) { 10.0 } else { 1.0 };
        let tourner = |v: DVec2, a: f64| DVec2::from_angle(a).rotate(v);
        let arrondi = |v: DVec2| {
            let l = v.length();
            if l < 1e-9 { v } else { v * ((l / pas).round() * pas / l) }
        };
        match precedent {
            Some(p) if maj => (p + arrondi(tourner(d2::ortho(DVec2::ZERO, tourner(c - p, -th)), th)), Accroche::Ortho),
            Some(p) => match d2::polaire(DVec2::ZERO, tourner(c - p, -th), 15.0, 2.5) {
                Some((q, a)) => (p + arrondi(tourner(q, th)), Accroche::Polaire(a)),
                None => ((c / pas).round() * pas, Accroche::Libre),
            },
            None => ((c / pas).round() * pas, Accroche::Libre),
        }
    }

    /// Enregistre les formes achevées : annotations de la vue, ou murs (une étape
    /// d'annulation par forme ; le dernier mur créé est sélectionné).
    fn enregistrer_formes(&mut self, formes: Vec<d2::Forme2D>) {
        if formes.is_empty() {
            return;
        }
        if self.usage_dessin().is_some_and(|(u, _)| u == Usage::Escalier || u == Usage::GardeCorps) {
            let escalier = self.usage_dessin().is_some_and(|(u, _)| u == Usage::Escalier);
            for f in &formes {
                let d2::Forme2D::Polyligne { points, .. } = f else { continue };
                if points.len() < 2 {
                    continue;
                }
                let trace: Vec<[f64; 2]> = points.iter().map(|p| p.to_array()).collect();
                let r = if escalier { self.creer_escalier(trace) } else { self.creer_garde_corps(trace, None).map(|_| ()) };
                if let Err(e) = r {
                    self.dessin2d.message = Some(e);
                }
            }
            return;
        }
        if self.usage_dessin().is_some_and(|(u, _)| matches!(u, Usage::Surface | Usage::Terrain | Usage::LimitePropriete | Usage::Cadrage)) {
            // le contour fermé rejoint l'élément en cours ; il devient dalle à la validation
            for f in &formes {
                let Some(c) = contour_surface(f) else {
                    self.dessin2d.message = Some("Contour fermé attendu : au moins trois sommets.".into());
                    continue;
                };
                match crate::modele::edition::contour_valide(&c) {
                    Err(e) => self.dessin2d.message = Some(e),
                    Ok(c) if self.dessin2d.dalle.iter().any(|x| crate::modele::commandes::croisement(x, &c)) => {
                        self.dessin2d.message = Some("Le contour croise un contour déjà tracé.".into());
                    }
                    Ok(c) => {
                        if self.usage_dessin().is_some_and(|(u, _)| matches!(u, Usage::Terrain | Usage::Cadrage)) && !self.dessin2d.dalle.is_empty() {
                            self.dessin2d.message = Some("Un seul contour est attendu.".into());
                            continue;
                        }
                        if self.usage_dessin().is_some_and(|(u, _)| u == Usage::Terrain) {
                            // cotes NGF : niveau du tracé, rapporté à la cote NGF de l'origine
                            let ngf = self.natif.as_ref().map_or(0.0, |(p, _)| p.ngf()) + self.altitude_trace();
                            self.dessin2d.terrain_cotes = vec![ngf; c.len()];
                            self.dessin2d.terrain_altitude = ngf;
                        }
                        self.dessin2d.dalle.push(c);
                        if let Some(e) = &mut self.dessin2d.edition_contour { e.fermees.push(true); }
                        self.dessin2d.message = None;
                    }
                }
            }
            return;
        }
        if self.usage_dessin().is_some_and(|(u, _)| u == Usage::Mur) {
            let options = self.ruban_edition.parametres_mur();
            for f in &formes {
                let Some((projet, _)) = &self.natif else { return };
                let mut projet = projet.clone();
                let ids: Result<Vec<String>, String> = axes_murs(f).into_iter().map(|axe| projet.ajouter_mur_axe(axe, &options)).collect();
                match ids.and_then(|ids| self.appliquer_projet(projet).map(|()| ids)) {
                    Ok(ids) => {
                        if let Some(id) = ids.last() {
                            let genre = if self.natif.as_ref().is_some_and(|(p, _)| p.assemblage(id).is_some()) { "assemblage" } else { "mur" };
                            self.selected = Some(scene::id_natif(genre, id));
                            self.reveal = self.selected;
                        }
                        self.status = format!("{} mur(s) créé(s) — Ctrl+S pour enregistrer.", ids.len());
                    }
                    Err(e) => self.error = Some(e),
                }
            }
            return;
        }
        let Some(vue) = self.vue_native_id() else { return };
        for f in formes {
            let annotation = Annotation { id: String::new(), vue: vue.clone(), hauteur: 2.5, forme: AnnotationForme::depuis_forme_2d(&f) };
            if let Err(e) = self.executer_bim(Commande::Annoter { annotation }) {
                self.error = Some(e);
            }
        }
        self.bim.selection.clear();
    }

    /// Clavier du dessin 2D, lu avant les raccourcis : chiffres et `@ < , ; . -` dans la
    /// ligne de commande, F fermer, Entrée ou Espace valider (vide : terminer),
    /// Retour arrière, Échap (saisie, puis tracé en cours, puis outil).
    pub(super) fn clavier_dessin(&mut self, ctx: &egui::Context) {
        if self.ruban_edition.selection_mur && self.usage_dessin().is_some_and(|(u,_)|u==Usage::Mur) {
            if self.dessin2d.dernier_usage!=Some(Usage::Mur) {self.dessin2d.dalle.clear();}
            self.dessin2d.dernier_usage=Some(Usage::Mur);
            return;
        }
        if self.poignee_axe.is_some() || self.dessin2d.arc_glisse.is_some() {return;}
        let Some((usage, outil)) = self.usage_dessin() else {
            self.dessin2d.dernier_usage = None;
            self.dessin2d.trace = None;
            self.dessin2d.reference = None;
            self.dessin2d.survol_accroche = None;
            self.dessin2d.guides.clear();
            self.dessin2d.saisie.clear();
            self.dessin2d.dalle.clear();
            self.reinitialiser_terrain();
            return;
        };
        if self.dessin2d.dernier_usage.is_some_and(|u| u != usage) {
            self.dessin2d.dalle.clear();
            self.reinitialiser_terrain();
            self.dessin2d.trace = None;
        }
        self.dessin2d.dernier_usage = Some(usage);
        if self.dessin2d.trace.as_ref().is_none_or(|t| t.outil != outil) {
            self.dessin2d.reference = None;
            self.dessin2d.survol_accroche = None;
            if self.dessin2d.trace.is_none() && usage == Usage::Annotation {
                self.select(None, false);
            }
            let mut trace=Trace::new(outil);
            trace.perimetre=usage==Usage::Mur && outil==OutilDessin::Cercle;
            self.dessin2d.trace = Some(trace);
            self.dessin2d.saisie.clear();
        }
        if ctx.egui_wants_keyboard_input() {
            return;
        }
        let mut valider = false;
        let mut inverser = false;
        let mut echap = false;
        let mut effacer = false;
        let mut supprimer_trait = false;
        // F (fermer) seulement pendant un tracé : sinon F cadre la vue
        let en_cours = self.trace_en_cours();
        ctx.input_mut(|i| {
            i.events.retain(|e| match e {
                egui::Event::Text(t) => {
                    let mut lu = false;
                    for ch in t.chars() {
                        if ch.is_ascii_digit() || "@<,;.-".contains(ch) || (en_cours && ch.eq_ignore_ascii_case(&'f')) {
                            self.dessin2d.saisie.push(ch);
                            self.dessin2d.message = None;
                            lu = true;
                        } else if ch == ' ' {
                            // Espace inverse le sens d'un mur ; ailleurs, il valide comme Entrée
                            if usage == Usage::Mur { inverser = true } else { valider = true }
                            lu = true;
                        }
                    }
                    // les autres lettres restent aux raccourcis
                    !lu
                }
                egui::Event::Key { key: egui::Key::Enter, pressed: true, .. } => {
                    valider = true;
                    false
                }
                egui::Event::Key { key: egui::Key::Escape, pressed: true, .. } => {
                    echap = true;
                    false
                }
                egui::Event::Key { key: egui::Key::Backspace, pressed: true, .. } => {
                    effacer = true;
                    false
                }
                egui::Event::Key { key: egui::Key::Space, .. } => false,
                egui::Event::Key { key: egui::Key::Delete, pressed: true, .. } if self.dessin2d.edition_contour.is_some() => { supprimer_trait = true; false },
                egui::Event::Key { key: egui::Key::Delete, .. } => !en_cours,
                egui::Event::Key { key: egui::Key::F, .. } => !en_cours,
                _ => true,
            });
        });
        if supprimer_trait { self.supprimer_trait_edition(); }
        if effacer {
            self.dessin2d.saisie.pop();
        }
        if echap {
            self.dessin2d.message = None;
            if !self.dessin2d.saisie.is_empty() {
                self.dessin2d.saisie.clear();
            } else if let Some(t) = self.dessin2d.trace.as_mut().filter(|t| !t.points.is_empty()) {
                // Échap abandonne le tracé en cours ; la ligne garde ses segments déjà créés
                t.points.clear();
                if let Some(e) = &mut self.dessin2d.edition_contour && e.raccord.take().is_some() { e.dessin = false; }
            } else {
                if self.dessin2d.edition_contour.is_some() { self.quitter_commande(false); }
                else { self.ruban_edition.annuler(); self.dessin2d.trace = None; }
            }
        }
        if inverser {
            self.ruban_edition.sens_inverse = !self.ruban_edition.sens_inverse;
        }
        if valider {
            // une valeur tapée donne un point ; sinon Entrée valide la commande
            if self.dessin2d.saisie.is_empty() {
                self.quitter_commande(true);
            } else {
                self.valider_saisie_dessin();
            }
        }
    }

    /// Quitte la commande en cours. Valider : le tracé en cours est terminé (comme Entrée),
    /// puis l'outil se ferme ; Annuler : tout ce qui est en cours est abandonné, puis
    /// l'outil se ferme.
    pub(super) fn quitter_commande(&mut self, valider: bool) {
        if valider && self.dessin2d.edition_contour.as_ref().is_some_and(|e| e.raccord.is_some()) {
            self.dessin2d.message = Some("Terminez le raccord ou annulez son tracé avec Échap.".into());
            return;
        }
        if valider && self.dessin2d.edition_contour.as_ref().is_some_and(|e| e.annotation.is_none() && e.fermees.iter().any(|f| !f)) {
            self.dessin2d.message = Some("Refermez tous les contours avant de valider.".into());
            return;
        }
        let usage = self.usage_dessin().map(|(u, _)| u);
        let surface = usage.is_some_and(|u| matches!(u, Usage::Surface | Usage::Terrain | Usage::LimitePropriete | Usage::Cadrage));
        if valider && self.dessin2d.trace.is_some() {
            if !self.dessin2d.saisie.is_empty() {
                self.valider_saisie_dessin();
            }
            if self.trace_en_cours() {
                if surface {
                    // polygone resté ouvert : fermé, il rejoint l'élément
                    if self.dessin2d.trace.as_ref().is_some_and(|t| t.outil != OutilDessin::Polyligne || t.points.len() < 3) {
                        self.dessin2d.message = Some("Terminez le contour en cours avant de valider.".into());
                        return;
                    }
                    let formes = self.dessin2d.trace.as_mut().map(|t| t.fermer()).unwrap_or_default();
                    self.enregistrer_formes(formes);
                    if self.dessin2d.message.is_some() { return; }
                } else {
                    self.valider_saisie_dessin();
                }
            }
        }
        if valider && usage == Some(Usage::Cadrage) {
            let Some(contour) = self.dessin2d.dalle.first().cloned() else { self.dessin2d.message = Some("Tracez d'abord un contour fermé.".into()); return; };
            if contour.len() > 128 { self.dessin2d.message = Some("Le contour de cadrage ne peut pas dépasser 128 sommets.".into()); return; }
            let Some(id) = self.vue_native_id() else { return };
            let Some((mut projet, _)) = self.natif.clone() else { return };
            if let Some(vue) = projet.vues.iter_mut().find(|v| v.id == id) { vue.contour_cadrage = contour; }
            if let Err(e) = self.appliquer_projet(projet) { self.dessin2d.message = Some(e); return; }
            self.visibility_dirty = true;
            self.coupe_dirty = true;
            self.status = "Cadrage de la vue modifié.".into();
        } else if valider && usage == Some(Usage::LimitePropriete) {
            if self.dessin2d.dalle.is_empty() { self.dessin2d.message = Some("Tracez d'abord un contour fermé.".into()); return; }
            let Some((mut projet, _)) = self.natif.clone() else { return };
            let altitude = self.altitude_trace();
            let mut ids = Vec::new();
            for contour in &self.dessin2d.dalle {
                let limite = crate::modele::site::LimitePropriete { id: String::new(), nom: "Limite de propriété".into(), altitude, contour: contour.clone() };
                match projet.executer(Commande::LimitePropriete { limite }) {
                    Ok((p, nouveaux)) => { projet = p; ids.extend(nouveaux); }
                    Err(e) => { self.dessin2d.message = Some(e); return; }
                }
            }
            if let Err(e) = self.appliquer_projet(projet) { self.dessin2d.message = Some(e); return; }
            self.bim.selection = ids.clone();
            self.selected = ids.last().map(|id| scene::id_natif("limite_propriete", id));
            self.reveal = self.selected;
        } else if valider && usage == Some(Usage::Terrain) {
            // création, ou terrain repris par Modifier tracé (même identifiant)
            let Some(terrain) = self.terrain_courant() else { self.dessin2d.message = Some("Tracez d'abord un contour fermé.".into()); return; };
            match self.executer_bim(Commande::Terrain { terrain }) {
                Ok(ids) => {
                    if let Some(id) = ids.first() {
                        self.selected = Some(scene::id_natif("terrain", id));
                        self.reveal = self.selected;
                    }
                }
                Err(e) => { self.dessin2d.message = Some(e); return; }
            }
        } else if valider && self.dessin2d.edition_contour.as_ref().is_some_and(|e| e.annotation.is_some()) {
            let edit = self.dessin2d.edition_contour.as_ref().unwrap();
            let modele = edit.annotation.clone().unwrap();
            let Some((mut projet, _)) = self.natif.clone() else { return };
            let mut ids = Vec::new();
            for (i, points) in self.dessin2d.dalle.iter().enumerate() {
                let fermee = edit.fermee(i);
                let minimum = if fermee { 3 } else { 2 };
                if points.len() < minimum || points.windows(2).any(|w| DVec2::from_array(w[0]).distance(DVec2::from_array(w[1])) < 1e-6)
                    || (fermee && DVec2::from_array(points[0]).distance(DVec2::from_array(*points.last().unwrap())) < 1e-6) {
                    self.dessin2d.message = Some("La polyligne doit comporter des segments de longueur non nulle.".into()); return;
                }
                let mut annotation = modele.clone();
                if i > 0 { annotation.id.clear(); }
                // le remplissage reste tant que la polyligne est fermée
                let remplissage = match &modele.forme {
                    AnnotationForme::Polyligne { remplissage, .. } if fermee => remplissage.clone(),
                    _ => None,
                };
                annotation.forme = AnnotationForme::Polyligne { points: points.clone(), fermee, remplissage };
                match projet.executer(Commande::Annoter { annotation }) {
                    Ok((p, nouveaux)) => { projet = p; ids.extend(nouveaux); }
                    Err(e) => { self.dessin2d.message = Some(e); return; }
                }
            }
            if let Err(e) = self.appliquer_projet(projet) { self.dessin2d.message = Some(e); return; }
            self.bim.selection = ids;
            self.status = "Polyligne modifiée — Ctrl+S pour enregistrer.".into();
        } else if surface && valider && self.dessin2d.edition_contour.is_some() {
            let id = self.dessin2d.edition_contour.as_ref().unwrap().id.clone();
            let contours = self.dessin2d.dalle.clone();
            if let Err(e) = self.executer_bim(Commande::Contour { id, contours }) {
                self.dessin2d.message = Some(e);
                return;
            }
            self.status = "Contour modifié — Ctrl+S pour enregistrer.".into();
        } else if surface && valider {
            let contours = std::mem::take(&mut self.dessin2d.dalle);
            if let (false, Some(parametres)) = (contours.is_empty(), self.ruban_edition.dalle_active()) {
                self.creer_dalle_contours(&contours, &parametres);
            }
        }
        self.dessin2d.dalle.clear();
        self.reinitialiser_terrain();
        self.dessin2d.edition_contour = None;
        self.visibility_dirty = true;
        self.coupe_dirty = true;
        self.dessin2d.geoms = None;
        self.dessin2d.saisie.clear();
        self.dessin2d.message = None;
        self.dessin2d.trace = None;
        self.bim.depart_deplacement = None;
        self.bim.depart_structure = None;
        self.coupe_depart = None;
        self.alignement_plan = false;
        self.arete_alignement = None;
        self.ruban_edition.annuler();
    }

    /// Terrain en cours : altitude absolue de chaque sommet du contour (S1, S2…) et de
    /// chaque point coté (1, 2… ; × le retire), modifiable à côté du point, dans l'unité
    /// du projet.
    pub(super) fn cotes_terrain(&mut self, ui: &egui::Ui, rect: egui::Rect) {
        if self.dessin2d.dalle.is_empty() || !self.usage_dessin().is_some_and(|(u, _)| u == Usage::Terrain) {
            return;
        }
        let (bleu, vert) = (Color32::from_rgb(0, 120, 215), Color32::from_rgb(20, 160, 60));
        let sommets: Vec<DVec2> = self.dessin2d.dalle[0].iter().map(|p| DVec2::from_array(*p)).collect();
        let points: Vec<DVec2> = self.dessin2d.terrain_points.iter().map(|p| DVec2::new(p[0], p[1])).collect();
        let mut retrait = None;
        for (point, (nom, couleur, k)) in sommets.iter().zip((0..).map(|i| (format!("S{}", i + 1), bleu, i))).chain(points.iter().zip((0..).map(|i| (format!("{}", i + 1), vert, sommets.len() + i)))) {
            let Some(q) = self.vers_ecran(rect, *point) else { continue };
            if !rect.contains(q) {
                continue;
            }
            egui::Area::new(egui::Id::unique(("cote_terrain", k)))
                .fixed_pos(q + egui::vec2(7.0, -24.0))
                .constrain_to(rect)
                .show(ui.ctx(), |ui| {
                    egui::Frame::NONE.fill(Color32::from_white_alpha(225)).corner_radius(3.0).inner_margin(egui::Margin::symmetric(3, 1)).show(ui, |ui| {
                        ui.spacing_mut().item_spacing.x = 3.0;
                        ui.horizontal(|ui| {
                            ui.label(RichText::new(nom).color(couleur).small().strong());
                            let z = match k.checked_sub(sommets.len()) {
                                None => self.dessin2d.terrain_cotes.get_mut(k),
                                Some(j) => self.dessin2d.terrain_points.get_mut(j).map(|p| &mut p[2]),
                            };
                            if let Some(z) = z {
                                ui.add(egui::DragValue::new(z).speed(10.0).dimensions()).on_hover_text("Altitude absolue");
                            }
                            if let Some(j) = k.checked_sub(sommets.len()) && ui.small_button("×").on_hover_text("Retirer ce point").clicked() {
                                retrait = Some(j);
                            }
                        });
                    });
                });
        }
        if let Some(j) = retrait {
            self.dessin2d.terrain_points.remove(j);
        }
    }

    /// Options et actions de la commande active, regroupées en haut à gauche de la vue.
    pub(super) fn barre_commande(&mut self, ui: &egui::Ui, rect: egui::Rect) {
        let edition = self.dessin2d.edition_contour.is_some();
        let actif = self.ruban_edition.outil().is_some() || edition;
        let options_dessin = !edition && self.usage_dessin().is_some_and(|(u, _)| u != Usage::Annotation);
        let options_modifier = !edition && self.a_options_modifier();
        if self.natif.is_none() || (!actif && !options_modifier) {
            return;
        }
        let mut action = None;
        egui::Area::new(egui::Id::unique("barre_commande"))
            .fixed_pos(rect.left_top() + egui::vec2(8.0, 8.0))
            .constrain_to(rect)
            .show(ui.ctx(), |ui| {
                egui::Frame::popup(ui.style()).show(ui, |ui| {
                    ui.horizontal_top(|ui| {
                        if edition || options_dessin || options_modifier {
                            egui::ScrollArea::horizontal().id_salt("options_barre_commande")
                                .max_width((rect.width() - if actif { 190.0 } else { 30.0 }).max(80.0))
                                .max_height(75.0)
                                .auto_shrink([true, true]).show(ui, |ui| {
                                    if edition { self.options_edition_contour(ui); }
                                    else if options_dessin { self.options_trace(ui); }
                                    else { self.options_modifier(ui); }
                                });
                        }
                        if actif {
                            if edition || options_dessin || options_modifier { ui.separator(); }
                            if bouton_commande(ui, true, "Valider").on_hover_text("Terminer et quitter la commande (Entrée)").clicked() {
                                action = Some(true);
                            }
                            if bouton_commande(ui, false, "Annuler").on_hover_text("Abandonner et quitter la commande (Échap)").clicked() {
                                action = Some(false);
                            }
                        }
                    });
                });
            });
        if let Some(valider) = action {
            self.quitter_commande(valider);
        }
    }

    /// Entrée : point saisi (coordonnées, distance), mot-clé F (fermer), ou fin du tracé.
    pub(super) fn valider_saisie_dessin(&mut self) {
        let saisie = std::mem::take(&mut self.dessin2d.saisie);
        if !saisie.is_empty() && self.dessin2d.edition_contour.as_ref().is_some_and(|e| e.raccord.is_some()) {
            let precedent = self.dessin2d.trace.as_ref().and_then(Trace::precedent);
            let direction = self.dessin2d.point.zip(precedent).map(|((c, _), p)| c - p);
            match d2::lire_saisie(&saisie, precedent, direction, crate::options::unite().facteur()) {
                Ok(p) => { self.ajouter_point_raccord(p); self.dessin2d.message = None; }
                Err(e) => self.dessin2d.message = Some(e),
            }
            return;
        }
        let Some(trace) = self.dessin2d.trace.as_mut() else { return };
        self.dessin2d.message = None;
        let formes = if saisie.is_empty() {
            trace.terminer()
        } else if saisie.eq_ignore_ascii_case("f") {
            trace.fermer()
        } else {
            let precedent = trace.precedent();
            let direction = self.dessin2d.point.zip(precedent).map(|((c, _), p)| c - p);
            match d2::lire_saisie(&saisie, precedent, direction, crate::options::unite().facteur()) {
                Ok(p) => trace.ajouter(p),
                Err(e) => {
                    self.dessin2d.message = Some(e);
                    Vec::new()
                }
            }
        };
        self.enregistrer_formes(formes);
    }

    /// Aperçu du terrain en cours, mis à jour à chaque glissé : surface teintée selon la
    /// cote (bas : vert, haut : sable), courbes de niveau à un pas rond, contour fin.
    fn apercu_terrain(&self, painter: &egui::Painter, rect: egui::Rect) {
        let Some(contour) = self.dessin2d.dalle.first() else { return };
        let ngf = self.natif.as_ref().map_or(0.0, |(p, _)| p.ngf());
        let bleu = Color32::from_rgb(0, 120, 215);
        let Some(Ok(tris)) = self.terrain_courant().map(|t| t.triangles()) else {
            let pts: Vec<DVec2> = contour.iter().map(|p| DVec2::from_array(*p)).collect();
            self.apercu_surface(painter, rect, std::slice::from_ref(&pts));
            return;
        };
        let (bas, haut) = tris.iter().flatten().fold((f64::MAX, f64::MIN), |(a, b), p| (a.min(p.z), b.max(p.z)));
        let etendue = (haut - bas).max(1e-6);
        let teinte = |z: f64| {
            let t = ((z - bas) / etendue) as f32;
            let (a, b, u) = if t < 0.5 { ([62.0, 138.0, 112.0], [150.0, 182.0, 96.0], t * 2.0) } else { ([150.0, 182.0, 96.0], [200.0, 158.0, 104.0], t * 2.0 - 1.0) };
            let c: [f32; 3] = std::array::from_fn(|k| a[k] + (b[k] - a[k]) * u);
            Color32::from_rgba_unmultiplied(c[0] as u8, c[1] as u8, c[2] as u8, 130)
        };
        let ecran = |p: DVec3| self.ecran_3d(rect, DVec3::new(p.x, p.y, p.z - ngf));
        let mut maillage = egui::Mesh::default();
        for t in &tris {
            let Some(e) = t.iter().map(|p| ecran(*p)).collect::<Option<Vec<_>>>() else { continue };
            let k = maillage.vertices.len() as u32;
            for (p, q) in t.iter().zip(e) {
                maillage.colored_vertex(q, teinte(p.z));
            }
            maillage.add_triangle(k, k + 1, k + 2);
        }
        painter.add(egui::Shape::mesh(maillage));
        // courbes de niveau : une dizaine au plus, au pas rond le plus fin
        if haut - bas > 1.0 {
            let pas = [50.0, 100.0, 200.0, 250.0, 500.0, 1000.0, 2000.0, 5000.0, 10_000.0].into_iter().find(|p| etendue / p <= 12.0).unwrap_or(20_000.0);
            let trait_ = egui::Stroke::new(1.0, Color32::from_white_alpha(150));
            for t in &tris {
                let mut z = (t.iter().map(|p| p.z).fold(f64::MAX, f64::min) / pas).ceil() * pas;
                let zmax = t.iter().map(|p| p.z).fold(f64::MIN, f64::max);
                while z < zmax {
                    let coupe: Vec<DVec3> = (0..3).filter_map(|i| {
                        let (a, b) = (t[i], t[(i + 1) % 3]);
                        ((a.z - z) * (b.z - z) < 0.0).then(|| a + (b - a) * ((z - a.z) / (b.z - a.z)))
                    }).collect();
                    if let [a, b] = coupe[..] && let (Some(a), Some(b)) = (ecran(a), ecran(b)) {
                        painter.line_segment([a, b], trait_);
                    }
                    z += pas;
                }
            }
        }
        let bord: Vec<egui::Pos2> = contour.iter().zip(&self.dessin2d.terrain_cotes).filter_map(|(p, z)| ecran(DVec3::new(p[0], p[1], *z))).collect();
        painter.add(egui::Shape::closed_line(bord, egui::Stroke::new(1.2, bleu.gamma_multiply(0.8))));
    }

    /// Poignées du terrain en cours à l'écran : sommets, points cotés, milieux des côtés.
    fn poignees_terrain(&self, rect: egui::Rect) -> Vec<(PoigneeTerrain, egui::Pos2)> {
        let Some(contour) = self.dessin2d.dalle.first() else { return Vec::new() };
        let n = contour.len();
        let ecran = |p: DVec2| self.vers_ecran(rect, p);
        let sommets = contour.iter().enumerate().filter_map(|(i, p)| Some((PoigneeTerrain::Sommet(i), ecran(DVec2::from_array(*p))?)));
        let points = self.dessin2d.terrain_points.iter().enumerate().filter_map(|(i, p)| Some((PoigneeTerrain::Point(i), ecran(DVec2::new(p[0], p[1]))?)));
        let milieux = (0..n).filter_map(|i| Some((PoigneeTerrain::Milieu(i), ecran((DVec2::from_array(contour[i]) + DVec2::from_array(contour[(i + 1) % n])) * 0.5)?)));
        sommets.chain(points).chain(milieux).collect()
    }

    /// Terrain en cours : survol des poignées, glissé d'un sommet ou d'un point (un milieu
    /// insère un sommet à la cote moyenne), clic droit : retirer, clic : point coté si
    /// « Placer des points cotés ». Un glissé qui rend le terrain impossible est repris.
    fn souris_terrain(&mut self, ui: &egui::Ui, rect: egui::Rect, resp: &egui::Response) {
        let poignees = self.poignees_terrain(rect);
        let proche = |q: egui::Pos2| poignees.iter().filter(|(_, e)| e.distance(q) < 10.0).min_by(|a, b| a.1.distance(q).total_cmp(&b.1.distance(q))).map(|(p, _)| *p);
        if self.dessin2d.terrain_glisse.is_none() {
            self.dessin2d.terrain_survol = resp.hover_pos().and_then(proche);
        }
        let d = &mut self.dessin2d;
        if resp.drag_started_by(PointerButton::Primary) && let Some(p) = ui.input(|i| i.pointer.press_origin()).and_then(proche) {
            let avant = (d.dalle[0].clone(), d.terrain_cotes.clone(), d.terrain_points.clone());
            let p = match p {
                PoigneeTerrain::Milieu(i) => {
                    let n = d.dalle[0].len();
                    let (a, b) = (DVec2::from_array(d.dalle[0][i]), DVec2::from_array(d.dalle[0][(i + 1) % n]));
                    d.dalle[0].insert(i + 1, ((a + b) * 0.5).to_array());
                    let z = (d.terrain_cotes[i] + d.terrain_cotes[(i + 1) % n]) * 0.5;
                    d.terrain_cotes.insert(i + 1, z);
                    PoigneeTerrain::Sommet(i + 1)
                }
                p => p,
            };
            d.terrain_glisse = Some((p, avant.0, avant.1, avant.2));
            d.terrain_survol = Some(p);
        }
        if let Some((p, ..)) = self.dessin2d.terrain_glisse
            && let Some(q) = resp.interact_pointer_pos().and_then(|q| self.depuis_ecran(rect, q))
        {
            let d = &mut self.dessin2d;
            match p {
                PoigneeTerrain::Sommet(i) => if let Some(s) = d.dalle[0].get_mut(i) { *s = q.to_array(); },
                PoigneeTerrain::Point(i) => if let Some(s) = d.terrain_points.get_mut(i) { s[0] = q.x; s[1] = q.y; },
                PoigneeTerrain::Milieu(_) => {}
            }
        }
        if resp.drag_stopped() && let Some((_, contour, cotes, points)) = self.dessin2d.terrain_glisse.take() {
            match self.terrain_courant().map(|t| t.triangles()) {
                Some(Err(e)) => {
                    let d = &mut self.dessin2d;
                    (d.dalle[0], d.terrain_cotes, d.terrain_points) = (contour, cotes, points);
                    d.message = Some(e);
                }
                _ => self.dessin2d.message = None,
            }
        }
        // clic droit sur une poignée : retirer le point ou le sommet (trois au moins)
        if resp.secondary_clicked() && let Some(p) = resp.interact_pointer_pos().and_then(proche) {
            let d = &mut self.dessin2d;
            match p {
                PoigneeTerrain::Point(i) => { d.terrain_points.remove(i); }
                PoigneeTerrain::Sommet(i) if d.dalle[0].len() > 3 => {
                    let avant = (d.dalle[0].clone(), d.terrain_cotes.clone());
                    d.dalle[0].remove(i);
                    d.terrain_cotes.remove(i);
                    if let Some(Err(e)) = self.terrain_courant().map(|t| t.triangles()) {
                        (self.dessin2d.dalle[0], self.dessin2d.terrain_cotes) = avant;
                        self.dessin2d.message = Some(e);
                    }
                }
                PoigneeTerrain::Sommet(_) => self.dessin2d.message = Some("Le contour garde au moins trois sommets.".into()),
                PoigneeTerrain::Milieu(_) => {}
            }
            self.dessin2d.terrain_survol = None;
        }
        if self.dessin2d.terrain_ajouter_point && resp.clicked() && self.dessin2d.terrain_survol.is_none()
            && let Some(p) = resp.interact_pointer_pos().and_then(|q| self.depuis_ecran(rect, q))
        {
            self.dessin2d.terrain_points.push([p.x, p.y, self.dessin2d.terrain_altitude]);
            match self.terrain_courant().map(|t| t.triangles()) {
                Some(Err(e)) => { self.dessin2d.terrain_points.pop(); self.dessin2d.message = Some(e); }
                _ => self.dessin2d.message = None,
            }
        }
        if self.dessin2d.terrain_survol.is_some() {
            ui.ctx().set_cursor_icon(if self.dessin2d.terrain_glisse.is_some() { egui::CursorIcon::Grabbing } else { egui::CursorIcon::Grab });
        }
    }

    /// Souris dans la vue avec un outil de dessin 2D : point accroché sous le curseur,
    /// clic gauche : point, clic droit : terminer (comme Entrée).
    pub(super) fn souris_dessin(&mut self, ui: &egui::Ui, rect: egui::Rect, resp: &egui::Response) {
        if self.usage_dessin().is_some_and(|(u,_)|u==Usage::Mur) && self.ruban_edition.selection_mur {
            self.dessin2d.point=None;
            self.dessin2d.ligne_selection=resp.hover_pos().and_then(|q|self.trait_mur_vise(rect,q));
            if resp.clicked() {
                if let Some(f)=self.dessin2d.ligne_selection.clone() {
                    if let d2::Forme2D::Cercle{centre,rayon}=f {
                        self.ruban_edition.selection_mur=false;
                        self.ruban_edition.forme_mur=OutilDessin::Cercle;
                        let mut t=Trace::new(OutilDessin::Cercle);t.perimetre=true;t.points=vec![centre,centre+DVec2::X*rayon];
                        self.dessin2d.trace=Some(t);
                    } else {self.enregistrer_formes(vec![f]);}
                } else {self.dessin2d.message=Some("Sélectionnez un trait du dessin dans la vue.".into());}
            }
            return;
        }
        self.dessin2d.ligne_selection=None;
        if !self.dessin2d.dalle.is_empty() && self.usage_dessin().is_some_and(|(u, _)| u == Usage::Terrain) {
            self.souris_terrain(ui, rect, resp);
            self.dessin2d.point = None;
            return;
        }
        if self.dessin2d.edition_contour.as_ref().is_some_and(|e| !e.dessin) {
            let curseur = resp.hover_pos().or(resp.interact_pointer_pos());
            self.dessin2d.curseur = curseur.and_then(|q| self.depuis_ecran(rect, q));
            let temps = ui.input(|i| i.time);
            let maj = ui.input(|i| i.modifiers.shift);
            self.dessin2d.point = self.dessin2d.curseur.map(|c| self.point_dessin(rect, c, maj, temps));
            if self.dessin2d.survol_accroche.is_some() { ui.ctx().request_repaint_after(std::time::Duration::from_millis(100)); }
            let q = if resp.drag_started_by(PointerButton::Primary) { ui.input(|i| i.pointer.press_origin()).or(resp.interact_pointer_pos()) } else { resp.interact_pointer_pos().or(resp.hover_pos()) };
            let mut sommet: Option<(f32, usize, usize)> = None;
            let mut arete: Option<(f32, usize, usize, egui::Pos2)> = None;
            if let Some(q) = q {
                for (ci, c) in self.dessin2d.dalle.iter().enumerate() {
                    let fermee = self.dessin2d.edition_contour.as_ref().unwrap().fermee(ci);
                    for i in 0..c.len() {
                        if let Some(a) = self.vers_ecran(rect, DVec2::from_array(c[i])) {
                            let distance = q.distance(a);
                            if distance < 10.0 && sommet.is_none_or(|s| distance < s.0) { sommet = Some((distance, ci, i)); }
                            if !fermee && i + 1 == c.len() { continue; }
                            let Some(b) = self.vers_ecran(rect, DVec2::from_array(c[(i + 1) % c.len()])) else { continue };
                            let v = b - a;
                            let t = if v.length_sq() > 0.0 { ((q - a).dot(v) / v.length_sq()).clamp(0.0, 1.0) } else { 0.0 };
                            let p = a + v * t;
                            let distance = q.distance(p);
                            if distance < 8.0 && arete.is_none_or(|s| distance < s.0) { arete = Some((distance, ci, i, p)); }
                        }
                    }
                }
            }
            if resp.drag_started_by(PointerButton::Primary) {
                if let Some((_, c, i)) = sommet {
                    let e = self.dessin2d.edition_contour.as_mut().unwrap();
                    e.selection = Some(c);
                    e.segment = None;
                    e.glisse = Some(GlisseContour::Sommet(c, i));
                } else if let (Some((_, c, i, _)), Some(q)) = (arete, q) {
                    if let Some(origine) = self.depuis_ecran(rect, q) {
                        let a = DVec2::from_array(self.dessin2d.dalle[c][i]);
                        let b = DVec2::from_array(self.dessin2d.dalle[c][(i + 1) % self.dessin2d.dalle[c].len()]);
                        let e = self.dessin2d.edition_contour.as_mut().unwrap();
                        e.selection = Some(c);
                        e.segment = Some((c, i));
                        e.glisse = Some(GlisseContour::Segment { contour: c, segment: i, origine, a, b });
                    }
                }
            }
            let glisse = self.dessin2d.edition_contour.as_ref().and_then(|e| e.glisse);
            let curseur = resp.interact_pointer_pos().or(resp.hover_pos());
            if let (Some(glisse), Some(q)) = (glisse, curseur) {
                if let Some(p) = self.depuis_ecran(rect, q) {
                    match glisse {
                        GlisseContour::Sommet(c, i) => {
                            let actuel = DVec2::from_array(self.dessin2d.dalle[c][i]);
                            let p = self.dessin2d.point.map(|x| x.0).filter(|q| q.distance(actuel) > 1e-6).unwrap_or(p);
                            self.dessin2d.dalle[c][i] = [(p.x * 10.0).round() / 10.0, (p.y * 10.0).round() / 10.0];
                        }
                        GlisseContour::Segment { contour, segment, origine, a, b } => {
                            let delta = p - origine;
                            let suivant = (segment + 1) % self.dessin2d.dalle[contour].len();
                            self.dessin2d.dalle[contour][segment] = (a + delta).to_array();
                            self.dessin2d.dalle[contour][suivant] = (b + delta).to_array();
                        }
                    }
                }
            }
            if resp.drag_stopped() { self.dessin2d.edition_contour.as_mut().unwrap().glisse = None; }
            if resp.clicked_by(PointerButton::Secondary) {
                if let Some((_, c, i)) = sommet {
                    let fermee = self.dessin2d.edition_contour.as_ref().unwrap().fermee(c);
                    if self.dessin2d.dalle[c].len() > if fermee { 3 } else { 2 } { self.dessin2d.dalle[c].remove(i); self.dessin2d.edition_contour.as_mut().unwrap().segment = None; }
                    else { self.dessin2d.message = Some("Le tracé ne peut pas comporter moins de sommets.".into()); }
                }
            } else if resp.double_clicked() {
                if let Some((_, c, i, q)) = arete.filter(|_| sommet.is_none()) {
                    if let Some(p) = self.depuis_ecran(rect, q) { self.dessin2d.dalle[c].insert(i + 1, p.to_array()); }
                }
            } else if resp.clicked() {
                if let Some((_, c, i)) = sommet {
                    let e = self.dessin2d.edition_contour.as_ref().unwrap();
                    if !e.fermee(c) && (i == 0 || i + 1 == self.dessin2d.dalle[c].len()) {
                        let p = DVec2::from_array(self.dessin2d.dalle[c][i]);
                        let e = self.dessin2d.edition_contour.as_mut().unwrap();
                        e.raccord = Some((c, i == 0));
                        e.dessin = true;
                        self.dessin2d.trace.as_mut().unwrap().points = vec![p];
                        self.dessin2d.message = None;
                        return;
                    }
                }
                let e = self.dessin2d.edition_contour.as_mut().unwrap();
                e.selection = sommet.map(|s| s.1).or(arete.map(|a| a.1));
                e.segment = if sommet.is_none() { arete.map(|a| (a.1, a.2)) } else { None };
            }
            return;
        }
        let maj = ui.input(|i| i.modifiers.shift);
        let survol = resp.hover_pos().or(resp.interact_pointer_pos());
        self.dessin2d.curseur = survol.and_then(|q| self.depuis_ecran(rect, q));
        let temps = ui.input(|i| i.time);
        self.dessin2d.point = self.dessin2d.curseur.map(|c| self.point_dessin(rect, c, maj, temps));
        if self.dessin2d.survol_accroche.is_some() { ui.ctx().request_repaint_after(std::time::Duration::from_millis(100)); }
        let angle = self.angle_axes_plan();
        if let Some(t) = self.dessin2d.trace.as_mut() {
            t.angle = angle;
        }
        if resp.clicked() {
            if let Some((p, _)) = self.dessin2d.point && self.ajouter_point_raccord(p) { return; }
            if let (Some((p, _)), Some(t)) = (self.dessin2d.point, self.dessin2d.trace.as_mut()) {
                self.dessin2d.saisie.clear();
                self.dessin2d.message = None;
                let formes = t.ajouter(p);
                self.enregistrer_formes(formes);
            }
        }
        if resp.clicked_by(PointerButton::Secondary) {
            self.valider_saisie_dessin();
        }
        if resp.hovered() {
            ui.ctx().set_cursor_icon(egui::CursorIcon::Crosshair);
        }
    }

    /// Tracé par-dessus la vue : réticule, aperçu élastique, repérage polaire, marque et
    /// nom de l'accrochage, cote dynamique, ligne de commande.
    pub(super) fn dessiner_dessin2d(&self, ui: &egui::Ui, rect: egui::Rect) {
        if self.ruban_edition.selection_mur && self.usage_dessin().is_some_and(|(u,_)|u==Usage::Mur) {
            if let Some(f)=&self.dessin2d.ligne_selection {
                let painter=ui.painter().with_clip_rect(rect);
                let lignes=f.polylignes();
                self.apercu_volume_murs(&painter,rect,&lignes);
                for l in lignes {
                    let pts=l.iter().filter_map(|p|self.vers_ecran(rect,*p)).collect();
                    painter.add(egui::Shape::line(pts,egui::Stroke::new(2.5,Color32::from_rgb(0,120,215))));
                }
            }
            return;
        }
        let Some(trace) = &self.dessin2d.trace else { return };
        let painter = ui.painter().with_clip_rect(rect);
        let bleu = Color32::from_rgb(0, 120, 215);
        let vert = Color32::from_rgb(20, 160, 60);
        for &(origine, direction) in &self.dessin2d.guides {
            if let (Some(a), Some(b)) = (self.vers_ecran(rect, origine), self.vers_ecran(rect, origine + direction * 1000.0)) {
                let d = (b - a).normalized();
                painter.add(egui::Shape::dashed_line(&[a - d * 4000.0, a + d * 4000.0], egui::Stroke::new(1.0, vert), 6.0, 4.0));
            }
        }
        if let Some(p) = self.dessin2d.reference.and_then(|p| self.vers_ecran(rect, p)) {
            painter.circle_stroke(p, 5.0, egui::Stroke::new(1.5, vert));
        }
        if let Some((p, accroche)) = self.dessin2d.point {
            if let Some(e) = self.vers_ecran(rect, p) {
                // réticule
                if !self.selection_traces_active() {
                    let gris = Color32::from_rgba_unmultiplied(60, 60, 60, 160);
                    painter.line_segment([e - egui::vec2(28.0, 0.0), e + egui::vec2(28.0, 0.0)], egui::Stroke::new(1.0, gris));
                    painter.line_segment([e - egui::vec2(0.0, 28.0), e + egui::vec2(0.0, 28.0)], egui::Stroke::new(1.0, gris));
                }
                // aperçu : ligne tracée, et volume pour un mur
                let apercu = trace.apercu(p);
                match self.usage_dessin().map(|(u, _)| u) {
                    Some(Usage::Mur) if trace.outil!=OutilDessin::Arc || trace.points.len()>=2 => self.apercu_volume_murs(&painter, rect, &apercu),
                    Some(Usage::Surface | Usage::LimitePropriete | Usage::Cadrage) => self.apercu_surface(&painter, rect, &apercu),
                    Some(Usage::Escalier) => self.apercu_escalier(&painter, rect, &apercu),
                    _ => {}
                }
                if trace.perimetre && trace.outil==OutilDessin::Cercle {
                    if let Some(c)=trace.points.first().and_then(|c|self.vers_ecran(rect,*c)) {
                        painter.add(egui::Shape::dashed_line(&[c,e],egui::Stroke::new(1.0,bleu.gamma_multiply(0.6)),5.0,4.0));
                    }
                }
                for l in apercu {
                    let pts: Vec<egui::Pos2> = l.iter().filter_map(|q| self.vers_ecran(rect, *q)).collect();
                    painter.add(egui::Shape::line(pts, egui::Stroke::new(1.5, bleu)));
                }
                // repérage polaire / ortho : ligne de construction en pointillés
                if self.dessin2d.guides.is_empty() && let (Some(prec), Accroche::Polaire(_) | Accroche::Ortho) = (trace.precedent(), accroche) {
                    if let Some(a) = self.vers_ecran(rect, prec) {
                        let dir = (e - a).normalized();
                        painter.add(egui::Shape::dashed_line(&[a - dir * 4000.0, a + dir * 4000.0], egui::Stroke::new(1.0, vert), 6.0, 4.0));
                    }
                }
                // marque d'accrochage
                let m = 6.0;
                let trait_ = egui::Stroke::new(2.0, vert);
                match accroche {
                    Accroche::Extremite => {
                        painter.rect_stroke(egui::Rect::from_center_size(e, egui::vec2(2.0 * m, 2.0 * m)), 0.0, trait_, egui::StrokeKind::Middle);
                    }
                    Accroche::Milieu => {
                        painter.add(egui::Shape::closed_line(vec![e + egui::vec2(0.0, -m), e + egui::vec2(m, m), e + egui::vec2(-m, m)], trait_));
                    }
                    Accroche::Centre => {
                        painter.circle_stroke(e, m, trait_);
                    }
                    Accroche::Quadrant => {
                        painter.add(egui::Shape::closed_line(vec![e + egui::vec2(0.0, -m), e + egui::vec2(m, 0.0), e + egui::vec2(0.0, m), e + egui::vec2(-m, 0.0)], trait_));
                    }
                    Accroche::Intersection => {
                        painter.line_segment([e + egui::vec2(-m, -m), e + egui::vec2(m, m)], trait_);
                        painter.line_segment([e + egui::vec2(-m, m), e + egui::vec2(m, -m)], trait_);
                    }
                    Accroche::Perpendiculaire => {
                        painter.add(egui::Shape::line(vec![e + egui::vec2(-m, -m), e + egui::vec2(-m, m), e + egui::vec2(m, m)], trait_));
                        painter.line_segment([e + egui::vec2(-m, 0.0), e + egui::vec2(0.0, 0.0)], trait_);
                    }
                    Accroche::Proche => {
                        painter.add(egui::Shape::closed_line(vec![e + egui::vec2(-m, -m), e + egui::vec2(m, -m), e + egui::vec2(-m, m), e + egui::vec2(m, m)], trait_));
                    }
                    _ => {}
                }
                // étiquettes : accrochage, longueur et angle depuis le point précédent
                let mut infos = Vec::new();
                if !self.dessin2d.saisie.is_empty() { infos.push(format!("{}▏", self.dessin2d.saisie)); }
                if accroche != Accroche::Libre {
                    infos.push(accroche.libelle());
                }
                if let Some(prec) = trace.precedent() {
                    let v = p - prec;
                    let u = crate::options::unite();
                    let decimales = u.facteur().log10().round().max(0.0) as usize;
                    infos.push(format!("{:.*}{} · {:.1}°", decimales, v.length() / u.facteur(), u.suffixe(), v.y.atan2(v.x).to_degrees().rem_euclid(360.0)));
                }
                if !infos.is_empty() {
                    let texte = painter.layout_no_wrap(infos.join("   "), egui::FontId::proportional(12.0), Color32::WHITE);
                    let cadre = egui::Rect::from_min_size(e + egui::vec2(16.0, 14.0), texte.size() + egui::vec2(10.0, 4.0));
                    painter.rect_filled(cadre, 3.0, Color32::from_rgba_unmultiplied(40, 40, 40, 220));
                    painter.galley(cadre.min + egui::vec2(5.0, 2.0), texte, Color32::WHITE);
                }
            }
        }
        // contours déjà fermés de l'élément horizontal en cours (parties et trémies)
        if self.dessin2d.edition_contour.is_some() || self.usage_dessin().is_some_and(|(u, _)| matches!(u, Usage::Surface | Usage::LimitePropriete | Usage::Cadrage)) {
            for (i, c) in self.dessin2d.dalle.iter().enumerate() {
                let pts: Vec<DVec2> = c.iter().map(|p| DVec2::from_array(*p)).collect();
                if let Some(edit) = &self.dessin2d.edition_contour {
                    let couleur = if edit.selection == Some(i) { Color32::from_rgb(65, 135, 190) } else { bleu };
                    let mut ecran: Vec<_> = pts.iter().filter_map(|p| self.vers_ecran(rect, *p)).collect();
                    if edit.fermee(i) && let Some(p) = ecran.first().copied() { ecran.push(p); }
                    painter.add(egui::Shape::line(ecran.clone(), egui::Stroke::new(2.0, couleur)));
                    if let Some((c, k)) = edit.segment && c == i && k + 1 < ecran.len() {
                        painter.line_segment([ecran[k], ecran[k + 1]], egui::Stroke::new(4.0, Color32::from_rgb(230, 110, 20)));
                    }
                    if !edit.dessin { for p in ecran.iter().take(c.len()) { painter.circle_filled(*p, 4.0, couleur); } }
                } else {
                    self.apercu_surface(&painter, rect, std::slice::from_ref(&pts));
                    if let Some(ecran) = pts.iter().map(|&p| self.vers_ecran(rect, p)).collect::<Option<Vec<_>>>() {
                        painter.add(egui::Shape::closed_line(ecran, egui::Stroke::new(1.5, bleu)));
                    }
                }
            }
        }
        // terrain en cours : surface colorée selon la cote, contour fin, poignées ; les
        // cotes sont à côté de chaque point (cotes_terrain)
        if self.usage_dessin().is_some_and(|(u, _)| u == Usage::Terrain) {
            self.apercu_terrain(&painter, rect);
            for (p, q) in self.poignees_terrain(rect) {
                let survol = self.dessin2d.terrain_survol == Some(p);
                match p {
                    PoigneeTerrain::Sommet(_) | PoigneeTerrain::Point(_) => {
                        let c = if matches!(p, PoigneeTerrain::Point(_)) { vert } else { bleu };
                        painter.circle_filled(q, if survol { 7.0 } else { 5.0 }, if survol { c } else { Color32::WHITE });
                        painter.circle_stroke(q, if survol { 7.0 } else { 5.0 }, egui::Stroke::new(2.0, c));
                    }
                    // milieu : insérer un sommet en le glissant
                    PoigneeTerrain::Milieu(_) => {
                        let r = if survol { 5.0 } else { 3.5 };
                        painter.circle_filled(q, r, Color32::from_white_alpha(if survol { 255 } else { 200 }));
                        painter.circle_stroke(q, r, egui::Stroke::new(1.2, bleu.gamma_multiply(if survol { 1.0 } else { 0.6 })));
                        if survol {
                            painter.line_segment([q - egui::vec2(2.5, 0.0), q + egui::vec2(2.5, 0.0)], egui::Stroke::new(1.2, bleu));
                            painter.line_segment([q - egui::vec2(0.0, 2.5), q + egui::vec2(0.0, 2.5)], egui::Stroke::new(1.2, bleu));
                        }
                    }
                }
            }
        }
        // points déjà donnés
        for q in &trace.points {
            if let Some(e) = self.vers_ecran(rect, *q) {
                painter.circle_filled(e, 3.0, bleu);
            }
        }
    }

    /// Instructions de la commande active, dans la bande noire au bas de la vue.
    pub(super) fn dessiner_instructions(&self, ui: &egui::Ui, rect: egui::Rect) {
        use crate::ruban::Outil;
        let painter = ui.painter().with_clip_rect(rect);
        let edition = self.dessin2d.edition_contour.as_ref();
        let usage = self.usage_dessin();
        let outil = self.ruban_edition.outil();
        let modification = if self.orientation_nord_en_cours { Some("Viser un trait puis cliquer près de l'extrémité indiquant le nord · Échap : annuler") } else { match outil {
            Some(Outil::Deplacer | Outil::Copier) => Some(if self.bim.depart_deplacement.is_some() { "Cliquer la destination · Maj : orthogonal · Échap ou clic droit : annuler" } else { "Cliquer le point de base, puis la destination" }),
            Some(Outil::Tourner) => Some("Régler le centre et l'angle, puis appliquer la rotation."),
            Some(Outil::Aligner) => Some("Choisir une référence, puis appliquer à l'axe."),
            Some(Outil::Ajuster) if self.ruban_edition.prolonger_angle => Some(if self.ajustement.as_ref().is_some_and(|a|a.reference.is_some()) {"Cliquer près de la seconde extrémité · Les deux murs sont raccordés · Aperçu bleu"} else {"Prolonger en angle : cliquer près de la première extrémité à raccorder"}),
            Some(Outil::Ajuster) => Some(if self.ajustement.as_ref().is_some_and(|a|a.reference.is_some()) {"Cliquer près de l'extrémité du mur à ajuster · Aperçu bleu · Échap / clic droit : changer de limite"} else {"Cliquer le mur de référence (son axe devient la limite) · Échap / clic droit : terminer"}),
            _ => None,
        }};
        if edition.is_none() && usage.is_none() && modification.is_none() { return; }
        let u = crate::options::unite();
        let nom = if self.orientation_nord_en_cours { "NORD DU PROJET".into() } else { match usage {
            Some((Usage::Mur, o)) => format!("MUR · {}", nom_outil(o)),
            Some((Usage::Escalier, _)) => "ESCALIER · LIGNE DE FOULÉE".into(),
            Some((Usage::GardeCorps, _)) => "GARDE-CORPS".into(),
            Some((Usage::LimitePropriete, o)) => format!("LIMITE DE PROPRIÉTÉ · {}", if o == OutilDessin::Polyligne { "POLYGONE" } else { nom_outil(o) }),
            Some((Usage::Cadrage, _)) => "CADRAGE DE LA VUE".into(),
            Some((Usage::Surface, o)) => format!(
                "{} · {}",
                self.ruban_edition.dalle_active().map_or("SURFACE".into(), |d| d.sorte.nom().to_uppercase()),
                if o == OutilDessin::Polyligne { "POLYGONE" } else { nom_outil(o) }
            ),
            _ => self.dessin2d.trace.as_ref().map_or("MODIFICATION".into(), |t| nom_outil(t.outil).into()),
        }};
        let invite = if let Some(e) = edition.filter(|_| self.selection_traces_active()) {
            if e.segment.is_some() { "Trait sélectionné : Suppr pour le retirer ; glisser un sommet pour le déplacer" }
            else { "Sélectionner un trait ou glisser un sommet ; cliquer un sommet ouvert pour prolonger" }
        } else if usage.is_some_and(|(u, _)| u == Usage::Terrain) && !self.dessin2d.dalle.is_empty() {
            if self.dessin2d.terrain_ajouter_point { "Cliquer dans le terrain pour placer un point coté · glisser un point pour le déplacer · clic droit : retirer · Entrée : valider" }
            else { "Glisser un sommet ou un point · glisser un milieu de côté pour ajouter un sommet · clic droit : retirer · cotes NGF à côté de chaque point · Entrée : valider" }
        } else if let Some(m) = modification { m }
        else if self.ruban_edition.selection_mur && usage.is_some_and(|(u,_)|u==Usage::Mur) {"Sélectionner un trait existant dans la vue · Échap : terminer"} else { self.dessin2d.trace.as_ref().map_or("", |t| t.invite()) };
        let ligne = match &self.dessin2d.message {
            Some(m) => format!("{nom} — {invite} : {m}"),
            None => format!("{nom} — {invite}"),
        };
        let contexte = match usage.map(|(u, _)| u) {
            Some(Usage::Mur) => "Espace : inverser le sens · ",
            Some(Usage::Escalier) => "Départ, un sommet par palier, arrivée · Marches calculées selon le niveau supérieur · Entrée ou clic droit : finir · ",
            Some(Usage::GardeCorps) => "Cliquer les sommets · Entrée ou clic droit : finir · Sur un escalier, sélectionner Garde-corps dans les options · ",
            Some(Usage::Cadrage) => "Tracer un contour fermé, puis valider · ",
            Some(Usage::Terrain) if self.dessin2d.dalle.is_empty() => "Tracer le périmètre fermé du terrain · ",
            _ => "",
        };
        let aide = if usage.is_some() { format!("{contexte}Clavier : x,y · @dx,dy · d<angle · distance seule ({}) · Maj : ortho · Entrée : valider · Échap : annuler", u.suffixe().trim()) } else { "Entrée : valider · Échap : annuler".into() };
        let police = egui::FontId::monospace(13.0);
        let largeur = (rect.width() - 20.0).max(1.0);
        let g1 = painter.layout(ligne, police, if self.dessin2d.message.is_some() { Color32::from_rgb(255, 170, 150) } else { Color32::WHITE }, largeur);
        let g2 = painter.layout(aide, egui::FontId::proportional(11.0), Color32::from_gray(200), largeur);
        let hauteur = g1.size().y + g2.size().y + 12.0;
        let bande = egui::Rect::from_min_max(egui::pos2(rect.left(), rect.bottom() - hauteur), rect.right_bottom());
        painter.rect_filled(bande, 0.0, Color32::from_rgba_unmultiplied(30, 30, 30, 225));
        let h1 = g1.size().y;
        painter.galley(bande.min + egui::vec2(10.0, 4.0), g1, Color32::WHITE);
        painter.galley(bande.min + egui::vec2(10.0, 8.0 + h1), g2, Color32::from_gray(200));
    }

    /// Volume des murs en cours : faces au sol et en tête, séparations des couches et
    /// arêtes verticales aux extrémités, le long de chaque polyligne de l'aperçu.
    fn apercu_volume_murs(&self, painter: &egui::Painter, rect: egui::Rect, lignes: &[Vec<DVec2>]) {
        let Some((projet, _)) = &self.natif else { return };
        let options = self.ruban_edition.parametres_mur();
        let Ok((bas, haut)) = options.bornes(projet) else { return };
        let gabarit = options.gabarit(projet);
        let e = gabarit.epaisseur();
        if e <= 0.0 || haut <= bas {
            return;
        }
        // milieu de l'épaisseur : décalé de la ligne tracée selon l'implantation
        let centre = -gabarit.decalage_reference(options.implantation.reference());
        let mut separations = vec![-0.5, 0.5];
        let mut k = 0.5;
        for c in gabarit.couches.iter().take(gabarit.couches.len().saturating_sub(1)) {
            k -= c.epaisseur / e;
            separations.push(k);
        }
        let couleur = Color32::from_rgb(55, 130, 200);
        let ecran = |q: DVec2, z: f64| self.ecran_3d(rect, DVec3::new(q.x, q.y, z));
        for l in lignes {
            for w in l.windows(2) {
                let (a, b) = (w[0], w[1]);
                let Some(n) = (b - a).try_normalize().map(|d| d.perp()) else { continue };
                let n = if options.inverse { -n } else { n };
                let c = |p: DVec2, k: f64| p + n * (centre + e * k);
                let sol: Option<Vec<_>> = [c(a, -0.5), c(b, -0.5), c(b, 0.5), c(a, 0.5)].iter().map(|&q| ecran(q, bas)).collect();
                if let Some(sol) = sol {
                    painter.add(egui::Shape::convex_polygon(sol, couleur.gamma_multiply(0.18), egui::Stroke::NONE));
                }
                for (i, &k) in separations.iter().enumerate() {
                    let trait_ = egui::Stroke::new(if i < 2 { 1.5 } else { 0.8 }, couleur.gamma_multiply(if i < 2 { 1.0 } else { 0.8 }));
                    for z in [bas, haut] {
                        if let (Some(p), Some(q)) = (ecran(c(a, k), z), ecran(c(b, k), z)) {
                            painter.line_segment([p, q], trait_);
                        }
                    }
                }
            }
            // arêtes verticales aux extrémités (pas pour une forme fermée)
            let ferme = l.len() > 2 && l.first().zip(l.last()).is_some_and(|(a, b)| a.distance(*b) < 1e-6);
            if ferme || l.len() < 2 {
                continue;
            }
            for (p, voisin) in [(l[0], l[1]), (l[l.len() - 1], l[l.len() - 2])] {
                let Some(n) = (p - voisin).try_normalize().map(|d| d.perp()) else { continue };
                let n = if options.inverse { -n } else { n };
                for k in [-0.5, 0.5] {
                    let q = p + n * (centre + e * k);
                    if let (Some(x), Some(y)) = (ecran(q, bas), ecran(q, haut)) {
                        painter.line_segment([x, y], egui::Stroke::new(1.0, couleur));
                    }
                }
            }
        }
    }

    /// Surface en cours : remplissage et aire du contour (fermé sur le curseur).
    fn apercu_surface(&self, painter: &egui::Painter, rect: egui::Rect, lignes: &[Vec<DVec2>]) {
        let couleur = Color32::from_rgb(55, 130, 200);
        for l in lignes {
            let mut pts = l.clone();
            if pts.len() > 2 && pts.first().zip(pts.last()).is_some_and(|(a, b)| a.distance(*b) < 1e-6) {
                pts.pop();
            }
            if pts.len() < 3 {
                continue;
            }
            let Some(ecr) = pts.iter().map(|&q| self.vers_ecran(rect, q)).collect::<Option<Vec<_>>>() else { continue };
            let contour: Vec<[f64; 2]> = pts.iter().map(|p| p.to_array()).collect();
            let aire = crate::modele::geometrie::aire_pair_impair(std::slice::from_ref(&contour));
            let mut m = egui::Mesh::default();
            for t in crate::texte::trianguler(vec![ecr.iter().map(|p| [p.x, p.y]).collect()]) {
                for p in t {
                    m.colored_vertex(egui::pos2(p[0], p[1]), couleur.gamma_multiply(0.18));
                }
            }
            m.indices = (0..m.vertices.len() as u32).collect();
            painter.add(egui::Shape::mesh(m));
            let mut ferme = ecr.clone();
            ferme.push(ecr[0]);
            painter.add(egui::Shape::dashed_line(&ferme[ferme.len() - 2..], egui::Stroke::new(1.0, couleur), 5.0, 4.0));
            let c = ecr.iter().fold(egui::Vec2::ZERO, |s, p| s + p.to_vec2()) / ecr.len() as f32;
            let galee = painter.layout_no_wrap(format!("{:.2} m²", aire / 1e6), egui::FontId::proportional(13.0), Color32::from_gray(30));
            let cadre = egui::Rect::from_center_size(c.to_pos2(), galee.size() + egui::vec2(12.0, 6.0));
            painter.rect(cadre, 3.0, Color32::from_rgba_unmultiplied(255, 255, 255, 235), egui::Stroke::new(1.0, couleur), egui::StrokeKind::Inside);
            painter.galley(cadre.min + egui::vec2(6.0, 3.0), galee, Color32::from_gray(30));
        }
    }

    /// Barre d'options du tracé, en haut de la vue : forme du mur (et chaînage) ou de
    /// l'élément horizontal.
    pub(super) fn reinitialiser_forme_cadrage(&mut self) {
        self.dessin2d.forme_cadrage=Some(OutilDessin::Rectangle);
        self.dessin2d.trace=None;
        self.dessin2d.dalle.clear();
    }

    pub(super) fn options_trace(&mut self, ui: &mut egui::Ui) {
        match self.usage_dessin().map(|(u, _)| u) {
            Some(Usage::Mur) => self.options_trace_mur(ui),
            Some(Usage::Escalier | Usage::GardeCorps) => {}
            Some(Usage::Cadrage) => { ui.horizontal(|ui| {
                let forme=self.dessin2d.forme_cadrage.unwrap_or(OutilDessin::Rectangle);
                for (valeur,nom) in [(OutilDessin::Rectangle,"Rectangle"),(OutilDessin::Polyligne,"Ligne")] {
                    if ui.selectable_label(forme==valeur,nom).clicked() {self.dessin2d.forme_cadrage=Some(valeur);self.dessin2d.trace=None;self.dessin2d.dalle.clear();}
                }
            }); }
            Some(Usage::Terrain) => {
                // type (épaisseur) ; cotes NGF des sommets et des points : à côté de chacun
                // dans la vue
                let types = self.natif.as_ref().map(|(p, _)| p.types_terrains_disponibles()).unwrap_or_default();
                ui.horizontal(|ui| {
                    ui.label("Type");
                    let actuel = self.dessin2d.terrain_type.as_ref().and_then(|id| types.iter().find(|t| &t.id == id)).or(types.first());
                    let libelle = |t: &crate::modele::site::TypeTerrain| format!("{} · {}", t.nom, crate::options::unite().afficher(t.epaisseur));
                    egui::ComboBox::from_id_salt("type_terrain_trace").selected_text(actuel.map(libelle).unwrap_or_default()).show_ui(ui, |ui| {
                        for t in &types {
                            if ui.selectable_label(actuel.is_some_and(|a| a.id == t.id), libelle(t)).clicked() {
                                // le type par défaut n'est pas enregistré tant que le projet n'en a aucun
                                self.dessin2d.terrain_type = self.natif.as_ref().is_some_and(|(p, _)| !p.types_terrains.is_empty()).then(|| t.id.clone());
                            }
                        }
                    });
                    if !self.dessin2d.dalle.is_empty() {
                        ui.separator();
                        ui.checkbox(&mut self.dessin2d.terrain_ajouter_point, "Placer des points cotés");
                        ui.label("Cote NGF");
                        ui.add(egui::DragValue::new(&mut self.dessin2d.terrain_altitude).speed(10.0).dimensions()).on_hover_text("Cote NGF des prochains points cotés");
                    }
                });
            }
            Some(Usage::Surface | Usage::LimitePropriete) => {
                ui.horizontal(|ui| {
                    ui.label("Forme");
                    let actuelle = self.usage_dessin().map(|(_, o)| o);
                    for (forme, nom, aide) in [
                        (OutilDessin::Polyligne, "Polygone", "Cliquer les sommets ; recliquer le premier, F ou clic droit pour fermer. Entrée valide les contours tracés."),
                        (OutilDessin::Rectangle, "Rectangle", "Deux coins opposés, dans les axes du plan"),
                        (OutilDessin::Cercle, "Cercle", "Centre puis rayon (contour à 48 côtés)"),
                    ] {
                        if ui.selectable_label(actuelle == Some(forme), nom).on_hover_text(aide).clicked() {
                            self.ruban_edition.forme_dalle = forme;
                        }
                    }
                });
            }
            _ => {}
        }
    }

    /// Le trait le plus proche : conserve les arcs/cercle et choisit un segment d'une polyligne.
    fn trait_mur_vise(&self,rect:egui::Rect,q:egui::Pos2)->Option<d2::Forme2D> {
        if !self.vue_en_plan() || !self.point_dans_cadrage(rect,q) {return None;}
        let (p,_)=self.natif.as_ref()?;
        let vue=self.vue_native_id()?;
        let v=p.vues.iter().find(|v|v.id==vue)?;
        let (o,u,h)=crate::modele::documentation::repere(p,v);
        let point=|a:DVec2| (o+u*a.x+h*a.y).truncate();
        let mut choix:Option<(f32,d2::Forme2D)>=None;
        let mut essayer=|forme:d2::Forme2D| {
            for ligne in forme.polylignes() {
                for w in ligne.windows(2) {
                    let (Some(a),Some(b))=(self.vers_ecran(rect,w[0]),self.vers_ecran(rect,w[1])) else {continue};
                    let ab=b-a;let t=if ab.length_sq()>0. {((q-a).dot(ab)/ab.length_sq()).clamp(0.,1.)}else{0.};
                    let distance=q.distance(a+ab*t);
                    if distance<=10. && choix.as_ref().is_none_or(|c|distance<c.0) {choix=Some((distance,forme.clone()));}
                }
            }
        };
        for a in p.annotations.iter().filter(|a|a.vue==vue && !v.elements_masques.contains(&a.id)) {
            let Some(f)=a.forme.forme_2d() else {continue};
            match f {
                d2::Forme2D::Ligne([a,b])=>essayer(d2::Forme2D::Ligne([point(a),point(b)])),
                d2::Forme2D::Arc(pts)=>essayer(d2::Forme2D::Arc(pts.map(point))),
                d2::Forme2D::Cercle{centre,rayon}=>essayer(d2::Forme2D::Cercle{centre:point(centre),rayon}),
                _=>for l in f.polylignes() {for w in l.windows(2){essayer(d2::Forme2D::Ligne([point(w[0]),point(w[1])]))}},
            }
        }
        choix.map(|c|c.1).or_else(||self.trait_annotation_vise(q,rect).map(|[a,b]| {
            let origine=self.scene.as_ref().map_or(DVec3::ZERO,|s|s.origin);
            d2::Forme2D::Ligne([(a.as_dvec3()+origine).truncate()*1000.,(b.as_dvec3()+origine).truncate()*1000.])
        }))
    }

    /// Barre d'options de l'outil Mur, en haut de la vue : forme tracée et chaînage.
    fn options_trace_mur(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            if ui.selectable_label(self.ruban_edition.selection_mur,"Sélection").on_hover_text("Sélectionner une ligne existante dans la vue pour tracer le mur").clicked() {
                self.ruban_edition.selection_mur=true;
                self.terminer_edition_axe();
            }
            for (forme, nom, aide) in [
                (OutilDessin::Ligne, "Ligne", "Murs droits, point par point"),
                (OutilDessin::Rectangle, "Rectangle", "Quatre murs par deux coins opposés"),
                (OutilDessin::Arc, "Arc", "Mur courbe : départ, point de passage, fin"),
                (OutilDessin::Cercle, "Cercle", "Centre, rayon, début et fin du périmètre"),
            ] {
                if ui.selectable_label(!self.ruban_edition.selection_mur && self.ruban_edition.forme_mur == forme, nom).on_hover_text(aide).clicked() {
                    self.ruban_edition.selection_mur=false;
                    self.ruban_edition.forme_mur = forme;
                }
            }
        });
    }

    /// Niveau du tracé d'un escalier ou d'un garde-corps : celui du plan ouvert, sinon
    /// le plus bas.
    pub(super) fn niveau_trace(&self) -> Option<String> {
        let (p, _) = self.natif.as_ref()?;
        let vue = self.vue_native_id().and_then(|id| p.vues.iter().find(|v| v.id == id));
        if let Some(crate::modele::GenreVue::Plan { niveau, .. }) = vue.map(|v| &v.genre) {
            return Some(niveau.clone());
        }
        p.niveaux.iter().min_by(|a, b| a.altitude.total_cmp(&b.altitude)).map(|n| n.id.clone())
    }

    /// Crée un escalier de ligne de foulée `trace` : du niveau de tracé au niveau
    /// immédiatement au-dessus (sinon 2,80 m), avec ses garde-corps selon le ruban.
    fn creer_escalier(&mut self, trace: Vec<[f64; 2]>) -> Result<(), String> {
        use crate::ruban::Outil;
        let (p, _) = self.natif.as_ref().ok_or("Aucun projet natif.")?;
        let niveau = self.niveau_trace().ok_or("Créez d'abord un niveau.")?;
        let alt = p.niveau(&niveau).map_or(0.0, |n| n.altitude);
        let haut = p.niveaux.iter().filter(|n| n.altitude > alt + 1.0).min_by(|a, b| a.altitude.total_cmp(&b.altitude)).map(|n| n.id.clone());
        let escalier = crate::modele::Escalier {
            id: String::new(),
            nom: format!("Escalier {}", p.escaliers.len() + 1),
            niveau: niveau.clone(),
            niveau_haut: haut,
            decalage_bas: 0.0,
            hauteur: 2800.0,
            trace,
            largeur: self.ruban_edition.nombre_option(Outil::Escalier, 0),
            hauteur_marche: self.ruban_edition.nombre_option(Outil::Escalier, 1),
            paillasse: 150.0,
        };
        let id = self.executer_bim(Commande::Escalier { escalier })?.into_iter().next().ok_or("Escalier non créé.")?;
        let cotes = match self.ruban_edition.texte_option(Outil::Escalier, 2).as_str() {
            "Gauche" => vec![0],
            "Droite" => vec![1],
            "Aucun" => vec![],
            _ => vec![0, 1],
        };
        self.garde_corps_escalier(&id, &cotes)?;
        self.status = "Escalier créé — Ctrl+S pour enregistrer.".into();
        Ok(())
    }

    /// Garde-corps hébergés par l'escalier `id` : côtés 0 (gauche) et 1 (droite) du
    /// sens de montée, à 50 mm du bord.
    pub(super) fn garde_corps_escalier(&mut self, id: &str, cotes: &[usize]) -> Result<(), String> {
        let (p, _) = self.natif.as_ref().ok_or("Aucun projet natif.")?;
        let e = p.escaliers.iter().find(|e| e.id == id).ok_or("Escalier absent.")?.clone();
        let traces = e.traces_garde_corps(50.0);
        for &k in cotes {
            self.creer_garde_corps(traces[k].clone(), Some(e.id.clone()))?;
        }
        Ok(())
    }

    /// Crée un garde-corps (réglages du ruban), hébergé ou non par un escalier.
    fn creer_garde_corps(&mut self, trace: Vec<[f64; 2]>, escalier: Option<String>) -> Result<String, String> {
        use crate::modele::Remplissage;
        use crate::ruban::Outil;
        let (p, _) = self.natif.as_ref().ok_or("Aucun projet natif.")?;
        let niveau = match escalier.as_ref().and_then(|id| p.escaliers.iter().find(|e| &e.id == id)) {
            Some(e) => e.niveau.clone(),
            None => self.niveau_trace().ok_or("Créez d'abord un niveau.")?,
        };
        let remplissage = match self.ruban_edition.texte_option(Outil::GardeCorps, 2).as_str() {
            "Panneau plein" => Remplissage::Plein,
            "Vitrage" => Remplissage::Vitre,
            _ => Remplissage::Barreaudage,
        };
        let garde_corps = crate::modele::GardeCorps {
            id: String::new(),
            nom: format!("Garde-corps {}", p.garde_corps.len() + 1),
            niveau,
            decalage: if escalier.is_some() { 0.0 } else { self.ruban_edition.nombre_option(Outil::GardeCorps, 1) },
            trace,
            hauteur: self.ruban_edition.nombre_option(Outil::GardeCorps, 0),
            remplissage,
            escalier,
        };
        self.executer_bim(Commande::GardeCorps { garde_corps })?.into_iter().next().ok_or_else(|| "Garde-corps non créé.".to_string())
    }

    /// Escalier en cours : emmarchement (bords de la largeur) et flèche de montée.
    fn apercu_escalier(&self, painter: &egui::Painter, rect: egui::Rect, lignes: &[Vec<DVec2>]) {
        let w = self.ruban_edition.nombre_option(crate::ruban::Outil::Escalier, 0) / 2.0;
        let couleur = Color32::from_rgb(55, 130, 200);
        for l in lignes.iter().filter(|l| l.len() >= 2) {
            let trace: Vec<[f64; 2]> = l.iter().map(|p| p.to_array()).collect();
            for d in [w, -w] {
                let bord: Vec<egui::Pos2> = crate::modele::escalier::decaler_polyligne(&trace, d).iter().filter_map(|q| self.vers_ecran(rect, DVec2::from_array(*q))).collect();
                painter.add(egui::Shape::line(bord, egui::Stroke::new(1.5, couleur)));
            }
            // flèche à l'arrivée
            let (a, b) = (l[l.len() - 2], l[l.len() - 1]);
            if let (Some(pa), Some(pb)) = (self.vers_ecran(rect, a), self.vers_ecran(rect, b)) {
                let d = (pb - pa).normalized() * 12.0;
                let n = egui::vec2(-d.y, d.x) * 0.5;
                painter.add(egui::Shape::convex_polygon(vec![pb, pb - d + n, pb - d - n], couleur, egui::Stroke::NONE));
            }
        }
    }
}

/// Bouton de la barre de commande : pictogramme dessiné (coche verte, croix rouge) et
/// libellé.
fn bouton_commande(ui: &mut egui::Ui, valider: bool, libelle: &str) -> egui::Response {
    let police = egui::FontId::proportional(13.0);
    let texte = ui.painter().layout_no_wrap(libelle.to_string(), police, ui.visuals().text_color());
    let (r, reponse) = ui.allocate_exact_size(egui::vec2(texte.size().x + 36.0, 26.0), Sense::click());
    if reponse.hovered() {
        ui.painter().rect_filled(r, 4.0, ui.visuals().widgets.hovered.bg_fill);
    }
    let c = egui::pos2(r.left() + 14.0, r.center().y);
    let (couleur, trait_) = if valider { (Color32::from_rgb(30, 150, 70), 2.2) } else { (Color32::from_rgb(200, 50, 45), 2.2) };
    let t = egui::Stroke::new(trait_, couleur);
    if valider {
        ui.painter().add(egui::Shape::line(vec![c + egui::vec2(-6.0, 0.0), c + egui::vec2(-2.0, 5.0), c + egui::vec2(6.0, -5.0)], t));
    } else {
        ui.painter().line_segment([c + egui::vec2(-5.0, -5.0), c + egui::vec2(5.0, 5.0)], t);
        ui.painter().line_segment([c + egui::vec2(-5.0, 5.0), c + egui::vec2(5.0, -5.0)], t);
    }
    ui.painter().galley(egui::pos2(r.left() + 28.0, r.center().y - texte.size().y / 2.0), texte, ui.visuals().text_color());
    reponse
}
