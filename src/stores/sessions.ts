import { defineStore } from "pinia";
import { listen } from "@tauri-apps/api/event";
import { invoke } from "@tauri-apps/api/core";
import type { Session } from "../lib/types";

export const useSessionsStore = defineStore("sessions", {
  state: () => ({
    sessions: [] as Session[],
    ready: false,
  }),
  actions: {
    async init() {
      this.sessions = await invoke<Session[]>("list_sessions");
      this.ready = true;

      await listen<Session>("session-updated", (event) => {
        const incoming = event.payload;
        const idx = this.sessions.findIndex(
          (s) => s.session_id === incoming.session_id,
        );
        if (idx === -1) {
          this.sessions.push(incoming);
        } else {
          this.sessions[idx] = incoming;
        }
      });

      await listen<string>("session-removed", (event) => {
        const sessionId = event.payload;
        this.sessions = this.sessions.filter(
          (s) => s.session_id !== sessionId,
        );
      });
    },
  },
});
