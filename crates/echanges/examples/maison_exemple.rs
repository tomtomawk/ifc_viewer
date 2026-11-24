//! Génère `samples/maison_exemple.bem` : maison R+1 amorcée par le gabarit par défaut
//! (types, matériaux, motifs, menuiseries), construite par l'API de création et les
//! commandes, comme dans l'interface.
//!
//! Volume principal maçonné (béton + doublage, enduit) sous toiture tuiles à deux pans ;
//! aile basse à ossature bois bardée, toiture végétalisée ; entrée en retrait sous auvent
//! sur poteau ; terrasse bois au sud fermée par un muret. Cloisons, pièces, escalier,
//! garde-corps, menuiseries de plusieurs types, cotes, textes, coupes, élévations,
//! nomenclatures et trois feuilles A2.
//!
//! `cargo run --release -p echanges --example maison_exemple [sortie.bem]`
use commandes::{Commande, Execution};
use modele::creation::{Implantation, ParametresDalle, ParametresMur};
use modele::famille::{FamilleFenetre, Vantail};
use modele::*;
use serde_json::{json, Value};

type R<T> = Result<T, String>;

fn cmd(p: &mut Projet, c: Value) -> R<Vec<String>> {
    let c: Commande = serde_json::from_value(c.clone()).map_err(|e| format!("{e} : {c}"))?;
    let (q, ids) = p.executer(c)?;
    *p = q;
    Ok(ids)
}

fn type_mur(p: &Projet, nom: &str) -> R<String> {
    p.types_murs.iter().find(|t| t.nom == nom).map(|t| t.id.clone()).ok_or(format!("type de mur absent : {nom}"))
}

fn type_dalle(p: &Projet, nom: &str) -> R<TypeCompose> {
    p.types_dalles.iter().find(|t| t.nom == nom).cloned().ok_or(format!("type de dalle absent : {nom}"))
}

fn type_ouv(p: &Projet, nom: &str) -> R<String> {
    p.types_ouvertures.iter().find(|t| t.nom == nom).map(|t| t.id.clone()).ok_or(format!("type d'ouverture absent : {nom}"))
}

/// Rectangle [x0, y0] – [x1, y1], sens trigonométrique.
fn rect(x0: f64, y0: f64, x1: f64, y1: f64) -> Vec<[f64; 2]> {
    vec![[x0, y0], [x1, y0], [x1, y1], [x0, y1]]
}

struct Maison {
    p: Projet,
    rdc: String,
    r1: String,
}

impl Maison {
    fn mur(&mut self, a: [f64; 2], b: [f64; 2], type_mur: &str, implantation: Implantation, niveau: &str, haut: Option<(&str, f64)>, hauteur: f64) -> R<String> {
        let t = type_mur.to_string();
        let prm = ParametresMur {
            hauteur: if hauteur > 0.0 { hauteur } else { 2800.0 },
            implantation,
            niveau: Some(niveau.into()),
            niveau_haut: haut.map(|h| h.0.to_string()),
            decalage_haut: haut.map_or(0.0, |h| h.1),
            type_mur: Some(t),
            ..Default::default()
        };
        self.p.ajouter_mur(a, b, &prm)
    }

    fn dalle(&mut self, contour: &[[f64; 2]], sorte: SorteDalle, t: &str, niveau: &str, decalage: f64, pente: Option<f64>) -> R<String> {
        let prm = ParametresDalle { sorte, type_dalle: type_dalle(&self.p, t)?, pente, niveau: Some(niveau.into()), decalage, phase: None, sous_projet: None, variante: None };
        self.p.ajouter_dalle(contour, &prm)
    }

    /// Ouverture centrée sur la projection de `point` sur l'axe de `mur`.
    fn ouverture(&mut self, mur: &str, genre: &str, type_nom: Option<&str>, point: [f64; 2], dims: [f64; 2], allege: f64) -> R<String> {
        let (axe, _, _) = modele::volumes::solides::hote(&self.p, mur)?;
        let Axe::Droit { debut, fin } = axe else { return Err("mur courbe".into()) };
        let (d, f, q) = (glam::DVec2::from(debut), glam::DVec2::from(fin), glam::DVec2::from(point));
        let abscisse = (q - d).dot((f - d).normalize());
        let type_ouverture = type_nom.map(|n| type_ouv(&self.p, n)).transpose()?;
        let ids = cmd(&mut self.p, json!({"action": "ouverture", "mur": mur, "genre": genre, "type_ouverture": type_ouverture,
            "abscisse": abscisse, "largeur": dims[0], "hauteur": dims[1], "allege": allege}))?;
        Ok(ids[0].clone())
    }

    fn piece(&mut self, niveau: &str, nom: &str, numero: &str, point: [f64; 2]) -> R<String> {
        let n = niveau.to_string();
        Ok(cmd(&mut self.p, json!({"action": "piece", "piece": {"id": "", "nom": nom, "numero": numero, "niveau": n, "point": point}}))?[0].clone())
    }

