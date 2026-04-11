// G1/G2: 多类型参数泛型

function pair<T, U>(x: T, y: U): number {
  return 0;
}

function main(): number {
  return pair(1, "x");  // 推断 T=number, U=string
}
