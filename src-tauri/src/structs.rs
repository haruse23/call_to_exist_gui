use std::fs::File;

use std::io::{Cursor, Read, Seek, SeekFrom, Write};

use crate::{read_u8, read_u16, read_u32, read_u64, read_f32, read_bytes, read_cstring, align};

use std::collections::HashMap;

use crate::enums::{DataStorage, DataType, RowData, NodeType};

use bitstream_io::{BitReader, BitRead, BigEndian};

use pyo3::prelude::*;

pub struct Packet {
    pub magic: u32,
    pub packet_mode: u32,
	pub packet_size: u64,
	pub packet_data: Vec<u8>,
}

impl Packet {
		pub fn build (file: &mut File) -> Self{
			let magic = read_u32(file, "Little");
			let packet_mode = read_u32(file, "Little");
			let packet_size = read_u64(file, "Little");

			let packet_data = read_bytes(file, packet_size);

			Self {
				magic,
				packet_mode,
				packet_size,
				packet_data,
			}
			
			
		}
		
		
		pub fn decrypt (&mut self) {
			let mut m: u32 = 0x0000655F;
			let t: u32 = 0x00004115;
			
			if self.packet_mode == 0 {
				for byte in &mut self.packet_data {
					*byte = *byte ^ (m & 0xFF) as u8;
					m = m.wrapping_mul(t); // Keep only the part that fits in m as u32
				
				
				}
			}
		
		}
		

}


pub struct UTFPacket {
	// UTF Header
	pub magic: u32,
	pub TableSize: u32,
	
	// Table Header
	pub RowsOffset: u32,
	pub StringsOffset: u32,
	pub DataOffset: u32,
	pub TableName: u32,
	pub ColumnsNumber: u16,
	pub RowLength: u16,
	pub RowsNumber: u32,
	
	pub UTFColumns: Vec< HashMap<String, Option<RowData> > >,
	
	pub UTFRowsOffsets: Vec<u64>,
	
	pub column_instances: Vec<Column>,
	
	pub UTFRowsData: Vec< HashMap<String, Option<RowData> > >,
	
}


impl UTFPacket {
		pub fn build(file: &mut Cursor<&Vec<u8>>) -> Self {
			// UTF Header
			let mut magic = read_u32(file, "Big");
			let mut TableSize = read_u32(file, "Big");
			// Table Header
			let mut RowsOffset = read_u32(file, "Big");
			let mut StringsOffset = read_u32(file, "Big");
			let mut DataOffset = read_u32(file, "Big");
			let mut TableName = read_u32(file, "Big");
			let mut ColumnsNumber = read_u16(file, "Big");
			let mut RowLength = read_u16(file, "Big");
			let mut RowsNumber = read_u32(file, "Big");
			
			// Table starts after UTFHeader which is 8 bytes, so add 8 to make an absolute offset
			RowsOffset += 8;
			StringsOffset += 8;
			DataOffset += 8;
			
			Self { // UTF Header
				magic: magic,
				TableSize: TableSize,
				
				// Table Header
				RowsOffset: RowsOffset,
				StringsOffset: StringsOffset,
				DataOffset: DataOffset,
				TableName: TableName,
				ColumnsNumber: ColumnsNumber,
				RowLength: RowLength,
				RowsNumber: RowsNumber,
				
				UTFColumns: vec![],
				
				UTFRowsOffsets: vec![],
				
				column_instances: vec![],
				
				UTFRowsData: vec![],
			
			}
				
				
		}
		
		
		pub fn read_column_definition(&mut self, file: &mut Cursor<&Vec<u8>>) {
			for i in 0..self.ColumnsNumber {
				let mut ColumnFlag = read_u8(file, "Big");
				
				if ColumnFlag == 0 {
					file.seek( SeekFrom::Current(3) ).unwrap();
					ColumnFlag = read_u8(file, "Big");
				}
				
				let mut ColumnNameOffset = read_u32(file, "Big");
				
				let mut ColumnValue: Option<RowData> = None;
				
				if ColumnFlag & DataStorage::STORAGE_MASK as u8 == DataStorage::STORAGE_CONSTANT as u8 {
					ColumnValue = Some( self.parse_row_data(file, ColumnFlag & DataType::DATATYPE_MASK as u8) );
				
				}
				
				
				let mut map: HashMap<String, Option<RowData>> = HashMap::new();

				map.insert("index".to_string(), Some(RowData::UInt16(i)) );
				map.insert("flag".to_string(), Some(RowData::UInt8(ColumnFlag)) );
				map.insert("name_offset".to_string(), Some(RowData::UInt32(ColumnNameOffset)) );
				map.insert("column_constant_value".to_string(), ColumnValue);
				
				self.UTFColumns.push(map);
			
			}
			
		}
			
			
		pub fn read_rows(&mut self, file: &mut Cursor<&Vec<u8>>) {
			for j in 0..self.RowsNumber {
				
				self.UTFRowsOffsets.push( file.stream_position().unwrap() );
				
				file.seek( SeekFrom::Current(self.RowLength as i64) ).unwrap();
			
			}
			
			
			
		}
		
		
		
