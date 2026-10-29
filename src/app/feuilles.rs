//! Jeux de feuilles : toutes les feuilles d'un jeu à la suite, à la manière d'InDesign.
//! Les vues se glissent depuis l'arborescence sur une feuille, se déplacent à la souris ;
//! feuilles et vues placées se règlent dans Propriétés, sans boîte de dialogue.
use super::*;
use crate::modele::commandes::Commande;
use crate::modele::documentation as doc;
use std::collections::HashMap;

/// Vue glissée depuis l'arborescence : identifiant de la vue du projet.
pub(super) struct VueGlissee(pub String);

/// Feuille glissée dans l'arborescence pour la réordonner : identifiant.
pub(super) struct FeuilleGlissee(pub String);

/// Nomenclature glissée depuis l'arborescence sur une feuille : identifiant.
pub(super) struct NomenclatureGlissee(pub String);

/// Sélection dans le jeu ouvert : une feuille, ou un objet placé (feuille, rang : les
/// vues placées puis les nomenclatures posées).
#[derive(Clone, PartialEq, Default)]
pub(super) enum SelectionFeuille {
    #[default]
    Aucune,
    Feuille(String),
    Vue(String, usize),
}

/// Dessin préparé pour l'écran : triangles et traits en mm du papier (y vers le haut),
/// étendue du dessin [x0, y0, x1, y1].
#[derive(Default)]
struct Rendu {
    tris: Vec<([egui::Pos2; 3], Color32)>,
    /// Traits : extrémités, couleur, épaisseur (mm du papier ; 0 : un pixel).
    traits: Vec<([egui::Pos2; 2], Color32, f32)>,
    etendue: [f64; 4],
    erreur: Option<String>,
}

