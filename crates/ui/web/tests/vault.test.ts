import { describe, expect, test } from "bun:test";
import {
  batchBody,
  Cancelled,
  changeTarget,
  describeChange,
  madeTokens,
  presenceKind,
  decisionTone,
  endpointLabel,
  host,
  injectLabel,
  injectValue,
  isMac,
  lines,
  relative,
  sandboxRequest,
  shownToken,
  tokenRequest,
  type TokenForm,
  validName,
  withPresence,
} from "../src/lib/vault";

const form = (over: Partial<TokenForm> = {}): TokenForm => ({
  keys: ["stripe"],
  sandbox: "host",
  mode: "app",
  methods: "read",
  methodList: "",
  prefixes: "",
  intent: "",
  ttl: "",
  uses: "",
  ...over,
});

describe("names and lists", () => {
  test("names follow the daemon's rule", () => {
    expect(validName("stripe")).toBe(true);
    expect(validName("my_key-2.v1")).toBe(true);
    expect(validName(".hidden")).toBe(false);
    expect(validName("has space")).toBe(false);
    expect(validName("a".repeat(65))).toBe(false);
    expect(validName("")).toBe(false);
  });

  test("URL lines drop blanks", () => {
    expect(lines(" https://a.example/v1/\n\n https://b.example/ ")).toEqual(["https://a.example/v1/", "https://b.example/"]);
  });

  test("hosts read short", () => {
    expect(host("https://api.stripe.com/v1/")).toBe("api.stripe.com/v1/");
    expect(host("https://uploads.github.com/")).toBe("uploads.github.com");
    expect(host("not a url")).toBe("not a url");
  });
});

describe("inject", () => {
  test("presets become the daemon's inject strings", () => {
    expect(injectValue("bearer", "ignored")).toBe("bearer");
    expect(injectValue("basic", "")).toBe("basic");
    expect(injectValue("query", " api_key ")).toBe("query:api_key");
    expect(injectValue("header", "X-Api-Key")).toBe("header:X-Api-Key");
    expect(injectValue("template", "X-Key: {secret}")).toBe("X-Key: {secret}");
  });

  test("labels are short", () => {
    expect(injectLabel("bearer")).toBe("Bearer");
    expect(injectLabel("query:api_key")).toBe("?api_key=");
    expect(injectLabel("header:X-Api-Key")).toBe("X-Api-Key:");
  });
});

describe("tokens", () => {
  test("an app token sends only what is set", () => {
    expect(tokenRequest(form())).toEqual({ request: { keys: ["stripe"], sandbox: "host", mode: "app", methods: "read" } });
  });

  test("custom methods, prefixes, ttl and uses", () => {
    const built = tokenRequest(
      form({ methods: "custom", methodList: "get, post", prefixes: "https://a.example/x\n", ttl: " 2h ", uses: "5" }),
    );
    expect(built).toEqual({
      request: {
        keys: ["stripe"],
        sandbox: "host",
        mode: "app",
        methods: "GET,POST",
        url_prefixes: ["https://a.example/x"],
        ttl: "2h",
        uses: 5,
      },
    });
  });

  test("an agent token needs an intent", () => {
    expect(tokenRequest(form({ mode: "agent" }))).toEqual({ problem: "An agent token needs an intent." });
    expect(tokenRequest(form({ mode: "agent", intent: " refund 1042 " }))).toMatchObject({ request: { intent: "refund 1042" } });
  });

  test("keys, sandbox and uses are checked", () => {
    expect(tokenRequest(form({ keys: [] }))).toHaveProperty("problem");
    expect(tokenRequest(form({ sandbox: "" }))).toHaveProperty("problem");
    expect(tokenRequest(form({ uses: "0" }))).toHaveProperty("problem");
    expect(tokenRequest(form({ uses: "1.5" }))).toHaveProperty("problem");
    expect(tokenRequest(form({ methods: "custom", methodList: " " }))).toHaveProperty("problem");
  });

  test("the revoke secret is never kept for display", () => {
    const made = {
      id: "tok_1",
      keys: ["stripe"],
      sandbox: "host",
      mode: "app" as const,
      methods: ["GET"],
      url_prefixes: [],
      intent: null,
      created: "",
      expires: "",
      uses_left: null,
      token: "t",
      revoke: "r",
    };
    expect(shownToken(made)).not.toHaveProperty("revoke");
    expect(shownToken(made).token).toBe("t");
  });
});

