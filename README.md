# ai2

ai2 rates the security of a folder a coding agent worked in:

- **Modern Security Rating (MSR)**: how exposed the folder's technology is to exploits published in recent days (OSV.dev and CISA KEV).
- **Run Risk Factor (RRF)**: what a coding harness (Muse Code or pi) finds in the files the run changed, with the vendored security skills; each finding is resolved where a small safe change does it.

Each rating is a percentage, the mean of three factors from 0 to 100.

## Install

```sh
curl -fsSL https://raw.githubusercontent.com/openwork-africa/ai2/main/install.sh | sh
```

It installs the release for your OS and CPU (Linux x86_64 and aarch64, macOS arm64 and x86_64) into `~/.local/bin`, after checking its SHA-256.

| Variable | Default | |
| --- | --- | --- |
| `AI2_VERSION` | the latest release | a release tag, e.g. `v0.1.0` |
| `AI2_INSTALL_DIR` | `~/.local/bin` | where `ai2` goes |

From source: `cargo install --git https://github.com/openwork-africa/ai2`.

## Usage

```sh
ai2 --run-path=./site --objective "Add a pricing page" --changed index.html,style.css --providers ~/.ai2/providers.json
```

| Flag | Default | |
| --- | --- | --- |
| `--run-path <dir>` | `.` | the folder the run worked in |
| `--objective <text>` | | what the run was asked to do |
| `--changed <a,b>` | the whole folder | the files the run changed, relative to the run path |
| `--providers <file>` | `$AI2_PROVIDERS`, else `~/.ai2/providers.json` | the models file (below) |
| `--model <provider/id>` | the file's default model | the model the steps ask |
| `--harness auto\|muse\|pi\|none` | `auto` | the coding harness for the RRF: `auto` takes Muse Code when `muse usage` succeeds, else pi |
| `--days <n>` | `7` | how many days back an exploit counts as recent |
| `--out <dir>` | `<run-path>/.ai2` | where `review.json` and `report.html` go |
| `--json` | | print `review.json` to stdout |
| `--disable-dashboard` | | write `review.json` alone, without `report.html` |

Without `--json`, ai2 prints the path of `report.html`, or of `review.json` with `--disable-dashboard`. Progress goes to stderr.

[syft](https://github.com/anchore/syft) on `PATH` lists the libraries; without it the model reads the manifests.

## Harnesses

`ai2 harness` installs coding agents into `~/.local/bin`, signs in to them and prints the headless command line for a task, each as JSON:

| Command | |
| --- | --- |
| `ai2 harness list` | every harness, whether it is installed and signed in, and its models |
| `ai2 harness install <id>` | installs it |
| `ai2 harness sign-in <id>` | starts its sign-in and prints what to do |
| `ai2 harness command <id> [--model m] [--personality p] [--prompt text] [--folder dir]` | the line that runs a task with it headless |

State is kept in `$AI2_HOME`, else `~/.ai2`. The same code is the `harness` crate in `crates/harness`, for use as a library (`harness = { git = "https://github.com/openwork-africa/ai2" }`); `harness::Registry` takes more harnesses.

## Providers file

The same shape as dm's `providers.json` and pi's `models.json`. `api` is `openai-completions` or `anthropic-messages`. A value that starts with `!` is run in a shell and its output used; one that starts with `$` is read from the environment.

```json
{
  "providers": {
    "nebius": {
      "api": "openai-completions",
      "baseUrl": "https://api.tokenfactory.nebius.com/v1",
      "apiKey": "!cat ~/.config/nebius/key",
      "models": [{ "id": "Qwen/Qwen3-235B-A22B-Instruct-2507", "default": true, "maxTokens": 8192 }]
    }
  }
}
```

For pi, ai2 passes the same file as its `models.json` in a temporary `PI_CODING_AGENT_DIR`.

## Outputs

`report.html` is a standalone page: one card per rating with its factors, then the technology and its share of the risk, recent exploits, findings, per-item scores and notes. It follows the system's light or dark theme.

`review.json`:

| Field | |
| --- | --- |
| `version`, `folder`, `objective`, `generated_at`, `harness` | the run |
| `ratings` | one entry per step, in order: `key`, `name`, `short`, `riskier` (true when a higher score is more risk), `rating` (`score` and `factors`, each `name`, `score`, `why`) or null with `missing` saying why, and `items` when the step scores items one by one (`index`, `text`, `score`, `factors`) |
| `technologies` | `name`, `kind` (`language`, `library`, `os`), `risk`: its percent of the risk |
| `exploits` | `id`, `technology`, `severity`, `summary`, `fixed_in` |
| `findings` | `title`, `technology`, `file`, `severity`, `likelihood`, `impact`, `confidence`, `fix`, `status` (`fixed` or `open`) |
| `spec` | `os`, `languages` (`name`, `lines`, `share`), `libraries` (`name`, `version`, `ecosystem`) |
| `notes` | what could not be done |

## Building ai2 with extra steps

ai2 is a library as well as a binary. A step implements `ai2::Step`; a crate that depends on ai2 builds one binary with the built-in steps and its own, with the same flags and outputs.

```toml
[dependencies]
ai2 = { git = "https://github.com/openwork-africa/ai2", tag = "v0.1.0" }
anyhow = "1"
clap = "4"
```

```rust
use ai2::steps::{Scale, Scored};
use ai2::{Rated, Run, Step};
use std::collections::BTreeMap;

const MINE: Scale = Scale {
    key: "mine",
    name: "My Rating",
    short: "MY",
    riskier: false,
    missing: "Needs --mine.",
    factors: &[("first", "First"), ("second", "Second"), ("third", "Third")],
};

struct Mine;

impl Step for Mine {
    fn scale(&self) -> &'static Scale {
        &MINE
    }

    fn args(&self) -> Vec<clap::Arg> {
        vec![clap::Arg::new("mine").long("mine")]
    }

    fn rate(&self, run: &Run) -> anyhow::Result<Rated> {
        let Some(_mine) = run.args.get_one::<String>("mine") else {
            return Ok(Rated::default());
        };
        let scored: BTreeMap<String, Scored> = BTreeMap::new();
        Ok(Rated { rating: Some(MINE.rate(&scored)), ..Rated::default() })
    }
}

fn main() -> anyhow::Result<()> {
    let mut steps = ai2::builtin();
    steps.push(Box::new(Mine));
    ai2::run(steps)
}
```

A step is given the `Run`: the folder, objective, changed files, providers file and model (`run.model.ask` returns the model's JSON answer as any `Deserialize` type), the harness choice, the spec, the exploits and findings of the steps before it, and the parsed flags (`run.args`). It returns a `Rated`: the rating, or none with a note saying why, and any exploits, findings and items it adds. A step that returns an error shows its card with the error.

`args` adds the step's own flags to the command line. Steps run in the order given.

`ai2::run_with(steps, commands, handler)` also takes subcommands (`clap::Command`s); `handler` is given the name and matches of the one that was used. `ai2::cli::harness_command()` and `ai2::cli::run_harness` are the `harness` subcommand.

## License

Apache-2.0 (`LICENSE`). The skills in `skills/` are under the MIT License, each with its own `LICENSE`; `NOTICE` lists them and their sources.
