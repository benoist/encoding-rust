use crate::bitpacker64::{num_bits, BLOCK_LEN};
use crate::{bitpacker64, zig_zag};
use std::collections::VecDeque;
use std::io::{Cursor, Read, Write};
use vlq::{ReadVlqExt, WriteVlqExt};

#[derive(Debug)]
pub struct Encoder {
    pub first_value: i64,
    pub previous_value: i64,
    pub bit_widths: Vec<u8>,
    pub total_count: usize,
    pub min_delta: i64,
    pub deltas: Vec<i64>,
    pub pos: usize,
    pub blocks_buffer: Cursor<Vec<u8>>,

    block_size: usize,
    mini_blocks: usize,
    mini_block_size: usize,
}

fn write_zig_zag_var_int<T: Write>(buffer: &mut T, value: i64) -> anyhow::Result<()> {
    buffer
        .write_vlq(zig_zag::encode64(value))
        .map_err(|e| anyhow::anyhow!(e))
}

fn decode_zig_zag_var_int<T: Read>(buffer: &mut T) -> anyhow::Result<i64> {
    let i = buffer.read_vlq().map_err(|e| anyhow::anyhow!(e))?;
    Ok(zig_zag::decode64(i))
}

impl Encoder {
    pub fn new(mini_blocks: usize) -> Self {
        let mini_block_size = BLOCK_LEN;
        let block_size = mini_block_size * mini_blocks;

        Self {
            first_value: 0,
            previous_value: 0,
            bit_widths: vec![0; mini_blocks],
            total_count: 0,
            min_delta: i64::MAX,
            deltas: vec![0; block_size],
            pos: 0,
            blocks_buffer: Default::default(),
            block_size,
            mini_blocks,
            mini_block_size,
        }
    }

    pub fn write_integer(&mut self, value: i64) -> anyhow::Result<()> {
        self.total_count += 1;

        if self.total_count == 1 {
            self.first_value = value;
            self.previous_value = self.first_value;
            return Ok(());
        }

        let delta = value - self.previous_value;
        self.previous_value = value;

        self.deltas[self.pos] = delta;
        self.pos += 1;

        self.min_delta = delta.min(self.min_delta);

        if self.block_size == self.pos {
            self.flush_buffer()?;
        }
        Ok(())
    }

    pub fn flush(&mut self) -> anyhow::Result<()> {
        if self.pos == 0 {
            return Ok(());
        }
        let extra_to_write = self.block_size - self.pos;
        for _ in 0..extra_to_write {
            self.write_integer(self.previous_value.wrapping_add(self.min_delta))?;
        }
        self.total_count -= extra_to_write;

        Ok(())
    }

    fn flush_buffer(&mut self) -> anyhow::Result<()> {
        if self.pos == 0 {
            return Ok(());
        }

        (0..self.pos).for_each(|i| {
            let new_delta = self.deltas.get(i).unwrap().wrapping_sub(self.min_delta);
            self.deltas[i] = new_delta;
        });

        self.write_min_delta()?;

        let mini_blocks_to_flush = self.mini_block_count_to_flush(self.pos);
        self.calculate_bit_widths_for_delta_block_buffer(mini_blocks_to_flush);
        let _ = self
            .blocks_buffer
            .write(&self.bit_widths[0..mini_blocks_to_flush]);

        (0..mini_blocks_to_flush).for_each(|index| {
            let bit_width = std::mem::take(&mut self.bit_widths[index]);
            if bit_width > 0 {
                let start = self.mini_block_size * index;
                let end = self.mini_block_size * (index + 1);

                let deltas: Vec<u64> = self.deltas[start..end].iter().map(|i| *i as u64).collect();

                let mut compressed = vec![0u8; BLOCK_LEN / 8 * bit_width as usize];
                bitpacker64::pack(&deltas, &mut compressed, bit_width);
                self.blocks_buffer.write(&compressed).unwrap();
            }
        });

        self.min_delta = i64::MAX;
        self.pos = 0;
        Ok(())
    }

    fn write_min_delta(&mut self) -> anyhow::Result<()> {
        write_zig_zag_var_int(&mut self.blocks_buffer, self.min_delta)
    }

    fn mini_block_count_to_flush(&self, number_count: usize) -> usize {
        ((number_count as f64) / (self.mini_block_size as f64)).ceil() as usize
    }

