const { invoke } = window.__TAURI__.core;

const $ = (sel) => document.querySelector(sel);

/**
 * Inputs: text. Outputs: clipboard write promise.
 */
async function copy(text) {
  await navigator.clipboard.writeText(text);
}

/**
 * Inputs: status and token. Outputs: updated endpoint UI.
 */
function renderEndpoint(status, token) {
  const running = !!status.running;
  $("#status-pill").textContent = running ? "running" : "stopped";
  $("#status-pill").className = `pill ${running ? "on" : "off"}`;
  $("#btn-toggle").textContent = running ? "Stop" : "Start";
  $("#endpoint-url").value = status.endpoint;
  $("#endpoint-token").value = token;
  $("#client-snippet").textContent = JSON.stringify(
    {
      mcpServers: {
        funnelit: {
          url: status.endpoint,
          headers: { Authorization: `Bearer ${token}` },
        },
      },
    },
    null,
    2,
  );
}

/**
 * Inputs: server list. Outputs: rebuilt MCP list DOM.
 */
function renderServers(servers) {
  const list = $("#server-list");
  list.replaceChildren();
  if (!servers.length) {
    const empty = document.createElement("li");
    empty.textContent = "No MCPs yet. Add a stdio command or paste an MCP URL.";
    list.append(empty);
    return;
  }
  for (const server of servers) {
    const li = document.createElement("li");
    const info = document.createElement("div");
    const title = document.createElement("strong");
    title.textContent = server.name;
    const meta = document.createElement("div");
    meta.className = "meta";
    const kind = server.transport.kind;
    const detail =
      kind === "stdio"
        ? `${server.transport.command} ${(server.transport.args || []).join(" ")}`
        : server.transport.url;
    meta.textContent = `${kind} · ${server.enabled ? "enabled" : "disabled"} · ${detail}`;
    info.append(title, meta);

    const actions = document.createElement("div");
    actions.className = "actions";
    const testBtn = document.createElement("button");
    testBtn.type = "button";
    testBtn.textContent = "Test";
    testBtn.addEventListener("click", async () => {
      try {
        alert(await invoke("test_server", { id: server.id }));
      } catch (e) {
        alert(String(e));
      }
    });
    const editBtn = document.createElement("button");
    editBtn.type = "button";
    editBtn.textContent = "Edit";
    editBtn.addEventListener("click", () => openEditor(server));
    const delBtn = document.createElement("button");
    delBtn.type = "button";
    delBtn.className = "danger";
    delBtn.textContent = "Delete";
    delBtn.addEventListener("click", async () => {
      if (!confirm(`Delete ${server.name}?`)) return;
      await invoke("remove_server", { id: server.id });
      await refresh();
    });
    actions.append(testBtn, editBtn, delBtn);
    li.append(info, actions);
    list.append(li);
  }
}

/**
 * Inputs: KEY=value text. Outputs: object map.
 */
function parsePairs(text) {
  const out = {};
  for (const line of text.split("\n")) {
    const trimmed = line.trim();
    if (!trimmed) continue;
    const i = trimmed.indexOf("=");
    if (i <= 0) continue;
    out[trimmed.slice(0, i).trim()] = trimmed.slice(i + 1);
  }
  return out;
}

/**
 * Inputs: none. Outputs: transport + secrets from the editor form.
 */
function readDraft() {
  const kind = $("#edit-kind").value;
  if (kind === "stdio") {
    const env = parsePairs($("#edit-env").value);
    return {
      transport: {
        kind: "stdio",
        command: $("#edit-command").value.trim(),
        args: $("#edit-args")
          .value.split("\n")
          .map((s) => s.trim())
          .filter(Boolean),
        env_keys: Object.keys(env),
      },
      secrets: { env, headers: {}, bearer: null },
    };
  }
  const headers = parsePairs($("#edit-headers").value);
  const bearer = $("#edit-bearer").value.trim();
  return {
    transport: {
      kind: "http",
      url: $("#edit-url").value.trim(),
      header_keys: Object.keys(headers),
      has_bearer: !!bearer,
    },
    secrets: { env: {}, headers, bearer: bearer || null },
  };
}

