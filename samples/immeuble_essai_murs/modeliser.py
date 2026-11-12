"""Modélise un immeuble de logements mur par mur par le pilote et chronomètre chaque opération.

Par niveau : 2 façades, 2 pignons, 2 murs de circulation, 30 refends, 64 cloisons de salle
d'eau (100 murs), 32 portes, 32 fenêtres, 1 plancher.
Usage : python modeliser.py <niveaux> <chemin.modele.json> <resultats.json>
"""
import json
import os
import statistics
import sys
import time

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from pilote import commande, attendre_repos

NIVEAUX = int(sys.argv[1])
CHEMIN = sys.argv[2].replace("\\", "/")
RESULTATS = sys.argv[3]
BUDGET = 20 * 60  # secondes, tout compris
L, P, H = 48000, 14000, 2800
TRAVEE = 3000
debut_total = time.perf_counter()
mesures = []  # (niveau, genre, murs avant l'opération, secondes)
bilans = []


def ok(r, quoi):
    if not r.get("ok"):
        raise RuntimeError(f"{quoi} : {json.dumps(r, ensure_ascii=False)[:300]}")
    return r


def murs_du_niveau():
    murs = [("facade", 0, 0, L, 0), ("facade", 0, P, L, P), ("pignon", 0, 0, 0, P), ("pignon", L, 0, L, P),
            ("circulation", 0, 6000, L, 6000), ("circulation", 0, 8000, L, 8000)]
    for k in range(1, L // TRAVEE):
        x = k * TRAVEE
        murs += [("refend", x, 0, x, 6000), ("refend", x, 8000, x, P)]
    for k in range(L // TRAVEE):
        x = k * TRAVEE
        murs += [("cloison", x + 1500, 3500, x + 1500, 6000), ("cloison", x, 3500, x + 1500, 3500),
                 ("cloison", x + 1500, 8000, x + 1500, 10500), ("cloison", x, 10500, x + 1500, 10500)]
    return murs


if os.path.exists(CHEMIN):
    os.remove(CHEMIN)
ok(commande("nouveau", {"chemin": CHEMIN, "nom": "Immeuble d'essai - murs"})[0], "nouveau")
attendre_repos()
total_murs = 0
arret = None
FACADE = "Façade essai|Enduit:15:finition|Béton armé:200:structure|Laine minérale:120:isolation|Plaque de plâtre:13:finition"
VOILE = "Voile essai|Béton armé:200:structure"
CLOISON = "Cloison essai|Plaque de plâtre:13:finition|Laine minérale:48:isolation|Plaque de plâtre:13:finition"
TYPES = {"facade": FACADE, "pignon": FACADE, "circulation": VOILE, "refend": VOILE, "cloison": CLOISON}
type_courant = None
for n in range(NIVEAUX):
    nom = "RDC" if n == 0 else f"R+{n}"
    if n > 0:
        r, t = commande("editer", {"action": "niveau", "nom": nom, "altitude": n * 3000})
        ok(r, "niveau")
        mesures.append((n, "niveau", total_murs, t))
        r, t = commande("vue_plan", nom)
        ok(r, "vue_plan")
        mesures.append((n, "vue_plan", total_murs, t))
        attendre_repos()
    ids = {}
    for genre, x0, y0, x1, y1 in murs_du_niveau():
        type_voulu = TYPES[genre]
        if type_voulu != type_courant:
            ok(commande("type_mur", type_voulu)[0], "type_mur")
            type_courant = type_voulu
        r, t = commande("mur", f"{x0} {y0} {x1} {y1}")
        ok(r, "mur")
        ids.setdefault(genre, []).append(r["mur"])
        mesures.append((n, "mur", total_murs, t))
        total_murs += 1
        if time.perf_counter() - debut_total > BUDGET:
            arret = f"budget de {BUDGET // 60} min atteint pendant les murs du niveau {nom}"
            break
    if arret:
        break
    # portes sur les murs de circulation, fenêtres sur les façades
    for k in range(L // TRAVEE):
        for mur, genre, largeur, hauteur, allege, abscisse in [
            (ids["circulation"][0], "porte", 900, 2100, 0, k * TRAVEE + 1800),
            (ids["circulation"][1], "porte", 900, 2100, 0, k * TRAVEE + 1800),
            (ids["facade"][0], "fenetre", 1200, 1300, 900, k * TRAVEE + 900),
            (ids["facade"][1], "fenetre", 1200, 1300, 900, k * TRAVEE + 900),
        ]:
            r, t = commande("editer", {"action": "ouverture", "mur": mur, "genre": genre, "abscisse": abscisse, "largeur": largeur, "hauteur": hauteur, "allege": allege})
            ok(r, "ouverture")
            mesures.append((n, "ouverture", total_murs, t))
    r, t = commande("dalle", f"0 0 {L} 0 {L} {P} 0 {P}")
    ok(r, "dalle")
    mesures.append((n, "dalle", total_murs, t))
    # opérations courantes à ce stade
    cloison = ids["cloison"][0]
    r, t_deplacer = commande("editer", {"action": "deplacer", "ids": [cloison], "delta": [100, 0, 0]})
    ok(r, "deplacer")
    r, t_annuler = commande("annuler")
    ok(r, "annuler")
    attendre_repos()
    image = os.path.splitext(RESULTATS)[0] + f"_plan_{n}.png"
    r, t_capture = commande("capture", f"vue {image}")
    etat, t_etat = commande("etat")
    lot = [m[3] for m in mesures if m[0] == n and m[1] == "mur"]
    ouvertures = [m[3] for m in mesures if m[0] == n and m[1] == "ouverture"]
    bilan = {
        "niveau": nom, "murs": total_murs,
        "mur_median_ms": round(statistics.median(lot) * 1000), "mur_max_ms": round(max(lot) * 1000),
        "ouverture_median_ms": round(statistics.median(ouvertures) * 1000), "ouverture_max_ms": round(max(ouvertures) * 1000),
        "dalle_ms": round(mesures[-1][3] * 1000), "deplacer_ms": round(t_deplacer * 1000), "annuler_ms": round(t_annuler * 1000),
        "capture_plan_ms": round(t_capture * 1000), "etat_ms": round(t_etat * 1000), "statut": etat.get("statut"),
        "ecoule_s": round(time.perf_counter() - debut_total),
    }
    bilans.append(bilan)
    print(json.dumps(bilan, ensure_ascii=False), flush=True)
    json.dump({"bilans": bilans, "mesures": mesures, "arret": arret}, open(RESULTATS, "w", encoding="utf-8"), ensure_ascii=False)
    if time.perf_counter() - debut_total > BUDGET:
        arret = f"budget de {BUDGET // 60} min atteint après le niveau {nom}"
        break

fin = {"arret": arret, "murs": total_murs}
r, t = commande("enregistrer")
fin["enregistrer_ms"] = round(t * 1000)
fin["enregistre"] = bool(r.get("ok"))
attendre_repos()
fin["taille_ko"] = round(os.path.getsize(CHEMIN) / 1024) if os.path.exists(CHEMIN) else None
print(json.dumps(fin, ensure_ascii=False), flush=True)
json.dump({"bilans": bilans, "mesures": mesures, "fin": fin}, open(RESULTATS, "w", encoding="utf-8"), ensure_ascii=False)
