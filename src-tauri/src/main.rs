#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::fs::{File, create_dir_all};
use std::io::{Cursor, Read, Seek, SeekFrom, Write};

use std::collections::HashMap;

use call_to_exist_gui_lib::structs::{Packet, UTFPacket, Column, TreeNode, DataTableRow, CPKResult, Crilayla, PAC, PACFileHeader, Texture, BMLSection, BMLMeshSection, Vertex, Index, Mesh, ExtractionProgress};

use call_to_exist_gui_lib::enums::{DataStorage, DataType, RowData, NodeType};

use call_to_exist_gui_lib::{read_cstring, read_bytes, read_u32, ParseMeshes};

use std::path::Path;

use bcdec_rs;

use pyo3::prelude::*;

use tauri::Emitter;

use std::sync::atomic::{AtomicBool, Ordering};

static CANCEL_EXTRACTION: AtomicBool = AtomicBool::new(false);


fn main() {
	
    tauri::Builder::default()
    .plugin(tauri_plugin_dialog::init())
	.plugin(tauri_plugin_fs::init())
    .invoke_handler(tauri::generate_handler![
        open_cpk_command,
		extract_cpk_command,
		
		open_pac_command,
		extract_pac_command,
		
		view_texture_command,
		batch_convert_texture_command,
		
		view_model_command,
		stop_extraction,
		
    ])
	.run(tauri::generate_context!())
	.expect("error while running tauri application");
	
		
}


// Node Counter
fn count_files(nodes: &Vec<TreeNode>) -> usize {
    let mut count = 0;

    for node in nodes {
        match node.node_type {
            NodeType::File { .. } => {
                count += 1;
            }

            NodeType::Folder => {
                count += count_files(&node.children);
            }
        }
    }

    count
}

// Stop Extraction Command
#[tauri::command]
fn stop_extraction() {
    CANCEL_EXTRACTION.store(true, Ordering::Relaxed);
}

#[tauri::command]
async fn extract_cpk_command(
    app: tauri::AppHandle,
    path: String,
    nodes: Vec<TreeNode>,
    checked: bool,
) -> Result<(), String> {
    tokio::task::spawn_blocking(move || {
		
		CANCEL_EXTRACTION.store(false, Ordering::Relaxed);
		
        let path = Path::new(&path);
        let filecount = count_files(&nodes);
        let mut finished_count = 0;

        // Tell Vue the total first
        app.emit("extraction-progress", (0, filecount))
            .map_err(|e| e.to_string())?;

        process_selected_nodes_cpk(
            &nodes,
            path,
            checked,
            &mut finished_count,
			filecount,
            &app,
        )?;

        Ok(())
    })
    .await
    .map_err(|e| e.to_string())?
}

#[tauri::command]
async fn extract_pac_command(
    app: tauri::AppHandle,
    path: String,
    nodes: Vec<TreeNode>,
) -> Result<ExtractionProgress, String> {
    tokio::task::spawn_blocking(move || {
        // println!("Output path: {}", path);
        // println!("Nodes: {}", nodes.len());
		
		CANCEL_EXTRACTION.store(false, Ordering::Relaxed);
		
        let path = Path::new(&path);
        let filecount = count_files(&nodes);
        let mut finished_count: usize = 0;

        // Initial progress
        app.emit(
            "pac-extraction-progress",
            (finished_count, filecount),
        )
        .map_err(|e| e.to_string())?;

        process_selected_nodes_pac(
            &nodes,
            path,
            &mut finished_count,
            filecount,
            &app,
        )?;

        Ok(ExtractionProgress {
            filecount,
            finished_count,
        })
    })
    .await
    .map_err(|e| e.to_string())?
}

