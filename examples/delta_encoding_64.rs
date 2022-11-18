use encoding_rust::delta_encoding_64;
use rand::{Rng, SeedableRng};
use rand_chacha::ChaCha8Rng;
use rust_decimal::Decimal;
use std::io::Seek;
use std::str::FromStr;

fn main() {
    let mut rng = ChaCha8Rng::seed_from_u64(3);
    let before: Vec<i64> = (0..4)
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

    let mut delta_vec = delta_encoding_64::DeltaVec::new();
    delta_vec.push(10);
    delta_vec.extend(vec![1, 2, 4]);
    let result = delta_vec.to_vec();
    assert_eq!(result, vec![10, 1, 2, 4]);

    delta_vec.push(10);
    delta_vec.extend(vec![1, 2, 4]);
    let result = delta_vec.to_vec();
    assert_eq!(result, vec![10, 1, 2, 4, 10, 1, 2, 4]);

    let mut delta_vec = delta_encoding_64::DeltaVecDecimal::new();
    delta_vec.push(Decimal::from(10));
    delta_vec.extend(vec![Decimal::from(1), Decimal::from(2), Decimal::from(4)]);
    let result = delta_vec.to_vec();
    assert_eq!(
        result,
        vec![
            Decimal::from(10),
            Decimal::from(1),
            Decimal::from(2),
            Decimal::from(4)
        ]
    );

    delta_vec.push(Decimal::from_str("3128558240363800.00000000000011").unwrap());
    delta_vec.extend(vec![
        Decimal::from_str("1.4").unwrap(),
        Decimal::from(2),
        Decimal::from(4),
    ]);
    let result = delta_vec.to_vec();
    assert_eq!(
        result,
        vec![
            Decimal::from(10),
            Decimal::from(1),
            Decimal::from(2),
            Decimal::from(4),
            Decimal::from_str("3128558240363800").unwrap(),
            Decimal::from_str("1.4").unwrap(),
            Decimal::from(2),
            Decimal::from(4)
        ]
    );
}
