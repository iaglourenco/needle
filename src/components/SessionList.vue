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
