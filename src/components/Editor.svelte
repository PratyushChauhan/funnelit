<!--
  @component
  Inputs: open flag, optional server, saved callback.
  Outputs: MCP editor dialog with OAuth probe/sign-in.
-->
<script>
  import { tick } from "svelte";
  import * as api from "$lib/api.js";
  import {
    chatgptOauthGuideUrl,
    isHttpSource,
    readDraft,
  } from "$lib/draft.js";
  import { Button } from "$lib/components/ui/button/index.js";
  import { Input } from "$lib/components/ui/input/index.js";
  import { Textarea } from "$lib/components/ui/textarea/index.js";
  import { Label } from "$lib/components/ui/label/index.js";
  import { Switch } from "$lib/components/ui/switch/index.js";
  import * as Dialog from "$lib/components/ui/dialog/index.js";

  let { open = $bindable(false), server = null, onSaved } = $props();

  let editId = $state("");
  let name = $state("");
  let enabled = $state(true);
  let source = $state("");
  let argsText = $state("");
  let envText = $state("");
  let bearer = $state("");
  let headersText = $state("");
  let oauthClientId = $state("");
  let oauthClientSecret = $state("");
  let advancedOpen = $state(false);
  let msg = $state("");
  let msgTone = $state("");
  let oauthProbe = $state(null);
  let oauthConnected = $state(false);
  let probing = $state(false);
  let probeTimer = null;
  let probeGen = 0;

  const http = $derived(isHttpSource(source));
  const empty = $derived(!source.trim());
  const showAdvanced = $derived(http || empty);
  const kindHint = $derived(
    empty
      ? "Paste a URL or a local command"
      : http
        ? "Detected HTTP MCP"
        : "Detected stdio command",
  );
  const showOauth = $derived(probing || !!oauthProbe?.oauth);
  const showGuide = $derived(
    !!oauthProbe?.oauth &&
      !oauthProbe.supports_dcr &&
      !oauthConnected &&
      !oauthProbe.has_saved_client,
  );
  const oauthStatus = $derived(
    probing
      ? "Checking sign-in…"
      : !oauthProbe?.oauth
        ? ""
        : oauthConnected
          ? "Connected"
          : oauthProbe.supports_dcr
            ? "Sign in registers automatically"
            : "Add Client ID under Advanced, then Sign in",
  );
  const oauthBtnLabel = $derived(
    oauthConnected ? "Reconnect" : oauthProbe?.label || "Sign in",
  );

  // Reset only on open edge so edits are not wiped while the dialog stays open.
  let wasOpen = false;
  $effect(() => {
    if (open && !wasOpen) resetFromServer(server);
    wasOpen = open;
  });

  $effect(() => {
    return () => clearTimeout(probeTimer);
  });

  /**
   * Inputs: optional server. Outputs: form fields reset for add/edit.
   */
  function resetFromServer(s) {
    editId = s?.id || "";
    name = s?.name || "";
    enabled = s?.enabled ?? true;
    source = "";
    argsText = "";
    envText = "";
    bearer = "";
    headersText = "";
    oauthClientId = "";
    oauthClientSecret = "";
    advancedOpen = false;
    msg = "";
    msgTone = "";
    probing = false;
    oauthConnected = !!(
      s?.transport?.kind === "http" && s.transport.has_bearer
    );
    oauthProbe = null;
    if (s?.transport?.kind === "http") {
      source = s.transport.url || "";
      headersText = (s.transport.header_keys || [])
        .map((k) => `${k}=`)
        .join("\n");
    } else if (s) {
      source = s.transport?.command || "";
      argsText = (s.transport?.args || []).join("\n");
      envText = (s.transport?.env_keys || []).map((k) => `${k}=`).join("\n");
    }
    queueProbe();
  }

  /**
   * Inputs: none. Outputs: edit id, creating one when missing.
   */
  function ensureEditId() {
    if (!editId) editId = crypto.randomUUID();
    return editId;
  }

  /**
   * Inputs: none. Outputs: debounced OAuth probe for HTTP sources.
   */
  function queueProbe() {
    clearTimeout(probeTimer);
    const gen = ++probeGen;
    if (!isHttpSource(source)) {
      probing = false;
      oauthProbe = null;
      oauthConnected = false;
      return;
    }
    probing = true;
    probeTimer = setTimeout(() => {
      probeAuth(gen).catch(console.error);
    }, 250);
  }

  /**
   * Inputs: probe generation. Outputs: probe state for the current draft/URL.
   */
  async function probeAuth(gen) {
    const url = source.trim();
    const id = editId || null;
    if (!isHttpSource(url)) {
      if (gen !== probeGen) return;
      probing = false;
      oauthProbe = null;
      oauthConnected = false;
      return;
    }
    probing = true;
    let next;
    let failed = false;
    try {
      next = await api.probeMcpAuth({ url, id });
    } catch {
      failed = true;
      next = null;
    }
    if (gen !== probeGen) return;
    probing = false;
    if (failed) {
      oauthProbe = null;
      return;
    }
    oauthProbe = next;
    oauthConnected = !!next.connected;
    if (next.supports_dcr) advancedOpen = false;
  }

  /**
   * Inputs: source input. Outputs: cleared oauthConnected + probe queue.
   */
  function onSourceInput() {
    oauthConnected = false;
    queueProbe();
  }

  /**
   * Inputs: paste event. Outputs: probe after value settles.
   */
  function onSourcePaste() {
    oauthConnected = false;
    queueMicrotask(queueProbe);
  }

  /**
   * Inputs: none. Outputs: browser OAuth flow result.
   */
  async function onOauth() {
    const url = source.trim();
    msg = "Waiting for browser sign-in…";
    msgTone = "";
    try {
      const id = ensureEditId();
      await api.startMcpOauth({
        url,
        id,
        clientId: oauthClientId.trim() || null,
        clientSecret: oauthClientSecret.trim() || null,
      });
      oauthConnected = true;
      oauthProbe = {
        ...(oauthProbe || { oauth: true }),
        oauth: true,
        needs_client_id: false,
        connected: true,
      };
      msg = "Signed in";
      msgTone = "ok";
    } catch (e) {
      const text = String(e);
      msg = text;
      msgTone = "err";
      if (/client id|advanced|registration/i.test(text)) {
        advancedOpen = true;
        await tick();
      }
    }
  }

  /**
   * Inputs: none. Outputs: opens ChatGPT OAuth guide URL.
   */
  async function onOauthGuide() {
    try {
      await api.openUrl(chatgptOauthGuideUrl(source.trim()));
    } catch (e) {
      alert(String(e));
    }
  }

  /**
   * Inputs: none. Outputs: draft test message.
   */
  async function onTest() {
    msg = "Testing…";
    msgTone = "";
    try {
      const draft = readDraft({
        source,
        headersText,
        bearer,
        envText,
        argsText,
        oauthConnected,
      });
      const result = await api.testDraft({
        name: name || "draft",
        transport: draft.transport,
        secrets: draft.secrets,
      });
      msg = result;
      msgTone = "ok";
    } catch (e) {
      msg = String(e);
      msgTone = "err";
    }
  }

  /**
   * Inputs: submit event. Outputs: saved server or error message.
   */
  async function onSubmit(e) {
    e.preventDefault();
    try {
      const draft = readDraft({
        source,
        headersText,
        bearer,
        envText,
        argsText,
        oauthConnected,
      });
      await api.upsertServer({
        id: oauthConnected ? ensureEditId() : editId || null,
        name: name.trim(),
        enabled,
        transport: draft.transport,
        secrets: draft.secrets,
      });
      open = false;
      await onSaved();
    } catch (error) {
      msg = String(error);
      msgTone = "err";
    }
  }
