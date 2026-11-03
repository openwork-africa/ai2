# Building ai2 with extra steps

ai2 is a library as well as a binary. A step implements `ai2::Step`; a crate that depends on ai2 builds one binary with the built-in steps and its own, with the same flags and outputs. This is the one git dependency a private build needs: `ai2::harness` re-exports the `harness` crate, so its tag always matches ai2's.

```toml
[dependencies]
ai2 = { git = "https://github.com/openwork-africa/ai2", tag = "v0.1.1" }
anyhow = "1"
clap = "4"
```

A step has a namespaced snake_case `id` (like `rating.msr`); `run_with` refuses two steps with the same id. Its lifecycle is `provision` (read its own flags and resolve what it needs, or fail and its card is `missing` before anything runs), then `rate`, then `cleanup` (always called, even after an error). Flags from `args` are prefixed with the step id, so two steps cannot collide on a flag name; the long flag name is unchanged. `after` names the steps that must run first. `model.ask` takes a system prompt that must answer with one JSON object (it retries once with a larger token budget) and returns that object as any `Deserialize` type.

```rust
use ai2::steps::{Scale, Scored};
use ai2::{Context, Found, Rated, Step};
use std::collections::BTreeMap;

const MINE: Scale = Scale {
    key: "mine",
    name: "My Rating",
    short: "MY",
    riskier: false,
    missing: "Needs --mine.",
    factors: &[("first", "First"), ("second", "Second"), ("third", "Third")],
};

struct Mine { flag: Option<String> }

impl Step for Mine {
    fn id(&self) -> &'static str { "rating.mine" }
    fn scale(&self) -> &'static Scale { &MINE }
    fn args(&self) -> Vec<clap::Arg> {
        vec![clap::Arg::new("mine").long("mine").value_name("VALUE")]
    }
    fn provision(&mut self, ctx: &Context) -> anyhow::Result<()> {
        self.flag = ctx.arg::<String>(self.id(), "mine").cloned();
        Ok(())
    }
    fn rate(&self, _ctx: &Context, _found: &Found) -> anyhow::Result<Rated> {
        if self.flag.is_none() { return Ok(Rated::default()); }
        let scored: BTreeMap<String, Scored> = BTreeMap::new();
        Ok(Rated { rating: Some(MINE.rate(&scored)), ..Rated::default() })
    }
}

fn main() -> std::process::ExitCode {
    let mut steps = ai2::builtin();
    steps.push(Box::new(Mine { flag: None }));
    ai2::run(steps)
}
```

`rate` is given the `Context` (folder, objective, changed files, days, spec, model and the parsed flags) and the `Found` accumulator (the exploits and findings of the steps before it, in `after` order). It returns a `Rated`: the rating, or none with a note saying why, and any exploits, findings and items it adds. A step that returns an error shows its card with the error.

**Adding a harness.** `ai2::harness::Registry` takes more harnesses: `Registry::put(setup, before)` inserts a `Setup` (or replaces the one with the same id), `on_installed(hook)` runs a hook after an install, and `list::agent`/`headless`/`flag`/`pi` build the pieces of a `Setup`. Pass that registry to `HarnessSubcommand::new(registry, state_dir)`. To make the RRF use a reviewer, construct it with one: `ai2::builtin_with(vec![ai2::harness::muse_reviewer(), ai2::harness::pi_reviewer()])`, and a private build can add its own `Reviewer { id, label, available, command }`; `--harness <id>` then accepts it.

**Adding a subcommand.** Implement `ai2::Subcommand` (`command()` returns its `clap::Command`, `run(matches)` returns the exit code) and pass it to `run_with`. ai2 prints one `{"error": ...}` JSON line and exits 1 when a subcommand fails; library code never calls `process::exit`.

**Building your own binary.** Pass your own identity: `ai2::run_with(steps, subcommands, ai2::Build { name: "mine", version: env!("CARGO_PKG_VERSION") })`. The build name and version drive `--version`, `review.json`'s `version`, the stderr prefix, the default output folder, the default providers path (`~/.<name>/providers.json`) and the providers env var (`<NAME>_PROVIDERS`). Ship releases as `<name>-<target>.tar.gz` plus a `SHA256SUMS` file in the same release, with the binary, `LICENSE`, `NOTICE` and `README.md` in the tarball.
