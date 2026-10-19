//! Métadonnées des fichiers Revit (.rvt, .rfa, .rte, .rft).
//!
//! Un fichier Revit est un conteneur OLE (Compound File Binary). La géométrie,
//! dans `Partitions/*` et `Global/*`, est sérialisée dans un format propriétaire
//! non décodé ici. Sont lus les flux descriptifs :
//! - `BasicFileInfo` : version, build, partage de projet, chemins (UTF-16) ;
//! - `ProjectInformation` (ZIP, projets) ou `PartAtom` (familles) : XML Atom ;
//! - `TransmissionData` : références externes (XML UTF-16) ;
//! - `RevitPreview4.0` : vignette PNG.

use std::fs::File;
use std::io::{Read, Seek, SeekFrom};
use std::path::Path;

pub const CFB_MAGIC: [u8; 8] = [0xD0, 0xCF, 0x11, 0xE0, 0xA1, 0xB1, 0x1A, 0xE1];
const END_OF_CHAIN: u32 = 0xFFFF_FFFA;
const NO_STREAM: u32 = 0xFFFF_FFFF;

/// Vrai si les premiers octets sont ceux d'un conteneur OLE.
pub fn is_cfb(head: &[u8]) -> bool {
    head.starts_with(&CFB_MAGIC)
}

pub fn is_revit_path(path: &Path) -> bool {
    let ext = path.extension().map(|e| e.to_string_lossy().to_lowercase()).unwrap_or_default();
    matches!(ext.as_str(), "rvt" | "rfa" | "rte" | "rft")
}

// ------------------------------------------------------------------ CFB

struct DirEntry {
    name: String,
    kind: u8,
    left: u32,
    right: u32,
    child: u32,
    start: u32,
    size: u64,
}

pub struct Cfb {
    f: File,
    sector_size: usize,
    mini_size: usize,
    mini_cutoff: u64,
    n_sectors: u32,
    fat: Vec<u32>,
    minifat: Vec<u32>,
    dirs: Vec<DirEntry>,
    ministream: Vec<u8>,
}

fn u16_at(b: &[u8], o: usize) -> u16 {
    u16::from_le_bytes([b[o], b[o + 1]])
}
fn u32_at(b: &[u8], o: usize) -> u32 {
    u32::from_le_bytes(b[o..o + 4].try_into().unwrap())
}

