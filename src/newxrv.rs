use std::io::prelude::*;
use std::{collections::HashMap, fs::File, io::BufReader};

#[derive(Debug)]
enum LineKind {
    Jump,
    Table,
    Style,
    Record,
}

enum ExpectField {
    Name,
    Colon,
    Value,
    Skip,
    Qoute,
}

#[derive(Debug)]
struct LineField<'b> {
    buffer: &'b [u8],
    kind: LineKind,
    name: &'b str,
    fields: Vec<Field<'b>>,
}

#[derive(Debug, Clone)]
struct Field<'b> {
    name: &'b str,
    value: &'b str,
}

#[derive(Debug)]

struct Jump<'b> {
    name: &'b str,
    seek: usize,
    len: usize,
}

#[derive(Debug)]
struct LineJump<'b> {
    buffer: &'b [u8],
    kind: LineKind,
    name: &'b str,
    jumps: Vec<Jump<'b>>,
}

#[derive(Debug)]
struct LineLink<'b> {
    buffer: &'b [u8],
    kind: LineKind,
    name: &'b [u8],
    links: Vec<Link>,
}

#[derive(Debug)]
struct Link {
    name_start: usize,
    name_end: usize,
    value_start: usize,
    value_end: usize,
}

impl<'b, 'l> TryFrom<LineLink<'l>> for LineJump<'b>
where
    LineLink<'l>: 'b,
{
    type Error = XRVErr;
    fn try_from(value: LineLink<'l>) -> Result<Self, Self::Error> {
        match std::str::from_utf8(value.name) {
            Err(_) => return Err(XRVErr::CantParseFieldName),
            Ok(s) => match s {
                "jumps" => {
                    let mut jumps: Vec<Jump<'b>> = Vec::new();
                    for link in value.links {
                        let name: &'b str = match std::str::from_utf8(
                            &value.buffer[link.name_start..link.name_end],
                        ) {
                            Err(_) => return Err(XRVErr::CantParseFieldStrName),
                            Ok(s) => s,
                        };

                        let value: &'b str = match std::str::from_utf8(
                            &value.buffer[link.value_start..link.value_end],
                        ) {
                            Err(_) => return Err(XRVErr::CantParseFieldStrValue),
                            Ok(s) => s,
                        };

                        let split: Vec<&'b str> = value.split("-").collect();
                        let seek = match split[0].parse::<usize>() {
                            Err(_) => return Err(XRVErr::CantParseFieldUsizeValue),
                            Ok(u) => u,
                        };
                        let len = match split[0].parse::<usize>() {
                            Err(_) => return Err(XRVErr::CantParseFieldUsizeValue),
                            Ok(u) => u,
                        };

                        jumps.push(Jump { name, seek, len });
                    }
                    return Ok(LineJump {
                        buffer: value.buffer,
                        kind: LineKind::Jump,
                        name: "jumps",
                        jumps,
                    });
                }
                _ => return Err(XRVErr::ItsNotAJumpsLine),
            },
        };
    }
}

impl<'b, 'l> TryFrom<LineLink<'l>> for LineField<'b>
where
    LineLink<'l>: 'b,
{
    type Error = XRVErr;
    fn try_from(value: LineLink<'b>) -> Result<Self, Self::Error> {
        let mut fields: Vec<Field<'b>> = Vec::new();
        let linename: &str = match std::str::from_utf8(value.name) {
            Err(_) => return Err(XRVErr::CantParseFieldName),
            Ok(s) => s,
        };
        for link in value.links {
            let name: &'b str =
                match std::str::from_utf8(&value.buffer[link.name_start..link.name_end]) {
                    Err(_) => return Err(XRVErr::CantParseFieldStrName),
                    Ok(s) => s,
                };
            let value: &'b str =
                match std::str::from_utf8(&value.buffer[link.value_start..link.value_end]) {
                    Err(_) => return Err(XRVErr::CantParseFieldStrValue),
                    Ok(s) => s,
                };
            fields.push(Field { name, value });
        }

        Ok(Self {
            buffer: value.buffer,
            kind: value.kind,
            name: linename,
            fields,
        })
    }
}

struct Pair {
    start: usize,
    end: usize,
}

const TABLE_ID: u8 = b't';
const STYLE_ID: u8 = b's';
const RECORD_ID: u8 = b'r';

