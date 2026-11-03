import http from "node:http";
import fs from "node:fs";
import os from "node:os";
import path from "node:path";

export async function gateway() {
  const seen = [];
  let release;
  const gate = { open: () => release?.() };

  const server = http.createServer(async (req, res) => {
    const chunks = [];
    for await (const chunk of req) chunks.push(chunk);
    const body = Buffer.concat(chunks).toString();
    const [, key = "", ...rest] = req.url.split("/");
    const target = new URL(`https://${rest.join("/")}`);
    seen.push({ method: req.method, path: req.url, key: decodeURIComponent(key), url: target.href, headers: req.headers, body });

    if (target.pathname === "/denied") {
      res.writeHead(403, { "content-type": "application/json" });
      return res.end(JSON.stringify({ error: "denied", reason: "url_not_allowed", intent: "pay invoices" }));
    }
    if (target.pathname === "/forbidden") {
      res.writeHead(403, { "content-type": "application/json" });
      return res.end(JSON.stringify({ error: "nope" }));
    }
    if (target.pathname === "/sse") {
      res.writeHead(200, { "content-type": "text/event-stream", "cache-control": "no-cache" });
      res.write("data: one\n\n");
      await new Promise((resolve) => (release = resolve));
      res.write("data: two\n\n");
      return res.end();
    }
    res.writeHead(201, { "content-type": "text/plain", "x-upstream": "yes" });
    res.end(`echo:${body}`);
  });

  const dir = fs.mkdtempSync(path.join(os.tmpdir(), "ai2-vault-"));
  const socket = path.join(dir, "gw.sock");
  await new Promise((resolve) => server.listen(socket, resolve));
  const tcpServer = http.createServer((req, res) => server.emit("request", req, res));
  await new Promise((resolve) => tcpServer.listen(0, "127.0.0.1", resolve));

  return {
    seen,
    gate,
    unix: `unix:${socket}`,
    tcp: `http://127.0.0.1:${tcpServer.address().port}`,
    close: async () => {
      gate.open();
      server.closeAllConnections?.();
      tcpServer.closeAllConnections?.();
      await Promise.all([new Promise((r) => server.close(r)), new Promise((r) => tcpServer.close(r))]);
      fs.rmSync(dir, { recursive: true, force: true });
    },
  };
}
