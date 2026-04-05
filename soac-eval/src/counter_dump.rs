#[cfg(not(target_endian = "little"))]
compile_error!("counter dump format currently requires little-endian hosts");

use std::collections::HashMap;
use std::mem::size_of;
use soac_blockpy::block_py::FunctionId;

pub const COUNTER_DUMP_MAGIC: [u8; 8] = *b"SOACCNTR";
pub const COUNTER_DUMP_VERSION: u16 = 2;
pub const COUNTER_DUMP_NONE_U32: u32 = u32::MAX;
pub const COUNTER_DUMP_NONE_U64: u64 = u64::MAX;

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct CounterDumpRecordHeader {
    pub magic: [u8; 8],
    pub version: u16,
    pub header_size: u16,
    pub record_len: u32,
    pub row_count: u32,
    pub string_count: u32,
    pub string_bytes_len: u32,
    pub module_name_string_id: u32,
    pub package_name_string_id: u32,
    pub string_offsets_offset: u32,
    pub string_bytes_offset: u32,
    pub counter_id_offset: u32,
    pub scope_offset: u32,
    pub kind_offset: u32,
    pub site_kind_offset: u32,
    pub function_id_offset: u32,
    pub current_function_id_offset: u32,
    pub function_qualname_offset: u32,
    pub block_label_offset: u32,
    pub value_offset: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CounterDumpRow {
    pub counter_id: u32,
    pub scope: String,
    pub kind: String,
    pub site_kind: String,
    pub function_id: Option<FunctionId>,
    pub current_function_id: Option<FunctionId>,
    pub function_qualname: Option<String>,
    pub block_label: Option<String>,
    pub value: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CounterDumpRecord {
    pub module_name: String,
    pub package_name: Option<String>,
    pub rows: Vec<CounterDumpRow>,
}

#[derive(Default)]
struct StringTable {
    ids: HashMap<String, u32>,
    strings: Vec<String>,
}

impl StringTable {
    fn intern(&mut self, value: &str) -> Result<u32, String> {
        if let Some(id) = self.ids.get(value).copied() {
            return Ok(id);
        }
        let id = u32::try_from(self.strings.len())
            .map_err(|_| "counter dump string table exceeds u32 capacity".to_string())?;
        let owned = value.to_string();
        self.ids.insert(owned.clone(), id);
        self.strings.push(owned);
        Ok(id)
    }
}

impl CounterDumpRecord {
    pub fn encode(&self) -> Result<Vec<u8>, String> {
        let row_count = u32::try_from(self.rows.len())
            .map_err(|_| "counter dump row count exceeds u32 capacity".to_string())?;
        let mut strings = StringTable::default();
        let module_name_string_id = strings.intern(self.module_name.as_str())?;
        let package_name_string_id = match self.package_name.as_deref() {
            Some(package_name) if !package_name.is_empty() => strings.intern(package_name)?,
            _ => COUNTER_DUMP_NONE_U32,
        };

        let mut counter_id = Vec::with_capacity(self.rows.len());
        let mut scope = Vec::with_capacity(self.rows.len());
        let mut kind = Vec::with_capacity(self.rows.len());
        let mut site_kind = Vec::with_capacity(self.rows.len());
        let mut function_id = Vec::with_capacity(self.rows.len());
        let mut current_function_id = Vec::with_capacity(self.rows.len());
        let mut function_qualname = Vec::with_capacity(self.rows.len());
        let mut block_label = Vec::with_capacity(self.rows.len());
        let mut value = Vec::with_capacity(self.rows.len());

        for row in &self.rows {
            counter_id.push(row.counter_id);
            scope.push(strings.intern(row.scope.as_str())?);
            kind.push(strings.intern(row.kind.as_str())?);
            site_kind.push(strings.intern(row.site_kind.as_str())?);
            function_id.push(
                row.function_id
                    .map(FunctionId::packed)
                    .unwrap_or(COUNTER_DUMP_NONE_U64),
            );
            current_function_id.push(
                row.current_function_id
                    .map(FunctionId::packed)
                    .unwrap_or(COUNTER_DUMP_NONE_U64),
            );
            function_qualname.push(match row.function_qualname.as_deref() {
                Some(qualname) => strings.intern(qualname)?,
                None => COUNTER_DUMP_NONE_U32,
            });
            block_label.push(match row.block_label.as_deref() {
                Some(block) => strings.intern(block)?,
                None => COUNTER_DUMP_NONE_U32,
            });
            value.push(row.value);
        }

        let string_count = u32::try_from(strings.strings.len())
            .map_err(|_| "counter dump string count exceeds u32 capacity".to_string())?;
        let mut string_offsets = Vec::with_capacity(strings.strings.len() + 1);
        let mut string_bytes = Vec::new();
        string_offsets.push(0u32);
        for string in &strings.strings {
            string_bytes.extend_from_slice(string.as_bytes());
            string_offsets.push(
                u32::try_from(string_bytes.len())
                    .map_err(|_| "counter dump string bytes exceed u32 capacity".to_string())?,
            );
        }

        let header_size = size_of::<CounterDumpRecordHeader>();
        let string_offsets_offset = align_up(header_size, 4);
        let string_bytes_offset = string_offsets_offset + string_offsets.len() * size_of::<u32>();
        let counter_id_offset = align_up(string_bytes_offset + string_bytes.len(), 4);
        let scope_offset = counter_id_offset + counter_id.len() * size_of::<u32>();
        let kind_offset = scope_offset + scope.len() * size_of::<u32>();
        let site_kind_offset = kind_offset + kind.len() * size_of::<u32>();
        let function_id_offset = align_up(site_kind_offset + site_kind.len() * size_of::<u32>(), 8);
        let current_function_id_offset =
            function_id_offset + function_id.len() * size_of::<u64>();
        let function_qualname_offset =
            current_function_id_offset + current_function_id.len() * size_of::<u64>();
        let block_label_offset =
            function_qualname_offset + function_qualname.len() * size_of::<u32>();
        let value_offset = align_up(block_label_offset + block_label.len() * size_of::<u32>(), 8);
        let record_len = align_up(value_offset + value.len() * size_of::<u64>(), 8);

        let header = CounterDumpRecordHeader {
            magic: COUNTER_DUMP_MAGIC,
            version: COUNTER_DUMP_VERSION,
            header_size: u16::try_from(header_size)
                .map_err(|_| "counter dump header size exceeds u16 capacity".to_string())?,
            record_len: u32::try_from(record_len)
                .map_err(|_| "counter dump record length exceeds u32 capacity".to_string())?,
            row_count,
            string_count,
            string_bytes_len: u32::try_from(string_bytes.len())
                .map_err(|_| "counter dump string bytes exceed u32 capacity".to_string())?,
            module_name_string_id,
            package_name_string_id,
            string_offsets_offset: u32::try_from(string_offsets_offset)
                .map_err(|_| "counter dump offset exceeds u32 capacity".to_string())?,
            string_bytes_offset: u32::try_from(string_bytes_offset)
                .map_err(|_| "counter dump offset exceeds u32 capacity".to_string())?,
            counter_id_offset: u32::try_from(counter_id_offset)
                .map_err(|_| "counter dump offset exceeds u32 capacity".to_string())?,
            scope_offset: u32::try_from(scope_offset)
                .map_err(|_| "counter dump offset exceeds u32 capacity".to_string())?,
            kind_offset: u32::try_from(kind_offset)
                .map_err(|_| "counter dump offset exceeds u32 capacity".to_string())?,
            site_kind_offset: u32::try_from(site_kind_offset)
                .map_err(|_| "counter dump offset exceeds u32 capacity".to_string())?,
            function_id_offset: u32::try_from(function_id_offset)
                .map_err(|_| "counter dump offset exceeds u32 capacity".to_string())?,
            current_function_id_offset: u32::try_from(current_function_id_offset)
                .map_err(|_| "counter dump offset exceeds u32 capacity".to_string())?,
            function_qualname_offset: u32::try_from(function_qualname_offset)
                .map_err(|_| "counter dump offset exceeds u32 capacity".to_string())?,
            block_label_offset: u32::try_from(block_label_offset)
                .map_err(|_| "counter dump offset exceeds u32 capacity".to_string())?,
            value_offset: u32::try_from(value_offset)
                .map_err(|_| "counter dump offset exceeds u32 capacity".to_string())?,
        };

        let mut bytes = vec![0u8; record_len];
        write_bytes(&mut bytes, 0, bytes_of(&header))?;
        write_bytes(
            &mut bytes,
            string_offsets_offset,
            bytes_of_slice(string_offsets.as_slice()),
        )?;
        write_bytes(&mut bytes, string_bytes_offset, string_bytes.as_slice())?;
        write_bytes(
            &mut bytes,
            counter_id_offset,
            bytes_of_slice(counter_id.as_slice()),
        )?;
        write_bytes(&mut bytes, scope_offset, bytes_of_slice(scope.as_slice()))?;
        write_bytes(&mut bytes, kind_offset, bytes_of_slice(kind.as_slice()))?;
        write_bytes(
            &mut bytes,
            site_kind_offset,
            bytes_of_slice(site_kind.as_slice()),
        )?;
        write_bytes(
            &mut bytes,
            function_id_offset,
            bytes_of_slice(function_id.as_slice()),
        )?;
        write_bytes(
            &mut bytes,
            current_function_id_offset,
            bytes_of_slice(current_function_id.as_slice()),
        )?;
        write_bytes(
            &mut bytes,
            function_qualname_offset,
            bytes_of_slice(function_qualname.as_slice()),
        )?;
        write_bytes(
            &mut bytes,
            block_label_offset,
            bytes_of_slice(block_label.as_slice()),
        )?;
        write_bytes(&mut bytes, value_offset, bytes_of_slice(value.as_slice()))?;
        Ok(bytes)
    }
}

fn align_up(offset: usize, align: usize) -> usize {
    let remainder = offset % align;
    if remainder == 0 {
        offset
    } else {
        offset + (align - remainder)
    }
}

fn write_bytes(dst: &mut [u8], offset: usize, src: &[u8]) -> Result<(), String> {
    let end = offset
        .checked_add(src.len())
        .ok_or_else(|| "counter dump byte range overflowed".to_string())?;
    let Some(target) = dst.get_mut(offset..end) else {
        return Err("counter dump byte range is out of bounds".to_string());
    };
    target.copy_from_slice(src);
    Ok(())
}

fn bytes_of<T>(value: &T) -> &[u8] {
    unsafe { std::slice::from_raw_parts((value as *const T).cast::<u8>(), size_of::<T>()) }
}

fn bytes_of_slice<T>(values: &[T]) -> &[u8] {
    unsafe {
        std::slice::from_raw_parts(values.as_ptr().cast::<u8>(), std::mem::size_of_val(values))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn read_u16(bytes: &[u8], offset: usize) -> u16 {
        let raw = bytes[offset..offset + 2]
            .try_into()
            .expect("u16 slice should have exact width");
        u16::from_le_bytes(raw)
    }

    fn read_u32(bytes: &[u8], offset: usize) -> u32 {
        let raw = bytes[offset..offset + 4]
            .try_into()
            .expect("u32 slice should have exact width");
        u32::from_le_bytes(raw)
    }

    fn read_u64(bytes: &[u8], offset: usize) -> u64 {
        let raw = bytes[offset..offset + 8]
            .try_into()
            .expect("u64 slice should have exact width");
        u64::from_le_bytes(raw)
    }

    #[test]
    fn encodes_columnar_record_layout() {
        let record = CounterDumpRecord {
            module_name: "counter_test".to_string(),
            package_name: Some("pkg".to_string()),
            rows: vec![
                CounterDumpRow {
                    counter_id: 3,
                    scope: "this".to_string(),
                    kind: "block_entry".to_string(),
                    site_kind: "block_entry".to_string(),
                    function_id: Some(FunctionId::new(1, 7)),
                    current_function_id: Some(FunctionId::new(1, 7)),
                    function_qualname: Some("f".to_string()),
                    block_label: Some("bb0".to_string()),
                    value: 11,
                },
                CounterDumpRow {
                    counter_id: 4,
                    scope: "global".to_string(),
                    kind: "runtime_incref".to_string(),
                    site_kind: "runtime".to_string(),
                    function_id: Some(FunctionId::global()),
                    current_function_id: Some(FunctionId::global()),
                    function_qualname: None,
                    block_label: None,
                    value: 19,
                },
            ],
        };

        let bytes = record.encode().expect("counter dump should encode");
        assert_eq!(&bytes[..8], COUNTER_DUMP_MAGIC.as_slice());
        assert_eq!(read_u16(&bytes, 8), COUNTER_DUMP_VERSION);
        let header_size = usize::from(read_u16(&bytes, 10));
        let record_len = read_u32(&bytes, 12) as usize;
        let row_count = read_u32(&bytes, 16);
        let string_count = read_u32(&bytes, 20);
        let string_bytes_len = read_u32(&bytes, 24) as usize;
        let string_offsets_offset = read_u32(&bytes, 36) as usize;
        let string_bytes_offset = read_u32(&bytes, 40) as usize;
        let counter_id_offset = read_u32(&bytes, 44) as usize;
        let scope_offset = read_u32(&bytes, 48) as usize;
        let kind_offset = read_u32(&bytes, 52) as usize;
        let site_kind_offset = read_u32(&bytes, 56) as usize;
        let function_id_offset = read_u32(&bytes, 60) as usize;
        let current_function_id_offset = read_u32(&bytes, 64) as usize;
        let function_qualname_offset = read_u32(&bytes, 68) as usize;
        let block_label_offset = read_u32(&bytes, 72) as usize;
        let value_offset = read_u32(&bytes, 76) as usize;

        assert_eq!(header_size, size_of::<CounterDumpRecordHeader>());
        assert_eq!(record_len, bytes.len());
        assert_eq!(row_count, 2);
        assert!(string_count >= 7);
        assert!(counter_id_offset >= string_bytes_offset);
        assert!(scope_offset > counter_id_offset);
        assert!(kind_offset > scope_offset);
        assert!(site_kind_offset > kind_offset);
        assert!(function_id_offset > site_kind_offset);
        assert!(current_function_id_offset > function_id_offset);
        assert!(function_qualname_offset > current_function_id_offset);
        assert!(block_label_offset > function_qualname_offset);
        assert!(value_offset > block_label_offset);
        assert_eq!(value_offset % 8, 0);

        let string_offsets_len = (string_count as usize + 1) * size_of::<u32>();
        let first_string_start = string_bytes_offset;
        let first_string_end = first_string_start
            + read_u32(&bytes, string_offsets_offset + size_of::<u32>()) as usize;
        let first_string = std::str::from_utf8(&bytes[first_string_start..first_string_end])
            .expect("module name should be utf-8");
        assert_eq!(first_string, "counter_test");

        assert_eq!(read_u32(&bytes, counter_id_offset), 3);
        assert_eq!(read_u32(&bytes, counter_id_offset + 4), 4);
        assert_eq!(
            read_u64(&bytes, function_id_offset),
            FunctionId::new(1, 7).packed()
        );
        assert_eq!(
            read_u64(&bytes, function_id_offset + 8),
            FunctionId::global().packed()
        );
        assert_eq!(
            read_u64(&bytes, current_function_id_offset),
            FunctionId::new(1, 7).packed()
        );
        assert_eq!(
            read_u64(&bytes, current_function_id_offset + 8),
            FunctionId::global().packed()
        );
        assert_eq!(read_u64(&bytes, value_offset), 11);
        assert_eq!(read_u64(&bytes, value_offset + 8), 19);

        let string_offsets_end = string_offsets_offset + string_offsets_len;
        assert!(string_bytes_offset + string_bytes_len <= counter_id_offset);
        assert!(string_offsets_end <= string_bytes_offset);
    }
}
