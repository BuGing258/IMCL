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
 * 内层：从外层上沿滑进来（up = true 时从下沿）。
 * 多滑出 2px：正好停在 -100% 时，菜单的 1px 浅色边框压在裁剪边上，抗锯齿会留下一条白色细线
 */
export function sheetIn(
  _node: Element,
  { duration = 240, up = false, shadow }: { duration?: number; up?: boolean; shadow?: string } = {},
) {
  const dir = up ? 1 : -1;
  return {
    duration,
    easing: (t: number) => 1 - (1 - t) ** 4,
    css: (t: number, u: number) =>
      `transform: translateY(calc(${(u * dir * 100).toFixed(2)}% + ${(u * dir * 2).toFixed(2)}px));` +
      (shadow ? ` box-shadow: ${fadeShadow(shadow, t)};` : ""),
  };
}

/**
 * 阴影层用：阴影浓淡跟着菜单露出的多少走。
 * 阴影会伸到阴影层下沿外面几十像素，菜单还完全藏着时这截阴影就已经露在裁剪框里了，
 * 顶上被裁成一条直边——看起来像一帧“截断的阴影”。按进度把透明度从 0 拉到原值就没有了
 */
function fadeShadow(shadow: string, k: number) {
  return shadow.replace(/rgba\(([^)]*?),\s*([\d.]+)\)/g, (_m, c, a) => `rgba(${c}, ${(Number(a) * Math.min(1, Math.max(0, k))).toFixed(3)})`);
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
