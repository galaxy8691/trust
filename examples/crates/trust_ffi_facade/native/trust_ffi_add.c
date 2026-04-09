/* Minimal C symbol linked into the Rust crate; Trust TS 只通过 `Cffi::add_nums` 间接调用。 */
int trust_example_c_add(int a, int b) { return a + b; }
