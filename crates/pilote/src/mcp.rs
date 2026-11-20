//! Serveur MCP de l'instance IFC/Revit (stdio, JSON-RPC 2.0), lancé par `bimedit --mcp`.
//!
//! Il pilote une instance par son canal local (voir `crates/pilote/src/lib.rs`) : une ligne
//! « @jeton commande argument » envoyée, une ligne JSON reçue. Les instances ouvertes, lancées
//! par l'utilisateur ou par le pont, sont lues dans le registre des instances
//! (`instances.rs`) ; `application_instances` les liste et en choisit une. Sans instance, il en
//! lance une (`BIMEDIT_EXE`, sinon ce même exécutable sans argument).
//!
//! Outils : ouvrir un fichier, capturer la vue ou la fenêtre (image PNG rendue au client),
//! lister et ouvrir les vues Revit, régler la caméra, lire les propriétés, désigner
//! l'objet sous un point, et une commande brute pour le reste.

use serde_json::{Map, Value, json};
use std::io::{BufRead, Read, Write};
use std::net::{Ipv4Addr, SocketAddr, TcpStream};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

/// Racine du workspace (le crate est dans `crates/app`) : `target/` et les exemples y sont.
const RACINE: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../..");
const NOM_EXE: &str = if cfg!(windows) { "bimedit.exe" } else { "bimedit" };

const AIDE: &str = include_str!("../../../docs/PILOTE.md");
const COMMANDES: &str = "Commandes BIMedit : aide ; etat ; nouveau <JSON> ; ouvrir <chemin> ; fermer [abandonner] ; importer <JSON> ; importer_dans <chemin.ifc> ; enregistrer [chemin] ; projet_natif ; editer <JSON> ; modifier_projet <JSON Patch avec empreinte> ; selection_native <JSON> ; outil <nom> ; saisie <JSON> ; annuler ; retablir ; historique ; historique_parcourir <JSON> ; exporter <chemin.svg> ; export <JSON> ; exporter_pdf <chemin> ; exporter_ifc <chemin> ; exporter_csv <JSON> ; exporter_jeu <chemin> [jeu] ; nomenclature <JSON> ; documents ; document <id|famille:rang> ; fermer_vue [autres] ; familles ; famille_editer <id> ; famille_ouvrir|famille_charger|famille_enregistrer <chemin> ; famille_vue [detail faible|moyen|eleve|apercu oui|non] ; famille_visibilite volume|extrusion|element_2d <indice> <JSON> ; famille_plan_tourne <JSON> ; famille_angle <JSON> ; famille_plan_actif [etat|sol|plan <nom>|face] ; famille_ecran <h> <v> ; niveau_detail [faible|moyen|eleve] ; collaborer <chemin> ; collaboration_associer <chemin> ; collaboration_sync ; collaboration_instantane ; collaboration_etat ; collaboration_reserver|collaboration_liberer|collaboration_reprendre <ids JSON> ; collaboration_resoudre <JSON> ; options [JSON] ; gabarit [chemin|defaut] ; ruban <onglet> ; vues ; vue <nom|indice> ; feuilles ; selection_feuille <JSON> ; proprietes_feuille ; camera <mode|angles> ; zoom <facteur> ; zone <bornes|on|off|ajuster> ; glisser <dx dy bouton> ; capture vue|fenetre ; taille <L H> ; reduire ; restaurer ; quitter. La commande aide ou application_aide détaille TOUTES les syntaxes, les unités et les outils de modélisation, annotation et visibilité.";

/// Résultat d'un outil : contenu MCP et indicateur d'erreur.
type Sortie = (Vec<Value>, bool);

fn dossier_captures() -> PathBuf {
    std::env::temp_dir().join("bimedit-mcp")
}

fn executable() -> Result<PathBuf, String> {
    // variable explicite d'abord, puis ce même exécutable (pont et instance de la même version)
    let mut candidats = Vec::new();
    if let Some(e) = std::env::var_os("BIMEDIT_EXE").filter(|e| !e.is_empty()) {
        candidats.push(PathBuf::from(e));
    }
    if let Ok(e) = std::env::current_exe() {
        candidats.push(e);
    }
    candidats.push(Path::new(RACINE).join("target").join("release").join(NOM_EXE));
    candidats
        .into_iter()
        .find(|c| c.is_file())
        .ok_or_else(|| "instance introuvable : construire avec cargo build --release ou fixer BIMEDIT_EXE".to_string())
}

use crate::instances::{self, Instance};

/// Instance jointe : port et jeton (absent en mode isolé sans `BIMEDIT_JETON`).
#[derive(Clone)]
struct Cible {
    pid: Option<u32>,
    port: u16,
    jeton: Option<String>,
}

/// Instance choisie par `application_instances` ou jointe en dernier.
static CHOISIE: std::sync::Mutex<Option<Cible>> = std::sync::Mutex::new(None);

fn cible_isolee() -> Option<Cible> {
    instances::port_isole().map(|port| Cible { pid: None, port, jeton: instances::jeton_impose() })
}

fn cible_de(i: &Instance) -> Cible {
    Cible { pid: Some(i.pid), port: i.port, jeton: Some(i.jeton.clone()) }
}

