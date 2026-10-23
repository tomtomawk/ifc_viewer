"""Coordonnées d'un projet Revit : emplacements, points de base, lieu géographique.

Usage : python -I coordonnees.py <fichier> [--comparer reference.json]

Emplacement de projet (`ProjectLocation` de l'API) = élément `GeoLocation` (→ Instance) :
nom `m_name`, transformation `m_pInstanceInfo.m_Trf` (lignes `m_3x3`, origine `m_or`),
lieu `m_pInstanceInfo.m_symbolId` → `GeoSite` (→ Symbol : `m_dLatitude`, `m_dLongitude`
en radians, `m_dElevation`, `m_dTimeZone`, `m_placeName`, `m_weatherStationName`).
`ActiveGeoLocationTrackingElement` : emplacement actif `m_activeGeoLocationId`, et
`m_projectGeoLocationId`, emplacement caché dont l'origine est le point de base du
projet. `BasePoint` (→ CoordinateElemBase) : `m_locationType` 1 point topographique,
0 point de base du projet ; position dans le repère interne `m_oGPoint` → `m_coord`.
Longueurs en pieds, angles en radians, comme l'API.
"""
import json, math, os, struct, sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from champs import Fin, herite, objets_courants
from elements import eid, lire


class Emplacement:
    def __init__(s, e, r):
        s.id, s.nom = e, r['m_name']
        info = r['m_pInstanceInfo']['corps']
        t = info['m_Trf']
        s.lignes = [list(l) for l in t['m_3x3']]
        s.origine = list(t['m_or'])
        s.site = eid(info['m_symbolId'])

    def base(s):
        """Axes x, y, z de GetTotalTransform (partagé → interne) : colonnes des lignes
        enregistrées."""
        return [[s.lignes[i][j] for i in range(3)] for j in range(3)]

    def vers_partage(s, p):
        """Point du repère interne → coordonnées partagées : transposée de la base
        appliquée à p − origine."""
        d = [p[k] - s.origine[k] for k in range(3)]
        b = s.base()
        return [sum(b[j][k] * d[k] for k in range(3)) for j in range(3)]

    def angle(s):
        """Angle par rapport au nord géographique (ProjectPosition.Angle)."""
        x = s.base()[0]
        return -math.atan2(x[1], x[0])

    def position(s):
        """ProjectPosition de l'origine interne : [EastWest, NorthSouth, Elevation, Angle]."""
        return s.vers_partage([0.0, 0.0, 0.0]) + [s.angle()]


def coordonnees(chemin):
    sch, cur = objets_courants(chemin)
    cls = {e: sch.byid[struct.unpack_from('<H', b)[0]]['name'] for e, b in cur.items()}

    def tous(nom):
        for e in sorted(cur):
            if herite(sch, struct.unpack_from('<H', cur[e])[0], nom):
                try:
                    yield e, lire(sch, cur[e])
                except Fin:
                    pass

    out = {'emplacements': {}, 'sites': {}, 'points': {}, 'actif': None, 'projet': None}
    for e, r in tous('GeoLocation'):
        out['emplacements'][e] = Emplacement(e, r)
    for e, r in tous('GeoSite'):
        out['sites'][e] = {'latitude': r.get('m_dLatitude', 0.0), 'longitude': r.get('m_dLongitude', 0.0),
                           'altitude': r.get('m_dElevation', 0.0), 'fuseau': r.get('m_dTimeZone', 0.0),
                           'lieu': r.get('m_placeName', ''), 'station': r.get('m_weatherStationName', '')}
    for e, r in tous('ActiveGeoLocationTrackingElement'):
        out['actif'], out['projet'] = eid(r['m_activeGeoLocationId']), eid(r['m_projectGeoLocationId'])
    for e, r in tous('BasePoint'):
        genre = 'topo' if r.get('m_locationType') == 1 else 'base'
        out['points'][genre] = (e, list(r['m_oGPoint']['corps']['m_coord']))
    return out


