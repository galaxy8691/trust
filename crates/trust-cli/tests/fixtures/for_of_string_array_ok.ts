function main(): number {
  let arr: string[] = ["a", "b", "c"];
  let len: number = 0;
  for (const s of arr) {
    len = len + s.length;
  }
  return len;
}