fn process_selected_nodes_cpk(nodes: &Vec<TreeNode>, current_dir: &Path, checked: bool, finished_count: &mut usize, filecount: usize, app: &tauri::AppHandle) -> Result<(), String> {
    for node in nodes {
		
		if CANCEL_EXTRACTION.load(Ordering::Relaxed) {
			break;
		}

        // Build path: current_dir + this node's label
        let node_path = current_dir.join(&node.label);

        if let NodeType::Folder = node.node_type {
            // 1. Create directory on disk
            create_dir_all(&node_path).map_err(|e| format!("Failed to create directory {:?}: {}", node_path, e))?;
                

            // 2. Pass 'node_path' (which contains all parent labels) down to children
            if !node.children.is_empty() {
                process_selected_nodes_cpk(&node.children, &node_path, checked, finished_count, filecount, app)?;
            }
        } else {
            // 3. File Node: Extract using current_dir as target location
            let (mut file, toc_offset, packet_rows_data) = OpenCPK(node.path.clone());
            
            // Pass target directory (current_dir) to your Extract function
            ExtractCPK(file, toc_offset, &node_path, node, checked);
			
			*finished_count += 1;
			
			app.emit(
				"extraction-progress",
				(*finished_count, filecount),
			)
			.map_err(|e| e.to_string())?;
			
        }
    }

    Ok(())
}


fn process_selected_nodes_pac(
    nodes: &Vec<TreeNode>,
    current_dir: &Path,
	finished_count: &mut usize,
	filecount: usize,
	app: &tauri::AppHandle
) -> Result<(), String> {

    for node in nodes {
		if CANCEL_EXTRACTION.load(Ordering::Relaxed) {
        break;
    }

        let archive_name = Path::new(&node.path)
            .file_stem()
            .unwrap()
            .to_string_lossy()
            .to_string();

        let archive_dir = current_dir.join( format!("{}_{}", &archive_name, "unpacked") );

        create_dir_all(&archive_dir)
            .map_err(|e| format!("Failed to create {:?}: {}", archive_dir, e))?;
		
		// Add filename (label) and contained_type
        let output_path = archive_dir.join(format!("{}_{}", node.label, node.contained_type));

        println!("PAC: {:?}", node.path);
        println!("OUTPUT: {:?}", output_path);
        println!("OFFSET: {}", node.get_file_offset());
        println!("SIZE: {}", node.get_file_size());

        let (mut file, _, _) = OpenPAC(node.path.clone());

        ExtractPAC(
            &mut file,
            &output_path,
            node
        );
		
		*finished_count += 1;
		
		app.emit(
			"pac-extraction-progress",
			(*finished_count, filecount),
		);

    }

    Ok(())
}

#[tauri::command]
async fn open_cpk_command(mut rows: Vec<DataTableRow>, mut nodes: Vec<TreeNode>, paths: Vec<String>) -> CPKResult {
	
	tokio::task::spawn_blocking(move || {
		
		let mut next_key = get_next_key_cpk(&nodes);
		
		for path in paths {
			let (mut file, toc_offset, packet_rows_data) = OpenCPK(path.clone());
			
			
			// Build Tree
			build_tree_cpk(&mut nodes, &mut next_key, &packet_rows_data, path.clone());
			
			
			// Build DataTable data
			build_table_cpk(&mut rows, &packet_rows_data, path.clone());
		}
		
		CPKResult {
			tree: nodes,
			rows: rows
		}
		
	})
	.await
	.unwrap()
}


#[tauri::command]
async fn open_pac_command(mut rows: Vec<DataTableRow>, mut nodes: Vec<TreeNode>, paths: Vec<String>) -> CPKResult {
	
	tokio::task::spawn_blocking(move || {
		
		for path in paths {
			let (mut file, pac_instance, pac_fh_instances) = OpenPAC(path.clone());
			
			
			
			build_tree_pac(&mut file, &mut nodes, &pac_instance, &pac_fh_instances, path.clone());
			
			build_table_pac(&mut file, &mut rows, &pac_instance, &pac_fh_instances, path.clone());
			
		}
		
		// println!("Total nodes: {}", nodes.len());
		
		CPKResult {
			tree: nodes,
			rows: rows
		}
		
	})
	.await
	.unwrap()
}


