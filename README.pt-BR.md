# Thumbring

[Read in English](README.md)

Ferramenta leve para Windows que dá a qualquer mouse **gestos** e um **anel de
ações** radial:

- **Segurar o botão voltar e arrastar** dispara um gesto (cima, baixo, esquerda
  ou direita).
- **Segurar o botão avançar** abre um anel com 8 slots ao redor do cursor. Cada
  slot é uma ação ou uma pasta com até 9 ações.
- **Um toque rápido** em qualquer um dos dois continua fazendo o que o botão
  sempre fez (voltar ou avançar a página).
- **Outros botões** podem fazer qualquer uma das duas funções: voltar, avançar
  ou o botão do meio (apertar a roda).
- **Uma janela de configurações** muda tudo isso, com uma prévia ao vivo do
  anel: botão direito no ícone da bandeja > **Open settings**.

<!--
  GIF: o anel. Salve como docs/media/ring.gif e troque este comentário por
  ![Segurar o avançar abre o anel de ações](docs/media/ring.gif)
-->

<!--
  GIF: os gestos. Salve como docs/media/gestures.gif e troque este comentário
  por
  ![Segurar o voltar e arrastar dispara um gesto](docs/media/gestures.gif)
-->

O Thumbring 0.1.0 é a primeira versão pública. O que mudou em cada versão está
no [changelog](CHANGELOG.md) (em inglês).

## Instalação

Requisitos:

- Windows 10 ou 11, 64 bits.
- Um mouse com os dois botões laterais, voltar e avançar. Num mouse sem eles,
  o botão do meio pode fazer os gestos ou abrir o anel.
- Para a janela de configurações, o WebView2 Runtime, a parte do Windows que
  mostra páginas web dentro dos apps. Ele já vem com o Windows 11, e o Windows
  Update já o instalou na maioria dos computadores com Windows 10. Onde ele
  faltar, o **Open settings** mostra uma mensagem com o link para baixá-lo, de
  graça; todo o resto do Thumbring funciona sem ele, e o arquivo de
  configuração continua editável à mão (menu da bandeja > **Open settings
  file**).

Para instalar:

