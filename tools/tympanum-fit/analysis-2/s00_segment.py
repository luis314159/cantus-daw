import sys; sys.path.insert(0, __import__("os").path.dirname(__import__("os").path.abspath(__file__)))
from common import *
for kit in KITS:
    for el in ELEMENTS:
        row=[]
        for v in ("prev","gen","rec"):
            hs=element_hits(kit,el,v)
            row.append(" ".join(f"{on/SR*1000:6.1f}ms/{db(np.abs(s).max()):5.1f}" for s,on in hs))
        print(f"{kit:6s} {el:14s}", " | ".join(row))
