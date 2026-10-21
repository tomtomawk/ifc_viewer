"""Paramètres Revit : définitions et valeurs, lues par le schéma (champs.py).

Usage : python -I parametres.py <fichier> definitions
        python -I parametres.py <fichier> valeurs [ElementId ...]
        python -I parametres.py <fichier> --comparer <reference.json>   (référence RiveTT)

Définitions (docs/LISEZMOI-RVT.md § 7, « Paramètres ») : un paramètre utilisateur est un
élément dérivé de ParamElem (de famille, partagé, de projet, global, de table de clés) ;
son nom est ParamElem.m_pParamDef → ParamDef.m_caption, et la classe de la définition
(ParamDefValue et son m_specTypeId, ParamDefInt, ParamDefYesNo, ParamDefString…) donne le
stockage. Un paramètre intégré a un id négatif (BuiltInParameter de l'API) : ni nom ni
stockage dans le fichier, table relevée dans Revit (src/rvt/donnees/parametres_integres.tsv).

Valeurs : jeux typés Element.m_pParamValueSet{Double,Int,AString,ElementId}
(m_paramSet : m_paramId, m_value) ; listes NamedParam (m_paramId et toutes les cases :
m_value réel, m_int, m_str, m_elemId, formule m_oExpression), sans étiquette de type :
FamilyInstance.m_pInstParams (occurrence), FamilySymbol.m_pParams (type),
Family.m_familyParams (document de famille : type courant) ; ParamElemGlobal.m_value.
"""
import json, os, sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from champs import Fin, herite
from elements import Modele, eid, lire

ICI = os.path.dirname(os.path.abspath(__file__))
TABLE = os.path.join(ICI, '..', '..', 'src', 'rvt', 'donnees', 'parametres_integres.tsv')

GENRES = {'ParamElemFamily': 'famille', 'ParamElemExternal': 'partagé', 'ParamElemProject': 'projet',
          'ParamElemGlobal': 'global', 'ParamElemRefTableEntry': 'table de clés'}

# classe de définition -> stockage (Double, Integer, String, ElementId)
STOCKAGE_DEF = [('ParamDefValue', 'Double'), ('ParamDefLength', 'Double'), ('ParamDefAngle', 'Double'),
                ('ParamDefArea', 'Double'), ('ParamDefVolume', 'Double'), ('ParamDefDouble', 'Double'),
                ('ParamDefYesNo', 'Integer'), ('ParamDefInt', 'Integer'),
                ('ParamDefMaterialBrowse', 'ElementId'), ('ParamDefMaterial', 'ElementId'),
                ('ParamDefFamType', 'ElementId'), ('ParamDefElementId', 'ElementId'),
                ('ParamDefRefTableEntry', 'ElementId'), ('ParamDefString', 'String')]

JEUX = (('m_pParamValueSetDouble', 'Double'), ('m_pParamValueSetInt', 'Integer'),
        ('m_pParamValueSetAString', 'String'), ('m_pParamValueSetElementId', 'ElementId'))


def integres():
    """{id: (nom d'énumération, libellé, stockage)} des paramètres intégrés."""
    out = {}
    if os.path.exists(TABLE):
        for l in open(TABLE, encoding='utf-8'):
            c = l.rstrip('\n').split('\t')
            if len(c) >= 4 and not l.startswith('#'):
                out[int(c[0])] = (c[1], c[2], c[3])
    return out


def guid(g):
    """GUID enregistré (octets du GUID Windows) -> forme canonique."""
    b = bytes.fromhex(g)
    return '%s-%s-%s-%s-%s' % (b[3::-1].hex(), b[5:3:-1].hex(), b[7:5:-1].hex(), b[8:10].hex(), b[10:].hex())


