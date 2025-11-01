use crate::nbt::binary_reader::BinaryReader;
use crate::nbt::parsers::parse_with_type::parse_with_type;
use crate::nbt::tag::Tag;

pub fn parse_tag(reader: &mut BinaryReader) -> Tag {
    let tag_type = reader.read_type();
    parse_with_type(reader, tag_type, false)
}