const COLON_CHAR: u8 = b':';
const QUOTE_CHAR: u8 = b'"';
const SPACE_CHAR: u8 = b' ';
const CR_CHAR: u8 = b'\r';
const NL_CHAR: u8 = b'\n';

impl<'b> TryFrom<Vec<u8>> for LineLink<'b> {
    type Error = XRVErr;
    fn try_from(value: Vec<u8>) -> Result<Self, XRVErr> {
        let mut state = ExpectField::Name;
        let mut seek: usize = 0;
        let mut idx: usize = 0;

        let mut pairs: Vec<Pair> = Vec::new();
        for byte in &value {
            match state {
                ExpectField::Name => match *byte {
                    COLON_CHAR | QUOTE_CHAR | CR_CHAR | NL_CHAR => {
                        return Err(XRVErr::ExpectSpaceOrAlpha)
                    }
                    SPACE_CHAR => continue,
                    _ => {
                        seek = idx;
                        state = ExpectField::Colon;
                    }
                },
                ExpectField::Colon => match *byte {
                    COLON_CHAR => {
                        pairs.push(Pair {
                            start: seek,
                            end: idx,
                        });
                        seek = idx + 1;
                        state = ExpectField::Value;
                    }
                    QUOTE_CHAR => return Err(XRVErr::NameMustNotContainQoutes),
                    SPACE_CHAR | CR_CHAR | NL_CHAR => return Err(XRVErr::NameMustFolowedByColon),
                    _ => continue,
                },
                ExpectField::Value => match *byte {
                    COLON_CHAR | SPACE_CHAR | CR_CHAR | NL_CHAR => return Err(XRVErr::ExpectAlpha),
                    QUOTE_CHAR => {
                        seek += 1;
                        state = ExpectField::Qoute;
                    }
                    _ => state = ExpectField::Skip,
                },
                ExpectField::Skip => match *byte {
                    COLON_CHAR | QUOTE_CHAR => return Err(XRVErr::ExpectingSpaceOrNewline),
                    SPACE_CHAR => {
                        pairs.push(Pair {
                            start: seek,
                            end: idx,
                        });
                        seek = idx;
                        state = ExpectField::Name;
                    }
                    CR_CHAR | NL_CHAR => {
                        pairs.push(Pair {
                            start: seek,
                            end: idx,
                        });
                        break;
                    }
                    _ => continue,
                },
                ExpectField::Qoute => match *byte {
                    COLON_CHAR | SPACE_CHAR => continue,
                    CR_CHAR | NL_CHAR => return Err(XRVErr::ExpectingQouteNotNewline),
                    QUOTE_CHAR => {
                        pairs.push(Pair {
                            start: seek,
                            end: idx,
                        });
                        seek = idx + 1;
                        state = ExpectField::Skip;
                    }
                    _ => continue,
                },
            };
            idx += 1;
        }

        let mut links: Vec<Link> = Vec::new();
        let mut pairs_it = pairs.into_iter();

        let kind: LineKind = match pairs_it.next() {
            None => return Err(XRVErr::FailToGetLineKind),
            Some(k) => match &value[k.start..k.end] {
                [b't'] => LineKind::Table,
                [b's'] => LineKind::Style,
                [b'r'] => LineKind::Record,
                _ => return Err(XRVErr::UnkwnownLineKind),
            },
        };

        let name: &'b [u8] = match pairs_it.next() {
            None => return Err(XRVErr::FailToGetLineName),
            Some(n) => &value[n.start..n.end],
        };

        loop {
            match pairs_it.next() {
                None => break,
                Some(name) => match pairs_it.next() {
                    None => return Err(XRVErr::FailedToConsumePairs),
                    Some(value) => links.push(Link {
                        name_start: name.start,
                        name_end: name.end,
                        value_start: value.start,
                        value_end: value.end,
                    }),
                },
            }
        }

        Ok(Self {
            buffer: &value,
            kind,
            name: &name,
            links,
        })
    }
}

impl<'b> TryInto<usize> for Field<'b> {
    type Error = XRVErr;
    fn try_into(self) -> Result<usize, Self::Error> {
        match self.value.parse::<usize>() {
            Err(_) => Err(XRVErr::CantParseFieldUsizeValue),
            Ok(u) => Ok(u),
        }
    }
}

#[derive(Debug)]
struct TableLine<'b> {
    id: &'b str,
    name: &'b str,
    pos: usize,
    len: usize,
    cols: Vec<Field<'b>>,
}

