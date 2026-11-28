//! Enregistrement et exports en arrière-plan : la fenêtre reste utilisable pendant le
//! travail, la barre haute montre l'étape et un bouton « Annuler ». Le travail porte sur
//! une copie du projet prise au lancement ; les modifications faites pendant ce temps ne
//! sont pas perdues, elles restent à enregistrer.
//!
//! L'annulation est vue avant chaque écriture de fichier : un fichier est écrit en entier
//! ou pas du tout (`ecrire_atomique`). Un export de plusieurs fichiers annulé en cours de
//! route garde les fichiers déjà écrits, et le dit.
//!
//! Le pilote garde ses commandes synchrones (`enregistrer`, `exporter_*`) : il attend
//! leur résultat dans la même réponse.
use crate::*;
use crate::documenter::export_jeu::Format;
use std::sync::atomic::{AtomicBool, Ordering};
use std::path::Path;
use std::sync::{Arc, Mutex};

/// Message d'un travail interrompu par l'utilisateur.
const ANNULE: &str = "Annulé.";

/// Suivi partagé entre l'interface et le fil du travail.
#[derive(Clone, Default)]
pub(crate) struct Controle {
    annule: Arc<AtomicBool>,
    etape: Arc<Mutex<String>>,
}

impl Controle {
    fn etape(&self, texte: impl Into<String>) {
        *self.etape.lock().unwrap_or_else(|e| e.into_inner()) = texte.into();
    }

    /// Erreur `ANNULE` si l'utilisateur a demandé l'arrêt.
    fn continuer(&self) -> Result<(), String> {
        if self.annule.load(Ordering::Relaxed) { Err(ANNULE.into()) } else { Ok(()) }
    }

    /// Écrit `octets` dans `chemin` sauf annulation demandée.
    fn ecrire(&self, chemin: &Path, octets: &[u8]) -> Result<(), String> {
        self.continuer()?;
        self.etape(format!("Écriture de {}…", chemin.display()));
        echanges::enregistrement::ecrire_atomique(chemin, octets)
    }
}

/// Résultat d'un travail réussi.
pub(crate) enum Fin {
    /// Projet natif enregistré : la copie enregistrée, pour savoir si le projet a
    /// changé depuis.
    Natif { projet: Box<modele::Projet>, chemin: PathBuf },
    /// Fichiers écrits : message de confirmation.
    Fichiers(String),
    /// Fichier lié lu (et mis en cache) : le lien à ajouter au projet, son nombre
    /// d'éléments et une alerte de position éventuelle.
    Lien { lien: modele::liens::LienIfc, elements: usize, alerte: Option<String> },
    /// IFC converti, à fusionner dans le projet actif (`Commande::Fusionner`).
    Import { projet: Box<modele::Projet>, nom: String },
}

/// Distance (m) au-delà de laquelle un lien est signalé loin de l'origine du projet.
const LIEN_LOIN: f64 = 2000.0;

/// Travail en cours.
pub(crate) struct Tache {
    nom: String,
    document: usize,
    controle: Controle,
    fin: Receiver<Result<Fin, String>>,
    debut: web_time::Instant,
}

/// Contenu d'un PDF à produire hors du fil de l'interface.
pub(crate) enum SourcePdf {
    /// Dessin de la vue déjà mis en SVG (le calcul de la vue reste sur l'interface).
    Svg(String),
    Feuille(String),
    Jeu(String),
}

/// PDF d'une source, avec l'étape en cours ; une page par feuille pour un jeu.
fn pdf(p: &modele::Projet, source: &SourcePdf, c: &Controle) -> Result<Vec<u8>, String> {
    let feuille = |id: &str| p.feuilles.iter().find(|f| f.id == id).ok_or_else(|| "Feuille introuvable.".to_string());
    match source {
        SourcePdf::Svg(svg) => moteur2d::pdf(svg),
        SourcePdf::Feuille(id) => moteur2d::pdf(&moteur2d::svg_feuille(p, feuille(id)?)?),
        SourcePdf::Jeu(jeu) => {
            let feuilles: Vec<_> = p.feuilles.iter().filter(|f| &f.jeu == jeu).collect();
            let mut svgs = Vec::with_capacity(feuilles.len());
            for (i, f) in feuilles.iter().enumerate() {
                c.continuer()?;
                c.etape(format!("Feuille {}/{} : {} {}", i + 1, feuilles.len(), f.numero, f.nom));
                svgs.push(moteur2d::svg_feuille(p, f)?);
            }
            c.etape("Assemblage du PDF…");
            moteur2d::pdf_pages(&svgs)
        }
    }
}

