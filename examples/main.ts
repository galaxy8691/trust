// test-ts：多文件手工回归 — 入口须为 `export function main`；依赖 `./math.ts`（math 再依赖 `./strutil.ts`）。
// 运行：`cargo run -p trust-cli -- run test-ts/main.ts`
// 覆盖：控制流、字面量/联合、`interface`/`type`、多文件 import、`Math`/工具函数、**泛型**（显式 `identity<number>(…)`）、**高阶函数**（`apply_num`、返回闭包的 `make_adder`）、**class / extends / super / 实例方法**（见下文 `OOBase`/`OOChild`）、通过 `Trust.toml` 的 Rust crate 互操作（`regex` / `url` / `diesel::sqlite::SqliteConnection::establish`），以及 `async`/`await` + **`std.http` 天气请求**（`fetchText` / `fetch` / `text`）。
// **仍不覆盖**（由 fixtures 或仍为 backlog）：`export class` 跨文件、完整 discriminated 收窄等 — 见 README 矩阵与 PROJECT-TODO。
// stdout 依次：`ok 1`、`cmp 1`、`void_fn 1`；随后一行 `out= 15 5 50 2`（空格分隔）；最后一行 stdout 为 main 返回值（以 `cargo run -p trust-cli -- run test-ts/main.ts` 为准）。
// stderr：`err 1` 与单独一行 `2`（`std.console.debug`，与 `console.debug` 等价）。
// 类型 `interface` / `type` 仅在本文件使用（具名表不跨模块合并，见 README）。

import {
  abs_diff,
  add,
  apply_num,
  clamp,
  div,
  early,
  eq,
  fib,
  fib_loop,
  greater,
  identity,
  ipow,
  len_label_twice,
  make_adder,
  math_builtin_sum,
  mul,
  sign,
  sub,
} from "./math.ts";
import { Regex } from "regex";
import { SqliteConnection } from "diesel";
import { Url } from "url";
import std from "std";

interface Point {
  x: number;
  y: number;
}

type P = Point;

// OO 子集：单文件内 class（与 class_extends_ok / class_this_method_ok 同模式；跨文件 `export class` 未支持）。
class OOBase {
  seed: number;

  constructor(seed: number) {
    this.seed = seed;
  }
}

class OOChild extends OOBase {
  k: number;

  constructor(seed: number, k: number) {
    super(seed);
    this.k = k;
  }

  // 避免与 main 内局部变量 `sum`（两数之和）脱糖后的全局名冲突，故不用方法名 `sum`。
  oo_sum(): number {
    return this.seed + this.k;
  }
}

function void_log_once(): void {
  std.console.log("void_fn", 1);
}

export async function main(): number {
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
    std.console.log("ok", 1);
  } else {
    std.console.log("no", 0);
  }

  if (greater(10, 5)) {
    std.console.log("cmp", 1);
  } else {
    std.console.log("cmp", 0);
  }

  void_log_once();

  std.console.error("err", 1);
  std.console.debug(2);

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
  std.console.log(label + sep, sum, diff, prod, quot);

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
  acc = acc + fib_loop(100);

  // 泛型 + 高阶函数（自 math 模块导入）
  acc = acc + identity<number>(11);
  acc = acc + apply_num((x: number): number => x * 2, 4);
  let add3: (x: number) => number = make_adder(3);
  acc = acc + add3(10);

  // class 实例
  let oo: OOChild = new OOChild(3, 4);
  acc = acc + oo.oo_sum();

  // Rust crate 互操作（Trust.toml：`regex::Regex` / `url::Url` 的类型 + `[[rust_binding]].method`）
  let re: Regex = new Regex("\\d+");
  acc = acc + (re.is_match("abc123") ? 1 : 0);
  acc = acc + (re.is_match("no digits here") ? 1 : 0);

  let u: Url = new Url("https://example.com/foo/bar?x=1");
  let sch: string = u.scheme();
  let pth: string = u.path();
  acc = acc + sch.length;
  acc = acc + pth.length;
  std.console.log("rust_url", sch, pth);

  // Diesel：清单里 `new` → `SqliteConnection::establish`（`Connection::establish`，非 `SimpleConnection`）；`:memory:` 仅作演示。
  let _sqlite: SqliteConnection = new SqliteConnection(":memory:");
  std.console.log("diesel_sqlite_establish", 1);

  // 天气请求（HTTP 走 Rust 生态实现）
  let weather_a: string = await std.http.fetchText("https://wttr.in/?format=3");
  std.console.log("weather_a", weather_a);
  let weather_resp: HttpResponse = await std.http.fetch("https://wttr.in/?format=3");
  let weather_b: string = await std.http.text(weather_resp);
  std.console.log("weather_b", weather_b);

  // 虚拟模块 `std`（须 `import std from "std"`）：与全局 `console` / `fetch*` / `JSON` 等能力一一对应，推荐统一写 `std.*`
  let jdoc: string = std.json.stringify({ n: 12 });
  let jstr: string = std.json.stringify("ab");
  let jsrc: string = "12" + "3";
  let jnum: number = std.json.parse(jsrc);
  let enc: string = std.uri.encodeURIComponent("a b");
  let dec: string = std.uri.decodeURIComponent(enc);
  let sm: string = "abcdef";
  let sub_s: string = std.string.slice(sm, 1, 4);
  let has_cd: boolean = std.string.includes(sm, "cd");
  let idx: number = std.string.indexOf(sm, "de");
  let utf16_len: number = std.string.length(sm);
  let mabs: number = std.math.abs(-3.0);
  let parsed: number = std.number.parseInt("9", 10);
  acc =
    acc +
    jdoc.length +
    jstr.length +
    jnum +
    dec.length +
    sub_s.length +
    idx +
    utf16_len +
    mabs +
    parsed;
  if (has_cd) {
    acc = acc + 1;
  }
  std.console.log(
    "stdlib_probe",
    jdoc,
    jstr,
    enc,
    dec,
    sub_s,
    has_cd,
    idx,
    utf16_len,
    mabs,
    parsed,
  );

  return acc;
}
