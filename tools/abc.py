"""Desassemblador AVM2/ABC mínimo para estudar o BrawlhallaAir.swf (ferramenta de pesquisa).

Uso:
  python tools/abc.py scan   <swf> [substr...]   # métodos que chamam as funções dadas
  python tools/abc.py dis    <swf> <abc#> <method#>
  python tools/abc.py strings <swf> <substr>      # strings do constant pool
"""
import os, struct, sys, zlib

# ---------------------------------------------------------------- SWF
def swf_body(path):
    b = open(path, 'rb').read()
    return zlib.decompress(b[8:]) if b[:3] == b'CWS' else b[8:]

def swf_abcs(body):
    nb = body[0] >> 3
    off = (5 + nb * 4 + 7) // 8 + 4
    out = []
    while off + 2 <= len(body):
        h = struct.unpack('<H', body[off:off + 2])[0]
        t, l = h >> 6, h & 63
        off += 2
        if l == 63:
            l = struct.unpack('<I', body[off:off + 4])[0]
            off += 4
        if t == 82:
            e = body.index(b'\0', off + 4)
            out.append(body[e + 1:off + l])
        elif t == 72:
            out.append(body[off:off + l])
        off += l
        if t == 0:
            break
    return out

# ---------------------------------------------------------------- ABC
class R:
    def __init__(s, b, o=0):
        s.b, s.o = b, o
    def u8(s):
        v = s.b[s.o]; s.o += 1; return v
    def u30(s):
        v = sh = 0
        while True:
            x = s.b[s.o]; s.o += 1
            v |= (x & 0x7f) << sh; sh += 7
            if not x & 0x80: return v
    def s32(s):
        v = s.u30()
        return v - (1 << 32) if v & 0x80000000 else v
    def raw(s, n):
        v = s.b[s.o:s.o + n]; s.o += n; return v

class Abc: pass

def parse_abc(b):
    r = R(b); a = Abc()
    r.o = 4
    a.ints = [0] + [r.s32() for _ in range(max(r.u30() - 1, 0))]
    a.uints = [0] + [r.u30() for _ in range(max(r.u30() - 1, 0))]
    n = r.u30(); a.doubles = [0.0] + [struct.unpack('<d', r.raw(8))[0] for _ in range(max(n - 1, 0))]
    n = r.u30(); a.strings = [''] + [r.raw(r.u30()).decode('utf8', 'replace') for _ in range(max(n - 1, 0))]
    n = r.u30(); a.ns = [(0, 0)] + [(r.u8(), r.u30()) for _ in range(max(n - 1, 0))]
    n = r.u30(); a.nssets = [[]]
    for _ in range(max(n - 1, 0)):
        a.nssets.append([r.u30() for _ in range(r.u30())])
    n = r.u30(); a.mn = [None]
    for _ in range(max(n - 1, 0)):
        k = r.u8()
        if k in (7, 0xD): a.mn.append((k, r.u30(), r.u30()))
        elif k in (0xF, 0x10): a.mn.append((k, r.u30()))
        elif k in (0x11, 0x12): a.mn.append((k,))
        elif k in (9, 0xE): a.mn.append((k, r.u30(), r.u30()))
        elif k in (0x1B, 0x1C): a.mn.append((k, r.u30()))
        elif k == 0x1D:
            nm = r.u30(); ps = [r.u30() for _ in range(r.u30())]; a.mn.append((k, nm, ps))
        else: raise ValueError('multiname kind %x' % k)
    n = r.u30(); a.methods = []
    for _ in range(n):
        pc = r.u30(); ret = r.u30(); pt = [r.u30() for _ in range(pc)]; name = r.u30(); fl = r.u8()
        if fl & 8:
            for _ in range(r.u30()): r.u30(); r.u8()
        if fl & 0x80:
            for _ in range(pc): r.u30()
        a.methods.append((pc, ret, pt, name, fl))
    for _ in range(r.u30()):
        r.u30()
        for _ in range(r.u30() * 2): r.u30()
    def traits():
        out = []
        for _ in range(r.u30()):
            name = r.u30(); kind = r.u8(); t = kind & 15; d = None
            if t in (0, 6):
                slot = r.u30(); ty = r.u30(); vi = r.u30()
                if vi: r.u8()
                d = (slot, ty)
            elif t == 4: d = (r.u30(), r.u30())
            elif t == 5: d = (r.u30(), r.u30())
            else: d = (r.u30(), r.u30())
            if kind & 0x40:
                for _ in range(r.u30()): r.u30()
            out.append((name, t, d))
        return out
    nc = r.u30(); a.instances = []
    for _ in range(nc):
        name = r.u30(); sup = r.u30(); fl = r.u8()
        if fl & 8: r.u30()
        for _ in range(r.u30()): r.u30()
        init = r.u30(); tr = traits()
        a.instances.append((name, sup, init, tr))
    a.classes = []
    for _ in range(nc):
        a.classes.append((r.u30(), traits()))
    a.scripts = [(r.u30(), traits()) for _ in range(r.u30())]
    a.bodies = {}
    for _ in range(r.u30()):
        m = r.u30(); ms = r.u30(); lc = r.u30(); isd = r.u30(); msd = r.u30()
        code = r.raw(r.u30())
        for _ in range(r.u30()):
            for _ in range(5): r.u30()
        traits()
        a.bodies[m] = (ms, lc, code)
    return a

