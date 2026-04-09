function main(): number {
  function inner(x: number): number {
    let x: number = 1;
    return x;
  }
  return inner(0);
}
