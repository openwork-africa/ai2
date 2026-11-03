#!/bin/sh
# Seeds an AI2_HOME with demo apps and reviews, for screenshots.
#
#   tests/demo/seed.sh start <dir>   registers four apps, copies two reviews,
#                                    and starts two of the apps with ai2
#   tests/demo/seed.sh stop <dir>    stops those two apps
#
# AI2 names the ai2 binary to use (default: ai2 on PATH).
# Then: AI2_HOME=<dir> cargo run -p ui --example serve -- --port 0 --no-open
# Run stop before you delete <dir>: the apps outlive the ui server.
set -eu

what=${1:?start or stop}
home=${2:?the AI2_HOME folder}
ai2=${AI2:-ai2}
here=$(cd "$(dirname "$0")" && pwd)
root=$(cd "$here/../.." && pwd)
demo="$home/demo"

export AI2_HOME="$home"

stop() {
  for id in notes weather; do
    "$ai2" apps stop "$id" >/dev/null 2>&1 || true
  done
}

if [ "$what" = stop ]; then
  stop
  exit 0
fi
[ "$what" = start ] || { echo "seed.sh: start or stop" >&2; exit 1; }

mkdir -p "$demo"

# app <id> <name> <port> <colour> <description> <svg body>
app() {
  id=$1 name=$2 port=$3 colour=$4 description=$5 glyph=$6
  dir="$demo/$id"
  mkdir -p "$dir"
  cat > "$dir/icon.svg" <<EOF
<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 32 32"><rect width="32" height="32" rx="8" fill="$colour"/>$glyph</svg>
EOF
  cat > "$dir/index.html" <<EOF
<!doctype html><title>$name</title><h1>$name</h1><p>$description</p>
EOF
  cat > "$dir/openapi.json" <<EOF
{
  "openapi": "3.1.0",
  "info": { "title": "$name", "version": "1.0.0", "description": "$description" },
  "paths": {
    "/index.html": { "get": { "summary": "The home page", "responses": { "200": { "description": "The page" } } } },
    "/openapi.json": { "get": { "summary": "This document", "responses": { "200": { "description": "The document" } } } }
  }
}
EOF
  "$ai2" apps register \
    --name "$name" \
    --description "$description" \
    --command "python3 -m http.server $port --bind 127.0.0.1" \
    --cwd "$dir" \
    --port "$port" \
    --openapi /openapi.json \
    --icon "$dir/icon.svg" \
    --no-port-env >/dev/null
}

app notes "Notes" 47811 "#F5B700" \
  "Plain notes you can search, with a small JSON API." \
  '<rect x="9" y="7" width="14" height="18" rx="2" fill="#fff"/><path d="M12 12h8M12 16h8M12 20h5" stroke="#F5B700" stroke-width="2" stroke-linecap="round"/>'
app weather "Weather" 47812 "#3B82F6" \
  "Today's forecast for your town, kept for an hour." \
  '<circle cx="13" cy="13" r="5" fill="#FDE68A"/><path d="M11 24a5 5 0 0 1 1-9.9A6 6 0 0 1 23.5 16 4 4 0 0 1 23 24z" fill="#fff"/>'
app invoices "Invoices" 47813 "#10B981" \
  "Make, send and track invoices for your clients." \
  '<path d="M10 6h12v20l-3-2-3 2-3-2-3 2z" fill="#fff"/><path d="M13 12h6M13 16h6" stroke="#10B981" stroke-width="2" stroke-linecap="round"/>'
app recipes "Recipes" 47814 "#EF4444" \
  "Your recipes, scaled for any number of plates." \
  '<path d="M7 15h18a9 9 0 0 1-18 0z" fill="#fff"/><path d="M12 7c-1 1.5 1 2.5 0 4M16 6c-1 1.5 1 2.5 0 4M20 7c-1 1.5 1 2.5 0 4" stroke="#fff" stroke-width="1.6" fill="none" stroke-linecap="round"/>'

# Two reviews from the UI's test fixtures, with demo folders.
review() {
  fixture=$1 folder=$2 objective=$3
  python3 - "$root/crates/ui/src/reviews/fixtures/$fixture.json" "$home/reviews" "$folder" "$objective" <<'EOF'
import json, os, sys
src, out, folder, objective = sys.argv[1:]
review = json.load(open(src))
review["folder"] = folder
review["objective"] = objective
h = 0xcbf29ce484222325
for b in folder.encode():
    h = ((h ^ b) * 0x100000001b3) & 0xFFFFFFFFFFFFFFFF
dir = os.path.join(out, f"{h:016x}")
os.makedirs(dir, exist_ok=True)
json.dump(review, open(os.path.join(dir, "review.json"), "w"), indent=2)
EOF
}
review workspot "/Users/you/code/invoices" "Add a pricing page with three plans and a Stripe checkout button"
review harness "/Users/you/code/recipes" "Let people save a recipe and share it with a link"

# Start two apps; the others stay stopped.
for id in notes weather; do
  "$ai2" apps start "$id" >/dev/null
done
echo "seeded $home"
