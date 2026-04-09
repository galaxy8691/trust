function main(): number {
  let a: number = 0;
  a = a + (JSON.parse("true") ? 40 : 0);
  a = a + decodeURIComponent(encodeURIComponent("z")).charCodeAt(0);
  let doc: string = "0.5";
  a = a + JSON.parse(doc);
  return a;
}
