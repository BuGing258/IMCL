<script lang="ts">
  import { fly } from "svelte/transition";
  import { toast } from "$lib/state.svelte";
</script>

{#if toast.visible}
  <div class="toast" role="status" transition:fly={{ y: 12, duration: 200 }}>{toast.msg}</div>
{/if}

<style>
  .toast {
    position: fixed;
    left: 50%;
    bottom: 24px;
    translate: -50% 0;
    z-index: 100;
    padding: 8px 16px;
    border-radius: 999px;
    background: var(--text);
    color: var(--bg);
    font-size: 13px;
    box-shadow: var(--shadow);
    pointer-events: none;
    white-space: nowrap;
  }
</style>
