use anyhow::anyhow;
use anyhow::Result;
use nom_derive::*;
use std::collections::HashMap;
use std::fmt::Display;
use std::fs::File;
use std::io::BufReader;
use std::io::*;
use std::os::unix::prelude::FileExt;

const MAGIC_BYTE: [u8; 4] = [0xf9, 0xbe, 0xb4, 0xd9];

pub struct TxParser {
    blocks: HashMap<u32, (u64, BitcoinBlock)>,
    reader: BufReader<File>,
    chain_start: u64,
}

impl TxParser {
    fn new(file_name: String) -> Result<Self> {
        let file = File::open(file_name).expect("file path does not exist");

        let mut buf = [0; 4];
        let mut offset = 0;

        // TODO: Optimize
        let _ = file.read_at(&mut buf, offset)?;
        while buf != MAGIC_BYTE {
            offset += 1;
            file.read_at(&mut buf, offset)?;
        }

        let mut reader = BufReader::new(file);
        reader.seek_relative(offset as i64)?;

        Ok(TxParser {
            blocks: HashMap::new(),
            reader,
            chain_start: offset,
        })
    }

    pub fn read_block(&mut self, block_height: u32) -> Result<BitcoinBlock> {
        // Check cache and return if hit
        if let Some((_, block)) = self.blocks.get(&block_height) {
            return Ok(block.clone());
        }

        // Parse the buffer
        let mut nearest_block = block_height - 1;
        while nearest_block != 0 && !self.blocks.contains_key(&nearest_block) {
            nearest_block -= 1;
        }

        let mut offset = if let Some((size, _)) = self.blocks.get(&nearest_block) {
            *size
        } else {
            // Initial parser state
            self.chain_start
        };

        for height in nearest_block..=block_height {
            let (size, block) = fetch_block(&mut self.reader, offset)?;
            self.blocks.insert(height, (offset, block));
            offset += size;
        }

        Ok(self
            .blocks
            .get(&block_height)
            .ok_or(anyhow!("block not present"))?
            .1
            .clone())
    }
}

fn fetch_block(reader: &mut BufReader<File>, offset: u64) -> Result<(u64, BitcoinBlock)> {
    // Place the buffer reader at start
    reader.seek(SeekFrom::Start(offset))?;

    // 4 bytes - Magic byte
    // 4 bytes - size
    // total => 8 Bytes
    let mut buf = [0u8; 8];

    reader.read_exact(&mut buf)?;

    let mut magic_bytes = [0u8; 4];
    magic_bytes.copy_from_slice(&buf[0..4]);

    if magic_bytes != MAGIC_BYTE {
        return Err(anyhow!("Magic byte incorrect"));
    }

    let mut size_bytes = [0u8; 4];
    size_bytes.copy_from_slice(&buf[4..8]);
    let transaction_block_size: u32 = u32::from_le_bytes(size_bytes);
    let mut transaction_block_buffer = vec![0u8; transaction_block_size as usize];
    reader.read_exact(&mut transaction_block_buffer)?;

    let (_, transaction_block) = TransactionBlock::parse_le(&transaction_block_buffer).unwrap();
    let block = BitcoinBlock {
        magic_bytes,
        size: transaction_block_size,
        block: transaction_block,
    };

    let next_block_pos: u64 = transaction_block_size as u64 + 8;
    Ok((next_block_pos, block))
}

#[repr(C)]
#[derive(Debug, Clone, Nom)]
pub struct BitcoinBlock {
    magic_bytes: [u8; 4],
    size: u32,
    block: TransactionBlock,
}

#[repr(C)]
#[derive(Debug, Clone, Nom)]
pub struct TransactionBlock {
    block_header: BlockHeader,
    tx_count: u32,
    tx_data: Vec<u8>,
}

#[derive(Debug, Clone, Copy, Nom)]
pub struct BlockHeader {
    version: i32,
    prev_block_hash: [u8; 32],
    merkle_hash: [u8; 32],
    time: u32,
    nbits: u32,
    nonce: u32,
}

impl Display for BlockHeader {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_fmt(format_args!(
            "
Block version: {},
Previous block hash: {},
Merkle hash: {},
time: {},
nbits: {},
nonce: {},
",
            self.version,
            self.prev_block_hash
                .iter()
                .map(|byte| format!("{:x}", byte))
                .fold(String::new(), |acc, x| acc + &x),
            self.merkle_hash
                .iter()
                .map(|byte| format!("{:x}", byte))
                .fold(String::new(), |acc, x| acc + &x),
            self.time,
            self.nbits,
            self.nonce
        ))
    }
}

fn main() -> Result<()> {
    let block_number = 2;
    let mut parser = TxParser::new("garbage_header.dat".into())?;
    let block = parser.read_block(block_number)?;

    println!(
        "Block {} header:\n{}",
        block_number, block.block.block_header
    );
    Ok(())
}