		pub fn parse_row_data(&self, file: &mut Cursor<&Vec<u8>>, data_type: u8) -> RowData {
			match data_type {
				0 | 1 => {
					RowData::UInt8(read_u8(file, "Big"))
				}

				2 | 3 => {
					RowData::UInt16(read_u16(file, "Big"))
				}

				4 | 5 => {
					RowData::UInt32(read_u32(file, "Big"))
				}

				6 | 7 => {
					RowData::UInt64(read_u64(file, "Big"))
				}

				8 => {
					RowData::Float(read_f32(file, "Big"))
				}

				0xA => {
					let offset = read_u32(file, "Big");

					let current_position = file.stream_position().unwrap();

					file.seek(SeekFrom::Start(
						(self.StringsOffset + offset) as u64
					)).unwrap();

					let string = read_cstring(file);

					file.seek(SeekFrom::Start(current_position)).unwrap();

					RowData::String(string)
				}

				0xB => {
					let offset = read_u32(file, "Big");
					let size = read_u32(file, "Big");

					let current_position = file.stream_position().unwrap();

					file.seek(SeekFrom::Start(
						(self.DataOffset + offset) as u64
					)).unwrap();

					let bytes = read_bytes(file, size as u64);

					file.seek(SeekFrom::Start(current_position)).unwrap();

					RowData::ByteArray(bytes)
				}
				
				_ => panic!("Unknown data type: {}", data_type)
				
				
			}
		
		}
		
		
		pub fn parse_decrypted_packet(&mut self, file: &mut Cursor<&Vec<u8>>) {
			for hashmap in &self.UTFColumns {
				let flag = match hashmap.get("flag") {
					Some(Some(RowData::UInt8(value))) => *value,
					_ => panic!("Invalid flag"),
				};
				
				let data_storage = flag & DataStorage::STORAGE_MASK as u8;
			
				let data_type = flag & DataType::DATATYPE_MASK as u8;
				
				let name_offset = match hashmap.get("name_offset") {
					Some(Some(RowData::UInt32(value))) => *value,
					_ => panic!("name_offset is not a UInt32"),
				};
				
				file.seek( SeekFrom::Start( (self.StringsOffset + name_offset) as u64) ).unwrap();
				
				let name = read_cstring(file);
				
				let column = Column {
					DataStorage: data_storage,
					DataType: data_type,
					Name: name,
					ColumnConstantValue: hashmap.get("column_constant_value").cloned().flatten()
				};
				
				&mut self.column_instances.push(column);
				
			}
			
			for (k, offset) in self.UTFRowsOffsets.iter().enumerate() {
				let mut hashmap = HashMap::new();
				
				file.seek( SeekFrom::Start(*offset as u64) ).unwrap();
				
				for column_instance in &self.column_instances {
					match column_instance.DataStorage {
						0x00 => hashmap.insert(column_instance.Name.clone(), None), // STORAGE_NONE
						
						0x10 => hashmap.insert(column_instance.Name.clone(), Some( RowData::UInt8(0) ) ), // STORAGE_ZERO
						
						0x30 => hashmap.insert(column_instance.Name.clone(), column_instance.ColumnConstantValue.clone() ), // STORAGE_CONSTANT
						
						0x50 => hashmap.insert(column_instance.Name.clone(), Some( self.parse_row_data(file, column_instance.DataType) ) ),
						
						 _ => panic!("Unknown DataStorage: {:02X}", column_instance.DataStorage),
						 
					};
					
					
					
				
				}
				
				self.UTFRowsData.push(hashmap);
		
			}
		}
	
	
}



