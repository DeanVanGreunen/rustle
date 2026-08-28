fn main() {
    println!("cargo:rerun-if-changed=assets/logo.ico");

    #[cfg(windows)]
    {
        let mut res = winresource::WindowsResource::new();
        res.set_icon("assets/logo.ico");
        if let Err(e) = res.compile() {
            println!("cargo:warning=failed to embed logo.ico: {e}");
        }
    }
}
