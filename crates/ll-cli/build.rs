// Links the MSVC C runtime statically on Windows so `labellab.exe` runs
// without the Visual C++ Redistributable (same approach tauri-build uses
// for the GUI). No-op on other targets.
fn main() {
    static_vcruntime::metabuild();
}
