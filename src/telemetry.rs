//! Observability (o11ty)
//!
//! This module configures tracing for the application.
//!
//! Call [`init`] once at application startup before any logging occurs.

use tracing_subscriber::EnvFilter;
use tracing_subscriber::layer::SubscriberExt as _;
use tracing_subscriber::util::SubscriberInitExt as _;
/// Initialises the global tracing subscriber.
///
/// Reads `RUST_LOG` if set, otherwise defaults to `info` (or `debug` when
/// `debug_assertions` are enabled, i.e. dev builds).
///
/// # Panics
///
/// Panics if a global subscriber has already been set.
pub(crate) fn init() {
    tracing_subscriber::registry()
        .with(EnvFilter::try_from_default_env().unwrap_or_else(|_| "jsond=info,tower_http=info".into()))
        .with(tracing_subscriber::fmt::layer())
        .init();
}

// pub(crate) fn init() {
//     let level = cfg_select! {
//         debug_assertions => {  "debug" }
//         _ =>               {  "info" }
//     };

//     let subscriber = build(level, io::stdout);
//     register(subscriber);
// }

// /// Compose a `fmt` subscriber with environment-driven filtering.
// ///
// /// `default_filter` is used as a fallback when `RUST_LOG` is not set.
// /// The sink is kept generic so tests can redirect output to a buffer.
// fn build<W>(default_filter: &str, sink: W) -> impl Subscriber + Sync + Send
// where
//     W: for<'a> MakeWriter<'a> + Send + Sync + 'static,
// {
//     let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| {
//         EnvFilter::builder()
//             .with_default_directive(LevelFilter::INFO.into())
//
// .parse_lossy(format!("{default_filter},jsond=info,tower_http=info"))     });

//     let fmt_layer = fmt::layer().with_target(true).with_writer(sink);

//     Registry::default().with(filter).with(fmt_layer)
// }

// /// Register a subscriber as the global default.
// ///
// /// # Panics
// ///
// /// Panics if called more than once.
// fn register(subscriber: impl Subscriber + Send + Sync) {
//     #[expect(clippy::expect_used)]
//     set_global_default(subscriber).expect("global tracing subscriber already
// set"); }
