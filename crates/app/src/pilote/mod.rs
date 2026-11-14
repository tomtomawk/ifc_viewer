//! Pilotage du viewer par un autre programme (serveur MCP `bimedit --mcp`, voir `mcp.rs`).
//!
//! Chaque viewer écoute sur 127.0.0.1 : le port 47811 s'il est libre, sinon un port choisi
//! par le système, et s'inscrit avec un jeton aléatoire dans le registre des instances
//! (`instances.rs`). Une connexion envoie une ligne « @jeton commande argument » en UTF-8
//! et reçoit une ligne JSON. Mode isolé (`BIMEDIT_PORT`) : port fixe, sans inscription,
//! jeton seulement si `BIMEDIT_JETON` est défini. Les commandes sont traitées par
//! l'application entre deux images (`ViewerApp::piloter`).

pub mod cli;
pub mod instances;
pub mod mcp;
pub mod edition;
pub mod saisie;
pub const AIDE: &str = include_str!("../../../../docs/PILOTE.md");
pub fn documentation(sujet: &str) -> Result<&'static str, String> {
    match sujet {
        "" | "pilote" => Ok(AIDE),
        "edition" => Ok(include_str!("../../../../docs/EDITION.md")),
        "modele" => Ok(include_str!("../../../../docs/MODELE.md")),
        "commandes" => Ok(include_str!("../modele/edition/commandes/mod.rs")),
        _ => Err("Sujet : pilote|edition|modele|commandes".into()),
    }
}

use eframe::egui;
use std::io::{BufRead, BufReader, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::mpsc::{Receiver, Sender, channel};
use std::time::Duration;

pub const PORT: u16 = 47811;

/// Commande reçue, avec le canal de sa réponse (une ligne JSON).
pub struct Requete {
    pub commande: String,
    pub argument: String,
    pub reponse: Sender<String>,
}

/// Démarre l'écoute. Mode isolé : None si le port fixe est déjà pris (nouvel essai plus
/// tard). Mode normal : 47811, sinon un port libre, puis inscription au registre.
pub fn demarrer(ctx: egui::Context) -> Option<Receiver<Requete>> {
    let (ecoute, jeton) = match instances::port_isole() {
        Some(port) => (TcpListener::bind(("127.0.0.1", port)).ok()?, instances::jeton_impose()),
        None => {
            let ecoute = TcpListener::bind(("127.0.0.1", PORT)).or_else(|_| TcpListener::bind(("127.0.0.1", 0))).ok()?;
            let jeton = instances::nouveau_jeton();
            instances::inscrire(ecoute.local_addr().ok()?.port(), &jeton);
            (ecoute, Some(jeton))
        }
    };
    let jeton = std::sync::Arc::new(jeton);
    let (tx, rx) = channel();
    std::thread::Builder::new()
        .name("pilote".into())
        .spawn(move || {
            for flux in ecoute.incoming().flatten() {
                let (tx, ctx, jeton) = (tx.clone(), ctx.clone(), jeton.clone());
                std::thread::spawn(move || servir(flux, tx, ctx, jeton.as_deref()));
            }
        })
        .ok()?;
    Some(rx)
}

fn servir(flux: TcpStream, tx: Sender<Requete>, ctx: egui::Context, jeton: Option<&str>) {
    let _ = flux.set_read_timeout(Some(Duration::from_secs(30)));
    let mut ligne = String::new();
    let Ok(copie) = flux.try_clone() else { return };
    if BufReader::new(copie).read_line(&mut ligne).is_err() {
        return;
    }
    let ligne = ligne.trim_end_matches(['\r', '\n']);
    let ligne = match instances::authentifier(ligne, jeton) {
        Ok(l) => l,
        Err(e) => {
            let mut flux = flux;
            let _ = writeln!(flux, "{}", erreur(e));
            return;
        }
    };
    let (commande, argument) = ligne.split_once(' ').unwrap_or((ligne, ""));
    let (rtx, rrx) = channel();
    if tx.send(Requete { commande: commande.to_string(), argument: argument.trim().to_string(), reponse: rtx }).is_err() {
        return;
    }
    ctx.request_repaint();
    // ouverture d'un gros fichier : jusqu'à 10 minutes
    let reponse = rrx.recv_timeout(Duration::from_secs(600)).unwrap_or_else(|_| erreur("délai dépassé"));
    let mut flux = flux;
    let _ = writeln!(flux, "{reponse}");
}

/// Chaîne JSON échappée.
pub fn texte(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 2);
    out.push('"');
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if (c as u32) < 0x20 => out.push_str(&format!("\\u{:04x}", c as u32)),
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

/// Objet JSON à partir de paires (clé, valeur JSON déjà écrite).
pub fn objet(paires: &[(&str, String)]) -> String {
    let corps: Vec<String> = paires.iter().map(|(k, v)| format!("{}:{v}", texte(k))).collect();
    format!("{{{}}}", corps.join(","))
}

/// Liste JSON de valeurs déjà écrites.
pub fn liste(valeurs: impl IntoIterator<Item = String>) -> String {
    format!("[{}]", valeurs.into_iter().collect::<Vec<_>>().join(","))
}

pub fn erreur(message: &str) -> String {
    objet(&[("ok", "false".into()), ("erreur", texte(message))])
}

pub fn succes(paires: &[(&str, String)]) -> String {
    let mut p = vec![("ok", "true".to_string())];
    p.extend(paires.iter().cloned());
    objet(&p)
}

/// Encode une image RGBA en PNG.
pub fn png(largeur: usize, hauteur: usize, rgba: &[u8]) -> Result<Vec<u8>, String> {
    let mut out = Vec::new();
    {
        let mut enc = png::Encoder::new(&mut out, largeur as u32, hauteur as u32);
        enc.set_color(png::ColorType::Rgba);
        enc.set_depth(png::BitDepth::Eight);
        let mut w = enc.write_header().map_err(|e| e.to_string())?;
        w.write_image_data(rgba).map_err(|e| e.to_string())?;
    }
    Ok(out)
}
