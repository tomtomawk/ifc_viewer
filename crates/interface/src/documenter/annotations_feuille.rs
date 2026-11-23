//! Annotations des feuilles : outils de dessin sur le papier (texte, ligne, polyligne,
//! rectangle, cercle), accrochage, aperçu et saisie du texte en place. Coordonnées en mm
//! du papier, origine en bas à gauche de la feuille.
use crate::*;
use modele::documentation as doc;
use modele::documentation::AnnotationForme;

#[derive(Clone, Copy, PartialEq, Eq, Default, Debug)]
pub(crate) enum OutilFeuille {
    #[default]
    Selection,
    Texte,
    Ligne,
    Polyligne,
    Rectangle,
    Cercle,
}

impl OutilFeuille {
    pub(crate) const TOUS: [OutilFeuille; 6] = [Self::Selection, Self::Texte, Self::Ligne, Self::Polyligne, Self::Rectangle, Self::Cercle];

    pub(crate) fn libelle(self) -> &'static str {
        match self {
            Self::Selection => "Sélection",
            Self::Texte => "Texte",
            Self::Ligne => "Ligne",
            Self::Polyligne => "Polyligne",
            Self::Rectangle => "Rectangle",
            Self::Cercle => "Cercle",
        }
    }

    pub(crate) fn consigne(self, points: usize) -> &'static str {
        match (self, points) {
            (Self::Texte, _) => "Texte : cliquer sur la feuille, taper, Entrée pour valider · Échap pour revenir à la sélection",
            (Self::Ligne, 0) | (Self::Rectangle, 0) => "Cliquer le premier point · Maj : orthogonal · Échap pour revenir à la sélection",
            (Self::Ligne, _) => "Cliquer le second point · Maj : orthogonal · Échap pour annuler",
            (Self::Rectangle, _) => "Cliquer le coin opposé · Échap pour annuler",
            (Self::Polyligne, 0) => "Cliquer le premier sommet · Échap pour revenir à la sélection",
            (Self::Polyligne, _) => "Cliquer les sommets · Entrée ou double-clic pour finir · clic sur le premier sommet pour fermer · Échap pour annuler",
            (Self::Cercle, 0) => "Cliquer le centre · Échap pour revenir à la sélection",
            (Self::Cercle, _) => "Cliquer un point du cercle · Échap pour annuler",
            (Self::Selection, _) => "",
        }
    }
}

/// Nom de l'outil pour le pilote (`outil <nom>` quand un jeu est ouvert).
pub(crate) fn nom_pilote(o: OutilFeuille) -> &'static str {
    match o {
        OutilFeuille::Selection => "selection",
        OutilFeuille::Texte => "texte",
        OutilFeuille::Ligne => "ligne",
        OutilFeuille::Polyligne => "polyligne",
        OutilFeuille::Rectangle => "rectangle",
        OutilFeuille::Cercle => "cercle",
    }
}

/// Texte saisi en place : feuille, rang de l'annotation modifiée (None : nouvelle).
pub(crate) struct TexteFeuille {
    pub feuille: String,
    pub rang: Option<usize>,
    pub point: [f64; 2],
    pub contenu: String,
    pub hauteur: f64,
    pub focus: bool,
}

/// Outil actif et tracé en cours.
#[derive(Default)]
pub(crate) struct TraceFeuille {
    pub outil: OutilFeuille,
    /// Feuille du tracé en cours et points posés.
    pub feuille: Option<String>,
    pub points: Vec<[f64; 2]>,
    pub texte: Option<TexteFeuille>,
    /// Hauteur des nouveaux textes (mm du papier).
    pub hauteur_texte: f64,
}

impl TraceFeuille {
    pub(crate) fn choisir(&mut self, outil: OutilFeuille) {
        self.outil = outil;
        self.feuille = None;
        self.points.clear();
    }

    fn hauteur(&self) -> f64 {
        if self.hauteur_texte > 0.0 { self.hauteur_texte } else { doc::HAUTEUR_TEXTE_FEUILLE }
    }

