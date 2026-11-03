import { useCallback, useEffect, useMemo, useState } from "react";
import { type Agent, listAgents, listRuns, type Run } from "@/lib/agents";
import { type Harness, listHarnesses } from "@/lib/harnesses";

export function useHarnesses() {
  const [harnesses, setHarnesses] = useState<Harness[] | null>(null);
  const refresh = useCallback(
    () =>
      listHarnesses()
        .then(({ harnesses }) => setHarnesses(harnesses.filter((harness) => harness.kind === "agent")))
        .catch(() => setHarnesses((had) => had ?? [])),
    [],
  );
  useEffect(() => {
    refresh();
  }, [refresh]);
  const byId = useMemo(() => new Map((harnesses ?? []).map((harness) => [harness.id, harness])), [harnesses]);
  return { harnesses, byId, refresh };
}

export function useAgents() {
  const [agents, setAgents] = useState<Agent[] | null>(null);
  const [error, setError] = useState<string | null>(null);
  const refresh = useCallback(
    () =>
      listAgents()
        .then(({ agents }) => {
          setAgents(agents);
          setError(null);
        })
        .catch((failure: Error) => setError(failure.message)),
    [],
  );
  useEffect(() => {
    refresh();
  }, [refresh]);
  return { agents, error, refresh, setError };
}

export function useRuns(agent?: string) {
  const [runs, setRuns] = useState<Run[] | null>(null);
  const refresh = useCallback(
    () =>
      listRuns()
        .then(({ runs }) => setRuns(agent === undefined ? runs : runs.filter((run) => run.agent === agent)))
        .catch(() => setRuns((had) => had ?? [])),
    [agent],
  );
  useEffect(() => {
    refresh();
  }, [refresh]);
  return { runs, refresh };
}