impl Cfb {
    pub fn open(path: &Path) -> Result<Cfb, String> {
        let mut f = File::open(path).map_err(|e| format!("Lecture impossible : {e}"))?;
        let len = f.metadata().map_err(|e| e.to_string())?.len();
        let mut h = [0u8; 512];
        f.read_exact(&mut h).map_err(|_| "Fichier trop court pour un conteneur OLE".to_string())?;
        if !is_cfb(&h) {
            return Err("Pas un conteneur OLE : ce n'est pas un fichier Revit".into());
        }
        let shift = u16_at(&h, 0x1E);
        let mini_shift = u16_at(&h, 0x20);
        if !(9..=16).contains(&shift) || mini_shift > shift {
            return Err("En-tête OLE invalide".into());
        }
        let sector_size = 1usize << shift;
        let n_sectors = (len.saturating_sub(sector_size as u64) / sector_size as u64) as u32;
        let mut cfb = Cfb {
            f,
            sector_size,
            mini_size: 1 << mini_shift,
            mini_cutoff: u32_at(&h, 0x38) as u64,
            n_sectors,
            fat: Vec::new(),
            minifat: Vec::new(),
            dirs: Vec::new(),
            ministream: Vec::new(),
        };
        let major = u16_at(&h, 0x1A);
        let n_fat = u32_at(&h, 0x2C) as usize;
        let first_dir = u32_at(&h, 0x30);
        let first_minifat = u32_at(&h, 0x3C);
        let n_minifat = u32_at(&h, 0x40);
        let mut difat_next = u32_at(&h, 0x44);
        let n_difat = u32_at(&h, 0x48);

        let mut fat_sectors: Vec<u32> = (0..109).map(|i| u32_at(&h, 0x4C + 4 * i)).collect();
        for _ in 0..n_difat {
            if difat_next >= END_OF_CHAIN {
                break;
            }
            let s = cfb.sector(difat_next)?;
            let n = sector_size / 4;
            fat_sectors.extend((0..n - 1).map(|i| u32_at(&s, 4 * i)));
            difat_next = u32_at(&s, 4 * (n - 1));
        }
        fat_sectors.retain(|&s| s < END_OF_CHAIN);
        fat_sectors.truncate(n_fat);
        for s in fat_sectors {
            let b = cfb.sector(s)?;
            cfb.fat.extend(b.chunks_exact(4).map(|c| u32::from_le_bytes(c.try_into().unwrap())));
        }

        let dir_data = cfb.chain_bytes(first_dir)?;
        for e in dir_data.chunks_exact(128) {
            let nl = (u16_at(e, 64) as usize).min(64);
            let units: Vec<u16> = e[..nl.saturating_sub(2)].chunks_exact(2).map(|c| u16::from_le_bytes([c[0], c[1]])).collect();
            let mut size = u64::from_le_bytes(e[120..128].try_into().unwrap());
            if major == 3 {
                size &= 0xFFFF_FFFF;
            }
            cfb.dirs.push(DirEntry {
                name: String::from_utf16_lossy(&units),
                kind: e[66],
                left: u32_at(e, 68),
                right: u32_at(e, 72),
                child: u32_at(e, 76),
                start: u32_at(e, 116),
                size,
            });
        }
        if cfb.dirs.is_empty() {
            return Err("Répertoire OLE vide".into());
        }
        if n_minifat > 0 {
            let m = cfb.chain_bytes(first_minifat)?;
            cfb.minifat = m.chunks_exact(4).map(|c| u32::from_le_bytes(c.try_into().unwrap())).collect();
        }
        let (root_start, root_size) = (cfb.dirs[0].start, cfb.dirs[0].size);
        if root_size > 0 && root_start < END_OF_CHAIN {
            let mut ms = cfb.chain_bytes(root_start)?;
            ms.truncate(root_size as usize);
            cfb.ministream = ms;
        }
        Ok(cfb)
    }

    fn sector(&mut self, n: u32) -> Result<Vec<u8>, String> {
        if n >= self.n_sectors {
            return Err(format!("Secteur OLE {n} hors du fichier"));
        }
        let mut b = vec![0u8; self.sector_size];
        self.f
            .seek(SeekFrom::Start((n as u64 + 1) * self.sector_size as u64))
            .and_then(|_| self.f.read_exact(&mut b))
            .map_err(|e| format!("Lecture du secteur {n} : {e}"))?;
        Ok(b)
    }

    fn chain(table: &[u32], start: u32) -> Vec<u32> {
        let mut out = Vec::new();
        let mut s = start;
        while s < END_OF_CHAIN && (s as usize) < table.len() && out.len() <= table.len() {
            out.push(s);
            s = table[s as usize];
        }
        out
    }

    fn chain_bytes(&mut self, start: u32) -> Result<Vec<u8>, String> {
        let mut out = Vec::new();
        for s in Self::chain(&self.fat, start) {
            out.extend(self.sector(s)?);
        }
        Ok(out)
    }

    /// Flux et dossiers, chemins absolus (`/Global/Latest`), triés.
    pub fn entries(&self) -> Vec<(String, bool, u64)> {
        let mut out = Vec::new();
        let mut stack: Vec<(u32, String)> = vec![(self.dirs[0].child, String::new())];
        let mut seen = vec![false; self.dirs.len()];
        while let Some((i, prefix)) = stack.pop() {
            if i == NO_STREAM || i as usize >= self.dirs.len() || seen[i as usize] {
                continue;
            }
            seen[i as usize] = true;
            let d = &self.dirs[i as usize];
            stack.push((d.left, prefix.clone()));
            stack.push((d.right, prefix.clone()));
            let path = format!("{prefix}/{}", d.name);
            match d.kind {
                1 => {
                    stack.push((d.child, path.clone()));
                    out.push((path, true, 0));
                }
                2 => out.push((path, false, d.size)),
                _ => {}
            }
        }
        out.sort();
        out
    }

