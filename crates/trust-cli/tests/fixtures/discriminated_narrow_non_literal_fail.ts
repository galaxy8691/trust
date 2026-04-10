// D1: Discriminated narrowing 负例
// 条件不是字面量时，不应该收窄

interface A { kind: 1; val: number }
interface B { kind: 2; num: number }
type AB = A | B;

function test(v: AB, dynamicKind: number): number {
  // 错误：条件是变量不是字面量
  if (v.kind === dynamicKind) {
    // 无法收窄，v 仍是 AB，访问 v.val 应该失败（因为 v 可能是 B）
    return v.val;
  }
  return 0;
}

function main(): number {
  return 0;
}