def comparer(c, ref, tol=1e-6):
    ecarts = []

    def proche(a, b, quoi):
        if any(abs(x - y) > tol * max(1.0, abs(y)) for x, y in zip(a, b)):
            ecarts.append((quoi, a, b))

    actif = c['emplacements'][c['actif']]
    # emplacements de l'API : ceux du lieu de l'emplacement actif
    nommes = sorted(e.id for e in c['emplacements'].values() if e.site == actif.site and e.id != c['projet'])
    if nommes != sorted(er['id'] for er in ref['emplacements']):
        ecarts.append(('emplacements listés', nommes, [er['id'] for er in ref['emplacements']]))
    for er in ref['emplacements']:
        e = c['emplacements'].get(er['id'])
        if e is None:
            ecarts.append(('emplacement absent', er['id'], er['nom']))
            continue
        if e.nom != er['nom']:
            ecarts.append(('nom', e.nom, er['nom']))
        if (c['actif'] == e.id) != er['actif']:
            ecarts.append(('actif', e.id, er['actif']))
        proche(e.origine, er['origine'], f"origine {e.nom}")
        proche(e.base()[0], er['x'], f"axe x {e.nom}")
        proche(e.base()[1], er['y'], f"axe y {e.nom}")
        # angle : comparé modulo 2π
        p, pr = e.position(), er['position']
        proche(p[:3], pr[:3], f"position {e.nom}")
        if abs((p[3] - pr[3] + math.pi) % (2 * math.pi) - math.pi) > 1e-9:
            ecarts.append((f"angle {e.nom}", p[3], pr[3]))
        if e.site != er['site']:
            ecarts.append(('site', e.site, er['site']))
    for sid, sr in ref['sites'].items():
        s = c['sites'].get(int(sid))
        if s is None:
            ecarts.append(('site absent', sid))
            continue
        for k in ('latitude', 'longitude', 'altitude', 'fuseau'):
            proche([s[k]], [sr[k]], f"site {k}")
        for k in ('lieu', 'station'):
            if s[k] != sr[k]:
                ecarts.append((f"site {k}", s[k], sr[k]))
    tp, pb = c['points']['topo'], c['points']['base']
    proche(tp[1], ref['point_topo']['position'], 'point topographique')
    proche(actif.vers_partage(tp[1]), ref['point_topo']['partagee'], 'point topographique partagé')
    proche(pb[1], ref['point_base']['position'], 'point de base')
    proche(actif.vers_partage(pb[1]), ref['point_base']['partagee'], 'point de base partagé')
    angle_nord = (-actif.angle()) % (2 * math.pi)
    if abs((angle_nord - ref['point_base']['angle_nord'] + math.pi) % (2 * math.pi) - math.pi) > 1e-9:
        ecarts.append(('angle du point de base', angle_nord, ref['point_base']['angle_nord']))
    proche(actif.vers_partage([0, 0, 0]), ref['origine_interne']['partagee'], 'origine interne partagée')
    return ecarts


def main():
    a = sys.argv[1:]
    c = coordonnees(a[0])
    if '--comparer' in a:
        ref = json.load(open(a[a.index('--comparer') + 1], encoding='utf-8'))[os.path.basename(a[0])]
        e = comparer(c, ref)
        print(os.path.basename(a[0]), ':', 'identique à Revit' if not e else f'{len(e)} écarts')
        for x in e:
            print('   ', x)
        return
    for e in c['emplacements'].values():
        p = e.position()
        mark = ' (actif)' if e.id == c['actif'] else ' (point de base du projet)' if e.id == c['projet'] else ''
        print(f"{e.id} {e.nom}{mark} : E {p[0] * 0.3048:.3f} m, N {p[1] * 0.3048:.3f} m, Z {p[2] * 0.3048:.3f} m, "
              f"angle {math.degrees(p[3]):.3f}°, lieu {e.site}")
    for k, (e, p) in c['points'].items():
        print(f"point {k} {e} : interne {[round(x * 304.8, 1) for x in p]} mm")
    for e, s in c['sites'].items():
        print(f"lieu {e} : {math.degrees(s['latitude']):.6f}°, {math.degrees(s['longitude']):.6f}°, {s['lieu']!r}")


if __name__ == '__main__':
    main()
