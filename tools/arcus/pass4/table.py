import json, sys
a = json.load(open(sys.argv[1])); b = json.load(open(sys.argv[2]))
keys = sys.argv[3].split(",") if len(sys.argv) > 3 else None
def fmt(v):
    if v is None: return "-"
    if "outside" in v:
        x = v.get("arcus"); o = v["outside"]
        try: xs = "%.3g" % float(x)
        except Exception: xs = str(x)
        return "%s (%.2f)" % (xs, float(o) if o not in (None, "None") else float("nan"))
    for k in ("outside_rms_db", "outside_rms_oct"):
        if k in v: return "(%.2f)" % float(v[k]) if v[k] not in (None, "None") else "-"
    return str(v)
for fam in ("cello", "bass"):
    print("|", fam, "| before | after | band |\n|---|---|---|---|")
    for k in a[fam]:
        if keys and not any(k.startswith(p) for p in keys): continue
        bd = a[fam][k].get("band")
        print("| %s | %s | %s | %s |" % (k, fmt(a[fam].get(k)), fmt(b[fam].get(k)), bd if bd else ""))
