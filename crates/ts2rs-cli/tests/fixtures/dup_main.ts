import { foo as fa } from "./dup_a.ts";
import { foo as fb } from "./dup_b.ts";
export function main(): number {
  return fa() + fb();
}
