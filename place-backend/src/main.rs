mod backend;
mod place;
mod settings;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let settings = settings::Settings::new()?;

    println!("{:?}", settings);

    Ok(())
}
