export type VaultKey = {
  name: string;
  description: string;
  approved_urls: string[];
  inject: string;
  created: string;
  rotated: string | null;
};

export type TokenMode = "app" | "agent";

export type VaultToken = {
  id: string;
  keys: string[];
  sandbox: string;
  mode: TokenMode;
  methods: string[];
  url_prefixes: string[];
  intent: string | null;
  created: string;
  expires: string;
  uses_left: number | null;
};

export type NewToken = VaultToken & { token: string; warnings?: string[] };

export type Sandbox =
  | { name: string; transport: "unix"; path: string }
  | { name: string; transport: "tcp"; listen: string; peers: string[] };

export type Approval = {
  id: string;
  token: string;
  sandbox: string;
  key: string;
  method: string;
  url: string;
  intent: string;
  request: string;
  line: string;
  created: string;
};

export type AuditEntry = {
  time: string;
  token: string | null;
  sandbox: string;
  key: string | null;
  method: string;
  url: string | null;
  request: string | null;
  decision: string;
  status: number;
  reason: string | null;
};

export type PresenceKind = "touch_id" | "polkit" | "passphrase";

export type VaultStatus = { master_key: boolean; platform: string; version: string; presence?: PresenceKind };

export const presenceKind = (status: { platform?: string; presence?: string } | null | undefined): PresenceKind => {
  if (status?.presence === "touch_id" || status?.presence === "polkit" || status?.presence === "passphrase") return status.presence;
  return /mac|darwin/i.test(status?.platform ?? "") ? "touch_id" : "passphrase";
};

export type KeyRequest = { description: string; approved_urls: string[]; inject: string; secret: string };

export type Change =
  | ({ action: "put_key"; name: string } & KeyRequest)
  | { action: "rotate_key"; name: string; secret: string }
  | { action: "delete_key"; name: string }
  | ({ action: "create_token" } & TokenRequest)
  | { action: "revoke_token"; id: string }
  | ({ action: "add_sandbox" } & SandboxRequest)
  | { action: "remove_sandbox"; name: string };

export const describeChange = (change: Change) => {
  switch (change.action) {
    case "put_key":
      return `Add the key ${change.name}, sent only to ${change.approved_urls.map(host).join(", ")}`;
    case "rotate_key":
      return `Rotate the secret of ${change.name}`;
    case "delete_key":
      return `Delete the key ${change.name}`;
    case "create_token":
      return `Make an ${change.mode} token for ${change.keys.join(", ")} in ${change.sandbox}`;
    case "revoke_token":
      return `Revoke ${change.id}`;
    case "add_sandbox":
      return `Add the sandbox ${change.name}${change.tcp ? ` on ${change.tcp}` : ""}`;
    case "remove_sandbox":
      return `Remove the sandbox ${change.name}`;
  }
};

export const changeTarget = (change: Change) =>
  change.action === "create_token" ? null : change.action === "revoke_token" ? change.id : change.name;

export const batchBody = (changes: Change[]) => ({ actions: changes });

export const madeTokens = (results: unknown[]): NewToken[] =>
  results
    .filter((result): result is NewToken & { revoke?: string } => typeof (result as { token?: unknown })?.token === "string")
    .map(shownToken);

export const NAME_PATTERN = "[A-Za-z0-9_\\-][A-Za-z0-9_.\\-]{0,63}";

export const validName = (name: string) => new RegExp(`^${NAME_PATTERN}$`).test(name);

export const lines = (text: string) =>
  text
    .split(/\s+/)
    .map((part) => part.trim())
    .filter(Boolean);

export const words = (text: string) =>
  text
    .split(/[\s,]+/)
    .map((part) => part.trim())
    .filter(Boolean);

export const segment = (text: string) => encodeURIComponent(text);

export type InjectPreset = "bearer" | "basic" | "query" | "header" | "template";

export const INJECT_PRESETS: { value: InjectPreset; label: string; hint: string; arg?: string }[] = [
  { value: "bearer", label: "Bearer token", hint: "Authorization: Bearer {secret}" },
  { value: "basic", label: "Basic auth", hint: "The secret is user:pass" },
  { value: "query", label: "Query parameter", hint: "?name={secret}", arg: "api_key" },
  { value: "header", label: "Header", hint: "Name: {secret}", arg: "X-Api-Key" },
  { value: "template", label: "Header template", hint: "Any header with {secret} in it", arg: "X-Api-Key: Token {secret}" },
];

export const injectValue = (preset: InjectPreset, arg: string) => {
  const value = arg.trim();
  if (preset === "query") return `query:${value}`;
  if (preset === "header") return `header:${value}`;
  if (preset === "template") return value;
  return preset;
};

export const injectLabel = (inject: string) => {
  if (inject === "bearer") return "Bearer";
  if (inject === "basic") return "Basic";
  if (inject.startsWith("query:")) return `?${inject.slice(6)}=`;
  if (inject.startsWith("header:")) return `${inject.slice(7)}:`;
  return inject;
};

export type MethodsPreset = "read" | "write" | "all" | "custom";

export const METHOD_PRESETS: { value: MethodsPreset; label: string; hint: string }[] = [
  { value: "read", label: "Read", hint: "GET, HEAD, OPTIONS" },
  { value: "write", label: "Write", hint: "POST, PUT, PATCH, DELETE" },
  { value: "all", label: "All", hint: "Every method" },
  { value: "custom", label: "Only these", hint: "A list you write" },
];

