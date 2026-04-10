// D1: Discriminated narrowing 基本测试
// 使用 number 字面量作为 discriminant

interface A { kind: 1; val: number }
interface B { kind: 2; num: number }
type AB = A | B;

function test(v: AB): number {
  if (v.kind === 1) {
    // v 应该被收窄为 A，可以访问 v.val
    return v.val;
  } else {
    // v 应该被收窄为 B，可以访问 v.num
    return v.num;
  }
}

function main(): number {
  let a: A = { kind: 1, val: 42 };
  return test(a);
}
