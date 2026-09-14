use testbed::run;

fn main() {
    unsafe { std::env::set_var("RUST_BACKTRACE", "1") };
    run();
}
