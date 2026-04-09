function main(): number {
  let obj: { a: number } = { a: 1 };
  let sum: number = 0;
  for (let k in obj) {
    let n: number = k;
    sum = sum + n;
  }
  return sum;
}
