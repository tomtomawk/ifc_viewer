"""Vues et éléments 2D d'un fichier Revit (lus dans les partitions, sans Revit).

Usage : python -I vues.py <fichier> [--vues] [--elements] [--json sortie.json]

Vues (classes dérivées de DBView) : nom DBView.m_viewName, échelle 1 / m_scale,
origine m_origin et repère m_horzDir (droite), m_vertDir (haut), m_viewDir (vers
l'observateur), niveau m_genElemId (plans), gabarit m_isTemplate ; cadrage d'une
coupe, d'une élévation ou d'une vue 3D dans son élément Viewer (m_viewerId) :
m_boundedSpace (origine, trois axes, bornes de 0 à m_boundOffset[i][0] sur chacun).
Classes : DBViewPlan (m_planViewType), DBViewSection (m_sectionViewType : coupe ou
élévation), DBView3d, DBViewAnonDrafting (vue de dessin), DBViewDrafting (feuille),
DBViewSchedule (nomenclature), DBViewLegend, DBViewProject (arborescence).

Éléments 2D (Element.m_ownerDBViewId ≠ −1) :
  CurveElem       ligne ou arc de détail : m_pCurveDriver → m_pCrv (GLine, GArc)
  TextNote        m_orig, m_baseVec, texte m_arrTextFragments[].m_text ou, à défaut,
                  le RTF de m_oSavedRtfData
  LinearDimString cote : ligne m_pDimLine, segments m_ArrSegInfo (valeur
                  m_values[0].m_value, milieu m_origin), points m_refPnts
  RadialDim       cote de rayon (mêmes champs)
  FilledRegion    région remplie : esquisse m_sketchId → m_absorbedCurves
  Viewport        fenêtre de vue d'une feuille : m_dbViewId, m_offset, m_scale
  RoomTag, FamilyInstance de vue (symboles, composants de détail) : comptés

Longueurs en mm (Revit stocke des pieds). Validé contre l'API de Revit 2026 (script
RiveTT rvt-reference-vues-2d) sur l'essai E13, la famille FEN_2PF.rfa et Projet1.rvt.
"""
import json, math, os, re, struct, sys
from collections import Counter

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from champs import FT, Fin, herite
from elements import Modele, eid


def mm(v):
    return [round(x * FT, 3) for x in v] if isinstance(v, list) else round(v * FT, 3)


def corps(p):
    return p['corps'] if isinstance(p, dict) and 'corps' in p else None


def courbe(c):
    """GLine ou GArc -> {'t', 'a', 'b', ('c', 'r')} en mm."""
    if c is None:
        return None
    t0, t1 = c['m_endParams']
    if c['_classe'] == 'GLine':
        o, d = c['m_origin'], c['m_dirVec']
        pt = lambda t: [o[i] + t * d[i] for i in range(3)]
        return {'t': 'Line', 'a': mm(pt(t0)), 'b': mm(pt(t1))}
    if c['_classe'] == 'GArc':
        ce, r, x, y = c['m_center'], c['m_radius'], c['m_xVec'], c['m_yVec']
        pt = lambda t: [ce[i] + r * (math.cos(t) * x[i] + math.sin(t) * y[i]) for i in range(3)]
        return {'t': 'Arc', 'a': mm(pt(t0)), 'b': mm(pt(t1)), 'c': mm(ce), 'r': mm(r)}
    return {'t': c['_classe']}


def rtf_texte(octets):
    """Texte visible d'un RTF simple (\\'xx en CP1252, \\par en saut de ligne)."""
    s = bytes(octets).rstrip(b'\0').decode('latin-1')
    s = re.sub(r"\\'([0-9a-f]{2})", lambda m: bytes([int(m.group(1), 16)]).decode('cp1252'), s)
    s = re.sub(r'\\par(?![a-z])', '\n', s)
    # groupes d'en-tête ({\fonttbl…}, {\colortbl…}, {\*…}) puis mots de contrôle
    while True:
        t = re.sub(r'\{\\(?:fonttbl|colortbl|\*)[^{}]*(\{[^{}]*\}[^{}]*)*\}', '', s)
        if t == s:
            break
        s = t
    s = re.sub(r'\\[a-z]+-?\d* ?', '', s).replace('{', '').replace('}', '')
    return s.strip().replace('\r', '')


def texte(r):
    frags = r.get('m_arrTextFragments') or []
    if frags:
        return ''.join(f['m_text'] for f in frags)
    rtf = corps(r.get('m_oSavedRtfData'))
    return rtf_texte(rtf['m_arrRtfStream']) if rtf else ''


def imbriquees(m):
    """Familles imbriquées dont le contenu est rangé en ligne dans le document
    (Element.m_famId = id de la famille) : leurs vues et éléments 2D (symboles
    d'annotation) ne sont pas ceux du document."""
    return {e for e, r in m.de('Family', True) if eid(r.get('m_famId', -1)) != -1}