pub struct Column {
	pub DataStorage: u8,
	pub DataType: u8,
	pub Name: String,
	pub ColumnConstantValue: Option<RowData>,
	
}
	
#[derive(serde::Serialize, serde::Deserialize)]
pub struct TreeNode {
    pub key: String,
    pub label: String,
    pub node_type: NodeType,
    pub children: Vec<TreeNode>,
	pub path: String,
	pub contained_type: String
}

impl TreeNode {
    pub fn get_file_offset(&self) -> u64 {
        match &self.node_type {
            NodeType::File { file_offset, .. } => *file_offset,
            NodeType::Folder => 0,
        }
    }

    pub fn get_file_size(&self) -> u32 {
        match &self.node_type {
            NodeType::File { file_size, .. } => *file_size,
            NodeType::Folder => 0,
        }
    }

    pub fn get_extract_size(&self) -> u32 {
        match &self.node_type {
            NodeType::File { extract_size, .. } => *extract_size,
            NodeType::Folder => 0,
        }
    }
}

#[derive(serde::Serialize, serde::Deserialize)]
pub struct DataTableRow {
    pub dir_name: String,
    pub file_name: String,
    pub file_size: u32,
	pub extract_size: u32,
    pub file_offset: u32,
	pub path: String
}


#[derive(serde::Serialize)]
pub struct CPKResult {
    pub tree: Vec<TreeNode>,
    pub rows: Vec<DataTableRow>,
}


pub struct Crilayla {
	magic: u64,
	uncompressed_size: u32,
	compressed_size: u32,
	
	compressed_data: Vec<u8>,
	uncompressed_data_header: Vec<u8>,
	
	output_decompressed: Cursor<Vec<u8>>
	
}


impl Crilayla {
	pub fn build(file: &mut Cursor<Vec<u8>> ) -> Self {
		let magic = read_u64(file, "Little");
		let uncompressed_size = read_u32(file, "Little");
		let compressed_size = read_u32(file, "Little");
		
		let compressed_data = read_bytes(file, compressed_size as u64);
		let uncompressed_data_header = read_bytes(file, 256);
		
		Self {
			magic: magic,
			uncompressed_size: uncompressed_size,
			compressed_size: compressed_size,
			
			compressed_data: compressed_data,
			uncompressed_data_header: uncompressed_data_header,
			
			output_decompressed: Cursor::new(Vec::new())
			
		}
		
	}
	
	pub fn levels() -> impl Iterator<Item = u32> { // 2, 3, 5, 8, 8, 8, ... and on
		[2, 3, 5, 8].into_iter().chain(std::iter::repeat(8))
	}
	
