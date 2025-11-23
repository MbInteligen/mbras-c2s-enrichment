// Thin namespace wrapper for API-layer components
pub mod handlers {
    pub use crate::handlers::*;
}

pub mod webhook_handler {
    pub use crate::webhook_handler::*;
}

pub mod google_ads_handler {
    pub use crate::google_ads_handler::*;
}
