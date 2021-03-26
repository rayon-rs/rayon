fn main() {
    let ac = autocfg::new();
    if ac.probe_expression("(0..10).step_by(2).rev()") {
        autocfg::emit("step_by");
    }
    if ac.probe_expression("{ fn foo<const N: usize>() {} }") {
        autocfg::emit("min_const_generics");
    }
}