	pub fn decompress_crilayla(&mut self) -> Vec<u8> {
		let compressed_data = self.compressed_data.reverse();
		
		let mut bitreader = BitReader::endian(Cursor::new(&self.compressed_data), BigEndian);
		
		let minimal_reference_length: u16 = 3;
		
		while self.output_decompressed.get_ref().len() < self.uncompressed_size as usize {
			// read control bit
			let control_bit = if bitreader.read_bit().unwrap() { 1u8 } else { 0u8 }; // Convert the bool to u8
			
			if control_bit == 0 { // Literal
				let byte = bitreader.read::<8, u8>().unwrap(); // Read a byte and store it in the output
		
				self.output_decompressed.write_all(&[byte]).unwrap();
				
			}
				
				
			else if control_bit == 1 {
                // Back-reference
                let offset = bitreader.read::<13, u16>().unwrap() + minimal_reference_length; // Read 13 bits and add 3 (MINIMAL REFERENCE LENGTH) to get offset
                
                let mut reference_length = minimal_reference_length;
				
				for lvl in Crilayla::levels() {
					let value = match lvl {
						2 => bitreader.read::<2, u8>().unwrap(),
						3 => bitreader.read::<3, u8>().unwrap(),
						5 => bitreader.read::<5, u8>().unwrap(),
						8 => bitreader.read::<8, u8>().unwrap(),
						_ => unreachable!(),
					};
					
					reference_length += value as u16;
					
					if value != (2u32.pow(lvl as u32) - 1) as u8 { // if the bits of the value are not all 1s
						break;
					}
					
				}
				
				while reference_length > 0 {
					self.output_decompressed.seek( SeekFrom::Current(-(offset as i64) ) ).unwrap(); // Seek back to the start of referenced bytes
					
					let mut referenced_bytes = vec![0u8; reference_length as usize];

					let bytes_read = self.output_decompressed.read(&mut referenced_bytes).unwrap(); // Read up to reference_length bytes (if available)

					referenced_bytes.truncate(bytes_read); // Truncate the referenced_bytes vector to the size of the actual bytes_read
					
					self.output_decompressed.seek( SeekFrom::End(0) ).unwrap(); // Seek to end of the decompressed output buffer
					
					self.output_decompressed.write_all(&referenced_bytes).unwrap();
					
					reference_length -= referenced_bytes.len() as u16;
					
				}
			}  
		}
				
		let mut result = self.uncompressed_data_header.clone();

		result.extend(self.output_decompressed.get_ref().iter().rev());

		result
	
	
	
	
	
	
	
	}
	
	
	
}


pub struct PAC {
	pub magic: u32,
	pub file_num: u16,
	pub file_num1: u64,
	pub archive_name: String,
	pub offsets: Vec<u32>,
	
	pub has_extra_header: bool
	
}

impl PAC {
	pub fn build<R: Read + Seek>(file: &mut R) -> Self {
		let has_extra_header;
		
		let extra_1 = read_u32(file, "Little"); // 1380011330, BMAR
		
		println!("extra_1 = {}", extra_1);
		
		let magic;

		if extra_1 == 1380011330 {
			let extra_2 = read_u32(file, "Little");
			// println!("extra_2 = {}", extra_2);

			magic = read_u32(file, "Little");
			// println!("magic = {}", magic);
			
			has_extra_header = true;
		}
		else if extra_1 == 4411969 {
			magic = extra_1;
			// println!("magic = {}", magic);
			
			has_extra_header = false;
		}
		else {
			panic!("Invalid PAC header: {}", extra_1);
		}

		file.seek( SeekFrom::Current(2) ).unwrap(); // Seek by 2 bytes
		
		let file_num = read_u16(file, "Little");
		let file_num1 = read_u64(file, "Little");
		
		let archive_name = String::from_utf8_lossy(&read_bytes(file, 32)).trim_end_matches('\0').to_string();
		
		let mut offsets = vec![];
		
		for i in 0..file_num {
			if has_extra_header {
				offsets.push(read_u32(file, "Little") + 8);
			}
			
			else {
				offsets.push(read_u32(file, "Little"));
			}
			
		}
		
		if has_extra_header {
			align(file, 8);
		}
		
		else {
			align(file, 32);
		}
		
		Self {
			magic: magic,
			file_num: file_num,
			file_num1: file_num1,
			archive_name: archive_name,
			offsets: offsets,
			
			has_extra_header: has_extra_header
		}
		
	}
	
	pub fn get_file_num (&self) -> u16 {
		self.file_num
	}
	
	pub fn get_offsets (&self) -> Vec<u32> {
		self.offsets.clone()
	}
	
