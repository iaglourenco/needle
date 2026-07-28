import { createI18n } from "vue-i18n";
import ptBR from "../locales/pt-BR.json";
import en from "../locales/en.json";

export const i18n = createI18n({
  legacy: false,
  locale: "pt-BR",
  fallbackLocale: "pt-BR",
  messages: {
    "pt-BR": ptBR,
    en,
  },
});
