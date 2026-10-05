# Notas de engenharia reversa do bytecode (BrawlhallaAir.swf, versão 10.11.84336)

Ferramentas: `tools/abc.py` (desassemblador), `tools/decomp.py` (pseudo-decompilador de pilha),
`tools/grepdec.py` (regex sobre todos os métodos decompilados). Só o ABC nº 11 tem código (15.103 métodos).
Os nomes (`_-xxx`) são ofuscados e **valem só para esta versão do jogo**. Antes de confiar numa leitura:
o compilador do jogo emite `pushfalse; X; iffalse L; pop; Y; L:` para `X && Y` (idem `pushtrue`/`||`);
o decompilador já trata isso. Vários booleanos são **bits empacotados** (`(campo >>> 24) & 31` indica o bit):
`r5Q`, `a30`, `p1F`, `k1X`, `m2O` etc. são flags sem nome legível; só dá para inferir o sentido pelo uso.

## Constantes do motor (`_-x16`, método 1198)
| nome | valor | significado |
|---|---|---|
| `_-A6g` | 0.384 | passo de tempo por tick: posição += vel * dt; aceleração/atrito/gravidade também × dt |
| `_-r3h` / `_-J1y` | '10.11' / '10.11.84336' | versão do jogo |

## Física do lutador (classe `_-F11`, base de lutadores/companheiros/itens)
- Campos padrão no construtor (método 2796): `_-q46` gravidade 3.75, `_-t5y` RunSpeed 30, `_-H4n` AirRunSpeed 30,
  `_-V26` Acceleration 4.7, `_-z5d` AirAcceleration 5.16, `_-W2F` Friction 2.5, `_-T2Q` AirFriction 1.2 (sobrescritos por `StatTypes`).
- Mapeamento XML→campo de `StatType` (método 11737): RunSpeed=`_-t5y`, AirRunSpeed=`_-H4n`, Acceleration=`_-V26`,
  AirAcceleration=`_-z5d`, Friction=`_-W2F`, AirFriction=`_-T2Q`, JumpXImpulse=`_-d5p`, AnimSpeed=`_-V2Z`.
- Velocidades: `_-m2j._-d16(_-45G)` = vx, `_-m2j._-d16(_-92E)` = vy. Buffers de impulso: `_-S61` (x), `_-b2x` (y), somados à velocidade no tick.
- Tick principal: `_-F11._-I30` (método 2893). Ordem aproximada: atrito (x) → aceleração (x) → aplicar impulsos (limite |v| 1000, 3000 em certo modo) → gravidade → mover com colisão.
  - aceleração: se vx não atingiu o alvo, `vx ± accel*dt` limitado a `RunSpeed` (chão) / `AirRunSpeed` (ar)
  - atrito: `|vx| -= friction*dt` até 0; valores especiais 3.534/5.655/4.013/9.0/3.25/4.0 em estados (hitstun, etc.), semântica pendente
  - gravidade: `vy += 3.75*dt` enquanto `vy < 70` (85 em queda rápida); queda rápida soma 12*dt; acima de 70 decai 6*dt até 70
- Pulo: `_-F11._-fc` (método 2961). `vy = 0` e impulso `-57` (× 0.86 se `a2`; salto com dash 170; parede usa `_-E3B`). Pulos no ar permitidos: `w5O() < I4w()`.
- Hitstun/knockback: `_-F11._-F3h` (método 2903); máscara de direção de entrada `_-p5C._-J6I`: 1=cima 2=baixo 4=esquerda 8=direita.

## Formato `.anm`
Ver `crates/bh-assets/src/anm.rs` (classes `_-p3N`, `_-T4f`, `_-S2a`, `_-N3T`, `_-X5F`); índice de osso = `BoneTypes` + 1.

## Combate (lido em 2026-10-03)
- `PowerType.static._-h2H` (6362) lê as colunas de `powerTypes`; classe `PowerType` não é ofuscada. Microssintaxe em `PowerType.static._-u1w` (6363):
  `a&b` hitboxes simultâneos, `a~b~c` interpolação linear por partes sobre os eventos, `a|b` alterna, `tN` total dividido, número = constante.
  `CastTime` = lista de segmentos `A:B@C-D`: `A` ticks de preparação, `B+1` eventos (um por tick); `@C-D` quadros-chave da animação (`PowerType._-56K`).
