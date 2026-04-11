// R2: Deep call/member chains with function references in object literals

interface Inner {
  val(): number;
}

interface Middle {
  getInner(): Inner;
}

interface Outer {
  getMiddle(): Middle;
}

function val(i: Inner): number {
  return 42;
}

function getInner(m: Middle): Inner {
  return { val: val };
}

function getMiddle(o: Outer): Middle {
  return { getInner: getInner };
}

function makeOuter(): Outer {
  return { getMiddle: getMiddle };
}

export function main(): number {
  // Deep chain: 3 levels of call + method access
  // makeOuter() -> Outer
  //   .getMiddle() -> Middle
  //     .getInner() -> Inner
  //       .val() -> number
  return makeOuter().getMiddle().getInner().val();
}
