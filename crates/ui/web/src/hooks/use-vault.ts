import { useCallback, useEffect, useState, useSyncExternalStore } from "react";
import { api } from "@/lib/api";
import {
  type Approval,
  batchBody,
  type Change,
  madeTokens,
  type NewToken,
  type PresenceKind,
  presenceKind,
  type VaultStatus,
  withPresence,
} from "@/lib/vault";

const PASSPHRASE_HEADER = "x-vault-passphrase";

const store = <S>(initial: S) => {
  let state = initial;
  const listeners = new Set<() => void>();
  return {
    get: () => state,
    set: (next: S) => {
      state = next;
      listeners.forEach((listener) => listener());
    },
    subscribe: (listener: () => void) => {
      listeners.add(listener);
      return () => {
        listeners.delete(listener);
      };
    },
    listeners,
  };
};

type Ask = { action: string; resolve: (passphrase: string | null) => void };

type PresenceState = { kind: PresenceKind; waiting: number; action: string; ask: Ask | null };

const presence = store<PresenceState>({ kind: "passphrase", waiting: 0, action: "", ask: null });

const patchPresence = (next: Partial<PresenceState>) => presence.set({ ...presence.get(), ...next });

export const usePresence = () => useSyncExternalStore(presence.subscribe, presence.get);

export const answerPassphrase = (passphrase: string | null) => {
  const ask = presence.get().ask;
  patchPresence({ ask: null });
  ask?.resolve(passphrase);
};

const revision = store(0);

const bump = () => revision.set(revision.get() + 1);

type Method = "GET" | "POST" | "PUT" | "DELETE";

export const vault = <T>(method: Method, path: string, body?: unknown, action: string | null = "make this change") => {
  const write = method !== "GET" && action !== null;
  const asking = action ?? "";
  return withPresence<T>(
    write,
    {
      kind: presence.get().kind,
      waiting: (on) => {
        const now = presence.get();
        patchPresence({ waiting: now.waiting + (on ? 1 : -1), action: on ? asking : now.action });
      },
      ask: () => new Promise<string | null>((resolve) => patchPresence({ ask: { action: asking, resolve } })),
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
  const seen = useSyncExternalStore(revision.subscribe, revision.get);

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
  }, [path, intervalMs, reload, seen]);

  return { ...state, reload };
}

export const useVaultStatus = () => {
  const query = useVaultQuery<VaultStatus>("/status");
  useEffect(() => {
    if (query.data) patchPresence({ kind: presenceKind(query.data) });
  }, [query.data]);
  return query;
};

export type Queued = { id: number; change: Change };

const queue = store<Queued[]>([]);
let nextId = 1;

export const useQueue = () => useSyncExternalStore(queue.subscribe, queue.get);

export const enqueue = (change: Change) => queue.set([...queue.get(), { id: nextId++, change }]);

export const dequeue = (id: number) => queue.set(queue.get().filter((entry) => entry.id !== id));

export const discardQueue = () => queue.set([]);

export const applyQueue = async (): Promise<NewToken[]> => {
  const entries = queue.get();
  if (!entries.length) return [];
  const count = entries.length;
  const out = await vault<{ results: unknown[] }>(
    "POST",
    "/batch",
    batchBody(entries.map((entry) => entry.change)),
    count === 1 ? "apply 1 change" : `apply ${count} changes`,
  );
  const sent = new Set(entries.map((entry) => entry.id));
  queue.set(queue.get().filter((entry) => !sent.has(entry.id)));
  bump();
  return madeTokens(out.results ?? []);
};

const POLL_MS = 3000;

type ApprovalsState = { approvals: Approval[] | null; error: string | null };

const approvals = store<ApprovalsState>({ approvals: null, error: null });
let approvalTimer = 0;

export const refreshApprovals = async () => {
  try {
    const data = await api<{ approvals: Approval[] }>("GET", "/api/vault/approvals");
    approvals.set({ approvals: data.approvals, error: null });
  } catch (failure) {
    approvals.set({ approvals: approvals.get().approvals, error: (failure as Error).message });
  }
};

const subscribeApprovals = (listener: () => void) => {
  const stop = approvals.subscribe(listener);
  if (approvals.listeners.size === 1) {
    refreshApprovals();
    approvalTimer = window.setInterval(() => !document.hidden && refreshApprovals(), POLL_MS);
  }
  return () => {
    stop();
    if (!approvals.listeners.size) window.clearInterval(approvalTimer);
  };
};

export const useApprovals = () => useSyncExternalStore(subscribeApprovals, approvals.get);

export const presenceNow = () => presence.get().kind;
