//! Couche sémantique IFC : unités, arborescence spatiale, propriétés, matériaux, styles.

use crate::step::{Entity, Id, IdMap, StepFile, Value};

pub struct IfcModel {
    pub file: StepFile,
    /// Facteur longueur fichier -> mètres.
    pub length_scale: f64,
    /// Facteur angle plan fichier -> radians.
    pub angle_scale: f64,
    pub project: Option<Id>,
    /// Enfants d'un nœud spatial (agrégation puis contenu).
    pub children: IdMap<Vec<Id>>,
    pub parent: IdMap<Id>,
    /// Objet -> définitions de propriétés (IfcPropertySet, IfcElementQuantity).
    pub property_defs: IdMap<Vec<Id>>,
    pub type_of: IdMap<Id>,
    pub material_of: IdMap<Id>,
    /// Élément hôte -> ouvertures (IfcRelVoidsElement).
    pub openings: IdMap<Vec<Id>>,
    /// Ouverture -> éléments de remplissage (IfcRelFillsElement).
    pub fillings: IdMap<Vec<Id>>,
    /// Item de représentation -> couleur RGBA (IfcStyledItem).
    pub item_color: IdMap<[f32; 4]>,
    /// Matériau -> couleur RGBA (IfcMaterialDefinitionRepresentation).
    pub material_color: IdMap<[f32; 4]>,
    /// Produits : entités avec un placement et une représentation.
    pub products: Vec<Id>,
}

pub struct PropertyGroup {
    pub name: String,
    pub props: Vec<(String, String)>,
}

impl IfcModel {
    pub fn new(file: StepFile) -> Self {
        let mut m = IfcModel {
            file,
            length_scale: 1.0,
            angle_scale: 1.0,
            project: None,
            children: IdMap::default(),
            parent: IdMap::default(),
            property_defs: IdMap::default(),
            type_of: IdMap::default(),
            material_of: IdMap::default(),
            openings: IdMap::default(),
            fillings: IdMap::default(),
            item_color: IdMap::default(),
            material_color: IdMap::default(),
            products: Vec::new(),
        };
        m.index();
        m
    }

    pub fn get(&self, id: Id) -> Option<&Entity> {
        self.file.get(id)
    }

