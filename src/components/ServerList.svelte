<script>
  import * as api from "$lib/api.js";
  import { providerIconForUrl } from "$lib/marketplace-icons.js";
  import { Button } from "$lib/components/ui/button/index.js";
  import * as Card from "$lib/components/ui/card/index.js";
  import ProviderIcon from "./ProviderIcon.svelte";

  /**
   * Inputs: servers list, edit/refresh callbacks.
   * Outputs: MCP list actions.
   */
  let { servers = [], onEdit, refresh } = $props();

  /**
   * Inputs: server. Outputs: test alert.
   */
  async function onTest(server) {
    try {
      alert(await api.testServer(server.id));
    } catch (e) {
      alert(String(e));
    }
  }

  /**
   * Inputs: server. Outputs: delete + refresh when confirmed.
   */
  async function onDelete(server) {
    if (!confirm(`Delete ${server.name}?`)) return;
    try {
      await api.removeServer(server.id);
      await refresh();
    } catch (e) {
      alert(String(e));
    }
  }

  /**
   * Inputs: server. Outputs: detail line for the list meta.
   */
  function detail(server) {
    const kind = server.transport.kind;
    return kind === "stdio"
      ? `${server.transport.command} ${(server.transport.args || []).join(" ")}`
      : server.transport.url;
  }

  /**
   * Inputs: server. Outputs: provider icon URL when catalog matches.
   */
  function iconFor(server) {
    return server?.transport?.kind === "http"
      ? providerIconForUrl(server.transport.url)
      : "";
  }
</script>

<Card.Root>
  <Card.Header class="flex-row items-center justify-between gap-4 space-y-0">
    <Card.Title>MCPs</Card.Title>
    <Button onclick={() => onEdit(null)}>Add MCP</Button>
  </Card.Header>
  <Card.Content>
    <ul class="m-0 p-0 list-none grid gap-2.5">
      {#if !servers.length}
        <li class="text-muted-foreground text-sm">
          No MCPs yet. Add a stdio command or paste an MCP URL.
        </li>
      {:else}
        {#each servers as server (server.id)}
          <li
            class="flex items-center justify-between gap-3 rounded-xl border bg-card p-3 max-sm:flex-col max-sm:items-stretch"
          >
            <div class="flex min-w-0 items-center gap-2.5">
              <ProviderIcon src={iconFor(server)} name={server.name} />
              <div class="min-w-0">
                <strong class="block font-semibold">{server.name}</strong>
                <div class="text-muted-foreground text-xs truncate">
                  {server.transport.kind} · {server.enabled
                    ? "enabled"
                    : "disabled"} · {detail(server)}
                </div>
              </div>
            </div>
            <div class="flex gap-2 flex-wrap">
              <Button variant="outline" size="sm" onclick={() => onTest(server)}>
                Test
              </Button>
              <Button variant="outline" size="sm" onclick={() => onEdit(server)}>
                Edit
              </Button>
              <Button
                variant="destructive"
                size="sm"
                onclick={() => onDelete(server)}>Delete</Button
              >
            </div>
          </li>
        {/each}
      {/if}
    </ul>
  </Card.Content>
</Card.Root>
