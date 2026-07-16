<script>
  import { onMount } from "svelte";
  import * as api from "$lib/api.js";
  import { Button } from "$lib/components/ui/button/index.js";
  import { Badge } from "$lib/components/ui/badge/index.js";
  import * as Tabs from "$lib/components/ui/tabs/index.js";
  import Titlebar from "./components/Titlebar.svelte";
  import Endpoint from "./components/Endpoint.svelte";
  import ServerList from "./components/ServerList.svelte";
  import Editor from "./components/Editor.svelte";
  import Marketplace from "./components/Marketplace.svelte";

  let status = $state({ running: false, endpoint: "" });
  let token = $state("");
  let servers = $state([]);
  let autostart = $state(false);
  let editorOpen = $state(false);
  let editing = $state(null);
  let tab = $state("configure");

  /**
   * Inputs: none. Outputs: refreshed status, token, servers, autostart.
   */
  async function refresh() {
    const [nextStatus, nextToken, nextServers, nextAutostart] =
      await Promise.all([
        api.getStatus(),
        api.getToken(),
        api.listServers(),
        api.getAutostart(),
      ]);
    status = nextStatus;
    token = nextToken;
    servers = nextServers;
    autostart = !!nextAutostart;
  }

  /**
   * Inputs: none. Outputs: toggled funnel running state.
   */
  async function onToggle() {
    try {
      const current = await api.getStatus();
      if (current.running) await api.stopFunnel();
      else await api.startFunnel();
      await refresh();
    } catch (e) {
      alert(String(e));
    }
  }

  /**
   * Inputs: optional server. Outputs: opened editor.
   */
  function onEdit(server) {
    editing = server;
    editorOpen = true;
  }

  onMount(() => {
    (async () => {
      for (let i = 0; i < 25; i++) {
        await refresh();
        if (status.running) break;
        await new Promise((r) => setTimeout(r, 100));
      }
    })().catch((e) => {
      console.error(e);
      alert(String(e));
    });
  });
</script>

<Titlebar />

<header class="flex items-center justify-between gap-4 max-sm:flex-col max-sm:items-start">
  <div>
    <h1 class="text-2xl tracking-tight font-semibold m-0">funnelit</h1>
    <p class="text-muted-foreground text-sm m-0">
      Configure here · funnel runs in the tray
    </p>
  </div>
  <div class="flex items-center gap-2 flex-wrap">
    <Badge variant={status.running ? "default" : "outline"}>
      {#if status.running}
        <span class="relative flex size-2" aria-hidden="true">
          <span
            class="absolute inset-0 animate-ping motion-reduce:animate-none rounded-full bg-current opacity-75"
          ></span>
          <span class="relative size-2 rounded-full bg-current"></span>
        </span>
      {/if}
      {status.running ? "running" : "stopped"}
    </Badge>
    <Button variant="ghost" onclick={onToggle}>
      {status.running ? "Pause" : "Resume"}
    </Button>
  </div>
</header>

<Tabs.Root bind:value={tab}>
  <Tabs.List>
    <Tabs.Trigger value="configure">Configure</Tabs.Trigger>
    <Tabs.Trigger value="marketplace">Marketplace</Tabs.Trigger>
  </Tabs.List>
  <Tabs.Content value="configure" class="grid gap-4">
    <Endpoint {status} {token} bind:autostart {refresh} />
    <ServerList {servers} {onEdit} {refresh} />
  </Tabs.Content>
  <Tabs.Content value="marketplace">
    <Marketplace {servers} {refresh} />
  </Tabs.Content>
</Tabs.Root>

<Editor bind:open={editorOpen} server={editing} onSaved={refresh} />
