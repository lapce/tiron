fn main() {
    if let Err(e) = tiron::core::cmd() {
        let _ = e.report_stderr();
    }
}
