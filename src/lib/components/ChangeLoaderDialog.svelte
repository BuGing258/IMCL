<script lang="ts">
  // 给已有版本换加载器或加载器版本：Mod、存档、配置都留着，只换游戏的启动配置
  import { Wrench, X, LoaderCircle, TriangleAlert } from "@lucide/svelte";
  import Modal from "./Modal.svelte";
  import Select from "./Select.svelte";
  import { api, type LoaderKind, type LoaderVersion } from "$lib/api";
  import { installs, settings, game } from "$lib/state.svelte";
  import type { Pack } from "$lib/mock";

  let { pack, onclose }: { pack: Pack; onclose: () => void } = $props();

  type Choice = LoaderKind | "原版";
  const KINDS: Choice[] = ["原版", "Fabric", "Quilt", "Forge", "NeoForge"];
  /** 哪些加载器的 Mod 能通用 */
  const family = (k: string) => (k === "Fabric" || k === "Quilt" ? "fabric" : k === "Forge" ? "forge" : k === "NeoForge" ? "neoforge" : "vanilla");

  // svelte-ignore state_referenced_locally
  let kind = $state<Choice>(pack.loader as Choice);
  let list = $state<LoaderVersion[] | null>(null);
  let err = $state("");
  let picked = $state("");
  let seq = 0;

  $effect(() => {
    const k = kind;
    list = null;
    err = "";
    picked = "";
    if (k === "原版") return;
    const my = ++seq;
    api
      .loaderVersions(settings.source, k, pack.mc)
      .then((l) => {
        if (my !== seq) return;
        list = l;
        // 同一个加载器默认选现在装的版本，否则选推荐 / 最新的
        const cur = k === pack.loader ? l.find((v) => v.id === pack.loaderVersion || v.version === pack.loaderVersion) : undefined;
        picked = (cur ?? l.find((v) => v.stable) ?? l[0])?.id ?? "";
      })
      .catch((e) => my === seq && (err = String(e)));
  });

  const same = $derived(kind === pack.loader && (kind === "原版" || picked === pack.loaderVersion || list?.find((v) => v.id === picked)?.version === pack.loaderVersion));
  const familyChange = $derived(pack.mods.length > 0 && family(kind) !== family(pack.loader));
  const busy = $derived(!!Object.values(installs.tasks).find((t) => t.loaderFor === pack.name && t.status !== "error"));
  const canGo = $derived(!same && !busy && game.status === "idle" && (kind === "原版" || (!!picked && !!list?.length)));

  function go() {
    if (!canGo) return;
    installs.startChangeLoader(pack, kind === "原版" ? null : { kind, id: picked });
    onclose();
  }
</script>

<Modal {onclose} width={460} labelledby="cl-title">
  <header>
    <span class="ic"><Wrench size={18} /></span>
    <div class="ht">
      <h2 id="cl-title">更换加载器</h2>
      <p>{pack.name} · Minecraft {pack.mc}</p>
    </div>
    <button class="btn btn-sm btn-icon" onclick={onclose} aria-label="关闭"><X size={15} /></button>
  </header>

  <div class="seg kinds">
    {#each KINDS as k (k)}
      <button class:on={kind === k} onclick={() => (kind = k)}>{k}</button>
    {/each}
  </div>

  {#if kind !== "原版"}
    <div class="row">
      <span class="lbl">{kind} 版本</span>
      {#if err}
        <span class="err-text">{err}</span>
      {:else if list === null}
        <span class="faint load"><LoaderCircle size={14} class="spin" />正在获取…</span>
      {:else if !list.length}
        <span class="faint">Minecraft {pack.mc} 没有 {kind}</span>
      {:else}
        <Select
          bind:value={picked}
          width="220px"
          ariaLabel="{kind} 版本"
          options={list.map((v) => ({ value: v.id, label: v.version, hint: v.id === pack.loaderVersion || v.version === pack.loaderVersion ? "当前" : v.stable ? (kind === "Forge" ? "推荐" : "") : "测试版" }))}
        />
      {/if}
    </div>
  {/if}

  {#if familyChange}
    <p class="warn"><TriangleAlert size={14} />换成 {kind} 后，现有的 {pack.mods.length} 个 Mod 大多用不了，需要换成 {kind} 版的</p>
  {/if}
  {#if pack.optifine}
    <p class="warn"><TriangleAlert size={14} />OptiFine 会被去掉</p>
  {/if}
  {#if game.status !== "idle"}
    <p class="warn"><TriangleAlert size={14} />游戏运行中，关掉游戏后才能更换</p>
  {/if}

  <footer>
    <span class="sp"></span>
    <button class="btn btn-sm" onclick={onclose}>取消</button>
    <button class="btn btn-sm btn-primary" onclick={go} disabled={!canGo}>{same ? "已经是这个版本" : "更换"}</button>
  </footer>
</Modal>

<style>
  header {
    display: flex;
    align-items: center;
    gap: 12px;
  }
  .ic {
    width: 36px;
    height: 36px;
    border-radius: 10px;
    display: grid;
    place-items: center;
    flex-shrink: 0;
    background: var(--red-soft);
    color: var(--red);
  }
  .ht {
    flex: 1;
    min-width: 0;
  }
  h2 {
    font-size: 16px;
    font-weight: 700;
  }
  .ht p {
    font-size: 12px;
    color: var(--text-2);
  }
  .kinds {
    width: 100%;
  }
  .kinds button {
    flex: 1;
  }
  .row {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 12px;
    min-height: 36px;
  }
  .lbl {
    font-size: 13px;
    font-weight: 600;
  }
  .load {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    font-size: 12.5px;
  }
  .warn {
    display: flex;
    align-items: center;
    gap: 6px;
    font-size: 12.5px;
    color: var(--amber);
  }
  footer {
    display: flex;
    align-items: center;
    gap: 8px;
  }
  .sp {
    flex: 1;
  }
</style>
