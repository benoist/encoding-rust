use encoding_rust::delta_encoding_64;
use rand::{Rng, SeedableRng};
use rand_chacha::ChaCha8Rng;
use std::io::Seek;

fn main() {
    let mut rng = ChaCha8Rng::seed_from_u64(3);
    let before: Vec<i64> = (0..1)
        .map(|_| rng.gen_range(1000000000000..=1100000000000))
        .collect();

    let mut encoder = delta_encoding_64::Encoder::new(1);

    before.iter().for_each(|i| {
        encoder.write_integer(*i as i64).unwrap();
    });

    encoder.flush().unwrap();

    let mut buffer = std::io::Cursor::new(vec![]);
    encoder.write(&mut buffer).unwrap();

    buffer.rewind().unwrap();

    let mut decoder = delta_encoding_64::Decoder::new(buffer).expect("aa");
    let numbers = decoder.read_integers().unwrap();

    assert_eq!(before, numbers);
}
