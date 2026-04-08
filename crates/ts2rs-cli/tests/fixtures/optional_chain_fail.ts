// Negative: optional call `?.()` is not supported (matrix: member / `?.` row boundary).
function f(): number {
  return 1;
}
function main(): number {
  return f?.();
}
