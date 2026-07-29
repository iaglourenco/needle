import { createApp } from "vue";
import { createPinia } from "pinia";
import { getCurrentWindow } from "@tauri-apps/api/window";
import App from "./App.vue";
import ToastView from "./components/ToastView.vue";
import "./style.css";
import { i18n } from "./i18n";
import { api } from "./lib/api";

async function bootstrap() {
  if (getCurrentWindow().label === "toast") {
    createApp(ToastView).mount("#app");
    return;
  }

  try {
    const settings = await api.getSettings();
    i18n.global.locale.value = settings.language;
  } catch (err) {
    console.error("failed to load settings, using default locale", err);
  }
  createApp(App).use(createPinia()).use(i18n).mount("#app");
}

bootstrap();
