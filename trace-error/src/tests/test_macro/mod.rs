
#[macro_export]
macro_rules! assert_error_line {
    ($left:expr, $right:expr $(,)?) => {
        {
            let msg = format!("{:?}", $left);
            assert!(msg.contains($right), "msg [{}]", msg);
        }
    };
}

mod poc;

mod ok;

mod error1;

mod error2;

mod error3;

mod error4;

mod error5;
