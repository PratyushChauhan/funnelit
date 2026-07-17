<!--
  @component
  Inputs: none.
  Outputs: frameless chrome — drag region + min/max/close (close hides to tray).
-->
<script>
  import { getCurrentWindow } from "@tauri-apps/api/window";
  import MinusIcon from "@lucide/svelte/icons/minus";
  import SquareIcon from "@lucide/svelte/icons/square";
  import XIcon from "@lucide/svelte/icons/x";

  const win = getCurrentWindow();
  const btn =
    "inline-flex size-9 items-center justify-center text-muted-foreground hover:bg-muted hover:text-foreground";

  /**
   * Inputs: window action promise.
   * Outputs: alert on failure.
   */
  function act(p) {
    p.catch((e) => alert(String(e)));
  }
</script>

<div
  class="-mx-5 flex h-9 shrink-0 items-center border-b border-border bg-background"
>
  <div class="min-w-0 flex-1 self-stretch" data-tauri-drag-region></div>
  <button class={btn} onclick={() => act(win.minimize())} aria-label="Minimize">
    <MinusIcon class="size-3.5" />
  </button>
  <button
    class={btn}
    onclick={() => act(win.toggleMaximize())}
    aria-label="Maximize or restore window"
  >
    <SquareIcon class="size-3" />
  </button>
  <button
    class="{btn} hover:bg-destructive/20 hover:text-destructive"
    onclick={() => act(win.close())}
    aria-label="Close"
  >
    <XIcon class="size-3.5" />
  </button>
</div>