/// Envoie une ligne à l'instance et lit la ligne JSON de réponse.
fn envoyer(c: &Cible, ligne: &str, delai: Duration) -> std::io::Result<Value> {
    if ligne.contains(['\n','\r']) { return Err(std::io::Error::new(std::io::ErrorKind::InvalidInput,"Une seule ligne de commande est autorisée")); }
    let adresse = SocketAddr::from((Ipv4Addr::LOCALHOST, c.port));
    let mut flux = TcpStream::connect_timeout(&adresse, delai.min(Duration::from_secs(5)))?;
    flux.set_read_timeout(Some(delai))?;
    flux.set_write_timeout(Some(delai))?;
    let ligne = match &c.jeton { Some(j) => format!("@{j} {ligne}\n"), None => format!("{ligne}\n") };
    flux.write_all(ligne.as_bytes())?;
    let mut tampon = Vec::new();
    let mut bloc = [0u8; 65536];
    while !tampon.ends_with(b"\n") {
        let n = flux.read(&mut bloc)?;
        if n == 0 {
            break;
        }
        tampon.extend_from_slice(&bloc[..n]);
    }
    serde_json::from_slice(&tampon).map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))
}

/// Instances inscrites qui répondent avec leur jeton ; les inscriptions mortes sont
/// effacées (instance arrêtée brutalement, ou pid réutilisé par un autre programme).
fn vivantes() -> Vec<(Instance, Value)> {
    instances::lire()
        .into_iter()
        .filter_map(|i| match envoyer(&cible_de(&i), "etat", Duration::from_secs(3)) {
            Ok(e) if est_ok(&e) => Some((i, e)),
            _ => {
                instances::oublier(i.pid);
                None
            }
        })
        .collect()
}

/// Lance une nouvelle instance et attend qu'elle soit inscrite et réponde.
fn lancer() -> Result<Cible, String> {
    let exe = executable()?;
    let mut cmd = Command::new(&exe);
    cmd.current_dir(RACINE).stdin(Stdio::null()).stdout(Stdio::null()).stderr(Stdio::null());
    if !Path::new(RACINE).is_dir() {
        cmd.current_dir(std::env::current_dir().unwrap_or_default());
    }
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const DETACHED_PROCESS: u32 = 0x0000_0008;
        const CREATE_NEW_PROCESS_GROUP: u32 = 0x0000_0200;
        cmd.creation_flags(DETACHED_PROCESS | CREATE_NEW_PROCESS_GROUP);
    }
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        cmd.process_group(0);
    }
    let pid = cmd.spawn().map_err(|e| format!("lancement de {} impossible : {e}", exe.display()))?.id();
    let fin = Instant::now() + Duration::from_secs(30);
    while Instant::now() < fin {
        let c = match cible_isolee() {
            Some(c) => Some(c),
            None => instances::lire().iter().find(|i| i.pid == pid).map(cible_de),
        };
        if let Some(c) = c
            && envoyer(&c, "etat", Duration::from_secs(5)).is_ok()
        {
            return Ok(c);
        }
        std::thread::sleep(Duration::from_millis(300));
    }
    Err(format!("l'instance lancée ({}, pid {pid}) ne répond pas", exe.display()))
}

fn liste_instances(vivantes: &[(Instance, Value)]) -> Value {
    let choisie = CHOISIE.lock().unwrap_or_else(|e| e.into_inner()).as_ref().and_then(|c| c.pid);
    Value::Array(vivantes.iter().map(|(i, e)| json!({
        "pid": i.pid,
        "port": i.port,
        "fichier": i.fichier,
        "partage": i.partage,
        "modifie": e["document_actif"]["modifie"],
        "debut": i.debut,
        "choisie": choisie == Some(i.pid),
    })).collect())
}

/// Instance à joindre : l'instance choisie si elle répond, sinon la seule vivante, sinon un
/// une nouvelle instance. Plusieurs instances sans choix : refus, pour ne pas modifier la
/// mauvaise fenêtre.
fn cible() -> Result<Cible, String> {
    if let Some(c) = cible_isolee() {
        return Ok(c);
    }
    let choisie = CHOISIE.lock().unwrap_or_else(|e| e.into_inner()).clone();
    if let Some(c) = choisie
        && envoyer(&c, "etat", Duration::from_secs(3)).is_ok()
    {
        return Ok(c);
    }
    *CHOISIE.lock().unwrap_or_else(|e| e.into_inner()) = None;
    let v = vivantes();
    let c = match v.as_slice() {
        [] => lancer()?,
        [(i, _)] => cible_de(i),
        _ => return Err(format!(
            "Plusieurs instances BIMedit sont ouverts : choisir avec application_instances (action choisir, pid). Instances : {}",
            liste_instances(&v)
        )),
    };
    *CHOISIE.lock().unwrap_or_else(|e| e.into_inner()) = Some(c.clone());
    Ok(c)
}

fn commande_delai(ligne: &str, delai: Duration) -> Result<Value, String> {
    let c = cible()?;
    match envoyer(&c, ligne, delai) {
        // instance fermée entre la vérification et l'envoi : nouvelle résolution (relance
        // seulement s'il n'en reste aucun, sinon le choix revient à l'agent)
        Err(e) if e.kind() == std::io::ErrorKind::ConnectionRefused => {
            *CHOISIE.lock().unwrap_or_else(|e| e.into_inner()) = None;
            let c = cible()?;
            envoyer(&c, ligne, delai).map_err(|e| e.to_string())
        }
        autre => autre.map_err(|e| e.to_string()),
    }
}

