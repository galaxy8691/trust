// R1: Interface 名义方法测试

interface Point {
  x: number;
  y: number;
  distance(other: Point): number;
}

function usePoint(p: Point): number {
  let q: Point = { x: 4, y: 6 };
  return p.distance(q);
}

function main(): number {
  let p: Point = { x: 1, y: 2 };
  return usePoint(p);
}