    fn index(&mut self) {
        let f = &self.file;
        let mut aggregates: Vec<(Id, Vec<Id>)> = Vec::new();
        let mut contained: Vec<(Id, Vec<Id>)> = Vec::new();
        let mut styled: Vec<(Id, Id)> = Vec::new();
        let mut material_reps: Vec<(Id, Id)> = Vec::new();

        let mut ids: Vec<Id> = f.entities.keys().copied().collect();
        ids.sort_unstable();

        for &id in &ids {
            let e = &f.entities[&id];
            match &*e.ty {
                "IFCPROJECT" => self.project = Some(id),
                "IFCRELAGGREGATES" | "IFCRELNESTS" => {
                    if let Some(p) = e.arg(4).as_ref() {
                        aggregates.push((p, e.arg(5).refs().collect()));
                    }
                }
                "IFCRELCONTAINEDINSPATIALSTRUCTURE" => {
                    if let Some(p) = e.arg(5).as_ref() {
                        contained.push((p, e.arg(4).refs().collect()));
                    }
                }
                "IFCRELDEFINESBYPROPERTIES" => {
                    // RelatedObjects : liste (IFC2x3/IFC4) ou référence unique (IFC4x3 inversé)
                    if let Some(def) = e.arg(5).as_ref() {
                        for o in e.arg(4).refs().chain(e.arg(4).as_ref()) {
                            self.property_defs.entry(o).or_default().push(def);
                        }
                    }
                }
                "IFCRELDEFINESBYTYPE" => {
                    if let Some(t) = e.arg(5).as_ref() {
                        for o in e.arg(4).refs() {
                            self.type_of.insert(o, t);
                        }
                    }
                }
                "IFCRELASSOCIATESMATERIAL" => {
                    if let Some(mat) = e.arg(5).as_ref() {
                        for o in e.arg(4).refs() {
                            self.material_of.insert(o, mat);
                        }
                    }
                }
                "IFCRELVOIDSELEMENT" => {
                    if let (Some(h), Some(o)) = (e.arg(4).as_ref(), e.arg(5).as_ref()) {
                        self.openings.entry(h).or_default().push(o);
                    }
                }
                "IFCRELFILLSELEMENT" => {
                    if let (Some(o), Some(el)) = (e.arg(4).as_ref(), e.arg(5).as_ref()) {
                        self.fillings.entry(o).or_default().push(el);
                    }
                }
                "IFCSTYLEDITEM" => {
                    if let Some(item) = e.arg(0).as_ref() {
                        styled.push((item, id));
                    }
                }
                "IFCMATERIALDEFINITIONREPRESENTATION" => {
                    if let Some(mat) = e.arg(3).as_ref() {
                        material_reps.push((mat, id));
                    }
                }
                _ => {
                    if is_product(f, e) {
                        self.products.push(id);
                    }
                }
            }
        }

        // Les ouvertures sont rattachées à leur élément hôte dans l'arborescence.
        let mut voids: Vec<(Id, Vec<Id>)> = self.openings.iter().map(|(h, o)| (*h, o.clone())).collect();
        voids.sort_unstable_by_key(|v| v.0);
        for (p, kids) in aggregates.into_iter().chain(contained).chain(voids) {
            for &k in &kids {
                self.parent.entry(k).or_insert(p);
            }
            self.children.entry(p).or_default().extend(kids);
        }

        for (item, styled_item) in styled {
            if let Some(c) = self.styled_item_color(styled_item) {
                self.item_color.entry(item).or_insert(c);
            }
        }
        // Couleurs de matériaux : IfcMaterialDefinitionRepresentation -> IfcStyledRepresentation -> IfcStyledItem.
        for (mat, rep) in material_reps {
            let col = self.get(rep).and_then(|r| {
                r.arg(2).refs().find_map(|sr| {
                    self.get(sr)?.arg(3).refs().find_map(|si| self.styled_item_color(si))
                })
            });
            if let Some(c) = col {
                self.material_color.insert(mat, c);
            }
        }

        self.compute_units();
    }

    fn styled_item_color(&self, styled_item: Id) -> Option<[f32; 4]> {
        let e = self.get(styled_item)?;
        e.arg(1).refs().find_map(|s| self.style_color(s, 0))
    }

    /// Parcours IfcPresentationStyleAssignment / IfcSurfaceStyle / IfcSurfaceStyleRendering.
    fn style_color(&self, id: Id, depth: u32) -> Option<[f32; 4]> {
        if depth > 4 {
            return None;
        }
        let e = self.get(id)?;
        match &*e.ty {
            "IFCPRESENTATIONSTYLEASSIGNMENT" => e.arg(0).refs().find_map(|s| self.style_color(s, depth + 1)),
            "IFCSURFACESTYLE" => e.arg(2).refs().find_map(|s| self.style_color(s, depth + 1)),
            "IFCSURFACESTYLERENDERING" | "IFCSURFACESTYLESHADING" => {
                let c = self.get(e.arg(0).as_ref()?)?;
                let rgb = [c.arg(1).as_f64()?, c.arg(2).as_f64()?, c.arg(3).as_f64()?];
                let t = e.arg(1).as_f64().unwrap_or(0.0).clamp(0.0, 1.0);
                Some([rgb[0] as f32, rgb[1] as f32, rgb[2] as f32, (1.0 - t) as f32])
            }
            _ => None,
        }
    }

