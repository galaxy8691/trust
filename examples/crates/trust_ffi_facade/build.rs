fn main() {
    let mut build = cc::Build::new();
    build.file("native/trust_ffi_add.c");
    build.compile("trust_ffi_add");
    println!("cargo:rerun-if-changed=native/trust_ffi_add.c");
}