describe("sandboxes", () => {
  test("unix with or without a path", () => {
    expect(sandboxRequest({ name: "box", transport: "unix", socket: "", listen: "", peers: "" })).toEqual({ request: { name: "box" } });
    expect(sandboxRequest({ name: "box", transport: "unix", socket: "/s.sock", listen: "x", peers: "y" })).toEqual({
      request: { name: "box", socket: "/s.sock" },
    });
  });

  test("tcp needs listen and peers", () => {
    expect(sandboxRequest({ name: "vm", transport: "tcp", socket: "", listen: "1.2.3.4:1", peers: "" })).toHaveProperty("problem");
    expect(sandboxRequest({ name: "vm", transport: "tcp", socket: "", listen: "1.2.3.4:1", peers: "10.0.0.2, 10.0.0.3" })).toEqual({
      request: { name: "vm", tcp: "1.2.3.4:1", peers: ["10.0.0.2", "10.0.0.3"] },
    });
    expect(sandboxRequest({ name: "../x", transport: "unix", socket: "", listen: "", peers: "" })).toHaveProperty("problem");
  });

  test("endpoints read as the old page did", () => {
    expect(endpointLabel({ name: "a", transport: "unix", path: "/p" })).toBe("/p");
    expect(endpointLabel({ name: "a", transport: "tcp", listen: "l:1", peers: ["p1", "p2"] })).toBe("l:1 from p1, p2");
  });
});

describe("display", () => {
  test("decisions get a tone", () => {
    expect(decisionTone("allow")).toBe("success");
    expect(decisionTone("deny")).toBe("destructive");
    expect(decisionTone("pending")).toBe("muted");
  });

  test("relative times", () => {
    const now = Date.parse("2026-11-03T12:00:00Z");
    expect(relative("2026-11-03T11:59:50Z", now)).toBe("just now");
    expect(relative("2026-11-03T11:50:00Z", now)).toBe("10 min ago");
    expect(relative("2026-11-03T20:00:00Z", now)).toBe("in 8h");
    expect(relative("2026-10-04T12:00:00Z", now)).toBe("30d ago");
    expect(relative(null, now)).toBe("");
  });

  test("platform", () => {
    expect(isMac("macos")).toBe(true);
    expect(isMac("linux")).toBe(false);
    expect(isMac(undefined)).toBe(false);
  });
});

describe("presence", () => {
  const presenceFailure = Object.assign(new Error("needs you present"), { status: 401, code: "presence_required" });

  test("on a Mac a write shows the Touch ID wait and never asks for a passphrase", async () => {
    const seen: boolean[] = [];
    let asked = 0;
    const out = await withPresence(
      true,
      { kind: "touch_id", waiting: (on) => seen.push(on), ask: async () => (asked++, "x") },
      async () => "done",
    );
    expect(out).toBe("done");
    expect(seen).toEqual([true, false]);
    expect(asked).toBe(0);
    await expect(
      withPresence(true, { kind: "touch_id", waiting: () => {}, ask: async () => "x" }, async () => {
        throw presenceFailure;
      }),
    ).rejects.toBe(presenceFailure);
  });

  test("elsewhere a 401 asks once and resends with the passphrase", async () => {
    const sent: (string | undefined)[] = [];
    const out = await withPresence(true, { kind: "passphrase", waiting: () => {}, ask: async () => "hunter2" }, async (passphrase) => {
      sent.push(passphrase);
      if (!passphrase) throw presenceFailure;
      return "ok";
    });
    expect(out).toBe("ok");
    expect(sent).toEqual([undefined, "hunter2"]);
  });

  test("a wrong passphrase is not asked for again", async () => {
    let asked = 0;
    await expect(
      withPresence(true, { kind: "passphrase", waiting: () => {}, ask: async () => (asked++, "wrong") }, async () => {
        throw presenceFailure;
      }),
    ).rejects.toBe(presenceFailure);
    expect(asked).toBe(1);
  });

  test("cancel changes nothing", async () => {
    await expect(
      withPresence(true, { kind: "passphrase", waiting: () => {}, ask: async () => null }, async () => {
        throw presenceFailure;
      }),
    ).rejects.toBeInstanceOf(Cancelled);
  });

  test("reads and other errors pass straight through", async () => {
    const other = Object.assign(new Error("nope"), { status: 400, code: "bad_request" });
    let asked = 0;
    const ask = async () => (asked++, "x");
    await expect(withPresence(false, { kind: "passphrase", waiting: () => {}, ask }, async () => {
      throw presenceFailure;
    })).rejects.toBe(presenceFailure);
    await expect(withPresence(true, { kind: "passphrase", waiting: () => {}, ask }, async () => {
      throw other;
    })).rejects.toBe(other);
    expect(asked).toBe(0);
  });
});