def mname(a, i):
    if i == 0: return '*'
    m = a.mn[i]
    if m[0] in (7, 0xD): return a.strings[m[2]]
    if m[0] in (0xF, 0x10): return a.strings[m[1]]
    if m[0] in (9, 0xE): return a.strings[m[1]]
    if m[0] == 0x1D: return '%s<%s>' % (mname(a, m[1]), ','.join(mname(a, p) for p in m[2]))
    return '<rt%x>' % m[0]

# ---------------------------------------------------------------- opcodes
U30_1 = {0x04,0x05,0x06,0x08,0x25,0x2C,0x2D,0x2E,0x2F,0x31,0x40,0x41,0x42,0x49,0x55,0x56,0x58,0x59,0x5A,
         0x5D,0x5E,0x5F,0x60,0x61,0x62,0x63,0x66,0x68,0x6A,0x6C,0x6D,0x80,0x86,0x92,0x94,0xB2,0xC2,0xC3,
         0xF0,0xF1,0xF2,0x53,0x65}
U30_2 = {0x32,0x43,0x44,0x45,0x46,0x4A,0x4C,0x4E,0x4F}
JUMPS = set(range(0x0C, 0x1B)) | {0x10}
NAMES = {0x24:'pushbyte',0x25:'pushshort',0x2C:'pushstring',0x2D:'pushint',0x2E:'pushuint',0x2F:'pushdouble',
 0x20:'pushnull',0x21:'pushundefined',0x26:'pushtrue',0x27:'pushfalse',0x28:'pushnan',0x29:'pop',0x2A:'dup',0x2B:'swap',
 0x30:'pushscope',0x47:'returnvoid',0x48:'returnvalue',0x46:'callproperty',0x4F:'callpropvoid',0x4C:'callproplex',
 0x4A:'constructprop',0x42:'construct',0x41:'call',0x5D:'findpropstrict',0x5E:'findproperty',0x60:'getlex',
 0x61:'setproperty',0x62:'getlocal',0x63:'setlocal',0x66:'getproperty',0x68:'initproperty',0x6C:'getslot',0x6D:'setslot',
 0x64:'getglobalscope',0x65:'getscopeobject',0x70:'convert_s',0x73:'convert_i',0x74:'convert_u',0x75:'convert_d',
 0x76:'convert_b',0x80:'coerce',0x82:'coerce_a',0x85:'coerce_s',0x91:'increment',0x93:'decrement',0xA0:'add',0xA1:'subtract',
 0xA2:'multiply',0xA3:'divide',0xA4:'modulo',0xA5:'lshift',0xA6:'rshift',0xA7:'urshift',0xA8:'bitand',0xA9:'bitor',
 0xAA:'bitxor',0xAB:'equals',0xAC:'strictequals',0xAD:'lessthan',0xAE:'lessequals',0xAF:'greaterthan',0xB0:'greaterequals',
 0x96:'not',0x90:'negate',0xC0:'increment_i',0xC1:'decrement_i',0x57:'newactivation',0x56:'newarray',0x55:'newobject',
 0x49:'constructsuper',0x1D:'popscope',0x1B:'lookupswitch',0x09:'label',0x02:'nop',0x03:'throw',0x35:'li8',0x36:'li16',
 0x37:'li32',0x38:'lf32',0x39:'lf64',0x59:'getdescendants',0x6A:'deleteproperty',0x1F:'hasnext',0x32:'hasnext2',
 0x1E:'nextname',0x23:'nextvalue',0xB1:'instanceof',0xB2:'istype',0xB3:'istypelate',0xB4:'in',0x95:'typeof',
 0xD0:'getlocal0',0xD1:'getlocal1',0xD2:'getlocal2',0xD3:'getlocal3',0xD4:'setlocal0',0xD5:'setlocal1',0xD6:'setlocal2',0xD7:'setlocal3',
 0x92:'inclocal',0x94:'declocal',0xC2:'inclocal_i',0xC3:'declocal_i',0xEF:'debug',0xF0:'debugline',0xF1:'debugfile',
 0x58:'newclass',0x40:'newfunction',0x4E:'callsupervoid',0x45:'callsuper',0x43:'callmethod',0x44:'callstatic',
 0x04:'getsuper',0x05:'setsuper',0x06:'dxns',0x08:'kill',0x53:'applytype',0x5A:'newcatch',0x5F:'finddef',0x31:'pushnamespace',
 0x86:'astype',0x87:'astypelate',0x97:'bitnot',0xC4:'negate_i',0xC5:'add_i',0xC6:'subtract_i',0xC7:'multiply_i',
 0x71:'esc_xelem',0x72:'esc_xattr',0x77:'convert_o',0x78:'checkfilter',0x1C:'pushwith',0x0C:'ifnlt',0x0D:'ifnle',
 0x0E:'ifngt',0x0F:'ifnge',0x10:'jump',0x11:'iftrue',0x12:'iffalse',0x13:'ifeq',0x14:'ifne',0x15:'iflt',0x16:'ifle',
 0x17:'ifgt',0x18:'ifge',0x19:'ifstricteq',0x1A:'ifstrictne',0xF3:'timestamp',0x01:'bkpt',0x07:'dxnslate',
 0x6E:'getglobalslot',0x6F:'setglobalslot',0x3A:'si8',0x3B:'si16',0x3C:'si32',0x3D:'sf32',0x3E:'sf64'}

