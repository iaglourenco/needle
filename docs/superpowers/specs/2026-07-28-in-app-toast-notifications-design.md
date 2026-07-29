# Notificação in-app (toast custom) — Needle

**Data:** 2026-07-28
**Status:** Aprovado

## Objetivo

Quando o usuário desliga as notificações nativas do Windows
(`notifications_enabled = false`, feature anterior), ele hoje fica sem
aviso nenhum de sessão precisando de atenção — só o ícone/tooltip da
bandeja, que exige olhar pra lá. Esta feature adiciona um toast próprio
do Needle (janela pequena, sem decoração, perto da bandeja, some sozinho)
que substitui a notificação nativa nesse caso.

## Escopo

- Toast aparece **só quando `notifications_enabled == false`** — nunca
  junto com a notificação nativa, nunca dobrado.
- Conteúdo igual ao que a notificação nativa mostraria hoje: título
  (`i18n::notif_title`) e corpo (`cwd` da sessão).
- Some sozinho depois de alguns segundos (janela some/hide, não fecha).
- Clique no toast abre o painel principal na aba Sessões — mesmo
  comportamento de clicar no ícone da bandeja hoje (`show_near_tray` +
  evento `show-view: sessions`). Não faz scroll/destaque até a sessão
  específica.
- Um toast de cada vez — um novo substitui/reagenda o anterior, nunca
  empilha.
- Fora de escopo: histórico de notificações, múltiplos toasts
  simultâneos, destacar a sessão específica no painel, notificação de
  "hooks configurados" (`main.rs`) — essa continua sempre nativa, como já
  decidido na feature do toggle.

## Arquitetura

Nova janela Tauri (`label: "toast"`), declarada em `tauri.conf.json` do
mesmo jeito que `main` hoje — invisível no startup, sem decoração,
`alwaysOnTop`, `skipTaskbar`, tamanho fixo pequeno. `main.ts` decide, ao
inicializar, qual componente Vue montar checando o `label` da janela
atual (`getCurrentWindow()` de `@tauri-apps/api/window`): a janela `main`
continua montando `App.vue` como hoje; a janela `toast` monta um novo
componente `ToastView.vue`, isolado, sem rotas nem store de sessões.

**Mudança na feature anterior:** `tray.rs::notify_if_needed` hoje decide
"notifica ou não" com o setting já embutido na decisão (se desligado,
não faz nada). Passa a decidir **canal**: transição pede atenção → sim
sempre (`should_alert`, sem o parâmetro `enabled` — renomeado de
`should_send_notification`); *canal* é nativo se `notifications_enabled`
estiver ligado, senão é o toast novo. Isso é uma correção necessária: o
comportamento atual de "desligar = silêncio total" deixa de existir,
substituído por "desligar = usa o canal in-app".

**`AppState` ganha 3 campos novos**, promovendo estado que hoje só existe
como variável local no closure de `main.rs::setup` (necessário porque
agora dois lugares — clique no ícone da bandeja e clique no toast —
precisam da mesma posição/janela):
- `main_window: WebviewWindow` — handle da janela `main` (hoje só
  existe localmente via `app.get_webview_window("main")`).
- `last_tray_pos: Mutex<PhysicalPosition<f64>>` — hoje é uma variável
  local (`Arc<Mutex<...>>`) só capturada pelos closures do ícone da
  bandeja; sobe pra `AppState` sem mudar de tipo.
- `toast_generation: Mutex<u64>` — contador monotônico; cada chamada de
  `toast::show` incrementa e captura o próprio número, e o timer que
  esconde a janela só age se o número ainda for o mais recente (evita um
  toast novo ser escondido pelo timer de um toast anterior).

## Componentes

### Backend (Rust)

- **`src-tauri/src/toast.rs`** (novo módulo, responsabilidade única:
  orquestrar a janela de toast):
  - `pub fn show(app_state: &AppState, session_id: &str, cwd: &str, title: &str, body: &str)`:
    posiciona a janela `toast` perto de `last_tray_pos` (mesma lógica de
    `position_near_tray` já existente em `main.rs`), emite evento
    `toast-show` com `{ sessionId, cwd, title, body }` pra ela, mostra a
    janela, incrementa `toast_generation` e agenda (`tauri::async_runtime::spawn`
    + `tokio::time::sleep`) o hide depois de N segundos — só executa o
    hide se o número capturado ainda bater com `toast_generation` atual.

