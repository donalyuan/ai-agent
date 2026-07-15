fn main() {
    // SQLx embeds migrations at compile time, so adding a file must invalidate the API build.
    println!("cargo:rerun-if-changed=migrations");
}
