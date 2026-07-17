<!--
  @component
  Inputs: icon src URL, display name for fallback letter.
  Outputs: provider icon or initial badge.
-->
<script>
  let { src = "", name = "" } = $props();
  let failedSrc = $state("");
</script>

{#if src && failedSrc !== src}
  <span
    class="bg-white flex size-5 shrink-0 items-center justify-center rounded-sm p-0.5 ring-1 ring-black/10"
  >
    <img
      src={src}
      onerror={() => (failedSrc = src)}
      alt=""
      width="16"
      height="16"
      class="size-full object-contain"
    />
  </span>
{:else}
  <span
    class="bg-muted text-muted-foreground flex size-5 shrink-0 items-center justify-center rounded-sm text-[10px] font-semibold"
    aria-hidden="true">{(name || "?").slice(0, 1)}</span
  >
{/if}
