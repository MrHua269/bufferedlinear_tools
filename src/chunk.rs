use crate::nbt::binary_reader::BinaryReader;
use crate::nbt::parse::parse_tag;
use crate::nbt::tag::Tag;

pub struct Chunk{
    position: i64,
    timestamp: i64,
    pub data: Tag
}

impl Chunk {

    pub fn from_region_index(
        chunk_index: usize,
        region_x: i32,
        region_z: i32,
        timestamp: i64,
        data: &[u8]
    ) -> Result<Self, &'static str> {
        let parsed_data = parse_tag(&mut BinaryReader::new(&data));

        let local_x = (chunk_index % 32) as i32;
        let local_z = (chunk_index / 32) as i32;

        let global_x = 1024 * region_x + local_x;
        let global_z = 1024 * region_z + local_z;

        Ok(Self::new_from_block_pos(global_x, global_z, timestamp, parsed_data))
    }

    pub fn from_sector(sector_index: i32, timestamp: i64, data: &[u8]) -> Result<Self, &'static str> {
        let parsed_data = parse_tag(&mut BinaryReader::new(&data));

        let x = sector_index & 31;
        let z = (sector_index >> 5) & 31;

        Ok(Self::new_from_block_pos(x, z, timestamp, parsed_data))
    }

    pub fn to_raw_bytes(&self) -> Vec<u8> {
        self.data.to_bytes()
    }

    pub fn new(position: i64, timestamp: i64, data: Tag) -> Self {
        Self {
            position,
            timestamp,
            data
        }
    }

    pub fn new_from_block_pos(x: i32, z: i32, timestamp: i64, data: Tag) -> Self {
        let position = ((x as i64) << 32) | (z as i64 & 0xFFFFFFFF);

        Self {
            position,
            timestamp,
            data
        }
    }

    pub fn position_to_sector_index(&self) -> i32 {
        let x = &self.x();
        let z = &self.z();

        ((x & 31) as usize + ((z & 31) as usize) << 5) as i32
    }

    pub fn x(&self) -> i32 {
        (self.position as u32) as i32
    }

    pub fn z(&self) -> i32 {
        ((self.position as u64 >> 32) as u32) as i32
    }

    pub fn get_data(&self) -> &Tag {
        &self.data
    }

    pub fn timestamp(&self) -> i64 {
        self.timestamp
    }
}