"""Essai de deux vraies instances graphiques via leurs pilotes locaux distincts.

Usage : python tools/test_collaboration_ui.py [target/release/bimedit.exe]
Les copies et journaux sont conservés dans un dossier temporaire dédié.
"""
import json
import os
from pathlib import Path
import shutil
import socket
import subprocess
import sys
import tempfile
import time

racine = Path(__file__).resolve().parent.parent
exe = Path(sys.argv[1]).resolve() if len(sys.argv) > 1 else racine / "target/release/bimedit.exe"
essai = Path(tempfile.mkdtemp(prefix="bimedit-ui-collaboration-"))
ports = [47991, 47992]
processus = []
journaux = []


def rpc(port, commande):
    with socket.create_connection(("127.0.0.1", port), timeout=5) as s:
        s.settimeout(30)
        s.sendall((commande + "\n").encode("utf-8"))
        with s.makefile("r", encoding="utf-8") as f:
            r = json.loads(f.readline())
    if not r.get("ok", True):
        raise RuntimeError(r)
    return r


def attendre(port, predicat):
    limite = time.monotonic() + 60
    while time.monotonic() < limite:
        try:
            r = rpc(port, "etat")
            if predicat(r):
                return r
        except (OSError, ValueError):
            pass
        time.sleep(0.05)
    raise RuntimeError(f"Délai dépassé : port {port} ; journaux : {essai}")


def synchroniser(port):
    rpc(port, "collaboration_sync")
    return attendre(port, lambda r: r.get("collaboration", {}).get("etat") == "Synchronisé")


def editer(port, commande):
    debut = time.perf_counter()
    r = rpc(port, "editer " + json.dumps(commande, ensure_ascii=False))
    return r, (time.perf_counter() - debut) * 1000


try:
    for n, port in enumerate(ports):
        # Refuse de parler à un pilote déjà présent.
        with socket.socket() as sonde:
            sonde.bind(("127.0.0.1", port))
        copie = essai / f"poste-{n}.json"
        shutil.copyfile(racine / "samples/maison_native.modele.json", copie)
        journal = open(essai / f"poste-{n}.log", "w", encoding="utf-8")
        journaux.append(journal)
        env = dict(os.environ, BIMEDIT_PORT=str(port), BIMEDIT_ARRIERE_PLAN=os.environ.get("BIMEDIT_ARRIERE_PLAN", "1"))
        processus.append(subprocess.Popen([str(exe), str(copie)], env=env, stdout=journal, stderr=journal, creationflags=0x08000000 if os.name == "nt" else 0))
        attendre(port, lambda r: not r.get("chargement", True) and "selection_native" in r)
        rpc(port, "collaboration_associer " + str(essai / "central"))
        attendre(port, lambda r: r.get("collaboration", {}).get("etat") == "Synchronisé")

    latences = []
    for n, port in enumerate(ports):
        _, ms = editer(port, {"action": "quadrillage", "nom": f"UI-{n}", "debut": [0, n*1000], "fin": [1000, n*1000]})
        latences.append({"latence_rpc_ms":ms,"mesures":rpc(port,"etat")["collaboration"]})
    synchroniser(ports[0])
    synchroniser(ports[1])
    synchroniser(ports[0])
    p = rpc(ports[0], "projet_natif")["projet"]
    vue = p["vues"][0]["id"]
    r, _ = editer(ports[0], {"action": "annoter", "annotation": {"id": "", "vue": vue, "hauteur": 2.5, "forme": "texte", "point": [0, 0], "texte": "Texte partagé"}})
    annotation_id = r["ids"][0]
    synchroniser(ports[0])
    synchroniser(ports[1])
    metriques = []
    for port, texte in zip(ports, ["Texte partagé A", "B Texte partagé"]):
        a = next(a for a in rpc(port, "projet_natif")["projet"]["annotations"] if a["id"] == annotation_id)
        a["texte"] = texte
        _, ms = editer(port, {"action": "annoter", "annotation": a})
        metriques.append({"port": port, "latence_rpc_texte_ms": ms, "apres_edition_texte": rpc(port, "etat")["collaboration"]})
        assert metriques[-1]["apres_edition_texte"]["preparation_transfert_gpu_ms"] == 0
    synchroniser(ports[0])
    synchroniser(ports[1])
    synchroniser(ports[0])
    contenus = []
    for port in ports:
        p = rpc(port, "projet_natif")["projet"]
        assert sum(q["nom"].startswith("UI-") for q in p["quadrillages"]) == 2
        contenus.append(next(a["texte"] for a in p["annotations"] if a["id"] == annotation_id))
    assert contenus[0] == contenus[1], contenus
    assert contenus[0].count("Texte partagé") == 1
    assert "A" in contenus[0] and "B" in contenus[0]
    rpc(ports[0],"annuler")
    synchroniser(ports[0]);synchroniser(ports[1]);synchroniser(ports[0])
    annule=next(a["texte"] for a in rpc(ports[1],"projet_natif")["projet"]["annotations"] if a["id"]==annotation_id)
    assert annule=="B Texte partagé",annule
    rpc(ports[0],"retablir")
    synchroniser(ports[0]);synchroniser(ports[1]);synchroniser(ports[0])
    retabli=next(a["texte"] for a in rpc(ports[1],"projet_natif")["projet"]["annotations"] if a["id"]==annotation_id)
    assert retabli==contenus[0],retabli
    rapport = {"essai": str(essai), "latences_rpc_geometrie_ms": latences, "texte_convergent": contenus[0], "texte_apres_annulation_locale":annule,"texte_retabli":retabli,"mesures": metriques}
    (essai / "rapport.json").write_text(json.dumps(rapport, indent=2, ensure_ascii=False), encoding="utf-8")
    print(json.dumps(rapport, indent=2, ensure_ascii=False))
finally:
    for p in processus:
        p.terminate()
        p.wait(timeout=15)
    for f in journaux:
        f.close()
