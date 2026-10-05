"""Pseudo-decompilador AVM2 (pesquisa): simula a pilha e imprime expressões legíveis, com `goto` para o fluxo.

  python tools/decomp.py <swf> <abc#> <method#> [--pos]
  python tools/decomp.py find <swf> <abc#> <substr>     # métodos cujo dono/nome contém substr (ex.: _-F11.)

Não é um decompilador completo: ifs/loops aparecem como `if (...) goto Lxxxx`; `&&`, `||` e ternários são
reconstruídos por heurística. Serve para ler lógica numérica (física, dano) sem decifrar bytecode à mão.
"""
import importlib.util, os, sys

spec = importlib.util.spec_from_file_location('bhabc', os.path.join(os.path.dirname(os.path.abspath(__file__)), 'abc.py'))
A = importlib.util.module_from_spec(spec)
spec.loader.exec_module(A)

BIN = {'add': '+', 'subtract': '-', 'multiply': '*', 'divide': '/', 'modulo': '%', 'lshift': '<<', 'rshift': '>>',
       'urshift': '>>>', 'bitand': '&', 'bitor': '|', 'bitxor': '^', 'equals': '==', 'strictequals': '===',
       'lessthan': '<', 'lessequals': '<=', 'greaterthan': '>', 'greaterequals': '>=', 'add_i': '+',
       'subtract_i': '-', 'multiply_i': '*', 'instanceof': 'instanceof', 'in': 'in', 'istypelate': 'is'}
JCOND = {'ifeq': '{a} == {b}', 'ifne': '{a} != {b}', 'iflt': '{a} < {b}', 'ifle': '{a} <= {b}', 'ifgt': '{a} > {b}',
         'ifge': '{a} >= {b}', 'ifstricteq': '{a} === {b}', 'ifstrictne': '{a} !== {b}',
         'ifnlt': '{a} >= {b}', 'ifnle': '{a} > {b}', 'ifngt': '{a} <= {b}', 'ifnge': '{a} < {b}'}
SKIP = {'convert_s', 'convert_i', 'convert_u', 'convert_d', 'convert_b', 'convert_o', 'coerce', 'coerce_a', 'coerce_s',
        'astype', 'pushscope', 'popscope', 'label', 'debug', 'debugline', 'debugfile', 'kill', 'nop', 'checkfilter',
        'esc_xelem', 'esc_xattr', 'applytype', 'timestamp', 'bkpt'}


def argn(arg):
    return int(arg.rsplit('(', 1)[1].rstrip(')'))


def name_of(arg):
    return arg.rsplit(' (', 1)[0] if arg.endswith(')') else arg


