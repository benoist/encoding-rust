// This is a modified version of the Scalar implementation to support u64 bitpacking
// Copyright (c) 2016 Paul Masurel
//
// Permission is hereby granted, free of charge, to any person obtaining a copy of this software and associated documentation files (the "Software"), to deal in the Software without restriction, including without limitation the rights to use, copy, modify, merge, publish, distribute, sublicense, and/or sell copies of the Software, and to permit persons to whom the Software is furnished to do so, subject to the following conditions:
//
// The above copyright notice and this permission notice shall be included in all copies or substantial portions of the Software.
//
// THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY, FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM, OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE SOFTWARE.

macro_rules! pack_unpack_with_bits {
    ($name:ident, $n:expr) => {
        pub mod $name {
            use crate::bitpacker64::*;
            use crunchy::unroll;
            use std::ptr::read_unaligned as load_unaligned;
            use std::ptr::write_unaligned as store_unaligned;

            const NUM_BITS: usize = $n;
            const NUM_BYTES_PER_BLOCK: usize = NUM_BITS * BLOCK_LEN / 8;

            pub unsafe fn pack(input_arr: &[u64], output_arr: &mut [u8]) -> usize {
                assert_eq!(
                    input_arr.len(),
                    BLOCK_LEN,
                    "Input block too small {}, (expected {})",
                    input_arr.len(),
                    BLOCK_LEN
                );
                assert!(
                    output_arr.len() >= NUM_BYTES_PER_BLOCK,
                    "Output array too small (numbits {}). {} <= {}",
                    NUM_BITS,
                    output_arr.len(),
                    NUM_BYTES_PER_BLOCK
                );

                let input_ptr = input_arr.as_ptr() as *const DataType;
                let mut output_ptr = output_arr.as_mut_ptr() as *mut DataType;
                let mut out_register: DataType = load_unaligned(input_ptr);

                unroll! {
                    for iter in 0..62 {
                        const i: usize = 1 + iter;
                        const bits_filled: usize = i * NUM_BITS;
                        const inner_cursor: usize = bits_filled % 64;
                        const remaining: usize = 64 - inner_cursor;

                        let offset_ptr = input_ptr.add(i);
                        let in_register = load_unaligned(offset_ptr);

                        out_register = if inner_cursor > 0 {
                            op_or(
                                out_register,
                                left_shift_64::<{ inner_cursor as i64 }>(in_register),
                            )
                        } else {
                            in_register
                        };

                        if remaining <= NUM_BITS {
                            store_unaligned(output_ptr, out_register);
                            output_ptr = output_ptr.offset(1);
                            if 0 < remaining && remaining < NUM_BITS {
                                out_register = right_shift_64::<{remaining as i64}>(in_register);
                            }
                        }
                    }
                }
                let in_register: DataType = load_unaligned(input_ptr.add(63));
                out_register = if 64 - NUM_BITS > 0 {
                    op_or(
                        out_register,
                        left_shift_64::<{ 64 - NUM_BITS as i64 }>(in_register),
                    )
                } else {
                    op_or(out_register, in_register)
                };
                store_unaligned(output_ptr, out_register);

                NUM_BYTES_PER_BLOCK
            }

            pub unsafe fn unpack(compressed: &[u8], output: &mut [u64]) -> usize {
                assert!(
                    compressed.len() >= NUM_BYTES_PER_BLOCK,
                    "Compressed array seems too small. ({} < {}) ",
                    compressed.len(),
                    NUM_BYTES_PER_BLOCK
                );

                let mut input_ptr = compressed.as_ptr() as *const DataType;
                let mut store = Store::new(output.as_mut_ptr() as *mut DataType);

                let mask_scalar: u64 = ((1u64 << NUM_BITS) - 1u64) as u64;
                let mask = set1(mask_scalar as i64);

                let mut in_register: DataType = load_unaligned(input_ptr);

                let out_register = op_and(in_register, mask);
                store.process(out_register);

                unroll! {
                    for iter in 0..63 {
                        const i: usize = iter + 1;

                        const inner_cursor: usize = (i * NUM_BITS) % 64;
                        const inner_capacity: usize = 64 - inner_cursor;

                        let shifted_in_register = if inner_cursor != 0 {
                            right_shift_64::<{inner_cursor as i64}>(in_register)
                        } else {
                            in_register
                        };
                        let mut out_register: DataType = op_and(shifted_in_register, mask);

                        // We consumed our current quadruplets entirely.
                        // We therefore read another one.
                        if inner_capacity <= NUM_BITS && i != 63 {
                            input_ptr = input_ptr.add(1);
                            in_register = load_unaligned(input_ptr);

                            // This quadruplets is actually cutting one of
                            // our `DataType`. We need to read the next one.
                            if inner_capacity < NUM_BITS {
                                let shifted = if inner_capacity != 0 {
                                    left_shift_64::<{inner_capacity as i64}>(in_register)
                                } else {
                                    in_register
                                };
                                let masked = op_and(shifted, mask);
                                out_register = op_or(out_register, masked);
                            }
                        }

                        store.process(out_register);
                    }
                }

                NUM_BYTES_PER_BLOCK
            }
        }
    };
}

