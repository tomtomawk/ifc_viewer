//! Compléments du pilote : les mutations passent par les transactions de l'interface.
use super::*;
use serde_json::{Value, json};

fn lire<T: serde::de::DeserializeOwned>(s: &str) -> Result<T, String> {
    serde_json::from_str(s).map_err(|e| e.to_string())
}
fn champ<'a>(v: &'a Value, cle: &str) -> Result<&'a str, String> {
    v.get(cle)
        .and_then(Value::as_str)
        .filter(|s| !s.trim().is_empty())
        .ok_or_else(|| format!("Champ texte requis : {cle}"))
}

impl ViewerApp {
    pub(super) fn commande_complete(
        &mut self,
        ctx: &egui::Context,
        nom: &str,
        argument: &str,
    ) -> Option<Result<Value, String>> {
        if nom=="mur_trace" {
            return Some((|| {
                if !argument.is_empty() {
                    use crate::dessin::dao::OutilDessin as D;
                    let forme=match argument {"ligne"|"selection"=>D::Ligne,"rectangle"=>D::Rectangle,"arc"=>D::Arc,"cercle"=>D::Cercle,_=>return Err("mur_trace ligne|rectangle|arc|cercle|selection".into())};
                    self.activer_outil_pilote("mur")?;
                    self.ruban_edition.forme_mur=forme;
                    self.ruban_edition.selection_mur=argument=="selection";
                    self.terminer_edition_axe();
                }
                let controle=self.arc_mur_selection().map(|(id,points,z)|json!({"id":id,"points":points.map(|p|p.to_array()),"ecran":points.map(|p|self.ecran_plan(self.rect_vue,p.to_array(),z).map(|p|[p.x,p.y]))}));
                Ok(json!({"selection":self.ruban_edition.selection_mur,"invite":self.dessin2d.trace.as_ref().map(|t|t.invite()),"points":self.dessin2d.trace.as_ref().map(|t|t.points.iter().map(|p|p.to_array()).collect::<Vec<_>>()),"controle_arc":controle}))
            })());
        }
        if nom=="volets_etat" {return Some(Ok(self.etat_volets()));}
        if nom=="volets_retablir" {self.retablir_volets();return Some(Ok(self.etat_volets()));}
        if nom=="ajuster_angle" {
            return Some((|| {
                let angle=match argument {"oui"=>true,"non"=>false,_=>return Err("ajuster_angle oui|non".into())};
                self.ruban_edition.prolonger_angle=angle;
                self.ajustement=Some(Default::default());
                Ok(self.etat_ajustement())
            })());
        }
        if nom=="ajuster_etat" {return Some(Ok(self.etat_ajustement()));}
        if nom == "parametres" {
            return Some((|| {
                match argument {
                    "ouvrir" => self.ouvrir_parametres(),
                    "fermer" => self.parametres = None,
                    "" | "etat" => {}
                    categorie
                        if ["general", "dessin", "raccourcis", "graphisme", "gabarit"]
                            .contains(&categorie) =>
                    {
                        self.ouvrir_parametres();
                        self.parametres.as_mut().unwrap().categorie = categorie.into();
                    }
                    _ => return Err(
                        "parametres ouvrir|fermer|etat|general|dessin|raccourcis|graphisme|gabarit"
                            .into(),
                    ),
                }
                Ok(
                    json!({"ouvert":self.parametres.is_some(),"categorie":self.parametres.as_ref().map(|p|&p.categorie)}),
                )
            })());
        }
        if nom == "raccourcis" {
            return Some(Ok(
                json!({"raccourcis":crate::config::options::raccourcis::COMMANDES.iter().map(|c|json!({"id":c.id,"nom":c.nom,"categorie":c.categorie,"defaut":c.defaut,"raccourci":self.options.raccourci(c)})).collect::<Vec<_>>()}),
            ));
        }
        if nom == "axe_selection" {
            return Some((|| {
                let (id, a, b, z, _) = self.axe_selection().ok_or("Sélectionner un mur droit")?;
                let ecran = |p| self.ecran_plan(self.rect_vue, p, z).map(|p| [p.x, p.y]);
                Ok(
                    json!({"id":id,"debut":a,"fin":b,"debut_ecran":ecran(a),"fin_ecran":ecran(b),"deplacement":self.poignee_axe.as_ref().map(|p|p.2),"accrochage":self.etat_accrochage()}),
                )
            })());
        }
        if let Some(r) = self.piloter_famille(nom, argument) {
            return Some(r);
        }
        if nom == "gabarit_exporter" {
            return Some((|| {
                if argument.trim().is_empty() {
                    return Err("Chemin de gabarit requis".into());
                }
                let g = crate::config::gabarit::actif();
                let json = serde_json::to_vec_pretty(&*g).map_err(|e| e.to_string())?;
                crate::modele::echanges::enregistrement::ecrire_atomique(
                    std::path::Path::new(argument.trim().trim_matches('"')),
                    &json,
                )?;
                Ok(json!({"chemin":argument}))
            })());
        }
        let commandes = [
            "aide",
            "nouveau",
            "fermer",
            "importer",
            "modifier_projet",
            "selection_native",
            "saisie",
            "options",
            "gabarit",
            "exporter_pdf",
            "exporter_ifc",
            "exporter_csv",
            "nomenclature",
            "collaboration_etat",
            "collaboration_reserver",
            "collaboration_liberer",
            "collaboration_reprendre",
            "collaboration_resoudre",
            "documents",
            "document",
            "fermer_vue",
            "historique",
            "historique_parcourir",
            "familles",
            "famille_editer",
            "erreur_effacer",
        ];
        if nom == "outil" {
            return Some(self.activer_outil_pilote(argument));
        }
        if nom == "export" {
            return Some((|| {
                let v: Value = lire(argument)?;
                let path = champ(&v, "chemin")?;
                let format = champ(&v, "format")?;
                if matches!(format, "feuille_svg" | "feuille_pdf") {
                    let id = champ(&v, "id")?;
                    let p = &self.natif.as_ref().ok_or("Projet natif requis")?.0;
                    let f = p
                        .feuilles
                        .iter()
                        .find(|f| f.id == id)
                        .ok_or("Feuille absente")?;
                    let svg = crate::modele::documentation::svg_feuille(p, f)?;
                    let bytes = if format == "feuille_pdf" {
                        crate::modele::documentation::pdf(&svg)?
                    } else {
                        svg.into_bytes()
                    };
                    crate::modele::echanges::enregistrement::ecrire_atomique(
                        std::path::Path::new(path),
                        &bytes,
                    )?;
                    return Ok(json!({"chemin":path,"octets":bytes.len()}));
                }
                if format == "jeu_pdf" {
                    let jeu = v
                        .get("jeu")
                        .and_then(Value::as_str)
                        .map(str::to_string)
                        .or_else(|| self.jeu_actif())
                        .or_else(|| {
                            self.natif.as_ref().and_then(|(p, _)| {
                                crate::modele::documentation::jeux(p).into_iter().next()
                            })
                        })
                        .ok_or("Aucun jeu de feuilles")?;
                    return Ok(
                        json!({"chemin":path,"pages":self.exporter_jeu(&jeu,std::path::Path::new(path))?}),
                    );
                }
                let (commande, argument) = match format {
                    "svg" => ("exporter", path.to_string()),
                    "pdf" => ("exporter_pdf", path.to_string()),
                    "ifc" => ("exporter_ifc", path.to_string()),
                    "csv" => (
                        "exporter_csv",
                        json!({"id":champ(&v,"id")?,"chemin":path}).to_string(),
                    ),
                    _ => {
                        return Err(
                            "format : svg|pdf|ifc|csv|jeu_pdf|feuille_svg|feuille_pdf".into()
                        );
                    }
                };
                lire(&self.commande(ctx, commande, &argument))
            })());
        }
        if !commandes.contains(&nom) {
            return None;
        }
        Some((|| {
            let vide = json!({});
            match nom {
                "aide" => Ok(json!({"documentation":pilote::documentation(argument)?})),
                "erreur_effacer" => {
                    self.error = None;
                    Ok(vide)
                }
                "nouveau" => {
                    self.pilote_disponible()?;
                    let v: Value = lire(argument)?;
                    let path = PathBuf::from(champ(&v, "chemin")?);
                    if path.exists() {
                        return Err("La destination existe déjà".into());
                    }
                    let nom = v
                        .get("nom")
                        .and_then(Value::as_str)
                        .unwrap_or("Nouveau projet");
                    let p = crate::modele::Projet::depuis_gabarit(nom, &crate::config::gabarit::actif());
                    let s = scene::depuis_natif(&p, &path)?;
                    crate::modele::echanges::enregistrement::enregistrer_natif(&p, &path)?;
                    self.install_scene(s);
                    let _ = self.historique.enregistrer(&p);
                    self.natif = Some((p, path));
                    self.error = None;
                    self.ouvrir_plan_initial_natif();
                    lire(&self.etat())
                }
                "fermer" => {
                    self.pilote_disponible()?;
                    if self.natif_modifie && argument.trim() != "abandonner" {
                        return Err(
                            "Modifications non enregistrées : enregistrer ou fermer abandonner"
                                .into(),
                        );
                    }
                    self.fermer_document();
                    Ok(vide)
                }
                "importer" => {
                    self.pilote_disponible()?;
                    let v: Value = lire(argument)?;
                    let source = PathBuf::from(champ(&v, "source")?);
                    let sortie = PathBuf::from(champ(&v, "sortie")?);
                    if !source.is_file() {
                        return Err("Source introuvable".into());
                    }
                    if sortie.exists() {
                        return Err("La destination existe déjà".into());
                    }
                    self.error = None;
                    self.importer_natif(source, sortie, ctx);
                    lire(&self.etat())
                }
                "modifier_projet" => {
                    self.pilote_disponible()?;
                    let p = &self.natif.as_ref().ok_or("Projet natif requis")?.0;
                    let q = pilote::edition::modifier(p, lire(argument)?)?;
                    self.appliquer_projet(q)?;
                    lire(&self.commande(ctx, "projet_natif", ""))
                }
                "selection_native" => {
                    let ids: Vec<String> = lire(argument)?;
                    let p = &self.natif.as_ref().ok_or("Projet natif requis")?.0;
                    let connus = p.identifiants();
                    if ids.iter().any(|id| !connus.contains(id)) {
                        return Err("Identifiant natif absent".into());
                    }
                    self.select(None, false);
                    self.bim.selection = ids;
                    Ok(json!({"selection":self.bim.selection}))
                }
                "saisie" => {
                    let e = pilote::saisie::evenements(lire(argument)?)?;
                    self.saisies_pilote.extend(e);
                    ctx.request_repaint();
                    Ok(json!({"en_attente":self.saisies_pilote.len()}))
                }
                "options" => {
                    if !argument.trim().is_empty() {
                        let v: Value = lire(argument)?;
                        let mut o = self.options.clone();
                        let m = v.as_object().ok_or("Objet JSON requis")?;
                        for (k, v) in m {
                            match k.as_str() {
                                "theme_interface" => {
                                    o.theme_interface=serde_json::from_value(v.clone()).map_err(|e|e.to_string())?
                                }
                                "navigation_ifc" => {
                                    o.navigation_ifc =
                                        v.as_bool().ok_or("Booléen navigation_ifc requis")?
                                }
                                "unite" => {
                                    o.unite = serde_json::from_value(v.clone())
                                        .map_err(|e| e.to_string())?
                                }
                                "delai_accrochage" => {
                                    o.delai_accrochage =
                                        v.as_f64().ok_or("Délai numérique requis")?
                                }
                                "raccourcis" => {
                                    o.raccourcis = serde_json::from_value(v.clone())
                                        .map_err(|e| e.to_string())?
                                }
                                _ => {
                                    return Err(format!(
                                        "Option inconnue : {k} ; utiliser gabarit pour le gabarit"
                                    ));
                                }
                            }
                        }
                        self.appliquer_options(o)?;
                        self.options.theme_interface.appliquer(ctx);
                    }
                    Ok(json!({"options":self.options}))
                }
                "gabarit" => {
                    if argument == "defaut" {
                        crate::config::gabarit::definir(crate::config::gabarit::Gabarit::defaut());
                        self.options.gabarit = None;
                        self.options.enregistrer();
                    } else if !argument.trim().is_empty() {
                        let path = PathBuf::from(argument.trim().trim_matches('"'));
                        crate::config::gabarit::definir(crate::config::gabarit::Gabarit::charger(&path)?);
                        self.options.gabarit = Some(path);
                        self.options.enregistrer();
                    }
                    Ok(json!({"gabarit":&*crate::config::gabarit::actif()}))
                }
                "exporter_pdf" | "exporter_ifc" => {
                    let path = PathBuf::from(argument.trim().trim_matches('"'));
                    if argument.trim().is_empty() {
                        return Err("Chemin de sortie requis".into());
                    }
                    let bytes = if nom == "exporter_ifc" {
                        crate::modele::echanges::export_ifc::exporter(
                            &self.natif.as_ref().ok_or("Projet natif requis")?.0,
                        )?
                        .into_bytes()
                    } else {
                        crate::modele::documentation::pdf(&self.dessin_vue()?.svg("Dessin"))?
                    };
                    crate::modele::echanges::enregistrement::ecrire_atomique(&path, &bytes)?;
                    Ok(json!({"chemin":path,"octets":bytes.len()}))
                }
                "nomenclature" | "exporter_csv" => {
                    let v: Value = lire(argument)?;
                    let id = champ(&v, "id")?;
                    let p = &self.natif.as_ref().ok_or("Projet natif requis")?.0;
                    let n = p
                        .nomenclatures
                        .iter()
                        .find(|n| n.id == id)
                        .ok_or("Nomenclature absente")?;
                    let lignes = crate::modele::documentation::tableau(p, n)?;
                    if nom == "exporter_csv" {
                        let path = PathBuf::from(champ(&v, "chemin")?);
                        crate::modele::echanges::enregistrement::ecrire_atomique(
                            &path,
                            crate::modele::documentation::csv(&lignes).as_bytes(),
                        )?;
                        Ok(json!({"chemin":path,"lignes":lignes.len()}))
                    } else {
                        Ok(json!({"nomenclature":n,"lignes":lignes}))
                    }
                }
                "collaboration_etat" => {
                    let c = self.collaboration.as_ref().ok_or("Copie non associée")?;
                    Ok(
                        json!({"etat":c.etat,"synchronisation":c.synchronisation_en_cours(),"copie":c.copie}),
                    )
                }
                "collaboration_reserver" | "collaboration_liberer" | "collaboration_reprendre" => {
                    let cles = if argument.trim().is_empty() {
                        self.cles_collaboration()
                    } else {
                        let ids: Vec<String> = lire(argument)?;
                        let p = &self.natif.as_ref().ok_or("Projet natif requis")?.0;
                        let objets = crate::modele::collaboration::objets(p)?;
                        let mut cles = Vec::new();
                        for id in ids {
                            let cle = objets
                                .iter()
                                .find(|(_, v)| v.get("id").and_then(Value::as_str) == Some(&id))
                                .map(|(k, _)| k.clone())
                                .ok_or_else(|| format!("Objet absent : {id}"))?;
                            cles.push(cle);
                        }
                        cles
                    };
                    if cles.is_empty() {
                        return Err("Sélection vide".into());
                    }
                    self.collaboration
                        .as_mut()
                        .ok_or("Copie non associée")?
                        .synchroniser(
                            Some((
                                cles,
                                nom == "collaboration_reprendre",
                                nom == "collaboration_liberer",
                            )),
                            false,
                        )?;
                    Ok(json!({"synchronisation_demandee":true}))
                }
                "collaboration_resoudre" => {
                    self.pilote_disponible()?;
                    let v: Value = lire(argument)?;
                    let id = champ(&v, "transaction")?;
                    let c = self.collaboration.as_mut().ok_or("Copie non associée")?;
                    let p = match champ(&v, "choix")? {
                        "locale" => c.resoudre_locale(id)?,
                        "acceptee" => c.abandonner(id)?,
                        _ => return Err("choix : locale|acceptee".into()),
                    };
                    self.appliquer_projet_interne(p, true)?;
                    Ok(vide)
                }
                "documents" => Ok(self.documents_pilote()),
                "document" => {
                    self.pilote_disponible()?;
                    self.activer_document_pilote(argument.trim())?;
                    self.suivre_document_actif();
                    lire(&self.etat())
                }
                "fermer_vue" => {
                    if argument == "autres" {
                        self.garder_derniere_vue_par_document();
                    } else if argument.is_empty() {
                        self.pilote_disponible()?;
                        if self.natif_modifie && self.vues_ouvertes.len() <= 1 {
                            return Err(
                                "Dernier onglet : enregistrer le projet ou fermer abandonner"
                                    .into(),
                            );
                        }
                        self.fermer_onglet_projet(self.document_actif, self.vue_active);
                    } else {
                        return Err("usage : fermer_vue [autres]".into());
                    }
                    Ok(vide)
                }
                "historique" => Ok(if let Some(ed) = self.famille.as_ref().filter(|_| self.famille_active) {
                    json!({"modifie":ed.modifiee(),"annuler":ed.libelles_historique(false),"retablir":ed.libelles_historique(true),"collaboratif":false,"famille":true})
                } else if let Some(c) = &self.collaboration {
                    json!({"modifie":self.natif_modifie,"annuler":c.copie.annulations,"retablir":c.copie.retablissements,"collaboratif":true})
                } else {
                    json!({"modifie":self.natif_modifie,"annuler":self.historique.libelles(false),"retablir":self.historique.libelles(true),"collaboratif":false})
                }),
                "historique_parcourir" => {
                    self.pilote_disponible()?;
                    let v: Value = lire(argument)?;
                    let n = v
                        .get("nombre")
                        .and_then(Value::as_u64)
                        .filter(|n| *n > 0)
                        .ok_or("Nombre positif requis")?;
                    let retablir = match champ(&v, "action")? {
                        "annuler" => false,
                        "retablir" => true,
                        _ => return Err("action : annuler|retablir".into()),
                    };
                    self.error = None;
                    self.parcourir_historique_n(retablir, n as usize);
                    if let Some(e) = &self.error {
                        return Err(e.clone());
                    }
                    lire(&self.etat())
                }
                "familles" => {
                    Ok(json!({"familles":self.familles_projet(),"editeur":self.titre_famille()}))
                }
                "famille_editer" => {
                    self.pilote_disponible()?;
                    if !self
                        .familles_projet()
                        .iter()
                        .any(|(id, _, _)| id == argument)
                    {
                        return Err("Famille absente".into());
                    }
                    self.ouvrir_editeur_famille(argument);
                    Ok(vide)
                }
                _ => unreachable!(),
            }
        })())
    }

