// 多文件样例：依赖模块 — 仅 `export function` 可被其它文件 import。
// 本文件再依赖 `./strutil.ts`（模块图：main → math → strutil）。
import { aux_label, utf16_len } from "./strutil.ts";

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

// 与 cli fixture math_builtin.ts 一致：Math 整数子集。
export function math_builtin_sum(): number {
  return Math.abs(-7) + Math.min(1, 9) + Math.max(0, 4) + Math.floor(3) + Math.ceil(2);
}

export function clamp(v: number, lo: number, hi: number): number {
  if (v < lo) {
    return lo;
  }
  if (v > hi) {
    return hi;
  }
  return v;
}

export function sign(n: number): number {
  if (n > 0) {
    return 1;
  }
  if (n < 0) {
    return -1;
  }
  return 0;
}

// 小指数整数幂（底数、指数均非负，避免溢出）。
export function ipow(base: number, exp: number): number {
  let r: number = 1;
  let i: number = 0;
  while (i < exp) {
    r = r * base;
    i = i + 1;
  }
  return r;
}

// 经 strutil 取 UTF-16 长度再翻倍，覆盖 math → strutil。
export function len_label_twice(): number {
  let s: string = aux_label();
  let n: number = utf16_len(s);
  return n + n;
}

// --- 泛型（可写显式实参，或在实参可合成类型时省略，见 generic_function_ok.ts）---
export function identity<T>(x: T): T {
  return x;
}

// --- 高阶函数：函数类型参数 + 返回闭包（见 hof_apply_ok / hof_return_closure_ok）---
export function apply_num(f: (x: number) => number, x: number): number {
  return f(x);
}

export function make_adder(n: number): (x: number) => number {
  return (x: number): number => x + n;
}
