"""Vérifie la règle UniqueId -> IfcGUID sur toutes les références :
épisode = History[ElemTable.A] (liste finale, à l'envers), ExportId = épisode XOR id
sur ses 4 derniers octets, IfcGUID = compression IFC (22 caractères)."""
import json, os, struct, sys, uuid
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from champs import load
from objets import inflate, unpad

ALPHABET = "0123456789ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz_$"


def compresser(b):
    """16 octets (ordre uuid.bytes) -> IfcGUID : 2 bits puis 21 groupes de 6 bits."""
    n = int.from_bytes(b, 'big')
    return ''.join(ALPHABET[(n >> (6 * (21 - i))) & 63] for i in range(22))


def episodes(h):
    """GUID des épisodes, index 0 en dernier : fin = u32 n, n × (GUID, u8), u32 0."""
    for n in range(1, 100000):
        debut = len(h) - 4 - 17 * n - 4
        if debut < 0:
            return None
        if struct.unpack_from('<I', h, debut)[0] == n:
            liste = [h[debut + 4 + 17 * k: debut + 4 + 17 * k + 16] for k in range(n)]
            return [uuid.UUID(bytes_le=bytes(g)) for g in reversed(liste)]


def table(t):
    n = struct.unpack_from('<I', t, 2)[0]
    size = (len(t) - 6) // n
    out = {}
    for k in range(n):
        r = t[6 + size * k: 6 + size * (k + 1)]
        if size == 40:
            e, a = struct.unpack_from('<qI', r)
        else:
            e, a = struct.unpack_from('<iI', r)
        out[e] = a
    return out, size


ref_all = json.load(open(sys.argv[1], encoding='utf-8'))
for chemin in sys.argv[2:]:
    nom = chemin.replace('\\', '/').split('/')[-1]
    c, by, sch, parts = load(chemin)
    eps = episodes(inflate(unpad(c.read(by['/Global/History']))[8:]))
    tab, size = table(inflate(unpad(c.read(by['/Global/ElemTable']))[8:]))
    ok = faux = 0
    exemples = []
    for l in ref_all[nom]['vals'].strip('\n').split('\n'):
        x = l.split('\t')
        if x[1] not in ('-1019000', '-1019001') or not x[2]:
            continue
        e = int(x[0])
        if e not in tab:
            exemples.append(("absent", e)); faux += 1; continue
        g = bytearray(eps[tab[e]].bytes)
        g[12:16] = struct.pack('>I', struct.unpack('>I', g[12:16])[0] ^ (e & 0xFFFFFFFF))
        if compresser(bytes(g)) == x[2]:
            ok += 1
        else:
            faux += 1
            exemples.append((e, x[1], x[2], compresser(bytes(g))))
    print(f"{nom} : {len(eps)} épisodes, ElemTable {size} octets ; IfcGUID justes {ok}, faux {faux} {exemples[:5]}")
