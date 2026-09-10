
pub enum DataStorage {

	STORAGE_MASK = 0xF0,
    
    STORAGE_NONE = 0x00,
    STORAGE_ZERO = 0x10,
    STORAGE_CONSTANT = 0x30,
    STORAGE_PERROW = 0x50,

}


pub enum DataType {

	DATATYPE_MASK = 0x0F,
    
    UINT8 = 0,
    UINT8_1 = 1,
    UINT16 = 2,
    UINT16_1 = 3,
    UINT32 = 4,
    UINT32_1 = 5,
    UINT64 = 6,
    UINT64_1 = 7,
    FLOAT = 8,
    STRING = 0xA,
    BYTEARRAY = 0xB,

}

#[derive(Clone)]
#[derive(Debug)]
pub enum RowData {
    UInt8(u8),
    UInt16(u16),
    UInt32(u32),
    UInt64(u64),
    Float(f32),
    String(String),
    ByteArray(Vec<u8>),
}

#[derive(serde::Serialize, serde::Deserialize)]
pub enum NodeType {
    Folder,
    File {
        file_offset: u64,
        file_size: u32,
        extract_size: u32,
    },
}

