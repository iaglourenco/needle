# Session Search Filter Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add a search icon to the session panel header that toggles a text
field filtering the visible session list by project name / cwd.

**Architecture:** Frontend-only change. Extract the existing inline
`projectName` computation out of `SessionItem.vue` into a shared
`src/lib/session.ts` helper, then add local search state + a filtered
computed to `SessionList.vue`, wired through new i18n keys.

**Tech Stack:** Vue 3 (`<script setup>`, Composition API), vue-i18n,
TypeScript, no frontend test runner (verification is `vue-tsc` type-check +
manual run via `npm run tauri dev`).

## Global Constraints

- No backend/Rust changes, no store (`sessions.ts`) or IPC changes — spec
  scope is UI-only.
- Filter matches on project name (last path segment of `cwd`) and full
  `cwd`, substring, case-insensitive.
- Closing the search field (toggle again or `Escape`) clears the query.
- New user-facing strings must be added to both `src/locales/pt-BR.json`
  and `src/locales/en.json`.
- Verify each task with `npm run build` (runs `vue-tsc --noEmit`) — no
  automated frontend test harness exists in this repo.

---

### Task 1: Extract shared `projectNameOf` helper

**Files:**
- Create: `src/lib/session.ts`
- Modify: `src/components/SessionItem.vue:1-36`

**Interfaces:**
- Produces: `projectNameOf(cwd: string): string` — exported from
  `src/lib/session.ts`. Task 2 imports this same function.

- [ ] **Step 1: Create the helper module**

```ts
// src/lib/session.ts
export function projectNameOf(cwd: string): string {
  const parts = cwd.split(/[\\/]/).filter(Boolean);
  return parts[parts.length - 1] ?? cwd;
}
```

- [ ] **Step 2: Use the helper in `SessionItem.vue`**

In `src/components/SessionItem.vue`, add the import alongside the existing
ones:

```ts
import { projectNameOf } from "../lib/session";
```

Replace the existing `projectName` computed (currently):

```ts
const projectName = computed(() => {
  const parts = props.session.cwd.split(/[\\/]/).filter(Boolean);
  return parts[parts.length - 1] ?? props.session.cwd;
});
```

with:

```ts
const projectName = computed(() => projectNameOf(props.session.cwd));
```

- [ ] **Step 3: Type-check**

Run: `npm run build`
Expected: no TypeScript errors (this is a pure refactor, output unchanged).

- [ ] **Step 4: Manual smoke check**

Run: `npm run tauri dev`
Open the panel, confirm project names still render exactly as before for
every session (no regression from the extraction).

- [ ] **Step 5: Commit**

```bash
git add src/lib/session.ts src/components/SessionItem.vue
git commit -m "refactor: extract projectNameOf into shared session helper"
```

---

### Task 2: Add search toggle + filter to `SessionList.vue`

**Files:**
- Modify: `src/components/SessionList.vue` (full file — script, template,
  style)
- Modify: `src/locales/pt-BR.json:6-18` (`sessions` block)
- Modify: `src/locales/en.json:6-18` (`sessions` block)

**Interfaces:**
- Consumes: `projectNameOf(cwd: string): string` from Task 1
  (`src/lib/session.ts`).
- Consumes: `Session` type (`session_id`, `cwd`, `last_event_at`) from
  `src/lib/types.ts` (unchanged).
- Produces: no new exports — this is the leaf UI component.

- [ ] **Step 1: Add new i18n keys (pt-BR)**

In `src/locales/pt-BR.json`, the `sessions` block currently is:

```json
"sessions": {
    "empty": "Nenhuma sessão ativa.",
    "states": {
      "Running": "Em execução",
      "WaitingInput": "Aguardando input",
      "NeedsAttention": "Precisa de atenção",
      "Idle": "Ociosa",
      "Error": "Erro",
      "Stale": "Obsoleta",
      "Ended": "Encerrada"
    },
    "delete": "Apagar sessão obsoleta"
  },
```

Replace with (adds `search`, `searchPlaceholder`, `noMatch`):

```json
"sessions": {
    "empty": "Nenhuma sessão ativa.",
    "noMatch": "Nenhuma sessão corresponde à busca.",
    "search": "Buscar sessões",
    "searchPlaceholder": "Buscar por projeto...",
    "states": {
      "Running": "Em execução",
      "WaitingInput": "Aguardando input",
      "NeedsAttention": "Precisa de atenção",
      "Idle": "Ociosa",
      "Error": "Erro",
      "Stale": "Obsoleta",
      "Ended": "Encerrada"
    },
    "delete": "Apagar sessão obsoleta"
  },
```

- [ ] **Step 2: Add new i18n keys (en)**

In `src/locales/en.json`, the `sessions` block currently is:

```json
"sessions": {
    "empty": "No active sessions.",
    "states": {
      "Running": "Running",
      "WaitingInput": "Waiting for input",
      "NeedsAttention": "Needs attention",
      "Idle": "Idle",
      "Error": "Error",
      "Stale": "Stale",
      "Ended": "Ended"
    },
    "delete": "Delete stale session"
  },
```

Replace with:

```json
"sessions": {
    "empty": "No active sessions.",
    "noMatch": "No session matches the search.",
    "search": "Search sessions",
    "searchPlaceholder": "Search by project...",
    "states": {
      "Running": "Running",
      "WaitingInput": "Waiting for input",
      "NeedsAttention": "Needs attention",
      "Idle": "Idle",
      "Error": "Error",
      "Stale": "Stale",
      "Ended": "Ended"
    },
    "delete": "Delete stale session"
  },
```

