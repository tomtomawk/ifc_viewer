"""Extraction d'éléments Revit courants à partir des objets décodés par champs.py.

Usage : python -I elements.py <fichier> niveaux|types-murs|sols|instances|pieces|surfaces
        python -I elements.py <fichier> --plat <ElementId>   (objet aplati : chemin = valeur)

Longueurs en mm (Revit stocke des pieds), angles en degrés.
"""
import math, os, struct, sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from champs import FT, Fin, Lecteur, _id, herite, objets_courants, record


def eid(v):
    """ElementId décodé -> entier."""
    return _id(v) if isinstance(v, dict) and v.get('_classe') == 'ElementId' else v


def plat(v, chemin=''):
    """(chemin, valeur) pour chaque valeur simple d'un objet décodé ; les ElementId sont
    réduits à leur entier et les pointeurs remplacés par leur corps."""
    if isinstance(v, dict):
        if v.get('_classe') == 'ElementId':
            yield chemin, eid(v)
            return
        if 'corps' in v and 'classe' in v:          # pointeur possédé
            if v['corps'] is not None:
                yield from plat(v['corps'], chemin + '>')
            return
        for k, x in v.items():
            if k != '_classe':
                yield from plat(x, chemin + '.' + k if chemin else k)
    elif isinstance(v, list):
        if v and all(isinstance(x, (int, float)) for x in v):
            yield chemin, v
        else:
            for i, x in enumerate(v):
                yield from plat(x, '%s[%d]' % (chemin, i))
    else:
        yield chemin, v


def lire(sch, body):
    return Lecteur(sch, body).element()


def params(root):
    """{id de paramètre: valeur} des jeux de paramètres double, int, chaîne, ElementId."""
    out = {}
    for k in ('m_pParamValueSetDouble', 'm_pParamValueSetInt', 'm_pParamValueSetAString', 'm_pParamValueSetElementId'):
        ps = root.get(k)
        if ps and ps['corps']:
            for x in ps['corps']['m_paramSet']:
                out[eid(x['m_paramId'])] = eid(x['m_value'])
    return out


ALL_MODEL_MARK = -1001203
FLOOR_HEIGHTABOVELEVEL_PARAM = -1001951
ROOM_NAME, ROOM_NUMBER, ROOM_DEPARTMENT = -1006900, -1006901, -1006907


class Modele:
    """Objets courants d'un fichier (une seule lecture), filtrables par classe de base.
    Comme les collecteurs de Revit, de() ignore les éléments d'un groupe non placé
    (m_unplacedOwnerId) ; il saute aussi les objets illisibles (données d'add-ins)."""

    def __init__(s, path):
        s.sch, s.cur = objets_courants(path)
        s.cls = {e: struct.unpack_from('<H', b)[0] for e, b in s.cur.items()}
        s.illisibles = []

    def de(s, base, non_places=False):
        for e in sorted(s.cur):
            if herite(s.sch, s.cls[e], base):
                try:
                    r = lire(s.sch, s.cur[e])
                except Fin:
                    s.illisibles.append(e); continue
                if non_places or eid(r.get('m_unplacedOwnerId', -1)) == -1:
                    yield e, r


def corps(p):
    return p['corps'] if p else None


def niveaux(m):
    """Nom et altitude (Z de l'origine du plan du niveau)."""
    for e, r in m.de('Level'):
        yield {'id': e, 'nom': r['m_text'], 'altitude': corps(r['m_pSurface'])['m_origin'][2] * FT}


def types_murs(m):
    """Nom et épaisseur (somme des couches de la structure composée ; absente pour les
    murs-rideaux et les murs empilés)."""
    for e, r in m.de('WallType'):
        cs = corps(r.get('m_pCompoundStructure'))
        ep = sum(c['m_layerWidth'] for c in cs['m_layers']) * FT if cs else None
        yield {'id': e, 'nom': corps(r['m_symbolInfo'])['m_name'], 'epaisseur': ep,
               'couches': [round(c['m_layerWidth'] * FT, 1) for c in cs['m_layers']] if cs else []}


def sols(m):
    for e, r in m.de('Floor'):
        p = params(r)
        yield {'id': e, 'type': eid(r['m_floorAttributesId']), 'niveau': eid(r['m_assocLevelId']),
               'decalage': p.get(FLOOR_HEIGHTABOVELEVEL_PARAM, 0) * FT,
               'dessous': r['m_bottom'] * FT, 'dessus': r['m_top'] * FT}


def repere_porte(r):
    """Les portes rangent leur repère dans FamInstSpec.m_doorNumber, pas en paramètre."""
    spec = corps(r.get('m_oFamInstSpec'))
    return spec.get('m_doorNumber', '') if spec else ''


