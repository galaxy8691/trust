// R1: Interface 方法参数类型不匹配（应报错）

interface Point {
  x: number;
  y: number;
  distance(other: Point): number;
}

function usePoint(p: Point): number {
  // 错误：传入 number 而不是 Point
  return p.distance(42);
}

function main(): number {
  return 1;
}