    fn compute_units(&mut self) {
        let Some(proj) = self.project.and_then(|p| self.get(p)) else { return };
        let Some(ua) = self.file.deref(proj, 8) else { return };
        for u in ua.arg(0).refs() {
            let Some(ue) = self.get(u) else { continue };
            match ue.arg(1).as_enum() {
                Some("LENGTHUNIT") => {
                    if let Some(s) = self.unit_scale(u, 0) {
                        self.length_scale = s;
                    }
                }
                Some("PLANEANGLEUNIT") => {
                    if let Some(s) = self.unit_scale(u, 0) {
                        self.angle_scale = s;
                    }
                }
                _ => {}
            }
        }
    }

    fn unit_scale(&self, id: Id, depth: u32) -> Option<f64> {
        if depth > 4 {
            return None;
        }
        let e = self.get(id)?;
        match &*e.ty {
            "IFCSIUNIT" => {
                let p = match e.arg(2).as_enum() {
                    Some("EXA") => 1e18,
                    Some("PETA") => 1e15,
                    Some("TERA") => 1e12,
                    Some("GIGA") => 1e9,
                    Some("MEGA") => 1e6,
                    Some("KILO") => 1e3,
                    Some("HECTO") => 1e2,
                    Some("DECA") => 1e1,
                    Some("DECI") => 1e-1,
                    Some("CENTI") => 1e-2,
                    Some("MILLI") => 1e-3,
                    Some("MICRO") => 1e-6,
                    Some("NANO") => 1e-9,
                    _ => 1.0,
                };
                Some(p)
            }
            "IFCCONVERSIONBASEDUNIT" | "IFCCONVERSIONBASEDUNITWITHOFFSET" => {
                let mwu = self.file.deref(e, 3)?;
                let v = mwu.arg(0).as_f64()?;
                let base = mwu.arg(1).as_ref().and_then(|u| self.unit_scale(u, depth + 1)).unwrap_or(1.0);
                Some(v * base)
            }
            _ => None,
        }
    }

    // ---------------------------------------------------------------- Accès

    pub fn ty(&self, id: Id) -> &str {
        self.get(id).map(|e| &*e.ty).unwrap_or("?")
    }

    pub fn name(&self, id: Id) -> String {
        let Some(e) = self.get(id) else { return String::new() };
        let n = e.arg(2).as_str().unwrap_or("");
        if !n.is_empty() {
            return n.to_string();
        }
        e.arg(5).as_str().map(|s| s.to_string()).unwrap_or_default()
    }

    pub fn label(&self, id: Id) -> String {
        let n = self.name(id);
        let t = pretty_type(self.ty(id));
        if n.is_empty() { format!("{t} #{id}") } else { n }
    }

    pub fn global_id(&self, id: Id) -> String {
        self.get(id).and_then(|e| e.arg(0).as_str()).unwrap_or("").to_string()
    }

    /// Niveau (IfcBuildingStorey) parent de l'élément, s'il existe.
    pub fn storey_of(&self, mut id: Id) -> Option<Id> {
        for _ in 0..32 {
            id = *self.parent.get(&id)?;
            if self.ty(id) == "IFCBUILDINGSTOREY" {
                return Some(id);
            }
        }
        None
    }

    /// Propriétés d'identité d'un objet.
    pub fn attributes(&self, id: Id) -> Vec<(String, String)> {
        let mut out = Vec::new();
        let Some(e) = self.get(id) else { return out };
        out.push(("Classe".into(), pretty_type(&e.ty)));
        out.push(("Id STEP".into(), format!("#{id}")));
        let names = ["GlobalId", "", "Nom", "Description", "ObjectType"];
        for (i, n) in names.iter().enumerate() {
            if n.is_empty() {
                continue;
            }
            let v = e.arg(i).display();
            if !v.is_empty() {
                out.push((n.to_string(), v));
            }
        }
        if let Some(t) = self.type_of.get(&id) {
            out.push(("Type".into(), self.label(*t)));
        }
        if let Some(s) = self.storey_of(id) {
            out.push(("Niveau".into(), self.label(s)));
        }
        if let Some(m) = self.material_of.get(&id) {
            let mats = self.material_names(*m);
            if !mats.is_empty() {
                out.push(("Matériaux".into(), mats.join(" | ")));
            }
        }
        if e.is("IFCBUILDINGSTOREY") {
            if let Some(z) = e.arg(9).as_f64() {
                out.push(("Élévation".into(), format!("{} m", crate::step::format_real(z * self.length_scale))));
            }
        }
        out
    }

