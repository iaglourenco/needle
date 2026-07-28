# Sessões encerradas persistidas — Needle

**Data:** 2026-07-28
**Status:** Aprovado

## Objetivo

Manter sessões no estado `Ended` visíveis no painel indefinidamente (em vez
de desaparecerem quase imediatamente, como hoje), pra que o custo total
mostrado no `UsageBanner` reflita também sessões já encerradas — não só as
ativas no momento. O usuário continua podendo apagar manualmente uma
sessão encerrada pelo botão de apagar já existente.

## Comportamento atual (achado durante brainstorming)

Ao contrário do que o nome "purga após 24h" (README) sugere para sessões
`Stale`, sessões `Ended` hoje são removidas quase na hora:

- `db.rs::list_sessions` filtra `WHERE state != 'Ended'` — uma sessão
  `Ended` nunca aparece na query usada tanto pelo comando Tauri
  `list_sessions` quanto por `tray.rs`.
- `tray.rs::on_session_changed` emite explicitamente `session-removed`
  pro frontend assim que `new_state == SessionState::Ended`
  (`tray.rs:48-50`) — o frontend some com a sessão da store no mesmo
  instante em que ela termina, sem esperar qualquer timeout.
- `main.rs::spawn_cleanup_job` chama `db::delete_ended_sessions`
  incondicionalmente a cada tick (60s) — apaga do banco fisicamente
  pouco depois. O purge de 24h (`STALE_PURGE_AFTER_SECS`) só se aplica a
  sessões `Stale`, nunca chega a valer pra `Ended`.

## Escopo

- Sessões `Ended` passam a aparecer no painel como qualquer outra, e a
  contar pro custo total, até serem apagadas manualmente.
- Botão de apagar (hoje só visível pra `Stale`) passa a valer também pra
  `Ended`.
- Fora de escopo: paginação/arquivamento de sessões antigas, exportação de
  custo, filtro por estado — o crescimento do banco ao longo do tempo é
  aceito como consequência direta do pedido ("não mais remover").

## Arquitetura

Sem mudança de schema. Muda o que é considerado "sessão ativa" pra fins de
listagem: `Ended` deixa de ser um estado que a query/emissão de eventos
trata como "já não existe mais" e passa a ser só mais um estado terminal,
igual a como `Stale` já é tratado (visível, com botão de apagar, sem
purge automático).

## Componentes

### Backend (Rust)

- `db.rs`:
  - `list_sessions`: query passa de
    `SELECT ... FROM sessions WHERE state != 'Ended' ORDER BY last_event_at DESC`
    para `SELECT ... FROM sessions ORDER BY last_event_at DESC` (sem
    filtro de estado).
  - Remove `delete_ended_sessions` (função e teste
    `delete_ended_sessions_removes_only_ended`) — sem mais nenhum
    chamador após a mudança no `main.rs`.
  - Teste `list_sessions_excludes_ended` é substituído por
    `list_sessions_includes_ended`, verificando que uma sessão `Ended`
    aparece no resultado.

- `main.rs`:
  - `spawn_cleanup_job`: remove a linha
    `db::delete_ended_sessions(&conn).ok();` — nenhuma rotina automática
    apaga sessões `Ended`.
  - Comando `delete_session`: guarda hoje
    `if current != Some(state::SessionState::Stale) { return Ok(false); }`
    passa a
    `if !matches!(current, Some(state::SessionState::Stale) | Some(state::SessionState::Ended)) { return Ok(false); }`.

- `tray.rs`:
  - `on_session_changed`: remove o bloco
    ```rust
    if new_state == SessionState::Ended {
        let _ = app_state.app_handle.emit("session-removed", session_id);
    }
    ```
    A sessão passa a fluir pelo caminho normal — como `list_sessions` já
    não a exclui mais, o `find` que localiza a sessão pra atualizar
    custo/model (`tray.rs:33`) e o emit de `session-updated`
    (`tray.rs:45-47`) passam a alcançá-la corretamente (hoje isso
    silenciosamente não acontecia pra transição final rumo a `Ended`,
    porque a sessão já não estava mais no vetor retornado por
    `list_sessions` no momento desse lookup).

### Frontend (Vue)

- `src/components/SessionItem.vue`: botão de apagar (`v-if` hoje
  `session.state === 'Stale'`) passa a
  `session.state === 'Stale' || session.state === 'Ended'`.
- `src/locales/pt-BR.json` / `en.json`: chave `sessions.delete` (hoje
  "Apagar sessão obsoleta" / "Delete stale session") passa a ser
  genérica — "Apagar sessão" / "Delete session" — já que vale pros dois
  estados.
- `src/components/UsageBanner.vue`: nenhuma mudança — `totalCost` já é
  `store.sessions.reduce(...)`, e `store.sessions` passa a incluir
  `Ended` automaticamente assim que a store para de receber
  `session-removed` pra esse caso.
- `src/components/SessionList.vue`: nenhuma mudança — sessões `Ended` já
  têm cor/label definidos em `SessionItem.vue` (`stateColor.Ended`,
  `sessions.states.Ended`), só passam a permanecer na lista.

## Fluxo de dados

1. Claude Code dispara hook `SessionEnd` → `server.rs` grava
   `state = Ended` no banco → `tray::on_session_changed` roda,
   encontra a sessão (agora incluída na query), atualiza custo/model se
   disponível, emite `session-updated` (não mais `session-removed`).
2. Frontend recebe `session-updated`, atualiza a sessão na store — ela
   continua na lista, com estado "Encerrada"/"Ended", cor cinza, contando
   pro `totalCost` do `UsageBanner`.
3. Usuário pode apagar manualmente pelo botão (agora visível também pra
   `Ended`) — chama o mesmo comando `delete_session` já existente, que
   agora aceita ambos os estados terminais.
4. Nenhuma rotina automática apaga `Ended` — só some do banco por ação
   explícita do usuário.

## Erros / Compatibilidade

Sem migração de schema. Instalações existentes: sessões `Ended` que já
foram apagadas pelo comportamento antigo simplesmente não existem mais no
banco — não há nada pra recuperar retroativamente. A partir do deploy
dessa mudança, toda sessão que terminar passa a persistir.

## Testes

- Rust: `list_sessions_includes_ended` (substitui
  `list_sessions_excludes_ended`) confirma que `Ended` aparece na
  listagem. Remoção de `delete_ended_sessions_removes_only_ended` (função
  removida). `cargo test` deve passar sem essas duas alterações quebrarem
  nada mais.
- Frontend: sem harness automatizado; `npm run build` (type-check) +
  verificação manual/trace: encerrar uma sessão de teste, confirmar que
  ela continua na lista com o botão de apagar visível e que o custo total
  no banner reflete o valor dela.
