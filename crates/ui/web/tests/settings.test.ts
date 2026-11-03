import { describe, expect, test } from "bun:test";
import type { Harness } from "../src/lib/harnesses";
import { PRESETS, presetOf, providerProblem, runsHarness, usable, waiting } from "../src/lib/settings";

const harness = (over: Partial<Harness>): Harness => ({
  id: "claude",
  label: "Claude Code",
  kind: "agent",
  command: "claude",
  logo: null,
  can_sign_in: true,
  sign_in_hint: null,
  asks: null,
  models: [],
  installed: true,
  signed_in: true,
  connected: true,
  job: null,
  ...over,
});

describe("providers", () => {
  test("the presets are Anthropic, OpenAI, Muse and Custom", () => {
    expect(PRESETS.map((preset) => preset.label)).toEqual(["Anthropic", "OpenAI", "Muse", "Custom"]);
    expect(PRESETS.find((preset) => preset.id === "muse")?.base_url).toBe("https://api.meta.ai/v1");
  });

  test("a saved provider is matched to its preset by address", () => {
    expect(presetOf({ name: "x", base_url: "https://api.openai.com/v1/" }).id).toBe("openai");
    expect(presetOf({ name: "x", base_url: "https://models.example.com" }).id).toBe("custom");
  });

  test("a provider needs a name, an https address and, the first time, a key", () => {
    const form = { name: "openai", base_url: "https://api.openai.com/v1", key: "sk" };
    expect(providerProblem(form, true)).toBeNull();
    expect(providerProblem({ ...form, key: "" }, true)).not.toBeNull();
    expect(providerProblem({ ...form, key: "" }, false)).toBeNull();
    expect(providerProblem({ ...form, name: "../x" }, true)).not.toBeNull();
    expect(providerProblem({ ...form, base_url: "http://a" }, true)).not.toBeNull();
  });
});

describe("harnesses", () => {
  test("only agents that are installed and signed in can be picked", () => {
    const all = [
      harness({}),
      harness({ id: "codex", connected: false, signed_in: false }),
      harness({ id: "gh", kind: "tool" }),
    ];
    expect(usable(all).map((h) => h.id)).toEqual(["claude"]);
    expect(waiting(all).map((h) => h.id)).toEqual(["codex"]);
  });

  test("auto, none and model are not harnesses", () => {
    for (const runs of ["auto", "none", "model"]) expect(runsHarness({ runs, provider: null, model: null })).toBe(false);
    expect(runsHarness({ runs: "claude", provider: null, model: null })).toBe(true);
  });
});
