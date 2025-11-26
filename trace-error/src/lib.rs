
#[cfg(test)]
mod tests;

mod error;

pub use error::*;

pub mod anyhow {
    pub use trace_error_macro::trace_error;
}

// pub mod rootcause {
//     pub use trace_error_macro::trace_error;
// }