    fn find(&self, path: &str) -> Option<usize> {
        let mut cur = 0usize;
        for part in path.trim_matches('/').split('/') {
            let mut stack = vec![self.dirs[cur].child];
            let mut found = None;
            let mut guard = 0;
            while let Some(i) = stack.pop() {
                guard += 1;
                if i == NO_STREAM || i as usize >= self.dirs.len() || guard > self.dirs.len() {
                    continue;
                }
                let d = &self.dirs[i as usize];
                if d.name.eq_ignore_ascii_case(part) {
                    found = Some(i as usize);
                    break;
                }
                stack.push(d.left);
                stack.push(d.right);
            }
            cur = found?;
        }
        Some(cur)
    }

    /// Contenu d'un flux, ou None s'il n'existe pas.
    pub fn read(&mut self, path: &str) -> Result<Option<Vec<u8>>, String> {
        let Some(i) = self.find(path) else { return Ok(None) };
        let (start, size) = (self.dirs[i].start, self.dirs[i].size);
        if self.dirs[i].kind != 2 {
            return Ok(None);
        }
        let mut out = if size < self.mini_cutoff {
            let mut v = Vec::new();
            for s in Self::chain(&self.minifat, start) {
                let o = s as usize * self.mini_size;
                let Some(chunk) = self.ministream.get(o..o + self.mini_size) else {
                    return Err(format!("Mini-secteur {s} hors du mini-flux"));
                };
                v.extend_from_slice(chunk);
            }
            v
        } else {
            self.chain_bytes(start)?
        };
        if (out.len() as u64) < size {
            return Err(format!("Flux {path} tronqué"));
        }
        out.truncate(size as usize);
        Ok(Some(out))
    }
}

// ------------------------------------------------------------------ Données

#[derive(Debug, Clone, Default)]
pub struct Param {
    pub name: String,
    pub value: String,
    /// Type Revit du paramètre (Texte, Longueur, Oui/Non…).
    pub kind: String,
    /// system, shared, custom…
    pub origin: String,
    pub units: Option<String>,
}

#[derive(Debug, Clone, Default)]
pub struct ParamGroup {
    pub title: String,
    pub params: Vec<Param>,
}

#[derive(Debug, Clone, Default)]
pub struct ExternalRef {
    pub kind: String,
    pub path: String,
    pub absolute_path: String,
    pub load_state: String,
}

#[derive(Default)]
pub struct RvtInfo {
    pub file_name: String,
    pub file_size: u64,
    /// Projet, Famille, Gabarit de projet, Gabarit de famille.
    pub kind: &'static str,
    /// Lignes « Clé: valeur » de BasicFileInfo.
    pub basic: Vec<(String, String)>,
    pub title: Option<String>,
    pub updated: Option<String>,
    pub category: Option<String>,
    pub groups: Vec<ParamGroup>,
    /// Types de famille (PartAtom).
    pub family_types: Vec<ParamGroup>,
    pub external_refs: Vec<ExternalRef>,
    pub streams: Vec<(String, bool, u64)>,
    pub preview_png: Option<Vec<u8>>,
    /// Problèmes non bloquants rencontrés pendant la lecture.
    pub warnings: Vec<String>,
}

impl RvtInfo {
    pub fn basic_value(&self, key: &str) -> Option<&str> {
        self.basic.iter().find(|(k, _)| k.eq_ignore_ascii_case(key)).map(|(_, v)| v.as_str())
    }

    /// Version Revit du format (« 2027 »).
    pub fn format(&self) -> Option<&str> {
        self.basic_value("Format")
    }
}

