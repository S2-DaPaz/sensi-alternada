# Sensibilidade alternada — design

Data: 2026-08-27

## Problema

Jogar Free Fire no BlueStacks MSI 5 com o mouse numa sensibilidade para movimentação e
noutra, mais baixa, **enquanto o botão de tiro está pressionado** — a "sensibilidade
alternada" dos mapeadores comerciais. Precisa funcionar com **qualquer mouse**, não só
com o Logitech G203 do dono.

## Por que não é DPI

Trocar DPI é proprietário de cada fabricante — Logitech tem Lua no G HUB, Razer tem
Synapse, mouse genérico não tem nada. O requisito "qualquer marca" mata esse caminho e
sobra **escalar o movimento em software**.

O RawAccel prova que a camada funciona (driver de filtro acima do `mouclass.sys`), mas
não serve de motor: a FAQ oficial diz que ele tem *"a one-second delay on write, so it
cannot be used to cheat"*. Um segundo por escrita; segurar o gatilho precisa de
milissegundos.

## Premissa herdada

**O dono pesquisou que o modo de tiro do BlueStacks lê o mouse pelo cursor do Windows.**
Isto é pesquisa dele, não medição deste projeto — e o desenho inteiro depende disso.

`HD-Player.exe` importa os dois caminhos (`RegisterRawInputDevices` + `GetRawInputData`
e `ClipCursor`/`SetCursorPos`/`GetCursorPos`), então leitura estática não decide.

**Sintoma se a premissa estiver errada:** mira **dobrada ou trêmula**, não "não
funciona". Medido em 26/08: suprimir no `WH_MOUSE_LL` não cega o raw input — 300
movimentos engolidos, cursor andou 0 px, raw input entregou os 300. Se o BlueStacks ler
raw input, ele recebe o movimento físico **somado** ao nosso escalado.

**Plano B, se acontecer:** driver de interceptação (Interception), com a mesma lógica em
user-mode.

## Mecânica

Hook `WH_MOUSE_LL` numa thread dedicada com laço de mensagens.

Para cada `WM_MOUSEMOVE`:

1. `dwExtraInfo == MARCA` → é injeção nossa. Repassa e atualiza `last_pos`.
2. Desligado, ou BlueStacks fora de foco, ou botão de tiro solto → repassa e atualiza
   `last_pos`.
3. Caso contrário → **suprime** (retorna 1), escala e reinjeta.

O delta físico é `pt - last_pos`, onde `last_pos` é onde o cursor realmente está —
atualizado por nós a cada injeção. A reinjeção é **absoluta**
(`MOUSEEVENTF_ABSOLUTE | MOUSEEVENTF_MOVE | MOUSEEVENTF_VIRTUALDESK`), que ignora a
velocidade de ponteiro e a aceleração do Windows: sem isso o fator seria aplicado duas
vezes, uma pela gente e outra pelo SO.

A marca em `dwExtraInfo` não é refinamento: sem ela o hook reprocessa a própria injeção
e realimenta.

O estado do botão de tiro é rastreado **dentro do hook** (`WM_LBUTTONDOWN`/`UP`,
`WM_RBUTTONDOWN/UP`, `WM_MBUTTONDOWN/UP`, `WM_XBUTTONDOWN/UP` com `mouseData`), e não
por `GetAsyncKeyState` — é exato e não sofre com troca de botões primário/secundário.

### Acumulador de resto

`dx`/`dy` são inteiros. Multiplicar por 0,5 e truncar transforma uma sequência de +1 em
uma sequência de zeros: **a mira trava no movimento lento**, que é exatamente onde a
precisão importa. O resto fracionário é acumulado entre eventos, por eixo, e some ao
próximo. O acumulador **zera** ao soltar o gatilho, para o primeiro evento do próximo
tiro não carregar sobra velha.

### Foco

`GetForegroundWindow` a cada movimento é caro demais para o caminho quente. Uma thread
separada resolve o executável em foco a cada 200 ms e publica num `AtomicBool`.

## Painel

```
┌─────────────────────────────────────┐
│  Sensibilidade alternada            │
│  A mira muda enquanto o gatilho...  │
│                                     │
│  ☑ Separar eixos X e Y              │
│  DPI base — X       [   800   ]     │
│  DPI base — Y       [  1200   ]     │
│  DPI atirando — X   [   400   ]     │
│  DPI atirando — Y   [   300   ]     │
│                                     │
│  Botão de tiro      [ Esquerdo ▾]   │
│  Cores       [Padrão]  [■]  [■]     │
│  Fator  X 0.50×  Y 0.25×            │
│                                     │
│  [          ATIVAR          ]       │
│  ● Desligado                        │
│  Ctrl+Alt+S liga/desliga            │
└─────────────────────────────────────┘
```

- `fator = DPI atirando ÷ DPI base`, **por eixo**. A caixa separa X e Y nos dois campos,
  base e tiro. Desmarcada, os campos Y espelham os X, para que marcar a caixa comece da
  paridade em vez de um número que o usuário nunca viu.
- Seletor de botão: esquerdo, direito, meio, lateral 1, lateral 2.
- **Duas cores**: uma para os botões e as letras, outra para o fundo. Todo o resto é
  mistura das duas, então nenhuma escolha desmonta o painel. A exceção é o texto *dentro*
  do botão: ele é derivado do brilho da cor escolhida com os pesos do sRGB, senão um botão
  claro receberia letra clara. Média simples não serve — ela chama verde saturado de
  escuro.
- Atalho global `Ctrl+Alt+S`, porque com o jogo em tela cheia não dá para clicar.
- A janela muda de altura conforme a caixa, para não sobrar espaço morto.
- Configuração salva em JSON no perfil do usuário, carregada no arranque. Campo novo entra
  com `#[serde(default = "...")]` **nomeado**: o default simples devolve zero, e zero numa
  cor é preto no preto — apagaria o painel de quem já tinha configuração salva.

## Stack

Rust + `eframe`/`egui`. Um `.exe`, sem instalador, sem Node, sem runtime. O código de
input é Win32 nativo pela crate `windows` — mesmo terreno do spike de 26/08, cuja
latência medida foi 0,023 ms de p95 no hook, contra um orçamento de 8 ms.

## Módulos

| Arquivo | Papel | Testável sem Windows |
|---|---|---|
| `scaling.rs` | fator por DPI + acumulador de resto | **sim** — é onde mora o TDD |
| `theme.rs` | contraste: que cor de texto sobrevive a um fundo | sim |
| `config.rs` | serialização, migração de campo novo, caminho do JSON | sim |
| `hook.rs` | hook de baixo nível, supressão, reinjeção | não |
| `foreground.rs` | executável em foco, em cache | não |
| `app.rs` | painel `egui` | não |

## Fora de escopo, por decisão

Compensação de recuo e autofire. Ajustar sensibilidade não automatiza ação nenhuma e
keymapping em emulador oficial é permitido pela Garena com cliente não modificado; recuo
automático e autofire são macro, são observáveis dentro do jogo, e são banimento.

## Limitação conhecida do v1

Se o BlueStacks prender o cursor na borda da janela em vez de recentralizar, movimento
longo pode perder curso. A correção é ler o delta cru do raw input em vez da posição do
cursor — desenho de duas APIs já validado em 26/08, evolução conhecida.
