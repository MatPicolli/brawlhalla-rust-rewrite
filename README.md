# Brawlhalla recomp (Rust + Bevy)

Reimplementação local e offline do Brawlhalla. Os assets **não** são distribuídos:
tudo é lido da sua instalação (`$BRAWLHALLA_DIR` ou `D:\Steam\steamapps\common\Brawlhalla`).

## Estrutura

- `crates/bh-assets` — leitura dos formatos do jogo (SWZ, SWF/ABC, `.anm`/`.bin` zlib)
- `crates/bh-cli` — binário `bhx`

## Uso

```
cargo run -p bh-cli -- key                 # acha a chave e conta as seções dos .swz
cargo run -p bh-cli -- swz-dump cache/swz  # decifra os dados de jogo (XML/CSV)
cargo run -p bh-cli -- unz anims/Animation_Aang.anm out.bin
```

```
cargo run -p bh-cli -- anm-check                      # valida o parser em todos os anims/*.anm
cargo run -p bh-cli -- anm-info Animation_Aang.anm
cargo run -p bh-viewer                      # visualizador de mapas (Bevy)
cargo run -p bh-viewer -- --level Brawlhaven --screenshot cache/shot.png
```

**Jogar** (duelo local contra um bot, um alvo parado ou um segundo jogador):

```
cargo run -p bh-viewer -- --play                                   # Bödvar + martelo no Brawlhaven, P2 = bot
cargo run -p bh-viewer -- --play --level Stadium --char Viking --weapon Hammer
cargo run -p bh-viewer -- --play --demo script --screenshot cache/x.png --shot-tick 300   # P1 roteirizado, para verificar
cargo run -p bh-viewer -- --play --demo bots                       # dois bots lutando
cargo test --workspace                                             # 60 testes (leitores + simulação)
cargo run -p bh-sim --example measure                              # velocidade, altura de pulo etc.
cargo run -p bh-sim --example combat                               # dano e knockback com os dados reais
```

- **P1:** A/D andar · W ou Espaço pular (chão + 2 no ar; W também vale como "cima") · S baixo (queda rápida, atravessar
  plataforma fina) · J golpe leve · K golpe pesado (segure para carregar) · L esquiva.
- **P2 humano:** ←/→ · ↑ pular · ↓ baixo · `,` leve · `.` pesado · `/` esquiva (ou 1, 2, 3 do teclado numérico).
- **Gerais:** P alterna o P2 (bot / parado / humano) · R reinicia · C liga/desliga colisões, normais, hurtboxes e hitboxes ·
  Esc sai.
- **Gamepads:** o primeiro controla P1 e o segundo P2 (analógico, A pula, X leve, B pesado, gatilhos esquivam).

Golpe com direção: lateral com ←/→, para baixo com S; no ar usa as versões aéreas. As cadeias (por exemplo os 3 golpes do
leve neutro do martelo) são automáticas. Encostar numa parede no ar agarra; a direção oposta solta; pular dá o pulo de parede.

> Use o perfil normal (sem `--release`): o `--release` recompila o Bevy inteiro (~15 min).
> A simulação (`bh-sim`) segue as fórmulas e constantes lidas do bytecode; ver `docs/bytecode-notes.md`.

Personagem animado e símbolos soltos:

```
cargo run -p bh-viewer -- --char Viking --weapon Hammer --anim Run
cargo run -p bh-viewer -- --char Viking --weapon Hammer --anim AttackCombo1 --frame 6 --screenshot cache/x.png
cargo run -p bh-viewer -- --symbol Gfx_Viking.swf a_Torso1_Viking
cargo run -p bh-cli --release -- swf-check        # tessela todas as formas dos Gfx_/Bones_ (110k formas, 0 erros)
cargo run -p bh-cli -- anm-frame Animation_Hammer.anm Run 3
```

Modo `--char`: ← → trocam de animação, Espaço pausa, `,` `.` passo a passo, ↑ ↓ velocidade.

Viewer de mapas: ← → trocam de mapa, roda = zoom, arrastar = mover, `C` colisões, `B` fundo, `Esc` sai.

`tools/abc.py` (desassemblador AVM2), `tools/decomp.py` (pseudo-decompilador de pilha) e `tools/grepdec.py`
(regex sobre todos os métodos) servem para ler o bytecode do `BrawlhallaAir.swf`. Notas em `docs/bytecode-notes.md`.
`cargo run -p bh-sim --example measure` imprime velocidade, altura de pulo etc. nas unidades reais.

## O que já sabemos do jogo (versão 10.11)

| Item | Formato |
|---|---|
| `Engine/Init/Game/Dynamic.swz` | XOR + PRNG WELL512; chave é uma constante `uint` no ABC de `BrawlhallaAir.swf` |
| Dados de gameplay | `Game.swz`: HeroTypes, powerTypes (golpes, em frames), hurtboxTypes, StatTypes (velocidade/atrito), spriteData |
| Mapas | `Dynamic.swz`: `LevelDesc` com plataformas, colisões, spawns, câmera |
| `anims/*.anm` | `[u32 LE tamanho][zlib]`; keyframes de partes (índice em `BoneTypes`, matriz 2x3, opacidade, frame do sprite). Parser em `bh-assets/src/anm.rs`, validado nos 104 arquivos (6947 animações, 229.755 quadros). Semântica dos 5 últimos `u32` do cabeçalho da animação ainda pendente |
| `languages/*.bin` | `[u32 LE tamanho][zlib]`; strings `chave\0valor\0` |
| Arte dos personagens | **vetorial** em `Gfx_*.swf` (pele, por traje) e `Bones_*.swf` (marcadores vazios do esqueleto). Renderizada por `shape.rs` (parse de DefineShape + costura de arestas + `lyon`) e `movie.rs` (sprites/display list) |
| Índice de osso no `.anm` | indexa `BoneTypes` do `Init.swz` com **deslocamento +1** (a tabela do jogo começa com `UNKNOWN`) |
| Resolução da pele | traje (`costumeTypes`, colunas `GfxType.CustomArt*`) e skin de arma (`weaponSkinTypes`) são listas de camadas `arquivo.swf/sufixo`; o símbolo do osso `B` é `B_sufixo`, a última camada que o tiver vence; `…Right` sem arte reaproveita o osso sem o sufixo; `…R` e ossos sem símbolo são ignorados |
| Arte dos mapas | PNG/JPG em `mapArt/<AssetDir>/`; plataformas aninhadas com transformação em `LevelDesc` |
| Lógica de física | bytecode AS3 ofuscado em `BrawlhallaAir.swf` (ainda não analisado) |

## Roadmap

1. [x] Decifrar `.swz`
2. [x] Parser do `.anm`
3. [x] Viewer de mapas em Bevy (arte, colisões, respawns)
3b. [x] Renderizar shapes vetoriais SWF e animar um legend com o `.anm` (Bödvar + martelo verificado)
3c. [ ] Pendências visuais: máscaras (clip depth), esquemas de cor (`ColorSwap`), sub-sprites animados, espelhar para virar o personagem
4. [x] Fatia jogável: duelo local (bot, alvo parado ou 2 jogadores) com movimento, colisão, golpes leves e pesados,
   esquiva, parede, dano, knockback, hitstun, KO pelas zonas reais do mapa e HUD.
4b. [ ] Ainda fora: dash e esquivas rápidas, DI, agarrões, projéteis, itens/segunda arma, hurtbox por quadro, máscaras e
   paletas de cor, áudio, menus.
5. [ ] Mais legends/armas, menus, áudio (Wwise)
