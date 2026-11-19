//! Canevas de l'éditeur de famille : dessin et interaction.
use super::*;

/// Saisie commune des deux projections : Échap abandonne d'abord le geste,
/// puis l'outil et enfin la sélection.
fn echap_esquisse(ui: &mut egui::Ui, resp: &egui::Response, ed: &mut EditeurFamille) {
    let en_cours = !ed.esquisse.is_empty() || !ed.clics.is_empty() || ed.debut.is_some() || ed.fenetre.is_some() || ed.deplacement_2d.is_some() || !ed.points_transfo.is_empty();
    if (resp.hovered() || en_cours) && !ui.ctx().egui_wants_keyboard_input() && ui.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::Escape)) {
        if let Some(geste) = ed.deplacement_2d.take() { ed.famille = geste.avant; }
        else if !ed.saisie_trace.texte.is_empty() { ed.saisie_trace.vider(); }
        else if en_cours { ed.abandonner_saisie(); }
        else if ed.outil != OutilFamille::Selection { ed.choisir_outil(OutilFamille::Selection); }
        else { ed.selectionner(Vec::new()); }
    }
}

fn ferme_profil(ed: &EditeurFamille, cadre: &Cadre, q: egui::Pos2) -> bool {
    ed.esquisse.len() >= 3 && ed.esquisse.first().is_some_and(|(_, p)| cadre.ecran(p.0, p.1).distance(q) < 9.0)
}

fn dessiner_axe_revolution(painter: &egui::Painter, cadre: &Cadre, ed: &EditeurFamille) {
    let axe = ed.axe_revolution.or_else(|| match ed.selection {
        SelectionFamille::Extrusion(k) => ed.famille.extrusions.get(k).filter(|e| e.support.is_none() && e.vue == ed.vue.esquisse()).and_then(|e| match &e.operation { OperationForme::Revolution { revolution } => Some(revolution.axe), _ => None }),
        _ => None,
    });
    if let Some([a,b]) = axe {
        let ligne = [cadre.ecran(a[0],a[1]),cadre.ecran(b[0],b[1])];
        painter.add(egui::Shape::dashed_line(&ligne,egui::Stroke::new(2.0,Color32::DARK_GREEN),8.0,4.0));
        painter.text(ligne[1],egui::Align2::LEFT_BOTTOM,"Axe",egui::FontId::proportional(11.0),Color32::DARK_GREEN);
    }
}

