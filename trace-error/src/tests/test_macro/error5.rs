use anyhow::{Result, Error};
use crate::tests::trace_result;

use crate::assert_contains;


pub const ERROR1: &'static str = "at src/tests/test_macro/error5.rs:11:5, error1()";

#[trace_result]
fn error1() -> Result<()> {
    Err(Error::msg(ERROR_MSG))
}


pub const THROW_ERR_1: &'static str = "at src/tests/test_macro/error5.rs:19:5, throw_err_1()";

#[trace_result]
fn throw_err_1() -> Result<()> {
    error1()
}

pub const THROW_ERR_2: &'static str = "at src/tests/test_macro/error5.rs:26:5, throw_err_2()";

#[trace_result]
fn throw_err_2() -> Result<()> {
    return error1()
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
}
