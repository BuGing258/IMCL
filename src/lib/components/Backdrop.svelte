<script lang="ts">
  // 页面最底下的背景图（只在实色背景材质下显示）
  import { settings, appearance } from "$lib/state.svelte";

  const image = $derived(appearance.backgroundKind === "image" ? appearance.bgImage : null);
</script>

{#if image}
  <div class="bd" aria-hidden="true">
    <div
      class="pic"
      style:background-image={`url("${image}")`}
      style:filter={settings.bgBlur ? `blur(${settings.bgBlur}px)` : undefined}
      style:inset={settings.bgBlur ? `-${settings.bgBlur * 2}px` : "0"}
    ></div>
  </div>
{/if}

<style>
  /* 图片下面垫底色：图片模糊后边缘会变半透明，不垫的话边上会透出窗口后面的东西 */
  .bd {
    position: fixed;
    inset: 0;
    z-index: -1;
    overflow: hidden;
    pointer-events: none;
    background: var(--bg-solid);
  }
  .pic {
    position: absolute;
    background-position: center;
    background-size: cover;
    background-repeat: no-repeat;
  }
</style>
