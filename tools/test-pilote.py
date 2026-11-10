"""Parcours du pilote sur une instance et un dossier temporaires isolés.

python tools/test-pilote.py --exe target/debug/bimedit.exe
"""
import argparse
import json
import os
from pathlib import Path
import socket
import subprocess
import tempfile
import time


def run(exe):
    with tempfile.TemporaryDirectory(prefix="bimedit-pilote-") as dossier:
        root = Path(dossier)
        with socket.socket() as s:
            s.bind(("127.0.0.1", 0))
            port = s.getsockname()[1]
        env = dict(os.environ, BIMEDIT_PORT=str(port), BIMEDIT_ARRIERE_PLAN=os.environ.get("BIMEDIT_ARRIERE_PLAN", "1"), BIMEDIT_CACHE_COLLABORATION=str(root / "cache"))
        # Ne lit ni n'écrit les préférences de l'utilisateur.
        env["APPDATA"] = str(root / "configuration")
        log = (root / "instance.log").open("w", encoding="utf-8")
        p = subprocess.Popen([str(exe.resolve())], env=env, stdout=log, stderr=log)
        second = None
        nombre = 0

        def commande(nom, argument=None, ok=True, port_pilote=None):
            nonlocal nombre
            if argument is not None and not isinstance(argument, str):
                argument = json.dumps(argument, ensure_ascii=False, separators=(",", ":"))
            ligne = nom + (" " + argument if argument is not None else "")
            with socket.create_connection(("127.0.0.1", port_pilote or port), timeout=90) as s:
                s.sendall((ligne + "\n").encode())
                resultat = json.loads(s.makefile("r", encoding="utf-8").readline())
            assert resultat.get("ok") == ok, (nom, resultat)
            nombre += 1
            return resultat

        def attendre(f, timeout=45):
            fin = time.monotonic() + timeout
            while time.monotonic() < fin:
                v = f()
                if v:
                    return v
                time.sleep(0.1)
            raise AssertionError("Délai dépassé")

        def charge():
            e = commande("etat")
            assert not e.get("erreur_chargement"), e
            return not e["chargement"]

        try:
            fin = time.monotonic() + 45
            while True:
                try:
                    commande("etat")
                    break
                except OSError:
                    if p.poll() is not None or time.monotonic() > fin:
                        raise AssertionError((root / "instance.log").read_text(encoding="utf-8"))
                    time.sleep(0.2)
            chemin = str(root / "Projet avec espaces.modele.json")
            commande("nouveau", {"chemin": chemin, "nom": "Essai pilote"})
            commande("nouveau", {"chemin": chemin}, ok=False)
            commande("mur", "0 0 4000 0 2800 200")
            a = commande("projet_natif")
            mur = a["projet"]["murs"][0]["id"]
            commande("selection_native", [mur])
            assert commande("etat")["selection_native"] == [mur]
            commande("editer", {"action": "deplacer", "ids": [mur], "delta": [0, 500, 0]})
            a = commande("projet_natif")
            patch = {"empreinte": a["empreinte"], "operations": [{"op": "replace", "path": "/nom", "value": "Renommé"}]}
            commande("modifier_projet", patch)
            commande("modifier_projet", patch, ok=False)
            commande("annuler")
            assert commande("projet_natif")["projet"]["nom"] == "Essai pilote"
            commande("retablir")
            assert commande("projet_natif")["projet"]["nom"] == "Renommé"
            for outil in ["mur", "poteau", "poutre", "dalle", "sol", "plafond", "toiture", "terrain", "limite_propriete", "escalier", "garde_corps", "ouverture", "texte", "cote", "etiquette", "ligne", "polyligne", "spline", "cercle", "arc", "zone", "coupe", "aucun"]:
                commande("outil", outil)
            commande("outil", "outil_inexistant", ok=False)
            commande("saisie", [{"action": "touche", "touche": "Escape"}])
            attendre(lambda: commande("etat")["saisies_en_attente"] == 0)
            # Deux clics dans la vue doivent réellement tracer un mur.
            commande("outil", "mur")
            x, y, w, h = commande("etat")["rectangle_vue"]
            assert w > 100 and h > 100
            avant = len(commande("projet_natif")["projet"]["murs"])
            commande("saisie", [{"action": "clic", "x": x + w * .35, "y": y + h * .6}, {"action": "clic", "x": x + w * .65, "y": y + h * .6}, {"action": "touche", "touche": "Escape"}])
            attendre(lambda: commande("etat")["saisies_en_attente"] == 0)
            assert len(commande("projet_natif")["projet"]["murs"]) > avant, "Le pilote n'a pas tracé le mur"
            commande("outil", "aucun")
            f=commande("famille_nouvelle","fenetre")["fichier"]
            f["nom"]="Fenêtre du pilote"
            commande("famille_modifier",f)
            commande("famille_fermer",ok=False)
            type_famille=commande("famille_charger_projet")["id"]
            commande("famille_enregistrer",str(root / "Famille avec espaces.json"))
            commande("famille_fermer")
            assert any(t[0]==type_famille for t in commande("familles")["familles"])
            commande("gabarit_exporter",str(root / "Gabarit agence.json"))
            commande("editer", {"action": "nomenclature", "nomenclature": {"id": "", "nom": "Quantités", "categorie": "", "regrouper": True}})
            nid = commande("projet_natif")["projet"]["nomenclatures"][0]["id"]
            assert commande("nomenclature", {"id": nid})["lignes"]
            commande("editer", {"action": "feuille", "feuille": {"id": "", "numero": "A01", "nom": "Essai", "jeu": "Plans", "largeur": 297, "hauteur": 210, "cartouche": "Essai", "vues": []}})
            fid=commande("projet_natif")["projet"]["feuilles"][0]["id"]
            for fmt in ["feuille_svg","feuille_pdf"]:
                sortie=root / (fmt + "." + fmt.split("_")[1])
                commande("export",{"format":fmt,"id":fid,"chemin":str(sortie)})
                assert sortie.stat().st_size>10
            for fmt, ext in [("svg", "svg"), ("pdf", "pdf"), ("ifc", "ifc"), ("csv", "csv"), ("jeu_pdf", "pdf")]:
                sortie = root / f"Sortie avec espaces {fmt}.{ext}"
                commande("export", {"format": fmt, "chemin": str(sortie), "id": nid, "jeu": "Plans"})
                assert sortie.stat().st_size > 10
            commande("enregistrer")
            commande("importer", {"source": str(root / "Sortie avec espaces ifc.ifc"), "sortie": str(root / "Import.modele.json")})
            attendre(charge)
            assert len(commande("documents")["documents"]) == 2
            commande("collaborer", str(root / "Projet partagé.bimpartage"))
            attendre(charge)
            c = commande("collaboration_etat")
            assert c["copie"]["base"]
            mid = commande("projet_natif")["projet"]["murs"][0]["id"]
            commande("collaboration_reserver", [mid])
            attendre(lambda: not commande("collaboration_etat")["synchronisation"])
            assert any(b["actif"] for b in commande("collaboration_etat")["copie"]["base"]["baux"].values())
            commande("collaboration_liberer", [mid])
            attendre(lambda: not commande("collaboration_etat")["synchronisation"])
            assert not any(b["actif"] for b in commande("collaboration_etat")["copie"]["base"]["baux"].values())
            commande("collaboration_instantane")
            attendre(lambda: not commande("collaboration_etat")["synchronisation"])
            # Deux vrais clients concurrents : archiver puis réappliquer une proposition.
            with socket.socket() as s:
                s.bind(("127.0.0.1",0))
                port2=s.getsockname()[1]
            env2=dict(env,BIMEDIT_PORT=str(port2),BIMEDIT_CACHE_COLLABORATION=str(root / "cache2"))
            second=subprocess.Popen([str(exe.resolve())],env=env2,stdout=log,stderr=log)
            def commande2(nom,argument=None,ok=True):
                return commande(nom,argument,ok,port_pilote=port2)
            def pret2():
                try:
                    return commande2("etat").get("ok")
                except OSError:
                    return False
            attendre(pret2)
            commande2("ouvrir",str(root / "Projet partagé.bimpartage"))
            # Première publication de deux séries de murs : rectangles proches,
            # bandes géométriques séparées. Toutes les éditions restent en file.
            avant=len(commande("projet_natif")["projet"]["murs"])
            for coords in ["96000 99500 100000 97500 2800 300","94000 95000 98000 95000 2800 300","102000 95000 106000 95000 2800 300"]:
                commande("mur",coords)
            murs_locaux=[]
            for coords in ["100000 105000 100000 100000 2800 300","100000 100000 104000 100000 2800 300","104000 100000 104000 105000 2800 300"]:
                murs_locaux.append(commande2("mur",coords)["mur"])
            for dx in [50,100]:
                commande2("editer",{"action":"deplacer","ids":[murs_locaux[-1]],"delta":[dx,0,0]})
            commande("collaboration_sync")
            attendre(lambda:not commande("collaboration_etat")["synchronisation"])
            commande2("collaboration_sync")
            attendre(lambda:not commande2("collaboration_etat")["synchronisation"])
            assert not commande2("collaboration_etat")["copie"]["conflits"]
            assert len(commande2("projet_natif")["projet"]["murs"])==avant+6
            commande("collaboration_sync")
            attendre(lambda:not commande("collaboration_etat")["synchronisation"])
            assert len(commande("projet_natif")["projet"]["murs"])==avant+6
            niveau=commande("projet_natif")["projet"]["niveaux"][0]
            for altitude,choix in [(111,"acceptee"),(333,"locale")]:
                commande("editer",{"action":"modifier_niveau","id":niveau["id"],"nom":niveau["nom"],"altitude":altitude})
                commande2("editer",{"action":"modifier_niveau","id":niveau["id"],"nom":niveau["nom"],"altitude":altitude+111})
                commande("collaboration_sync")
                attendre(lambda:not commande("collaboration_etat")["synchronisation"])
                commande2("collaboration_sync")
                attendre(lambda:not commande2("collaboration_etat")["synchronisation"])
                conflits=commande2("collaboration_etat")["copie"]["conflits"]
                assert conflits,"Conflit numérique attendu entre deux copies"
                assert commande2("projet_natif")["projet"]["niveaux"][0]["altitude"]==altitude+111,"La vue doit garder la proposition locale"
                for tid in conflits:
                    commande2("collaboration_resoudre",{"transaction":tid,"choix":choix})
                assert not commande2("collaboration_etat")["copie"]["conflits"]
                commande2("collaboration_sync")
                attendre(lambda:not commande2("collaboration_etat")["synchronisation"])
                commande("collaboration_sync")
                attendre(lambda:not commande("collaboration_etat")["synchronisation"])
                attendu=altitude if choix=="acceptee" else altitude+111
                assert commande("projet_natif")["projet"]["niveaux"][0]["altitude"]==attendu
            commande2("quitter","abandonner")
            second.wait(timeout=15)
            # Vérifier aussi le vrai protocole MCP, sur la même instance isolée.
            # Le serveur MCP est le même exécutable lancé avec --mcp.
            if True:
                requetes = [
                    {"jsonrpc":"2.0","id":1,"method":"tools/list"},
                    {"jsonrpc":"2.0","id":2,"method":"tools/call","params":{"name":"viewer_projet_natif","arguments":{}}},
                    {"jsonrpc":"2.0","id":3,"method":"tools/call","params":{"name":"viewer_collaboration","arguments":{"action":"etat"}}},
                    {"jsonrpc":"2.0","id":4,"method":"tools/call","params":{"name":"viewer_editer","arguments":{"transaction":{"action":"niveau","nom":"Étage MCP","altitude":10000}}}},
                    {"jsonrpc":"2.0","id":5,"method":"tools/call","params":{"name":"viewer_exporter","arguments":{"format":"ifc","chemin":str(root / "Export MCP.ifc")}}},
                ]
                r = subprocess.run([str(exe.resolve()), "--mcp"], input="\n".join(json.dumps(q) for q in requetes)+"\n",capture_output=True,text=True,encoding="utf-8",env=env,check=True,timeout=60)
                reponses = [json.loads(l) for l in r.stdout.splitlines()]
                assert len(reponses[0]["result"]["tools"]) == 28
                assert all(not r["result"]["isError"] for r in reponses[1:]), reponses
                nombre += len(reponses)
            commande("quitter", "abandonner")
            p.wait(timeout=15)
            print(f"Pilote : {nombre} requêtes validées, fichiers/édition/saisie/exports/import/collaboration OK")
        finally:
            if second is not None and second.poll() is None:
                second.terminate()
                second.wait(timeout=10)
            if p.poll() is None:
                p.terminate()
                p.wait(timeout=10)
            log.close()


if __name__ == "__main__":
    parser = argparse.ArgumentParser()
    parser.add_argument("--exe", type=Path, default=Path("target/debug/bimedit.exe"))
    run(parser.parse_args().exe)
