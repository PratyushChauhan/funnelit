<!--
  @component
  Inputs: servers list, refresh callback.
  Outputs: DCR marketplace browse + one-click install.
-->
<script>
  import * as api from "$lib/api.js";
  import {
    filterMarketplaceEntries,
    isInstalled,
    listMarketplaceEntries,
  } from "$lib/marketplace.js";
  import { marketplaceIcon } from "$lib/marketplace-icons.js";
  import { Button } from "$lib/components/ui/button/index.js";
  import { Input } from "$lib/components/ui/input/index.js";
  import * as Card from "$lib/components/ui/card/index.js";
  import ProviderIcon from "./ProviderIcon.svelte";

  let { servers = [], refresh } = $props();

  const entries = listMarketplaceEntries();
  let query = $state("");
  let installingId = $state("");
  let msg = $state("");
  let msgTone = $state("");
  /**
   * Inputs: none.
   * Outputs: URLs saved via upsert even if parent refresh fails.
   */
  let justInstalledUrls = $state([]);

  const visible = $derived(filterMarketplaceEntries(entries, query));

  /**
   * Inputs: catalog entry. Outputs: true when configured or just installed.
   */
  function shownInstalled(entry) {
    return (
      isInstalled(entry, servers) || justInstalledUrls.includes(entry.url)
    );
  }

  /**
   * Inputs: catalog entry. Outputs: OAuth-first install or error message.
   */
  async function onInstall(entry) {
    if (installingId || shownInstalled(entry)) return;
    installingId = entry.id;
    msg = "Opening browser sign-in…";
    msgTone = "";
    const id = crypto.randomUUID();
    try {
      const probe = await api.probeMcpAuth({ url: entry.url, id });
      if (!probe?.oauth || !probe.supports_dcr) {
        throw new Error(
          "This server no longer supports automatic registration. Add it from Configure instead.",
        );
      }
      await api.startMcpOauth({ url: entry.url, id });
      await api.upsertServer({
        id,
        name: entry.name,
        enabled: true,
        transport: {
          kind: "http",
          url: entry.url,
          header_keys: [],
          has_bearer: true,
        },
        secrets: {},
      });
      justInstalledUrls = [...justInstalledUrls, entry.url];
      msg = `${entry.name} installed`;
      msgTone = "ok";
      try {
        await refresh();
      } catch {
        /* keep success; justInstalledUrls blocks duplicate install */
      }
    } catch (e) {
      msg = String(e);
      msgTone = "err";
    } finally {
      installingId = "";
    }
  }
</script>

<Card.Root size="sm">
  <Card.Header
    class="flex-row items-center justify-between gap-3 space-y-0 pb-0"
  >
    <Card.Title class="text-base">Marketplace</Card.Title>
    <Input
      class="h-7 max-w-[11rem] text-xs"
      type="search"
      placeholder="Search"
      bind:value={query}
      autocomplete="off"
    />
  </Card.Header>
  <Card.Content class="grid gap-2">
    {#if msg}
      <p
        aria-live="polite"
        role="status"
        class="text-xs m-0 {msgTone === 'err'
          ? 'text-destructive'
          : msgTone === 'ok'
            ? 'text-ok'
            : 'text-muted-foreground'}"
      >
        {msg}
      </p>
    {/if}
    <ul class="m-0 p-0 list-none grid gap-1.5 sm:grid-cols-2">
      {#if !visible.length}
        <li class="text-muted-foreground text-xs sm:col-span-2">
          No matches. Add advanced MCPs under Configure.
        </li>
      {:else}
        {#each visible as entry (entry.id)}
          {@const installed = shownInstalled(entry)}
          {@const busy = installingId === entry.id}
          <li
            class="flex items-center gap-2 rounded-lg border bg-card px-2 py-1.5"
          >
            <ProviderIcon src={marketplaceIcon(entry.id)} name={entry.name} />
            <div class="min-w-0 flex-1">
              <div class="truncate text-sm font-medium leading-tight">
                {entry.name}
              </div>
              <div class="text-muted-foreground truncate text-[11px] leading-tight">
                {entry.description}
              </div>
            </div>
            {#if installed}
              <Button variant="outline" size="xs" disabled>Installed</Button>
            {:else}
              <Button
                size="xs"
                disabled={!!installingId}
                onclick={() => onInstall(entry)}
              >
                {busy ? "…" : "Install"}
              </Button>
            {/if}
          </li>
        {/each}
      {/if}
    </ul>
  </Card.Content>
</Card.Root>
