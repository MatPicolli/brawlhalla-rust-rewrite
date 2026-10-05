"""Decompila todos os métodos de um ABC e procura um regex: python tools/grepdec.py <swf> <abc#> <regex> [máx]"""
import os, re, sys, importlib.util

spec = importlib.util.spec_from_file_location('bhdecomp', os.path.join(os.path.dirname(os.path.abspath(__file__)), 'decomp.py'))
D = importlib.util.module_from_spec(spec)
spec.loader.exec_module(D)
A = D.A

swf, ai, pat = sys.argv[1], int(sys.argv[2]), re.compile(sys.argv[3])
limit = int(sys.argv[4]) if len(sys.argv) > 4 else 60
a = [A.parse_abc(b) for b in A.swf_abcs(A.swf_body(swf))][ai]
own = A.method_owner(a)
shown = 0
for m in sorted(a.bodies):
    try:
        lines = D.decompile(a, m)
    except Exception:
        continue
    hits = [l for l in lines if pat.search(l)]
    if hits:
        print('method %d %s' % (m, own.get(m, '')))
        for h in hits[:6]:
            print('    ', h.strip()[:200])
        shown += 1
        if shown >= limit:
            break