/// Libellé français des clés connues de BasicFileInfo.
pub fn basic_label(key: &str) -> &str {
    match key {
        "Format" => "Version du format",
        "Build" => "Build Revit",
        "Worksharing" => "Partage de projet",
        "Username" => "Utilisateur",
        "Central Model Path" => "Modèle central",
        "Last Save Path" => "Dernier enregistrement",
        "Locale when saved" => "Langue à l'enregistrement",
        "Unique Document GUID" => "GUID du document",
        "Unique Document Increments" => "Incréments du document",
        "Model Identity" => "Identité du modèle",
        "Central Model Identity" => "Identité du modèle central",
        "Open Workset Default" => "Sous-projets ouverts par défaut",
        "IsSingleUserCloudModel" => "Modèle cloud mono-utilisateur",
        "Author" => "Auteur",
        "ClientAppName" => "Application",
        "All Local Changes Saved To Central" => "Modifications locales synchronisées",
        "Central model's version number corresponding to the last reload latest" => "Version du central au dernier rechargement",
        "Central model's episode GUID corresponding to the last reload latest" => "Épisode du central au dernier rechargement",
        "Project Spark File" => "Fichier Spark",
        _ => key,
    }
}

pub fn load(path: &Path) -> Result<RvtInfo, String> {
    let mut cfb = Cfb::open(path)?;
    let mut info = RvtInfo {
        file_name: path.file_name().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default(),
        file_size: std::fs::metadata(path).map(|m| m.len()).unwrap_or(0),
        streams: cfb.entries(),
        ..Default::default()
    };
    let ext = path.extension().map(|e| e.to_string_lossy().to_lowercase()).unwrap_or_default();
    info.kind = match ext.as_str() {
        "rfa" => "Famille",
        "rte" => "Gabarit de projet",
        "rft" => "Gabarit de famille",
        _ => "Projet",
    };

    match cfb.read("BasicFileInfo")? {
        Some(b) => info.basic = parse_basic_file_info(&b),
        None => return Err("Flux BasicFileInfo absent : ce conteneur OLE n'est pas un fichier Revit".into()),
    }
    if info.basic.is_empty() {
        info.warnings.push("BasicFileInfo illisible (version de Revit antérieure ?)".into());
    }

    let atom = if let Some(zip) = cfb.read("ProjectInformation")? {
        match zip_first_entry(&zip) {
            Ok(xml) => Some(decode_text(&xml)),
            Err(e) => {
                info.warnings.push(format!("ProjectInformation : {e}"));
                None
            }
        }
    } else {
        cfb.read("PartAtom")?.map(|b| decode_text(&b))
    };
    if let Some(xml) = atom {
        match xml::parse(&xml) {
            Ok(root) => apply_atom(&mut info, &root),
            Err(e) => info.warnings.push(format!("XML Atom : {e}")),
        }
    }

    if let Some(b) = cfb.read("TransmissionData")? {
        let text = utf16_after_len(&b);
        match xml::parse(&text) {
            Ok(root) => info.external_refs = parse_transmission(&root),
            Err(e) => info.warnings.push(format!("TransmissionData : {e}")),
        }
    }

    if let Some(b) = cfb.read("RevitPreview4.0")? {
        info.preview_png = extract_png(&b);
    }
    Ok(info)
}

// ------------------------------------------------------------------ Flux

/// Lignes « Clé: valeur » du bloc texte de BasicFileInfo (UTF-16LE).
pub fn parse_basic_file_info(b: &[u8]) -> Vec<(String, String)> {
    // Le bloc texte n'est pas aligné sur 2 octets d'un fichier à l'autre : essayer les deux parités.
    let mut best: Vec<(String, String)> = Vec::new();
    for parity in 0..2 {
        let units: Vec<u16> = b[parity..].chunks_exact(2).map(|c| u16::from_le_bytes([c[0], c[1]])).collect();
        for run in units.split(|&u| !(u >= 0x20 || u == 0x0A || u == 0x0D || u == 0x09) || (0xD800..0xE000).contains(&u)) {
            if run.len() < 8 {
                continue;
            }
            // Certaines fins de ligne sont écrites en octets (0D 0A) et se lisent comme U+0A0D.
            let s = String::from_utf16_lossy(run).replace('\u{0A0D}', "\n");
            let pairs: Vec<(String, String)> = s
                .lines()
                .filter_map(|l| {
                    let (k, v) = l.split_once(':')?;
                    // Le préfixe de longueur du bloc peut se lire comme un caractère parasite.
                    let k = k.trim_start_matches(|c: char| !c.is_ascii_alphanumeric()).trim();
                    // Une lettre de lecteur (« C:\… ») n'est pas une clé.
                    (k.len() > 1 && !k.is_empty()).then(|| (k.to_string(), v.trim().to_string()))
                })
                .collect();
            if pairs.len() > best.len() && pairs.iter().any(|(k, _)| k == "Format" || k == "Build" || k == "Worksharing") {
                best = pairs;
            }
        }
    }
    best
}

