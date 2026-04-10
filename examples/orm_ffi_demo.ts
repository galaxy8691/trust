// Diesel ORM 与 C FFI 演示（README「薄封装 / FFI」对应实现）。
// 运行：`cargo run -p trust-cli -- run examples/orm_ffi_demo.ts`
//
// - `OrmFacade`：Rust 内使用 Diesel（`filter(...).load::<User>(...)?` 等），TS 只调用 `users_named_tom_count()`。
// - `Cffi`：`native/trust_ffi_add.c` 经 `build.rs` 编译链接；TS 只调用 `add_nums`。

import { Cffi } from "trust_ffi_facade";
import { OrmFacade } from "trust_orm_facade";

export async function main(): number {
  // Trust：`new RustExtern(...)` 目前固定为**一个 string** 实参；占位即可。
  const orm: OrmFacade = new OrmFacade("");
  const tomRows: number = orm.users_named_tom_count();
  console.log("diesel_facade_tom_rows", tomRows);

  const c: Cffi = new Cffi("");
  const ffiSum: number = c.add_nums(40, 2);
  console.log("ffi_c_add", ffiSum);

  return tomRows + ffiSum;
}
