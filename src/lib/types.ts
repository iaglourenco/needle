export type SessionState =
  | "Running"
  | "WaitingInput"
  | "NeedsAttention"
  | "Idle"
  | "Error"
  | "Stale"
  | "Ended";

export interface Session {
  session_id: string;
  cwd: string;
  started_at: number;
  last_event_at: number;
  state: SessionState;
  last_message_snippet: string | null;
  model: string | null;
  cost_usd: number | null;
}

export interface WindowUsage {
  utilization: number;
  resetsAt: string | null;
}

export interface AccountUsage {
  fiveHour: WindowUsage | null;
  sevenDay: WindowUsage | null;
}

export interface Settings {
  waitingTimeoutSecs: number;
  staleTimeoutSecs: number;
  autostart: boolean;
  language: "pt-BR" | "en";
}

export interface HookStatus {
  registered: boolean;
  settings_path: string;
  exe_path: string;
}