    /// Clic de l'outil au point `q` de la feuille `feuille` : rend l'annotation achevée.
    pub(crate) fn clic(&mut self, feuille: &str, q: [f64; 2]) -> Option<doc::AnnotationFeuille> {
        if self.feuille.as_deref().is_some_and(|f| f != feuille) {
            self.points.clear();
        }
        self.feuille = Some(feuille.to_string());
        let hauteur = self.hauteur();
        let fin = |forme| Some(doc::AnnotationFeuille { hauteur, forme });
        match (self.outil, self.points.first().copied()) {
            (OutilFeuille::Selection, _) => None,
            (OutilFeuille::Texte, _) => {
                self.texte = Some(TexteFeuille { feuille: feuille.into(), rang: None, point: q, contenu: String::new(), hauteur, focus: true });
                None
            }
            (OutilFeuille::Ligne, Some(a)) => {
                self.points.clear();
                (a != q).then_some(()).and_then(|_| fin(AnnotationForme::Ligne { debut: a, fin: q }))
            }
            (OutilFeuille::Rectangle, Some(a)) => {
                self.points.clear();
                (a[0] != q[0] && a[1] != q[1]).then_some(()).and_then(|_| fin(AnnotationForme::Polyligne { points: vec![a, [q[0], a[1]], q, [a[0], q[1]]], fermee: true, courbures: Vec::new(), remplissage: None, traits: Vec::new() }))
            }
            (OutilFeuille::Cercle, Some(c)) => {
                self.points.clear();
                let rayon = ((q[0] - c[0]).powi(2) + (q[1] - c[1]).powi(2)).sqrt();
                (rayon > 0.0).then_some(()).and_then(|_| fin(AnnotationForme::Cercle { centre: c, rayon, type_trait: String::new() }))
            }
            (OutilFeuille::Polyligne, Some(a)) if self.points.len() >= 3 && q == a => self.terminer(true),
            (_, _) => {
                if self.points.last() != Some(&q) { self.points.push(q); }
                None
            }
        }
    }

    /// Fin de la polyligne en cours (Entrée, double-clic, clic sur le premier sommet).
    pub(crate) fn terminer(&mut self, fermee: bool) -> Option<doc::AnnotationFeuille> {
        if self.outil != OutilFeuille::Polyligne { return None; }
        let points = std::mem::take(&mut self.points);
        (points.len() >= 2).then(|| doc::AnnotationFeuille { hauteur: self.hauteur(), forme: AnnotationForme::Polyligne { points, fermee, courbures: Vec::new(), remplissage: None, traits: Vec::new() } })
    }

    /// Point visé : accroché aux sommets proches (`candidats`, à `tol` mm), sinon
    /// orthogonal au dernier point avec Maj.
    pub(crate) fn viser(&self, q: [f64; 2], candidats: &[[f64; 2]], tol: f64, ortho: bool) -> [f64; 2] {
        let proche = candidats.iter().chain(self.points.first()).map(|c| (c, (c[0] - q[0]).hypot(c[1] - q[1]))).filter(|(_, d)| *d <= tol).min_by(|a, b| a.1.total_cmp(&b.1));
        if let Some((c, _)) = proche { return *c; }
        match self.points.last() {
            Some(a) if ortho => if (q[0] - a[0]).abs() >= (q[1] - a[1]).abs() { [q[0], a[1]] } else { [a[0], q[1]] },
            _ => q,
        }
    }

    /// Aperçu du tracé en cours jusqu'au point visé, en points du papier.
    pub(crate) fn apercu(&self, vise: [f64; 2]) -> Vec<Vec<[f64; 2]>> {
        let Some(a) = self.points.first().copied() else { return Vec::new() };
        match self.outil {
            OutilFeuille::Ligne => vec![vec![a, vise]],
            OutilFeuille::Rectangle => vec![vec![a, [vise[0], a[1]], vise, [a[0], vise[1]], a]],
            OutilFeuille::Cercle => {
                let r = (vise[0] - a[0]).hypot(vise[1] - a[1]);
                vec![(0..=48).map(|k| { let t = k as f64 / 48.0 * std::f64::consts::TAU; [a[0] + r * t.cos(), a[1] + r * t.sin()] }).collect(), vec![a, vise]]
            }
            OutilFeuille::Polyligne => vec![self.points.iter().copied().chain([vise]).collect()],
            _ => Vec::new(),
        }
    }
}

