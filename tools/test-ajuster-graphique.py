"""Régression du parcours graphique Ajuster / prolonger dans une instance isolée."""
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
            commande("nouveau",{"chemin":str(root/"Ajuster.modele.json")})
            ref=commande("mur","4000 -3000 4000 6000 2800 300")["mur"]
            ids=[commande("mur",v)["mur"] for v in ["0 0 2000 0 2800 300","0 2000 6000 2000 2800 300","6000 4000 8000 4000 2800 300"]]
            commande("camera","cadrer")
            def point(id,t):
                commande("selection_native",[id]);a=commande("axe_selection")
                return [a["debut_ecran"][i]*(1-t)+a["fin_ecran"][i]*t for i in range(2)]
            rp=point(ref,.12)
            pts=[point(ids[0],.85),point(ids[1],.85),point(ids[2],.15)]
            commande("outil","ajuster")
            def clic(p):saisie([{"action":"clic","x":p[0],"y":p[1]}])
            clic(rp)
            assert commande("ajuster_etat")["reference"]==ref
            for id,pt,fin in zip(ids,pts,[True,True,False]):
                avant=commande("projet_natif")["empreinte"]
                saisie([{"action":"pointer","x":pt[0],"y":pt[1]}])
                a=commande("ajuster_etat")
                assert a["apercu"] and a["apercu"]["id"]==id and a["apercu"]["fin"]==fin,a
                assert commande("projet_natif")["empreinte"]==avant,"Le survol a modifié le projet"
                clic(pt)
                assert commande("projet_natif")["empreinte"]!=avant
                commande("annuler")
                assert commande("projet_natif")["empreinte"]==avant
                commande("retablir")
                assert commande("ajuster_etat")["reference"]==ref
            saisie([{"action":"touche","touche":"Escape"}])
            assert commande("ajuster_etat")["reference"] is None
            assert commande("ajuster_etat")["actif"]
            saisie([{"action":"touche","touche":"Escape"}])
            assert not commande("ajuster_etat")["actif"]
            commande("quitter","abandonner");p.wait(timeout=15)
            print(f"Ajuster graphique : {nombre} requêtes validées, aperçu sans mutation, deux extrémités, annuler/rétablir et Échap OK")
        except Exception:
            log.flush()
            print((root/"instance.log").read_text(encoding="utf-8")[-2500:])
            raise
        finally:
            if p is not None and p.poll() is None:p.terminate();p.wait(timeout=10)
            log.close()

if __name__=="__main__":
    parser=argparse.ArgumentParser()
    parser.add_argument("--exe",type=Path,default=Path("target/ajuster-prolonger/release/bimedit.exe"))
    args=parser.parse_args();run(args.exe,Path("exports/verification-ajuster"))
