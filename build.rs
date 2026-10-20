// This is for publishing to crates.io.
// fn main() {
//     println!("cargo:rustc-check-cfg=cfg(rustc_1_100)");

//     let v = rustc_version::version().unwrap();

//     println!("cargo:warning=Detected rustc version: {}", v);

//     if v >= rustc_version::Version::new(1, 100, 0) {
//         println!("cargo:rustc-cfg=rustc_1_100");
//     }
// }

fn main() {
    println!("cargo:rustc-check-cfg=cfg(rustc_1_100)");

    let v = rustc_version::version().unwrap();

    if v.major > 1 || (v.major == 1 && v.minor >= 100) {
        println!("cargo:rustc-cfg=rustc_1_100");
    }
}