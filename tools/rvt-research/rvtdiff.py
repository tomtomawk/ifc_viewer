"""Compare deux fichiers Revit flux par flux et membre gzip par membre gzip.

Usage : python -I rvtdiff.py <A> <B> [--cherche valeur ...] [--contexte N]

Pour chaque flux : présent dans A seul, B seul, ou identique (octets bruts).
Sinon, les membres gzip décompressés sont appariés par contenu : membres
inchangés, retirés de A, ajoutés dans B ; pour les membres de même rang qui
diffèrent, premier octet différent et contexte hexadécimal.

--cherche : valeurs-sentinelles recherchées dans le contenu décompressé de B.
Un nombre est cherché en double (valeur brute, et millimètres convertis en
pieds, unité interne de Revit) ; un texte en UTF-16LE et en ASCII. Le résultat
indique si le membre qui le contient est OK ou KO.
"""
import hashlib, os, struct, sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from cfb import CFB
from extract import members, unpad


def load(path):
    c = CFB(path)
    out = {}
    for p, d in c.tree():
        if d['type'] == 2:
            raw = c.read(d)
            out[p] = (raw, members(unpad(raw)))
    return out


def motifs(v):
    try:
        x = float(v.replace(',', '.'))
        return [('double %s' % v, struct.pack('<d', x)),
                ('double %s mm -> pieds' % v, struct.pack('<d', x / 304.8))]
    except ValueError:
        return [('utf16 %r' % v, v.encode('utf-16-le')), ('ascii %r' % v, v.encode('latin-1', 'replace'))]


def hexa(b):
    return ' '.join('%02x' % x for x in b)


def diff_flux(p, a, b, ctx):
    (ra, ma), (rb, mb) = a, b
    if ra == rb:
        return None
    lines = ['~ %s  brut %d -> %d o' % (p, len(ra), len(rb))]
    if not ma and not mb:
        i = next((k for k in range(min(len(ra), len(rb))) if ra[k] != rb[k]), min(len(ra), len(rb)))
        lines.append('    sans gzip ; 1re différence @%d : %s | %s' % (i, hexa(ra[i:i + ctx]), hexa(rb[i:i + ctx])))
        return lines
    ha = [hashlib.sha1(m[1]).hexdigest() for m in ma]
    hb = [hashlib.sha1(m[1]).hexdigest() for m in mb]
    sa, sb = set(ha), set(hb)
    ko = lambda ms: sum(1 for m in ms if not m[2])
    lines.append('    membres %d -> %d (KO %d -> %d) ; inchangés %d, retirés %d, ajoutés %d'
                 % (len(ma), len(mb), ko(ma), ko(mb), len(sa & sb), len(sa - sb), len(sb - sa)))
    shown = 0
    for k in range(min(len(ma), len(mb))):
        da, db = ma[k][1], mb[k][1]
        if ha[k] == hb[k] or hb[k] in sa:
            continue
        i = next((j for j in range(min(len(da), len(db))) if da[j] != db[j]), min(len(da), len(db)))
        lines.append('    membre %d [%s/%s] %d -> %d o, 1re diff @%d' % (
            k, 'OK' if ma[k][2] else 'KO', 'OK' if mb[k][2] else 'KO', len(da), len(db), i))
        lines.append('      A: %s' % hexa(da[max(0, i - 8):i + ctx]))
        lines.append('      B: %s' % hexa(db[max(0, i - 8):i + ctx]))
        shown += 1
        if shown >= 10:
            lines.append('    … (limité à 10 membres)')
            break
    return lines


def main():
    args = sys.argv[1:]
    ctx = 32
    if '--contexte' in args:
        i = args.index('--contexte'); ctx = int(args[i + 1]); del args[i:i + 2]
    cherche = []
    if '--cherche' in args:
        i = args.index('--cherche'); cherche = args[i + 1:]; args = args[:i]
    A, B = load(args[0]), load(args[1])
    for p in sorted(set(A) - set(B)):
        print('- %s  (%d o)' % (p, len(A[p][0])))
    for p in sorted(set(B) - set(A)):
        print('+ %s  (%d o)' % (p, len(B[p][0])))
    same = 0
    for p in sorted(set(A) & set(B)):
        r = diff_flux(p, A[p], B[p], ctx)
        if r is None:
            same += 1
        else:
            print('\n'.join(r))
    print('= %d flux identiques' % same)
    for v in cherche:
        for label, pat in motifs(v):
            hits = []
            for p, (raw, ms) in B.items():
                for k, m in enumerate(ms):
                    j = m[1].find(pat)
                    while j >= 0 and len(hits) < 20:
                        hits.append('%s#%d@%d[%s]' % (p, k, j, 'OK' if m[2] else 'KO'))
                        j = m[1].find(pat, j + 1)
            print('? %-32s %s' % (label, ', '.join(hits) if hits else 'absent'))


if __name__ == '__main__':
    main()
