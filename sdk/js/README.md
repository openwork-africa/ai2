# @ai2/vault

Call APIs through the ai2 vault gateway. Your code holds a vault token, never the API key. ESM, no dependencies, Node 20+, Bun and Deno.

```js
import { vault, VaultDenied } from "@ai2/vault";

const v = vault({ token: process.env.AI2_VAULT_TOKEN, endpoint: process.env.AI2_VAULT_ENDPOINT });

const res = await v.fetch("stripe", "https://api.stripe.com/v1/charges", { method: "GET" });

const refund = await v.fetch("stripe", "https://api.stripe.com/v1/refunds", {
  method: "POST",
  body: new URLSearchParams({ charge: "ch_1" }),
  request: "refund order 1042 the user asked for",
});
```

`vault()` with no options reads `AI2_VAULT_TOKEN` and `AI2_VAULT_ENDPOINT`.

- `endpoint` is `unix:/path/to.sock` or `http://host:port`.
- A call goes to `<endpoint>/<key>/<host>/<path>?<query>` with `Authorization: Bearer <token>`; any HTTP client can call the gateway the same way.
- `v.fetch(key, url, init)` takes the same `init` as `fetch`, plus `request`: what the call is for, needed with agent tokens. It is sent as `X-Vault-Request`, whitespace collapsed and percent-encoded as UTF-8, so any text works.
- It returns a standard `Response`. The body streams, so SSE works as it arrives.
- Redirects are returned, not followed.

## Refusals

A call the vault refuses throws `VaultDenied`:

```js
try {
  await v.fetch("stripe", url, { method: "POST", body, request: "..." });
} catch (error) {
  if (error instanceof VaultDenied) console.log(error.reason, error.intent);
}
```

`reason` is one of `token_unknown`, `token_expired`, `token_used_up`, `wrong_sandbox`, `key_not_allowed`, `method_not_allowed`, `url_not_allowed`, `vetting_denied`, `vetting_unavailable`, `approval_denied`, `approval_timeout`. Any other status, the API's own 403 included, comes back as a normal `Response`.

## Wrapping fetch

```js
const f = v.wrap(fetch, {
  "https://api.stripe.com/": "stripe",
  "https://api.openai.com/": "openai",
});

await f("https://api.stripe.com/v1/charges");
await f("https://example.com/");
```

Calls whose URL starts with a mapped prefix go through the vault with that key; the longest prefix wins. Everything else goes to the `fetch` you passed, unchanged. Hand `f` to any client that takes a custom `fetch`.

## Tests

```sh
node --test test/vault.test.js
bun test ./test/vault.test.js
deno test --allow-read --allow-write --allow-net --allow-env --allow-sys test/vault.test.js
```
