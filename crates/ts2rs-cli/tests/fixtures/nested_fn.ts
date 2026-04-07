function main(): number {
  function inner(): number {
    return 9;
  }
  return inner();
}
