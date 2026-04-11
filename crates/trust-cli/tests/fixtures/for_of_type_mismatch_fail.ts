// 类型不匹配：声明为 string[] 但按 number 使用
function main(): number {
  let arr: string[] = ["a", "b"];
  let sum: number = 0;
  for (const x of arr) {
    sum = sum + x;  // x is string, cannot add to number
  }
  return sum;
}
