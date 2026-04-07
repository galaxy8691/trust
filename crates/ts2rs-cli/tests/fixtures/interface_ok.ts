interface Point {
  x: number;
  y: number;
}

function main(): number {
  let p: Point = { x: 1, y: 2 };
  return p.x + p.y;
}
