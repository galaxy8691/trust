function main(): number {
  let arr: number[] = [10, 20, 30];
  let s: number = 0;
  for (let k in arr) {
    if (k == "0") {
      s = s + 1;
    } else if (k == "1") {
      s = s + 2;
    } else if (k == "2") {
      s = s + 3;
    }
  }
  return s;
}
