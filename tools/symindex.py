"""Indexa os SymbolClass de todos os SWF do jogo -> cache/symbols.json {simbolo: [arquivo, ...]}"""
import glob, json, os, sys
sys.path.insert(0, os.path.dirname(__file__))
import swfx

G = r"D:\Steam\steamapps\common\Brawlhalla"
idx = {}
files = glob.glob(os.path.join(G, '*.swf')) + glob.glob(os.path.join(G, 'bones', '*.swf'))
for f in files:
    try:
        b, ts = swfx.load(f)
    except Exception as e:
        print('falha', f, e)
        continue
    rel = os.path.relpath(f, G).replace(os.sep, '/')
    for name in swfx.symbols(b, ts):
        idx.setdefault(name, []).append(rel)
os.makedirs('cache', exist_ok=True)
json.dump(idx, open('cache/symbols.json', 'w'))
print(len(files), 'swf,', len(idx), 'símbolos')
