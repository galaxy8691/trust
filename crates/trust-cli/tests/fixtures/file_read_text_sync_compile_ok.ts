function main(): number {
  let s: string = readFileText("./README.md");
  return s.length;
}
