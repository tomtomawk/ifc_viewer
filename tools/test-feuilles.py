"""Feuilles : outils du ruban Documenter sur le papier (polyligne, texte saisi en place,
cartouches), glisser d'une annotation et d'un cartouche, Suppr, annulation."""
import argparse
import json
import os
from pathlib import Path
import socket
import subprocess
import tempfile
import time


def run(exe):
    with tempfile.TemporaryDirectory(prefix="bimedit-feuilles-") as dossier:
        root = Path(dossier)
        with socket.socket() as s:
            s.bind(("127.0.0.1", 0))
            port = s.getsockname()[1]
        env = dict(os.environ, BIMEDIT_PORT=str(port), BIMEDIT_ARRIERE_PLAN=os.environ.get("BIMEDIT_ARRIERE_PLAN", "1"), APPDATA=str(root / "configuration"))
        with (root / "instance.log").open("w", encoding="utf-8") as log:
            processus = subprocess.Popen([str(exe.resolve())], env=env, stdout=log, stderr=log)

            def commande(nom, argument=None, ok=True):
                ligne = nom if argument is None else nom + " " + (argument if isinstance(argument, str) else json.dumps(argument, ensure_ascii=False))
                with socket.create_connection(("127.0.0.1", port), timeout=30) as connexion:
                    connexion.sendall((ligne + "\n").encode("utf-8"))
                    resultat = json.loads(connexion.makefile(encoding="utf-8").readline())
                assert resultat.get("ok") == ok, (nom, resultat)
                return resultat

            def saisie(evenements):
                commande("saisie", evenements)
                limite = time.monotonic() + 15
                while commande("etat")["saisies_en_attente"]:
                    if time.monotonic() > limite:
                        raise TimeoutError("Saisie graphique non terminée")
                    time.sleep(0.05)
                time.sleep(0.15)

            def ecran(x, y):
                return commande("ecran", f"{x} {y}")["ecran"]

            def clic(x, y):
                # survol d'abord, puis clic ; délai pour ne pas faire un double-clic
                q = ecran(x, y)
                saisie([{"action": "pointer", "x": q[0], "y": q[1]}])
                saisie([{"action": "clic", "x": q[0], "y": q[1]}])
                time.sleep(0.4)

            def glisser(depart, arrivee):
                a, b = ecran(*depart), ecran(*arrivee)
                saisie([{"action": "pointer", "x": a[0], "y": a[1]}])
                saisie([{"action": "bouton", "x": a[0], "y": a[1], "bouton": "gauche", "appuye": True}])
                for t in (0.1, 0.4, 0.7, 1.0):
                    saisie([{"action": "pointer", "x": a[0] + (b[0] - a[0]) * t, "y": a[1] + (b[1] - a[1]) * t}])
                saisie([{"action": "bouton", "x": b[0], "y": b[1], "bouton": "gauche", "appuye": False}])
                time.sleep(0.4)

            def feuille():
                return commande("projet_natif")["projet"]["feuilles"][0]

            try:
                limite = time.monotonic() + 45
                while True:
                    try:
                        commande("etat")
                        break
                    except OSError:
                        if processus.poll() is not None or time.monotonic() > limite:
                            raise RuntimeError("Instance de test indisponible")
                        time.sleep(0.2)
                commande("nouveau", {"chemin": str(root / "Projet.bem")})
                commande("editer", {"action": "feuille", "feuille": {"id": "", "numero": "A01", "nom": "Essai", "jeu": "Plans", "largeur": 420, "hauteur": 297, "cartouche": "", "vues": []}})
                fid = feuille()["id"]
                commande("selection_feuille", {"jeu": "Plans", "feuille": fid})
                time.sleep(0.5)
                # polyligne de deux sommets (outil du ruban Documenter), Entrée pour finir
                commande("outil", "polyligne")
                clic(60, 60)
                clic(160, 60)
                saisie([{"action": "touche", "touche": "Enter"}])
                a = feuille()["annotations"]
                assert len(a) == 1 and a[0]["forme"] == "polyligne", a
                assert all(abs(u - v) < 1.0 for u, v in zip(a[0]["points"][0] + a[0]["points"][1], [60, 60, 160, 60])), a
                # texte : clic, frappe en place, Entrée
                commande("outil", "texte")
                clic(60, 120)
                saisie([{"action": "texte", "texte": "Plan {numero}"}])
                saisie([{"action": "touche", "touche": "Enter"}])
                a = feuille()["annotations"]
                assert len(a) == 2 and a[1]["forme"] == "texte" and a[1]["texte"] == "Plan {numero}", a
                assert abs(a[1]["point"][0] - 60) < 1.0 and abs(a[1]["point"][1] - 120) < 1.0, a
                # Échap : retour à la sélection ; glisser la ligne de 30 mm vers le haut
                saisie([{"action": "touche", "touche": "Escape"}])
                glisser((110, 60), (110, 90))
                a = feuille()["annotations"]
                assert abs(a[0]["points"][0][1] - 90) < 1.5 and abs(a[0]["points"][1][1] - 90) < 1.5, a
                assert abs(a[0]["points"][0][0] - 60) < 1.5, a
                # cartouches : deux posés par l'outil (centrés sur le clic, 180 × 40 mm), l'outil
                # reste actif ; Échap, puis le second glissé de 30 mm vers le bas
                assert not feuille().get("cartouches")
                commande("outil", "cartouche")
                clic(300, 150)
                clic(120, 230)
                c = feuille()["cartouches"]
                assert len(c) == 2, c
                assert abs(c[0]["position"][0] - 210) < 1.5 and abs(c[0]["position"][1] - 130) < 1.5, c
                assert abs(c[1]["position"][0] - 30) < 1.5 and abs(c[1]["position"][1] - 210) < 1.5, c
                saisie([{"action": "touche", "touche": "Escape"}])
                glisser((120, 230), (120, 200))
                c = feuille()["cartouches"]
                assert abs(c[1]["position"][1] - 180) < 1.5 and abs(c[1]["position"][0] - 30) < 1.5, c
                # Suppr : le premier cartouche puis le texte ; Ctrl+Z remet le texte
                clic(300, 150)
                s = commande("proprietes_feuille")["selection"]
                assert s["type"] == "cartouche" and s["indice"] == 0, s
                saisie([{"action": "touche", "touche": "Delete"}])
                c = feuille()["cartouches"]
                assert len(c) == 1 and abs(c[0]["position"][1] - 180) < 1.5, c
                clic(62, 121)
                assert commande("proprietes_feuille")["selection"]["type"] == "annotation"
                saisie([{"action": "touche", "touche": "Delete"}])
                assert len(feuille()["annotations"]) == 1
                commande("annuler")
                assert len(feuille()["annotations"]) == 2
                print("Feuilles : OK")
            finally:
                processus.terminate()
                processus.wait(timeout=15)


if __name__ == "__main__":
    parser = argparse.ArgumentParser()
    parser.add_argument("--exe", type=Path, default=Path("target/debug/bimedit.exe"))
    run(parser.parse_args().exe)
