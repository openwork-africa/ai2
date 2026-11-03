(() => {
  "use strict";

  const nonce = new URLSearchParams(location.search).get("nonce") || "";
  const root = document.getElementById("vault") || (() => {
    const made = document.createElement("section");
    made.id = "vault";
    (document.querySelector("main") || document.body).append(made);
    return made;
  })();
  const tab = document.getElementById("tab-vault");
  if (tab) tab.checked = true;

  const h = (tag, props, ...kids) => {
    const node = document.createElement(tag);
    for (const kid of kids.flat(Infinity)) {
      if (kid === undefined || kid === null || kid === false) continue;
      node.append(kid instanceof Node ? kid : String(kid));
    }
    for (const [key, value] of Object.entries(props || {})) {
      if (value === undefined || value === null || value === false) continue;
      if (key.startsWith("on")) node.addEventListener(key.slice(2), value);
      else if (key === "class") node.className = value;
      else if (key === "value") node.value = value;
      else node.setAttribute(key, value === true ? "" : value);
    }
    return node;
  };

  const state = { view: "keys", platform: "", refresh: null, waiting: 0 };
  const notice = h("p", { class: "notice", role: "status", "aria-live": "polite" });
  const statusLine = h("p", { class: "status" }, "Connecting to the vault daemon.");
  const nav = h("nav", { class: "sub", "aria-label": "Vault" });
  const view = h("section", { class: "view" });

  const isMac = () => /mac|darwin/i.test(state.platform || navigator.userAgent);
  const say = (text, tone) => {
    notice.textContent = text || "";
    notice.className = "notice" + (tone ? " " + tone : "");
  };
  const cell = (value) => {
    if (value instanceof Node) return value;
    if (value === undefined || value === null || value === "") return "—";
    if (Array.isArray(value)) return value.length ? value.map((v) => (typeof v === "object" ? JSON.stringify(v) : String(v))).join(", ") : "—";
    if (typeof value === "object") return JSON.stringify(value);
    return String(value);
  };
  const when = (value) => {
    if (value === undefined || value === null || value === "") return "—";
    const date = typeof value === "number" ? new Date(value < 1e12 ? value * 1000 : value) : new Date(value);
    return isNaN(date) ? String(value) : date.toLocaleString();
  };
  const urlLines = (text) => text.split(/\s+/).map((s) => s.trim()).filter(Boolean);
  const words = (text) => text.split(/[\s,]+/).map((s) => s.trim()).filter(Boolean);
  const part = (text) => encodeURIComponent(text);

  async function parse(res) {
    const text = await res.text();
    if (!text) return {};
    try {
      return JSON.parse(text);
    } catch {
      return { error: "bad_response", message: text.slice(0, 300) };
    }
  }

  function send(method, path, body, passphrase) {
    const headers = { "X-Vault-UI-Nonce": nonce, Accept: "application/json" };
    if (body !== undefined) headers["Content-Type"] = "application/json";
    if (passphrase) headers["X-Vault-Passphrase"] = passphrase;
    return fetch("/api" + path, {
      method,
      headers,
      body: body === undefined ? undefined : JSON.stringify(body),
      cache: "no-store",
      credentials: "same-origin",
    });
  }

  function failure(data, res) {
    const error = new Error(data.message || data.reason || data.error || res.status + " " + res.statusText);
    error.code = data.error;
    error.status = res.status;
    return error;
  }

  async function api(method, path, body, action) {
    const write = method !== "GET";
    const touch = write && isMac();
    if (touch) {
      state.waiting += 1;
      say("Confirm with Touch ID on this Mac.", "wait");
    }
    let res;
    let data;
    try {
      res = await send(method, path, body);
      data = await parse(res);
    } finally {
      if (touch) {
        state.waiting -= 1;
        if (!state.waiting) say("");
      }
    }
    if (res.status === 401 && data.error === "presence_required" && !isMac()) {
      const answer = await ask(
        "Passphrase",
        "The vault needs your passphrase to " + (action || "do this") + ". It is sent once, for this action only.",
        [{ name: "passphrase", label: "Vault passphrase", type: "password", autocomplete: "current-password" }],
        "Confirm",
      );
      if (!answer) throw new Error("Cancelled: nothing was changed.");
      res = await send(method, path, body, answer.passphrase);
      answer.passphrase = "";
      data = await parse(res);
    }
    if (!res.ok) throw failure(data, res);
    return data;
  }

  function ask(title, text, fields, submit) {
    return new Promise((resolve) => {
      let settled = false;
      const inputs = fields.map((f) =>
        h("input", { name: f.name, type: f.type || "text", autocomplete: f.autocomplete || "off", required: true, spellcheck: "false" }),
      );
      const dialog = h("dialog", { class: "vault-ask" });
      const finish = (answer) => {
        if (settled) return;
        settled = true;
        for (const input of inputs) input.value = "";
        if (dialog.open) dialog.close();
        dialog.remove();
        resolve(answer);
      };
      const form = h(
        "form",
        {
          onsubmit: (event) => {
            event.preventDefault();
            const answer = {};
            fields.forEach((f, i) => (answer[f.name] = inputs[i].value));
            finish(answer);
          },
        },
        h("h2", {}, title),
        h("p", {}, text),
        fields.map((f, i) => h("label", {}, f.label, inputs[i])),
        h(
          "p",
          { class: "actions" },
          h("button", { type: "button", onclick: () => finish(null) }, "Cancel"),
          h("button", { type: "submit", class: "primary" }, submit),
        ),
      );
      dialog.append(form);
      dialog.addEventListener("close", () => finish(null));
      document.body.append(dialog);
      dialog.showModal();
      inputs[0].focus();
    });
  }

  async function act(work, success) {
    try {
      const out = await work();
      if (state.refresh) await state.refresh();
      if (success) say(success, "good");
      return out;
    } catch (error) {
      say(error.message, "poor");
      return undefined;
    }
  }

  async function load(refresh) {
    try {
      await refresh();
    } catch (error) {
      say(error.message, "poor");
    }
  }

  function table(rows, columns, empty) {
    if (!rows.length) return h("p", { class: "missing" }, empty || "None.");
    return h(
      "table",
      {},
      h("thead", {}, h("tr", {}, columns.map(([label]) => h("th", {}, label)))),
      h(
        "tbody",
        {},
        rows.map((row) => h("tr", {}, columns.map(([label, get]) => h("td", { "data-label": label }, cell(get(row)))))),
      ),
    );
  }

  const field = (label, control, wide) => h("label", { class: wide ? "wide" : undefined }, label, control);
  const button = (text, onclick, tone) => h("button", { type: "button", class: tone, onclick }, text);

  function keysView(out) {
    const box = h("section");
    const name = h("input", { required: true, autocomplete: "off", spellcheck: "false", pattern: "[A-Za-z0-9_.\\-]+", placeholder: "stripe" });
    const description = h("textarea", { rows: 2, placeholder: "What the key is for. The vetting model reads this." });
    const urls = h("textarea", { rows: 3, spellcheck: "false", placeholder: "https://api.stripe.com/v1/\nOne URL prefix per line" });
    const inject = h(
      "select",
      {},
      h("option", { value: "bearer" }, "Bearer: Authorization: Bearer {secret}"),
      h("option", { value: "basic" }, "Basic: secret as user:pass"),
      h("option", { value: "query" }, "Query parameter"),
      h("option", { value: "header" }, "Header"),
      h("option", { value: "template" }, "Header template with {secret}"),
    );
    const injectArg = h("input", { autocomplete: "off", spellcheck: "false", disabled: true, placeholder: "Not needed for this preset" });
    const hints = { query: "Parameter name, e.g. api_key", header: "Header name, e.g. X-Api-Key", template: "X-Api-Key: {secret}" };
    inject.addEventListener("change", () => {
      const hint = hints[inject.value];
      injectArg.disabled = !hint;
      injectArg.required = Boolean(hint);
      injectArg.placeholder = hint || "Not needed for this preset";
      if (!hint) injectArg.value = "";
    });
    const secret = h("input", { type: "password", required: true, autocomplete: "new-password", spellcheck: "false" });
    const injectValue = () => {
      const arg = injectArg.value.trim();
      if (inject.value === "query") return "query:" + arg;
      if (inject.value === "header") return "header:" + arg;
      if (inject.value === "template") return arg;
      return inject.value;
    };
    const propose = button("Propose URLs", async () => {
      if (!name.value.trim() || !description.value.trim()) {
        say("Give the key a name and a description first.", "poor");
        return;
      }
      try {
        const data = await api("POST", "/keys/" + part(name.value.trim()) + "/propose", { description: description.value.trim() }, "propose URLs for " + name.value.trim());
        const found = data.approved_urls;
        urls.value = found.join("\n");
        say(found.length ? "Check the proposed URLs before you add the key." : "The vetting model proposed no URLs.", found.length ? "good" : "poor");
      } catch (error) {
        say(error.message, "poor");
      }
    });
    const form = h(
      "form",
      {
        autocomplete: "off",
        onsubmit: async (event) => {
          event.preventDefault();
          const key = name.value.trim();
          const approved = urlLines(urls.value);
          if (!approved.length) {
            say("Add at least one approved URL, or press Propose URLs and check what comes back.", "poor");
            return;
          }
          const body = { description: description.value.trim(), approved_urls: approved, inject: injectValue(), secret: secret.value };
          const done = await act(() => api("PUT", "/keys/" + part(key), body, "add the key " + key), "Added " + key + ".");
          body.secret = "";
          if (done !== undefined) {
            form.reset();
            inject.dispatchEvent(new Event("change"));
          }
        },
      },
      field("Name", name),
      field("Inject", inject),
      field("Inject value", injectArg),
      field("Description", description, true),
      field("Approved URLs", urls, true),
      field("Secret", secret, true),
      h("p", { class: "actions wide" }, h("button", { type: "submit", class: "primary" }, "Add key"), propose, h("small", {}, "The secret goes to the vault daemon and is never shown again.")),
    );
    out.append(h("h2", {}, "Keys"), box, h("h2", {}, "Add a key"), form);
    return async () => {
      const data = await api("GET", "/keys");
      box.replaceChildren(
        table(
          data.keys,
          [
            ["Name", (k) => k.name],
            ["Description", (k) => k.description],
            ["Approved URLs", (k) => k.approved_urls],
            ["Inject", (k) => k.inject],
            ["Created", (k) => when(k.created)],
            ["Rotated", (k) => when(k.rotated)],
            [
              "",
              (k) =>
                h(
                  "span",
                  {},
                  button("Rotate", async () => {
                    const answer = await ask("Rotate " + k.name, "The new secret replaces the old one and is never shown again.", [{ name: "secret", label: "New secret", type: "password", autocomplete: "new-password" }], "Rotate");
                    if (!answer) return;
                    await act(() => api("POST", "/keys/" + part(k.name) + "/rotate", { secret: answer.secret }, "rotate " + k.name), "Rotated " + k.name + ".");
                    answer.secret = "";
                  }),
                  button(
                    "Delete",
                    async () => {
                      if (!confirm("Delete the key " + k.name + "? Apps and agents that use it stop working.")) return;
                      await act(() => api("DELETE", "/keys/" + part(k.name), undefined, "delete " + k.name), "Deleted " + k.name + ".");
                    },
                    "danger",
                  ),
                ),
            ],
          ],
          "No keys yet.",
        ),
      );
    };
  }

  function tokensView(out) {
    const box = h("section");
    const shown = h("section");
    const keys = h("span", { class: "choices" }, h("small", {}, "Loading keys."));
    const sandbox = h("select", { required: true });
    const mode = h("select", {}, h("option", { value: "app" }, "app: preapproved calls"), h("option", { value: "agent" }, "agent: every call is vetted"));
    const methods = h("select", {}, h("option", { value: "read" }, "read: GET, HEAD, OPTIONS"), h("option", { value: "write" }, "write: POST, PUT, PATCH, DELETE"), h("option", { value: "all" }, "all"), h("option", { value: "custom" }, "Only these methods"));
    const methodList = h("input", { autocomplete: "off", spellcheck: "false", disabled: true, placeholder: "GET,POST" });
    methods.addEventListener("change", () => {
      methodList.disabled = methods.value !== "custom";
      methodList.required = methods.value === "custom";
    });
    const prefixes = h("textarea", { rows: 2, spellcheck: "false", placeholder: "Optional. One URL prefix per line; the keys' approved URLs when empty" });
    const intent = h("textarea", { rows: 2, placeholder: "Agent tokens: what the run is for, written from the user's own request" });
    const ttl = h("input", { autocomplete: "off", placeholder: "8h for agent, 30d for app" });
    const uses = h("input", { type: "number", min: "1", step: "1", placeholder: "Unlimited" });
    const warn = h("small", { class: "fair" });
    const sandboxWarning = () => {
      warn.textContent = sandbox.value === "host" && isMac() ? "Not safe: an app on this Mac outside a container can read anything you can." : "";
    };
    sandbox.addEventListener("change", sandboxWarning);
    mode.addEventListener("change", () => (intent.required = mode.value === "agent"));

    const reveal = (data) => {
      const token = data.token;
      if (!token) {
        shown.replaceChildren();
        return;
      }
      const input = h("input", { readonly: true, value: token, spellcheck: "false", "aria-label": "New token" });
      const notes = data.warnings || [];
      shown.replaceChildren(
        h(
          "section",
          { class: "once" },
          h("h2", {}, "New token" + (data.id ? " " + data.id : "")),
          h("p", {}, "Copy it now. It is shown once; the vault keeps only its hash."),
          input,
          notes.map((n) => h("p", { class: "fair" }, n)),
          h(
            "p",
            { class: "actions" },
            button(
              "Copy",
              async () => {
                try {
                  await navigator.clipboard.writeText(token);
                } catch {
                  input.select();
                  document.execCommand("copy");
                }
                say("Copied the token.", "good");
              },
              "primary",
            ),
            button("Done", () => shown.replaceChildren()),
          ),
        ),
      );
    };

    const form = h(
      "form",
      {
        autocomplete: "off",
        onsubmit: async (event) => {
          event.preventDefault();
          const chosen = [...keys.querySelectorAll("input:checked")].map((i) => i.value);
          if (!chosen.length) {
            say("Choose at least one key.", "poor");
            return;
          }
          const body = { keys: chosen, sandbox: sandbox.value, mode: mode.value, methods: methods.value === "custom" ? words(methodList.value.toUpperCase()).join(",") : methods.value };
          const urlPrefixes = urlLines(prefixes.value);
          if (urlPrefixes.length) body.url_prefixes = urlPrefixes;
          if (intent.value.trim()) body.intent = intent.value.trim();
          if (ttl.value.trim()) body.ttl = ttl.value.trim();
          if (uses.value) body.uses = Number(uses.value);
          if (body.mode === "agent" && !body.intent) {
            say("An agent token needs an intent.", "poor");
            return;
          }
          shown.replaceChildren();
          const data = await act(() => api("POST", "/tokens", body, "create a token for " + chosen.join(", ")), "Token created.");
          if (data) {
            reveal(data);
            form.reset();
            methods.dispatchEvent(new Event("change"));
            sandboxWarning();
          }
        },
      },
      field("Keys", keys, true),
      field("Sandbox", sandbox),
      field("Mode", mode),
      field("Methods", methods),
      field("Methods list", methodList),
      field("URL prefixes", prefixes, true),
      field("Intent", intent, true),
      field("Time to live", ttl),
      field("Uses", uses),
      h("p", { class: "actions wide" }, h("button", { type: "submit", class: "primary" }, "Create token"), warn),
    );
    out.append(shown, h("h2", {}, "Tokens"), box, h("h2", {}, "Create a token"), form);

    const choices = async () => {
      const [keyData, sandboxData] = await Promise.all([api("GET", "/keys"), api("GET", "/sandboxes")]);
      const names = keyData.keys.map((k) => k.name);
      keys.replaceChildren(...(names.length ? names.map((n) => h("label", {}, h("input", { type: "checkbox", value: n }), n)) : [h("small", {}, "Add a key first.")]));
      const boxes = sandboxData.sandboxes.map((s) => s.name);
      const previous = sandbox.value;
      sandbox.replaceChildren(...boxes.map((n) => h("option", { value: n }, n)));
      if (boxes.includes(previous)) sandbox.value = previous;
      sandboxWarning();
    };
    load(choices);
    return async () => {
      const data = await api("GET", "/tokens");
      box.replaceChildren(
        table(
          data.tokens,
          [
            ["Id", (t) => t.id],
            ["Keys", (t) => t.keys],
            ["Sandbox", (t) => t.sandbox],
            ["Mode", (t) => t.mode],
            ["Methods", (t) => t.methods],
            ["URL prefixes", (t) => t.url_prefixes],
            ["Intent", (t) => t.intent],
            ["Expires", (t) => when(t.expires)],
            ["Uses left", (t) => t.uses_left],
            [
              "",
              (t) =>
                button(
                  "Revoke",
                  async () => {
                    if (!confirm("Revoke the token " + t.id + "? Calls made with it are refused from now on.")) return;
                    await act(() => api("DELETE", "/tokens/" + part(t.id), undefined, "revoke the token " + t.id), "Revoked " + t.id + ".");
                  },
                  "danger",
                ),
            ],
          ],
          "No tokens.",
        ),
      );
    };
  }

  function sandboxesView(out) {
    const box = h("section");
    const name = h("input", { required: true, autocomplete: "off", spellcheck: "false", pattern: "[A-Za-z0-9_.\\-]+", placeholder: "agent-1" });
    const form = h(
      "form",
      {
        autocomplete: "off",
        onsubmit: async (event) => {
          event.preventDefault();
          const value = name.value.trim();
          const done = await act(() => api("POST", "/sandboxes", { name: value }, "add the sandbox " + value), "Added " + value + ".");
          if (done !== undefined) form.reset();
        },
      },
      field("Name", name),
      h("p", { class: "actions" }, h("button", { type: "submit", class: "primary" }, "Add sandbox"), h("small", {}, "host is for apps that run on this machine itself.")),
    );
    out.append(h("h2", {}, "Sandboxes"), box, h("h2", {}, "Add a sandbox"), form);
    return async () => {
      const data = await api("GET", "/sandboxes");
      box.replaceChildren(
        table(
          data.sandboxes,
          [
            ["Name", (s) => s.name],
            ["Endpoint", (s) => (s.transport === "unix" ? s.path : s.listen + " from " + s.peers.join(", "))],
            [
              "",
              (s) =>
                button(
                  "Remove",
                  async () => {
                    if (!confirm("Remove the sandbox " + s.name + "? Its endpoint stops answering.")) return;
                    await act(() => api("DELETE", "/sandboxes/" + part(s.name), undefined, "remove the sandbox " + s.name), "Removed " + s.name + ".");
                  },
                  "danger",
                ),
            ],
          ],
          "No sandboxes.",
        ),
      );
    };
  }

  function approvalsView(out) {
    const box = h("section");
    const decide = (a, approve) =>
      act(() => api("POST", "/approvals/" + part(a.id), { approve }, (approve ? "approve" : "deny") + " this call"), (approve ? "Approved " : "Denied ") + a.id + ".");
    out.append(h("h2", {}, "Waiting for you"), h("p", { class: "missing" }, "Write calls on agent tokens wait here. Undecided after 120 seconds they are denied."), box);
    return async () => {
      const data = await api("GET", "/approvals");
      const rows = data.approvals;
      count(rows.length);
      box.replaceChildren(
        table(
          rows,
          [
            ["Id", (a) => a.id],
            ["Key", (a) => a.key],
            ["Method", (a) => a.method],
            ["URL", (a) => a.url],
            ["Request", (a) => a.request],
            ["Intent", (a) => a.intent],
            ["Sandbox", (a) => a.sandbox],
            ["Asked", (a) => when(a.created)],
            ["", (a) => h("span", {}, button("Approve", () => decide(a, true), "primary"), button("Deny", () => decide(a, false), "danger"))],
          ],
          "Nothing is waiting.",
        ),
      );
    };
  }

  function auditView(out) {
    const box = h("section");
    const tail = h("select", { "aria-label": "How many entries" }, [50, 200, 1000].map((n) => h("option", { value: String(n) }, "Last " + n)));
    tail.value = "200";
    let refresh;
    tail.addEventListener("change", () => load(refresh));
    out.append(h("h2", {}, "Audit log"), h("p", { class: "actions" }, tail, button("Refresh", () => load(refresh))), h("p"), box);
    refresh = async () => {
      const data = await api("GET", "/audit?tail=" + tail.value);
      const rows = data.entries.slice().reverse();
      box.replaceChildren(
        table(
          rows,
          [
            ["Time", (e) => when(e.time)],
            ["Token", (e) => e.token],
            ["Sandbox", (e) => e.sandbox],
            ["Key", (e) => e.key],
            ["Method", (e) => e.method],
            ["URL", (e) => e.url],
            [
              "Decision",
              (e) => {
                const decision = e.decision;
                const tone = /allow|approv|ok/i.test(decision || "") ? "good" : /den|refus|block/i.test(decision || "") ? "poor" : undefined;
                return h("span", { class: tone }, cell(decision));
              },
            ],
            ["Status", (e) => e.status],
            ["Reason", (e) => e.reason],
          ],
          "The audit log is empty.",
        ),
      );
    };
    return refresh;
  }

  const views = {
    keys: ["Keys", keysView],
    tokens: ["Tokens", tokensView],
    sandboxes: ["Sandboxes", sandboxesView],
    approvals: ["Approvals", approvalsView],
    audit: ["Audit", auditView],
  };
  const badge = h("span", { class: "count", hidden: true });
  const tabs = Object.entries(views).map(([id, [label]]) =>
    h("button", { type: "button", "data-view": id, onclick: () => show(id) }, label, id === "approvals" ? badge : null),
  );
  nav.append(...tabs);

  function count(n) {
    badge.textContent = String(n);
    badge.hidden = !n;
  }

  async function show(id) {
    state.view = id;
    for (const t of tabs) {
      if (t.dataset.view === id) t.setAttribute("aria-current", "page");
      else t.removeAttribute("aria-current");
    }
    say("");
    view.replaceChildren();
    const refresh = views[id][1](view);
    state.refresh = refresh;
    await load(refresh);
  }

  async function poll() {
    if (document.hidden || !nonce) return;
    try {
      if (state.view === "approvals" && state.refresh) await state.refresh();
      else count((await api("GET", "/approvals")).approvals.length);
    } catch {
      return;
    }
  }

  async function start() {
    root.replaceChildren(h("header", {}, h("h1", {}, "Vault"), statusLine), notice, nav, view);
    if (!nonce) {
      statusLine.textContent = "";
      say("Open the link ai2 vault ui printed: this page needs its nonce.", "poor");
      return;
    }
    try {
      const status = await api("GET", "/status");
      state.platform = status.platform;
      statusLine.textContent = Object.entries(status)
        .map(([key, value]) => key.replace(/_/g, " ") + ": " + cell(value))
        .join(" · ");
    } catch (error) {
      statusLine.textContent = error.message;
      statusLine.className = "status poor";
    }
    await show(state.view);
    setInterval(poll, 2000);
  }

  start();
})();