def decompile(a, m, show_pos=False):
    pc = a.methods[m][0]
    ins = A.disasm(a, a.bodies[m][2])
    targets = set()
    for pos, nm, arg in ins:
        if arg.startswith('->'):
            targets.add(int(arg[3:], 16))
    st, out = [], []
    jumps = {}           # alvo -> [(tipo, snapshot da pilha)]
    cond_at = {}         # alvo -> condição (positiva) do desvio que cai ali
    pending_cond = None
    assigned = set()
    names = {0: 'this'}

    def loc(n):
        if n in names:
            return names[n]
        return 'a%d' % n if n <= pc else 'v%d' % n

    def emit(s, pos=None):
        out.append(('%04x  ' % pos if show_pos and pos is not None else '') + s)

    def pop():
        return st.pop() if st else '<?>'

    def obj_member(obj, name):
        return name if obj.startswith('§') and obj[1:] == name else (obj[1:] if obj.startswith('§') else obj) + '.' + name

    def rd_prop(obj, name):
        if obj.startswith('§'):
            return name
        return '%s.%s' % (obj, name)

    for pos, nm, arg in ins:
        if pos in targets:
            snaps = jumps.pop(pos, [])
            if snaps and st:
                merged = False
                for kind, snap, _c in snaps:
                    if kind == 'jump' and len(st) == len(snap) + 1 and pending_cond is not None:
                        y, x = st.pop(), st.pop()
                        st.append('(%s ? %s : %s)' % (pending_cond, x, y))
                        merged = True
                        break
                if not merged:
                    for kind, snap, c in reversed(snaps):
                        if kind in ('iffalse', 'iftrue') and snap and len(snap) == len(st):
                            op = '&&' if kind == 'iffalse' else '||'
                            if snap[-1] in ('false', 'true') and c is not None and snap[-1] == ('false' if kind == 'iffalse' else 'true'):
                                # o compilador empilha o resultado do curto-circuito antes da condição
                                st[-1] = '(%s %s %s)' % (c, op, st[-1])
                            elif snap[-1] != st[-1]:
                                st[-1] = '(%s %s %s)' % (snap[-1], op, st[-1])
            pending_cond = cond_at.get(pos)
            emit('L%04x:' % pos)
        if nm in SKIP:
            continue
        if nm == 'pushbyte' or nm == 'pushshort' or nm in ('pushint', 'pushuint', 'pushdouble', 'pushstring'):
            st.append(arg)
        elif nm == 'pushnull': st.append('null')
        elif nm == 'pushundefined': st.append('undefined')
        elif nm == 'pushtrue': st.append('true')
        elif nm == 'pushfalse': st.append('false')
        elif nm == 'pushnan': st.append('NaN')
        elif nm == 'getlocal': st.append(loc(int(arg)))
        elif nm.startswith('getlocal') and nm[-1].isdigit(): st.append(loc(int(nm[-1])))
        elif nm == 'setlocal' or (nm.startswith('setlocal') and nm[-1].isdigit()):
            n = int(arg) if nm == 'setlocal' else int(nm[-1])
            v = pop()
            if n not in assigned and v in ('NaN', 'null', 'false', '0', 'undefined'):
                assigned.add(n)
            else:
                assigned.add(n)
                emit('%s = %s' % (loc(n), v), pos)
        elif nm in ('findproperty', 'findpropstrict', 'finddef'): st.append('§' + arg)
        elif nm == 'getlex': st.append(arg)
        elif nm == 'getglobalscope': st.append('global')
        elif nm == 'getscopeobject': st.append('scope%s' % arg)
        elif nm == 'newactivation': st.append('activation')
        elif nm == 'getproperty':
            if arg.startswith('<rt'):
                idx, obj = pop(), pop()
                st.append('%s[%s]' % (obj.lstrip('§'), idx))
            else:
                st.append(rd_prop(pop(), arg))
        elif nm in ('setproperty', 'initproperty'):
            if arg.startswith('<rt'):
                v, idx, obj = pop(), pop(), pop()
                emit('%s[%s] = %s' % (obj.lstrip('§'), idx, v), pos)
            else:
                v, obj = pop(), pop()
                emit('%s = %s' % (rd_prop(obj, arg), v), pos)
        elif nm == 'getslot': st.append('%s.slot%s' % (pop(), arg))
        elif nm == 'setslot':
            v, obj = pop(), pop()
            emit('%s.slot%s = %s' % (obj, arg, v), pos)
        elif nm in ('callproperty', 'callproplex', 'callpropvoid'):
            n = argn(arg)
            args = [pop() for _ in range(n)][::-1]
            obj = pop()
            call = '%s(%s)' % (rd_prop(obj, name_of(arg)), ', '.join(args))
            if nm == 'callpropvoid':
                emit(call, pos)
            else:
                st.append(call)
        elif nm == 'constructprop':
            n = argn(arg)
            args = [pop() for _ in range(n)][::-1]
            pop()
            st.append('new %s(%s)' % (name_of(arg), ', '.join(args)))
        elif nm == 'construct':
            n = int(arg) if arg.isdigit() else 0
            args = [pop() for _ in range(n)][::-1]
            st.append('new %s(%s)' % (pop(), ', '.join(args)))
        elif nm == 'constructsuper':
            n = int(arg) if arg.isdigit() else 0
            args = [pop() for _ in range(n)][::-1]
            pop()
            emit('super(%s)' % ', '.join(args), pos)
        elif nm in ('callsuper', 'callsupervoid'):
            n = argn(arg)
            args = [pop() for _ in range(n)][::-1]
            pop()
            call = 'super.%s(%s)' % (name_of(arg), ', '.join(args))
            emit(call, pos) if nm == 'callsupervoid' else st.append(call)
        elif nm in ('callmethod', 'callstatic'):
            i, n = arg.split()
            args = [pop() for _ in range(int(n))][::-1]
            pop()
            st.append('%s#%s(%s)' % (nm, i, ', '.join(args)))
        elif nm == 'call':
            n = int(arg)
            args = [pop() for _ in range(n)][::-1]
            recv, fn = pop(), pop()
            st.append('%s(%s)' % (fn, ', '.join(args)))
        elif nm in BIN:
            b, a_ = pop(), pop()
            st.append('(%s %s %s)' % (a_, BIN[nm], b))
        elif nm == 'negate' or nm == 'negate_i': st.append('(-%s)' % pop())
        elif nm == 'not': st.append('!%s' % pop())
        elif nm == 'bitnot': st.append('~%s' % pop())
        elif nm in ('increment', 'increment_i'): st.append('(%s + 1)' % pop())
        elif nm in ('decrement', 'decrement_i'): st.append('(%s - 1)' % pop())
        elif nm == 'typeof': st.append('typeof(%s)' % pop())
        elif nm == 'istype': st.append('(%s is %s)' % (pop(), arg))
        elif nm in ('inclocal', 'inclocal_i'): emit('%s++' % loc(int(arg)), pos)
        elif nm in ('declocal', 'declocal_i'): emit('%s--' % loc(int(arg)), pos)
        elif nm == 'dup': st.append(st[-1] if st else '<?>')
        elif nm == 'swap':
            if len(st) >= 2: st[-1], st[-2] = st[-2], st[-1]
        elif nm == 'pop':
            e = pop()
            if '(' in e: emit(e, pos)
        elif nm == 'returnvalue': emit('return %s' % pop(), pos)
        elif nm == 'returnvoid': emit('return', pos)
        elif nm == 'throw': emit('throw %s' % pop(), pos)
        elif nm == 'newarray':
            n = int(arg)
            st.append('[%s]' % ', '.join([pop() for _ in range(n)][::-1]))
        elif nm == 'newobject':
            n = int(arg)
            items = [pop() for _ in range(n * 2)][::-1]
            st.append('{%s}' % ', '.join('%s: %s' % (items[i], items[i + 1]) for i in range(0, len(items), 2)))
        elif nm in ('newclass', 'newfunction', 'newcatch'): st.append('<%s %s>' % (nm, arg))
        elif nm == 'getdescendants': st.append('%s..%s' % (pop(), arg))
        elif nm == 'deleteproperty': st.append('delete %s.%s' % (pop(), arg))
        elif nm == 'hasnext2': emit('hasnext2 %s' % arg, pos); st.append('hasnext')
        elif nm in ('nextname', 'nextvalue', 'hasnext'):
            b, a_ = pop(), pop()
            st.append('%s(%s, %s)' % (nm, a_, b))
        elif nm == 'lookupswitch': emit('switch (%s) { %s }' % (pop(), arg), pos)
        elif nm == 'jump':
            t = int(arg[3:], 16)
            jumps.setdefault(t, []).append(('jump', list(st), None))
            emit('goto L%04x' % t, pos)
        elif nm in ('iftrue', 'iffalse'):
            c = pop()
            t = int(arg[3:], 16)
            jumps.setdefault(t, []).append((nm, list(st), c))
            cond_at[t] = c if nm == 'iffalse' else '!%s' % c
            emit('if (%s) goto L%04x' % (c if nm == 'iftrue' else '!%s' % c, t), pos)
        elif nm in JCOND:
            b, a_ = pop(), pop()
            t = int(arg[3:], 16)
            c = JCOND[nm].format(a=a_, b=b)
            jumps.setdefault(t, []).append((nm, list(st), None))
            emit('if (%s) goto L%04x' % (c, t), pos)
        else:
            emit('/* %s %s */' % (nm, arg), pos)
    return out


def main():
    argv = sys.argv[1:]
    if argv and argv[0] == 'find':
        swf, ai, sub = argv[1], int(argv[2]), argv[3]
        a = [A.parse_abc(b) for b in A.swf_abcs(A.swf_body(swf))][ai]
        for m, name in sorted(A.method_owner(a).items()):
            if sub in name:
                print(m, name, len(a.bodies[m][2]) if m in a.bodies else 0)
        return
    swf, ai, m = argv[0], int(argv[1]), int(argv[2])
    a = [A.parse_abc(b) for b in A.swf_abcs(A.swf_body(swf))][ai]
    print('//', A.method_owner(a).get(m, ''), 'params=%d' % a.methods[m][0])
    print('\n'.join(decompile(a, m, '--pos' in argv)))


if __name__ == '__main__':
    main()
