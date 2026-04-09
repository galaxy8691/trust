// if/else 已穷尽返回，后续语句不可达（警告）。
export function main(): number {
  if (1) {
    return 1;
  } else {
    return 2;
  }
  let x: number = 1;
  return x;
}
