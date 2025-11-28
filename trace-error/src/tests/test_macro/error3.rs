use anyhow::{Result, Error};
use crate::tests::trace_result;

use crate::assert_contains;


pub const ERROR1: &'static str = "at src/tests/test_macro/error3.rs:11:5, error1()";

#[trace_result]
fn error1() -> Result<()> {
    Err(Error::msg(ERROR_MSG))
}

pub const THROW_ERR_1: &'static str = "at src/tests/test_macro/error3.rs:18:13, throw_err_1()";

#[trace_result]
fn throw_err_1() -> Result<()> {
    error1()?;
    Ok(())
}

pub const THROW_ERR_2: &'static str = "at src/tests/test_macro/error3.rs:26:45, throw_err_2()";

#[trace_result]
fn throw_err_2() -> Result<()> {
    error1().with_context(||"error1 failed")?;
    Ok(())
}

pub const THROW_ERR_3: &'static str = "at src/tests/test_macro/error3.rs:35:41, throw_err_3()";

#[trace_result]
fn throw_err_3() -> Result<()> {
    error1()
        .with_context(||"error1 failed")?;
    Ok(())
}


// 以上行数不要修改
// ====================================

const ERROR_MSG: &'static str = "it-is-error-message";


#[test]
fn test_error() {
    assert_contains!(throw_err_1(), ERROR1);
    assert_contains!(throw_err_1(), THROW_ERR_1);

    assert_contains!(throw_err_2(), ERROR1);
    assert_contains!(throw_err_2(), THROW_ERR_2);

    assert_contains!(throw_err_3(), ERROR1);
    assert_contains!(throw_err_3(), THROW_ERR_3);
}