- [ ] **Step 3: Rewrite `SessionList.vue`**

Replace the full contents of `src/components/SessionList.vue` with:

```vue
<script setup lang="ts">
import { computed, nextTick, onMounted, ref } from "vue";
import { useI18n } from "vue-i18n";
import { useSessionsStore } from "../stores/sessions";
import { projectNameOf } from "../lib/session";
import SessionItem from "./SessionItem.vue";
import UsageBanner from "./UsageBanner.vue";

const store = useSessionsStore();
const { t } = useI18n();

const searchOpen = ref(false);
const searchQuery = ref("");
const searchInput = ref<HTMLInputElement | null>(null);

onMounted(() => {
  if (!store.ready) store.init();
});

const sorted = computed(() =>
  [...store.sessions].sort((a, b) => b.last_event_at - a.last_event_at),
);

const filtered = computed(() => {
  const q = searchQuery.value.trim().toLowerCase();
  if (!q) return sorted.value;
  return sorted.value.filter(
    (s) =>
      projectNameOf(s.cwd).toLowerCase().includes(q) ||
      s.cwd.toLowerCase().includes(q),
  );
});

async function toggleSearch() {
  searchOpen.value = !searchOpen.value;
  if (searchOpen.value) {
    await nextTick();
    searchInput.value?.focus();
  } else {
    searchQuery.value = "";
  }
}

function closeSearch() {
  searchOpen.value = false;
  searchQuery.value = "";
}
</script>

<template>
  <div class="session-list">
    <header>
      <h1 v-if="!searchOpen">Needle</h1>
      <input
        v-else
        ref="searchInput"
        v-model="searchQuery"
        type="text"
        class="search-input"
        :placeholder="t('sessions.searchPlaceholder')"
        @keydown.escape="closeSearch"
      />
      <button
        type="button"
        class="search-toggle"
        :title="t('sessions.search')"
        :aria-label="t('sessions.search')"
        @click="toggleSearch"
      >
        <svg viewBox="0 0 16 16" width="14" height="14" aria-hidden="true">
          <circle
            cx="7"
            cy="7"
            r="5"
            fill="none"
            stroke="currentColor"
            stroke-width="1.3"
          />
          <path
            d="M11 11l3.5 3.5"
            stroke="currentColor"
            stroke-width="1.3"
            stroke-linecap="round"
          />
        </svg>
      </button>
    </header>
    <UsageBanner />
    <ul v-if="filtered.length">
      <SessionItem
        v-for="session in filtered"
        :key="session.session_id"
        :session="session"
      />
    </ul>
    <p v-else-if="sorted.length" class="empty">{{ t("sessions.noMatch") }}</p>
    <p v-else class="empty">{{ t("sessions.empty") }}</p>
  </div>
</template>

<style scoped>
.session-list {
  display: flex;
  flex-direction: column;
  flex: 1;
  min-height: 0;
  font-family: system-ui, sans-serif;
  font-size: 0.9rem;
}
header {
  display: flex;
  align-items: center;
  gap: 0.4rem;
  flex-shrink: 0;
  padding: 0.5rem 0.75rem;
  border-bottom: 1px solid rgba(128, 128, 128, 0.3);
}
h1 {
  font-size: 0.95rem;
  margin: 0;
  flex: 1;
}
.search-input {
  flex: 1;
  min-width: 0;
  font: inherit;
  font-size: 0.8rem;
  padding: 0.2rem 0.4rem;
  border: 1px solid rgba(128, 128, 128, 0.4);
  border-radius: 4px;
  background: transparent;
  color: inherit;
}
.search-input:focus {
  outline: none;
  border-color: #3b82f6;
}
.search-toggle {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  flex-shrink: 0;
  padding: 0.2rem;
  border: none;
  border-radius: 4px;
  background: transparent;
  color: inherit;
  opacity: 0.6;
  cursor: pointer;
}
.search-toggle:hover {
  opacity: 1;
  background: rgba(128, 128, 128, 0.15);
}
ul {
  flex: 1;
  min-height: 0;
  overflow-y: auto;
  list-style: none;
  margin: 0;
  padding: 0;
}
.empty {
  padding: 1rem 0.75rem;
  opacity: 0.6;
}
</style>
```

- [ ] **Step 4: Type-check**

Run: `npm run build`
Expected: no TypeScript errors.

- [ ] **Step 5: Manual verification**

Run: `npm run tauri dev`

Check, in order:
1. Panel opens showing "Needle" title, no input visible.
2. Click the search icon → title is replaced by an empty text input,
   focused (typing works immediately, no extra click needed).
3. Type a substring matching an existing session's project folder name →
   list narrows to matching sessions only.
4. Type a substring that matches nothing → list disappears, message
   "Nenhuma sessão corresponde à busca." (or "No session matches the
   search." in EN) shows instead of the "no active sessions" message.
5. Press `Escape` → input closes, title "Needle" reappears, list returns
   to full.
6. Click the search icon again while a query is active → input closes,
   query clears, list returns to full (same as `Escape`).

- [ ] **Step 6: Commit**

```bash
git add src/components/SessionList.vue src/locales/pt-BR.json src/locales/en.json
git commit -m "feat: add search filter to session panel header"
```
