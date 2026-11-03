import { api } from "@/lib/api";

export type Phase = "idle" | "checking" | "running" | "done";
export type StepStatus = "pending" | "running" | "ready" | "missing" | "unsupported" | "unknown" | "failed" | "skipped";
export type Stream = "out" | "err" | "say" | "cmd";

export type LogLine = { t: number; stream: Stream; line: string };

export type StepView = {
  id: string;
  status: StepStatus;
  warnings: string[];
  info: Record<string, unknown>;
  error?: string;
  ms?: number;
  log: LogLine[];
};

export type InputField = { label: string; secret: boolean };

export type Input = {
  id: string;
  step?: string;
  kind: "passphrase" | "secret" | "sudo" | "choice" | string;
  title: string;
  why: string;
  fields: InputField[];
  error?: string;
};

export type Warning = { step: string; text: string };

export type Snapshot = {
  phase: Phase;
  check: boolean;
  ok?: boolean;
  ms?: number;
  steps: StepView[];
  input?: Input;
  report?: unknown;
  error?: string;
  warnings: Warning[];
};

export type Host = {
  os: string;
  version: number | null;
  arm64: boolean | null;
  systemd: number | null;
  user: string;
  supports_containers: boolean;
};

export type StepInfo = { id: string; title: string; why: string; fact: string };
export type StepsInfo = { steps: StepInfo[]; inputs: { kind: string; title: string; why: string; fields: InputField[] }[] };

export const getHost = () => api<Host>("GET", "/api/setup/host");
export const getStepsInfo = () => api<StepsInfo>("GET", "/api/setup/steps/info");
export const startSetup = (body: { check?: boolean; from?: string }) =>
  api<{ phase: Phase }>("POST", "/api/setup/start", body);
export const answerInput = (id: string, values: string[]) =>
  api<unknown>("POST", "/api/setup/answer", { id, values });
export type Dashboard = { here: boolean; url: string | null };
export const getDashboard = () => api<Dashboard>("GET", "/api/setup/dashboard");
export const handOver = () => api<{ stopping: boolean }>("POST", "/api/setup/handover");
export const cancelInput =(id: string) => api<unknown>("POST", "/api/setup/answer", { id, cancel: true });
