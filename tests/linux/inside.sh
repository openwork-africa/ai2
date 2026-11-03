#!/bin/bash
set -u

export PATH=/opt/rust/cargo/bin:/usr/local/sbin:/usr/local/bin:/usr/sbin:/usr/bin:/sbin:/bin
export RUSTUP_HOME=/opt/rust/rustup CARGO_HOME=${CARGO_HOME_DIR:-/src/target/linux/cargo}
export CARGO_TARGET_DIR=${CARGO_TARGET_DIR:-/src/target/linux}
export HOME=/home/dev
export PASS=${PASS:-correct horse battery staple}
AI2=/home/dev/ai2
HERE=$(cd "$(dirname "$0")" && pwd)
OUT=/home/dev/out
mkdir -p "$OUT"
failed=0

pass() { echo "PASS $1"; }
fail() { echo "FAIL $1${2:+: $2}"; failed=$((failed + 1)); }
expect_ok() { local name=$1; shift; if "$@"; then pass "$name"; else fail "$name"; fi; }
answer() { expect "$HERE/answer.exp" "$@"; }
field() { sed -n "s/.*\"$1\":\"\\([^\"]*\\)\".*/\\1/p" "$2" | head -1; }

build() {
    cd /src || exit 1
    expect_ok "cargo fmt --check" cargo fmt --all --check
    expect_ok "cargo clippy -D warnings" cargo clippy --workspace --all-targets -- -D warnings
    expect_ok "cargo test" cargo test --workspace
    expect_ok "cargo build" cargo build
    cp "$CARGO_TARGET_DIR/debug/ai2" "$AI2"
}

setup() {
    cd /home/dev || exit 1
    "$AI2" setup --check > "$OUT/check1.json"
    echo "before: $(cat "$OUT/check1.json")"
    expect_ok "ai2 setup" answer sh -c "$AI2 setup > $OUT/setup.json"
    echo "setup: $(cat "$OUT/setup.json")"
    "$AI2" setup --check > "$OUT/check2.json"
    echo "after: $(cat "$OUT/check2.json")"
    expect_ok "setup --check is ok after setup" grep -q '"ok":true' "$OUT/check2.json"
    for step in containers master-key presence daemon; do
        expect_ok "step $step ready" grep -q "\"$step\":{\"status\":\"ready\"" "$OUT/check2.json"
    done
    expect_ok "master.cred exists" sudo test -s /etc/ai2-vault/master.cred
    expect_ok "master.cred decrypts to 32 bytes" test "$(sudo systemd-creds decrypt --name=master /etc/ai2-vault/master.cred - | wc -c)" = 32
    expect_ok "service binary installed" cmp -s "$AI2" /usr/local/libexec/ai2-vault/ai2
    expect_ok "ai2-vault.service active" systemctl is-active --quiet ai2-vault.service
    expect_ok "AI2_VAULT_OWNERS is dev's uid" test "$(systemctl show -P Environment ai2-vault.service)" = "AI2_VAULT_OWNERS=$(id -u dev)"
    expect_ok "dev is in ai2-vault-admin" sh -c 'id -nG dev | grep -qw ai2-vault-admin'
    expect_ok "admin.sock is 0660 ai2-vault:ai2-vault-admin" test "$(stat -c '%a %U %G %F' /run/ai2-vault/admin.sock)" = "660 ai2-vault ai2-vault-admin socket"
    expect_ok "passphrase hash is private" test "$(sudo stat -c '%a %U' /var/lib/ai2-vault/passphrase)" = "600 ai2-vault"
    expect_ok "passphrase hash is argon2id" sudo grep -q '^\$argon2id\$' /var/lib/ai2-vault/passphrase
    expect_ok "state dir is 0700 ai2-vault" test "$(sudo stat -c '%a %U' /var/lib/ai2-vault)" = "700 ai2-vault"
}

