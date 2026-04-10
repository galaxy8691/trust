// D1: Discriminated narrowing else 分支测试
// 使用 number 字面量作为 discriminant

interface A { kind: 1; val: number }
interface B { kind: 2; num: number }
interface C { kind: 3; count: number }
type ABC = A | B | C;

function test(v: ABC): number {
  if (v.kind === 1) {
    return v.val;
  } else {
    // v 应该被收窄为 B | C
    if (v.kind === 2) {
      return v.num;
    } else {
      // v 应该被收窄为 C
      return v.count;
    }
  }
}

function main(): number {
  let c: C = { kind: 3, count: 100 };
  return test(c);
}
