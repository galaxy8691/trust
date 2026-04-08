function bad(): number {
  return this.x;
}

function main(): number {
  return bad();
}
