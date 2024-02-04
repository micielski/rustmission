pub mod app;
mod components;
pub mod tui;

use anyhow::Result;
use app::App;
use rm_config::Config;

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<()> {
    let config = Config::init()?;

    let mut app = App::new(&config);
    app.run().await?;

    Ok(())
}
