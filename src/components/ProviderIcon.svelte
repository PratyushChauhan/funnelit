<script>
  /**
   * Inputs: icon src URL, display name for fallback letter.
   * Outputs: provider icon or initial badge.
   */
  let { src = "", name = "" } = $props();
  let failedSrc = $state("");
</script>

{#if src && failedSrc !== src}
  <img
    src={src}
    onerror={() => (failedSrc = src)}
    alt=""
    width="20"
    height="20"
    class="size-5 shrink-0 rounded-sm object-contain"
  />
{:else}
  <span
    class="bg-muted text-muted-foreground flex size-5 shrink-0 items-center justify-center rounded-sm text-[10px] font-semibold"
    aria-hidden="true">{(name || "?").slice(0, 1)}</span
  >
{/if}