/// Vue de travail : mur générique, volumes, vides, plans, cotes ; interactions selon
/// l'outil.
pub(super) fn canevas(ui: &mut egui::Ui, ed: &mut EditeurFamille, projet: &modele::Projet, delai_accrochage: f64) {
    let selection_avant = ed.selection.clone();
    let mut garder_multiple = false;
    let taille = ui.available_size().max(egui::vec2(80.0, 80.0));
    let (rect, resp) = ui.allocate_exact_size(taille, Sense::click_and_drag());
    crate::socle::aides::curseur_dessin(ui, &resp, ed.outil != OutilFamille::Selection);
    let painter = ui.painter_at(rect);
    let r = ed.resoudre();
    let (ah, av) = ed.vue.axes();
    let pos = |nom: &str| r.positions.get(nom).copied().unwrap_or(0.0);
    // cadrage : plans visibles, baie et mur
    let etendue = |axe: AxePlan, defaut: [f64; 2]| {
        ed.famille.plans.iter().filter(|p| p.axe == axe).map(|p| pos(&p.nom)).fold(defaut, |[a, b], x| [a.min(x), b.max(x)])
    };
    let base = |axe: AxePlan| match axe {
        AxePlan::X => [-ed.largeur / 2.0, ed.largeur / 2.0],
        AxePlan::Y => [-ed.epaisseur / 2.0, ed.epaisseur / 2.0],
        AxePlan::Z => [0.0, ed.hauteur],
    };
    let (h0, h1, v0, v1, marge) = cadrage_vue(ed, &r, etendue(ah, base(ah)), etendue(av, base(av)));
    ed.rect_navigation=rect;
    let nav=&mut ed.navigation[ed.vue as usize];
    nav.souris(ui, &resp, rect);
    let cadre=nav.preparer(rect, (ah as usize, av as usize), [h0, h1, v0, v1]);
    let echelle=cadre.echelle;
    let boite = |a: (f64, f64), b: (f64, f64)| egui::Rect::from_two_pos(cadre.ecran(a.0, a.1), cadre.ecran(b.0, b.1));
    let (ih, iv) = (ah as usize, av as usize);
    let boites = ed.famille.boites(&r);
    super::plans_tournes::dessiner_traces(&painter, &cadre, ed, &r);

    // Les solides, le mur générique, les éléments 2D et les cotes de mesure sont rendus
    // par le `Renderer` du projet (rendu.rs) ; ici ne restent que les superpositions de
    // l'éditeur et les zones de sélection.
    if matches!(ed.famille.categorie, modele::famille::modelee::CategorieFamille::Porte | modele::famille::modelee::CategorieFamille::Fenetre) {
        let ext = match ed.vue {
            VueFamille::Elevation => None,
            VueFamille::Plan => Some((egui::Align2::CENTER_BOTTOM, cadre.ecran(0.0, ed.epaisseur / 2.0 + marge * 0.6))),
            VueFamille::Coupe => Some((egui::Align2::LEFT_CENTER, cadre.ecran(ed.epaisseur / 2.0 + marge * 0.3, (v0 + v1) / 2.0))),
        };
        if let Some((a, p)) = ext {
            painter.text(p, a, "Extérieur", egui::FontId::proportional(12.0), Color32::from_gray(110));
        }
    }

    // vides : contours tiretés (ils ne sont pas des solides) ; sélection en orange
    let orange = Color32::from_rgb(230, 120, 20);
    // volumes pleins du plus loin au plus proche (sélection au clic : le plus proche)
    let profondeur = 3 - ih - iv;
    let mut pleins: Vec<_> = boites.iter().filter(|b| !b.vide).collect();
    pleins.sort_by(|a, b| a.min[profondeur].total_cmp(&b.min[profondeur]));
    if ed.vue == VueFamille::Elevation { pleins.reverse(); }
    for b in &boites {
        let choisi = ed.choisi(&SelectionFamille::Volume(b.indice));
        if !b.vide && !choisi { continue; }
        let r = boite((b.min[ih], b.min[iv]), (b.max[ih], b.max[iv]));
        let pts = [r.left_top(), r.right_top(), r.right_bottom(), r.left_bottom(), r.left_top()];
        if b.vide { painter.add(egui::Shape::dashed_line(&pts, egui::Stroke::new(if choisi { 2.5 } else { 1.5 }, orange), 7.0, 4.0)); }
        else { painter.add(egui::Shape::line(pts.to_vec(), egui::Stroke::new(2.5, orange))); }
    }

    // extrusions : profil dans leur vue d'esquisse, emprise ailleurs (zones de sélection)
    let solides = ed.famille.solides(&r);
    let decalage_ext = ed.famille.volumes.len();
    let mut contours_ext: Vec<(usize, Vec<egui::Pos2>)> = Vec::new();
    for (k, e) in ed.famille.extrusions.iter().enumerate() {
        let pts: Vec<egui::Pos2> = if e.support.is_none() && e.vue == ed.vue.esquisse() {
            e.profil.iter().map(|p| { let (h, v) = ed.famille.point_esquisse(&r, p); cadre.ecran(h, v) }).collect()
        } else {
            let Some(s) = solides.get(decalage_ext + k) else { continue };
            let (mut a, mut b) = ((f64::INFINITY, f64::INFINITY), (f64::NEG_INFINITY, f64::NEG_INFINITY));
            for q in s.tris.iter().flatten() {
                let q = q.to_array();
                a = (a.0.min(q[ih]), a.1.min(q[iv]));
                b = (b.0.max(q[ih]), b.1.max(q[iv]));
            }
            if !a.0.is_finite() {
                continue;
            }
            let rr = boite(a, b);
            vec![rr.left_top(), rr.right_top(), rr.right_bottom(), rr.left_bottom()]
        };
        if pts.len() < 3 {
            continue;
        }
        let choisi = ed.choisi(&SelectionFamille::Extrusion(k));
        if e.vide || choisi {
            let mut ferme = pts.clone();
            ferme.push(pts[0]);
            if e.vide { painter.add(egui::Shape::dashed_line(&ferme, egui::Stroke::new(if choisi { 2.5 } else { 1.5 }, orange), 7.0, 4.0)); }
            else { painter.add(egui::Shape::line(ferme, egui::Stroke::new(2.5, orange))); }
        }
        contours_ext.push((k, pts));
    }

    // Éléments 2D : zones de sélection (le dessin est celui des annotations du projet).
    let mut textes_2d = Vec::new();
    let mut lignes_2d: Vec<(usize, Vec<egui::Pos2>)> = Vec::new();
    for (indice, element) in ed.famille.elements_2d.iter().enumerate().filter(|(_, e)| e.vue == ed.vue.esquisse()) {
        use modele::documentation::AnnotationForme;
        let element = &ed.famille.element_resolu(&r, element);
        if let AnnotationForme::Texte { point, texte } = &element.forme {
            let papier = matches!(ed.famille.categorie, modele::famille::modelee::CategorieFamille::Cartouche | modele::famille::modelee::CategorieFamille::EtiquettePiece);
            let taille = taille_texte(element.hauteur, papier, ed.navigation[ed.vue as usize].echelle_vue, &cadre);
            let apercu = crate::outils::annoter::outils_texte::apercu_famille(texte, projet, ed.vue.nom(), ed.famille.categorie, ed.cible_feuille.as_deref(), ed.cible_piece.as_deref());
            let largeur = noyau::texte::largeur(&apercu, "Arial", taille as f64, false) as f32;
            let p = cadre.ecran(point[0], point[1]);
            let x0 = if ed.famille.categorie == modele::famille::modelee::CategorieFamille::EtiquettePiece { p.x - largeur / 2.0 } else { p.x };
            textes_2d.push((indice, egui::Rect::from_min_max(egui::pos2(x0, p.y - taille), egui::pos2(x0 + largeur, p.y)).expand(5.0)));
        } else if let Some(forme) = element.forme.forme_2d() {
            for ligne in forme.polylignes() {
                lignes_2d.push((indice, ligne.into_iter().map(|p| cadre.ecran(p.x, p.y)).collect()));
            }
        }
    }

    // cotes de mesure de la vue (angle, diamètre, altimétrie) : zones de sélection
    let mut lignes_mesures: Vec<(usize, Vec<[egui::Pos2; 2]>)> = Vec::new();
    for (k, c) in ed.famille.cotes_mesure.iter().enumerate() {
        if c.vue != ed.vue.esquisse() { continue; }
        let pts: Vec<glam::DVec2> = c.points.iter().map(|p| { let (h, v) = ed.famille.point_esquisse(&r, p); glam::DVec2::new(h, v) }).collect();
        let Some(g) = geometrie_mesure(c.genre, c.vue, &pts, echelle) else { continue };
        let segs: Vec<[egui::Pos2; 2]> = g.traits.iter().map(|[a, b]| [cadre.ecran(a.x, a.y), cadre.ecran(b.x, b.y)]).collect();
        lignes_mesures.push((k, segs));
    }

    // plans de référence
    let vert = Color32::from_rgb(40, 150, 90);
    let bleu = Color32::from_rgb(30, 90, 200);
    let police = egui::FontId::proportional(11.0);
    let mut lignes: Vec<(String, bool, f32)> = Vec::new(); // (nom, vertical, coordonnée écran)
    let (mut rang_pv, mut rang_ph) = (0usize, 0usize);
    for p in ed.famille.plans.iter().filter(|p| p.axe == ah || p.axe == av) {
        let vertical = p.axe == ah;
        let quinconce = if vertical { rang_pv += 1; (rang_pv % 3) as f32 * 13.0 } else { rang_ph += 1; (rang_ph % 2) as f32 * 90.0 };
        let x = pos(&p.nom);
        let choisi = ed.choisi(&SelectionFamille::Plan(p.nom.clone())) || ed.clics.contains(&p.nom);
        let couleur = if choisi { bleu } else if p.hote.is_some() { Color32::from_rgb(140, 90, 40) } else { vert };
        let trait_ = egui::Stroke::new(if choisi { 1.8 } else { 1.0 }, couleur);
        let (a, b, etiquette) = if vertical {
            let e = cadre.ecran(x, 0.0).x;
            (egui::pos2(e, rect.top()), egui::pos2(e, rect.bottom()), egui::pos2(e + 3.0, rect.bottom() - 4.0 - quinconce))
        } else {
            let e = cadre.ecran(0.0, x).y;
            (egui::pos2(rect.left(), e), egui::pos2(rect.right(), e), egui::pos2(rect.right() - 4.0 - quinconce, e - 2.0))
        };
        painter.add(egui::Shape::dashed_line(&[a, b], trait_, 8.0, 3.0));
        let ancrage = if vertical { egui::Align2::LEFT_BOTTOM } else { egui::Align2::RIGHT_BOTTOM };
        let nom = if p.fixe { format!("{} (fixe)", p.nom) } else { p.nom.clone() };
        painter.text(etiquette, ancrage, nom, police.clone(), couleur);
        lignes.push((p.nom.clone(), vertical, if vertical { a.x } else { a.y }));
    }

    // cotes et égalités dont les plans sont visibles, dessinées comme celles du projet
    // (geometrie_cote) ; la ligne de cote se glisse, sinon elle est rangée au bord
    let mut etiquettes: Vec<(usize, egui::Rect)> = Vec::new();
    let mut lignes_cotes: Vec<(usize, [egui::Pos2; 2])> = Vec::new();
    let (mut rang_h, mut rang_v) = (0, 0);
    for (k, c) in ed.famille.contraintes.iter().enumerate().filter(|(_, c)| !matches!(c, Contrainte::Angle { .. })) {
        let noms = c.plans();
        let Some(axe) = ed.famille.plan(noms[0]).map(|p| p.axe) else { continue };
        if axe != ah && axe != av {
            continue;
        }
        let horizontale = axe == ah;
        let conflit = r.conflits.contains(&k);
        let choisie = ed.choisi(&SelectionFamille::Contrainte(k)) || ed.cote_deplacee == Some(k);
        let couleur = if conflit { Color32::from_rgb(210, 40, 40) } else if choisie { bleu } else { Color32::from_gray(40) };
        let rang = if horizontale { rang_h += 1; rang_h } else { rang_v += 1; rang_v };
        let bord = 14.0 + 18.0 * (rang - 1) as f32;
        let ligne = c.ligne().unwrap_or_else(|| if horizontale { cadre.modele(egui::pos2(0.0, rect.top() + bord)).1 } else { cadre.modele(egui::pos2(rect.left() + bord, 0.0)).0 });
        let (attache, oblique) = (10.0 / echelle, 8.0 / echelle);
        let mut xs: Vec<f64> = noms.iter().map(|n| pos(n)).collect();
        xs.sort_by(f64::total_cmp);
        let points: Vec<glam::DVec2> = xs.iter().map(|&x| if horizontale { glam::DVec2::new(x, ligne - attache) } else { glam::DVec2::new(ligne + attache, x) }).collect();
        let g = modele::documentation::geometrie_cote(&points, attache, oblique);
        let ecran = |p: glam::DVec2| cadre.ecran(p.x, p.y);
        let trait_ = egui::Stroke::new(if choisie { 1.6 } else { 1.0 }, couleur);
        for [a, b] in &g.traits {
            painter.line_segment([ecran(*a), ecran(*b)], trait_);
        }
        if let (Some(a), Some(b)) = (points.first(), points.last()) {
            let n = if horizontale { glam::DVec2::new(0.0, attache) } else { glam::DVec2::new(-attache, 0.0) };
            lignes_cotes.push((k, [ecran(*a + n), ecran(*b + n)]));
        }
        let textes: Vec<(glam::DVec2, String)> = match c {
            Contrainte::Distance { valeur, .. } => g.textes.iter().map(|&(p, v)| (p, if valeur.trim().parse::<f64>().is_ok() { modele::unite::unite().afficher(v) } else { format!("{valeur} = {}", modele::unite::unite().afficher(v)) })).collect(),
            Contrainte::Egalite { .. } => g.textes.iter().map(|&(p, _)| (p, "EQ".to_string())).collect(),
            Contrainte::Angle { .. } => Vec::new(),
        };
        for (p, texte) in textes {
            let galee = painter.layout_no_wrap(texte, police.clone(), couleur);
            let decalage_texte = if horizontale { egui::vec2(0.0, -8.0) } else { egui::vec2(10.0 + galee.size().x / 2.0, 0.0) };
            let cadre_txt = egui::Rect::from_center_size(ecran(p) + decalage_texte, galee.size() + egui::vec2(6.0, 2.0));
            painter.rect_filled(cadre_txt, 2.0, Color32::from_rgba_unmultiplied(255, 255, 255, 230));
            painter.galley(cadre_txt.min + egui::vec2(3.0, 1.0), galee, couleur);
            etiquettes.push((k, cadre_txt));
        }
    }

    crate::socle::aides::repere_origine_famille(&painter, cadre.ecran(0.0, 0.0));
    // interactions
    let curseur = resp.hover_pos().or(resp.interact_pointer_pos());
    echap_esquisse(ui, &resp, ed);
    let plan_vise = |q: egui::Pos2| {
        lignes
            .iter()
            .map(|(n, vertical, c)| (n.clone(), if *vertical { (q.x - c).abs() } else { (q.y - c).abs() }))
            .filter(|x| x.1 < 7.0)
            .min_by(|a, b| a.1.total_cmp(&b.1))
            .map(|x| x.0)
    };
    let tol = 7.0 / echelle;
    // Source de géométrie de la famille pour le moteur d'accrochage commun aux vues du
    // projet : plans de référence en droites, contours des formes, éléments 2D.
    let source = {
        use noyau::forme2d::Geom;
        use glam::DVec2;
        let mut source = crate::saisie::moteur_accrochage::Geometrie::default();
        for plan in &ed.famille.plans {
            let valeur = pos(&plan.nom);
            if plan.axe == ah { source.droites.push((DVec2::new(valeur, 0.0), DVec2::Y)); }
            else if plan.axe == av { source.droites.push((DVec2::new(0.0, valeur), DVec2::X)); }
        }
        let vue = ed.vue.esquisse();
        source.geoms = super::rendu::aretes_vue(ed, &r);
        for points in ed.famille.extrusions.iter().filter(|e| e.support.is_none() && e.vue == vue).map(|e| &e.profil) {
            let pts: Vec<_> = points.iter().map(|p| { let (h, v) = ed.famille.point_esquisse(&r, p); DVec2::new(h, v) }).collect();
            for i in 0..pts.len() { source.geoms.push(Geom::Segment([pts[i], pts[(i+1) % pts.len()]])); }
        }
        source.geoms.extend(ed.famille.elements_2d_resolus(&r, vue).iter()
            .filter_map(|e| e.forme.forme_2d()).flat_map(|forme| forme.geoms()));
        let pts: Vec<_> = ed.esquisse.iter().map(|(_, p)| DVec2::new(p.0, p.1)).collect();
        source.geoms.extend(pts.windows(2).map(|w| Geom::Segment([w[0], w[1]])));
        if let [p] = pts.as_slice() { source.geoms.push(Geom::Segment([*p, *p])); }
        source
    };
    let tol_accroche = crate::saisie::moteur_accrochage::TOLERANCE_PX / echelle;
    let maj = ui.input(|i| i.modifiers.shift);
    let depart_courant = ed.esquisse.last().map(|(_, p)| *p).or(ed.debut).or(ed.points_transfo.last().copied());
    let curseur_guide = curseur.filter(|_| resp.hovered() && ed.outil != OutilFamille::Selection);
    let papier = matches!(ed.famille.categorie, modele::famille::modelee::CategorieFamille::Cartouche | modele::famille::modelee::CategorieFamille::EtiquettePiece);
    let facteur = if papier { 1.0 } else { modele::unite::unite().facteur() };
    let point_guide = curseur_guide.map(|q| {
        let (h, v) = cadre.modele(q);
        let visee = crate::saisie::moteur_accrochage::Visee { tol: tol_accroche, precedent: depart_courant.map(|p| glam::DVec2::new(p.0, p.1)), maj, temps: ui.input(|i| i.time), delai: delai_accrochage, angle_axes: 0.0, trace_actif: ed.trace_polyligne.is_some(), saisie: &ed.saisie_trace.texte, facteur, pas: 1.0 };
        crate::saisie::moteur_accrochage::viser(&mut ed.reperage, &source, glam::DVec2::new(h, v), &visee)
    });
    let (o, h1, v1) = (cadre.ecran(0.0, 0.0), cadre.ecran(1.0, 0.0), cadre.ecran(0.0, 1.0));
    ed.reperage.repere_ecran = [o.x, o.y, h1.x - o.x, h1.y - o.y, v1.x - o.x, v1.y - o.y];
    if curseur_guide.is_some() {
        ed.reperage.peindre(&painter, |p| cadre.ecran(p.x, p.y));
        if ed.reperage.survol_accroche.is_some() || ed.reperage.survol_segment.is_some() {
            ui.ctx().request_repaint_after(std::time::Duration::from_millis(100));
        }
    }
    let viser = |q: egui::Pos2, depart: Option<(f64, f64)>| -> ((f64, f64), moteur2d::dao::Accroche) {
        let (p, a) = match point_guide.filter(|_| curseur_guide == Some(q) && depart == depart_courant) {
            Some(x) => x,
            None => {
                let (h, v) = cadre.modele(q);
                crate::saisie::moteur_accrochage::viser_simple(&source, glam::DVec2::new(h, v), tol_accroche, depart.map(|p| glam::DVec2::new(p.0, p.1)), maj, 0.0, 1.0)
            }
        };
        ((p.x, p.y), a)
    };
    // repère d'accrochage sous la souris (hors sélection) : marqueur commun
    if ed.outil != OutilFamille::Selection || ed.poignee.is_some() {
        if let Some(q) = curseur.filter(|_| resp.hovered() || ed.poignee.is_some()) {
            let (p, a) = viser(q, depart_courant);
            crate::saisie::moteur_accrochage::peindre_marqueur(&painter, cadre.ecran(p.0, p.1), a);
        }
    }
    dessiner_axe_revolution(&painter, &cadre, ed);
    match ed.outil {
        OutilFamille::Selection | OutilFamille::ModifierTrace => {
            // cycle de sélection commun au projet (présélection, Tab)
            let zones = cycle_famille::Zones { textes_2d: &textes_2d, lignes_2d: &lignes_2d, etiquettes: &etiquettes, lignes_mesures: &lignes_mesures, contours: &contours_ext, plans: &lignes, cadre: rect,
                volumes: pleins.iter().rev().copied().chain(boites.iter().filter(|b| b.vide)).map(|b| (b.indice, boite((b.min[ih], b.min[iv]), (b.max[ih], b.max[iv])))).collect() };
            let o = cadre.ecran(0.0, 0.0);
            cycle_famille::survol(ui, ed, &resp, &painter, &zones, crate::saisie::cycle::empreinte(&[o.x.to_bits() as u64, o.y.to_bits() as u64, echelle.to_bits(), ed.vue as u64, (textes_2d.len() + lignes_2d.len() + contours_ext.len() + boites.len() + lignes.len()) as u64]));
            // poignées de l'élément choisi : sommets et milieux d'arêtes (insertion) d'un
            // profil ou d'un symbole de la vue, milieux des faces d'un volume
            let vue = ed.vue.esquisse();
            let mut poignees: Vec<(Poignee, egui::Pos2, bool)> = Vec::new(); // (poignée, position, milieu à insérer)
            let ecran_pt = |p: &PointEsquisse| { let (h, v) = ed.famille.point_esquisse(&r, p); cadre.ecran(h, v) };
            match ed.selection.clone() {
                SelectionFamille::Extrusion(k) if ed.famille.extrusions.get(k).is_some_and(|e| e.support.is_none() && e.vue == vue) => {
                    let pts: Vec<egui::Pos2> = ed.famille.extrusions[k].profil.iter().map(ecran_pt).collect();
                    for (i, &q) in pts.iter().enumerate() {
                        poignees.push((Poignee::Sommet { extrusion: true, k, i }, q, false));
                        poignees.push((Poignee::Sommet { extrusion: true, k, i: i + 1 }, q.lerp(pts[(i + 1) % pts.len()], 0.5), true));
                    }
                }
                SelectionFamille::Element2D(k) if ed.element_ancre(k).is_some() => {
                    for (i, (h, v), milieu) in ed.poignees_ancrees(&r, k) {
                        poignees.push((Poignee::Sommet { extrusion: false, k, i }, cadre.ecran(h, v), milieu));
                    }
                }
                SelectionFamille::Element2D(k) => {
                    if let Some(element) = ed.famille.elements_2d.get(k).filter(|e| e.vue == vue) {
                        for (i, point) in crate::outils::dessiner::dessin_plan::poignees_forme(&element.forme).into_iter().enumerate() {
                            poignees.push((Poignee::Element2D { k, i }, cadre.ecran(point[0], point[1]), false));
                        }
                    }
                }
                SelectionFamille::Volume(k) => {
                    if let Some(b) = boites.iter().find(|b| b.indice == k) {
                        let (mh, mv) = ((b.min[ih] + b.max[ih]) / 2.0, (b.min[iv] + b.max[iv]) / 2.0);
                        poignees.push((Poignee::Face { k, axe: ih, max: false }, cadre.ecran(b.min[ih], mv), false));
                        poignees.push((Poignee::Face { k, axe: ih, max: true }, cadre.ecran(b.max[ih], mv), false));
                        poignees.push((Poignee::Face { k, axe: iv, max: false }, cadre.ecran(mh, b.min[iv]), false));
                        poignees.push((Poignee::Face { k, axe: iv, max: true }, cadre.ecran(mh, b.max[iv]), false));
                    }
                }
                _ => {}
            }
            traits::peindre_segment(&painter, ed, &cadre, &r); for (_, q, milieu) in &poignees {
                if *milieu {
                    painter.circle(*q, 3.5, Color32::WHITE, egui::Stroke::new(1.2, bleu));
                } else {
                    painter.rect(egui::Rect::from_center_size(*q, egui::vec2(8.0, 8.0)), 0.0, Color32::WHITE, egui::Stroke::new(1.5, bleu), egui::StrokeKind::Middle);
                }
            }
            let visee = |q: egui::Pos2| poignees.iter().filter(|p| p.1.distance(q) < 7.0).min_by(|a, b| a.1.distance(q).total_cmp(&b.1.distance(q))).copied();
            // clic droit sur un sommet : le retirer (au moins 3 sommets pour un profil, 2
            // pour une ligne, l'arc garde ses 3 points)
            if resp.clicked_by(PointerButton::Secondary) {
                if let Some((Poignee::Sommet { extrusion, k, i }, _, false)) = resp.interact_pointer_pos().and_then(visee) {
                    if extrusion && ed.supprimer_sommet_forme(k, i) {
                    } else if !extrusion && ed.supprimer_sommet_ancre(k, i) {
                    } else {
                        ed.message = Some("Sommet conservé : nombre minimal atteint.".into());
                    }
                }
            }
            // point d'appui du glissement (egui signale le glissement après quelques pixels)
            let appui = ui.input(|i| i.pointer.press_origin()).or(resp.interact_pointer_pos());
            // ligne de cote glissée : étiquette ou ligne visée
            if resp.drag_started_by(PointerButton::Primary) {
                ed.cote_deplacee = appui.and_then(|q| {
                    etiquettes.iter().find(|(_, r)| r.contains(q)).map(|e| e.0).or_else(|| lignes_cotes.iter().find(|(_, [a, b])| distance_segment(q, *a, *b) < 5.0).map(|l| l.0))
                });
            }
            if let (Some(k), Some(q)) = (ed.cote_deplacee, resp.interact_pointer_pos().filter(|_| resp.dragged_by(PointerButton::Primary))) {
                let (h, v) = cadre.modele(q);
                let horizontale = ed.famille.contraintes.get(k).and_then(|c| ed.famille.plan(c.plans()[0])).is_some_and(|p| p.axe == ah);
                if let Some(c) = ed.famille.contraintes.get_mut(k) {
                    *c.ligne_mut() = Some((if horizontale { v } else { h }).round());
                }
                ed.selection = SelectionFamille::Contrainte(k);
            }
            if resp.drag_stopped_by(PointerButton::Primary) && ed.cote_deplacee.take().is_some() {
                ed.poignee = None;
                ed.deplace = None;
            }
            if resp.drag_started_by(PointerButton::Primary) && ed.cote_deplacee.is_none() {
                ed.poignee = None;
                if let Some((p, _, milieu)) = appui.and_then(visee) {
                    // milieu d'arête : un nouveau sommet est inséré puis déplacé
                    if let (true, Poignee::Sommet { extrusion, k, i }) = (milieu, p) {
                        if extrusion { ed.inserer_sommet_forme(k, i); }
                        else { ed.inserer_sommet_ancre(k, i); }
                    }
                    ed.poignee = Some(p);
                }
            }
            if let (Some(p), Some(q)) = (ed.poignee, resp.interact_pointer_pos().filter(|_| resp.dragged_by(PointerButton::Primary))) {
                let (h, v) = cadre.modele(q);
                match p {
                    Poignee::Sommet { extrusion, k, i } => {
                        let (h, v) = viser(q, None).0;
                        if let (Some(fh), Some(fv)) = (ed.accrocher(&r, ah, h, tol), ed.accrocher(&r, av, v, tol)) {
                            let points = if extrusion { &mut ed.famille.extrusions[k].profil } else { &mut ed.famille.elements_2d[k].ancrage };
                            if let Some(pt) = points.get_mut(i) {
                                *pt = PointEsquisse { h: fh, v: fv };
                            }
                        }
                    }
                    Poignee::Element2D { k, i } => {
                        let (h, v) = viser(q, None).0;
                        if let Some(forme) = ed.famille.elements_2d.get_mut(k).map(|e| &mut e.forme) {
                            crate::outils::dessiner::dessin_plan::deplacer_poignee_forme(forme, i, [h, v]);
                        }
                    }
                    Poignee::Face { k, axe, max } => {
                        let x = if axe == ih { h } else { v };
                        if let Some(f) = ed.accrocher(&r, AxePlan::TOUS[axe], x, tol) {
                            let vol = &mut ed.famille.volumes[k];
                            if max { vol.max[axe] = f } else { vol.min[axe] = f }
                        }
                    }
                }
                ed.message = Some("Poignée : accrochée au plan le plus proche (décalage au-delà de 7 px) ; clic droit sur un sommet pour le retirer.".into());
            }
            if resp.drag_stopped_by(PointerButton::Primary) {
                ed.poignee = None;
            }
            if resp.drag_started_by(PointerButton::Primary) && ed.poignee.is_none() && ed.cote_deplacee.is_none() && ed.outil == OutilFamille::Selection {
                let cible_2d = appui.and_then(|q| textes_2d.iter().rev().find(|(_, r)| r.contains(q)).map(|x| x.0)
                    .or_else(|| lignes_2d.iter().rev().find(|(_, l)| l.windows(2).any(|w| distance_segment(q, w[0], w[1]) < 6.0)).map(|x| x.0)));
                if let (Some(k), Some(depart)) = (cible_2d, appui) {
                    let cible = SelectionFamille::Element2D(k);
                    if !ed.choisi(&cible) { ed.selectionner(vec![cible]); }
                    ed.deplacement_2d = Some(DeplacementFamille { selection: ed.selectionnes(), precedent: depart, avant: ed.famille.clone() });
                    garder_multiple = true;
                } else if let Some(depart) = appui {
                    let cible = contours_ext.iter().rev().find(|(k, p)| ed.choisi(&SelectionFamille::Extrusion(*k)) && dans_polygone(depart, p)).map(|x| SelectionFamille::Extrusion(x.0))
                        .or_else(|| { let (h, v) = cadre.modele(depart); boites.iter().find(|b| ed.choisi(&SelectionFamille::Volume(b.indice)) && (b.min[ih]..=b.max[ih]).contains(&h) && (b.min[iv]..=b.max[iv]).contains(&v)).map(|b| SelectionFamille::Volume(b.indice)) });
                    if cible.is_some() {
                        ed.deplacement_2d = Some(DeplacementFamille { selection: ed.selectionnes(), precedent: depart, avant: ed.famille.clone() });
                        garder_multiple = true;
                    } else { ed.deplace = plan_vise(depart); }
                }
            }
            if let Some(mut geste) = ed.deplacement_2d.take() {
                if ui.input(|i| i.pointer.button_pressed(PointerButton::Secondary)) {
                    ed.famille = geste.avant;
                } else if let Some(q) = resp.interact_pointer_pos().filter(|_| resp.dragged_by(PointerButton::Primary) || resp.drag_stopped_by(PointerButton::Primary)) {
                    let (h0, v0) = cadre.modele(geste.precedent);
                    let (h1, v1) = cadre.modele(q);
                    if let Err(e) = ed.translater_selection(&geste.selection, h1 - h0, v1 - v0) { ed.message = Some(e); }
                    geste.precedent = q;
                    if !resp.drag_stopped_by(PointerButton::Primary) { ed.deplacement_2d = Some(geste); }
                    garder_multiple = true;
                } else if !resp.drag_stopped_by(PointerButton::Primary) { ed.deplacement_2d = Some(geste); }
            }
            if let (Some(nom), Some(q)) = (ed.deplace.clone(), resp.interact_pointer_pos().filter(|_| resp.dragged_by(PointerButton::Primary))) {
                let (h, v) = cadre.modele(q);
                let contraint = ed.famille.contraintes.iter().any(|c| c.plans().contains(&nom.as_str()));
                if let Some(p) = ed.famille.plans.iter_mut().find(|p| p.nom == nom) {
                    if p.fixe || p.hote.is_some() {
                        ed.message = Some(format!("« {nom} » est fixe."));
                    } else {
                        p.position = (if p.axe == ah { h } else { v }).round();
                        ed.message = contraint.then(|| format!("« {nom} » suit ses contraintes : modifier la cote, ou la supprimer pour le déplacer librement."));
                    }
                }
                ed.selection = SelectionFamille::Plan(nom);
            }
            if resp.drag_stopped_by(PointerButton::Primary) {
                ed.deplace = None;
            }
            if resp.clicked() && !traits::clic_segment(ed, &cadre, &r, resp.interact_pointer_pos()) {
                if let Some(q) = resp.interact_pointer_pos() {
                    let cible = cycle_famille::au_clic(ed, &zones, q);
                    // Ctrl+clic, comme dans le projet : ajoute ou retire
                    if ui.input(|i| i.modifiers.ctrl || i.modifiers.command) {
                        let mut liste = ed.selectionnes();
                        if let Some(k) = liste.iter().position(|s| *s == cible) {
                            liste.remove(k);
                        } else {
                            liste.insert(0, cible);
                        }
                        ed.selectionner(liste);
                        garder_multiple = true;
                    } else {
                        ed.selection = cible;
                    }
                }
            }
            // fenêtre de sélection : glisser hors des poignées, cotes et plans
            if resp.drag_started_by(PointerButton::Primary) && ed.poignee.is_none() && ed.cote_deplacee.is_none() && ed.deplace.is_none() && ed.deplacement_2d.is_none() && ed.outil == OutilFamille::Selection {
                ed.fenetre = appui.map(|d| FenetreSelection { debut: d, fin: d, ajouter: ui.input(|i| i.modifiers.ctrl || i.modifiers.command) });
            }
            if let (Some(f), Some(q)) = (ed.fenetre.as_mut(), resp.interact_pointer_pos()) {
                f.fin = q.clamp(rect.min, rect.max);
            }
            if let Some(f) = ed.fenetre {
                f.dessiner(&painter);
            }
            if resp.drag_stopped_by(PointerButton::Primary) {
                if let Some(f) = ed.fenetre.take() {
                    // segments de chaque élément à l'écran, puis la règle commune
                    let mut elements: Vec<(SelectionFamille, Vec<[egui::Pos2; 2]>)> = Vec::new();
                    let fermes = |pts: &[egui::Pos2]| (0..pts.len()).map(|i| [pts[i], pts[(i + 1) % pts.len()]]).collect::<Vec<_>>();
                    for b in &boites {
                        let r = boite((b.min[ih], b.min[iv]), (b.max[ih], b.max[iv]));
                        elements.push((SelectionFamille::Volume(b.indice), fermes(&[r.left_top(), r.right_top(), r.right_bottom(), r.left_bottom()])));
                    }
                    for (k, pts) in &contours_ext {
                        elements.push((SelectionFamille::Extrusion(*k), fermes(pts)));
                    }
                    for (k, pts) in &lignes_2d {
                        elements.push((SelectionFamille::Element2D(*k), pts.windows(2).map(|w| [w[0], w[1]]).collect()));
                    }
                    for (k, r) in &textes_2d {
                        elements.push((SelectionFamille::Element2D(*k), fermes(&[r.left_top(), r.right_top(), r.right_bottom(), r.left_bottom()])));
                    }
                    for (n, vertical, c) in &lignes {
                        let seg = if *vertical { [egui::pos2(*c, rect.top()), egui::pos2(*c, rect.bottom())] } else { [egui::pos2(rect.left(), *c), egui::pos2(rect.right(), *c)] };
                        elements.push((SelectionFamille::Plan(n.clone()), vec![seg]));
                    }
                    for (k, segs) in &lignes_mesures {
                        elements.push((SelectionFamille::CoteMesure(*k), segs.clone()));
                    }
                    for (k, seg) in &lignes_cotes {
                        elements.push((SelectionFamille::Contrainte(*k), vec![*seg]));
                    }
                    let (zone, croisement) = (f.rectangle(), f.croisement());
                    let mut ids = moteur3d::selection_rectangle::groupes(
                        elements.iter().enumerate().flat_map(|(i, (_, segs))| segs.iter().map(move |&[a, b]| (i, (true, moteur3d::selection_rectangle::segment(zone, a, b, false), moteur3d::selection_rectangle::segment(zone, a, b, true))))),
                        croisement,
                    );
                    ids.sort_unstable();
                    let mut liste: Vec<SelectionFamille> = if f.ajouter { ed.selectionnes() } else { Vec::new() };
                    liste.extend(ids.into_iter().map(|i| elements[i].0.clone()).filter(|e| !ed.choisi(e) || !f.ajouter));
                    ed.selectionner(liste);
                    garder_multiple = true;
                }
            }
        }
        OutilFamille::PlanReference => {
            // deux points : le plan passe par le premier et suit la direction dominante
            let vertical = |a: (f64, f64), b: (f64, f64)| (b.1 - a.1).abs() > (b.0 - a.0).abs();
            let poser = |ed: &mut EditeurFamille, a: (f64, f64), b: (f64, f64)| {
                let v = vertical(a, b);
                let nom = nom_libre(|n| ed.famille.existe_plan(n), "Plan");
                ed.famille.plans.push(PlanRef { nom: nom.clone(), axe: if v { ah } else { av }, position: (if v { a.0 } else { a.1 }).round(), fixe: false, hote: None });
                ed.selection = SelectionFamille::Plan(nom);
                ed.message = Some("Plan posé : le nommer dans Plans de référence ; Échap pour revenir à la sélection.".into());
            };
            let assez_long = |a: (f64, f64), q: egui::Pos2| cadre.ecran(a.0, a.1).distance(q) > 4.0;
            if let Some(q) = curseur.filter(|_| resp.hovered()) {
                let trait_ = egui::Stroke::new(1.0, vert);
                match ed.debut {
                    None => {
                        painter.circle_stroke(q, 4.0, trait_);
                    }
                    Some(a) => {
                        let c = viser(q, Some(a)).0;
                        let v = vertical(a, c);
                        let (ea, fin) = (cadre.ecran(a.0, a.1), if v { cadre.ecran(a.0, c.1) } else { cadre.ecran(c.0, a.1) });
                        let ligne = if v { [egui::pos2(ea.x, rect.top()), egui::pos2(ea.x, rect.bottom())] } else { [egui::pos2(rect.left(), ea.y), egui::pos2(rect.right(), ea.y)] };
                        painter.add(egui::Shape::dashed_line(&ligne, egui::Stroke::new(1.0, vert.gamma_multiply(0.6)), 8.0, 3.0));
                        painter.line_segment([ea, fin], egui::Stroke::new(2.0, vert));
                        painter.circle_filled(ea, 3.5, vert);
                        painter.circle_filled(fin, 3.5, vert);
                    }
                }
            }
            if resp.drag_started_by(PointerButton::Primary) && ed.debut.is_none() {
                ed.debut = ui.input(|i| i.pointer.press_origin()).map(|q| viser(q, None).0);
            }
            if resp.drag_stopped_by(PointerButton::Primary) {
                if let (Some(a), Some(q)) = (ed.debut, resp.interact_pointer_pos()) {
                    if assez_long(a, q) {
                        ed.debut = None;
                        poser(ed, a, viser(q, Some(a)).0);
                    }
                }
            } else if resp.clicked() {
                if let Some(q) = resp.interact_pointer_pos() {
                    match ed.debut {
                        Some(a) if assez_long(a, q) => {
                            ed.debut = None;
                            poser(ed, a, viser(q, Some(a)).0);
                        }
                        Some(_) => {}
                        None => {
                            ed.debut = Some(viser(q, None).0);
                            ed.message = Some("Plan de référence : cliquer le second point.".into());
                        }
                    }
                }
            }
        }
        OutilFamille::Volume | OutilFamille::Vide => {
            let vide = ed.outil == OutilFamille::Vide;
            if resp.drag_started_by(PointerButton::Primary) {
                ed.debut = ui.input(|i| i.pointer.press_origin()).map(|q| viser(q, None).0);
            }
            if let (Some(a), Some(q)) = (ed.debut, curseur) {
                let r = boite(a, viser(q, None).0);
                if vide {
                    let pts = [r.left_top(), r.right_top(), r.right_bottom(), r.left_bottom(), r.left_top()];
                    painter.add(egui::Shape::dashed_line(&pts, egui::Stroke::new(1.5, orange), 7.0, 4.0));
                } else {
                    painter.rect(r, 0.0, couleur_materiau(&ed.famille, Materiau::Menuiserie).gamma_multiply(0.6), egui::Stroke::new(1.5, orange), egui::StrokeKind::Inside);
                }
            }
            if resp.drag_stopped_by(PointerButton::Primary) {
                if let (Some(a), Some(q)) = (ed.debut.take(), resp.interact_pointer_pos()) {
                    let b = viser(q, None).0;
                    let accroche = |axe: AxePlan, x: f64| match ed.plan_proche(&r, axe, x) {
                        Some((n, d)) if d.abs() <= tol => Face { plan: n, decalage: 0.0 },
                        Some((n, d)) => Face { plan: n, decalage: d.round() },
                        None => Face { plan: String::new(), decalage: 0.0 },
                    };
                    let axe_p = AxePlan::TOUS[profondeur];
                    let [p0, p1] = ed.faces_profondeur(&r, axe_p, vide);
                    let mut min = [p0.clone(), p0.clone(), p0];
                    let mut max = [p1.clone(), p1.clone(), p1];
                    min[ih] = accroche(ah, a.0.min(b.0));
                    max[ih] = accroche(ah, a.0.max(b.0));
                    min[iv] = accroche(av, a.1.min(b.1));
                    max[iv] = accroche(av, a.1.max(b.1));
                    if (a.0 - b.0).abs() * echelle > 4.0 && (a.1 - b.1).abs() * echelle > 4.0 && min.iter().chain(&max).all(|f| !f.plan.is_empty()) {
                        let nom = nom_libre(|n| ed.famille.volumes.iter().any(|v| v.nom == n), if vide { "Vide" } else { "Volume" });
                        ed.famille.volumes.push(Volume { visibilite: Default::default(), nom, vide, materiau: Materiau::Menuiserie, min, max, visible: None, materiau_parametre: None });
                        ed.selection = SelectionFamille::Volume(ed.famille.volumes.len() - 1);
                        ed.message = Some("Faces verrouillées sur les plans les plus proches (avec décalage au-delà de 7 px).".into());
                    }
                }
            }
        }
        OutilFamille::Texte | OutilFamille::Libelle => {
            if resp.clicked() && let Some(q) = resp.interact_pointer_pos() {
                use modele::documentation::AnnotationForme;
                use modele::famille::modelee::{CategorieFamille, Element2DFamille};
                let (h, v) = viser(q, None).0;
                let texte = if ed.outil == OutilFamille::Texte { "Texte" } else {
                    match ed.famille.categorie { CategorieFamille::Cartouche => "{numero}", CategorieFamille::EtiquettePiece => "{nom}", _ => "Libellé" }
                };
                ed.famille.elements_2d.push(Element2DFamille::fixe(ed.vue.esquisse(), AnnotationForme::Texte { point: [h, v], texte: texte.into() }, 2.5));
                let indice = ed.famille.elements_2d.len() - 1;
                ed.choisir_outil(OutilFamille::Selection);
                ed.selectionner(vec![SelectionFamille::Element2D(indice)]);
            }
        }
        OutilFamille::PlanTroisPoints => {}
        OutilFamille::CheminGuidage => {
            let vise = curseur.filter(|_| resp.hovered()).map(|q| viser(q, ed.modifier.chemin.points.last().copied()).0);
            chemin_trace::canevas_chemin(ui, ed, &painter, &cadre, &resp, vise);
        }
        OutilFamille::Aligner => {
            aligner::canevas_aligner(ed, &painter, &cadre, &r, &resp, curseur);
            garder_multiple = true;
        }
        OutilFamille::Deplacer | OutilFamille::Copier | OutilFamille::Tourner | OutilFamille::Symetrie | OutilFamille::Echelle | OutilFamille::Reseau | OutilFamille::ReseauPolaire => {
            // points cliqués, puis élastique jusqu'au curseur accroché ; tourner et
            // échelle : les deux mesures partent du premier point
            let depuis_centre = ed.transformation_depuis_premier_point();
            if let Some(q) = curseur.filter(|_| resp.hovered()) {
                let c = viser(q, depart_courant).0;
                // sélection telle qu'elle sera après le clic
                for ligne in ed.apercu_transformation(&r, c) {
                    let points: Vec<egui::Pos2> = ligne.into_iter().map(|p| cadre.ecran(p.0, p.1)).collect();
                    if points.len() >= 2 { painter.add(egui::Shape::dashed_line(&points, egui::Stroke::new(1.5, bleu), 6.0, 4.0)); }
                }
                let trait_ = egui::Stroke::new(1.5, orange);
                let points: Vec<egui::Pos2> = ed.points_transfo.iter().map(|p| cadre.ecran(p.0, p.1)).collect();
                let cible = cadre.ecran(c.0, c.1);
                if depuis_centre {
                    for p in points.iter().skip(1).chain(std::iter::once(&cible)) {
                        if let Some(centre) = points.first() { painter.line_segment([*centre, *p], trait_); }
                    }
                } else if let Some(base) = points.last() {
                    painter.add(egui::Shape::dashed_line(&[*base, cible], trait_, 6.0, 4.0));
                }
                for p in &points { painter.circle_filled(*p, 3.5, orange); }
            }
            if resp.clicked_by(PointerButton::Secondary) {
                // clic droit : revenir d'un point, puis quitter l'outil
                if ed.points_transfo.pop().is_none() { ed.choisir_outil(OutilFamille::Selection); }
                else { ed.message = Some(ed.invite_transformation().into()); }
            } else if resp.clicked() && let Some(q) = resp.interact_pointer_pos() {
                ed.clic_transformation(viser(q, depart_courant).0);
            }
            garder_multiple = true;
        }
        OutilFamille::Ligne | OutilFamille::Arc | OutilFamille::Spline | OutilFamille::Cercle | OutilFamille::Rectangle => {
            use moteur2d::dao::{EvenementTrace, Trace};
            use noyau::forme2d::{Forme2D};
            let outil = ed.outil.outil_dessin().expect("outil de dessin");
            let mut trace = ed.trace_polyligne.take().unwrap_or_else(|| Trace::new(outil));
            let depart = ed.esquisse.last().map(|(_, p)| *p);
            if let Some(q) = curseur.filter(|_| resp.hovered()) {
                // angle et distance tapés : appliqués par le moteur d'accrochage
                let (h, v) = viser(q, depart).0;
                let point = glam::DVec2::new(h, v);
                for ligne in trace.apercu(point) {
                    painter.add(egui::Shape::line(ligne.into_iter().map(|p| cadre.ecran(p.x, p.y)).collect(), egui::Stroke::new(1.5, orange)));
                }
                if ed.outil == OutilFamille::Ligne && ferme_profil(ed, &cadre, q) {
                    let premier = cadre.ecran(ed.esquisse[0].1.0, ed.esquisse[0].1.1);
                    painter.circle_stroke(premier, 7.0, egui::Stroke::new(2.0, Color32::from_rgb(30, 140, 60)));
                }
            }
            let clavier = !ui.ctx().egui_wants_keyboard_input();
            let valider_saisie = ed.saisie_trace.clavier(ui.ctx(), trace.precedent().is_some());
            let fermer = clavier && ui.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::F));
            let terminer = resp.clicked_by(PointerButton::Secondary) || (clavier && ui.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::Enter) || i.consume_key(egui::Modifiers::NONE, egui::Key::Space)));
            if !ed.saisie_trace.texte.is_empty() {
                crate::socle::messages::consigne_vue(ui, rect, 32.0, &format!("Point : {} · Entrée pour valider · Tab pour l'angle", ed.saisie_trace.texte));
            }
            let mut point_verrouille = None;
            let evenement = if valider_saisie || (resp.clicked() && !ed.saisie_trace.texte.is_empty()) {
                let precedent = trace.precedent();
                let curseur_modele = curseur.map(|q| { let (h, v) = viser(q, depart).0; glam::DVec2::new(h, v) });
                match ed.saisie_trace.point(precedent, curseur_modele, facteur) {
                    Ok(p) => match (ed.plan_proche(&r, ah, p.x), ed.plan_proche(&r, av, p.y)) {
                        (Some((h, dh)), Some((v, dv))) => {
                            point_verrouille = Some((PointEsquisse { h: Face { plan: h, decalage: dh }, v: Face { plan: v, decalage: dv } }, (p.x, p.y)));
                            Some(EvenementTrace::Point(p))
                        }
                        _ => { ed.message = Some("Aucun plan de référence disponible pour ce point.".into()); None }
                    },
                    Err(e) => { ed.message = Some(e); None }
                }
            } else if fermer { Some(EvenementTrace::Fermer) }
                else if terminer { Some(EvenementTrace::Terminer) }
                else if resp.clicked() {
                    resp.interact_pointer_pos().and_then(|q| {
                        if ed.outil == OutilFamille::Ligne && ferme_profil(ed, &cadre, q) { return Some(EvenementTrace::Fermer); }
                        let (h, v) = viser(q, depart).0;
                        match (ed.accrocher(&r, ah, h, tol), ed.accrocher(&r, av, v, tol)) {
                            (Some(fh), Some(fv)) => {
                                let point = PointEsquisse { h: fh, v: fv };
                                let (h, v) = ed.famille.point_esquisse(&r, &point);
                                point_verrouille = Some((point, (h, v)));
                                Some(EvenementTrace::Point(glam::DVec2::new(h, v)))
                            }
                            _ => None,
                        }
                    })
                } else { None };
            if let Some(evenement) = evenement {
                let avant = trace.points.len();
                let formes = trace.appliquer(evenement);
                if trace.points.len() > avant {
                    if let Some(point) = point_verrouille { ed.esquisse.push(point); }
                }
                if let Some(forme) = formes.into_iter().next() {
                    if let (OutilFamille::Ligne, Forme2D::Polyligne { fermee, .. }) = (ed.outil, &forme) {
                        let points = std::mem::take(&mut ed.esquisse).into_iter().map(|(p, _)| p).collect();
                        ed.ajouter_polyligne_ancree(points, *fermee);
                    } else {
                        ed.esquisse.clear();
                        ed.famille.elements_2d.push(modele::famille::modelee::Element2DFamille::fixe(ed.vue.esquisse(), modele::documentation::AnnotationForme::depuis_forme_2d(&forme), 2.5));
                        ed.element_2d_cree();
                    }
                } else if matches!(evenement, EvenementTrace::Terminer | EvenementTrace::Fermer) && trace.points.is_empty() {
                    ed.esquisse.clear();
                }
            }
            ed.trace_polyligne = Some(trace);
        }
        OutilFamille::Extrusion | OutilFamille::ExtrusionVide | OutilFamille::Revolution | OutilFamille::RevolutionVide | OutilFamille::ExtrusionChemin | OutilFamille::Raccordement | OutilFamille::RaccordementChemin | OutilFamille::CoteAngle | OutilFamille::CoteDiametre | OutilFamille::Altimetrie => {
            // sommet accroché : sur chaque axe, le plan le plus proche (décalage au-delà
            // de 7 px)
            let accroche = |axe: AxePlan, x: f64| match ed.plan_proche(&r, axe, x) {
                Some((n, d)) if d.abs() <= tol => (Face { plan: n, decalage: 0.0 }, x - d),
                Some((n, d)) => (Face { plan: n, decalage: d.round() }, x - d + d.round()),
                None => (Face { plan: String::new(), decalage: 0.0 }, x),
            };
            let depart = ed.esquisse.last().map(|(_, p)| *p);
            let point = |q: egui::Pos2| {
                let (h, v) = viser(q, depart).0;
                let ((fh, h), (fv, v)) = (accroche(ah, h), accroche(av, v));
                (PointEsquisse { h: fh, v: fv }, (h, v))
            };
            let profil = ed.outil.est_forme();
            let premier = ed.esquisse.first().map(|(_, p)| cadre.ecran(p.0, p.1));
            // élastique et repère d'accrochage
            if let Some(q) = curseur.filter(|_| resp.hovered()) {
                let (_, c) = point(q);
                let mesure = ed.outil.cote_mesure();
                // cote de mesure complète sous le curseur
                let mut modele: Vec<glam::DVec2> = ed.esquisse.iter().map(|(_, p)| glam::DVec2::new(p.0, p.1)).collect();
                modele.push(glam::DVec2::new(c.0, c.1));
                if let Some(g) = mesure.filter(|g| modele.len() == g.points()).and_then(|g| geometrie_mesure(g, ed.vue.esquisse(), &modele, echelle)) {
                    for [a, b] in &g.traits { painter.line_segment([cadre.ecran(a.x, a.y), cadre.ecran(b.x, b.y)], egui::Stroke::new(1.5, orange)); }
                    painter.text(cadre.ecran(g.texte.0.x, g.texte.0.y), egui::Align2::CENTER_BOTTOM, &g.texte.2, egui::FontId::proportional(11.0), orange);
                }
                let c = cadre.ecran(c.0, c.1);
                let mut pts: Vec<egui::Pos2> = ed.esquisse.iter().map(|(_, p)| cadre.ecran(p.0, p.1)).collect();
                pts.push(c);
                if mesure.is_none_or(|g| pts.len() < g.points()) {
                    painter.add(egui::Shape::line(pts, egui::Stroke::new(1.5, orange)));
                }
                if let Some(p0) = premier.filter(|p| profil && ed.esquisse.len() >= 3 && p.distance(q) < 9.0) {
                    painter.circle_stroke(p0, 7.0, egui::Stroke::new(2.0, Color32::from_rgb(30, 140, 60)));
                }
            }
            let clavier = !ui.ctx().egui_wants_keyboard_input();
            let mut finir = resp.clicked_by(PointerButton::Secondary) || (clavier && ui.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::Enter)));
            if resp.clicked() {
                if let Some(q) = resp.interact_pointer_pos() {
                    if profil && ferme_profil(ed, &cadre, q) {
                        finir = true;
                    } else {
                        let p = point(q);
                        if p.0.h.plan.is_empty() || p.0.v.plan.is_empty() {
                            ed.message = Some("Ajouter d'abord des plans de référence dans cette vue.".into());
                        } else {
                            ed.esquisse.push(p);
                            if ed.capter_axe_revolution() { return; }
                            if let Some(g) = ed.outil.cote_mesure().filter(|g| ed.esquisse.len() < g.points()) {
                                ed.message = Some(g.invite(ed.esquisse.len()).into());
                            }
                        }
                    }
                }
                if ed.outil.cote_mesure().is_some_and(|g| ed.esquisse.len() >= g.points()) {
                    finir = true;
                }
            }
            if finir && ed.outil.est_forme() {
                ed.erreur = ed.terminer_forme(None).err();
                return;
            }
            if finir {
                let points: Vec<PointEsquisse> = std::mem::take(&mut ed.esquisse).into_iter().map(|(p, _)| p).collect();
                let vue = ed.vue.esquisse();
                match ed.outil {
                    o if o.cote_mesure().is_some_and(|g| points.len() == g.points()) => {
                        let genre = o.cote_mesure().unwrap();
                        let modele: Vec<glam::DVec2> = points.iter().map(|p| { let (h, v) = ed.famille.point_esquisse(&r, p); glam::DVec2::new(h, v) }).collect();
                        if geometrie_mesure(genre, vue, &modele, echelle).is_some() {
                            ed.famille.cotes_mesure.push(CoteMesure { vue, genre, points });
                            ed.selection = SelectionFamille::CoteMesure(ed.famille.cotes_mesure.len() - 1);
                            ed.message = Some(format!("Cote ({}) créée : ses points suivent les plans de référence.", genre.nom().to_lowercase()));
                        } else {
                            ed.message = Some(if genre == GenreCote::Altimetrie && vue == VueEsquisse::Plan { "Altimétrie : ouvrir une élévation ou une coupe." } else { "Cote impossible : points confondus ou alignés." }.into());
                        }
                    }
                    _ => ed.message = Some("Esquisse abandonnée : pas assez de points.".into()),
                }
            }
        }
        OutilFamille::Cote | OutilFamille::Egalite => {
            let n = if ed.outil == OutilFamille::Cote { 2 } else { 3 };
            // plan visé : coloré (rouge s'il n'est pas parallèle aux plans déjà choisis),
            // et cote provisoire depuis le dernier plan choisi
            let axe_choisi = ed.clics.first().and_then(|p| ed.famille.plan(p)).map(|p| p.axe);
            if let Some((q, nom)) = curseur.filter(|_| resp.hovered()).and_then(|q| plan_vise(q).map(|n| (q, n))) {
                if let Some((_, vertical, c)) = lignes.iter().find(|l| l.0 == nom) {
                    let parallele = axe_choisi.is_none() || ed.famille.plan(&nom).map(|p| p.axe) == axe_choisi;
                    let couleur = if parallele { Color32::from_rgb(0, 160, 230) } else { Color32::from_rgb(210, 40, 40) };
                    let (a, b) = if *vertical { (egui::pos2(*c, rect.top()), egui::pos2(*c, rect.bottom())) } else { (egui::pos2(rect.left(), *c), egui::pos2(rect.right(), *c)) };
                    painter.line_segment([a, b], egui::Stroke::new(3.0, couleur.gamma_multiply(0.35)));
                    painter.line_segment([a, b], egui::Stroke::new(1.5, couleur));
                    let precedent = ed.clics.last().filter(|p| **p != nom).and_then(|p| lignes.iter().find(|l| &l.0 == p));
                    if let Some((_, _, c0)) = precedent.filter(|_| parallele) {
                        let (p0, p1) = if *vertical { (egui::pos2(*c0, q.y), egui::pos2(*c, q.y)) } else { (egui::pos2(q.x, *c0), egui::pos2(q.x, *c)) };
                        let trait_ = egui::Stroke::new(1.0, couleur);
                        painter.line_segment([p0, p1], trait_);
                        let t = if *vertical { egui::vec2(0.0, 5.0) } else { egui::vec2(5.0, 0.0) };
                        for p in [p0, p1] {
                            painter.line_segment([p - t, p + t], trait_);
                        }
                        let valeur = (pos(&nom) - pos(&ed.clics[ed.clics.len() - 1])).abs();
                        painter.text(p0.lerp(p1, 0.5) - t * 2.0, egui::Align2::CENTER_BOTTOM, modele::unite::unite().afficher(valeur), police.clone(), couleur);
                    }
                }
            }
            // cote : après les deux plans, le troisième clic place la ligne de cote
            // (méthode commune : deux références, puis position)
            let position = ed.outil == OutilFamille::Cote && ed.clics.len() == 2;
            let horizontale = ed.clics.first().and_then(|p| ed.famille.plan(p)).is_some_and(|p| p.axe == ah);
            let ligne_cote = |q: egui::Pos2| { let (h, v) = cadre.modele(q); if horizontale { v } else { h } };
            if position && let Some(q) = curseur.filter(|_| resp.hovered()) {
                let ligne = ligne_cote(q);
                let mut xs: Vec<f64> = ed.clics.iter().map(|n| pos(n)).collect();
                xs.sort_by(f64::total_cmp);
                let attache = 10.0 / echelle;
                let points: Vec<glam::DVec2> = xs.iter().map(|&x| if horizontale { glam::DVec2::new(x, ligne - attache) } else { glam::DVec2::new(ligne + attache, x) }).collect();
                for [a, b] in modele::documentation::geometrie_cote(&points, attache, 8.0 / echelle).traits {
                    painter.line_segment([cadre.ecran(a.x, a.y), cadre.ecran(b.x, b.y)], egui::Stroke::new(1.5, orange));
                }
            }
            if resp.clicked() && position {
                if let Some(q) = resp.interact_pointer_pos() {
                    let p = std::mem::take(&mut ed.clics);
                    let (a, b) = if pos(&p[0]) <= pos(&p[1]) { (p[0].clone(), p[1].clone()) } else { (p[1].clone(), p[0].clone()) };
                    let v = (pos(&b) - pos(&a)).round();
                    ed.famille.contraintes.push(Contrainte::Distance { a, b, valeur: format!("{v}"), priorite: 5, ligne: Some(ligne_cote(q)) });
                    ed.selection = SelectionFamille::Contrainte(ed.famille.contraintes.len() - 1);
                    ed.message = Some("Cote créée : la libeller par un paramètre dans Contraintes.".into());
                }
            } else if resp.clicked() {
                if let Some(nom) = resp.interact_pointer_pos().and_then(plan_vise) {
                    let axe = ed.famille.plan(&nom).map(|p| p.axe);
                    if ed.clics.first().and_then(|p| ed.famille.plan(p)).is_some_and(|p| Some(p.axe) != axe) {
                        ed.message = Some("Choisir des plans parallèles.".into());
                    } else if !ed.clics.contains(&nom) {
                        ed.clics.push(nom);
                    }
                }
                if ed.clics.len() == n && n == 3 {
                    let p = std::mem::take(&mut ed.clics);
                    ed.famille.contraintes.push(Contrainte::Egalite { a: p[0].clone(), m: p[1].clone(), b: p[2].clone(), priorite: 5, ligne: None });
                    ed.selection = SelectionFamille::Contrainte(ed.famille.contraintes.len() - 1);
                    ed.message = Some("Égalité créée.".into());
                } else if n == 2 {
                    ed.message = Some(GenreCote::Lineaire.invite(ed.clics.len()).replace("point à coter", "plan à coter").into());
                } else {
                    ed.message = Some(format!("{} plan(s) sur {n}.", ed.clics.len()));
                }
            }
        }
    }
    if !garder_multiple && ed.selection != selection_avant {
        ed.multiple.clear();
    }
}

/// Géométrie d'une cote de mesure de famille (repère de la vue, mm), par la méthode
/// commune (`GenreCote`) ; `echelle` : px par mm, pour des traits et un écart de texte
/// constants à l'écran. Altimétrie : altitude depuis le bas de la baie, seulement en
/// élévation et en coupe.
pub(super) fn geometrie_mesure(genre: GenreCote, vue: VueEsquisse, pts: &[glam::DVec2], echelle: f64) -> Option<modele::documentation::GeometrieMesure> {
    use modele::documentation as doc;
    if genre == GenreCote::Altimetrie && vue == VueEsquisse::Plan { return None; }
    let forme = genre.forme(pts, false, 2)?;
    let unite = modele::unite::unite();
    let longueur = |x: f64| unite.afficher(x);
    let altitude = |q: [f64; 2], _| q[1];
    let style = doc::StyleCote { oblique: 8.0 / echelle, taille: 11.0 / echelle, longueur: &longueur, altitude: &altitude };
    let (traits, mut textes) = doc::geometrie(&forme, None, &style)?;
    Some(doc::GeometrieMesure { traits, texte: textes.pop()? })
}
