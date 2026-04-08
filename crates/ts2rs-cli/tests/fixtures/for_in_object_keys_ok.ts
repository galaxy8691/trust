function main(): number {
  let obj: { a: number; b: number; c: number } = { a: 1, b: 2, c: 3 };
  let n: number = 0;
  for (let k in obj) {
    if (k == "a" || k == "b" || k == "c") {
      n = n + 1;
    }
  }
  return n;
}
