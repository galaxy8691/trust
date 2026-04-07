function main(): number {
  let a: 42 = 42;
  let b: number = 3;
  let s: "ok" = "ok";
  let flag: true = true;
  let pad: number = s.length + (flag ? 1 : 0);
  return a - 40 + b + pad;
}
