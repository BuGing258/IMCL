/**
 * 云母下的弹出层（下拉框、首页版本菜单）：把它下面的页面内容挖掉，让窗口底下的云母直接透出来。
 *
 * 云母是 Windows 画在整个网页“后面”的；网页自己画的字、卡片都盖在它上面。弹出层下面压着字和卡片时，
 * 半透明 + 背景模糊只能糊这些内容，怎么调都变不成云母。所以弹出层打开时，给页面主体（.app）加一层遮罩，
 * 在弹出层露出来的地方挖一个一样形状的洞——洞里什么都不画，看到的就是系统的云母。
 * 弹出层自己挂在 body 上（不在 .app 里），不会被挖掉；它的底色用卡片材质，看起来和卡片一样是“云母上的一层”。
 * 弹出层滑进滑出时，每一帧按它此刻露出来的位置更新洞；不动了就停下，不占用每一帧。
 * 只在背景材质是云母时这样做（实色背景下面没有云母，挖了只会露出空白）。
 */

interface Entry {
  /** 裁剪框：弹出层最终占的位置，圆角也在它上面 */
  clip: HTMLElement;
  /** 在裁剪框里滑动的内层 */
  sheet: HTMLElement;
}

const entries = new Set<Entry>();
let raf = 0;
let still = 0;
let last = "";

const micaOn = () => document.documentElement.dataset.material === "mica";

/** 弹出层此刻露出来的部分：内层和裁剪框的交集 */
function shown({ clip, sheet }: Entry) {
  const a = sheet.getBoundingClientRect();
  const b = clip.getBoundingClientRect();
  const x = Math.max(a.left, b.left);
  const y = Math.max(a.top, b.top);
  const w = Math.min(a.right, b.right) - x;
  const h = Math.min(a.bottom, b.bottom) - y;
  if (w < 1 || h < 1) return null;
  const r = Math.min(parseFloat(getComputedStyle(clip).borderTopLeftRadius) || 0, w / 2, h / 2);
  return { x, y, w, h, r };
}

/** 一个圆角矩形的 SVG，当遮罩里的“洞” */
const roundRect = (w: number, h: number, r: number) =>
  `url("data:image/svg+xml,${encodeURIComponent(
    `<svg xmlns='http://www.w3.org/2000/svg' width='${w}' height='${h}'><rect width='${w}' height='${h}' rx='${r}' fill='black'/></svg>`,
  )}")`;

/** 按当前位置更新遮罩；返回这一帧有没有变化 */
function apply() {
  const app = document.querySelector<HTMLElement>(".app");
  if (!app) return false;
  const holes = micaOn() ? [...entries].map(shown).filter((h) => h !== null) : [];
  const key = holes.map((h) => `${h.x.toFixed(1)},${h.y.toFixed(1)},${h.w.toFixed(1)},${h.h.toFixed(1)}`).join("|");
  if (key === last) return false;
  last = key;
  const s = app.style;
  if (!holes.length) {
    s.maskImage = s.maskSize = s.maskPosition = s.maskRepeat = s.maskComposite = "";
    return true;
  }
  const o = app.getBoundingClientRect();
  // 洞比弹出层大半个像素：边缘抗锯齿时不会漏出一圈下面的内容（这半像素压在弹出层的边框下）
  const P = 0.5;
  s.maskImage = [...holes.map((h) => roundRect(h.w + 2 * P, h.h + 2 * P, h.r + P)), "linear-gradient(#000 0 0)"].join(", ");
  s.maskSize = [...holes.map((h) => `${h.w + 2 * P}px ${h.h + 2 * P}px`), "100% 100%"].join(", ");
  s.maskPosition = [...holes.map((h) => `${h.x - o.left - P}px ${h.y - o.top - P}px`), "0 0"].join(", ");
  s.maskRepeat = "no-repeat";
  // 每个洞从下面的整块里减掉
  s.maskComposite = "exclude";
  return true;
}

function frame() {
  raf = 0;
  const changed = apply();
  still = changed ? 0 : still + 1;
  // 连续十几帧没动就停；弹出层开始动的时候会再叫醒（poke）
  if (entries.size && still < 12) raf = requestAnimationFrame(frame);
}

/** 弹出层要动了（打开、关闭、换位置）：每帧跟着更新，直到停下 */
export function poke() {
  still = 0;
  if (!raf) raf = requestAnimationFrame(frame);
}

/**
 * Svelte action，用在弹出层的裁剪框上（里面第一个子元素是滑动的内层）。
 * 第一次更新放在下一帧开头：这时进场动画已经开始，读到的是动画起点的位置，不会先闪一帧整块的洞
 */
export function micaHole(clip: HTMLElement) {
  const sheet = clip.firstElementChild as HTMLElement | null;
  if (!sheet) return;
  const e: Entry = { clip, sheet };
  entries.add(e);
  const wake = () => poke();
  sheet.addEventListener("introstart", wake);
  sheet.addEventListener("outrostart", wake);
  poke();
  return {
    destroy() {
      sheet.removeEventListener("introstart", wake);
      sheet.removeEventListener("outrostart", wake);
      entries.delete(e);
      // 弹出层已经拿掉了，同一帧里把洞补上
      apply();
    },
  };
}

/** 把元素挂到 body 上：不被父元素的 overflow 裁掉，也不在 .app 里（不会被自己挖的洞挖掉） */
export function portal(node: HTMLElement) {
  document.body.appendChild(node);
  return { destroy: () => node.remove() };
}
