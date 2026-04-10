// 使用 import type 导入类型
import type { Point, Vector } from "./import_type_types.ts";

function main(): number {
  let p: Point = { x: 1, y: 2 };
  let v: Vector = { x: 3, y: 4 };
  return p.x + p.y + v.x + v.y;
}
