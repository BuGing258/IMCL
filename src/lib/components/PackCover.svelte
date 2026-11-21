<script lang="ts">
  // 版本图标：按加载器区分。原版是叶子，Forge 灰色铁砧，Fabric 黄色布料，NeoForge 橙色狐狸头
  import { Box, Leaf, Anvil } from "@lucide/svelte";
  import type { Pack } from "$lib/mock";

  let { pack, size = 40 }: { pack: Pick<Pack, "color" | "icon">; size?: number } = $props();

  const LUCIDE = { cube: Box, leaf: Leaf, anvil: Anvil, sword: Anvil };
  const px = $derived(Math.round(size * 0.5));
  const sw = $derived(size > 80 ? 1.5 : 2);
  const Icon = $derived(pack.icon === "fabric" || pack.icon === "fox" ? null : LUCIDE[pack.icon]);
</script>

<div class="cover" style:width="{size}px" style:height="{size}px" style:background={pack.color} style:border-radius="{Math.round(size * 0.22)}px">
  {#if Icon}
    <Icon size={px} strokeWidth={sw} />
  {:else}
    <svg width={px} height={px} viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width={sw} stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">
      {#if pack.icon === "fabric"}
        <!-- 一块布：上下边是波浪，里面一圈缝线 -->
        <path d="M3.5 6.5c2.8-1.6 5.7 1.6 8.5 0s5.7-1.6 8.5 0v11c-2.8 1.6-5.7-1.6-8.5 0s-5.7 1.6-8.5 0z" />
        <path d="M7 9.5c1.7-.7 3.3.7 5 0s3.3-.7 5 0v5c-1.7.7-3.3-.7-5 0s-3.3.7-5 0z" stroke-dasharray="1.6 2" />
      {:else}
        <!-- 狐狸头：尖耳朵、尖下巴 -->
        <path d="M3.5 3.5 9 7.5h6l5.5-4-1.8 8L12 20.5l-6.7-9z" />
        <path d="M5.3 11.5 9 13M18.7 11.5 15 13" />
        <path d="M9.2 10.6h.01M14.8 10.6h.01" stroke-width={sw + 0.6} />
        <path d="M11 16.3h2l-1 1.2z" />
      {/if}
    </svg>
  {/if}
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
