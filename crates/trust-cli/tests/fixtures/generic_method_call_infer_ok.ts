// `p.get(3)` 脱糖为 `get(p, 3)`；`get` 为泛型，由实参推断 `T`。
interface Point {
  x: number;
  y: number;
}

function get<T>(p: Point, v: T): T {
  return v;
}

export function main(): number {
  let p: Point = { x: 1, y: 2 };
  return get(p, 3);
}