/// Texte UTF-16LE précédé de sa longueur en caractères (u32).
fn utf16_after_len(b: &[u8]) -> String {
    let body = if b.len() >= 4 && u32_at(b, 0) as usize * 2 <= b.len() - 4 { &b[4..4 + u32_at(b, 0) as usize * 2] } else { b };
    let units: Vec<u16> = body.chunks_exact(2).map(|c| u16::from_le_bytes([c[0], c[1]])).collect();
    String::from_utf16_lossy(&units).trim_end_matches('\0').to_string()
}

/// UTF-8 si valide, sinon Windows-1252 (Revit écrit parfois l'XML en ANSI malgré l'en-tête).
pub fn decode_text(b: &[u8]) -> String {
    let b = b.strip_prefix(&[0xEF, 0xBB, 0xBF]).unwrap_or(b);
    match std::str::from_utf8(b) {
        Ok(s) => s.to_string(),
        Err(_) => b.iter().map(|&c| cp1252(c)).collect(),
    }
}

fn cp1252(c: u8) -> char {
    const HIGH: [char; 32] = [
        '€', '\u{81}', '‚', 'ƒ', '„', '…', '†', '‡', 'ˆ', '‰', 'Š', '‹', 'Œ', '\u{8D}', 'Ž', '\u{8F}', '\u{90}', '‘', '’', '“', '”',
        '•', '–', '—', '˜', '™', 'š', '›', 'œ', '\u{9D}', 'ž', 'Ÿ',
    ];
    if (0x80..0xA0).contains(&c) { HIGH[(c - 0x80) as usize] } else { c as char }
}

/// Première entrée d'une archive ZIP (stockée ou deflate), via le répertoire central.
pub fn zip_first_entry(z: &[u8]) -> Result<Vec<u8>, String> {
    let eocd = (0..z.len().saturating_sub(21)).rev().find(|&i| z[i..].starts_with(b"PK\x05\x06")).ok_or("archive ZIP sans répertoire central")?;
    let cd = u32_at(z, eocd + 16) as usize;
    if !z.get(cd..).is_some_and(|s| s.starts_with(b"PK\x01\x02")) {
        return Err("répertoire central ZIP invalide".into());
    }
    let method = u16_at(z, cd + 10);
    let csize = u32_at(z, cd + 20) as usize;
    let usize_ = u32_at(z, cd + 24) as usize;
    let local = u32_at(z, cd + 42) as usize;
    if !z.get(local..).is_some_and(|s| s.len() >= 30 && s.starts_with(b"PK\x03\x04")) {
        return Err("en-tête local ZIP invalide".into());
    }
    let data = local + 30 + u16_at(z, local + 26) as usize + u16_at(z, local + 28) as usize;
    let raw = z.get(data..data + csize).ok_or("entrée ZIP tronquée")?;
    match method {
        0 => Ok(raw.to_vec()),
        8 => miniz_oxide::inflate::decompress_to_vec_with_limit(raw, usize_.max(1) * 2 + 1024)
            .map_err(|e| format!("décompression ZIP : {e:?}")),
        m => Err(format!("méthode de compression ZIP {m} non prise en charge")),
    }
}

/// Image PNG complète (signature → IEND) incluse dans le flux de vignette.
pub fn extract_png(b: &[u8]) -> Option<Vec<u8>> {
    let start = b.windows(8).position(|w| w == [0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A])?;
    let mut o = start + 8;
    while o + 12 <= b.len() {
        let len = u32::from_be_bytes(b[o..o + 4].try_into().unwrap()) as usize;
        let end = o + 12 + len;
        if end > b.len() {
            return None;
        }
        if &b[o + 4..o + 8] == b"IEND" {
            return Some(b[start..end].to_vec());
        }
        o = end;
    }
    None
}

