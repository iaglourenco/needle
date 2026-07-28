<script setup lang="ts">
import { computed } from "vue";
import { useI18n } from "vue-i18n";
import { useNow } from "../composables/useNow";
import type { Session } from "../lib/types";

const props = defineProps<{ session: Session }>();
const { t } = useI18n();
const now = useNow();

const stateColor: Record<Session["state"], string> = {
  Running: "#3b82f6",
  WaitingInput: "#eab308",
  NeedsAttention: "#ef4444",
  Idle: "#22c55e",
  Error: "#ef4444",
  Stale: "#6b7280",
  Ended: "#6b7280",
};

const projectName = computed(() => {
  const parts = props.session.cwd.split(/[\\/]/).filter(Boolean);
  return parts[parts.length - 1] ?? props.session.cwd;
});

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
.snippet {
  font-size: 0.8rem;
  opacity: 0.6;
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}
</style>
