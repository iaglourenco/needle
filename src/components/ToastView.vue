<script setup lang="ts">
import { onMounted, ref } from "vue";
import { listen } from "@tauri-apps/api/event";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { api } from "../lib/api";

interface ToastPayload {
  title: string;
  body: string;
}

const title = ref("");
const body = ref("");

onMounted(() => {
  listen<ToastPayload>("toast-show", (event) => {
    title.value = event.payload.title;
    body.value = event.payload.body;
  }).catch(console.error);
});

async function onClick() {
  await api.openPanelFromToast();
  await getCurrentWindow().hide();
}
</script>

<template>
  <div class="toast" @click="onClick">
    <strong class="title">{{ title }}</strong>
    <p class="body">{{ body }}</p>
  </div>
</template>

<style scoped>
.toast {
  display: flex;
  flex-direction: column;
  gap: 0.2rem;
  height: 100%;
  box-sizing: border-box;
  padding: 0.6rem 0.75rem;
  font-family: system-ui, sans-serif;
  cursor: pointer;
  background: #1e1e1e;
  color: #fff;
}
.title {
  font-size: 0.85rem;
}
.body {
  margin: 0;
  font-size: 0.75rem;
  opacity: 0.75;
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}
</style>
