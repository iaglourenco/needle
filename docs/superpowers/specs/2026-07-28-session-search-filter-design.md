# Filtro de busca no painel de sessões — Needle

**Data:** 2026-07-28
**Status:** Aprovado

## Objetivo

Permitir localizar rapidamente uma sessão em meio a muitas, filtrando a
lista por texto (nome do projeto / caminho do `cwd`) através de um campo
de busca que aparece ao clicar num ícone no header do painel.

## Escopo

- Ícone de lupa no header (`SessionList.vue`), ao lado do título "Needle".
- Clique no ícone abre campo de texto inline no header; clique de novo (ou
  `Escape`) fecha o campo e limpa o filtro.
- Filtro é aplicado sobre `projectName` (nome da última pasta do `cwd`,
  já derivado em `SessionItem.vue`) e sobre o `cwd` completo — substring
  match, case-insensitive.
- Fora de escopo: busca por `last_message_snippet`, por modelo, ou por
  estado da sessão; persistência da query entre reaberturas do painel.

## Arquitetura

Estado 100% local ao frontend, sem mudança de store (`sessions.ts`) nem
de backend (Rust). `SessionList.vue` já centraliza a ordenação (`sorted`
computed) — o filtro se encaixa como mais um `computed` na mesma cadeia,
antes da renderização do `<ul>`.

## Componentes

### Frontend (Vue)

- `SessionList.vue`:
  - dois novos `ref`s: `searchOpen` (boolean, default `false`) e
    `searchQuery` (string, default `""`).
  - botão de lupa no `<header>`, `@click` alterna `searchOpen`; ao fechar
    (`searchOpen = false`), zera `searchQuery`.
  - `<input>` de texto renderizado com `v-if="searchOpen"` dentro do
    header, com `@keydown.escape` fechando e limpando; `autofocus` ao
    abrir.
  - lógica de projeto extraída para uma função utilitária compartilhada
    (`projectNameOf(cwd)`), já que hoje esse cálculo mora só dentro de
    `SessionItem.vue` como `computed` local e o filtro precisa do mesmo
    resultado em `SessionList.vue`. Vai para `src/lib/session.ts`
    (novo arquivo pequeno) e `SessionItem.vue` passa a importar de lá em
    vez de recalcular.
  - novo `computed filtered`: aplica sobre `sorted` — se `searchQuery`
    vazio, retorna a lista inteira; senão, `filter` por
    `projectNameOf(cwd).toLowerCase().includes(q) || cwd.toLowerCase().includes(q)`.
  - `<ul>` passa a iterar `filtered` em vez de `sorted`.
  - texto vazio (`sessions.empty`) só aparece quando `store.sessions` está
    vazio; quando o filtro não bate com nada mas existem sessões, mostra
    uma mensagem nova (`sessions.noMatch`) para diferenciar "nenhuma
    sessão" de "nenhum resultado pro filtro".

- `src/locales/pt-BR.json` / `en.json`: novas chaves em `sessions`:
  - `search` (aria-label/title do botão de lupa)
  - `searchPlaceholder` (placeholder do input)
  - `noMatch` (mensagem de lista vazia por filtro, distinta de `empty`)

## Fluxo de dados

1. Usuário clica ícone de lupa → `searchOpen = true` → input aparece e
   ganha foco.
2. Usuário digita → `searchQuery` atualiza reativamente → `filtered`
   recalcula → `<ul>` re-renderiza com o subconjunto.
3. Usuário clica lupa de novo, ou `Escape` no input → `searchOpen = false`,
   `searchQuery = ""` → lista volta ao estado completo.

## Erros / Compatibilidade

Nenhuma mudança de schema, store ou IPC — risco de regressão limitado ao
próprio componente. Extração de `projectNameOf` para módulo compartilhado
é um pequeno refactor sem mudança de comportamento (mesma lógica, mesmo
resultado).

## Testes

Sem harness de teste automatizado de frontend no repo hoje (só
`vue-tsc --noEmit` via `npm run build`). Verificação manual via
`npm run tauri dev`: abrir/fechar campo, digitar substring de projeto
existente e inexistente, confirmar `noMatch` vs `empty`, confirmar
`Escape` limpa e fecha.
