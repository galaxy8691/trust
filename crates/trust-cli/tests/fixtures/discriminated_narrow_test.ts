// 简化测试
interface A { kind: 1; val: number }
type AB = A;

function test(v: AB): number {
  return v.val;
}

function main(): number {
  let a: A = { kind: 1, val: 42 };
  return test(a);
}