    /// Noms des matériaux associés (couches avec épaisseur).
    pub fn material_names(&self, id: Id) -> Vec<String> {
        let mut out = Vec::new();
        self.collect_materials(id, &mut out, 0);
        out
    }

    fn collect_materials(&self, id: Id, out: &mut Vec<String>, depth: u32) {
        if depth > 5 {
            return;
        }
        let Some(e) = self.get(id) else { return };
        match &*e.ty {
            "IFCMATERIAL" => out.push(e.arg(0).display()),
            "IFCMATERIALLAYERSETUSAGE" | "IFCMATERIALPROFILESETUSAGE" | "IFCMATERIALPROFILESETUSAGETAPERING" => {
                if let Some(s) = e.arg(0).as_ref() {
                    self.collect_materials(s, out, depth + 1)
                }
            }
            "IFCMATERIALLAYERSET" => {
                for l in e.arg(0).refs() {
                    self.collect_materials(l, out, depth + 1)
                }
            }
            "IFCMATERIALLAYER" => {
                let name = e.arg(0).as_ref().map(|m| self.material_names(m).join(", ")).unwrap_or_default();
                let th = e.arg(1).as_f64().map(|t| t * self.length_scale * 1000.0);
                match th {
                    Some(t) => out.push(format!("{name} ({} mm)", crate::step::format_real(t))),
                    None => out.push(name),
                }
            }
            "IFCMATERIALLIST" => {
                for m in e.arg(0).refs() {
                    self.collect_materials(m, out, depth + 1)
                }
            }
            "IFCMATERIALCONSTITUENTSET" | "IFCMATERIALPROFILESET" => {
                for c in e.arg(2).refs() {
                    self.collect_materials(c, out, depth + 1)
                }
            }
            "IFCMATERIALCONSTITUENT" | "IFCMATERIALPROFILE" => {
                if let Some(m) = e.arg(2).as_ref() {
                    self.collect_materials(m, out, depth + 1)
                }
            }
            _ => {}
        }
    }

    /// Premier IfcMaterial porteur d'une couleur pour un matériau associé.
    pub fn material_rgba(&self, id: Id) -> Option<[f32; 4]> {
        self.material_rgba_rec(id, 0)
    }

    fn material_rgba_rec(&self, id: Id, depth: u32) -> Option<[f32; 4]> {
        if depth > 5 {
            return None;
        }
        if let Some(c) = self.material_color.get(&id) {
            return Some(*c);
        }
        let e = self.get(id)?;
        let next: Vec<Id> = match &*e.ty {
            "IFCMATERIALLAYERSETUSAGE" | "IFCMATERIALPROFILESETUSAGE" | "IFCMATERIALPROFILESETUSAGETAPERING" => {
                e.arg(0).as_ref().into_iter().collect()
            }
            "IFCMATERIALLAYERSET" | "IFCMATERIALLIST" => e.arg(0).refs().collect(),
            "IFCMATERIALLAYER" => e.arg(0).as_ref().into_iter().collect(),
            "IFCMATERIALCONSTITUENTSET" | "IFCMATERIALPROFILESET" => e.arg(2).refs().collect(),
            "IFCMATERIALCONSTITUENT" | "IFCMATERIALPROFILE" => e.arg(2).as_ref().into_iter().collect(),
            _ => vec![],
        };
        next.into_iter().find_map(|n| self.material_rgba_rec(n, depth + 1))
    }

