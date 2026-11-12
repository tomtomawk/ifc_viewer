"""Déplacements mixtes dans le projet et une famille, en instance isolée."""
import argparse
import json
import os
from pathlib import Path
import socket
import subprocess
import tempfile
import time


def run(exe):
    with tempfile.TemporaryDirectory(prefix="bimedit-mixte-") as dossier:
        root = Path(dossier)
        with socket.socket() as s:
            s.bind(("127.0.0.1", 0))
            port = s.getsockname()[1]
        env = dict(os.environ, BIMEDIT_PORT=str(port), BIMEDIT_ARRIERE_PLAN="1", APPDATA=str(root / "configuration"))
        with (root / "instance.log").open("w", encoding="utf-8") as log:
            processus = subprocess.Popen([str(exe.resolve())], env=env, stdout=log, stderr=log)

            def commande(nom, argument=None):
                ligne = nom if argument is None else nom + " " + (argument if isinstance(argument, str) else json.dumps(argument, ensure_ascii=False))
                with socket.create_connection(("127.0.0.1", port), timeout=30) as connexion:
                    connexion.sendall((ligne + "\n").encode("utf-8"))
                    resultat = json.loads(connexion.makefile(encoding="utf-8").readline())
                assert resultat.get("ok"), (nom, resultat)
                return resultat

            def saisie(evenements):
                commande("saisie", evenements)
                limite = time.monotonic() + 15
                while commande("etat")["saisies_en_attente"]:
                    if time.monotonic() > limite:
                        raise TimeoutError("Saisie graphique non terminée")
                    time.sleep(0.05)
                time.sleep(0.15)

            def glisser(a, b, annuler=False):
                evenements = [{"action": "bouton", "x": a[0], "y": a[1], "bouton": "gauche", "appuye": True},
                              {"action": "pointer", "x": b[0], "y": b[1]}]
                if annuler:
                    evenements.append({"action": "touche", "touche": "Escape"})
                evenements.append({"action": "bouton", "x": b[0], "y": b[1], "bouton": "gauche", "appuye": False})
                saisie(evenements)

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
                commande("nouveau", {"chemin": str(root / "Projet.modele.json")})
                mur = commande("mur", "-4000 -4000 4000 -4000 2800 200")["mur"]
                commande("camera", "cadrer")
                projet = commande("projet_natif")["projet"]
                commande("vue_plan", projet["niveaux"][0]["id"])
                projet = commande("projet_natif")["projet"]
                vue = [v["id"] for v in projet["vues"] if v["genre"] == "plan"][-1]
                commande("editer", {"action": "annoter", "annotation": {"id": "", "vue": vue, "hauteur": 2.5, "forme": "cercle", "centre": [0, 0], "rayon": 500}})
                avant = commande("projet_natif")["projet"]
                commande("selection_native", [avant["annotations"][-1]["id"], mur])
                a = commande("ecran", "0 -4100")["ecran"]
                b = commande("ecran", "500 -3600")["ecran"]
                glisser(a, b)
                apres = commande("projet_natif")["projet"]
                assert abs(apres["annotations"][-1]["centre"][0] - 500) < 5
                assert apres["murs"][0]["axe"] != avant["murs"][0]["axe"]
                commande("annuler")
                retour = commande("projet_natif")["projet"]
                assert retour["annotations"][-1]["centre"] == [0, 0] and retour["murs"][0]["axe"] == avant["murs"][0]["axe"]

                commande("famille_nouvelle", "composant")
                commande("famille_navigation", "plan")
                commande("famille_outil", "cercle")
                saisie([{"action": "pointer", "x": 700, "y": 450}, {"action": "texte", "texte": "@0;0"}, {"action": "touche", "touche": "Enter"},
                        {"action": "texte", "texte": "#10;0"}, {"action": "touche", "touche": "Enter"}])
                commande("famille_outil", "selection")
                saisie([{"action": "clic", "x": 1050, "y": 800}])
                glisser((450, 390), (1020, 700))
                avant = commande("famille_document")["fichier"]["famille"]
                glisser((740, 511), (760, 490), annuler=True)
                assert commande("famille_document")["fichier"]["famille"] == avant
                glisser((740, 511), (760, 490))
                apres = commande("famille_document")["fichier"]["famille"]
                assert apres["elements_2d"][0]["forme"]["centre"][0] > 20
                assert apres["volumes"][0]["min"][0]["decalage"] > avant["volumes"][0]["min"][0]["decalage"] + 20
                commande("annuler")
                assert commande("famille_document")["fichier"]["famille"] == avant
                print("Déplacements mixtes et annulation : OK")
            finally:
                processus.terminate()
                processus.wait(timeout=15)


if __name__ == "__main__":
    parser = argparse.ArgumentParser()
    parser.add_argument("--exe", type=Path, default=Path("target/debug/bimedit.exe"))
    run(parser.parse_args().exe)
