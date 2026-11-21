<script lang="ts">
  import { fill } from "$lib/theme";
  import { RotateCcw, Info, Trash2, LoaderCircle } from "@lucide/svelte";
  import Select from "$lib/components/Select.svelte";
  import { library, settings, accounts, javas, javaOptions, ACCOUNT_LABELS, WINDOW_LABELS, fmtMem, nav, memory, game, installs, toast } from "$lib/state.svelte";
  import type { VersionSettings } from "$lib/api";
  import type { Pack } from "$lib/mock";

  let { pack }: { pack: Pack } = $props();

  let err = $state("");
  $effect(() => {
    if (!pack.vs) library.loadSettings(pack).catch((e) => (err = String(e)));
  });

  const vs = $derived<VersionSettings>(pack.vs ?? {});
  const overrides = $derived(Object.keys(vs).length);
  const save = (patch: Partial<VersionSettings>) => library.saveSettings(pack, patch);

  /* Java */
  const javaOpts = $derived.by(() => {
    const opts = [{ value: "", label: "跟随全局", hint: javas.label(settings.java) }, ...javaOptions()];
    if (vs.java && !opts.some((o) => o.value === vs.java)) opts.push({ value: vs.java, label: "找不到这个 Java", hint: vs.java });
    return opts;
  });
  async function browseJava() {
    const p = await javas.browse();
    if (p) save({ java: p });
  }

  /* 账户 */
  const accountOpts = $derived.by(() => {
    const opts = [
      { value: "", label: "跟随全局", hint: accounts.current?.name ?? "" },
      ...accounts.list.map((a) => ({ value: a.id, label: a.name, hint: ACCOUNT_LABELS[a.type], group: "我的账户" })),
    ];
    if (vs.account && !accounts.list.some((a) => a.id === vs.account))
      opts.push({ value: vs.account, label: "账户已删除", hint: "" });
    return opts;
  });

  /* 内存：null 跟随全局，0 自动分配，其他是手动分配的 MB；拖动时只改草稿，松手再保存 */
  const plan = $derived(memory.plan(pack.name));
  const memMax = $derived(plan?.maxMb ?? 16384);
  const memMode = $derived(vs.memory == null ? "global" : vs.memory === 0 ? "auto" : "manual");
  let memDraft = $state(4096);
  $effect(() => {
    memDraft = Math.min(vs.memory || settings.memory, memMax);
  });
  const globalMem = $derived(settings.memoryMode === "auto" ? "自动分配" : fmtMem(Math.min(settings.memory, memMax)));

  /* 窗口 */
  const windowOpts = $derived([
    { value: "", label: "跟随全局", hint: WINDOW_LABELS[settings.windowMode] },
    ...(Object.entries(WINDOW_LABELS) as [string, string][]).map(([value, label]) => ({ value, label })),
  ]);

  /* JVM 参数 */
  let argsDraft = $state("");
  $effect(() => {
    argsDraft = vs.jvmArgs ?? "";
  });

  const isolated = $derived(pack.isolated ?? true);

  /* 删除版本：点一次变成“再点一次确认”，3 秒内再点才删；正在运行、正在换加载器时不能删 */
  let confirmDelete = $state(false);
  let deleting = $state(false);
  let confirmTimer: ReturnType<typeof setTimeout> | undefined;
  const running = $derived(game.status !== "idle" && game.packId === pack.id);
  const changingLoader = $derived(Object.values(installs.tasks).some((t) => t.loaderFor === pack.name && t.status !== "error"));
  async function deleteVersion() {
    if (deleting || running || changingLoader) return;
    if (!confirmDelete) {
      confirmDelete = true;
      clearTimeout(confirmTimer);
      confirmTimer = setTimeout(() => (confirmDelete = false), 3000);
      return;
    }
    clearTimeout(confirmTimer);
    confirmDelete = false;
    deleting = true;
    const name = pack.name;
    try {
      await library.deleteVersion(pack);
      toast.show(`已把「${name}」移到回收站`);
    } catch (e) {
      toast.show(String(e));
    } finally {
      deleting = false;
    }
  }

  function resetAll() {
    save({ java: null, memory: null, account: null, windowMode: null, jvmArgs: null, isolation: null });
  }
