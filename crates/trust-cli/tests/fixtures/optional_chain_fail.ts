// Negative: optional call with callee that is not identifier / `obj.prop` (arrow expression).
function main(): number {
  return ((): number => 1)?.();
}
