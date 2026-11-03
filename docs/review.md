# The review

Each rating is a percentage, the mean of three factors from 0 to 100.

- **Modern Security Rating (MSR)**: how exposed the folder's stack is to exploits published in recent days (OSV.dev and CISA KEV).
- **Run Risk Factor (RRF)**: what a coding harness you pick with `--harness` (Claude Code, Codex, pi and Muse Code are built in) finds in the files the run changed, with the four security skills in `skills/`. Each finding is fixed where a small, safe change does it.

The MSR asks the model in your providers file to match advisories to your stack, so the list of your stack goes to that model (and, without syft, the manifests it reads). The RRF harness reads the changed files with its own model.

```sh
ai2 --run-path=./site --objective "Add a pricing page" --changed index.html,style.css --providers ~/.ai2/providers.json
```

| Flag | Default | |
| --- | --- | --- |
| `--run-path <dir>` | `.` | the folder the run worked in |
| `--objective <text>` | | what you asked the agent to do |
| `--changed <a,b>` | the whole folder | the files the run changed, relative to the run path |
| `--providers <file>` | `$AI2_PROVIDERS`, else `~/.ai2/providers.json` | the models file (below) |
| `--model <provider/id>` | the file's default model | the model the steps ask |
| `--harness auto\|<id>\|none` | `auto` | the reviewer for the RRF: `auto` takes the first one ready, `none` skips it |
| `--days <n>` | `7` | how many days back an exploit counts as recent |
| `--out <dir>` | `$AI2_HOME/reviews/<folder-hash>` | where `review.json` and `report.html` go; outside the reviewed folder |
| `--budget <minutes>` | `35` | the whole review's time; `review.json` is written after every step |
| `--json` | | print `review.json` to stdout |
| `--disable-dashboard` | | write `review.json` alone, without `report.html` |

Without `--json`, ai2 prints the path of `report.html`, or of `review.json` with `--disable-dashboard`. Progress goes to stderr.
[syft](https://github.com/anchore/syft) on `PATH` lists the libraries; without it the model reads the manifests.
