use tracing::info;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt::init();

    info!("Iniciando Gnosis - Recomp do Obsidian em Rust");
    println!("Gnosis v0.1.0 inicializado com sucesso.");

    Ok(())
}
