#![no_std]
#![no_main]

#[no_mangle]
fn main() {
    libvanadinite::print("hello world\n\r");
    libvanadinite::exit();
}
