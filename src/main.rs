use std::borrow::Cow;
use std::io::{prelude::*, SeekFrom};
use std::{fs::File, io::BufReader};

#[derive(Debug)]
struct ReadRef {
    pos: usize,
    len: usize,
}

#[derive(Debug)]
struct Link {
    name: ReadRef,
    seek: ReadRef,
}

#[derive(Debug)]
enum LinkErr {
    FstFailToParse(usize),
    SndFailToParse(usize),
    NextFailToParse(usize),

    FailedToConsumedRefs,
    NotTalbeLine,
    NotStyleLine,
    NotRecordLine,

    FailedToGetLinkFromBuf,
}

#[derive(Debug)]
enum ParseState {
    ExpectLinkMid,
    ExpectLinkEnd,
    ExpectLinkStart,
}

fn main() {
    let mut file: File;
    match File::open("test.xrv") {
        Err(err) => panic!("{:?}", err),
        Ok(f) => file = f,
    }

    let mut buf = BufReader::new(file);
    let mut reader_read_bytes: usize = 0;

    let mut rl = ReadLine {
        buffer: Vec::new(),
        read_bytes: 0,
    };
    match read_line(&mut buf) {
        Err(err) => panic!("{:?}", err),
        Ok(trl) => rl = trl,
    }
    let mut links: Vec<Link> = Vec::new();
    match parse_links(&rl.buffer, reader_read_bytes) {
        Err(err) => panic!("{:?}", err),
        Ok(ls) => links = ls,
    }
    dbg!(&links[1]);
    let mut res: Vec<Vec<u8>> = Vec::new();
    match get_link(&mut buf, &links[1]) {
        Err(err) => panic!("{:?}", err),
        Ok(r) => res = r,
    }

    dbg!(&res);
}

struct ReadLine {
    buffer: Vec<u8>,
    read_bytes: usize,
}

fn read_line(buffer: &mut BufReader<File>) -> std::io::Result<ReadLine> {
    let mut temp: Vec<u8> = Vec::new();
    match buffer.read_until(b'\n', &mut temp) {
        Err(err) => Err(err),
        Ok(rb) => Ok(ReadLine {
            buffer: temp,
            read_bytes: rb,
        }),
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
const NL: u8 = b'\n';

fn parse_links(line: &[u8], start: usize) -> Result<Vec<Link>, LinkErr> {
    let mut state = ParseState::ExpectLinkMid;
    let mut seek: usize = 0;
    let it = line.bytes();

    let mut refs: Vec<ReadRef> = Vec::new();

    for (i, b) in it.enumerate() {
        match b {
            Err(err) => panic!("{:?}", err),
            Ok(byte) => {
                match (byte, &state) {
                    (COLON, ParseState::ExpectLinkMid) => {
                        refs.push(ReadRef {
                            pos: start + seek,
                            len: i - seek,
                        });
                        seek = i + 1;
                        state = ParseState::ExpectLinkEnd;
                    }

                    (COLON, _) => return Err(LinkErr::FstFailToParse(i)),
                    (SPACE, ParseState::ExpectLinkEnd) => {
                        refs.push(ReadRef {
                            pos: start + seek,
                            len: i - seek,
                        });
                        seek = i + 1;
                        state = ParseState::ExpectLinkStart;
                    }
                    (SPACE, _) => return Err(LinkErr::SndFailToParse(i)),
                    (CR, ParseState::ExpectLinkEnd) => {
                        refs.push(ReadRef {
                            pos: start + seek,
                            len: i - seek,
                        });
                        break;
                    }
                    (NL, ParseState::ExpectLinkEnd) => {
                        refs.push(ReadRef {
                            pos: start + seek,
                            len: i - seek,
                        });
                        break;
                    }
                    (_, ParseState::ExpectLinkStart) => match byte {
                        COLON => return Err(LinkErr::NextFailToParse(i)),
                        SPACE => continue,
                        CR => return Err(LinkErr::NextFailToParse(i)),
                        NL => return Err(LinkErr::NextFailToParse(i)),
                        _ => {
                            state = ParseState::ExpectLinkMid;
                        }
                    },
                    (_, ParseState::ExpectLinkMid) => continue,
                    (_, ParseState::ExpectLinkEnd) => {
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
                None => return Err(LinkErr::FailedToConsumedRefs),
                Some(seek) => {
                    links.push(Link { name, seek });
                }
            },
        }
    }
    Ok(links)
}

const TABLECHAR: u8 = b't';

fn parse_table_line(line: &[u8], start: usize) -> Result<Vec<Link>, LinkErr> {
    match line[0] {
        TABLECHAR => match parse_links(line, start) {
            Err(err) => Err(err),
            Ok(links) => Ok(links),
        },

        _ => Err(LinkErr::NotTalbeLine),
    }
}

fn get_link(buffer: &mut BufReader<File>, link: &Link) -> std::io::Result<Vec<Vec<u8>>> {
    let mut ret: Vec<Vec<u8>> = Vec::new();
    buffer.seek(SeekFrom::Current(link.name.pos as i64));
    let mut name: Vec<u8> = Vec::new();
    name.resize(link.name.len, b'0');
    match buffer.read_exact(&mut name) {
        Err(err) => return Err(err),
        Ok(_) => ret.push(name),
    };
    buffer.seek(SeekFrom::Current(1));
    let mut seek: Vec<u8> = Vec::new();
    seek.resize(link.seek.len, b'0');

    match buffer.read_exact(&mut seek) {
        Err(err) => return Err(err),
        Ok(_) => ret.push(seek),
    };
    Ok(ret)
}
