<p align="center">
  <img src="assets/readme/hero.svg" width="100%" alt="Needle — monitor de sessões Claude Code na bandeja, com legenda de estados por cor: rodando, aguardando, atenção/erro, ociosa, obsoleta" />
</p>

<p align="center">
  <a href="../../releases"><img src="https://img.shields.io/badge/download-instalador%20Windows-3b82f6?style=flat-square" alt="Download do instalador" /></a>
  <img src="https://img.shields.io/badge/stack-Tauri%20%2B%20Vue%203%20%2B%20SQLite-12151b?style=flat-square" alt="Stack: Tauri, Vue 3, SQLite" />
  <img src="https://img.shields.io/badge/license-MIT-6b7280?style=flat-square" alt="Licença MIT" />
</p>

Quando várias sessões do Claude Code rodam em paralelo, a pergunta chata é
sempre a mesma: **qual delas está esperando eu responder algo agora?**
Needle mora na bandeja do Windows e responde isso com uma olhada — sem
alternar entre terminais.

<p align="center">
  <img src="assets/readme/proof-board.svg" width="100%" alt="Mockup do painel da bandeja do Needle mostrando cinco sessões em estados diferentes: rodando, aguardando input, precisa de atenção, ociosa e obsoleta" />
</p>

## O que ele mostra

- Todas as sessões ativas, agrupadas por projeto.
- O estado de cada uma, sempre um destes seis: **rodando** · **aguardando
  input** · **precisa de atenção** · **ociosa** · **erro** · **obsoleta**.
- Notificação nativa do Windows assim que uma sessão passa a precisar de
  você.
- Ícone da bandeja refletindo o pior estado entre todas as sessões abertas
  — um vermelho no ícone já basta pra saber que tem algo pendente.

## Instalação

Sem pré-requisitos. Nada de Node, Rust, ou qualquer runtime pra instalar
antes.

1. Baixe o instalador mais recente (`needle_x64-setup.exe`) na página de
   [Releases](../../releases).
2. Rode o instalador — instala só pro seu usuário, sem pedir permissão de
   administrador.
3. Abra o Needle uma vez. Ele **se auto-configura sozinho**: registra os
   hooks do Claude Code em `~/.claude/settings.json`, sem tocar em hooks de
   outras ferramentas que já estejam lá (faz backup antes de qualquer
   escrita).
4. Pronto. Use o Claude Code normalmente — o ícone da bandeja já reflete o
   estado das suas sessões.

Clique no ícone da bandeja pra abrir o painel. O menu da bandeja tem
atalhos pra **Configurações**, **Reconfigurar hooks** e **Sair**.

## Como funciona

<p align="center">
  <img src="assets/readme/architecture.svg" width="100%" alt="Fluxo: Claude Code dispara hook, needle hook envia HTTP local, servidor grava no SQLite, bandeja e painel atualizam" />
</p>

O Claude Code dispara [hooks](https://docs.claude.com/claude-code) nativos
a cada evento de sessão. O próprio executável do Needle atende esses
hooks — chamado como `needle.exe hook`, ele lê o payload do stdin e
repassa por HTTP local pro app em execução. Sem Node, sem script externo,
sem dependência além do que já foi instalado.

O app grava tudo em SQLite local, recalcula o estado da sessão e atualiza
bandeja + painel em tempo real.

## Configurações

Pela aba "Configurações" do painel (ou pelo menu da bandeja):

| Opção | O que faz |
| --- | --- |
| Limiar "precisa de atenção" | segundos aguardando input antes do estado escalar |
| Limiar "obsoleta" | minutos sem qualquer evento antes da sessão sumir da lista |
| Iniciar com o sistema | liga o Needle junto com o Windows |
| Status dos hooks | mostra se estão registrados, com botões pra reconfigurar ou remover |

## Desenvolvimento

Pré-requisitos: [Rust](https://rustup.rs) (toolchain MSVC), [Visual Studio
Build Tools](https://visualstudio.microsoft.com/visual-cpp-build-tools/)
(workload C++), Node.js 20+.

```bash
npm install
npm run tauri dev     # modo desenvolvimento
npm run tauri build   # gera o instalador NSIS em src-tauri/target/release/bundle/nsis
```

Testes do backend:

```bash
cd src-tauri && cargo test
```

**Estrutura:**

- `src-tauri/src/` — servidor HTTP local, SQLite, máquina de estado, tray,
  auto-configuração dos hooks, modo `hook` sem GUI.
- `src/` — painel (Vue 3 + TS): lista de sessões e tela de configurações.
- `hook/README.md` — detalhes do hook e configuração manual/por projeto.

## Licença

[MIT](LICENSE)