    /// Jeux de propriétés et de quantités, y compris ceux du type.
    pub fn property_groups(&self, id: Id) -> Vec<PropertyGroup> {
        let mut out = Vec::new();
        if let Some(defs) = self.property_defs.get(&id) {
            for &d in defs {
                if let Some(g) = self.property_group(d, "") {
                    out.push(g);
                }
            }
        }
        if let Some(&t) = self.type_of.get(&id) {
            if let Some(te) = self.get(t) {
                // IfcTypeObject.HasPropertySets (index 5)
                for d in te.arg(5).refs() {
                    if let Some(g) = self.property_group(d, " (type)") {
                        out.push(g);
                    }
                }
            }
            if let Some(defs) = self.property_defs.get(&t) {
                for &d in defs {
                    if let Some(g) = self.property_group(d, " (type)") {
                        out.push(g);
                    }
                }
            }
        }
        out
    }

    fn property_group(&self, id: Id, suffix: &str) -> Option<PropertyGroup> {
        let e = self.get(id)?;
        let name = format!("{}{}", e.arg(2).as_str().unwrap_or("?"), suffix);
        let mut props = Vec::new();
        match &*e.ty {
            "IFCPROPERTYSET" => {
                for p in e.arg(4).refs() {
                    self.collect_property(p, "", &mut props, 0);
                }
            }
            "IFCELEMENTQUANTITY" => {
                for q in e.arg(5).refs() {
                    self.collect_quantity(q, &mut props);
                }
            }
            _ => return None,
        }
        Some(PropertyGroup { name, props })
    }

    fn collect_property(&self, id: Id, prefix: &str, out: &mut Vec<(String, String)>, depth: u32) {
        let Some(e) = self.get(id) else { return };
        let name = format!("{prefix}{}", e.arg(0).as_str().unwrap_or("?"));
        let val = match &*e.ty {
            "IFCPROPERTYSINGLEVALUE" => e.arg(2).display(),
            "IFCPROPERTYENUMERATEDVALUE" | "IFCPROPERTYLISTVALUE" => e.arg(2).display(),
            "IFCPROPERTYBOUNDEDVALUE" => format!("{} … {}", e.arg(3).display(), e.arg(2).display()),
            "IFCPROPERTYTABLEVALUE" => format!("{} → {}", e.arg(2).display(), e.arg(3).display()),
            "IFCPROPERTYREFERENCEVALUE" => e.arg(2).display(),
            "IFCCOMPLEXPROPERTY" => {
                if depth < 4 {
                    for p in e.arg(3).refs() {
                        self.collect_property(p, &format!("{name}."), out, depth + 1);
                    }
                }
                return;
            }
            _ => String::new(),
        };
        out.push((name, val));
    }

    fn collect_quantity(&self, id: Id, out: &mut Vec<(String, String)>) {
        let Some(e) = self.get(id) else { return };
        let name = e.arg(0).as_str().unwrap_or("?").to_string();
        let raw = e.arg(3).as_f64();
        let val = match (&*e.ty, raw) {
            ("IFCQUANTITYLENGTH", Some(v)) => format!("{} m", crate::step::format_real(v * self.length_scale)),
            ("IFCQUANTITYAREA", Some(v)) => {
                format!("{} m²", crate::step::format_real(v * self.length_scale * self.length_scale))
            }
            ("IFCQUANTITYVOLUME", Some(v)) => {
                format!("{} m³", crate::step::format_real(v * self.length_scale.powi(3)))
            }
            ("IFCQUANTITYWEIGHT", Some(v)) => format!("{} kg", crate::step::format_real(v)),
            ("IFCPHYSICALCOMPLEXQUANTITY", _) => {
                for q in e.arg(2).refs() {
                    self.collect_quantity(q, out);
                }
                return;
            }
            (_, _) => e.arg(3).display(),
        };
        out.push((name, val));
    }
}

