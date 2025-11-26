
use trace_error_macro::trace_error;
use anyhow::Result;



#[test]
#[ignore = "poc"]
fn print_chain() {
    println!("{:?}", foo_chain1());
}


#[trace_error]
fn foo_chain1() -> Result<()> {
    foo_chain2()?;
    Result::<()>::Ok(())
}

#[trace_error]
fn foo_chain2() -> Result<()> {
    foo_chain3()
}

#[trace_error]
fn foo_chain3() -> Result<()> {
    return foo_chain4()
}

#[trace_error]
fn foo_chain4() -> Result<()> {
    return foo_chain5()?;
}

#[trace_error]
fn foo_chain5() -> Result<Result<()>> {
    return Ok(foo_chain6()?) 
}

#[trace_error]
fn foo_chain6() -> Result<Result<()>> {
    foo_chain_n()?;
    Result::<_>::Ok(Ok(()))
}

#[trace_error]
fn foo_chain_n() -> Result<()> {
    chain_error1()?;
    chain_error2()?;
    Ok(())
}

#[trace_error]
fn chain_error1() -> Result<()> {
    Result::<()>::Err(anyhow::Error::msg("error msg1"))
}

#[trace_error]
fn chain_error2() -> Result<()> {
    Result::<()>::Err(anyhow::Error::msg("error msg2"))
}
