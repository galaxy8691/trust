function main(): number {
  let f: true | false = true;
  if (f) {
    let n: 0 | 1 = 1;
    if (n) {
      return 2;
    }
  }
  return 0;
}
