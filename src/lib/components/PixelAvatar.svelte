<script lang="ts">
  // 根据用户名生成一个 8×8 的像素头像（左右对称），同名永远长一样。
  // 以后接入正版皮肤后，可以换成皮肤的脸部。
  let { name, size = 40 }: { name: string; size?: number } = $props();

  const SKINS = ["#F2C6A0", "#E0AC82", "#C68B61", "#9C6845", "#F5D3B8"];
  const HAIRS = ["#3B2A1E", "#6B4226", "#C9A253", "#1E1E1E", "#8C3B2E", "#5A5A66"];
  const EYES = ["#2F5DA8", "#3B7D3B", "#4A3426", "#5B3E8C", "#1F6E73"];

  function rng(seed: string) {
    let h = 2166136261;
    for (const ch of seed) h = Math.imul(h ^ ch.charCodeAt(0), 16777619);
    return () => {
      h += 0x6d2b79f5;
      let t = h;
      t = Math.imul(t ^ (t >>> 15), t | 1);
      t ^= t + Math.imul(t ^ (t >>> 7), t | 61);
      return ((t ^ (t >>> 14)) >>> 0) / 4294967296;
    };
  }

  const pixels = $derived.by(() => {
    const r = rng(name.toLowerCase());
    const pick = <T,>(a: T[]) => a[Math.floor(r() * a.length)];
    const skin = pick(SKINS);
    const hair = pick(HAIRS);
    const eye = pick(EYES);
    const fringe = 1 + Math.floor(r() * 2); // 刘海厚度
    const out: string[] = [];
    for (let y = 0; y < 8; y++) {
      const row: string[] = [];
      for (let x = 0; x < 4; x++) {
        let c = skin;
        if (y < fringe) c = hair;
        else if (y === fringe && r() < 0.5) c = hair;
        else if (x === 0 && y < 5 && r() < 0.6) c = hair; // 鬓角
        if (y === 4 && x === 1) c = "#FFFFFF";
        if (y === 4 && x === 2) c = eye;
        if (y === 6 && x === 3) c = "#8A4B3A";
        if (y === 6 && x === 2 && r() < 0.4) c = "#8A4B3A";
        row.push(c);
      }
      out.push(...row, ...[...row].reverse());
    }
    return out;
  });
</script>

<div class="px" style:width="{size}px" style:height="{size}px" style:border-radius="{Math.max(4, Math.round(size * 0.14))}px" aria-hidden="true">
  {#each pixels as c}<i style:background={c}></i>{/each}
</div>

<style>
  .px {
    display: grid;
    grid-template-columns: repeat(8, 1fr);
    grid-template-rows: repeat(8, 1fr);
    overflow: hidden;
    flex-shrink: 0;
    box-shadow: 0 0 0 1px var(--border);
    image-rendering: pixelated;
  }
  i {
    display: block;
  }
</style>
