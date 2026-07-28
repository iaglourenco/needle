<script setup lang="ts">
import { computed, ref } from "vue";
import { useI18n } from "vue-i18n";
import { api } from "../lib/api";
import { projectNameOf } from "../lib/session";
import { useNow } from "../composables/useNow";
import type { Session } from "../lib/types";

const props = defineProps<{ session: Session }>();
const { t } = useI18n();
const now = useNow();
const deleting = ref(false);

async function onDelete() {
  if (deleting.value) return;
  deleting.value = true;
  try {
    await api.deleteSession(props.session.session_id);
  } finally {
    deleting.value = false;
  }
}

const stateColor: Record<Session["state"], string> = {
  Running: "#3b82f6",
  WaitingInput: "#eab308",
  NeedsAttention: "#ef4444",
  Idle: "#22c55e",
  Error: "#ef4444",
  Stale: "#6b7280",
  Ended: "#6b7280",
};

const projectName = computed(() => projectNameOf(props.session.cwd));

const elapsed = computed(() => {
  const seconds = Math.max(
    0,
    Math.floor(now.value / 1000 - props.session.last_event_at),
  );
  if (seconds < 60) return `${seconds}s`;
  const minutes = Math.floor(seconds / 60);
  if (minutes < 60) return `${minutes}min`;
  return `${Math.floor(minutes / 60)}h`;
});

const modelLabel = computed(() => {
  const model = props.session.model;
  return model ? model.replace(/^claude-/, "") : null;
});

const costLabel = computed(() => {
  const cost = props.session.cost_usd;
  if (!cost) return null;
  return `$${cost.toFixed(2)}`;
});
</script>

<template>
  <li class="session-item">
    <span
      class="dot"
      :style="{ backgroundColor: stateColor[session.state] }"
    ></span>
    <div class="info">
      <div class="row">
        <span class="project">{{ projectName }}</span>
        <span class="state">{{ t(`sessions.states.${session.state}`) }}</span>
        <span v-if="modelLabel" class="model">{{ modelLabel }}</span>
        <span v-if="costLabel" class="cost">{{ costLabel }}</span>
        <span class="elapsed">{{ elapsed }}</span>
        <button
          v-if="session.state === 'Stale'"
          type="button"
          class="delete-btn"
          :disabled="deleting"
          :title="t('sessions.delete')"
          :aria-label="t('sessions.delete')"
          @click="onDelete"
        >
          <svg viewBox="0 0 16 16" width="13" height="13" aria-hidden="true">
            <path
              d="M4 4.5h8M6.5 4.5v-1a1 1 0 0 1 1-1h1a1 1 0 0 1 1 1v1M6 7v4M10 7v4M4.5 4.5l.6 8a1 1 0 0 0 1 .9h3.8a1 1 0 0 0 1-.9l.6-8"
              fill="none"
              stroke="currentColor"
              stroke-width="1.2"
              stroke-linecap="round"
              stroke-linejoin="round"
            />
          </svg>
        </button>
      </div>
      <div v-if="session.last_message_snippet" class="snippet">
        {{ session.last_message_snippet }}
      </div>
    </div>
  </li>
</template>

<style scoped>
.session-item {
  display: flex;
  align-items: flex-start;
  gap: 0.5rem;
  padding: 0.5rem 0.75rem;
  border-bottom: 1px solid rgba(128, 128, 128, 0.2);
}
.dot {
  width: 0.6rem;
  height: 0.6rem;
  border-radius: 50%;
  margin-top: 0.35rem;
  flex-shrink: 0;
}
.info {
  flex: 1;
  min-width: 0;
}
.row {
  display: flex;
  gap: 0.5rem;
  align-items: baseline;
}
.project {
  font-weight: 600;
}
.state {
  font-size: 0.75rem;
  opacity: 0.7;
}
.model {
  font-size: 0.7rem;
  opacity: 0.6;
  padding: 0.05rem 0.35rem;
  border: 1px solid currentColor;
  border-radius: 0.75rem;
}
.cost {
  font-size: 0.7rem;
  opacity: 0.6;
}
.elapsed {
  font-size: 0.75rem;
  opacity: 0.5;
  margin-left: auto;
}
.delete-btn {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  padding: 0.15rem;
  border: none;
  border-radius: 4px;
  background: transparent;
  color: inherit;
  cursor: pointer;
  opacity: 0;
  pointer-events: none;
  transition: opacity 0.15s ease;
}
.session-item:hover .delete-btn {
  opacity: 0.55;
  pointer-events: auto;
}
.delete-btn:hover {
  opacity: 1;
  color: #ef4444;
  background: rgba(239, 68, 68, 0.12);
}
.delete-btn:disabled {
  opacity: 0.25;
  cursor: default;
  pointer-events: none;
}
.snippet {
  font-size: 0.8rem;
  opacity: 0.6;
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}
</style>
