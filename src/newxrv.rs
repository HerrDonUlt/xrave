use crate::xrv::XRVErr;

enum LineKind {
    Table,
    Style,
    Record,
}

// enum TableLine {
//     LineDesc,
//     LinePos,
//     LineLen,
//     TableCol,
// }

enum StyleLine {
    StyleProp,
}

enum ExpectField {
    Name,
    Colon,
    Value,
    Skip,
    Qoute,
}

struct LineField<'b> {
    kind: LineKind,
    name: &'b str,
    fields: Vec<Field<'b>>,
}

struct Field<'b> {
    name: &'b str,
    value: &'b str,
}

struct LineLink<'b> {
    buffer: &'b [u8],
    kind: LineKind,
    name: &'b [u8],
    links: Vec<Link>,
}

struct Link {
    name_start: usize,
    name_end: usize,
    value_start: usize,
    value_end: usize,
}

impl<'b> TryFrom<LineLink<'b>> for LineField<'b> {
    type Error = XRVErr;
    fn try_from(value: LineLink) -> Result<Self, Self::Error> {
        let mut fields: Vec<Field<'b>> = Vec::new();
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

        let linename: &'b str = match std::str::from_utf8(value.name) {
            Err(_) => return Err(XRVErr::CantParseFieldName),
            Ok(s) => s,
        };
        Ok(Self {
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

impl<'b> TryFrom<&'b [u8]> for LineLink<'b> {
    type Error = XRVErr;
    fn try_from(value: &'b [u8]) -> Result<Self, XRVErr> {
        let mut state = ExpectField::Name;
        let it = value.into_iter();
        let mut seek: usize = 0;
        let mut idx: usize = 0;

        let mut pairs: Vec<Pair> = Vec::new();
        for byte in value {
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
                },
            };
            idx += 1;
        }

        let mut links: Vec<Link> = Vec::new();
        let mut pairs_it = pairs.into_iter();

        let kind: LineKind = match pairs_it.next() {
            None => return Err(XRVErr::FailToGetLineKind),
            Some(k) => match value[k.start..k.end] {
                [b't'] => LineKind::Table,
                [b's'] => LineKind::Style,
                [b'r'] => LineKind::Record,
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

                    _ => return Err(XRVErr::FirstFieldMustBeName),
                };

                let pos: usize = match value.fields[2].name {
                    "pos" => value.fields[2].try_into()?,
                    _ => return Err(XRVErr::SecondFieldMustBePos),
                };
                let len: usize = match value.fields[3].name {
                    "len" => value.fields[3].try_into()?,
                    _ => return Err(XRVErr::SecondFieldMustBePos),
                };

                let mut cols: Vec<Field<'b>> = Vec::new();

                for col in value.fields[4..].iter() {
                    cols.push(*col);
                }

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
