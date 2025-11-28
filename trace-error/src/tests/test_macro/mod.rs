
#[macro_export]
macro_rules! assert_contains {
    ($left:expr, $right:expr $(,)?) => {
        {
            let msg = format!("{:?}", $left);
            assert!(msg.contains($right), "msg [{}]", msg);
        }
    };
}

#[macro_export]
macro_rules! assert_not_contains {
    ($left:expr, $right:expr $(,)?) => {
        {
            let msg = format!("{:?}", $left);
            assert!(!msg.contains($right), "msg [{}]", msg);
        }
    };
}


mod ok;

mod error1;

mod error2;

mod error3;

mod error4;

mod error5;

mod option;

mod macro_rules;
