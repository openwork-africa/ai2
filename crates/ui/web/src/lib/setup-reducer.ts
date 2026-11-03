import type { Input, LogLine, Phase, Snapshot, StepStatus, StepView, Warning } from "./setup";

export const STEP_LOG = 2000;

export type SetupState = {
  loaded: boolean;
  phase: Phase;
  check: boolean;
  ok?: boolean;
  ms?: number;
  error?: string;
  report?: unknown;
  warnings: Warning[];
  steps: StepView[];
  input?: Input;
  since: Record<string, number>;
};

export type SetupEvent = { event: string; id?: string; data: unknown };

export const initialState = (ids: string[] = []): SetupState => ({
  loaded: false,
  phase: "idle",
  check: false,
  warnings: [],
  steps: ids.map((id) => ({ id, status: "pending", warnings: [], info: {}, log: [] })),
  since: {},
});

const fromSnapshot = (state: SetupState, snapshot: Snapshot, now: number): SetupState => {
  const since: Record<string, number> = {};
  for (const step of snapshot.steps) {
    if (step.status === "running") since[step.id] = state.since[step.id] ?? now;
  }
  return {
    loaded: true,
    phase: snapshot.phase,
    check: snapshot.check,
    ok: snapshot.ok,
    ms: snapshot.ms,
    error: snapshot.error,
    report: snapshot.report,
    warnings: snapshot.warnings ?? [],
    steps: snapshot.steps.map((step) => ({ ...step, log: [...(step.log ?? [])] })),
    input: snapshot.input,
    since,
  };
};

type StepEvent = {
  id: string;
  status: StepStatus;
  ms?: number;
  warnings?: string[];
  info?: Record<string, unknown>;
  error?: string;
};

type LogEvent = LogLine & { step: string | null };

type DoneEvent = { ok: boolean; ms: number; check: boolean; report?: unknown; warnings: Warning[]; error?: string };

const withStep = (state: SetupState, id: string, change: (step: StepView) => StepView): SetupState => ({
  ...state,
  steps: state.steps.map((step) => (step.id === id ? change(step) : step)),
});

export const reduce = (state: SetupState, event: SetupEvent, now: number): SetupState => {
  switch (event.event) {
    case "snapshot":
      return fromSnapshot(state, event.data as Snapshot, now);
    case "step": {
      const data = event.data as StepEvent;
      const since = { ...state.since };
      if (data.status === "running") since[data.id] = now;
      else delete since[data.id];
      const next = withStep(state, data.id, (step) => ({
        ...step,
        status: data.status,
        ms: data.ms,
        warnings: data.warnings ?? [],
        info: data.info ?? {},
        error: data.error,
      }));
      return { ...next, since };
    }
    case "log": {
      const data = event.data as LogEvent;
      if (!data.step) return state;
      const line: LogLine = { t: data.t, stream: data.stream, line: data.line };
      return withStep(state, data.step, (step) => ({
        ...step,
        log: step.log.length >= STEP_LOG ? [...step.log.slice(1), line] : [...step.log, line],
      }));
    }
    case "input":
      return { ...state, input: event.data as Input };
    case "answered": {
      const data = event.data as { id: string };
      return state.input?.id === data.id ? { ...state, input: undefined } : state;
    }
    case "done": {
      const data = event.data as DoneEvent;
      return {
        ...state,
        phase: "done",
        ok: data.ok,
        ms: data.ms,
        check: data.check,
        report: data.report,
        warnings: data.warnings ?? [],
        error: data.error,
        input: undefined,
        since: {},
      };
    }
    default:
      return state;
  }
};

export const finished = (status: StepStatus) =>
  status === "ready" || status === "unsupported" || status === "failed" || status === "skipped";

export const progress = (state: SetupState) => {
  const total = state.steps.length;
  const done = state.steps.filter((step) => finished(step.status)).length;
  const running = state.steps.findIndex((step) => step.status === "running");
  const current = state.phase === "done" ? total : running >= 0 ? running + 1 : Math.min(done + 1, total);
  return { total, done, current, value: done + (running >= 0 ? 0.5 : 0) };
};
