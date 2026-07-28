<script setup lang="ts">
import { computed, onMounted } from "vue";
import { useI18n } from "vue-i18n";
import { useSessionsStore } from "../stores/sessions";
import SessionItem from "./SessionItem.vue";

const store = useSessionsStore();
const { t } = useI18n();

onMounted(() => {
  if (!store.ready) store.init();
});

const sorted = computed(() =>
  [...store.sessions].sort((a, b) => b.last_event_at - a.last_event_at),
);
</script>

<template>
  <div class="session-list">
    <header>
      <h1>Needle</h1>
    </header>
    <ul v-if="sorted.length">
      <SessionItem
        v-for="session in sorted"
        :key="session.session_id"
        :session="session"
      />
    </ul>
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
  flex-shrink: 0;
  padding: 0.5rem 0.75rem;
  border-bottom: 1px solid rgba(128, 128, 128, 0.3);
}
h1 {
  font-size: 0.95rem;
  margin: 0;
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
