use anyhow::{Result, Error};
use trace_error_macro::trace_error;

use crate::assert_error_line;


pub const ERROR1: &'static str = "at src/tests/test_macro/error5.rs:11:5, error1()";

#[trace_error]
fn error1() -> Result<()> {
    Err(Error::msg(ERROR_MSG))
}


pub const THROW_ERR_1: &'static str = "at src/tests/test_macro/error5.rs:19:5, throw_err_1()";

#[trace_error]
fn throw_err_1() -> Result<()> {
    error1()
}

pub const THROW_ERR_2: &'static str = "at src/tests/test_macro/error5.rs:26:5, throw_err_2()";

#[trace_error]
fn throw_err_2() -> Result<()> {
    return error1()
}


// 以上行数不要修改
// ====================================

const ERROR_MSG: &'static str = "it-is-error-message";


#[test]
fn test_error() {
    assert_error_line!(throw_err_1(), ERROR1);
    assert_error_line!(throw_err_1(), THROW_ERR_1);

    assert_error_line!(throw_err_2(), ERROR1);
    assert_error_line!(throw_err_2(), THROW_ERR_2);
}
