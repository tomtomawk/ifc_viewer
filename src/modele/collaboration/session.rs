//! Copie de travail persistante et synchronisation sur un thread dédié.
use super::stockage::{Depot, Etat};
use super::texte::{Editeur, EtatTexte};
use super::*;
use std::path::PathBuf;
use std::sync::mpsc::{self, Receiver};

#[derive(Clone, Serialize, Deserialize)]
pub struct Copie {
    #[serde(default)]
    pub partage: Option<PathBuf>,
    #[serde(default)]
    pub editeurs: BTreeMap<String, texte::SauvegardeEditeur>,
    pub session: String,
    pub central: PathBuf,
    pub base: Etat,
    pub attente: Vec<Transaction>,
    pub conflits: BTreeMap<String, Vec<Conflit>>,
    /// Vrai dès qu'une synchronisation a trouvé ou créé le dépôt central :
    /// un dépôt disparu n'est alors jamais recréé en silence.
    #[serde(default)]
    pub central_connu: bool,
    /// Dernier document local valide, affiché sans le publier pendant un conflit.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub apercu_conflit: Option<Projet>,
    pub annulations: Vec<Transaction>,
    pub retablissements: Vec<Transaction>,
}
pub struct Session {
    pub copie: Copie,
    pub chemin: PathBuf,
    pub etat: String,
    pub mesure_sync_ms: f64,
    pub mesure_geometrie_ms: f64,
    pub mesure_gpu_ms: f64,
    pub mesures_fichiers: stockage::Mesures,
    pub mesure_validation_ms: f64,
    messages: Option<
        Receiver<
            Resultat<(
                Etat,
                Vec<Transaction>,
                BTreeMap<String, Vec<Conflit>>,
                f64,
                stockage::Mesures,
            )>,
        >,
    >,
    editeurs: BTreeMap<String, Editeur>,
    // Le verrou local reste vivant pour interdire deux propriétaires de la même queue.
    _local: std::fs::File,
}
fn overlay(base: &Etat, transactions: &[Transaction]) -> Resultat<Etat> {
    let mut etat = base.clone();
    for t in transactions {
        if etat.acceptes.contains_key(&t.id) {
            continue;
        }
        let mut local = t.clone();
        // Les préconditions sont vérifiées contre la nouvelle base ; aucune proposition
        // refusée n'est superposée silencieusement à l'état accepté.
        for cle in local.cles_reservation(&etat.objets) {
            etat.baux.insert(
                cle.clone(),
                Bail {
                    proprietaire: local.session.clone(),
                    generation: 1,
                    actif: true,
                },
            );
            local.reservations.insert(cle, 1);
        }
        etat = etat
            .accepter(&local)
            .map_err(|c| format!("Conflit de réapplication : {c:?}"))?;
    }
    Ok(etat)
}
impl Session {
    fn depot_local(chemin: &std::path::Path) -> Depot {
        Depot::nouveau(chemin.with_extension("ressources-locales"))
    }
    pub fn lire_copie(chemin: &std::path::Path) -> Resultat<Copie> {
        Self::depot_local(chemin).lire_document(chemin)
    }
    pub fn ouvrir(p: &Projet, fichier_local: PathBuf, central: PathBuf) -> Resultat<Self> {
        if fichier_local.to_string_lossy().starts_with(r"\\") {
            return Err(
                "La copie de travail doit être sur le disque local, pas sur un chemin UNC".into(),
            );
        }
        let chemin = fichier_local.with_extension("collaboration.json");
        let verrou = chemin.with_extension("lock");
        let mut opts = std::fs::OpenOptions::new();
        opts.write(true).create(true).truncate(false);
        #[cfg(windows)]
        {
            use std::os::windows::fs::OpenOptionsExt;
            opts.share_mode(0);
        }
        let local = opts.open(verrou).map_err(|e| {
            if e.raw_os_error() == Some(32) || e.kind() == std::io::ErrorKind::WouldBlock {
                format!("Copie locale déjà ouverte : {e}")
            } else {
                format!("Protection de la copie locale impossible : {e}")
            }
        })?;
        #[cfg(not(windows))]
        local
            .try_lock()
            .map_err(|e| format!("Copie locale déjà ouverte : {e}"))?;
        let copie = if chemin.exists() {
            let c = Self::lire_copie(&chemin)?;
            if c.base.projet != p.identite || c.central != central {
                return Err("La copie locale appartient à un autre dépôt/projet".into());
            }
            c
        } else {
            let mut p = p.clone();
            p.figer_references();
            Copie {
                partage: None,
                editeurs: BTreeMap::new(),
                session: uuid(),
                central,
                base: Etat::initial(&p)?,
                attente: Vec::new(),
                conflits: BTreeMap::new(),
                central_connu: false,
                apercu_conflit: None,
                annulations: Vec::new(),
                retablissements: Vec::new(),
            }
        };
        let mut editeurs = BTreeMap::new();
        for (id, s) in &copie.editeurs {
            editeurs.insert(id.clone(), Editeur::restaurer(s, &copie.session)?);
        }
        let mut s = Self {
            copie,
            chemin,
            etat: "Enregistré localement".into(),
            mesure_sync_ms: 0.0,
            mesure_geometrie_ms: 0.0,
            mesure_gpu_ms: 0.0,
            mesures_fichiers: Default::default(),
            mesure_validation_ms: 0.0,
            messages: None,
            editeurs,
            _local: local,
        };
        s.persister()?;
        Ok(s)
    }
    pub fn persister(&mut self) -> Resultat<()> {
        for (id, e) in &self.editeurs {
            self.copie.editeurs.insert(id.clone(), e.sauvegarder());
        }
        Self::depot_local(&self.chemin).ecrire_document(&self.chemin, &self.copie)
    }
    pub fn travail(&self) -> Resultat<Projet> {
        overlay(
            &self.copie.base,
            &self
                .copie
                .attente
                .iter()
                .filter(|t| !self.copie.conflits.contains_key(&t.id))
                .cloned()
                .collect::<Vec<_>>(),
        )?
        .projection()
    }
    /// L'affichage d'une proposition n'est jamais un état accepté ni une base d'édition.
    pub fn affichage(&self) -> Resultat<Projet> {
        if !self.copie.conflits.is_empty() && let Some(p)=&self.copie.apercu_conflit {return Ok(p.clone());}
        self.travail()
    }
    pub fn texte_local(&self, id: &str) -> Resultat<EtatTexte> {
        let mut e = self.copie.base.textes.get(id).cloned().unwrap_or_default();
        for t in &self.copie.attente {
            if let Some(u) = t.textes.get(id) {
                e.recevoir(u)?;
            }
        }
        Ok(e)
    }
    pub fn editer(&mut self, avant: &Projet, apres: &Projet) -> Resultat<()> {
        let debut = std::time::Instant::now();
        if !self.copie.conflits.is_empty() {
            return Err("Résoudre les conflits avant de poursuivre l'édition".into());
        }
        let mut t =
            Transaction::entre(avant, apres, &self.copie.session, self.copie.base.revision)?;
        for cle in t.cles_reservation(&objets(avant)?) {
            if let Some(b) = self
                .copie
                .base
                .baux
                .get(&cle)
                .filter(|b| b.actif && b.proprietaire == self.copie.session)
            {
                t.reservations.insert(cle, b.generation);
            }
        }
        for a in &apres.annotations {
            if let super::super::documentation::AnnotationForme::Texte { texte, .. } = &a.forme {
                let precedent = avant
                    .annotations
                    .iter()
                    .find(|b| b.id == a.id)
                    .and_then(|b| {
                        if let super::super::documentation::AnnotationForme::Texte {
                            texte, ..
                        } = &b.forme
                        {
                            Some(texte.as_str())
                        } else {
                            None
                        }
                    });
                if precedent != Some(texte.as_str()) {
                    if !self.editeurs.contains_key(&a.id) {
                        self.editeurs.insert(
                            a.id.clone(),
                            Editeur::ouvrir(&self.texte_local(&a.id)?, &self.copie.session)?,
                        );
                    }
                    t.textes.insert(
                        a.id.clone(),
                        self.editeurs.get_mut(&a.id).unwrap().remplacer(texte),
                    );
                }
            }
        }
        if t.changements.is_empty() && t.textes.is_empty() {
            return Ok(());
        }
        // Les propositions successives observent l'overlay, pas une ancienne révision.
        t.base = self.copie.base.revision + self.copie.attente.len() as u64;
        let sauvegarde = self.copie.clone();
        self.copie.annulations.push(t.clone());
        self.copie.retablissements.clear();
        self.copie.attente.push(t);
        if let Err(e) = self.travail().and_then(|_| self.persister()) {
            self.copie = sauvegarde;
            self.editeurs.clear();
            return Err(e);
        }
        self.mesure_validation_ms = debut.elapsed().as_secs_f64() * 1000.0;
        self.etat = "Enregistré localement — en attente".into();
        Ok(())
    }
    pub fn synchroniser(
        &mut self,
        reservation: Option<(Vec<String>, bool, bool)>,
        instantane: bool,
    ) -> Resultat<()> {
        self.synchroniser_interne(reservation, instantane, true)
    }
    pub fn reception_seule(&mut self) -> Resultat<()> {
        self.synchroniser_interne(None, false, false)
    }
    pub fn synchronisation_en_cours(&self) -> bool {
        self.messages.is_some()
    }
    fn synchroniser_interne(
        &mut self,
        reservation: Option<(Vec<String>, bool, bool)>,
        instantane: bool,
        publier_attente: bool,
    ) -> Resultat<()> {
        if self.messages.is_some() {
            return Err("Synchronisation déjà en cours".into());
        }
        self.persister()?;
        let protection_locale = self._local.try_clone().map_err(|e| e.to_string())?;
        let copie = self.copie.clone();
        let journal_local = self.chemin.clone();
        let (tx, rx) = mpsc::channel();
        self.messages = Some(rx);
        self.etat = "Synchronisation…".into();
        std::thread::Builder::new()
            .name("collaboration-nas".into())
            .spawn(move || {
                let _protection_locale=protection_locale;
                stockage::reinitialiser_mesures();
                let debut = std::time::Instant::now();
                let resultat = (|| {
                    let existant=copie.partage.is_some()||copie.central_connu||!publier_attente;
                    let depot = Depot::nouveau(copie.central);
                    if !existant {depot.preparer()?;}
                    let mut a=depot.proteger_attente(!existant,std::time::Duration::from_secs(2))?;
                    let mut etat = if !existant {a.initialiser_etat(&copie.base)?}else{a.lire()?.ok_or("Dépôt partagé sans état initial")?};
                    if etat.projet!=copie.base.projet || etat.revision<copie.base.revision{return Err("Identité du dépôt incorrecte ou révision centrale antérieure à la copie locale".into());}
                    if let Some((cles, reprise, liberer)) = reservation {
                        a.reserver(&mut etat, &cles, &copie.session, reprise, liberer)?;
                    }
                    let mut attente = Vec::new();
                    let mut conflits = if publier_attente{BTreeMap::new()}else{copie.conflits.clone()};
                    for mut t in copie.attente {
                        let prepare =
                            journal_local.with_extension(format!("prepare-{}.json", t.id));
                        if prepare.exists() {
                            t = Self::depot_local(&journal_local).lire_document(&prepare)?;
                        }
                        // Une réponse perdue se résout par UUID ET contenu préparé.
                        if let Some(digest)=etat.acceptes.get(&t.id){
                            if digest!=&hash(&t){return Err("Décision connue mais contenu local différent : conserver la copie pour diagnostic".into());}
                            conflits.remove(&t.id);continue;
                        }
                        if !publier_attente {attente.push(t);continue;}
                        if !attente.is_empty() {
                            attente.push(t);
                            continue;
                        }
                        let cles=t.cles_reservation(&etat.objets);
                        // Une génération déjà enregistrée ne doit jamais être remplacée.
                        let manquantes: Vec<_> = cles
                            .into_iter()
                            .filter(|k| !t.reservations.contains_key(k))
                            .collect();
                        let temporaires:Vec<_>=manquantes.iter().filter(|k|!etat.baux.get(*k).is_some_and(|b|b.actif)).cloned().collect();
                        if !manquantes.is_empty() {
                            if let Err(e) =
                                a.reserver(&mut etat, &manquantes, &copie.session, false, false)
                            {
                                if !a.protection_valide() {
                                    return Err(e);
                                }
                                conflits.insert(
                                    t.id.clone(),
                                    vec![Conflit {
                                        cle: "réservation".into(),
                                        base: None,
                                        locale: None,
                                        acceptee: None,
                                        raison: e,
                                    }],
                                );
                                attente.push(t);
                                continue;
                            }
                            for k in manquantes {
                                t.reservations.insert(k.clone(), etat.baux[&k].generation);
                            }
                        }
                        // Ne jamais abaisser une révision future pour contourner un
                        // parent absent ; les transactions locales sont publiées dans l'ordre.
                        Self::depot_local(&journal_local).ecrire_document(&prepare, &t)?;
                        let c = a.publier(&mut etat, &t)?;
                        // Libération explicite sous la même autorité des seules prises
                        // nécessaires à cette publication. Un bail demandé par l'utilisateur reste actif.
                        if !temporaires.is_empty(){a.reserver(&mut etat,&temporaires,&copie.session,false,true)?;}
                        if !c.is_empty() {
                            conflits.insert(t.id.clone(), c);
                            attente.push(t);
                        }
                    }
                    if instantane {
                        a.instantane(&etat)?;
                    }
                    drop(a);
                    Ok((
                        etat,
                        attente,
                        conflits,
                        debut.elapsed().as_secs_f64() * 1000.0,
                        stockage::mesures(),
                    ))
                })();
                drop(_protection_locale);
                let _ = tx.send(resultat);
            })
            .map_err(|e| e.to_string())?;
        Ok(())
    }
    /// Appelé uniquement hors saisie : une réponse n'écrase jamais un champ actif.
    pub fn recevoir(&mut self) -> Resultat<Option<Projet>> {
        let Some(rx) = &self.messages else {
            return Ok(None);
        };
        let resultat = match rx.try_recv() {
            Ok(r) => r,
            Err(mpsc::TryRecvError::Empty) => return Ok(None),
            Err(e) => Err(e.to_string()),
        };
        self.messages = None;
        match resultat {
            Err(e) => {
                self.etat = format!("Hors connexion / erreur : {e}");
                Ok(None)
            }
            Ok((base, mut attente, conflits, ms, mesures)) => {
                let apercu = self.affichage()?;
                // Conserve les éditions effectuées pendant le transfert.
                let presents: BTreeSet<_> = attente
                    .iter()
                    .map(|t| t.id.clone())
                    .chain(base.acceptes.keys().cloned())
                    .collect();
                for t in &self.copie.attente {
                    if !presents.contains(&t.id) {
                        attente.push(t.clone());
                    }
                }
                self.copie.base = base;
                self.copie.attente = attente;
                self.copie.conflits = conflits;
                self.copie.central_connu = true;
                // Les préparations des transactions acceptées ne servent plus.
                for id in self.copie.base.acceptes.keys() {
                    let _ = std::fs::remove_file(self.chemin.with_extension(format!("prepare-{id}.json")));
                }
                self.mesure_sync_ms = ms;
                self.mesures_fichiers = mesures;
                // Réapplication séquentielle ; un refus conserve la proposition entière.
                let mut valides = Vec::new();
                for t in &self.copie.attente {
                    if self.copie.conflits.contains_key(&t.id) {
                        continue;
                    }
                    let mut essais = valides.clone();
                    essais.push(t.clone());
                    if let Err(e) = overlay(&self.copie.base, &essais) {
                        self.copie.conflits.insert(
                            t.id.clone(),
                            vec![Conflit {
                                cle: "réapplication".into(),
                                base: None,
                                locale: None,
                                acceptee: None,
                                raison: e,
                            }],
                        );
                    } else {
                        valides = essais;
                    }
                }
                for (id, e) in &mut self.editeurs {
                    if let Some(t) = self.copie.base.textes.get(id) {
                        for u in t.mises_a_jour.values() {
                            e.recevoir(u)?;
                        }
                    }
                }
                self.etat = if !self.copie.conflits.is_empty() {
                    "Conflit — modifications locales conservées à l'écran"
                } else if self.copie.attente.is_empty() {
                    "Synchronisé"
                } else {
                    "Enregistré localement — en attente"
                }
                .into();
                self.copie.apercu_conflit = (!self.copie.conflits.is_empty()).then_some(apercu);
                self.persister()?;
                Ok(Some(self.travail()?))
            }
        }
    }
    pub fn abandonner(&mut self, id: &str) -> Resultat<Projet> {
        if self.messages.is_some() {
            return Err("Synchronisation en cours : attendre sa fin avant d'abandonner".into());
        }
        // Abandon explicite, archivé dans le fichier local de récupération.
        let archive = self
            .chemin
            .with_extension(format!("conflit-{}.json", uuid()));
        stockage::sauver_local(&archive, &encoder(&self.copie)?)?;
        self.copie.attente.retain(|t| t.id != id);
        self.copie.annulations.retain(|t| t.id != id);
        self.copie.retablissements.retain(|t| t.id != id);
        self.copie.conflits.remove(id);
        self.copie.apercu_conflit = None;
        self.persister()?;
        self.travail()
    }
    pub fn resoudre_locale(&mut self, id: &str) -> Resultat<Projet> {
        if self.messages.is_some() {
            return Err("Synchronisation en cours : attendre sa fin avant de résoudre".into());
        }
        let proposition = self
            .copie
            .attente
            .iter()
            .find(|t| t.id == id)
            .cloned()
            .ok_or("Proposition absente")?;
        let courant = self.travail()?;
        let mut cibles = objets(&courant)?;
        for c in concurrence::decomposer(proposition.changements.clone()) {
            concurrence::affecter(&mut cibles, &c.cle, c.apres.clone());
        }
        let cible = projet(&cibles)?;
        let mut resolution = Transaction::entre(
            &courant,
            &cible,
            &self.copie.session,
            self.copie.base.revision,
        )?;
        resolution.textes = proposition.textes.clone();
        resolution.compense = Some(proposition.id.clone());
        let archive = self
            .chemin
            .with_extension(format!("conflit-{}.json", uuid()));
        stockage::sauver_local(&archive, &encoder(&self.copie)?)?;
        let sauvegarde = self.copie.clone();
        self.copie.attente.retain(|t| t.id != id);
        self.copie.annulations.retain(|t| t.id != id);
        self.copie.conflits.remove(id);
        self.copie.apercu_conflit = None;
        self.copie.attente.push(resolution.clone());
        match self.travail().and_then(|p| {
            self.persister()?;
            Ok(p)
        }) {
            Ok(p) => {
                self.copie.annulations.push(resolution);
                self.persister()?;
                Ok(p)
            }
            Err(e) => {
                self.copie = sauvegarde;
                Err(e)
            }
        }
    }
    pub fn annuler(&mut self, retablir: bool) -> Resultat<Projet> {
        if !self.copie.conflits.is_empty() {
            return Err("Résoudre le conflit avant compensation".into());
        }
        let pile = if retablir {
            &self.copie.retablissements
        } else {
            &self.copie.annulations
        };
        let original = pile.last().cloned().ok_or("Historique collaboratif vide")?;
        let mut t = original.inverse(self.copie.base.revision + self.copie.attente.len() as u64);
        let actuel = self.travail()?;
        // Les effets inverses portent leurs propres valeurs attendues. Les références
        // et le modèle résultant sont validés à nouveau.
        let o = objets(&actuel)?;
        let index = Index::construire(&o);
        t.lectures.clear();
        concurrence::ajouter_spatial(&mut t, &o);
        for c in &t.changements {
            t.lectures
                .insert(format!("#usages/{}", c.cle), index.empreinte(&c.cle, &o));
        }
        t.reservations.clear();
        t.textes.clear();
        appliquer(
            &t,
            &o,
            self.copie.base.revision + self.copie.attente.len() as u64,
            &self.copie.base.projet,
        )
        .map_err(|e| format!("Compensation en conflit : {e:?}"))?;
        for id in original.textes.keys() {
            if let Some(e) = self.editeurs.get_mut(id) {
                t.textes.insert(id.clone(), e.annuler(retablir)?);
            } else {
                return Err(
                    "Annulation textuelle disponible dans la session d'édition d'origine".into(),
                );
            }
        }
        for c in &t.changements {
            if c.avant.is_none() {
                if let Some(id) = c.cle.strip_prefix("annotations/") {
                    if let Some(etat) = self.copie.base.textes.get(id) {
                        // Restauration métier explicite ; les mises à jour tardives restent dans
                        // le document Yrs et ne peuvent jamais restaurer l'existence à elles seules.
                        use yrs::{ReadTxn, StateVector, Transact};
                        t.textes.entry(id.into()).or_insert(
                            etat.document()?
                                .transact()
                                .encode_state_as_update_v1(&StateVector::default()),
                        );
                    }
                }
            }
        }
        let sauvegarde = self.copie.clone();
        self.copie.attente.push(t.clone());
        let p = match self.travail() {
            Ok(p) => p,
            Err(e) => {
                self.copie = sauvegarde;
                return Err(e);
            }
        };
        if retablir {
            self.copie.retablissements.pop();
            self.copie.annulations.push(t);
        } else {
            self.copie.annulations.pop();
            self.copie.retablissements.push(t);
        }
        self.persister()?;
        self.etat = "Compensation enregistrée localement — en attente".into();
        Ok(p)
    }
}