1. Baixe a versão mais recente na
   [página de Releases](https://github.com/iglesiaskun/thumbring/releases/latest):
   - **`thumbring-<versão>-windows-x64.msi`** (recomendado) instala o Thumbring
     só para o seu usuário, sem pedir permissão de administrador, coloca no
     menu Iniciar e já abre o programa. Para atualizar, rode o `.msi` mais novo.
   - **`thumbring-<versão>-windows-x64.zip`** roda sem instalar: descompacte
     onde quiser e abra o `thumbring.exe`.
2. Se o Windows mostrar "O Windows protegeu o computador", clique em **Mais
   informações** e depois em **Executar assim mesmo**. Os arquivos ainda não
   têm assinatura digital; "Limitações conhecidas", mais abaixo, explica por
   quê e como conferir o download.
3. Na primeira execução, uma notificação avisa que o Thumbring está rodando. O
   ícone fica na área de notificação, perto do relógio; o Windows pode
   escondê-lo atrás da setinha **^**. Para deixá-lo sempre à vista, arraste-o
   para a barra de tarefas.
4. Para o Thumbring abrir junto com o Windows, clique com o botão direito no
   ícone e marque **Start with Windows**. Para ele funcionar também em janelas
   que rodam como administrador, marque **Start with Windows as
   administrator** no lugar; "Limitações conhecidas", abaixo, explica essa
   opção.
5. Para mudar os gestos, o anel ou os botões, clique com o botão direito no
   ícone e escolha **Open settings**.

**Desinstalar.** Com o `.msi`, remova o Thumbring em Configurações >
Aplicativos > Aplicativos instalados; isso também faz ele parar de abrir com o
Windows. Com o `.zip`, primeiro clique com o botão direito no ícone da bandeja
e desmarque **Start with Windows** (ou **Start with Windows as
administrator**); depois feche o programa e apague a pasta dele. A
configuração e o log continuam em `%APPDATA%\thumbring` e
`%LOCALAPPDATA%\thumbring`; apague essas pastas para remover tudo.

## Como usar

Segure o botão voltar do mouse, arraste uns 3 cm e solte (os botões podem ser
trocados; veja "Configuração"):

| Gesto | Padrão |
|---|---|
| Cima | `Win+Tab` (visão de tarefas) |
| Baixo | `Win+D` (mostrar a área de trabalho) |
| Esquerda | `Ctrl+Win+Right` (próxima área de trabalho virtual) |
| Direita | `Ctrl+Win+Left` (área de trabalho virtual anterior) |
| Toque rápido | voltar, como sempre |

Segure o botão avançar por um instante: o anel abre ao redor do cursor.
Aponte para um item e clique, ou solte o avançar e clique depois: o anel
continua aberto onde quer que você solte, até no centro. Uma pasta abre quando
o cursor para nela. Clique no × do centro, clique fora do anel, botão direito,
Esc, avançar de novo ou trocar de janela fecham o anel sem fazer nada. Com o
anel aberto, cliques e a roda do mouse nunca chegam à janela de baixo. No modo
rápido (`execute_on_release = true` em `[behavior]`), soltar sobre uma ação
executa e soltar no centro fecha o anel.

| Anel padrão (sentido horário, a partir do topo) | Faz |
|---|---|
| Play or pause | `PlayPause` |
| Screenshot | captura de tela com recorte |
| Media (pasta) | faixa anterior e próxima, volume, mudo |
| Lock screen | bloqueia a sessão |
| Editing (pasta) | copiar, colar, recortar, desfazer, refazer, histórico da área de transferência |
| Emoji | painel de emojis |
| File Explorer | abre o Explorador de Arquivos |
| Apps (pasta) | Calculadora, Bloco de Notas, Terminal, Configurações |

Os nomes do anel padrão ficam em inglês, como o resto do arquivo de
configuração; dá para trocar qualquer um.

Clique no ícone da bandeja para pausar ou retomar o Thumbring. Com o botão
direito, abre o menu: pausar, abrir a janela de configurações, abrir o arquivo
de configuração, abrir o log, iniciar com o Windows (normalmente ou como
administrador) e sair.

### Janela de configurações

Botão direito no ícone da bandeja > **Open settings**. A janela tem uma página
para cada parte do Thumbring:

| Página | O que muda |
|---|---|
| Ring | os 8 slots, numa prévia ao vivo: arrastar ações para os slots, abrir pastas, escolher um ícone ou usar um SVG ou PNG seu, gravar um atalho |
| Gestures | o comando de cada direção e quanto arrastar, com um quadro para testar |
| Apps | onde o Thumbring fica desligado (tela cheia, apps excluídos) e os perfis de gestos por app |
| Behavior | quais botões do mouse fazem os gestos e abrem o anel, e como o anel abre |
| Appearance | cores, transparência, tamanho e distância dos itens, sombra e rótulo; **Show on my screen** mostra o anel de verdade antes de salvar |
| Animation | o estilo e a velocidade da animação, com uma demonstração em câmera lenta |

<!--
  GIF: a janela de configurações. Salve como docs/media/settings.gif e troque
  este comentário por
  ![A janela de configurações, com o anel numa prévia ao vivo](docs/media/settings.gif)
-->

Nada muda até você clicar em **Save** (`Ctrl+S`); `Ctrl+Z` e `Ctrl+Y`
desfazem e refazem. O Thumbring passa a usar o que foi salvo na hora, e a
janela avisa. Ela grava no mesmo arquivo de configuração descrito abaixo e
mantém os comentários dele, então dá para usar qualquer um dos dois.

## Configuração

Tudo desta seção pode ser mudado na janela de configurações. O arquivo por
trás dela fica em `%APPDATA%\thumbring\config.toml`. O Thumbring cria esse
arquivo na primeira execução, com todos os padrões e um comentário por opção (o
modelo é [`core/src/config/default.toml`](core/src/config/default.toml), em
inglês), e relê sempre que ele é salvo. Valor inválido ou fora da faixa volta
ao padrão e gera um aviso, sem travar o app; um arquivo ilegível mantém a
configuração anterior. O log fica em `%LOCALAPPDATA%\thumbring\engine.log`
(menu da bandeja > Open log) e também mostra o nome do `.exe` de cada app que
ganha o foco.

A primeira linha, `version = 1`, é o formato do arquivo. Se uma versão futura
mudar o formato, o Thumbring usa esse número para atualizar arquivos antigos.
Um arquivo sem ela conta como versão 1.

```toml
version = 1

[gestures]
button = "back"             # "back", "forward" ou "middle"
up = "Win+Tab"
down = "Win+D"
left = "Ctrl+Win+Right"
right = "Ctrl+Win+Left"
click = "native"
threshold_px = 30           # a 100% de escala; cresce com o DPI do monitor

[apps]
disable_in_fullscreen = true
excluded = ["mstsc.exe"]    # nesses apps o botão funciona normal

# Um perfil troca só os gestos que ele lista.
[[profiles]]
name = "Navegador"
apps = ["browser.exe"]      # o executável do seu navegador
gestures = { left = "Ctrl+Tab", right = "Ctrl+Shift+Tab" }
```

`button` em `[gestures]` é o botão que faz os gestos, e `button` em `[ring]`,
o que abre o anel: `"back"` (voltar), `"forward"` (avançar) ou `"middle"`
(apertar a roda). Sem mudar nada, são o voltar e o avançar. Um botão não faz
as duas coisas: se os dois apontarem para o mesmo botão, os dois voltam ao
padrão, com um aviso. Com o anel desligado, os gestos podem usar o botão dele.
O toque rápido, assim como o comando `"native"`, envia o clique do botão que
foi segurado.

Comandos, iguais para gestos e ações do anel:

| Texto | Faz |
|---|---|
| `"Ctrl+Shift+T"` | atalho de teclado (Ctrl, Win, Alt, Shift mais uma tecla) |
| `"VolumeUp"`, `"PlayPause"`, `"NextTrack"` | teclas de mídia e volume |
| `'open:C:\Users\voce\Downloads'`, `'open:https://...'` | abre app, arquivo, pasta ou URL; `%VARIAVEIS%` são expandidas |
| `'open:"C:\Program Files\App\app.exe" --opcao'` | o mesmo, com argumentos |
| `"screenshot"`, `"emoji"`, `"explorer"`, `"lock"` | ações prontas |
| `"native"` | o que o botão faz sem o Thumbring |
| `"none"` | nada (desliga um gesto num perfil) |

O anel é configurado no mesmo arquivo, que já vem com o anel padrão escrito
por extenso. São 8 slots, começando no topo, em sentido horário; cada um tem
uma ação, uma pasta com até 9 ações ou nada (`type = "empty"`). O que for
salvo vale na próxima vez que o anel abrir. Para desligar o anel e manter o
botão dele como sempre foi, deixe a lista vazia: `slots = []` em `[ring]`.

Ícones: `calculator`, `calendar`, `clipboard`, `clipboard-list`,
`clipboard-paste`, `code`, `copy`, `folder-open`, `globe`, `headphones`,
`layout-grid`, `lock`, `mail`, `music`, `play`, `redo-2`, `scan`, `scissors`,
`settings`, `skip-back`, `skip-forward`, `smile`, `sticky-note`, `terminal`,
`undo-2`, `volume-1`, `volume-2`, `volume-x` e `x`. O ícone também pode ser
um arquivo SVG ou PNG seu, pelo caminho completo, como
`icon = 'C:\Users\voce\Imagens\estrela.svg'` (`%VARIÁVEIS%` funcionam, e o
arquivo pode ter até 2 MB). O SVG usa as cores dos ícones onde usa
`currentColor` ou não define cor; as outras cores dele ficam, e o PNG mantém
as suas. O Thumbring lê o arquivo quando carrega as configurações: depois de
trocar a imagem, salve as configurações de novo. Sem ícone, ou com um
desconhecido ou um arquivo que não abre (o que gera um aviso no log), o item
mostra a primeira letra do nome.

A seção opcional `[appearance]` muda as cores, a transparência e o tamanho do
anel. Sem ela, o anel fica como sempre foi: os valores abaixo são os padrões.
As cores são escritas como `"#RRGGBB"`.

```toml
[appearance]
item_color = "#FFFFFF"            # os itens e o botão central
item_opacity = 0.96               # 0.3 a 1; o destaque é sempre sólido
icon_color = "#2A2B31"
highlight_color = "#121214"       # item sob o cursor, pasta aberta, gota
highlight_icon_color = "#FFFFFF"
cancel_color = "#E64054"          # botão central sob o cursor: ele fecha
item_size_px = 52                 # diâmetro do item a 100% de escala, 32 a 96
ring_radius_px = 93.6             # distância do centro até cada item
shadow = true                     # sombra suave em volta dos itens
label = "light"                   # "light" (claro) ou "dark" (escuro)
```

`ring_radius_px` vai de 1,5 a 3 vezes `item_size_px`, para os itens nunca se
encostarem; sem ele, vale 1,8 vez `item_size_px`. As pastas abrem ao dobro da
distância do centro, e o botão central acompanha o tamanho do item. Os
tamanhos são a 100% de escala e crescem com a escala do monitor; num monitor
pequeno demais para uma pasta aberta, o anel encolhe por inteiro. Para itens
escuros, experimente `item_color = "#1F2026"`, `icon_color = "#E8E9EE"`,
`highlight_color = "#FFFFFF"`, `highlight_icon_color = "#16171B"` e
`label = "dark"`.

```toml
[animation]
style = "viscous"     # ou "elastic"
speed = 1.0           # 0.5 a 2.0

[behavior]
open_with = "hold"    # ou "tap"
hold_to_open_ms = 150
path_tolerance_ms = 50

[ring]
button = "forward"    # "forward", "back" ou "middle"

[[ring.slots]]
type = "action"
name = "Captura de tela"
icon = "scan"
command = "Win+Shift+S"

[[ring.slots]]
type = "folder"
name = "Mídia"
icon = "music"
items = [
  { name = "Mudo", icon = "volume-x", command = "VolumeMute" },
]
```

## Limitações conhecidas

**Janelas que rodam como administrador.** O Windows não deixa um programa sem
permissão de administrador observar ou enviar teclas e cliques para janelas
que rodam como administrador, como o Gerenciador de Tarefas, um terminal de
administrador ou alguns instaladores. Enquanto uma delas está na frente, os
gestos e o anel não funcionam ali, a não ser que o Thumbring também tenha
permissão de administrador:

1. Clique com o botão direito no ícone da bandeja e marque **Start with
   Windows as administrator**. O Windows pede permissão uma vez (a janela do
   UAC) e cria uma tarefa, "Thumbring (seu nome de usuário)", no Agendador de
   Tarefas.
2. A partir do próximo login, essa tarefa abre o Thumbring como
   administrador, sem perguntar nada.

Os apps que o Thumbring abre (comandos `open:`, o arquivo de configuração, o
log) continuam rodando sem permissão de administrador: quem abre é o
Explorador de Arquivos, e se ele não responder, o Thumbring não abre nada e
avisa. Desmarcar a opção, ou marcar **Start with Windows**,
apaga a tarefa. Depois de uma atualização, o instalador abre o Thumbring sem
permissão de administrador até o próximo login.

Pense antes de ligar. Um programa que abre como administrador é tão seguro
quanto a pasta dele, e o `.msi` instala o Thumbring na sua pasta de usuário,
onde qualquer programa que você roda pode gravar. Um programa malicioso
poderia trocar o `thumbring.exe` ali e ganhar permissão de administrador no
seu próximo login, sem a janela do UAC. Se isso preocupa você, deixe a opção
desligada, ou descompacte o `.zip` numa pasta que só administradores podem
alterar, como `C:\Program Files\Thumbring`, e ligue a opção pelo Thumbring
dessa pasta. Pelo mesmo motivo, enquanto tem permissão de administrador, o
Thumbring ignora os botões do mouse que outros programas simulam, como
ferramentas de acesso remoto ou programas que remapeiam os botões do mouse:
só os botões de verdade fazem gestos e abrem o anel.

**Jogos e anti-cheat.** O Thumbring usa os mesmos recursos do Windows que
ferramentas de macro: um hook de mouse de baixo nível e teclas simuladas.
Alguns sistemas anti-cheat bloqueiam ou marcam programas assim. O Thumbring se
desliga quando um app está em tela cheia, mas um jogo em janela sem borda pode
não contar como tela cheia. Antes de jogar online, pause o Thumbring pela
bandeja ou coloque o `.exe` do jogo em `excluded`, na seção `[apps]` do arquivo
de configuração.

**Avisos do SmartScreen e de antivírus.** Os arquivos das versões ainda não
têm assinatura digital: o certificado é caro para um projeto gratuito, e o
plano é pedir a assinatura gratuita para projetos open source agora que a
primeira versão saiu. Até lá, o
SmartScreen do Windows avisa sobre um programa novo e sem reputação ("O
Windows protegeu o computador"), e um antivírus pode desconfiar de um programa
desconhecido que monitora o mouse e simula teclas, que é exatamente o que o
Thumbring faz. Para conferir se o download é o mesmo que o GitHub compilou a
partir deste repositório:

- compare a saída de `Get-FileHash <arquivo>` no PowerShell com a linha desse
  arquivo no `SHA256SUMS` da mesma versão;
- ou, com o [GitHub CLI](https://cli.github.com), rode
  `gh attestation verify <arquivo> --repo iglesiaskun/thumbring`.

Você também pode compilar o Thumbring a partir do código (veja abaixo).

**Software do mouse.** Se o software do seu mouse muda a função dos botões
laterais, o Thumbring deixa de enxergá-los como voltar e avançar. Volte os
dois para voltar e avançar (ou "padrão") nesse software.

**Janela de configurações sem o WebView2.** Num computador sem o WebView2
Runtime (veja "Instalação"), a janela de configurações não abre; no lugar dela
aparece uma mensagem dizendo onde baixá-lo. Os gestos, o anel e o arquivo de
configuração funcionam normalmente.

**Só Windows, em inglês.** Por enquanto o Thumbring roda só no Windows, e os
menus, as mensagens e a janela de configurações estão em inglês.

## Perguntas frequentes

**O Thumbring coleta dados ou usa a internet?**
Não. Ele não faz nenhuma conexão de rede. O log fica no seu computador; ele
lista os nomes dos apps que ganham o foco, o que ajuda a escrever perfis.

**Funciona com o meu mouse?**
Sim. Por padrão, o Thumbring usa os dois botões laterais, voltar e avançar (às
vezes chamados de botões 4 e 5). Num mouse sem eles, escolha o botão do meio
para os gestos ou para o anel na janela de configurações (Behavior > Mouse
buttons), ou escreva `button = "middle"` em `[gestures]` ou em `[ring]` no
arquivo de configuração, e apertar a roda faz os gestos ou abre o anel.

**Como mudo os gestos ou o anel?**
Botão direito no ícone da bandeja > Open settings, mude o que quiser e salve.
O Thumbring passa a usar a configuração nova na hora. Também dá para editar o
próprio arquivo: menu da bandeja > Open settings file; a seção "Configuração",
acima, explica as opções.

**O Open settings abre o arquivo de configuração, ou diz que falta o WebView2. Por quê?**
A janela de configurações é um programa à parte, o `thumbring-settings.exe`,
que precisa estar ao lado do `thumbring.exe` (o `.msi` e o `.zip` já o colocam
lá), e precisa do WebView2 Runtime. Sem o programa, o Thumbring abre o arquivo
de configuração no lugar e avisa; sem o runtime, a janela mostra o link para
baixá-lo. Os gestos e o anel continuam funcionando nos dois casos.

**Como volto para a configuração padrão?**
Feche o Thumbring, apague `%APPDATA%\thumbring\config.toml` e abra o Thumbring
de novo: ele cria um arquivo novo com os padrões.

**Os gestos não funcionam no Gerenciador de Tarefas nem num terminal de administrador. Por quê?**
Essas janelas rodam como administrador, e o Windows mantém os programas sem
essa permissão longe delas. Marque **Start with Windows as administrator** no
menu da bandeja e faça login de novo; "Limitações conhecidas", acima, explica
o que você troca por isso.

**Os gestos pararam de funcionar. E agora?**
Olhe o ícone da bandeja: um anel apagado quer dizer que o Thumbring está
pausado, desligado num app excluído ou desligado porque um app está em tela
cheia; a dica do ícone diz qual. Lembre da limitação com janelas de
administrador. Se nada disso explicar, o log (menu da bandeja > Open log)
costuma explicar; por favor,
[abra uma issue](https://github.com/iglesiaskun/thumbring/issues/new/choose) com as
linhas do problema.

**Por que "Thumbring"?**
O polegar (*thumb*) aperta os botões laterais, e o anel (*ring*) abre ao
redor do cursor.

## Como compilar

Pré-requisitos (só Windows):

- Rust estável com o alvo `x86_64-pc-windows-msvc` (instalado pelo `rustup`).
- Visual Studio ou Build Tools com a carga **Desenvolvimento para desktop com
  C++** (traz o linker MSVC e o SDK do Windows).

```bash
cargo test
```

Versão de desenvolvimento, com o log num console (`Ctrl+C` encerra):

```bash
cargo run -p thumbring
```

Versão de release dos dois programas, que rodam sem console:

```bash
cargo build --release -p thumbring -p thumbring-settings
```

Depois abra `target\release\thumbring.exe`. `--dir <pasta>` guarda o arquivo
de configuração e o log numa pasta só, o que ajuda a testar sem mexer na sua
configuração de verdade. Os arquivos de cada versão são montados por
[`packaging/package.ps1`](packaging/package.ps1).

A janela de configurações é um programa à parte, que precisa do WebView2
Runtime (ele vem com o Windows 11 e com as atualizações do Windows 10). A
interface dela é HTML, CSS e JavaScript puros, em
[`settings/ui`](settings/ui), sem etapa de build:

```bash
cargo run -p thumbring-settings -- --dir <pasta>
```

`--dir` funciona como no motor; sem ele, a janela edita o seu arquivo de
configuração de verdade. O menu da bandeja do motor > Open settings abre a
janela que está ao lado do `thumbring.exe` e repassa o `--dir` do próprio
motor, então os dois usam o mesmo arquivo.

## Estrutura

```
core/       biblioteca (thumbring-core): lógica pura e testável, sem APIs do
            Windows: máquina de estados do anel, gestos, comandos, perfis e
            arquivo de configuração
engine/     programa Windows (thumbring.exe): hook de mouse, ícone na bandeja,
            executor de comandos e o anel, desenhado na GPU
settings/   janela de configurações (thumbring-settings.exe): edita o arquivo
            de configuração mantendo os comentários; interface em settings/ui
packaging/  empacotamento das versões: instalador, avisos de licença
```

## Como contribuir

Relatos de bugs, ideias e pull requests são bem-vindos. O
[CONTRIBUTING.md](CONTRIBUTING.md) (em inglês) explica como as mudanças
entram. Problemas de segurança devem ser relatados em particular, como
descrito no [SECURITY.md](SECURITY.md).

## Licença

Distribuído sob uma das duas licenças abaixo, à sua escolha:

- Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE) ou
  <https://www.apache.org/licenses/LICENSE-2.0>)
- Licença MIT ([LICENSE-MIT](LICENSE-MIT) ou <https://opensource.org/licenses/MIT>)

Os ícones do anel em [`engine/assets/icons`](engine/assets/icons) e os da
janela de configurações em [`settings/ui/icons`](settings/ui/icons) vêm do
[Lucide](https://lucide.dev) e estão sob a licença ISC
([LICENSE-LUCIDE](engine/assets/icons/LICENSE-LUCIDE)).

### Contribuições

Salvo indicação explícita em contrário, qualquer contribuição enviada
intencionalmente para inclusão neste trabalho, conforme definido na licença
Apache-2.0, será licenciada duplamente como acima, sem termos ou condições
adicionais.
