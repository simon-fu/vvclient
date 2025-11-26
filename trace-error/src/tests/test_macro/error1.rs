use anyhow::{Result, Error};
use trace_error_macro::trace_error;
use crate::assert_error_line;


const ERROR1: &'static str = "at src/tests/test_macro/error1.rs:10:5, error1()";

#[trace_error]
fn error1() -> Result<()> {
    Err(Error::msg(ERROR_MSG))
}

const ERROR2: &'static str = "at src/tests/test_macro/error1.rs:17:5, error2()";

#[trace_error]
fn error2() -> Result<()> {
    return Err(Error::msg(ERROR_MSG))
}

const ERROR3: &'static str = "at src/tests/test_macro/error1.rs:24:5, error3()";

#[trace_error]
fn error3() -> Result<()> {
    return Err(Error::msg(ERROR_MSG));
}

const ERROR4: &'static str = "at src/tests/test_macro/error1.rs:31:5, error4()";

#[trace_error]
fn error4() -> Result<()> {
    return Err(Error::msg(ERROR_MSG))  ;
}

const ERROR5: &'static str = "at src/tests/test_macro/error1.rs:38:5, error5()";

#[trace_error]
fn error5() -> Result<()> {
    return Err(Error::msg(ERROR_MSG)).with_context(||"context in error");
}


// 以上行数不要修改
// ====================================

const ERROR_MSG: &'static str = "it-is-error-message";


#[test]
fn test_error1() {
    assert_error_line!(error1(), ERROR1);
    assert_error_line!(error2(), ERROR2);
    assert_error_line!(error3(), ERROR3);
    assert_error_line!(error4(), ERROR4);
    assert_error_line!(error5(), ERROR5);
}
