import { poke } from "./micaHole";

// 动画曲线：全部用非线性曲线，进场快、收尾柔，弹出类带一点回弹。
// CSS 里对应的变量在 app.css：--ease / --ease-spring / --ease-in / --ease-io
export { quintOut as smooth, cubicIn as accel, cubicInOut as inOut } from "svelte/easing";

/** 轻微回弹（超出一点再回来），弹窗、提示条进场用 */
export function pop(t: number) {
  const s = 1.15;
  const u = t - 1;
  return u * u * ((s + 1) * u + s) + 1;
}

/*
 * 下拉菜单展开（和 Win11 的菜单一样，从上沿滑出来）。
 *
 * 带背景模糊（backdrop-filter）的元素，在做 opacity 或 clip-path 动画时浏览器不计算模糊，
 * 动画结束才突然出现：动画过程中菜单是“半透明但没模糊”，下面的红色开始游戏卡片会透上来。
 * 所以拆成三层：
 *   外层：只定位，自己不动；
 *   阴影层（shade）：和 sheet 一起平移，套在一个不动的 shade-clip 里，伸出菜单上沿的部分被裁掉，
 *     阴影跟着菜单往下长（不能用 clip-path 直接裁 shade：clip-path 跟着元素自己一起移动，等于没裁）；
 *     shade-clip 不是 sheet 的祖先，不影响 sheet 的模糊；
 *   裁剪层：只负责圆角裁剪（overflow: hidden，不会影响模糊）；
 *   sheet：材质（底色 + 模糊）都在它身上，只用 transform 平移进来——transform 动画不影响模糊，第一帧就是完整的材质。
 */

/**
 * 下拉菜单的滑进滑出，用在弹出层的外层上（里面要有 .sheet，可选 .shade）。
 *
 * 不用 Svelte 的 transition：它把曲线按 60Hz 采成十几个关键帧，浏览器在关键帧之间走直线，
 * 240Hz 屏幕上每 4 帧速度跳一档，看起来一顿一顿的；关闭时还是“先快后慢”，最后一截细条拖很久，
 * 而且看不见了之后还要再等几十毫秒才拿掉（逐帧检查见 2026-11 的记录）。
 * 这里直接用浏览器的动画（WAAPI）：只给起点、终点和 cubic-bezier 曲线，每一帧都由浏览器精确插值。
 *   打开：先快后慢，220ms；关闭：起步柔、越走越快，160ms，滑到裁剪边正好消失，马上 onclosed。
 *   中途反向（关到一半又打开，或反过来）：从当前位置接着走，时长按剩下的距离折算。
 *   多滑出 2px：停在 -100% 时菜单的 1px 浅色边框压在裁剪边上，抗锯齿会留下一条细线。
 *   阴影层用 opacity 跟着淡入淡出（阴影层没有背景模糊，opacity 动画不影响任何东西）。
 */
const OPEN = { duration: 220, easing: "cubic-bezier(0.25, 1, 0.5, 1)" };
// 关闭用标准加速曲线：一按就看得出在动（前 50ms 走两成），越走越快，滑到边上利落地消失。
// 更陡的加速曲线前 50ms 几乎不动，像点了没反应
const CLOSE = { duration: 160, easing: "cubic-bezier(0.3, 0, 1, 1)" };

export function dropSheet(node: HTMLElement, params: { open: boolean; up?: boolean; onclosed?: () => void }) {
  const found = node.querySelector<HTMLElement>(".sheet");
  const shade = node.querySelector<HTMLElement>(".shade");
  if (!found) return;
  const sheet: HTMLElement = found;
  let p = params;
  let anims: Animation[] = [];
  let isOpen = false;

  /** 完全藏起来时的位置（px） */
  const hidden = () => (p.up ? 1 : -1) * (sheet.offsetHeight + 2);
  /** 现在的位置：正在动就取动画此刻的值 */
  const current = () => {
    const t = getComputedStyle(sheet).transform;
    return t === "none" ? 0 : new DOMMatrixReadOnly(t).m42;
  };

  function run(open: boolean) {
    isOpen = open;
    const h = hidden();
    const from = anims.length ? current() : open ? h : 0;
    const to = open ? 0 : h;
    for (const a of anims) a.cancel();
    const left = Math.abs(to - from) / Math.abs(h);
    const c = open ? OPEN : CLOSE;
    const opts: KeyframeAnimationOptions = { duration: c.duration * Math.max(0.35, left), easing: c.easing, fill: "forwards" };
    const shown = (y: number) => 1 - Math.min(1, Math.abs(y / h));
    anims = [sheet.animate([{ transform: `translateY(${from}px)` }, { transform: `translateY(${to}px)` }], opts)];
    if (shade)
      anims.push(
        shade.animate(
          [
            { transform: `translateY(${from}px)`, opacity: shown(from) },
            { transform: `translateY(${to}px)`, opacity: shown(to) },
          ],
          opts,
        ),
      );
    if (!open) anims[0].onfinish = () => p.onclosed?.();
    poke();
  }

  if (p.open) run(true);
  return {
    update(next: { open: boolean; up?: boolean; onclosed?: () => void }) {
      p = next;
      if (next.open !== isOpen) run(next.open);
    },
    destroy() {
      for (const a of anims) a.cancel();
    },
  };
}

/**
 * 带材质的弹窗进出场：只缩放。
 * 不碰 opacity / 遮罩（理由同上），背景模糊从第一帧起就是完整的。用 pop 这种会超过 1 的曲线时，缩放可以回弹
 */
export function materialPop(
  _node: HTMLElement,
  { duration = 300, start = 0.94, easing = (t: number) => 1 - (1 - t) ** 4 }: { duration?: number; start?: number; easing?: (t: number) => number } = {},
) {
  return {
    duration,
    easing,
    css: (t: number) => `transform: scale(${start + (1 - start) * t});`,
  };
}