/// Écrit les fichiers d'un export de jeu, un par un ; message final.
fn ecrire_jeu(p: &modele::Projet, a_ecrire: &[crate::documenter::export_jeu::Fichier], dossier: &Path, c: &Controle) -> Result<String, String> {
    for (i, (nom, ids, format)) in a_ecrire.iter().enumerate() {
        if c.continuer().is_err() {
            return Err(format!("Export annulé : {i} fichier(s) écrit(s) sur {} dans {}.", a_ecrire.len(), dossier.display()));
        }
        c.etape(format!("Fichier {}/{} : {nom}", i + 1, a_ecrire.len()));
        let feuilles: Vec<&modele::documentation::Feuille> = ids.iter().filter_map(|id| p.feuilles.iter().find(|f| &f.id == id)).collect();
        let octets = match (format, feuilles.as_slice()) {
            (Format::Ifc, _) => echanges::export_ifc::exporter_fichier(p, nom)?.into_bytes(),
            (Format::Pdf, [f]) => moteur2d::pdf(&moteur2d::svg_feuille(p, f)?)?,
            // PDF groupé : une page par feuille
            (Format::Pdf, fs) if !fs.is_empty() => moteur2d::pdf_pages(&fs.iter().map(|f| moteur2d::svg_feuille(p, f)).collect::<Result<Vec<_>, _>>()?)?,
            (Format::Dwg, [f]) => moteur2d::dwg_feuille(p, f)?,
            (Format::Dxf, [f]) => moteur2d::dxf_feuille(p, f)?.into_bytes(),
            _ => continue,
        };
        c.ecrire(&dossier.join(nom), &octets)?;
    }
    Ok(format!("{} fichier(s) écrit(s) dans {}", a_ecrire.len(), dossier.display()))
}

