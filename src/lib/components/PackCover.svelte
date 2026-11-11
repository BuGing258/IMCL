<script lang="ts">
  import { Sword, Box, Leaf } from "@lucide/svelte";
  import type { Pack } from "$lib/mock";

  let { pack, size = 40 }: { pack: Pick<Pack, "color" | "icon">; size?: number } = $props();

  const ICONS = { sword: Sword, cube: Box, leaf: Leaf };
  const Icon = $derived(ICONS[pack.icon]);
</script>

<div class="cover" style:width="{size}px" style:height="{size}px" style:background={pack.color} style:border-radius="{Math.round(size * 0.22)}px">
  <Icon size={Math.round(size * 0.5)} strokeWidth={size > 80 ? 1.5 : 2} />
</div>

<style>
  .cover {
    flex-shrink: 0;
    display: flex;
    align-items: center;
    justify-content: center;
    color: #fff;
  }
</style>
