// 多文件样例：依赖模块 — 仅 `export function` 可被其它文件 import。
export function add(a: number, b: number): number {
  return a + b;
}

export function sub(a: number, b: number): number {
  return a - b;
}

export function mul(a: number, b: number): number {
  return a * b;
}

export function div(a: number, b: number): number {
  return a / b;
}

export function greater(a: number, b: number): boolean {
  return a > b;
}

export function eq(a: number, b: number): boolean {
  return a == b;
}

export function abs_diff(a: number, b: number): number {
  if (a < b) {
    return sub(b, a);
  } else {
    return sub(a, b);
  }
}

export function early(a: number): number {
  while (a) {
    return a;
  }
  return 0;
}

export function fib(n: number): number {
  if (n <= 1) {
    return n;
  }
  return fib(n - 1) + fib(n - 2);
}

export function fib_loop(n: number): number {
  let a: number = 0;
  let b: number = 1;
  for (let i: number = 0; i < n; i = i + 1) {
    let tmp: number = a;
    a = a + b;
    b = tmp;
  }
  return a;
}
