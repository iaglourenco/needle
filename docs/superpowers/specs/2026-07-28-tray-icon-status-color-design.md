# Ícone de bandeja colorido por status — Needle

**Data:** 2026-07-28
**Status:** Aprovado

## Objetivo

Fazer o ícone da bandeja refletir visualmente o pior estado agregado entre
as sessões monitoradas — hoje só o tooltip (texto, exige hover) e o painel
mostram isso; o ícone em si é estático desde o setup do app.

## Comportamento atual

`main.rs:297-298` seta o ícone da bandeja uma única vez, no setup:

```rust
TrayIconBuilder::with_id(tray::TRAY_ID)
    .icon(app.default_window_icon().unwrap().clone())
```

Nunca mais muda. `tray.rs` já recalcula `worst_state` e atualiza o
tooltip em dois pontos (`on_session_changed`, `refresh_from_db`) — o
ícone deveria acompanhar a mesma recomputação.

## Escopo

- Ícone vira um círculo colorido de ~32x32 quando `worst_state` tiver
  severidade > 0 (`Running`, `WaitingInput`, `NeedsAttention`, `Error`,
  `Idle`).
- Ícone volta ao logo padrão do Needle quando não há sessão nenhuma, ou
  quando só existem sessões `Stale`/`Ended` (severidade 0) — evita que o
  ícone fique permanentemente colorido agora que `Ended` persiste pra
  sempre (feature anterior).
- Paleta reaproveita as cores já usadas em `SessionItem.vue`
  (`stateColor`): Running `#3b82f6`, WaitingInput `#eab308`,
  NeedsAttention/Error `#ef4444`, Idle `#22c55e`.
- Fora de escopo: animação/piscar do ícone, ícone diferente por SO
  (Windows é o único alvo do projeto), configuração pra desabilitar essa
  troca de ícone.

## Arquitetura

Sem dependência nova. `tauri::image::Image::new_owned(rgba: Vec<u8>, w,
h)` já existe no crate `tauri` (feature `tray-icon`, já habilitada) e
não exige as features `image-png`/`image-ico` — construção de imagem via
buffer RGBA cru, sem carregar arquivo. O ícone é desenhado em runtime
(um círculo preenchido sobre fundo transparente), não é um asset
versionado.

## Componentes

### Backend (Rust) — `src-tauri/src/tray.rs`

- `icon_color_for(state: SessionState) -> Option<(u8, u8, u8)>`: mapeia
  estado → cor RGB; `Stale`/`Ended` retornam `None`.
- `dot_icon(color: (u8, u8, u8)) -> tauri::image::Image<'static>`:
  desenha um bitmap RGBA `32x32`, círculo preenchido com a cor dada,
  antialiasing simples ou borda dura, resto do buffer transparente
  (`alpha = 0`).
- `icon_for_worst(app_state: &AppState, worst: Option<SessionState>) ->
  Option<tauri::image::Image<'static>>`: `worst.and_then(icon_color_for)
  .map(dot_icon)`, com fallback pro `app_state.app_handle
  .default_window_icon().cloned()` quando o resultado for `None`.
- Chamado nos dois pontos que já recalculam `worst` e o tooltip:
  `on_session_changed` (`tray.rs:43-57`) e `refresh_from_db`
  (`tray.rs:64-77`) — logo após `tray.set_tooltip(...)`, adiciona
  `let _ = tray.set_icon(icon_for_worst(app_state, worst));`.

## Fluxo de dados

1. Qualquer evento de hook ou tick do job de limpeza recalcula `worst`
   (já acontece hoje).
2. Tooltip é atualizado (já acontece hoje) e, na sequência, o ícone
   também — mesma fonte de verdade (`worst`), sem estado adicional pra
   sincronizar.
3. Se `worst` for `None` (sem sessões) ou só houver `Stale`/`Ended`
   (severidade 0), o ícone volta pro logo padrão do Needle.

## Erros / Compatibilidade

Sem mudança de schema, sem asset novo pra empacotar no instalador. Se
`app_state.app_handle.default_window_icon()` retornar `None` (não deveria
acontecer — o ícone padrão vem do `tauri.conf.json`), `tray.set_icon`
recebe `None`, que no backend de bandeja do Windows na verdade **zera** o
ícone (`NIM_MODIFY` com handle nulo), não mantém o atual — mas isso é
inofensivo na prática: `default_window_icon()` já é `.unwrap()`ado em
outro ponto do `main.rs`, então o app teria travado no startup bem antes
desse código rodar, se esse `None` fosse sequer possível.

## Testes

- Rust: `icon_color_for` — um teste por estado confirmando a cor certa
  (ou `None` pra `Stale`/`Ended`). `dot_icon` — dimensão do buffer
  (`32 * 32 * 4` bytes), pixel central com a cor e alpha 255, pixel de
  canto (0,0) com alpha 0. Todos rodam via `cargo test`, sem precisar de
  runtime Tauri.
- Frontend: nenhuma mudança, nenhuma verificação necessária.
- Verificação manual (não coberta por teste automatizado): rodar
  `npm run tauri dev`, disparar sessões em estados diferentes (ou usar o
  hook real do Claude Code) e observar o ícone da bandeja mudar de cor —
  isso exige olhar a bandeja do Windows de verdade, que nenhum subagent
  headless consegue fazer; fica pro humano confirmar antes do merge.
