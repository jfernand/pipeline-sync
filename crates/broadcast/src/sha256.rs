pub fn sha256(input: &[u8]) -> [u8; 32] {
    let mut h0 = 0x6a09e667u32;
    let mut h1 = 0xbb67ae85u32;
    let mut h2 = 0x3c6ef372u32;
    let mut h3 = 0xa54ff53au32;
    let mut h4 = 0x510e527fu32;
    let mut h5 = 0x9b05688cu32;
    let mut h6 = 0x1f83d9abu32;
    let mut h7 = 0x5be0cd19u32;

    let message_length_bits = (input.len() as u64) * 8;
    // Pad with 0x80, then zeros, until length ≡ 56 (mod 64), then the original bit length as a
    // big-endian 64-bit integer — bringing the total to a multiple of 64 bytes.
    // After the 0x80 byte, leave room for the 8-byte length field. The zero
    // padding therefore brings the message length to 56 (mod 64).
    let zero_padding_count = (55 + 64 - (input.len() % 64)) % 64;
    let length_bytes: Vec<u8> = (0..8)
        .map(|i| ((message_length_bits >> ((7 - i) * 8)) & 0xff) as u8)
        .collect();

    let mut padded = Vec::with_capacity(input.len() + 1 + zero_padding_count + 8);
    padded.extend_from_slice(input);
    padded.push(0x80);
    padded.resize(padded.len() + zero_padding_count, 0);
    padded.extend_from_slice(&length_bytes);

    for chunk_start in (0..padded.len()).step_by(64) {
        let mut w = [0u32; 64];
        for i in 0..16 {
            let base = chunk_start + i * 4;
            w[i] = ((padded[base] as u32) << 24)
                | ((padded[base + 1] as u32) << 16)
                | ((padded[base + 2] as u32) << 8)
                | (padded[base + 3] as u32);
        }
        for i in 16..64 {
            let s0 = rotr(w[i - 15], 7) ^ rotr(w[i - 15], 18) ^ (w[i - 15] >> 3);
            let s1 = rotr(w[i - 2], 17) ^ rotr(w[i - 2], 19) ^ (w[i - 2] >> 10);
            w[i] = w[i - 16]
                .wrapping_add(s0)
                .wrapping_add(w[i - 7])
                .wrapping_add(s1);
        }

        let mut a = h0;
        let mut b = h1;
        let mut c = h2;
        let mut d = h3;
        let mut e = h4;
        let mut f = h5;
        let mut g = h6;
        let mut h = h7;

        for i in 0..64 {
            let s1 = rotr(e, 6) ^ rotr(e, 11) ^ rotr(e, 25);
            let ch = (e & f) ^ (!e & g);
            let temp1 = h
                .wrapping_add(s1)
                .wrapping_add(ch)
                .wrapping_add(ROUND_CONSTANTS[i])
                .wrapping_add(w[i]);
            let s0 = rotr(a, 2) ^ rotr(a, 13) ^ rotr(a, 22);
            let maj = (a & b) ^ (a & c) ^ (b & c);
            let temp2 = s0.wrapping_add(maj);

            h = g;
            g = f;
            f = e;
            e = d.wrapping_add(temp1);
            d = c;
            c = b;
            b = a;
            a = temp1.wrapping_add(temp2);
        }

        h0 = h0.wrapping_add(a);
        h1 = h1.wrapping_add(b);
        h2 = h2.wrapping_add(c);
        h3 = h3.wrapping_add(d);
        h4 = h4.wrapping_add(e);
        h5 = h5.wrapping_add(f);
        h6 = h6.wrapping_add(g);
        h7 = h7.wrapping_add(h);
    }

    let mut result = [0u8; 32];
    for (index, &word) in [h0, h1, h2, h3, h4, h5, h6, h7]
        .iter()
        .enumerate()
    {
        result[index * 4] = (word >> 24) as u8;
        result[index * 4 + 1] = (word >> 16) as u8;
        result[index * 4 + 2] = (word >> 8) as u8;
        result[index * 4 + 3] = word as u8;
    }
    result
}