fn commande(ligne: &str) -> Result<Value, String> {
    commande_delai(ligne, Duration::from_secs(600))
}

/// Outil `application_instances` : lister, choisir ou lancer une instance.
fn instances_outil(a: &Map<String, Value>) -> Result<Sortie, String> {
    if cible_isolee().is_some() {
        return Err("Mode isolé (BIMEDIT_PORT) : une seule instance, sur ce port.".into());
    }
    match a.get("action").and_then(Value::as_str).unwrap_or("lister") {
        "lister" => {
            let v = vivantes();
            Ok(reponse(json!({"ok": true, "instances": liste_instances(&v)})))
        }
        "choisir" => {
            let pid = a.get("pid").and_then(Value::as_u64).ok_or("pid requis (voir action lister)")? as u32;
            let v = vivantes();
            let (i, _) = v.iter().find(|(i, _)| i.pid == pid).ok_or_else(|| format!("aucune instance joignable avec le pid {pid}"))?;
            *CHOISIE.lock().unwrap_or_else(|e| e.into_inner()) = Some(cible_de(i));
            Ok(reponse(json!({"ok": true, "instances": liste_instances(&v)})))
        }
        "nouvelle" => {
            let c = lancer()?;
            *CHOISIE.lock().unwrap_or_else(|e| e.into_inner()) = Some(c);
            let v = vivantes();
            Ok(reponse(json!({"ok": true, "instances": liste_instances(&v)})))
        }
        _ => Err("action : lister|choisir|nouvelle".into()),
    }
}

fn est_ok(r: &Value) -> bool {
    r.get("ok").and_then(Value::as_bool).unwrap_or(false)
}

fn texte(r: &Value) -> Value {
    json!({"type": "text", "text": serde_json::to_string_pretty(r).unwrap_or_default()})
}

/// Texte de la réponse, en erreur si l'instance répond `ok: false`.
fn reponse(r: Value) -> Sortie {
    let erreur = !est_ok(&r);
    (vec![texte(&r)], erreur)
}

fn base64(octets: &[u8]) -> String {
    const T: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity(octets.len().div_ceil(3) * 4);
    for bloc in octets.chunks(3) {
        let n = (bloc[0] as u32) << 16 | (*bloc.get(1).unwrap_or(&0) as u32) << 8 | *bloc.get(2).unwrap_or(&0) as u32;
        out.push(T[(n >> 18) as usize & 63] as char);
        out.push(T[(n >> 12) as usize & 63] as char);
        out.push(if bloc.len() > 1 { T[(n >> 6) as usize & 63] as char } else { '=' });
        out.push(if bloc.len() > 2 { T[n as usize & 63] as char } else { '=' });
    }
    out
}

fn capture(zone: &str, largeur: Option<String>, hauteur: Option<String>) -> Result<Sortie, String> {
    let mut taille = String::new();
    if let (Some(l), Some(h)) = (largeur.filter(|l| !l.is_empty()), hauteur.filter(|h| !h.is_empty())) {
        if zone == "fenetre" {
            commande(&format!("taille {l} {h}"))?;
            std::thread::sleep(Duration::from_millis(400)); // le redimensionnement arrive à l'image suivante
        } else {
            taille = format!("{l}x{h} "); // rendu hors écran à cette taille
        }
    }
    let dossier = dossier_captures();
    std::fs::create_dir_all(&dossier).map_err(|e| e.to_string())?;
    let ms = SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_millis()).unwrap_or(0);
    let chemin = dossier.join(format!("capture-{ms}.png"));
    let r = commande_delai(&format!("capture {zone} {taille}{}", chemin.display()), Duration::from_secs(60))?;
    if !est_ok(&r) {
        return Ok((vec![texte(&r)], true));
    }
    let donnees = base64(&std::fs::read(&chemin).map_err(|e| e.to_string())?);
    Ok((vec![texte(&r), json!({"type": "image", "data": donnees, "mimeType": "image/png"})], false))
}

fn obj(props: Value, requis: &[&str]) -> Value {
    json!({"type": "object", "properties": props, "required": requis})
}

