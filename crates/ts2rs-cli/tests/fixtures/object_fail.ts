// Negative: object literal field values must be numberish (matrix: object literal row boundary).
function main(): number {
  return { x: "a" }.x;
}
