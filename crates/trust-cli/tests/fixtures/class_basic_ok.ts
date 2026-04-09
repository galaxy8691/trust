class Counter {
  value: number;

  constructor(v: number) {
    this.value = v;
  }

  c_add(delta: number): number {
    return this.value + delta;
  }
}

function main(): number {
  let c: Counter = new Counter(2);
  return c.c_add(3);
}
