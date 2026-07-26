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
}

export interface Settings {
  waitingTimeoutSecs: number;
  staleTimeoutSecs: number;
  autostart: boolean;
}

export interface HookStatus {
  registered: boolean;
  settings_path: string;
  exe_path: string;
}
