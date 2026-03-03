use shadow_rs::ShadowBuilder;

fn main() {
    ShadowBuilder::builder()
        .deny_const(Default::default())
        .build()
        .unwrap();

    if cfg!(target_os = "windows") {
        let mut res = winres::WindowsResource::new();
        res.set_resource_file("app.rc");
        res.compile().expect("compile Windows resources");
        println!("cargo:rerun-if-changed=app.rc");
        println!("cargo:rerun-if-changed=../../src-tauri/icons/icon.ico");
    }
}