def courbe(crv):
    """Fonction t -> point (pieds) d'une GLine ou d'un GArc, t dans l'espace de
    paramètres de la courbe (celui de m_endParams)."""
    if crv['_classe'] == 'GLine':
        return lambda t: [o + t * d for o, d in zip(crv['m_origin'], crv['m_dirVec'])]
    if crv['_classe'] == 'GArc':
        c, rad = crv['m_center'], crv['m_radius']
        return lambda t: [ci + rad * (math.cos(t) * x + math.sin(t) * y) for ci, x, y in zip(c, crv['m_xVec'], crv['m_yVec'])]
    return None


def lignes_murs(m):
    """{id de mur: fonction t -> point} pour les murs droits et courbes."""
    out = {}
    for e, r in m.de('VWall', non_places=True):
        drv = corps(r.get('m_pCurveDriver'))
        crv = corps(drv.get('m_pCrv')) if drv else None
        f = courbe(crv) if crv else None
        if f:
            out[e] = f
    return out


def instances(m):
    """Type (m_masterSymbolId : le Symbol de l'API), hôte, niveau, point d'insertion,
    direction de référence et repère (paramètre ALL_MODEL_MARK, ou numéro de porte).

    Point (comme LocationPoint dans l'API) : pour une instance hébergée par un mur, le
    point de la ligne du mur au paramètre m_hostParam, à l'altitude du niveau plus
    m_elevation (allège), car m_instOrigin y vaut 0 dans les fichiers 2025 ; pour les
    autres instances de modèle, m_instOrigin ; pour un élément de vue, l'origine de la
    transformation d'instance (m_Trf.m_or), sans garantie.
    'vue' : vue propriétaire des éléments 2D (composants de détail, symboles), -1 pour
    un élément de modèle ; la position des éléments 2D n'est pas garantie."""
    alt = {n['id']: n['altitude'] for n in niveaux(m)}
    murs = lignes_murs(m)
    for e, r in m.de('FamilyInstance'):
        p = params(r)
        hote, niv = eid(r['m_hostId']), eid(r['m_assocLevelId'])
        if hote in murs and r.get('m_hostParam') is not None:
            x, y, _ = murs[hote](r['m_hostParam'])
            pt = [x * FT, y * FT, alt.get(niv, 0) + r.get('m_elevation', 0) * FT]
        elif eid(r['m_ownerDBViewId']) == -1:
            pt = [v * FT for v in r['m_instOrigin']]
        else:
            pt = [v * FT for v in corps(r['m_pInstanceInfo'])['m_Trf']['m_or']]
        yield {'id': e, 'type': eid(r['m_masterSymbolId']), 'hote': hote, 'niveau': niv, 'point': pt,
               'direction': r['m_RefDir'], 'repere': p.get(ALL_MODEL_MARK) or repere_porte(r),
               'vue': eid(r['m_ownerDBViewId'])}


def _espace(r):
    """RoomElem porte aussi les surfaces (m_areaSchemeId) et les zones (m_zoneSchemeId)."""
    if eid(r.get('m_areaSchemeId', -1)) != -1:
        return 'surface'
    if eid(r.get('m_zoneSchemeId', -1)) != -1:
        return 'zone'
    return 'piece'


def pieces(m, genre='piece'):
    """Pièces (genre 'piece'), surfaces ('surface') ou zones ('zone') : nom, numéro,
    service, niveau et point de placement (None si la pièce n'est pas placée)."""
    for e, r in m.de('RoomElem'):
        if _espace(r) != genre:
            continue
        p = params(r)
        yield {'id': e, 'nom': p.get(ROOM_NAME, ''), 'numero': p.get(ROOM_NUMBER, ''),
               'service': p.get(ROOM_DEPARTMENT, ''), 'niveau': eid(r['m_assocLevelId']),
               'point': None if r.get('m_bIsLocationless') else [x * FT for x in r['m_point']]}


def surfaces(m):
    for x in pieces(m, 'surface'):
        yield x


EXTRACTEURS = {'niveaux': niveaux, 'types-murs': types_murs, 'sols': sols, 'instances': instances, 'pieces': pieces,
               'surfaces': surfaces}


def main():
    args = sys.argv[1:]
    if '--plat' in args:
        sch, b = record(args[0], int(args[args.index('--plat') + 1]))
        for k, v in plat(lire(sch, b)):
            if v not in (None, -1, 0, 0.0, '', []):
                print('%-70s %r' % (k, v))
        return
    m = Modele(args[0])
    for nom in args[1:] or list(EXTRACTEURS):
        print('== %s' % nom)
        for x in EXTRACTEURS[nom](m):
            print('  ' + '  '.join('%s=%s' % (k, ('%.1f' % v) if isinstance(v, float) else
                                    ('(%s)' % ' ; '.join('%.1f' % y for y in v) if isinstance(v, list) and v and isinstance(v[0], float) else v))
                                   for k, v in x.items()))


if __name__ == '__main__':
    main()
