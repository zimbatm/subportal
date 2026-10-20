use iroh::endpoint::{Builder, presets};

pub fn builder() -> Builder {
    let builder = iroh::Endpoint::builder(presets::N0);
    // On Android, rustls-platform-verifier needs JVM setup from the app, so keep the embedded roots there.
    #[cfg(not(target_os = "android"))]
    let builder = builder.ca_tls_config(iroh::tls::CaTlsConfig::system());
    builder
}