pub fn to_hex(bytes: &[u8]) -> String {
    bytes
        .iter()
        .map(|byte| format!("{:02x}", byte))
        .collect::<String>()
}

fn rotr(value: u32, bits: u32) -> u32 {
    value.rotate_right(bits)
}

const ROUND_CONSTANTS: [u32; 64] = [
    0x428a2f98, 0x71374491, 0xb5c0fbcf, 0xe9b5dba5, 0x3956c25b, 0x59f111f1, 0x923f82a4, 0xab1c5ed5,
    0xd807aa98, 0x12835b01, 0x243185be, 0x550c7dc3, 0x72be5d74, 0x80deb1fe, 0x9bdc06a7, 0xc19bf174,
    0xe49b69c1, 0xefbe4786, 0x0fc19dc6, 0x240ca1cc, 0x2de92c6f, 0x4a7484aa, 0x5cb0a9dc, 0x76f988da,
    0x983e5152, 0xa831c66d, 0xb00327c8, 0xbf597fc7, 0xc6e00bf3, 0xd5a79147, 0x06ca6351, 0x14292967,
    0x27b70a85, 0x2e1b2138, 0x4d2c6dfc, 0x53380d13, 0x650a7354, 0x766a0abb, 0x81c2c92e, 0x92722c85,
    0xa2bfe8a1, 0xa81a664b, 0xc24b8b70, 0xc76c51a3, 0xd192e819, 0xd6990624, 0xf40e3585, 0x106aa070,
    0x19a4c116, 0x1e376c08, 0x2748774c, 0x34b0bcb5, 0x391c0cb3, 0x4ed8aa4a, 0x5b9cca4f, 0x682e6ff3,
    0x748f82ee, 0x78a5636f, 0x84c87814, 0x8cc70208, 0x90befffa, 0xa4506ceb, 0xbef9a3f7, 0xc67178f2,
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_input() {
        assert_hash(
            "",
            "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
        )
    }

    fn assert_hash(value: &str, hash: &str) {
        assert_eq!(to_hex(&sha256(value.as_bytes())), hash)
    }

    #[test]
    fn short_input() {
        assert_hash(
            "abc",
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad",
        )
    }

    #[test]
    fn pangram_length_input_() {
        assert_hash(
            "The quick brown fox jumps over the lazy dog",
            "d7a8fbb307d7809469ca9abcb0082e4f8d5651e46d3cdb762d02d0bf37c9e592",
        )
    }

    #[test]
    fn input_exactly_at_the_padding_boundary() {
        // 55 bytes: input.size % 64 == 55, the largest size that still fits in a single block.
        assert_hash(
            &"a".repeat(55),
            "9f4390f8d30c2dd92ec9f095b65e2b9ae9b0a925a5258e241c9f1e910f734318",
        );
        // 56 bytes: one more forces a second block purely for padding.
        assert_hash(
            &"a".repeat(56),
            "b35439a4ac6f0948b6d6f9e3c6af0f5f590ce20f1bde7090ef7970686ec6738a",
        );
    }

    #[test]
    fn input_spanning_exactly_one_full_block() {
        assert_hash(
            &"a".repeat(64),
            "ffe054fe7ae0cb6dc65c3af9b61d5209f439851db43d0ba5997337df154668eb",
        )
    }

    #[test]
    fn input_spanning_two_blocks() {
        assert_hash(
            &"a".repeat(100),
            "2816597888e4a0d3a36b82b83316ab32680eb8f00f8cd3b904d681246d285a0e",
        )
    }

    #[test]
    fn different_inputs_never_collide_in_practice() {
        assert_ne!(
            to_hex(&sha256("a".as_bytes())),
            to_hex(&sha256("b".as_bytes()))
        )
    }
}
