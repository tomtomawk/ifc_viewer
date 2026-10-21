"""Correspondance paramètre intégré -> champ de l'objet, apprise sur une référence Revit :
pour chaque (classe, paramètre), chemins dont la valeur égale celle de Revit sur tous
les éléments de la classe ; retenue si au moins deux valeurs distinctes (ou une valeur
non triviale) l'ont confirmée. Puis contrôle sur un second fichier.

Usage : python -I parametres_apprendre.py ref.json appris.json fichier1[+fichier2…] [fichier_controle…]
(ref.json : sortie du script RiveTT rvt-reference-parametres-projet ; appris.json -> src/rvt/donnees/parametres_champs.tsv)"""
import collections, json, os, re, sys
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from champs import Fin
from elements import Modele, lire, plat
import parametres

ref_all = json.load(open(sys.argv[1], encoding='utf-8'))
sortie = sys.argv[2]
fichiers = sys.argv[3:]
TRIVIAL = {'', '0', '-1', '1'}


def egal(v, brut, st):
    if st == 'Double':
        return isinstance(v, float) and abs(v - float(brut)) <= 1e-9 * max(1, abs(v))
    if st in ('Integer', 'ElementId'):
        return isinstance(v, int) and not isinstance(v, bool) and v == int(brut)
    return isinstance(v, str) and v == brut.replace('\\n', '\n').replace('\\t', '\t').replace('\\r', '\r').replace('\\\\', '\\')


def charger(chemin):
    nom = chemin.replace('\\', '/').split('/')[-1]
    ref = ref_all[nom]
    rdefs = {int(l.split('\t')[0]): l.split('\t') for l in ref['defs'].strip('\n').split('\n')}
    m = Modele(chemin)
    defs, table = parametres.definitions(m), parametres.integres()
    lus = set()
    obj = {}
    for e in m.cur:
        try:
            r = lire(m.sch, m.cur[e])
        except Fin:
            continue
        obj[e] = r
        for p, src, v in parametres.valeurs(r, defs, table):
            lus.add((e, p))
    restes = []   # (e, p, brut, st) non trouvés dans les jeux
    for l in ref['vals'].strip('\n').split('\n'):
        c = l.split('\t')
        e, p = int(c[0]), int(c[1])
        if p < 0 and (e, p) not in lus and e in obj:
            restes.append((e, p, c[2], rdefs[p][2]))
    return m, obj, restes, rdefs


def classe(m, e):
    return m.sch.byid[m.cls[e]]['name']


apprentissage = fichiers[0].split('+')
cand = {}            # (classe, p) -> set de chemins
valeurs = collections.defaultdict(set)
nb = collections.Counter()
total = 0
noms = {}
for f in apprentissage:
    m, obj, restes, rdefs = charger(f)
    noms.update({p: d[1] for p, d in rdefs.items()})
    total += len(restes)
    aplati = {}
    for e, p, brut, st in restes:
        if e not in aplati:
            aplati[e] = list(plat(obj[e]))
        c = classe(m, e)
        ch = {k for k, v in aplati[e] if egal(v, brut, st)}
        cle = (c, p)
        cand[cle] = ch if cle not in cand else cand[cle] & ch
        valeurs[cle].add(brut)
        nb[cle] += 1

appris = {}
ambigus = 0
for cle, ch in cand.items():
    if not ch:
        continue
    vs = valeurs[cle]
    if len(vs) < 2 and not (vs - TRIVIAL):
        continue                     # rien de discriminant
    # préférer le chemin le plus court (champ direct plutôt que copie dans une géométrie)
    ch = sorted(ch, key=lambda k: (k.count('>') + k.count('.'), len(k)))
    if len(ch) > 1:
        ambigus += 1
    appris[f"{cle[0]}|{cle[1]}"] = {'chemin': ch[0], 'autres': ch[1:4], 'n': nb[cle], 'valeurs': len(vs),
                                    'nom': noms[cle[1]]}
print(f"apprentissage {fichiers[0].split('/')[-1]} : {len(cand)} couples, appris {len(appris)}, ambigus {ambigus}")
couverts = sum(nb[(k.split('|')[0], int(k.split('|')[1]))] for k in appris)
print(f"valeurs couvertes : {couverts} / {total}")
json.dump(appris, open(sortie, 'w', encoding='utf-8'), ensure_ascii=False, indent=0)

for f in fichiers[1:]:
    m2, obj2, restes2, rdefs2 = charger(f)
    ok = faux = sans = 0
    erreurs = collections.Counter()
    for e, p, brut, st in restes2:
        k = f"{classe(m2, e)}|{p}"
        if k not in appris:
            sans += 1
            continue
        chemin = appris[k]['chemin']
        v = next((v for kk, v in plat(obj2[e]) if kk == chemin), None)
        if egal(v, brut, st):
            ok += 1
        else:
            faux += 1
            erreurs[(k, appris[k]['nom'], chemin)] += 1
    print(f"contrôle {f.split('/')[-1]} : {len(restes2)} valeurs hors jeux, chemin appris juste {ok}, faux {faux}, classe/paramètre non appris {sans}")
    for x, n in erreurs.most_common(12):
        print('   faux', n, x)

# bilan des couples non appris
sans_cand = collections.Counter()
triviaux = collections.Counter()
for cle, ch in cand.items():
    k = f"{cle[0]}|{cle[1]}"
    if k in appris:
        continue
    (triviaux if ch else sans_cand)[(cle[1], noms[cle[1]])] += nb[cle]
print("non appris, aucun champ égal :", sum(sans_cand.values()))
for x, n in sans_cand.most_common(30):
    print(f"   {n:>6} {x}")
print("non appris, valeurs non discriminantes :", sum(triviaux.values()))
for x, n in triviaux.most_common(12):
    print(f"   {n:>6} {x}")
