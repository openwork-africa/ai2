import { describe, expect, test } from "bun:test";
import { type Connections, type McpClient, clientLabel, connectedNames, minutesLeft, pairLine, pasteLine, showCard } from "../src/lib/mcp";

const client = (over: Partial<McpClient>): McpClient => ({
  name: "claude-code",
  version: "2.1.0",
  transport: "stdio",
  token: null,
  first_seen: 1,
  last_seen: 1,
  ...over,
});

const connections = (clients: McpClient[]): Connections => ({ clients, tokens: [], pairing: null, url: "http://127.0.0.1:7790/mcp" });

describe("the connection card", () => {
  test("it shows only once the list is in and no client has ever connected", () => {
    expect(showCard(null)).toBe(false);
    expect(showCard(connections([]))).toBe(true);
    expect(showCard(connections([client({})]))).toBe(false);
    expect(showCard(connections([client({ transport: "http", token: "mcp_1" })]))).toBe(false);
  });

  test("the connected line names each client once, the latest first, by its plain name", () => {
    expect(connectedNames(null)).toEqual([]);
    expect(
      connectedNames(
        connections([
          client({ last_seen: 1 }),
          client({ name: "codex-mcp-client", last_seen: 5 }),
          client({ transport: "http", last_seen: 3 }),
          client({ name: "my-tool", last_seen: 2 }),
        ]),
      ),
    ).toEqual(["Codex", "Claude Code", "my-tool"]);
    expect(clientLabel("claude-code")).toBe("Claude Code");
  });

  test("the lines to paste point at this page's skill", () => {
    expect(pasteLine("http://127.0.0.1:7790")).toBe("Read http://127.0.0.1:7790/skill.md and set up ai2.");
    expect(pairLine("http://127.0.0.1:7790", "ABCD2345")).toBe(
      "Read http://127.0.0.1:7790/skill.md and connect to ai2 over HTTP with pairing code ABCD2345.",
    );
    expect(minutesLeft(1600, 1000 * 1000)).toBe(10);
    expect(minutesLeft(1000, 2000 * 1000)).toBe(0);
  });
});
