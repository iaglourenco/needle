<script setup lang="ts">
import { onMounted, ref } from "vue";
import { useI18n } from "vue-i18n";
import { api } from "../lib/api";
import type { HookStatus, Settings } from "../lib/types";

const settings = ref<Settings | null>(null);
const hookStatus = ref<HookStatus | null>(null);
const savedFlash = ref(false);
const busy = ref(false);
const { t, locale } = useI18n();

async function refresh() {
  settings.value = await api.getSettings();
  hookStatus.value = await api.getHookStatus();
  if (settings.value) locale.value = settings.value.language;
}

onMounted(refresh);

async function save() {
  if (!settings.value) return;
  busy.value = true;
  try {
    await api.saveSettings(settings.value);
    savedFlash.value = true;
    setTimeout(() => (savedFlash.value = false), 1500);
  } finally {
    busy.value = false;
  }
}

async function reconfigure() {
  busy.value = true;
  try {
    await api.reconfigureHooks();
    hookStatus.value = await api.getHookStatus();
  } finally {
    busy.value = false;
  }
}

async function removeHooks() {
  busy.value = true;
  try {
    await api.removeHooks();
    hookStatus.value = await api.getHookStatus();
  } finally {
    busy.value = false;
  }
}
</script>

<template>
  <div v-if="settings" class="settings">
    <section>
      <h2>{{ t("settings.hooksTitle") }}</h2>
      <p class="status" :class="{ ok: hookStatus?.registered }">
        {{ hookStatus?.registered ? t("settings.hooksRegistered") : t("settings.hooksNotRegistered") }}
      </p>
      <p class="path">{{ hookStatus?.settings_path }}</p>
      <div class="actions">
        <button :disabled="busy" @click="reconfigure">{{ t("settings.reconfigure") }}</button>
        <button :disabled="busy" class="danger" @click="removeHooks">
          {{ t("settings.remove") }}
        </button>
      </div>
    </section>

    <section>
      <h2>{{ t("settings.thresholdsTitle") }}</h2>
      <label>
        {{ t("settings.waitingTimeoutLabel") }}
        <input
          v-model.number="settings.waitingTimeoutSecs"
          type="number"
          min="10"
        />
      </label>
      <label>
        {{ t("settings.staleTimeoutLabel") }}
        <input
          :value="Math.round(settings.staleTimeoutSecs / 60)"
          type="number"
          min="1"
          @input="
            settings.staleTimeoutSecs =
              Number(($event.target as HTMLInputElement).value) * 60
          "
        />
      </label>
    </section>

    <section>
      <h2>{{ t("settings.generalTitle") }}</h2>
      <label class="checkbox">
        <input v-model="settings.autostart" type="checkbox" />
        {{ t("settings.autostart") }}
      </label>
      <label>
        {{ t("settings.language") }}
        <select v-model="settings.language" @change="locale = settings.language">
          <option value="pt-BR">Português (Brasil)</option>
          <option value="en">English</option>
        </select>
      </label>
    </section>

    <div class="footer">
      <button :disabled="busy" class="primary" @click="save">{{ t("settings.save") }}</button>
      <span v-if="savedFlash" class="saved">{{ t("settings.saved") }}</span>
    </div>
  </div>
</template>

<style scoped>
.settings {
  flex: 1;
  min-height: 0;
  overflow-y: auto;
  padding: 0.75rem;
  font-family: system-ui, sans-serif;
  font-size: 0.85rem;
  display: flex;
  flex-direction: column;
  gap: 1rem;
}
h2 {
  font-size: 0.85rem;
  margin: 0 0 0.4rem;
  opacity: 0.8;
}
label {
  display: flex;
  flex-direction: column;
  gap: 0.2rem;
  margin-bottom: 0.5rem;
}
label.checkbox {
  flex-direction: row;
  align-items: center;
  gap: 0.4rem;
}
input[type="number"] {
  width: 100%;
  box-sizing: border-box;
  padding: 0.3rem;
}
select {
  width: 100%;
  box-sizing: border-box;
  padding: 0.3rem;
}
.status {
  font-weight: 600;
  color: #ef4444;
  margin: 0;
}
.status.ok {
  color: #22c55e;
}
.path {
  font-size: 0.7rem;
  opacity: 0.5;
  word-break: break-all;
  margin: 0.2rem 0 0.5rem;
}
.actions {
  display: flex;
  gap: 0.5rem;
}
button {
  cursor: pointer;
  border: 1px solid rgba(128, 128, 128, 0.4);
  background: transparent;
  border-radius: 4px;
  padding: 0.35rem 0.7rem;
  font-size: 0.8rem;
}
button.primary {
  background: #3b82f6;
  border-color: #3b82f6;
  color: white;
}
button.danger {
  color: #ef4444;
  border-color: #ef4444;
}
.footer {
  display: flex;
  align-items: center;
  gap: 0.5rem;
}
.saved {
  color: #22c55e;
  font-size: 0.8rem;
}
</style>
