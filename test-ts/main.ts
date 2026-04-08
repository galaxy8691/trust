// test-ts：多文件手工回归 — 入口须为 `export function main`；依赖 `./math.ts`（math 再依赖 `./strutil.ts`）。
// 运行：`cargo run -p ts2rs-cli -- run test-ts/main.ts`
// stdout 依次：`ok 1`、`cmp 1`、`void_fn 1`；随后一行 `out= 15 5 50 2`（空格分隔）；最后一行 stdout 为 main 返回值（当前实现为 13808）。
// stderr：`err 1` 与单独一行 `2`（console.debug）。
// 类型 `interface` / `type` 仅在本文件使用（具名表不跨模块合并，见 README）。

import {
  abs_diff,
  add,
  clamp,
  div,
  early,
  eq,
  fib,
  fib_loop,
  greater,
  ipow,
  len_label_twice,
  math_builtin_sum,
  mul,
  sign,
  sub,
} from "./math.ts";

interface Point {
  x: number;
  y: number;
}

type P = Point;

function void_log_once(): void {
  console.log("void_fn", 1);
}

export function main(): number {
  let acc: number = 0;

  const seed: number = 1;
  acc = acc + seed;

  let mu: number = 2;
  mu = mu + 1;
  acc = acc + mu;

  for (let ii: number = 0; ii < 3; ii = ii + 1) {
    acc = acc + 1;
  }

  let dw: number = 0;
  do {
    dw = dw + 1;
  } while (dw < 2);
  acc = acc + dw;

  let n: number = 0;
  while (true) {
    n = n + 1;
    if (n >= 2) {
      break;
    }
  }
  acc = acc + n;

  let sum_loop: number = 0;
  let j: number = 0;
  while (j < 5) {
    j = j + 1;
    if (j == 2) {
      continue;
    }
    sum_loop = sum_loop + j;
  }
  acc = acc + sum_loop;

  function inner(k: number): number {
    return k + 10;
  }
  acc = acc + inner(2);

  let p: P = { x: 3, y: 4 };
  acc = acc + p.x + p.y;

  let xs: number[] = [1, 2, 3];
  acc = acc + xs[0] + xs[2];

  let z: null = null;
  let one: number = 1;
  acc = acc + (z ?? 5);
  acc = acc + (z ?? one);

  let s: string = "abc";
  acc = acc + s?.length;

  let tpl: string = `a${1}b`;
  acc = acc + tpl.length;

  acc = acc + (1, 2, 4);

  acc = acc + (1 > 0 ? 10 : 0);

  if (!false) {
    acc = acc + 2;
  }

  if (1 && 2) {
    acc = acc + 1;
  }

  if (0 || 3) {
    acc = acc + 3;
  }

  let flag: true | false = true;
  if (flag) {
    let bit: 0 | 1 = 1;
    if (bit) {
      acc = acc + 2;
    }
  }

  let t: boolean = true;
  if (t) {
    console.log("ok", 1);
  } else {
    console.log("no", 0);
  }

  if (greater(10, 5)) {
    console.log("cmp", 1);
  } else {
    console.log("cmp", 0);
  }

  void_log_once();

  console.error("err", 1);
  console.debug(2);

  let olen: { length: number; tag: number } = { length: 7, tag: 1 };
  acc = acc + olen.length;

  let x: number = 10;
  let y: number = 5;
  let sum: number = add(x, y);
  let diff: number = sub(x, y);
  let prod: number = mul(x, y);
  let quot: number = div(x, y);

  let label: string = "out";
  let sep: string = "=";
  console.log(label + sep, sum, diff, prod, quot);

  let d: number = abs_diff(x, y);
  let e: number = early(7);

  acc = acc + sum + diff + prod + quot + d + e;

  {
  }
  if (eq(3, 3)) {
    acc = acc + 1;
  }
  acc = acc + math_builtin_sum();
  acc = acc + clamp(99, 0, 50);
  acc = acc + sign(-2) + sign(0) + sign(4);
  acc = acc + ipow(2, 5);
  acc = acc + len_label_twice();

  acc = acc + fib(20);
  acc = acc + fib_loop(20);

  return acc;
}
