// `obj.m()` 脱糖为全局函数 `m(obj, ...)`；与严格 tsc 对 interface 成员的检查可能不一致（trust 扩展）。
interface Point {
  x: number;
  y: number;
}

function sum_xy(p: Point): number {
  return p.x + p.y;
}

export function main(): number {
  let p: Point = { x: 1, y: 2 };
  return p.sum_xy();
}
