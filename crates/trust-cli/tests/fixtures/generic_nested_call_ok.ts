// G1-1: 嵌套泛型调用显式实参

function f<T>(x: T): T {
  return x;
}

function g<U>(x: U): U {
  return x;
}

function main(): number {
  return f<number>(g<number>(3));
}