    fn annoter(&mut self, vue: &str, hauteur: f64, forme: Value) -> R<()> {
        let mut a = json!({"id": "", "vue": vue, "hauteur": hauteur});
        a.as_object_mut().unwrap().extend(forme.as_object().unwrap().clone());
        cmd(&mut self.p, json!({"action": "annoter", "annotation": a}))?;
        Ok(())
    }

    fn texte(&mut self, vue: &str, h: f64, point: [f64; 2], texte: &str) -> R<()> {
        self.annoter(vue, h, json!({"forme": "texte", "point": point, "texte": texte}))
    }

    /// Chaîne de cotes horizontale (y fixe) ou verticale (x fixe) par points successifs.
    fn chaine(&mut self, vue: &str, points: &[[f64; 2]]) -> R<()> {
        for w in points.windows(2) {
            self.annoter(vue, 2.0, json!({"forme": "cote", "debut": {"ancrage": "fixe", "point": w[0]}, "fin": {"ancrage": "fixe", "point": w[1]}, "decalage": 0.0}))?;
        }
        Ok(())
    }
}

/// Libellé court d'une menuiserie en plan.
fn abrege(nom: &str) -> String {
    let cotes = nom.rsplit(' ').next().unwrap_or("");
    let code = if nom.starts_with("Baie coulissante") { "BC" } else if nom.starts_with("Châssis fixe") { "CF" } else if nom.starts_with("Châssis vitré") { "CI" }
        else if nom.starts_with("Fenêtre oscillo") { "OB" } else if nom.starts_with("Porte-fenêtre") { return "PF 140×215".into() } else if nom == "Fenêtre OF 2 vantaux" { return "OF 120×135".into() } else { "OF" };
    format!("{code} {cotes}")
}

fn types_supplementaires(p: &mut Projet) -> R<()> {
    // murs : muret de jardin (assemblage enduit / parpaing / enduit)
    let couche = |m: &str, e: f64, f: FonctionCouche| Couche { materiau: Some(m.into()), epaisseur: e, fonction: f, ..Default::default() };
    p.types_murs.push(TypeCompose { sorte: None, id: "type-mur-muret".into(), nom: "Muret parpaing 200 enduit".into(),
        couches: vec![couche("Enduit", 15.0, FonctionCouche::Finition), couche("Parpaing", 200.0, FonctionCouche::Structure), couche("Enduit", 15.0, FonctionCouche::Finition)],
        categorie: None, couches_detaillees: true, origine: None });
    // terrasse bois sur lambourdes
    p.types_dalles.push(TypeCompose { sorte: Some(SorteDalle::Dalle), id: "type-dalle-terrasse".into(), nom: "Terrasse bois sur lambourdes".into(),
        couches: vec![couche("Lames bois terrasse", 28.0, FonctionCouche::Finition), couche("Lambourdes bois", 45.0, FonctionCouche::Structure)],
        categorie: None, couches_detaillees: true, origine: None });
    p.completer_materiaux();

    // menuiseries : déclinaisons des familles du gabarit, et fenêtres paramétriques
    let gab = |nom: &str| p.types_ouvertures.iter().find(|t| t.nom == nom).cloned().ok_or(format!("type absent : {nom}"));
    let decliner = |base: &TypeOuverture, id: &str, nom: &str, l: f64, h: f64| TypeOuverture { id: id.into(), nom: nom.into(), largeur: l, hauteur: h, famille_nom: Some(base.nom.clone()), ..base.clone() };
    let of2 = gab("Fenêtre OF 2 vantaux")?;
    let porte = gab("Porte intérieure 1 vantail")?;
    let mut nouveaux = vec![
        decliner(&of2, "type-ouv-of2-100x125", "Fenêtre OF 2 vantaux 100×125", 1000.0, 1250.0),
        decliner(&porte, "type-ouv-porte-83", "Porte intérieure 83×204", 830.0, 2040.0),
        decliner(&porte, "type-ouv-porte-73", "Porte intérieure 73×204", 730.0, 2040.0),
    ];
    let fen = |id: &str, nom: &str, l: f64, h: f64, f: FamilleFenetre| TypeOuverture { id: id.into(), nom: nom.into(), genre: GenreOuverture::Fenetre, largeur: l, hauteur: h, famille: Some(f), modele: None, origine: None, famille_nom: None };
    let anthracite = [62, 66, 70];
    nouveaux.push(fen("type-ouv-coulissant", "Baie coulissante 240×215", 2400.0, 2150.0,
        FamilleFenetre { vantaux: vec![Vantail::Coulissant, Vantail::Coulissant], appui: 0.0, dormant: 70.0, couleur: anthracite, ..Default::default() }));
    nouveaux.push(fen("type-ouv-ob-60x75", "Fenêtre oscillo-battante 60×75", 600.0, 750.0,
        FamilleFenetre { vantaux: vec![Vantail::OscilloBattantDroite], couleur: anthracite, ..Default::default() }));
    nouveaux.push(fen("type-ouv-fixe-180x60", "Châssis fixe 180×60", 1800.0, 600.0,
        FamilleFenetre { vantaux: vec![Vantail::Fixe], couleur: anthracite, ..Default::default() }));
    nouveaux.push(fen("type-ouv-imposte-90x215", "Châssis vitré à imposte 90×215", 900.0, 2150.0,
        FamilleFenetre { vantaux: vec![Vantail::Fixe], imposte: 500.0, soubassement: 900.0, appui: 0.0, couleur: anthracite, ..Default::default() }));
    p.types_ouvertures.extend(nouveaux);
    p.types_ouvertures.push(TypeOuverture { id: "type-ouv-entree".into(), nom: "Porte d'entrée pleine 100×215".into(), genre: GenreOuverture::Porte, largeur: 1000.0, hauteur: 2150.0, famille: None, modele: None, origine: None, famille_nom: None });
    Ok(())
}

