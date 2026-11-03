import { useCallback, useEffect, useRef, useState } from "react";
import { type App, appEvents, getApp, listApps } from "@/lib/apps";

const retry = (signal: AbortSignal, run: () => Promise<void>) => {
  const loop = async () => {
    while (!signal.aborted) {
      await run().catch(() => undefined);
      if (signal.aborted) return;
      await new Promise((resolve) => window.setTimeout(resolve, 2000));
    }
  };
  loop();
};

export function useApps() {
  const [apps, setApps] = useState<App[] | null>(null);
  const [error, setError] = useState<string | null>(null);

  const refresh = useCallback(
    () =>
      listApps()
        .then(({ apps }) => {
          setApps(apps);
          setError(null);
        })
        .catch((failure: Error) => setError(failure.message)),
    [],
  );

  const replace = useCallback((app: App) => {
    setApps((all) => (all ? all.map((one) => (one.id === app.id ? app : one)) : all));
  }, []);

  const drop = useCallback((id: string) => {
    setApps((all) => (all ? all.filter((one) => one.id !== id) : all));
  }, []);

  useEffect(() => {
    refresh();
    const controller = new AbortController();
    retry(controller.signal, () =>
      appEvents((event) => {
        setApps((all) =>
          all
            ? all.map((one) =>
                one.id === event.id
                  ? { ...one, running: event.running, started_here: event.started_here, pid: event.pid }
                  : one,
              )
            : all,
        );
        if (event.running) window.setTimeout(refresh, 400);
      }, controller.signal),
    );
    const slow = window.setInterval(refresh, 30000);
    return () => {
      controller.abort();
      window.clearInterval(slow);
    };
  }, [refresh]);

  return { apps, error, refresh, replace, drop, setError };
}

export function useApp(id: string) {
  const [app, setApp] = useState<App | null>(null);
  const [error, setError] = useState<{ status: number; message: string } | null>(null);
  const timer = useRef<number | undefined>(undefined);

  const refresh = useCallback(
    () =>
      getApp(id)
        .then(({ app }) => {
          setApp(app);
          setError(null);
        })
        .catch((failure: Error & { status?: number }) => setError({ status: failure.status ?? 0, message: failure.message })),
    [id],
  );

  useEffect(() => {
    refresh();
    const controller = new AbortController();
    retry(controller.signal, () =>
      appEvents((event) => {
        if (event.id !== id) return;
        setApp((one) => (one ? { ...one, running: event.running, started_here: event.started_here, pid: event.pid } : one));
        window.clearTimeout(timer.current);
        timer.current = window.setTimeout(refresh, 400);
      }, controller.signal),
    );
    return () => {
      controller.abort();
      window.clearTimeout(timer.current);
    };
  }, [id, refresh]);

  return { app, error, refresh, setApp };
}
