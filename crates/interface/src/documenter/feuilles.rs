//! Jeux de feuilles : toutes les feuilles d'un jeu à la suite, à la manière d'InDesign.
//! Les vues se glissent depuis l'arborescence sur une feuille, se déplacent à la souris ;
//! feuilles et vues placées se règlent dans Propriétés, sans boîte de dialogue.
use commandes::Execution;
use crate::*;
use commandes::Commande;
use modele::documentation as doc;
use std::collections::HashMap;

/// Vue glissée depuis l'arborescence : identifiant de la vue du projet.
pub(crate) struct VueGlissee(pub String);

/// Feuille glissée dans l'arborescence pour la réordonner : identifiant.
pub(crate) struct FeuilleGlissee(pub String);

/// Nomenclature glissée depuis l'arborescence sur une feuille : identifiant.
pub(crate) struct NomenclatureGlissee(pub String);

/// Sélection dans le jeu ouvert : une feuille, ou un objet placé (feuille, rang : les
/// vues placées puis les nomenclatures posées), son cartouche ou une de ses annotations.
#[derive(Clone, PartialEq, Default, Debug)]
pub(crate) enum SelectionFeuille {
    #[default]
    Aucune,
    Feuille(String),
    Vue(String, usize),
    Cartouche(String),
    Annotation(String, usize),
}

/// Objet d'une feuille qui se choisit et se déplace à la souris.
#[derive(Clone, Copy, PartialEq, Debug)]
pub(crate) enum ObjetFeuille {
    /// Vue placée puis nomenclature posée (rang commun).
    Place(usize),
    Cartouche,
    Annotation(usize),
}

impl SelectionFeuille {
    /// Feuille de la sélection.
    pub(crate) fn feuille(&self) -> Option<&str> {
        match self {
            Self::Feuille(id) | Self::Vue(id, _) | Self::Cartouche(id) | Self::Annotation(id, _) => Some(id),
            Self::Aucune => None,
        }
    }

    pub(crate) fn objet(&self) -> Option<(&str, ObjetFeuille)> {
        match self {
            Self::Vue(id, j) => Some((id, ObjetFeuille::Place(*j))),
            Self::Cartouche(id) => Some((id, ObjetFeuille::Cartouche)),
            Self::Annotation(id, i) => Some((id, ObjetFeuille::Annotation(*i))),
            _ => None,
        }
    }

    pub(crate) fn de_objet(feuille: &str, o: ObjetFeuille) -> Self {
        match o {
            ObjetFeuille::Place(j) => Self::Vue(feuille.into(), j),
            ObjetFeuille::Cartouche => Self::Cartouche(feuille.into()),
            ObjetFeuille::Annotation(i) => Self::Annotation(feuille.into(), i),
        }
    }
}

/// Dessin préparé pour l'écran : triangles et traits en mm du papier (y vers le haut),
/// étendue du dessin [x0, y0, x1, y1].
#[derive(Default)]
pub(crate) struct Rendu {
    pub tris: Vec<([egui::Pos2; 3], Color32)>,
    /// Traits : extrémités, couleur, épaisseur (mm du papier ; 0 : un pixel).
    pub traits: Vec<([egui::Pos2; 2], Color32, f32)>,
    pub(crate) etendue: [f64; 4],
    pub(crate) erreur: Option<String>,
}

pub(crate) fn rendu(prims: &[noyau::trace2d::Primitive], etendue: [f64; 4]) -> Rendu {
    let mm = |q: glam::DVec3| egui::pos2((q.x * 1000.0) as f32, (q.y * 1000.0) as f32);
    let couleur = |c: [u8; 4]| Color32::from_rgba_unmultiplied(c[0], c[1], c[2], c[3]);
    let mut r = Rendu { etendue, ..Default::default() };
    // traits épais en segments à bouts francs, prolongés selon les traits rejoints
    // (comme l'export SVG) : jamais plus fins qu'un pixel à l'écran
    let raccords = noyau::trace2d::Raccords::new(prims, 1.0);
    for p in prims {
        let c = couleur(p.style.couleur);
        let segments: Option<Vec<[glam::DVec3; 2]>> = match &p.forme {
            _ if p.style.epaisseur <= 0.0 => None,
            noyau::trace2d::Forme::Polyligne { points, fermee } => {
                let mut v: Vec<[glam::DVec3; 2]> = points.windows(2).map(|w| [w[0], w[1]]).collect();
                if *fermee && points.len() > 2 {
                    v.push([points[points.len() - 1], points[0]]);
                }
                Some(v)
            }
            noyau::trace2d::Forme::Segments(s) => Some(s.clone()),
            _ => None,
        };
        match segments {
            Some(seg) => {
                let h = p.style.epaisseur / 2000.0;
                for [a, b] in seg {
                    let u = (b - a).normalize_or_zero();
                    let (a, b) = (a - u * raccords.prolongement(a, b, h), b + u * raccords.prolongement(b, a, h));
                    r.traits.push(([mm(a), mm(b)], c, p.style.epaisseur as f32));
                }
            }
            None => {
                let m = p.mailler(glam::DVec3::Z, 1.0);
                r.tris.extend(m.triangles.iter().map(|t| (t.map(mm), c)));
                r.traits.extend(m.traits.iter().map(|t| (t.map(mm), c, 0.0)));
            }
        }
    }
    r
}

