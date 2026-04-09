function main(): number {
  let a: number = Math.pow(2, 4);
  a = a + Number.parseInt("10");
  a = a + Number.parseFloat("9.9");
  a = a + JSON.parse("100");
  a = a + "A".charCodeAt(0);
  a = a + "xy".indexOf("y");
  a = a + ("z".includes("z") ? 5 : 0);
  a = a + "abc"[1].charCodeAt(0);
  a = a + JSON.stringify(0).length;
  a = a + Math.sign(3) + Math.sign(-2) + Math.trunc(5) + Math.round(6);
  a = a + "ab".slice(0, 1).length;
  a = a + "yx".substring(1, 0).length;
  return a;
}
