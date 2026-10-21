mod direct;
mod hostpath;

fn main() {
    let n: usize = std::env::args().nth(1).and_then(|s| s.parse().ok()).unwrap_or(16 << 20);
    println!("== Part A: direct rquickjs (N = {n} f32) ==");
    for l in direct::run(n) {
        println!("  {l}");
    }
    println!("== Part B: svelters host, ABI v3 ==");
    for l in hostpath::run() {
        println!("  {l}");
    }
}
