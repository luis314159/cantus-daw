import sys; sys.path.insert(0, __import__("os").path.dirname(__import__("os").path.abspath(__file__)))
from feats import *
for kit, el in (("jazz","caja"),("jazz","tom-medio"),("jazz","tom-de-piso"),("jazz","bombo"),("jazz","crash"),("studio","caja")):
    for v in ("gen","rec","prev"):
        m, on = element_hits(kit, el, v)[2]
        m = np.pad(m,(0,SR))
        n=1<<16; y=m[on:on+int(0.08*SR)]
        X=20*np.log10(np.abs(np.fft.rfft(y*np.hanning(len(y)),n))+1e-12); f=np.fft.rfftfreq(n,1/SR)
        X-=X.max()
        sel=[15,25,35,45,55,70,90,110,150,200]
        print(kit,el,v,"0-80ms spectrum re max at",sel,":"," ".join(f"{X[np.argmin(abs(f-s))]:5.0f}" for s in sel))
    # waveform DC check: low-passed 30 Hz waveform peak re full peak
    for v in ("gen","rec"):
        m, on = element_hits(kit, el, v)[2]
        lp = signal.sosfiltfilt(signal.butter(2, 40, fs=SR, output="sos"), m)
        print("   ", v, "LP40 peak re full peak dB", round(float(db(np.abs(lp).max()/np.abs(m).max())),1), "at ms", round((np.argmax(np.abs(lp))-on)/SR*1000,1))