/// Décode une vignette PNG en RGBA 8 bits : (largeur, hauteur, pixels).
pub fn decode_png_rgba(data: &[u8]) -> Option<(usize, usize, Vec<u8>)> {
    let mut dec = png::Decoder::new(std::io::Cursor::new(data));
    dec.set_transformations(png::Transformations::EXPAND | png::Transformations::STRIP_16 | png::Transformations::ALPHA);
    let mut reader = dec.read_info().ok()?;
    let mut buf = vec![0; reader.output_buffer_size()?];
    let out = reader.next_frame(&mut buf).ok()?;
    let (w, h) = (out.width as usize, out.height as usize);
    let rgba = match out.color_type {
        png::ColorType::Rgba => buf.get(..w * h * 4)?.to_vec(),
        png::ColorType::GrayscaleAlpha => buf.get(..w * h * 2)?.chunks_exact(2).flat_map(|p| [p[0], p[0], p[0], p[1]]).collect(),
        _ => return None,
    };
    Some((w, h, rgba))
}

fn param_of(n: &xml::Node) -> Param {
    Param {
        name: n.attr("displayName").unwrap_or(&n.name).replace('_', " ").to_string(),
        value: n.text.trim().to_string(),
        kind: n.attr("typeOfParameter").unwrap_or("").to_string(),
        origin: n.attr("type").unwrap_or("").to_string(),
        units: n.attr("units").map(str::to_string),
    }
}

fn apply_atom(info: &mut RvtInfo, root: &xml::Node) {
    info.title = root.child("title").map(|n| n.text.trim().to_string()).filter(|s| !s.is_empty());
    info.updated = root.child("updated").map(|n| n.text.trim().to_string());
    info.category = root.child("category").and_then(|c| c.child("term")).map(|n| n.text.trim().to_string());
    if let Some(features) = root.child("A:features") {
        for feat in features.children_named("A:feature") {
            let ft = feat.child("A:title").map(|n| n.text.trim()).unwrap_or("");
            for g in feat.children_named("A:group") {
                let gt = g.child("A:title").map(|n| n.text.trim()).unwrap_or("");
                let params: Vec<Param> = g.children.iter().filter(|c| c.name != "A:title").map(param_of).collect();
                if !params.is_empty() {
                    info.groups.push(ParamGroup { title: format!("{ft} — {gt}"), params });
                }
            }
        }
    }
    if let Some(fam) = root.child("A:family") {
        for part in fam.children_named("A:part") {
            let title = part.child("title").map(|n| n.text.trim().to_string()).unwrap_or_default();
            let params = part.children.iter().filter(|c| c.name != "title").map(param_of).collect();
            info.family_types.push(ParamGroup { title, params });
        }
    }
}

fn parse_transmission(root: &xml::Node) -> Vec<ExternalRef> {
    let text = |n: &xml::Node, k: &str| n.child(k).map(|c| c.text.trim().to_string()).unwrap_or_default();
    root.children_named("ExternalFileReference")
        .map(|r| ExternalRef {
            kind: text(r, "ExternalFileReferenceType"),
            path: text(r, "LastSavedPath"),
            absolute_path: text(r, "LastSavedAbsolutePath"),
            load_state: text(r, "LastSavedLoadState"),
        })
        .collect()
}

// ------------------------------------------------------------------ XML

/// Analyseur XML minimal : éléments, attributs, texte. Suffisant pour les flux Revit.
pub mod xml {
    #[derive(Debug, Default)]
    pub struct Node {
        pub name: String,
        pub attrs: Vec<(String, String)>,
        pub text: String,
        pub children: Vec<Node>,
    }

