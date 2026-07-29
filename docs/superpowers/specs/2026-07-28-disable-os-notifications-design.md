# Desabilitar notificações do Windows — Needle

**Data:** 2026-07-28
**Status:** Aprovado

## Objetivo

Permitir que o usuário desligue as notificações nativas do Windows que o
Needle dispara quando uma sessão entra num estado que pede atenção
(`NeedsAttention`, `Error`, `WaitingInput`). Primeiro passo de uma feature
maior (tela de notificações in-app), tratado como spec/plano/implementação
separado — ver "Fora de escopo".

## Escopo

- Novo campo `notifications_enabled` em `Settings` (Rust) / `Settings`
  (TypeScript), default `true` — comportamento atual (sempre notifica)
  preservado pra instalações existentes e novas, até o usuário desligar
  explicitamente.
- Checkbox novo em Configurações → Geral, ao lado de "Iniciar com o
  sistema".
- Quando desligado, nenhuma notificação nativa dispara pras transições de
  estado de sessão (`tray.rs::notify_if_needed`).
- **Fora de escopo** (confirmado com o usuário): as notificações de
  "hooks configurados"/"hooks reconfigurados" (`main.rs`) são eventos de
  aplicativo, não de sessão — continuam sempre ativas, esse toggle não as
  afeta. Uma tela de notificações in-app (pra não ficar sem aviso nenhum
  quando esse toggle estiver desligado) é um projeto separado, spec
  própria, feito depois deste.

## Arquitetura

Sem mudança estrutural — mais um campo em `Settings`, mais uma checagem
de guarda no único ponto que já dispara notificação de sessão
(`notify_if_needed`). Mesmo padrão já usado pros outros campos de
`Settings` (roteado pelos comandos Tauri `get_settings`/`save_settings`
já existentes, sem novo comando).

## Componentes

### Backend (Rust)

- `settings.rs`:
  - `Settings` ganha `pub notifications_enabled: bool` com
    `#[serde(default = "default_notifications_enabled")]`, onde
    `default_notifications_enabled() -> bool { true }` — `settings.json`
    antigo sem a chave carrega `true`, sem quebra (mesmo padrão já usado
    pro campo `language` no passado, adaptado pra bool com função de
    default já que `serde(default)` sozinho usaria `bool::default() ==
    false`, o que inverteria o comportamento atual).
  - `Default for Settings` ganha `notifications_enabled: true`.

- `tray.rs`:
  - `notify_if_needed` ganha, logo no início da função, antes de
    qualquer outra lógica:
    ```rust
    if !app_state.settings.lock().unwrap().notifications_enabled {
        return;
    }
    ```
    Mantém a assinatura da função inalterada — só adiciona uma guarda no
    topo do corpo.

### Frontend (Vue)

- `src/lib/types.ts`: `Settings` ganha `notificationsEnabled: boolean`.
- `src/components/SettingsView.vue`: novo `<label class="checkbox">` na
  seção "Geral" (`settings.generalTitle`), logo após o checkbox de
  `autostart`, ligado a `settings.notificationsEnabled` via `v-model`
  — mesmo padrão do autostart (só persiste ao clicar "Salvar", igual a
  todo o resto do formulário).
- `src/locales/pt-BR.json` / `en.json`: nova chave
  `settings.notificationsEnabled` — "Notificações do Windows" / "Windows
  notifications" (ou texto equivalente, curto, mesmo estilo dos outros
  labels do formulário).

## Fluxo de dados

1. Usuário desmarca o checkbox em Configurações → Geral → clica Salvar →
   `save_settings` (comando Tauri já existente) persiste
   `notifications_enabled: false` em `settings.json` e atualiza
   `AppState.settings` em memória.
2. Próxima vez que uma sessão transicionar pra `NeedsAttention`/`Error`/
   `WaitingInput`, `on_session_changed` chama `notify_if_needed` como
   sempre — a nova guarda no início da função lê `AppState.settings` e
   sai sem chamar a API de notificação do SO.
3. Ícone da bandeja (cor) e tooltip continuam funcionando normalmente —
   esse toggle afeta só o toast nativo do Windows, nada mais.

## Erros / Compatibilidade

`settings.json` de instalações existentes não tem a chave
`notifications_enabled`; `#[serde(default = "default_notifications_enabled")]`
cobre isso carregando `true` — sem migração, sem mudança de comportamento
pra quem já usa o app.

## Testes

- Rust: teste de roundtrip do campo em `settings.rs` (save → load
  preserva `true`/`false`); teste confirmando que `settings.json` antigo
  sem a chave carrega `notifications_enabled: true` (mesmo padrão dos
  testes já existentes pro campo `language`). Teste em `tray.rs` (ou
  extraído como função pura testável, se `notify_if_needed` continuar
  difícil de testar por depender de `AppState`/notificação real do SO)
  garantindo que a guarda de fato impede a chamada quando desligado —
  avaliar na hora do plano se isso exige extrair a condição pra uma
  função pura primeiro.
- Frontend: sem harness automatizado; `npm run build` (type-check) +
  verificação manual via `npm run tauri dev`: desmarcar o checkbox,
  salvar, forçar uma sessão a precisar de atenção, confirmar que nenhum
  toast do Windows aparece (ícone/tooltip da bandeja continuam mudando
  normalmente).