</script>

<Dialog.Root bind:open>
  <Dialog.Content class="sm:max-w-lg max-h-[90vh] overflow-y-auto">
    <Dialog.Header>
      <Dialog.Title>{server ? "Edit MCP" : "Add MCP"}</Dialog.Title>
    </Dialog.Header>
    <form class="grid gap-3" onsubmit={onSubmit}>
      <div class="grid gap-2">
        <Label for="edit-name">Name</Label>
        <Input id="edit-name" bind:value={name} required />
      </div>
      <div class="flex items-center gap-3">
        <Switch id="edit-enabled" bind:checked={enabled} />
        <Label for="edit-enabled">Enabled</Label>
      </div>
      <div class="grid gap-2">
        <Label for="edit-source">Command or MCP URL</Label>
        <Input
          id="edit-source"
          bind:value={source}
          required
          placeholder="https://mcp.slack.com/mcp or npx"
          autocomplete="off"
          oninput={onSourceInput}
          onpaste={onSourcePaste}
        />
        <p class="text-muted-foreground text-xs m-0">{kindHint}</p>
      </div>
      {#if !(http || empty)}
        <div class="grid gap-3">
          <div class="grid gap-2">
            <Label for="edit-args">Args (one per line)</Label>
            <Textarea
              id="edit-args"
              bind:value={argsText}
              rows={3}
              placeholder={"-y\n@modelcontextprotocol/server-everything"}
            />
          </div>
          <div class="grid gap-2">
            <Label for="edit-env">Env secrets (KEY=value per line)</Label>
            <Textarea
              id="edit-env"
              bind:value={envText}
              rows={3}
              placeholder="API_KEY=..."
            />
          </div>
        </div>
      {/if}
      {#if http && showOauth}
        <div class="grid gap-3">
          <div class="flex items-center justify-between gap-3">
            <span class="text-muted-foreground text-xs">{oauthStatus}</span>
            <Button type="button" onclick={onOauth}>{oauthBtnLabel}</Button>
          </div>
          {#if showGuide}
            <div class="grid gap-2 rounded-xl border bg-muted/40 p-3 text-xs leading-relaxed">
              <p class="text-muted-foreground m-0">
                This server does not support automatic app registration. Create
                an OAuth app with the provider, add redirect URI
                <code>http://127.0.0.1:7342/oauth/callback</code>, paste the
                Client ID (and secret if they give you one) under Advanced, then
                Sign in.
              </p>
              <Button
                type="button"
                variant="ghost"
                size="sm"
                class="justify-self-start"
                onclick={onOauthGuide}>Ask ChatGPT how</Button
              >
            </div>
          {/if}
        </div>
      {/if}
      {#if showAdvanced}
        <details class="rounded-xl border px-3 py-2" bind:open={advancedOpen}>
          <summary class="cursor-pointer text-muted-foreground text-sm">
            Advanced
          </summary>
          <div class="grid gap-3 pt-3">
            <div class="grid gap-2">
              <Label for="edit-oauth-client-id">OAuth client ID (optional)</Label>
              <Input
                id="edit-oauth-client-id"
                bind:value={oauthClientId}
                autocomplete="off"
                placeholder="From your OAuth app"
              />
            </div>
            <div class="grid gap-2">
              <Label for="edit-oauth-client-secret"
                >OAuth client secret (optional)</Label
              >
              <Input
                id="edit-oauth-client-secret"
                bind:value={oauthClientSecret}
                type="password"
              />
            </div>
            <p class="text-muted-foreground text-xs m-0">
              Needed when Sign in cannot register automatically (see guide
              above). Redirect URI:
              <code>http://127.0.0.1:7342/oauth/callback</code>
            </p>
            <div class="grid gap-2">
              <Label for="edit-bearer">Bearer token (optional)</Label>
              <Input id="edit-bearer" bind:value={bearer} type="password" />
            </div>
            <div class="grid gap-2">
              <Label for="edit-headers">Headers (Name=value per line)</Label>
              <Textarea
                id="edit-headers"
                bind:value={headersText}
                rows={3}
                placeholder="X-Api-Key=..."
              />
            </div>
          </div>
        </details>
      {/if}
      <p
        class="min-h-5 text-sm m-0 {msgTone === 'err'
          ? 'text-destructive'
          : msgTone === 'ok'
            ? 'text-ok'
            : 'text-muted-foreground'}"
      >
        {msg}
      </p>
      <Dialog.Footer class="gap-2 sm:justify-end">
        <Button type="button" variant="outline" onclick={onTest}>Test</Button>
        <Button type="button" variant="ghost" onclick={() => (open = false)}>
          Cancel
        </Button>
        <Button type="submit">Save</Button>
      </Dialog.Footer>
    </form>
  </Dialog.Content>
</Dialog.Root>
