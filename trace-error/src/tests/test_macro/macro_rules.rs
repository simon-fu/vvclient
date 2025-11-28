
use anyhow::Result;
use crate::tests::trace_result;
// use crate::assert_contains;

macro_rules! return_err {
    ($msg:literal $(,)?) => {
        return Err(anyhow::anyhow!($msg))
    };
    ($err:expr $(,)?) => {
        return Err(anyhow::anyhow!($err))
    };
    ($fmt:expr, $($arg:tt)*) => {
        return Err(anyhow::anyhow!($fmt, $($arg)*))
    };
}

// const ERROR1: &'static str = "at src/tests/test_macro/macro_rules.rs:28:9, error1()";

#[trace_result]
fn error1() -> Result<()> {
    let yes = false;
  
    let opt = if yes {
        Some(())
    } else {
        // None
        return_err!("unsupported") // 宏会被忽略
    };

    assert_eq!(opt, Some(()));

    Ok(())
}


#[test]
fn test_macro_expr() {
    assert_not_contains!(error1(), ", error1()");
}
