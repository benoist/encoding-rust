pub fn encode32(int: i32) -> u64 {
    ((int << 1) ^ (int >> 31)) as u64
}

pub fn decode32(int: u64) -> i32 {
    (int as u32 >> 1) as i32 ^ -(int as i32 & 1)
}

pub fn encode64(int: i64) -> u64 {
    ((int << 1) ^ (int >> 63)) as u64
}

pub fn decode64(int: u64) -> i64 {
    ((int >> 1) ^ (-((int & 1) as i64)) as u64) as i64
}
