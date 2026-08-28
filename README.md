# Sensibilidade alternada

Muda a sensibilidade do mouse **enquanto o botão de tiro está pressionado**, escalando o
movimento do ponteiro do Windows. Funciona com qualquer mouse, de qualquer marca.

> ## ⚠ Não funciona dentro do BlueStacks — leia antes de baixar
>
> Medido em 28/08/2026 no aparelho para o qual isto foi escrito: **o modo de tiro do
> BlueStacks não lê o cursor do Windows**, lê as contagens cruas do dispositivo. O painel
> escala o ponteiro com precisão exata — verificado, erro de 0 px — e o jogo simplesmente
> não percebe.
>
> A prova é direta: **trocar a DPI pelo software do mouse muda a sensibilidade no jogo;
> escalar o cursor por software não muda nada.** Nenhum ajuste no painel resolve isso —
> é uma limitação da camada, não um defeito de implementação.
>
> **O que resolve, hoje e de graça:** se o seu mouse for Logitech G, o script em
> [`docs/logitech-g-hub/`](docs/logitech-g-hub/sensibilidade-alternada.lua) troca a DPI no
> firmware enquanto o gatilho está pressionado. Sem instalar nada, sem driver. Razer,
> Corsair e outros têm o equivalente no software da marca.
>
> **Para valer em qualquer marca** seria preciso um driver de filtro de kernel, como o
> RawAccel faz. Não está implementado aqui.
>
> O painel continua válido para qualquer aplicação que leia o **cursor** do Windows.

![painel](docs/painel.png)

## Baixar

[**Última versão**](https://github.com/S2-DaPaz/sensi-alternada/releases/latest) — um `.exe`,
sem instalador, Windows 10/11 x64. O binário não é assinado, então o SmartScreen avisa no
primeiro arranque: **Mais informações → Executar assim mesmo**. Quem preferir compila do
fonte e compara o hash publicado na release.

## Como usar

1. Rode o `.exe` baixado, ou compile com `cargo build --release`.
2. **DPI base** — a DPI que o seu mouse está usando hoje.
3. **DPI atirando** — a DPI que você quer enquanto segura o gatilho. Menor = mais
   precisão.
4. **Separar eixos X e Y** — marque para dar valores diferentes na horizontal e na
   vertical. Vale para os dois campos: a base e a de tiro ganham X e Y próprios, então
   cada eixo tem o seu fator.
5. **Botão de tiro** — esquerdo, direito, meio, lateral 1 ou lateral 2.
6. **Cores** — dois seletores: um para os botões e as letras, outro para o fundo.
   Qualquer cor. O botão **Padrão** volta às originais.
7. **ATIVAR**, ou `Ctrl + Alt + S` a qualquer momento, inclusive com o jogo em tela
   cheia.

A janela acompanha o estado: com os eixos juntos ela encolhe, com eles separados cresce
para caber os quatro campos.

O ajuste só vale com o **BlueStacks em foco**. No resto do Windows o mouse continua
exatamente como sempre foi, mesmo com o script ativo.

Desativar remove o hook do sistema por completo — não fica nada instalado nem residente.
As configurações ficam em `%APPDATA%\sensi-alternada\settings.json`.

## Como funciona

O painel não troca a DPI do mouse: isso é proprietário de cada fabricante e não daria
para fazer com qualquer marca. Ele intercepta o movimento com um hook
`WH_MOUSE_LL`, **engole** o evento físico e reinjeta a versão multiplicada pelo fator
`DPI atirando ÷ DPI base`.

O movimento é medido pelo **raw input**, que entrega a contagem crua do dispositivo. O hook
serve só para engolir o movimento físico. Medir pela posição de cursor que o hook entrega
não funciona: ela chega acelerada pelo Windows e disputada pelas próprias reinjeções — para
400 px injetados ela reportou 700, depois 100, depois **−150**.

Dois detalhes carregam o peso:

- A reinjeção é **absoluta**, o que ignora a velocidade de ponteiro e a "melhor precisão
  do ponteiro" do Windows. Se fosse relativa, o sistema aplicaria o fator uma segunda vez
  e o número do painel deixaria de significar o que diz.
- O resto fracionário é **acumulado entre eventos**. Multiplicar `+1` por 0,5 e truncar
  daria uma sequência de zeros: a mira travaria justamente no movimento lento, que é onde
  a precisão importa. Provado quebrando o acumulador de propósito — dez movimentos de
  `+1` somaram **0** em vez de 5.

A cor do texto **dentro** do botão não é a que você escolhe: ela é derivada do brilho da
cor escolhida, com os pesos do sRGB. Sem isso um botão amarelo receberia letra branca e
ninguém leria o que está escrito.

## Variáveis de ambiente

| Variável | Para quê |
|---|---|
| `SENSI_TARGET_EXE` | Outro emulador — `LDPlayer.exe`, `AndroidEmulator.exe`. `*` vale em qualquer janela. |
| `SENSI_DEBUG=1` | Escreve contadores em `%TEMP%\sensi-debug.txt`: foco, hook, inscrição no raw input, eventos vistos, suprimidos e injetados. |

## Se a mira ficar dobrada ou trêmula

Esse sintoma tem uma causa só: o BlueStacks estaria lendo o mouse por **raw input** em
vez do cursor do Windows. Suprimir um evento no hook não cega o raw input — medido em
26/08/2026: 300 movimentos engolidos, o cursor andou 0 px e o raw input entregou os 300
assim mesmo. Nesse caso o movimento físico chega **somado** ao nosso.

Não há conserto em user-mode; a saída é um driver de interceptação. Ver
`docs/2026-08-27-sensibilidade-alternada-design.md`.

## O que este projeto não faz, por decisão

Compensação de recuo e autofire. Ajustar sensibilidade não automatiza ação nenhuma, e
keymapping em emulador oficial com cliente não modificado é permitido pela Garena. Recuo
automático e disparo em laço são macro, são observáveis dentro do jogo, e são banimento.

## Testes

```
cargo test
```

A lógica pura — fator, acumulador de resto, leitura do botão, configuração — é testada
sem depender do Windows. O hook, o foco e o painel são verificados rodando.
