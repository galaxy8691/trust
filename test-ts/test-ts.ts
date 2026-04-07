// test-ts：手工回归样例（覆盖 let / if / while / boolean / string / console.log / 比较等）
// 运行：`cargo run -p ts2rs-cli -- run test-ts/test-ts.ts`
// 当前 codegen 下多参数 log 为连续 `{}` 无分隔符；本例约输出 ok1、cmp1、out=155502，最后一行打印 main 返回值 84。

function add(a: number, b: number): number {
    return a + b;
}

function sub(a: number, b: number): number {
    return a - b;
}

function mul(a: number, b: number): number {
    return a * b;
}

function div(a: number, b: number): number {
    return a / b;
}

// 比较与布尔返回
function greater(a: number, b: number): boolean {
    return a > b;
}

function eq(a: number, b: number): boolean {
    return a == b;
}

// 一元 `-` 与分支（模拟 abs）
function abs_diff(a: number, b: number): number {
    if (a < b) {
        return sub(b, a);
    } else {
        return sub(a, b);
    }
}

// while：条件为 number，体内直接 return（无赋值语句时的典型写法）
function early(a: number): number {
    while (a) {
        return a;
    }
    return 0;
}

function main(): number {
    let x: number = 10;
    let y: number = 5;
    let t: boolean = true;

    if (t) {
        console.log("ok", 1);
    } else {
        console.log("no", 0);
    }

    if (greater(x, y)) {
        console.log("cmp", 1);
    } else {
        console.log("cmp", 0);
    }

    let sum: number = add(x, y);
    let diff: number = sub(x, y);
    let prod: number = mul(x, y);
    let quot: number = div(x, y);

    let label: string = "out";
    let sep: string = "=";
    console.log(label + sep, sum, diff, prod, quot);

    let d: number = abs_diff(x, y);
    let e: number = early(7);

    // 15+5+50+2 + abs_diff(10,5)=5 + early(7)=7 → 84
    return sum + diff + prod + quot + d + e;
}
