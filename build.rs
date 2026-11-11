fn main() {
    let cfg = slint_build::CompilerConfiguration::new().with_style("material".into()).with_bundled_translations("lang");
    slint_build::compile_with_config("ui/main.slint", cfg).unwrap();
    #[cfg(windows)]
    {
        let mut res = winresource::WindowsResource::new();
        res.set_icon("assets/icon.ico");
        if let Err(e) = res.compile() {
            println!("cargo:warning=icono del exe no incrustado: {e}");
        }
    }
}