fn rendu(prims: &[crate::trace2d::Primitive], etendue: [f64; 4]) -> Rendu {
    let mm = |q: glam::DVec3| egui::pos2((q.x * 1000.0) as f32, (q.y * 1000.0) as f32);
    let couleur = |c: [u8; 4]| Color32::from_rgba_unmultiplied(c[0], c[1], c[2], c[3]);
    let mut r = Rendu { etendue, ..Default::default() };
    // traits épais en segments à bouts francs, prolongés selon les traits rejoints
    // (comme l'export SVG) : jamais plus fins qu'un pixel à l'écran
    let raccords = crate::trace2d::Raccords::new(prims, 1.0);
    for p in prims {
        let c = couleur(p.style.couleur);
        let segments: Option<Vec<[glam::DVec3; 2]>> = match &p.forme {
            _ if p.style.epaisseur <= 0.0 => None,
            crate::trace2d::Forme::Polyligne { points, fermee } => {
                let mut v: Vec<[glam::DVec3; 2]> = points.windows(2).map(|w| [w[0], w[1]]).collect();
                if *fermee && points.len() > 2 {
                    v.push([points[points.len() - 1], points[0]]);
                }
                Some(v)
            }
            crate::trace2d::Forme::Segments(s) => Some(s.clone()),
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
fn table_zero(p: &crate::modele::Projet, id: &str) -> Rendu {
    match doc::nomenclature_placee(p, &doc::NomenclaturePlacee { nomenclature: id.into(), position: [0.0, 0.0] }) {
        Ok((prims, e)) => rendu(&prims, e),
        Err(e) => Rendu { erreur: Some(e), ..Default::default() },
    }
}

#[derive(Default)]
pub(super) struct EtatFeuilles {
    /// Pixels par mm du papier, position écran de l'origine du jeu (coin haut gauche de
    /// la première feuille) par rapport au coin de la vue.
    zoom: f32,
    decalage: egui::Vec2,
    /// Jeu pour lequel le cadrage a été fait.
    cadre_pour: Option<String>,
    pub selection: SelectionFeuille,
    /// Vue placée en cours de déplacement : feuille, rang, déplacement à l'écran.
    glisse: Option<(String, usize, egui::Vec2)>,
    /// Feuille glissée par son titre pour la remettre à un autre rang du jeu.
    glisse_feuille: Option<String>,
    /// Dessins des vues à l'échelle de la feuille, position nulle : (vue, échelle).
    vues: HashMap<(String, u64), Rendu>,
    /// Cadres et cartouches des feuilles.
    cadres: HashMap<String, Rendu>,
    /// Tableaux des nomenclatures posées (position nulle), par nomenclature.
    tables: HashMap<String, Rendu>,
    /// (révision, empreinte du modèle hors feuilles) : les dessins des vues ne sont
    /// recalculés que si le modèle change.
    empreinte: Option<(u64, u64)>,
    /// Feuille à amener en haut de la vue.
    pub defiler_vers: Option<String>,
}

/// Écart entre deux feuilles (mm du papier).
const ECART: f32 = 40.0;

impl ViewerApp {
    /// Nom du jeu de feuilles de la vue active, s'il y en a un.
    pub(super) fn jeu_actif(&self) -> Option<String> {
        let i = self.vue_active?;
        let v = self.scene.as_ref()?.navigateur.as_ref()?.vues.get(i)?;
        (v.genre == "Jeu de feuilles").then(|| v.nom.clone())
    }

    /// Ouvre le jeu `jeu` (onglet), et amène la feuille `feuille` en haut.
    pub(super) fn ouvrir_jeu(&mut self, jeu: &str, feuille: Option<String>) {
        let i = self.scene.as_ref().and_then(|s| s.navigateur.as_ref()).and_then(|n| n.vues.iter().position(|v| v.genre == "Jeu de feuilles" && v.nom == jeu));
        if let Some(i) = i {
            self.activer_vue(i);
            self.vue_active = Some(i);
            if let Some(f) = feuille {
                self.feuilles.selection = SelectionFeuille::Feuille(f.clone());
                self.feuilles.defiler_vers = Some(f);
            }
        }
    }

    /// Nouvelle feuille à la suite du jeu : format de sa dernière feuille (A3 sinon),
    /// numéro suivant.
    pub(super) fn nouvelle_feuille(p: &crate::modele::Projet, jeu: &str) -> doc::Feuille {
        let derniere = p.feuilles.iter().rev().find(|f| f.jeu == jeu);
        let (largeur, hauteur) = derniere.map_or((420.0, 297.0), |f| (f.largeur, f.hauteur));
        let numero = (p.feuilles.len() + 1..).map(|n| format!("A{n:02}")).find(|n| !p.feuilles.iter().any(|f| &f.numero == n)).unwrap_or_default();
        doc::Feuille { id: String::new(), numero, nom: "Nouvelle feuille".into(), jeu: jeu.into(), largeur, hauteur, cartouche: derniere.map_or(String::new(), |f| f.cartouche.clone()), vues: Vec::new(), nomenclatures: Vec::new() }
    }

    /// Ruban, Documenter → Feuilles : ouvre le premier jeu ; sans feuille, en crée une.
    pub(super) fn commande_feuilles(&mut self) {
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

    /// Vue du jeu ouvert : feuilles l'une sous l'autre, molette : zoom, glisser le fond :
    /// défiler, glisser une vue placée : la déplacer, déposer une vue de l'arborescence :
    /// la placer ; dernière page en pointillés : ajouter une feuille.
    pub(super) fn vue_jeu(&mut self, ui: &mut egui::Ui) {
        let Some(jeu) = self.jeu_actif() else { return };
        let mut etat = std::mem::take(&mut self.feuilles);
        let mut commandes: Vec<Commande> = Vec::new();
        // feuille à déplacer : (feuille, feuille devant laquelle la mettre ; None : à la fin)
        let mut deplacement: Option<(String, Option<String>)> = None;
        let mut ouvrir_vue: Option<String> = None;
        if let Some((p, _)) = &self.natif {
            // dessins des vues recalculés seulement si le modèle (hors feuilles) change
            if etat.empreinte.map(|e| e.0) != Some(self.bim.revision) {
                use std::hash::{Hash, Hasher};
                let mut q = p.clone();
                q.feuilles.clear();
                q.positions_plan.clear();
                let mut h = std::collections::hash_map::DefaultHasher::new();
                q.json().unwrap_or_default().hash(&mut h);
                let h = h.finish();
                if etat.empreinte.is_some_and(|e| e.1 != h) {
                    etat.vues.clear();
                }
                etat.cadres.clear();
                etat.tables.clear();
                etat.empreinte = Some((self.bim.revision, h));
            }
            let feuilles: Vec<&doc::Feuille> = p.feuilles.iter().filter(|f| f.jeu == jeu).collect();
            let (rect, resp) = ui.allocate_exact_size(ui.available_size(), Sense::click_and_drag());
            let painter = ui.painter_at(rect);
            painter.rect_filled(rect, 0.0, Color32::from_gray(205));
            // feuilles l'une sous l'autre, puis la page « ajouter »
            let mut ys = Vec::new();
            let mut y = 0.0;
            for f in &feuilles {
                ys.push(y);
                y += f.hauteur as f32 + ECART;
            }
            let (l_ajout, h_ajout) = feuilles.last().map_or((420.0, 297.0), |f| (f.largeur as f32, f.hauteur as f32));
            let y_ajout = y;
            let largeur_max = feuilles.iter().map(|f| f.largeur as f32).fold(l_ajout, f32::max);
            if etat.cadre_pour.as_deref() != Some(jeu.as_str()) || etat.zoom <= 0.0 {
                etat.zoom = (rect.width() * 0.85 / largeur_max).max(0.05);
                etat.decalage = egui::vec2((rect.width() - largeur_max * etat.zoom) * 0.5, 36.0);
                etat.cadre_pour = Some(jeu.clone());
            }
            // feuille demandée : cadrée entière en haut de la vue
            if let Some(id) = etat.defiler_vers.take() {
                if let Some(k) = feuilles.iter().position(|f| f.id == id) {
                    let (l, h) = (feuilles[k].largeur as f32, feuilles[k].hauteur as f32);
                    etat.zoom = (rect.width() * 0.85 / l).min((rect.height() - 70.0) / h).max(0.05);
                    etat.decalage = egui::vec2((rect.width() - l * etat.zoom) * 0.5, 36.0 - ys[k] * etat.zoom);
                }
            }
            // molette : zoom autour du curseur
            if let Some(q) = resp.hover_pos() {
                let d = ui.input(|i| i.smooth_scroll_delta.y);
                if d != 0.0 {
                    let facteur = (d * 0.0015).exp();
                    let zoom = (etat.zoom * facteur).clamp(0.02, 60.0);
                    let o = rect.min + etat.decalage;
                    etat.decalage = q - rect.min - (q - o) * (zoom / etat.zoom);
                    etat.zoom = zoom;
                }
            }
            let (z, o) = (etat.zoom, rect.min + etat.decalage);
            let ecran = |k: usize, x: f64, yh: f64| o + egui::vec2(x as f32, ys[k] + feuilles[k].hauteur as f32 - yh as f32) * z;
            let papier = |q: egui::Pos2| -> Option<(usize, [f64; 2])> {
                let w = (q - o) / z;
                (0..feuilles.len()).find(|&k| w.x >= 0.0 && w.x <= feuilles[k].largeur as f32 && w.y >= ys[k] && w.y <= ys[k] + feuilles[k].hauteur as f32).map(|k| (k, [w.x as f64, (ys[k] + feuilles[k].hauteur as f32 - w.y) as f64]))
            };
            let page = |k: usize| egui::Rect::from_min_size(o + egui::vec2(0.0, ys[k]) * z, egui::vec2(feuilles[k].largeur as f32, feuilles[k].hauteur as f32) * z);
            let page_ajout = egui::Rect::from_min_size(o + egui::vec2(0.0, y_ajout) * z, egui::vec2(l_ajout, h_ajout) * z);
            // dessins des vues placées (position nulle), calculés à la demande
            for f in &feuilles {
                for v in &f.vues {
                    etat.vues.entry((v.vue.clone(), v.echelle.to_bits())).or_insert_with(|| {
                        let zero = doc::VuePlacee { vue: v.vue.clone(), position: [0.0, 0.0], echelle: v.echelle };
                        match doc::vue_placee(p, &zero) {
                            Ok((prims, e)) => rendu(&prims, e),
                            Err(e) => Rendu { erreur: Some(e), ..Default::default() },
                        }
                    });
                }
                etat.cadres.entry(f.id.clone()).or_insert_with(|| rendu(&doc::cadre_feuille(p, f), [0.0; 4]));
                for n in &f.nomenclatures {
                    etat.tables.entry(n.nomenclature.clone()).or_insert_with(|| table_zero(p, &n.nomenclature));
                }
            }
            // cadre d'une vue placée sur sa feuille (mm du papier), titre compris
            let etendues: HashMap<(String, u64), [f64; 4]> = etat.vues.iter().map(|(c, r)| (c.clone(), r.etendue)).collect();
            let etendues_tables: HashMap<String, [f64; 4]> = etat.tables.iter().map(|(c, r)| (c.clone(), r.etendue)).collect();
            // objet j d'une feuille : ses vues placées, puis ses nomenclatures
            let boite = |f: &doc::Feuille, j: usize| -> Option<[f64; 4]> {
                if let Some(v) = f.vues.get(j) {
                    let e = *etendues.get(&(v.vue.clone(), v.echelle.to_bits()))?;
                    return Some([e[0] + v.position[0] - 2.0, e[1] + v.position[1] - 16.0, e[2] + v.position[0] + 2.0, e[3] + v.position[1] + 2.0]);
                }
                let n = f.nomenclatures.get(j - f.vues.len())?;
                let e = *etendues_tables.get(&n.nomenclature)?;
                Some([e[0] + n.position[0] - 2.0, e[1] + n.position[1] - 2.0, e[2] + n.position[0] + 2.0, e[3] + n.position[1] + 2.0])
            };
            let vue_sous = |q: egui::Pos2| -> Option<(usize, usize)> {
                let (k, pt) = papier(q)?;
                let f = feuilles[k];
                (0..f.vues.len() + f.nomenclatures.len()).rev().find(|&j| boite(f, j).is_some_and(|b| pt[0] >= b[0] && pt[0] <= b[2] && pt[1] >= b[1] && pt[1] <= b[3])).map(|j| (k, j))
            };
            // titre d'une feuille (au-dessus de la page) : poignée pour la déplacer
            let titre_sous = |q: egui::Pos2| -> Option<usize> {
                let w = (q - o) / z;
                (0..feuilles.len()).find(|&k| w.x >= 0.0 && w.x <= feuilles[k].largeur as f32 && w.y >= ys[k] - ECART * 0.8 && w.y < ys[k])
            };
            // rang d'insertion sous le curseur : nombre de feuilles dont le milieu est au-dessus
            let rang_sous = |q: egui::Pos2| -> usize {
                let w = (q - o) / z;
                (0..feuilles.len()).filter(|&k| ys[k] + feuilles[k].hauteur as f32 * 0.5 < w.y).count()
            };
            // souris : sélection, déplacement d'une vue placée ou d'une feuille, défilement
            if resp.drag_started_by(egui::PointerButton::Primary) {
                let depart = ui.input(|i| i.pointer.press_origin());
                if let Some((k, j)) = depart.and_then(vue_sous) {
                    etat.glisse = Some((feuilles[k].id.clone(), j, egui::Vec2::ZERO));
                    etat.selection = SelectionFeuille::Vue(feuilles[k].id.clone(), j);
                } else if let Some(k) = depart.and_then(titre_sous) {
                    etat.glisse_feuille = Some(feuilles[k].id.clone());
                    etat.selection = SelectionFeuille::Feuille(feuilles[k].id.clone());
                }
            }
            if resp.dragged() {
                match &mut etat.glisse {
                    Some((_, _, d)) => *d += resp.drag_delta(),
                    None if etat.glisse_feuille.is_some() => {}
                    None => etat.decalage += resp.drag_delta(),
                }
            }
            if resp.drag_stopped() {
                if let (Some(id), Some(q)) = (etat.glisse_feuille.take(), resp.interact_pointer_pos().or(ui.input(|i| i.pointer.latest_pos()))) {
                    let t = rang_sous(q);
                    if let Some(k) = feuilles.iter().position(|f| f.id == id) {
                        if t != k && t != k + 1 {
                            deplacement = Some((id, feuilles.get(t).map(|f| f.id.clone())));
                        }
                    }
                }
                if let Some((fid, j, d)) = etat.glisse.take() {
                    if let Some(f) = feuilles.iter().find(|f| f.id == fid) {
                        let mut f = (*f).clone();
                        let nv = f.vues.len();
                        let position = if j < nv { f.vues.get_mut(j).map(|v| &mut v.position) } else { f.nomenclatures.get_mut(j - nv).map(|n| &mut n.position) };
                        if let Some(pos) = position {
                            pos[0] += (d.x / z) as f64;
                            pos[1] -= (d.y / z) as f64;
                            if d.length() > 1.0 {
                                commandes.push(Commande::Feuille { feuille: f });
                            }
                        }
                    }
                }
            }
            if resp.clicked() {
                let q = resp.interact_pointer_pos();
                etat.selection = match (q.and_then(vue_sous), q.and_then(papier)) {
                    (Some((k, j)), _) => SelectionFeuille::Vue(feuilles[k].id.clone(), j),
                    (None, Some((k, _))) => SelectionFeuille::Feuille(feuilles[k].id.clone()),
                    _ => match q.and_then(titre_sous) {
                        Some(k) => SelectionFeuille::Feuille(feuilles[k].id.clone()),
                        None => SelectionFeuille::Aucune,
                    },
                };
                if q.is_some_and(|q| page_ajout.contains(q)) {
                    let f = Self::nouvelle_feuille(p, &jeu);
                    commandes.push(Commande::Feuille { feuille: f });
                }
            }
            if resp.double_clicked() {
                if let Some((k, j)) = resp.interact_pointer_pos().and_then(vue_sous) {
                    match feuilles[k].vues.get(j) {
                        Some(v) => ouvrir_vue = Some(v.vue.clone()),
                        None => ouvrir_vue = feuilles[k].nomenclatures.get(j - feuilles[k].vues.len()).map(|n| format!("nomenclature:{}", n.nomenclature)),
                    }
                }
            }
            // vue déposée depuis l'arborescence : placée, centrée sur le curseur
            let survol_depot = resp.dnd_hover_payload::<VueGlissee>().map(|_| ()).or(resp.dnd_hover_payload::<NomenclatureGlissee>().map(|_| ())).and_then(|_| resp.hover_pos().or(ui.input(|i| i.pointer.hover_pos()))).and_then(papier).map(|(k, _)| k);
            // (une lecture du dépôt retire l'objet glissé quel que soit son type : ne lire
            // que celui qui est glissé)
            let depot_vue = resp.dnd_hover_payload::<VueGlissee>().is_some();
            let depot_table = resp.dnd_hover_payload::<NomenclatureGlissee>().is_some();
            if let Some(g) = depot_vue.then(|| resp.dnd_release_payload::<VueGlissee>()).flatten() {
                match ui.input(|i| i.pointer.hover_pos()).and_then(papier) {
                    Some((k, pt)) => {
                        let echelle = p.vues.iter().find(|v| v.id == g.0).map_or(100.0, |v| v.echelle);
                        let zero = doc::VuePlacee { vue: g.0.clone(), position: [0.0, 0.0], echelle };
                        let r = etat.vues.entry((g.0.clone(), echelle.to_bits())).or_insert_with(|| match doc::vue_placee(p, &zero) {
                            Ok((prims, e)) => rendu(&prims, e),
                            Err(e) => Rendu { erreur: Some(e), ..Default::default() },
                        });
                        if let Some(e) = &r.erreur {
                            self.error = Some(e.clone());
                        } else {
                            let c = [(r.etendue[0] + r.etendue[2]) * 0.5, (r.etendue[1] + r.etendue[3]) * 0.5];
                            let mut f = feuilles[k].clone();
                            f.vues.push(doc::VuePlacee { vue: g.0.clone(), position: [pt[0] - c[0], pt[1] - c[1]], echelle });
                            etat.selection = SelectionFeuille::Vue(f.id.clone(), f.vues.len() - 1);
                            commandes.push(Commande::Feuille { feuille: f });
                        }
                    }
                    None => self.status = "Déposer la vue sur une feuille.".into(),
                }
            }
            // nomenclature déposée : tableau posé, coin haut gauche sous le curseur
            if let Some(g) = depot_table.then(|| resp.dnd_release_payload::<NomenclatureGlissee>()).flatten() {
                match ui.input(|i| i.pointer.hover_pos()).and_then(papier) {
                    Some((k, pt)) => {
                        let mut f = feuilles[k].clone();
                        f.nomenclatures.push(doc::NomenclaturePlacee { nomenclature: g.0.clone(), position: pt });
                        etat.selection = SelectionFeuille::Vue(f.id.clone(), f.vues.len() + f.nomenclatures.len() - 1);
                        commandes.push(Commande::Feuille { feuille: f });
                    }
                    None => self.status = "Déposer la nomenclature sur une feuille.".into(),
                }
            }
            // tracé : pages, cadres, vues, sélection
            let mut mesh = egui::Mesh::default();
            let mut traits: Vec<egui::Shape> = Vec::new();
            let ajouter = |r: &Rendu, k: usize, dx: f64, dy: f64, decal: egui::Vec2, mesh: &mut egui::Mesh, traits: &mut Vec<egui::Shape>| {
                for (t, c) in &r.tris {
                    let i = mesh.vertices.len() as u32;
                    for q in t {
                        mesh.colored_vertex(ecran(k, q.x as f64 + dx, q.y as f64 + dy) + decal, *c);
                    }
                    mesh.add_triangle(i, i + 1, i + 2);
                }
                for (t, c, e) in &r.traits {
                    traits.push(egui::Shape::line_segment(t.map(|q| ecran(k, q.x as f64 + dx, q.y as f64 + dy) + decal), egui::Stroke::new((e * z).max(1.0), *c)));
                }
            };
            let bleu = Color32::from_rgb(0, 120, 215);
            for (k, f) in feuilles.iter().enumerate() {
                let r = page(k);
                if !r.intersects(rect) {
                    continue;
                }
                painter.rect_filled(r.translate(egui::vec2(4.0, 4.0)), 0.0, Color32::from_black_alpha(60));
                painter.rect_filled(r, 0.0, Color32::WHITE);
                if survol_depot == Some(k) {
                    painter.rect_stroke(r, 0.0, egui::Stroke::new(3.0, bleu), egui::StrokeKind::Outside);
                }
                if let Some(c) = etat.cadres.get(&f.id) {
                    ajouter(c, k, 0.0, 0.0, egui::Vec2::ZERO, &mut mesh, &mut traits);
                }
                for (j, v) in f.vues.iter().enumerate() {
                    let decal = match &etat.glisse {
                        Some((fid, jj, d)) if *fid == f.id && *jj == j => *d,
                        _ => egui::Vec2::ZERO,
                    };
                    if let Some(rv) = etat.vues.get(&(v.vue.clone(), v.echelle.to_bits())) {
                        if let Some(e) = &rv.erreur {
                            painter.text(ecran(k, v.position[0], v.position[1]), egui::Align2::LEFT_BOTTOM, e, egui::FontId::proportional(12.0), Color32::DARK_RED);
                        }
                        ajouter(rv, k, v.position[0], v.position[1], decal, &mut mesh, &mut traits);
                    }
                }
                // nomenclatures posées
                for (i, n) in f.nomenclatures.iter().enumerate() {
                    let j = f.vues.len() + i;
                    let decal = match &etat.glisse {
                        Some((fid, jj, d)) if *fid == f.id && *jj == j => *d,
                        _ => egui::Vec2::ZERO,
                    };
                    if let Some(rt) = etat.tables.get(&n.nomenclature) {
                        if let Some(e) = &rt.erreur {
                            painter.text(ecran(k, n.position[0], n.position[1]), egui::Align2::LEFT_TOP, e, egui::FontId::proportional(12.0), Color32::DARK_RED);
                        }
                        ajouter(rt, k, n.position[0], n.position[1], decal, &mut mesh, &mut traits);
                    }
                }
                // objet choisi : cadre en tirets
                for j in 0..f.vues.len() + f.nomenclatures.len() {
                    if etat.selection != SelectionFeuille::Vue(f.id.clone(), j) {
                        continue;
                    }
                    let decal = match &etat.glisse {
                        Some((fid, jj, d)) if *fid == f.id && *jj == j => *d,
                        _ => egui::Vec2::ZERO,
                    };
                    if let Some(b) = boite(f, j) {
                        let cadre = egui::Rect::from_two_pos(ecran(k, b[0], b[1]), ecran(k, b[2], b[3])).translate(decal);
                        painter.add(egui::Shape::dashed_line(&[cadre.left_top(), cadre.right_top(), cadre.right_bottom(), cadre.left_bottom(), cadre.left_top()], egui::Stroke::new(1.5, bleu), 6.0, 4.0));
                    }
                }
                let choisie = matches!(&etat.selection, SelectionFeuille::Feuille(id) if *id == f.id);
                // titre : poignée (trois traits) et numéro — nom ; glisser pour déplacer
                let survol_titre = resp.hover_pos().and_then(titre_sous) == Some(k) || etat.glisse_feuille.as_deref() == Some(f.id.as_str());
                let couleur_titre = if choisie || survol_titre { bleu } else { Color32::from_gray(40) };
                let base = r.left_top() - egui::vec2(0.0, 10.0);
                for d in [-3.0, 0.0, 3.0] {
                    painter.line_segment([base + egui::vec2(0.0, d), base + egui::vec2(10.0, d)], egui::Stroke::new(1.4, couleur_titre));
                }
                painter.text(base + egui::vec2(16.0, 0.0), egui::Align2::LEFT_CENTER, format!("{} — {}", f.numero, f.nom), egui::FontId::proportional(13.0), couleur_titre);
                if choisie {
                    painter.rect_stroke(r, 0.0, egui::Stroke::new(2.0, bleu), egui::StrokeKind::Outside);
                }
            }
            painter.add(egui::Shape::mesh(mesh));
            painter.extend(traits);
            // feuille glissée : barre d'insertion entre deux feuilles
            if etat.glisse_feuille.is_some() {
                if let Some(q) = ui.input(|i| i.pointer.latest_pos()) {
                    let t = rang_sous(q);
                    let y = if t < feuilles.len() { ys[t] - ECART * 0.9 } else { y_ajout - ECART * 0.9 };
                    let a = o + egui::vec2(-10.0 / z, y) * z;
                    painter.line_segment([a, a + egui::vec2((largeur_max + 20.0 / z) * z, 0.0)], egui::Stroke::new(4.0, bleu));
                    painter.circle_filled(a, 5.0, bleu);
                }
            }
            // page « ajouter une feuille »
            let survol_ajout = resp.hover_pos().is_some_and(|q| page_ajout.contains(q));
            let gris = if survol_ajout { bleu } else { Color32::from_gray(120) };
            let r = page_ajout;
            painter.add(egui::Shape::dashed_line(&[r.left_top(), r.right_top(), r.right_bottom(), r.left_bottom(), r.left_top()], egui::Stroke::new(1.5, gris), 8.0, 6.0));
            painter.text(r.center(), egui::Align2::CENTER_CENTER, "+ Ajouter une feuille", egui::FontId::proportional(16.0), gris);
            if feuilles.is_empty() {
                painter.text(rect.center_top() + egui::vec2(0.0, 14.0), egui::Align2::CENTER_TOP, "Jeu vide : cliquer la page en pointillés", egui::FontId::proportional(13.0), Color32::from_gray(60));
            } else {
                let aide = painter.layout_no_wrap("Glisser une vue ou une nomenclature de l'arborescence sur une feuille · glisser le titre d'une feuille : la déplacer · molette : zoom · glisser le fond : défiler · double-clic sur une vue : l'ouvrir · Suppr : retirer".into(), egui::FontId::proportional(12.0), Color32::from_gray(40));
                let cadre = egui::Rect::from_min_size(rect.left_bottom() + egui::vec2(8.0, -aide.size().y - 14.0), aide.size() + egui::vec2(12.0, 6.0));
                painter.rect_filled(cadre, 4.0, Color32::from_rgba_unmultiplied(245, 245, 245, 230));
                painter.galley(cadre.min + egui::vec2(6.0, 3.0), aide, Color32::from_gray(40));
            }
            let curseur = if etat.glisse.is_some() || etat.glisse_feuille.is_some() {
                Some(egui::CursorIcon::Grabbing)
            } else if survol_ajout {
                Some(egui::CursorIcon::PointingHand)
            } else if resp.hover_pos().and_then(vue_sous).is_some() || resp.hover_pos().and_then(titre_sous).is_some() {
                Some(egui::CursorIcon::Grab)
            } else {
                None
            };
            if let Some(c) = curseur {
                ui.ctx().set_cursor_icon(c);
            }
        } else {
            ui.centered_and_justified(|ui| ui.label("Les feuilles demandent un projet natif."));
        }
        self.feuilles = etat;
        if let Some((id, avant)) = deplacement {
            self.deplacer_feuille(&id, &jeu, avant.as_deref());
        }
        for c in commandes {
            if let Err(e) = self.executer_bim(c) {
                self.error = Some(e);
            }
        }
        if let Some(id) = ouvrir_vue.as_deref().and_then(|x| x.strip_prefix("nomenclature:")) {
            self.ouvrir_nomenclature(id);
        } else if let Some(id) = ouvrir_vue {
            let n = crate::scene::numero_vue(&id);
            if let Some(i) = self.scene.as_ref().and_then(|s| s.navigateur.as_ref()).and_then(|nav| nav.vues.iter().position(|v| v.revit_id == n)) {
                self.activer_vue(i);
            }
        }
    }

    /// Propriétés du jeu ouvert : jeu (renommer, nouveau), feuille choisie (numéro, nom,
    /// format, cartouche, export, suppression) ou vue placée (échelle, position).
    pub(super) fn proprietes_jeu(&mut self, ui: &mut egui::Ui, jeu: &str) {
        let Some((p, _)) = &self.natif else { return };
        let mut commande: Option<Commande> = None;
        let mut projet_modifie: Option<crate::modele::Projet> = None;
        let mut export: Option<(&str, String)> = None;
        let mut ouvrir: Option<String> = None;
        ui.heading("Jeu de feuilles");
        // nom du jeu : tampon propre, appliqué à Entrée ou en quittant le champ (le jeu
        // ouvert est retrouvé par son nom)
        let cle = egui::Id::unique(("nom_jeu", jeu));
        let mut nom_jeu = ui.data(|d| d.get_temp::<String>(cle)).unwrap_or_else(|| jeu.to_string());
        let r = ui.horizontal(|ui| {
            ui.label("Nom");
            ui.text_edit_singleline(&mut nom_jeu)
        }).inner;
        if r.has_focus() {
            ui.data_mut(|d| d.insert_temp(cle, nom_jeu.clone()));
        } else {
            ui.data_mut(|d| d.remove::<String>(cle));
        }
        if r.lost_focus() && !self.echap_saisie && nom_jeu.trim() != jeu && !nom_jeu.trim().is_empty() {
            let nom_jeu = nom_jeu.trim().to_string();
            let mut q = p.clone();
            for f in q.feuilles.iter_mut().filter(|f| f.jeu == jeu) {
                f.jeu = nom_jeu.clone();
            }
            projet_modifie = Some(q);
        }
        let n = p.feuilles.iter().filter(|f| f.jeu == jeu).count();
        ui.label(RichText::new(format!("{n} feuille(s)")).weak());
        ui.horizontal(|ui| {
            if ui.button("Ajouter une feuille").clicked() {
                commande = Some(Commande::Feuille { feuille: Self::nouvelle_feuille(p, jeu) });
            }
            if ui.button("Nouveau jeu").on_hover_text("Nouveau jeu de feuilles, avec une première feuille").clicked() {
                let existants = doc::jeux(p);
                let nom = (2..).map(|k| format!("Jeu {k}")).find(|n| !existants.contains(n)).unwrap_or_default();
                commande = Some(Commande::Feuille { feuille: Self::nouvelle_feuille(p, &nom) });
                ouvrir = Some(nom);
            }
        });
        if ui.add_enabled(n > 0, egui::Button::new("Exporter le jeu en PDF…")).on_hover_text("Un PDF, une page par feuille dans l'ordre du jeu, chacune à son format").clicked() {
            export = Some(("jeu", String::new()));
        }
        ui.separator();
        let selection = self.feuilles.selection.clone();
        let fid = match &selection {
            SelectionFeuille::Feuille(id) | SelectionFeuille::Vue(id, _) => Some(id.clone()),
            SelectionFeuille::Aucune => None,
        };
        let feuille = fid.and_then(|id| p.feuilles.iter().find(|f| f.id == id && f.jeu == jeu));
        match (feuille, &selection) {
            (Some(f), SelectionFeuille::Vue(_, j)) if *j >= f.vues.len() && *j < f.vues.len() + f.nomenclatures.len() => {
                let mut g = f.clone();
                let k = *j - f.vues.len();
                let n = &mut g.nomenclatures[k];
                let id_nomenclature = n.nomenclature.clone();
                let nom = p.nomenclatures.iter().find(|x| x.id == n.nomenclature).map_or("Nomenclature absente".to_string(), |x| x.nom.clone());
                ui.heading("Nomenclature posée");
                ui.label(RichText::new(&nom).strong());
                egui::Grid::new("table_posee").num_columns(2).spacing([10.0, 4.0]).show(ui, |ui| {
                    ui.label("Position X");
                    ui.add(egui::DragValue::new(&mut n.position[0]).speed(1.0).max_decimals(1).suffix(" mm"));
                    ui.end_row();
                    ui.label("Position Y");
                    ui.add(egui::DragValue::new(&mut n.position[1]).speed(1.0).max_decimals(1).suffix(" mm"));
                    ui.end_row();
                });
                ui.label(RichText::new("Coin haut gauche du tableau ; colonnes et regroupement : ouvrir la nomenclature").small().weak());
                ui.horizontal(|ui| {
                    if ui.button("Ouvrir la nomenclature").clicked() {
                        ouvrir = Some(format!("nomenclature:{id_nomenclature}"));
                    }
                    if ui.button("Retirer de la feuille").on_hover_text("Suppr").clicked() {
                        g.nomenclatures.remove(k);
                        self.feuilles.selection = SelectionFeuille::Feuille(g.id.clone());
                    }
                });
                if serde_json::to_string(&g).ok() != serde_json::to_string(f).ok() {
                    commande = Some(Commande::Feuille { feuille: g });
                }
            }
            (Some(f), SelectionFeuille::Vue(_, j)) if *j < f.vues.len() => {
                let mut g = f.clone();
                let v = &mut g.vues[*j];
                let nom = p.vues.iter().find(|x| x.id == v.vue).map_or("Vue absente".to_string(), |x| x.nom.clone());
                ui.heading("Vue placée");
                ui.label(RichText::new(&nom).strong());
                egui::Grid::new("vue_placee").num_columns(2).spacing([10.0, 4.0]).show(ui, |ui| {
                    ui.label("Échelle");
                    ui.add(egui::DragValue::new(&mut v.echelle).speed(1.0).range(1.0..=10_000.0).prefix("1 : "));
                    ui.end_row();
                    ui.label("Position X");
                    ui.add(egui::DragValue::new(&mut v.position[0]).speed(1.0).max_decimals(1).suffix(" mm"));
                    ui.end_row();
                    ui.label("Position Y");
                    ui.add(egui::DragValue::new(&mut v.position[1]).speed(1.0).max_decimals(1).suffix(" mm"));
                    ui.end_row();
                });
                let vue = v.vue.clone();
                ui.horizontal(|ui| {
                    if ui.button("Ouvrir la vue").clicked() {
                        ouvrir = Some(format!("vue:{vue}"));
                    }
                    if ui.button("Retirer de la feuille").on_hover_text("Suppr").clicked() {
                        g.vues.remove(*j);
                        self.feuilles.selection = SelectionFeuille::Feuille(g.id.clone());
                    }
                });
                if serde_json::to_string(&g).ok() != serde_json::to_string(f).ok() {
                    commande = Some(Commande::Feuille { feuille: g });
                }
            }
            (Some(f), _) => {
                let mut g = f.clone();
                ui.heading("Feuille");
                egui::Grid::new("feuille_proprietes").num_columns(2).spacing([10.0, 4.0]).show(ui, |ui| {
                    ui.label("Numéro");
                    ui.text_edit_singleline(&mut g.numero);
                    ui.end_row();
                    ui.label("Nom");
                    ui.text_edit_singleline(&mut g.nom);
                    ui.end_row();
                    ui.label("Format");
                    let nom_format = doc::FORMATS.iter().find(|(_, l, h)| (g.largeur.max(g.hauteur) - l).abs() < 0.5 && (g.largeur.min(g.hauteur) - h).abs() < 0.5).map_or("Personnalisé", |x| x.0);
                    let portrait = g.hauteur > g.largeur;
                    ui.horizontal(|ui| {
                        egui::ComboBox::from_id_salt("format_feuille").selected_text(nom_format).width(70.0).show_ui(ui, |ui| {
                            for (n, l, h) in doc::FORMATS {
                                if ui.selectable_label(n == nom_format, n).clicked() {
                                    (g.largeur, g.hauteur) = if portrait { (h, l) } else { (l, h) };
                                }
                            }
                        });
                        if ui.selectable_label(!portrait, "Paysage").clicked() && portrait {
                            std::mem::swap(&mut g.largeur, &mut g.hauteur);
                        }
                        if ui.selectable_label(portrait, "Portrait").clicked() && !portrait {
                            std::mem::swap(&mut g.largeur, &mut g.hauteur);
                        }
                    });
                    ui.end_row();
                    ui.label("Dimensions");
                    ui.horizontal(|ui| {
                        ui.add(egui::DragValue::new(&mut g.largeur).speed(1.0).range(50.0..=5000.0).suffix(" mm"));
                        ui.add(egui::DragValue::new(&mut g.hauteur).speed(1.0).range(50.0..=5000.0).suffix(" mm"));
                    });
                    ui.end_row();
                    ui.label("Cartouche");
                    ui.text_edit_singleline(&mut g.cartouche);
                    ui.end_row();
                });
                ui.label(RichText::new(format!("{} vue(s) placée(s), {} nomenclature(s) · glisser une vue ou une nomenclature de l'arborescence sur la feuille", f.vues.len(), f.nomenclatures.len())).small().weak());
                ui.horizontal(|ui| {
                    if ui.button("Exporter PDF…").clicked() {
                        export = Some(("pdf", f.id.clone()));
                    }
                    if ui.button("Exporter SVG…").clicked() {
                        export = Some(("svg", f.id.clone()));
                    }
                });
                if ui.button(RichText::new("Supprimer la feuille").color(Color32::from_rgb(180, 45, 40))).on_hover_text("Suppr").clicked() {
                    commande = Some(Commande::Supprimer { ids: vec![f.id.clone()] });
                    self.feuilles.selection = SelectionFeuille::Aucune;
                }
                if commande.is_none() && serde_json::to_string(&g).ok() != serde_json::to_string(f).ok() {
                    commande = Some(Commande::Feuille { feuille: g });
                }
            }
            _ => {
                ui.label(RichText::new("Cliquer une feuille ou une vue placée pour la régler.").weak());
            }
        }
        if let Some(q) = projet_modifie {
            if let Err(e) = self.appliquer_projet(q) {
                self.error = Some(e);
            }
        }
        if let Some(c) = commande {
            if let Err(e) = self.executer_bim(c) {
                self.error = Some(e);
            }
        }
        if let Some(id) = ouvrir.as_deref().and_then(|o| o.strip_prefix("nomenclature:")) {
            self.ouvrir_nomenclature(id);
        } else if let Some(o) = ouvrir {
            match o.strip_prefix("vue:") {
                Some(id) => {
                    let n = crate::scene::numero_vue(id);
                    if let Some(i) = self.scene.as_ref().and_then(|s| s.navigateur.as_ref()).and_then(|nav| nav.vues.iter().position(|v| v.revit_id == n)) {
                        self.activer_vue(i);
                    }
                }
                None => self.ouvrir_jeu(&o, None),
            }
        }
        match export {
            Some(("jeu", _)) => self.exporter_jeu_dialogue(jeu),
            Some((extension, id)) => self.exporter_feuille(extension, &id),
            None => {}
        }
    }

    /// Demande le fichier, puis exporte le jeu `jeu` en un PDF de plusieurs pages.
    pub(super) fn exporter_jeu_dialogue(&mut self, jeu: &str) {
        let Some(path) = rfd::FileDialog::new().set_file_name(format!("{jeu}.pdf")).add_filter("pdf", &["pdf"]).save_file() else {
            return;
        };
        match self.exporter_jeu(jeu, &path) {
            Ok(n) => self.status = format!("Jeu « {jeu} » : {n} page(s) exportée(s) dans {}", path.display()),
            Err(e) => self.error = Some(e),
        }
    }

    /// PDF du jeu `jeu` (une page par feuille) écrit dans `chemin` ; nombre de pages.
    pub(super) fn exporter_jeu(&self, jeu: &str, chemin: &std::path::Path) -> Result<usize, String> {
        let (p, _) = self.natif.as_ref().ok_or("Aucun projet natif ouvert.")?;
        let n = p.feuilles.iter().filter(|f| f.jeu == jeu).count();
        if n == 0 {
            return Err(format!("Le jeu « {jeu} » n'a aucune feuille."));
        }
        let pdf = doc::pdf_jeu(p, jeu)?;
        crate::modele::enregistrement::ecrire_atomique(chemin, &pdf)?;
        Ok(n)
    }

    fn exporter_feuille(&mut self, extension: &str, id: &str) {
        let Some((p, _)) = &self.natif else { return };
        let Some(f) = p.feuilles.iter().find(|f| f.id == id) else { return };
        let Some(path) = rfd::FileDialog::new().set_file_name(format!("{} {}.{extension}", f.numero, f.nom)).add_filter(extension, &[extension]).save_file() else {
            return;
        };
        let contenu = doc::svg_feuille(p, f).and_then(|s| if extension == "pdf" { doc::pdf(&s) } else { Ok(s.into_bytes()) });
        match contenu.and_then(|b| crate::modele::enregistrement::ecrire_atomique(&path, &b)) {
            Ok(()) => self.status = format!("Export : {}", path.display()),
            Err(e) => self.error = Some(e),
        }
    }

    /// Ouvre la fenêtre des nomenclatures sur la nomenclature `id`.
    pub(super) fn ouvrir_nomenclature(&mut self, id: &str) {
        if let Some(k) = self.natif.as_ref().and_then(|(p, _)| p.nomenclatures.iter().position(|n| n.id == id)) {
            self.bim.nomenclature = k;
            self.bim.documentation = true;
        }
    }

    /// Remet la feuille `id` dans le jeu `jeu`, avant la feuille `avant` (à la fin sans).
    pub(super) fn deplacer_feuille(&mut self, id: &str, jeu: &str, avant: Option<&str>) {
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

    /// Suppr dans un jeu de feuilles : retire la vue placée choisie, ou supprime la feuille.
    /// Faux si rien n'est choisi dans le jeu.
    pub(super) fn supprimer_dans_jeu(&mut self) -> bool {
        let Some(jeu) = self.jeu_actif() else { return false };
        let Some((p, _)) = &self.natif else { return false };
        let commande = match self.feuilles.selection.clone() {
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
