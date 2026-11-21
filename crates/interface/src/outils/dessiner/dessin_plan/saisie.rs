//! Saisie des points, clavier, annulation et fin de commande.
use commandes::Execution;
use super::*;

use crate::saisie::saisie_trace::{ajouter_caractere_saisie, effacer_caractere_saisie};

#[cfg(test)]
use crate::saisie::reperage::directions_au_point;

impl Application {
    /// Point retenu pour le curseur `c` : accrochage, sinon ortho (Maj) ou repérage
    /// polaire (tous les 15°) depuis le point précédent, sinon le point libre.
    pub(crate) fn point_dessin(&mut self, rect: egui::Rect, c: DVec2, maj: bool, temps: f64) -> (DVec2, Accroche) {
        // arc en attente de sa courbure : le curseur reste libre (point de passage, ou
        // côté du rayon tapé), sans repérage depuis le dernier point
        let precedent=self.dessin2d.trace.as_ref().filter(|t| !t.attend_courbure()).and_then(Trace::precedent);
        self.point_dessin_depuis(rect,c,maj,temps,precedent)
    }

    pub(crate) fn point_dessin_depuis(&mut self, rect:egui::Rect,c:DVec2,maj:bool,temps:f64,precedent:Option<DVec2>)->(DVec2,Accroche) {
        use crate::saisie::moteur_accrochage::{viser, Geometrie, Visee};
        // tolérance : 12 pixels en mm du plan de la vue
        let tol = self.vers_ecran(rect, c).zip(self.vers_ecran(rect, c + DVec2::X * 1000.0)).map_or(200.0, |(a, b)| crate::saisie::moteur_accrochage::TOLERANCE_PX * 1000.0 / (a.distance(b) as f64).max(1e-6));
        let source = Geometrie { geoms: self.geoms_trace(), droites: Vec::new() };
        let visee = Visee { tol, precedent, maj, temps, delai: self.options.delai_accrochage, angle_axes: self.angle_axes_plan(), trace_actif: self.dessin2d.trace.is_some(), saisie: &self.dessin2d.saisie, facteur: modele::unite::unite().facteur(), pas: 10.0 };
        viser(&mut self.dessin2d.reperage, &source, c, &visee)
    }

    /// Source d'accrochage des vues du projet : géométrie de la vue, tracé en cours et
    /// contours en édition.
    fn geoms_trace(&mut self) -> Vec<Geom> {
        let mut geoms = self.geoms_dessin();
        // vue d'un motif de hachures : le cadre du carré (coins, milieux, bords)
        if let Some(c) = self.cadre_motif() {
            geoms.extend((0..4).map(|i| Geom::Segment([DVec2::from_array(c[i]), DVec2::from_array(c[(i + 1) % 4])])));
        }
        if let Some(t) = &self.dessin2d.trace {
            let k = t.courbures();
            geoms.extend(t.points.windows(2).enumerate().map(|(i, w)| noyau::forme2d::geom_segment(w[0], w[1], k.get(i).copied().unwrap_or(0.0))));
            if let [p] = t.points.as_slice() {
                geoms.push(Geom::Segment([*p, *p]));
            }
        }
        if let Some(e) = self.dessin2d.edition_contour.as_ref() {
            for ci in 0..self.dessin2d.dalle.len() {
                let fermee = e.fermee(ci);
                geoms.extend(self.dessin2d.segments_contour(ci, fermee).into_iter().map(|(a, b, k)| noyau::forme2d::geom_segment(a, b, k)));
            }
        }
        geoms
    }

