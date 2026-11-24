<script lang="ts">
  // 联机 Mod（Remote Play）的图标：两个方块（两边玩家的世界）由一条虚线弧连到中间的中转点，上面是信号
  let { size = 128 }: { size?: number } = $props();
  // 页面上可能同时有好几个图标，渐变的 id 不能重复
  const id = `rp-${Math.random().toString(36).slice(2, 8)}`;
</script>

<svg width={size} height={size} viewBox="0 0 128 128" aria-hidden="true">
  <defs>
    <linearGradient id="{id}-bg" x1="0" y1="0" x2="1" y2="1">
      <stop offset="0" stop-color="#ff6a6f" />
      <stop offset="1" stop-color="#d4213d" />
    </linearGradient>
    <linearGradient id="{id}-hi" x1="0" y1="0" x2="0" y2="1">
      <stop offset="0" stop-color="#fff" stop-opacity="0.22" />
      <stop offset="0.5" stop-color="#fff" stop-opacity="0" />
    </linearGradient>
  </defs>
  <rect width="128" height="128" rx="30" fill="url(#{id}-bg)" />
  <rect width="128" height="128" rx="30" fill="url(#{id}-hi)" />

  <!-- 连线：从两个方块顶上弯到中间的中转点 -->
  <path d="M32 66 C32 34 96 34 96 66" fill="none" stroke="#fff" stroke-opacity="0.75" stroke-width="4.5" stroke-linecap="round" stroke-dasharray="0.1 9" />

  <!-- 信号 -->
  <path d="M53.3 31 A14 14 0 0 1 74.7 31" fill="none" stroke="#fff" stroke-width="5" stroke-linecap="round" />
  <path d="M45.6 23.6 A24 24 0 0 1 82.4 23.6" fill="none" stroke="#fff" stroke-opacity="0.6" stroke-width="5" stroke-linecap="round" />
  <!-- 中转点 -->
  <circle cx="64" cy="42" r="8" fill="#fff" />
  <circle cx="64" cy="42" r="3.2" fill="#d4213d" />

  <!-- 两个方块 -->
  {#each [32, 96] as x}
    <path d="M{x} 68 L{x + 15} 76.5 L{x} 85 L{x - 15} 76.5 Z" fill="#fff" />
    <path d="M{x - 15} 76.5 L{x} 85 L{x} 102 L{x - 15} 93.5 Z" fill="#fff" fill-opacity="0.7" />
    <path d="M{x} 85 L{x + 15} 76.5 L{x + 15} 93.5 L{x} 102 Z" fill="#fff" fill-opacity="0.86" />
  {/each}
</svg>