	pub fn get_archive_name (&self) -> String {
		self.archive_name.clone()
	}
	
	
	
}
	
	
pub struct PACFileHeader {
	pub contained_type: String,
	pub contained_id: u16,
	
	pub file_id: u16,
	
	pub filename_length: u16,
	
	pub filename: String
}


impl PACFileHeader {
	pub fn build<R: Read + Seek>(file: &mut R, has_extra_header: bool) -> Self {
		let contained_type = String::from_utf8_lossy(&read_bytes(file, 4)).trim_end_matches('\0').to_string();
	
		let contained_id = read_u16(file, "Little");
		
		let file_id = read_u16(file, "Little");
		
		file.seek( SeekFrom::Current(4) ).unwrap(); // Filename Hash ?
		
		file.seek( SeekFrom::Current(2) ).unwrap();
		
		let filename_length = read_u16(file, "Little");
		
		file.seek( SeekFrom::Current(4) ).unwrap();
		
		let filename = String::from_utf8_lossy(&read_bytes(file, filename_length as u64)).trim_end_matches('\0').to_string();
		
		if has_extra_header {
			align(file, 8);
		}
		
		else {
			align(file, 32);
		}
		
		Self {
			contained_type: contained_type,
			contained_id: contained_id,
			
			file_id: file_id,
			
			filename_length: filename_length,
			
			filename: filename
			
		}
		
	}
	
	
	pub fn get_contained_type(&self) -> String {
		self.contained_type.clone()
	}
	
	pub fn get_contained_id(&self) -> u16 {
		self.contained_id
	}
	
	pub fn get_file_id(&self) -> u16 {
		self.file_id
	}
	
	pub fn get_filename(&self) -> String {
		self.filename.clone()
	}
	
}



pub struct Texture {
	pub magic: u32,
	pub width: u16,
	pub height: u16,
	pub pitch: u16,
	pub mipmap_count: u16,
	
	pub header_size: u32,
	
	pub header_size_without_texture_name: u32,
	
	pub texture_name: String,
	
	pub pixel_data: Vec<u8>
	
}


impl Texture {
	pub fn build(file: &mut File) -> Self {
		let magic = read_u32(file, "Little");
		
		file.seek( SeekFrom::Current(4) ).unwrap();
		
		let width = read_u16(file, "Little");
		let height = read_u16(file, "Little");
		
		let pitch = read_u16(file, "Little");
		file.seek( SeekFrom::Current(2) ).unwrap();
		
		let mipmap_count = read_u16(file, "Little");
		file.seek( SeekFrom::Current(6) ).unwrap();
		
		let header_size = read_u32(file, "Little");
		file.seek( SeekFrom::Current(4) ).unwrap(); // FF 00 00 00
		
		file.seek( SeekFrom::Current(4) ).unwrap(); // Some Size ??
		
		let header_size_without_texture_name = read_u32(file, "Little");
		
		file.seek( SeekFrom::Current(8) ).unwrap();
		
		let texture_name_length = header_size - header_size_without_texture_name;
		let texture_name = String::from_utf8_lossy(&read_bytes(file, texture_name_length as u64)).trim_end_matches('\0').to_string();
	
		
		let mut pixel_data = Vec::new();
		file.read_to_end(&mut pixel_data).unwrap();
		
		
		Self {
			magic: magic,
			width: width,
			height: height,
			pitch: pitch,
			mipmap_count: mipmap_count,
			
			header_size: header_size,
			
			header_size_without_texture_name: header_size_without_texture_name,
			
			texture_name: texture_name,
			
			pixel_data: pixel_data
		}
		
	}
	
}


#[derive(Debug, Clone)]
#[pyclass]
pub struct BMLSection {
	#[pyo3(get)]
	pub name: String,
	
	#[pyo3(get)]
	pub size: u32,
	
	#[pyo3(get)]
	pub unk: u32,
	
	#[pyo3(get)]
	pub section_data_offset: u64,
	
	#[pyo3(get)]
	pub section_data: Vec<u8>
}


impl BMLSection {
	

