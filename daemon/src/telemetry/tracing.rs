use tracing_subscriber::{fmt, prelude::*, Registry};

pub fn init_subscriber(log_level: &str, use_json: bool) {
    let env_filter = tracing_subscriber::EnvFilter::new(log_level);

    if use_json {
        let json_layer = fmt::layer().json().with_filter(env_filter);
        let _ = Registry::default().with(json_layer).try_init();
    } else {
        let text_layer = fmt::layer().with_filter(env_filter);
        let _ = Registry::default().with(text_layer).try_init();
    }
}
