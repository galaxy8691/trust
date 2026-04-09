function main(): number {
  let sum: number = 0;
  let i: number = 0;
  while (i < 5) {
    i = i + 1;
    if (i == 2) continue;
    sum = sum + i;
  }
  return sum;
}
