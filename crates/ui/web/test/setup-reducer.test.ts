import { expect, test } from "bun:test";
import { initialState, progress, reduce, type SetupEvent } from "../src/lib/setup-reducer";

const ids = ["containers", "master-key", "presence", "daemon"];
const step = (id: string) => ({ id, status: "pending", warnings: [], info: {}, log: [] });

const events: SetupEvent[] = [
  { event: "snapshot", id: "1", data: { phase: "running", check: false, steps: ids.map(step), warnings: [] } },
  { event: "step", id: "2", data: { id: "containers", status: "running" } },
  { event: "log", id: "3", data: { step: "containers", t: 5, stream: "cmd", line: "$ brew install container" } },
  { event: "step", id: "4", data: { id: "containers", status: "ready", ms: 4200, warnings: [], info: {} } },
  { event: "step", id: "5", data: { id: "master-key", status: "running" } },
  {
    event: "input",
    id: "6",
    data: { id: "p1", step: "master-key", kind: "sudo", title: "Your password, for sudo", why: "", fields: [{ label: "Password", secret: true }] },
  },
];

test("events become steps, logs, inputs and a done summary", () => {
  let state = initialState(ids);
  for (const event of events) state = reduce(state, event, 1000);
  expect(state.phase).toBe("running");
  expect(state.steps[0]).toMatchObject({ status: "ready", ms: 4200 });
  expect(state.steps[0].log[0].line).toBe("$ brew install container");
  expect(state.steps[1].status).toBe("running");
  expect(state.since["master-key"]).toBe(1000);
  expect(state.input?.id).toBe("p1");
  expect(progress(state)).toMatchObject({ total: 4, done: 1, current: 2 });

  state = reduce(state, { event: "answered", data: { id: "p9" } }, 1100);
  expect(state.input?.id).toBe("p1");
  state = reduce(state, { event: "answered", data: { id: "p1" } }, 1100);
  expect(state.input).toBeUndefined();

  state = reduce(
    state,
    { event: "step", data: { id: "master-key", status: "failed", error: "no key", warnings: [], info: {} } },
    1200,
  );
  state = reduce(state, { event: "step", data: { id: "presence", status: "skipped" } }, 1200);
  state = reduce(
    state,
    { event: "done", data: { ok: false, ms: 5000, check: false, warnings: [{ step: "containers", text: "careful" }] } },
    1300,
  );
  expect(state.phase).toBe("done");
  expect(state.ok).toBe(false);
  expect(state.steps[1].error).toBe("no key");
  expect(state.warnings).toEqual([{ step: "containers", text: "careful" }]);
  expect(state.since).toEqual({});
  expect(progress(state).current).toBe(4);
});

test("a later snapshot replaces the whole state", () => {
  let state = initialState(ids);
  for (const event of events) state = reduce(state, event, 1000);
  state = reduce(state, { event: "snapshot", data: { phase: "checking", check: true, steps: ids.map(step), warnings: [] } }, 2000);
  expect(state.check).toBe(true);
  expect(state.input).toBeUndefined();
  expect(state.steps.every((s) => s.status === "pending" && s.log.length === 0)).toBe(true);
});
