// 调试 narrowing
interface A { kind: 1; val: number }
interface B { kind: 2; num: number }
type AB = A | B;

function test(v: AB): number {
  // 这里应该触发 narrowing
  if (v.kind === 1) {
    // 如果 narrowing 生效，这里应该能访问 v.val
    return v.val;
  } else {
    // 如果 narrowing 生效，这里应该能访问 v.num
    return v.num;
  }
}

function main(): number {
  let a: A = { kind: 1, val: 42 };
  return test(a);
}
