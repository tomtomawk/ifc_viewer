//! Autorité de fichiers. Le fichier controle n'est jamais remplacé ni supprimé.
use super::texte::EtatTexte;
use super::*;
use std::fs::{File, OpenOptions};
use std::io::{Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};
use std::time::Instant;

#[derive(Clone, Default, Serialize, Deserialize, Debug)]
pub struct Mesures {
    pub octets_lus: u64,
    pub octets_ecrits: u64,
    pub lectures: u64,
    pub ecritures: u64,
    pub publications: u64,
    pub exclusion_ms: f64,
}
thread_local! {static MESURES:std::cell::RefCell<Mesures>=std::cell::RefCell::new(Mesures::default());}
pub fn mesures() -> Mesures {
    MESURES.with(|m| m.borrow().clone())
}
pub fn reinitialiser_mesures() {
    MESURES.with(|m| *m.borrow_mut() = Mesures::default());
}
fn lu(n: usize) {
    MESURES.with(|m| {
        let mut m = m.borrow_mut();
        m.lectures += 1;
        m.octets_lus += n as u64;
    });
}
fn ecrit(n: usize) {
    MESURES.with(|m| {
        let mut m = m.borrow_mut();
        m.ecritures += 1;
        m.octets_ecrits += n as u64;
    });
}
fn frontiere(_nom: &str) {
    #[cfg(test)]
    if std::env::var("BIMEDIT_TEST_CRASH").ok().as_deref() == Some(_nom) {
        std::process::exit(86);
    }
}

#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct Etat {
    pub protocole: u32,
    pub projet: String,
    pub revision: u64,
    pub objets: Objets,
    pub textes: BTreeMap<String, EtatTexte>,
    pub baux: BTreeMap<String, Bail>,
    pub acceptes: BTreeMap<String, String>,
}
impl Etat {
    pub fn initial(p: &Projet) -> Resultat<Self> {
        let mut etat = Self {
            protocole: PROTOCOLE,
            projet: p.identite.clone(),
            revision: 0,
            objets: objets(p)?,
            textes: BTreeMap::new(),
            baux: BTreeMap::new(),
            acceptes: BTreeMap::new(),
        };
        for a in &p.annotations {
            if let modele::documentation::AnnotationForme::Texte { texte, .. } = &a.forme {
                etat.textes
                    .insert(a.id.clone(), EtatTexte::initial(&p.identite, &a.id, texte)?);
            }
        }
        Ok(etat)
    }
    pub fn projection(&self) -> Resultat<Projet> {
        let mut p = projet(&self.objets)?;
        texte::projection(&mut p, &self.textes)?;
        Ok(p)
    }
    pub fn accepter(&self, t: &Transaction) -> Result<Self, Vec<Conflit>> {
        let mut out = self.clone();
        out.objets = appliquer(t, &self.objets, self.revision, &self.projet)?;
        let erreur = |raison: String| {
            vec![Conflit {
                cle: "réservation/texte".into(),
                base: None,
                locale: None,
                acceptee: None,
                raison,
            }]
        };
        let necessaires = t.cles_reservation(&self.objets);
        for k in necessaires {
            let bail = self
                .baux
                .get(&k)
                .ok_or_else(|| erreur(format!("Réservation requise : {k}")))?;
            if !bail.actif
                || bail.proprietaire != t.session
                || t.reservations.get(&k) != Some(&bail.generation)
            {
                return Err(erreur(format!("Réservation périmée : {k}")));
            }
        }
        for (id, update) in &t.textes {
            out.textes
                .entry(id.clone())
                .or_default()
                .recevoir(update)
                .map_err(erreur)?;
        }
        for c in &t.changements {
            if let Some(id) = c.cle.strip_prefix("annotations/") {
                if c.apres.is_none() {
                    out.textes.entry(id.into()).or_default().supprime = true;
                } else if c.avant.is_none() && out.textes.get(id).is_some_and(|e| e.supprime) {
                    if t.compense
                        .as_ref()
                        .is_some_and(|id| self.acceptes.contains_key(id))
                    {
                        out.textes.get_mut(id).unwrap().supprime = false;
                    } else {
                        return Err(erreur("Identité d'annotation supprimée : restauration compensatrice explicite nécessaire".into()));
                    }
                }
                if c.avant.is_none()
                    && c.apres
                        .as_ref()
                        .is_some_and(|v| v.get("forme").and_then(Value::as_str) == Some("texte"))
                    && !t.textes.contains_key(id)
                {
                    return Err(erreur("Création textuelle sans état Yrs".into()));
                }
            }
        }
        // Seule la projection Yrs est réinjectée, jamais une chaîne concurrente.
        out.objets = objets(&out.projection().map_err(erreur)?).map_err(erreur)?;
        out.protocole = out.protocole.max(t.protocole);
        out.revision += 1;
        out.acceptes.insert(t.id.clone(), hash(t));
        Ok(out)
    }
}

