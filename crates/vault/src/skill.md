# Using the ai2 vault

You have a vault token, not the secrets. The vault gateway puts the real secret into each call and sends it on. You never see a secret, and you do not need to.

## What you are given

- `AI2_VAULT_TOKEN`: a token starting with `vt_`. It works only from the sandbox it was made for, for the keys it lists, until it expires.
- `AI2_VAULT_ENDPOINT`: the gateway, `unix:/path/to.sock` or `http://host:port`.

A human gives you these by running you under `ai2 vault with <keys> -- <your command>`. If they are not set, ask the human to start you that way. Do not look for secrets in files or the environment instead.

## Calling a service

Send the call to the gateway instead of the service:

```
<METHOD> <endpoint>/<key>/<host>/<path>?<query>
Authorization: Bearer $AI2_VAULT_TOKEN
X-Vault-Request: <what this call is for, percent-encoded>
```

It goes on to `https://<host>/<path>?<query>`. Method, other headers and body are passed as they are; the response streams back.

```sh
curl --unix-socket /run/ai2-vault.sock \
  -H "Authorization: Bearer $AI2_VAULT_TOKEN" \
  -H "X-Vault-Request: list%20charges%20for%20the%20monthly%20report" \
  http://vault/stripe/api.stripe.com/v1/charges
```

For an `http://` endpoint, drop `--unix-socket` and use the endpoint as the URL base.

An SDK that takes a base URL and an API key needs nothing else: base URL `<endpoint>/<key>/<host>/<base path>`, API key `$AI2_VAULT_TOKEN`. For example `http://127.0.0.1:7777/openai/api.openai.com/v1`.

## X-Vault-Request

With an agent token, every call says in a few plain words what it is for and why, percent-encoded UTF-8. The vault checks it against the task the human gave the token. Be specific and truthful: "refund order 1042 the user asked for", not "api call". Not percent-encoded gets `400`. App tokens do not need it.

## When a call is refused

A refusal is `403` with `{ "error": "denied", "reason": "<code>", "intent": "<the task the token is for>" }`. Never retry a denial in a loop and never try to get around it. Stop, and tell the human the reason and the call.

- `token_unknown`, `token_expired`, `token_used_up`: the token is no good. Ask the human for a new one.
- `wrong_sandbox`, `wrong_user`: you are calling from somewhere the token was not made for.
- `key_not_allowed`: the token does not list that key.
- `method_not_allowed`: the token does not allow that method (often: read only).
- `url_not_allowed`, `vetting_denied`: that URL is outside what the key or the task allows. Stay inside the task in `intent`.
- `vetting_unavailable`: the vault could not check the call. Say so; do not retry in a loop.
- `approval_denied`: the human refused this call. Do not ask again for the same thing.
- `approval_timeout`: the human did not approve it in time. It was not approved. Tell them what you wanted to do and let them decide.

Write calls on an agent token wait for a human to approve them; that can take up to two minutes.

## ai2 vault exit codes

`0` ok, `1` error, `2` denied (presence or approval refused), `64` usage (wrong arguments). `ai2 vault with` exits with your command's code.

## Never

- Print, log, echo or write `AI2_VAULT_TOKEN` anywhere, or put it in a URL, a commit or a message.
- Ask for, look for or try to read a secret.
- Retry a refused call in a loop.
