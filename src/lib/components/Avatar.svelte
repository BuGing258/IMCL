<script lang="ts">
  // 账户头像：从皮肤图里取脸部（8×8）和帽子层，像游戏里一样按像素放大。
  // 还没有可用的皮肤（比如一个游戏都没装）时，用按名字生成的像素头像代替。
  import PixelAvatar from "./PixelAvatar.svelte";
  import { skins, type Account } from "$lib/state.svelte";

  let {
    account,
    src: srcProp,
    name = "",
    size = 40,
  }: { account?: Account; src?: string | null; name?: string; size?: number } = $props();

  const src = $derived(srcProp !== undefined ? srcProp : account ? skins.of(account) : null);
  const label = $derived(account?.name ?? name);

  let canvas = $state<HTMLCanvasElement>();
  let failed = $state(false);

  /** 旧格式（64×32）皮肤的帽子层如果整块不透明，游戏会当作没有帽子 */
  function hatIsSolid(ctx: CanvasRenderingContext2D, u: number) {
    const d = ctx.getImageData(32 * u, 0, 32 * u, 16 * u).data;
    for (let i = 3; i < d.length; i += 4) if (d[i] < 128) return false;
    return true;
  }

  $effect(() => {
    const url = src;
    const el = canvas;
    if (!url || !el) return;
    failed = false;
    let live = true;
    const img = new Image();
    img.onload = () => {
      if (!live) return;
      const u = img.width / 64;
      const legacy = img.height * 2 === img.width;
      // 先画到原尺寸的画布上，方便检查帽子层
      const raw = document.createElement("canvas");
      raw.width = img.width;
      raw.height = img.height;
      const rc = raw.getContext("2d", { willReadFrequently: true })!;
      rc.drawImage(img, 0, 0);
      const hat = !(legacy && hatIsSolid(rc, u));

      const px = Math.round(size * (window.devicePixelRatio || 1));
      el.width = el.height = px;
      const ctx = el.getContext("2d")!;
      ctx.imageSmoothingEnabled = false;
      ctx.clearRect(0, 0, px, px);
      ctx.drawImage(raw, 8 * u, 8 * u, 8 * u, 8 * u, 0, 0, px, px);
      if (hat) ctx.drawImage(raw, 40 * u, 8 * u, 8 * u, 8 * u, 0, 0, px, px);
    };
    img.onerror = () => live && (failed = true);
    img.src = url;
    return () => {
      live = false;
    };
  });
</script>

{#if src && !failed}
  <canvas
    bind:this={canvas}
    class="face"
    style:width="{size}px"
    style:height="{size}px"
    style:border-radius="{Math.max(4, Math.round(size * 0.14))}px"
    aria-hidden="true"
  ></canvas>
{:else}
  <PixelAvatar name={label} {size} />
{/if}

<style>
  .face {
    display: block;
    flex-shrink: 0;
    box-shadow: 0 0 0 1px var(--border);
    image-rendering: pixelated;
    background: var(--hover);
  }
</style>
