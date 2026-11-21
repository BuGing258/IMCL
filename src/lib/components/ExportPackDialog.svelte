<script lang="ts">
  // 把一个版本导出成 Modrinth 整合包（.mrpack），发给朋友导入就能玩一样的 Mod
  import Modal from "./Modal.svelte";
  import { X, LoaderCircle, PackageOpen, FolderOpen, TriangleAlert, Check, Folder, FileText } from "@lucide/svelte";
  import { revealItemInDir } from "@tauri-apps/plugin-opener";
  import { api, fmtBytes, type ExportEntry, type ExportResult } from "$lib/api";
  import { settings, toast, isTauri } from "$lib/state.svelte";
  import type { Pack } from "$lib/mock";

  let { pack, onclose }: { pack: Pack; onclose: () => void } = $props();

  /** 常见文件夹的中文说明 */
  const LABELS: Record<string, string> = {
    mods: "Mod",
    config: "Mod 配置",
    defaultconfigs: "默认配置",
    kubejs: "KubeJS 脚本",
    scripts: "脚本",
    resourcepacks: "材质包",
    shaderpacks: "光影",
    global_packs: "全局数据包",
    datapacks: "数据包",
    "options.txt": "游戏设置（按键、画面、音量）",
    "optionsof.txt": "OptiFine 设置",
    "optionsshaders.txt": "光影设置",
    "servers.dat": "服务器列表",
    saves: "存档",
    screenshots: "截图",
    schematics: "投影文件",
    journeymap: "JourneyMap 地图数据",
    xaero: "Xaero 地图数据",
  };

  // 只用版本名做默认值，之后用户自己改
  // svelte-ignore state_referenced_locally
  let name = $state(pack.name);
  let packVersion = $state("1.0.0");
  let summary = $state("");
  let entries = $state<ExportEntry[]>([]);
  let picked = $state<Record<string, boolean>>({});
  let loading = $state(true);
  let loadErr = $state("");
  let busy = $state(false);
  let err = $state("");
  let result = $state<ExportResult | null>(null);

  async function load() {
    loading = true;
    loadErr = "";
    try {
      entries = await api.exportEntries(settings.gameDir, pack.name);
      picked = Object.fromEntries(entries.map((e) => [e.name, e.default]));
    } catch (e) {
      loadErr = String(e);
    } finally {
      loading = false;
    }
  }
  load();

  const chosen = $derived(entries.filter((e) => picked[e.name]));
  const total = $derived(chosen.reduce((s, e) => s + e.size, 0));
  const bigSaves = $derived(picked["saves"] && (entries.find((e) => e.name === "saves")?.size ?? 0) > 50 * 1048576);
  const nameErr = $derived(!name.trim() ? "名称不能为空" : "");
  const verErr = $derived(!packVersion.trim() ? "版本号不能为空" : "");

  async function run() {
    if (nameErr || verErr || busy) return;
    err = "";
    const fileName = `${name.trim()} ${packVersion.trim()}`.replace(/[\\/:*?"<>|]/g, " ").replace(/\s+/g, " ").trim();
    const out = await api.pickSaveMrpack(fileName);
    if (!out) return;
    busy = true;
    try {
      result = await api.exportModpack({
        gameDir: settings.gameDir,
        version: pack.name,
        out,
        name: name.trim(),
        packVersion: packVersion.trim(),
        summary: summary.trim() || null,
        include: chosen.map((e) => e.name),
      });
    } catch (e) {
      err = String(e);
    } finally {
      busy = false;
    }
  }

  async function reveal() {
    if (!result) return;
    if (!isTauri()) return toast.show("浏览器预览里打不开文件夹");
    try {
      await revealItemInDir(result.path);
    } catch (e) {
      toast.show(`打不开：${e}`);
    }
  }

</script>

<Modal {onclose} width={560} labelledby="ex-title" closable={!busy}>
    <header>
      <span class="ic"><PackageOpen size={18} /></span>
      <div class="ht">
        <h2 id="ex-title">导出整合包</h2>
        <p>{pack.name} · Minecraft {pack.mc}{pack.loader !== "原版" ? ` · ${pack.loader}` : ""}</p>
      </div>
      <button class="btn btn-sm btn-icon" onclick={onclose} disabled={busy} aria-label="关闭"><X size={15} /></button>
    </header>

    {#if result}
      <div class="done">
        <span class="ok"><Check size={22} /></span>
        <b>导出好了</b>
        <p class="faint">{fmtBytes(result.size)}</p>
        <p class="path" title={result.path}>{result.path}</p>
        {#each result.warnings as w}
          <p class="warn"><TriangleAlert size={13} />{w}</p>
        {/each}
        <p class="faint tip">朋友在 Mod 页「整合包」里点「从文件导入」就能装</p>
      </div>
      <footer>
        <span class="sp"></span>
        <button class="btn btn-sm" onclick={reveal}><FolderOpen size={14} />打开所在文件夹</button>
        <button class="btn btn-sm btn-primary" onclick={onclose}>完成</button>
      </footer>
    {:else}
      <div class="fields">
        <label class="f">
          <span>名称</span>
          <input class="input" class:invalid={!!nameErr} bind:value={name} maxlength="60" spellcheck="false" />
        </label>
        <label class="f short">
          <span>版本</span>
          <input class="input mono" class:invalid={!!verErr} bind:value={packVersion} maxlength="20" spellcheck="false" />
        </label>
        <label class="f full">
          <span>简介</span>
          <input class="input" bind:value={summary} maxlength="200" placeholder="可以不填" />
        </label>
      </div>

      <section class="box">
        <div class="box-head">
          <b>包含的内容</b>
          <span class="faint">{chosen.length ? `已选 ${chosen.length} 项 · ${fmtBytes(total)}` : "什么都没选"}</span>
        </div>
        {#if loading}
          <p class="state"><LoaderCircle size={15} class="spin" />正在读取版本文件夹…</p>
        {:else if loadErr}
          <p class="err-text">{loadErr}</p>
        {:else if !entries.length}
          <p class="state">这个版本文件夹里没有可以导出的东西</p>
        {:else}
          <ul>
            {#each entries as e (e.name)}
              <li>
                <label>
                  <input type="checkbox" bind:checked={picked[e.name]} disabled={busy} />
                  <span class="ei">{#if e.isDir}<Folder size={14} />{:else}<FileText size={14} />{/if}</span>
                  <span class="en">
                    <b>{LABELS[e.name.toLowerCase()] ?? e.name}</b>
                    {#if LABELS[e.name.toLowerCase()]}<small>{e.name}</small>{/if}
                  </span>
                  <span class="es">{e.isDir ? `${e.files} 个文件 · ` : ""}{fmtBytes(e.size)}</span>
                </label>
              </li>
            {/each}
          </ul>
        {/if}
        {#if bigSaves}<p class="warn"><TriangleAlert size={13} />存档比较大，整合包会变得很大，发给朋友可能不方便</p>{/if}
        {#if pack.optifine}<p class="warn"><TriangleAlert size={13} />OptiFine 不能放进整合包，朋友那边需要自己装</p>{/if}
      </section>

      {#if err}<p class="err-text">{err}</p>{/if}

      <footer>
        <span class="sp"></span>
        <button class="btn btn-sm" onclick={onclose} disabled={busy}>取消</button>
        <button class="btn btn-sm btn-primary" onclick={run} disabled={busy || loading || !!nameErr || !!verErr || !chosen.length}>
          {#if busy}<LoaderCircle size={14} class="spin" />正在导出…{:else}导出…{/if}
        </button>
      </footer>
    {/if}
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
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .fields {
    display: grid;
    grid-template-columns: 1fr 120px;
    gap: 8px 10px;
  }
  .f {
    display: flex;
    flex-direction: column;
    gap: 4px;
    font-size: 12px;
    font-weight: 600;
    color: var(--text-2);
    min-width: 0;
  }
  .f.full {
    grid-column: 1 / -1;
  }
  .f .input {
    width: 100%;
    font-weight: 400;
  }
  .mono {
    font-family: var(--mono);
  }
  .box {
    display: flex;
    flex-direction: column;
    gap: 6px;
    padding: 10px 12px;
    border-radius: var(--r);
    background: var(--bg);
    border: 1px solid var(--border);
  }
  .box-head {
    display: flex;
    align-items: baseline;
    justify-content: space-between;
    font-size: 12.5px;
  }
  .box-head .faint {
    font-size: 12px;
  }
  ul {
    list-style: none;
    margin: 0;
    padding: 0;
    max-height: 220px;
    overflow-y: auto;
  }
  li label {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 5px 4px;
    border-radius: var(--r-sm);
    font-size: 12.5px;
    cursor: pointer;
  }
  li label:hover {
    background: var(--hover);
  }
  .ei {
    color: var(--text-3);
    display: grid;
  }
  .en {
    flex: 1;
    min-width: 0;
    display: flex;
    align-items: baseline;
    gap: 6px;
    overflow: hidden;
  }
  .en b {
    font-weight: 500;
    white-space: nowrap;
  }
  .en small {
    color: var(--text-3);
    font-family: var(--mono);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .es {
    color: var(--text-3);
    font-size: 11.5px;
    white-space: nowrap;
    font-variant-numeric: tabular-nums;
  }
  .state {
    display: flex;
    align-items: center;
    justify-content: center;
    gap: 6px;
    padding: 24px 0;
    font-size: 12.5px;
    color: var(--text-2);
  }
  .warn {
    display: flex;
    align-items: center;
    gap: 6px;
    font-size: 12px;
    color: var(--amber);
  }
  .done {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 6px;
    padding: 10px 0 4px;
    text-align: center;
  }
  .ok {
    width: 44px;
    height: 44px;
    border-radius: 50%;
    display: grid;
    place-items: center;
    background: var(--green-soft);
    color: var(--green);
  }
  .done b {
    font-size: 15px;
  }
  .done .faint {
    font-size: 12px;
  }
  .path {
    max-width: 100%;
    font-family: var(--mono);
    font-size: 11.5px;
    color: var(--text-2);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .tip {
    margin-top: 4px;
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
