// G2-2: 从函数返回类型推断泛型参数

function getNumber(): number {
  return 1;
}

function id<T>(x: T): T {
  return x;
}

function main(): number {
  return id(getNumber());  // 从 getNumber 返回类型推断 T=number
}
