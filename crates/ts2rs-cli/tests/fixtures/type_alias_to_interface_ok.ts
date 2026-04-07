interface Point {
  x: number;
  y: number;
}

type P = Point;

function main(): number {
  let p: P = { x: 1, y: 2 };
  return p.x + p.y;
}
