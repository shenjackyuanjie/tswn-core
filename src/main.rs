#[allow(dead_code)]
mod engine;
/// 就一堆错误
mod error;
/// 万里长征, 始于足下
#[allow(dead_code)]
mod player;
#[allow(dead_code)]
mod rc4;

fn main() {
    println!("欢迎来到 tswn - {}, 某个充满怨念的人向你问好", env!("CARGO_PKG_VERSION"));
}