def definitions(m):
    """{id: définition} des paramètres utilisateur du fichier."""
    out = {}
    for e, r in m.de('ParamElem', non_places=True):
        p = r['m_pParamDef']
        d = p['corps']
        if d is None:
            continue
        classe_def = m.sch.byid[p['classe']]['name']
        stockage = next((s for base, s in STOCKAGE_DEF if herite(m.sch, p['classe'], base)), 'String')
        spec = (d.get('m_specTypeId') or {}).get('m_typeId', '')
        if herite(m.sch, p['classe'], 'ParamDefYesNo'):
            spec = 'autodesk.spec:spec.bool-1.0.0'
        x = {'nom': d['m_caption'], 'genre': GENRES.get(m.sch.byid[m.cls[e]]['name'], m.sch.byid[m.cls[e]]['name']),
             'definition': classe_def, 'stockage': stockage, 'spec': spec,
             'groupe': d['m_groupTypeId']['m_typeId'], 'visible': d['m_userVisible']}
        if 'm_instanceParam' in r:
            x['occurrence'] = r['m_instanceParam']
            x['famille'] = eid(r['m_famId'])
        if 'm_externalParamKey' in r:
            x['guid'] = guid(r['m_externalParamKey']['m_guidValue']['m_guid'])
        if 'm_bindingIds' in r:
            x['liaisons'] = [eid(b) for b in r['m_bindingIds']]
        if 'm_value' in r and isinstance(r['m_value'], dict):    # global : valeur propre
            x['valeur'] = r['m_value']
        out[e] = x
    return out


def stockage(pid, defs, table):
    if pid in defs:
        return defs[pid]['stockage']
    return table.get(pid, ('', '', ''))[2]


def valeur_nommee(np, st):
    """Valeur d'un NamedParam selon le stockage de son paramètre ; formule : sa valeur."""
    if st == 'Double':
        return np['m_value']
    if st == 'Integer':
        return np['m_int']
    if st == 'ElementId':
        return eid(np['m_elemId'])
    if st == 'String':
        return np['m_str']
    # stockage inconnu : la case remplie
    for k in ('m_str', 'm_int', 'm_value'):
        if np.get(k):
            return np[k]
    return eid(np['m_elemId'])


def valeurs(r, defs, table):
    """[(id du paramètre, source, valeur)] d'un objet décodé."""
    out = []
    for champ, st in JEUX:
        ps = r.get(champ)
        if ps and ps['corps']:
            for x in ps['corps']['m_paramSet']:
                v = eid(x['m_value'])
                out.append((eid(x['m_paramId']), champ[2:], v))
    # jeu imbriqué des éléments de propriétés (actifs physiques et thermiques des matériaux)
    js = r.get('m_oParamSet')
    if js and js.get('corps'):
        for champ, st in (('m_pDoubleParams', 'Double'), ('m_pIntParams', 'Integer'),
                          ('m_pAStringParams', 'String'), ('m_pElementIdParams', 'ElementId')):
            ps = js['corps'].get(champ)
            if ps and ps['corps']:
                for x in ps['corps']['m_paramSet']:
                    out.append((eid(x['m_paramId']), 'oParamSet', eid(x['m_value'])))
    for champ in ('m_pInstParams', 'm_pParams', 'm_familyParams'):
        ps = r.get(champ)
        if ps and ps.get('corps'):
            for np in ps['corps']['m_params']:
                pid = eid(np['m_paramId'])
                out.append((pid, champ[2:], valeur_nommee(np, stockage(pid, defs, table))))
    return out


def tout(path):
    m = Modele(path)
    defs, table = definitions(m), integres()
    vals = {}
    for e in sorted(m.cur):
        try:
            r = lire(m.sch, m.cur[e])
        except Fin:
            continue
        v = valeurs(r, defs, table)
        if v:
            vals[e] = v
    return m, defs, table, vals