#[derive(Serialize, Deserialize)]
enum Evenement {
    Initial(String),
    Reservation(BTreeMap<String, Bail>),
    Accepte { paquet: String, resultat: String },
    Instantane { revision: u64, ressource: String },
}

#[derive(Serialize, Deserialize)]
struct Repere {
    offset: usize,
    prefixe: String,
    instantane: String,
    contenu: String,
}

#[derive(Clone)]
pub struct Depot {
    pub racine: PathBuf,
}
impl Depot {
    pub fn nouveau(p: impl Into<PathBuf>) -> Self {
        Self { racine: p.into() }
    }
    pub fn preparer(&self) -> Resultat<()> {
        for d in ["ressources", "paquets", "instantanes", "temporaires"] {
            std::fs::create_dir_all(self.racine.join(d)).map_err(|e| e.to_string())?;
        }
        Ok(())
    }
    pub fn lire(&self, categorie: &str, id: &str) -> Resultat<Vec<u8>> {
        if id.len() != 64 || !id.bytes().all(|b| b.is_ascii_hexdigit()) {
            return Err("Empreinte invalide".into());
        }
        let bytes = std::fs::read(self.racine.join(categorie).join(id))
            .map_err(|e| format!("Ressource absente {id} : {e}"))?;
        lu(bytes.len());
        if empreinte(&bytes) != id {
            return Err(format!("Corruption de {categorie}/{id}"));
        }
        Ok(bytes)
    }
    pub fn publier(&self, categorie: &str, bytes: &[u8]) -> Resultat<String> {
        let id = empreinte(bytes);
        let dest = self.racine.join(categorie).join(&id);
        if dest.exists() {
            self.lire(categorie, &id)?;
            return Ok(id);
        }
        let tmp = self.racine.join("temporaires").join(uuid());
        let mut f = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&tmp)
            .map_err(|e| e.to_string())?;
        frontiere("temporaire-cree");
        let ecrire = || -> Resultat<()> {
            f.write_all(bytes).map_err(|e| e.to_string())?;
            ecrit(bytes.len());
            frontiere("temporaire-ecrit");
            f.sync_all().map_err(|e| e.to_string())?;
            frontiere("temporaire-durable");
            drop(f);
            if std::fs::metadata(&tmp).map_err(|e| e.to_string())?.len() != bytes.len() as u64
                || empreinte(&std::fs::read(&tmp).map_err(|e| e.to_string())?) != id
            {
                return Err("Dépôt incomplet".into());
            }
            std::fs::rename(&tmp, &dest).map_err(|e| e.to_string())
        };
        if let Err(e) = ecrire() {
            // Un échec ne laisse pas de temporaire orphelin sur le NAS.
            let _ = std::fs::remove_file(&tmp);
            return Err(e);
        }
        MESURES.with(|m| m.borrow_mut().publications += 1);
        frontiere("ressource-publiee");
        self.lire(categorie, &id)?;
        Ok(id)
    }
    fn figer(&self, v: &mut Value) -> Resultat<()> {
        match v {
            Value::Object(m) => {
                for (k, v) in m {
                    if matches!(k.as_str(), "maillage" | "representation")
                        && !v.is_null()
                        && v.get("$ressource").is_none()
                    {
                        let bytes = encoder(v)?;
                        let id = self.publier("ressources", &bytes)?;
                        *v = serde_json::json!({"$ressource":id,"octets":bytes.len()});
                    } else {
                        self.figer(v)?;
                    }
                }
            }
            Value::Array(a) => {
                for v in a {
                    self.figer(v)?;
                }
            }
            _ => (),
        }
        Ok(())
    }
    fn hydrater(&self, v: &mut Value) -> Resultat<()> {
        if let Some(id) = v.get("$ressource").and_then(Value::as_str) {
            let b = self.lire("ressources", id)?;
            if v.get("octets").and_then(Value::as_u64) != Some(b.len() as u64) {
                return Err("Taille de ressource incorrecte".into());
            }
            *v = serde_json::from_slice(&b).map_err(|e| e.to_string())?;
        } else {
            match v {
                Value::Object(m) => {
                    for v in m.values_mut() {
                        self.hydrater(v)?;
                    }
                }
                Value::Array(a) => {
                    for v in a {
                        self.hydrater(v)?;
                    }
                }
                _ => (),
            }
        }
        Ok(())
    }
    pub fn sauver<T: Serialize>(&self, cat: &str, v: &T) -> Resultat<String> {
        let mut v = serde_json::to_value(v).map_err(|e| e.to_string())?;
        self.figer(&mut v)?;
        self.publier(cat, &encoder(&v)?)
    }
    pub fn ecrire_document<T: Serialize>(&self, path: &Path, v: &T) -> Resultat<()> {
        self.preparer()?;
        let mut v = serde_json::to_value(v).map_err(|e| e.to_string())?;
        self.figer(&mut v)?;
        sauver_local(path, &encoder(&v)?)
    }
    pub fn lire_document<T: serde::de::DeserializeOwned>(&self, path: &Path) -> Resultat<T> {
        let mut v: Value = serde_json::from_slice(&std::fs::read(path).map_err(|e| e.to_string())?)
            .map_err(|e| e.to_string())?;
        self.hydrater(&mut v)?;
        serde_json::from_value(v).map_err(|e| e.to_string())
    }
    pub fn charger<T: serde::de::DeserializeOwned>(&self, cat: &str, id: &str) -> Resultat<T> {
        let mut v: Value =
            serde_json::from_slice(&self.lire(cat, id)?).map_err(|e| e.to_string())?;
        self.hydrater(&mut v)?;
        serde_json::from_value(v).map_err(|e| e.to_string())
    }
    pub fn proteger(&self) -> Resultat<Autorite> {
        self.proteger_interne(true)
    }
    /// Réessaie la prise de l'autorité pendant `delai` (verrou tenu par un autre poste).
    pub fn proteger_attente(&self, creer: bool, delai: std::time::Duration) -> Resultat<Autorite> {
        let limite = std::time::Instant::now() + delai;
        loop {
            match self.proteger_interne(creer) {
                Ok(a) => return Ok(a),
                Err(e) if std::time::Instant::now() >= limite => return Err(e),
                Err(_) => std::thread::sleep(std::time::Duration::from_millis(20)),
            }
        }
    }
    fn proteger_interne(&self, creer: bool) -> Resultat<Autorite> {
        let mut opts = OpenOptions::new();
        opts.read(true).write(true).create(creer).truncate(false);
        #[cfg(windows)]
        {
            use std::os::windows::fs::OpenOptionsExt;
            opts.share_mode(0);
        }
        let controle = opts
            .open(self.racine.join("controle"))
            .map_err(|e| format!("Autorité indisponible : {e}"))?;
        #[cfg(not(windows))]
        controle
            .try_lock()
            .map_err(|e| format!("Autorité indisponible : {e}"))?;
        Ok(Autorite {
            depot: self.clone(),
            controle,
            debut: Instant::now(),
            duree_exclusion_ms: 0.0,
            valide: true,
        })
    }
}