    impl Node {
        pub fn attr(&self, k: &str) -> Option<&str> {
            self.attrs.iter().find(|(a, _)| a == k).map(|(_, v)| v.as_str())
        }
        pub fn child(&self, name: &str) -> Option<&Node> {
            self.children.iter().find(|c| c.name == name)
        }
        pub fn children_named<'a>(&'a self, name: &'a str) -> impl Iterator<Item = &'a Node> + 'a {
            self.children.iter().filter(move |c| c.name == name)
        }
    }

    pub fn unescape(s: &str) -> String {
        if !s.contains('&') {
            return s.to_string();
        }
        let mut out = String::with_capacity(s.len());
        let mut rest = s;
        while let Some(i) = rest.find('&') {
            out.push_str(&rest[..i]);
            rest = &rest[i..];
            let Some(j) = rest.find(';').filter(|&j| j <= 10) else {
                out.push('&');
                rest = &rest[1..];
                continue;
            };
            let ent = &rest[1..j];
            let c = match ent {
                "amp" => Some('&'),
                "lt" => Some('<'),
                "gt" => Some('>'),
                "quot" => Some('"'),
                "apos" => Some('\''),
                _ if ent.starts_with("#x") => u32::from_str_radix(&ent[2..], 16).ok().and_then(char::from_u32),
                _ if ent.starts_with('#') => ent[1..].parse().ok().and_then(char::from_u32),
                _ => None,
            };
            match c {
                Some(c) => {
                    out.push(c);
                    rest = &rest[j + 1..];
                }
                None => {
                    out.push('&');
                    rest = &rest[1..];
                }
            }
        }
        out.push_str(rest);
        out
    }

    /// Élément racine du document.
    pub fn parse(src: &str) -> Result<Node, String> {
        let mut stack: Vec<Node> = vec![Node::default()];
        let mut rest = src;
        while !rest.is_empty() {
            let Some(lt) = rest.find('<') else {
                stack.last_mut().unwrap().text.push_str(&unescape(rest));
                break;
            };
            if lt > 0 {
                stack.last_mut().unwrap().text.push_str(&unescape(&rest[..lt]));
            }
            rest = &rest[lt..];
            if let Some(r) = rest.strip_prefix("<!--") {
                rest = r.find("-->").map(|i| &r[i + 3..]).ok_or("commentaire non fermé")?;
            } else if let Some(r) = rest.strip_prefix("<![CDATA[") {
                let i = r.find("]]>").ok_or("CDATA non fermé")?;
                stack.last_mut().unwrap().text.push_str(&r[..i]);
                rest = &r[i + 3..];
            } else if rest.starts_with("<?") || rest.starts_with("<!") {
                rest = rest.find('>').map(|i| &rest[i + 1..]).ok_or("déclaration non fermée")?;
            } else if let Some(r) = rest.strip_prefix("</") {
                let gt = r.find('>').ok_or("balise fermante non terminée")?;
                let name = r[..gt].trim();
                let node = stack.pop().filter(|_| !stack.is_empty()).ok_or("balise fermante orpheline")?;
                if node.name != name {
                    return Err(format!("balise </{name}> au lieu de </{}>", node.name));
                }
                stack.last_mut().unwrap().children.push(node);
                rest = &r[gt + 1..];
            } else {
                let gt = tag_end(rest).ok_or("balise ouvrante non terminée")?;
                let inner = &rest[1..gt];
                let (inner, self_closing) = match inner.strip_suffix('/') {
                    Some(s) => (s, true),
                    None => (inner, false),
                };
                let node = open_tag(inner)?;
                if self_closing {
                    stack.last_mut().unwrap().children.push(node);
                } else {
                    stack.push(node);
                }
                rest = &rest[gt + 1..];
            }
        }
        if stack.len() != 1 {
            return Err(format!("élément <{}> non fermé", stack.last().unwrap().name));
        }
        stack.pop().unwrap().children.into_iter().next().ok_or_else(|| "document XML vide".to_string())
    }

    /// Position du `>` qui termine la balise, hors valeurs d'attribut.
    fn tag_end(s: &str) -> Option<usize> {
        let mut quote = None;
        for (i, c) in s.char_indices() {
            match (quote, c) {
                (None, '"' | '\'') => quote = Some(c),
                (Some(q), c) if c == q => quote = None,
                (None, '>') => return Some(i),
                _ => {}
            }
        }
        None
    }

    fn open_tag(inner: &str) -> Result<Node, String> {
        let inner = inner.trim();
        let name_end = inner.find(|c: char| c.is_whitespace()).unwrap_or(inner.len());
        let mut node = Node { name: inner[..name_end].to_string(), ..Default::default() };
        if node.name.is_empty() {
            return Err("balise sans nom".into());
        }
        let mut rest = inner[name_end..].trim_start();
        while !rest.is_empty() {
            let eq = rest.find('=').ok_or_else(|| format!("attribut sans valeur dans <{}>", node.name))?;
            let key = rest[..eq].trim().to_string();
            let after = rest[eq + 1..].trim_start();
            let q = after.chars().next().filter(|c| *c == '"' || *c == '\'').ok_or("valeur d'attribut sans guillemets")?;
            let close = after[1..].find(q).ok_or("valeur d'attribut non fermée")?;
            node.attrs.push((key, unescape(&after[1..1 + close])));
            rest = after[close + 2..].trim_start();
        }
        Ok(node)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn utf16(s: &str) -> Vec<u8> {
        s.encode_utf16().flat_map(|u| u.to_le_bytes()).collect()
    }

    #[test]
    fn basic_file_info_odd_alignment() {
        let mut b = vec![0x0e, 0, 0, 0, 1];
        b.extend(utf16("C:\\x.rvt"));
        b.extend([0, 0, 0]);
        b.extend(utf16("RevitApplication"));
        b.extend([0x0D, 0x0A]);
        b.extend(utf16("Worksharing: Not enabled\r\nFormat: 2027\r\nBuild: 20260716_1515(x64)\r\nLast Save Path: C:\\a\\b.rvt\r\n"));
        let kv = parse_basic_file_info(&b);
        assert_eq!(kv[0], ("Worksharing".into(), "Not enabled".into()));
        assert!(kv.contains(&("Format".into(), "2027".into())));
        assert!(kv.contains(&("Last Save Path".into(), "C:\\a\\b.rvt".into())));
    }

    #[test]
    fn xml_atom() {
        let src = r#"<?xml version="1.0"?><entry xmlns:A="u"><title>Essai &amp; co</title><A:features><A:feature><A:title>Project Information</A:title>
<A:group><A:title>Autre</A:title><Nom_du_projet displayName="Nom du projet" type="system" typeOfParameter="Texte">Nom</Nom_du_projet><Vide/></A:group></A:feature></A:features>
<A:family type="user"><A:part type="user"><title>A4</title><Largeur type="custom" typeOfParameter="Longueur" units="mm">420.000</Largeur></A:part></A:family></entry>"#;
        let mut info = RvtInfo::default();
        apply_atom(&mut info, &xml::parse(src).unwrap());
        assert_eq!(info.title.as_deref(), Some("Essai & co"));
        assert_eq!(info.groups[0].title, "Project Information — Autre");
        assert_eq!(info.groups[0].params[0].name, "Nom du projet");
        assert_eq!(info.groups[0].params[0].value, "Nom");
        assert_eq!(info.family_types[0].params[0].units.as_deref(), Some("mm"));
    }

    #[test]
    fn xml_errors() {
        assert!(xml::parse("<a><b></a>").is_err());
        assert!(xml::parse("<a>").is_err());
        assert_eq!(xml::parse("<a x='1>2'/>").unwrap().attr("x"), Some("1>2"));
    }

    #[test]
    fn cp1252_fallback() {
        assert_eq!(decode_text(b"\xc9tude \x9c"), "Étude œ");
        assert_eq!(decode_text("Étude".as_bytes()), "Étude");
    }

    #[test]
    fn png_bounds() {
        let mut b = b"xx\x89PNG\r\n\x1a\n".to_vec();
        b.extend([0, 0, 0, 0]);
        b.extend(b"IEND");
        b.extend([1, 2, 3, 4, 9, 9]);
        assert_eq!(extract_png(&b).unwrap().len(), 8 + 12);
        assert!(extract_png(&b[..15]).is_none());
    }
}