/// Tableau d'une nomenclature posé en position nulle (coin haut gauche à l'origine).
pub(crate) fn table_zero(p: &modele::Projet, id: &str) -> Rendu {
    match doc::nomenclature_placee(p, &doc::NomenclaturePlacee { nomenclature: id.into(), position: [0.0, 0.0] }) {
        Ok((prims, e)) => rendu(&prims, e),
        Err(e) => Rendu { erreur: Some(e), ..Default::default() },
    }
}

#[derive(Default)]
pub(crate) struct EtatFeuilles {
    /// Fenêtre d'export multiformat du jeu (n° 13).
    pub export: Option<crate::documenter::export_jeu::ExportJeu>,
    /// Export de jeu : un seul PDF pour le jeu (sinon un par feuille) ; gardé pendant la
    /// session.
    pub pdf_groupe: bool,
    /// Pixels par mm du papier, position écran de l'origine du jeu (coin haut gauche de
    /// la première feuille) par rapport au coin de la vue.
    pub(crate) zoom: f32,
    pub(crate) decalage: egui::Vec2,
    /// Jeu pour lequel le cadrage a été fait.
    pub(crate) cadre_pour: Option<String>,
    pub selection: SelectionFeuille,
    /// Objet en cours de déplacement : feuille, objet, déplacement à l'écran (aimanté
    /// aux cadres voisins).
    pub(crate) glisse: Option<(String, ObjetFeuille, egui::Vec2)>,
    /// Déplacement brut de la souris pendant ce glissement, avant aimantation.
    pub(crate) glisse_brut: egui::Vec2,
    /// Feuille glissée par son titre pour la remettre à un autre rang du jeu.
    pub(crate) glisse_feuille: Option<String>,
    /// Dessins des vues à leur échelle, en position nulle : (vue, échelle).
    pub(crate) vues: HashMap<(String, u64), Rendu>,
    /// Cartouches des feuilles.
    pub(crate) cadres: HashMap<String, Rendu>,
    /// Annotations des feuilles, dans l'ordre de la feuille.
    pub(crate) annotations: HashMap<String, Vec<Rendu>>,
    /// Tableaux des nomenclatures posées (position nulle), par nomenclature.
    pub(crate) tables: HashMap<String, Rendu>,
    /// (document, révision, empreinte du modèle hors feuilles) : les dessins des vues ne sont
    /// recalculés que si le modèle change.
    pub(crate) empreinte: Option<(usize, u64, u64)>,
    /// Feuille à amener en haut de la vue.
    pub defiler_vers: Option<String>,
    /// Outil d'annotation actif, tracé et texte en cours.
    pub trace: crate::documenter::annotations_feuille::TraceFeuille,
    /// Position écran du coin bas gauche de chaque feuille affichée (pilote : `ecran`).
    pub(crate) origines: HashMap<String, egui::Pos2>,
}

impl EtatFeuilles {
    /// Position écran du point `q` (mm du papier) de la feuille `id`, telle qu'affichée.
    pub(crate) fn ecran_papier(&self, id: &str, q: [f64; 2]) -> Option<egui::Pos2> {
        self.origines.get(id).map(|o| *o + egui::vec2(q[0] as f32, -q[1] as f32) * self.zoom)
    }
}

/// Écart entre deux feuilles (mm du papier).
pub(crate) const ECART: f32 = 24.0;
pub(crate) const HAUT_FEUILLES: f32 = 64.0;

