"""Explorador de SWF (pesquisa): símbolos e árvore de sprites.

  python tools/swfx.py symbols <swf> [filtro]
  python tools/swfx.py sprite  <swf> <símbolo>     # estrutura do sprite (tags internas por quadro)
"""
import os, struct, sys, zlib, collections

NAMES = {0: 'End', 1: 'ShowFrame', 2: 'DefineShape', 4: 'PlaceObject', 5: 'RemoveObject', 9: 'SetBackgroundColor', 22: 'DefineShape2',
         26: 'PlaceObject2', 28: 'RemoveObject2', 32: 'DefineShape3', 37: 'DefineEditText', 39: 'DefineSprite', 43: 'FrameLabel',
         56: 'ExportAssets', 69: 'FileAttributes', 70: 'PlaceObject3', 76: 'SymbolClass', 82: 'DoABC2', 83: 'DefineShape4', 86: 'DefineSceneAndFrameLabelData',
         12: 'DoAction', 14: 'DefineSound', 18: 'SoundStreamHead', 45: 'SoundStreamHead2', 62: 'DefineFontInfo2', 75: 'DefineFont3', 6: 'DefineBits', 20: 'DefineBitsLossless', 21: 'DefineBitsJPEG2', 35: 'DefineBitsJPEG3', 36: 'DefineBitsLossless2', 34: 'DefineButton2', 7: 'DefineButton', 10: 'DefineFont', 11: 'DefineText', 33: 'DefineText2', 46: 'DefineMorphShape', 84: 'DefineMorphShape2', 87: 'DefineBinaryData'}

def body(path):
    b = open(path, 'rb').read()
    return zlib.decompress(b[8:]) if b[:3] == b'CWS' else b[8:]

def rect_end(b):
    nb = b[0] >> 3
    return (5 + nb * 4 + 7) // 8

def tags(b, off=0, end=None):
    end = end or len(b)
    out = []
    while off + 2 <= end:
        h = struct.unpack('<H', b[off:off + 2])[0]; t, l = h >> 6, h & 63; off += 2
        if l == 63:
            l = struct.unpack('<I', b[off:off + 4])[0]; off += 4
        out.append((t, off, l)); off += l
        if t == 0: break
    return out

def load(path):
    b = body(path)
    start = rect_end(b) + 4
    return b, tags(b, start)

def symbols(b, ts):
    d = {}
    for t, o, l in ts:
        if t == 76:
            n = struct.unpack('<H', b[o:o + 2])[0]; p = o + 2
            for _ in range(n):
                cid = struct.unpack('<H', b[p:p + 2])[0]; e = b.index(b'\0', p + 2)
                d[b[p + 2:e].decode('utf8', 'replace')] = cid; p = e + 1
    return d

if __name__ == '__main__':
    cmd, path = sys.argv[1], sys.argv[2]
    b, ts = load(path)
    sym = symbols(b, ts)
    if cmd == 'symbols':
        flt = sys.argv[3] if len(sys.argv) > 3 else ''
        for k, v in sym.items():
            if flt in k: print(v, k)
        print(len(sym), 'símbolos;', collections.Counter(NAMES.get(t, t) for t, o, l in ts).most_common(12))
    elif cmd == 'sprite':
        cid = sym[sys.argv[3]]
        for t, o, l in ts:
            if t == 39 and struct.unpack('<H', b[o:o + 2])[0] == cid:
                frames = struct.unpack('<H', b[o + 2:o + 4])[0]
                print('sprite', cid, 'frames', frames)
                for it, io, il in tags(b, o + 4, o + l):
                    extra = ''
                    if it in (26, 70):
                        fl = b[io]; p = io + 1
                        if it == 70: p += 1
                        depth = struct.unpack('<H', b[p:p + 2])[0]; p += 2
                        if fl & 2: extra += ' char=%d' % struct.unpack('<H', b[p:p + 2])[0]; p += 2
                        extra += ' depth=%d flags=%02x' % (depth, fl)
                    print('  ', NAMES.get(it, it), il, extra)