#[tauri::command]
async fn view_texture_command(path: String) -> Result<Vec<u8>, String> {
	
	tokio::task::spawn_blocking(move || {
		
		let mut texture_file = File::open(&path).map_err(|e| e.to_string())?;
		
		let texture_instance = Texture::build(&mut texture_file);
		
		let width = texture_instance.width as usize;
		let height = texture_instance.height as usize;

		// BC7 = 16 bytes per 4x4 block
		let blocks_x = (width + 3) / 4;
		let blocks_y = (height + 3) / 4;

		let mut rgba = vec![0u8; width * height * 4];

		for by in 0..blocks_y {
			for bx in 0..blocks_x {
				let offset = (by * blocks_x + bx) * 16;

				let compressed = &texture_instance.pixel_data[offset..offset + 16];

				let mut block = [0u8; 64];

				bcdec_rs::bc7(compressed, &mut block, 16);

				for y in 0..4 {
					let dst = ((by * 4 + y) * width + bx * 4) * 4;
					let src = y * 16;

					let pixels_to_copy = ((width - bx * 4).min(4)) * 4;

					if by * 4 + y < height {
						rgba[dst..dst + pixels_to_copy]
							.copy_from_slice(&block[src..src + pixels_to_copy]);
					}
				}
			}
		}
		
		let image = image::RgbaImage::from_raw(
			width as u32,
			height as u32,
			rgba,
		)
		.unwrap();
		
		let mut png_data = Cursor::new(Vec::new());

		image.write_to(
			&mut png_data,
			image::ImageFormat::Png,
		).unwrap();

		Ok(png_data.into_inner())
		
	})
	.await
	.map_err(|e| e.to_string())?
}


#[tauri::command]
async fn batch_convert_texture_command(paths: Vec<String>) -> Result<(), String> {
	
	tokio::task::spawn_blocking(move || {
		
		for path in paths {
			let mut texture_file = File::open(&path).map_err(|e| e.to_string())?;
			
			let texture_instance = Texture::build(&mut texture_file);
			
			let width = texture_instance.width as usize;
			let height = texture_instance.height as usize;

			// BC7 = 16 bytes per 4x4 block
			let blocks_x = (width + 3) / 4;
			let blocks_y = (height + 3) / 4;

			let mut rgba = vec![0u8; width * height * 4];

			for by in 0..blocks_y {
				for bx in 0..blocks_x {
					let offset = (by * blocks_x + bx) * 16;

					let compressed = &texture_instance.pixel_data[offset..offset + 16];

					let mut block = [0u8; 64];

					bcdec_rs::bc7(compressed, &mut block, 16);

					for y in 0..4 {
						let dst = ((by * 4 + y) * width + bx * 4) * 4;
						let src = y * 16;

						let pixels_to_copy = ((width - bx * 4).min(4)) * 4;

						if by * 4 + y < height {
							rgba[dst..dst + pixels_to_copy]
								.copy_from_slice(&block[src..src + pixels_to_copy]);
						}
					}
				}
			}
			
			let image = image::RgbaImage::from_raw(
				width as u32,
				height as u32,
				rgba,
			)
			.ok_or("Failed to create image")?;

			// Get filename without extension
			let input_path = Path::new(&path);

			let filename = input_path
				.file_stem()
				.ok_or("Invalid filename")?
				.to_string_lossy();

			// Put PNG next to original file
			let output_path = input_path
				.with_file_name(format!("{}.png", filename));

			image.save(&output_path)
				.map_err(|e| e.to_string())?;
		}
		
		Ok(())
		
	})
	.await
	.map_err(|e| e.to_string())?
}


fn get_next_key_cpk(tree_nodes: &Vec<TreeNode>) -> usize {
    let mut max_key = 0;

    for node in tree_nodes {
        let key: usize = node.key.parse().unwrap();

        if key >= max_key {
            max_key = key + 1;
        }

        let child_key = get_next_key_cpk(&node.children);

        if child_key > max_key {
            max_key = child_key;
        }
    }

    max_key
}

