pub mod bitpacker64;
pub mod delta_encoding_64;
pub mod zig_zag;

pub fn add(left: usize, right: usize) -> usize {
    left + right
}

#[cfg(test)]
mod tests {
    use super::*;
    use rand::{Rng, SeedableRng};
    use rand_chacha::ChaCha8Rng;
    use std::io::Seek;

    #[test]
    fn it_works() {
        let mut rng = ChaCha8Rng::seed_from_u64(3);
        let before: Vec<i64> = (0..64)
            .map(|_| rng.gen_range(1000000000000..=1100000000000))
            .collect();

        // let mut bytes = [0u8; 512];
        // bitpacker64::pack(&before, &mut bytes, 64).unwrap();
        // let mut after = [0u64; 64];
        // bitpacker64::unpack(&bytes, &mut after, 64).unwrap();

        // print!("l ");
        // bytes.iter().for_each(|a| {
        //     print!("{:08b}", a);
        // });
        //
        // println!("");
        // println!("b {:?}", before);
        // println!("a {:?}", after);

        let mut encoder = delta_encoding_64::Encoder::new(1);

        before.iter().for_each(|i| {
            encoder.write_integer(*i as i64).unwrap();
        });

        encoder.flush().unwrap();

        println!("{:?}", before);
        // println!("{:?}", encoder);

        let mut buffer = std::io::Cursor::new(vec![]);
        encoder.write(&mut buffer).unwrap();

        buffer.rewind().unwrap();

        println!(
            "{}",
            buffer.clone().into_inner().len() as f64 / (before.len() * 8) as f64
        );
        let mut decoder = delta_encoding_64::Decoder::new(buffer).unwrap();
        let numbers = decoder.read_integers().unwrap();

        assert_eq!(before, numbers);

        //
        // println!("{}", i32::MAX);
        //
        // println!("{:?}", 1234_i64.to_le_bytes())

        // let int = zig_zag::encode32(-1);
        // assert_eq!(int, 1u64);
        // let int = zig_zag::decode32(int);
        //
        //
        // let mut data = std::io::Cursor::new(vec![]);
        // data.write_vlq(8).unwrap();
        // data.set_position(0);
        //
        // let data = data.into_inner();
        // let mut data = std::io::Cursor::new(data);
        //
        // let x: u64 = data.read_vlq().unwrap();
        // println!("{}", x);
        //
        //
        // let mut my_data = vec![1u64; 128];
        // let mut my_data_32: Vec<u32> = Vec::with_capacity(256);
        // for i in &before {
        //     let bytes = i.to_le_bytes();
        //     let left = [bytes[0], bytes[1], bytes[2], bytes[3]];
        //     let right = [bytes[4], bytes[5], bytes[6], bytes[7]];
        //
        //     my_data_32.push(u32::from_le_bytes(left));
        //     my_data_32.push(u32::from_le_bytes(right));
        // }
        //
        // // Detects if `SSE3` is available on the current computed
        // // and uses the best available implementation accordingly.
        // let bitpacker = BitPacker8x::new();
        //
        // // Computes the number of bits used for each integers in the blocks.
        // // my_data is assumed to have a len of 128 for `BitPacker4x`.
        // let num_bits: u8 = bitpacker.num_bits(&my_data_32);
        //
        // // The compressed array will take exactly `num_bits * BitPacker4x::BLOCK_LEN / 8`.
        // // But it is ok to have an output with a different len as long as it is larger
        // // than this.
        // let mut compressed = vec![0u8; 4 * BitPacker8x::BLOCK_LEN];
        //
        // // Compress returns the len.
        // let compressed_len = bitpacker.compress(&my_data_32, &mut compressed[..], num_bits);
        // assert_eq!(
        //     (num_bits as usize) * BitPacker8x::BLOCK_LEN / 8,
        //     compressed_len
        // );
        //
        // // Decompressing
        // let mut decompressed = vec![0u32; BitPacker8x::BLOCK_LEN];
        //
        // bitpacker.decompress(
        //     &compressed[..compressed_len],
        //     &mut decompressed[..],
        //     num_bits,
        // );
        //
        // let mut after: Vec<i64> = Vec::with_capacity(128);
        // for chunk in decompressed.chunks(2) {
        //     let left = chunk[0].to_le_bytes();
        //     let right = chunk[1].to_le_bytes();
        //
        //     let i64 = i64::from_le_bytes([
        //         left[0], left[1], left[2], left[3], right[0], right[1], right[2], right[3],
        //     ]);
        //     after.push(i64)
        // }
        //
        // assert_eq!(&before, &after);
    }
}
