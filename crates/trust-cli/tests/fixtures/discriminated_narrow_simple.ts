// 最简单的 narrowing 测试
interface A { kind: 1; val: number }
interface B { kind: 2; num: number }
type AB = A | B;

function main(): number {
  let v: AB = { kind: 1, val: 42 };
  
  // 测试联合类型成员访问
  let k: 1 | 2 = v.kind;
  
  if (v.kind === 1) {
    return v.val;
  }
  
  return 0;
}