/**
 * Inputs: optional server. Outputs: opened editor dialog.
 */
function openEditor(server) {
  $("#editor-title").textContent = server ? "Edit MCP" : "Add MCP";
  $("#edit-id").value = server?.id || "";
  $("#edit-name").value = server?.name || "";
  $("#edit-enabled").checked = server?.enabled ?? true;
  $("#editor-msg").textContent = "";
  $("#editor-msg").className = "msg";
  const kind = server?.transport?.kind || "stdio";
  $("#edit-kind").value = kind;
  toggleKind();
  if (kind === "stdio") {
    $("#edit-command").value = server?.transport?.command || "";
    $("#edit-args").value = (server?.transport?.args || []).join("\n");
    $("#edit-env").value = (server?.transport?.env_keys || [])
      .map((k) => `${k}=`)
      .join("\n");
  } else {
    $("#edit-url").value = server?.transport?.url || "";
    $("#edit-bearer").value = "";
    $("#edit-headers").value = (server?.transport?.header_keys || [])
      .map((k) => `${k}=`)
      .join("\n");
  }
  $("#editor").showModal();
}

/**
 * Inputs: none. Outputs: toggled transport fields.
 */
function toggleKind() {
  const http = $("#edit-kind").value === "http";
  $("#stdio-fields").classList.toggle("hidden", http);
  $("#http-fields").classList.toggle("hidden", !http);
}

/**
 * Inputs: none. Outputs: refreshed status, token, and server list.
 */
async function refresh() {
  const [status, token, servers] = await Promise.all([
    invoke("get_status"),
    invoke("get_token"),
    invoke("list_servers"),
  ]);
  renderEndpoint(status, token);
  renderServers(servers);
}

window.addEventListener("DOMContentLoaded", () => {
  $("#edit-kind").addEventListener("change", toggleKind);
  $("#btn-add").addEventListener("click", () => openEditor(null));
  $("#btn-copy-url").addEventListener("click", () => copy($("#endpoint-url").value));
  $("#btn-copy-token").addEventListener("click", () => copy($("#endpoint-token").value));
  $("#btn-show-token").addEventListener("click", () => {
    const input = $("#endpoint-token");
    const hide = input.type === "text";
    input.type = hide ? "password" : "text";
    $("#btn-show-token").textContent = hide ? "Show" : "Hide";
  });
  $("#btn-rotate-token").addEventListener("click", async () => {
    await invoke("rotate_token");
    await refresh();
  });
  $("#btn-toggle").addEventListener("click", async () => {
    const status = await invoke("get_status");
    if (status.running) await invoke("stop_funnel");
    else await invoke("start_funnel");
    await refresh();
  });
  $("#btn-test-draft").addEventListener("click", async () => {
    const msg = $("#editor-msg");
    msg.className = "msg";
    msg.textContent = "Testing…";
    try {
      const draft = readDraft();
      const result = await invoke("test_draft", {
        name: $("#edit-name").value || "draft",
        transport: draft.transport,
        secrets: draft.secrets,
      });
      msg.textContent = result;
      msg.className = "msg ok";
    } catch (e) {
      msg.textContent = String(e);
      msg.className = "msg err";
    }
  });
  $("#editor-form").addEventListener("submit", async (e) => {
    if (e.submitter?.value !== "save") return;
    e.preventDefault();
    const draft = readDraft();
    await invoke("upsert_server", {
      id: $("#edit-id").value || null,
      name: $("#edit-name").value.trim(),
      enabled: $("#edit-enabled").checked,
      transport: draft.transport,
      secrets: draft.secrets,
    });
    $("#editor").close();
    await refresh();
  });
  refresh().catch((e) => {
    console.error(e);
    alert(String(e));
  });
});
