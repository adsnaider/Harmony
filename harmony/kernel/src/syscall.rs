use crate::sdbg;

pub extern "sysv64" fn handle(a: usize, b: usize, c: usize, d: usize, e: usize, f: usize) -> usize {
    sdbg!(a, b, c, d, e, f);
    todo!();
}
