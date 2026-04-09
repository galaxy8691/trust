interface Outer {
  a: number;
  inner: { x: number; y?: number };
}

function main(): number {
  let o: Outer = { a: 1, inner: { x: 2 } };
  return o.a + o.inner.x;
}