    pub(super) fn pilote_disponible(&self) -> Result<(), String> {
        if self.loading.is_some()
            || self.sauvegarde.is_some()
            || self.partage_en_cours
            || self.saisie_active
            || self.avant_saisie.is_some()
            || !self.saisies_pilote.is_empty()
            || self.saisie_pilote_image
        {
            return Err("Opération ou saisie en cours ; attendre ou valider la saisie".into());
        }
        Ok(())
    }

    fn activer_outil_pilote(&mut self, nom: &str) -> Result<Value, String> {
        if nom == "aucun" {
            self.ruban_edition.annuler();
            return Ok(json!({}));
        }
        self.pilote_disponible()?;
        self.activer_outil_interface(nom)
    }

    pub(super) fn activer_outil_interface(&mut self, nom: &str) -> Result<Value, String> {
        use crate::ui::ruban::Outil as O;
        if self.natif.is_none() {
            return Err("Projet natif requis".into());
        }
        let (o, onglet) = match nom {
            "niveau" => (O::Niveau, OngletRuban::Modeliser),
            "quadrillage" => (O::Quadrillage, OngletRuban::Modeliser),
            "mur" => (O::Mur, OngletRuban::Modeliser),
            "terrain" => (O::Terrain, OngletRuban::Modeliser),
            "limite_propriete" => (O::LimitePropriete, OngletRuban::Modeliser),
            "dalle" => (O::Dalle, OngletRuban::Modeliser),
            "sol" => (O::Sol, OngletRuban::Modeliser),
            "plafond" => (O::Plafond, OngletRuban::Modeliser),
            "toiture" => (O::Toiture, OngletRuban::Modeliser),
            "poteau" => (O::Poteau, OngletRuban::Modeliser),
            "poutre" => (O::Poutre, OngletRuban::Modeliser),
            "escalier" => (O::Escalier, OngletRuban::Modeliser),
            "garde_corps" => (O::GardeCorps, OngletRuban::Modeliser),
            "ouverture" => (O::Ouverture, OngletRuban::Modeliser),
            "deplacer" => (O::Deplacer, OngletRuban::Modifier),
            "copier" => (O::Copier, OngletRuban::Modifier),
            "tourner" => (O::Tourner, OngletRuban::Modifier),
            "aligner" => (O::Aligner, OngletRuban::Modifier),
            "ajuster" => (O::Ajuster, OngletRuban::Modifier),
            "modifier_trace" => (O::ModifierContour, OngletRuban::Modifier),
            "creer_identique" => (O::CreerIdentique, OngletRuban::Modifier),
            "supprimer" => (O::Supprimer, OngletRuban::Modifier),
            "cadrer" => (O::Cadrer, OngletRuban::Modifier),
            "isoler" => (O::Isoler, OngletRuban::Modifier),
            "masquer" => (O::Masquer, OngletRuban::Modifier),
            "verrouiller" => (O::Verrouiller, OngletRuban::Modifier),
            "deverrouiller" => (O::Deverrouiller, OngletRuban::Modifier),
            "texte" => (O::Texte, OngletRuban::Annoter),
            "cote" => (O::Cote, OngletRuban::Annoter),
            "etiquette" => (O::Etiquette, OngletRuban::Annoter),
            "ligne" => (O::Ligne, OngletRuban::Annoter),
            "polyligne" => (O::Polyligne, OngletRuban::Annoter),
            "spline" => (O::Spline, OngletRuban::Annoter),
            "cercle" => (O::Cercle, OngletRuban::Annoter),
            "arc" => (O::Arc, OngletRuban::Annoter),
            "zone" => (O::Zone, OngletRuban::Annoter),
            "vue_plan" => (O::VuePlan, OngletRuban::Annoter),
            "coupe" | "vue_coupe" => (O::VueCoupe, OngletRuban::Annoter),
            "feuilles" => (O::Feuilles, OngletRuban::Annoter),
            "nomenclatures" => (O::Nomenclatures, OngletRuban::Annoter),
            _ => {
                return Err(format!(
                    "Outil inconnu ou non implémenté : {nom} ; voir aide"
                ));
            }
        };
        self.onglet_ruban = onglet;
        self.ajustement=if o==O::Ajuster {Some(Default::default())} else {None};
        self.error = None;
        match o {
            O::Mur => self.ruban_edition.activer_mur(),
            O::Poteau | O::Poutre => self.ruban_edition.activer_structure(o == O::Poutre),
            O::Dalle | O::Sol | O::Plafond | O::Toiture => {
                self.ruban_edition.activer_surface(match o {
                    O::Sol => crate::modele::SorteDalle::Sol,
                    O::Plafond => crate::modele::SorteDalle::Plafond,
                    O::Toiture => crate::modele::SorteDalle::Toiture,
                    _ => crate::modele::SorteDalle::Dalle,
                })
            }
            O::VueCoupe => self.ruban_edition.activer_coupe(),
            O::ModifierContour => self.commencer_edition_contour(),
            O::CreerIdentique => self.creer_identique()?,
            O::Supprimer => self.demander_suppression(),
            O::Cadrer => self.demarrer_cadrage_vue(),
            O::Isoler | O::Masquer => self.visibilite_selection(o),
            O::Verrouiller | O::Deverrouiller => self.verrouiller_selection(o == O::Verrouiller),
            O::Feuilles => self.commande_feuilles(),
            O::Nomenclatures => self.bim.documentation = true,
            _ => self.ruban_edition.activer_outil(o),
        }
        if let Some(e) = &self.error {
            return Err(e.clone());
        }
        lire(&self.etat())
    }
}
