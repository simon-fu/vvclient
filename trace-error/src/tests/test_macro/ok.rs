use trace_error_macro::trace_error;
use anyhow::Result;

// -----------------------------------------

#[trace_error]
fn foo_ok11() -> Result<()> {
    Ok(())
}

#[trace_error]
fn foo_ok12() -> Result<()> {
    return Ok(())
}

#[trace_error]
fn foo_ok13() -> Result<()> {
    return Ok(());
}

// -----------------------------------------

#[trace_error]
fn foo_ok21() -> Result<()> {
    Result::<_>::Ok(())
}

#[trace_error]
fn foo_ok22() -> Result<()> {
    return Result::<_>::Ok(())
}

#[trace_error]
fn foo_ok23() -> Result<()> {
    return Result::<_>::Ok(());
}

// -----------------------------------------

#[trace_error]
fn foo_ok31() -> Result<Result<()>> {
    Ok(Ok(()))
}

#[trace_error]
fn foo_ok32() -> Result<Result<()>> {
    return Ok(Ok(()))
}

#[trace_error]
fn foo_ok33() -> Result<Result<()>> {
    return Ok(Ok(()));
}

// -----------------------------------------

#[trace_error]
fn foo_ok41() -> Result<Result<()>> {
    Result::<_>::Ok(Ok(()))
}

#[trace_error]
fn foo_ok42() -> Result<Result<()>> {
    return Result::<_>::Ok(Ok(()))
}

#[trace_error]
fn foo_ok43() -> Result<Result<()>> {
    return Result::<_>::Ok(Ok(()));
}

#[trace_error]
fn foo_ok44() -> Result<Result<()>> {
    return Result::<_>::Ok(Ok(())).with_context(||"never failed");
}

// -----------------------------------------

#[test]
fn test() {
    assert_eq!(format!("{:?}", foo_ok11()), "Ok(())");
    assert_eq!(format!("{:?}", foo_ok12()), "Ok(())");
    assert_eq!(format!("{:?}", foo_ok13()), "Ok(())");

    assert_eq!(format!("{:?}", foo_ok21()), "Ok(())");
    assert_eq!(format!("{:?}", foo_ok22()), "Ok(())");
    assert_eq!(format!("{:?}", foo_ok23()), "Ok(())");

    assert_eq!(format!("{:?}", foo_ok31()), "Ok(Ok(()))");
    assert_eq!(format!("{:?}", foo_ok32()), "Ok(Ok(()))");
    assert_eq!(format!("{:?}", foo_ok33()), "Ok(Ok(()))");

    assert_eq!(format!("{:?}", foo_ok41()), "Ok(Ok(()))");
    assert_eq!(format!("{:?}", foo_ok42()), "Ok(Ok(()))");
    assert_eq!(format!("{:?}", foo_ok43()), "Ok(Ok(()))");
    assert_eq!(format!("{:?}", foo_ok44()), "Ok(Ok(()))");
}
