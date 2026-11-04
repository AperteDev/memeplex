//! What a GIF says about its frames, which the decoders of the platforms do not give.

/// How long each frame of the GIF at `path` shows, in milliseconds, in order; `None` if it is
/// not a GIF that can be read. As browsers do, a delay of 0 or 1 hundredths of a second is 100 ms.
pub fn frame_delays(path: &str) -> Option<Vec<u32>> {
    parse(&std::fs::read(path).ok()?)
}

fn parse(b: &[u8]) -> Option<Vec<u32>> {
    if !(b.starts_with(b"GIF87a") || b.starts_with(b"GIF89a")) {
        return None;
    }
    // Skips the data sub-blocks from `p`: each is a length byte and that many bytes, up to a
    // length of 0.
    let skip = |mut p: usize| -> Option<usize> {
        loop {
            let n = *b.get(p)? as usize;
            p += 1 + n;
            if n == 0 {
                return Some(p);
            }
        }
    };
    let table = |flags: u8| {
        if flags & 0x80 != 0 {
            3 << ((flags & 7) + 1)
        } else {
            0
        }
    };
    let mut p = 13 + table(*b.get(10)?);
    let (mut delays, mut delay) = (Vec::new(), 0u32);
    loop {
        match *b.get(p)? {
            // An extension; the graphic control one (0xF9) holds the delay of the next frame.
            0x21 => {
                if *b.get(p + 1)? == 0xF9 {
                    delay = u32::from(u16::from_le_bytes([*b.get(p + 4)?, *b.get(p + 5)?]));
                }
                p = skip(p + 2)?;
            }
            // A frame: its descriptor, a local color table maybe, the code size, the data.
            0x2C => {
                p += 10 + table(*b.get(p + 9)?) + 1;
                p = skip(p)?;
                delays.push(if delay <= 1 { 100 } else { delay * 10 });
                delay = 0;
            }
            0x3B => break,
            _ => return None,
        }
    }
    Some(delays)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_the_delay_of_each_frame() {
        let mut g = b"GIF89a".to_vec();
        g.extend([1, 0, 1, 0, 0x80, 0, 0, 0, 0, 0, 255, 255, 255]); // 1x1, a 2-color table
        for delay in [5u8, 0] {
            g.extend([0x21, 0xF9, 4, 0, delay, 0, 0, 0]); // graphic control
            g.extend([0x2C, 0, 0, 0, 0, 1, 0, 1, 0, 0]); // a frame, no local table
            g.extend([2, 2, 0x44, 0x01, 0]); // its code size and one data block
        }
        g.push(0x3B);
        assert_eq!(parse(&g), Some(vec![50, 100]));
        assert_eq!(parse(b"GIF89a"), None);
        assert_eq!(parse(b"PNG"), None);
    }
}