    /// Enregistre les formes achevées : annotations de la vue, ou murs (une étape
    /// d'annulation par forme ; le dernier mur créé est sélectionné).
    pub(crate) fn enregistrer_formes(&mut self, formes: Vec<noyau::forme2d::Forme2D>) {
        // cotation : le tracé ne sert qu'à recueillir les points ; Entrée ou clic droit
        // avant le dernier point abandonne la cote en cours
        if self.outil_cotation().is_some() {
            if !formes.is_empty() && let Some(t) = self.dessin2d.trace.as_mut() { t.points.clear(); }
            return;
        }
        if formes.is_empty() {
            return;
        }
        if self.usage_dessin().is_some_and(|(u, _)| u == Usage::Poutre) {
            self.creer_poutres(&formes);
            return;
        }
        if self.usage_dessin().is_some_and(|(u, _)| u == Usage::Escalier || u == Usage::GardeCorps) {
            let escalier = self.usage_dessin().is_some_and(|(u, _)| u == Usage::Escalier);
            for f in &formes {
                let noyau::forme2d::Forme2D::Polyligne { points, .. } = f else { continue };
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
                match modele::creation::contour_valide(&c) {
                    Err(e) => self.dessin2d.message = Some(e),
                    Ok(c) if self.dessin2d.dalle.iter().any(|x| modele::creation::croisement(x, &c)) => {
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
                        { self.dessin2d.message = None; self.dessin2d.information = None; };
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
                            self.selected = Some(modele::identifiants::id_natif(genre, id));
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
        let mut crees = 0;
        for f in formes {
            let mut forme = AnnotationForme::depuis_forme_2d(&f);
            // polyligne, spline, cercle : type de trait choisi à la création (tous les segments)
            if !self.dessin2d.trait_creation.is_empty() {
                crate::outils::annoter::types_traits::definir_trait(&mut forme, None, &self.dessin2d.trait_creation.clone());
            }
            let annotation = Annotation { id: String::new(), vue: vue.clone(), hauteur: 2.5, forme };
            match self.executer_bim(Commande::Annoter { annotation }) {
                Ok(_) => crees += 1,
                Err(e) => self.error = Some(e),
            }
        }
        if crees > 0 {
            self.informer(if crees == 1 { "Dessin enregistré".into() } else { format!("{crees} dessins enregistrés") });
        }
        self.bim.selection.clear();
    }

    /// Changement de commande (outil du ruban) : la précédente s'arrête et son tracé en
    /// cours est abandonné ; les modes propres à un outil sont remis à zéro. Appelé à chaque
    /// image, et par le pilote juste après avoir activé un outil (ses réglages suivants,
    /// comme « Suivre un trait », ne doivent pas être remis à zéro à l'image d'après).
    pub(crate) fn changer_commande(&mut self) {
        let cle = self.ruban_edition.outil();
        if cle != self.dessin2d.commande_cle || (cle.is_none() && self.dessin2d.historique_depart.is_some()) {
            // nouvelle commande : la précédente s'arrête, son tracé en cours est abandonné
            if cle.is_some() && self.dessin2d.commande_cle.is_some()
                && (self.dessin2d.edition_contour.as_ref().is_some_and(|e| e.creation.is_some()) || self.dessin2d.trace.as_ref().is_some_and(|t| !t.points.is_empty()))
            {
                self.dessin2d.vider_contours();
                self.reinitialiser_terrain();
                self.dessin2d.edition_contour = None;
                self.dessin2d.geoms = None;
                self.dessin2d.saisie.clear();
                { self.dessin2d.message = None; self.dessin2d.information = None; };
                self.dessin2d.trace = None;
                self.bim.depart_structure = None;
                self.coupe_depart = None;
                self.visibility_dirty = true;
                self.coupe_dirty = true;
                self.status = "Commande précédente arrêtée.".into();
            }
            self.dessin2d.commande_cle = cle;
            // un texte ouvert par double-clic active l'outil Texte : il est gardé
            if cle != Some(crate::zones::ruban::Outil::Texte) { self.bim.texte_saisi = None; }
            // le mode « Suivre un trait » ne passe pas d'un outil à l'autre
            self.ruban_edition.suivre_trait = false;
            if cle != Some(crate::zones::ruban::Outil::Escalier) { self.dessin2d.giron_impose = None; }
            self.dessin2d.historique_depart = cle.map(|_| self.profondeur_historique());
        }
    }

    /// Clavier du dessin 2D, lu avant les raccourcis : chiffres et `@ < , ; . -` dans la
    /// ligne de commande, F fermer, Entrée ou Espace valider (vide : terminer),
    /// Retour arrière, Échap (saisie, puis tracé en cours, puis outil).
    pub(crate) fn clavier_dessin(&mut self, ctx: &egui::Context) {
        let segments = if self.usage_dessin().is_some_and(|(u, _)| u == Usage::Escalier) { self.dessin2d.trace.as_ref().map_or(0, |t| t.points.len().saturating_sub(1)).max(self.dessin2d.edition_contour.as_ref().filter(|e| e.creation == Some(Usage::Escalier)).map_or(0, |_| self.dessin2d.dalle.first().map_or(0, |c| c.len().saturating_sub(1)))) } else { 0 };
        let courante = self.dessin2d.justification_escalier;
        self.dessin2d.just_segments.truncate(segments);
        while self.dessin2d.just_segments.len() < segments { self.dessin2d.just_segments.push(courante); }
        self.changer_commande();
        if self.suivi_trait_actif() {
            let usage = self.usage_dessin().map_or(Usage::Mur, |(u, _)| u);
            if self.dessin2d.dernier_usage != Some(usage) { self.dessin2d.vider_contours(); }
            self.dessin2d.dernier_usage = Some(usage);
            // Suivre un trait : Ctrl seul change l'implantation suivie, Espace inverse le
            // côté ; l'aperçu (apercu_volume_murs) les relit à l'image suivante.
            if usage == Usage::Mur && !ctx.egui_wants_keyboard_input() {
                let (justifier, inverser) = super::suivre_trait::raccourcis(ctx, &mut self.dessin2d.controle_seul);
                if justifier { self.ruban_edition.implantation_suivante(); }
                if inverser { self.ruban_edition.sens_inverse = !self.ruban_edition.sens_inverse; }
                if justifier || inverser { ctx.request_repaint(); }
            }
            return;
        }
        if self.poignee_axe.is_some() || self.dessin2d.arc_glisse.is_some() {
            if !ctx.egui_wants_keyboard_input() {
                ctx.input_mut(|i| i.events.retain(|e| match e {
                    egui::Event::Text(texte) if texte.chars().all(|c| c.is_ascii_digit() || matches!(c, ',' | '.')) => {
                        self.dessin2d.saisie.push_str(texte);
                        false
                    }
                    egui::Event::Key { key: egui::Key::Backspace, pressed: true, .. } => { self.dessin2d.saisie.pop(); false }
                    _ => true,
                }));
            }
            return;
        }
        let usage_dessin = self.usage_dessin();
        if !usage_dessin.is_some_and(|(usage, _)| usage == Usage::LimitePropriete) { self.bim.altitude_limite = None; }
        let Some((usage, outil)) = usage_dessin else {
            self.dessin2d.dernier_usage = None;
            self.dessin2d.trace = None;
            self.dessin2d.reperage.oublier();
            self.dessin2d.saisie.clear();
            self.dessin2d.vider_contours();
            self.reinitialiser_terrain();
            return;
        };
        if self.dessin2d.dernier_usage.is_some_and(|u| u != usage) {
            self.dessin2d.vider_contours();
            self.reinitialiser_terrain();
            self.dessin2d.trace = None;
        }
        self.dessin2d.dernier_usage = Some(usage);
        if self.dessin2d.trace.as_ref().is_none_or(|t| t.outil != outil) {
            self.dessin2d.reperage.oublier();
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
        let mut justifier = false;
        let mut echap = false;
        let mut effacer = false;
        let mut supprimer_trait = false;
        let mut supprimer_contour = false;
        // F (fermer) seulement pendant un tracé : sinon F cadre la vue
        let en_cours = self.trace_en_cours();
        let tab_vue = en_cours && self.tab_dans_vue(ctx);
        let mut tab_utilise = false;
        ctx.input_mut(|i| {
            i.events.retain(|e| match e {
                egui::Event::Key { key: egui::Key::ControlLeft | egui::Key::ControlRight, pressed: true, repeat: false, .. } => {
                    self.dessin2d.controle_seul = true;
                    true
                }
                egui::Event::Key { key: egui::Key::ControlLeft | egui::Key::ControlRight, pressed: false, .. } => {
                    if self.dessin2d.controle_seul { justifier = true; }
                    self.dessin2d.controle_seul = false;
                    true
                }
                egui::Event::Key { pressed: true, modifiers, .. } if modifiers.ctrl => { self.dessin2d.controle_seul = false; true }
                egui::Event::Key { key: egui::Key::Tab, pressed: true, modifiers, .. }
                    if tab_vue && modifiers.is_none() && !self.dessin2d.saisie.starts_with(['@', '#']) => {
                    tab_utilise = true;
                    self.dessin2d.champ_angle = !self.dessin2d.champ_angle;
                    if self.dessin2d.champ_angle && !self.dessin2d.saisie.contains(['<', '>']) { self.dessin2d.saisie.push('<'); }
                    false
                }
                egui::Event::Text(t) => {
                    let mut lu = false;
                    for ch in t.chars() {
                        if ch.is_ascii_digit() || "@#,;.-".contains(ch) || (en_cours && ch.eq_ignore_ascii_case(&'f')) {
                            ajouter_caractere_saisie(&mut self.dessin2d.saisie, &mut self.dessin2d.champ_angle, ch);
                            { self.dessin2d.message = None; self.dessin2d.information = None; };
                            lu = true;
                        } else if ch == ' ' {
                            // Espace inverse le sens d'un mur ; ailleurs, il valide comme Entrée
                            if usage == Usage::Mur || usage == Usage::Poutre { inverser = true } else if usage == Usage::Escalier { justifier = true } else { valider = true }
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
                // Suppr : le trait sélectionné ; Maj+Suppr : son contour entier
                egui::Event::Key { key: egui::Key::Delete, pressed: true, modifiers, .. } if self.dessin2d.edition_contour.is_some() => { if modifiers.shift { supprimer_contour = true } else { supprimer_trait = true }; false },
                egui::Event::Key { key: egui::Key::Delete, .. } => !en_cours,
                egui::Event::Key { key: egui::Key::F, .. } => !en_cours,
                _ => true,
            });
        });
        if tab_utilise {
            ctx.memory_mut(|m| m.move_focus(egui::FocusDirection::None));
        }
        if supprimer_trait { self.supprimer_trait_edition(); }
        if supprimer_contour { self.supprimer_contour_edition(); }
        if effacer {
            effacer_caractere_saisie(&mut self.dessin2d.saisie, self.dessin2d.champ_angle);
        }
        if echap {
            { self.dessin2d.message = None; self.dessin2d.information = None; };
            if let Some(e) = self.dessin2d.edition_contour.as_mut().filter(|e| e.commande.is_some()) {
                e.commande = None; e.depart = None; e.repere = None;
            } else if !self.dessin2d.saisie.is_empty() {
                self.dessin2d.saisie.clear();
                self.dessin2d.champ_angle = false;
            } else if self.dessin2d.trace.as_ref().is_some_and(|t| !t.points.is_empty()) {
                // Un raccord reste dans le contour commencé, même après un nouvel Échap.
                self.terminer_raccord_ouvert();
                // Échap interrompt le tracé en cours ; ce qui est déjà dessiné reste
                if usage != Usage::Mur && self.dessin2d.edition_contour.is_none() && !matches!(usage, Usage::Escalier | Usage::GardeCorps | Usage::Annotation) {
                    self.interrompre_trace(usage);
                } else if self.dessin2d.edition_contour.is_none() && matches!(usage, Usage::Escalier | Usage::GardeCorps | Usage::Annotation)
                    && outil == OutilDessin::Polyligne && let Some((points, k)) = self.dessin2d.trace.as_ref().filter(|t| t.points.len() >= 2).map(|t| (t.points.clone(), t.courbures())) {
                    self.dessin2d.pousser_ouvert(points, k);
                } else if self.dessin2d.edition_contour.as_ref().is_some_and(|e| e.creation.is_some() && e.raccord.is_none())
                    && let Some((points, k)) = self.dessin2d.trace.as_ref().filter(|t| t.outil == OutilDessin::Polyligne && t.points.len() >= 2).map(|t| (t.points.iter().map(|p| p.to_array()).collect(), t.courbures())) {
                    self.dessin2d.pousser_contour(points, k);
                    if let Some(e) = self.dessin2d.edition_contour.as_mut() { e.fermees.push(false); }
                }
                if let Some(t) = self.dessin2d.trace.as_mut() { t.points.clear(); }
                // dessin 2D : ce qui est tracé reste et l'outil se ferme, la sélection
                // est de nouveau possible
                if usage == Usage::Annotation && outil != OutilDessin::Polyligne && self.dessin2d.edition_contour.is_none() {
                    self.ruban_edition.annuler();
                    self.dessin2d.trace = None;
                }
                if let Some(e) = &mut self.dessin2d.edition_contour { e.raccord = None; e.dessin = false; }
                if self.dessin2d.edition_contour.is_none() && (matches!(usage, Usage::Surface | Usage::LimitePropriete | Usage::Terrain | Usage::Cadrage) || (matches!(usage, Usage::Escalier | Usage::GardeCorps | Usage::Annotation) && outil == OutilDessin::Polyligne && self.outil_cotation().is_none())) {
                    self.editer_contours_en_creation(usage);
                }
            } else if let Some(e) = self.dessin2d.edition_contour.as_mut()
                && (e.point_topo.is_some() || e.segment.is_some() || e.sommet.is_some() || e.selection.is_some() || e.dessin)
            {
                // premier Échap : retour à la sélection des segments et des sommets
                e.point_topo = None;
                (e.segment, e.sommet, e.selection, e.dessin) = (None, None, None, false);
            } else if let Some(e) = self.dessin2d.edition_contour.as_ref() {
                // tracé modifié : un premier Échap prévient, un second (dans les 4 s) abandonne
                let temps = ctx.input(|i| i.time);
                let modifie = e.initial != self.dessin2d.dalle;
                if !modifie || self.dessin2d.echap_abandon.is_some_and(|t| temps - t <= 4.0) {
                    self.dessin2d.echap_abandon = None;
                    self.quitter_commande(false);
                } else {
                    self.dessin2d.echap_abandon = Some(temps);
                    self.dessin2d.message = Some("Tracé modifié : Entrée pour valider, Échap à nouveau pour l'abandonner.".into());
                }
            } else if matches!(usage, Usage::Surface | Usage::LimitePropriete | Usage::Terrain | Usage::Cadrage) && !(self.dessin2d.dalle.is_empty() && self.dessin2d.ouverts.is_empty()) {
                self.editer_contours_en_creation(usage);
            } else {
                self.ruban_edition.annuler();
                self.dessin2d.trace = None;
            }
        }
        if usage == Usage::Poutre { self.raccourci_poutre(justifier, inverser); }
        if justifier {
            if usage == Usage::Mur { self.ruban_edition.implantation_suivante(); }
            else if usage == Usage::Escalier { self.dessin2d.justification_escalier = (self.dessin2d.justification_escalier + 1) % 3; }
        }
        if inverser && usage == Usage::Mur {
            self.ruban_edition.sens_inverse = !self.ruban_edition.sens_inverse;
        }
        if valider {
            // commande de Modifier sur le tracé : Entrée applique la valeur tapée
            let commande = self.dessin2d.edition_contour.as_ref().filter(|e| !e.dessin).and_then(|e| e.commande);
            if let (Some(commande), false) = (commande, self.dessin2d.saisie.is_empty()) {
                if let Some((p, _)) = self.dessin2d.point {
                    self.clic_commande_contour(egui::Rect::NOTHING, commande, p, None);
                }
            // une valeur tapée donne un point ; sinon Entrée valide la commande
            } else if self.dessin2d.saisie.is_empty() {
                self.quitter_commande(true);
            } else {
                self.valider_saisie_dessin();
            }
        }
    }

    pub(super) fn editer_contours_en_creation(&mut self, usage: Usage) {
        let altitude = self.altitude_trace();
        let mut fermees = vec![true; self.dessin2d.dalle.len()];
        let courbures = std::mem::take(&mut self.dessin2d.ouverts_courbures);
        for (i, contour) in std::mem::take(&mut self.dessin2d.ouverts).into_iter().enumerate() {
            self.dessin2d.pousser_contour(contour.into_iter().map(|p| p.to_array()).collect(), courbures.get(i).cloned().unwrap_or_default());
            fermees.push(false);
        }
        if usage == Usage::Terrain && let Some(contour) = self.dessin2d.dalle.first() {
            let ngf = self.natif.as_ref().map_or(0.0, |(p, _)| p.ngf()) + altitude;
            self.dessin2d.terrain_cotes.resize(contour.len(), ngf);
        }
        self.dessin2d.edition_contour = Some(EditionContour { id: String::new(), creation: Some(usage), altitude, dessin: false, selection: None, segment: None, sommet: None, point_topo: None, commande: None, depart: None, repere: None, glisse: None, annotation: None, fermees, raccord: None, element: None, initial: Vec::new() });
    }

    pub(crate) fn selectionner_trace_en_creation(&mut self) {
        if let Some((usage @ (Usage::Surface | Usage::LimitePropriete | Usage::Terrain | Usage::Cadrage | Usage::Escalier | Usage::GardeCorps | Usage::Annotation), outil)) = self.usage_dessin() {
            if matches!(usage, Usage::Escalier | Usage::GardeCorps | Usage::Annotation) {
                if outil != OutilDessin::Polyligne || self.outil_cotation().is_some() { return; }
                if let Some((points, k)) = self.dessin2d.trace.as_ref().filter(|t| t.points.len() >= 2).map(|t| (t.points.clone(), t.courbures())) { self.dessin2d.pousser_ouvert(points, k); }
            } else { self.interrompre_trace(usage); }
            if let Some(t) = &mut self.dessin2d.trace { t.points.clear(); }
            self.editer_contours_en_creation(usage);
        }
    }

    /// Profondeur de la pile d'annulation du document actif.
    pub(crate) fn profondeur_historique(&self) -> usize {
        match &self.collaboration {
            Some(c) => c.copie.annulations.len(),
            None => self.historique.libelles(false).len(),
        }
    }

    /// Croix Annuler : défait ce que la commande a créé depuis son démarrage, puis la quitte.
    pub(crate) fn annuler_commande(&mut self) {
        if self.dessin2d.edition_contour.is_none() && let Some(depart) = self.dessin2d.historique_depart {
            let n = self.profondeur_historique().saturating_sub(depart);
            if n > 0 {
                if self.collaboration.is_some() {
                    for _ in 0..n { self.parcourir_historique_n(false, 1); }
                } else {
                    self.parcourir_historique_n(false, n);
                }
            }
        }
        self.quitter_commande(false);
    }

    /// Échap pendant un tracé : le tracé s'arrête là où il en est. Un contour ouvert
    /// d'au moins trois sommets est fermé et conservé ; les autres tracés se terminent
    /// comme avec Entrée.
    pub(crate) fn interrompre_trace(&mut self, usage: Usage) {
        if matches!(usage, Usage::Surface | Usage::Terrain | Usage::LimitePropriete | Usage::Cadrage) {
            // le contour reste ouvert, au stade où il en était
            if let Some(points) = self.dessin2d.trace.as_ref().filter(|t| t.outil == OutilDessin::Polyligne && t.points.len() >= 2).map(|t| t.points.clone()) {
                self.dessin2d.pousser_ouvert(points, Vec::new());
                { self.dessin2d.message = None; self.dessin2d.information = None; };
                self.dessin2d.information = Some("Contour laissé ouvert : double-cliquer une extrémité pour le reprendre.".into());
            }
        } else {
            self.valider_saisie_dessin();
        }
    }

    /// Quitte la commande en cours. Valider : le tracé en cours est terminé (comme Entrée),
    /// puis l'outil se ferme ; Annuler : tout ce qui est en cours est abandonné, puis
    /// l'outil se ferme.
    pub(crate) fn quitter_commande(&mut self, valider: bool) {
        // texte en cours de saisie : créé par Valider, abandonné par Annuler
        if valider && let Err(e) = self.valider_texte_saisi() {
            self.error = Some(e);
        }
        self.bim.texte_saisi = None;
        if valider && self.transfo.outil == Some(crate::zones::ruban::Outil::Scinder) && self.transfo.element.is_some()
            && !self.valider_coupe_surface() { return; }
        if valider && !self.dessin2d.saisie.is_empty() && self.dessin2d.edition_contour.as_ref().is_some_and(|e| e.raccord.is_some()) {
            self.valider_saisie_dessin();
            if self.dessin2d.message.is_some() { return; }
        }
        if valider { self.terminer_raccord_ouvert(); }
        if valider && self.dessin2d.edition_contour.as_ref().is_some_and(|e| !e.ligne() && !matches!(e.creation, Some(Usage::Escalier | Usage::GardeCorps | Usage::Annotation)) && e.fermees.iter().any(|f| !f)) {
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
                    let formes = self.dessin2d.trace.as_mut().map(|t| t.appliquer(d2::EvenementTrace::Fermer)).unwrap_or_default();
                    self.enregistrer_formes(formes);
                    if self.dessin2d.message.is_some() { return; }
                } else {
                    self.valider_saisie_dessin();
                    if usage == Some(Usage::Escalier) && self.dessin2d.esc_balance.is_some() && self.dessin2d.message.is_some() { return; }
                }
            }
        }
        if valider && surface && !self.dessin2d.ouverts.is_empty() {
            self.dessin2d.message = Some("Contour non fermé : reprendre son tracé et le fermer, ou annuler avec la croix.".into());
            return;
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
                let limite = modele::elements::site::LimitePropriete { id: String::new(), nom: "Limite de propriété".into(), altitude, contour: contour.clone() };
                match projet.executer(Commande::LimitePropriete { limite }) {
                    Ok((p, nouveaux)) => { projet = p; ids.extend(nouveaux); }
                    Err(e) => { self.dessin2d.message = Some(e); return; }
                }
            }
            if let Err(e) = self.appliquer_projet(projet) { self.dessin2d.message = Some(e); return; }
            self.bim.selection = ids.clone();
            self.selected = ids.last().map(|id| modele::identifiants::id_natif("limite_propriete", id));
            self.reveal = self.selected;
        } else if valider && usage == Some(Usage::Terrain) {
            // création, ou terrain repris par Modifier tracé (même identifiant)
            let Some(terrain) = self.terrain_courant() else { self.dessin2d.message = Some("Tracez d'abord un contour fermé.".into()); return; };
            match self.executer_bim(Commande::Terrain { terrain }) {
                Ok(ids) => {
                    if let Some(id) = ids.first() {
                        self.selected = Some(modele::identifiants::id_natif("terrain", id));
                        self.reveal = self.selected;
                    }
                }
                Err(e) => { self.dessin2d.message = Some(e); return; }
            }
        } else if let (true, Some((element, id))) = (valider, self.dessin2d.edition_contour.as_ref().and_then(|e| e.element.map(|el| (el, e.id.clone())))) {
            // garde-corps ou limite de propriété : un seul tracé, repris tel quel
            let [trace] = self.dessin2d.dalle.as_slice() else { self.dessin2d.message = Some("Un seul tracé est attendu.".into()); return; };
            let trace = trace.clone();
            if trace.len() < 2 || trace.windows(2).any(|w| DVec2::from_array(w[0]).distance(DVec2::from_array(w[1])) < 1e-6) {
                self.dessin2d.message = Some("Le tracé doit comporter des segments de longueur non nulle.".into()); return;
            }
            let Some((projet, _)) = &self.natif else { return };
            let commande = match element {
                ElementTrace::GardeCorps => projet.garde_corps.iter().find(|g| g.id == id).cloned().map(|mut garde_corps| { garde_corps.trace = trace; Commande::GardeCorps { garde_corps } }),
                ElementTrace::Limite => projet.limites_propriete.iter().find(|l| l.id == id).cloned().map(|mut limite| { limite.contour = trace; Commande::LimitePropriete { limite } }),
            };
            let Some(commande) = commande else { self.dessin2d.message = Some("Élément introuvable.".into()); return; };
            if let Err(e) = self.executer_bim(commande) { self.dessin2d.message = Some(e); return; }
            self.status = "Tracé modifié — Ctrl+S pour enregistrer.".into();
        } else if valider && let Some(edit) = self.dessin2d.edition_contour.as_ref() && let Some(modele) = edit.annotation.clone() {
            let Some((mut projet, _)) = self.natif.clone() else { return };
            let mut ids = Vec::new();
            for (i, points) in self.dessin2d.dalle.iter().enumerate() {
                let fermee = edit.fermee(i);
                let minimum = if fermee { 3 } else { 2 };
                if points.len() < minimum || points.windows(2).any(|w| DVec2::from_array(w[0]).distance(DVec2::from_array(w[1])) < 1e-6)
                    || (fermee && points.first().zip(points.last()).is_some_and(|(a, b)| DVec2::from_array(*a).distance(DVec2::from_array(*b)) < 1e-6)) {
                    self.dessin2d.message = Some("La polyligne doit comporter des segments de longueur non nulle.".into()); return;
                }
                let mut annotation = modele.clone();
                if i > 0 { annotation.id.clear(); }
                // le remplissage reste tant que la polyligne est fermée
                let remplissage = match &modele.forme {
                    AnnotationForme::Polyligne { remplissage, .. } if fermee => remplissage.clone(),
                    _ => None,
                };
                // types de trait des segments gardés, ramenés au nombre de segments
                let mut traits = match &modele.forme {
                    AnnotationForme::Polyligne { traits, .. } if i == 0 => traits.clone(),
                    _ => Vec::new(),
                };
                traits.truncate(points.len() - 1 + usize::from(fermee));
                let courbures = self.dessin2d.courbures_enregistrees(i, fermee);
                annotation.forme = AnnotationForme::Polyligne { points: points.clone(), fermee, courbures, remplissage, traits };
                match projet.executer(Commande::Annoter { annotation }) {
                    Ok((p, nouveaux)) => { projet = p; ids.extend(nouveaux); }
                    Err(e) => { self.dessin2d.message = Some(e); return; }
                }
            }
            if let Err(e) = self.appliquer_projet(projet) { self.dessin2d.message = Some(e); return; }
            self.bim.selection = ids;
            self.status = "Polyligne modifiée — Ctrl+S pour enregistrer.".into();
            self.informer("Polyligne modifiée");
        } else if valider && matches!(usage, Some(Usage::Escalier | Usage::GardeCorps | Usage::Annotation)) && self.dessin2d.edition_contour.as_ref().is_some_and(|e| e.creation.is_some()) {
            let formes = self.dessin2d.dalle.iter().enumerate().map(|(i, c)| {
                let fermee = self.dessin2d.edition_contour.as_ref().is_none_or(|e| e.fermee(i));
                noyau::forme2d::Forme2D::Polyligne { points: c.iter().copied().map(DVec2::from_array).collect(), fermee, courbures: self.dessin2d.courbures_enregistrees(i, fermee) }
            }).collect();
            self.enregistrer_formes(formes);
            if self.dessin2d.message.is_some() { return; }
        } else if surface && valider && let Some(id) = self.dessin2d.edition_contour.as_ref().filter(|e| e.creation.is_none()).map(|e| e.id.clone()) {
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
        self.dessin2d.vider_contours();
        self.reinitialiser_terrain();
        self.dessin2d.edition_contour = None;
        self.visibility_dirty = true;
        self.coupe_dirty = true;
        self.dessin2d.geoms = None;
        self.dessin2d.saisie.clear();
        { self.dessin2d.message = None; self.dessin2d.information = None; };
        self.dessin2d.trace = None;
        self.dessin2d.historique_depart = None;
        self.dessin2d.commande_cle = None;
        self.transfo.reinitialiser();
        self.bim.depart_structure = None;
        self.coupe_depart = None;
        self.alignement_plan = false;
        self.arete_alignement = None;
        self.ruban_edition.annuler();
    }

    /// Terrain en cours : altitude absolue de chaque sommet du contour (S1, S2…) et de
    /// chaque point coté (1, 2…), modifiable à côté du point, dans l'unité
    /// du projet.
    pub(crate) fn cotes_terrain(&mut self, ui: &egui::Ui, rect: egui::Rect) {
        if self.dessin2d.dalle.is_empty() || !self.usage_dessin().is_some_and(|(u, _)| u == Usage::Terrain) {
            return;
        }
        let (bleu, vert) = (Color32::from_rgb(0, 120, 215), Color32::from_rgb(20, 160, 60));
        let sommets: Vec<DVec2> = self.dessin2d.dalle[0].iter().map(|p| DVec2::from_array(*p)).collect();
        let points: Vec<DVec2> = self.dessin2d.terrain_points.iter().map(|p| DVec2::new(p[0], p[1])).collect();
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
                        });
                    });
                });
        }
    }

    /// Options et actions de la commande active, regroupées en haut à gauche de la vue.
    pub(crate) fn barre_commande(&mut self, ui: &egui::Ui, rect: egui::Rect) {
        let edition = self.dessin2d.edition_contour.is_some();
        // Création ou modification, tracé ou sélection : la même barre d'options, au
        // même endroit.
        let actif = self.ruban_edition.outil().is_some() || edition;
        let options_dessin = !edition && self.usage_dessin().is_some_and(|(u, o)| u != Usage::Annotation || (o == OutilDessin::Polyligne && self.outil_cotation().is_none()));
        let options_modifier = !edition && self.a_options_modifier();
        // autres outils (cotes, texte, placements, spline, cercle) : leurs réglages
        let options_outil = !edition && !options_dessin && !options_modifier && self.a_options_outil();
        if self.natif.is_none() || (!actif && !options_modifier) {
            self.dessin2d.bas_barre = 0.0;
            return;
        }
        use crate::outils::bandeau::{bandeau, Action, Fin};
        let fin = if actif { Fin::COMPLETE } else { Fin::AUCUNE };
        let retour = if edition || options_dessin || options_modifier || options_outil {
            let mut options = |ui: &mut egui::Ui| if edition || options_dessin { self.options_trace(ui); } else if options_modifier { self.options_modifier(ui, false); } else { self.options_outil(ui); };
            bandeau(ui, rect, "commande", fin, Some(&mut options))
        } else {
            bandeau(ui, rect, "commande", fin, None)
        };
        self.dessin2d.bas_barre = retour.bas;
        match retour.action {
            Some(Action::Valider) => self.quitter_commande(true),
            Some(Action::Annuler) => self.annuler_commande(),
            None => {}
        }
    }

    /// Entrée : point saisi (coordonnées, distance), mot-clé F (fermer), ou fin du tracé.
    pub(crate) fn valider_saisie_dessin(&mut self) {
        let mut saisie = std::mem::take(&mut self.dessin2d.saisie);
        self.dessin2d.champ_angle = false;
        if saisie.is_empty() && self.dessin2d.edition_contour.as_ref().is_some_and(|e| e.raccord.is_some()) {
            self.terminer_raccord_ouvert();
            return;
        }
        if saisie.find(['<', '>']).is_some_and(|i| i > 0 && i + 1 == saisie.len()) { saisie.pop(); }
        // arc en attente : la valeur tapée est son rayon, du côté du curseur
        if !saisie.is_empty() && !saisie.starts_with(['@', '#']) && !saisie.eq_ignore_ascii_case("f")
            && let Some(trace) = self.dessin2d.trace.as_ref().filter(|t| t.attend_courbure())
        {
            let curseur = self.dessin2d.curseur.or(self.dessin2d.point.map(|p| p.0));
            let passage = match (crate::saisie::moteur_accrochage::distance_saisie(&saisie, modele::unite::unite().facteur()), curseur) {
                (Some(rayon), Some(curseur)) => trace.point_de_rayon(rayon, curseur),
                (Some(_), None) => Err("Rayon : placer le curseur du côté où l'arc doit bomber.".into()),
                (None, _) => Err("Rayon : valeur positive attendue.".into()),
            };
            match passage {
                Ok(p) => {
                    { self.dessin2d.message = None; self.dessin2d.information = None; };
                    if !self.ajouter_point_raccord(p) {
                        let formes = self.dessin2d.trace.as_mut().map(|t| t.appliquer(d2::EvenementTrace::Point(p))).unwrap_or_default();
                        self.enregistrer_formes(formes);
                    }
                }
                Err(e) => { self.dessin2d.saisie = saisie; self.dessin2d.message = Some(e); }
            }
            return;
        }
        if saisie.eq_ignore_ascii_case("f") && let Some((c, premier)) = self.dessin2d.edition_contour.as_ref().and_then(|e| e.raccord) {
            let contour = &self.dessin2d.dalle[c];
            let Some(&cible) = (if premier { contour.last() } else { contour.first() }) else { return };
            self.ajouter_point_raccord(DVec2::from_array(cible));
            return;
        }
        if !saisie.is_empty() && self.dessin2d.edition_contour.as_ref().is_some_and(|e| e.raccord.is_some()) {
            let precedent = self.dessin2d.trace.as_ref().and_then(Trace::precedent);
            let direction = self.dessin2d.point.zip(precedent).map(|((c, _), p)| c - p);
            match d2::lire_saisie(&saisie, precedent, direction, modele::unite::unite().facteur()) {
                Ok(p) => { self.ajouter_point_raccord(p); { self.dessin2d.message = None; self.dessin2d.information = None; }; }
                Err(e) => self.dessin2d.message = Some(e),
            }
            return;
        }
        if saisie.is_empty() && self.dessin2d.esc_balance.is_some()
            && self.usage_dessin().is_some_and(|(u, _)| u == Usage::Escalier)
            && let Some(t) = &self.dessin2d.trace
            && !t.points.is_empty()
            && let Err(e) = self.verifier_trace_escalier_balance(&t.points)
        {
            self.dessin2d.message = Some(e);
            return;
        }
        let Some(trace) = self.dessin2d.trace.as_mut() else { return };
        { self.dessin2d.message = None; self.dessin2d.information = None; };
        let formes = if saisie.is_empty() {
            trace.appliquer(d2::EvenementTrace::Terminer)
        } else if saisie.eq_ignore_ascii_case("f") {
            trace.appliquer(d2::EvenementTrace::Fermer)
        } else {
            // distance seule le long d'un guide issu du point de référence : depuis ce point
            let depart = self.dessin2d.point.filter(|_| !saisie.starts_with(['@', '#']) && !saisie.contains(['<', '>']))
                .and_then(|(c, _)| self.dessin2d.reperage.depart_saisie(c));
            let precedent = depart.or(trace.precedent());
            let direction = self.dessin2d.point.zip(precedent).map(|((c, _), p)| c - p);
            match d2::lire_saisie(&saisie, precedent, direction, modele::unite::unite().facteur()) {
                Ok(p) => trace.appliquer(d2::EvenementTrace::Point(p)),
                Err(e) => {
                    self.dessin2d.message = Some(e);
                    Vec::new()
                }
            }
        };
        self.enregistrer_formes(formes);
    }

}