mod pack_unpack_64 {
    use super::BLOCK_LEN;
    use super::{load_unaligned, store_unaligned, DataType, Store};
    use crunchy::unroll;

    const NUM_BITS: usize = 64;
    const NUM_BYTES_PER_BLOCK: usize = NUM_BITS * BLOCK_LEN / 8;

    pub unsafe fn pack(input_arr: &[u64], output_arr: &mut [u8]) -> usize {
        assert_eq!(
            input_arr.len(),
            BLOCK_LEN,
            "Input block too small {}, (expected {})",
            input_arr.len(),
            BLOCK_LEN
        );
        assert!(
            output_arr.len() >= NUM_BYTES_PER_BLOCK,
            "Output array too small (numbits {}). {} <= {}",
            NUM_BITS,
            output_arr.len(),
            NUM_BYTES_PER_BLOCK
        );

        let input_ptr: *const DataType = input_arr.as_ptr() as *const DataType;
        let output_ptr = output_arr.as_mut_ptr() as *mut DataType;
        unroll! {
            for i in 0..64 {
                let input_offset_ptr = input_ptr.offset(i as isize);
                let output_offset_ptr = output_ptr.offset(i as isize);
                let input_register = load_unaligned(input_offset_ptr);
                let output_register = input_register;
                store_unaligned(output_offset_ptr, output_register);
            }
        }
        NUM_BYTES_PER_BLOCK
    }

    pub unsafe fn unpack(compressed: &[u8], output: &mut [u64]) -> usize {
        assert!(
            compressed.len() >= NUM_BYTES_PER_BLOCK,
            "Compressed array seems too small. ({} < {}) ",
            compressed.len(),
            NUM_BYTES_PER_BLOCK
        );
        let input_ptr = compressed.as_ptr() as *const DataType;
        let mut store = Store::new(output.as_mut_ptr() as *mut DataType);
        for i in 0..64 {
            let input_offset_ptr = input_ptr.offset(i as isize);
            let in_register: DataType = load_unaligned(input_offset_ptr);
            store.process(in_register);
        }
        NUM_BYTES_PER_BLOCK
    }
}

use crunchy::unroll;
use std::ptr::read_unaligned as load_unaligned;
use std::ptr::write_unaligned as store_unaligned;
type DataType = u64;
pub const BLOCK_LEN: usize = 64;

struct Store {
    output_ptr: *mut DataType,
}

impl Store {
    fn new(output_ptr: *mut DataType) -> Store {
        Store { output_ptr }
    }

    #[inline]
    unsafe fn process(&mut self, out_register: DataType) {
        store_unaligned(self.output_ptr, out_register);
        self.output_ptr = self.output_ptr.add(1);
    }
}

fn set1(el: i64) -> DataType {
    el as u64
}

fn op_or(left: DataType, right: DataType) -> DataType {
    left | right
}
fn op_and(left: DataType, right: DataType) -> DataType {
    left & right
}

fn left_shift_64<const N: i64>(el: DataType) -> DataType {
    el << N
}

fn right_shift_64<const N: i64>(el: DataType) -> DataType {
    el >> N
}

fn or_collapse_to_u64(accumulator: DataType) -> u64 {
    accumulator
}

fn most_significant_bit(v: u64) -> u8 {
    if v == 0 {
        0
    } else {
        64u8 - (v.leading_zeros() as u8)
    }
}

