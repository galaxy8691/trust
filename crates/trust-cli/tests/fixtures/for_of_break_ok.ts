function main(): number {
  let arr: number[] = [1, 2, 3, 4, 5];
  let sum: number = 0;
  for (const x of arr) {
    if (x > 2) {
      break;
    }
    sum = sum + x;
  }
  return sum;
}