fn build_tree_cpk(tree_nodes: &mut Vec<TreeNode>, next_key: &mut usize, packet_rows_data: &Vec<HashMap<String, Option<RowData>>>, cpk_path: String) {
	
	for packet_row_data in packet_rows_data {
		let file_offset = match packet_row_data.get("FileOffset") { // UInt64, borrowed
		Some( Some(RowData::UInt64(value)) ) => value,
		_ => panic!("FileOffset is not UInt64"),
		};
		
		let file_size = match packet_row_data.get("FileSize") { // UInt32, borrowed
		Some( Some(RowData::UInt32(value)) ) => value,
		_ => panic!("FileSize is not UInt32"),
		};
		
		let extract_size = match packet_row_data.get("ExtractSize") { // UInt32, borrowed
		Some( Some(RowData::UInt32(value)) ) => value,
		_ => panic!("ExtractSize is not UInt32"),
		};
		
		let dir_name = match packet_row_data.get("DirName") { // String, borrowed
		Some( Some(RowData::String(value)) ) => value,
		_ => panic!("DirName type is not String"),
		};
		
		let file_name = match packet_row_data.get("FileName") { // String, borrowed
		Some( Some(RowData::String(value)) ) => value,
		_ => panic!("FileName type is not String"),
		};
		
		let parts: Vec<&str> = dir_name.split('/').collect();
		
		let found = tree_nodes.iter().position(|node| node.label == parts[0]); // Does first node already exist in tree_nodes ?

		let index = match found {
			Some(index) => index, // return index directly if found

			None => {
				tree_nodes.push(TreeNode { // Add if doesn't exist
					key: next_key.to_string(),
					label: parts[0].to_string(),
					node_type: NodeType::Folder,
					children: vec![],
					path: cpk_path.clone(),
					contained_type: String::from("")
				});
				
				*next_key += 1;
				tree_nodes.len() - 1 // if now it has length 1 (one element), return 0 to access first index
			}
		};
		
		let mut current = &mut tree_nodes[index]; // Access tree_nodes using index and get first node
		
		for part in parts.iter().skip(1) {
			let found = current.children.iter().position(|node| node.label == *part); // Does rest of the nodes already exist in tree_nodes ?
					
			let index = match found {
					Some(index) => index, // if so, return index for each node of those in its own iteration

					None => {
						current.children.push(TreeNode { // if no, add each node in its own iteration
							key: next_key.to_string(),
							label: part.to_string(),
							node_type: NodeType::Folder,
							children: vec![],
							path: cpk_path.clone(),
							contained_type: String::from("")
							
						});
						
						*next_key += 1;
						current.children.len() - 1 // if current now has children of length 1 (one child), return 0
					}
				};

				current = &mut current.children[index]; // Access current_node's children using index and get the next child
		}
		
		
		let exists = current.children.iter().any(|node| node.label == *file_name); // Does this file node already exist in tree_nodes ?
		
		if !exists { // If it doesn't exist
			let file_node = TreeNode { // e.g.: sh_custom.pac
			key: next_key.to_string(),
			label: file_name.clone(),
			node_type: NodeType::File {
						file_offset: *file_offset,
						file_size: *file_size,
						extract_size: *extract_size,
					},
			children: vec![],
			path: cpk_path.clone(),
			contained_type: String::from("")
			};
			
			*next_key += 1;
		
			current.children.push(file_node);
		}
		
	}
	
	
}


fn build_table_cpk(rows: &mut Vec<DataTableRow>,
    packet_rows_data: &Vec<HashMap<String, Option<RowData>>>,
    cpk_path: String
) {

    for packet_row_data in packet_rows_data {

        let file_offset = match packet_row_data.get("FileOffset") {
            Some(Some(RowData::UInt64(value))) => *value,
            _ => panic!("FileOffset is not UInt64"),
        };

        let file_size = match packet_row_data.get("FileSize") {
            Some(Some(RowData::UInt32(value))) => *value,
            _ => panic!("FileSize is not UInt32"),
        };

        let extract_size = match packet_row_data.get("ExtractSize") {
            Some(Some(RowData::UInt32(value))) => *value,
            _ => panic!("ExtractSize is not UInt32"),
        };

        let dir_name = match packet_row_data.get("DirName") {
            Some(Some(RowData::String(value))) => value.clone(),
            _ => panic!("DirName type is not String"),
        };

        let file_name = match packet_row_data.get("FileName") {
            Some(Some(RowData::String(value))) => value.clone(),
            _ => panic!("FileName type is not String"),
        };

        let row = DataTableRow {
            dir_name,
            file_name,
            file_size,
            file_offset: file_offset as u32,
            extract_size,
            path: cpk_path.clone(),
    

		};
		
		
		 // Don't add duplicate
        if !rows.iter().any(|x| {
			x.dir_name == row.dir_name &&
			x.file_name == row.file_name &&
			x.file_offset == row.file_offset &&
			x.file_size == row.file_size &&
			x.extract_size == row.extract_size &&
			x.path == row.path
		}) {
			rows.push(row);
		}
		
		
  
    }	
	
	
}





