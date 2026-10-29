//! Export IFC4 en millimètres : classes, niveaux, matériaux et géométrie tessellée.
use super::*;
use std::collections::HashMap;
use std::fmt::Write;
struct Step {
    lignes: Vec<String>,
}
impl Step {
    fn add(&mut self, s: String) -> usize {
        self.lignes.push(s);
        self.lignes.len()
    }
    fn mesh(&mut self, tris: &[[glam::DVec3; 3]], contexte: usize) -> usize {
        let m = Maillage::depuis_triangles(tris);
        if m.triangles.is_empty() {
            return 0;
        }
        let points = m
            .sommets
            .iter()
            .map(|p| format!("({:.6},{:.6},{:.6})", p[0], p[1], p[2]))
            .collect::<Vec<_>>()
            .join(",");
        let pts = self.add(format!("IFCCARTESIANPOINTLIST3D(({points}))"));
        let faces = m
            .triangles
            .iter()
            .map(|t| format!("({},{},{})", t[0] + 1, t[1] + 1, t[2] + 1))
            .collect::<Vec<_>>()
            .join(",");
        let faces = self.add(format!("IFCTRIANGULATEDFACESET(#{pts},$,$,({faces}),$)"));
        let rep = self.add(format!(
            "IFCSHAPEREPRESENTATION(#{contexte},'Body','Tessellation',(#{faces}))"
        ));
        self.add(format!("IFCPRODUCTDEFINITIONSHAPE($,$,(#{rep}))"))
    }
}
fn texte(s: &str) -> String {
    let mut out = String::from("'");
    for c in s.chars() {
        if c == '\'' {
            out.push_str("''");
        } else if c == '\\' || !c.is_ascii() || c.is_control() {
            out.push_str("\\X2\\");
            for n in c.encode_utf16(&mut [0; 2]).iter() {
                let _ = write!(out, "{n:04X}");
            }
            out.push_str("\\X0\\");
        } else {
            out.push(c);
        }
    }
    out.push('\'');
    out
}
fn ref_(i: usize) -> String {
    if i == 0 { "$".into() } else { format!("#{i}") }
}
fn refs(ids: &[usize]) -> String {
    format!(
        "({})",
        ids.iter()
            .map(|i| format!("#{i}"))
            .collect::<Vec<_>>()
            .join(",")
    )
}
fn guid(cle: &str, origine: Option<&Origine>) -> String {
    let alphabet = b"0123456789ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz_$";
    if let Some(g) = origine.and_then(|o| o.guid.as_ref()).filter(|g| {
        g.len() == 22 && g.bytes().all(|b| alphabet.contains(&b)) && g.as_bytes()[0] <= b'3'
    }) {
        return g.clone();
    }
    let mut h = 0x6c62272e07bb014262b821756295c58du128;
    for b in cle.bytes() {
        h = (h ^ (b as u128)).wrapping_mul(0x1000000000000000000013b);
    }
    let mut s = [b'0'; 22];
    for i in (0..22).rev() {
        s[i] = alphabet[(h & 63) as usize];
        h >>= 6;
    }
    String::from_utf8(s.to_vec()).unwrap()
}

