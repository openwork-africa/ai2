import { describe, it, before, after } from "node:test";
import assert from "node:assert/strict";
import { vault, VaultDenied } from "../index.js";
import { gateway } from "./gateway.js";

for (const kind of ["unix", "tcp"]) {
  describe(`vault over ${kind}`, () => {
    let gw;
    let v;
    before(async () => {
      gw = await gateway();
      v = vault({ token: "vt_test", endpoint: gw[kind] });
    });
    after(() => gw.close());

    it("sends the key and target in the path and the token as Bearer", async () => {
      const res = await v.fetch("stripe", "https://api.stripe.com/v1/charges?limit=3", {
        method: "POST",
        headers: { "content-type": "application/json", "x-vault-token": "forged", authorization: "Bearer mine" },
        body: JSON.stringify({ amount: 5 }),
      });
      assert.equal(res.status, 201);
      assert.equal(res.headers.get("x-upstream"), "yes");
      assert.equal(await res.text(), 'echo:{"amount":5}');
      const got = gw.seen.at(-1);
      assert.equal(got.method, "POST");
      assert.equal(got.path, "/stripe/api.stripe.com/v1/charges?limit=3");
      assert.equal(got.headers.authorization, "Bearer vt_test");
      assert.equal(got.headers["x-vault-token"], undefined);
      assert.equal(got.key, "stripe");
      assert.equal(got.url, "https://api.stripe.com/v1/charges?limit=3");
      assert.equal(got.headers["content-type"], "application/json");
      assert.equal(got.headers["x-vault-request"], undefined);
    });

    it("sends init.request as X-Vault-Request and drops it from init", async () => {
      const init = { method: "POST", body: "x", request: "refund order 1042\nthe user asked for" };
      await (await v.fetch("stripe", "https://api.stripe.com/v1/refunds", init)).text();
      const got = gw.seen.at(-1);
      assert.equal(got.headers["x-vault-request"], "refund%20order%201042%20the%20user%20asked%20for");
      assert.equal(got.headers.request, undefined);
      assert.equal(init.request, "refund order 1042\nthe user asked for");
    });

    it("percent-encodes X-Vault-Request as UTF-8", async () => {
      await (await v.fetch("k", "https://example.com/", { request: "pay 🚀 счёт 東京" })).text();
      const sent = gw.seen.at(-1).headers["x-vault-request"];
      assert.equal(sent, "pay%20%F0%9F%9A%80%20%D1%81%D1%87%D1%91%D1%82%20%E6%9D%B1%E4%BA%AC");
      assert.equal(decodeURIComponent(sent), "pay 🚀 счёт 東京");
    });

    it("streams a request body", async () => {
      const body = new ReadableStream({
        start(c) {
          c.enqueue(new TextEncoder().encode("a"));
          c.enqueue(new TextEncoder().encode("b"));
          c.close();
        },
      });
      const res = await v.fetch("k", "https://example.com/up", { method: "PUT", body });
      assert.equal(await res.text(), "echo:ab");
    });

    it("streams an SSE response chunk by chunk", { timeout: 5000 }, async () => {
      const res = await v.fetch("openai", "https://api.openai.com/sse");
      assert.equal(res.headers.get("content-type"), "text/event-stream");
      const reader = res.body.pipeThrough(new TextDecoderStream()).getReader();
      const first = await reader.read();
      assert.equal(first.value, "data: one\n\n");
      gw.gate.open();
      let rest = "";
      for (;;) {
        const { done, value } = await reader.read();
        if (done) break;
        rest += value;
      }
      assert.equal(rest, "data: two\n\n");
    });

    it("throws VaultDenied on a denied 403", async () => {
      await assert.rejects(v.fetch("stripe", "https://api.stripe.com/denied"), (error) => {
        assert.ok(error instanceof VaultDenied);
        assert.equal(error.reason, "url_not_allowed");
        assert.equal(error.intent, "pay invoices");
        return true;
      });
    });

    it("returns other 403s normally", async () => {
      const res = await v.fetch("stripe", "https://api.stripe.com/forbidden");
      assert.equal(res.status, 403);
      assert.deepEqual(await res.json(), { error: "nope" });
    });

    it("wrap routes mapped prefixes through the vault", async () => {
      const passed = [];
      const plain = async (input, init) => {
        passed.push([input, init]);
        return new Response("plain");
      };
      const f = v.wrap(plain, { "https://api.stripe.com/": "stripe", "https://api.stripe.com/v2/": "stripe2" });
      const before = gw.seen.length;

      assert.equal(await (await f("https://api.stripe.com/v1/x")).text(), "echo:");
      assert.equal(gw.seen.at(-1).key, "stripe");
      await (await f(new URL("https://api.stripe.com/v2/y"))).text();
      assert.equal(gw.seen.at(-1).key, "stripe2");
      await (await f(new Request("https://api.stripe.com/v1/z", { method: "POST", body: "r" }))).text();
      assert.equal(gw.seen.at(-1).body, "r");
      assert.equal(gw.seen.length, before + 3);

      const init = { method: "GET" };
      assert.equal(await (await f("https://other.example/", init)).text(), "plain");
      assert.deepEqual(passed, [["https://other.example/", init]]);
      assert.equal(gw.seen.length, before + 3);
    });
  });
}

describe("vault options", () => {
  it("needs a token and an endpoint", () => {
    assert.throws(() => vault({ token: "", endpoint: "unix:/x" }), TypeError);
    assert.throws(() => vault({ token: "t", endpoint: "ftp://x" }), TypeError);
  });

  it("reads AI2_VAULT_TOKEN and AI2_VAULT_ENDPOINT", async () => {
    const gw = await gateway();
    const set = (k, val) => (globalThis.Deno ? globalThis.Deno.env.set(k, val) : (process.env[k] = val));
    set("AI2_VAULT_TOKEN", "vt_env");
    set("AI2_VAULT_ENDPOINT", gw.unix);
    try {
      await (await vault().fetch("k", "https://example.com/")).text();
      assert.equal(gw.seen.at(-1).headers.authorization, "Bearer vt_env");
    } finally {
      await gw.close();
    }
  });
});
