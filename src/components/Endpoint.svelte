<!--
  @component
  Inputs: status, token, autostart, refresh callback.
  Outputs: endpoint panel UI events.
-->
<script>
  import * as api from "$lib/api.js";
  import { Button } from "$lib/components/ui/button/index.js";
  import { Input } from "$lib/components/ui/input/index.js";
  import { Label } from "$lib/components/ui/label/index.js";
  import { Switch } from "$lib/components/ui/switch/index.js";
  import * as Card from "$lib/components/ui/card/index.js";
  import { Separator } from "$lib/components/ui/separator/index.js";

  let { status, token, autostart = $bindable(false), refresh } = $props();

  let revealToken = $state(false);

  const snippet = $derived(
    JSON.stringify(
      {
        mcpServers: {
          sumeru: {
            url: status?.endpoint ?? "",
            headers: {
              Authorization: revealToken
                ? `Bearer ${token}`
                : "Bearer <TOKEN>",
            },
          },
        },
      },
      null,
      2,
    ),
  );

  /**
   * Inputs: text. Outputs: clipboard write or user-facing error.
   */
  async function copy(text) {
    try {
      await navigator.clipboard.writeText(text);
    } catch (e) {
      alert(String(e));
    }
  }

  /**
   * Inputs: checked. Outputs: persisted autostart or revert.
   */
  async function onAutostart(checked) {
    try {
      await api.setAutostart(checked);
    } catch (err) {
      autostart = !checked;
      alert(String(err));
    }
  }

  /**
   * Inputs: none. Outputs: rotated token via refresh.
   */
  async function onRotate() {
    try {
      await api.rotateToken();
      await refresh();
    } catch (e) {
      alert(String(e));
    }
  }
</script>

<Card.Root>
  <Card.Header>
    <Card.Title>Endpoint</Card.Title>
  </Card.Header>
  <Card.Content class="grid gap-4">
    <div class="grid gap-2">
      <Label for="endpoint-url">URL</Label>
      <div class="flex gap-2 flex-wrap">
        <Input id="endpoint-url" value={status?.endpoint ?? ""} readonly class="flex-1 min-w-48" />
        <Button variant="outline" onclick={() => copy(status?.endpoint ?? "")}>
          Copy
        </Button>
      </div>
    </div>
    <div class="grid gap-2">
      <Label for="endpoint-token">Bearer token</Label>
      <div class="flex gap-2 flex-wrap">
        <Input
          id="endpoint-token"
          type={revealToken ? "text" : "password"}
          value={token}
          readonly
          class="flex-1 min-w-48"
        />
        <Button variant="outline" onclick={() => (revealToken = !revealToken)}>
          {revealToken ? "Hide" : "Show"}
        </Button>
        <Button variant="outline" onclick={() => copy(token)}>Copy</Button>
        <Button variant="outline" onclick={onRotate}>Rotate</Button>
      </div>
    </div>
    <div class="grid gap-2">
      <Label>Cursor mcp.json</Label>
      <pre
        class="m-0 overflow-auto rounded-2xl bg-background p-3 font-mono text-xs leading-relaxed text-foreground ring-1 ring-foreground/10"
      >{snippet}</pre>
      <Button variant="outline" class="w-fit" onclick={() => copy(snippet)}>Copy snippet</Button>
    </div>
    <Separator />
    <div class="flex items-center justify-between gap-4">
      <div class="grid gap-0.5">
        <Label for="autostart">Run at system startup</Label>
        <p class="text-muted-foreground text-xs m-0">
          Starts hidden in the tray after login
        </p>
      </div>
      <Switch
        id="autostart"
        bind:checked={autostart}
        onCheckedChange={onAutostart}
      />
    </div>
  </Card.Content>
</Card.Root>
