fn main() {
    // MKL linking is handled by the intel-mkl-src crate.
    // We can add custom linking logic here if needed for specific platforms.
    println!("cargo:rerun-if-changed=build.rs");
}