pub fn exporter(p: &Projet) -> Result<String, String> {
    p.verifier()?;
    let mut w = Step { lignes: Vec::new() };
    let zero = w.add("IFCCARTESIANPOINT((0.,0.,0.))".into());
    let axe = w.add(format!("IFCAXIS2PLACEMENT3D(#{zero},$,$)"));
    let placement = w.add(format!("IFCLOCALPLACEMENT($,#{axe})"));
    let contexte = w.add(format!(
        "IFCGEOMETRICREPRESENTATIONCONTEXT($,'Model',3,0.00001,#{axe},$)"
    ));
    let unite = w.add("IFCSIUNIT(*,.LENGTHUNIT.,.MILLI.,.METRE.)".into());
    let unites = w.add(format!("IFCUNITASSIGNMENT((#{unite}))"));
    let projet = w.add(format!(
        "IFCPROJECT('{}',$,{},$,$,$,$,(#{contexte}),#{unites})",
        guid(&format!("projet:{}", p.nom), None),
        texte(&p.nom)
    ));
    let bat = w.add(format!(
        "IFCBUILDING('{}',$,{},$,$,#{placement},$,$,.ELEMENT.,$,$,$)",
        guid(&format!("bat:{}", p.nom), None),
        texte(&p.nom)
    ));
    w.add(format!(
        "IFCRELAGGREGATES('{}',$,$,$,#{projet},(#{bat}))",
        guid("rel-projet", None)
    ));
    let mut niveaux = HashMap::new();
    let mut ns = Vec::new();
    for n in &p.niveaux {
        let id = w.add(format!(
            "IFCBUILDINGSTOREY('{}',$,{},$,$,#{placement},$,$,.ELEMENT.,{})",
            guid(&n.id, n.origine.as_ref()),
            texte(&n.nom),
            format!("{:.6}", n.altitude)
        ));
        niveaux.insert(n.id.clone(), id);
        ns.push(id);
    }
    if !ns.is_empty() {
        w.add(format!(
            "IFCRELAGGREGATES('{}',$,$,$,#{bat},{})",
            guid("rel-niveaux", None),
            refs(&ns)
        ));
    }
    let mut types = HashMap::new();
    for t in p.types_murs.iter().chain(&p.types_dalles) {
        let mut cs = Vec::new();
        for c in &t.couches {
            let mat = w.add(format!(
                "IFCMATERIAL({},$,$)",
                texte(c.materiau.as_deref().unwrap_or("Matériau"))
            ));
            cs.push(w.add(format!(
                "IFCMATERIALLAYER(#{mat},{},$,$,$,$,$)",
                format!("{:.6}", c.epaisseur)
            )));
        }
        let set = w.add(format!(
            "IFCMATERIALLAYERSET({},{},$)",
            refs(&cs),
            texte(&t.nom)
        ));
        types.insert(t.id.clone(), set);
    }
    let mut contenus: HashMap<usize, Vec<usize>> = HashMap::new();
    let mut produits = HashMap::new();
    let jonctions = jonctions::coupes(p);
    for m in &p.murs {
        let rep = w.mesh(&solides::mur(p, m, &jonctions)?, contexte);
        let id = w.add(format!(
            "IFCWALL('{}',$,{},$,$,#{placement},{},$,.NOTDEFINED.)",
            guid(&m.id, m.origine.as_ref()),
            texte(&m.nom),
            ref_(rep)
        ));
        contenus
            .entry(*niveaux.get(&m.niveau).unwrap_or(&bat))
            .or_default()
            .push(id);
        produits.insert(m.id.clone(), id);
        w.add(format!(
            "IFCRELASSOCIATESMATERIAL('{}',$,$,$,(#{id}),#{})",
            guid(&format!("mat:{}", m.id), None),
            types[&m.type_mur]
        ));
    }
    for a in &p.assemblages {
        let id = w.add(format!(
            "IFCELEMENTASSEMBLY('{}',$,{},$,$,#{placement},$,$,$,.NOTDEFINED.)",
            guid(&a.id, a.origine.as_ref()),
            texte(&a.nom)
        ));
        let couches: Vec<_> = a
            .couches
            .iter()
            .filter_map(|id| produits.get(id).copied())
            .collect();
        for objets in contenus.values_mut() {
            objets.retain(|x| !couches.contains(x));
        }
        let niveau = p
            .couches_assemblage(&a.id)
            .first()
            .and_then(|m| niveaux.get(&m.niveau))
            .copied()
            .unwrap_or(bat);
        contenus.entry(niveau).or_default().push(id);
        w.add(format!(
            "IFCRELAGGREGATES('{}',$,$,$,#{id},{})",
            guid(&format!("couches:{}", a.id), None),
            refs(&couches)
        ));
        produits.insert(a.id.clone(), id);
    }
    for d in &p.dalles {
        let t = p.type_dalle(&d.type_dalle).unwrap();
        let z = p.niveau(&d.niveau).unwrap().altitude + d.decalage;
        let (b, h) = d.decalages_couches(t.epaisseur());
        let tris = pente::couche(&d.contours, &pente::plans(d), z, b, h)
            .into_iter()
            .map(|t| t.map(|v| v * 1000.0))
            .collect::<Vec<_>>();
        let rep = w.mesh(&tris, contexte);
        let (classe, genre) = match d.sorte {
            SorteDalle::Dalle => ("IFCSLAB", "FLOOR"),
            SorteDalle::Sol => ("IFCCOVERING", "FLOORING"),
            SorteDalle::Plafond => ("IFCCOVERING", "CEILING"),
            SorteDalle::Toiture => ("IFCROOF", "NOTDEFINED"),
        };
        let id = w.add(format!(
            "{classe}('{}',$,{},$,$,#{placement},{},$,.{genre}.)",
            guid(&d.id, d.origine.as_ref()),
            texte(&d.nom),
            ref_(rep)
        ));
        contenus.entry(niveaux[&d.niveau]).or_default().push(id);
        w.add(format!(
            "IFCRELASSOCIATESMATERIAL('{}',$,$,$,(#{id}),#{})",
            guid(&format!("mat:{}", d.id), None),
            types[&d.type_dalle]
        ));
    }
    for b in &p.barres {
        let rep = w.mesh(&solides::barre(p, b)?, contexte);
        let classe = match b.genre { GenreBarre::Poteau => "IFCCOLUMN", GenreBarre::Poutre => "IFCBEAM" };
        let id = w.add(format!("{classe}('{}',$,{},$,$,#{placement},{},$,.NOTDEFINED.)", guid(&b.id, None), texte(&b.nom), ref_(rep)));
        contenus.entry(niveaux[&b.niveau]).or_default().push(id);
    }
    for o in &p.ouvertures {
        let (axe, e, niveau) = solides::hote(p, &o.mur)?;
        let z = p.niveau(&niveau).unwrap().altitude + o.allege;
        let vide = solides::prisme(
            &solides::bande(
                &axe,
                o.abscisse - o.largeur / 2.0,
                o.abscisse + o.largeur / 2.0,
                e,
            )?,
            z,
            z + o.hauteur,
        );
        let rep = w.mesh(&vide, contexte);
        let id = w.add(format!(
            "IFCOPENINGELEMENT('{}',$,{},$,$,#{placement},{},$,.OPENING.)",
            guid(&format!("vide:{}", o.id), None),
            texte(&o.nom),
            ref_(rep)
        ));
        // IFC impose un IfcElement hôte ; un assemblage est un IfcElement.
        w.add(format!(
            "IFCRELVOIDSELEMENT('{}',$,$,$,#{},#{id})",
            guid(&format!("perc:{}", o.id), None),
            produits[&o.mur]
        ));
        if o.genre != GenreOuverture::Libre {
            let rep = w.mesh(&solides::ouverture(p, o)?, contexte);
            let classe = if o.genre == GenreOuverture::Porte {
                "IFCDOOR"
            } else {
                "IFCWINDOW"
            };
            let element = w.add(format!(
                "{classe}('{}',$,{},$,$,#{placement},{},$,{},{},.NOTDEFINED.,.NOTDEFINED.,$)",
                guid(&o.id, o.origine.as_ref()),
                texte(&o.nom),
                ref_(rep),
                format!("{:.6}", o.hauteur),
                format!("{:.6}", o.largeur)
            ));
            contenus.entry(niveaux[&niveau]).or_default().push(element);
            w.add(format!(
                "IFCRELFILLSELEMENT('{}',$,$,$,#{id},#{element})",
                guid(&format!("rempl:{}", o.id), None)
            ));
        }
    }
    for e in &p.importes {
        let rep = w.mesh(&solides::maillage(&e.maillage)?, contexte);
        let id = w.add(format!(
            "IFCBUILDINGELEMENTPROXY('{}',$,{},{},{},#{placement},{},$,.NOTDEFINED.)",
            guid(&e.id, e.origine.as_ref()),
            texte(&e.nom),
            texte(&e.raison),
            texte(&e.categorie),
            ref_(rep)
        ));
        contenus
            .entry(
                e.niveau
                    .as_ref()
                    .and_then(|n| niveaux.get(n))
                    .copied()
                    .unwrap_or(bat),
            )
            .or_default()
            .push(id);
    }
    let mut contenus: Vec<_> = contenus.into_iter().collect();
    contenus.sort_by_key(|(id, _)| *id);
    for (n, ids) in contenus {
        w.add(format!(
            "IFCRELCONTAINEDINSPATIALSTRUCTURE('{}',$,$,$,{},#{n})",
            guid(&format!("contenu:{n}"), None),
            refs(&ids)
        ));
    }
    let mut out = String::from(
        "ISO-10303-21;\nHEADER;\nFILE_DESCRIPTION(('ViewDefinition [ReferenceView_V1.2]'),'2;1');\nFILE_NAME('modele.ifc','',(''),(''),'ifc_viewer','ifc_viewer','');\nFILE_SCHEMA(('IFC4'));\nENDSEC;\nDATA;\n",
    );
    for (i, l) in w.lignes.iter().enumerate() {
        let _ = writeln!(out, "#{}={l};", i + 1);
    }
    out.push_str("ENDSEC;\nEND-ISO-10303-21;\n");
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn export_ifc4_relecture_geometrie_et_identifiants() {
        let (p, _) =
            depuis_ifc::convertir(std::path::Path::new("samples/test_ifc4_murs.ifc"), &|_| {})
                .unwrap();
        let s = exporter(&p).unwrap();
        assert_eq!(s, exporter(&p).unwrap());
        let m = crate::model::IfcModel::new(crate::step::StepFile::parse(s.as_bytes()).unwrap());
        assert_eq!(m.file.schema, "IFC4");
        assert!(!m.products.is_empty());
        let stats = crate::geom::GeomStats::default();
        let count = m
            .products
            .iter()
            .filter(|&&id| crate::geom::product_geometry(&m, &stats, id).is_some())
            .count();
        assert!(count >= p.murs.len() + p.dalles.len() + p.importes.len());
    }
}