</script>

<div class="vs">
  <div class="vs-head">
    <p>
      {#if overrides}
        <span class="badge red">已单独设置 {overrides} 项</span>
      {:else}
        <span class="badge gray">全部跟随全局</span>
      {/if}
      <span class="faint">只对 {pack.name} 生效</span>
    </p>
    <div class="vs-head-actions">
      <button class="btn btn-sm" onclick={() => nav.go("settings")}>全局设置</button>
      <button class="btn btn-sm" disabled={!overrides} onclick={resetAll}><RotateCcw size={13} />全部恢复默认</button>
    </div>
  </div>

  {#if err}<p class="err-text">读取版本设置失败：{err}</p>{/if}

  <section class="group">
    <h3>启动</h3>
    <div class="row">
      <div class="lbl"><b>Java</b></div>
      <div class="ctl">
        <Select
          value={vs.java ?? ""}
          options={javaOpts}
          actions={[{ label: "浏览 java.exe…", onclick: browseJava }]}
          width="230px"
          ariaLabel="Java"
          onchange={(v) => save({ java: v || null })}
        />
      </div>
    </div>
    <div class="row">
      <div class="lbl"><b>启动账户</b></div>
      <div class="ctl">
        <Select
          value={vs.account ?? ""}
          options={accountOpts}
          actions={[{ label: "管理账户…", onclick: () => nav.go("accounts") }]}
          width="230px"
          ariaLabel="启动账户"
          onchange={(v) => save({ account: v || null })}
        />
      </div>
    </div>
    <div class="row">
      <div class="lbl"><b>内存分配</b>{#if memMode === "global"}<span>跟随全局（{globalMem}）</span>{/if}</div>
      <div class="ctl">
        <div class="seg">
          <button class:on={memMode === "global"} onclick={() => save({ memory: null })}>跟随全局</button>
          <button class:on={memMode === "auto"} onclick={() => save({ memory: 0 })}>自动分配</button>
          <button class:on={memMode === "manual"} onclick={() => memMode !== "manual" && save({ memory: memDraft })}>手动分配</button>
        </div>
      </div>
    </div>
    {#if memMode !== "global"}
      <div class="row sub">
        <div class="lbl"></div>
        <div class="ctl">
          {#if memMode === "auto"}
            <input type="range" class="locked" min="1024" max={memMax} step="512" value={plan?.autoMb ?? 1024} use:fill={[plan?.autoMb, memMax]} disabled aria-label="内存占用" />
            <span class="val">{plan ? fmtMem(plan.autoMb) : "…"}</span>
          {:else}
            <input
              type="range"
              min="1024"
              max={memMax}
              step="512"
              bind:value={memDraft}
              use:fill={[memDraft, memMax]}
              onchange={() => save({ memory: memDraft })}
              aria-label="内存占用"
            />
            <span class="val">{fmtMem(memDraft)}</span>
          {/if}
        </div>
      </div>
    {/if}
    <div class="row">
      <div class="lbl"><b>游戏窗口</b></div>
      <div class="ctl">
        <Select
          value={vs.windowMode ?? ""}
          options={windowOpts}
          width="230px"
          ariaLabel="游戏窗口"
          onchange={(v) => save({ windowMode: v || null })}
        />
      </div>
    </div>
    <div class="row">
      <div class="lbl"><b>JVM 参数</b><span>留空跟随全局</span></div>
      <div class="ctl">
        <input
          class="input mono"
          bind:value={argsDraft}
          placeholder={settings.jvmArgs ? `跟随全局：${settings.jvmArgs}` : "跟随全局（空）"}
          spellcheck="false"
          aria-label="JVM 参数"
          onchange={() => save({ jvmArgs: argsDraft.trim() || null })}
          onkeydown={(e) => e.key === "Enter" && (e.currentTarget as HTMLInputElement).blur()}
        />
      </div>
    </div>
  </section>

  <section class="group">
    <h3>文件</h3>
    <div class="row">
      <div class="lbl">
        <b>版本隔离</b>
        <span>
          {isolated ? "Mod、存档、配置和其他版本分开" : "和其他版本共用 Mod、存档、配置"}
        </span>
      </div>
      <div class="ctl">
        {#if vs.isolation == null}<span class="faint auto">自动判断</span>{/if}
        <button
          class="switch"
          class:on={isolated}
          role="switch"
          aria-checked={isolated}
          aria-label="版本隔离"
          onclick={() => save({ isolation: !isolated })}
        ></button>
      </div>
    </div>
    <div class="row">
      <div class="lbl">
        <b>删除版本</b>
        <span>
          {#if running}游戏正在运行，关掉后才能删除
          {:else if changingLoader}正在更换加载器，完成后才能删除
          {:else if isolated}整个版本文件夹移到回收站，包括它自己的 Mod、存档和配置
          {:else}版本文件夹移到回收站，共用的 Mod 和存档不受影响{/if}
        </span>
      </div>
      <div class="ctl">
        <button class="btn btn-sm del" class:confirm={confirmDelete} onclick={deleteVersion} disabled={deleting || running || changingLoader}>
          {#if deleting}<LoaderCircle size={14} class="spin" />正在删除{:else if confirmDelete}再点一次确认删除{:else}<Trash2 size={14} />删除{/if}
        </button>
      </div>
    </div>
  </section>

  {#if !pack.real}
    <p class="note"><Info size={13} />演示数据，设置不会保存到硬盘</p>
  {/if}
</div>

<style>
  .vs-head {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 12px;
    margin-bottom: 12px;
  }
  .vs-head p {
    display: flex;
    align-items: center;
    gap: 8px;
    font-size: 12px;
    min-width: 0;
  }
  .vs-head-actions {
    display: flex;
    gap: 6px;
    flex-shrink: 0;
  }
  .group {
    margin-bottom: 14px;
  }
  h3 {
    font-size: 12px;
    font-weight: 600;
    color: var(--text-3);
    letter-spacing: 1px;
    margin: 0 4px 6px;
  }
  .row {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 16px;
    padding: 10px 14px;
    min-height: 56px;
    background: var(--surface);
    backdrop-filter: var(--card-filter);
    border: 1px solid var(--border);
    border-bottom-width: 0;
  }
  .row:first-of-type {
    border-radius: var(--r-lg) var(--r-lg) 0 0;
  }
  .row:last-child {
    border-bottom-width: 1px;
    border-radius: 0 0 var(--r-lg) var(--r-lg);
  }
  .row:first-of-type:last-child {
    border-radius: var(--r-lg);
  }
  .row.sub {
    min-height: 44px;
    padding-top: 0;
    border-top: 0;
  }
  .del:not(:disabled):hover,
  .del.confirm {
    border-color: var(--red);
    color: var(--red);
  }
  .del.confirm {
    background: var(--red-soft);
  }
  .lbl {
    display: flex;
    flex-direction: column;
    min-width: 0;
  }
  .lbl b {
    font-size: 14px;
    font-weight: 600;
  }
  .lbl span {
    font-size: 12px;
    color: var(--text-2);
  }
  .ctl {
    display: flex;
    align-items: center;
    gap: 10px;
    flex-shrink: 0;
  }
  .ctl input[type="range"] {
    width: 170px;
  }
  .ctl .input.mono {
    width: 230px;
    font-family: var(--mono);
    font-size: 12px;
  }
  .val {
    min-width: 52px;
    text-align: right;
    font-variant-numeric: tabular-nums;
    font-size: 13px;
    font-weight: 600;
  }
  .auto {
    font-size: 12px;
  }
  .note {
    display: flex;
    align-items: center;
    gap: 6px;
    font-size: 12px;
    color: var(--text-3);
  }
</style>
