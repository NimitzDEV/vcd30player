fn main() {
    cc::Build::new()
        .include("c_src")
        .include("vendor/pl_mpeg")
        .file("c_src/pl_mpeg_impl.c")
        .warnings(false)
        .compile("pl_mpeg");

    println!("cargo:rerun-if-changed=vendor/pl_mpeg/pl_mpeg.h");
    println!("cargo:rerun-if-changed=c_src/pl_mpeg_impl.c");

    #[cfg(target_os = "windows")]
    {
        let mut res = winres::WindowsResource::new();
        res.set("ProductName", "VCD 3.0 Player");
        res.set("FileDescription", "VCD 3.0 Interactive Player & Runtime");
        res.set("CompanyName", "NimitzDEV");
        res.set("LegalCopyright", "Copyright (C) 2026 NimitzDEV");
        res.set("OriginalFilename", "vcd30_player.exe");
        res.set("InternalName", "vcd30_player");
        if let Err(e) = res.compile() {
            eprintln!("cargo:warning=Failed to compile Windows resource: {}", e);
        }
    }
}
