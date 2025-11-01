use crate::chunk::Chunk;
use crate::nbt::binary_reader::BinaryReader;
use crate::nbt::parse::parse_tag;
use crate::region_file::ParseError::VersionError;
use std::hash::Hasher;
use thiserror::Error;
use twox_hash::XxHash32;

#[derive(Error, Debug)]
pub enum ParseError {
    #[error("I/O error")]
    ReadError,
    #[error("Invalid file header!")]
    HeaderError,
    #[error("Target version is not supported!")]
    VersionError
}

pub struct Region {
    chunks: Vec<Chunk>,
    timestamp: i64
}

impl Region {
    pub fn from_bytes_linear_v2(bytes: &[u8]) -> Result<Self, ParseError> {
        let file_head = 0xc3ff13183cca9d9au64;
        let version = 0x03;

        let file_head_got = u64::from_be_bytes(bytes[0..8].try_into().unwrap());
        if file_head_got != file_head {
            return Err(ParseError::HeaderError);
        }

        let version_got = bytes[8];
        if version_got != version {
            return Err(VersionError);
        }

        let timestamp = i64::from_be_bytes(bytes[9..17].try_into().unwrap());

        let grid_size = bytes[17];
        let region_x = i32::from_be_bytes(bytes[18..22].try_into().unwrap());
        let region_z = i32::from_be_bytes(bytes[22..26].try_into().unwrap());

        let mut curr_read_pointer = 26 + 128;

        loop {
            let feature_name_length = bytes[curr_read_pointer];
            curr_read_pointer += 1;

            if feature_name_length == 0 {
                break;
            }

            curr_read_pointer += feature_name_length as usize;
            curr_read_pointer += 4;
        }

        let mut bucket_sizes: Vec<i32> = Vec::new();
        let mut bucket_compression_levels: Vec<u8> = Vec::new();

        for _ in 0..(grid_size as usize * grid_size as usize) {
            let size_this_bucket = i32::from_be_bytes(bytes[curr_read_pointer..curr_read_pointer + 4].try_into().unwrap());
            curr_read_pointer += 4;

            let compression_level_this_bucket = bytes[curr_read_pointer];
            curr_read_pointer += 1;

            curr_read_pointer += 8;

            bucket_sizes.push(size_this_bucket);
            bucket_compression_levels.push(compression_level_this_bucket);
        }

        let mut chunks = Vec::with_capacity(1024);

        for x in 0..(grid_size as i32) {
            for z in 0..(grid_size as i32) {
                let index = (x * grid_size as i32 + z) as usize;

                let bucket_data_len = *bucket_sizes.get(index).unwrap_or(&0);
                if bucket_data_len <= 0 {
                    continue;
                }

                let bucket_data_compressed = &bytes[curr_read_pointer..curr_read_pointer + bucket_data_len as usize];
                curr_read_pointer += bucket_data_len as usize;

                let decompressed = zstd::decode_all(bucket_data_compressed).unwrap();

                let mut read_pointer_this_loop = 0usize;
                let bucket_dim = 32 / grid_size as i32;

                for ix in 0..bucket_dim {
                    for iz in 0..bucket_dim {
                        let chunk_index = (x * bucket_dim + ix) + (z * bucket_dim + iz) * 32;

                        if read_pointer_this_loop + 12 > decompressed.len() {
                            break;
                        }

                        let chunk_size = i32::from_be_bytes(
                            decompressed[read_pointer_this_loop..read_pointer_this_loop + 4]
                                .try_into().unwrap()
                        );
                        read_pointer_this_loop += 4;

                        let chunk_timestamp = i64::from_be_bytes(
                            decompressed[read_pointer_this_loop..read_pointer_this_loop + 8]
                                .try_into().unwrap()
                        );
                        read_pointer_this_loop += 8;

                        if chunk_size <= 0 {
                            continue;
                        }

                        let chunk_data_size = (chunk_size - 8) as usize;

                        let chunk_data = &decompressed[read_pointer_this_loop..read_pointer_this_loop + chunk_data_size];
                        read_pointer_this_loop += chunk_data_size;

                        let global_x = 32 * region_x + (chunk_index % 32);
                        let global_z = 32 * region_z + (chunk_index / 32);

                        let parsed_data = parse_tag(&mut BinaryReader::new(&chunk_data));

                        chunks.push(Chunk::new_from_block_pos(global_x, global_z, chunk_timestamp, parsed_data));
                    }
                }
            }
        }

        Ok(Self {
            chunks,
            timestamp
        })
    }

