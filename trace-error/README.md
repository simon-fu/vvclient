# TraceError

## Types

#[trace_result]

trace_here!()

trace_error!(Error)

fmt_error!(Error)

trace_fmt!()


## Example

### Macro trace_result!

```rust
#[trace_result]
fn foo() -> Result<()> {
    log::debug!("this is trace_line [{}]", trace_here!());

    let err = trace_error!("bar failed", anyhow::Error::msg("any msg"));
    log::debug!("this is trace_error [{}]", fmt_error!(err));

    Err(anyhow::Error::msg("hello-error"))
}

#[trace_result]
fn bar() -> Result<()> {
    let r = foo();
    match r {
        Ok(_v) => log::debug!("success"),
        Err(e) => {
            log::error!("{}", trace_fmt!("foo failed", e))
        },
    }
    Ok(())
}

```

## Test

```bash
cargo test test_macro -- --nocapture 
```
