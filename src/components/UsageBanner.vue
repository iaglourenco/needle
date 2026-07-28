<script setup lang="ts">
import { computed, onMounted, onUnmounted, ref } from "vue";
import { useI18n } from "vue-i18n";
import { api } from "../lib/api";
import { useSessionsStore } from "../stores/sessions";
import type { AccountUsage } from "../lib/types";

const REFRESH_MS = 60_000;

// Paleta de status fixa (nunca reaproveitada como cor categórica) — mesmos
// tons validados pra contraste em superfície escura.
const SEVERITY = {
  good: "#0ca30c",
  warning: "#fab219",
  serious: "#ec835a",
  critical: "#d03b3b",
};

const { t, locale } = useI18n();
const store = useSessionsStore();

const usage = ref<AccountUsage | null>(null);
const failed = ref(false);
let interval: ReturnType<typeof setInterval> | undefined;

async function refresh() {
  try {
    usage.value = await api.getAccountUsage();
    failed.value = false;
  } catch {
    failed.value = true;
  }
}

onMounted(() => {
  refresh();
  interval = setInterval(refresh, REFRESH_MS);
});

onUnmounted(() => {
  if (interval) clearInterval(interval);
});

const totalCost = computed(() =>
  store.sessions.reduce((sum, session) => sum + (session.cost_usd ?? 0), 0),
);

function formatReset(iso: string | null, opts: Intl.DateTimeFormatOptions) {
  if (!iso) return "";
  const date = new Date(iso);
  if (Number.isNaN(date.getTime())) return "";
  return new Intl.DateTimeFormat(locale.value, opts).format(date);
}

function severityColor(pct: number) {
  if (pct >= 95) return SEVERITY.critical;
  if (pct >= 80) return SEVERITY.serious;
  if (pct >= 50) return SEVERITY.warning;
  return SEVERITY.good;
}

const fiveHourPct = computed(() =>
  Math.min(100, Math.round(usage.value?.fiveHour?.utilization ?? 0)),
);
const fiveHourColor = computed(() => severityColor(fiveHourPct.value));
const fiveHourReset = computed(() =>
  formatReset(usage.value?.fiveHour?.resetsAt ?? null, {
    hour: "numeric",
    minute: "2-digit",
  }),
);

const sevenDayPct = computed(() =>
  Math.min(100, Math.round(usage.value?.sevenDay?.utilization ?? 0)),
);
const sevenDayColor = computed(() => severityColor(sevenDayPct.value));
const sevenDayReset = computed(() =>
  formatReset(usage.value?.sevenDay?.resetsAt ?? null, {
    month: "short",
    day: "numeric",
  }),
);

const costLabel = computed(() => `$${totalCost.value.toFixed(2)}`);
</script>

<template>
  <div class="usage-banner">
    <template v-if="usage && !failed">
      <div class="tile">
        <span class="label">{{ t("usage.fiveHour") }}</span>
        <span class="value">{{ fiveHourPct }}%</span>
        <div class="meter">
          <div
            class="meter-fill"
            :style="{ width: `${fiveHourPct}%`, background: fiveHourColor }"
          ></div>
        </div>
        <span v-if="fiveHourReset" class="reset">{{
          t("usage.resets", { time: fiveHourReset })
        }}</span>
      </div>

      <div class="tile">
        <span class="label">{{ t("usage.weekly") }}</span>
        <span class="value">{{ sevenDayPct }}%</span>
        <div class="meter">
          <div
            class="meter-fill"
            :style="{ width: `${sevenDayPct}%`, background: sevenDayColor }"
          ></div>
        </div>
        <span v-if="sevenDayReset" class="reset">{{
          t("usage.resets", { time: sevenDayReset })
        }}</span>
      </div>

      <div class="tile">
        <span class="label">{{ t("usage.cost") }}</span>
        <span class="value cost">{{ costLabel }}</span>
      </div>
    </template>
    <span v-else-if="failed" class="unavailable">{{
      t("usage.unavailable")
    }}</span>
  </div>
</template>

<style scoped>
.usage-banner {
  display: grid;
  grid-template-columns: repeat(auto-fit, minmax(88px, 1fr));
  gap: 0.6rem;
  padding: 0.65rem 0.75rem;
  border-bottom: 1px solid rgba(128, 128, 128, 0.25);
  background: rgba(255, 255, 255, 0.02);
}
.tile {
  display: flex;
  flex-direction: column;
  padding: 0.5rem 0.6rem;
  border-radius: 8px;
  background: rgba(255, 255, 255, 0.035);
  border: 1px solid rgba(255, 255, 255, 0.06);
}
.label {
  font-size: 0.68rem;
  color: #c3c2b7;
  margin-bottom: 0.15rem;
}
.value {
  font-size: 1.15rem;
  font-weight: 600;
  color: #fff;
  line-height: 1.2;
}
.value.cost {
  color: #0ca30c;
}
.meter {
  height: 4px;
  border-radius: 2px;
  background: rgba(255, 255, 255, 0.08);
  margin: 0.4rem 0 0.3rem;
  overflow: hidden;
}
.meter-fill {
  height: 100%;
  border-radius: 2px;
  transition: width 0.3s ease;
}
.reset {
  font-size: 0.65rem;
  color: #898781;
}
.unavailable {
  grid-column: 1 / -1;
  font-size: 0.75rem;
  color: #898781;
  padding: 0.2rem 0;
}
</style>
