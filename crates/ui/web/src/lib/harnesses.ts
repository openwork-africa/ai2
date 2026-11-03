import { api, events } from "@/lib/api";

export type JobSummary = {
  id: string;
  harness: string;
  action: "install" | "sign-in";
  running: boolean;
  ok: boolean | null;
  message: string | null;
};

export type Harness = {
  id: string;
  label: string;
  kind: "agent" | "tool";
  command: string;
  logo: string | null;
  can_sign_in: boolean;
  sign_in_hint: string | null;
  asks: "key" | "code" | null;
  models: { id: string; label: string }[];
  installed: boolean;
  signed_in: boolean;
  connected: boolean;
  skills: string | null;
  skill: boolean;
  job: JobSummary | null;
};

export type JobEvent =
  | { type: "line"; text: string }
  | { type: "link"; url: string }
  | { type: "code"; code: string }
  | { type: "error"; error: string }
  | { type: "done"; ok: boolean; message?: string };

export type Action = JobSummary["action"];

export const listHarnesses = () => api<{ harnesses: Harness[] }>("GET", "/api/harnesses");

export const startJob = (id: string, action: Action) =>
  api<{ job: JobSummary }>("POST", `/api/harnesses/${encodeURIComponent(id)}/${action}`);

export const addSkill = (id: string) =>
  api<{ harness: string; path: string }>("POST", `/api/harnesses/${encodeURIComponent(id)}/skill`);

export const answerJob = (job: string, text: string) =>
  api<void>("POST", `/api/harnesses/jobs/${encodeURIComponent(job)}/answer`, { text });

export const followJob = (
  job: string,
  onEvent: (event: JobEvent, id: string | undefined) => void,
  options: { signal?: AbortSignal; lastEventId?: string } = {},
) =>
  events(
    `/api/harnesses/jobs/${encodeURIComponent(job)}/events`,
    (sent) => onEvent(sent.data as JobEvent, sent.id),
    options,
  );

export type HarnessState = "ready" | "sign-in" | "missing";

export const stateOf = (harness: Harness): HarnessState =>
  harness.connected ? "ready" : harness.installed ? "sign-in" : "missing";
