function main(): number {
  let arr: number[] = [1, 2, 3];
  let sum: number = 0;
  for (const x of arr) {
    sum = sum + x;
  }
  return sum;
}
