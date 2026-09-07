#[cfg(windows)]
fn main() {
    let mut res = winres::WindowsResource::new();
    res.set_icon("dgrlauncher.ico");
    res.compile().unwrap();
}
#[cfg(unix)]
fn main() {}
