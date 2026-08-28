# sensi-alternada

Painel Win32 que escala o movimento do mouse enquanto o botão de tiro está pressionado,
para jogar no BlueStacks com qualquer mouse. Rust + `eframe`/`egui`, um `.exe`.

## Fronteira do escopo — não negociável

Compensação de recuo e autofire ficam **fora**. Ajustar sensibilidade não automatiza ação
de jogo e keymapping em emulador oficial é permitido pela Garena; recuo automático e
disparo em laço são macro, observáveis dentro do jogo, e são banimento. Não acrescentar
`MoveMouseRelative` de recuo nem `PressMouseButton` em laço.

## Onde mexer

| Quero mudar | Arquivo |
|---|---|
| como o fator é calculado, ou o acumulador de resto | `src/scaling.rs` — **tem testes** |
| qual botão dispara, e como o evento é lido | `src/fire_button.rs` — **tem testes** |
| o que é salvo e onde | `src/config.rs` — **tem testes** |
| supressão e reinjeção | `src/hook.rs` |
| contraste de texto sobre uma cor | `src/theme.rs` — **tem testes** |
| qual executável conta como "em foco" | `src/foreground.rs` (`TARGET_EXE`) |
| o painel, e como as cores viram estilo | `src/app.rs` |

`scaling.rs`, `fire_button.rs`, `config.rs` e `theme.rs` são lógica pura e rodam sem
Windows. **Mudança neles entra por teste primeiro** — os três bugs mais caros até agora
apareceram assim: `inf` que arremessa o cursor, fator `0,0` que congela a mira, e cor
`[0,0,0]` que apagaria o painel de quem já tinha configuração salva.

## Três armadilhas do eframe/egui já pagas

- **`clear_color` ignora `panel_fill`.** O default do eframe é um cinza fixo com alpha 180.
  Sem sobrescrever, a cor de fundo escolhida nunca aparece e a janela fica translúcida.
- **`override_text_color` não alcança `strong()`.** Texto forte lê o traço do widget
  ativo; sem ajustar `widgets.*.fg_stroke`, o título sai branco sobre fundo claro.
- **Campo novo em `Settings` precisa de `#[serde(default = "...")]` nomeado.** O
  `#[serde(default)]` simples devolve zero, e zero numa cor é **preto no preto**.

## Duas coisas que parecem detalhe e não são

- A injeção é **absoluta**. Trocar por relativa faz o Windows aplicar a velocidade de
  ponteiro por cima do nosso fator, e o número do painel deixa de significar o que diz.
- A marca em `dwExtraInfo` (`TAG`) é o que impede o hook de reprocessar a própria injeção.
  Sem ela o cursor foge sozinho.

## Se o comportamento em jogo estiver errado

**Mira dobrada ou trêmula** tem uma causa só: o BlueStacks estaria lendo raw input, não o
cursor. Suprimir no hook não cega o raw input — medido, ver
`docs/2026-08-27-sensibilidade-alternada-design.md`. Não há conserto em user-mode.

## QA

`cargo test` (14 testes) e `cargo fmt`. O painel se verifica rodando e capturando a
janela; capturar com `PrintWindow(hwnd, hdc, PW_RENDERFULLCONTENT)` e o retângulo de
`DwmGetWindowAttribute(9)`. `CopyFromScreen` fotografa o que está por cima e
`GetWindowRect` inclui a borda invisível do Win10 — os dois já produziram captura errada
aqui.