def vues(m):
    out = []
    exclues = imbriquees(m)
    for e, r in m.de('DBView', True):
        if eid(r.get('m_famId', -1)) in exclues:
            continue
        v = {'id': e, 'cls': r['_classe'], 'nom': r.get('m_viewName'), 'gabarit': r.get('m_isTemplate'),
             'echelle': round(1 / r['m_scale']) if r.get('m_scale') else None,
             'origine': mm(r['m_origin']), 'droite': r['m_horzDir'], 'haut': r['m_vertDir'], 'dir': r['m_viewDir']}
        if r['_classe'] == 'DBViewPlan':
            v['niveau'] = eid(r['m_genElemId'])
            v['plan'] = r.get('m_planViewType')
        if r['_classe'] == 'DBViewSection':
            v['coupe'] = r.get('m_sectionViewType')
        out.append(v)
    # cadrage des vues qui ont un Viewer
    viewers = {e: r for e, r in m.de('Viewer', True)}
    par_vue = {}
    for e, r in viewers.items():
        b = r.get('m_boundedSpace')
        if b and eid(r.get('m_dbViewId', -1)) != -1:
            par_vue[eid(r['m_dbViewId'])] = {'origine': mm(b['m_orig']), 'axes': b['m_basis'],
                                             'bornes': [mm(o[0]) for o in b['m_boundOffset']], 'actif': b['m_isOn']}
    for v in out:
        if v['id'] in par_vue:
            v['cadrage'] = par_vue[v['id']]
    return out


def elements(m):
    out = []
    exclues = imbriquees(m)
    for e, r in m.de('Element', True):
        vue = eid(r.get('m_ownerDBViewId', -1))
        if vue in (-1, None) or eid(r.get('m_famId', -1)) in exclues:
            continue
        k = r['_classe']
        o = {'id': e, 'cls': k, 'vue': vue}
        if k == 'CurveElem':
            o['courbe'] = courbe(corps(corps(r.get('m_pCurveDriver')) and corps(r['m_pCurveDriver']).get('m_pCrv')))
        elif k == 'TextNote':
            o['texte'], o['pt'], o['base'] = texte(r), mm(r['m_orig']), r['m_baseVec']
        elif k in ('LinearDimString', 'RadialDim', 'AngularDim', 'ArcLengthDim'):
            ligne = corps(r.get('m_pDimLine'))
            if ligne and ligne['_classe'] == 'GLine':
                o['ligne'] = {'o': mm(ligne['m_origin']), 'd': ligne['m_dirVec']}
            o['segments'] = [[mm(s['m_values'][0]['m_value']) if s.get('m_values') and 'm_value' in s['m_values'][0] else None,
                              mm(s['m_origin'])] for s in r.get('m_ArrSegInfo') or []]
            o['points'] = [mm(p) for p in r.get('m_refPnts') or []]
        elif k == 'FilledRegion':
            o['esquisse'] = eid(r.get('m_sketchId', -1))
        elif k == 'Viewport':
            o['vueId'], o['decalage'], o['echelle'] = eid(r['m_dbViewId']), mm(r['m_offset']), r['m_scale']
        out.append(o)
    # contours des régions remplies : courbes absorbées de leur esquisse
    esquisses = {e: r for e, r in m.de('Sketch', True)}
    for o in out:
        if o['cls'] == 'FilledRegion' and o['esquisse'] in esquisses:
            cs = [courbe(corps(c)) for c in esquisses[o['esquisse']].get('m_absorbedCurves') or []]
            o['contour'] = [c['a'] for c in cs if c and 'a' in c]
    return out


def main():
    a = sys.argv[1:]
    m = Modele(a[0])
    res = {}
    if '--vues' in a or '--elements' not in a:
        res['vues'] = vues(m)
        for v in res['vues']:
            print('%10d %-20s %-40s 1:%-5s %s' % (v['id'], v['cls'], v['nom'], v['echelle'], 'gabarit' if v['gabarit'] else ''))
    if '--elements' in a or '--vues' not in a:
        res['elements'] = elements(m)
        print(Counter(o['cls'] for o in res['elements']).most_common())
        for o in res['elements']:
            if o['cls'] in ('TextNote', 'LinearDimString', 'RadialDim', 'Viewport', 'FilledRegion') or (o['cls'] == 'CurveElem' and '--detail' in a):
                print('  ', {k: v for k, v in o.items() if k not in ('points',)})
    if '--json' in a:
        json.dump(res, open(a[a.index('--json') + 1], 'w', encoding='utf-8'), ensure_ascii=False, indent=1)


if __name__ == '__main__':
    main()