def comparer(path, ref_path):
    """Compare aux valeurs relevées dans Revit (rvt-reference-parametres-projet) : pour
    chaque paramètre modifiable porteur de valeur dans Revit, valeur décodée identique ?"""
    m, defs, table, vals = tout(path)
    nom = os.path.basename(path)
    ref = json.load(open(ref_path, encoding='utf-8'))
    ref = ref.get(nom) or next(iter(ref.values()))
    rdefs = {}
    for l in ref['defs'].strip('\n').split('\n'):
        c = l.split('\t')
        rdefs[int(c[0])] = c
    # noms et stockages des définitions utilisateur
    bons_noms = sum(1 for p, d in defs.items() if p in rdefs and rdefs[p][1] == d['nom'])
    faux_noms = [(p, d['nom'], rdefs[p][1]) for p, d in defs.items() if p in rdefs and rdefs[p][1] != d['nom']]
    faux_st = [(p, d['nom'], d['stockage'], rdefs[p][2]) for p, d in defs.items() if p in rdefs and rdefs[p][2] != d['stockage']]
    base = lambda s: s.rsplit('-', 1)[0]      # « autodesk.spec.aec:length-2.0.0 » sans version
    faux_spec = [(p, d['nom'], d['spec'], rdefs[p][3]) for p, d in defs.items()
                 if p in rdefs and d['stockage'] == 'Double' and rdefs[p][3] and base(d['spec']) != base(rdefs[p][3])]
    print(f"définitions utilisateur : {len(defs)}, vues par Revit {sum(1 for p in defs if p in rdefs)}, "
          f"noms identiques {bons_noms}, différents {len(faux_noms)} {faux_noms[:5]}")
    print(f"stockages différents : {len(faux_st)} {faux_st[:8]}")
    print(f"unités différentes : {len(faux_spec)} {faux_spec[:8]}")
    faux_int = [(p, rdefs[p][1], table[p][1], table[p][2], rdefs[p][2]) for p in rdefs if p < 0 and p in table
                and (table[p][2] != rdefs[p][2])]
    print(f"intégrés : {sum(1 for p in rdefs if p < 0)} vus, absents de la table {[p for p in rdefs if p < 0 and p not in table][:10]}, "
          f"stockage différent {len(faux_int)} {faux_int[:5]}")
    # valeurs
    lus = {}
    for e, vs in vals.items():
        for p, src, v in vs:
            lus.setdefault((e, p), (src, v))
    ok, faux, absents, sources, defaut = 0, [], [], {}, 0
    for l in ref['vals'].strip('\n').split('\n'):
        c = l.split('\t')
        e, p, brut = int(c[0]), int(c[1]), c[2]
        st = rdefs[p][2]
        if (e, p) not in lus:
            if brut in ('', '0', '-1'):
                defaut += 1         # rien d'enregistré : valeur par défaut
            else:
                absents.append((e, p, rdefs[p][1], brut))
            continue
        src, v = lus[(e, p)]
        if st == 'Double':
            bon = isinstance(v, (int, float)) and abs(float(brut) - v) <= 1e-9 * max(1, abs(v))
        elif st in ('Integer', 'ElementId'):
            bon = v == int(brut)
        else:
            bon = (v or '') == brut.replace('\\n', '\n').replace('\\t', '\t').replace('\\r', '\r').replace('\\\\', '\\')
        if bon:
            ok += 1
            sources[src] = sources.get(src, 0) + 1
        else:
            faux.append((e, p, rdefs[p][1], st, brut, v, src))
    print(f"valeurs Revit : {ok + len(faux) + len(absents) + defaut}, identiques {ok} {sources}, différentes {len(faux)}, "
          f"absentes à valeur par défaut {defaut}, non trouvées {len(absents)}")
    return faux, absents


def main():
    args = sys.argv[1:]
    if '--comparer' in args:
        faux, absents = comparer(args[0], args[args.index('--comparer') + 1])
        import collections
        print('différences par paramètre :', collections.Counter((f[1], f[2]) for f in faux).most_common(15))
        for f in faux[:15]:
            print('   ', f)
        print('non trouvées par paramètre :', collections.Counter((a[1], a[2]) for a in absents).most_common(25))
        return
    m, defs, table, vals = tout(args[0])
    if args[1] == 'definitions':
        for p, d in sorted(defs.items(), key=lambda x: (x[1]['genre'], x[1]['nom'])):
            print(p, json.dumps(d, ensure_ascii=False))
    else:
        ids = [int(x) for x in args[2:]] or list(vals)[:20]
        for e in ids:
            print(f'[{e}]')
            for p, src, v in vals.get(e, []):
                n = defs[p]['nom'] if p in defs else table.get(p, ('', '?'))[1]
                print(f'   {p:>10} {src:<22} {n:<40} {v!r}')


if __name__ == '__main__':
    main()
