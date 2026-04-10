// 测试交集类型 A & B - 使用当前支持的 number 字段
type Point2D = { x: number; y: number };
type Point3D = Point2D & { z: number };

function main(): number {
  let p: Point3D = { x: 1, y: 2, z: 3 };
  console.log(p.x);
  console.log(p.y);
  console.log(p.z);
  return p.x + p.y + p.z;
}
