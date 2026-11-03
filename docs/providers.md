# Providers file

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
The vault uses this file too, for the model that checks agent calls; without it, calls that need checking are refused.
