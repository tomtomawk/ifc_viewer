//! Préparation d'un nouveau placement à partir de l'élément sélectionné.
use crate::zones::ruban::Outil;
use crate::*;

impl ViewerApp {
    pub(crate) fn creer_identique(&mut self) -> Result<(), String> {
        let ids = self.selection_bim();
        if ids.len() != 1 {
            return Err("Sélectionnez un seul élément pour créer à l'identique.".into());
        }
        let (p, _) = self.natif.as_ref().ok_or("Ouvrez un projet natif.")?;
        let id = &ids[0];
        let contexte = p.contextes.get(id).cloned().unwrap_or_default();
        let mur = p.murs.iter().find(|m| &m.id == id).or_else(|| {
            p.assemblage(id)
                .and_then(|a| a.couches.first())
                .and_then(|id| p.murs.iter().find(|m| &m.id == id))
        });
        if let Some(mur) = mur {
            self.ruban_edition.mur_identique(p, mur)?;
        } else if let Some(d) = p.dalles.iter().find(|d| &d.id == id) {
            self.ruban_edition.surface_identique(p, d)?;
        } else if let Some(b) = p.barres.iter().find(|b| &b.id == id) {
            if b.genre == modele::GenreBarre::Poutre {
                self.bim.poutre = crate::outils::modeliser::poutres::ReglagesPoutre {
                    type_poutre: b.pose.type_poutre.clone(),
                    justification: b.pose.justification,
                    arase: b.pose.arase,
                    niveau: Some(b.niveau.clone()),
                    decalage: b.debut[2],
                    largeur: b.largeur,
                    hauteur: b.hauteur,
                };
            } else {
                self.bim.barre = b.clone();
                self.bim.barre.id.clear();
                self.bim.barre.origine = None;
                self.bim.niveau_haut_poteau = None;
                self.bim.decalage_haut_poteau = 0.0;
            }
            self.ruban_edition
                .activer_structure(b.genre == modele::GenreBarre::Poutre);
        } else if let Some(o) = p.ouvertures.iter().find(|o| &o.id == id) {
            self.ruban_edition.ouverture_identique(o)?;
        } else if let Some(e) = p.escaliers.iter().find(|e| &e.id == id) {
            use crate::outils::dessiner::dessin_plan::HautEscalier;
            self.dessin2d.esc_bas = Some(e.niveau.clone());
            self.dessin2d.esc_haut = e
                .niveau_haut
                .clone()
                .map_or(HautEscalier::Libre, HautEscalier::Niveau);
            self.dessin2d.esc_hauteur_libre = e.hauteur;
            self.dessin2d.esc_decalage_bas = e.decalage_bas;
            self.dessin2d.esc_decalage_haut = e.decalage_haut;
            self.dessin2d.esc_balance = e.balance;
            self.dessin2d.esc_paillasse = Some(e.paillasse);
            self.dessin2d.esc_sous_face = e.sous_face;
            self.dessin2d.giron_impose = None;
            self.dessin2d.justification_escalier = 0;
            self.dessin2d.just_segments.clear();
            self.ruban_edition
                .definir_nombre_option(Outil::Escalier, 0, e.largeur);
            self.ruban_edition
                .definir_nombre_option(Outil::Escalier, 1, e.hauteur_marche);
            self.ruban_edition
                .definir_texte_option(Outil::Escalier, 2, "Aucun");
            self.ruban_edition.activer_outil(Outil::Escalier);
        } else if let Some(g) = p.garde_corps.iter().find(|g| &g.id == id) {
            self.ruban_edition.type_garde_corps = g.type_garde_corps.clone();
            self.ruban_edition
                .definir_nombre_option(Outil::GardeCorps, 0, g.hauteur);
            self.ruban_edition
                .definir_nombre_option(Outil::GardeCorps, 1, g.decalage);
            self.ruban_edition
                .definir_texte_option(Outil::GardeCorps, 2, g.remplissage.nom());
            self.ruban_edition.activer_outil(Outil::GardeCorps);
        } else if let Some(c) = p.composants.iter().find(|c| &c.id == id) {
            self.bim.composant = c.clone();
            self.ruban_edition.activer_outil(Outil::Composant);
        } else if let Some(a) = p.arbres.iter().find(|a| &a.id == id) {
            self.bim.arbre = a.clone();
            self.ruban_edition.activer_outil(Outil::Arbre);
        } else if let Some(piece) = p.pieces.iter().find(|piece| &piece.id == id) {
            self.bim.nom_piece = piece.nom.clone();
            self.bim.etiquette_piece = piece.etiquette.clone();
            self.ruban_edition.activer_outil(Outil::Piece);
        } else if let Some(t) = p.terrains.iter().find(|t| &t.id == id) {
            self.dessin2d.terrain_type = t.type_terrain.clone();
            self.dessin2d.terrain_altitude = t.contour.first().map_or(0.0, |point| point[2]);
            self.reinitialiser_terrain();
            self.dessin2d.vider_contours();
            self.ruban_edition.activer_outil(Outil::Terrain);
        } else if let Some(limite) = p.limites_propriete.iter().find(|limite| &limite.id == id) {
            self.bim.altitude_limite = Some(limite.altitude);
            self.ruban_edition.activer_outil(Outil::LimitePropriete);
        } else if let Some(annotation) = p.annotations.iter().find(|a| &a.id == id) {
            use modele::documentation::{AnnotationForme as F, PointCote};
            if self.vue_native_id().as_ref() != Some(&annotation.vue) {
                return Err("Ouvrez la vue de l'annotation pour la créer à l'identique.".into());
            }
            let outil = match &annotation.forme {
                F::Texte { point, texte } => {
                    self.bim.points[0] = *point;
                    self.bim.texte = texte.clone();
                    Outil::Texte
                }
                // une ligne se recrée comme une polyligne d'un segment
                F::Ligne { .. } => Outil::Polyligne,
                F::Polyligne { .. } => Outil::Polyligne,
                F::Spline { .. } => Outil::Spline,
                F::Cercle { .. } => Outil::Cercle,
                // un arc se retrace comme un segment d'arc de polyligne (Alt+clic)
                F::Arc { .. } => Outil::Polyligne,
                F::Zone { contour } => {
                    if let (Some(a), Some(b)) = (contour.first(), contour.get(2)) {
                        self.bim.points = [*a, *b];
                    }
                    Outil::Zone
                }
                F::Cote { debut, fin, .. } => {
                    if let PointCote::Fixe { point } = debut {
                        self.bim.points[0] = *point;
                    }
                    if let PointCote::Fixe { point } = fin {
                        self.bim.points[1] = *point;
                    }
                    Outil::Cote
                }
                F::Etiquette { point, .. } => {
                    self.bim.points[0] = *point;
                    Outil::Etiquette
                }
                F::CoteAngulaire { .. } => Outil::CoteAngulaire,
                F::CoteDiametre { .. } => Outil::CoteDiametre,
                F::Altimetrie { .. } => Outil::Altimetrie,
            };
            self.bim.annotation_identique = Some(annotation.clone());
            self.ruban_edition.activer_outil(outil);
            self.onglet_ruban = OngletRuban::Annoter;
            self.select(None, false);
            self.status = "Créer à l'identique : placez la nouvelle annotation.".into();
            return Ok(());
        } else if let Some(vue) = p
            .vues
            .iter()
            .find(|v| &v.id == id && matches!(v.genre, modele::GenreVue::Coupe { .. }))
        {
            self.ruban_edition.coupe_identique(vue)?;
            self.onglet_ruban = OngletRuban::Annoter;
            self.select(None, false);
            self.status = "Créer à l'identique : tracez la coupe ou l'élévation.".into();
            return Ok(());
        } else if let Some(g) = p.quadrillages.iter().find(|g| &g.id == id) {
            self.bim.points = [g.debut, g.fin];
            self.ruban_edition.quadrillage_identique(&g.nom);
        } else {
            return Err("Créer à l'identique n'est pas disponible pour cet élément.".into());
        }
        if matches!(
            self.ruban_edition.outil(),
            Some(
                Outil::Poteau
                    | Outil::Poutre
                    | Outil::Ouverture
                    | Outil::Escalier
                    | Outil::GardeCorps
                    | Outil::Composant
            )
        ) {
            self.ruban_edition.phase_creation = contexte.phase;
            self.ruban_edition.sous_projet_creation = contexte.sous_projet;
            self.ruban_edition.variante_creation = contexte.variante;
        }
        self.onglet_ruban = OngletRuban::Modeliser;
        self.select(None, false);
        self.status = "Créer à l'identique : placez le nouvel élément.".into();
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use glam::DVec2;
    use modele::elements::poutre::{Arase, Justification, PosePoutre};

    fn projet() -> modele::Projet {
        let mut projet = modele::Projet::nouveau("Créer à l'identique");
        projet.niveaux.push(modele::Niveau {
            id: "rdc".into(),
            nom: "RDC".into(),
            altitude: 0.0,
            origine: None,
        });
        projet.ajouter_vue_plan("rdc", 1200.0).unwrap();
        projet
    }

    fn app(projet: modele::Projet, id: &str) -> (ViewerApp, egui::Context) {
        let ctx = egui::Context::default();
        let mut app = ViewerApp::new(&eframe::CreationContext::_new_kittest(ctx.clone()), None);
        app.options = Default::default();
        let chemin =
            std::env::temp_dir().join(format!("identique-{}.modele.json", noyau::uuid::uuid()));
        app.natif = Some((projet, chemin));
        app.bim.selection = vec![id.to_owned()];
        (app, ctx)
    }

    fn taper_cs(app: &mut ViewerApp, ctx: &egui::Context) {
        for lettre in ["c", "s"] {
            let mut sortie = ctx.run_ui(
                egui::RawInput {
                    events: vec![egui::Event::Text(lettre.into())],
                    ..Default::default()
                },
                |ui| app.clavier_raccourcis(ui.ctx()),
            );
            // sans rendu, les textures ne sont pas appliquées : epaint refuse de les jeter en debug
            sortie.textures_delta.clear();
        }
    }

    #[test]
    fn cs_reprend_les_reglages_utilises_par_le_trace_de_poutre() {
        let mut projet = projet();
        projet.barres.push(modele::Barre {
            id: "poutre".into(),
            nom: "Poutre".into(),
            genre: modele::GenreBarre::Poutre,
            niveau: "rdc".into(),
            debut: [0.0, 0.0, 2450.0],
            fin: [4000.0, 0.0, 2450.0],
            largeur: 360.0,
            hauteur: 510.0,
            rotation: 0.0,
            origine: None,
            pose: PosePoutre {
                type_poutre: None,
                justification: Justification::Droite,
                arase: Arase::Inferieure,
            },
        });
        let (mut app, ctx) = app(projet, "poutre");
        taper_cs(&mut app, &ctx);
        assert_eq!(app.ruban_edition.outil(), Some(Outil::Poutre));
        assert!(app.selection_bim().is_empty());
        app.creer_poutres(&[noyau::forme2d::Forme2D::Ligne([
            DVec2::Y * 2000.0,
            DVec2::new(5000.0, 2000.0),
        ])]);
        let (projet, chemin) = app.natif.as_ref().unwrap();
        echanges::enregistrement::effacer_recuperation(chemin);
        let barres = &projet.barres;
        assert_eq!(barres.len(), 2, "{:?}", app.error);
        let (source, nouvelle) = (&barres[0], &barres[1]);
        assert_ne!(nouvelle.id, source.id);
        assert_eq!(nouvelle.debut, [0.0, 2000.0, 2450.0]);
        assert_eq!(
            (nouvelle.largeur, nouvelle.hauteur),
            (source.largeur, source.hauteur)
        );
        assert_eq!(nouvelle.pose.justification, source.pose.justification);
        assert_eq!(nouvelle.pose.arase, source.pose.arase);
    }

    #[test]
    fn cs_cree_un_escalier_parametrique_sans_symboles_en_plan() {
        let mut projet = projet();
        let source = modele::Escalier {
            id: "escalier".into(),
            nom: "Escalier".into(),
            niveau: "rdc".into(),
            niveau_haut: None,
            decalage_bas: 100.0,
            decalage_haut: 0.0,
            hauteur: 3000.0,
            trace: vec![[0.0, 0.0], [4500.0, 0.0]],
            largeur: 1350.0,
            hauteur_marche: 165.0,
            paillasse: 200.0,
            sous_face: modele::elements::escalier::SousFace::Lissee,
            balance: None,
        };
        projet.escaliers.push(source.clone());
        let (mut app, ctx) = app(projet, &source.id);
        taper_cs(&mut app, &ctx);
        assert_eq!(app.ruban_edition.outil(), Some(Outil::Escalier));
        let trace = vec![[0.0, 4000.0], [5000.0, 4000.0]];
        let resultat = app.creer_escalier(trace.clone());
        let (projet, chemin) = app.natif.as_ref().unwrap();
        echanges::enregistrement::effacer_recuperation(chemin);
        resultat.unwrap();
        assert_eq!(projet.escaliers.len(), 2);
        let nouveau = &projet.escaliers[1];
        assert_ne!(nouveau.id, source.id);
        assert_eq!(nouveau.trace, trace);
        assert_eq!(
            (nouveau.largeur, nouveau.hauteur_marche, nouveau.paillasse),
            (source.largeur, source.hauteur_marche, source.paillasse)
        );
        assert_eq!(nouveau.sous_face, source.sous_face);
        assert_eq!(
            (nouveau.decalage_bas, nouveau.hauteur),
            (source.decalage_bas, source.hauteur)
        );
        assert!(modele::documentation::annotations(projet, &projet.vues[0]).is_empty());
    }
}
