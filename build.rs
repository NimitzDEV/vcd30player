fn main() {
    cc::Build::new()
        .include("c_src")
        .include("vendor/pl_mpeg")
        .file("c_src/pl_mpeg_impl.c")
        .warnings(false)
        .compile("pl_mpeg");

    println!("cargo:rerun-if-changed=vendor/pl_mpeg/pl_mpeg.h");
    println!("cargo:rerun-if-changed=c_src/pl_mpeg_impl.c");
}
