class Base {
  seed: number;

  constructor(seed: number) {
    this.seed = seed;
  }
}

class Child extends Base {
  inc: number;

  constructor(seed: number, inc: number) {
    super(seed);
    this.inc = inc;
  }

  child_sum(): number {
    return this.seed + this.inc;
  }
}

function main(): number {
  let c: Child = new Child(2, 5);
  return c.child_sum();
}
