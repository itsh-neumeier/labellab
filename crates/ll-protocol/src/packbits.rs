//! TIFF/PackBits encoder used for raster row compression (`M 02`, see
//! `docs/PROTOCOL.md`). Only encoding is needed in production; the printer
//! does the decoding. A decoder exists under `#[cfg(test)]` to verify
//! round-trips.

const MAX_RUN: usize = 128;

/// Encodes `data` using TIFF PackBits run-length encoding.
pub fn encode(data: &[u8]) -> Vec<u8> {
    let mut out = Vec::new();
    let mut i = 0;
    while i < data.len() {
        let run = run_length(data, i);
        if run >= 2 {
            out.push((1i32 - run as i32) as u8);
            out.push(data[i]);
            i += run;
        } else {
            let start = i;
            let mut lit_len = 0usize;
            while i < data.len() && lit_len < MAX_RUN && run_length(data, i) < 2 {
                lit_len += 1;
                i += 1;
            }
            out.push((lit_len - 1) as u8);
            out.extend_from_slice(&data[start..start + lit_len]);
        }
    }
    out
}

fn run_length(data: &[u8], start: usize) -> usize {
    let mut run = 1;
    while start + run < data.len() && data[start + run] == data[start] && run < MAX_RUN {
        run += 1;
    }
    run
}

#[cfg(test)]
mod tests {
    use super::*;

    fn decode(data: &[u8]) -> Vec<u8> {
        let mut out = Vec::new();
        let mut i = 0;
        while i < data.len() {
            let control = data[i] as i8;
            i += 1;
            if control >= 0 {
                let len = control as usize + 1;
                out.extend_from_slice(&data[i..i + len]);
                i += len;
            } else if control != -128 {
                let len = (1 - control as isize) as usize;
                let byte = data[i];
                i += 1;
                out.extend(std::iter::repeat_n(byte, len));
            }
        }
        out
    }

    #[test]
    fn roundtrip_all_same_byte() {
        let input = vec![0xAAu8; 10];
        assert_eq!(decode(&encode(&input)), input);
    }

    #[test]
    fn roundtrip_long_run_over_max_chunk() {
        let input = vec![0x00u8; 300];
        assert_eq!(decode(&encode(&input)), input);
    }

    #[test]
    fn roundtrip_all_distinct_bytes() {
        let input: Vec<u8> = (0..200).map(|i| (i % 256) as u8).collect();
        assert_eq!(decode(&encode(&input)), input);
    }

    #[test]
    fn roundtrip_mixed() {
        let mut input = vec![0x00, 0x01, 0x02];
        input.extend(vec![0xFFu8; 5]);
        input.extend([0x10, 0x20, 0x10, 0x20]);
        assert_eq!(decode(&encode(&input)), input);
    }

    #[test]
    fn empty_input_encodes_to_empty() {
        assert_eq!(encode(&[]), Vec::<u8>::new());
    }
}
