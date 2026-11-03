import { describe, expect, test } from "bun:test";
import { asName, duration, emptyTranscript, endLine, fold, NAME_RULE, personalityLine, phaseOf, type Run, type RunEvent } from "../src/lib/agents";

const run = (over: Partial<Run> = {}): Run => ({
  id: "019a4c6e1f20ab01",
  agent: "scout",
  harness: "claude",
  folder: "/tmp/project",
  prompt: "Look it over",
  state: "running",
  started: "2026-11-03T10:00:00Z",
  ended: null,
  exit: null,
  log: "/tmp/output.log",
  review: null,
  ...over,
});

describe("names", () => {
  test("typed names become valid agent names", () => {
    expect(asName("Release Notes!")).toBe("release-notes");
    expect(NAME_RULE.test("release-notes")).toBe(true);
    expect(NAME_RULE.test("-lead")).toBe(false);
    expect(asName("x".repeat(30)).length).toBe(24);
  });
});

describe("personality line", () => {
  test("prefers the description, then the seed, then the first line", () => {
    const personality = { content: "\nYou are scout.\nMore.", seed: "a scout" };
    expect(personalityLine({ personality, description: "Finds things." })).toBe("Finds things.");
    expect(personalityLine({ personality })).toBe("a scout");
    expect(personalityLine({ personality: { content: "\nYou are scout.\nMore." } })).toBe("You are scout.");
    expect(personalityLine({})).toBeNull();
  });
});

describe("runs", () => {
  test("phase and end line follow the record", () => {
    expect(phaseOf(run())).toBe("working");
    expect(phaseOf(run({ state: "reviewing" }))).toBe("checking");
    expect(phaseOf(run({ state: "ended", exit: 0 }))).toBe("done");
    expect(phaseOf(run({ state: "ended", exit: 2 }))).toBe("failed");
    expect(endLine({ exit: 2 })).toBe("Ended with 2");
    expect(endLine({ exit: null })).toBe("Stopped");
    expect(endLine({ exit: null, error: "claude is not installed" })).toBe("claude is not installed");
  });

  test("events fold into a transcript", () => {
    const events: RunEvent[] = [
      { event: "started", run: run() },
      { event: "line", stream: "out", text: "hello" },
      { event: "line", stream: "err", text: "oops" },
      { event: "exited", exit: 0 },
      { event: "review", text: "Looking at the folder" },
      { event: "reviewed", hash: "a1b2c3d4e5f60718" },
      { event: "done", run: run({ state: "ended", exit: 0, review: "a1b2c3d4e5f60718" }) },
    ];
    let seen = emptyTranscript();
    const states: string[] = [];
    events.forEach((event, at) => {
      seen = fold(seen, event, String(at));
      states.push(seen.run?.state ?? "none");
    });
    expect(seen.lines).toEqual([
      { stream: "out", text: "hello" },
      { stream: "err", text: "oops" },
    ]);
    expect(states).toEqual(["running", "running", "running", "reviewing", "reviewing", "reviewing", "ended"]);
    expect(seen.reviewed?.hash).toBe("a1b2c3d4e5f60718");
    expect(seen.checking).toBeNull();
    expect(seen.last).toBe("6");
  });

  test("durations read plainly", () => {
    expect(duration("2026-11-03T10:00:00Z", "2026-11-03T10:00:42Z")).toBe("42s");
    expect(duration("2026-11-03T10:00:00Z", "2026-11-03T10:04:05Z")).toBe("4m 05s");
    expect(duration("2026-11-03T10:00:00Z", "2026-11-03T12:07:00Z")).toBe("2h 07m");
    expect(duration("nope", null)).toBeNull();
  });
});