fn OpenCPK(path: String) -> (File, u64, Vec<HashMap<String, Option<RowData>>>) { 
	let mut file = File::open(path).unwrap();
	
	
	let file_size = file.metadata().unwrap().len();
	
	let mut packets: Vec<&Packet> = vec![];
	

	let mut packet = Packet::build(&mut file); // Make a new Packet
	packet.decrypt(); // Decrypt if encrypted
	
	
	// println!("Magic is {}, PacketMode is {}, PacketSize is {}", packet.magic, packet.packet_mode, packet.packet_size);

	packets.push(&packet);

	
	let mut in_memory_utf = Cursor::new(&packet.packet_data);
	
	let mut cpk_packet = UTFPacket::build(&mut in_memory_utf); // Make a new UTFPacket
	
	cpk_packet.read_column_definition(&mut in_memory_utf); // Read each column's definition
	
	cpk_packet.read_rows(&mut in_memory_utf); // Read Rows
	
	cpk_packet.parse_decrypted_packet(&mut in_memory_utf); // Parse Decrypted Packet
	
	let cpk_dict = match cpk_packet.UTFRowsData.get(0) {
		Some(value) => value,
		_ => panic!("No rows in cpk packet."),
	};
	

	let toc_offset = match cpk_dict.get("TocOffset") { // UInt64
		Some(Some(RowData::UInt64(value))) => *value,
		_ => panic!("TocOffset is not UInt64"),
	};

	let toc_size = match cpk_dict.get("TocSize") { // UInt64
		Some(Some(RowData::UInt64(value))) => *value,
		_ => panic!("TocSize is not UInt64"),
	};

	file.seek(SeekFrom::Start(toc_offset)).unwrap();

	packet = Packet::build(&mut file);
	packet.decrypt();

	in_memory_utf = Cursor::new(&packet.packet_data);

	let mut toc_packet = UTFPacket::build(&mut in_memory_utf);

	toc_packet.read_column_definition(&mut in_memory_utf);
	toc_packet.read_rows(&mut in_memory_utf);
	toc_packet.parse_decrypted_packet(&mut in_memory_utf);
	
	// println!("{:#?}", toc_packet.UTFRowsData);
	
	(file, toc_offset, toc_packet.UTFRowsData)

}



fn ExtractCPK(mut file: File, toc_offset: u64, output_path: &Path, file_node: &TreeNode, pac_decompress_check: bool) {
		
		let file_offset = file_node.get_file_offset();
		let file_size = file_node.get_file_size();
		let extract_size = file_node.get_extract_size();
			
		
		
		
		// Seek to file_offset starting from toc_offset
		file.seek( SeekFrom::Start (toc_offset + file_offset) ).unwrap();
		
		let mut file_data = read_bytes(&mut file, file_size as u64);
		
		
		if pac_decompress_check && file_size != extract_size {
				let mut cursor_instance = Cursor::new(file_data.clone());
				let mut crilayla_instance = Crilayla::build(&mut cursor_instance);
				
				file_data = crilayla_instance.decompress_crilayla();
			}
		
		
		let mut written_file = File::create(output_path).unwrap();
		
		written_file.write_all(&file_data).unwrap(); // Write
		
		
	
}


fn OpenPAC(path: String) -> (File, PAC, Vec<PACFileHeader>) { 
	let mut file = File::open(path).unwrap();
	
	
	let pac_instance = PAC::build(&mut file);
	
	let mut pac_fh_instances: Vec<PACFileHeader> = vec![];
	for i in 0..pac_instance.file_num {
		
		pac_fh_instances.push( PACFileHeader::build(&mut file, pac_instance.has_extra_header) );
		
		
		
	}
	
	
	(file, pac_instance, pac_fh_instances)
	
	

}


