class Parent {
  v: number;

  constructor(v: number) {
    this.v = v;
  }

}

class ChildBad extends Parent {
  constructor(v: number) {
    super(v);
  }

  override score(x: number): boolean {
    return x > 0;
  }
}

function main(): number {
  let c: ChildBad = new ChildBad(1);
  return 0;
}
