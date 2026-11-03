//! Serveur MCP du viewer IFC/Revit (stdio, JSON-RPC 2.0).
//!
//! Il pilote le viewer par son canal local (127.0.0.1:47811, voir `src/pilote.rs`) : une
//! ligne « commande argument » envoyée, une ligne JSON reçue. Si aucun viewer n'écoute, il
//! en lance un (`BIMEDIT_EXE`, sinon l'exécutable voisin, sinon `target/release`).
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

const RACINE: &str = env!("CARGO_MANIFEST_DIR");
const NOM_EXE: &str = if cfg!(windows) { "bimedit.exe" } else { "bimedit" };

const COMMANDES: &str = "Commandes du viewer (outil viewer_commande) :
projet_natif (JSON du document et identifiants natifs) ; editer <JSON> (transaction, voir docs/EDITION.md) ; annuler ; retablir ;
etat ; ouvrir <chemin> ; capture vue|fenetre [LxH] <chemin.png> ; taille <largeur> <hauteur> ;
outil mur|poteau|poutre|terrain|aucun ; modifier_trace <id natif> (contour, polyligne, terrain) ; mur <x0 mm> <y0 mm> <x1 mm> <y1 mm> [hauteur mm epaisseur mm] ;
enregistrer [chemin.modele.json] (RVT : conversion en arrière-plan ; etat.enregistrement indique si elle est en cours) ;
reduire ; restaurer ; quitter ;
vues ; vue <indice|nom> ; camera dessus|face|iso|cadrer|ortho|perspective ;
feuilles ; selection_feuille <JSON> ; proprietes_feuille (sélection et propriétés du jeu, de la feuille ou de la vue placée) ;
camera <lacet°> <tangage°> [distance] (vue 3D seulement) ; zoom <facteur> ;
zone on|off|ajuster ou zone <xmin> <ymin> <zmin> <xmax> <ymax> <zmax> (zone de coupe 3D, m) ;
glisser <dx> <dy> [gauche|droit|milieu] [maj] (comme à la souris : orbite au bouton
gauche et regard sur place au bouton droit en vue 3D, panoramique sinon) ; afficher <CLASSES> ;
masquer <CLASSES> ; tout_afficher ; classes ; categories (catégories de modèle et
d'annotation) ; visibilite oui|non (boîte de visibilité) ; visibilite <catégorie> oui|non ;
panneau proprietes|arborescence gauche|droite|flottant ; elements [CLASSE|*] [texte] ;
selectionner <id> ; isoler <id> ; cadrer <id> ; proprietes [id] ; objet <x> <y>
(0..1 depuis le coin haut gauche de la vue) ; fiche oui|non (fiche Revit) ;
onglet tout|vues|feuilles|nomenclatures|familles|materiaux|groupes|liens|modele|classes ;
style filaire|lignes_cachees|ombre|couleurs|realiste|rendu (style visuel) ; dessin oui|non (lignes cachées ou réaliste) ; coupe non | coupe <hauteur m> [niveau]
(coupe horizontale au-dessus du niveau, mode dessin) ; niveaux ; poche hachure|plein|vide ;
echelle <dénominateur> (épaisseur des contours coupés, pas des hachures) ;
exporter <chemin.svg> (dessin vectoriel de la vue orthographique : coupe, arêtes vues
sans parties cachées, éléments 2D de la vue Revit ouverte).";

/// Résultat d'un outil : contenu MCP et indicateur d'erreur.
type Sortie = (Vec<Value>, bool);

fn port() -> u16 {
    std::env::var("BIMEDIT_PORT").ok().and_then(|p| p.parse().ok()).unwrap_or(47811)
}

fn dossier_captures() -> PathBuf {
    std::env::temp_dir().join("bimedit-mcp")
}

fn executable() -> Result<PathBuf, String> {
    // variable explicite d'abord, puis l'exécutable voisin, puis la compilation du projet
    let mut candidats = Vec::new();
    if let Some(e) = std::env::var_os("BIMEDIT_EXE").filter(|e| !e.is_empty()) {
        candidats.push(PathBuf::from(e));
    }
    if let Some(dossier) = std::env::current_exe().ok().and_then(|e| e.parent().map(Path::to_path_buf)) {
        candidats.push(dossier.join(NOM_EXE));
    }
    candidats.push(Path::new(RACINE).join("target").join("release").join(NOM_EXE));
    candidats
        .into_iter()
        .find(|c| c.is_file())
        .ok_or_else(|| "viewer introuvable : construire avec cargo build --release ou fixer BIMEDIT_EXE".to_string())
}

/// Envoie une ligne au viewer et lit la ligne JSON de réponse.
fn envoyer(ligne: &str, delai: Duration) -> std::io::Result<Value> {
    let adresse = SocketAddr::from((Ipv4Addr::LOCALHOST, port()));
    let mut flux = TcpStream::connect_timeout(&adresse, delai)?;
    flux.set_read_timeout(Some(delai))?;
    flux.set_write_timeout(Some(delai))?;
    flux.write_all(format!("{ligne}\n").as_bytes())?;
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

fn lancer() -> Result<(), String> {
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
    cmd.spawn().map_err(|e| format!("lancement de {} impossible : {e}", exe.display()))?;
    let fin = Instant::now() + Duration::from_secs(30);
    while Instant::now() < fin {
        if envoyer("etat", Duration::from_secs(5)).is_ok() {
            return Ok(());
        }
        std::thread::sleep(Duration::from_millis(300));
    }
    Err(format!("le viewer lancé ({}) ne répond pas sur le port {}", exe.display(), port()))
}

fn commande_delai(ligne: &str, delai: Duration) -> Result<Value, String> {
    match envoyer(ligne, delai) {
        Err(e) if e.kind() == std::io::ErrorKind::ConnectionRefused => {
            lancer()?;
            envoyer(ligne, delai).map_err(|e| e.to_string())
        }
        autre => autre.map_err(|e| e.to_string()),
    }
}

fn commande(ligne: &str) -> Result<Value, String> {
    commande_delai(ligne, Duration::from_secs(600))
}

fn est_ok(r: &Value) -> bool {
    r.get("ok").and_then(Value::as_bool).unwrap_or(false)
}

fn texte(r: &Value) -> Value {
    json!({"type": "text", "text": serde_json::to_string_pretty(r).unwrap_or_default()})
}

/// Texte de la réponse, en erreur si le viewer répond `ok: false`.
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
    vec![
        ("viewer_etat", "État du viewer : fichier ouvert, vue active, caméra, classes masquées, sélection. Lance le viewer s'il ne tourne pas.".into(), obj(json!({}), &[])),
        ("viewer_ouvrir", "Ouvre un fichier IFC ou Revit (.rvt, .rfa, .rte) et attend la fin du chargement.".into(),
         obj(json!({"chemin": {"type": "string", "description": "Chemin absolu du fichier"}}), &["chemin"])),
        ("viewer_capture", "Capture du viewer, rendue en image PNG. zone « vue » (défaut) : vue 3D rendue hors écran, à la taille de la vue ou à largeur × hauteur, même fenêtre réduite. zone « fenetre » : interface entière ; largeur et hauteur redimensionnent alors la fenêtre.".into(),
         obj(json!({"zone": {"type": "string", "enum": ["vue", "fenetre"]}, "largeur": {"type": "integer"}, "hauteur": {"type": "integer"}}), &[])),
        ("viewer_vues", "Liste des vues, feuilles et nomenclatures du fichier Revit ouvert (indice, genre, type, nom, échelle).".into(), obj(json!({}), &[])),
        ("viewer_feuilles", "Liste les jeux natifs, leurs feuilles et leurs vues et nomenclatures placées.".into(), obj(json!({}), &[])),
        ("viewer_selection_feuille", "Ouvre un jeu et sélectionne éventuellement une feuille puis une vue ou une nomenclature placée ; sans feuille, sélectionne le jeu.".into(),
         obj(json!({"jeu": {"type": "string"}, "feuille": {"type": "string", "description": "Identifiant natif de la feuille"}, "vue": {"type": "integer", "minimum": 0, "description": "Indice de la vue placée, puis des nomenclatures"}}), &["jeu"])),
        ("viewer_proprietes_feuille", "Lit les propriétés affichées de la sélection dans le jeu ouvert.".into(), obj(json!({}), &[])),
        ("viewer_modifier_feuille", "Modifie une feuille native avec son objet complet obtenu par viewer_feuilles ou viewer_proprietes_feuille. La modification est enregistrée dans l'historique et nécessite ensuite un enregistrement du projet.".into(),
         obj(json!({"feuille": {"type": "object", "description": "Objet feuille complet avec son identifiant existant"}}), &["feuille"])),
        ("viewer_vue", "Ouvre une vue Revit comme un double-clic dans l'arborescence : par indice (voir viewer_vues) ou par nom (« Plan d'étage : Niveau 0 », ou une partie du nom).".into(),
         obj(json!({"vue": {"type": "string"}}), &["vue"])),
        ("viewer_camera", "Règle la caméra : mode dessus|face|iso|cadrer|ortho|perspective, ou lacet et tangage en degrés (et distance en mètres).".into(),
         obj(json!({"mode": {"type": "string"}, "lacet": {"type": "number"}, "tangage": {"type": "number"}, "distance": {"type": "number"}}), &[])),
        ("viewer_proprietes", "Attributs et jeux de propriétés d'une entité (id STEP, ou la sélection si absent).".into(),
         obj(json!({"id": {"type": "integer"}}), &[])),
        ("viewer_objet", "Objet visible sous un point de la vue (x, y de 0 à 1 depuis le coin haut gauche) et point 3D touché.".into(),
         obj(json!({"x": {"type": "number"}, "y": {"type": "number"}}), &["x", "y"])),
        ("viewer_commande", format!("Commande brute du viewer.\n{COMMANDES}"),
         obj(json!({"commande": {"type": "string", "description": "Ligne « commande argument »"}}), &["commande"])),
    ]
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
    match nom {
        "viewer_etat" => Ok((vec![texte(&commande("etat")?)], false)),
        "viewer_ouvrir" => Ok(reponse(commande(&format!("ouvrir {}", arg(a, "chemin")?))?)),
        "viewer_capture" => {
            let zone = a.get("zone").and_then(Value::as_str).unwrap_or("vue");
            capture(zone, arg_opt(a, "largeur"), arg_opt(a, "hauteur"))
        }
        "viewer_vues" => Ok((vec![texte(&commande("vues")?)], false)),
        "viewer_feuilles" => Ok(reponse(commande("feuilles")?)),
        "viewer_selection_feuille" => {
            let corps = serde_json::to_string(a).map_err(|e| e.to_string())?;
            Ok(reponse(commande(&format!("selection_feuille {corps}"))?))
        }
        "viewer_proprietes_feuille" => Ok(reponse(commande("proprietes_feuille")?)),
        "viewer_modifier_feuille" => {
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
        "viewer_vue" => Ok(reponse(commande(&format!("vue {}", arg(a, "vue")?))?)),
        "viewer_camera" => {
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
        "viewer_proprietes" => {
            let ligne = format!("proprietes {}", arg_opt(a, "id").unwrap_or_default());
            Ok(reponse(commande(ligne.trim())?))
        }
        "viewer_objet" => Ok(reponse(commande(&format!("objet {} {}", arg(a, "x")?, arg(a, "y")?))?)),
        "viewer_commande" => {
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
            "instructions": "Pilote le viewer IFC/Revit du projet : viewer_ouvrir puis viewer_capture pour voir le rendu ; viewer_vue pour ouvrir une vue Revit.",
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

fn main() {
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
    fn treize_outils_avec_schema() {
        let o = outils();
        assert_eq!(o.len(), 13);
        assert!(o.iter().all(|(_, _, s)| s["type"] == "object"));
    }
}
