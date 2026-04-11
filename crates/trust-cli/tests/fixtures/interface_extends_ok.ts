// B2+: Interface extends support

interface Point {
  x: number;
  y: number;
}

interface Point3D extends Point {
  z: number;
}

function usePoint(p: Point): number {
  return p.x + p.y;
}

function usePoint3D(p: Point3D): number {
  return p.x + p.y + p.z;
}

export function main(): number {
  // Point3D extends Point, so it has x, y, z
  let p3d: Point3D = { x: 1, y: 2, z: 3 };
  // Width subtyping: Point3D is assignable to Point
  let sum3d: number = usePoint3D(p3d);
  return sum3d;  // 1 + 2 + 3 = 6
}
