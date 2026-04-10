pub mod console;
#[cfg(feature = "http")]
pub mod http;
pub mod io;
pub mod math;
pub mod number;

pub mod json {
    pub fn escape_string(s: &str) -> String {
        let mut out = String::with_capacity(s.len() + 2);
        out.push('"');
        for ch in s.chars() {
            match ch {
                '\\' => out.push_str("\\\\"),
                '"' => out.push_str("\\\""),
                '\n' => out.push_str("\\n"),
                '\r' => out.push_str("\\r"),
                '\t' => out.push_str("\\t"),
                c => out.push(c),
            }
        }
        out.push('"');
        out
    }

    pub fn parse_number(s: &str) -> f64 {
        serde_json::from_str::<f64>(s.trim()).expect("JSON.parse: expected JSON number")
    }
}

pub mod uri {
    pub fn encode_component(s: &str) -> String {
        urlencoding::encode(s).into_owned()
    }

    pub fn decode_component(s: &str) -> String {
        urlencoding::decode(s)
            .expect("decodeURIComponent: invalid percent-encoding")
            .into_owned()
    }
}

pub mod string {
    /// UTF-16 码元个数，与 `string.length` / `std.string.length` 一致。
    pub fn utf16_len(s: &str) -> f64 {
        s.encode_utf16().count() as f64
    }

    pub fn utf16_slice(s: &str, start: i32, end: Option<i32>, substring_swap: bool) -> String {
        let v: Vec<u16> = s.encode_utf16().collect();
        let nlen = v.len() as i32;
        let fix = |i: i32| -> i32 {
            if i < 0 {
                nlen + i
            } else {
                i
            }
        };
        let mut a = fix(start).clamp(0, nlen);
        let mut b = fix(end.unwrap_or(nlen)).clamp(0, nlen);
        if substring_swap && a > b {
            std::mem::swap(&mut a, &mut b);
        }
        if a > b {
            return String::new();
        }
        String::from_utf16_lossy(&v[a as usize..b as usize])
    }

    pub fn utf16_index_of(haystack: &str, needle: &str, from_utf16: i32) -> f64 {
        let h: Vec<u16> = haystack.encode_utf16().collect();
        let n: Vec<u16> = needle.encode_utf16().collect();
        let hl = h.len() as i32;
        let nl = n.len() as i32;
        let mut start = from_utf16;
        if from_utf16 < 0 {
            start = hl + from_utf16;
        }
        if start < 0 {
            start = 0;
        }
        let usize_start = (start as usize).min(h.len());
        if n.is_empty() {
            return ((usize_start as i32).min(hl)) as f64;
        }
        if nl > hl || usize_start > h.len().saturating_sub(n.len()) {
            return -1_f64;
        }
        let last = h.len() - n.len();
        for i in usize_start..=last {
            if h[i..i + n.len()] == n[..] {
                return (i as i32) as f64;
            }
        }
        -1_f64
    }
}