fn outils() -> Vec<(&'static str, String, Value)> {
    let mut liste = vec![
        ("application_etat", "État de l'instance : fichier ouvert, vue active, caméra, classes masquées, sélection. Lance l'instance si elle ne tourne pas.".into(), obj(json!({}), &[])),
        ("application_ouvrir", "Ouvre un projet natif, partagé (.bimpartage), IFC ou Revit (.rvt, .rfa, .rte) et attend la fin du chargement.".into(),
         obj(json!({"chemin": {"type": "string", "description": "Chemin absolu du fichier"}}), &["chemin"])),
        ("application_capture", "Capture de l'instance, rendue en image PNG. zone « vue » (défaut) : vue 3D rendue hors écran, à la taille de la vue ou à largeur × hauteur, même fenêtre réduite. zone « fenetre » : interface entière ; largeur et hauteur redimensionnent alors la fenêtre.".into(),
         obj(json!({"zone": {"type": "string", "enum": ["vue", "fenetre"]}, "largeur": {"type": "integer"}, "hauteur": {"type": "integer"}}), &[])),
        ("application_vues", "Liste des vues, feuilles et nomenclatures du fichier Revit ouvert (indice, genre, type, nom, échelle).".into(), obj(json!({}), &[])),
        ("application_feuilles", "Liste les jeux natifs, leurs feuilles et leurs vues et nomenclatures placées.".into(), obj(json!({}), &[])),
        ("application_selection_feuille", "Ouvre un jeu et sélectionne éventuellement une feuille puis une vue ou une nomenclature placée ; sans feuille, sélectionne le jeu.".into(),
         obj(json!({"jeu": {"type": "string"}, "feuille": {"type": "string", "description": "Identifiant natif de la feuille"}, "vue": {"type": "integer", "minimum": 0, "description": "Indice de la vue placée, puis des nomenclatures"}}), &["jeu"])),
        ("application_proprietes_feuille", "Lit les propriétés affichées de la sélection dans le jeu ouvert.".into(), obj(json!({}), &[])),
        ("application_modifier_feuille", "Modifie une feuille native avec son objet complet obtenu par application_feuilles ou application_proprietes_feuille. La modification est enregistrée dans l'historique et nécessite ensuite un enregistrement du projet.".into(),
         obj(json!({"feuille": {"type": "object", "description": "Objet feuille complet avec son identifiant existant"}}), &["feuille"])),
        ("application_vue", "Ouvre une vue Revit comme un double-clic dans l'arborescence : par indice (voir application_vues) ou par nom (« Plan d'étage : Niveau 0 », ou une partie du nom).".into(),
         obj(json!({"vue": {"type": "string"}}), &["vue"])),
        ("application_camera", "Règle la caméra : mode dessus|face|iso|cadrer|ortho|perspective, ou lacet et tangage en degrés (et distance en mètres).".into(),
         obj(json!({"mode": {"type": "string"}, "lacet": {"type": "number"}, "tangage": {"type": "number"}, "distance": {"type": "number"}}), &[])),
        ("application_proprietes", "Attributs et jeux de propriétés d'une entité (id STEP, ou la sélection si absent).".into(),
         obj(json!({"id": {"type": "integer"}}), &[])),
        ("application_objet", "Objet visible sous un point de la vue (x, y de 0 à 1 depuis le coin haut gauche) et point 3D touché.".into(),
         obj(json!({"x": {"type": "number"}, "y": {"type": "number"}}), &["x", "y"])),
        ("application_commande", format!("Commande brute de l'instance.\n{COMMANDES}"),
         obj(json!({"commande": {"type": "string", "description": "Ligne « commande argument »"}}), &["commande"])),
    ];
    liste.extend(outils_complets());
    liste
}

/// Argument rendu comme texte (les chaînes sans guillemets, les nombres tels quels).
fn arg(a: &Map<String, Value>, cle: &str) -> Result<String, String> {
    match a.get(cle) {
        Some(Value::String(s)) => Ok(s.clone()),
        Some(Value::Null) | None => Err(format!("argument manquant : {cle}")),
        Some(v) => Ok(v.to_string()),
    }
}

fn arg_opt(a: &Map<String, Value>, cle: &str) -> Option<String> {
    arg(a, cle).ok()
}

