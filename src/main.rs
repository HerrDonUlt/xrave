mod xrv;

fn main() -> Result<(), xrv::XRVErr> {
    let reader = xrv::XRVReader::new("test.xrv".into())?;

    Ok(())
}
