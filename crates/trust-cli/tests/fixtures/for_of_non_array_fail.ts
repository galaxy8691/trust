function main(): number {
  let obj: { a: number } = { a: 1 };
  for (const x of obj) {
    return x;
  }
  return 0;
}