describe("the batch", () => {
  test("changes are sent as the daemon's tagged actions, in order", () => {
    const body = batchBody([
      { action: "put_key", name: "stripe", description: "d", approved_urls: ["https://api.stripe.com/v1/"], inject: "bearer", secret: "s" },
      { action: "add_sandbox", name: "box" },
      { action: "create_token", keys: ["stripe"], sandbox: "box", mode: "app", methods: "read" },
      { action: "revoke_token", id: "tok_1" },
    ]);
    expect(body.actions.map((action) => action.action)).toEqual(["put_key", "add_sandbox", "create_token", "revoke_token"]);
    expect(body.actions[0]).toMatchObject({ name: "stripe", secret: "s" });
  });

  test("each change says what it does", () => {
    expect(describeChange({ action: "delete_key", name: "stripe" })).toBe("Delete the key stripe");
    expect(describeChange({ action: "add_sandbox", name: "vm", tcp: "1.2.3.4:1", peers: ["5.6.7.8"] })).toBe("Add the sandbox vm on 1.2.3.4:1");
    expect(describeChange({ action: "create_token", keys: ["a", "b"], sandbox: "host", mode: "agent", methods: "read" })).toBe(
      "Make an agent token for a, b in host",
    );
  });

  test("rows find their queued change", () => {
    expect(changeTarget({ action: "revoke_token", id: "tok_1" })).toBe("tok_1");
    expect(changeTarget({ action: "remove_sandbox", name: "box" })).toBe("box");
    expect(changeTarget({ action: "create_token", keys: [], sandbox: "", mode: "app", methods: "read" })).toBeNull();
  });

  test("only new tokens are shown, without their revoke value", () => {
    const tokens = madeTokens([
      { key: { name: "stripe" } },
      { id: "tok_2", token: "secret-token", revoke: "secret-revoke", keys: [], sandbox: "host", mode: "app" },
      { revoked: "tok_1" },
    ]);
    expect(tokens).toHaveLength(1);
    expect(tokens[0].token).toBe("secret-token");
    expect(tokens[0]).not.toHaveProperty("revoke");
  });
});

describe("presence kind", () => {
  test("the daemon's word wins, else the platform decides", () => {
    expect(presenceKind({ platform: "linux", presence: "polkit" })).toBe("polkit");
    expect(presenceKind({ platform: "macos" })).toBe("touch_id");
    expect(presenceKind({ platform: "linux" })).toBe("passphrase");
    expect(presenceKind(null)).toBe("passphrase");
  });

  test("polkit waits for the system prompt and never asks for a passphrase", async () => {
    const seen: boolean[] = [];
    let asked = 0;
    const failure = Object.assign(new Error("no"), { status: 401, code: "presence_required" });
    await expect(
      withPresence(true, { kind: "polkit", waiting: (on) => seen.push(on), ask: async () => (asked++, "x") }, async () => {
        throw failure;
      }),
    ).rejects.toBe(failure);
    expect(seen).toEqual([true, false]);
    expect(asked).toBe(0);
  });
});