#[cfg(test)]
mod tests_guides_perpendiculaires {
    use super::*;

    #[test]
    fn saisie_alterne_les_champs_sans_perdre_les_valeurs() {
        let mut texte = String::new();
        let mut angle = false;
        for ch in "2,5>45".chars() { ajouter_caractere_saisie(&mut texte, &mut angle, ch); }
        assert_eq!(texte, "2,5>45");
        angle = false;
        ajouter_caractere_saisie(&mut texte, &mut angle, '0');
        assert_eq!(texte, "2,50>45");
        effacer_caractere_saisie(&mut texte, false);
        effacer_caractere_saisie(&mut texte, true);
        assert_eq!(texte, "2,5>4");
    }

    #[test]
    fn reference_sur_segment_incline_et_cercle() {
        let segment = Geom::Segment([DVec2::ZERO, DVec2::new(100.0, 100.0)]);
        let p = DVec2::new(30.0, 30.0);
        let directions = directions_au_point(p, &[segment]);
        assert_eq!(directions.len(), 1);
        assert!((directions[0].perp().dot(DVec2::ONE)).abs() < 1e-9);
        let cercle = Geom::Cercle { centre: DVec2::ZERO, rayon: 100.0 };
        let directions = directions_au_point(DVec2::new(100.0, 0.0), &[cercle]);
        assert_eq!(directions, vec![DVec2::Y]);
    }
}