    pub fn to_bytes_blinear(&self, timestamp: i64, compression_level: u8) -> Vec<u8>{
        let mut result = Vec::new();

        let file_head = -0x200812250269i64;
        let version = 0x02u8;
        let hash_seed = 0x0721i32 as u32;

        // whole file head part
        // 8 + 1 + 8 + 1
        let mut file_header = [0_u8; 18];

        file_header[0..8].copy_from_slice(&file_head.to_be_bytes()); // superblock
        file_header[8..9].copy_from_slice(&version.to_be_bytes()); // version
        file_header[9..17].copy_from_slice(&timestamp.to_be_bytes()); // master file timestamp
        file_header[17..18].copy_from_slice(&compression_level.to_be_bytes()); // compression level

        result.extend_from_slice(&file_header); // append file head

        let mut region_data = Vec::new();

        for index in 0..1024 {
            let mut target_chunk = None;

            for chunk in &self.chunks {
                if chunk.position_to_sector_index() == index {
                    target_chunk = Some(chunk);
                    break
                }
            }

            if target_chunk.is_none() {
                region_data.extend_from_slice(&0i32.to_be_bytes());
                continue;
            }

            let mut hasher = XxHash32::with_seed(hash_seed);

            let chunk_data = target_chunk.unwrap().to_raw_bytes(); // 3
            let length_of_chunk_data = (chunk_data.len() as i32).to_be_bytes(); // 0
            let timestamp_of_chunk = target_chunk.unwrap().timestamp().to_be_bytes(); // 1

            hasher.write(&chunk_data);
            let xxhash32_of_chunk_data = (hasher.finish() as i32).to_be_bytes(); // 2

            let mut local_temp_buffer = Vec::new();

            local_temp_buffer.extend_from_slice(&length_of_chunk_data); // len
            local_temp_buffer.extend_from_slice(&timestamp_of_chunk); // timestamp of chunk
            local_temp_buffer.extend_from_slice(&xxhash32_of_chunk_data); // xxhash32 of chunk data
            local_temp_buffer.extend_from_slice(&chunk_data); // chunk data

            region_data.extend_from_slice(&(local_temp_buffer.len() as i32).to_be_bytes());
            region_data.extend_from_slice(local_temp_buffer.as_slice());
        }

        if let Ok(compressed) = zstd::encode_all(region_data.as_slice(), compression_level as i32) {
            result.extend_from_slice(&compressed);
        }

        result
    }

    pub fn from_bytes_blinear(bytes: &[u8]) -> Result<Self, ParseError> {
        let mut chunk_sections = Vec::with_capacity(1024);

        // 8 + 1 + 8 + 1
        let file_head = i64::from_be_bytes(bytes[0..8].try_into().unwrap());
        let version = &bytes[8..9];

        // incorrect file
        if file_head != -0x200812250269 {
            return Err(ParseError::HeaderError);
        }

        if version[0] != 0x02 {
            return Err(VersionError);
        }

        let timestamp_of_master_file = i64::from_be_bytes(bytes[9..17].try_into().unwrap());
        let _compression_level = &bytes[17..18];

        let decompressed_region_sections_data = zstd::decode_all(&bytes[18..bytes.len()])
            .map_err(|_| ParseError::ReadError)?;

        let mut buffer_pointer = 0;
        for sector_index in 0..1024 {
            let sector_len = i32::from_be_bytes(decompressed_region_sections_data[buffer_pointer..buffer_pointer + 4].try_into().unwrap()) as usize;
            buffer_pointer += 4;

            if sector_len <= 0 {
                continue;
            }

            let section_data_this_section = &decompressed_region_sections_data[buffer_pointer..buffer_pointer + sector_len];
            buffer_pointer += sector_len;


            let _length_of_chunk = i32::from_be_bytes(section_data_this_section[0..4].try_into().unwrap()); // unused
            let timestamp_of_chunk = i64::from_be_bytes(section_data_this_section[4..12].try_into().unwrap());
            let _xxhash32_of_chunk = i32::from_be_bytes(section_data_this_section[12..16].try_into().unwrap()); // unused

            let data_of_chunk = &section_data_this_section[16..section_data_this_section.len()];

            if let Ok(chunk) = Chunk::from_sector(sector_index, timestamp_of_chunk, data_of_chunk) {
                chunk_sections.push(chunk);
            }
        }

        Ok(Self{
            chunks: chunk_sections,
            timestamp: timestamp_of_master_file
        })
    }
}