import { ref } from "vue";

const TICK_MS = 15_000;

const now = ref(Date.now());

let interval: ReturnType<typeof setInterval> | undefined;
if (typeof window !== "undefined") {
  interval = setInterval(() => {
    now.value = Date.now();
  }, TICK_MS);
}

export function useNow() {
  return now;
}