- Campos de `PowerType`: BaseDamage=`_-n2H`, FixedImpulse=`_-N6P`, VariableImpulse=`_-X21`, ImpulseOffsetX/Y=`_-tI`/`_-S1o`, AoERadiusX/Y=`_-S6G`/`_-06A`,
  CenterOffsetX/Y=`_-R3v`/`_-M5B`, FixedStunTime=`_-s3D` (só o primeiro número), atrasos por evento=`_-24A`, MinimumImpulse=`_-R5L`, ImpulseToPoint=`_-11l`.
- Acerto: `_-E4h._-V1c` (1645). dano = BaseDamage × `r37[11 + Força_atacante(_-m1H) − Defesa_alvo(_-H3s)]` (tabela `_-E4h._-r37`, 23 valores, método 1649);
  multiplicadores `LX`, `K1t`, `z36`, `w1d`, `s4f`, `e3Q` valem 1.0 por padrão (buffs/modos mudam). Na tela: dano × (config de dano / 100).
- Knockback: `_-E4h._-i2J` (1644) = `max(Variável × (d + d²/2) × ImpulseMult(Força) × z36 / w1d − ImpulseNegation, 0)` com `d = (dano_alvo + dano_do_golpe)/100`;
  total = i2J + FixedImpulse. O vetor é normalizado para essa magnitude e vira o buffer de impulso do alvo (`_-S61`/`_-b2x`), somado à velocidade no tick seguinte
  (`_-E4h._-713`, 1647: zera a velocidade se o vetor tem y ≠ 0). Stun = `FixedStunTime × 16` unidades de tempo (16 por tick), fixo por poder.
- Direção: `_-Z1a._-J2J` (1698) = normalizar((alvo − atacante) + (±OffsetX, OffsetY)); se vetor nulo sobe (0,−1); `LockTo45Degrees` trava em 45°;
  `ImpulseToPoint` (n ticks) calcula a velocidade balística até (atacante + offset) compensando atrito de hitstun 4.0·dt e gravidade, e o impulso fixo é limitado a essa velocidade.
- Hitbox: `_-6z._-M3S` (65): centro = posição do atacante + (±CenterOffsetX, CenterOffsetY) (x espelha com o lado); testado por `_-R48._-R4X` (não lido).
  A posição dos golpes é o **centro do corpo** (hurtbox padrão 145×160, `hurtboxTypes.DEFAULT`); os pés ficam ~80 px abaixo.
- Execução do poder: `_-6z._-e58` (46); tempo em unidades de 16 por tick; um alvo é atingido no máximo uma vez por poder (`_-fe`), exceto multi-hit.
- Máscara de entrada `_-p5C._-J6I`: 1=cima, 2=baixo, 4=esquerda, 8=direita.

## Movimento e colisão (lido em 2026-10-03, substitui o modelo provisório)
- O corpo é uma **linha vertical de 120 px**: ponto dos pés (posição física `_-si`/`_-94q`) e ponto da cabeça. O centro usado
  pelos golpes (`_-CG`/`_-RI`) é pés − metade da altura da hurtbox (`_-F11._-g40`).
- Varredura `_-B3Y._-b3i` (1551): superfícies têm normal fixa (`_-r2g`) e bloqueiam **só quem se move contra a normal**;
  plataformas finas (tipo 2) exigem partir do lado da normal. Recuo de 1.01 px ao bater; sonda de 2.02 px para baixo no chão.
- Normais calculadas ao carregar o mapa (`_-93X._-s1F`, 5110): paridade de raio a partir do ponto médio (ajustado a uma grade
  de 10): ímpar = interior daquele lado. `NormalX/NormalY` do XML têm prioridade (`_-G1F._-F2T`, 5183).
- No chão o lutador anda **ao longo da superfície** e troca para a seguinte ao encontrá-la; no ar varre pés e cabeça com
  (vx·dt, vy·dt), usa a varredura mais curta e, se a linha do corpo cruzar uma superfície dura, refaz só com o y.
- Tick (`_-F11._-I30`): no chão vy = 0; **atrito sempre** (`_-35J` nunca é ligada), depois aceleração se há direção: ganho
  líquido = (Acceleration − Friction)·dt. No ar, virar contra a velocidade multiplica a aceleração por max(1 − |vx|/50, 0).