fn outils_complets() -> Vec<(&'static str, String, Value)> {
    let texte = json!({"type":"string","minLength":1});
    let ids = json!({"type":"array","items":{"type":"string"}});
    vec![
        ("application_aide", "Documentation complète du pilote ; sujets edition/modele pour les formats JSON, commandes pour le contrat métier Rust sérialisable. Sans lancer BIMedit.".into(),obj(json!({"sujet":{"type":"string","enum":["pilote","edition","modele","commandes"]}}),&[])),
        ("application_nouveau", "Crée un projet natif sans dialogue et l'ouvre en conservant les documents précédents. Refuse un chemin existant.".into(),obj(json!({"chemin":texte,"nom":texte}),&["chemin"])),
        ("application_importer", "Convertit IFC/RVT/RFA/RTE en nouveau projet natif sans dialogue. Consulter application_etat jusqu'à chargement=false puis vérifier erreur_chargement.".into(),obj(json!({"source":texte,"sortie":texte}),&["source","sortie"])),
        ("application_projet_natif", "Lit le document JSON complet et son empreinte pour une modification optimiste.".into(),obj(json!({}),&[])),
        ("application_editer", "Exécute une commande métier BIM atomique, annulable et collaborative. Les coordonnées sont en mm. Voir application_aide et docs/EDITION.md.".into(),obj(json!({"transaction":{"type":"object","required":["action"],"properties":{"action":{"type":"string","enum":["terrain","limite_propriete","coordonnees_origine","barre","escalier","garde_corps","modifier_vue","modifier_quadrillage","niveau","quadrillage","annoter","feuille","nomenclature","modifier_niveau","ouverture","deplacer","copier","copier_sur_niveau","tourner","aligner","ajuster","ajuster_angle","modifier_trajectoire_mur","actualiser_type","attacher","priorite_jonction","supprimer","contour","tremie","fusionner"]}}}}),&["transaction"])),
        ("application_modifier_projet", "Modifie toutes les propriétés natives (matériaux, types, familles, phases, vues, plumes…). JSON Patch atomique, validé, avec historique et collaboration. Empreinte requise de application_projet_natif ; refus si le document a changé.".into(),obj(json!({"empreinte":texte,"operations":{"type":"array","minItems":1,"items":{"type":"object","required":["op","path"],"properties":{"op":{"type":"string","enum":["add","replace","remove","test","copy","move"]},"path":{"type":"string"},"from":{"type":"string"},"value":{}}}}}),&["empreinte","operations"])),
        ("application_selection_native", "Sélectionne plusieurs éléments par identifiants natifs ; tableau vide pour désélectionner.".into(),obj(json!({"ids":ids}),&["ids"])),
        ("application_outil", "Active un outil opérationnel du ruban. Voir application_aide pour la liste ; utiliser application_saisie pour les réglages et tracés, ou application_editer pour une édition directe.".into(),obj(json!({"outil":texte}),&["outil"])),
        ("application_saisie", "Envoie des entrées graphiques egui (clic, pointer, bouton, defiler, texte, touche) en points logiques de la fenêtre. Une étape par image ; attendre application_etat.saisies_en_attente=0. Ne pilote pas les dialogues natifs du système.".into(),obj(json!({"entrees":{"type":"array","items":{"type":"object","required":["action"],"properties":{"action":{"type":"string","enum":["clic","pointer","bouton","defiler","texte","touche"]},"x":{"type":"number"},"y":{"type":"number"},"dx":{"type":"number"},"dy":{"type":"number"},"bouton":{"type":"string","enum":["gauche","droit","milieu"]},"appuye":{"type":"boolean"},"texte":{"type":"string"},"touche":{"type":"string"},"maj":{"type":"boolean"},"ctrl":{"type":"boolean"},"alt":{"type":"boolean"}}}}}),&["entrees"])),
        ("application_exporter", "Exporte sans dialogue : dessin SVG/PDF, projet IFC4, nomenclature CSV (id requis), jeu de feuilles PDF (jeu optionnel), feuille seule SVG/PDF (id requis).".into(),obj(json!({"format":{"type":"string","enum":["svg","pdf","ifc","csv","jeu_pdf","feuille_svg","feuille_pdf"]},"chemin":texte,"id":texte,"jeu":texte}),&["format","chemin"])),
        ("application_collaboration", "État résumé (révision, baux, propositions en attente, conflits ; etat_complet : copie entière, volumineuse), création/association, synchro, instantané, réservation/libération/reprise explicite (ids : identifiants natifs ou clés collection/id) ou résolution d'un conflit (transaction : identifiant ou préfixe non ambigu). Les opérations de synchronisation sont asynchrones ; vérifier l'état ensuite.".into(),obj(json!({"action":{"type":"string","enum":["etat","etat_complet","creer","associer","sync","instantane","reserver","liberer","reprendre","resoudre"]},"chemin":texte,"ids":ids,"transaction":texte,"choix":{"type":"string","enum":["locale","acceptee"]}}),&["action"])),
        ("application_options", "Lit les options, ou modifie et mémorise les options fournies (unités de l'interface et navigation IFC). Les commandes restent en mm.".into(),obj(json!({"navigation_ifc":{"type":"boolean"},"unite":{"type":"string","enum":["millimetre","centimetre","metre"]},"delai_accrochage":{"type":"number","minimum":0.1,"maximum":5},"raccourcis":{"type":"object","additionalProperties":{"type":"string"}}}),&[])),
        ("application_famille", "Éditeur de familles sans dialogue : nouvelle, lire, modifier le fichier complet, charger dans le projet, fermer, ouvrir/enregistrer un fichier, importer un type dans le projet ; plans tournés (plan_tourne : nom, hote, appui, angle en degrés ; angle : plan et valeur, formule de la cote angulaire, null la retire), plan de dessin actif (plan_actif : choix etat|sol|plan <nom>|face) et position écran d'un point du plan actif en vue 3D (ecran : h, v en mm) pour y saisir à la souris (application_saisie). Lire avant de modifier ; charger propage les occurrences via l'historique et la collaboration.".into(),obj(json!({"action":{"type":"string","enum":["nouvelle","lire","modifier","charger","fermer","ouvrir","enregistrer","importer","plan_tourne","angle","plan_actif","ecran"]},"nom":texte,"hote":texte,"appui":texte,"angle":{"type":"number"},"plan":texte,"valeur":{"type":["string","number","null"]},"choix":texte,"h":{"type":"number"},"v":{"type":"number"},"genre":{"type":"string","enum":["porte","fenetre","composant","profil","cartouche","etiquette"]},"fichier":{"type":"object"},"chemin":texte,"nouveau":{"type":"boolean"},"abandonner":{"type":"boolean"}}),&["action"])),
        ("application_instances", "Instances BIMedit ouvertes (lancées par l'utilisateur ou par le pont) : lister (pid, port, fichier, partage, modifie, choisie), choisir celui que les autres outils pilotent, ou lancer une nouvelle instance. Avec plusieurs instances ouverts, choisir avant toute autre commande. Deux instances associées au même .bimpartage travaillent sur le même projet en collaboration.".into(),obj(json!({"action":{"type":"string","enum":["lister","choisir","nouvelle"]},"pid":{"type":"integer"}}),&[])),
        ("application_historique", "Lit l'historique, ou annule/rétablit un nombre d'étapes (une transaction à la fois en collaboration).".into(),obj(json!({"action":{"type":"string","enum":["annuler","retablir"]},"nombre":{"type":"integer","minimum":1}}),&[])),
    ]
}