	pub fn build<R: Read + Seek>(file: &mut R) -> Self {
		let name = String::from_utf8_lossy(&read_bytes(file, 4)).trim_end_matches('\0').to_string();
		
		let size = read_u32(file, "Little");
		
		let unk = read_u32(file, "Little"); // 262144
		
		/* println!(
			"Section '{}' | size = {} | position = {}",
			name,
			size,
			file.stream_position().unwrap()
		); */

		let section_data_offset = file.stream_position().unwrap();
		
		let section_data = read_bytes(file, size as u64);
		
		align(file, 4);
		
		Self {
			name: name,
			size: size,
			unk: unk,
			
			section_data_offset: section_data_offset,
			section_data: section_data
		}
		
	}
	
}


#[pyclass]
pub struct BMLMeshSection {
	
	#[pyo3(get)]
	pub MeshNameOffset: u64,
	
	#[pyo3(get)]
	pub MeshVertexBufferOffset: u64,
	
	#[pyo3(get)]
	pub MeshIndexBufferOffset: u64,
	
	#[pyo3(get)]
	pub MeshIndexBufferInfoOffset: u64,
	
	#[pyo3(get)]
	pub MeshBoneDataOffset: u64,
	
	#[pyo3(get)]
	pub MeshLOD1Offset: u64,
	
	#[pyo3(get)]
	pub MeshLOD2Offset: u64,
	
	#[pyo3(get)]
	pub MeshLOD3Offset: u64,
	
	#[pyo3(get)]
	pub MeshLOD4Offset: u64,
	
	#[pyo3(get)]
	pub unk_1: u64,
	
	#[pyo3(get)]
	pub IndexCount: u32,
	
	#[pyo3(get)]
	pub VertexCount: u32,
	
	#[pyo3(get)]
	pub IndexSize: u16,
	
	#[pyo3(get)]
	pub VertexStride: u16,
	
}


impl BMLMeshSection {
	

	pub fn build<R: Read + Seek>(file: &mut R) -> Self {

		let mesh_name_offset = read_u64(file, "Little");
		let mesh_vertex_buffer_offset = read_u64(file, "Little");
		let mesh_index_buffer_offset = read_u64(file, "Little");
		let mesh_index_buffer_info_offset = read_u64(file, "Little");
		
		let mesh_bonedata_offset = read_u64(file, "Little");
		
		let mesh_lod1_offset = read_u64(file, "Little");
		let mesh_lod2_offset = read_u64(file, "Little");
		let mesh_lod3_offset = read_u64(file, "Little");
		let mesh_lod4_offset = read_u64(file, "Little");
		
		let unk_1 = read_u64(file, "Little");
		
		let index_count = read_u32(file, "Little");
		let vertex_count = read_u32(file, "Little");
		
		let index_size = read_u16(file, "Little");
		let vertex_stride = read_u16(file, "Little");
		
		
		file.seek( SeekFrom::Current(12) ).unwrap(); // There still might be some unknown important data here
		
		
		Self {

			MeshNameOffset: mesh_name_offset,
			MeshVertexBufferOffset: mesh_vertex_buffer_offset,
			MeshIndexBufferOffset: mesh_index_buffer_offset,
			MeshIndexBufferInfoOffset: mesh_index_buffer_info_offset,
			
			MeshBoneDataOffset: mesh_bonedata_offset,
			
			MeshLOD1Offset: mesh_lod1_offset,
			MeshLOD2Offset: mesh_lod2_offset,
			MeshLOD3Offset: mesh_lod3_offset,
			MeshLOD4Offset: mesh_lod4_offset,
			
			unk_1: unk_1,
			
			IndexCount: index_count,
			VertexCount: vertex_count,
			
			IndexSize: index_size,
			VertexStride: vertex_stride
			
		}
		
	}
	
	
}



#[derive(serde::Serialize, serde::Deserialize)]
#[pyclass]
#[derive(Clone)]
pub struct Vertex {
	#[pyo3(get)]
    positions: [f32; 3],
	
