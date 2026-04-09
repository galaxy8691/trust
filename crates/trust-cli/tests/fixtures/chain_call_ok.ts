// 链式：`f().prop` 与 `f().m()` 的 receiver 为调用表达式返回值。
interface Point {
  x: number;
  y: number;
}

function make(): Point {
  return { x: 1, y: 2 };
}

function sum_xy(p: Point): number {
  return p.x + p.y;
}

export function main(): number {
  // 成员链 `make().x` 与 方法链 `make().sum_xy()` 均一层。
  return make().x + make().y + make().sum_xy();
}
