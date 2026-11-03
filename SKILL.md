---
name: ai2
description: Install and set up ai2 (ai squared) on the user's machine, connect it to this coding agent over MCP, then register a local app or run a security review of a folder. Use when the user asks to set up, install or connect ai2 or ai squared.
---

# Set up ai2

ai squared keeps the user's local apps, their coding agents' work and their API keys on their own machine. The command is `ai2`.

You will:

1. Install the binary.
2. Run setup, while the human approves in the browser.
3. Check it worked.
4. Connect ai2 to yourself over MCP.
5. Register an app, or run a security review.

Talk to the human in short, plain lines. Never ask them to paste a secret into this chat.

## 1. Install

Check first: `ai2 --version`. If it prints a version, skip to step 2.

macOS and Linux:

```sh
curl -fsSL https://raw.githubusercontent.com/openwork-africa/ai2/main/install.sh | sh
```

It puts `ai2` in `~/.local/bin` after checking its SHA-256.
If the script says to add `~/.local/bin` to `PATH`, do it for this shell (`export PATH="$HOME/.local/bin:$PATH"`) and tell the human to add it to their shell profile.

Windows: there is no Windows build. Install inside WSL2 the same way. Tell the human this should work but has not been tried yet. Setup there needs systemd on: `[boot]` and `systemd=true` in `/etc/wsl.conf`, then `wsl --shutdown` from Windows and open WSL again.

## 2. Run setup

Setup needs the human: it asks them to approve once or twice.

- On a Mac: Touch ID, or the Mac password.
- On Linux: the login password through polkit (a desktop prompt), or a vault passphrase they choose if polkit is not installed. Linux setup also asks for `sudo` once.

Start the page and leave it running (run it in the background; it does not exit):

```sh
ai2 setup
```

It opens the browser and prints one JSON line with the page's `url`. Tell the human:

> Setup is open in your browser. Go through the welcome screens, then approve with Touch ID (or your password) when it asks. Tell me when it says it is done.

If no browser opened, give them the `url` from that line.

If there is no desktop (a server, WSL2, SSH), use the terminal mode instead, in a terminal the human can type in:

```sh
ai2 setup --headless
```

It prints one JSON object when done. The human answers the prompts there (pkttyagent for polkit, or the passphrase).

If setup is already done, `ai2 setup` just opens the page; that is fine.

## 3. Check it worked

```sh
ai2 doctor --json
```

It prints `[{"check", "state", "detail", "fix"}]`. `state` is `ok`, `warn` or `fail`. The command exits 0 when nothing is `fail`.

- All `ok` or `warn`: setup is done.
- A `fail`: read its `fix` and do it, or tell the human what it says.
- A `warn` on `containers` means agents cannot be boxed in on this machine (it needs macOS 26 on Apple silicon, or systemd on Linux). Tell the human: agents will run on the host. Do not try to fix it.
- A `warn` or `fail` on `vetting-model` means there is no providers file yet. Say so and go on; step 5 says where to set it.

## 4. Connect ai2 to yourself

ai2 serves its tools over MCP on stdio: `ai2 mcp`.

- Claude Code: `claude mcp add ai2 -- ai2 mcp`
- Codex: `codex mcp add ai2 -- ai2 mcp`
- Any other MCP client: add `{ "command": "ai2", "args": ["mcp"] }` to its MCP servers.
- No MCP support (for example pi): use the `ai2` commands below instead.

The tools include `apps_register`, `apps_list`, `apps_start`, `reviews_run`, `reviews_list`, `vault_list_keys`, `harness_list` and `setup_check`. You may need a restart of your session to see them; say so if so.

## 5. Do one useful thing

Ask the human which they want, or do the one that fits what they are working on.

### Register an app

For a local app with a web server and an OpenAPI document. Run it in the app's folder:

```sh
ai2 apps register --name "Notes" --description "One line on what it is for" --command "deno task start" --openapi /openapi.json --icon favicon.svg
```

- `--openapi` is required: a path under the app's URL, or a file.
- `--port` if ai2 cannot tell the port from the command.
- `--icon <file>` is required: the app's favicon, a png, svg, ico, webp or jpg up to 512 KiB.
- Or the MCP tool `apps_register` with `name`, `description`, `command`, `cwd` (absolute), `openapi` and `icon` (its type and base64 bytes).

Then `ai2 apps start <id>` and `ai2 apps status <id>`. Tell the human it is on the **Apps** page of `ai2 ui`.

### Run a security review

For a folder a coding agent worked in:

```sh
ai2 --run-path <folder> --objective "<what the run was asked to do>" --changed <a,b> --json
```

Or the MCP tool `reviews_run`. A review can take many minutes (the budget is 35 by default).

- The Modern Security Rating checks the folder's stack against exploits published in recent days. It needs a model in `~/.ai2/providers.json`.
- The Run Risk Factor has a coding harness (Muse Code or pi) read the files the run changed, with four security skills.
- It does not scan the machine for malware or watch what agents do. Do not say it does.

If a rating is missing, its `missing` field says why: most often no providers file or no harness. Tell the human to open **Security** in `ai2 ui` and press the cog to pick the model and harness, or `ai2 harness list` to see which are installed.

The review is written to `~/.ai2/reviews/<hash>/` and shows on the **Security** page.

### Add a key to the vault

Do not take the secret yourself. Tell the human:

> Open the Vault page (`ai2 vault ui`) and press Add key, or run `ai2 vault put --name <name> --approved-url <https://api.example.com/>` in your own terminal. It asks for the secret without showing it, then for Touch ID.

To use a key later, the human starts you with `ai2 vault with <keys> -- <your command>`; `ai2 vault skill` prints how to call the gateway.

## When done

Tell the human, in a few lines:

- what is installed (`ai2`, the vault service, and the dashboard at `http://127.0.0.1:7797`, which starts at login like the vault and is opened with `ai2 ui`);
- what you connected (MCP);
- what you registered or reviewed, and where to see it.
