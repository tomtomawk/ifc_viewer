//! Formules Revit : arbres d'expression des paramètres de familles
//! (`NamedParam.m_oExpression`) et des champs calculés des nomenclatures
//! (`ScheduleField.m_pExpression`) (LISEZMOI-RVT.md du dossier Decodage RVT, § 7
//! « Formules »).
//!
//! Classes dérivées de `Expression` : `BinaryOperatorExpression` (`m_binaryOperator`,
//! `m_pLeftSubexpression`, `m_pRightSubexpression`), `FunctionExpression`
//! (`m_function`, `m_subexpressions`), `ParenExpression` (`m_pSubexpression`),
//! `UnaryOperatorExpression`, `NumberConstantExpression` (`m_value` en unités
//! internes, `m_specTypeId`), `StringConstantExpression`, `ParameterExpression`
//! (`m_paramId`), `ScheduleFieldExpression` (`m_fieldId`).
//!
//! Codes établis en évaluant les formules des familles contre les valeurs que Revit
//! enregistre dans les types (572 formules de Kerreut et Saint-Malo) : opérateurs 1 +,
//! 2 −, 3 ×, 4 ÷, 6 =, 7 >, 8 < ; fonctions 2 cos, 10 if, 12 and, 13 not, 18 round,
//! 19 roundup. Seuls les codes vérifiés sont évalués : une formule qui en emploie un
//! autre n'a pas de valeur.

use super::champs::{Obj, Record, Val};
use super::parametres::{Brut, Stockage};

#[derive(Clone, Debug, PartialEq)]
pub enum Expr {
    Nombre(f64),
    Texte(String),
    /// Paramètre (id de l'élément paramètre, ou id négatif d'un paramètre intégré).
    Param(i64),
    /// Autre champ de la nomenclature (`ScheduleFieldId.m_id`).
    Champ(i64),
    Binaire(i64, Box<Expr>, Box<Expr>),
    Unaire(i64, Box<Expr>),
    Fonction(i64, Vec<Expr>),
    Parenthese(Box<Expr>),
    /// Classe non lue.
    Autre(String),
}

/// Valeur d'une expression : nombre (unités internes ; un Oui/Non vaut 1 ou 0) ou texte.
#[derive(Clone, Debug, PartialEq)]
pub enum Valeur {
    Nombre(f64),
    Texte(String),
}

impl Valeur {
    pub fn nombre(&self) -> Option<f64> {
        match self {
            Valeur::Nombre(x) => Some(*x),
            Valeur::Texte(_) => None,
        }
    }
}

/// Valeur d'un paramètre pour une formule (un ElementId vaut son numéro).
pub fn de_brut(b: &Brut, st: Stockage) -> Valeur {
    match b.selon(st) {
        Brut::Reel(x) => Valeur::Nombre(x),
        Brut::Entier(i) | Brut::Id(i) => Valeur::Nombre(i as f64),
        Brut::Texte(s) => Valeur::Texte(s),
        Brut::Nomme { reel, .. } => Valeur::Nombre(reel),
    }
}

/// Objet désigné par un pointeur possédé, ou incorporé.
fn corps<'r, 's>(r: &'r Record<'s>, v: &'r Val<'s>) -> Option<&'r Obj<'s>> {
    r.deref(v).map(|b| &b.obj).or(v.obj())
}

/// Arbre d'une expression.
pub fn lire(r: &Record, o: &Obj) -> Option<Expr> {
    let sous = |k: &str| o.get(k).and_then(|v| corps(r, v)).and_then(|x| lire(r, x)).map(Box::new);
    let int = |k: &str| o.get(k).and_then(Val::int).unwrap_or(-1);
    Some(match o.class_name() {
        "BinaryOperatorExpression" => Expr::Binaire(int("m_binaryOperator"), sous("m_pLeftSubexpression")?, sous("m_pRightSubexpression")?),
        "UnaryOperatorExpression" => Expr::Unaire(int("m_unaryOperator"), sous("m_pSubexpression")?),
        "ParenExpression" => Expr::Parenthese(sous("m_pSubexpression")?),
        "FunctionExpression" => Expr::Fonction(
            int("m_function"),
            o.get("m_subexpressions").map(Val::list).unwrap_or_default().iter().map(|v| corps(r, v).and_then(|x| lire(r, x))).collect::<Option<Vec<_>>>()?,
        ),
        "NumberConstantExpression" => Expr::Nombre(o.get("m_value").and_then(Val::real)?),
        "StringConstantExpression" => Expr::Texte(o.get("m_value").and_then(Val::str).unwrap_or_default().to_string()),
        "ParameterExpression" => Expr::Param(o.get("m_paramId").and_then(Val::element_id)?),
        "ScheduleFieldExpression" => Expr::Champ(o.get("m_fieldId").and_then(Val::obj).and_then(|f| f.get("m_id")).and_then(Val::int)?),
        c => Expr::Autre(c.to_string()),
    })
}

/// Valeur d'une expression ; `variable` donne celle d'un paramètre ou d'un champ. None
/// si une variable manque ou si un code n'est pas établi.
pub fn evaluer(e: &Expr, variable: &dyn Fn(&Expr) -> Option<Valeur>) -> Option<Valeur> {
    let n = |e: &Expr| evaluer(e, variable)?.nombre();
    let oui = |b: bool| Valeur::Nombre(if b { 1.0 } else { 0.0 });
    Some(match e {
        Expr::Nombre(x) => Valeur::Nombre(*x),
        Expr::Texte(s) => Valeur::Texte(s.clone()),
        Expr::Param(_) | Expr::Champ(_) => variable(e)?,
        Expr::Parenthese(x) => evaluer(x, variable)?,
        Expr::Binaire(op, a, b) => {
            let (a, b) = (n(a)?, n(b)?);
            match op {
                1 => Valeur::Nombre(a + b),
                2 => Valeur::Nombre(a - b),
                3 => Valeur::Nombre(a * b),
                4 if b != 0.0 => Valeur::Nombre(a / b),
                6 => oui((a - b).abs() <= 1e-9 * a.abs().max(b.abs()).max(1.0)),
                7 => oui(a > b),
                8 => oui(a < b),
                _ => return None,
            }
        }
        Expr::Fonction(f, args) => match (f, args.as_slice()) {
            (2, [a]) => Valeur::Nombre(n(a)?.cos()),
            // if : seule la branche retenue est évaluée
            (10, [c, a, b]) => evaluer(if n(c)? != 0.0 { a } else { b }, variable)?,
            (12, l) => oui(l.iter().map(|x| n(x).map(|v| v != 0.0)).collect::<Option<Vec<_>>>()?.into_iter().all(|v| v)),
            (13, [a]) => oui(n(a)? == 0.0),
            // round (14,67 → 15), roundup (2,11 → 3) ; tolérance pour un quotient
            // tombant juste (10,000…01 → 10)
            (18, [a]) => Valeur::Nombre((n(a)? + 1e-9).round()),
            (19, [a]) => Valeur::Nombre((n(a)? - 1e-9).ceil()),
            _ => return None,
        },
        Expr::Unaire(..) | Expr::Autre(_) => return None,
    })
}
