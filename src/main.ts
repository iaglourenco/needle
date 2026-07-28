import { createApp } from "vue";
import { createPinia } from "pinia";
import App from "./App.vue";
import "./style.css";
import { i18n } from "./i18n";
import { api } from "./lib/api";

async function bootstrap() {
  try {
    const settings = await api.getSettings();
    i18n.global.locale.value = settings.language;
  } catch (err) {
    console.error("failed to load settings, using default locale", err);
  }
  createApp(App).use(createPinia()).use(i18n).mount("#app");
}

bootstrap();
