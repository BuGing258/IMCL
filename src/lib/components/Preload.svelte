<script lang="ts">
  // 预渲染的页面外面包一层：告诉里面的页面“你是预渲染的”，别去加载数据（见 isPreload）
  import { setContext, type Snippet } from "svelte";
  import { PRELOAD } from "$lib/state.svelte";

  let { children }: { children: Snippet } = $props();
  setContext(PRELOAD, true);
</script>

{@render children()}