fn construire() -> R<Projet> {
    let g = modele::gabarit::Gabarit::defaut();
    let mut p = Projet::depuis_gabarit("Maison d'exemple", &g);
    p.coordonnees_origine = [0.0, 0.0, 48_000.0];
    let rdc = p.niveaux.first().ok_or("gabarit sans niveau")?.id.clone();
    let r1 = cmd(&mut p, json!({"action": "niveau", "nom": "R+1", "altitude": 2900.0}))?[0].clone();
    p.ajouter_vue_plan(&r1, 1200.0)?;
    types_supplementaires(&mut p)?;
    let mut m = Maison { p, rdc: rdc.clone(), r1: r1.clone() };
    let (ext, ossa, cloison, muret) = (type_mur(&m.p, "Mur BA 200 + doublage isolé")?, type_mur(&m.p, "Mur ossature bois 145")?, type_mur(&m.p, "Cloison 72/48")?, "type-mur-muret".to_string());
    use Implantation::{Axe as A, Exterieure as E};

    // ---- volume principal : 12,00 × 7,50 m (nus extérieurs), murs de 348 mm
    // tracés dans le sens horaire : l'extérieur est à gauche
    let contour_principal = [[0.0, 0.0], [0.0, 7500.0], [12000.0, 7500.0], [12000.0, 0.0]];
    let mut ext_rdc = Vec::new();
    let mut ext_r1 = Vec::new();
    for k in 0..4 {
        let (a, b) = (contour_principal[k], contour_principal[(k + 1) % 4]);
        ext_rdc.push(m.mur(a, b, &ext, E, &rdc, Some((&r1, 0.0)), 0.0)?);
        ext_r1.push(m.mur(a, b, &ext, E, &r1, None, 2500.0)?);
    }
    let [ouest, nord, est, sud] = [0, 1, 2, 3];

    // ---- aile basse : ossature bois bardée (285 mm), acrotère à 3,40 m
    let aile_s = m.mur([17000.0, 1000.0], [12000.0, 1000.0], &ossa, E, &rdc, None, 3400.0)?;
    let aile_e = m.mur([17000.0, 7500.0], [17000.0, 1000.0], &ossa, E, &rdc, None, 3400.0)?;
    let aile_n = m.mur([12000.0, 7500.0], [17000.0, 7500.0], &ossa, E, &rdc, None, 3400.0)?;

    // ---- cloisons (72/48 : 74 mm), axe ; nus intérieurs du volume principal à 348 mm
    let sous_dalle = Some((r1.as_str(), -310.0));
    let c_sejour = m.mur([7000.0, 348.0], [7000.0, 7152.0], &cloison, A, &rdc, sous_dalle, 0.0)?;
    let c_cuisine = m.mur([7000.0, 3700.0], [11652.0, 3700.0], &cloison, A, &rdc, sous_dalle, 0.0)?;
    let c_aile_x = m.mur([14300.0, 1285.0], [14300.0, 7215.0], &cloison, A, &rdc, None, 2570.0)?;
    let c_wc = m.mur([12000.0, 5600.0], [14300.0, 5600.0], &cloison, A, &rdc, None, 2570.0)?;
    let mut cloisons_r1 = Vec::new();
    for (a, b) in [([3600.0, 348.0], [3600.0, 3200.0]), ([7000.0, 348.0], [7000.0, 3200.0]), ([348.0, 3200.0], [7000.0, 3200.0]),
        ([348.0, 4200.0], [11652.0, 4200.0]), ([4000.0, 4200.0], [4000.0, 7152.0]), ([6600.0, 4200.0], [6600.0, 7152.0])] {
        cloisons_r1.push(m.mur(a, b, &cloison, A, &r1, None, 2500.0)?);
    }

    // ---- muret de la terrasse (banc de 0,60 m)
    m.mur([0.0, -3500.0], [4600.0, -3500.0], &muret, A, &rdc, None, 600.0)?;

    // ---- planchers et toitures
    m.dalle(&rect(0.0, 0.0, 12000.0, 7500.0), SorteDalle::Dalle, "Dalle sur vide sanitaire", &rdc, 0.0, None)?;
    m.dalle(&rect(12000.0, 1000.0, 17000.0, 7500.0), SorteDalle::Dalle, "Dalle sur vide sanitaire", &rdc, 0.0, None)?;
    // plancher haut du rez-de-chaussée, trémie de l'escalier en encoche sur le mur sud
    let plancher = [[348.0, 348.0], [7100.0, 348.0], [7100.0, 1480.0], [11060.0, 1480.0], [11060.0, 348.0], [11652.0, 348.0], [11652.0, 7152.0], [348.0, 7152.0]];
    m.dalle(&plancher, SorteDalle::Dalle, "Dalle BA 230 + chape isolée", &r1, 0.0, None)?;
    m.dalle(&rect(0.0, -3500.0, 7000.0, 0.0), SorteDalle::Dalle, "Terrasse bois sur lambourdes", &rdc, -120.0, None)?;
    // toiture à deux pans, 35°, égout à 2,50 m au-dessus du R+1, débords de 40 et 50 cm
    let toit = m.dalle(&rect(-400.0, -500.0, 12400.0, 8000.0), SorteDalle::Toiture, "Toiture tuiles isolée", &r1, 2500.0, Some(35.0))?;
    {
        let d = m.p.dalles.iter_mut().find(|d| d.id == toit).ok_or("toiture")?;
        let c = d.contours[0].clone();
        // pignons est et ouest : arêtes verticales en plan sans pente
        d.pentes = vec![(0..c.len()).map(|k| if (c[k][0] - c[(k + 1) % c.len()][0]).abs() < 1.0 { None } else { Some(35.0) }).collect()];
    }
    m.dalle(&rect(12000.0, 1285.0, 16715.0, 7215.0), SorteDalle::Toiture, "Toiture végétalisée", &rdc, 3000.0, None)?;
    let auvent = m.dalle(&rect(12000.0, -300.0, 14700.0, 1000.0), SorteDalle::Toiture, "Toiture bac acier", &rdc, 2700.0, None)?;
    let _ = auvent;
    let mut attaches = ext_r1.clone();
    attaches.extend(cloisons_r1.iter().cloned());
    cmd(&mut m.p, json!({"action": "attacher", "ids": attaches, "toiture": toit}))?;
    cmd(&mut m.p, json!({"action": "barre", "barre": {"id": "", "nom": "Poteau de l'auvent", "genre": "poteau", "niveau": rdc,
        "debut": [14550.0, -150.0, 0.0], "fin": [14550.0, -150.0, 2500.0], "largeur": 120.0, "hauteur": 120.0}}))?;

    // ---- menuiseries
    let (of2, of100, ob, fixe, coul, pf, entree, p83, p73, imposte) = ("Fenêtre OF 2 vantaux", "Fenêtre OF 2 vantaux 100×125", "Fenêtre oscillo-battante 60×75", "Châssis fixe 180×60",
        "Baie coulissante 240×215", "Porte-fenêtre 2 vantaux, tableaux et seuil", "Porte d'entrée pleine 100×215", "Porte intérieure 83×204", "Porte intérieure 73×204", "Châssis vitré à imposte 90×215");
    let f = "fenetre";
    let po = "porte";
    // rez-de-chaussée, volume principal
    m.ouverture(&ext_rdc[sud], f, Some(coul), [1900.0, 0.0], [0.0, 0.0], 0.0)?;
    m.ouverture(&ext_rdc[sud], f, Some(coul), [5100.0, 0.0], [0.0, 0.0], 0.0)?;
    m.ouverture(&ext_rdc[sud], f, Some(fixe), [9100.0, 0.0], [0.0, 0.0], 1550.0)?;
    m.ouverture(&ext_rdc[ouest], f, Some(pf), [0.0, 3750.0], [0.0, 0.0], 0.0)?;
    m.ouverture(&ext_rdc[nord], f, Some(of2), [2000.0, 7500.0], [0.0, 0.0], 800.0)?;
    m.ouverture(&ext_rdc[nord], f, Some(of2), [4900.0, 7500.0], [0.0, 0.0], 800.0)?;
    m.ouverture(&ext_rdc[nord], f, Some(of100), [9300.0, 7500.0], [0.0, 0.0], 1100.0)?;
    m.ouverture(&ext_rdc[est], po, Some(p83), [12000.0, 2700.0], [0.0, 0.0], 0.0)?;
    // aile
    m.ouverture(&aile_s, po, Some(entree), [13150.0, 1000.0], [0.0, 0.0], 0.0)?;
    m.ouverture(&aile_s, f, Some(imposte), [12450.0, 1000.0], [0.0, 0.0], 0.0)?;
    m.ouverture(&aile_s, f, Some(of2), [15500.0, 1000.0], [0.0, 0.0], 800.0)?;
    m.ouverture(&aile_e, f, Some(of2), [17000.0, 4250.0], [0.0, 0.0], 800.0)?;
    m.ouverture(&aile_n, f, Some(ob), [13150.0, 7500.0], [0.0, 0.0], 1400.0)?;
    m.ouverture(&aile_n, f, Some(fixe), [15500.0, 7500.0], [0.0, 0.0], 1550.0)?;
    // portes et passages intérieurs du rez-de-chaussée
    m.ouverture(&c_sejour, "libre", None, [7000.0, 2600.0], [1400.0, 2150.0], 0.0)?;
    m.ouverture(&c_sejour, "libre", None, [7000.0, 5400.0], [1800.0, 2150.0], 0.0)?;
    m.ouverture(&c_cuisine, po, Some(p83), [7700.0, 3700.0], [0.0, 0.0], 0.0)?;
    m.ouverture(&c_aile_x, po, Some(p83), [14300.0, 3400.0], [0.0, 0.0], 0.0)?;
    m.ouverture(&c_wc, po, Some(p73), [13150.0, 5600.0], [0.0, 0.0], 0.0)?;
    // étage
    m.ouverture(&ext_r1[sud], f, Some(of2), [2000.0, 0.0], [0.0, 0.0], 800.0)?;
    m.ouverture(&ext_r1[sud], f, Some(of100), [5300.0, 0.0], [0.0, 0.0], 900.0)?;
    m.ouverture(&ext_r1[sud], f, Some(fixe), [9100.0, 0.0], [0.0, 0.0], 1300.0)?;
    m.ouverture(&ext_r1[ouest], f, Some(of100), [0.0, 1600.0], [0.0, 0.0], 900.0)?;
    m.ouverture(&ext_r1[ouest], f, Some(of100), [0.0, 5700.0], [0.0, 0.0], 900.0)?;
    m.ouverture(&ext_r1[nord], f, Some(of2), [2000.0, 7500.0], [0.0, 0.0], 800.0)?;
    m.ouverture(&ext_r1[nord], f, Some(ob), [5300.0, 7500.0], [0.0, 0.0], 1400.0)?;
    m.ouverture(&ext_r1[nord], f, Some(pf), [9100.0, 7500.0], [0.0, 0.0], 0.0)?;
    m.ouverture(&ext_r1[est], f, Some(of100), [12000.0, 5600.0], [0.0, 0.0], 900.0)?;
    let [c_ch1, c_bur, c_couloir_s, c_couloir_n, _c_sdb_o, _c_sdb_e] = [0, 1, 2, 3, 4, 5].map(|k| cloisons_r1[k].clone());
    let _ = (c_ch1, c_bur);
    m.ouverture(&c_couloir_s, po, Some(p83), [2400.0, 3200.0], [0.0, 0.0], 0.0)?;
    m.ouverture(&c_couloir_s, po, Some(p83), [5300.0, 3200.0], [0.0, 0.0], 0.0)?;
    m.ouverture(&c_couloir_n, po, Some(p83), [2400.0, 4200.0], [0.0, 0.0], 0.0)?;
    m.ouverture(&c_couloir_n, po, Some(p73), [5300.0, 4200.0], [0.0, 0.0], 0.0)?;
    m.ouverture(&c_couloir_n, po, Some(p83), [7800.0, 4200.0], [0.0, 0.0], 0.0)?;

    // ---- escalier droit, garde-corps
    let esc = cmd(&mut m.p, json!({"action": "escalier", "escalier": {"id": "", "nom": "Escalier principal", "niveau": rdc, "niveau_haut": r1,
        "hauteur": 2900.0, "trace": [[7150.0, 900.0], [11050.0, 900.0]], "largeur": 1000.0, "hauteur_marche": 180.0, "paillasse": 180.0, "sous_face": "lissee"}}))?[0].clone();
    cmd(&mut m.p, json!({"action": "garde_corps", "garde_corps": {"id": "", "nom": "Main courante", "niveau": rdc, "trace": [[7150.0, 1370.0], [11050.0, 1370.0]],
        "hauteur": 900.0, "remplissage": "barreaudage", "escalier": esc}}))?;
    cmd(&mut m.p, json!({"action": "garde_corps", "garde_corps": {"id": "", "nom": "Garde-corps de la trémie", "niveau": r1,
        "trace": [[7130.0, 400.0], [7130.0, 1510.0], [11000.0, 1510.0]], "hauteur": 1000.0, "remplissage": "vitre"}}))?;
    cmd(&mut m.p, json!({"action": "garde_corps", "garde_corps": {"id": "", "nom": "Garde-corps de la porte-fenêtre", "niveau": r1,
        "trace": [[8300.0, 7560.0], [9900.0, 7560.0]], "hauteur": 1000.0, "remplissage": "barreaudage"}}))?;

    // ---- pièces
    for (niv, nom, num, pt) in [
        (&rdc, "Séjour", "001", [3500.0, 3700.0]), (&rdc, "Hall et escalier", "002", [9300.0, 2600.0]), (&rdc, "Cuisine", "003", [9300.0, 5500.0]),
        (&rdc, "Entrée", "004", [13150.0, 3400.0]), (&rdc, "WC", "005", [13150.0, 6400.0]), (&rdc, "Bureau", "006", [15500.0, 4200.0]),
        (&r1, "Chambre 1", "101", [2000.0, 1600.0]), (&r1, "Chambre 3", "102", [5300.0, 1600.0]), (&r1, "Palier", "103", [9000.0, 3700.0]),
        (&r1, "Chambre 2", "104", [2000.0, 5600.0]), (&r1, "Salle de bains", "105", [5300.0, 5600.0]), (&r1, "Chambre parentale", "106", [9300.0, 5600.0]),
    ] {
        m.piece(niv, nom, num, pt)?;
    }

    // ---- jardin : arbres, table de terrasse
    use modele::elements::generateur_vegetal::Essence;
    for (e, pos) in [(Essence::Feuillu, [[-8000.0, 3500.0], [23500.0, 2500.0]].to_vec()), (Essence::Conifere, vec![[21500.0, -2500.0]]), (Essence::Fastigie, vec![[-3000.0, 11000.0], [1000.0, 11000.0], [5000.0, 11000.0]])] {
        let t = serde_json::to_value(modele::elements::vegetation::type_vegetal(e)).map_err(|e| e.to_string())?;
        let mut t = t;
        t["id"] = json!("");
        let tid = cmd(&mut m.p, json!({"action": "type_composant", "type_composant": t}))?[0].clone();
        for q in pos {
            cmd(&mut m.p, json!({"action": "composant", "composant": {"id": "", "nom": "", "type_composant": tid, "niveau": rdc, "position": q}}))?;
        }
    }
    let mut table = serde_json::to_value(modele::elements::composant::type_table()).map_err(|e| e.to_string())?;
    table["id"] = json!("");
    let tid = cmd(&mut m.p, json!({"action": "type_composant", "type_composant": table}))?[0].clone();
    cmd(&mut m.p, json!({"action": "composant", "composant": {"id": "", "nom": "Table de terrasse", "type_composant": tid, "niveau": rdc, "position": [3500.0, -1800.0], "decalage": -120.0}}))?;
    cmd(&mut m.p, json!({"action": "composant", "composant": {"id": "", "nom": "Table du séjour", "type_composant": tid, "niveau": rdc, "position": [3500.0, 5200.0]}}))?;

    // ---- terrain naturel 15 cm sous le rez-de-chaussée
    let z = m.p.ngf() - 150.0;
    cmd(&mut m.p, json!({"action": "terrain", "terrain": {"id": "", "nom": "Terrain naturel",
        "contour": [[-12000.0, -12000.0, z], [28000.0, -12000.0, z], [28000.0, 18000.0, z], [-12000.0, 18000.0, z]]}}))?;

    // ---- quadrillages
    for (nom, a, b) in [("A", [0.0, -6500.0], [0.0, 10000.0]), ("B", [7000.0, -6500.0], [7000.0, 10000.0]), ("C", [12000.0, -6500.0], [12000.0, 10000.0]), ("D", [17000.0, -6500.0], [17000.0, 10000.0]),
        ("1", [-6000.0, 0.0], [20000.0, 0.0]), ("2", [-6000.0, 7500.0], [20000.0, 7500.0])] {
        cmd(&mut m.p, json!({"action": "quadrillage", "nom": nom, "debut": a, "fin": b}))?;
    }

    // ---- vues
    let vue_plan = |p: &Projet, n: &str| p.vues.iter().find(|v| matches!(&v.genre, GenreVue::Plan { niveau, .. } if niveau == n)).map(|v| v.id.clone()).ok_or("plan absent");
    let (plan0, plan1) = (vue_plan(&m.p, &rdc)?, vue_plan(&m.p, &r1)?);
    for (id, nom) in [(&plan0, "Plan du rez-de-chaussée"), (&plan1, "Plan du R+1")] {
        m.p.renommer_vue(id, nom)?;
    }
    let coupe_aa = m.p.ajouter_vue_coupe([5300.0, -4000.0], [5300.0, 9500.0], 6000.0, false, "Coupe de bâtiment", 100.0)?;
    m.p.renommer_vue(&coupe_aa, "Coupe AA (transversale)")?;
    let coupe_bb = m.p.ajouter_vue_coupe([-2000.0, 900.0], [19000.0, 900.0], 7000.0, false, "Coupe de bâtiment", 100.0)?;
    m.p.renommer_vue(&coupe_bb, "Coupe BB (escalier)")?;
    let mut elevations = Vec::new();
    for (nom, a, b) in [("Façade sud", [-3000.0, -7000.0], [20000.0, -7000.0]), ("Façade nord", [20000.0, 10000.0], [-3000.0, 10000.0]),
        ("Façade est", [19500.0, -4000.0], [19500.0, 10000.0]), ("Façade ouest", [-4000.0, 10000.0], [-4000.0, -4000.0])] {
        let id = m.p.ajouter_vue_coupe(a, b, 30000.0, true, "Élévation extérieure", 100.0)?;
        m.p.renommer_vue(&id, nom)?;
        elevations.push(id);
    }
    let camera = |id: &str, nom: &str, oeil: [f64; 3], cible: [f64; 3], champ: f64| json!({"id": id, "nom": nom, "type_vue": "", "echelle": 100.0, "genre": "camera", "oeil": oeil, "cible": cible, "champ": champ});
    for v in [camera("vue-sud-ouest", "Vue sud-ouest", [-15000.0, -17000.0, 7000.0], [8000.0, 3500.0, 2500.0], 45.0),
        camera("vue-nord-est", "Vue nord-est", [32000.0, 24000.0, 9000.0], [8000.0, 3500.0, 2500.0], 45.0),
        camera("vue-terrasse", "Vue depuis le jardin", [-2500.0, -11000.0, 1600.0], [7000.0, 2000.0, 2600.0], 55.0)] {
        m.p.vues.push(serde_json::from_value(v).map_err(|e| e.to_string())?);
    }

    // cadrage des plans : bâtiment, terrasse et abords
    for (id, c) in [(&plan0, rect(-7500.0, -7000.0, 20500.0, 10000.0)), (&plan1, rect(-3500.0, -3000.0, 19000.0, 9500.0))] {
        m.p.vues.iter_mut().find(|v| &v.id == id).ok_or("vue")?.contour_cadrage = c;
    }
    // ---- annotations du plan du rez-de-chaussée
    m.chaine(&plan0, &[[0.0, -5200.0], [7000.0, -5200.0], [12000.0, -5200.0], [17000.0, -5200.0]])?;
    m.chaine(&plan0, &[[0.0, -6200.0], [17000.0, -6200.0]])?;
    m.chaine(&plan0, &[[-4500.0, -3500.0], [-4500.0, 0.0], [-4500.0, 7500.0]])?;
    m.chaine(&plan0, &[[19500.0, 1000.0], [19500.0, 7500.0]])?;
    m.chaine(&plan0, &[[348.0, 6200.0], [7000.0, 6200.0], [11652.0, 6200.0]])?;
    m.texte(&plan0, 3.0, [3500.0, -2600.0], "Terrasse bois")?;
    m.texte(&plan0, 2.5, [13350.0, 300.0], "Auvent")?;
    m.texte(&plan0, 3.0, [-6500.0, 9500.0], "Jardin")?;
    for (pt, rep) in [([3500.0, 2000.0], [4500.0, 1500.0]), ([5500.0, -1200.0], [6200.0, -700.0]), ([13150.0, 2000.0], [13900.0, 1700.0])] {
        m.annoter(&plan0, 2.0, json!({"forme": "altimetrie", "point": pt, "repere": rep}))?;
    }
    let ouvertures_rdc: Vec<(String, [f64; 2])> = m.p.ouvertures.iter().filter(|o| o.genre == GenreOuverture::Fenetre)
        .filter_map(|o| { let mur = m.p.murs.iter().find(|x| x.id == o.mur || x.assemblage.as_deref() == Some(&o.mur))?; (mur.niveau == rdc).then_some(o.id.clone()) })
        .map(|id| (id, [0.0, 0.0])).collect();
    for (id, _) in ouvertures_rdc {
        let o = m.p.ouvertures.iter().find(|o| o.id == id).unwrap().clone();
        let (axe, _, _) = modele::volumes::solides::hote(&m.p, &o.mur)?;
        let Axe::Droit { debut, fin } = axe else { continue };
        let (d, f) = (glam::DVec2::from(debut), glam::DVec2::from(fin));
        let u = (f - d).normalize();
        // type de la menuiserie à 1,20 m à l'extérieur (à gauche de l'axe) ; les étiquettes
        // ne désignent pas encore les ouvertures
        let q = d + u * o.abscisse + glam::DVec2::new(-u.y, u.x) * 1200.0;
        let nom = abrege(&o.nom);
        m.texte(&plan0, 1.8, q.to_array(), &nom)?;
    }
    // ---- annotations du plan du R+1
    m.chaine(&plan1, &[[0.0, -1800.0], [3600.0, -1800.0], [7000.0, -1800.0], [12000.0, -1800.0]])?;
    m.chaine(&plan1, &[[-1800.0, 0.0], [-1800.0, 3200.0], [-1800.0, 4200.0], [-1800.0, 7500.0]])?;
    m.texte(&plan1, 2.5, [9000.0, 900.0], "Vide sur hall")?;
    m.texte(&plan1, 2.5, [14500.0, 4300.0], "Toiture végétalisée")?;
    m.annoter(&plan1, 2.0, json!({"forme": "altimetrie", "point": [9000.0, 2200.0], "repere": [10000.0, 2600.0]}))?;
    // ---- coupes et façades : textes et cotes de hauteur
    m.texte(&elevations[0], 2.5, [3500.0, -1500.0], "Enduit gratté fin, ton pierre")?;
    m.texte(&elevations[0], 2.5, [14500.0, -1500.0], "Bardage bois à claire-voie")?;

    // ---- nomenclatures
    let nomenclature = |id: &str, nom: &str, cat: &str, regrouper: bool, colonnes: Value| json!({"action": "nomenclature", "nomenclature": {"id": id, "nom": nom, "categorie": cat, "regrouper": regrouper, "colonnes": colonnes}});
    cmd(&mut m.p, nomenclature("nom-fenetres", "Fenêtres", "Fenêtres", true, json!(["type", "largeur", "hauteur", "nombre"])))?;
    cmd(&mut m.p, nomenclature("nom-portes", "Portes", "Portes", true, json!(["type", "largeur", "hauteur", "nombre"])))?;
    cmd(&mut m.p, nomenclature("nom-pieces", "Surfaces des pièces", "Pièces", true, json!(["niveau", "type", "surface"])))?;
    cmd(&mut m.p, nomenclature("nom-murs", "Murs", "Murs", true, json!(["type", "longueur", "surface", "volume"])))?;

    // ---- feuilles A2 : vues centrées dans leur case (mm du papier)
    let cartouche = "Maison d'exemple — BIMedit";
    let placer = |p: &Projet, vue: &str, case: [f64; 4]| -> R<Value> {
        let essai = modele::documentation::VuePlacee { vue: vue.into(), position: [0.0, 0.0], echelle: 100.0 };
        let (_, e) = moteur2d::vue_placee(p, &essai)?;
        let c = [(case[0] + case[2]) / 2.0, (case[1] + case[3]) / 2.0];
        Ok(json!({"vue": vue, "position": [c[0] - (e[0] + e[2]) / 2.0, c[1] - (e[1] + e[3]) / 2.0], "echelle": 100.0}))
    };
    let f1 = json!({"id": "feuille-a01", "numero": "A01", "nom": "Plans", "largeur": 594.0, "hauteur": 420.0, "cartouche": cartouche,
        "vues": [placer(&m.p, &plan0, [15.0, 130.0, 300.0, 410.0])?, placer(&m.p, &plan1, [300.0, 130.0, 585.0, 410.0])?],
        "nomenclatures": [{"nomenclature": "nom-fenetres", "position": [20.0, 120.0]}, {"nomenclature": "nom-portes", "position": [200.0, 120.0]}, {"nomenclature": "nom-pieces", "position": [380.0, 120.0]}]});
    let f2 = json!({"id": "feuille-a02", "numero": "A02", "nom": "Coupes", "largeur": 594.0, "hauteur": 420.0, "cartouche": cartouche,
        "vues": [placer(&m.p, &coupe_aa, [15.0, 60.0, 230.0, 410.0])?, placer(&m.p, &coupe_bb, [230.0, 60.0, 585.0, 410.0])?]});
    let f3 = json!({"id": "feuille-a03", "numero": "A03", "nom": "Façades", "largeur": 594.0, "hauteur": 420.0, "cartouche": cartouche,
        "vues": [placer(&m.p, &elevations[0], [15.0, 230.0, 300.0, 410.0])?, placer(&m.p, &elevations[1], [300.0, 230.0, 585.0, 410.0])?,
                 placer(&m.p, &elevations[2], [15.0, 50.0, 300.0, 230.0])?, placer(&m.p, &elevations[3], [300.0, 50.0, 585.0, 230.0])?]});
    for f in [f1, f2, f3] {
        cmd(&mut m.p, json!({"action": "feuille", "feuille": f}))?;
    }
    let _ = (&m.rdc, &m.r1);
    m.p.verifier()?;
    Ok(m.p)
}

fn main() {
    let sortie = std::env::args().nth(1).unwrap_or_else(|| concat!(env!("CARGO_MANIFEST_DIR"), "/../../samples/maison_exemple.bem").into());
    let r = construire().and_then(|p| {
        maillage::depuis_natif(&p, std::path::Path::new(&sortie))?;
        echanges::enregistrement::enregistrer_natif(&p, std::path::Path::new(&sortie))?;
        Ok(p)
    });
    match r {
        Ok(p) => println!("{sortie} : {} murs ({} assemblages), {} ouvertures, {} pièces, {} annotations, {} vues, {} feuilles", p.murs.len(), p.assemblages.len(), p.ouvertures.len(), p.pieces.len(), p.annotations.len(), p.vues.len(), p.feuilles.len()),
        Err(e) => {
            eprintln!("Erreur : {e}");
            std::process::exit(1);
        }
    }
}
