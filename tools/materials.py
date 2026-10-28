"""Builds crates/core/data/materials.json, Datum Mesh's materials library, from Datum Atlas.

python3 tools/materials.py <path to a Datum-Atlas checkout>

Values are SI base units. E, density, conductivity and expansion are the middle of Atlas's
typical range; the yield strength is the low end of the range (a safety factor should not lean on
the best grade). Atlas has no Poisson's ratio, so ν comes from the table below: typical values
for each material family, from general engineering handbooks.
"""
import json, sys, os

NU = [  # (id prefix, ν)
    ("steel-", 0.29), ("ss-17", 0.27), ("ss-", 0.29), ("ci-grey", 0.26), ("ci-ductile", 0.275),
    ("al-", 0.33), ("cu-c11000", 0.34), ("cu-c26000", 0.35), ("cu-c51000", 0.34), ("cu-c70600", 0.34),
    ("ti-", 0.34), ("mg-", 0.35), ("ni-", 0.29), ("cocr", 0.30), ("zn-", 0.27), ("pb", 0.44),
    ("sn", 0.36), ("w", 0.28),
    ("pe-", 0.46), ("pp", 0.42), ("ps", 0.35), ("pvc", 0.40), ("pmma", 0.37), ("pc", 0.37),
    ("abs", 0.35), ("pa", 0.39), ("pet", 0.38), ("pbt", 0.38), ("pom", 0.35), ("ptfe", 0.46),
    ("peek", 0.40), ("pps", 0.37), ("pla", 0.36), ("ep", 0.36), ("up", 0.35), ("pf", 0.35), ("pi", 0.34),
]

def g(v):
    return float(f"{v:.6g}")

def nu(mid):
    for p, v in NU:
        if mid.startswith(p):
            return v
    raise SystemExit(f"no Poisson's ratio for {mid}")

def main(atlas):
    data = os.path.join(atlas, "data")
    titles = {s["id"]: s["title"] for s in json.load(open(os.path.join(data, "sources.json")))["sources"]}
    out = []
    for f in ["metals.json", "polymers.json"]:
        d = json.load(open(os.path.join(data, f)))
        items = next(v for v in d.values() if isinstance(v, list))
        for it in items:
            p = it["props"]
            mid = lambda k: (p[k][0] + p[k][1]) / 2 if k in p and p[k][0] is not None else 0.0
            fy = p["sy"][0] * 1e6 if "sy" in p and p["sy"][0] is not None else 0.0
            srcs = []
            for k in ["E", "sy", "rho", "lam", "alpha"]:
                if k in p and p[k][-1] not in srcs:
                    srcs.append(p[k][-1])
            out.append({
                "id": it["id"], "name": it["name"], "family": " › ".join(it["family"][:3]),
                "e": g(mid("E") * 1e9), "nu": nu(it["id"]), "rho": g(mid("rho")), "fy": g(fy),
                "k": g(mid("lam")), "alpha": g(mid("alpha") * 1e-6),
                "source": "Datum Atlas: " + "; ".join(titles.get(s, s) for s in srcs) + ". ν: typical value for the family (Datum Mesh)",
            })
    path = os.path.join(os.path.dirname(__file__), "..", "crates", "core", "data", "materials.json")
    with open(path, "w") as fh:
        json.dump(out, fh, indent=1, ensure_ascii=False)
        fh.write("\n")
    print(f"{len(out)} materials")

main(sys.argv[1] if len(sys.argv) > 1 else "../Datum-Atlas")
