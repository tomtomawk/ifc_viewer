//! Vue du jeu ouvert : feuilles l'une sous l'autre. Molette : zoom ; glisser le fond :
//! défiler ; glisser une vue placée, une nomenclature, le cartouche ou une annotation :
//! le déplacer (aimanté) ; déposer une vue de l'arborescence : la placer ; outils
//! d'annotation (barre en haut à gauche) ; dernière page en pointillés : ajouter une
//! feuille.
use crate::*;
use crate::documenter::annotations_feuille::{self as annot, OutilFeuille};
use crate::documenter::feuilles::{rendu, table_zero, NomenclatureGlissee, ObjetFeuille, Rendu, SelectionFeuille, VueGlissee, ECART, HAUT_FEUILLES};
use commandes::Commande;
use modele::documentation as doc;
use std::collections::HashMap;

/// Commande du texte saisi en place : annotation ajoutée, modifiée, ou retirée si vide.
fn commande_texte(p: &modele::Projet, t: annot::TexteFeuille) -> Option<Commande> {
    let mut f = p.feuilles.iter().find(|f| f.id == t.feuille)?.clone();
    let contenu = t.contenu.trim_end().to_string();
    match t.rang {
        Some(i) if i < f.annotations.len() => {
            if contenu.is_empty() {
                f.annotations.remove(i);
            } else if let doc::AnnotationForme::Texte { texte, .. } = &mut f.annotations[i].forme {
                if *texte == contenu { return None; }
                *texte = contenu;
            }
        }
        Some(_) => return None,
        None if contenu.is_empty() => return None,
        None => f.annotations.push(doc::AnnotationFeuille { hauteur: t.hauteur, forme: doc::AnnotationForme::Texte { point: t.point, texte: contenu } }),
    }
    Some(Commande::Feuille { feuille: f })
}

/// Action demandée par le menu contextuel, faite après le tracé.
enum ActionMenu {
    Supprimer,
    ModifierCartouche(String),
    CartoucheDefaut(String),
}

