// 可选调用 `f?.()` / `recv?.m()`：与普通过调用类型一致时与直接调用等价。
function f(): number {
  return 2;
}

interface Box {
  v: number;
}

function make(): Box {
  return { v: 3 };
}

function get_v(b: Box): number {
  return b.v;
}

export function main(): number {
  return f?.() + make()?.get_v();
}
