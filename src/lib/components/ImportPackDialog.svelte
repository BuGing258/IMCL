<script lang="ts">
  // 从本地 .mrpack 文件导入整合包：看看里面是什么，填个版本名，然后像下载的整合包一样安装
  import Modal from "./Modal.svelte";
  import { X, LoaderCircle, PackagePlus } from "@lucide/svelte";
  import { api, checkVersionName, type PackInfo } from "$lib/api";
  import { installs, library, toast } from "$lib/state.svelte";

  let { file, onclose }: { file: string; onclose: () => void } = $props();

  const fileName = $derived(file.split(/[\\/]/).pop() ?? file);
  let info = $state<PackInfo | null>(null);
  let err = $state("");
  let name = $state("");
  let touched = $state(false);

  const existing = $derived(library.packs.map((p) => p.name));
  const nameErr = $derived(checkVersionName(name, existing));

  // 窗口打开时读一次（换文件会重新打开窗口）
  // svelte-ignore state_referenced_locally
  api
    .readModpack(file)
    .then((i) => {
      info = i;
      const base = (i.name || fileName.replace(/\.(mrpack|zip)$/i, "")).replace(/[\\/:*?"<>|]/g, " ").replace(/\s+/g, " ").trim().slice(0, 60);
      let n = base;
      for (let k = 2; existing.includes(n); k++) n = `${base} (${k})`;
      name = n;
    })
    .catch((e) => (err = String(e)));

  function go() {
    touched = true;
    if (!info || nameErr) return;
    installs.startModpack({
      name: name,
      mc: info.mc,
      loader: info.loader ? info.loader.split(" ")[0].toLowerCase() : null,
      projectId: `file:${file}`,
      versionId: "",
      title: info.name || fileName,
      file,
    });
    toast.show(`开始导入 ${info.name || fileName}，完成后会出现在版本管理里`);
    onclose();
  }

  function onKey(e: KeyboardEvent) {
    if (e.key === "Enter" && info) go();
  }
</script>

<svelte:window onkeydowncapture={onKey} />

<Modal {onclose} width={480} labelledby="im-title">
    <header>
      <span class="ic"><PackagePlus size={18} /></span>
      <div class="ht">
        <h2 id="im-title">导入整合包</h2>
        <p title={file}>{fileName}</p>
      </div>
      <button class="btn btn-sm btn-icon" onclick={onclose} aria-label="关闭"><X size={15} /></button>
    </header>

    {#if err}
      <p class="err-text">{err}</p>
    {:else if !info}
      <p class="state"><LoaderCircle size={15} class="spin" />正在读取整合包…</p>
    {:else}
      <dl>
        <dt>整合包</dt>
        <dd>{info.name || "（没写名字）"}{info.version ? ` ${info.version}` : ""}</dd>
        <dt>游戏版本</dt>
        <dd>Minecraft {info.mc}{info.loader ? ` · ${info.loader}` : " · 原版"}</dd>
        {#if info.summary}<dt>简介</dt><dd>{info.summary}</dd>{/if}
      </dl>
      <label class="f">
        <span>版本名称</span>
        <input class="input" class:invalid={touched && !!nameErr} bind:value={name} oninput={() => (touched = true)} maxlength="64" spellcheck="false" />
      </label>
      {#if touched && nameErr}<p class="err-text">{nameErr}</p>{/if}
    {/if}

    <footer>
      <span class="sp"></span>
      <button class="btn btn-sm" onclick={onclose}>取消</button>
      <button class="btn btn-sm btn-primary" onclick={go} disabled={!info || (touched && !!nameErr)}>导入</button>
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
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  dl {
    display: grid;
    grid-template-columns: 64px 1fr;
    gap: 6px 12px;
    margin: 0;
    padding: 10px 12px;
    border-radius: var(--r);
    background: var(--bg);
    border: 1px solid var(--border);
    font-size: 12.5px;
  }
  dt {
    color: var(--text-3);
  }
  dd {
    margin: 0;
    min-width: 0;
    word-break: break-word;
  }
  .f {
    display: flex;
    flex-direction: column;
    gap: 4px;
    font-size: 12px;
    font-weight: 600;
    color: var(--text-2);
  }
  .f .input {
    font-weight: 400;
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
  footer {
    display: flex;
    align-items: center;
    gap: 8px;
  }
  .sp {
    flex: 1;
  }
</style>
