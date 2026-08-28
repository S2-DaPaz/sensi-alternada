# sensi-alternada

Painel que troca a **DPI do mouse** enquanto o gatilho está pressionado, falando HID++ com
o dispositivo. Rust + `eframe`/`egui`, um `.exe`, sem driver.

## Fronteira do escopo — não negociável

Compensação de recuo e autofire ficam **fora**. Trocar DPI não automatiza ação de jogo e
keymapping em emulador oficial é permitido pela Garena; recuo automático e disparo em laço
são macro, observáveis dentro do jogo, e são banimento.

## Onde mexer

| Quero mudar | Arquivo |
|---|---|
| a lista de marcas e seus rótulos | `src/brand.rs` — **tem testes** |
| qual motor atende qual marca | `src/engine.rs` |
| achar a coleção HID de um dispositivo | `src/hid.rs` |
| enquadramento das mensagens HID++ | `src/hidpp.rs` — **tem testes** |
| enquadramento do protocolo SINOWEALTH | `src/sinowealth.rs` — **tem testes** |
| conversa com mouse SINOWEALTH | `src/sinowealth_mouse.rs` — ⚠ sem hardware |
| abrir o mouse, ler e escrever | `src/mouse.rs` |
| qual botão dispara, e como o evento é lido | `src/fire_button.rs` — **tem testes** |
| o que é salvo e onde | `src/config.rs` — **tem testes** |
| contraste de texto sobre uma cor | `src/theme.rs` — **tem testes** |
| quando a troca vale | `src/hook.rs`, `src/foreground.rs` (`DEFAULT_TARGET_EXE`) |
| o painel | `src/app.rs` |

`hidpp.rs`, `sinowealth.rs`, `brand.rs`, `fire_button.rs`, `config.rs` e `theme.rs` são
lógica pura e rodam sem hardware. **Mudança neles entra por teste primeiro.**

## Acrescentar uma marca

Variante em `brand.rs`, módulo de protocolo puro (testável), módulo de dispositivo usando
`hid.rs`, e um braço em `Engine::for_brand`.

**Procure sempre o comando de seleção — perfil, slot, índice — nunca o de escrita de
configuração.** No SINOWEALTH, mudar o slot de DPI ativo escreveria 520 bytes na memória
persistente do mouse; a cada tiro isso gastaria os ciclos de gravação do dispositivo. O
motor recusa quando só há um perfil, e essa recusa é deliberada.

O motor recebe **nível** (`Level::Base` ou `Level::Atirando`), não DPI crua: nem toda marca
define DPI arbitrária. Quem sabe o que "atirando" significa é o motor.

## O que já custou caro aqui

- **Escalar o cursor não funciona.** Foi o desenho original, ficou com precisão exata, e o
  jogo ignorava. O BlueStacks lê contagem crua do dispositivo. Não voltar para lá.
- **Casar resposta HID++ só pelo `swId` defasa a conversa em um pedido, para sempre.** O
  `swId` é constante, então toda resposta casa com qualquer pedido. `request()` drena o
  buffer antes de escrever e casa por **feature + byte de função inteiro**.
- **A coleção HID++ curta (`0xFF00/0x01`) aceita a escrita e nunca responde.** Só a longa
  (`0xFF00/0x02`, report `0x11`) fala. Falha silenciosa: não há erro, só ausência.
- **A troca de DPI não roda dentro do callback do hook.** São ~4 ms; ali dentro atrasaria
  cada clique. Ela é postada para o laço de mensagens.
- **`clear_color` do eframe ignora `panel_fill`**, e **`override_text_color` não alcança
  `strong()`**. As duas cores precisam ser forçadas à mão.
- **Campo novo em `Settings` precisa de `#[serde(default)]`.** Sem ele a desserialização
  inteira falha e o `unwrap_or_default()` apaga **toda** a configuração de quem já usava o
  programa — medido: acrescentar `brand` sem default quebrou dois testes de migração de uma
  vez. Para cor, o default precisa ser **nomeado**: o simples devolve zero, e zero numa cor
  é preto no preto.
- **A thread do motor sobe antes do painel.** Ela monta o motor com a marca padrão, então
  `App::new` precisa chamar `hook::request_brand_change()` — senão a marca salva em disco é
  ignorada e o painel mostra um motor que não é o escolhido.

## Como diagnosticar

`SENSI_DEBUG=1` escreve o estado em `%TEMP%\sensi-debug.txt`: alvo, foco, mouse encontrado,
suporte a eixo, gatilho, DPIs e a última mensagem do motor. `SENSI_TARGET_EXE=*` desliga o
portão de foco, que é como se mede sem depender de quem está em primeiro plano.

A verdade sobre a DPI é **lida do mouse**, não do que o app diz ter feito.

## QA

`cargo test` (20 testes) e `cargo fmt`. O painel se verifica rodando e capturando a janela
com `PrintWindow(hwnd, hdc, PW_RENDERFULLCONTENT)` e o retângulo de
`DwmGetWindowAttribute(9)` — `CopyFromScreen` fotografa o que está por cima.
