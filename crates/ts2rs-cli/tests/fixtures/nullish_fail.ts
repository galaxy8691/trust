// Negative: `??` requires nullable-left or same-type operands (matrix: `??` row boundary).
function main(): string {
  let a: number = 1;
  let b: string = "x";
  return a ?? b;
}