/// Un produit IFC a GlobalId (22 car.), ObjectPlacement (5) et Representation (6).
fn is_product(f: &StepFile, e: &Entity) -> bool {
    if e.args.len() < 7 {
        return false;
    }
    if !matches!(e.arg(0), Value::Str(s) if s.len() == 22) {
        return false;
    }
    let Some(rep) = e.arg(6).as_ref() else { return false };
    matches!(f.get(rep), Some(r) if r.is("IFCPRODUCTDEFINITIONSHAPE"))
}

/// "IFCWALLSTANDARDCASE" -> "IfcWallStandardCase".
pub fn pretty_type(ty: &str) -> String {
    const WORDS: &[&str] = &[
        "STANDARD", "CASE", "ELEMENTED", "BUILDING", "STOREY", "TYPE",
        "FURNISHING", "FURNITURE", "ELEMENT", "PROXY", "OPENING", "COVERING", "CURTAIN", "RAILING",
        "FLIGHT", "STAIR", "RAMP", "WALL", "SLAB", "ROOF", "DOOR", "WINDOW", "BEAM", "COLUMN", "MEMBER",
        "PLATE", "FOOTING", "PILE", "SPACE", "SITE", "PROJECT", "ANNOTATION", "GRID", "SEGMENT",
        "FITTING", "TERMINAL", "FLOW", "DISTRIBUTION", "CONTROLLER", "ENERGY", "CONVERSION", "DEVICE",
        "STORAGE", "MOVING", "TREATMENT", "CONTROL", "TRANSPORT", "ASSEMBLY", "VIRTUAL", "SHADING",
        "CHIMNEY", "SYSTEM", "FASTENER", "MECHANICAL", "DISCRETE", "ACCESSORY", "REINFORCING", "BAR",
        "MESH", "TENDON", "ANCHOR", "GEOGRAPHIC", "CIVIL", "LIGHT", "FIXTURE", "SANITARY", "AIR",
        "DUCT", "PIPE", "CABLE", "CARRIER", "VALVE", "PUMP", "FAN", "SENSOR", "ACTUATOR", "ALARM",
        "OUTLET", "SWITCHING", "PROTECTIVE", "TANK", "BOILER", "CHILLER", "COIL", "UNITARY", "EQUIPMENT",
        "TERRAIN", "SURFACE", "FEATURE", "PROJECTION", "ZONE", "PORT", "COURSE", "PAVEMENT", "KERB",
        "BEARING", "EARTHWORKS", "FILL", "CUT", "RAIL", "SIGN", "SIGNAL", "TRACK", "ALIGNMENT",
    ];
    let Some(rest) = ty.strip_prefix("IFC") else { return ty.to_string() };
    // Mots les plus longs d'abord (ELEMENTED avant ELEMENT).
    let mut words: Vec<&str> = WORDS.to_vec();
    words.sort_by_key(|w| std::cmp::Reverse(w.len()));
    let mut out = String::from("Ifc");
    let mut i = 0;
    let b = rest.as_bytes();
    'outer: while i < b.len() {
        for w in &words {
            if rest[i..].starts_with(w) {
                let mut cs = w.chars();
                out.push(cs.next().unwrap());
                out.extend(cs.map(|c| c.to_ascii_lowercase()));
                i += w.len();
                continue 'outer;
            }
        }
        // Mot inconnu : on garde le reste en minuscules avec majuscule initiale.
        let mut cs = rest[i..].chars();
        out.push(cs.next().unwrap());
        out.extend(cs.map(|c| c.to_ascii_lowercase()));
        break;
    }
    out
}

#[cfg(test)]
mod tests {
    #[test]
    fn pretty() {
        assert_eq!(super::pretty_type("IFCBUILDINGELEMENTPROXY"), "IfcBuildingElementProxy");
        assert_eq!(super::pretty_type("IFCWALLSTANDARDCASE"), "IfcWallStandardCase");
        assert_eq!(super::pretty_type("IFCBUILDINGSTOREY"), "IfcBuildingStorey");
        assert_eq!(super::pretty_type("IFCFURNISHINGELEMENT"), "IfcFurnishingElement");
    }
}