/// Construction testable, indépendante du canal TCP ; aucun argument n'est exécuté par un shell.
fn ligne_complete(nom: &str, a: &Map<String, Value>) -> Result<Option<String>, String> {
    let json = |v: &Value| serde_json::to_string(v).map_err(|e|e.to_string());
    let objet = Value::Object(a.clone());
    let ligne = match nom {
        "application_nouveau" => format!("nouveau {}",json(&objet)?),
        "application_importer" => format!("importer {}",json(&objet)?),
        "application_projet_natif" => "projet_natif".into(),
        "application_editer" => format!("editer {}",json(a.get("transaction").filter(|v|v.is_object()).ok_or("Objet transaction requis")?)?),
        "application_modifier_projet" => format!("modifier_projet {}",json(&objet)?),
        "application_selection_native" => format!("selection_native {}",json(a.get("ids").filter(|v|v.is_array()).ok_or("Tableau ids requis")?)?),
        "application_outil" => format!("outil {}",arg(a,"outil")?),
        "application_saisie" => format!("saisie {}",json(a.get("entrees").filter(|v|v.is_array()).ok_or("Tableau entrees requis")?)?),
        "application_exporter" => format!("export {}",json(&objet)?),
        "application_options" => format!("options {}",json(&objet)?),
        "application_famille" => match arg(a,"action")?.as_str() {
            "nouvelle"=>format!("famille_nouvelle {}",arg(a,"genre")?),"lire"=>"famille_document".into(),
            "modifier"=>format!("famille_modifier {}",json(a.get("fichier").filter(|v|v.is_object()).ok_or("Objet fichier requis")?)?),
            "charger"=>format!("famille_charger_projet {}",if a.get("nouveau").and_then(Value::as_bool).unwrap_or(false) {"nouveau"} else {""}),
            "fermer"=>format!("famille_fermer {}",if a.get("abandonner").and_then(Value::as_bool).unwrap_or(false) {"abandonner"} else {""}),
            "ouvrir"=>format!("famille_ouvrir {}",arg(a,"chemin")?),"enregistrer"=>format!("famille_enregistrer {}",arg(a,"chemin")?),"importer"=>format!("famille_charger {}",arg(a,"chemin")?),
            "plan_tourne"=>{ let mut v=objet.clone(); v.as_object_mut().map(|o|o.remove("action")); format!("famille_plan_tourne {}",json(&v)?) }
            "angle"=>format!("famille_angle {}",json(&json!({"plan":arg(a,"plan")?,"valeur":a.get("valeur").cloned().unwrap_or(Value::Null)}))?),
            "plan_actif"=>format!("famille_plan_actif {}",a.get("choix").and_then(Value::as_str).unwrap_or("etat")),
            "ecran"=>format!("famille_ecran {} {}",a.get("h").and_then(Value::as_f64).ok_or("h requis")?,a.get("v").and_then(Value::as_f64).ok_or("v requis")?),
            _=>return Err("Action de famille inconnue".into()),
        },
        "application_historique" => if a.contains_key("action") {
            let mut v=objet; if v.get("nombre").is_none() {v["nombre"]=1.into();}
            format!("historique_parcourir {}",json(&v)?)
        } else {"historique".into()},
        "application_collaboration" => match arg(a,"action")?.as_str() {
            "etat"=>"collaboration_etat".into(),"etat_complet"=>"collaboration_etat complet".into(),"sync"=>"collaboration_sync".into(),"instantane"=>"collaboration_instantane".into(),
            "creer"=>format!("collaborer {}",arg(a,"chemin")?),"associer"=>format!("collaboration_associer {}",arg(a,"chemin")?),
            action @ ("reserver"|"liberer"|"reprendre")=>format!("collaboration_{action} {}",a.get("ids").map(json).transpose()?.unwrap_or_default()),
            "resoudre"=>format!("collaboration_resoudre {}",json(&json!({"transaction":arg(a,"transaction")?,"choix":arg(a,"choix")?}))?),
            _=>return Err("Action collaborative inconnue".into()),
        },
        _=>return Ok(None),
    };
    Ok(Some(ligne))
}

/// Nombre non nul (équivalent d'un « truthy »).
fn non_nul(v: Option<&Value>) -> bool {
    v.and_then(Value::as_f64).is_some_and(|n| n != 0.0)
}

/// Jeton « LxH » de la commande `capture`.
fn dimensions(mot: &str) -> Option<(&str, &str)> {
    let (l, h) = mot.split_once('x')?;
    let chiffres = mot.replace('x', "");
    (!chiffres.is_empty() && chiffres.bytes().all(|c| c.is_ascii_digit())).then_some((l, h))
}

fn capture_brute(ligne: &str) -> Result<Sortie, String> {
    let mots: Vec<&str> = ligne.split_whitespace().collect();
    let zone = match mots.get(1) {
        Some(&"fenetre") => "fenetre",
        _ => "vue",
    };
    let (l, h) = mots.iter().skip(2).find_map(|m| dimensions(m)).unzip();
    capture(zone, l.map(str::to_string), h.map(str::to_string))
}

