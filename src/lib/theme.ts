// 主题色和渐变背景

export const ACCENTS: { name: string; color: string }[] = [
  { name: "红", color: "#ef323d" },
  { name: "橙", color: "#f0701f" },
  { name: "金", color: "#d99a12" },
  { name: "绿", color: "#1fa463" },
  { name: "青", color: "#109fa8" },
  { name: "蓝", color: "#2f7cf6" },
  { name: "紫", color: "#7c5cf0" },
  { name: "粉", color: "#e4508e" },
];

function rgbOf(hex: string): [number, number, number] | null {
  const m = /^#?([0-9a-f]{6})$/i.exec(hex.trim());
  if (!m) return null;
  const n = parseInt(m[1], 16);
  return [(n >> 16) & 255, (n >> 8) & 255, n & 255];
}

/** 主题色上的文字用白色还是黑色 */
export function onColor(hex: string) {
  const c = rgbOf(hex);
  if (!c) return "#ffffff";
  const [r, g, b] = c.map((v) => {
    const s = v / 255;
    return s <= 0.03928 ? s / 12.92 : ((s + 0.055) / 1.055) ** 2.4;
  });
  const lum = 0.2126 * r + 0.7152 * g + 0.0722 * b;
  return lum > 0.45 ? "#1c1c21" : "#ffffff";
}

/** 滑条：把当前位置写进 --p，让左边那段显示主题色 */
export function fill(el: HTMLInputElement, _value?: unknown) {
  const set = () => {
    const min = Number(el.min || 0);
    const max = Number(el.max || 100);
    const p = max > min ? ((Number(el.value) - min) / (max - min)) * 100 : 0;
    el.style.setProperty("--p", `${p}%`);
  };
  set();
  el.addEventListener("input", set);
  return {
    // 值从别处改了（比如重置）也要跟上
    update: () => queueMicrotask(set),
    destroy: () => el.removeEventListener("input", set),
  };
}

/**
 * 只滚动列表本身，把某一项滚进可见范围。
 * 不用 scrollIntoView：它会连带滚动所有外层容器（包括平时不滚动的页面），整页会突然跳一下
 */
export function scrollWithin(list: HTMLElement | null | undefined, item: HTMLElement | null | undefined, block: "nearest" | "center" = "nearest") {
  if (!list || !item) return;
  const l = list.getBoundingClientRect();
  const r = item.getBoundingClientRect();
  const top = r.top - l.top + list.scrollTop;
  if (block === "center") {
    list.scrollTop = top - (list.clientHeight - r.height) / 2;
  } else if (r.top < l.top) {
    list.scrollTop = top;
  } else if (r.bottom > l.bottom) {
    list.scrollTop = top + r.height - list.clientHeight;
  }
}
