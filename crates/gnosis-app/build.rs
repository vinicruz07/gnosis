//! Build script para compilação estática dos arquivos de interface .slint.

#![deny(unsafe_code)]

fn main() {
    if let Err(err) = slint_build::compile("ui/appwindow.slint") {
        eprintln!("Erro ao compilar componentes Slint: {err}");
        std::process::exit(1);
    }
}