fn build_tree_pac(file: &mut File, tree_nodes: &mut Vec<TreeNode>, pac_instance: &PAC, pac_fh_instances: &Vec<PACFileHeader>, pac_path: String) {
	
	for (offset, pac_fh_instance) in pac_instance.offsets.iter().zip(pac_fh_instances.iter()) {
		
		let contained_type = pac_fh_instance.get_contained_type();
		let contained_id = pac_fh_instance.get_contained_id();
		
		let file_id = pac_fh_instance.get_file_id();
		let filename = pac_fh_instance.get_filename();
		
		
		
		
			
		file.seek( SeekFrom::Start(*offset as u64) ).unwrap();
		
		let FileSize = read_u32(file, "Little");
		let FileOffsetRelative = read_u32(file, "Little");
		
		let AbsoluteFileOffset = FileOffsetRelative + offset;
		
		/*file.seek( SeekFrom:Start(AbsoluteFileOffset as u64) ).unwrap();
		
		let file_data = read_bytes(file, FileSize as u64);
		
		let mut cursor_instance = Cursor::new(&file_data);
		
		let pac_instance = PAC::build(&mut cursor_instance);

		let mut pac_fh_instances: Vec<PACFileHeader> = vec![];
		for i in 0..pac_instance.file_num {
			pac_fh_instances.push(PACFileHeader::build(&mut cursor_instance));
			
		}*/

			
		let new_node = TreeNode {
			key: format!("{}:{}", pac_path, file_id),
			label: filename,
			node_type: NodeType::File {
				file_offset: AbsoluteFileOffset as u64,
				file_size: FileSize,
				extract_size: 0,
			},
			children: vec![],
			path: pac_path.clone(),
			contained_type: contained_type
		};

		if !tree_nodes.iter().any(|node| node.key == new_node.key) {
			tree_nodes.push(new_node);
		}
		
	
		}
		

		
		
	}
	
	
fn build_table_pac(file: &mut File, rows: &mut Vec<DataTableRow>, pac_instance: &PAC, pac_fh_instances: &Vec<PACFileHeader>, pac_path: String) {

   for (offset, pac_fh_instance) in pac_instance.offsets.iter().zip(pac_fh_instances.iter()) {
		
		let contained_type = pac_fh_instance.get_contained_type();
		let contained_id = pac_fh_instance.get_contained_id();
		
		let file_id = pac_fh_instance.get_file_id();
		let filename = pac_fh_instance.get_filename();
		
		
		
		
			
		file.seek( SeekFrom::Start(*offset as u64) ).unwrap();
		
		let FileSize = read_u32(file, "Little");
		let FileOffsetRelative = read_u32(file, "Little");
		
		let AbsoluteFileOffset = FileOffsetRelative + offset;


			
		 let row = DataTableRow {
            dir_name: "".to_string(),
            file_name: filename,
            file_size: FileSize,
            file_offset: AbsoluteFileOffset,
            extract_size: 0,
            path: pac_path.clone(),
    

		};
		
		
		 // Don't add duplicate
        if !rows.iter().any(|x| {
			x.file_name == row.file_name &&
			x.file_offset == row.file_offset &&
			x.file_size == row.file_size &&
			x.path == row.path
		}) {
			rows.push(row);
		}

		
		
	}

}

fn ExtractPAC(file: &mut File, output_path: &Path, file_node: &TreeNode) {
		
	let file_offset = file_node.get_file_offset();
	let file_size = file_node.get_file_size();
	
	// Seek to file_offset starting from toc_offset
	file.seek( SeekFrom::Start (file_offset) ).unwrap();

	let file_data = read_bytes(file, file_size as u64);
	
	let mut written_file = File::create(output_path).unwrap();
	
	written_file.write_all(&file_data).unwrap(); // Write
		

}










		
		
	
#[tauri::command]	
fn view_model_command(path: String) -> Vec<Vec<Mesh>> {
	let meshes = ParseMeshes(path.clone());
	
	meshes
	
}


