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

<!--
  GIF: o anel. Salve como docs/media/ring.gif e troque este comentário por
  ![Segurar o avançar abre o anel de ações](docs/media/ring.gif)
-->

<!--
  GIF: os gestos. Salve como docs/media/gestures.gif e troque este comentário
  por
  ![Segurar o voltar e arrastar dispara um gesto](docs/media/gestures.gif)
-->

O Thumbring 0.1.0 é a primeira versão pública. A configuração é feita por um
arquivo; um editor visual está planejado para a 0.2.0. O que mudou em cada
versão está no [changelog](CHANGELOG.md) (em inglês).

## Instalação

O Thumbring roda no Windows 10 e 11 (64 bits) e funciona com qualquer mouse que
tenha os dois botões laterais, voltar e avançar.

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

**Desinstalar.** Com o `.msi`, remova o Thumbring em Configurações >
Aplicativos > Aplicativos instalados; isso também faz ele parar de abrir com o
Windows. Com o `.zip`, primeiro clique com o botão direito no ícone da bandeja
e desmarque **Start with Windows** (ou **Start with Windows as
administrator**); depois feche o programa e apague a pasta dele. A
configuração e o log continuam em `%APPDATA%\thumbring` e
`%LOCALAPPDATA%\thumbring`; apague essas pastas para remover tudo.

## Como usar

Segure o botão voltar do mouse, arraste uns 3 cm e solte:

| Gesto | Padrão |
|---|---|
| Cima | `Win+Tab` (visão de tarefas) |
| Baixo | `Win+D` (mostrar a área de trabalho) |
| Esquerda | `Ctrl+Win+Right` (próxima área de trabalho virtual) |
| Direita | `Ctrl+Win+Left` (área de trabalho virtual anterior) |
| Toque rápido | voltar, como sempre |

Segure o botão avançar por um instante: o anel abre ao redor do cursor.
Aponte para um item e clique, ou solte o avançar e clique depois. Uma pasta
abre quando o cursor para nela. Clique fora do anel, botão direito, Esc,
avançar de novo ou trocar de janela fecham o anel sem fazer nada. Com o anel
aberto, cliques e a roda do mouse nunca chegam à janela de baixo.

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
direito, abre o menu: pausar, abrir o arquivo de configuração, abrir o log,
iniciar com o Windows (normalmente ou como administrador) e sair.

## Configuração

O arquivo fica em `%APPDATA%\thumbring\config.toml`. O Thumbring cria esse
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
botão avançar como sempre foi, deixe a lista vazia: `slots = []` em `[ring]`.

Ícones: `calculator`, `calendar`, `clipboard`, `clipboard-list`,
`clipboard-paste`, `code`, `copy`, `folder-open`, `globe`, `headphones`,
`layout-grid`, `lock`, `mail`, `music`, `play`, `redo-2`, `scan`, `scissors`,
`settings`, `skip-back`, `skip-forward`, `smile`, `sticky-note`, `terminal`,
`undo-2`, `volume-1`, `volume-2`, `volume-x` e `x`. Sem ícone (ou com um
desconhecido, que gera um aviso no log), o item mostra a primeira letra do
nome.

```toml
[animation]
style = "viscous"     # ou "elastic"
speed = 1.0           # 0.5 a 2.0

[behavior]
open_with = "hold"    # ou "tap"
hold_to_open_ms = 150
path_tolerance_ms = 50

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
Thumbring ignora o voltar e o avançar que outros programas simulam, como
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
têm assinatura digital: o certificado é caro para um projeto gratuito, e a
assinatura gratuita para projetos open source já foi pedida. Até lá, o
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

## Perguntas frequentes

**O Thumbring coleta dados ou usa a internet?**
Não. Ele não faz nenhuma conexão de rede. O log fica no seu computador; ele
lista os nomes dos apps que ganham o foco, o que ajuda a escrever perfis.

**Funciona com o meu mouse?**
Com qualquer mouse que tenha os dois botões laterais, voltar e avançar (às
vezes chamados de botões 4 e 5).

**Como mudo os gestos ou o anel?**
Botão direito no ícone da bandeja > Open settings file, edite e salve. O
Thumbring relê o arquivo na hora; a seção "Configuração", acima, explica as
opções.

**Tem uma janela de configurações?**
Ainda não. A versão 0.1.0 é configurada pelo arquivo, e um editor visual está
planejado para a 0.2.0.

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

Versão de release, que roda sem console:

```bash
cargo build --release -p thumbring
```

Depois abra `target\release\thumbring.exe`. `--dir <pasta>` guarda o arquivo
de configuração e o log numa pasta só, o que ajuda a testar sem mexer na sua
configuração de verdade. Os arquivos de cada versão são montados por
[`packaging/package.ps1`](packaging/package.ps1).

## Estrutura

```
core/       biblioteca (thumbring-core): lógica pura e testável, sem APIs do
            Windows: máquina de estados do anel, gestos, comandos, perfis e
            arquivo de configuração
engine/     programa Windows (thumbring.exe): hook de mouse, ícone na bandeja,
            executor de comandos e o anel, desenhado na GPU
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

Os ícones do anel em [`engine/assets/icons`](engine/assets/icons) vêm do
[Lucide](https://lucide.dev) e estão sob a licença ISC
([LICENSE-LUCIDE](engine/assets/icons/LICENSE-LUCIDE)).

### Contribuições

Salvo indicação explícita em contrário, qualquer contribuição enviada
intencionalmente para inclusão neste trabalho, conforme definido na licença
Apache-2.0, será licenciada duplamente como acima, sem termos ou condições
adicionais.
