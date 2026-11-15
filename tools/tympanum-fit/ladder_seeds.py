"""The ladder's noise ceiling (round 40, ladder): R3 rendered with three seeds, each measured
against the others with the ladder's tolerances — how often a stochastic rung meets *itself*.
The hits files hold one take per velocity, so this stands in for the recording's own spread.

    python -I ladder_seeds.py
"""
import os
import sys
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import numpy as np, ladder as L, ladder_measure as M
from early import onset1
items=[('drs','snare',127),('drs','kick',127),('drs','crash',127),('virt','ride',80),('drs','hat_open',127),('drs','hat_closed',127),('virt','tom_mid',127)]
tot={}
for kit,art,vel in items:
    m,on=L.recorded(kit,art,vel); n=len(m); tab,lay=L.layer(kit,art,vel)
    a0=onset1(m,on)-24; s0=L.seed(kit,art,vel); rng=lambda k: np.random.default_rng(s0+k)
    bands=L.place(L.render_bands(tab,lay,n-on,rng(1)),n,on); modes=L.place(L.render_modes(lay,n-on,rng(2)),n,on)
    att=L.attack(m,a0,modes+bands*L.late_gate(n,a0))
    P,Pres=L.residual(m,[modes,att],a0); g,s=L.clip_window(n,a0); gate=L.onset_gate(n,a0)
    P3=L.table_power(L.table_of(Pres,a0),len(P),a0)
    ys=[]
    for sd in (5,105,205):
        modes2=L.place(L.render_modes(lay,n-on,np.random.default_rng(s0+2+sd)),n,on)
        ys.append(((modes2+att+L.noise_resynth(P3,n,np.random.default_rng(s0+sd))*gate)*g)[s:])
    ms=[M.measures(y,on-s,a0-s,art) for y in ys]
    ks=list(ms[0]); met=[]
    for a in range(3):
        for b in range(3):
            if a!=b: met+= [bool(M.meets(k,ms[b][k],ms[a][k])) for k in ks if M.meets(k,ms[b][k],ms[a][k]) is not None]
    print(kit,art,vel,'R3 vs R3 (other seeds): %.0f %% met'%(100*np.mean(met)), flush=True)
