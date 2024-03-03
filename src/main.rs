use xrv::XRVReader;

mod xrv;

fn main() -> Result<(), xrv::XRVErr> {
    let mut reader = xrv::XRVReader::new("test.xrv".into())?;

    // reader.parse_next()?;
    // reader.parse_next()?;
    // reader.parse_next()?;

    while let Ok(line) = reader.parse_next() {
        dbg!(line);
    }

    // dbg!(&reader.tables);
    // dbg!(&reader.styles);
    Ok(())
}
