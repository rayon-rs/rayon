fn main() {
    let ac = autocfg::new();
    if ac.probe_path("std::ops::ControlFlow") {
        autocfg::emit("has_control_flow");
    }
}
