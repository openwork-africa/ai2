import { api } from "@/lib/api";

export type McpClient = {
  name: string;
  version: string;
  transport: "stdio" | "http" | string;
  token: string | null;
  first_seen: number;
  last_seen: number;
};

export type McpToken = { id: string; name: string; created: number; last_used: number | null };

export type PairingCode = { code: string; expires: number };

export type Connections = {
  clients: McpClient[];
  tokens: McpToken[];
  pairing: PairingCode | null;
  url: string;
};

export const listConnections = () => api<Connections>("GET", "/api/mcp/clients");

export const makePairingCode = () => api<PairingCode>("POST", "/api/mcp/pairing");

export const revokeToken = (id: string) => api<{ revoked: string }>("DELETE", `/api/mcp/tokens/${encodeURIComponent(id)}`);

const KNOWN: Record<string, string> = {
  "claude-code": "Claude Code",
  "codex-mcp-client": "Codex",
  codex: "Codex",
  opencode: "opencode",
  cursor: "Cursor",
  "cursor-vscode": "Cursor",
  "gemini-cli-mcp-client": "Gemini CLI",
  "muse-code": "Muse Code",
  "Visual Studio Code": "VS Code",
};

export const clientLabel = (name: string) => KNOWN[name] ?? name;

export const showCard = (connections: Connections | null) => connections !== null && connections.clients.length === 0;

export const connectedNames = (connections: Connections | null) => {
  if (!connections) return [];
  const latest = [...connections.clients].sort((a, b) => b.last_seen - a.last_seen);
  return [...new Set(latest.map((client) => clientLabel(client.name)))];
};

export const pasteLine = (origin: string) => `Read ${origin}/skill.md and set up ai2.`;

export const pairLine = (origin: string, code: string) => `Read ${origin}/skill.md and connect to ai2 over HTTP with pairing code ${code}.`;

export const minutesLeft = (expires: number, now: number) => Math.max(0, Math.ceil((expires - now / 1000) / 60));
