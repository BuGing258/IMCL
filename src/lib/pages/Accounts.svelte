<script lang="ts">
  import { slide } from "svelte/transition";
  import { listen } from "@tauri-apps/api/event";
  import { openUrl } from "@tauri-apps/plugin-opener";
  import { Trash2, UserPlus, Info, Globe, KeyRound, Copy, ExternalLink, LoaderCircle, ChevronDown } from "@lucide/svelte";
  import PixelAvatar from "$lib/components/PixelAvatar.svelte";
  import { accounts, toast, settings, isTauri, ACCOUNT_LABELS, type AccountType } from "$lib/state.svelte";
  import { api, type DeviceCode } from "$lib/api";

  let addType = $state<AccountType>("offline");
  let newName = $state("");
  let nameErr = $state("");
  let server = $state("");
  let serverErr = $state("");

  let confirmId = $state<string | null>(null);
  let confirmTimer: ReturnType<typeof setTimeout> | undefined;

  function addOffline(e: SubmitEvent) {
    e.preventDefault();
    nameErr = accounts.validate(newName);
    if (nameErr) return;
    accounts.addOffline(newName);
    toast.show(`已添加并切换到 ${newName.trim()}`);
    newName = "";
  }

  function addAuthlib(e: SubmitEvent) {
    e.preventDefault();
    const s = server.trim();
    if (!s) {
      serverErr = "先填写认证服务器地址";
      return;
    }
    if (!/^https?:\/\/\S+$/.test(s)) {
      serverErr = "地址要以 http:// 或 https:// 开头";
      return;
    }
    serverErr = "";
    toast.show("外置登录还没接入（界面演示）");
  }

  /* ---------- 微软登录 ---------- */
  let msBusy = $state(false);
  let msErr = $state("");
  let code = $state<DeviceCode | null>(null);
  let showClientId = $state(!settings.msClientId);

  function open(url: string) {
    if (isTauri()) openUrl(url).catch((e) => toast.show(String(e)));
    else window.open(url, "_blank");
  }

  async function copyCode() {
    if (!code) return;
    try {
      await navigator.clipboard.writeText(code.userCode);
      toast.show("代码已复制");
    } catch {
      /* 复制不了就手动输入 */
    }
  }

  async function msLogin() {
    msErr = "";
    if (!isTauri()) return toast.show("浏览器预览里不能登录");
    if (!settings.msClientId.trim()) {
      showClientId = true;
      msErr = "先填写 Client ID";
      return;
    }
    msBusy = true;
    code = null;
    const un = await listen<DeviceCode>("ms-device-code", async (e) => {
      code = e.payload;
      // 自动复制代码并打开登录网页，用户只需要粘贴
      await copyCode();
      open(e.payload.verificationUri);
    });
    try {
      const info = await api.msLogin(settings.msClientId.trim());
      accounts.addMicrosoft(info);
      toast.show(`已登录 ${info.name}`);
    } catch (e) {
      const m = String(e);
      if (!m.includes("已取消")) msErr = m;
    } finally {
      un();
      msBusy = false;
      code = null;
    }
  }

  function msCancel() {
    api.msLoginCancel();
  }

  function askRemove(id: string) {
    if (confirmId === id) {
      const name = accounts.list.find((a) => a.id === id)?.name;
      accounts.remove(id);
      confirmId = null;
      toast.show(`已删除 ${name}`);
      return;
    }
    confirmId = id;
    clearTimeout(confirmTimer);
    confirmTimer = setTimeout(() => (confirmId = null), 3000);
  }
</script>