fn appeler(nom: &str, a: &Map<String, Value>) -> Result<Sortie, String> {
    if nom == "application_aide" {
        let texte=match a.get("sujet").and_then(Value::as_str).unwrap_or("pilote") {
            "pilote"=>AIDE,"edition"=>include_str!("../../../docs/EDITION.md"),"modele"=>include_str!("../../../docs/MODELE.md"),"commandes"=>include_str!("../../commandes/src/socle/commande.rs"),
            _=>return Err("Sujet : pilote|edition|modele|commandes".into()),
        };
        return Ok((vec![json!({"type":"text","text":texte})],false));
    }
    if nom == "application_instances" { return instances_outil(a); }
    if let Some(ligne) = ligne_complete(nom,a)? { return Ok(reponse(commande(&ligne)?)); }
    match nom {
        "application_etat" => Ok(reponse(commande("etat")?)),
        "application_ouvrir" => Ok(reponse(commande(&format!("ouvrir {}", arg(a, "chemin")?))?)),
        "application_capture" => {
            let zone = a.get("zone").and_then(Value::as_str).unwrap_or("vue");
            capture(zone, arg_opt(a, "largeur"), arg_opt(a, "hauteur"))
        }
        "application_vues" => Ok(reponse(commande("vues")?)),
        "application_feuilles" => Ok(reponse(commande("feuilles")?)),
        "application_selection_feuille" => {
            let corps = serde_json::to_string(a).map_err(|e| e.to_string())?;
            Ok(reponse(commande(&format!("selection_feuille {corps}"))?))
        }
        "application_proprietes_feuille" => Ok(reponse(commande("proprietes_feuille")?)),
        "application_modifier_feuille" => {
            let feuille = a.get("feuille").unwrap_or(&Value::Null);
            let Some(identifiant) = feuille.get("id").filter(|i| !i.is_null() && *i != "") else {
                return Err("Indiquer l'identifiant d'une feuille existante".into());
            };
            let existantes = commande("feuilles")?;
            if !est_ok(&existantes) {
                return Ok((vec![texte(&existantes)], true));
            }
            let trouvee = existantes["jeux"]
                .as_array()
                .into_iter()
                .flatten()
                .flat_map(|jeu| jeu["feuilles"].as_array().into_iter().flatten())
                .any(|f| f.get("id") == Some(identifiant));
            if !trouvee {
                return Err("Feuille introuvable".into());
            }
            let corps = serde_json::to_string(&json!({"action": "feuille", "feuille": feuille})).map_err(|e| e.to_string())?;
            Ok(reponse(commande(&format!("editer {corps}"))?))
        }
        "application_vue" => Ok(reponse(commande(&format!("vue {}", arg(a, "vue")?))?)),
        "application_camera" => {
            let ligne = if let Some(mode) = a.get("mode").and_then(Value::as_str).filter(|m| !m.is_empty()) {
                format!("camera {mode}")
            } else {
                let nombre = |cle: &str, defaut: i32| a.get(cle).filter(|v| !v.is_null()).map_or(defaut.to_string(), Value::to_string);
                let mut ligne = format!("camera {} {}", nombre("lacet", -90), nombre("tangage", 30));
                if non_nul(a.get("distance")) {
                    ligne += &format!(" {}", a["distance"]);
                }
                ligne
            };
            Ok(reponse(commande(&ligne)?))
        }
        "application_proprietes" => {
            let ligne = format!("proprietes {}", arg_opt(a, "id").unwrap_or_default());
            Ok(reponse(commande(ligne.trim())?))
        }
        "application_objet" => Ok(reponse(commande(&format!("objet {} {}", arg(a, "x")?, arg(a, "y")?))?)),
        "application_commande" => {
            let ligne = arg(a, "commande")?;
            let ligne = ligne.trim();
            if ligne.starts_with("capture") {
                return capture_brute(ligne);
            }
            Ok(reponse(commande(ligne)?))
        }
        _ => Err(format!("outil inconnu : {nom}")),
    }
}

fn repondre(id: &Value, resultat: Result<Value, Value>) {
    let mut m = json!({"jsonrpc": "2.0", "id": id});
    match resultat {
        Ok(r) => m["result"] = r,
        Err(e) => m["error"] = e,
    }
    let mut sortie = std::io::stdout().lock();
    let _ = writeln!(sortie, "{m}");
    let _ = sortie.flush();
}

fn traiter(msg: &Value) {
    let id = msg.get("id").cloned().unwrap_or(Value::Null);
    if id.is_null() {
        return; // notification
    }
    let methode = msg.get("method").and_then(Value::as_str).unwrap_or("");
    let vide = Map::new();
    let p = msg.get("params").and_then(Value::as_object).unwrap_or(&vide);
    match methode {
        "initialize" => repondre(&id, Ok(json!({
            "protocolVersion": p.get("protocolVersion").cloned().unwrap_or_else(|| json!("2025-06-18")),
            "capabilities": {"tools": {}},
            "serverInfo": {"name": "bimedit", "version": "1.0"},
            "instructions": "Pilote BIMedit : application_aide décrit toutes les capacités. application_projet_natif puis application_editer ou application_modifier_projet pour les transactions ; application_outil puis application_saisie pour tracer. application_collaboration pilote synchronisation et conflits. application_capture permet de vérifier le rendu.",
        }))),
        "ping" => repondre(&id, Ok(json!({}))),
        "tools/list" => {
            let liste: Vec<Value> = outils().into_iter().map(|(n, d, s)| json!({"name": n, "description": d, "inputSchema": s})).collect();
            repondre(&id, Ok(json!({"tools": liste})));
        }
        "tools/call" => {
            let nom = p.get("name").and_then(Value::as_str).unwrap_or("");
            let arguments = p.get("arguments").and_then(Value::as_object).unwrap_or(&vide);
            // erreur rendue au client, le serveur continue
            let (contenu, est_erreur) = appeler(nom, arguments).unwrap_or_else(|e| (vec![json!({"type": "text", "text": e})], true));
            repondre(&id, Ok(json!({"content": contenu, "isError": est_erreur})));
        }
        _ => repondre(&id, Err(json!({"code": -32601, "message": format!("méthode inconnue : {methode}")}))),
    }
}

