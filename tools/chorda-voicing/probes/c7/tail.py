import sys, os, numpy as np, soundfile as sf
for d in sys.argv[1:]:
    out=[]
    for ph, t0 in (('ballad', 4*60/72*4 + 1.0), ('soft_chords', 9.2)):
        x, sr = sf.read(os.path.join(d, ph + '.wav'), always_2d=True)
        e = (x ** 2).sum(1); h = int(0.05 * sr)
        env = 10 * np.log10(np.array([e[i:i + h].mean() for i in range(0, len(e) - h, h)]) + 1e-20)
        pk = env.max(); i = int(t0 / 0.05)
        out.append(f'{ph}: +0.5 s {env[i+10]-pk:.1f}  +1 s {env[i+20]-pk:.1f}')
    print(os.path.basename(os.path.normpath(d)), ' | '.join(out))