pub fn num_bits(decompressed: &[u64]) -> u8 {
    assert_eq!(
        decompressed.len(),
        BLOCK_LEN,
        "`decompressed`'s len is not `BLOCK_LEN={}`",
        BLOCK_LEN
    );

    let mut accumulator = decompressed[0];
    unroll! {
        for iter in 0..63 {
            let i = iter + 1;
            let newvec = decompressed[i];
            accumulator = op_or(accumulator, newvec);
        }
    }
    most_significant_bit(or_collapse_to_u64(accumulator))
}

pack_unpack_with_bits!(pack_unpack_1, 1);
pack_unpack_with_bits!(pack_unpack_2, 2);
pack_unpack_with_bits!(pack_unpack_3, 3);
pack_unpack_with_bits!(pack_unpack_4, 4);
pack_unpack_with_bits!(pack_unpack_5, 5);
pack_unpack_with_bits!(pack_unpack_6, 6);
pack_unpack_with_bits!(pack_unpack_7, 7);
pack_unpack_with_bits!(pack_unpack_8, 8);
pack_unpack_with_bits!(pack_unpack_9, 9);
pack_unpack_with_bits!(pack_unpack_10, 10);
pack_unpack_with_bits!(pack_unpack_11, 11);
pack_unpack_with_bits!(pack_unpack_12, 12);
pack_unpack_with_bits!(pack_unpack_13, 13);
pack_unpack_with_bits!(pack_unpack_14, 14);
pack_unpack_with_bits!(pack_unpack_15, 15);
pack_unpack_with_bits!(pack_unpack_16, 16);
pack_unpack_with_bits!(pack_unpack_17, 17);
pack_unpack_with_bits!(pack_unpack_18, 18);
pack_unpack_with_bits!(pack_unpack_19, 19);
pack_unpack_with_bits!(pack_unpack_20, 20);
pack_unpack_with_bits!(pack_unpack_21, 21);
pack_unpack_with_bits!(pack_unpack_22, 22);
pack_unpack_with_bits!(pack_unpack_23, 23);
pack_unpack_with_bits!(pack_unpack_24, 24);
pack_unpack_with_bits!(pack_unpack_25, 25);
pack_unpack_with_bits!(pack_unpack_26, 26);
pack_unpack_with_bits!(pack_unpack_27, 27);
pack_unpack_with_bits!(pack_unpack_28, 28);
pack_unpack_with_bits!(pack_unpack_29, 29);
pack_unpack_with_bits!(pack_unpack_30, 30);
pack_unpack_with_bits!(pack_unpack_31, 31);
pack_unpack_with_bits!(pack_unpack_32, 32);
pack_unpack_with_bits!(pack_unpack_33, 33);
pack_unpack_with_bits!(pack_unpack_34, 34);
pack_unpack_with_bits!(pack_unpack_35, 35);
pack_unpack_with_bits!(pack_unpack_36, 36);
pack_unpack_with_bits!(pack_unpack_37, 37);
pack_unpack_with_bits!(pack_unpack_38, 38);
pack_unpack_with_bits!(pack_unpack_39, 39);
pack_unpack_with_bits!(pack_unpack_40, 40);
pack_unpack_with_bits!(pack_unpack_41, 41);
pack_unpack_with_bits!(pack_unpack_42, 42);
pack_unpack_with_bits!(pack_unpack_43, 43);
pack_unpack_with_bits!(pack_unpack_44, 44);
pack_unpack_with_bits!(pack_unpack_45, 45);
pack_unpack_with_bits!(pack_unpack_46, 46);
pack_unpack_with_bits!(pack_unpack_47, 47);
pack_unpack_with_bits!(pack_unpack_48, 48);
pack_unpack_with_bits!(pack_unpack_49, 49);
pack_unpack_with_bits!(pack_unpack_50, 50);
pack_unpack_with_bits!(pack_unpack_51, 51);
pack_unpack_with_bits!(pack_unpack_52, 52);
pack_unpack_with_bits!(pack_unpack_53, 53);
pack_unpack_with_bits!(pack_unpack_54, 54);
pack_unpack_with_bits!(pack_unpack_55, 55);
pack_unpack_with_bits!(pack_unpack_56, 56);
pack_unpack_with_bits!(pack_unpack_57, 57);
pack_unpack_with_bits!(pack_unpack_58, 58);
pack_unpack_with_bits!(pack_unpack_59, 59);
pack_unpack_with_bits!(pack_unpack_60, 60);
pack_unpack_with_bits!(pack_unpack_61, 61);
pack_unpack_with_bits!(pack_unpack_62, 62);
pack_unpack_with_bits!(pack_unpack_63, 63);

