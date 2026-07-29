import { invoke } from "@tauri-apps/api/core";
import type { AccountUsage, HookStatus, Session, Settings } from "./types";

export const api = {
  listSessions: () => invoke<Session[]>("list_sessions"),
  getSettings: () => invoke<Settings>("get_settings"),
  saveSettings: (settings: Settings) =>
    invoke<void>("save_settings", { newSettings: settings }),
  getHookStatus: () => invoke<HookStatus>("get_hook_status"),
  reconfigureHooks: () => invoke<boolean>("reconfigure_hooks"),
  removeHooks: () => invoke<boolean>("remove_hooks_command"),
  getAccountUsage: () => invoke<AccountUsage>("get_account_usage"),
  deleteSession: (sessionId: string) =>
    invoke<boolean>("delete_session", { sessionId }),
  openPanelFromToast: () => invoke<void>("open_panel_from_toast"),
};
