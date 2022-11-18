use crate::delta_encoding_64::{Decoder, Encoder};
use rust_decimal::prelude::ToPrimitive;
use rust_decimal::Decimal;
use serde_derive::{Deserialize, Serialize};
use std::io::Cursor;
use std::ops::Div;

#[derive(Serialize, Deserialize)]
pub struct DeltaVecDecimal {
    multiplier: Decimal,
    max_value: Decimal,
    delta_vec: DeltaVec,
}

impl DeltaVecDecimal {
    pub fn new() -> Self {
        Self {
            multiplier: Decimal::from(1),
            max_value: Decimal::MIN,
            delta_vec: DeltaVec::new(),
        }
    }

    pub fn push(&mut self, item: Decimal) {
        self.extend(vec![item])
    }

    pub fn extend(&mut self, items: Vec<Decimal>) {
        let mut current_values = self.to_vec();
        current_values.extend(items.clone());
        let mut precision = 0;
        for item in &current_values {
            self.max_value = self.max_value.max(*item);
            precision = precision.max(item.normalize().scale())
        }

        self.multiplier = if precision > 0 {
            Decimal::from(10u32.pow(precision.clamp(0, 8)))
        } else {
            Decimal::ONE
        };

        if self.max_value > Decimal::ONE {
            self.multiplier = self
                .multiplier
                .min(Decimal::from(i64::MAX).div(self.max_value).floor());
        }

        let ints = current_values
            .iter()
            .map(|decimal| (decimal * self.multiplier).to_i64().unwrap_or_default())
            .collect();
        self.delta_vec.replace(ints);
    }

    pub fn to_vec(&self) -> Vec<Decimal> {
        self.delta_vec
            .to_vec()
            .into_iter()
            .map(|int| Decimal::from(int) / self.multiplier)
            .collect()
    }
}

#[derive(Serialize, Deserialize)]
pub struct DeltaVec {
    bytes: Vec<u8>,
}

impl DeltaVec {
    pub fn new() -> Self {
        Self { bytes: vec![] }
    }

    pub fn push(&mut self, item: i64) {
        let mut encoder = Encoder::new(1);
        for int in self.to_vec() {
            encoder.write_integer(int).unwrap_or_default();
        }
        encoder.write_integer(item).unwrap_or_default();
        self.flush(&mut encoder);
    }

    pub fn extend(&mut self, items: Vec<i64>) {
        let mut encoder = Encoder::new(1);
        for int in self.to_vec() {
            encoder.write_integer(int).unwrap_or_default();
        }
        for item in items {
            encoder.write_integer(item).unwrap_or_default()
        }
        self.flush(&mut encoder);
    }

    pub fn replace(&mut self, items: Vec<i64>) {
        self.bytes.clear();
        self.extend(items);
    }

    pub fn to_vec(&self) -> Vec<i64> {
        if self.bytes.len() == 0 {
            return vec![];
        }

        let buffer = Cursor::new(self.bytes.to_vec());
        Decoder::new(buffer).unwrap().read_integers().unwrap()
    }

    fn flush(&mut self, encoder: &mut Encoder) {
        encoder.flush().unwrap();
        let mut buffer = std::io::Cursor::new(vec![]);
        encoder.write(&mut buffer).unwrap();
        self.bytes = buffer.into_inner();
    }
}
