function main(): number {
  let x: number = 3;
  let n: number = 0;
  for (let k in x) {
    n = n + 1;
  }
  return n;
}
