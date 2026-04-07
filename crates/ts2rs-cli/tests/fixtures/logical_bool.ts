function main(): number {
  let ok: boolean = true && (false || true);
  if (ok && true) {
    return 1;
  }
  return 0;
}
