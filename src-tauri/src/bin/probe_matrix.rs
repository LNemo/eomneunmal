use eomneunmal::platform::macos::run_macos_inventory_probe;
use eomneunmal::platform::probe::AdapterProbe;
use eomneunmal::platform::windows::WindowsProbe;

fn main() {
    println!("| OS | App | OS/App version | Permissions | Input method | Send signal | Text acquisition method | Sensitive-exclusion result | Status | Evidence notes |");
    println!("|---|---|---|---|---|---|---|---|---|---|");
    let mut rows = Vec::new();
    if cfg!(target_os = "macos") {
        rows.extend(run_macos_inventory_probe());
    }
    rows.extend(WindowsProbe::default().probe_rows());
    for row in rows {
        println!("{}", row.markdown_row());
    }
}