hardening() {
    local pid
    pid=$(systemctl show -P MainPID ai2-vault.service)
    for want in "NoNewPrivileges=yes" "MemoryDenyWriteExecute=yes" "CapabilityBoundingSet=" "AmbientCapabilities=" \
        "RestrictAddressFamilies=AF_INET AF_INET6 AF_UNIX" "ProtectSystem=strict" "ProtectHome=yes" "PrivateTmp=yes" \
        "PrivateDevices=yes" "LimitCORE=0" "User=ai2-vault" "Group=ai2-vault-admin"; do
        expect_ok "unit has $want" sh -c "systemctl show ai2-vault.service -p '${want%%=*}' | grep -qxF '$want'"
    done
    expect_ok "daemon has no effective capabilities" grep -qE '^CapEff:\s+0+$' "/proc/$pid/status"
    expect_ok "daemon has no bounding capabilities" grep -qE '^CapBnd:\s+0+$' "/proc/$pid/status"
    expect_ok "daemon runs as ai2-vault" test "$(stat -c %U "/proc/$pid")" = ai2-vault
    echo "systemd-analyze security: $(systemd-analyze security ai2-vault.service 2>/dev/null | tail -1)"
}

vault() {
    if ! id -nG | grep -qw ai2-vault-admin; then
        exec sudo -u dev --preserve-env=PASS "$0" vault
    fi
    cd /home/dev || exit 1
    local sock token id
    expect_ok "vault status" sh -c "$AI2 vault status > $OUT/status.json"
    echo "status: $(cat "$OUT/status.json")"
    expect_ok "status: master key loaded" grep -q '"master_key":true' "$OUT/status.json"
    expect_ok "vault put example" answer sh -c "printf 'not-a-real-secret-123' | $AI2 vault put --name example --approved-url https://example.com/ --secret - > $OUT/put.json"
    echo "put: $(cat "$OUT/put.json")"
    expect_ok "vault put with a wrong passphrase is refused" sh -c "PASS='wrong passphrase!' expect $HERE/answer.exp sh -c '$AI2 vault put --name bad --approved-url https://example.com/ --secret x > $OUT/bad.json 2>/dev/null; test \$? = 2'"
    "$AI2" vault list > "$OUT/list.json"
    echo "list: $(cat "$OUT/list.json")"
    expect_ok "vault list shows example" grep -q '"example"' "$OUT/list.json"
    expect_ok "vault list has no secret" sh -c "! grep -q not-a-real-secret $OUT/list.json"
    expect_ok "vault has example" sh -c "$AI2 vault has --name example | grep -q '\"exists\":true'"
    expect_ok "vault has missing exits 1" sh -c "$AI2 vault has --name nope >/dev/null; test \$? = 1"
    expect_ok "sandbox add box" answer sh -c "$AI2 vault sandbox add --name box > $OUT/box.json"
    echo "box: $(cat "$OUT/box.json")"
    expect_ok "sandbox add host" answer sh -c "$AI2 vault sandbox add --name host > $OUT/host.json"
    sock=$(field path "$OUT/box.json")
    echo "box socket: $sock $(stat -c '%a %U %G' "$sock") in $(stat -c '%a %U %G' "$(dirname "$sock")")"
    expect_ok "token create" answer sh -c "$AI2 vault token create --keys example --sandbox box --mode app > $OUT/token.json"
    token=$(field token "$OUT/token.json")
    expect_ok "token printed" test -n "$token"
    expect_ok "revoke value not printed" sh -c "! grep -q '\"revoke\"' $OUT/token.json"
    curl -sS --max-time 60 --unix-socket "$sock" -H "Authorization: Bearer $token" http://v/example/example.com/ -o "$OUT/page.html" -w '%{http_code}' > "$OUT/page.code"
    echo "gateway call: HTTP $(cat "$OUT/page.code")"
    expect_ok "gateway reaches example.com" grep -q "Example Domain" "$OUT/page.html"
    curl -sS --max-time 60 --unix-socket "$sock" -H "Authorization: Bearer nope" http://v/example/example.com/ -o "$OUT/unknown.json" -w '%{http_code}' > "$OUT/unknown.code"
    expect_ok "unknown token is 403 token_unknown" sh -c "test \"\$(cat $OUT/unknown.code)\" = 403 && grep -q token_unknown $OUT/unknown.json"
    curl -sS --max-time 150 --unix-socket "$sock" -H "Authorization: Bearer $token" http://v/example/www.iana.org/ -o "$OUT/outside.json" -w '%{http_code}' > "$OUT/outside.code" &
    local waiter=$!
    id=""
    for _ in $(seq 1 30); do
        "$AI2" vault approvals > "$OUT/approvals.json"
        id=$(field id "$OUT/approvals.json")
        [ -n "$id" ] && break
        sleep 1
    done
    echo "waiting: $(cat "$OUT/approvals.json")"
    expect_ok "outside call waits for approval" test -n "$id"
    expect_ok "outside call still waiting" kill -0 "$waiter"
    expect_ok "approvals --deny" answer sh -c "$AI2 vault approvals --deny $id > $OUT/deny.json"
    wait "$waiter"
    echo "outside call: HTTP $(cat "$OUT/outside.code") $(cat "$OUT/outside.json")"
    expect_ok "outside call denied" sh -c "test \"\$(cat $OUT/outside.code)\" = 403 && grep -q approval_denied $OUT/outside.json"
    expect_ok "vault with" answer sh -c "$AI2 vault with example --mode app -- sh -c 'echo \$AI2_VAULT_ENDPOINT; curl -sS --max-time 60 --unix-socket \${AI2_VAULT_ENDPOINT#unix:} -H \"Authorization: Bearer \$AI2_VAULT_TOKEN\" http://v/example/example.com/' > $OUT/with.txt"
    echo "with: $(head -1 "$OUT/with.txt")"
    expect_ok "with gives a unix endpoint" grep -q '^unix:/run/ai2-vault/sandboxes/host/gateway.sock$' "$OUT/with.txt"
    expect_ok "with's token works" grep -q "Example Domain" "$OUT/with.txt"
    "$AI2" vault token list > "$OUT/tokens.json"
    expect_ok "with revoked its token" test "$(grep -o '"id":"tok_' "$OUT/tokens.json" | wc -l)" = 1
    "$AI2" vault audit > "$OUT/audit.json"
    echo "audit: $(cat "$OUT/audit.json")"
    expect_ok "audit has the allowed call" grep -q 'example.com' "$OUT/audit.json"
    expect_ok "audit has the denied call" grep -q 'approval_denied' "$OUT/audit.json"
    expect_ok "audit has no secret" sh -c "! grep -q not-a-real-secret $OUT/audit.json"
    expect_ok "audit has no token" sh -c "! grep -qF '$token' $OUT/audit.json"
    expect_ok "eve may not open admin.sock" sh -c "sudo -u eve curl -s --max-time 10 --unix-socket /run/ai2-vault/admin.sock http://v/status; test \$? = 7"
    expect_ok "eve may not open the box socket" sh -c "sudo -u eve curl -s --max-time 10 --unix-socket $sock http://v/; test \$? = 7"
    expect_ok "eve's ai2 says it may not reach the daemon" sh -c "cd /tmp && sudo -u eve /usr/local/libexec/ai2-vault/ai2 vault status | grep -q 'may not reach'"
    expect_ok "root on admin.sock is 403 wrong_user" sh -c "sudo curl -sS --max-time 10 --unix-socket /run/ai2-vault/admin.sock http://v/status | grep -q wrong_user"
    expect_ok "root on the box socket is 403 wrong_user" sh -c "sudo curl -sS --max-time 10 --unix-socket $sock -H 'Authorization: Bearer $token' http://v/example/example.com/ | grep -q wrong_user"
    sudo usermod -aG ai2-vault-admin eve
    expect_ok "eve in the admin group is 403 wrong_user" sh -c "sudo -u eve curl -sS --max-time 10 --unix-socket /run/ai2-vault/admin.sock http://v/status | grep -q wrong_user"
    sudo gpasswd -d eve ai2-vault-admin >/dev/null
    expect_ok "eve may not read the passphrase hash" sh -c "! sudo -u eve cat /var/lib/ai2-vault/passphrase 2>/dev/null"
    sudo systemctl restart ai2-vault.service
    sleep 2
    expect_ok "daemon survives a restart and reopens box" sh -c "curl -sS --max-time 60 --unix-socket $sock -H 'Authorization: Bearer $token' http://v/example/example.com/ | grep -q 'Example Domain'"
    echo "daemon log:"
    sudo journalctl -u ai2-vault.service --no-pager -o cat | tail -20
}

case "${1:-}" in
    build) build ;;
    setup) setup; hardening ;;
    vault) vault ;;
esac
echo "failures: $failed"
exit $((failed > 0))