impl Application {
    pub(crate) fn creer_jeu_arborescence(&mut self, source: Option<&str>) {
        let Some((p, _)) = &self.natif else { return };
        let existants = doc::jeux(p);
        let base = source.map_or("Jeu".to_string(), |s| format!("{s} - copie"));
        let nom = if !existants.contains(&base) { base.clone() } else {
            (2..).map(|n| format!("{base} {n}")).find(|n| !existants.contains(n)).unwrap_or_default()
        };
        let mut q = p.clone();
        let copies: Vec<_> = source.map(|s| p.feuilles.iter().filter(|f| f.jeu == s).cloned().collect()).unwrap_or_default();
        if copies.is_empty() {
            let f = Self::nouvelle_feuille(&q, &nom);
            match q.executer(Commande::Feuille { feuille: f }) {
                Ok((projet, _)) => q = projet,
                Err(e) => { self.error = Some(e); return; }
            }
        } else {
            for mut f in copies {
                let famille = q.cartouches_feuilles.get(&f.id).cloned();
                f.id.clear();
                f.jeu = nom.clone();
                f.numero = Self::nouvelle_feuille(&q, &nom).numero;
                match q.executer(Commande::Feuille { feuille: f }) {
                    Ok((mut projet, ids)) => {
                        if let (Some(nom), Some(id)) = (famille, ids.first()) { projet.cartouches_feuilles.insert(id.clone(), nom); }
                        q = projet;
                    },
                    Err(e) => { self.error = Some(e); return; }
                }
            }
        }
        if let Err(e) = self.appliquer_projet(q) { self.error = Some(e); return; }
        self.ouvrir_jeu(&nom, None);
    }

    /// Nom du jeu de feuilles de la vue active, s'il y en a un.
    pub(crate) fn jeu_actif(&self) -> Option<String> {
        let i = self.vue_active?;
        let v = self.scene.as_ref()?.navigateur.as_ref()?.vues.get(i)?;
        (v.genre == "Jeu de feuilles").then(|| v.nom.clone())
    }

    /// Ouvre le jeu `jeu` (onglet), et amène la feuille `feuille` en haut.
    pub(crate) fn ouvrir_jeu(&mut self, jeu: &str, feuille: Option<String>) {
        let i = self.scene.as_ref().and_then(|s| s.navigateur.as_ref()).and_then(|n| n.vues.iter().position(|v| v.genre == "Jeu de feuilles" && v.nom == jeu));
        if let Some(i) = i {
            self.famille_selectionnee = None;
            self.activer_vue(i);
            self.vue_active = Some(i);
            if let Some(f) = feuille {
                self.feuilles.selection = SelectionFeuille::Feuille(f.clone());
                self.feuilles.defiler_vers = Some(f);
            } else {
                self.feuilles.selection = SelectionFeuille::Aucune;
            }
        }
    }

    /// Nouvelle feuille à la suite du jeu : format de sa dernière feuille (A3 sinon),
    /// numéro suivant.
    pub(crate) fn nouvelle_feuille(p: &modele::Projet, jeu: &str) -> doc::Feuille {
        let derniere = p.feuilles.iter().rev().find(|f| f.jeu == jeu);
        let (largeur, hauteur) = derniere.map_or((420.0, 297.0), |f| (f.largeur, f.hauteur));
        let numero = (p.feuilles.len() + 1..).map(|n| format!("A{n:02}")).find(|n| !p.feuilles.iter().any(|f| &f.numero == n)).unwrap_or_default();
        doc::Feuille { id: String::new(), numero, nom: "Nouvelle feuille".into(), jeu: jeu.into(), largeur, hauteur, cartouche: derniere.map_or(String::new(), |f| f.cartouche.clone()), vues: Vec::new(), nomenclatures: Vec::new(), position_cartouche: None, sans_cartouche: false, annotations: Vec::new() }
    }

    /// Ruban, Documenter → Feuilles : ouvre le premier jeu ; sans feuille, en crée une.
    pub(crate) fn commande_feuilles(&mut self) {
        let Some((p, _)) = &self.natif else {
            self.status = "Feuilles : ouvrir un projet natif.".into();
            return;
        };
        let jeu = doc::jeux(p).into_iter().next();
        let jeu = match jeu {
            Some(j) => j,
            None => {
                let f = Self::nouvelle_feuille(p, doc::JEU_DEFAUT);
                if let Err(e) = self.executer_bim(Commande::Feuille { feuille: f }) {
                    self.error = Some(e);
                    return;
                }
                doc::JEU_DEFAUT.to_string()
            }
        };
        self.ouvrir_jeu(&jeu, None);
    }

    /// Demande le fichier, puis exporte le jeu `jeu` en un PDF de plusieurs pages.
    /// PDF de ce qui est affiché quand un jeu de feuilles est ouvert : la feuille
    /// choisie (ou celle de la vue choisie), sinon tout le jeu ; None hors jeu. Cadre,
    /// cartouche, vues et nomenclatures à leur place, au format du papier (n° 12).
    pub(crate) fn pdf_feuilles_affichees(&self) -> Option<Result<Vec<u8>, String>> {
        let jeu = self.jeu_actif()?;
        let (p, _) = self.natif.as_ref()?;
        let choisie = self.feuilles.selection.feuille().and_then(|id| p.feuilles.iter().find(|f| f.id == id && f.jeu == jeu));
        Some(match choisie {
            Some(f) => moteur2d::svg_feuille(p, f).and_then(|s| moteur2d::pdf(&s)),
            None => moteur2d::pdf_jeu(p, &jeu),
        })
    }