pub fn pack(unpacked: &[u64], packed: &mut [u8], bit_width: u8) -> usize {
    unsafe {
        match bit_width {
            1 => pack_unpack_1::pack(unpacked, packed),
            2 => pack_unpack_2::pack(unpacked, packed),
            3 => pack_unpack_3::pack(unpacked, packed),
            4 => pack_unpack_4::pack(unpacked, packed),
            5 => pack_unpack_5::pack(unpacked, packed),
            6 => pack_unpack_6::pack(unpacked, packed),
            7 => pack_unpack_7::pack(unpacked, packed),
            8 => pack_unpack_8::pack(unpacked, packed),
            9 => pack_unpack_9::pack(unpacked, packed),
            10 => pack_unpack_10::pack(unpacked, packed),
            11 => pack_unpack_11::pack(unpacked, packed),
            12 => pack_unpack_12::pack(unpacked, packed),
            13 => pack_unpack_13::pack(unpacked, packed),
            14 => pack_unpack_14::pack(unpacked, packed),
            15 => pack_unpack_15::pack(unpacked, packed),
            16 => pack_unpack_16::pack(unpacked, packed),
            17 => pack_unpack_17::pack(unpacked, packed),
            18 => pack_unpack_18::pack(unpacked, packed),
            19 => pack_unpack_19::pack(unpacked, packed),
            20 => pack_unpack_20::pack(unpacked, packed),
            21 => pack_unpack_21::pack(unpacked, packed),
            22 => pack_unpack_22::pack(unpacked, packed),
            23 => pack_unpack_23::pack(unpacked, packed),
            24 => pack_unpack_24::pack(unpacked, packed),
            25 => pack_unpack_25::pack(unpacked, packed),
            26 => pack_unpack_26::pack(unpacked, packed),
            27 => pack_unpack_27::pack(unpacked, packed),
            28 => pack_unpack_28::pack(unpacked, packed),
            29 => pack_unpack_29::pack(unpacked, packed),
            30 => pack_unpack_30::pack(unpacked, packed),
            31 => pack_unpack_31::pack(unpacked, packed),
            32 => pack_unpack_32::pack(unpacked, packed),
            33 => pack_unpack_33::pack(unpacked, packed),
            34 => pack_unpack_34::pack(unpacked, packed),
            35 => pack_unpack_35::pack(unpacked, packed),
            36 => pack_unpack_36::pack(unpacked, packed),
            37 => pack_unpack_37::pack(unpacked, packed),
            38 => pack_unpack_38::pack(unpacked, packed),
            39 => pack_unpack_39::pack(unpacked, packed),
            40 => pack_unpack_40::pack(unpacked, packed),
            41 => pack_unpack_41::pack(unpacked, packed),
            42 => pack_unpack_42::pack(unpacked, packed),
            43 => pack_unpack_43::pack(unpacked, packed),
            44 => pack_unpack_44::pack(unpacked, packed),
            45 => pack_unpack_45::pack(unpacked, packed),
            46 => pack_unpack_46::pack(unpacked, packed),
            47 => pack_unpack_47::pack(unpacked, packed),
            48 => pack_unpack_48::pack(unpacked, packed),
            49 => pack_unpack_49::pack(unpacked, packed),
            50 => pack_unpack_50::pack(unpacked, packed),
            51 => pack_unpack_51::pack(unpacked, packed),
            52 => pack_unpack_52::pack(unpacked, packed),
            53 => pack_unpack_53::pack(unpacked, packed),
            54 => pack_unpack_54::pack(unpacked, packed),
            55 => pack_unpack_55::pack(unpacked, packed),
            56 => pack_unpack_56::pack(unpacked, packed),
            57 => pack_unpack_57::pack(unpacked, packed),
            58 => pack_unpack_58::pack(unpacked, packed),
            59 => pack_unpack_59::pack(unpacked, packed),
            60 => pack_unpack_60::pack(unpacked, packed),
            61 => pack_unpack_61::pack(unpacked, packed),
            62 => pack_unpack_62::pack(unpacked, packed),
            63 => pack_unpack_63::pack(unpacked, packed),
            64 => pack_unpack_64::pack(unpacked, packed),
            _ => 0,
        }
    }
}