	#[pyo3(get)]
    normals: [f32; 4],
	
	#[pyo3(get)]
    binormals: [f32; 4],
	
	#[pyo3(get)]
    tangents: [f32; 4],

	#[pyo3(get)]
    texcoords: [f32; 2],
	
	#[pyo3(get)]
    colors: [f32; 4],
	
	#[pyo3(get)]
    blendweights: [f32; 4],
	
	#[pyo3(get)]
    blendindices: [u8; 4],
	
	#[pyo3(get)]
    blendweights_1: [f32; 4],
	
	#[pyo3(get)]
    blendindices_1: [u8; 4]
}


impl Vertex {
	

    pub fn build(VertexBuffer: &Vec<u8>, VertexCount: &u32, VertexStride: &u16) -> Vec<Vertex> {
        let mut Vertices = Vec::new();

        for i in (0..*VertexCount as usize * *VertexStride as usize).step_by(*VertexStride as usize) {

            // positions

            let px = f32::from_le_bytes(VertexBuffer[i..i+4].try_into().unwrap());
            let py = f32::from_le_bytes(VertexBuffer[i+4..i+8].try_into().unwrap());
            let pz = f32::from_le_bytes(VertexBuffer[i+8..i+12].try_into().unwrap());

            let positions = [px, py, pz];


            // blendweights and blendindices usage_index 0

            let bw1 = f32::from_le_bytes(VertexBuffer[i+12..i+16].try_into().unwrap());
            let bw2 = f32::from_le_bytes(VertexBuffer[i+16..i+20].try_into().unwrap());
            let bw3 = f32::from_le_bytes(VertexBuffer[i+20..i+24].try_into().unwrap());
            let bw4 = f32::from_le_bytes(VertexBuffer[i+24..i+28].try_into().unwrap());

            let blendweights = [bw1, bw2, bw3, bw4];


            let bi1 = VertexBuffer[i+28];
            let bi2 = VertexBuffer[i+29];
            let bi3 = VertexBuffer[i+30];
            let bi4 = VertexBuffer[i+31];

            let blendindices = [bi1, bi2, bi3, bi4];


            // blendweights and blendindices usage_index 1

            let bw1_1 = f32::from_le_bytes(VertexBuffer[i+32..i+36].try_into().unwrap());
            let bw2_1 = f32::from_le_bytes(VertexBuffer[i+36..i+40].try_into().unwrap());
            let bw3_1 = f32::from_le_bytes(VertexBuffer[i+40..i+44].try_into().unwrap());
            let bw4_1 = f32::from_le_bytes(VertexBuffer[i+44..i+48].try_into().unwrap());

            let blendweights_1 = [bw1_1, bw2_1, bw3_1, bw4_1];


            let bi1_1 = VertexBuffer[i+48];
            let bi2_1 = VertexBuffer[i+49];
            let bi3_1 = VertexBuffer[i+50];
            let bi4_1 = VertexBuffer[i+51];

            let blendindices_1 = [bi1_1, bi2_1, bi3_1, bi4_1];


            // normals

            let bits = u16::from_le_bytes([VertexBuffer[i+52], VertexBuffer[i+53]]);
            let value = half::f16::from_bits(bits);
            let nx: f32 = value.into();

            let bits = u16::from_le_bytes([VertexBuffer[i+54], VertexBuffer[i+55]]);
            let value = half::f16::from_bits(bits);
            let ny: f32 = value.into();

            let bits = u16::from_le_bytes([VertexBuffer[i+56], VertexBuffer[i+57]]);
            let value = half::f16::from_bits(bits);
            let nz: f32 = value.into();

            let bits = u16::from_le_bytes([VertexBuffer[i+58], VertexBuffer[i+59]]);
            let value = half::f16::from_bits(bits);
            let nw: f32 = value.into();

            let normals = [nx, ny, nz, nw];


            // colors, R8G8B8A8_UNORM

            let c1 = VertexBuffer[i+60] as f32 / 255.0;
            let c2 = VertexBuffer[i+61] as f32 / 255.0;
            let c3 = VertexBuffer[i+62] as f32 / 255.0;
            let c4 = VertexBuffer[i+63] as f32 / 255.0;

            let colors = [c1, c2, c3, c4];


            // texcoords

            let bits = u16::from_le_bytes([VertexBuffer[i+64], VertexBuffer[i+65]]);
            let value = half::f16::from_bits(bits);
            let t1: f32 = value.into();

            let bits = u16::from_le_bytes([VertexBuffer[i+66], VertexBuffer[i+67]]);
            let value = half::f16::from_bits(bits);
            let t2: f32 = value.into();

            let texcoords = [t1, t2];


            // tangents

            let bits = u16::from_le_bytes([VertexBuffer[i+68], VertexBuffer[i+69]]);
            let value = half::f16::from_bits(bits);
            let tx: f32 = value.into();

            let bits = u16::from_le_bytes([VertexBuffer[i+70], VertexBuffer[i+71]]);
            let value = half::f16::from_bits(bits);
            let ty: f32 = value.into();

            let bits = u16::from_le_bytes([VertexBuffer[i+72], VertexBuffer[i+73]]);
            let value = half::f16::from_bits(bits);
            let tz: f32 = value.into();

            let bits = u16::from_le_bytes([VertexBuffer[i+74], VertexBuffer[i+75]]);
            let value = half::f16::from_bits(bits);
            let tw: f32 = value.into();

            let tangents = [tx, ty, tz, tw];


            // binormals

            let bits = u16::from_le_bytes([VertexBuffer[i+76], VertexBuffer[i+77]]);
            let value = half::f16::from_bits(bits);
            let bx: f32 = value.into();

            let bits = u16::from_le_bytes([VertexBuffer[i+78], VertexBuffer[i+79]]);
            let value = half::f16::from_bits(bits);
            let by: f32 = value.into();

            let bits = u16::from_le_bytes([VertexBuffer[i+80], VertexBuffer[i+81]]);
            let value = half::f16::from_bits(bits);
            let bz: f32 = value.into();

            let bits = u16::from_le_bytes([VertexBuffer[i+82], VertexBuffer[i+83]]);
            let value = half::f16::from_bits(bits);
            let bw: f32 = value.into();

            let binormals = [bx, by, bz, bw];


            let Vertex = Self {
                positions: positions,

                blendweights: blendweights,
                blendindices: blendindices,

                blendweights_1: blendweights_1,
                blendindices_1: blendindices_1,

                normals: normals,

                colors: colors,
                texcoords: texcoords,

                binormals: binormals,
                tangents: tangents
            };

            Vertices.push(Vertex);
        }

        Vertices
    }
}


