import { createContext, useContext, useEffect, useState } from "react";
import { status as fetchStatus, type Status } from "@/lib/api";

export type StatusState = { status: Status | null; error: string | null };

export const StatusContext = createContext<StatusState>({ status: null, error: null });

export const useStatus = () => useContext(StatusContext);

export function usePolledStatus(intervalMs = 15000): StatusState {
  const [state, setState] = useState<StatusState>({ status: null, error: null });

  useEffect(() => {
    let alive = true;
    const load = () =>
      fetchStatus()
        .then((status) => alive && setState({ status, error: null }))
        .catch((failure: Error) => alive && setState((prev) => ({ ...prev, error: failure.message })));
    load();
    const timer = window.setInterval(load, intervalMs);
    return () => {
      alive = false;
      window.clearInterval(timer);
    };
  }, [intervalMs]);

  return state;
}
