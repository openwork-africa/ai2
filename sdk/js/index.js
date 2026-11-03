export class VaultDenied extends Error {
  constructor(reason, intent) {
    super(`vault denied: ${reason}`);
    this.name = "VaultDenied";
    this.reason = reason;
    this.intent = intent;
  }
}

const NULL_BODY = new Set([101, 204, 205, 304]);

function env(name) {
  try {
    return globalThis.Deno ? globalThis.Deno.env.get(name) : globalThis.process?.env?.[name];
  } catch {
    return undefined;
  }
}

function tcp(endpoint) {
  return ({ method, path, headers, body, signal }) =>
    fetch(new URL(path, endpoint), { method, headers, body, signal, duplex: "half", redirect: "manual" });
}

function bunUnix(socketPath) {
  return ({ method, path, headers, body, signal }) =>
    fetch(`http://localhost${path}`, { unix: socketPath, method, headers, body, signal, redirect: "manual" });
}

function responseHeaders(raw) {
  const headers = new Headers();
  for (let i = 0; i < raw.length; i += 2) headers.append(raw[i], raw[i + 1]);
  return headers;
}

async function writeBody(req, body) {
  if (!body) return req.end();
  if (body instanceof Uint8Array) return req.end(body);
  const reader = body.getReader();
  try {
    for (;;) {
      const { done, value } = await reader.read();
      if (done) break;
      if (!req.write(value)) await new Promise((resolve) => req.once("drain", resolve));
    }
    req.end();
  } catch (error) {
    req.destroy(error);
  }
}

function nodeUnix(socketPath) {
  const modules = Promise.all([import("node:http"), import("node:stream")]);
  return async ({ method, path, headers, body, signal }) => {
    const [http, stream] = await modules;
    return new Promise((resolve, reject) => {
      const req = http.request(
        { socketPath, method, path, headers: Object.fromEntries(headers), signal },
        (res) => {
          const empty = method === "HEAD" || NULL_BODY.has(res.statusCode);
          if (empty) res.resume();
          resolve(
            new Response(empty ? null : stream.Readable.toWeb(res), {
              status: res.statusCode,
              statusText: res.statusMessage,
              headers: responseHeaders(res.rawHeaders),
            }),
          );
        },
      );
      req.on("error", reject);
      writeBody(req, body);
    });
  };
}

function transport(endpoint) {
  if (endpoint.startsWith("unix:")) {
    const socketPath = endpoint.slice("unix:".length);
    return globalThis.Bun ? bunUnix(socketPath) : nodeUnix(socketPath);
  }
  if (!/^http:\/\//.test(endpoint)) throw new TypeError(`vault: endpoint must be unix:<path> or http://host:port, got ${endpoint}`);
  return tcp(endpoint);
}

async function requestBody(req, input, given) {
  if (!req.body) return undefined;
  if (given instanceof ReadableStream || (given === undefined && input instanceof Request)) return req.body;
  return new Uint8Array(await req.arrayBuffer());
}

function urlOf(input) {
  if (typeof input === "string") return input;
  return input.url ?? input.href;
}

export function vault(options = {}) {
  const token = options.token ?? env("AI2_VAULT_TOKEN");
  const endpoint = options.endpoint ?? env("AI2_VAULT_ENDPOINT");
  if (!token) throw new TypeError("vault: token is required (or AI2_VAULT_TOKEN)");
  if (!endpoint) throw new TypeError("vault: endpoint is required (or AI2_VAULT_ENDPOINT)");
  const send = transport(endpoint);

  async function vaultFetch(key, input, init = {}) {
    const { request, ...rest } = init ?? {};
    const req = new Request(input, { ...rest, duplex: "half" });
    const target = new URL(req.url);
    const headers = new Headers(req.headers);
    for (const name of [...headers.keys()]) if (name.startsWith("x-vault-")) headers.delete(name);
    headers.set("Authorization", `Bearer ${token}`);
    if (request != null) headers.set("X-Vault-Request", encodeURIComponent(String(request).replace(/\s+/g, " ").trim()));
    const body = await requestBody(req, input, rest.body);
    const path = `/${encodeURIComponent(key)}/${target.host}${target.pathname}${target.search}`;
    const res = await send({ method: req.method, path, headers, body, signal: req.signal });
    if (res.status === 403) {
      const denied = await res.clone().json().catch(() => null);
      if (denied?.error === "denied") {
        await res.body?.cancel();
        throw new VaultDenied(denied.reason, denied.intent);
      }
    }
    return res;
  }

  function wrap(fetchFn, routes) {
    const entries = Object.entries(routes).sort(([a], [b]) => b.length - a.length);
    return (input, init) => {
      const url = urlOf(input);
      const hit = entries.find(([prefix]) => url.startsWith(prefix));
      return hit ? vaultFetch(hit[1], input, init) : fetchFn(input, init);
    };
  }

  return { fetch: vaultFetch, wrap };
}