pub fn unpack(packed: &[u8], unpacked: &mut [u64], bit_width: u8) -> usize {
    unsafe {
        match bit_width {
            1 => pack_unpack_1::unpack(packed, unpacked),
            2 => pack_unpack_2::unpack(packed, unpacked),
            3 => pack_unpack_3::unpack(packed, unpacked),
            4 => pack_unpack_4::unpack(packed, unpacked),
            5 => pack_unpack_5::unpack(packed, unpacked),
            6 => pack_unpack_6::unpack(packed, unpacked),
            7 => pack_unpack_7::unpack(packed, unpacked),
            8 => pack_unpack_8::unpack(packed, unpacked),
            9 => pack_unpack_9::unpack(packed, unpacked),
            10 => pack_unpack_10::unpack(packed, unpacked),
            11 => pack_unpack_11::unpack(packed, unpacked),
            12 => pack_unpack_12::unpack(packed, unpacked),
            13 => pack_unpack_13::unpack(packed, unpacked),
            14 => pack_unpack_14::unpack(packed, unpacked),
            15 => pack_unpack_15::unpack(packed, unpacked),
            16 => pack_unpack_16::unpack(packed, unpacked),
            17 => pack_unpack_17::unpack(packed, unpacked),
            18 => pack_unpack_18::unpack(packed, unpacked),
            19 => pack_unpack_19::unpack(packed, unpacked),
            20 => pack_unpack_20::unpack(packed, unpacked),
            21 => pack_unpack_21::unpack(packed, unpacked),
            22 => pack_unpack_22::unpack(packed, unpacked),
            23 => pack_unpack_23::unpack(packed, unpacked),
            24 => pack_unpack_24::unpack(packed, unpacked),
            25 => pack_unpack_25::unpack(packed, unpacked),
            26 => pack_unpack_26::unpack(packed, unpacked),
            27 => pack_unpack_27::unpack(packed, unpacked),
            28 => pack_unpack_28::unpack(packed, unpacked),
            29 => pack_unpack_29::unpack(packed, unpacked),
            30 => pack_unpack_30::unpack(packed, unpacked),
            31 => pack_unpack_31::unpack(packed, unpacked),
            32 => pack_unpack_32::unpack(packed, unpacked),
            33 => pack_unpack_33::unpack(packed, unpacked),
            34 => pack_unpack_34::unpack(packed, unpacked),
            35 => pack_unpack_35::unpack(packed, unpacked),
            36 => pack_unpack_36::unpack(packed, unpacked),
            37 => pack_unpack_37::unpack(packed, unpacked),
            38 => pack_unpack_38::unpack(packed, unpacked),
            39 => pack_unpack_39::unpack(packed, unpacked),
            40 => pack_unpack_40::unpack(packed, unpacked),
            41 => pack_unpack_41::unpack(packed, unpacked),
            42 => pack_unpack_42::unpack(packed, unpacked),
            43 => pack_unpack_43::unpack(packed, unpacked),
            44 => pack_unpack_44::unpack(packed, unpacked),
            45 => pack_unpack_45::unpack(packed, unpacked),
            46 => pack_unpack_46::unpack(packed, unpacked),
            47 => pack_unpack_47::unpack(packed, unpacked),
            48 => pack_unpack_48::unpack(packed, unpacked),
            49 => pack_unpack_49::unpack(packed, unpacked),
            50 => pack_unpack_50::unpack(packed, unpacked),
            51 => pack_unpack_51::unpack(packed, unpacked),
            52 => pack_unpack_52::unpack(packed, unpacked),
            53 => pack_unpack_53::unpack(packed, unpacked),
            54 => pack_unpack_54::unpack(packed, unpacked),
            55 => pack_unpack_55::unpack(packed, unpacked),
            56 => pack_unpack_56::unpack(packed, unpacked),
            57 => pack_unpack_57::unpack(packed, unpacked),
            58 => pack_unpack_58::unpack(packed, unpacked),
            59 => pack_unpack_59::unpack(packed, unpacked),
            60 => pack_unpack_60::unpack(packed, unpacked),
            61 => pack_unpack_61::unpack(packed, unpacked),
            62 => pack_unpack_62::unpack(packed, unpacked),
            63 => pack_unpack_63::unpack(packed, unpacked),
            64 => pack_unpack_64::unpack(packed, unpacked),
            _ => 0,
        }
    }
}
