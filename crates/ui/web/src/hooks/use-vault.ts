import { useCallback, useEffect, useState, useSyncExternalStore } from "react";
import { api } from "@/lib/api";
import { type Approval, isMac, type VaultStatus, withPresence } from "@/lib/vault";

const PASSPHRASE_HEADER = "x-vault-passphrase";

type Ask = { action: string; resolve: (passphrase: string | null) => void };

type PresenceState = { waiting: number; action: string; ask: Ask | null };

let presence: PresenceState = { waiting: 0, action: "", ask: null };
let platform = "";
const presenceListeners = new Set<() => void>();

const setPresence = (next: Partial<PresenceState>) => {
  presence = { ...presence, ...next };
  presenceListeners.forEach((listener) => listener());
};

const subscribePresence = (listener: () => void) => {
  presenceListeners.add(listener);
  return () => {
    presenceListeners.delete(listener);
  };
};

export const usePresence = () => useSyncExternalStore(subscribePresence, () => presence);

export const setPlatform = (value: string) => {
  platform = value;
};

export const answerPassphrase = (passphrase: string | null) => {
  const ask = presence.ask;
  setPresence({ ask: null });
  ask?.resolve(passphrase);
};

type Method = "GET" | "POST" | "PUT" | "DELETE";

export const vault = <T>(method: Method, path: string, body?: unknown, action: string | null = "make this change") => {
  const write = method !== "GET" && action !== null;
  const asking = action ?? "";
  return withPresence<T>(
    write,
    {
      mac: isMac(platform),
      waiting: (on) => setPresence({ waiting: presence.waiting + (on ? 1 : -1), action: on ? asking : presence.action }),
      ask: () => new Promise<string | null>((resolve) => setPresence({ ask: { action: asking, resolve } })),
    },
    (passphrase) => api<T>(method, `/api/vault${path}`, body, passphrase ? { [PASSPHRASE_HEADER]: passphrase } : {}),
  );
};

export type Query<T> = { data: T | null; error: string | null; loading: boolean; reload: () => Promise<void> };

export function useVaultQuery<T>(path: string | null, intervalMs = 0): Query<T> {
  const [state, setState] = useState<{ data: T | null; error: string | null; loading: boolean }>({
    data: null,
    error: null,
    loading: path !== null,
  });

  const reload = useCallback(async () => {
    if (path === null) return;
    try {
      const data = await vault<T>("GET", path);
      setState({ data, error: null, loading: false });
    } catch (failure) {
      setState((prev) => ({ ...prev, error: (failure as Error).message, loading: false }));
    }
  }, [path]);

  useEffect(() => {
    if (path === null) return;
    let alive = true;
    setState((prev) => ({ ...prev, loading: true }));
    const load = () => alive && !document.hidden && reload();
    reload();
    const timer = intervalMs ? window.setInterval(load, intervalMs) : 0;
    return () => {
      alive = false;
      if (timer) window.clearInterval(timer);
    };
  }, [path, intervalMs, reload]);

  return { ...state, reload };
}

export const useVaultStatus = () => {
  const query = useVaultQuery<VaultStatus>("/status");
  useEffect(() => {
    if (query.data) setPlatform(query.data.platform);
  }, [query.data]);
  return query;
};

const POLL_MS = 3000;

type ApprovalsState = { approvals: Approval[] | null; error: string | null };

let approvals: ApprovalsState = { approvals: null, error: null };
const approvalListeners = new Set<() => void>();
let approvalTimer = 0;

const publishApprovals = (next: ApprovalsState) => {
  approvals = next;
  approvalListeners.forEach((listener) => listener());
};

export const refreshApprovals = async () => {
  try {
    const data = await api<{ approvals: Approval[] }>("GET", "/api/vault/approvals");
    publishApprovals({ approvals: data.approvals, error: null });
  } catch (failure) {
    publishApprovals({ approvals: approvals.approvals, error: (failure as Error).message });
  }
};

const subscribeApprovals = (listener: () => void) => {
  approvalListeners.add(listener);
  if (approvalListeners.size === 1) {
    refreshApprovals();
    approvalTimer = window.setInterval(() => !document.hidden && refreshApprovals(), POLL_MS);
  }
  return () => {
    approvalListeners.delete(listener);
    if (!approvalListeners.size) window.clearInterval(approvalTimer);
  };
};

export const useApprovals = () => useSyncExternalStore(subscribeApprovals, () => approvals);