export type TokenForm = {
  keys: string[];
  sandbox: string;
  mode: TokenMode;
  methods: MethodsPreset;
  methodList: string;
  prefixes: string;
  intent: string;
  ttl: string;
  uses: string;
};

export type TokenRequest = {
  keys: string[];
  sandbox: string;
  mode: TokenMode;
  methods: string;
  url_prefixes?: string[];
  intent?: string;
  ttl?: string;
  uses?: number;
};

export const tokenRequest = (form: TokenForm): { request: TokenRequest } | { problem: string } => {
  if (!form.keys.length) return { problem: "Choose at least one key." };
  if (!form.sandbox) return { problem: "Choose a sandbox." };
  const methods = form.methods === "custom" ? words(form.methodList.toUpperCase()).join(",") : form.methods;
  if (!methods) return { problem: "Write at least one method." };
  const intent = form.intent.trim();
  if (form.mode === "agent" && !intent) return { problem: "An agent token needs an intent." };
  const request: TokenRequest = { keys: form.keys, sandbox: form.sandbox, mode: form.mode, methods };
  const prefixes = lines(form.prefixes);
  if (prefixes.length) request.url_prefixes = prefixes;
  if (intent) request.intent = intent;
  if (form.ttl.trim()) request.ttl = form.ttl.trim();
  if (form.uses.trim()) {
    const uses = Number(form.uses);
    if (!Number.isInteger(uses) || uses < 1) return { problem: "Uses is a whole number, at least 1." };
    request.uses = uses;
  }
  return { request };
};

export const shownToken = (made: NewToken & { revoke?: string }): NewToken => {
  const { revoke: _revoke, ...rest } = made;
  return rest;
};

export type SandboxForm = { name: string; transport: "unix" | "tcp"; socket: string; listen: string; peers: string };

export type SandboxRequest = { name: string; socket?: string; tcp?: string; peers?: string[] };

export const sandboxRequest = (form: SandboxForm): { request: SandboxRequest } | { problem: string } => {
  const name = form.name.trim();
  if (!validName(name)) return { problem: "A name is letters, digits, - _ . and at most 64." };
  if (form.transport === "unix") {
    const socket = form.socket.trim();
    return { request: socket ? { name, socket } : { name } };
  }
  const listen = form.listen.trim();
  const peers = words(form.peers);
  if (!listen) return { problem: "Give the address to listen on, like 192.168.64.1:7801." };
  if (!peers.length) return { problem: "A TCP sandbox needs at least one peer address." };
  return { request: { name, tcp: listen, peers } };
};

export const endpointLabel = (sandbox: Sandbox) =>
  sandbox.transport === "unix" ? sandbox.path : `${sandbox.listen} from ${sandbox.peers.join(", ")}`;

export type Tone = "success" | "destructive" | "muted";

export const decisionTone = (decision: string): Tone =>
  /allow|approv|^ok/i.test(decision) ? "success" : /den|refus|block/i.test(decision) ? "destructive" : "muted";

export const isMac = (platform: string | undefined) => /mac|darwin/i.test(platform ?? "");

const toDate = (value: string | number) =>
  typeof value === "number" ? new Date(value < 1e12 ? value * 1000 : value) : new Date(value);

export const absolute = (value: string | number | null | undefined) => {
  if (value === null || value === undefined || value === "") return "";
  const date = toDate(value);
  return Number.isNaN(date.getTime()) ? String(value) : date.toLocaleString();
};

export const relative = (value: string | number | null | undefined, now = Date.now()) => {
  if (value === null || value === undefined || value === "") return "";
  const date = toDate(value);
  if (Number.isNaN(date.getTime())) return String(value);
  const seconds = Math.round((date.getTime() - now) / 1000);
  const size = Math.abs(seconds);
  const [count, unit] =
    size < 45
      ? [0, "now"]
      : size < 3600
        ? [Math.round(size / 60), "min"]
        : size < 86400
          ? [Math.round(size / 3600), "h"]
          : [Math.round(size / 86400), "d"];
  if (unit === "now") return "just now";
  const span = `${count}${unit === "min" ? " min" : unit}`;
  return seconds > 0 ? `in ${span}` : `${span} ago`;
};

export type Presence = {
  kind: PresenceKind;
  waiting: (on: boolean) => void;
  ask: () => Promise<string | null>;
};

export class Cancelled extends Error {
  constructor() {
    super("Cancelled: nothing was changed.");
  }
}

const needsPresence = (failure: unknown) =>
  typeof failure === "object" &&
  failure !== null &&
  (failure as { status?: number }).status === 401 &&
  (failure as { code?: string }).code === "presence_required";

export const withPresence = async <T>(write: boolean, presence: Presence, send: (passphrase?: string) => Promise<T>) => {
  const touch = write && presence.kind !== "passphrase";
  if (touch) presence.waiting(true);
  try {
    return await send();
  } catch (failure) {
    if (!write || presence.kind !== "passphrase" || !needsPresence(failure)) throw failure;
  } finally {
    if (touch) presence.waiting(false);
  }
  const passphrase = await presence.ask();
  if (passphrase === null) throw new Cancelled();
  return send(passphrase);
};

export const host = (url: string) => {
  try {
    const parsed = new URL(url);
    return parsed.host + (parsed.pathname === "/" ? "" : parsed.pathname);
  } catch {
    return url;
  }
};
