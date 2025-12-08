

#[cfg(test)]
mod tests;

mod error;


// pub trait WithLine<O> {
//     fn with_line(self) -> O;
// }


// pub use error::*;

pub mod anyhow {
    
    pub use trace_error_macro::trace_result_anyhow as trace_result;
    
    // use super::WithLine;
    // use ::anyhow::{Result, Context};

    // impl<T> WithLine<Result<T>> for Result<T> {

    //     #[track_caller]
    //     fn with_line(self) -> Result<T> {
    //         let location = std::panic::Location::caller();
    //         self.with_context(||format!("at {}:{}:{}", location.file(), location.line(), location.column()))
    //     }
    // }
}

// pub mod rootcause {
//     pub use trace_error_macro::trace_error;
// }
