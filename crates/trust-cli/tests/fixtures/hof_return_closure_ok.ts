function makeAdder(n: number): (x: number) => number {
  return (x: number): number => x + n;
}

function main(): number {
  let add2: (x: number) => number = makeAdder(2);
  return add2(5);
}
