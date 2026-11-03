# Agents

An agent is a harness, a model, a personality (its system prompt) and a default folder, kept by name in `$AI2_HOME/agents`. A run starts the harness headless in a folder, keeps its record, output and events in `$AI2_HOME/runs/<id>`, then reviews the folder as `ai2 --run-path <folder> --objective <prompt> --json` does and keeps the review's hash on the record. Each command prints JSON; `ai2 run` prints one event per line, ending with `done`:

| Command | |
| --- | --- |
| `ai2 agents create <name> --harness <id> [--model m] [--personality p] [--folder dir]` | keeps an agent; a personality under 280 characters is expanded into a full system prompt with the providers file's model (`--enhancer <provider>/<model-id>` picks another) |
| `ai2 agents list` / `show <name>` / `remove <name>` | every agent, one, or forgets one |
| `ai2 agents enhance <name>` | expands its short personality now |
| `ai2 run --agent <name> \| --harness <id> [--model m] [--personality p] [--folder dir] "<prompt>"` | runs it, then reviews the folder |

The same pieces are the MCP tools `agents_list`, `agents_create`, `agent_run`, `runs_list` and `runs_get`, and the `ai2 ui` routes under `/api/agents` and `/api/runs` (with `/api/runs/{id}/events` as server-sent events). The code is the `agents` crate in `crates/agents`. A private build keeps agents elsewhere by implementing `agents::AgentStore` (list, get, put, remove) and passing it to `ai2::agents::with_store(build, store, registry)`, which adds ai2's enhancer and review launcher (`agents::Agents::new(store, registry, ai2_home)` gives the bare pieces); `AgentsSubcommand`, `RunSubcommand`, the MCP `Pieces::agents` and `ui::Options::agents` take that `Agents`.
