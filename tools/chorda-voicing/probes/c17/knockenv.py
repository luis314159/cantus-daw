import sys, os, numpy as np, soundfile as sf
from scipy import signal
SR=48000; KEYS=list(range(21,109,3))
dyn=sys.argv[1]; keys=[int(k) for k in sys.argv[2].split(',')]; srcs=sys.argv[3:]
bands=[(60,180),(180,500),(500,1400),(1400,2800)]
ts=[0,5,10,20,40,80,160,320]
for lab_d in srcs:
    lab,d=lab_d.split('=')
    x,_=sf.read(os.path.join(d,f'notes_{dyn}.wav'),always_2d=True); m=x.mean(1)
    acc=np.zeros((len(bands),len(ts)-1))
    for k in keys:
        a=int(KEYS.index(k)*6*SR); seg=np.abs(m[a:a+int(0.3*SR)]); on=a+int(np.argmax(seg>seg.max()*0.05))
        y=m[on-int(0.002*SR):on+int(0.35*SR)]
        ref=10*np.log10((m[on:on+int(0.06*SR)]**2).mean())
        f0=440*2**((k-69)/12)
        for i,(lo,hi) in enumerate(bands):
            hi2=min(hi,0.7*f0)
            if hi2<=lo: acc[i]+=np.nan; continue
            sos=signal.butter(4,[lo,hi2],'band',fs=SR,output='sos'); z=signal.sosfilt(sos,y)**2
            for j in range(len(ts)-1):
                s=z[int(ts[j]*SR/1000):int(ts[j+1]*SR/1000)+1]
                acc[i,j]+=10*np.log10(s.mean()+1e-30)-ref
    acc/=len(keys)
    print(lab, 'dB re note (0-60ms) per band, windows ms', ts)
    for i,b in enumerate(bands): print(f'  {b[0]:5d}-{b[1]:<5d}'+''.join(f'{v:7.1f}' for v in acc[i]))
