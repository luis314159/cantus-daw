import sys, numpy as np, soundfile as sf
from scipy import signal
SR=48000
x=sf.read(sys.argv[1],always_2d=True)[0]
out=[]
for key in (57,60,63,66):
    f0=440*2**((key-69)/12)
    y=x[(key-21)*4*SR:((key-21)*4+3)*SR]
    on=int(np.argmax(np.abs(y).sum(1)>0.05*np.abs(y).sum(1).max())); y=y[on:]
    f,t,Z=signal.stft(y.T,SR,nperseg=4800,noverlap=4200); P=(np.abs(Z)**2).sum(0)
    tot=10*np.log10(P[(f>50)&(f<5000)].sum(0)+1e-30).max()
    def env(n):
        g=n*f0*(1+0.0004*n*n*(key/60)**2)**0.5; b=(f>g*0.985)&(f<g*1.025); return 10*np.log10(P[b].max(0)+1e-30)-tot
    out.append('%d p8 %.0f/%.0f p9 %.0f/%.0f' % (key, env(8)[1], env(8)[120], env(9)[1], env(9)[120]))
print(' | '.join(out))
