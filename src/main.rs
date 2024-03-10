mod newxrv;

use std::{fs::File, io::Read, time::Instant};

fn main() -> Result<(), std::io::Error> {
    // let mut reader = newxrv::Reader::new("test.xrv".into())?;

    // let line_link = reader.next();
    // dbg!(&line_link);

    // reader.parse_next()?;
    // reader.parse_next()?;
    // reader.parse_next()?;

    // dbg!(&reader.tables);
    // dbg!(&reader.styles);

    let mut file = File::open("big.txt")?;

    let now = Instant::now();
    let mut buf: [u8; 4096] = [0; 4096];
    // let r = file.read_exact(&mut buf);
    let r = file.read(&mut buf);
    let e = now.elapsed();

    dbg!(e, buf.len(), r);

    Ok(())
}
