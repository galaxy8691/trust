function main(): number {
  let x: number = 1;
  {
    let x: number = 2;
    return x;
  }
}