#[derive(serde::Serialize, serde::Deserialize)]
#[pyclass]
#[derive(Clone)]
pub struct Index {
	
	#[pyo3(get)]
    index: u32
}


impl Index {
	

    pub fn build(IndexBuffer: &Vec<u8>, IndexCount: &u32, IndexSize: &u16) -> Vec<Index> {
        let mut Indices = Vec::new();

        for i in (0..*IndexCount as usize * *IndexSize as usize)
            .step_by(*IndexSize as usize) {

            let index = if *IndexSize == 2 {

                u16::from_le_bytes(
                    IndexBuffer[i..i+2].try_into().unwrap()
                ) as u32

            } else {

                u32::from_le_bytes(
                    IndexBuffer[i..i+4].try_into().unwrap()
                )

            };

            let Index = Self {
                index: index
            };

            Indices.push(Index);
        }

        Indices
    }
}



#[derive(serde::Serialize, serde::Deserialize)]
#[pyclass]
pub struct Mesh {
	#[pyo3(get)]
	pub vertices: Vec<Vertex>,
	
	#[pyo3(get)]
	pub indices: Vec<Index>,
	
	#[pyo3(get)]
	pub name: String
	
}
		
		
#[derive(serde::Serialize)]
pub struct ExtractionProgress {
    pub filecount: usize,
    pub finished_count: usize,
}