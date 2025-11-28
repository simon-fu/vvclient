use anyhow::Result;
use crate::tests::trace_result;
use crate::assert_contains;


const ERROR1: &'static str = "at src/tests/test_macro/option.rs:22:5, error1()";

#[trace_result]
fn error1() -> Result<()> {
    let yes = false;

    // 对于anyhow，如果添加了 Option 添加了 with_context，类型会出错

    let opt = if yes {
        Some(())
    } else {
        None
    };

    assert_eq!(opt, None);

    opt.with_context(||"is None")
}

// 以上行数不要修改
// ====================================

#[test]
fn test_error1() {
    assert_contains!(error1(), ERROR1);
}
