#!/bin/bash
set -u

repo=$(cd "$(dirname "$0")/../.." && pwd)
uid=$(id -u)
run_id="$$"
label="africa.openwork.ai2-vault-e2e.$run_id"
service="ai2-vault"
real_label="africa.openwork.ai2-vault"
login_kc="$HOME/Library/Keychains/login.keychain-db"
plist="$HOME/Library/LaunchAgents/$label.plist"
container_name="ai2e2e-$run_id"
kc_pass="e2e-$run_id-$(openssl rand -hex 8)"
secret="e2e-secret-$(openssl rand -hex 16)"

T=$(mktemp -d /tmp/ai2e.XXXXXX)
kc="$T/v.keychain-db"
bin="$T/bin/ai2"
mkdir -p "$T/bin" "$T/stub" "$T/h"
export AI2_HOME="$T/h"
export AI2_VAULT_KEYCHAIN="$kc"
export AI2_VAULT_LABEL="$label"

passed=0
failed=0
started_pids=""
search_list_file="$T/search-list"

say() { printf '%s\n' "$*" >&2; }
pass() { passed=$((passed + 1)); say "PASS $1"; }
fail() { failed=$((failed + 1)); say "FAIL $1"; }
check() {
	local name="$1"
	shift
	if "$@"; then pass "$name"; else fail "$name"; fi
}
not() { ! "$@"; }
must() {
	if [ "$failed" -ne 0 ]; then
		say "stopping after $1 failed, so no more prompts are shown"
		exit 1
	fi
}
prompt() { say ""; say ">>> PROMPT: Touch ID / password will ask: \"$1\""; }
login_item() { security find-generic-password -s "$service" "$login_kc" 2>/dev/null | grep -E '"cdat"|"acct"|"svce"' || true; }
restore_search_list() {
	[ -s "$search_list_file" ] || return 0
	local now
	now=$(security list-keychains -d user)
	if [ "$now" != "$(cat "$search_list_file.raw")" ]; then
		local list=()
		while IFS= read -r line; do list+=("$line"); done <"$search_list_file"
		security list-keychains -d user -s "${list[@]}"
		say "restored the user keychain search list"
	fi
}

cleanup() {
	say ""
	say "== cleanup"
	for pid in $started_pids; do kill "$pid" 2>/dev/null; done
	if launchctl print "gui/$uid/$label" >/dev/null 2>&1; then
		launchctl bootout "gui/$uid/$label" 2>/dev/null
	fi
	if launchctl print "gui/$uid/$label" >/dev/null 2>&1; then say "CLEANUP FAIL: $label still loaded"; else say "cleanup: $label not loaded"; fi
	rm -f "$plist"
	if [ -e "$plist" ]; then say "CLEANUP FAIL: $plist still there"; else say "cleanup: $plist removed"; fi
	if container list --all 2>/dev/null | awk '{print $1}' | grep -qx "$container_name"; then
		container rm -f "$container_name" >/dev/null 2>&1
	fi
	if container list --all 2>/dev/null | awk '{print $1}' | grep -qx "$container_name"; then say "CLEANUP FAIL: container $container_name"; else say "cleanup: no container $container_name"; fi
	if [ -e "$kc" ]; then security delete-keychain "$kc" 2>/dev/null; fi
	if [ -e "$kc" ]; then say "CLEANUP FAIL: $kc still there"; else say "cleanup: test keychain deleted"; fi
	restore_search_list
	if security list-keychains -d user | grep -q "$T"; then say "CLEANUP FAIL: test keychain in search list"; else say "cleanup: search list has no test keychain"; fi
	if [ "$(login_item)" = "$login_before" ]; then say "cleanup: login keychain items for $service unchanged"; else say "CLEANUP FAIL: login keychain items for $service changed"; fi
	if pgrep -f "$bin vault serve" >/dev/null; then say "CLEANUP FAIL: daemon still running"; else say "cleanup: no daemon from $bin"; fi
	rm -rf "$T"
	if [ -e "$T" ]; then say "CLEANUP FAIL: $T still there"; else say "cleanup: $T removed"; fi
	say ""
	say "passed $passed, failed $failed"
}

login_before=$(login_item)
real_before=$(launchctl print "gui/$uid/$real_label" >/dev/null 2>&1 && echo loaded || echo absent)
trap cleanup EXIT
trap 'exit 130' INT TERM

say "== build"
(cd "$repo" && cargo build --quiet --bin ai2) || exit 1
target=$(cd "$repo" && cargo metadata --format-version 1 --no-deps | python3 -c 'import json,sys;print(json.load(sys.stdin)["target_directory"])')
cp "$target/debug/ai2" "$bin"
say "binary $bin, AI2_HOME $AI2_HOME, label $label, keychain $kc"

say "== test keychain"
security list-keychains -d user >"$search_list_file.raw"
sed -e 's/^ *"//' -e 's/"$//' "$search_list_file.raw" >"$search_list_file"
security create-keychain -p "$kc_pass" "$kc" || exit 1
security set-keychain-settings "$kc"
security unlock-keychain -p "$kc_pass" "$kc" || exit 1
restore_search_list

