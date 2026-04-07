function main(): number {
  let n: number = 0;
  if (1 && 2) {
    n = n + 10;
  }
  if (0 || 0) {
    n = n + 100;
  }
  if (0 || 1) {
    n = n + 1;
  }
  if (true && 1) {
    n = n + 1;
  }
  return n;
}