- **`src-tauri/src/tray.rs`**:
  - `should_send_notification(enabled, previous, new)` renomeada pra
    `should_alert(previous, new)`, perde o parâmetro `enabled` — vira
    "essa transição pede atenção?", independente de canal.
  - `notify_if_needed` (renomeia pra `alert_if_needed` já que não é mais
    só notificação nativa): se `should_alert` for falso, sai. Senão, lê
    `notifications_enabled`; se ligado, chama a API de notificação do SO
    (como hoje); se desligado, chama `toast::show(...)` com o mesmo
    título/corpo que seria usado.

- **`main.rs`**: novo comando Tauri `open_panel_from_toast()` — sem
  parâmetros, usa `AppState.main_window` + `AppState.last_tray_pos` pra
  chamar `show_near_tray` e emitir `show-view: sessions`, mesma lógica já
  existente no handler de clique do ícone da bandeja (que passa a
  reutilizar essa mesma função em vez de duplicar a lógica inline).
  `tauri.conf.json` ganha a segunda entrada de janela (`toast`).

### Frontend (Vue)

- **`src/main.ts`**: antes de montar, lê `getCurrentWindow().label`; se
  `"toast"`, monta `ToastView.vue` (sem Pinia/i18n de sessões — só
  precisa exibir texto simples, já vem traduzido do backend); senão,
  fluxo atual (`App.vue`) inalterado.
- **`src/components/ToastView.vue`** (novo, pequeno): escuta o evento
  `toast-show`, guarda `{ title, body }` num `ref`, renderiza; `@click`
  no corpo chama `invoke("open_panel_from_toast")` e depois
  `getCurrentWindow().hide()`.
- **`src/lib/api.ts`**: novo método `openPanelFromToast: () =>
  invoke<void>("open_panel_from_toast")`.

## Fluxo de dados

1. Sessão transiciona pra estado que pede atenção (via hook ou via job de
   limpeza — os dois já chamam `alert_if_needed` depois da feature
   anterior e do bugfix de `NeedsAttention`).
2. `alert_if_needed`: `should_alert` verdadeiro → checa
   `notifications_enabled`. Ligado → toast nativo do Windows, como hoje.
   Desligado → `toast::show(...)`.
3. `toast::show` posiciona/mostra a janela `toast`, emite `toast-show`
   com o conteúdo, agenda o hide automático.
4. `ToastView.vue` recebe o evento, mostra o texto. Usuário clica →
   `open_panel_from_toast` (mostra `main` perto da bandeja, muda pra aba
   Sessões) e o toast se esconde. Ou o usuário não faz nada → o timer do
   backend esconde a janela sozinho.

## Erros / Compatibilidade

Nenhuma mudança de schema. Sem migração — o comportamento novo só se
manifesta quando `notifications_enabled` já estiver `false` (setting da
feature anterior); com o valor default (`true`), nada muda visivelmente
pra quem nunca mexeu no toggle.

## Testes

- Rust: `should_alert` — testável puro (transição pede atenção ou não,
  sem depender de canal), migra os casos já cobertos por
  `should_send_notification` (menos os de `enabled=false`, que deixam de
  fazer sentido nesse formato). Geração/posicionamento/timer de janela
  não são testáveis via `cargo test` — dependem de runtime Tauri real.
- Frontend: sem harness automatizado; `npm run build` (type-check).
  Verificação manual (não coberta por teste automatizado, exige olhar a
  tela de verdade): desligar notificação nativa, forçar uma sessão a
  precisar de atenção (via `WaitingInput`/`Error` — não via
  `NeedsAttention` direto, que só é alcançável pelo timeout do job de
  limpeza), confirmar que o toast aparece perto da bandeja com o texto
  certo, some sozinho depois de alguns segundos, e que clicar nele abre o
  painel na aba Sessões.
