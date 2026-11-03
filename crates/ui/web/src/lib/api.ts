export const NONCE_HEADER = "X-Ai2-UI-Nonce";
const STORED = "ai2-ui-nonce";

let nonce: string | null = null;
const lockedListeners = new Set<() => void>();
let locked = false;

const readStored = (): string | null => {
  try {
    return sessionStorage.getItem(STORED);
  } catch {
    return null;
  }
};

const writeStored = (value: string) => {
  try {
    sessionStorage.setItem(STORED, value);
  } catch {
    return;
  }
};

export const takeNonce = () => {
  const url = new URL(window.location.href);
  const given = url.searchParams.get("nonce");
  if (given) {
    nonce = given;
    writeStored(given);
    url.searchParams.delete("nonce");
    window.history.replaceState(window.history.state, "", url.pathname + url.search + url.hash);
  } else {
    nonce = readStored();
  }
  if (!nonce) lock();
};

const lock = () => {
  if (locked) return;
  locked = true;
  lockedListeners.forEach((listener) => listener());
};

export const isLocked = () => locked;

export const onLocked = (listener: () => void) => {
  lockedListeners.add(listener);
  return () => {
    lockedListeners.delete(listener);
  };
};

export class ApiError extends Error {
  status: number;
  code: string;
  constructor(status: number, code: string, message: string) {
    super(message);
    this.status = status;
    this.code = code;
  }
}

type Method = "GET" | "POST" | "PUT" | "PATCH" | "DELETE";

export const headers = (extra: Record<string, string> = {}): Record<string, string> => ({
  ...(nonce ? { [NONCE_HEADER]: nonce } : {}),
  ...extra,
});

export const api = async <T>(
  method: Method,
  path: string,
  body?: unknown,
  extra: Record<string, string> = {},
): Promise<T> => {
  const json = body !== undefined && !(body instanceof FormData);
  const res = await fetch(path, {
    method,
    headers: headers(json ? { "content-type": "application/json", ...extra } : extra),
    body: body === undefined ? undefined : json ? JSON.stringify(body) : (body as FormData),
  });
  const data = await res.json().catch(() => ({}));
  if (res.status === 403) lock();
  if (!res.ok) throw new ApiError(res.status, data.error ?? "error", data.message ?? res.statusText);
  return data as T;
};

export type ServerEvent = { event: string; id?: string; data: unknown };

export const events = async (
  path: string,
  onEvent: (event: ServerEvent) => void,
  options: { signal?: AbortSignal; lastEventId?: string } = {},
) => {
  const res = await fetch(path, {
    headers: headers({
      accept: "text/event-stream",
      ...(options.lastEventId ? { "last-event-id": options.lastEventId } : {}),
    }),
    signal: options.signal,
  });
  if (res.status === 403) lock();
  if (!res.ok || !res.body) throw new ApiError(res.status, "stream_failed", res.statusText);
  const reader = res.body.pipeThrough(new TextDecoderStream()).getReader();
  let buffer = "";
  for (;;) {
    const { value, done } = await reader.read();
    if (done) return;
    buffer += value;
    let end = buffer.indexOf("\n\n");
    while (end >= 0) {
      const block = buffer.slice(0, end);
      buffer = buffer.slice(end + 2);
      const parsed: ServerEvent = { event: "message", data: null };
      const lines: string[] = [];
      for (const line of block.split("\n")) {
        const colon = line.indexOf(":");
        if (colon === 0) continue;
        const field = colon < 0 ? line : line.slice(0, colon);
        const text = colon < 0 ? "" : line.slice(colon + 1).replace(/^ /, "");
        if (field === "event") parsed.event = text;
        else if (field === "id") parsed.id = text;
        else if (field === "data") lines.push(text);
      }
      if (lines.length) {
        const raw = lines.join("\n");
        try {
          parsed.data = JSON.parse(raw);
        } catch {
          parsed.data = raw;
        }
        onEvent(parsed);
      }
      end = buffer.indexOf("\n\n");
    }
  }
};

export type Status = {
  version: string;
  features: string[];
  vault: { reachable: boolean; version: string | null; master_key: boolean | null };
  setup: "done" | "needed" | "unknown";
};

export const status = () => api<Status>("GET", "/api/status");
