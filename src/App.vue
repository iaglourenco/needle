<script setup lang="ts">
import { onMounted, ref } from "vue";
import { listen } from "@tauri-apps/api/event";
import SessionList from "./components/SessionList.vue";
import SettingsView from "./components/SettingsView.vue";

type View = "sessions" | "settings";

const view = ref<View>("sessions");

onMounted(() => {
  listen<View>("show-view", (event) => {
    view.value = event.payload;
  });
});
</script>

<template>
  <div class="app">
    <nav>
      <button :class="{ active: view === 'sessions' }" @click="view = 'sessions'">
        Sessões
      </button>
      <button :class="{ active: view === 'settings' }" @click="view = 'settings'">
        Configurações
      </button>
    </nav>
    <SessionList v-if="view === 'sessions'" />
    <SettingsView v-else />
  </div>
</template>

<style scoped>
.app {
  display: flex;
  flex-direction: column;
  height: 100%;
  overflow: hidden;
}
nav {
  display: flex;
  flex-shrink: 0;
  border-bottom: 1px solid rgba(128, 128, 128, 0.3);
}
nav button {
  flex: 1;
  padding: 0.5rem;
  border: none;
  background: transparent;
  cursor: pointer;
  font-size: 0.8rem;
  opacity: 0.6;
  font-family: system-ui, sans-serif;
}
nav button.active {
  opacity: 1;
  font-weight: 600;
  border-bottom: 2px solid #3b82f6;
}
</style>
