//! Interface du dessin 2D : clics et saisie au clavier, accrochages, repérage polaire
//! (Maj : ortho), aperçu élastique, invite et ligne de commande en bas de la vue, à la
//! manière de CADCraft. Sert aux annotations (Documenter → Dessin 2D, dans le repère de
//! la vue native ouverte) et aux murs (Modéliser → Mur, sur le plan horizontal de leur
//! base : ligne, rectangle, arc ou cercle).

mod saisie;
mod affichage;
mod options;
mod escalier;
mod cotation;
mod cotes_aretes;
mod commandes_contour;
use super::*;
use crate::dessin::dao::{self as d2, Accroche, Geom, OutilDessin, Trace};
use crate::modele::edition::commandes::Commande;
use crate::modele::documentation::{Annotation, AnnotationForme};
use glam::{DVec2, DVec3};

/// État du dessin 2D : tracé en cours, saisie au clavier, point sous le curseur (brut
/// et accroché), géométries d'accrochage de la vue (révision, vue).
/// Limite haute d'un escalier en création.
#[derive(Default, Clone, PartialEq)]
pub(super) enum HautEscalier {
    /// Premier niveau au-dessus de la base.
    #[default]
    Auto,
    Niveau(String),
    Libre,
}

#[derive(Default)]
pub(super) struct EtatDessin2D {
    /// Outil de la commande en cours et profondeur de l'historique à son démarrage :
    /// la croix Annuler défait tout ce que la commande a créé.
    commande_cle: Option<crate::ui::ruban::Outil>,
    historique_depart: Option<usize>,
    /// Bas de la barre de commande (depuis le haut de la vue) : les aides s'y alignent.
    pub bas_barre: f32,
    pub trace: Option<Trace>,
    /// Contours laissés ouverts par Échap, au stade où le tracé s'est arrêté : un clic
    /// sur une extrémité reprend le tracé. Ils bloquent la validation.
    pub ouverts: Vec<Vec<DVec2>>,
    /// Giron imposé à l'escalier en création (mm) ; None : celui du tracé.
    pub giron_impose: Option<f64>,
    /// Limites de l'escalier en création : niveau bas (None : niveau du plan) et son
    /// décalage, limite haute, hauteur si elle est libre (0 : 2,80 m).
    pub esc_bas: Option<String>,
    pub esc_decalage_bas: f64,
    pub esc_decalage_haut: f64,
    pub esc_haut: HautEscalier,
    pub esc_hauteur_libre: f64,
    /// Justification de la ligne tracée d'un escalier : 0 axe, 1 bord gauche, 2 bord droit.
    pub justification_escalier: u8,
    /// Justification figée de chaque volée déjà posée du tracé d'escalier en cours.
    pub just_segments: Vec<u8>,
    forme_cadrage: Option<OutilDessin>,
    ligne_selection: Option<d2::Forme2D>,
    pub arc_glisse: Option<(String,usize,[DVec2;3])>,
    pub saisie: String,
    champ_angle: bool,
    controle_seul: bool,
    curseur: Option<DVec2>,
    point: Option<(DVec2, Accroche)>,
    geoms: Option<(u64, String, Vec<Geom>)>,
    survol_accroche: Option<(DVec2, f64)>,
    survol_segment: Option<(DVec2, DVec2, f64, bool)>,
    reference: Option<DVec2>,
    directions_reference: Vec<DVec2>,
    guides: Vec<(DVec2, DVec2)>,
    pub message: Option<String>,
    information: Option<String>,
    /// Contours fermés de l'élément horizontal en cours : parties et trémies, créés à la
    /// validation.
    pub dalle: Vec<Vec<[f64; 2]>>,
    /// Terrain en cours : cote NGF de chaque sommet du contour (`dalle[0]`), points cotés
    /// (cote NGF), cote des prochains points, placement de points.
    terrain_cotes: Vec<f64>,
    terrain_points: Vec<[f64; 3]>,
    terrain_altitude: f64,
    /// Terrain existant repris par Modifier tracé (identifiant, nom), type choisi.
    terrain_modifie: Option<(String, String)>,
    terrain_type: Option<String>,
    /// Poignée sous le curseur, poignée glissée et état d'avant le glissé (repris si le
    /// terrain devient impossible à trianguler).
    terrain_survol: Option<PoigneeTerrain>,
    terrain_glisse: Option<(PoigneeTerrain, Vec<[f64; 2]>, Vec<f64>, Vec<[f64; 3]>)>,
    dernier_usage: Option<Usage>,
    pub edition_contour: Option<EditionContour>,
    /// Heure du premier Échap sur un tracé modifié : un second, dans les 4 s, l'abandonne.
    echap_abandon: Option<f64>,
    /// Type de trait des polylignes créées (id ; vide : continu fin).
    pub trait_creation: String,
    /// Cotation par arêtes : références déjà cliquées (élément, arête, abscisse relative),
    /// première arête, arête sous le curseur et arêtes de la vue (révision, vue).
    aretes_cote: Vec<(String, usize, f64)>,
    arete_premiere: Option<cotes_aretes::AreteVisee>,
    arete_survol: Option<cotes_aretes::AreteVisee>,
    aretes_cache: Option<(u64, String, Vec<(String, usize, [DVec2; 2])>)>,
}

pub(super) struct EditionContour {
    id: String,
    creation: Option<Usage>,
    altitude: f64,
    dessin: bool,
    selection: Option<usize>,
    segment: Option<(usize, usize)>,
    sommet: Option<(usize, usize)>,
    commande: Option<CommandeContour>,
    depart: Option<DVec2>,
    repere: Option<DVec2>,
    glisse: Option<GlisseContour>,
    annotation: Option<Annotation>,
    fermees: Vec<bool>,
    raccord: Option<(usize, bool)>,
    /// Élément dont le tracé est repris, autre qu'une dalle ou une polyligne.
    element: Option<ElementTrace>,
    /// Contours à l'ouverture du tracé (vide en création) : Échap ne l'abandonne sans
    /// confirmation que s'ils n'ont pas changé.
    initial: Vec<Vec<[f64; 2]>>,
}

