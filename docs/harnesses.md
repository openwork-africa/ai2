# Harnesses

`ai2 harness` installs coding agents into `~/.local/bin`, signs in to them and prints the headless command line for a task, each as JSON:

| Command | |
| --- | --- |
| `ai2 harness list` | every harness, whether it is installed and signed in, and its models |
| `ai2 harness install <id>` | installs it |
| `ai2 harness sign-in <id>` | starts its sign-in and prints what to do |
| `ai2 harness command <id> [--model m] [--personality p] [--prompt text] [--folder dir]` | the line that runs a task with it headless |

The same code is the `harness` crate in `crates/harness`, for use as a library (`harness = { git = "https://github.com/openwork-africa/ai2" }`); `harness::Registry` takes more harnesses.
