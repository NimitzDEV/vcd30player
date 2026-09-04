fn main() {
    cc::Build::new()
        .include("c_src")
        .file("c_src/pl_mpeg_impl.c")
        .warnings(false)
        .compile("pl_mpeg");

    println!("cargo:rerun-if-changed=c_src/pl_mpeg.h");
    println!("cargo:rerun-if-changed=c_src/pl_mpeg_impl.c");
}
