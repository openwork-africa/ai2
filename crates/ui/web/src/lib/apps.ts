import { api, events, headers } from "@/lib/api";

export type Restart = "never" | "on-failure" | "always";
export type ApiState = "unchecked" | "valid" | "unreachable" | "invalid";

export type AppApi = {
  openapi: string;
  href: string;
  state: ApiState;
  checked: string | null;
  title?: string;
  version?: string;
  operations?: number;
  error?: string;
};

export type App = {
  id: string;
  name: string;
  description: string;
  command: string;
  cwd: string;
  port: number;
  url: string;
  env: Record<string, string>;
  set_port: boolean;
  restart: Restart;
  icon: { type: string; file: string; bytes: number } | null;
  openapi: string;
  added: string;
  updated: string;
  running: boolean;
  started_here: boolean;
  pid: number | null;
  since: string | null;
  restarts: number;
  last_exit: { code: number | null; signal: number | null } | null;
  crashed: boolean;
  api: AppApi;
  document?: OpenApiDocument;
};

export type AppEvent = { id: string; running: boolean; started_here: boolean; pid: number | null };

export type LogLine = { t: number | null; stream: "out" | "err" | "log"; line: string };

export type Operation = { method: string; path: string; summary?: string; operationId?: string };

export type OpenApiDocument = {
  openapi: string;
  info?: { title?: string; version?: string; description?: string };
  paths?: Record<string, Record<string, { summary?: string; operationId?: string; description?: string }>>;
};

export type AppFields = {
  name: string;
  description: string;
  command: string;
  openapi: string;
  cwd: string;
  port?: number;
  env: Record<string, string>;
  set_port: boolean;
  restart: Restart;
};

const path = (id: string, rest = "") => `/api/apps/${encodeURIComponent(id)}${rest}`;

export const listApps = () => api<{ apps: App[] }>("GET", "/api/apps");
export const getApp = (id: string) => api<{ app: App }>("GET", path(id));
export const startApp = (id: string) => api<{ app: App }>("POST", path(id, "/start"));
export const stopApp = (id: string) => api<{ app: App }>("POST", path(id, "/stop"));
export const restartApp = (id: string) => api<{ app: App }>("POST", path(id, "/restart"));
export const checkApp = (id: string) => api<{ api: AppApi }>("POST", path(id, "/check"));
export const getLogs = (id: string, tail = 200) => api<{ lines: LogLine[] }>("GET", path(id, `/logs?tail=${tail}`));
export const getDocument = (id: string) => api<OpenApiDocument>("GET", path(id, "/openapi.json"));

export const removeApp = async (id: string) => {
  await fetch(path(id), { method: "DELETE", headers: headers() }).then((res) => {
    if (!res.ok && res.status !== 204) throw new Error(`Could not remove: HTTP ${res.status}`);
  });
};

export const registerApp = (fields: AppFields, icon: File) => {
  const form = new FormData();
  form.append("app", new Blob([JSON.stringify(fields)], { type: "application/json" }));
  form.append("icon", icon, icon.name);
  return api<{ app: App }>("POST", "/api/apps", form);
};

const base64 = (file: File) =>
  new Promise<string>((resolve, reject) => {
    const reader = new FileReader();
    reader.onload = () => resolve(String(reader.result).split(",")[1] ?? "");
    reader.onerror = () => reject(reader.error);
    reader.readAsDataURL(file);
  });

export const updateApp = async (id: string, fields: AppFields, icon: File | null) => {
  const body: Record<string, unknown> = { ...fields };
  if (icon) body.icon = { type: icon.type || undefined, base64: await base64(icon) };
  return api<{ app: App }>("PATCH", path(id), body);
};

export const appEvents = (onEvent: (event: AppEvent) => void, signal: AbortSignal) =>
  events("/api/apps/events", (event) => event.event === "app" && onEvent(event.data as AppEvent), { signal });

export const logEvents = (id: string, onLine: (line: LogLine) => void, signal: AbortSignal) =>
  events(path(id, "/logs/events"), (event) => event.event === "log" && onLine(event.data as LogLine), { signal });

const blobs = new Map<string, Promise<string>>();

export const iconUrl = (id: string, version: string) => {
  const key = `${id}@${version}`;
  let found = blobs.get(key);
  if (!found) {
    found = fetch(path(id, "/icon"), { headers: headers() }).then(async (res) => {
      if (!res.ok) throw new Error(`icon ${res.status}`);
      return URL.createObjectURL(await res.blob());
    });
    found.catch(() => blobs.delete(key));
    blobs.set(key, found);
  }
  return found;
};

export const openDocument = async (id: string) => {
  const tab = window.open("about:blank", "_blank");
  try {
    const res = await fetch(path(id, "/openapi.json"), { headers: headers() });
    const text = await res.text();
    const pretty = res.ok ? JSON.stringify(JSON.parse(text), null, 2) : text;
    const url = URL.createObjectURL(new Blob([pretty], { type: "application/json" }));
    if (tab) tab.location.href = url;
  } catch (failure) {
    tab?.close();
    throw failure;
  }
};

export const openApp = async (app: App, onUp: (app: App) => void) => {
  if (app.running) {
    window.open(app.url, "_blank", "noopener");
    return;
  }
  const tab = window.open("about:blank", "_blank");
  try {
    const { app: up } = await startApp(app.id);
    onUp(up);
    if (tab) tab.location.href = up.url;
  } catch (failure) {
    tab?.close();
    throw failure;
  }
};

const METHODS = ["get", "post", "put", "patch", "delete", "options", "head", "trace"];

export const operations = (document: OpenApiDocument): Operation[] =>
  Object.entries(document.paths ?? {}).flatMap(([route, item]) =>
    Object.entries(item ?? {})
      .filter(([method]) => METHODS.includes(method))
      .map(([method, op]) => ({ method: method.toUpperCase(), path: route, summary: op?.summary ?? op?.description, operationId: op?.operationId })),
  );

export type Standing = { tone: "success" | "warning" | "destructive" | "muted"; label: string };

export const standing = (app: App): Standing => {
  if (app.crashed) return { tone: "destructive", label: "Crashed" };
  if (app.running && app.started_here) return { tone: "success", label: "Running" };
  if (app.running) return { tone: "success", label: "Running, started elsewhere" };
  if (app.started_here) return { tone: "warning", label: "Starting" };
  return { tone: "muted", label: "Stopped" };
};

export const apiStanding = (api: AppApi): Standing => {
  if (api.state === "valid") {
    const count = api.operations ?? 0;
    return { tone: "success", label: `${count} ${count === 1 ? "operation" : "operations"}` };
  }
  if (api.state === "unreachable") return { tone: "warning", label: "unreachable" };
  if (api.state === "invalid") return { tone: "warning", label: "invalid" };
  return { tone: "muted", label: "unchecked" };
};

export const uptime = (since: string | null, now = Date.now()) => {
  if (!since) return null;
  const seconds = Math.max(0, Math.floor((now - Date.parse(since)) / 1000));
  if (seconds < 60) return `${seconds}s`;
  const minutes = Math.floor(seconds / 60);
  if (minutes < 60) return `${minutes}m`;
  const hours = Math.floor(minutes / 60);
  if (hours < 24) return `${hours}h ${minutes % 60}m`;
  return `${Math.floor(hours / 24)}d ${hours % 24}h`;
};

export const home = (cwd: string) => cwd.replace(/^\/(Users|home)\/[^/]+/, "~");