impl Application {
    /// Lance `travail` dans un fil ; refuse si un travail ou un chargement est en cours.
    fn lancer_tache(&mut self, ctx: &egui::Context, nom: &str, travail: impl FnOnce(&Controle) -> Result<Fin, String> + Send + 'static) {
        if self.tache.is_some() || self.sauvegarde.is_some() {
            self.error = Some("Un enregistrement ou un export est déjà en cours.".into());
            return;
        }
        let controle = Controle::default();
        controle.etape(format!("{nom}…"));
        let (tx, rx) = channel();
        let (c, contexte) = (controle.clone(), ctx.clone());
        let lance = noyau::fil::lancer(&format!("tache : {nom}"), move || {
            let r = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| travail(&c)))
                .unwrap_or_else(|_| Err("Erreur interne pendant le travail (panique) ; aucun fichier partiel n'a été écrit.".into()));
            let _ = tx.send(r);
            contexte.request_repaint();
        });
        match lance {
            Ok(_) => self.tache = Some(Tache { nom: nom.into(), document: self.document_actif, controle, fin: rx, debut: web_time::Instant::now() }),
            Err(e) => self.error = Some(format!("Démarrage impossible : {e}")),
        }
    }

    /// Ctrl+S et bouton Enregistrer : projet natif enregistré en arrière-plan. Le
    /// fichier Revit et la confirmation de fermeture gardent leur parcours.
    pub(crate) fn enregistrer_projet_arriere_plan(&mut self, ctx: &egui::Context) {
        if self.natif.is_none() || self.loading.is_some() {
            self.enregistrer_projet(ctx);
            return;
        }
        self.enregistrer_rendu_vue();
        let Some((projet, chemin)) = self.natif.clone() else { return };
        self.lancer_tache(ctx, "Enregistrement", move |c| {
            c.etape(format!("Vérification de {}…", projet.nom));
            projet.verifier()?;
            let json = projet.json()?;
            modele::Projet::lire_json(&json)?;
            c.ecrire(&chemin, json.as_bytes())?;
            Ok(Fin::Natif { projet: Box::new(projet), chemin })
        });
    }

    pub(crate) fn exporter_ifc_arriere_plan(&mut self, ctx: &egui::Context, chemin: PathBuf) {
        let Some((projet, _)) = self.natif.clone() else { return };
        self.lancer_tache(ctx, "Export IFC4", move |c| {
            c.etape("Export IFC4 : géométrie et relations…");
            let texte = echanges::export_ifc::exporter_fichier(&projet, &chemin.file_name().map_or_else(String::new, |n| n.to_string_lossy().into_owned()))?;
            c.ecrire(&chemin, texte.as_bytes())?;
            Ok(Fin::Fichiers(format!("IFC4 exporté : {}", chemin.display())))
        });
    }

    /// PDF de ce qui est affiché : la feuille choisie (ou celle de la vue choisie) d'un
    /// jeu ouvert, sinon tout le jeu ; hors jeu, la vue orthographique (n° 12).
    pub(crate) fn exporter_pdf_arriere_plan(&mut self, ctx: &egui::Context, chemin: PathBuf) {
        let projet = self.natif.as_ref().map(|(p, _)| p.clone()).unwrap_or_else(|| modele::Projet::nouveau("Dessin"));
        let source = match self.jeu_actif() {
            Some(jeu) => match &self.feuilles.selection {
                crate::documenter::feuilles::SelectionFeuille::Feuille(id)
                | crate::documenter::feuilles::SelectionFeuille::Vue(id, _)
                | crate::documenter::feuilles::SelectionFeuille::Cartouche(id, _)
                    if projet.feuilles.iter().any(|f| &f.id == id && f.jeu == jeu) => SourcePdf::Feuille(id.clone()),
                _ => SourcePdf::Jeu(jeu),
            },
            None => match self.dessin_vue() {
                Ok(d) => SourcePdf::Svg(d.svg("Dessin")),
                Err(e) => { self.error = Some(e); return; }
            },
        };
        self.lancer_tache(ctx, "Export PDF", move |c| {
            let octets = pdf(&projet, &source, c)?;
            c.ecrire(&chemin, &octets)?;
            Ok(Fin::Fichiers(format!("PDF exporté : {}", chemin.display())))
        });
    }

    /// Une feuille du navigateur en SVG ou en PDF.
    pub(crate) fn exporter_feuille_arriere_plan(&mut self, ctx: &egui::Context, extension: &str, id: String, chemin: PathBuf) {
        let Some((projet, _)) = self.natif.clone() else { return };
        let pdf_demande = extension == "pdf";
        self.lancer_tache(ctx, "Export de la feuille", move |c| {
            let octets = if pdf_demande {
                pdf(&projet, &SourcePdf::Feuille(id), c)?
            } else {
                let f = projet.feuilles.iter().find(|f| f.id == id).ok_or("Feuille introuvable.")?;
                moteur2d::svg_feuille(&projet, f)?.into_bytes()
            };
            c.ecrire(&chemin, &octets)?;
            Ok(Fin::Fichiers(format!("Export : {}", chemin.display())))
        });
    }

    /// Export multiformat d'un jeu (n° 13) : un fichier après l'autre.
    /// SVG d'un dessin déjà calculé : mise en texte et écriture en arrière-plan.
    pub(crate) fn exporter_svg_arriere_plan(&mut self, ctx: &egui::Context, d: noyau::trace2d::Dessin, titre: String, chemin: PathBuf) {
        self.lancer_tache(ctx, "Export SVG", move |c| {
            c.etape("Écriture du SVG…");
            let texte = d.svg(&titre);
            c.ecrire(&chemin, texte.as_bytes())?;
            Ok(Fin::Fichiers(format!("{} : {} primitives exportées", chemin.display(), d.primitives.len())))
        });
    }

    pub(crate) fn exporter_jeu_arriere_plan(&mut self, ctx: &egui::Context, jeu: String, a_ecrire: Vec<crate::documenter::export_jeu::Fichier>, dossier: PathBuf) {
        let Some((projet, _)) = self.natif.clone() else { return };
        self.lancer_tache(ctx, "Export du jeu", move |c| {
            ecrire_jeu(&projet, &a_ecrire, &dossier, c).map(|m| Fin::Fichiers(format!("Jeu « {jeu} » : {m}")))
        });
    }

    /// Lien IFC : lecture du fichier (géométrie et arêtes, gardées en cache), puis ajout
    /// ou remplacement du lien dans le projet actif, en une transaction annulable.
    pub(crate) fn lier_ifc_arriere_plan(&mut self, ctx: &egui::Context, lien: modele::liens::LienIfc) {
        self.lancer_tache(ctx, "Lien IFC", move |c| {
            c.etape(format!("Lecture du lien {}…", lien.nom));
            let f = maillage::liens::charger(Path::new(&lien.chemin))?;
            c.continuer()?;
            let centre = (f.min + f.max) * 0.5;
            let alerte = if f.elements.is_empty() {
                Some(format!("Lien « {} » : aucun élément affichable dans le fichier.", lien.nom))
            } else {
                let d = (lien.transformer(centre).truncate()).length();
                (d > LIEN_LOIN).then(|| format!("Lien « {} » : son centre est à {:.1} km de l'origine du projet ; régler son décalage (arborescence > Liens).", lien.nom, d / 1000.0))
            };
            Ok(Fin::Lien { elements: f.elements.len(), lien, alerte })
        });
    }

    /// Import IFC dans le projet ouvert : lecture et conversion en arrière-plan, puis
    /// fusion en une transaction annulable.
    pub(crate) fn importer_dans(&mut self, ctx: &egui::Context, chemin: PathBuf) {
        if self.natif.is_none() {
            self.error = Some("Importer dans le projet : ouvrir ou créer d'abord un projet natif.".into());
            return;
        }
        let nom = chemin.file_name().map_or_else(|| "IFC".into(), |n| n.to_string_lossy().into_owned());
        self.lancer_tache(ctx, "Import IFC", move |c| {
            let etape = c.clone();
            let (projet, _) = echanges::depuis_ifc::convertir(&chemin, &move |t| etape.etape(t))?;
            c.continuer()?;
            Ok(Fin::Import { projet: Box::new(projet), nom })
        });
    }

    /// Une fois par image : fin du travail en cours.
    pub(crate) fn suivre_tache(&mut self) {
        let Some(t) = &self.tache else { return };
        let resultat = match t.fin.try_recv() {
            Ok(r) => r,
            Err(std::sync::mpsc::TryRecvError::Empty) => return,
            Err(std::sync::mpsc::TryRecvError::Disconnected) => Err("Le travail a été interrompu.".into()),
        };
        let Some(t) = self.tache.take() else { return };
        match resultat {
            Ok(Fin::Fichiers(message)) => self.status = message,
            Ok(Fin::Natif { projet, chemin }) => self.fin_enregistrement(t.document, &projet, &chemin),
            Ok(Fin::Lien { lien, elements, alerte }) if t.document == self.document_actif => {
                let nom = lien.nom.clone();
                match self.executer_bim(commandes::Commande::Lien { lien }) {
                    Ok(_) => {
                        self.status = format!("Lien « {nom} » : {elements} élément(s) — Ctrl+Z pour annuler.");
                        if let Some(a) = alerte { self.signaler(crate::socle::messages::Niveau::Avertissement, a); }
                    }
                    Err(e) => self.error = Some(e),
                }
            }
            Ok(Fin::Import { projet, nom }) if t.document == self.document_actif => {
                match self.executer_bim(commandes::Commande::Fusionner { projet, decalage: [0.0; 3] }) {
                    Ok(_) => self.status = format!("Import de {nom} dans le projet — Ctrl+Z pour annuler."),
                    Err(e) => self.error = Some(format!("Import de {nom} refusé : {e}")),
                }
            }
            Ok(Fin::Import { nom, .. }) => self.error = Some(format!("Import de {nom} non fait : le document a changé pendant la lecture.")),
            Ok(Fin::Lien { lien, .. }) => self.error = Some(format!("Lien « {} » non ajouté : le document a changé pendant la lecture.", lien.nom)),
            Err(e) if e == ANNULE => self.status = format!("{} annulé.", t.nom),
            Err(e) => self.error = Some(e),
        }
    }

    /// Le projet enregistré est la copie prise au lancement : le document reste modifié
    /// s'il a changé depuis, et sa copie de récupération est alors gardée.
    fn fin_enregistrement(&mut self, document: usize, projet: &modele::Projet, chemin: &Path) {
        let (historique, natif, modifie) = if document == self.document_actif {
            (&mut self.historique, &self.natif, &mut self.natif_modifie)
        } else if let Some(d) = self.documents.get_mut(&document) {
            (&mut d.historique, &d.natif, &mut d.modifie)
        } else {
            // document fermé entre-temps : le fichier est écrit
            self.status = format!("{} — enregistré", chemin.display());
            return;
        };
        if natif.as_ref().is_some_and(|(_, c)| c == chemin) {
            let _ = historique.enregistrer(projet);
            *modifie = natif.as_ref().is_some_and(|(p, _)| historique.modifie(p));
            if !*modifie { echanges::enregistrement::effacer_recuperation(chemin); }
        }
        self.status = format!("{} — enregistré", chemin.display());
    }

    /// Barre haute : étape du chargement, de l'enregistrement Revit ou du travail en
    /// cours, avec « Annuler » pour un travail.
    pub(crate) fn barre_progression(&mut self, ui: &mut egui::Ui, zones_deplacement: &mut Vec<egui::Rect>) {
        if self.loading.is_none() && self.sauvegarde.is_none() && self.tache.is_none() { return; }
        ui.spinner();
        let texte = match (&self.tache, &self.sauvegarde) {
            (Some(t), _) => {
                let etape = t.controle.etape.lock().unwrap_or_else(|e| e.into_inner()).clone();
                let annule = if t.controle.annule.load(Ordering::Relaxed) { " — annulation…" } else { "" };
                format!("{etape} ({} s){annule}", t.debut.elapsed().as_secs())
            }
            (None, Some(s)) => s.progression.clone(),
            // chargement : son étape est déjà à gauche d'Annuler, sauf en collaboration
            (None, None) if self.collaboration.is_none() => return,
            (None, None) => self.status.clone(),
        };
        if let Some(t) = &self.tache {
            if ui.button("Annuler").on_hover_text("Arrêter avant l'écriture du fichier suivant ; aucun fichier n'est écrit à moitié").clicked() {
                t.controle.annule.store(true, Ordering::Relaxed);
            }
            // l'étape et le temps écoulé changent sans événement de l'utilisateur
            ui.ctx().request_repaint_after(std::time::Duration::from_millis(250));
        }
        let reponse = ui.add_sized([(ui.available_width() - 60.0).max(0.0), 24.0], egui::Label::new(&texte).truncate()).on_hover_text(&texte);
        zones_deplacement.push(reponse.rect);
    }

    /// Un travail empêche de quitter : il serait interrompu.
    pub(crate) fn tache_en_cours(&self) -> bool {
        self.tache.is_some() || self.sauvegarde.is_some()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dossier(nom: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("bimedit-taches-{}-{nom}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    fn projet() -> modele::Projet {
        modele::Projet::lire_json(include_str!("../../../../samples/maison_native.bem")).unwrap()
    }

    /// Annulé avant l'écriture : le fichier existant n'est pas touché.
    #[test]
    fn annulation_avant_ecriture_garde_le_fichier() {
        let d = dossier("annulation");
        let chemin = d.join("projet.ifc");
        std::fs::write(&chemin, b"ancien").unwrap();
        let c = Controle::default();
        c.annule.store(true, Ordering::Relaxed);
        assert_eq!(c.ecrire(&chemin, b"nouveau"), Err(ANNULE.into()));
        assert_eq!(std::fs::read(&chemin).unwrap(), b"ancien");
        let c = Controle::default();
        c.ecrire(&chemin, b"nouveau").unwrap();
        assert_eq!(std::fs::read(&chemin).unwrap(), b"nouveau");
    }

    /// Export de jeu : même contenu que l'export direct ; annulé, il dit combien de
    /// fichiers sont écrits.
    #[test]
    fn export_de_jeu_identique_et_annulable() {
        let p = projet();
        let f = p.feuilles.first().expect("une feuille dans l'exemple");
        let a_ecrire = vec![
            ("maison.ifc".to_string(), Vec::new(), Format::Ifc),
            ("A01.pdf".to_string(), vec![f.id.clone()], Format::Pdf),
            ("A01.dxf".to_string(), vec![f.id.clone()], Format::Dxf),
            ("A01.dwg".to_string(), vec![f.id.clone()], Format::Dwg),
            ("jeu.pdf".to_string(), p.feuilles.iter().map(|f| f.id.clone()).collect(), Format::Pdf),
        ];
        let d = dossier("jeu");
        let m = ecrire_jeu(&p, &a_ecrire, &d, &Controle::default()).unwrap();
        assert!(m.starts_with("5 fichier(s) écrit(s)"), "{m}");
        // l'en-tête porte l'heure et le nom du fichier : la section DATA est comparée
        let donnees = |t: String| t[t.find("DATA;").unwrap()..].to_string();
        let ifc = std::fs::read_to_string(d.join("maison.ifc")).unwrap();
        assert!(ifc.contains("FILE_NAME('maison.ifc'"), "{}", &ifc[..300]);
        assert_eq!(donnees(ifc), donnees(echanges::export_ifc::exporter(&p).unwrap()));
        // l'ordre des arêtes vues d'une feuille change d'un calcul à l'autre (antérieur à
        // ce module) : mêmes lignes, dans un ordre quelconque
        let lignes = |t: String| { let mut l: Vec<String> = t.lines().map(str::to_string).collect(); l.sort(); l };
        assert_eq!(lignes(std::fs::read_to_string(d.join("A01.dxf")).unwrap()), lignes(moteur2d::dxf_feuille(&p, f).unwrap()));
        assert!(std::fs::read(d.join("A01.pdf")).unwrap().starts_with(b"%PDF"));
        assert!(std::fs::read(d.join("A01.dwg")).unwrap().starts_with(b"AC1018"));
        // PDF groupé : une page par feuille du projet
        let groupe = std::fs::read(d.join("jeu.pdf")).unwrap();
        assert!(groupe.windows(10).filter(|w| w == b"/Type /Pag" || w == b"/Type/Page").count() >= p.feuilles.len());
        let d = dossier("jeu-annule");
        let c = Controle::default();
        c.annule.store(true, Ordering::Relaxed);
        let e = ecrire_jeu(&p, &a_ecrire, &d, &c).unwrap_err();
        assert!(e.starts_with("Export annulé : 0 fichier(s) écrit(s) sur 5"), "{e}");
        assert_eq!(std::fs::read_dir(&d).unwrap().count(), 0);
    }

    /// PDF d'un jeu : une page par feuille, l'étape nomme la feuille.
    #[test]
    fn pdf_de_jeu_par_feuille() {
        let p = projet();
        let jeu = p.feuilles[0].jeu.clone();
        let c = Controle::default();
        let octets = pdf(&p, &SourcePdf::Jeu(jeu.clone()), &c).unwrap();
        let pages = |o: &[u8]| String::from_utf8_lossy(o).matches("/Type /Page\n").count() + String::from_utf8_lossy(o).matches("/Type /Page ").count() + String::from_utf8_lossy(o).matches("/Type /Page/").count();
        let n = p.feuilles.iter().filter(|f| f.jeu == jeu).count();
        assert!(octets.starts_with(b"%PDF") && n > 0);
        assert_eq!(pages(&octets), pages(&moteur2d::pdf_jeu(&p, &jeu).unwrap()));
        assert_eq!(*c.etape.lock().unwrap(), "Assemblage du PDF…");
        c.annule.store(true, Ordering::Relaxed);
        assert_eq!(pdf(&p, &SourcePdf::Jeu(jeu), &c), Err(ANNULE.into()));
    }
}
