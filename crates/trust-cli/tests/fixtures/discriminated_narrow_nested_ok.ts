// D1: Discriminated narrowing 嵌套测试

interface A { kind: 1; val: number }
interface B { kind: 2; num: number }
interface C { kind: 3; count: number }
type ABC = A | B | C;

function main(): number {
  let v: ABC = { kind: 3, count: 100 };
  
  if (v.kind === 1) {
    return v.val;
  } else {
    // v 收窄为 B | C
    if (v.kind === 2) {
      return v.num;
    } else {
      // v 应该收窄为 C
      return v.count;
    }
  }
}
