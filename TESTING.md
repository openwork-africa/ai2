# Testing ai2

Four levels, from fast and safe to slow and real. Run them in this order.

## 1. Unit and feature tests (any machine, a few minutes)

```sh
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
cargo build --no-default-features
cargo build --no-default-features --features harness,vault
cd crates/ui/web && bun install && bun run build && bun run lint && bun test test tests
```

- The second build is the shape ara compiles. `cargo tree -e normal --no-default-features --features harness,vault` must show no `setup`, `apps`, `ui`, `mcp`, `axum`, `tokio` or `rmcp`.
- These tests never touch the real Keychain, Touch ID, polkit, systemd, launchd or containers. Those paths sit behind fakes; the live checks are `#[ignore]`.
- The rust toolchain is 1.88. If a lock update pulls `uuid` past 1.18.1, run `cargo update uuid --precise 1.18.1`.
- `bun run build` rewrites `crates/ui/dist`. Commit it only when the web code changed on purpose; CI checks it matches.

## 2. Linux, end to end, in an Apple container (macOS 26 on Apple silicon)

```sh
./tests/linux/run.sh          # builds, sets up and checks everything, then removes the container and image
KEEP=1 ./tests/linux/run.sh   # keeps them for poking around: container exec -it ai2-linux-test bash
```

The flavour is **Debian trixie (`debian:trixie-slim`) with systemd as PID 1**, built from `tests/linux/Containerfile`:

- systemd, dbus, sudo, expect, build tools and a stable Rust toolchain;
- a user `dev` with passwordless sudo who runs the tests, and a user `eve` who checks that another user is refused;
- run with `--cap-add ALL` (systemd cannot mount `/run` without it), and `/` made rshared before systemd starts (`mount --make-rshared / && exec /sbin/init`), so `LoadCredentialEncrypted` reaches the vault service. Both are only needed in this container; a normal Linux host does them itself.

`tests/linux/inside.sh` runs three phases as `dev`: **build** (fmt, clippy, tests inside Linux), **setup** (`ai2 setup --headless`, the passphrase typed through a pty by `answer.exp`, then `ai2 doctor --json`), and **vault** (keys, sandboxes, tokens, a real call through the gateway to example.com, a call outside the approved URLs that waits and is denied, `vault with`, the audit log, other users refused, a daemon restart). It prints `PASS` and `FAIL` lines and exits non-zero on any failure.

Build output and the Cargo cache go to `target/linux/` in the repo, so later runs are quick. Use only the local `debian:trixie-slim` base; never touch other containers or images on the machine.

## 3. macOS, end to end, in a throwaway place

```sh
./tests/macos/vault-e2e.sh
```

It builds ai2 and runs `ai2 setup --headless` against a throwaway home, never the real one:

- `AI2_HOME` is a temp folder under `/tmp` (the sandbox socket path must stay under 104 bytes);
- `AI2_VAULT_KEYCHAIN` points at a test keychain file it creates and deletes;
- `AI2_VAULT_LABEL` gives the vault LaunchAgent a test label, booted out at the end.

Expect **four Touch ID prompts**, each named before it shows: store a key, add a sandbox, make a token, and `vault with`. It checks a real call through the gateway, a denied call outside the approved URLs, a container calling the gateway socket (`debian:trixie-slim`, the socket mounted as a file with `--volume <sock>:/run/ai2-vault.sock`), and the UI's nonce checks. Cleanup runs on exit or Ctrl-C and leaves no keychain item, LaunchAgent, temp folder or container behind.

## 4. The UI by eye

```sh
./tests/demo/seed.sh start /tmp/ai2-demo        # four demo apps (two running), two reviews
AI2_HOME=/tmp/ai2-demo cargo run -p ui --example serve -- --port 0 --no-open
./tests/demo/seed.sh stop /tmp/ai2-demo
cargo run -p ui --example setup_demo -- running   # the live setup page, driven by a fake: running|input|sudo|failed|unsupported|warned|live
```

`serve` prints a URL with its nonce. For screenshots use Playwright's own Chromium (`playwright screenshot …`, or its Python package), never your everyday browser. The README images are `.github/home.png` and `.github/security.png`, taken at 1440×900 @2x from the seeded demo.

## Not covered yet

- **Windows through WSL2** should work as Linux with `systemd=true` in `/etc/wsl.conf`, but nobody has run it.
- **A real install from a release** (`install.sh`, then `ai2 setup` in the browser) is checked by hand on a clean Mac after each tag.