pub struct Autorite {
    pub depot: Depot,
    controle: File,
    debut: Instant,
    pub duree_exclusion_ms: f64,
    valide: bool,
}
impl Drop for Autorite {
    fn drop(&mut self) {
        MESURES
            .with(|m| m.borrow_mut().exclusion_ms += self.debut.elapsed().as_secs_f64() * 1000.0);
    }
}
impl Autorite {
    pub fn copier_registre(&mut self) -> Resultat<Vec<u8>> {
        if !self.valide {
            return Err("Protection invalidée".into());
        }
        self.controle
            .seek(SeekFrom::Start(0))
            .map_err(|e| e.to_string())?;
        let mut b = Vec::new();
        self.controle
            .read_to_end(&mut b)
            .map_err(|e| e.to_string())?;
        Ok(b)
    }
    pub fn protection_valide(&self) -> bool {
        self.valide
    }
    fn ajouter(&mut self, event: &Evenement) -> Resultat<()> {
        if !self.valide {
            return Err("Protection invalidée : réacquérir et relire".into());
        }
        let bytes = encoder(event)?;
        self.valide = false;
        // Le même handle porte la décision ; toute erreur doit détruire Autorite.
        self.controle
            .seek(SeekFrom::End(0))
            .map_err(|e| e.to_string())?;
        self.controle
            .write_all(&(bytes.len() as u64).to_le_bytes())
            .map_err(|e| e.to_string())?;
        frontiere("registre-longueur");
        self.controle.write_all(&bytes).map_err(|e| e.to_string())?;
        frontiere("registre-contenu");
        self.controle
            .write_all(empreinte(&bytes).as_bytes())
            .map_err(|e| e.to_string())?;
        frontiere("registre-empreinte");
        self.controle.sync_all().map_err(|e| e.to_string())?;
        ecrit(bytes.len() + 72);
        frontiere("registre-durable");
        self.duree_exclusion_ms = self.debut.elapsed().as_secs_f64() * 1000.0;
        self.valide = true;
        Ok(())
    }
    pub fn lire(&mut self) -> Resultat<Option<Etat>> {
        if !self.valide {
            return Err("Protection invalidée : réacquérir et relire".into());
        }
        self.valide = false;
        let resultat = self.lire_interne();
        if resultat.is_ok() {
            self.valide = true;
        }
        resultat
    }
    fn lire_interne(&mut self) -> Resultat<Option<Etat>> {
        self.controle
            .seek(SeekFrom::Start(0))
            .map_err(|e| e.to_string())?;
        let mut bytes = Vec::new();
        self.controle
            .read_to_end(&mut bytes)
            .map_err(|e| e.to_string())?;
        lu(bytes.len());
        let mut pos = 0;
        let mut etat: Option<Etat> = None;
        let repere = self.depot.racine.join("repere-instantane.json");
        if repere.exists() {
            let r: Repere =
                serde_json::from_slice(&std::fs::read(&repere).map_err(|e| e.to_string())?)
                    .map_err(|e| format!("Repère corrompu : {e}"))?;
            if r.offset > bytes.len() || empreinte(&bytes[..r.offset]) != r.prefixe {
                return Err("Corruption du préfixe accepté ou repère invalide".into());
            }
            let s: Etat = self.depot.charger("instantanes", &r.instantane)?;
            if hash(&s) != r.contenu {
                return Err("Instantané altéré".into());
            }
            pos = r.offset;
            etat = Some(s);
        }
        while pos < bytes.len() {
            let debut = pos;
            if bytes.len() - pos < 8 {
                self.tronquer(debut)?;
                break;
            }
            let len = u64::from_le_bytes(bytes[pos..pos + 8].try_into().unwrap()) as usize;
            pos += 8;
            if len > 256 * 1024 * 1024 {
                return Err("Corruption : longueur de registre invalide".into());
            }
            if bytes.len() - pos < len + 64 {
                self.tronquer(debut)?;
                break;
            }
            let payload = &bytes[pos..pos + len];
            pos += len;
            if empreinte(payload).as_bytes() != &bytes[pos..pos + 64] {
                return Err("Corruption d'un historique accepté : arrêt".into());
            }
            pos += 64;
            let event: Evenement = serde_json::from_slice(payload).map_err(|e| e.to_string())?;
            match event {
                Evenement::Initial(id) => {
                    if etat.is_some() {
                        return Err("Initialisation dupliquée".into());
                    }
                    etat = Some(self.depot.charger("instantanes", &id)?);
                }
                Evenement::Reservation(baux) => {
                    etat.as_mut().ok_or("Registre sans origine")?.baux = baux;
                }
                Evenement::Accepte { paquet, resultat } => {
                    let tx: Transaction = self.depot.charger("paquets", &paquet)?;
                    let suivant = etat
                        .as_ref()
                        .ok_or("Registre sans origine")?
                        .accepter(&tx)
                        .map_err(|e| format!("Historique incohérent : {e:?}"))?;
                    if hash(&suivant) != resultat {
                        return Err("Empreinte d'état accepté incorrecte".into());
                    }
                    etat = Some(suivant);
                }
                Evenement::Instantane {
                    revision,
                    ressource,
                } => {
                    let s: Etat = self.depot.charger("instantanes", &ressource)?;
                    if etat
                        .as_ref()
                        .is_none_or(|e| e.revision != revision || hash(e) != hash(&s))
                    {
                        return Err("Instantané incohérent".into());
                    }
                }
            }
        }
        if etat
            .as_ref()
            .is_some_and(|e| !protocole_lisible(e.protocole))
        {
            return Err("Protocole incompatible".into());
        }
        Ok(etat)
    }
    fn tronquer(&mut self, longueur: usize) -> Resultat<()> {
        self.controle
            .set_len(longueur as u64)
            .and_then(|_| self.controle.sync_all())
            .map_err(|e| e.to_string())
    }
    pub fn initialiser(&mut self, p: &Projet) -> Resultat<Etat> {
        self.initialiser_etat(&Etat::initial(p)?)
    }
    pub fn initialiser_etat(&mut self, base: &Etat) -> Resultat<Etat> {
        if let Some(etat) = self.lire()? {
            if etat.projet != base.projet {
                return Err("Autre projet dans ce dépôt".into());
            }
            return Ok(etat);
        }
        if base.revision != 0 || !base.acceptes.is_empty() {
            return Err("Dépôt central absent : restauration explicite nécessaire".into());
        }
        let etat = base.clone();
        let id = self.depot.sauver("instantanes", &etat)?;
        self.ajouter(&Evenement::Initial(id))?;
        Ok(etat)
    }
    pub fn reserver(
        &mut self,
        etat: &mut Etat,
        cles: &[String],
        session: &str,
        reprise: bool,
        liberer: bool,
    ) -> Resultat<()> {
        if !self.valide {
            return Err("Protection invalidée : réacquérir et relire".into());
        }
        // Validation de toutes les clés avant de changer le moindre bail.
        if !reprise
            && cles.iter().any(|k| {
                etat.baux
                    .get(k)
                    .is_some_and(|b| b.actif && b.proprietaire != session)
            })
        {
            return Err("Réservé par un autre poste ; reprise explicite nécessaire".into());
        }
        let mut baux = etat.baux.clone();
        for k in cles {
            let b = baux.entry(k.clone()).or_insert(Bail {
                proprietaire: String::new(),
                generation: 0,
                actif: false,
            });
            if liberer {
                if b.proprietaire != session && !reprise {
                    return Err("Libération d'un autre propriétaire interdite".into());
                }
                b.actif = false;
            } else if !b.actif || b.proprietaire != session || reprise {
                b.generation += 1;
                b.proprietaire = session.into();
                b.actif = true;
            }
        }
        self.ajouter(&Evenement::Reservation(baux.clone()))?;
        etat.baux = baux;
        Ok(())
    }
    pub fn publier(&mut self, etat: &mut Etat, t: &Transaction) -> Resultat<Vec<Conflit>> {
        if !self.valide {
            return Err("Protection invalidée : réacquérir et relire".into());
        }
        if let Some(digest) = etat.acceptes.get(&t.id) {
            if digest != &hash(t) {
                return Err("UUID de transaction réutilisé avec un contenu différent".into());
            }
            return Ok(Vec::new());
        }
        let suivant = match etat.accepter(t) {
            Ok(s) => s,
            Err(c) => return Ok(c),
        };
        let paquet = self.depot.sauver("paquets", t)?;
        for id in &t.ressources {
            self.depot.lire("ressources", id)?;
        }
        self.ajouter(&Evenement::Accepte {
            paquet,
            resultat: hash(&suivant),
        })?;
        *etat = suivant;
        Ok(Vec::new())
    }
    pub fn instantane(&mut self, etat: &Etat) -> Resultat<String> {
        if !self.valide {
            return Err("Protection invalidée : réacquérir et relire".into());
        }
        let id = self.depot.sauver("instantanes", etat)?;
        self.ajouter(&Evenement::Instantane {
            revision: etat.revision,
            ressource: id.clone(),
        })?;
        self.controle
            .seek(SeekFrom::Start(0))
            .map_err(|e| e.to_string())?;
        let mut bytes = Vec::new();
        self.controle
            .read_to_end(&mut bytes)
            .map_err(|e| e.to_string())?;
        let repere = Repere {
            offset: bytes.len(),
            prefixe: empreinte(&bytes),
            instantane: id.clone(),
            contenu: hash(etat),
        };
        // Le repère est un accélérateur. Le journal reste intégral et vérifié par
        // empreinte avant de sauter son préfixe ; aucun état CRDT n'est purgé.
        sauver_local(
            &self.depot.racine.join("repere-instantane.json"),
            &encoder(&repere)?,
        )?;
        Ok(id)
    }
}

/// Sauvegarde locale durable sans remplacer un dossier partagé ou son contrôle.
pub fn sauver_local(path: &Path, bytes: &[u8]) -> Resultat<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    crate::echanges::enregistrement::ecrire_atomique(path, bytes)
}
