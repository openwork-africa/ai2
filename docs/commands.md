# Commands

| Command | |
| --- | --- |
| `ai2 [flags]` | runs a security review of a folder (flags below) |
| `ai2 setup [--headless]` | sets up the vault: containers, the master key, the human check, the service |
| `ai2 apps register\|list\|show\|check\|api\|start\|stop\|restart\|status\|logs\|remove` | your local apps; each needs an OpenAPI document and an icon |
| `ai2 vault put\|list\|has\|delete\|rotate\|token\|sandbox\|approvals\|audit\|status\|apply\|migrate\|skill\|with\|ui` | keys, tokens, sandboxes and the gateway |
| `ai2 harness list\|install\|sign-in\|command` | coding agents (below) |
| `ai2 ui [--port n] [--no-open] [--tab page]` | the page on `127.0.0.1`; port 7797, or `AI2_UI_PORT`; opens the dashboard already running, or serves one; `--port 0` picks a free one |
| `ai2 mcp` | MCP over stdio; `ai2 mcp openapi` prints the tools as an OpenAPI document |
| `ai2 doctor [--json]` | checks the binary, versions, setup, sockets, sandboxes and the vetting model; `--json` prints `[{check, state, detail, fix}]` |

`ai2 vault with <keys> -- <command>` runs a command with a pass for some keys, and takes the pass back when it exits. A throwaway container run asks you once: the sandbox and the pass are made in one batch, and both are taken back without a second prompt. `ai2 vault migrate --take|--all` stores every chosen key with one prompt.
`ai2 vault skill` prints the guide an agent reads to call the gateway.

State is kept in `$AI2_HOME`, else `~/.ai2`.
