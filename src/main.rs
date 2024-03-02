use std::collections::HashMap;
use std::io::{prelude::*, SeekFrom};
use std::{fs::File, io::BufReader};

struct Field {
    name: Vec<u8>,
    value: Vec<u8>,
}

#[derive(Debug)]
enum XRVErr {
    FstFailToParse(usize),
    SndFailToParse(usize),
    NextFailToParse(usize),

    FailedToConsumedRefs,
    NotParsedLine,

    FailedToGetLinkFromBuf,
    FailToOpenFile(std::io::Error),
    FailToReadUntil(std::io::Error),
    FailToEnumerateByte(usize),

    NotMetaLine(usize),
    NotRecordLine(usize),
    UnknownLine(usize),
    FailToGetStreamPosition(std::io::Error),
}

#[derive(Debug)]
enum FieldState {
    ExpectMid,
    ExpectEnd,
    ExpectStart,
}

enum Lines {
    TableLine(Line),
    StyleLine(Line),
    RecordLine(Line),
}

struct Line {
    buffer: Vec<u8>,
    start: u64,
    len: usize,
}

struct XRVReader {
    buffer: BufReader<File>,
    read_bytes: usize,
    jumps: HashMap<Vec<u8>, usize>,
    tables: HashMap<Vec<u8>, Vec<Field>>,
    styles: HashMap<Vec<u8>, Vec<Field>>,
}

impl XRVReader {
    fn new(path: String) -> Result<XRVReader, XRVErr> {
        match File::open(path) {
            Err(err) => Err(XRVErr::FailToOpenFile(err)),
            Ok(file) => Ok(XRVReader {
                buffer: BufReader::new(file),
                read_bytes: 0,
                jumps: HashMap::new(),
                tables: HashMap::new(),
                styles: HashMap::new(),
            }),
        }
    }

    fn next(&mut self, meta: bool) -> Result<Lines, XRVErr> {
        let mut buffer: Vec<u8> = Vec::new();
        let start = match self.buffer.stream_position() {
            Err(err) => return Err(XRVErr::FailToGetStreamPosition(err)),
            Ok(pos) => pos,
        };
        match self.buffer.read_until(NEWLINE, &mut buffer) {
            Err(err) => Err(XRVErr::FailToReadUntil(err)),
            Ok(len) => match (buffer[0], meta) {
                (RECORDCHAR, true) => Err(XRVErr::NotMetaLine(len)),
                (RECORDCHAR, false) => Ok(Lines::RecordLine(Line { buffer, start, len })),
                (TABLECHAR, true) => Ok(Lines::TableLine(Line { buffer, start, len })),
                (TABLECHAR, false) => Err(XRVErr::NotRecordLine(len)),
                (STYLECHAR, true) => Ok(Lines::StyleLine(Line { buffer, start, len })),
                (STYLECHAR, false) => Err(XRVErr::NotRecordLine(len)),
                (_, _) => Err(XRVErr::UnknownLine(len)),
            },
        }
    }

    fn fields(line: &[u8], start: usize) -> Result<Vec<Field>, XRVErr> {
        let mut state = FieldState::ExpectMid;
        let mut seek: usize = 0;
        let it = line.bytes();

        let mut refs: Vec<Field> = Vec::new();

        for (i, b) in it.enumerate() {
            match b {
                Err(err) => return Err(XRVErr::FailToEnumerateByte(i)),
                Ok(byte) => {
                    match (byte, &state) {
                        (COLON, FieldState::ExpectMid) => {
                            refs.push(Field {
                                pos: start + seek,
                                len: i - seek,
                            });
                            seek = i + 1;
                            state = FieldState::ExpectEnd;
                        }

                        (COLON, _) => return Err(XRVErr::FstFailToParse(i)),
                        (SPACE, FieldState::ExpectEnd) => {
                            refs.push(ReadRef {
                                pos: start + seek,
                                len: i - seek,
                            });
                            seek = i + 1;
                            state = FieldState::ExpectStart;
                        }
                        (SPACE, _) => return Err(XRVErr::SndFailToParse(i)),
                        (CR, FieldState::ExpectEnd) => {
                            refs.push(ReadRef {
                                pos: start + seek,
                                len: i - seek,
                            });
                            break;
                        }
                        (NEWLINE, FieldState::ExpectEnd) => {
                            refs.push(ReadRef {
                                pos: start + seek,
                                len: i - seek,
                            });
                            break;
                        }
                        (_, FieldState::ExpectStart) => match byte {
                            COLON => return Err(XRVErr::NextFailToParse(i)),
                            SPACE => continue,
                            CR => return Err(XRVErr::NextFailToParse(i)),
                            NEWLINE => return Err(XRVErr::NextFailToParse(i)),
                            _ => {
                                state = FieldState::ExpectMid;
                            }
                        },
                        (_, FieldState::ExpectMid) => continue,
                        (_, FieldState::ExpectEnd) => {
                            continue;
                        }
                    };
                }
            }
        }
        let mut links: Vec<Link> = Vec::new();
        let mut ref_it = refs.into_iter();

        loop {
            match ref_it.next() {
                None => break,
                Some(name) => match ref_it.next() {
                    None => return Err(XRVErr::FailedToConsumedRefs),
                    Some(seek) => {
                        links.push(Link { name, seek });
                    }
                },
            }
        }
        Ok(links)
    }
}

