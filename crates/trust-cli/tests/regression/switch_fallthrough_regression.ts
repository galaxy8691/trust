// Regression anchor: empty `case` fall-through must be rejected (see fixtures/switch_fail.ts).
function main(): number {
  let x: number = 1;
  switch (x) {
    case 0:
    case 1:
      return 0;
  }
}