/// Sommets d'accrochage d'une feuille : coins du papier et des marges, du cartouche et
/// sommets des annotations.
pub(crate) fn sommets_feuille(p: &modele::Projet, f: &doc::Feuille) -> Vec<[f64; 2]> {
    let (l, h, m) = (f.largeur, f.hauteur, doc::MARGE);
    let mut v = vec![[0.0, 0.0], [l, 0.0], [l, h], [0.0, h], [m, m], [l - m, m], [l - m, h - m], [m, h - m]];
    if let Some([x0, y0, x1, y1]) = doc::cadre_cartouche(p, f) {
        v.extend([[x0, y0], [x1, y0], [x1, y1], [x0, y1]]);
    }
    for a in &f.annotations {
        let mut b = a.clone();
        v.extend(b.points_mut().into_iter().map(|q| *q));
    }
    v
}

/// Barre des outils de la feuille, en bas à gauche de la vue (le haut porte les
/// consignes et les messages).
pub(crate) fn barre_outils(ui: &mut egui::Ui, rect: egui::Rect, trace: &mut TraceFeuille) {
    let cadre = egui::Rect::from_min_size(egui::pos2(rect.min.x + 8.0, rect.max.y - 34.0), egui::vec2(rect.width() - 16.0, 26.0));
    ui.scope_builder(egui::UiBuilder::new().max_rect(cadre), |ui| {
        egui::Frame::NONE.fill(Color32::from_white_alpha(235)).stroke(egui::Stroke::new(1.0, Color32::from_gray(170))).corner_radius(4.0).inner_margin(egui::Margin::symmetric(4, 2)).show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.label(RichText::new("Annoter").small().weak());
                for o in OutilFeuille::TOUS {
                    if ui.selectable_label(trace.outil == o, o.libelle()).clicked() {
                        trace.choisir(o);
                    }
                }
                if trace.outil == OutilFeuille::Texte {
                    let mut h = trace.hauteur();
                    ui.add(egui::DragValue::new(&mut h).speed(0.1).range(1.0..=50.0).max_decimals(1).suffix(" mm")).on_hover_text("Hauteur des nouveaux textes (papier)");
                    trace.hauteur_texte = h;
                }
            });
        });
    });
}

/// Champ du texte saisi en place, coin bas gauche en `e` (écran), à la taille du texte
/// (`taille` : pixels de la hauteur des capitales). Some(true) : valider ; Some(false) :
/// abandonner.
pub(crate) fn champ_texte(ui: &egui::Ui, rect: egui::Rect, e: egui::Pos2, taille: f32, t: &mut TexteFeuille) -> Option<bool> {
    let entree = ui.input_mut(|i| !i.modifiers.shift && i.consume_key(egui::Modifiers::NONE, egui::Key::Enter));
    let echap = ui.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::Escape));
    let taille = (taille / 0.72).clamp(11.0, 64.0);
    let couleur = Color32::from_rgb(0, 90, 180);
    let mut quitte = false;
    egui::Area::new(egui::Id::unique("texte_saisi_feuille"))
        .pivot(egui::Align2::LEFT_BOTTOM)
        .fixed_pos(e)
        .constrain_to(rect)
        .show(ui.ctx(), |ui| {
            egui::Frame::NONE.fill(Color32::from_white_alpha(230)).stroke(egui::Stroke::new(1.0, couleur)).inner_margin(egui::Margin::symmetric(2, 0)).show(ui, |ui| {
                let largeur = t.contenu.lines().chain(std::iter::once("Texte"))
                    .map(|l| crate::zones::ruban::selecteur::largeur_texte(ui, l, taille))
                    .fold(0.0f32, f32::max) + taille;
                let lignes = t.contenu.lines().count().max(1);
                let champ = egui::TextEdit::multiline(&mut t.contenu)
                    .font(egui::FontId::proportional(taille))
                    .text_color(Color32::from_gray(30))
                    .frame(egui::Frame::NONE)
                    .desired_rows(lignes)
                    .desired_width(largeur.min(rect.width() * 0.8))
                    .hint_text("Texte");
                let r = ui.add(champ);
                if t.focus {
                    r.request_focus();
                    t.focus = false;
                } else if r.lost_focus() {
                    quitte = true;
                }
            });
        });
    if echap { Some(false) } else if entree || quitte { Some(true) } else { None }
}
