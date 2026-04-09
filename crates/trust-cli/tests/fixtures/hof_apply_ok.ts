function apply(f: (x: number) => number, x: number): number {
  return f(x);
}

function main(): number {
  return apply((x: number): number => x + 2, 3);
}
