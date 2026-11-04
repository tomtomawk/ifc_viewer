"""Régressions des fermetures, accrochages des murs et paramètres personnels.

python -X utf8 tools/test-ajustements-ui.py --exe target/release/bimedit.exe
"""
import argparse
import json
import os
from pathlib import Path
import socket
import subprocess
import tempfile
import time


def run(exe, captures):
    captures.mkdir(parents=True, exist_ok=True)
    captures=captures.resolve()
    with tempfile.TemporaryDirectory(prefix="bimedit-interface-") as dossier:
        root=Path(dossier)
        with socket.socket() as s:
            s.bind(("127.0.0.1",0));port=s.getsockname()[1]
        env=dict(os.environ,BIMEDIT_PORT=str(port),APPDATA=str(root/"configuration"),BIMEDIT_CACHE_COLLABORATION=str(root/"cache"))
        log=(root/"instance.log").open("w",encoding="utf-8")
        p=None
        nombre=0

        def commande(nom,argument=None,ok=True):
            nonlocal nombre
            if argument is not None and not isinstance(argument,str):
                argument=json.dumps(argument,ensure_ascii=False,separators=(",",":"))
            ligne=nom+(" "+argument if argument is not None else "")
            with socket.create_connection(("127.0.0.1",port),timeout=30) as s:
                s.sendall((ligne+"\n").encode());r=json.loads(s.makefile("r",encoding="utf-8").readline())
            assert r.get("ok")==ok,(nom,r)
            nombre+=1
            return r

        def attendre(f,timeout=15):
            fin=time.monotonic()+timeout
            while time.monotonic()<fin:
                r=f()
                if r:return r
                time.sleep(.05)
            raise AssertionError("Délai dépassé")

        def demarrer():
            nonlocal p
            p=subprocess.Popen([str(exe.resolve())],env=env,stdout=log,stderr=log)
            def pret():
                try:return commande("etat")["ok"]
                except OSError:return False
            attendre(pret,45)

        def saisie(entrees):
            commande("saisie",entrees)
            attendre(lambda:commande("etat")["saisies_en_attente"]==0)

        def capture(nom,zone="fenetre"):
            commande("capture",f"{zone} {captures/nom}")

        try:
            demarrer()
            assert commande("options")["options"]["delai_accrochage"]==.8
            chemin=str(root/"Projet 1.modele.json")
            commande("nouveau",{"chemin":chemin,"nom":"Premier projet"})
            id1=commande("mur","0 0 3000 0 2800 300")["mur"]
            id2=commande("mur","3000 2000 6000 2000 2800 300")["mur"]
            commande("camera","cadrer")
            commande("selection_native",[id2]);cible=commande("axe_selection")
            commande("selection_native",[id1]);source=commande("axe_selection")
            commande("modifier_trace",id1)
            x,y=source["fin_ecran"];tx,ty=cible["debut_ecran"]
            # Légèrement à côté du point pour prouver que l'accrochage retient l'extrémité.
            commande("saisie",[{"action":"bouton","x":x,"y":y,"bouton":"gauche","appuye":True},{"action":"pointer","x":tx+3,"y":ty+2}])
            attendre(lambda:commande("etat")["saisies_en_attente"]==0)
            accroche=commande("axe_selection")
            assert accroche["deplacement"] is not None,"La poignée du mur doit être glissée"
            assert accroche["accrochage"]["reference"] is None
            time.sleep(.25)
            assert commande("axe_selection")["accrochage"]["reference"] is None
            attendre(lambda:commande("axe_selection")["accrochage"]["reference"] is not None,2)
            capture("mur-accrochage.png")
            saisie([{"action":"bouton","x":tx+3,"y":ty+2,"bouton":"gauche","appuye":False}])
            fin=commande("axe_selection")["fin"]
            assert sum((a-b)**2 for a,b in zip(fin,cible["debut"]))<1.0,("Extrémité non accrochée",fin,cible["debut"])
            commande("annuler")
            origine=commande("axe_selection")["fin"]
            assert sum((a-b)**2 for a,b in zip(origine,source["fin"]))<1.0
            commande("retablir")
            commande("options",{"raccourcis":{"mur":"MW"}})
            commande("options",{"raccourcis":{"mur":"CO"}},ok=False)
            assert commande("options")["options"]["raccourcis"]["mur"]=="MW"
            commande("outil","aucun")
            saisie([{"action":"texte","texte":"MU"}])
            assert commande("etat")["outil"] is None
            saisie([{"action":"texte","texte":"MW"}])
            assert commande("etat")["outil"]=="Mur"
            saisie([{"action":"touche","touche":"Escape"}])
            for categorie in ["general","dessin","raccourcis","graphisme","gabarit"]:
                assert commande("parametres",categorie)["categorie"]==categorie
                # Laisser une image au nouveau contenu avant la capture de fenêtre.
                time.sleep(.3);capture(f"parametres-{categorie}.png")
            commande("parametres","fermer")
            commande("enregistrer")
            # Le dernier onglet ferme le projet, puis restaure l'autre projet.
            commande("nouveau",{"chemin":str(root/"Projet 2.modele.json"),"nom":"Deuxième projet"})
            assert len(commande("documents")["documents"])==2
            commande("fermer_vue")
            assert len(commande("documents")["documents"])==1
            assert commande("etat")["sortie_v0"]==chemin
            # Une fermeture avec modifications ne doit pas perdre le dernier onglet.
            commande("mur","10000 0 14000 0 2800 200")
            commande("fermer_vue",ok=False)
            assert len(commande("documents")["documents"])==1
            commande("enregistrer")
            capture("projet-avant-fermeture.png","vue")
            commande("fermer_vue")
            assert commande("documents")["documents"]==[]
            assert commande("etat")["fichier"] is None
            capture("aucun-projet.png")
            capture("vue-videe.png","vue")
            commande("nouveau",{"chemin":str(root/"Projet 3.modele.json")})
            commande("mur","0 0 2000 0 2800 200")
            commande("fermer","abandonner")
            assert commande("documents")["documents"]==[]
            assert commande("etat")["fichier"] is None
            commande("quitter");p.wait(timeout=15)
            # Les préférences survivent à une nouvelle instance.
            demarrer()
            assert commande("options")["options"]["raccourcis"]["mur"]=="MW"
            assert commande("options")["options"]["delai_accrochage"]==.8
            commande("quitter");p.wait(timeout=15)
            print(f"Interface : {nombre} requêtes validées ; fermetures, accrochages et raccourcis persistants OK")
        except Exception:
            log.flush()
            print((root/"instance.log").read_text(encoding="utf-8")[-2500:])
            raise
        finally:
            if p is not None and p.poll() is None:p.terminate();p.wait(timeout=10)
            log.close()


if __name__=="__main__":
    parser=argparse.ArgumentParser()
    parser.add_argument("--exe",type=Path,default=Path("target/release/bimedit.exe"))
    parser.add_argument("--captures",type=Path,default=Path("exports/verification-interface"))
    args=parser.parse_args();run(args.exe,args.captures)
