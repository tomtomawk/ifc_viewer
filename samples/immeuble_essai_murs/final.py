"""Mesures finales sur le projet modélisé : 3D, sélection, réouverture."""
import json
import os
import statistics
import sys
import time

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from pilote import commande, attendre_repos

ici = os.path.dirname(os.path.abspath(__file__)).replace("\\", "/")
CHEMIN = sys.argv[1].replace("\\", "/")
r = {}
v, _ = commande("vues")
print("vues", json.dumps(v, ensure_ascii=False)[:600])
x, t = commande("vue", "0")
r["ouvrir_vue_3d_ms"] = round((t + attendre_repos()) * 1000)
commande("camera", "iso")
commande("camera", "cadrer")
attendre_repos()
x, t = commande("capture", f"vue {ici}/immeuble_3d.png")
r["capture_3d_ms"] = round(t * 1000)
durees = []
for k in range(10):
    x, t1 = commande("glisser", "40 5 gauche")
    x, t2 = commande("capture", f"vue {ici}/orbite.png")
    durees.append(t1 + t2)
r["orbite_image_median_ms"] = round(statistics.median(durees) * 1000)
r["orbite_image_max_ms"] = round(max(durees) * 1000)
e, _ = commande("etat")
r["statut"] = e.get("statut")
r["objets"] = e.get("objets")
r["triangles"] = e.get("triangles")
p, t = commande("projet_natif")
r["projet_natif_ms"] = round(t * 1000)
d = p["projet"]
r["murs_couches"] = len(d["murs"])
r["assemblages"] = len(d.get("assemblages", []))
r["ouvertures"] = len(d["ouvertures"])
r["dalles"] = len(d["dalles"])
mur = d["assemblages"][0]["id"]
x, t = commande("selection_native", [mur])
r["selection_ms"] = round(t * 1000)
x, t = commande("editer", {"action": "deplacer", "ids": [mur], "delta": [0, -200, 0]})
r["deplacer_facade_ms"] = round(t * 1000)
r["deplacer_ok"] = bool(x.get("ok"))
x, t = commande("annuler")
r["annuler_ms"] = round(t * 1000)
x, t = commande("enregistrer")
r["enregistrer_ms"] = round(t * 1000)
attendre_repos()
x, t = commande("fermer", "abandonner")
r["fermer_ms"] = round(t * 1000)
debut = time.perf_counter()
x, t = commande("ouvrir", CHEMIN)
r["ouvrir_ok"] = bool(x.get("ok"))
attendre_repos()
r["ouvrir_total_ms"] = round((time.perf_counter() - debut) * 1000)
e, _ = commande("etat")
r["statut_apres_ouverture"] = e.get("statut")
print(json.dumps(r, ensure_ascii=False, indent=1))
json.dump(r, open(ici + "/final.json", "w", encoding="utf-8"), ensure_ascii=False)