<div class="page-scroll">
  <div class="wrap">
    <div class="page-head">
      <div>
        <h1>账户管理</h1>
        <p>选择进入游戏时用哪个账户</p>
      </div>
    </div>

    <div class="cols">
      <section class="card mine">
        <h2>我的账户</h2>
        <ul class="list">
          {#each accounts.list as a (a.id)}
            {@const isCur = a.id === accounts.currentId}
            <li class:on={isCur} transition:slide={{ duration: 200 }}>
              <PixelAvatar name={a.name} size={40} />
              <div class="a-text">
                <b>{a.name}</b>
                <span class:pro={a.type === "microsoft"}>{a.type === "microsoft" ? "正版账户" : ACCOUNT_LABELS[a.type]}</span>
              </div>
              {#if isCur}
                <span class="badge red">使用中</span>
              {:else}
                <button class="btn btn-sm" onclick={() => (accounts.currentId = a.id)}>使用</button>
              {/if}
              <button
                class="btn btn-sm del"
                class:btn-icon={confirmId !== a.id}
                class:confirm={confirmId === a.id}
                disabled={accounts.list.length <= 1}
                title={accounts.list.length <= 1 ? "至少要保留一个账户" : "删除"}
                aria-label="删除 {a.name}"
                onclick={() => askRemove(a.id)}
              >
                {#if confirmId === a.id}确认删除{:else}<Trash2 size={14} />{/if}
              </button>
            </li>
          {/each}
        </ul>
      </section>

      <section class="card add">
        <h2><UserPlus size={18} />添加账户</h2>
        <div class="seg type">
          <button class:on={addType === "offline"} onclick={() => (addType = "offline")}>离线</button>
          <button class:on={addType === "microsoft"} onclick={() => (addType = "microsoft")}>微软</button>
          <button class:on={addType === "authlib"} onclick={() => (addType = "authlib")}>外置登录</button>
        </div>

        {#if addType === "offline"}
          <form onsubmit={addOffline} novalidate>
            <p class="hint">不需要联网，适合单人和局域网游戏。名字会显示在游戏里。</p>
            <div class="field">
              <input
                class="input"
                class:invalid={!!nameErr}
                placeholder="Alex_2024"
                maxlength="16"
                bind:value={newName}
                oninput={() => (nameErr = "")}
                aria-label="离线用户名"
              />
              <button class="btn btn-primary" type="submit">添加</button>
            </div>
            {#if nameErr}<p class="err-text">{nameErr}</p>{/if}
          </form>
        {:else if addType === "microsoft"}
          <p class="hint">用购买了 Minecraft 的微软账户登录，可以进正版服务器，显示正版皮肤。</p>
          {#if code}
            <div class="code-box">
              <p>在打开的网页里输入这个代码（已经复制好了）：</p>
              <button class="code" onclick={copyCode} title="点击复制">{code.userCode}<Copy size={15} /></button>
              <div class="code-actions">
                <button class="btn btn-sm" onclick={() => code && open(code.verificationUri)}><ExternalLink size={14} />重新打开网页</button>
                <button class="btn btn-sm" onclick={msCancel}>取消</button>
              </div>
              <p class="faint wait"><LoaderCircle size={13} class="spin" />网页上登录完成后会自动回到这里</p>
            </div>
          {:else}
            <button class="btn btn-primary wide" onclick={msLogin} disabled={msBusy}>
              {#if msBusy}<LoaderCircle size={14} class="spin" />正在连接微软…{:else}<KeyRound size={14} />使用微软账户登录{/if}
            </button>
          {/if}
          {#if msErr}<p class="err-text">{msErr}</p>{/if}

          <button class="fold" onclick={() => (showClientId = !showClientId)}>
            <ChevronDown size={14} class={showClientId ? "rot" : ""} />Client ID 设置
            {#if settings.msClientId}<span class="badge green">已填写</span>{:else}<span class="badge amber">未填写</span>{/if}
          </button>
          {#if showClientId}
            <div class="cid" transition:slide={{ duration: 180 }}>
              <input
                class="input mono"
                placeholder="00000000-0000-0000-0000-000000000000"
                bind:value={settings.msClientId}
                spellcheck="false"
                aria-label="Client ID"
              />
              <p class="faint">
                微软要求每个启动器用自己的 Client ID，并经过 Mojang 批准才能登录 Minecraft。
                在 Azure 注册一个应用（支持个人账户、开启“公共客户端流”），再提交 Mojang 的申请表，批下来后把 ID 填到这里。
              </p>
              <div class="links">
                <button class="link" onclick={() => open("https://portal.azure.com/#view/Microsoft_AAD_RegisteredApps/ApplicationsListBlade")}>
                  Azure 应用注册<ExternalLink size={11} />
                </button>
                <button class="link" onclick={() => open("https://aka.ms/mce-reviewappid")}>Mojang 申请表<ExternalLink size={11} /></button>
              </div>
            </div>
          {/if}
        {:else}
          <form onsubmit={addAuthlib} novalidate>
            <p class="hint">用皮肤站之类的第三方认证服务器登录（authlib-injector）。</p>
            <div class="field">
              <input
                class="input"
                class:invalid={!!serverErr}
                placeholder="https://example.com/api/yggdrasil"
                bind:value={server}
                oninput={() => (serverErr = "")}
                aria-label="认证服务器地址"
              />
              <button class="btn btn-primary" type="submit"><Globe size={14} />下一步</button>
            </div>
            {#if serverErr}<p class="err-text">{serverErr}</p>{/if}
          </form>
        {/if}

        <p class="note"><Info size={13} />账户数据只保存在这台电脑上，正版登录令牌不会发给任何第三方</p>
      </section>
    </div>
  </div>
</div>

<style>
  .wrap {
    max-width: 860px;
    margin: 0 auto;
  }
  .cols {
    display: grid;
    grid-template-columns: minmax(0, 1.1fr) minmax(0, 1fr);
    gap: 14px;
    align-items: start;
  }
  .card {
    padding: 16px 18px;
    animation: rise 0.45s var(--ease) backwards;
  }
  .add {
    animation-delay: 70ms;
  }
  h2 {
    display: flex;
    align-items: center;
    gap: 8px;
    font-size: 15px;
    font-weight: 700;
    margin-bottom: 12px;
  }
  .list {
    list-style: none;
    margin: 0;
    padding: 0;
    display: flex;
    flex-direction: column;
    gap: 4px;
  }
  .list li {
    display: flex;
    align-items: center;
    gap: 12px;
    padding: 8px 10px;
    border-radius: var(--r);
    border: 1px solid transparent;
    transition: background 0.15s, border-color 0.15s;
  }
  .list li:hover {
    background: var(--hover);
  }
  .list li.on {
    border-color: var(--red);
    background: var(--red-soft);
  }
  .a-text {
    flex: 1;
    min-width: 0;
    display: flex;
    flex-direction: column;
  }
  .a-text b {
    font-size: 14px;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .a-text span {
    font-size: 12px;
    color: var(--text-2);
  }
  .del.confirm {
    border-color: var(--red);
    color: var(--red);
  }
  .type {
    width: 100%;
    margin-bottom: 12px;
  }
  .type button {
    flex: 1;
  }
  .hint {
    font-size: 12px;
    color: var(--text-2);
    margin-bottom: 10px;
  }
  .field {
    display: flex;
    gap: 8px;
  }
  .field .input {
    flex: 1;
    min-width: 0;
  }
  .wide {
    width: 100%;
  }
  .a-text span.pro {
    color: var(--green);
    font-weight: 600;
  }
  .code-box {
    padding: 12px;
    border-radius: var(--r);
    background: var(--bg);
    border: 1px solid var(--border);
    font-size: 12px;
    color: var(--text-2);
    display: flex;
    flex-direction: column;
    gap: 8px;
  }
  .code {
    align-self: center;
    display: inline-flex;
    align-items: center;
    gap: 10px;
    padding: 6px 14px;
    border-radius: var(--r);
    background: var(--surface);
    border: 1px dashed var(--red);
    color: var(--red);
    font-family: var(--mono);
    font-size: 24px;
    font-weight: 700;
    letter-spacing: 3px;
  }
  .code-actions {
    display: flex;
    justify-content: center;
    gap: 8px;
  }
  .wait {
    display: flex;
    align-items: center;
    justify-content: center;
    gap: 6px;
  }
  .fold {
    display: flex;
    align-items: center;
    gap: 6px;
    margin-top: 14px;
    font-size: 12px;
    color: var(--text-2);
  }
  .fold :global(svg) {
    transition: transform 0.2s var(--ease);
    transform: rotate(-90deg);
  }
  .fold :global(svg.rot) {
    transform: none;
  }
  .cid {
    display: flex;
    flex-direction: column;
    gap: 6px;
    margin-top: 8px;
  }
  .cid .input {
    width: 100%;
  }
  .mono {
    font-family: var(--mono);
    font-size: 12px;
  }
  .cid p {
    font-size: 11.5px;
    line-height: 1.6;
  }
  .links {
    display: flex;
    gap: 14px;
  }
  .link {
    display: inline-flex;
    align-items: center;
    gap: 3px;
    font-size: 12px;
    color: var(--red);
  }
  .link:hover {
    text-decoration: underline;
  }
  .note {
    display: flex;
    align-items: center;
    gap: 6px;
    margin-top: 16px;
    font-size: 12px;
    color: var(--text-3);
  }
</style>