impl<'b> TryFrom<LineField<'b>> for TableLine<'b> {
    type Error = XRVErr;
    fn try_from(value: LineField<'b>) -> Result<Self, Self::Error> {
        match value.kind {
            LineKind::Table => {
                let id: &'b str = value.fields[0].value;
                let name: &'b str = match value.fields[1].name {
                    "name" => value.fields[1].value,
                    _ => return Err(XRVErr::FirstTableFieldMustBeName),
                };

                let pos: usize = match value.fields[2].name {
                    "pos" => value.fields[2].clone().try_into()?,
                    _ => return Err(XRVErr::SecondTableFieldMustBePos),
                };
                let len: usize = match value.fields[3].name {
                    "len" => value.fields[3].clone().try_into()?,
                    _ => return Err(XRVErr::ThirdTableFieldMustBeLen),
                };

                let mut cols: Vec<Field<'b>> = value.fields[4..].to_owned();

                Ok(TableLine {
                    id,
                    name,
                    pos,
                    len,
                    cols,
                })
            }
            _ => return Err(XRVErr::NotTableLine),
        }
    }
}

struct StyleLine<'b> {
    id: &'b str,
    cols: Vec<Field<'b>>,
}

impl<'b> TryFrom<LineField<'b>> for StyleLine<'b> {
    type Error = XRVErr;
    fn try_from(value: LineField<'b>) -> Result<Self, Self::Error> {
        match value.kind {
            LineKind::Style => {
                let id: &'b str = value.fields[0].value;
                let mut cols: Vec<Field<'b>> = Vec::new();
                for col in value.fields[1..].iter() {
                    cols.push(col.clone());
                }

                Ok(StyleLine { id, cols })
            }
            _ => return Err(XRVErr::NotStyleLine),
        }
    }
}

struct RecordLine<'b> {
    id: &'b str,
    cols: Vec<Field<'b>>,
}

impl<'b> TryFrom<LineField<'b>> for RecordLine<'b> {
    type Error = XRVErr;
    fn try_from(value: LineField<'b>) -> Result<Self, Self::Error> {
        match value.kind {
            LineKind::Record => {
                let id: &'b str = value.fields[0].value;
                let mut cols: Vec<Field<'b>> = Vec::new();
                for col in value.fields[1..].iter() {
                    cols.push(col.clone());
                }

                Ok(RecordLine { id, cols })
            }
            _ => Err(XRVErr::NotRecordLine),
        }
    }
}

const DEFAULT_XRAVE_NEW_BUFFER_CAPACITY: usize = 4 * 1024;

#[derive(Debug)]
struct XraveBuffer {
    buffer: Vec<u8>,
    line: usize,
}

impl XraveBuffer {
    fn new() -> Self {
        XraveBuffer {
            buffer: Vec::new(),
            line: 0,
        }
    }
}

#[derive(Debug)]
pub struct Reader<'b> {
    buffer: XraveBuffer,
    line_jump: LineJump<'b>,
}

impl<'b> Reader<'b> {
    pub fn new(path: String) -> Result<Reader<'b>, XRVErr> {
        match File::open(path) {
            Err(err) => Err(XRVErr::FailToOpenFile(err)),
            Ok(mut file) => {
                let mut meta = Vec::with_capacity(DEFAULT_XRAVE_NEW_BUFFER_CAPACITY);
                file.read_exact(&mut meta);
                let line_link: LineLink<'b> = meta.try_into()?;
                let line_jump: LineJump<'b> = line_link.try_into()?;
                Ok(Reader {
                    buffer: XraveBuffer::new(),
                    line_jump,
                })
            }
        }
    }
}

#[derive(Debug)]
pub enum XRVErr {
    FailToOpenFile(std::io::Error),
    NameMustFolowedByColon,
    NameMustNotContainQoutes,
    ExpectSpaceOrAlpha,
    ExpectAlpha,
    ExpectingSpaceOrNewline,
    ExpectingQouteNotNewline,
    FailedToConsumePairs,
    FailToGetLineKind,
    FailToGetLineName,
    NotTableLine,
    CantParseFieldUsizeValue,
    CantParseFieldStrName,
    CantParseFieldStrValue,
    CantParseFieldName,
    FirstTableFieldMustBeName,
    SecondTableFieldMustBePos,
    ItsNotAJumpsLine,
    NotStyleLine,
    NotRecordLine,
    UnkwnownLineKind,
    ThirdTableFieldMustBeLen,
}