/// Point d'entrée : `bimedit --mcp`.
pub fn servir() {
    let entree = std::io::stdin();
    let mut brut = Vec::new();
    loop {
        brut.clear();
        match entree.lock().read_until(b'\n', &mut brut) {
            Ok(0) | Err(_) => break,
            Ok(_) => {}
        }
        let ligne = brut.trim_ascii();
        if ligne.is_empty() {
            continue;
        }
        match serde_json::from_slice::<Value>(ligne) {
            Ok(msg) => traiter(&msg),
            Err(_) => repondre(&Value::Null, Err(json!({"code": -32700, "message": "JSON invalide"}))),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn base64_standard() {
        assert_eq!(base64(b""), "");
        assert_eq!(base64(b"f"), "Zg==");
        assert_eq!(base64(b"fo"), "Zm8=");
        assert_eq!(base64(b"foo"), "Zm9v");
        assert_eq!(base64(b"foobar"), "Zm9vYmFy");
    }

    #[test]
    fn jeton_dimensions() {
        assert_eq!(dimensions("800x600"), Some(("800", "600")));
        assert_eq!(dimensions("x"), None);
        assert_eq!(dimensions("a.png"), None);
        assert_eq!(dimensions("C:/x/y.png"), None);
    }

    #[test]
    fn arguments_rendus_en_texte() {
        let a = json!({"n": 0.5, "s": "a", "i": 3}).as_object().cloned().unwrap();
        assert_eq!(arg(&a, "n").unwrap(), "0.5");
        assert_eq!(arg(&a, "s").unwrap(), "a");
        assert_eq!(arg(&a, "i").unwrap(), "3");
        assert!(arg(&a, "absent").is_err());
    }

    #[test]
    fn outils_avec_schema_et_noms_uniques() {
        let o = outils();
        assert_eq!(o.len(), 28);
        assert!(o.iter().all(|(_, _, s)| s["type"] == "object"));
        let noms: std::collections::HashSet<_> = o.iter().map(|(n,_,_)|n).collect();
        assert_eq!(noms.len(),o.len());
    }

    #[test]
    fn commandes_structurees_preservent_les_chemins_et_le_json() {
        let args=json!({"format":"pdf","chemin":"C:/Projet partagé/étage 1.pdf"}).as_object().cloned().unwrap();
        let ligne=ligne_complete("application_exporter",&args).unwrap().unwrap();
        let (_,json)=ligne.split_once(' ').unwrap();
        assert_eq!(serde_json::from_str::<Value>(json).unwrap()["chemin"],args["chemin"]);
        let args=json!({"transaction":{"action":"annoter","texte":"Ligne 1\nLigne 2"}}).as_object().cloned().unwrap();
        let ligne=ligne_complete("application_editer",&args).unwrap().unwrap();
        assert!(!ligne.contains('\n'));
        assert!(ligne_complete("application_editer",&Map::new()).is_err());
    }

    #[test]
    fn routage_collaboration_et_historique() {
        let a=json!({"action":"reserver","ids":["mur-1","mur-2"]}).as_object().cloned().unwrap();
        assert_eq!(ligne_complete("application_collaboration",&a).unwrap().unwrap(),r#"collaboration_reserver ["mur-1","mur-2"]"#);
        let a=json!({"action":"etat_complet"}).as_object().cloned().unwrap();
        assert_eq!(ligne_complete("application_collaboration",&a).unwrap().unwrap(),"collaboration_etat complet");
        let a=json!({"action":"annuler"}).as_object().cloned().unwrap();
        let ligne=ligne_complete("application_historique",&a).unwrap().unwrap();
        let (_,j)=ligne.split_once(' ').unwrap();
        assert_eq!(serde_json::from_str::<Value>(j).unwrap()["nombre"],1);
        assert!(envoyer(&Cible { pid: None, port: 9, jeton: None },"etat\nquitter",Duration::from_millis(1)).is_err());
    }

    #[test]
    fn routage_plans_tournes_et_plan_actif() {
        let ligne = |v: Value| ligne_complete("application_famille", v.as_object().unwrap()).unwrap().unwrap();
        let l = ligne(json!({"action":"plan_tourne","nom":"Incliné","hote":"Bas","appui":"Gauche","angle":30}));
        let (c, j) = l.split_once(' ').unwrap();
        assert_eq!((c, serde_json::from_str::<Value>(j).unwrap()), ("famille_plan_tourne", json!({"nom":"Incliné","hote":"Bas","appui":"Gauche","angle":30})));
        assert_eq!(ligne(json!({"action":"angle","plan":"Incliné","valeur":"Pente"})), r#"famille_angle {"plan":"Incliné","valeur":"Pente"}"#);
        assert_eq!(ligne(json!({"action":"plan_actif","choix":"plan Incliné"})), "famille_plan_actif plan Incliné");
        assert_eq!(ligne(json!({"action":"ecran","h":0,"v":250.5})), "famille_ecran 0 250.5");
    }
}