/// Élément à tracé unique repris par Modifier.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(super) enum ElementTrace {
    /// Tracé ouvert d'un garde-corps.
    GardeCorps,
    /// Contour fermé d'une limite de propriété.
    Limite,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum CommandeContour { Deplacer, Copier, /** Nombre de copies. */ Reseau(usize), Tourner, Symetrie, Echelle, Decaler, Scinder, Aligner, Ajuster }

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

fn fermer_par_extremite(contour: &mut Vec<[f64; 2]>, sommet: usize) -> bool {
    if contour.len() < 4 || (sommet != 0 && sommet + 1 != contour.len()) { return false; }
    contour.remove(sommet);
    true
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
    fn fermeture_par_extremite_ne_garde_pas_de_sommet_double() {
        let trace = vec![[0., 0.], [4., 0.], [4., 4.], [0., 0.]];
        let mut fin = trace.clone();
        assert!(fermer_par_extremite(&mut fin, 3));
        assert_eq!(fin, vec![[0., 0.], [4., 0.], [4., 4.]]);
        let mut debut = trace;
        assert!(fermer_par_extremite(&mut debut, 0));
        assert_eq!(debut, vec![[4., 0.], [4., 4.], [0., 0.]]);
        assert!(!fermer_par_extremite(&mut debut, 0));
    }

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
    /// Tracé unique et éventuellement ouvert (polyligne, garde-corps) : pas d'autre contour.
    fn ligne(&self) -> bool { self.annotation.is_some() || self.element == Some(ElementTrace::GardeCorps) }
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
    pub(super) fn commande_ruban_contour(&mut self, outil: crate::ui::ruban::Outil) {
        use crate::ui::ruban::Outil;
        if outil == Outil::Supprimer { self.supprimer_trait_edition(); return; }
        let Some(e) = self.dessin2d.edition_contour.as_mut() else { return };
        if e.dessin { self.dessin2d.message = Some("Appuyez sur Échap pour sélectionner un segment ou un sommet.".into()); return; }
        e.commande = match outil {
            Outil::Deplacer if e.segment.is_some() || e.sommet.is_some() => Some(CommandeContour::Deplacer),
            Outil::Copier | Outil::CreerIdentique if e.segment.is_some() => Some(CommandeContour::Copier),
            Outil::Reseau if e.segment.is_some() => Some(CommandeContour::Reseau(3)),
            Outil::Scinder if e.segment.is_some() => Some(CommandeContour::Scinder),
            Outil::Aligner if e.segment.is_some() => Some(CommandeContour::Aligner),
            Outil::Ajuster if e.segment.is_some() => Some(CommandeContour::Ajuster),
            Outil::Tourner if e.segment.is_some() || e.sommet.is_some() => Some(CommandeContour::Tourner),
            Outil::Symetrie if e.segment.is_some() || e.sommet.is_some() => Some(CommandeContour::Symetrie),
            Outil::Echelle if e.segment.is_some() || e.sommet.is_some() => Some(CommandeContour::Echelle),
            Outil::Decaler if e.segment.is_some() => Some(CommandeContour::Decaler),
            _ => { self.dessin2d.message = Some("Sélectionnez un segment ou un sommet ; cette commande ne s'applique pas encore au tracé.".into()); None }
        };
        e.depart = None;
        e.repere = None;
    }
    pub(super) fn selection_traces_active(&self) -> bool {
        self.dessin2d.edition_contour.as_ref().is_some_and(|e| !e.dessin)
    }
    pub(super) fn objet_en_edition(&self) -> Option<(&str, bool)> {
        self.dessin2d.edition_contour.as_ref().map(|e| (e.id.as_str(), e.annotation.is_some()))
    }
    fn supprimer_trait_edition(&mut self) {
        // sommet sélectionné : retiré, tant que le tracé garde assez de sommets
        if let Some(e) = self.dessin2d.edition_contour.as_mut() && let (None, Some((c, i))) = (e.segment, e.sommet) {
            if matches!(e.creation, Some(Usage::Terrain | Usage::Escalier | Usage::GardeCorps | Usage::Annotation)) { return; }
            if self.dessin2d.dalle.get(c).is_some_and(|x| i < x.len() && x.len() > if e.fermee(c) { 3 } else { 2 }) {
                self.dessin2d.dalle[c].remove(i);
                e.sommet = None;
            } else {
                self.dessin2d.message = Some("Le tracé ne peut pas comporter moins de sommets.".into());
            }
            return;
        }
        if self.dessin2d.edition_contour.as_ref().is_some_and(|e| matches!(e.creation, Some(Usage::Terrain | Usage::Escalier | Usage::GardeCorps))) { return; }
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
        { self.dessin2d.message = None; self.dessin2d.information = None; };
        self.dessin2d.information = Some("Cliquez une extrémité ouverte pour reprendre le tracé.".into());
    }

    /// Maj+Suppr : retire le contour du trait ou du sommet sélectionné (pas le dernier,
    /// ni celui d'un tracé à contour unique).
    fn supprimer_contour_edition(&mut self) {
        let Some(e) = self.dessin2d.edition_contour.as_mut() else { return };
        if e.ligne() || matches!(e.creation, Some(Usage::Terrain | Usage::Cadrage | Usage::Escalier | Usage::GardeCorps | Usage::Annotation)) || self.dessin2d.dalle.len() < 2 {
            self.dessin2d.message = Some("Ce contour ne peut pas être supprimé.".into());
            return;
        }
        let Some(i) = e.selection.take().filter(|i| *i < self.dessin2d.dalle.len()) else { return };
        self.dessin2d.dalle.remove(i);
        e.fermees.remove(i);
        (e.segment, e.sommet) = (None, None);
    }

    /// Conserve les points déjà ajoutés lorsqu'on interrompt ou termine un raccord.
    fn terminer_raccord_ouvert(&mut self) {
        let Some((c, premier)) = self.dessin2d.edition_contour.as_ref().and_then(|e| e.raccord) else { return };
        let Some(trace) = self.dessin2d.trace.as_mut() else { return };
        let nombre = trace.points.len().saturating_sub(1);
        if self.dessin2d.edition_contour.as_ref().is_some_and(|e| e.creation == Some(Usage::Terrain)) {
            let z = if premier { self.dessin2d.terrain_cotes.first().copied() } else { self.dessin2d.terrain_cotes.last().copied() }.unwrap_or(self.dessin2d.terrain_altitude);
            if premier { self.dessin2d.terrain_cotes.splice(0..0, std::iter::repeat_n(z, nombre)); }
            else { self.dessin2d.terrain_cotes.extend(std::iter::repeat_n(z, nombre)); }
        }
        raccorder_contour(&mut self.dessin2d.dalle[c], premier, &trace.points);
        trace.points.clear();
        let e = self.dessin2d.edition_contour.as_mut().unwrap();
        e.raccord = None;
        e.dessin = false;
    }

    fn ajouter_point_raccord(&mut self, p: DVec2) -> bool {
        let Some((c, premier)) = self.dessin2d.edition_contour.as_ref().and_then(|e| e.raccord) else { return false };
        if self.dessin2d.trace.is_none() {
            return false;
        }
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
            { self.dessin2d.message = None; self.dessin2d.information = None; };
            return true;
        }
        let contour = &mut self.dessin2d.dalle[c];
        let cible = DVec2::from_array(if premier { *contour.last().unwrap() } else { contour[0] });
        let trace = self.dessin2d.trace.as_mut().unwrap();
        if p.distance(cible) < 1e-6 {
            if self.dessin2d.edition_contour.as_ref().is_some_and(|e| e.creation == Some(Usage::Terrain)) {
                let n = trace.points.len().saturating_sub(1);
                let z = if premier { self.dessin2d.terrain_cotes.first().copied() } else { self.dessin2d.terrain_cotes.last().copied() }.unwrap_or(self.dessin2d.terrain_altitude);
                if premier { self.dessin2d.terrain_cotes.splice(0..0, std::iter::repeat_n(z, n)); }
                else { self.dessin2d.terrain_cotes.extend(std::iter::repeat_n(z, n)); }
            }
            raccorder_contour(contour, premier, &trace.points);
            trace.points.clear();
            let e = self.dessin2d.edition_contour.as_mut().unwrap();
            e.fermees[c] = true;
            e.raccord = None;
            e.dessin = false;
            { self.dessin2d.message = None; self.dessin2d.information = None; };
        } else if trace.points.last().is_none_or(|q| q.distance(p) > 1e-6) { trace.points.push(p); }
        true
    }
    /// Clic sur une extrémité d'un contour laissé ouvert : le tracé reprend de là.
    fn reprendre_contour_ouvert(&mut self, rect: egui::Rect, clic: Option<egui::Pos2>) -> bool {
        let Some(clic) = clic else { return false };
        if self.dessin2d.trace.as_ref().is_none_or(|t| !t.points.is_empty() || t.outil != OutilDessin::Polyligne) { return false; }
        if let Some(e) = self.dessin2d.edition_contour.as_ref() && e.dessin && e.raccord.is_none() {
            let proche = |p: &[f64; 2]| self.vers_ecran(rect, DVec2::from_array(*p)).is_some_and(|q| q.distance(clic) <= 10.0);
            let trouve = self.dessin2d.dalle.iter().enumerate().filter(|(c, _)| !e.fermee(*c)).find_map(|(c, points)| {
                if points.last().is_some_and(&proche) { Some((c, false)) }
                else if points.first().is_some_and(&proche) { Some((c, true)) } else { None }
            });
            if let Some((c, premier)) = trouve {
                let points = &self.dessin2d.dalle[c];
                let point = if premier { points[0] } else { *points.last().unwrap() };
                self.dessin2d.trace.as_mut().unwrap().points.push(DVec2::from_array(point));
                self.dessin2d.edition_contour.as_mut().unwrap().raccord = Some((c, premier));
                self.dessin2d.saisie.clear();
                self.dessin2d.message = None; self.dessin2d.information = None;
                return true;
            }
        }
        if self.dessin2d.ouverts.is_empty() {
            return false;
        }
        let proche = |q: &DVec2| self.vers_ecran(rect, *q).is_some_and(|e| e.distance(clic) <= 10.0);
        let trouve = self.dessin2d.ouverts.iter().enumerate().find_map(|(i, c)| {
            if c.last().is_some_and(&proche) { Some((i, false)) } else if c.first().is_some_and(&proche) { Some((i, true)) } else { None }
        });
        let Some((i, debut)) = trouve else { return false };
        let mut points = self.dessin2d.ouverts.remove(i);
        if debut { points.reverse(); }
        if let Some(t) = self.dessin2d.trace.as_mut() { t.points = points; }
        self.dessin2d.saisie.clear();
        { self.dessin2d.message = None; self.dessin2d.information = None; };
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
        d.terrain_modifie = None;
        d.terrain_survol = None;
        d.terrain_glisse = None;
    }

    /// Terrain du tracé en cours (contour fermé, cotes, points, type).
    fn terrain_courant(&self) -> Option<crate::modele::elements::site::Terrain> {
        let contour = self.dessin2d.dalle.first()?;
        if contour.len() != self.dessin2d.terrain_cotes.len() { return None; }
        let (id, nom) = self.dessin2d.terrain_modifie.clone().unwrap_or_else(|| (String::new(), "Terrain".into()));
        Some(crate::modele::elements::site::Terrain {
            id,
            nom,
            contour: contour.iter().zip(&self.dessin2d.terrain_cotes).map(|(p, z)| [p[0], p[1], *z]).collect(),
            points: self.dessin2d.terrain_points.clone(),
            type_terrain: self.dessin2d.terrain_type.clone(),
        })
    }

    /// Sélecteur du type et cote des points pendant la création ou l'édition du terrain.
    pub(in crate::ui) fn proprietes_terrain_creation(&mut self, ui: &mut egui::Ui) -> bool {
        if !self.usage_dessin().is_some_and(|(u, _)| u == Usage::Terrain) { return false; }
        let Some((p, _)) = &self.natif else { return false };
        let types = p.types_terrains_disponibles();
        let actuel = self.dessin2d.terrain_type.as_ref().and_then(|id| types.iter().find(|t| &t.id == id)).or(types.first());
        ui.add_space(8.0);
        ui.label(RichText::new("Terrain").small().weak());
        egui::ComboBox::from_id_salt("type_terrain_creation_proprietes").width((ui.available_width() - 8.0).max(1.0)).selected_text(actuel.map_or("Terrain", |t| t.nom.as_str())).show_ui(ui, |ui| {
            for t in &types {
                if ui.selectable_label(actuel.is_some_and(|a| a.id == t.id), &t.nom).clicked() {
                    self.dessin2d.terrain_type = (!p.types_terrains.is_empty()).then(|| t.id.clone());
                }
            }
        });
        ui.horizontal(|ui| { ui.label("Cote NGF des points"); ui.add(egui::DragValue::new(&mut self.dessin2d.terrain_altitude).speed(10.0).dimensions()); });
        ui.separator();
        true
    }

    /// Modifier tracé d'un terrain : l'outil Terrain reprend son contour, ses cotes, ses
    /// points et son type ; la validation remplace le terrain.
    fn commencer_edition_terrain(&mut self, id: &str) {
        let Some(t) = self.natif.as_ref().and_then(|(p, _)| p.terrains.iter().find(|t| t.id == id)).cloned() else { return };
        self.ruban_edition.activer_outil(crate::ui::ruban::Outil::Terrain);
        self.onglet_ruban = OngletRuban::Modeliser;
        self.dessin2d.edition_contour = None;
        self.dessin2d.dalle = vec![t.contour.iter().map(|p| [p[0], p[1]]).collect()];
        self.dessin2d.terrain_cotes = t.contour.iter().map(|p| p[2]).collect();
        self.dessin2d.terrain_points = t.points.clone();
        self.dessin2d.terrain_altitude = t.contour.first().map_or(0.0, |p| p[2]);
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
        // garde-corps et limite de propriété : leur tracé ; autre élément à famille : sa famille
        let trace = ids.len() == 1 && (p.garde_corps.iter().any(|g| g.id == ids[0]) || p.limites_propriete.iter().any(|l| l.id == ids[0]));
        if !trace && self.cible_famille().is_some() {
            self.modifier_famille_selectionnee();
            return;
        }
        let Some((p, _)) = &self.natif else { return };
        if ids.len() != 1 { self.status = "Sélectionnez un élément à modifier.".into(); return; }
        let mut element = None;
        let (altitude, contours, annotation) = if let Some(d) = p.dalles.iter().find(|d| d.id == ids[0]) {
            (p.niveau(&d.niveau).map_or(0.0, |n| n.altitude) + d.decalage, d.contours.clone(), None)
        } else if let Some(g) = p.garde_corps.iter().find(|g| g.id == ids[0]) {
            if g.escalier.is_some() { self.status = "Ce garde-corps suit son escalier : modifiez l'escalier.".into(); return; }
            element = Some(ElementTrace::GardeCorps);
            (p.niveau(&g.niveau).map_or(0.0, |n| n.altitude) + g.decalage, vec![g.trace.clone()], None)
        } else if let Some(l) = p.limites_propriete.iter().find(|l| l.id == ids[0]) {
            element = Some(ElementTrace::Limite);
            (l.altitude, vec![l.contour.clone()], None)
        } else if let Some(a) = p.annotations.iter().find(|a| a.id == ids[0]) {
            // une ligne est une polyligne d'un seul segment : elle le devient à la validation
            let mut a = a.clone();
            if let AnnotationForme::Ligne { debut, fin } = &a.forme {
                a.forme = AnnotationForme::Polyligne { points: vec![*debut, *fin], fermee: false, remplissage: None, traits: Vec::new() };
            }
            let AnnotationForme::Polyligne { points, .. } = &a.forme else { self.status = "Sélectionnez un élément horizontal ou une polyligne.".into(); return; };
            if self.vue_native_id().as_deref() != Some(a.vue.as_str()) { self.status = "Ouvrez la vue de la polyligne pour la modifier.".into(); return; }
            (0.0, vec![points.clone()], Some(a))
        } else { self.status = "Sélectionnez un élément horizontal ou une polyligne.".into(); return; };
        let ouvert = element == Some(ElementTrace::GardeCorps);
        self.dessin2d.dernier_usage = Some(if annotation.is_some() || ouvert { Usage::Annotation } else { Usage::Surface });
        let fermees = vec![!ouvert && annotation.as_ref().is_none_or(|a| matches!(&a.forme, AnnotationForme::Polyligne { fermee: true, .. })); contours.len()];
        let initial = contours.clone();
        self.dessin2d.dalle = contours;
        self.dessin2d.geoms = None;
        self.dessin2d.edition_contour = Some(EditionContour { id: ids[0].clone(), creation: None, altitude, dessin: false, selection: None, segment: None, sommet: None, commande: None, depart: None, repere: None, glisse: None, annotation, fermees, raccord: None, element, initial });
        self.dessin2d.trace = Some(Trace::new(OutilDessin::Polyligne));
        self.ruban_edition.forme_dalle = OutilDessin::Polyligne;
        self.ruban_edition.annuler();
        self.visibility_dirty = true;
        self.coupe_dirty = true;
    }

    /// Outil Polyligne actif : type de trait des polylignes créées, dans Propriétés.
    pub(in crate::ui) fn proprietes_creation_polyligne(&mut self, ui: &mut egui::Ui) {
        if self.dessin2d.edition_contour.is_some() || self.usage_dessin() != Some((Usage::Annotation, OutilDessin::Polyligne)) {
            return;
        }
        let types = self.natif.as_ref().map(|(p, _)| p.types_traits.clone()).unwrap_or_default();
        if !types.iter().any(|t| t.id == self.dessin2d.trait_creation) {
            self.dessin2d.trait_creation.clear();
        }
        ui.label(RichText::new("Polyligne").strong());
        egui::Grid::new("creation_polyligne").num_columns(2).spacing([10.0, 4.0]).show(ui, |ui| {
            ui.label("Type de trait").on_hover_text("Type de trait des segments de la polyligne créée ; chaque segment se change ensuite dans Modifier");
            if let Some(id) = crate::ui::types_traits::choix_type_trait(ui, "trait_creation", Some(&self.dessin2d.trait_creation), &types) {
                self.dessin2d.trait_creation = id;
            }
            ui.end_row();
        });
        ui.separator();
    }

    /// Tracé en cours, trait sélectionné : Propriétés montre ce trait à la place de
    /// l'objet (longueur ; type de trait d'une polyligne ; pente ou talus de l'arête d'un
    /// élément horizontal). Sans trait sélectionné, Propriétés reste à l'objet.
    /// Retourne vrai si la section est affichée.
    pub(in crate::ui) fn proprietes_segment_edition(&mut self, ui: &mut egui::Ui) -> bool {
        let Some(e) = self.dessin2d.edition_contour.as_ref() else { return false };
        let Some((c, i)) = e.segment else { return false };
        let Some(points) = self.dessin2d.dalle.get(c).filter(|p| i < p.len()) else { return false };
        let (a, b) = (points[i], points[(i + 1) % points.len()]);
        let longueur = DVec2::from_array(a).distance(DVec2::from_array(b));
        let projet = self.natif.as_ref().map(|(p, _)| p);
        let dalle = projet.filter(|_| e.creation.is_none() && e.annotation.is_none() && e.element.is_none()).and_then(|p| p.dalles.iter().find(|d| d.id == e.id)).cloned();
        let objet: String = if e.annotation.is_some() { "Polyligne".into() } else {
            match (e.element, e.creation) {
                (Some(ElementTrace::GardeCorps), _) | (_, Some(Usage::GardeCorps)) => "Garde-corps".into(),
                (Some(ElementTrace::Limite), _) | (_, Some(Usage::LimitePropriete)) => "Limite de propriété".into(),
                (_, Some(Usage::Terrain)) => "Terrain".into(),
                (_, Some(Usage::Escalier)) => "Escalier".into(),
                (_, Some(Usage::Cadrage)) => "Cadrage de la vue".into(),
                (_, Some(Usage::Annotation)) => "Polyligne".into(),
                _ => dalle.as_ref().map(|d| d.sorte.nom().to_string()).or_else(|| self.ruban_edition.dalle_active().map(|d| d.sorte.nom().to_string())).unwrap_or_else(|| "Élément horizontal".into()),
            }
        };
        let types = projet.map(|p| p.types_traits.clone()).unwrap_or_default();
        let trait_actuel = match e.annotation.as_ref().map(|a| &a.forme) {
            Some(AnnotationForme::Polyligne { traits, .. }) if c == 0 => Some(traits.get(i).cloned().unwrap_or_default()),
            _ => None,
        };
        // arête d'origine de l'élément : mêmes extrémités, comme à la validation du contour
        let reglable = dalle.as_ref().is_some_and(|d| d.sorte.aretes_reglables());
        // (terre-plein dont les contours gardent leurs sommets : l'arête de même rang)
        let memes_aretes = dalle.as_ref().is_some_and(|d| d.sorte.talus() && d.contours.len() == self.dessin2d.dalle.len() && d.contours.iter().zip(&self.dessin2d.dalle).all(|(x, y)| x.len() == y.len()));
        let arete = if memes_aretes { Some((c, i)) } else {
            dalle.as_ref().filter(|_| reglable).and_then(|d| d.contours.iter().enumerate().find_map(|(cs, x)| (0..x.len()).find(|&k| x[k] == a && x[(k + 1) % x.len()] == b).map(|k| (cs, k))))
        };
        let mut pente_defaut = self.pente_defaut;
        let mut choix_trait = None;
        let mut dalle_modifiee = None;
        ui.label(RichText::new(format!("{objet} · trait {}", i + 1)).strong());
        egui::Grid::new("segment_edition").num_columns(2).spacing([10.0, 4.0]).show(ui, |ui| {
            ui.label("Longueur");
            ui.label(crate::config::options::unite().afficher(longueur.round()));
            ui.end_row();
            if let Some(actuel) = &trait_actuel {
                ui.label("Type de trait");
                choix_trait = crate::ui::types_traits::choix_type_trait(ui, "trait_segment_edition", Some(actuel), &types);
                ui.end_row();
            }
            if let (Some(d), Some((cs, k))) = (&dalle, arete) {
                let mut d = d.clone();
                if d.sorte.talus() {
                    ui.label("Talus");
                    if ui.horizontal(|ui| crate::ui::proprietes::champs_talus(ui, &mut d, cs, k)).inner { dalle_modifiee = Some(d); }
                } else {
                    ui.label("Pente");
                    let mut pente = d.pente(cs, k);
                    let mut oui = pente.is_some();
                    ui.horizontal(|ui| {
                        if ui.checkbox(&mut oui, "Définit la pente").on_hover_text("Le versant monte depuis cette arête vers l'intérieur").changed() { pente = oui.then_some(pente_defaut); }
                        if let Some(angle) = pente.as_mut() && ui.add(egui::DragValue::new(angle).speed(0.5).range(0.5..=85.0).max_decimals(1).suffix(" °")).changed() { pente_defaut = *angle; }
                    });
                    if pente != d.pente(cs, k) {
                        d.definir_pente(cs, k, pente);
                        dalle_modifiee = Some(d);
                    }
                }
                ui.end_row();
            }
        });
        if reglable && arete.is_none() {
            ui.label(RichText::new("Arête déplacée ou nouvelle : valider le tracé pour régler sa pente ou son talus.").small().weak());
        }
        ui.label(RichText::new("Échap : désélectionner le trait et revenir aux propriétés de l'objet.").small().weak());
        ui.separator();
        self.pente_defaut = pente_defaut;
        if let Some(id) = choix_trait
            && let Some(AnnotationForme::Polyligne { traits, .. }) = self.dessin2d.edition_contour.as_mut().and_then(|e| e.annotation.as_mut()).map(|a| &mut a.forme)
        {
            if traits.len() <= i {
                traits.resize(i + 1, String::new());
            }
            traits[i] = id;
            if traits.iter().all(String::is_empty) {
                traits.clear();
            }
        }
        // pente ou talus : appliqué tout de suite à l'élément, le tracé reste en cours
        if let Some(d) = dalle_modifiee && let Some((projet, _)) = &self.natif {
            let mut p = projet.clone();
            if let Some(x) = p.dalles.iter_mut().find(|x| x.id == d.id) { *x = d; }
            if let Err(e) = self.appliquer_projet(p) { self.error = Some(e); }
        }
        true
    }

    /// Outil de dessin actif : annotation ou mur.
    pub(super) fn outil_dessin(&self) -> Option<OutilDessin> {
        self.usage_dessin().map(|(_, o)| o)
    }

    /// Tracé actif et son usage : dessin 2D (Documenter, vue native ouverte) ou mur
    /// (Modéliser, projet natif chargé).
    pub(super) fn usage_dessin(&self) -> Option<(Usage, OutilDessin)> {
        use crate::ui::ruban::Outil;
        if self.dessin2d.edition_contour.is_some() {
            if self.dessin2d.edition_contour.as_ref().is_some_and(|e| e.ligne()) { return Some((Usage::Annotation, OutilDessin::Polyligne)); }
            if let Some(usage @ (Usage::Escalier | Usage::GardeCorps | Usage::Annotation)) = self.dessin2d.edition_contour.as_ref().and_then(|e| e.creation) { return Some((usage, OutilDessin::Polyligne)); }
            let o = match self.ruban_edition.forme_dalle {
                o @ (OutilDessin::Rectangle | OutilDessin::Cercle) => o,
                _ => OutilDessin::Polyligne,
            };
            return Some((self.dessin2d.edition_contour.as_ref().and_then(|e| e.creation).unwrap_or(Usage::Surface), o));
        }
        if self.ruban_edition.outil() == Some(Outil::Cadrer) && self.vue_en_plan() && self.vue_native_id().is_some() {
            return Some((Usage::Cadrage, self.dessin2d.forme_cadrage.unwrap_or(OutilDessin::Rectangle)));
        }
        let modele_disponible = self.natif.is_some() && self.scene.is_some() && self.loading.is_none();
        let modeliser = matches!(self.onglet_ruban, OngletRuban::Modeliser | OngletRuban::Modifier) && modele_disponible;
        if modeliser && self.ruban_edition.outil() == Some(Outil::Terrain) { return Some((Usage::Terrain, self.ruban_edition.forme_dalle)); }
        if modele_disponible && self.ruban_edition.outil() == Some(Outil::LimitePropriete) {
            let forme = match self.ruban_edition.forme_dalle {
                f @ (OutilDessin::Rectangle | OutilDessin::Cercle) => f,
                _ => OutilDessin::Polyligne,
            };
            return Some((Usage::LimitePropriete, forme));
        }
        if modeliser && self.ruban_edition.mur_actif() {
            return Some((Usage::Mur, self.ruban_edition.forme_mur));
        }
        if modele_disponible && self.ruban_edition.dalle_active().is_some() {
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
        if !matches!(self.onglet_ruban, OngletRuban::Annoter | OngletRuban::Modifier) || self.vue_native_id().is_none() {
            return None;
        }
        // cotation : points successifs recueillis par un tracé polyligne
        if self.outil_cotation().is_some() { return Some((Usage::Annotation, OutilDessin::Polyligne)); }
        Some((Usage::Annotation, self.ruban_edition.outil()?.forme_dessin()?))
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
                let decalage = if self.usage_dessin().is_some_and(|(u, _)| u == Usage::GardeCorps) { self.ruban_edition.nombre_option(crate::ui::ruban::Outil::GardeCorps, 1) } else { 0.0 };
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
        self.geoms_dessin_avec_mode(false)
    }

    fn geoms_dessin_avec_mode(&mut self, selection: bool) -> Vec<Geom> {
        let Some((usage, _)) = self.usage_dessin().or_else(||(self.poignee_axe.is_some() || self.dessin2d.arc_glisse.is_some()).then_some((Usage::Mur,OutilDessin::Ligne))) else { return Vec::new() };
        let vue = self.vue_native_id();
        let altitude = if let Some((id,_,_))=&self.dessin2d.arc_glisse {self.natif.as_ref().and_then(|(p,_)|p.trajectoire_justification(id)).map_or(0.0,|a|a.1)} else if self.poignee_axe.is_some() {self.axe_selection().map_or(0.0,|a|a.3)} else {self.altitude_trace()};
        let exclu=self.poignee_axe.as_ref().map(|a|a.0.as_str()).or_else(||self.dessin2d.arc_glisse.as_ref().map(|a|a.0.as_str()));
        let visibilite = if selection {
            use std::hash::{Hash, Hasher};
            let mut h = std::collections::hash_map::DefaultHasher::new();
            self.visible.hash(&mut h);
            h.finish()
        } else { 0 };
        let plan_coupe = if selection { self.plan_coupe().map(|p| p.to_array().map(f32::to_bits)) } else { None };
        let orientation = if selection { Some((self.camera.forward().to_array().map(f32::to_bits), self.camera.up().to_array().map(f32::to_bits))) } else { None };
        let cle = format!("{usage:?}|{}|{altitude}|{exclu:?}|{selection}|{visibilite}|{plan_coupe:?}|{orientation:?}", vue.as_deref().unwrap_or(""));
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
        // origine du projet : point accrochable (segment de longueur nulle)
        let mut g = if selection { Vec::new() } else { vec![Geom::Segment([DVec2::ZERO, DVec2::ZERO])] };
        // dessins de la vue : toujours pour une annotation, en plan pour un mur
        if !selection && let Some(vue) = vue.as_ref().filter(|_| usage == Usage::Annotation || plan.is_some()) {
            for a in p.annotations.iter().filter(|a| &a.vue == vue && self.dessin2d.edition_contour.as_ref().is_none_or(|e| e.id != a.id)) {
                if let Some(f) = a.forme.forme_2d() {
                    g.extend(f.geoms());
                }
            }
        }
        let modele = usage != Usage::Annotation || plan.is_some();
        if modele {
            let seg = |a: [f64; 2], b: [f64; 2]| Geom::Segment([DVec2::from_array(a), DVec2::from_array(b)]);
            let visible = |genre: &str, id: &str| !selection || self.scene.as_ref().and_then(|s| s.object_of.get(&scene::id_natif(genre, id))).and_then(|i| self.visible.get(*i as usize)).copied().unwrap_or(false);
            let coupes = crate::modele::murs::jonctions::coupes(p);
            let retenu = |m: &crate::modele::Mur| match usage {
                Usage::Annotation => plan.as_ref() == Some(&m.niveau),
                _ => p.bas_haut(m).is_some_and(|(b, h)| altitude >= b - 0.1 && altitude <= h + 0.1),
            };
            // `exclu` : mur dont on déplace la poignée (et ses couches) ; sans poignée, aucun
            // mur n'est exclu (comparer deux `None` excluait tous les murs simples)
            let exclu_mur = |m: &crate::modele::Mur| exclu.is_some_and(|e| e == m.id || m.assemblage.as_deref() == Some(e));
            for m in p.murs.iter().filter(|m| (selection || retenu(m)) && !exclu_mur(m) && visible("mur", &m.id)) {
                if let Some(c) = crate::modele::murs::jonctions::contour(p, m, &coupes) {
                    for k in 0..4 {
                        g.push(seg(c[k], c[(k + 1) % 4]));
                    }
                }
                match m.axe {
                    crate::modele::Axe::Droit { debut, fin } => if !selection { g.push(seg(debut, fin)); },
                    crate::modele::Axe::Arc { debut, fin, centre, .. } => {
                        let o = DVec2::from_array(centre);
                        let (a, b) = (DVec2::from_array(debut), DVec2::from_array(fin));
                        let r = a.distance(o);
                        let a0 = (a - o).to_angle();
                        let direct = matches!(m.axe, crate::modele::Axe::Arc { direct: true, .. });
                        let mut balayage = (b - o).to_angle() - a0;
                        balayage = if direct { balayage.rem_euclid(std::f64::consts::TAU) } else { -(-balayage).rem_euclid(std::f64::consts::TAU) };
                        if selection {
                            let e = p.type_mur(&m.type_mur).map_or(0.0, |t| t.epaisseur()) * 0.5;
                            if r > e && e > 0.0 {
                                for rayon in [r - e, r + e] { g.push(Geom::Arc { centre: o, rayon, depart: a0, balayage }); }
                                for angle in [a0, a0 + balayage] {
                                    let d = DVec2::from_angle(angle);
                                    g.push(Geom::Segment([o + d * (r - e), o + d * (r + e)]));
                                }
                            }
                        } else { g.push(Geom::Arc { centre: o, rayon: r, depart: a0, balayage }); }
                    }
                }
            }
            for d in p.dalles.iter().filter(|d| (usage != Usage::Annotation || plan.as_ref() == Some(&d.niveau)) && self.dessin2d.edition_contour.as_ref().is_none_or(|e| e.id != d.id) && visible("dalle", &d.id)) {
                for c in &d.contours {
                    for k in 0..c.len() {
                        g.push(seg(c[k], c[(k + 1) % c.len()]));
                    }
                }
            }
            for t in p.terrains.iter().filter(|t| visible("terrain", &t.id)) {
                for i in 0..t.contour.len() {
                    let a = t.contour[i]; let b = t.contour[(i + 1) % t.contour.len()];
                    g.push(seg([a[0], a[1]], [b[0], b[1]]));
                }
            }
            for l in p.limites_propriete.iter().filter(|l| visible("limite_propriete", &l.id)) {
                for i in 0..l.contour.len() { g.push(seg(l.contour[i], l.contour[(i + 1) % l.contour.len()])); }
            }
            for q in &p.quadrillages {
                g.push(seg(q.debut, q.fin));
            }
        }
        if selection && let Some(scene) = &self.scene && let Some(plan) = self.plan_coupe() {
            // Les contours réellement coupés comprennent notamment les ouvertures et
            // les éléments qui ne disposent pas d'un contour natif en plan.
            let arcs: std::collections::HashSet<_> = p.murs.iter().filter(|m| matches!(m.axe, crate::modele::Axe::Arc { .. })).map(|m| scene::id_natif("mur", &m.id)).collect();
            let mut traits = crate::dessin::coupe(scene, &self.visible, plan, self.echelle, crate::dessin::Poche::Vide);
            traits.extend(self.aretes_au_dessus_plan());
            let point = |v: DVec3| (v + scene.origin).truncate() * 1000.0;
            for prim in traits {
                if !matches!(prim.role, crate::noyau::trace2d::Role::Coupe | crate::noyau::trace2d::Role::Vu) || prim.element.is_some_and(|id| arcs.contains(&id)) { continue; }
                match prim.forme {
                    crate::noyau::trace2d::Forme::Segments(segments) => for [a,b] in segments { g.push(Geom::Segment([point(a),point(b)])); },
                    crate::noyau::trace2d::Forme::Polyligne { points, fermee } => {
                        for w in points.windows(2) { g.push(Geom::Segment([point(w[0]),point(w[1])])); }
                        if fermee && points.len() > 2 { g.push(Geom::Segment([point(*points.last().unwrap()),point(points[0])])); }
                    }
                    _ => {}
                }
            }
        }
        // Le fond de plan est visible : ses contours projetés participent aussi aux
        // accrochages, sans rendre ses éléments sélectionnables ou modifiables.
        if let Some(source)=vue.as_ref().and_then(|id|p.vues.iter().find(|v|&v.id==id)).and_then(|v|v.fond_plan.as_ref()).and_then(|id|p.vues.iter().find(|v|&v.id==id)) {
            if let Some(scene)=&self.scene {
                let point=|v:glam::DVec3| (v+scene.origin).truncate()*1000.0;
                for prim in self.primitives_fond_plan() {
                    match prim.forme {
                        crate::noyau::trace2d::Forme::Segments(segments)=>for [a,b] in segments {g.push(Geom::Segment([point(a),point(b)]));},
                        crate::noyau::trace2d::Forme::Polyligne{points,fermee}=>{
                            for w in points.windows(2) {g.push(Geom::Segment([point(w[0]),point(w[1])]));}
                            if fermee && points.len()>2 {g.push(Geom::Segment([point(*points.last().unwrap()),point(points[0])]));}
                        },
                        _=>{}
                    }
                }
                for a in p.annotations.iter().filter(|a|a.vue==source.id && !source.elements_masques.contains(&a.id)) {
                    if let Some(f)=a.forme.forme_2d() {g.extend(f.geoms());}
                }
            }
        }
        self.dessin2d.geoms = Some((self.bim.revision, cle, g.clone()));
        g
    }

    /// Extrémité de mur glissée : accrochages et cote depuis l'extrémité fixe.
    pub(super) fn point_edition_axe(&mut self,ui:&egui::Ui,rect:egui::Rect,q:egui::Pos2,maj:bool,fixe:[f64;2],_origine:[f64;2])->Option<[f64;2]> {
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
        self.dessin2d.directions_reference.clear();
        self.dessin2d.survol_accroche=None;
        self.dessin2d.saisie.clear();
    }
    pub(super) fn etat_accrochage(&self)->serde_json::Value {
        serde_json::json!({"trace":self.dessin2d.trace.as_ref().map(|t|t.points.iter().map(|p|p.to_array()).collect::<Vec<_>>()),"reference":self.dessin2d.reference.map(|p|p.to_array()),"point":self.dessin2d.point.map(|(p,a)|serde_json::json!({"position":p.to_array(),"accroche":a.libelle()}))})
    }

}

