import { foo } from "./import_missing_export_lib.ts";
export function main(): number {
  return foo();
}
