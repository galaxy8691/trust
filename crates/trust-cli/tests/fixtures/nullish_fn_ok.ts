type MaybeFn = null | (x: number) => number;

function run(f: MaybeFn, d: (x: number) => number): number {
  let g: (x: number) => number = f ?? d;
  return g(4);
}

function main(): number {
  return run(null, (x: number): number => x + 1);
}