impl Application {
    pub(crate) fn vue_jeu(&mut self, ui: &mut egui::Ui) {
        let Some(jeu) = self.jeu_actif() else { return };
        let mut etat = std::mem::take(&mut self.feuilles);
        let mut commandes: Vec<Commande> = Vec::new();
        // feuille à déplacer : (feuille, feuille devant laquelle la mettre ; None : à la fin)
        let mut deplacement: Option<(String, Option<String>)> = None;
        let mut ouvrir_vue: Option<String> = None;
        let mut action: Option<ActionMenu> = None;
        if let Some((p, _)) = &self.natif {
            // dessins des vues recalculés seulement si le modèle (hors feuilles) change
            if etat.empreinte.map(|(doc, rev, _)| (doc, rev)) != Some((self.document_actif, self.bim.revision)) {
                use std::hash::{Hash, Hasher};
                let mut q = p.clone();
                q.feuilles.clear();
                q.positions_plan.clear();
                let mut h = std::collections::hash_map::DefaultHasher::new();
                q.json().unwrap_or_default().hash(&mut h);
                let h = h.finish();
                if etat.empreinte.is_some_and(|(doc, _, ancien)| doc != self.document_actif || ancien != h) {
                    etat.vues.clear();
                }
                etat.cadres.clear();
                etat.annotations.clear();
                etat.tables.clear();
                etat.empreinte = Some((self.document_actif, self.bim.revision, h));
            }
            let feuilles: Vec<&doc::Feuille> = p.feuilles.iter().filter(|f| f.jeu == jeu).collect();
            let (rect, resp) = ui.allocate_exact_size(ui.available_size(), Sense::click_and_drag());
            if rect.is_positive() { self.rect_vue = rect; }
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
                etat.decalage = egui::vec2((rect.width() - largeur_max * etat.zoom) * 0.5, HAUT_FEUILLES);
                etat.cadre_pour = Some(jeu.clone());
            }
            // feuille demandée : cadrée entière en haut de la vue
            if let Some(id) = etat.defiler_vers.take()
                && let Some(k) = feuilles.iter().position(|f| f.id == id) {
                    let (l, h) = (feuilles[k].largeur as f32, feuilles[k].hauteur as f32);
                    etat.zoom = (rect.width() * 0.85 / l).min((rect.height() - HAUT_FEUILLES - 34.0) / h).max(0.05);
                    etat.decalage = egui::vec2((rect.width() - l * etat.zoom) * 0.5, HAUT_FEUILLES - ys[k] * etat.zoom);
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
            // point écran dans le repère de la feuille k, même hors du papier
            let sur_feuille = |k: usize, q: egui::Pos2| -> [f64; 2] {
                let w = (q - o) / z;
                [w.x as f64, (ys[k] + feuilles[k].hauteur as f32 - w.y) as f64]
            };
            let papier = |q: egui::Pos2| -> Option<(usize, [f64; 2])> {
                let w = (q - o) / z;
                (0..feuilles.len()).find(|&k| w.x >= 0.0 && w.x <= feuilles[k].largeur as f32 && w.y >= ys[k] && w.y <= ys[k] + feuilles[k].hauteur as f32).map(|k| (k, sur_feuille(k, q)))
            };
            let page = |k: usize| egui::Rect::from_min_size(o + egui::vec2(0.0, ys[k]) * z, egui::vec2(feuilles[k].largeur as f32, feuilles[k].hauteur as f32) * z);
            let page_ajout = egui::Rect::from_min_size(o + egui::vec2(0.0, y_ajout) * z, egui::vec2(l_ajout, h_ajout) * z);
            etat.origines = feuilles.iter().enumerate().map(|(k, f)| (f.id.clone(), ecran(k, 0.0, 0.0))).collect();
            // dessins des vues placées (position nulle), cartouches, annotations
            let cle_vue = |v: &doc::VuePlacee| (v.vue.clone(), v.echelle_sur_feuille(p).to_bits());
            for f in &feuilles {
                for v in &f.vues {
                    etat.vues.entry(cle_vue(v)).or_insert_with(|| {
                        let zero = doc::VuePlacee { position: [0.0, 0.0], ..v.clone() };
                        match moteur2d::vue_placee(p, &zero) {
                            Ok((prims, e)) => rendu(&prims, e),
                            Err(e) => Rendu { erreur: Some(e), ..Default::default() },
                        }
                    });
                }
                etat.cadres.entry(f.id.clone()).or_insert_with(|| rendu(&doc::cartouche_feuille(p, f), [0.0; 4]));
                etat.annotations.entry(f.id.clone()).or_insert_with(|| f.annotations.iter().map(|a| rendu(&doc::primitives_annotation_feuille(p, f, a), [0.0; 4])).collect());
                for n in &f.nomenclatures {
                    etat.tables.entry(n.nomenclature.clone()).or_insert_with(|| table_zero(p, &n.nomenclature));
                }
            }
            let etendues: HashMap<(String, u64), [f64; 4]> = etat.vues.iter().map(|(c, r)| (c.clone(), r.etendue)).collect();
            let etendues_tables: HashMap<String, [f64; 4]> = etat.tables.iter().map(|(c, r)| (c.clone(), r.etendue)).collect();
            // cadre d'un objet sur sa feuille (mm du papier), titre des vues compris
            let boite = |f: &doc::Feuille, objet: ObjetFeuille| -> Option<[f64; 4]> {
                match objet {
                    ObjetFeuille::Place(j) => {
                        if let Some(v) = f.vues.get(j) {
                            let e = *etendues.get(&cle_vue(v))?;
                            return Some([e[0] + v.position[0] - 2.0, e[1] + v.position[1] - 16.0, e[2] + v.position[0] + 2.0, e[3] + v.position[1] + 2.0]);
                        }
                        let n = f.nomenclatures.get(j - f.vues.len())?;
                        let e = *etendues_tables.get(&n.nomenclature)?;
                        Some([e[0] + n.position[0] - 2.0, e[1] + n.position[1] - 2.0, e[2] + n.position[0] + 2.0, e[3] + n.position[1] + 2.0])
                    }
                    ObjetFeuille::Cartouche => doc::cadre_cartouche(p, f),
                    ObjetFeuille::Annotation(i) => f.annotations.get(i)?.etendue(f, p).map(|e| [e[0] - 1.0, e[1] - 1.0, e[2] + 1.0, e[3] + 1.0]),
                }
            };
            let dans = |b: [f64; 4], pt: [f64; 2]| pt[0] >= b[0] && pt[0] <= b[2] && pt[1] >= b[1] && pt[1] <= b[3];
            // objet sous le curseur : annotations (dessus), cartouche, puis vues et tableaux
            let objet_sous = |q: egui::Pos2| -> Option<(usize, ObjetFeuille)> {
                let (k, pt) = papier(q)?;
                let f = feuilles[k];
                let tol = (6.0 / z) as f64;
                if let Some(i) = (0..f.annotations.len()).rev().find(|&i| f.annotations[i].distance(f, p, pt) <= tol) {
                    return Some((k, ObjetFeuille::Annotation(i)));
                }
                if boite(f, ObjetFeuille::Cartouche).is_some_and(|b| dans(b, pt)) {
                    return Some((k, ObjetFeuille::Cartouche));
                }
                (0..f.vues.len() + f.nomenclatures.len()).rev().find(|&j| boite(f, ObjetFeuille::Place(j)).is_some_and(|b| dans(b, pt))).map(|j| (k, ObjetFeuille::Place(j)))
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
            let selection_sous = |q: egui::Pos2| {
                if let Some((k, objet)) = objet_sous(q) {
                    SelectionFeuille::de_objet(&feuilles[k].id, objet)
                } else if let Some(k) = papier(q).map(|(k, _)| k).or_else(|| titre_sous(q)) {
                    SelectionFeuille::Feuille(feuilles[k].id.clone())
                } else {
                    SelectionFeuille::Aucune
                }
            };
            let outil = etat.trace.outil;
            let en_trace = outil != OutilFeuille::Selection;
            // texte saisi en place : validé quand le champ perd la main, abandonné à Échap
            let mut texte_valide = false;
            if let Some(t) = etat.trace.texte.as_mut()
                && let Some(k) = feuilles.iter().position(|f| f.id == t.feuille) {
                    let e = ecran(k, t.point[0], t.point[1]);
                    match annot::champ_texte(ui, rect, e, (t.hauteur as f32 * z).max(1.0), t) {
                        Some(true) => {
                            texte_valide = true;
                            if let Some(c) = etat.trace.texte.take().and_then(|t| commande_texte(p, t)) { commandes.push(c); }
                        }
                        Some(false) => etat.trace.texte = None,
                        None => {}
                    }
                }
            // point visé par l'outil : feuille du tracé en cours, sinon celle sous le curseur
            let k_trace = etat.trace.feuille.as_ref().and_then(|id| feuilles.iter().position(|f| &f.id == id)).filter(|_| !etat.trace.points.is_empty());
            let maj = ui.input(|i| i.modifiers.shift);
            let viser = |q: egui::Pos2| -> Option<(usize, [f64; 2])> {
                let k = k_trace.or_else(|| papier(q).map(|(k, _)| k))?;
                Some((k, etat.trace.viser(sur_feuille(k, q), &annot::sommets_feuille(p, feuilles[k]), (8.0 / z) as f64, maj)))
            };
            let vise = resp.hover_pos().filter(|_| en_trace).and_then(viser);
            if resp.secondary_clicked() && let Some(q) = resp.interact_pointer_pos() {
                let choix = selection_sous(q);
                if choix != SelectionFeuille::Aucune {
                    self.famille_selectionnee = None;
                    etat.selection = choix;
                }
            }
            let choix_menu = etat.selection.clone();
            resp.context_menu(|ui| {
                if let SelectionFeuille::Cartouche(id) = &choix_menu {
                    if ui.button("Modifier la famille du cartouche…").clicked() {
                        action = Some(ActionMenu::ModifierCartouche(id.clone()));
                        ui.close();
                    }
                    if ui.button("Remettre à la place par défaut").clicked() {
                        action = Some(ActionMenu::CartoucheDefaut(id.clone()));
                        ui.close();
                    }
                }
                let libelle = match &choix_menu {
                    SelectionFeuille::Cartouche(_) => Some("Supprimer le cartouche"),
                    SelectionFeuille::Annotation(..) => Some("Supprimer l'annotation"),
                    SelectionFeuille::Vue(..) => Some("Retirer de la feuille"),
                    SelectionFeuille::Feuille(_) => Some("Supprimer la feuille"),
                    SelectionFeuille::Aucune => None,
                };
                match libelle {
                    Some(l) => if ui.button(l).clicked() {
                        action = Some(ActionMenu::Supprimer);
                        ui.close();
                    },
                    None => { ui.label(RichText::new("Rien de sélectionné").weak()); }
                }
            });
            // souris : déplacement d'un objet ou d'une feuille, défilement
            if resp.drag_started_by(egui::PointerButton::Primary) {
                let depart = ui.input(|i| i.pointer.press_origin());
                if let Some((k, objet)) = depart.filter(|_| !en_trace).and_then(objet_sous) {
                    self.famille_selectionnee = None;
                    etat.glisse = Some((feuilles[k].id.clone(), objet, egui::Vec2::ZERO));
                    etat.glisse_brut = egui::Vec2::ZERO;
                    etat.selection = SelectionFeuille::de_objet(&feuilles[k].id, objet);
                } else if let Some(k) = depart.filter(|_| !en_trace).and_then(titre_sous) {
                    self.famille_selectionnee = None;
                    etat.glisse_feuille = Some(feuilles[k].id.clone());
                    etat.selection = SelectionFeuille::Feuille(feuilles[k].id.clone());
                }
            }
            if resp.dragged() {
                match &mut etat.glisse {
                    Some((fid, objet, d)) => {
                        // bords et angles du papier, des marges, du cartouche et des autres objets ;
                        // déplacement mesuré depuis l'appui (le seuil de glisser n'est pas perdu)
                        etat.glisse_brut = match ui.input(|i| (i.pointer.press_origin(), i.pointer.latest_pos())) {
                            (Some(a), Some(b)) => b - a,
                            _ => etat.glisse_brut + resp.drag_delta(),
                        };
                        *d = etat.glisse_brut;
                        if let Some(f) = feuilles.iter().find(|f| &f.id == fid) && let Some(b) = boite(f, *objet) {
                            let (dx, dy) = ((d.x / z) as f64, -(d.y / z) as f64);
                            let (mut xs, mut ys) = crate::documenter::aimantation::bords_feuille(f.largeur, f.hauteur, doc::MARGE);
                            let autres = (0..f.vues.len() + f.nomenclatures.len()).map(ObjetFeuille::Place).chain([ObjetFeuille::Cartouche]);
                            for autre in autres.filter(|a| a != objet).filter_map(|a| boite(f, a)) {
                                xs.extend([autre[0], autre[2]]);
                                ys.extend([autre[1], autre[3]]);
                            }
                            let [cx, cy] = crate::documenter::aimantation::aimanter([b[0] + dx, b[1] + dy, b[2] + dx, b[3] + dy], &xs, &ys, (8.0 / z) as f64);
                            *d += egui::vec2((cx * z as f64) as f32, -(cy * z as f64) as f32);
                        }
                    }
                    None if etat.glisse_feuille.is_some() => {}
                    None => etat.decalage += resp.drag_delta(),
                }
            }
            if resp.drag_stopped() {
                if let (Some(id), Some(q)) = (etat.glisse_feuille.take(), resp.interact_pointer_pos().or(ui.input(|i| i.pointer.latest_pos()))) {
                    let t = rang_sous(q);
                    if let Some(k) = feuilles.iter().position(|f| f.id == id)
                        && t != k && t != k + 1 {
                            deplacement = Some((id, feuilles.get(t).map(|f| f.id.clone())));
                        }
                }
                if let Some((fid, objet, d)) = etat.glisse.take()
                    && d.length() > 1.0
                    && let Some(f) = feuilles.iter().find(|f| f.id == fid) {
                        let mut f = (*f).clone();
                        let (dx, dy) = ((d.x / z) as f64, -(d.y / z) as f64);
                        let arrondi = |x: f64| (x * 10.0).round() / 10.0;
                        match objet {
                            ObjetFeuille::Place(j) => {
                                let nv = f.vues.len();
                                let position = if j < nv { f.vues.get_mut(j).map(|v| &mut v.position) } else { f.nomenclatures.get_mut(j - nv).map(|n| &mut n.position) };
                                if let Some(pos) = position {
                                    pos[0] += dx;
                                    pos[1] += dy;
                                }
                            }
                            ObjetFeuille::Cartouche => {
                                if let Some([x0, y0, _, _]) = doc::cadre_cartouche(p, &f) {
                                    f.position_cartouche = Some([arrondi(x0 + dx), arrondi(y0 + dy)]);
                                }
                            }
                            ObjetFeuille::Annotation(i) => {
                                if let Some(a) = f.annotations.get_mut(i) { a.translater([dx, dy]); }
                            }
                        }
                        commandes.push(Commande::Feuille { feuille: f });
                    }
            }
            if resp.clicked() {
                self.famille_selectionnee = None;
                let q = resp.interact_pointer_pos();
                if en_trace {
                    // clic de l'outil : un texte validé par ce clic ne lance pas le suivant
                    if !texte_valide && let Some((k, pt)) = q.and_then(viser) && let Some(a) = etat.trace.clic(&feuilles[k].id, pt) {
                        let mut f = feuilles[k].clone();
                        f.annotations.push(a);
                        commandes.push(Commande::Feuille { feuille: f });
                    }
                } else {
                    etat.selection = q.map_or(SelectionFeuille::Aucune, selection_sous);
                    if q.is_some_and(|q| page_ajout.contains(q)) {
                        let f = Self::nouvelle_feuille(p, &jeu);
                        commandes.push(Commande::Feuille { feuille: f });
                    }
                }
            }
            if resp.double_clicked() {
                if en_trace {
                    if let Some(a) = etat.trace.terminer(false) && let Some(k) = k_trace {
                        let mut f = feuilles[k].clone();
                        f.annotations.push(a);
                        commandes.push(Commande::Feuille { feuille: f });
                    }
                } else if let Some((k, objet)) = resp.interact_pointer_pos().and_then(objet_sous) {
                    let f = feuilles[k];
                    match objet {
                        ObjetFeuille::Place(j) => match f.vues.get(j) {
                            Some(v) => ouvrir_vue = Some(v.vue.clone()),
                            None => ouvrir_vue = f.nomenclatures.get(j - f.vues.len()).map(|n| format!("nomenclature:{}", n.nomenclature)),
                        },
                        ObjetFeuille::Cartouche => action = Some(ActionMenu::ModifierCartouche(f.id.clone())),
                        ObjetFeuille::Annotation(i) => {
                            if let Some(a) = f.annotations.get(i) && let doc::AnnotationForme::Texte { point, texte } = &a.forme {
                                etat.trace.texte = Some(annot::TexteFeuille { feuille: f.id.clone(), rang: Some(i), point: *point, contenu: texte.clone(), hauteur: a.hauteur, focus: true });
                            }
                        }
                    }
                }
            }
            // clavier : Entrée achève la polyligne, Échap annule le tracé puis l'outil
            if en_trace && etat.trace.texte.is_none() && !ui.ctx().egui_wants_keyboard_input() {
                if ui.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::Enter))
                    && let Some(a) = etat.trace.terminer(false) && let Some(k) = k_trace {
                        let mut f = feuilles[k].clone();
                        f.annotations.push(a);
                        commandes.push(Commande::Feuille { feuille: f });
                    }
                if ui.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::Escape)) {
                    if etat.trace.points.is_empty() { etat.trace.choisir(OutilFeuille::Selection); } else { etat.trace.points.clear(); }
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
                        let r = etat.vues.entry((g.0.clone(), echelle.to_bits())).or_insert_with(|| match moteur2d::vue_placee(p, &zero) {
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
            // tracé : pages, cartouches, vues, tableaux, annotations, sélection
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
            let decalage_de = |fid: &str, objet: ObjetFeuille| match &etat.glisse {
                Some((g, o, d)) if g == fid && *o == objet => *d,
                _ => egui::Vec2::ZERO,
            };
            let bleu = Color32::from_rgb(0, 120, 215);
            let texte_edite = etat.trace.texte.as_ref().and_then(|t| Some((t.feuille.clone(), t.rang?)));
            for (k, f) in feuilles.iter().enumerate() {
                let r = page(k);
                if !r.intersects(rect) {
                    continue;
                }
                painter.rect_filled(r, 0.0, Color32::WHITE);
                if survol_depot == Some(k) {
                    painter.rect_stroke(r, 0.0, egui::Stroke::new(3.0, bleu), egui::StrokeKind::Outside);
                }
                if let Some(c) = etat.cadres.get(&f.id) {
                    ajouter(c, k, 0.0, 0.0, decalage_de(&f.id, ObjetFeuille::Cartouche), &mut mesh, &mut traits);
                }
                for (j, v) in f.vues.iter().enumerate() {
                    if let Some(rv) = etat.vues.get(&cle_vue(v)) {
                        if let Some(e) = &rv.erreur {
                            painter.text(ecran(k, v.position[0], v.position[1]), egui::Align2::LEFT_BOTTOM, e, egui::FontId::proportional(12.0), Color32::DARK_RED);
                        }
                        ajouter(rv, k, v.position[0], v.position[1], decalage_de(&f.id, ObjetFeuille::Place(j)), &mut mesh, &mut traits);
                    }
                }
                // nomenclatures posées
                for (i, n) in f.nomenclatures.iter().enumerate() {
                    if let Some(rt) = etat.tables.get(&n.nomenclature) {
                        if let Some(e) = &rt.erreur {
                            painter.text(ecran(k, n.position[0], n.position[1]), egui::Align2::LEFT_TOP, e, egui::FontId::proportional(12.0), Color32::DARK_RED);
                        }
                        ajouter(rt, k, n.position[0], n.position[1], decalage_de(&f.id, ObjetFeuille::Place(f.vues.len() + i)), &mut mesh, &mut traits);
                    }
                }
                // annotations (le texte en cours de saisie est remplacé par son champ)
                for (i, ra) in etat.annotations.get(&f.id).into_iter().flatten().enumerate() {
                    if texte_edite.as_ref().is_some_and(|(fid, r)| *fid == f.id && *r == i) { continue; }
                    ajouter(ra, k, 0.0, 0.0, decalage_de(&f.id, ObjetFeuille::Annotation(i)), &mut mesh, &mut traits);
                }
                // objet choisi : cadre en tirets (le cartouche : cadre plein)
                if let Some((fid, objet)) = etat.selection.objet().filter(|(fid, _)| *fid == f.id)
                    && let Some(b) = boite(f, objet) {
                        let cadre = egui::Rect::from_two_pos(ecran(k, b[0], b[1]), ecran(k, b[2], b[3])).translate(decalage_de(fid, objet));
                        if objet == ObjetFeuille::Cartouche {
                            painter.rect_stroke(cadre, 0.0, egui::Stroke::new(2.0, bleu), egui::StrokeKind::Outside);
                        } else {
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
            // aperçu de l'outil : tracé en cours et point visé
            if let Some((k, pt)) = vise {
                for l in etat.trace.apercu(pt) {
                    let pts: Vec<egui::Pos2> = l.iter().map(|q| ecran(k, q[0], q[1])).collect();
                    painter.add(egui::Shape::line(pts, egui::Stroke::new(1.2, bleu)));
                }
                for q in &etat.trace.points {
                    painter.circle_filled(ecran(k, q[0], q[1]), 2.5, bleu);
                }
                let c = ecran(k, pt[0], pt[1]);
                painter.line_segment([c - egui::vec2(6.0, 0.0), c + egui::vec2(6.0, 0.0)], egui::Stroke::new(1.0, bleu));
                painter.line_segment([c - egui::vec2(0.0, 6.0), c + egui::vec2(0.0, 6.0)], egui::Stroke::new(1.0, bleu));
            }
            // feuille glissée : barre d'insertion entre deux feuilles
            if etat.glisse_feuille.is_some()
                && let Some(q) = ui.input(|i| i.pointer.latest_pos()) {
                    let t = rang_sous(q);
                    let y = if t < feuilles.len() { ys[t] - ECART * 0.9 } else { y_ajout - ECART * 0.9 };
                    let a = o + egui::vec2(-10.0 / z, y) * z;
                    painter.line_segment([a, a + egui::vec2((largeur_max + 20.0 / z) * z, 0.0)], egui::Stroke::new(4.0, bleu));
                    painter.circle_filled(a, 5.0, bleu);
                }
            // page « ajouter une feuille »
            let survol_ajout = !en_trace && resp.hover_pos().is_some_and(|q| page_ajout.contains(q));
            let gris = if survol_ajout { bleu } else { Color32::from_gray(120) };
            let r = page_ajout;
            painter.add(egui::Shape::dashed_line(&[r.left_top(), r.right_top(), r.right_bottom(), r.left_bottom(), r.left_top()], egui::Stroke::new(1.5, gris), 8.0, 6.0));
            painter.text(r.center(), egui::Align2::CENTER_CENTER, "+ Ajouter une feuille", egui::FontId::proportional(16.0), gris);
            if !feuilles.is_empty() {
                annot::barre_outils(ui, rect, &mut etat.trace);
            }
            if self.error.is_none() {
                let aide = if feuilles.is_empty() {
                    "Cliquer sur la page en pointillés pour ajouter une feuille."
                } else if en_trace {
                    etat.trace.outil.consigne(etat.trace.points.len())
                } else {
                    "Déposer une vue ou une nomenclature · Glisser un objet, le cartouche ou le titre pour le déplacer · Suppr pour retirer · Double-clic : ouvrir la vue, modifier le cartouche ou le texte · Molette pour zoomer"
                };
                crate::socle::messages::consigne_vue(ui, rect, 0.0, aide);
            }
            let curseur = if etat.glisse.is_some() || etat.glisse_feuille.is_some() {
                Some(egui::CursorIcon::Grabbing)
            } else if en_trace && vise.is_some() {
                Some(egui::CursorIcon::Crosshair)
            } else if survol_ajout {
                Some(egui::CursorIcon::PointingHand)
            } else if !en_trace && (resp.hover_pos().and_then(objet_sous).is_some() || resp.hover_pos().and_then(titre_sous).is_some()) {
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
        match action {
            Some(ActionMenu::Supprimer) => { self.supprimer_dans_jeu(); }
            Some(ActionMenu::ModifierCartouche(id)) => self.editer_cartouche_feuille(&id),
            Some(ActionMenu::CartoucheDefaut(id)) => self.placer_cartouche(&id, None),
            None => {}
        }
        if let Some(id) = ouvrir_vue.as_deref().and_then(|x| x.strip_prefix("nomenclature:")) {
            self.ouvrir_nomenclature(id);
        } else if let Some(id) = ouvrir_vue {
            let n = modele::identifiants::numero_vue(&id);
            if let Some(i) = self.scene.as_ref().and_then(|s| s.navigateur.as_ref()).and_then(|nav| nav.vues.iter().position(|v| v.revit_id == n)) {
                self.activer_vue(i);
            }
        }
    }

    /// Place le cartouche de la feuille `id` : coin bas gauche (mm du papier), ou à sa
    /// place par défaut avec None ; le remet s'il avait été retiré.
    pub(crate) fn placer_cartouche(&mut self, id: &str, position: Option<[f64; 2]>) {
        let Some(f) = self.natif.as_ref().and_then(|(p, _)| p.feuilles.iter().find(|f| f.id == id)) else { return };
        let g = doc::Feuille { position_cartouche: position, sans_cartouche: false, ..f.clone() };
        if let Err(e) = self.executer_bim(Commande::Feuille { feuille: g }) {
            self.error = Some(e);
        }
    }
}
