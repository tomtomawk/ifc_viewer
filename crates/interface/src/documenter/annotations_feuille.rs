//! Annotations des feuilles : les outils du ruban (Documenter → Texte, Polyligne, Cercle,
//! Cartouche) appliqués au papier quand un jeu est ouvert ; leurs réglages sont ceux de
//! la palette de création (hauteur de texte, type de trait, famille de cartouche).
//! Accrochage, aperçu et saisie du texte en place. Coordonnées en mm du papier, origine
//! en bas à gauche de la feuille.
use crate::*;
use crate::zones::ruban::Outil;
use modele::documentation as doc;
use modele::documentation::AnnotationForme;

#[derive(Clone, Copy, PartialEq, Eq, Default, Debug)]
pub(crate) enum OutilFeuille {
    #[default]
    Selection,
    Texte,
    Polyligne,
    Cercle,
    /// Pose d'un cartouche de la famille choisie, autant de fois que voulu.
    Cartouche,
}

impl OutilFeuille {
    /// Outil de feuille de l'outil actif du ruban (sélection pour les autres).
    pub(crate) fn du_ruban(outil: Option<Outil>) -> Self {
        match outil {
            Some(Outil::Texte) => Self::Texte,
            Some(Outil::Polyligne) => Self::Polyligne,
            Some(Outil::Cercle) => Self::Cercle,
            Some(Outil::Cartouche) => Self::Cartouche,
            _ => Self::Selection,
        }
    }

    pub(crate) fn consigne(self, points: usize) -> &'static str {
        match (self, points) {
            (Self::Texte, _) => "Texte : cliquer sur la feuille, taper, Entrée pour valider · Échap pour terminer",
            (Self::Polyligne, 0) => "Polyligne : cliquer le premier sommet · Maj : orthogonal · Échap pour terminer",
            (Self::Polyligne, _) => "Cliquer les sommets · Entrée ou double-clic pour finir · clic sur le premier sommet pour fermer · Échap pour annuler",
            (Self::Cercle, 0) => "Cercle : cliquer le centre · Échap pour terminer",
            (Self::Cercle, _) => "Cliquer un point du cercle · Échap pour annuler",
            (Self::Cartouche, _) => "Cartouche : famille dans Propriétés, clic pour le poser (aimanté aux marges), autant de fois que voulu · Échap pour terminer",
            (Self::Selection, _) => "",
        }
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
    /// Hauteur des nouveaux textes (mm du papier) : réglage de l'outil Texte.
    pub hauteur_texte: f64,
    /// Type de trait des polylignes et cercles : réglage du dessin 2D.
    pub type_trait: String,
    /// Famille des cartouches posés (nom ; vide : cartouche standard).
    pub famille_cartouche: String,
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
        match (self.outil, self.points.first().copied()) {
            (OutilFeuille::Selection | OutilFeuille::Cartouche, _) => None,
            (OutilFeuille::Texte, _) => {
                self.texte = Some(TexteFeuille { feuille: feuille.into(), rang: None, point: q, contenu: String::new(), hauteur, focus: true });
                None
            }
            (OutilFeuille::Cercle, Some(c)) => {
                self.points.clear();
                let rayon = ((q[0] - c[0]).powi(2) + (q[1] - c[1]).powi(2)).sqrt();
                (rayon > 0.0).then(|| doc::AnnotationFeuille { hauteur, forme: AnnotationForme::Cercle { centre: c, rayon, type_trait: self.type_trait.clone() } })
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
        if points.len() < 2 { return None; }
        let segments = if fermee { points.len() } else { points.len() - 1 };
        let traits = if self.type_trait.is_empty() { Vec::new() } else { vec![self.type_trait.clone(); segments] };
        Some(doc::AnnotationFeuille { hauteur: self.hauteur(), forme: AnnotationForme::Polyligne { points, fermee, courbures: Vec::new(), remplissage: None, traits } })
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
            OutilFeuille::Cercle => {
                let r = (vise[0] - a[0]).hypot(vise[1] - a[1]);
                vec![(0..=48).map(|k| { let t = k as f64 / 48.0 * std::f64::consts::TAU; [a[0] + r * t.cos(), a[1] + r * t.sin()] }).collect(), vec![a, vise]]
            }
            OutilFeuille::Polyligne => vec![self.points.iter().copied().chain([vise]).collect()],
            _ => Vec::new(),
        }
    }
}

/// Sommets d'accrochage d'une feuille : coins du papier et des marges, des cartouches et
/// sommets des annotations.
pub(crate) fn sommets_feuille(p: &modele::Projet, f: &doc::Feuille) -> Vec<[f64; 2]> {
    let (l, h, m) = (f.largeur, f.hauteur, doc::MARGE);
    let mut v = vec![[0.0, 0.0], [l, 0.0], [l, h], [0.0, h], [m, m], [l - m, m], [l - m, h - m], [m, h - m]];
    for c in &f.cartouches {
        let [x0, y0, x1, y1] = doc::cadre_cartouche(p, c);
        v.extend([[x0, y0], [x1, y0], [x1, y1], [x0, y1]]);
    }
    for a in &f.annotations {
        let mut b = a.clone();
        v.extend(b.points_mut().into_iter().map(|q| *q));
    }
    v
}

impl Application {
    /// Palette de l'outil Cartouche (Propriétés) : famille des cartouches à poser.
    pub(crate) fn selecteur_cartouche_pose(&mut self, ui: &mut egui::Ui) {
        use crate::zones::ruban::selecteur;
        let Some((p, _)) = &self.natif else { return };
        let choisie = &mut self.feuilles.trace.famille_cartouche;
        if !choisie.is_empty() && !p.familles_cartouches.iter().any(|f| &f.nom == choisie) {
            choisie.clear();
        }
        let actuel = if choisie.is_empty() { doc::NOM_CARTOUCHE_STANDARD.to_string() } else { choisie.clone() };
        let titre = format!("Cartouche · {actuel}");
        let bouton = selecteur::entete(ui, &titre, None, &format!("{titre}\nFamille des cartouches posés par clic sur la feuille."), None);
        let largeur = selecteur::largeur_liste(ui, p.familles_cartouches.iter().map(|f| f.nom.as_str()).chain([doc::NOM_CARTOUCHE_STANDARD]), 0.0, 0, 0.0);
        let mut choix: Option<String> = None;
        selecteur::liste(&bouton, largeur, 0.0, |ui| {
            let l = selecteur::Ligne { nom: doc::NOM_CARTOUCHE_STANDARD, courant: actuel == doc::NOM_CARTOUCHE_STANDARD, etiquette: None, sous_titre: None, detail: None, actions: &[] };
            if selecteur::ligne(ui, &l, None).choisi { choix = Some(String::new()); }
            for f in p.familles_cartouches.iter().filter(|f| f.nom != doc::NOM_CARTOUCHE_STANDARD) {
                let l = selecteur::Ligne { nom: &f.nom, courant: actuel == f.nom, etiquette: None, sous_titre: None, detail: None, actions: &[] };
                if selecteur::ligne(ui, &l, None).choisi { choix = Some(f.nom.clone()); }
            }
        });
        if let Some(c) = choix {
            self.feuilles.trace.famille_cartouche = c;
        }
    }
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
