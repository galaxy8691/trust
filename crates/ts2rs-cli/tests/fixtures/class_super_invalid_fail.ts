class BadBase {
  x: number;

  constructor(x: number) {
    this.x = x;
  }
}

class BadChild extends BadBase {
  y: number;

  constructor(x: number, y: number) {
    this.y = y;
  }
}

function main(): number {
  let b: BadChild = new BadChild(1, 2);
  return b.x + b.y;
}