j() { python3 -c 'import json,sys
d=json.load(sys.stdin)
for k in sys.argv[1].split("."):
    d=d[int(k)] if isinstance(d,list) else d.get(k) if isinstance(d,dict) else None
print("" if d is None else (json.dumps(d) if isinstance(d,(dict,list,bool)) else d))' "$1"; }

say "== setup --check"
"$bin" setup --check >"$T/check1.json"
say "$(cat "$T/check1.json")"
check "check: containers ready" test "$(j steps.containers.status <"$T/check1.json")" = ready
check "check: master key missing" test "$(j steps.master-key.status <"$T/check1.json")" = missing
check "check: daemon missing" test "$(j steps.daemon.status <"$T/check1.json")" = missing
if [ "$(j steps.containers.status <"$T/check1.json")" != ready ]; then
	say "containers is not ready, so ai2 setup would install or start Apple container; stopping"
	exit 1
fi

say "== setup"
"$bin" setup >"$T/setup.json"
say "$(cat "$T/setup.json")"
check "setup: ok" test "$(j ok <"$T/setup.json")" = true
for step in containers master-key presence daemon; do
	check "setup: $step ready" test "$(j "steps.$step.status" <"$T/setup.json")" = ready
done

"$bin" setup --check >"$T/check2.json"
say "$(cat "$T/check2.json")"
check "check after setup: ok" test "$(j ok <"$T/check2.json")" = true

check "master key in the test keychain" security find-generic-password -s "$service" -a master "$kc" >/dev/null
check "login keychain items for $service unchanged" test "$(login_item)" = "$login_before"
check "plist written under the test label" grep -q "<string>$label</string>" "$plist"
check "plist names the test keychain" grep -q "<string>$kc</string>" "$plist"
launchctl print "gui/$uid/$label" >"$T/launchctl.txt" 2>&1
check "LaunchAgent running" grep -q "state = running" "$T/launchctl.txt"
check "LaunchAgent runs <bin> vault serve" grep -q "program = $bin" "$T/launchctl.txt"
check "daemon process is $bin vault serve" pgrep -f "$bin vault serve"

for _ in $(seq 1 50); do [ -S "$AI2_HOME/vault/admin.sock" ] && break; sleep 0.2; done
check "admin socket up" test -S "$AI2_HOME/vault/admin.sock"

say "== vault"
"$bin" vault status >"$T/status.json"
say "$(cat "$T/status.json")"
check "status: master key loaded" test "$(j master_key <"$T/status.json")" = true

box_socket="$AI2_HOME/vault/sandboxes/box/gateway.sock"
say ""
say "The next four prompts come one after another:"
say "  1. Store the vault key example, to be sent only to https://example.com/ as bearer"
say "  2. Add the vault sandbox box at the socket $box_socket"
say "  3. Make an app token for the keys example in the sandbox box: methods GET, HEAD, OPTIONS, URLs the keys' approved URLs, for 30d"
say "  4. Make an app token for the keys example in the sandbox box: methods GET, HEAD, OPTIONS, URLs the keys' approved URLs, for 30d (for ai2 vault with)"

prompt "Store the vault key example, to be sent only to https://example.com/ as bearer"
printf '%s' "$secret" | "$bin" vault put --name example --approved-url https://example.com/ --secret - >"$T/put.json"
put_rc=$?
say "$(cat "$T/put.json")"
check "put" test "$put_rc" -eq 0
must "put"

"$bin" vault list >"$T/list.json"
check "list names example" grep -q '"example"' "$T/list.json"
check "list holds no secret" not grep -q "$secret" "$T/list.json"
"$bin" vault has --name example >"$T/has.json"
check "has example" test "$(j exists <"$T/has.json")" = true
"$bin" vault has --name nothing >/dev/null
check "has nothing exits 1" test $? -eq 1

prompt "Add the vault sandbox box at the socket $box_socket"
"$bin" vault sandbox add --name box >"$T/box.json"
say "$(cat "$T/box.json")"
check "sandbox add box" test -S "$box_socket"
must "sandbox add"

prompt "Make an app token for the keys example in the sandbox box: methods GET, HEAD, OPTIONS, URLs the keys' approved URLs, for 30d"
"$bin" vault token create --keys example --sandbox box --mode app >"$T/token.json"
token=$(j token <"$T/token.json")
check "token create" test -n "$token" -a "${token#vt_}" != "$token"
must "token create"

"$bin" vault with example --mode app -- sh -c 'echo $AI2_VAULT_ENDPOINT' >"$T/with-host.out" 2>"$T/with-host.err"
with_host=$?
say "with (default sandbox host): exit $with_host: $(cat "$T/with-host.err")"
check "with without a host sandbox says so, with no prompt" grep -q "no sandbox host" "$T/with-host.err"

prompt "Make an app token for the keys example in the sandbox box: methods GET, HEAD, OPTIONS, URLs the keys' approved URLs, for 30d"
"$bin" vault with example --mode app --sandbox box -- sh -c 'echo $AI2_VAULT_ENDPOINT' >"$T/with.out" 2>"$T/with.err"
say "with: $(cat "$T/with.out") / $(cat "$T/with.err")"
check "with prints the endpoint" test "$(cat "$T/with.out")" = "unix:$box_socket"
check "with revokes its token" grep -q "revoked when the command exits" "$T/with.err"
say "No more prompts after this point."

say "== gateway"
curl -s -i --max-time 60 --unix-socket "$box_socket" -H "Authorization: Bearer $token" http://v/example/example.com/ >"$T/call.txt"
say "$(head -1 "$T/call.txt")"
check "call: 200" grep -q "^HTTP/1.1 200" "$T/call.txt"
check "call: example.com's page" grep -q "Example Domain" "$T/call.txt"
check "call: no secret in the response" not grep -q "$secret" "$T/call.txt"

curl -s -i --unix-socket "$box_socket" -H "Authorization: Bearer vt_nope" http://v/example/example.com/ >"$T/bad.txt"
check "unknown token: 403 token_unknown" grep -q "token_unknown" "$T/bad.txt"

say "outside the approved prefix: waits for an approval, then denied after 120s"
curl -s -i --max-time 200 --unix-socket "$box_socket" -H "Authorization: Bearer $token" http://v/example/www.iana.org/ >"$T/outside.txt" &
outside_pid=$!
started_pids="$started_pids $outside_pid"
waiting=""
for _ in $(seq 1 50); do
	"$bin" vault approvals >"$T/approvals.json"
	waiting=$(j approvals.0.line <"$T/approvals.json" 2>/dev/null)
	[ -n "$waiting" ] && break
	sleep 0.2
done
say "waiting: $waiting"
check "outside call waits in the approval queue" test -n "$waiting"

say "== container sandbox (while the outside call waits)"
(cd "$repo" && AI2_E2E_SOCKET="$box_socket" AI2_E2E_TOKEN="$token" AI2_E2E_PATH=/example/example.com/ AI2_E2E_CONTAINER="$container_name" \
	cargo test --quiet -p vault -- --ignored --exact sandbox::tests::a_vault_gateway_answers_from_inside_an_apple_container --nocapture) >"$T/container.txt" 2>&1
container_rc=$?
say "$(grep -E '^HTTP/|test result' "$T/container.txt")"
check "container: gateway call passes" test $container_rc -eq 0
check "container: 200 from inside" grep -q "^HTTP/1.1 200" "$T/container.txt"
check "container: example.com's page" grep -q "Example Domain" "$T/container.txt"
check "container: no secret" not grep -q "$secret" "$T/container.txt"

wait "$outside_pid"
say "$(head -1 "$T/outside.txt") $(tail -1 "$T/outside.txt")"
check "outside call: 403 approval_timeout" grep -q "approval_timeout" "$T/outside.txt"

say "== audit"
"$bin" vault audit >"$T/audit.json"
check "audit has the allowed call" grep -q '"allow"' "$T/audit.json"
check "audit has the approval timeout" grep -q "approval_timeout" "$T/audit.json"
check "no secret or token anywhere in the state" not grep -rqF -D skip -e "$secret" -e "$token" "$AI2_HOME"

say "== ui"
printf '#!/bin/sh\nexit 0\n' >"$T/stub/open"
chmod +x "$T/stub/open"
PATH="$T/stub:$PATH" AI2_UI_PORT=0 "$bin" vault ui >"$T/ui.json" 2>"$T/ui.err" &
ui_pid=$!
started_pids="$started_pids $ui_pid"
for _ in $(seq 1 50); do [ -s "$T/ui.json" ] && break; sleep 0.1; done
url=$(j url <"$T/ui.json")
origin=$(printf '%s' "$url" | sed -E 's|^(http://[^/]+).*|\1|')
nonce=${url##*nonce=}
check "ui opens on the vault page" test "${url%%\?nonce=*}" = "$origin/vault"
check "ui page with the nonce: 200" test "$(curl -s -o "$T/ui.html" -w '%{http_code}' "$url")" = 200
check "ui page is html" grep -qi "<html" "$T/ui.html"
check "ui page with a wrong nonce: 403" test "$(curl -s -o /dev/null -w '%{http_code}' "$origin/?nonce=wrong")" = 403
check "ui api with the nonce: 200" test "$(curl -s -o "$T/ui-keys.json" -w '%{http_code}' -H "X-Ai2-UI-Nonce: $nonce" "$origin/api/vault/keys")" = 200
check "ui api lists example" grep -q '"example"' "$T/ui-keys.json"
check "ui api without the nonce: 403" test "$(curl -s -o /dev/null -w '%{http_code}' "$origin/api/vault/keys")" = 403
kill "$ui_pid" 2>/dev/null
wait "$ui_pid" 2>/dev/null

real_after=$(launchctl print "gui/$uid/$real_label" >/dev/null 2>&1 && echo loaded || echo absent)
check "real LaunchAgent $real_label unchanged ($real_before)" test "$real_after" = "$real_before"

[ "$failed" -eq 0 ]
