use brain_services::runtime::{ApplicationRuntime, RuntimeObserver};
use std::sync::Arc;

struct LogObserver;
impl RuntimeObserver for LogObserver {
    fn on_started(&self, _runtime: &ApplicationRuntime) {
        println!("ApplicationRuntime observer: started.");
    }
    fn on_stopping(&self, _runtime: &ApplicationRuntime) {
        println!("ApplicationRuntime observer: stopping.");
    }
    fn on_stopped(&self, _runtime: &ApplicationRuntime) {
        println!("ApplicationRuntime observer: stopped.");
    }
}

// Additional implementations (e.g. EmbeddedClient) can be added in the future
// if an in-process runtime mode is introduced.


#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = std::env::args().collect();

    // Check if running in daemon service mode
    let is_daemon = args.len() > 1 && args[1] == "daemon";

    if is_daemon {
        println!("Starting Brain v2 Engine composition root as background daemon...");

        // 1. Resolve configuration
        let defaults_src = brain_config::loader::DefaultsSource;
        let config = brain_config::loader::resolve(&[Box::new(defaults_src)])?;

        // 2. Build the application runtime
        let runtime = Arc::new(
            ApplicationRuntime::builder()
                .with_config(config)
                .register_observer(Arc::new(LogObserver))
                .build()?,
        );

        // 3. Start the runtime
        let startup_report = runtime.start()?;
        println!(
            "Runtime successfully started in {}ms. Completed phases: {:?}",
            startup_report.duration().as_millis(),
            startup_report.completed_phases()
        );

        println!("Running... Press Ctrl+C to stop.");

        // 4. Wait for Ctrl+C signal
        tokio::signal::ctrl_c().await?;

        println!("Ctrl+C received. Initiating graceful shutdown...");

        // 5. Shutdown the runtime
        runtime.shutdown()?;
        println!("Shutdown sequence completed successfully.");
    } else {
        // Start interactive Ratatui TUI mode by default
        let client = Box::new(brain_tui::client::UdsClient::default());
        if let Err(e) = brain_tui::run(client).await {
            eprintln!("TUI Error: {}", e);
            std::process::exit(1);
        }
    }

    Ok(())
}
