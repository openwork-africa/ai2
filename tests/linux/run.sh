#!/bin/sh
set -eu

HERE=$(cd "$(dirname "$0")" && pwd)
REPO=$(cd "$HERE/../.." && pwd)
NAME=${NAME:-ai2-linux-test}
IMAGE=${IMAGE:-ai2-linux-test:dev}
KEEP=${KEEP:-}

remove() {
    container stop "$NAME" >/dev/null 2>&1 || true
    container rm "$NAME" >/dev/null 2>&1 || true
}

cleanup() {
    if [ -z "$KEEP" ]; then
        remove
        container image delete "$IMAGE" >/dev/null 2>&1 || true
    fi
}

if ! container image inspect "$IMAGE" >/dev/null 2>&1; then
    container build --progress plain -t "$IMAGE" -f "$HERE/Containerfile" "$HERE"
fi
remove
trap cleanup EXIT INT TERM
container run -d --name "$NAME" --cap-add ALL --cpus 6 --memory 6g \
    --volume "$REPO:/src" "$IMAGE" >/dev/null

booted=""
for _ in $(seq 1 60); do
    state=$(container exec "$NAME" systemctl is-system-running 2>/dev/null || true)
    case "$state" in
        running | degraded) booted=yes; break ;;
    esac
    sleep 1
done
if [ -z "$booted" ]; then
    echo "systemd did not boot in the container"
    container logs "$NAME" | tail -40
    exit 1
fi
echo "systemd is PID 1: $(container exec "$NAME" cat /proc/1/comm), $state"

status=0
for phase in build setup vault; do
    echo "== $phase"
    container exec -u dev "$NAME" /src/tests/linux/inside.sh "$phase" || status=1
done
exit $status