    fn calculate_bit_widths_for_delta_block_buffer(&mut self, mini_blocks_to_flush: usize) {
        (0..mini_blocks_to_flush).for_each(|index| {
            let deltas: Vec<u64> = self.deltas
                [self.mini_block_size * index..self.mini_block_size * (index + 1)]
                .iter()
                .map(|i| *i as u64)
                .collect();
            let bit_width = num_bits(&deltas);
            self.bit_widths[index] = bit_width;
        })
    }

    pub fn write<T: Write>(&mut self, io: &mut T) -> anyhow::Result<()> {
        io.write_vlq(self.block_size)
            .map_err(|e| anyhow::anyhow!(e))?;
        io.write_vlq(self.mini_blocks)
            .map_err(|e| anyhow::anyhow!(e))?;
        io.write_vlq(self.total_count)
            .map_err(|e| anyhow::anyhow!(e))?;

        write_zig_zag_var_int(io, self.first_value)?;

        let bytes = std::mem::take(&mut self.blocks_buffer).into_inner();
        io.write_all(&bytes).map_err(|e| anyhow::anyhow!(e))?;
        Ok(())
    }
}

#[derive(Debug)]
pub struct Decoder<T: Read> {
    pub first_value: i64,
    pub previous_value: i64,
    pub bit_widths: VecDeque<u8>,
    pub total_count: usize,
    pub min_delta: i64,
    pub deltas: VecDeque<i64>,
    pub values_read: usize,
    pub io: T,

    mini_blocks: usize,
    mini_block_size: usize,
}

impl<T: Read> Decoder<T> {
    pub fn new(mut io: T) -> anyhow::Result<Self> {
        let block_size: usize = io.read_vlq().unwrap();
        let mini_blocks = io.read_vlq().unwrap();
        let total_count = io.read_vlq().unwrap();

        if block_size == 0 || mini_blocks == 0 || block_size / mini_blocks != BLOCK_LEN {
            anyhow::bail!("Invalid header {} {}", block_size, mini_blocks)
        }

        let first_value = decode_zig_zag_var_int(&mut io)?;

        let mut decoder = Self {
            total_count,
            first_value,
            previous_value: first_value,
            bit_widths: VecDeque::with_capacity(mini_blocks),
            min_delta: 0,
            deltas: Default::default(),
            values_read: 0,
            io,
            mini_blocks,
            mini_block_size: BLOCK_LEN,
        };

        if total_count > 1 {
            decoder.read_block()?;
        }

        Ok(decoder)
    }

    pub fn read_integers(&mut self) -> anyhow::Result<Vec<i64>> {
        let mut values = vec![0; self.total_count];

        for i in 0..self.total_count {
            values[i] = self.read_integer()?
        }

        Ok(values)
    }

    pub fn read_integer(&mut self) -> anyhow::Result<i64> {
        self.check_read();

        self.values_read += 1;
        if self.values_read == 1 {
            return Ok(self.first_value);
        }

        if self.deltas.len() == 0 {
            self.read_deltas()?;
        }

        let value = self.previous_value + self.deltas.pop_front().unwrap();
        self.previous_value = value;
        Ok(value)
    }

    pub fn check_read(&self) {
        if self.all_read() {
            panic!("All values read");
        }
    }

    pub fn all_read(&self) -> bool {
        self.values_read == self.total_count
    }

    pub fn read_block(&mut self) -> anyhow::Result<()> {
        self.min_delta = decode_zig_zag_var_int(&mut self.io)?;

        let mut bit_widths = vec![0; self.mini_blocks];
        self.io
            .read(&mut bit_widths)
            .map_err(|e| anyhow::anyhow!(e))?;
        self.bit_widths.extend(bit_widths.iter());
        Ok(())
    }

    pub fn read_deltas(&mut self) -> anyhow::Result<()> {
        if self.bit_widths.len() == 0 {
            self.read_block()?;
        }

        let bit_width = self.bit_widths.pop_front().unwrap();
        let result = if bit_width > 0u8 {
            let mut packed = vec![0_u8; BLOCK_LEN / 8 * bit_width as usize];
            self.io.read(&mut packed).map_err(|e| anyhow::anyhow!(e))?;
            let mut result = vec![0_u64; self.mini_block_size];

            bitpacker64::unpack(&packed, &mut result, bit_width);
            result
        } else {
            vec![0; self.mini_block_size]
        };

        self.deltas.extend(
            result
                .iter()
                .map(|i| (*i as i64).wrapping_add(self.min_delta)),
        );

        Ok(())
    }
}
