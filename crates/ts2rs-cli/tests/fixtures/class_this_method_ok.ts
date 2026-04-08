class Acc {
  base: number;

  constructor(x: number) {
    this.base = x;
  }

  acc_twice(): number {
    return this.base + this.base;
  }
}

function main(): number {
  let a: Acc = new Acc(4);
  return a.acc_twice();
}