/// Bouton de la barre de commande : pictogramme seul (coche verte, croix rouge), à la
/// hauteur des autres commandes de la barre ; le libellé est dans l'infobulle.
fn bouton_commande(ui: &mut egui::Ui, valider: bool, libelle: &str) -> egui::Response {
    let h = ui.spacing().interact_size.y;
    let (r, reponse) = ui.allocate_exact_size(egui::vec2(h * 1.4, h), Sense::click());
    if reponse.hovered() {
        ui.painter().rect_filled(r, 4.0, ui.visuals().widgets.hovered.bg_fill);
    }
    let c = r.center();
    let u = h / 26.0;
    let couleur = if valider { Color32::from_rgb(30, 150, 70) } else { Color32::from_rgb(200, 50, 45) };
    let t = egui::Stroke::new(2.0, couleur);
    if valider {
        ui.painter().add(egui::Shape::line(vec![c + egui::vec2(-6.0, 0.0) * u, c + egui::vec2(-2.0, 5.0) * u, c + egui::vec2(6.0, -5.0) * u], t));
    } else {
        ui.painter().line_segment([c + egui::vec2(-5.0, -5.0) * u, c + egui::vec2(5.0, 5.0) * u], t);
        ui.painter().line_segment([c + egui::vec2(-5.0, 5.0) * u, c + egui::vec2(5.0, -5.0) * u], t);
    }
    reponse.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Button, true, libelle));
    reponse
}

/// Champ numérique dont la saisie n'est appliquée qu'à Entrée ou à la perte du focus :
/// rien n'est recalculé pendant la frappe. `facteur` : unité affichée (mm → cm = 10).
fn champ_valeur(ui: &mut egui::Ui, nom: &str, valeur: f64, facteur: f64, decimales: Option<usize>, largeur: f32) -> Option<f64> {
    let id = ui.make_persistent_id(("champ_valeur", nom));
    let tampon = ui.data(|d| d.get_temp::<String>(id));
    let mut texte = tampon.unwrap_or_else(|| match decimales {
        Some(n) => format!("{:.*}", n, valeur / facteur),
        None => crate::formats::ifc::step::format_real(valeur / facteur),
    });
    let reponse = ui.add(egui::TextEdit::singleline(&mut texte).desired_width(largeur));
    let mut resultat = None;
    if reponse.has_focus() {
        ui.data_mut(|d| d.insert_temp(id, texte.clone()));
    } else {
        ui.data_mut(|d| d.remove::<String>(id));
        if reponse.lost_focus() {
            resultat = texte.trim().replace(',', ".").parse::<f64>().ok().filter(|v| v.is_finite()).map(|v| v * facteur);
        }
    }
    resultat
}