- Gravidade: 3.75·dt enquanto vy < limite, mais `_-T4w(vy)` = 3.25 − (3.25/48)·|vy| para |vy| ≤ 48 (fora do hitstun).
  Limite 70; 85 em queda rápida ou segurando ↓. Queda rápida é um estado (liga com ↓ no ar e vy ≥ −38, sem poder ativo;
  soma 12·dt). Acima de 70 decai 6·dt. Suspensa com `CancelGravity`, `AntigravTime` ou esquiva.
- Pulo (`_-fc`, chamado por `_-K2c._-p3j`): impulso 57 no chão e no ar (2 pulos no ar); com direção apertada cancela a
  velocidade contrária ou soma JumpXImpulse/2 se já está no máximo. Pulo de parede: 53 para cima e 48 para o lado.
- Parede (`_-n2N`: 1 = à esquerda, 2 = à direita): agarra se a cabeça também encosta; no primeiro contato vy = 0 e 112
  unidades de tempo sem gravidade; depois desliza devolvendo 85% da gravidade; mantém ±10 contra a parede; devolve os pulos
  no ar. Soltar (`_-tE`): direção oposta → impulso 30 para longe. Encaixe de borda: cabeça acima da quina, pés a menos de
  60 px do topo e vy > −35.
- Hitstun (`_-k1X`): atrito 4.0; ao bater quica (`_-jT`) com elasticidade 0.8 se a velocidade ≥ 25.
- Flags (setters): `_-i1g` → `p1F` = no ar; `_-Z1p` → `k1X` = hitstun; `_-45j` → `m2O` = direção apertada; `_-n5X` → `a30` =
  olhando para a esquerda; `_-O3w` → `r5Q` = esquiva/dash.

## Execução dos poderes (complemento)
- Tempo dos eventos (`_-6z._-T6n`/`_-e58`): 1º evento em início + A; cada evento seguinte avança max(1, atraso) ticks.
- `FireImpulseX/Y` (`_-6z._-A6b`, 47): **define** vx/vy do atacante em cada evento (x espelhado); `!` = não definido.
  `CastImpulseX/Y` e `SpeedLimit*` (`_-6z._-S2F`, 91) agem no início do poder.
- Encadeamento (`_-Z1a._-X1Y`, 1699): automático. Ao acertar entra `ComboOverrideIfHit`; ao fim dos eventos entra
  `ComboName`; sem próximo, vale `RecoverTime`. Cada poder tem sua própria lista de alvos atingidos.
- Recuperação = floor(RecoverTime ÷ modificador da Destreza) + FixedRecoverTime (o leitor de StatType guarda 1/valor para
  `RecoverMod`, `AirRecoverMod`, `SigRecoverMod`, `MinChargeMod`).
- Animação do poder (`_-6z._-61u`, 86): quadro interpolado linearmente entre os marcadores `@N` do `CastTime` e o do
  `RecoverTime`.
- Golpes pesados: `TargetMethod Smash` carrega entre `FixedMinChargeTime ÷ MinChargeMod` e o máximo (2º segmento do
  `CastTime`); ao soltar entra o `ComboName` (`SmashRelease`), com dano e impulso variável × (1 + 0.25 × carga/máximo).
- Esquiva (`DodgeTypes`, classe `_-36I`): velocidade = 60 × fator `StandardHorizontal` (0.6..1.0) × multiplicador entre
  `Speed*Mult` e `Speed*MaxMult`; sem gravidade nem atrito; invulnerável a partir de `StartInvuln`; recargas
  `Cooldown`/`CooldownGround`/`CooldownLanded`.

## Limitações do decompilador (`tools/decomp.py`)
Ternários com `null`/constantes empurradas antes (p.ex. `r37[11 + (x is F ? x.m1H : 0) − …]`) perdem o `11`: conferir no bytecode cru com `tools/abc.py dis`.
`v++` aparece antes do `getlocal` que o consome (laços são 0-based). Predicados `a30()`, `r5Q()`, `p1F()`... são bits ofuscados.

## Ainda não lido
`_-R4X` (teste elipse×corpo exato), dash e esquivas rápidas (`Fast*`), influência de direção (DI) no knockback, golpes de
agarrão (`SmashReleaseGrab`, `HoldHitEnts`), projéteis, hurtbox por quadro do atacante, segunda arma e itens, condições de
combo por botão/direção.