fn main() -> Result<(), XRVErr> {
    let reader = XRVReader::new("test.xrv".into())?;

    let mut links = match parse_links(&read_line.buffer, reader_read_bytes) {
        Err(err) => panic!("{:?}", err),
        Ok(ls) => ls,
    };

    dbg!(&links[1]);
    let mut res: Vec<Vec<u8>> = Vec::new();
    match get_link(&mut buf, &links[1]) {
        Err(err) => panic!("{:?}", err),
        Ok(r) => res = r,
    }

    dbg!(&res);
    Ok(())
}

fn parse_line(line: ReadLine, start: usize) -> Result<Lines, LinkErr> {
    match line.buffer[0] {
        TABLECHAR => {}
        STYLECHAR => Ok(Lines::StyleLine(parse_links(line, start))),
        RECORDCHAR => Ok(Lines::RecordLine(parse_links(line, start))),
        _ => Err(LinkErr::NotParsedLine),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_links_test() {
        match parse_links("some:0 table:0\n".as_bytes(), 10) {
            Err(err) => panic!("{:?}", err),
            Ok(mut links) => {
                assert!(links.len() == 2);
                let l1 = links.pop().unwrap();
                assert!(l1.name.pos == 17);
                assert!(l1.name.len == 5);
                assert!(l1.seek.pos == 23);
                assert!(l1.seek.len == 1);
            }
        };
    }

    #[test]
    fn parse_table_line_test() {
        let line = "t:some Header:string Position:i32\n".as_bytes();
        match parse_links(line, 10) {
            Err(err) => panic!("{:?}", err),
            Ok(links) => {
                assert!(links.len() == 3);
                let l1 = &links[1];
                assert!(l1.name.pos == 17);
                assert!(l1.name.len == 6);
                assert!(l1.seek.pos == 24);
                assert!(l1.seek.len == 6);
            }
        };
    }
}

const COLON: u8 = b':';
const SPACE: u8 = b' ';
const BRACKET: u8 = b'"';
const CR: u8 = b'\r';
const NEWLINE: u8 = b'\n';

const TABLECHAR: u8 = b't';
const STYLECHAR: u8 = b's';
const RECORDCHAR: u8 = b'r';

fn get_link(buffer: &mut BufReader<File>, link: &Link) -> std::io::Result<Vec<Vec<u8>>> {
    let mut ret: Vec<Vec<u8>> = Vec::new();
    let _ = buffer.seek(SeekFrom::Current(link.name.pos as i64));
    let mut name: Vec<u8> = Vec::new();
    name.resize(link.name.len, b'0');
    match buffer.read_exact(&mut name) {
        Err(err) => return Err(err),
        Ok(_) => ret.push(name),
    };
    let _ = buffer.seek(SeekFrom::Current(1));
    let mut seek: Vec<u8> = Vec::new();
    seek.resize(link.seek.len, b'0');

    match buffer.read_exact(&mut seek) {
        Err(err) => return Err(err),
        Ok(_) => ret.push(seek),
    };
    Ok(ret)
}
