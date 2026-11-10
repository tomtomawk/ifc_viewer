"""Parcours réel : Collaborer, puis Ouvrir le même .bimpartage dans deux instances.

python tools/test_partage_ui.py [target/release/bimedit.exe]
Les fichiers et captures sont conservés dans un répertoire temporaire dédié.
"""
from concurrent.futures import ThreadPoolExecutor
import hashlib
import json
import os
from pathlib import Path
import shutil
import socket
import subprocess
import sys
import tempfile
import time

repo=Path(__file__).resolve().parent.parent
exe=Path(sys.argv[1]).resolve() if len(sys.argv)>1 else repo/'target/release/bimedit.exe'
root=Path(tempfile.mkdtemp(prefix='bimedit-partage-ui-'))
entree=root/'Maison.bimpartage'
cache=root/'cache-local'
source=root/'maison.modele.json'
shutil.copyfile(repo/'samples/maison_native.modele.json',source)
empreinte_source=hashlib.sha256(source.read_bytes()).hexdigest()
processus=[]
journaux=[]
ports=[47993,47994]

def rpc(port,commande):
    with socket.create_connection(('127.0.0.1',port),timeout=5) as s:
        s.settimeout(30);s.sendall((commande+'\n').encode('utf-8'))
        with s.makefile('r',encoding='utf-8') as f:r=json.loads(f.readline())
    if not r.get('ok',True):raise RuntimeError(r)
    return r

def attendre(port,critere):
    fin=time.monotonic()+60
    while time.monotonic()<fin:
        try:
            r=rpc(port,'etat')
            if r.get('erreur_chargement'):raise RuntimeError(r)
            if critere(r):return r
        except (OSError,ValueError):pass
        time.sleep(.05)
    raise RuntimeError(f'Délai dépassé : {root}')

def lancer(port,path):
    with socket.socket() as s:s.bind(('127.0.0.1',port))
    log=open(root/f'{port}-{len(processus)}.log','w',encoding='utf-8');journaux.append(log)
    env=dict(os.environ,BIMEDIT_PORT=str(port), BIMEDIT_ARRIERE_PLAN=os.environ.get("BIMEDIT_ARRIERE_PLAN", "1"),BIMEDIT_CACHE_COLLABORATION=str(cache))
    p=subprocess.Popen([str(exe),str(path)],env=env,stdout=log,stderr=log,creationflags=0x08000000 if os.name=='nt' else 0)
    processus.append(p)
    r=attendre(port,lambda r:not r.get('chargement',True) and 'selection_native' in r)
    return p,r

def synchro(port):
    rpc(port,'collaboration_sync')
    return attendre(port,lambda r:r.get('collaboration',{}).get('etat')=='Synchronisé')

def grille(port,nom,y):
    rpc(port,'editer '+json.dumps(dict(action='quadrillage',nom=nom,debut=[0,y],fin=[1000,y])))

try:
    a,_=lancer(ports[0],source)
    rpc(ports[0],'ruban debut');rpc(ports[0],f'capture fenetre {root / "debut.png"}')
    rpc(ports[0],f'collaborer {entree}')
    ra=attendre(ports[0],lambda r:not r.get('chargement',True) and r.get('collaboration',{}).get('etat')=='Synchronisé')
    manifeste=json.loads(entree.read_text(encoding='utf-8'))
    assert manifeste['format']=='bimedit-projet-partage'
    assert (root/manifeste['depot']/'controle').is_file()
    b,rb=lancer(ports[1],entree)
    local_a=Path(ra['sortie_v0']);local_b=Path(rb['sortie_v0'])
    assert local_a!=local_b and local_a.is_relative_to(cache) and local_b.is_relative_to(cache)
    assert local_a.is_file() and local_b.is_file()
    rpc(ports[0],'ruban sortie');rpc(ports[0],f'capture fenetre {root / "sortie.png"}')
    # Les publications géométriques des deux postes partent de la même base.
    murs=[]
    for port,y in zip(ports,[20000,40000]):
        murs.append(rpc(port,f'mur 20000 {y} 24000 {y} 2800 200')['mur'])
    with ThreadPoolExecutor(max_workers=2) as pool:list(pool.map(synchro,ports))
    synchro(ports[0]);synchro(ports[1])
    for port in ports:
        p=rpc(port,'projet_natif')['projet']
        assert set(murs)<=set(m['id'] for m in p['murs'])
    for port,ident,dx in zip(ports,murs,[500,-500]):
        rpc(port,'editer '+json.dumps(dict(action='deplacer',ids=[ident],delta=[dx,0,0])))
    with ThreadPoolExecutor(max_workers=2) as pool:list(pool.map(synchro,ports))
    synchro(ports[0]);synchro(ports[1])
    for port in ports:
        p=rpc(port,'projet_natif')['projet']
        for ident,x in zip(murs,[20500,19500]):
            mur=next(m for m in p['murs'] if m['id']==ident)
            assert mur['axe']['debut'][0]==x
    for port,y in zip(ports,[20000,40000]):
        niveau=rpc(port,'projet_natif')['projet']['niveaux'][0]['id']
        rpc(port,'editer '+json.dumps(dict(action='barre',barre=dict(id='',nom='Poteau de test',genre='poteau',niveau=niveau,debut=[40000,y,0],fin=[40000,y,3000],largeur=250,hauteur=250,rotation=0))))
    with ThreadPoolExecutor(max_workers=2) as pool:list(pool.map(synchro,ports))
    synchro(ports[0]);synchro(ports[1])
    for port in ports:
        assert sum(b['nom']=='Poteau de test' for b in rpc(port,'projet_natif')['projet']['barres'])==2
    assert rpc(ports[0],'projet_natif')['projet']==rpc(ports[1],'projet_natif')['projet']
    grille(ports[0],'PARTAGE-A',1200);synchro(ports[0]);synchro(ports[1])
    # Une proposition de B est durable, mais ne doit pas se publier à sa réouverture.
    grille(ports[1],'PARTAGE-B',2400)
    b.terminate();b.wait(timeout=15)
    b,rb=lancer(ports[1],entree)
    assert Path(rb['sortie_v0'])==local_b
    assert rb['collaboration']['en_attente']==1
    assert any(x['nom']=='PARTAGE-B' for x in rpc(ports[1],'projet_natif')['projet']['quadrillages'])
    assert not any(x['nom']=='PARTAGE-B' for x in rpc(ports[0],'projet_natif')['projet']['quadrillages'])
    synchro(ports[1]);synchro(ports[0])
    for port in ports:
        noms={x['nom'] for x in rpc(port,'projet_natif')['projet']['quadrillages']}
        assert {'PARTAGE-A','PARTAGE-B'}<=noms
    assert hashlib.sha256(source.read_bytes()).hexdigest()==empreinte_source
    assert json.loads(entree.read_text(encoding='utf-8'))==manifeste
    print(json.dumps(dict(essai=str(root),fichier_partage=str(entree),copie_a=str(local_a),copie_b=str(local_b),reprise_locale_sans_publication=True,convergence=True,captures=['debut.png','sortie.png']),ensure_ascii=False,indent=2))
finally:
    for p in processus:
        if p.poll() is None:p.terminate();p.wait(timeout=15)
    for log in journaux:log.close()
