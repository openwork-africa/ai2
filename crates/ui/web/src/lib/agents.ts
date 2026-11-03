import { api, events } from "@/lib/api";

export type Personality = {
  content: string;
  seed?: string;
  enhanced?: boolean;
  enhanced_at?: string;
  enhance_failed?: string;
};

export type Agent = {
  name: string;
  harness: string;
  model?: string;
  personality?: Personality;
  folder?: string;
  description?: string;
  created: string;
  updated: string;
};

export type NewAgent = {
  name: string;
  harness: string;
  model?: string;
  personality?: string;
  folder?: string;
};

export type Enhanced = {
  status: "enhanced" | "skipped" | "failed";
  reason?: string;
  agent: Agent;
};

export type RunState = "running" | "reviewing" | "ended";

export type Run = {
  id: string;
  agent?: string;
  harness: string;
  model?: string;
  folder: string;
  prompt: string;
  state: RunState;
  started: string;
  ended: string | null;
  exit: number | null;
  log: string;
  review: string | null;
  review_error?: string;
  error?: string;
};

export type RunAsk = {
  agent?: string;
  harness?: string;
  model?: string;
  personality?: string;
  folder?: string;
  prompt: string;
};

export type RunEvent =
  | { event: "started"; run: Run }
  | { event: "line"; stream: "out" | "err"; text: string }
  | { event: "exited"; exit: number | null; error?: string }
  | { event: "review"; text: string }
  | { event: "reviewed"; hash: string | null; error?: string }
  | { event: "done"; run: Run };

const named = (name: string) => `/api/agents/${encodeURIComponent(name)}`;

export const listAgents = () => api<{ agents: Agent[] }>("GET", "/api/agents");
export const getAgent = (name: string) => api<{ agent: Agent }>("GET", named(name));
export const saveAgent = (agent: NewAgent) => api<{ agent: Agent }>("POST", "/api/agents", agent);
export const removeAgent = (name: string) => api<{ removed: string }>("DELETE", named(name));
export const enhanceAgent = (name: string) => api<Enhanced>("POST", `${named(name)}/enhance`);

export const listRuns = () => api<{ runs: Run[] }>("GET", "/api/runs");
export const getRun = (id: string) => api<{ run: Run }>("GET", `/api/runs/${encodeURIComponent(id)}`);
export const startRun = (ask: RunAsk) => api<{ run: Run }>("POST", "/api/runs", ask);

export const followRun = (
  id: string,
  onEvent: (event: RunEvent, at: string | undefined) => void,
  options: { signal?: AbortSignal; lastEventId?: string } = {},
) => events(`/api/runs/${encodeURIComponent(id)}/events`, (sent) => onEvent(sent.data as RunEvent, sent.id), options);

export const NAME_RULE = /^[a-z0-9][a-z0-9-]{0,23}$/;

export const asName = (text: string) =>
  text
    .toLowerCase()
    .replace(/\s+/g, "-")
    .replace(/[^a-z0-9-]/g, "")
    .slice(0, 24);

export const personalityLine = (agent: Pick<Agent, "personality" | "description">) => {
  const personality = agent.personality;
  if (!personality) return null;
  const text = agent.description?.trim() || personality.seed?.trim() || personality.content.trim();
  return text.split(/\n+/).find((line) => line.trim())?.trim() ?? null;
};

export type Phase = "working" | "checking" | "done" | "failed";

export const phaseOf = (run: Pick<Run, "state" | "exit" | "error">): Phase => {
  if (run.state === "running") return "working";
  if (run.state === "reviewing") return "checking";
  return run.exit === 0 && !run.error ? "done" : "failed";
};

export const endLine = (run: Pick<Run, "exit" | "error">) => {
  if (run.error) return run.error;
  if (run.exit === 0) return "Done";
  if (run.exit === null) return "Stopped";
  return `Ended with ${run.exit}`;
};

export type Transcript = {
  run: Run | null;
  lines: { stream: "out" | "err"; text: string }[];
  checking: string | null;
  reviewed: { hash: string | null; error?: string } | null;
  exited: { exit: number | null; error?: string } | null;
  last?: string;
};

export const emptyTranscript = (run: Run | null = null): Transcript => ({
  run,
  lines: [],
  checking: null,
  reviewed: null,
  exited: null,
});

export const fold = (seen: Transcript, event: RunEvent, at?: string): Transcript => {
  const next = { ...seen, last: at ?? seen.last };
  switch (event.event) {
    case "started":
      return { ...next, run: event.run };
    case "line":
      return { ...next, lines: [...seen.lines, { stream: event.stream, text: event.text }] };
    case "exited":
      return {
        ...next,
        exited: { exit: event.exit, error: event.error },
        run: seen.run ? { ...seen.run, exit: event.exit, error: event.error, state: "reviewing" } : seen.run,
      };
    case "review":
      return {
        ...next,
        checking: event.text,
        run: seen.run && seen.run.state === "running" ? { ...seen.run, state: "reviewing" } : seen.run,
      };
    case "reviewed":
      return { ...next, checking: null, reviewed: { hash: event.hash, error: event.error } };
    case "done":
      return { ...next, checking: null, run: event.run };
  }
};

export const duration = (from: string, to: string | null, now = Date.now()) => {
  const start = Date.parse(from);
  const end = to ? Date.parse(to) : now;
  if (Number.isNaN(start) || Number.isNaN(end)) return null;
  const seconds = Math.max(0, Math.round((end - start) / 1000));
  if (seconds < 60) return `${seconds}s`;
  const minutes = Math.floor(seconds / 60);
  if (minutes < 60) return `${minutes}m ${String(seconds % 60).padStart(2, "0")}s`;
  return `${Math.floor(minutes / 60)}h ${String(minutes % 60).padStart(2, "0")}m`;
};
