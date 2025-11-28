
use crate::tests::trace_result;
use anyhow::Result;



#[test]
#[ignore = "poc"]
fn print_chain() {
    println!("{:?}", foo_chain1());
}


#[trace_result]
fn foo_chain1() -> Result<()> {
    foo_chain2()?;
    Result::<()>::Ok(())
}

#[trace_result]
fn foo_chain2() -> Result<()> {
    foo_chain3()
}

#[trace_result]
fn foo_chain3() -> Result<()> {
    return foo_chain4()
}

#[trace_result]
fn foo_chain4() -> Result<()> {
    return foo_chain5()?;
}

#[trace_result]
fn foo_chain5() -> Result<Result<()>> {
    return Ok(foo_chain6()?) 
}

#[trace_result]
fn foo_chain6() -> Result<Result<()>> {
    foo_chain_n()?;
    Result::<_>::Ok(Ok(()))
}

#[trace_result]
fn foo_chain_n() -> Result<()> {
    chain_error1()?;
    chain_error2()?;
    Ok(())
}

#[trace_result]
fn chain_error1() -> Result<()> {
    Result::<()>::Err(anyhow::Error::msg("error msg1"))
}

#[trace_result]
fn chain_error2() -> Result<()> {
    Result::<()>::Err(anyhow::Error::msg("error msg2"))
}


#[cfg(test)]
mod poc {
    use anyhow::{Context, Result};

    #[test]
    fn test_print() {
        let _r = foo(Ok(()));
        let _r = loop_it();
        let _r = mac_fn_line();
    }

    // #[trace_error_macro::debug_traverse_fn]
    #[trace_error_macro::trace_result_anyhow]
    fn mac_fn_line() {
        let name = trace_here!();
        println!("it's error\n{}\n{:?}\nname {}", trace_here!(), "foo", name);
    }

    // #[trace_error_macro::debug_print_fn]
    fn loop_it() -> Result<()> {
        let mut num = 0;
        loop {
            num += 1;
            if num >= 10 {
                return Ok(())
            }
        }
    }


    // #[trace_error_macro::debug_traverse_fn]
    fn foo(arg: Result<()>) -> Result<()> {

        let func = move || {
            let _r1 = Result::<_>::Ok("111")?;
            let _r2 = Result::<_>::Ok(Result::<_>::Ok("222")?)?;
            let _r3 = Result::<_>::Ok(Result::<_>::Ok("333"))?.with_context(||"333 context")?;
            arg
        };

        let arg1 = func();

        let arg2 = match arg1 {
            Ok(v) => Ok(v),
            Err(e) => return Err(e),
        };

        match arg2 {
            Ok(v) => Ok(v),
            Err(e) => Err(e),
        }
    }
}




// error!("{}", trace_fmt!("phase_work failed", e));
