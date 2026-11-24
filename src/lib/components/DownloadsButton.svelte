<script lang="ts">
  // 标题栏的下载按钮（最小化左边）：点开是下载面板，安装游戏、导入 / 安装整合包、下载 Java、换加载器的进度都在这里。
  // 有任务在跑时按钮外圈显示总进度；有失败的任务时右上角一个红点
  import { Download, X, Check, Box, Package, Coffee, Wrench, CircleAlert } from "@lucide/svelte";
  import Popover from "./Popover.svelte";
  import { installs, taskPct, taskKind, type InstallTask } from "$lib/state.svelte";
  import { fmtBytes, fmtRelative } from "$lib/api";

  const list = $derived(Object.values(installs.tasks));
  const running = $derived(list.filter((t) => t.status !== "error"));
  const failed = $derived(list.some((t) => t.status === "error"));
  /** 外圈：所有进行中任务的平均进度 */
  const ring = $derived(running.length ? running.reduce((a, t) => a + taskPct(t), 0) / running.length : 0);
  const R = 9;
  const C = 2 * Math.PI * R;

  const iconOf = (t: InstallTask) => (t.java ? Coffee : t.loaderFor ? Wrench : t.modpack ? Package : Box);
</script>

<Popover align="end" width="360px" pad="8px" role="dialog" label="下载">
  {#snippet trigger({ open, toggle })}
    <button
      class="dl-btn"
      class:open
      class:busy={running.length > 0}
      onclick={toggle}
      aria-label="下载进度"
      title={running.length ? `${running.length} 个任务进行中` : "下载进度"}
    >
      <span class="dl-icon">
        <svg viewBox="0 0 24 24" aria-hidden="true">
          {#if running.length}
            <circle class="track" cx="12" cy="12" r={R} />
            <circle class="arc" cx="12" cy="12" r={R} stroke-dasharray="{(C * Math.max(4, ring)) / 100} {C}" />
          {/if}
        </svg>
        <Download size={running.length ? 11 : 15} />
      </span>
      {#if failed}<span class="dot"></span>{/if}
    </button>
  {/snippet}

  {#snippet children()}
    <div class="head">
      <b>下载</b>
      <span class="faint">{running.length ? `${running.length} 个进行中` : "没有进行中的任务"}</span>
    </div>

    {#each list as t (t.id)}
      {@const Icon = iconOf(t)}
      {@const bad = t.status === "error"}
      {@const pct = taskPct(t)}
      <div class="task" class:bad>
        <span class="ic">{#if bad}<CircleAlert size={16} />{:else}<Icon size={16} />{/if}</span>
        <div class="body">
          <div class="top">
            <span class="name" title={t.name}>{t.name}</span>
            <span class="kind">{taskKind(t)}</span>
          </div>
          {#if bad}
            <p class="err">{t.error}</p>
          {:else}
            <div class="bar" class:indet={t.stageIndex === 0}><i style:width="{pct}%"></i></div>
            <div class="meta">
              <span class="stage" title={t.stage}>
                {t.status === "cancelling" ? "正在取消…" : t.stageIndex ? `(${t.stageIndex}/${t.stageCount}) ${t.stage}` : t.stage}
              </span>
              <span class="num">
                {#if t.total > 0}{t.done}/{t.total}{/if}{#if t.speed > 1024}{t.total > 0 ? " · " : ""}{fmtBytes(t.speed)}/s{/if}
              </span>
              <b class="pct">{pct}%</b>
            </div>
          {/if}
        </div>
        <button
          class="btn btn-sm btn-icon"
          onclick={() => installs.cancel(t.id)}
          disabled={t.status === "cancelling"}
          aria-label={bad ? "关闭" : "取消"}
          title={bad ? "关闭" : "取消"}
        >
          <X size={13} />
        </button>
      </div>
    {/each}

    {#if installs.recent.length}
      <div class="sec">
        <span>最近完成</span>
        <button class="link" onclick={() => installs.clearRecent()}>清除</button>
      </div>
      {#each installs.recent as r (r.id)}
        <div class="done">
          <Check size={14} />
          <span class="d-name" title={r.name}>{r.kind} · {r.name}</span>
          <span class="faint">{fmtRelative(r.at)}</span>
        </div>
      {/each}
    {/if}

    {#if !list.length && !installs.recent.length}
      <p class="empty">安装游戏、导入整合包、下载 Java 的进度都会显示在这里</p>
    {/if}
  {/snippet}
</Popover>

<style>
  /* 和旁边的最小化、关闭按钮一样大小 */
  .dl-btn {
    position: relative;
    width: 46px;
    height: 40px;
    display: flex;
    align-items: center;
    justify-content: center;
    color: var(--text-2);
    transition: background 0.12s var(--ease), color 0.12s var(--ease);
  }
  .dl-btn:hover,
  .dl-btn.open {
    background: var(--hover);
    color: var(--text);
  }
  .dl-btn.busy {
    color: var(--red);
  }
  .dl-icon {
    position: relative;
    width: 24px;
    height: 24px;
    display: flex;
    align-items: center;
    justify-content: center;
  }
  .dl-icon svg {
    position: absolute;
    inset: 0;
    transform: rotate(-90deg);
  }
  .track,
  .arc {
    fill: none;
    stroke-width: 2.2;
  }
  .track {
    stroke: var(--border-strong);
  }
  .arc {
    stroke: var(--red);
    stroke-linecap: round;
    transition: stroke-dasharray 0.3s var(--ease);
  }
  .dot {
    position: absolute;
    top: 9px;
    right: 12px;
    width: 7px;
    height: 7px;
    border-radius: 50%;
    background: var(--red);
    box-shadow: 0 0 0 2px var(--bg-solid);
  }

  .head {
    display: flex;
    align-items: baseline;
    justify-content: space-between;
    padding: 4px 6px 8px;
    font-size: 13px;
  }
  .head .faint {
    font-size: 12px;
  }
  .task {
    display: flex;
    align-items: flex-start;
    gap: 10px;
    padding: 10px;
    border-radius: 10px;
    background: var(--hover);
  }
  .task + .task {
    margin-top: 6px;
  }
  .ic {
    flex-shrink: 0;
    width: 30px;
    height: 30px;
    border-radius: 8px;
    display: flex;
    align-items: center;
    justify-content: center;
    background: var(--red-soft);
    color: var(--red);
  }
  .task.bad .ic {
    background: color-mix(in srgb, var(--red) 22%, transparent);
  }
  .body {
    flex: 1;
    min-width: 0;
  }
  .top {
    display: flex;
    align-items: baseline;
    gap: 8px;
    font-size: 13px;
  }
  .name {
    flex: 1;
    min-width: 0;
    font-weight: 600;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .kind {
    flex-shrink: 0;
    font-size: 11.5px;
    color: var(--text-3);
  }
  .bar {
    height: 5px;
    margin: 8px 0 6px;
    border-radius: 3px;
    background: var(--border-strong);
    overflow: hidden;
  }
  .bar i {
    display: block;
    height: 100%;
    border-radius: 3px;
    background: var(--red);
    transition: width 0.3s var(--ease);
  }
  .bar.indet i {
    width: 30% !important;
    animation: indet 1.2s var(--ease-io) infinite;
  }
  @keyframes indet {
    from {
      transform: translateX(-100%);
    }
    to {
      transform: translateX(340%);
    }
  }
  .meta {
    display: flex;
    align-items: center;
    gap: 8px;
    font-size: 11.5px;
    color: var(--text-2);
  }
  .stage {
    flex: 1;
    min-width: 0;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .num {
    flex-shrink: 0;
    font-variant-numeric: tabular-nums;
  }
  .pct {
    flex-shrink: 0;
    width: 32px;
    text-align: right;
    color: var(--text);
    font-variant-numeric: tabular-nums;
  }
  .err {
    margin-top: 4px;
    font-size: 12px;
    line-height: 1.5;
    color: var(--red);
    max-height: 72px;
    overflow-y: auto;
    user-select: text;
    word-break: break-all;
  }
  .sec {
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: 12px 6px 4px;
    font-size: 12px;
    color: var(--text-3);
  }
  .link {
    font-size: 12px;
    color: var(--text-3);
    padding: 2px 6px;
    border-radius: 6px;
  }
  .link:hover {
    color: var(--text);
    background: var(--hover);
  }
  .done {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 6px;
    font-size: 12.5px;
    color: var(--green);
  }
  .d-name {
    flex: 1;
    min-width: 0;
    color: var(--text);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .done .faint {
    font-size: 11.5px;
    flex-shrink: 0;
  }
  .empty {
    padding: 14px 10px 16px;
    font-size: 12.5px;
    line-height: 1.6;
    color: var(--text-3);
    text-align: center;
  }
</style>
