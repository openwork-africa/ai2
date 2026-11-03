import { api } from "@/lib/api";
import type { Harness } from "@/lib/harnesses";

export type Page = "security" | "vault";

export type Choice = { runs: string; provider: string | null; model: string | null };

export type Saved = { page: Page; choice: Choice; saved: boolean; restart: boolean };

export type Tested = { ok: boolean; message: string };

export type Api = "openai-completions" | "anthropic-messages";

export type Provider = {
  name: string;
  api: Api;
  base_url: string;
  key: "vault" | "other" | "none";
  stored: boolean;
  pass: boolean;
  models: string[];
};

export type Preset = { id: string; label: string; name: string; api: Api; base_url: string; hint: string };

export const PRESETS: Preset[] = [
  {
    id: "anthropic",
    label: "Anthropic",
    name: "anthropic",
    api: "anthropic-messages",
    base_url: "https://api.anthropic.com",
    hint: "Claude models, with a key from console.anthropic.com.",
  },
  {
    id: "openai",
    label: "OpenAI",
    name: "openai",
    api: "openai-completions",
    base_url: "https://api.openai.com/v1",
    hint: "GPT models, with a key from platform.openai.com.",
  },
  {
    id: "muse",
    label: "Muse",
    name: "muse",
    api: "openai-completions",
    base_url: "https://api.meta.ai/v1",
    hint: "Meta's Muse models, with a key from dev.meta.ai.",
  },
  {
    id: "custom",
    label: "Custom",
    name: "",
    api: "openai-completions",
    base_url: "",
    hint: "Any address that speaks the OpenAI or Anthropic way.",
  },
];

export const API_LABELS: Record<Api, string> = {
  "openai-completions": "OpenAI style",
  "anthropic-messages": "Anthropic style",
};

export const NAME_PATTERN = /^[A-Za-z0-9_-][A-Za-z0-9_.-]{0,63}$/;

export const presetOf = (provider: Pick<Provider, "name" | "base_url">) =>
  PRESETS.find((preset) => preset.id !== "custom" && preset.base_url === provider.base_url.replace(/\/+$/, "")) ??
  PRESETS[PRESETS.length - 1];

export const providerProblem = (form: { name: string; base_url: string; key: string }, adding: boolean) => {
  if (!NAME_PATTERN.test(form.name)) return "A name is letters, digits, - _ and ., at most 64.";
  if (!/^https:\/\/[^/\s]+/.test(form.base_url.trim())) return "The base URL starts with https://.";
  if (adding && !form.key.trim()) return "The key is needed the first time.";
  return null;
};

export const runsHarness = (choice: Choice) => !["auto", "none", "model"].includes(choice.runs);

export const usable = (harnesses: Harness[]) =>
  harnesses.filter((harness) => harness.kind === "agent" && harness.installed && harness.connected);

export const waiting = (harnesses: Harness[]) =>
  harnesses.filter((harness) => harness.kind === "agent" && !(harness.installed && harness.connected));

export const getSettings = (page: Page) => api<Saved>("GET", `/api/settings/${page}`);

export const saveSettings = (page: Page, choice: Choice) => api<Saved>("PUT", `/api/settings/${page}`, choice);

export const testSettings = (page: Page, choice: Choice) => api<Tested>("POST", `/api/settings/${page}/test`, choice);

export const restartVault = () => api<Tested>("POST", "/api/settings/vault/restart");

export const listProviders = () => api<{ providers: Provider[] }>("GET", "/api/providers");

export const providerModels = (name: string) =>
  api<{ models: string[] }>("GET", `/api/providers/${encodeURIComponent(name)}/models`);

export const testProvider = (name: string, model?: string) =>
  api<Tested>("POST", `/api/providers/${encodeURIComponent(name)}/test`, model ? { model } : {});
