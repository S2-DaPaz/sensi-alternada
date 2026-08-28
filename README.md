# Sensibilidade alternada

Troca a **DPI do mouse** enquanto o botão de tiro está pressionado, para jogar no
BlueStacks com uma sensibilidade para se movimentar e outra, mais precisa, para atirar.

Sem driver, sem instalar nada no kernel, sem depender do software do fabricante estar
aberto.

![painel](docs/painel.png)

## Requisito: mouse Logitech com DPI programável

O painel fala **HID++**, o protocolo que o próprio G HUB usa para conversar com o mouse.
Isso significa Logitech. Razer, Corsair e outros têm protocolo próprio, ainda não
implementado aqui, e mouse sem DPI programável não tem caminho por software nenhum.

Se o seu mouse não for encontrado, o painel diz isso e o botão ATIVAR fica desligado.

## Baixar

[**Última versão**](https://github.com/S2-DaPaz/sensi-alternada/releases/latest) — um `.exe`,
sem instalador, Windows 10/11 x64. O binário não é assinado, então o SmartScreen avisa no
primeiro arranque: **Mais informações → Executar assim mesmo**. Quem preferir compila do
fonte e compara o hash publicado na release.

## Como usar

1. Rode o `.exe`, ou compile com `cargo build --release`.
2. **DPI base** — a DPI que você quer para se movimentar.
3. **DPI atirando** — a DPI enquanto o gatilho está pressionado. Menor = mais precisão.
4. **Botão de tiro** — esquerdo, direito, meio, lateral 1 ou lateral 2.
5. **Cores** — dois seletores: um para os botões e as letras, outro para o fundo.
6. **ATIVAR**, ou `Ctrl + Alt + S` a qualquer momento, inclusive com o jogo em tela cheia.

A troca só acontece com o **BlueStacks em foco**. No resto do Windows o mouse fica na DPI
base. Desligar restaura a base imediatamente, mesmo com o gatilho pressionado.

## Por que DPI, e não escalar o movimento

A primeira versão deste projeto escalava o ponteiro do Windows. Funcionava com precisão
exata — erro de 0 px, medido — e **o jogo não percebia nada**.

O motivo, medido em 28/08/2026: o modo de tiro do BlueStacks lê as **contagens cruas do
dispositivo**, não o cursor. A prova é direta — trocar a DPI pelo software do mouse muda a
sensibilidade no jogo; escalar o cursor não muda nada. Nenhum truque em user-mode alcança
isso.

Trocar a DPI muda as contagens no firmware, antes de qualquer camada de software. Por isso
chega ao jogo.

E não precisa de driver: mandar comando **ao** mouse é HID comum, de user-mode. Driver só
seria necessário para **interceptar** o que o mouse manda — que é o que o RawAccel faz, e
que o RawAccel não permite reaproveitar: ele tem, por design, **1000 ms de atraso em toda
escrita de configuração** (`WRITE_DELAY` em `rawaccel-base.hpp`).

Medido neste projeto, num G203: **3,74 ms de média** por troca de DPI, pior caso 4,13 ms,
zero erros em 10 trocas.

## DPI por eixo X e Y

Só funciona se o mouse tiver a feature HID++ `0x2202` (*Extended Adjustable DPI*). O G203
**não tem** — o painel detecta isso, desabilita a caixa e diz o porquê, em vez de aceitar
um valor que seria ignorado.

## Variáveis de ambiente

| Variável | Para quê |
|---|---|
| `SENSI_TARGET_EXE` | Outro emulador — `LDPlayer.exe`, `AndroidEmulator.exe`. `*` vale em qualquer janela. |
| `SENSI_DEBUG=1` | Escreve o estado do motor em `%TEMP%\sensi-debug.txt`. |

## Alternativa sem este programa

Se você já usa o G HUB, o script em
[`docs/logitech-g-hub/`](docs/logitech-g-hub/sensibilidade-alternada.lua) faz o mesmo por
dentro dele, em ~15 linhas de Lua. Este painel existe para não depender do G HUB e para,
mais adiante, cobrir outras marcas.

## O que este projeto não faz, por decisão

Compensação de recuo e autofire. Trocar DPI não automatiza ação nenhuma, e keymapping em
emulador oficial com cliente não modificado é permitido pela Garena. Recuo automático e
disparo em laço são macro, são observáveis dentro do jogo, e são banimento.

## Testes

```
cargo test
```

O enquadramento do protocolo HID++ é testado sem hardware, inclusive contra bytes reais
capturados de um G203. A conversa com o dispositivo e o painel se verificam rodando.