def disasm(a, code):
    r = R(code); out = []
    while r.o < len(code):
        pos = r.o; op = r.u8(); name = NAMES.get(op, 'op%02x' % op); arg = ''
        if op == 0x24: v = r.u8(); arg = str(v - 256 if v > 127 else v)
        elif op == 0x25: arg = str(r.u30())
        elif op in JUMPS:
            v = code[r.o] | code[r.o+1] << 8 | code[r.o+2] << 16; r.o += 3
            if v & 0x800000: v -= 1 << 24
            arg = '-> %04x' % (r.o + v)
        elif op == 0x1B:
            d = code[r.o] | code[r.o+1] << 8 | code[r.o+2] << 16; r.o += 3
            n = r.u30(); r.o += 3 * (n + 1); arg = 'cases=%d' % (n + 1)
        elif op == 0xEF: r.u8(); r.u30(); r.u8(); r.u30()
        elif op in (0x2C,): arg = repr(a.strings[r.u30()])
        elif op == 0x2D: arg = str(a.ints[r.u30()])
        elif op == 0x2E: arg = str(a.uints[r.u30()])
        elif op == 0x2F: arg = str(a.doubles[r.u30()])
        elif op in (0x46,0x4F,0x4C,0x4A,0x45,0x4E):
            m = r.u30(); n = r.u30(); arg = '%s (%d)' % (mname(a, m), n)
        elif op in (0x5D,0x5E,0x60,0x61,0x66,0x68,0x6A,0x80,0x59,0xB2,0x5F,0x04,0x05,0x86):
            arg = mname(a, r.u30())
        elif op == 0x32: arg = '%d %d' % (r.u30(), r.u30())
        elif op in (0x43, 0x44): arg = '%d %d' % (r.u30(), r.u30())
        elif op in U30_1 or op == 0xF0:
            arg = str(r.u30())
        out.append((pos, name, arg))
    return out

def method_owner(a):
    """método -> 'Classe.nome' quando conhecido via traits."""
    own = {}
    for ci, (iname, sup, init, tr) in enumerate(a.instances):
        cn = mname(a, iname); own[init] = cn + '.<init>'
        for name, t, d in tr:
            if t in (1, 2, 3): own[d[1]] = '%s.%s' % (cn, mname(a, name))
        cinit, ctr = a.classes[ci]; own[cinit] = cn + '.<cinit>'
        for name, t, d in ctr:
            if t in (1, 2, 3): own[d[1]] = '%s.static.%s' % (cn, mname(a, name))
    return own

if __name__ == '__main__':
    cmd, path = sys.argv[1], sys.argv[2]
    abcs = [parse_abc(b) for b in swf_abcs(swf_body(path))]
    if cmd == 'scan':
        wanted = sys.argv[3:] or ['readFloat']
        for ai, a in enumerate(abcs):
            own = method_owner(a)
            for m, (ms, lc, code) in a.bodies.items():
                calls = {}
                for pos, nm, arg in disasm(a, code):
                    if nm in ('callproperty', 'callproplex', 'callpropvoid'):
                        n = arg.split(' ')[0]
                        if n in wanted: calls[n] = calls.get(n, 0) + 1
                if len(calls) >= 1 and sum(calls.values()) >= int(os.environ.get('MINCALLS','2')):
                    print('abc%d method %d %s len=%d %s' % (ai, m, own.get(m, ''), len(code), calls))
    elif cmd == 'dis':
        a = abcs[int(sys.argv[3])]; m = int(sys.argv[4])
        print(method_owner(a).get(m, ''))
        for pos, nm, arg in disasm(a, a.bodies[m][2]): print('%04x  %-16s %s' % (pos, nm, arg))
    elif cmd == 'refs':
        needle = sys.argv[3]
        for ai, a in enumerate(abcs):
            own = method_owner(a)
            for m, (ms, lc, code) in a.bodies.items():
                hits = [(pos, nm, arg) for pos, nm, arg in disasm(a, code) if needle in arg and nm not in ('getlocal',)]
                if hits:
                    print('abc%d method %d %s: %s' % (ai, m, own.get(m, ''), ', '.join('%s %s' % (nm, arg) for pos, nm, arg in hits[:4])))
    elif cmd == 'strings':
        for ai, a in enumerate(abcs):
            for i, s in enumerate(a.strings):
                if sys.argv[3] in s: print('abc%d str%d %r' % (ai, i, s))