    /// PDF du jeu `jeu` (une page par feuille) écrit dans `chemin` ; nombre de pages.
    pub(crate) fn exporter_jeu(&self, jeu: &str, chemin: &std::path::Path) -> Result<usize, String> {
        let (p, _) = self.natif.as_ref().ok_or("Aucun projet natif ouvert.")?;
        let n = p.feuilles.iter().filter(|f| f.jeu == jeu).count();
        if n == 0 {
            return Err(format!("Le jeu « {jeu} » n'a aucune feuille."));
        }
        let pdf = moteur2d::pdf_jeu(p, jeu)?;
        echanges::enregistrement::ecrire_atomique(chemin, &pdf)?;
        Ok(n)
    }

    /// Une feuille en SVG ou en PDF, écrite en arrière-plan (`socle::taches`).
    pub(crate) fn exporter_feuille(&mut self, ctx: &egui::Context, extension: &str, id: &str) {
        let Some((p, _)) = &self.natif else { return };
        let Some(f) = p.feuilles.iter().find(|f| f.id == id) else { return };
        let Some(path) = rfd::FileDialog::new().set_file_name(format!("{} {}.{extension}", f.numero, f.nom)).add_filter(extension, &[extension]).save_file() else {
            return;
        };
        self.exporter_feuille_arriere_plan(ctx, extension, id.to_string(), path);
    }

    /// Ouvre la fenêtre des nomenclatures sur la nomenclature `id`.
    pub(crate) fn ouvrir_nomenclature(&mut self, id: &str) {
        if let Some(k) = self.natif.as_ref().and_then(|(p, _)| p.nomenclatures.iter().position(|n| n.id == id)) {
            self.bim.nomenclature = k;
            self.bim.documentation = true;
        }
    }

    /// Remet la feuille `id` dans le jeu `jeu`, avant la feuille `avant` (à la fin sans).
    pub(crate) fn deplacer_feuille(&mut self, id: &str, jeu: &str, avant: Option<&str>) {
        let Some((p, _)) = &self.natif else { return };
        let mut q = p.clone();
        match doc::deplacer_feuille(&mut q, id, jeu, avant).and_then(|_| self.appliquer_projet(q)) {
            Ok(()) => {
                self.feuilles.selection = SelectionFeuille::Feuille(id.into());
                self.status = "Feuille déplacée — Ctrl+Z pour annuler.".into();
            }
            Err(e) => self.error = Some(e),
        }
    }

    /// Suppr dans un jeu de feuilles : retire la vue placée, le cartouche ou l'annotation
    /// choisis, ou supprime la feuille. Faux si rien n'est choisi dans le jeu.
    pub(crate) fn supprimer_dans_jeu(&mut self) -> bool {
        let Some(jeu) = self.jeu_actif() else { return false };
        let Some((p, _)) = &self.natif else { return false };
        let commande = match self.feuilles.selection.clone() {
            SelectionFeuille::Cartouche(id) => p.feuilles.iter().find(|f| f.id == id && f.jeu == jeu && !f.sans_cartouche).map(|f| {
                self.feuilles.selection = SelectionFeuille::Feuille(id.clone());
                self.status = "Cartouche retiré de la feuille (Propriétés de la feuille pour le remettre) — Ctrl+Z pour annuler.".into();
                Commande::Feuille { feuille: doc::Feuille { sans_cartouche: true, ..f.clone() } }
            }),
            SelectionFeuille::Annotation(id, i) => p.feuilles.iter().find(|f| f.id == id && f.jeu == jeu && i < f.annotations.len()).map(|f| {
                let mut g = f.clone();
                g.annotations.remove(i);
                self.feuilles.selection = SelectionFeuille::Feuille(id.clone());
                Commande::Feuille { feuille: g }
            }),
            SelectionFeuille::Vue(id, j) => p.feuilles.iter().find(|f| f.id == id && f.jeu == jeu && j < f.vues.len() + f.nomenclatures.len()).map(|f| {
                let mut g = f.clone();
                if j < g.vues.len() {
                    g.vues.remove(j);
                } else {
                    g.nomenclatures.remove(j - f.vues.len());
                }
                self.feuilles.selection = SelectionFeuille::Feuille(id.clone());
                Commande::Feuille { feuille: g }
            }),
            SelectionFeuille::Feuille(id) if p.feuilles.iter().any(|f| f.id == id) => {
                self.feuilles.selection = SelectionFeuille::Aucune;
                Some(Commande::Supprimer { ids: vec![id] })
            }
            _ => None,
        };
        let Some(c) = commande else { return true };
        if let Err(e) = self.executer_bim(c) {
            self.error = Some(e);
        }
        true
    }
}

