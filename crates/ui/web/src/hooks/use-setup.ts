import { useCallback, useEffect, useReducer, useState } from "react";
import { ApiError, events } from "@/lib/api";
import { answerInput, cancelInput, getHost, getStepsInfo, type Host, startSetup, type StepInfo } from "@/lib/setup";
import { initialState, reduce, type SetupEvent, type SetupState } from "@/lib/setup-reducer";
import { STEPS } from "@/copy/setup";

const RETRY_MS = 1500;

const reducer = (state: SetupState, event: SetupEvent) => reduce(state, event, Date.now());

export function useSetup() {
  const [state, dispatch] = useReducer(reducer, STEPS, initialState);
  const [connected, setConnected] = useState(false);
  const [host, setHost] = useState<Host | null>(null);
  const [info, setInfo] = useState<Record<string, StepInfo>>({});
  const [failure, setFailure] = useState<string | null>(null);

  useEffect(() => {
    let alive = true;
    getHost()
      .then((value) => alive && setHost(value))
      .catch(() => undefined);
    getStepsInfo()
      .then((value) => alive && setInfo(Object.fromEntries(value.steps.map((step) => [step.id, step]))))
      .catch(() => undefined);
    return () => {
      alive = false;
    };
  }, []);

  useEffect(() => {
    const abort = new AbortController();
    let lastEventId: string | undefined;
    const connect = async () => {
      while (!abort.signal.aborted) {
        try {
          await events(
            "/api/setup/events",
            (event) => {
              setConnected(true);
              if (event.id) lastEventId = event.id;
              dispatch(event);
            },
            { signal: abort.signal, lastEventId },
          );
        } catch (error) {
          if (abort.signal.aborted) return;
          if (error instanceof ApiError && error.status === 403) return;
        }
        setConnected(false);
        await new Promise((resolve) => setTimeout(resolve, RETRY_MS));
      }
    };
    connect();
    return () => abort.abort();
  }, []);

  const run = useCallback(async (body: { check?: boolean; from?: string }) => {
    setFailure(null);
    try {
      await startSetup(body);
    } catch (error) {
      setFailure((error as Error).message);
    }
  }, []);

  const answer = useCallback((id: string, values: string[]) => answerInput(id, values), []);
  const cancel = useCallback((id: string) => cancelInput(id), []);

  return { state, connected, host, info, failure, run, answer, cancel };
}
