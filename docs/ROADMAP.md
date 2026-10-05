# Roadmap

Estado em 2026-10-04. Tamanhos são estimativas relativas (P, M, G), não prazos.

## Feito

- Leitura dos arquivos do jogo: `.swz`, `.anm`, SWF vetorial, mapas, poderes, esquivas, atributos.
- Visualizadores: mapas, símbolos e personagem animado.
- Simulação (`bh-sim`): movimento e colisão do tick real, paredes, golpes leves e pesados, esquiva, dano, knockback, hitstun, KO.
- Duelo local (`--play`): P1 contra bot, alvo parado ou segundo jogador, com HUD.

## Fase 0 — Validar o que existe (P)

Nada do que está abaixo vale a pena antes disto.

- [ ] Jogar de verdade com teclado e gamepad e anotar o que parece errado.
- [ ] Medir desempenho (FPS, custo de tesselação) e, se preciso, reduzir triângulos ou guardar malhas em cache.
- [ ] Comparar com o jogo original lado a lado: altura do pulo, tempo de corrida, alcance e dano de 3 ou 4 golpes.
- [ ] `git init` e primeiro commit (o projeto ainda não está versionado).

## Fase 1 — Fidelidade do combate (G)

- [ ] Teste exato de acerto (`_-R4X`) no lugar da aproximação elipse × caixa.
- [ ] Hurtbox por quadro do atacante (`hurtboxTypes` já tem os dados).
- [ ] Influência de direção do alvo no knockback (DI).
- [ ] Agarrões (`SmashReleaseGrab`, `HoldHitEnts`): o pesado para baixo do Bödvar depende disso.
- [ ] Condições de combo por botão, direção e soltura (`ComboOverrideIf*`).
- [ ] Golpes aéreos pesados e recuperação conferidos contra os dados, em vez de nomes deduzidos.
- [ ] Gravity cancel e prioridade entre golpes que se cruzam.

## Fase 2 — Movimento que falta (M)

- [ ] Dash e esquivas rápidas (`Fast*`), incluindo dash-jump (impulso 170 já localizado no bytecode).
- [ ] Rampas: sonda de 50 px e deslizar por baixo de superfícies inclinadas.
- [ ] Condições exatas de queda rápida e de agarrar parede (hoje simplificadas).
- [ ] Animações de transição: `RunStart`, `RunStop`, `RunTurn`, `JumpStart`.

## Fase 3 — Armas e itens (G)

- [ ] Segunda arma do herói (espada do Bödvar) e desarmado.
- [ ] Itens no mapa: surgimento, pegar, arremessar (`itemTypes`, `ItemSpawn`).
- [ ] Projéteis e poderes que não são de área em volta do lutador.

## Fase 4 — Visual (M)

- [ ] Paletas de cor dos trajes (`ColorSwap`, `ColorSchemeTypes`).
- [ ] Máscaras (clip depth) e sub-sprites animados.
- [ ] Efeitos de golpe (`CastGfx`, `FireGfx`, `HitGfx`) e poeira de pulo.
- [ ] Fundos animados e plataformas móveis dos mapas.

## Fase 5 — Conteúdo (G, repetitivo)

- [ ] Generalizar para qualquer herói e arma; hoje só Bödvar com martelo foi testado.
- [ ] Varredura automática: carregar todos os heróis, armas e mapas e reportar o que falha.
- [ ] Mapas com mecânicas próprias (lava, plataformas que quicam, placas de pressão).

## Fase 6 — Partida completa (M)

- [ ] Vidas, tempo, fim de partida e tela de resultado.
- [ ] Seleção de herói, traje, arma e mapa.
- [ ] Mais de dois jogadores e times.
- [ ] Modos de jogo (`GameModeTypes`).

## Fase 7 — Áudio (M, risco alto)

- [ ] Extrair e tocar os `.wem` (Wwise) e mapear eventos dos poderes (`CastSoundEvent`, `HitSoundEvent`).
- [ ] Música dos mapas.

## Fase 8 — Bot e acabamento (M)

- [ ] Bot mais equilibrado, com níveis de dificuldade.
- [ ] Interpolação visual entre ticks e opção de remapear controles.
- [ ] Gravação e reprodução de partidas (a simulação já é independente do motor gráfico).

## Riscos

- **Bytecode ofuscado:** cada mecânica nova exige leitura; os nomes mudam a cada versão do jogo.
- **Atualizações do jogo:** a chave dos `.swz` e os nomes ofuscados são da versão 10.11. Um patch pode quebrar leitores e notas.
- **Áudio Wwise:** formato fechado; pode exigir ferramenta externa.
- **Escopo:** são dezenas de heróis e milhares de poderes. A Fase 5 só é viável se for dirigida por dados, com varredura automática.

## Ordem sugerida

Fase 0 → 1 → 2 → 3, com a Fase 4 em paralelo quando o combate cansar. Fases 5 a 8 dependem das anteriores.
