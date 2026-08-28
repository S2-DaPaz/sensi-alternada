# Motor de DPI — design

Data: 2026-08-28. Substitui `2026-08-27-escalar-cursor-design-abandonado.md`, que fica no
repositório porque a medição que o derrubou é o motivo deste existir.

## O que mudou, e por quê

O desenho anterior escalava o movimento do ponteiro do Windows. Ficou com **erro de 0 px**,
medido, e mesmo assim a sensibilidade dentro do jogo **não mudava**.

Dois fatos fecharam o caso:

| Experimento | Sensibilidade no jogo |
|---|---|
| Escalar o ponteiro por software, com precisão exata | **não muda** |
| Trocar a DPI pelo software do mouse | **muda** |

O modo de tiro do BlueStacks lê as **contagens cruas do dispositivo**, não o cursor. Nenhum
truque em user-mode alcança isso — é a camada errada, não um defeito de implementação.

## Por que não é driver

Alterar as contagens **no caminho** exigiria um driver de filtro, como o RawAccel. Duas
razões para não ir por lá:

- O driver do RawAccel não pode ser dirigido: `rawaccel-io-def.h` define três IOCTLs
  (`READ`, `WRITE`, `GET_VERSION`) e o `case WRITE` chama `WriteDelay()` **antes de ler o
  buffer**, com `WRITE_DELAY = 1000` ms. Um segundo por escrita, no kernel, de propósito. E
  o callback dele nunca olha `ButtonFlags` — não existe noção de gatilho.
- Um driver próprio precisaria de assinatura. Na prática, `testsigning` ligado, que derruba
  a verificação de assinatura da máquina inteira.

## O caminho certo não precisa de driver nenhum

**Mandar comando ao mouse é HID de user-mode.** Driver só é necessário para *interceptar* o
que o mouse manda. O software do fabricante troca DPI escrevendo um relatório HID numa
coleção de fornecedor — no Logitech, o protocolo **HID++**, em `usage page 0xFF00`.

Trocar a DPI muda as contagens no firmware, antes de qualquer camada de software. Por isso
chega ao jogo.

## Mecânica

- `Mouse::find()` enumera as coleções `0xFF00/0x02` de dispositivos Logitech e pergunta ao
  root feature qual é o índice de `0x2201` (*Adjustable DPI*). O primeiro que responder é o
  mouse.
- O hook `WH_MOUSE_LL` **só vê o botão passar**. Não toca em movimento, não suprime nada.
- Na transição do gatilho, o hook **posta** `WM_APP_SET_DPI` para o laço de mensagens. A
  escrita HID leva ~4 ms; dentro do callback ela atrasaria cada clique.
- Desligar, ou perder o foco, restaura a DPI base.

### Medições, num G203

| | |
|---|---|
| Feature `0x2201` | índice 10, device `0xFF` |
| `setSensorDpi`, 10 trocas | média **3,74 ms** · mediana 3,78 ms · pior 4,13 ms · **0 erros** |
| Ponta a ponta, lido do mouse | 600 antes · **400** com o gatilho preso · **800** ao soltar |

### Armadilhas do protocolo, ambas medidas

- **A coleção curta (`0xFF00/0x01`, report `0x10`) aceita a escrita e nunca responde.** Só
  a longa fala. Não há erro — só ausência.
- **Casar resposta pelo `swId` defasa a conversa em um pedido, para sempre.** O `swId` é
  constante, então toda resposta casa com qualquer pedido; com uma resposta velha no buffer,
  o pedido N recebe a resposta N−1. O sintoma foi uma DPI de **1**, valor impossível, que
  quase virou escrita inválida no mouse. A correção é drenar antes de escrever e casar por
  **feature + byte de função inteiro**.

## Limites conhecidos

- **Só Logitech.** HID++ é proprietário. Outras marcas exigiriam o protocolo delas.
- **DPI por eixo X/Y só com a feature `0x2202`**, que o G203 não tem. O painel detecta,
  desabilita a caixa e diz o porquê.
- **O G HUB pode reimpor a DPI dele** ao trocar de perfil. Não aconteceu nos testes, mas a
  DPI lida no arranque veio como 600 em vez do 800 que havia sido deixado.

## Fora de escopo, por decisão

Compensação de recuo e autofire.
