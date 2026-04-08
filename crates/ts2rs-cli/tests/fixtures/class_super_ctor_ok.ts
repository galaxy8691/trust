class P {
  n: number;

  constructor(n: number) {
    this.n = n;
  }
}

class C extends P {
  m: number;

  constructor(n: number, m: number) {
    super(n);
    this.m = m;
  }
}

function main(): number {
  let c: C = new C(3, 4);
  return c.m + 3;
}
