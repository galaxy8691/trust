// B2+: Circular interface extends should fail

interface A extends B {
  a: number;
}

interface B extends A {
  b: number;
}

export function main(): number {
  return 1;
}
