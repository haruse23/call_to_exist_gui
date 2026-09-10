use std::fs::File;

pub mod structs;
pub mod enums;

use std::io::{Cursor, Read, Seek, SeekFrom, Write};

use std::collections::HashMap;

use crate::structs::{Packet, UTFPacket, Column, TreeNode, DataTableRow, CPKResult, Crilayla, PAC, PACFileHeader, Texture, BMLSection, BMLMeshSection, Vertex, Index, Mesh};

use pyo3::prelude::*;

pub fn read_u8<R: Read>(file: &mut R, Endianness: &str) -> u8 {
    let mut buffer = [0u8; 1];
    file.read_exact(&mut buffer).unwrap();
	
	if Endianness == "Big" {
		u8::from_be_bytes(buffer)
	}
	else {
		u8::from_le_bytes(buffer)
	}
   
}

pub fn read_u16<R: Read>(file: &mut R, Endianness: &str) -> u16 {
    let mut buffer = [0u8; 2];
    file.read_exact(&mut buffer).unwrap();
	
	if Endianness == "Big" {
		u16::from_be_bytes(buffer)
	}
	else {
		u16::from_le_bytes(buffer)
	}
   
}


pub fn read_u32<R: Read>(file: &mut R, Endianness: &str) -> u32 {
    let mut buffer = [0u8; 4];
    file.read_exact(&mut buffer).unwrap();
	
	if Endianness == "Big" {
		u32::from_be_bytes(buffer)
	}
	else {
		u32::from_le_bytes(buffer)
	}
   
}

pub fn read_u64<R: Read>(file: &mut R, Endianness: &str) -> u64 {
    let mut buffer = [0u8; 8];
    file.read_exact(&mut buffer).unwrap();
    
	if Endianness == "Big" {
		u64::from_be_bytes(buffer)
	}
	else {
		u64::from_le_bytes(buffer)
	}
	
}

pub fn read_f32<R: Read>(file: &mut R, Endianness: &str) -> f32 {
    let mut buffer = [0u8; 4];
    file.read_exact(&mut buffer).unwrap();
    
	if Endianness == "Big" {
		f32::from_be_bytes(buffer)
	}
	else {
		f32::from_le_bytes(buffer)
	}
	
}



pub fn read_bytes<R: Read>(file: &mut R, size: u64) -> Vec<u8> {
	let mut buffer = vec![0u8; size as usize];
	
	// println!("Trying to read {} bytes", size);
	 
	file.read_exact(&mut buffer).unwrap();
	
	buffer
}



pub fn read_cstring<R: Read>(file: &mut R) -> String {
    let mut bytes = Vec::new();
    let mut byte = [0u8; 1];

    loop {
        file.read_exact(&mut byte).unwrap();

        if byte[0] == 0 {
            break;
        }

        bytes.push(byte[0]);
    }

    String::from_utf8(bytes).unwrap()
}


fn align<R: Seek>(file: &mut R, alignment: u64) -> u64 {
    let offset = file.stream_position().unwrap();

    let aligned_offset = (offset + alignment - 1) & !(alignment - 1);

    let padding = aligned_offset - offset;

    file.seek(SeekFrom::Current(padding as i64)).unwrap();

    aligned_offset
}


fn calculate_hash(string: String) -> u32 {
	let mut hash: u32 = 0;
	
	for b in string.bytes() {
		hash = hash * 33 + b as u32;
	}
	
	hash += hash >> 5;
	
	hash
	
}

// Mesh
fn ProcessMesh(cursor_instance: &mut Cursor<&Vec<u8>>, BMLFile: &mut File) -> (BMLMeshSection, Mesh) {
	let mut mesh = BMLMeshSection::build(cursor_instance);
	
	
	let mut mesh_name = String::from("");
	
	if mesh.MeshNameOffset != 0 {
		BMLFile.seek( SeekFrom::Start( mesh.MeshNameOffset ) ).unwrap();
		
		mesh_name = read_cstring(BMLFile);
	}
	
	
	BMLFile.seek( SeekFrom::Start( mesh.MeshVertexBufferOffset ) ).unwrap();
	
	let mesh_vertex_buffer = read_bytes(BMLFile, (mesh.VertexCount * mesh.VertexStride as u32) as u64);
	
	
	BMLFile.seek( SeekFrom::Start( mesh.MeshIndexBufferOffset ) ).unwrap();
	
	
	let mesh_index_buffer = read_bytes(BMLFile, (mesh.IndexCount * mesh.IndexSize as u32) as u64);
	
	
	
	let Vertices: Vec<Vertex> = Vertex::build(&mesh_vertex_buffer, &mesh.VertexCount, &mesh.VertexStride);
	
	
	let Indices: Vec<Index> = Index::build(&mesh_index_buffer, &mesh.IndexCount, &mesh.IndexSize);
	
	
	
	let mesh_instance = Mesh {
		vertices: Vertices,
		indices: Indices,
		
		name: mesh_name
	};
	
	
	
	
	
	(mesh, mesh_instance)
		
		
}


pub fn ParseMeshes(path: String) -> Vec<Vec<Mesh>> {
	let mut BMLFile = File::open(&path).unwrap();
	
	let mut MeshSections: Vec<BMLSection> = Vec::new();
	
	let mut LODSections: HashMap<u64, BMLSection> = HashMap::new();
	
	while BMLFile.stream_position().unwrap() < BMLFile.metadata().unwrap().len() {
		
		let section = BMLSection::build(&mut BMLFile);
		
		if section.name == "MESH" {
			MeshSections.push(section.clone());
		}
		
		if section.name == "LODM" {
			LODSections.insert(section.section_data_offset, section);
			
		}
		
		
		
	}
	
	// println!("Mesh Sections: {:#?}", MeshSections);
	
	
	let mut Meshes: Vec<Vec<Mesh>> = Vec::new(); // Each Mesh and its LODs
	
	
	
	for mesh_section in MeshSections {
		let mut LODs: Vec<Mesh> = Vec::new();
		
		let mut cursor_instance: Cursor<&Vec<u8>> = Cursor::new(&mesh_section.section_data);
			
		let (mesh, mesh_instance) = ProcessMesh(&mut cursor_instance, &mut BMLFile);
		
		LODs.push(mesh_instance);
		
		for (offset, lod_section) in &LODSections {
			
			if *offset == mesh.MeshLOD1Offset || *offset == mesh.MeshLOD2Offset || *offset == mesh.MeshLOD3Offset || *offset == mesh.MeshLOD4Offset
				{
			
				let mut cursor_instance_lod: Cursor<&Vec<u8>> = Cursor::new(&lod_section.section_data);
				
				let (lod, lod_instance) = ProcessMesh(&mut cursor_instance_lod, &mut BMLFile);
				
				LODs.push(lod_instance);
				
			}
			
		}
		
		
		
		
		
		
		
		
		Meshes.push(LODs);
		
		
	}
	
	Meshes
		
		
		
	
	
	
	
	
	
}



#[pyfunction]
fn parse_meshes_pyo3(path: String) -> Vec<Vec<Mesh>> {
    let meshes = ParseMeshes(path.clone());
	
	meshes
}


#[pymodule]
fn call_to_exist_gui_lib(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<Mesh>()?;
    m.add_class::<Vertex>()?;
    m.add_class::<Index>()?;
    m.add_function(wrap_pyfunction!(parse_meshes_pyo3, m)?)?;

    Ok(())
}