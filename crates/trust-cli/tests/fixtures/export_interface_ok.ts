export interface Box {
  w: number;
  h: number;
}

function main(): number {
  let b: Box = { w: 3, h: 4 };
  return b.w * b.h;
}
