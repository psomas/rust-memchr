/*!
This crate defines two functions, `memchr` and `memrchr`, which expose a safe interface
to the corresponding functions in `libc`.
*/

#![deny(missing_docs)]
#![allow(unused_imports)]

extern crate libc;

use libc::c_void;
use libc::{c_int, size_t};

/// A safe interface to `memchr`.
///
/// Returns the index corresponding to the first occurrence of `needle` in
/// `haystack`, or `None` if one is not found.
///
/// memchr reduces to super-optimized machine code at around an order of
/// magnitude faster than `haystack.iter().position(|&b| b == needle)`.
/// (See benchmarks.)
///
/// # Example
///
/// This shows how to find the first position of a byte in a byte string.
///
/// ```rust
/// use memchr::memchr;
///
/// let haystack = b"the quick brown fox";
/// assert_eq!(memchr(b'k', haystack), Some(8));
/// ```
pub fn memchr(needle: u8, haystack: &[u8]) -> Option<usize> {
    // libc memchr
    #[cfg(any(not(target_os = "windows"),
              not(any(target_pointer_width = "32",
                      target_pointer_width = "64"))))]
    fn memchr_specific(needle: u8, haystack: &[u8]) -> Option<usize> {
        use libc::memchr as libc_memchr;

        let p = unsafe {
            libc_memchr(
                haystack.as_ptr() as *const c_void,
                needle as c_int,
                haystack.len() as size_t)
        };
        if p.is_null() {
            None
        } else {
            Some(p as usize - (haystack.as_ptr() as usize))
        }
    }

    // use fallback on windows, since it's faster
    #[cfg(all(target_os = "windows",
              any(target_pointer_width = "32",
                  target_pointer_width = "64")))]
    fn memchr_specific(needle: u8, haystack: &[u8]) -> Option<usize> {
        fallback::memchr(needle, haystack)
    }

    memchr_specific(needle, haystack)
}

/// A safe interface to `memrchr`.
///
/// Returns the index corresponding to the last occurrence of `needle` in
/// `haystack`, or `None` if one is not found.
///
/// # Example
///
/// This shows how to find the last position of a byte in a byte string.
///
/// ```rust
/// use memchr::memrchr;
///
/// let haystack = b"the quick brown fox";
/// assert_eq!(memrchr(b'o', haystack), Some(17));
/// ```
pub fn memrchr(needle: u8, haystack: &[u8]) -> Option<usize> {

    #[cfg(target_os = "linux")]
    fn memrchr_specific(needle: u8, haystack: &[u8]) -> Option<usize> {
        // GNU's memrchr() will - unlike memchr() - error if haystack is empty.
        if haystack.is_empty() {return None}
        let p = unsafe {
            ffi::memrchr(
                haystack.as_ptr() as *const c_void,
                needle as c_int,
                haystack.len() as size_t)
        };
        if p.is_null() {
            None
        } else {
            Some(p as usize - (haystack.as_ptr() as usize))
        }
    }

    #[cfg(all(not(target_os = "linux"),
              any(target_pointer_width = "32", target_pointer_width = "64")))]
    fn memrchr_specific(needle: u8, haystack: &[u8]) -> Option<usize> {
        fallback::memrchr(needle, haystack)
    }

    // For the rare case of neither 32 bit nor 64-bit platform.
    #[cfg(all(not(target_os = "linux"),
              not(target_pointer_width = "32"),
              not(target_pointer_width = "64")))]
    fn memrchr_specific(needle: u8, haystack: &[u8]) -> Option<usize> {
        haystack.iter().rposition(|&b| b == needle)
    }

    memrchr_specific(needle, haystack)
}

#[allow(dead_code)]
#[cfg(all(not(target_os = "linux"),
          any(target_pointer_width = "32", target_pointer_width = "64")))]
mod fallback {
    use std::cmp;

    const LO_U64: u64 = 0x0101010101010101;
    const HI_U64: u64 = 0x8080808080808080;

    // use truncation
    const LO_USIZE: usize = LO_U64 as usize;
    const HI_USIZE: usize = HI_U64 as usize;

    #[cfg(target_pointer_width = "32")]
    const USIZE_BYTES: usize = 4;
    #[cfg(target_pointer_width = "64")]
    const USIZE_BYTES: usize = 8;

    /// Return `true` if `x` contains any zero byte.
    ///
    /// From *Matters Computational*, J. Arndt
    ///
    /// "The idea is to subtract one from each of the bytes and then look for
    /// bytes where the borrow propagated all the way to the most significant
    /// bit."
    #[inline]
    fn contains_zero_byte(x: usize) -> bool {
        x.wrapping_sub(LO_USIZE) & !x & HI_USIZE != 0
    }

    #[cfg(target_pointer_width = "32")]
    #[inline]
    fn repeat_byte(b: u8) -> usize {
        let mut rep = (b as usize) << 8 | b as usize;
        rep = rep << 16 | rep;
        rep
    }

    #[cfg(target_pointer_width = "64")]
    #[inline]
    fn repeat_byte(b: u8) -> usize {
        let mut rep = (b as usize) << 8 | b as usize;
        rep = rep << 16 | rep;
        rep = rep << 32 | rep;
        rep
    }

    /// Return the first index matching the byte `a` in `text`.
    pub fn memchr(x: u8, text: &[u8]) -> Option<usize> {
        // Scan for a single byte value by reading two `usize` words at a time.
        //
        // Split `text` in three parts
        // - unaligned inital part, before the first word aligned address in text
        // - body, scan by 2 words at a time
        // - the last remaining part, < 2 word size
        let len = text.len();
        let ptr = text.as_ptr();

        // search up to an aligned boundary
        let align = (ptr as usize) & (USIZE_BYTES - 1);
        let mut offset;
        if align > 0 {
            offset = cmp::min(USIZE_BYTES - align, len);
            if let Some(index) = text[..offset].iter().position(|elt| *elt == x) {
                return Some(index);
            }
        } else {
            offset = 0;
        }

        // search the body of the text
        let repeated_x = repeat_byte(x);

        if len >= 2 * USIZE_BYTES {
            while offset <= len - 2 * USIZE_BYTES {
                unsafe {
                    let u = *(ptr.offset(offset as isize) as *const usize);
                    let v = *(ptr.offset((offset + USIZE_BYTES) as isize) as *const usize);

                    // break if there is a matching byte
                    let zu = contains_zero_byte(u ^ repeated_x);
                    let zv = contains_zero_byte(v ^ repeated_x);
                    if zu || zv {
                        break;
                    }
                }
                offset += USIZE_BYTES * 2;
            }
        }

        // find the byte after the point the body loop stopped
        text[offset..].iter().position(|elt| *elt == x).map(|i| offset + i)
    }

    /// Return the last index matching the byte `a` in `text`.
    pub fn memrchr(x: u8, text: &[u8]) -> Option<usize> {
        // Scan for a single byte value by reading two `usize` words at a time.
        //
        // Split `text` in three parts
        // - unaligned tail, after the last word aligned address in text
        // - body, scan by 2 words at a time
        // - the first remaining bytes, < 2 word size
        let len = text.len();
        let ptr = text.as_ptr();

        // search to an aligned boundary
        let end_align = (ptr as usize + len) & (USIZE_BYTES - 1);
        let mut offset;
        if end_align > 0 {
            offset = len - cmp::min(USIZE_BYTES - end_align, len);
            if let Some(index) = text[offset..].iter().rposition(|elt| *elt == x) {
                return Some(offset + index);
            }
        } else {
            offset = len;
        }

        // search the body of the text
        let repeated_x = repeat_byte(x);

        while offset >= 2 * USIZE_BYTES {
            unsafe {
                let u = *(ptr.offset(offset as isize - 2 * USIZE_BYTES as isize) as *const usize);
                let v = *(ptr.offset(offset as isize - USIZE_BYTES as isize) as *const usize);

                // break if there is a matching byte
                let zu = contains_zero_byte(u ^ repeated_x);
                let zv = contains_zero_byte(v ^ repeated_x);
                if zu || zv {
                    break;
                }
            }
            offset -= 2 * USIZE_BYTES;
        }

        // find the byte before the point the body loop stopped
        text[..offset].iter().rposition(|elt| *elt == x)
    }
}

#[cfg(target_os = "linux")]
mod ffi {
    use libc::c_void;
    use libc::{c_int, size_t};
    extern {
        pub fn memrchr(cx: *const c_void, c: c_int, n: size_t) -> *mut c_void;
    }
}

#[cfg(test)]
mod tests {
    extern crate quickcheck;

    use super::{memchr, memrchr};

    #[test]
    fn matches_one() {
        assert_eq!(Some(0), memchr(b'a', b"a"));
    }

    #[test]
    fn matches_begin() {
        assert_eq!(Some(0), memchr(b'a', b"aaaa"));
    }

    #[test]
    fn matches_end() {
        assert_eq!(Some(4), memchr(b'z', b"aaaaz"));
    }

    #[test]
    fn matches_nul() {
        assert_eq!(Some(4), memchr(b'\x00', b"aaaa\x00"));
    }

    #[test]
    fn matches_past_nul() {
        assert_eq!(Some(5), memchr(b'z', b"aaaa\x00z"));
    }

    #[test]
    fn no_match_empty() {
        assert_eq!(None, memchr(b'a', b""));
    }

    #[test]
    fn no_match() {
        assert_eq!(None, memchr(b'a', b"xyz"));
    }

    #[test]
    fn qc_never_fail() {
        fn prop(needle: u8, haystack: Vec<u8>) -> bool {
            memchr(needle, &haystack); true
        }
        quickcheck::quickcheck(prop as fn(u8, Vec<u8>) -> bool);
    }

    #[test]
    fn matches_one_reversed() {
        assert_eq!(Some(0), memrchr(b'a', b"a"));
    }

    #[test]
    fn matches_begin_reversed() {
        assert_eq!(Some(3), memrchr(b'a', b"aaaa"));
    }

    #[test]
    fn matches_end_reversed() {
        assert_eq!(Some(0), memrchr(b'z', b"zaaaa"));
    }

    #[test]
    fn matches_nul_reversed() {
        assert_eq!(Some(4), memrchr(b'\x00', b"aaaa\x00"));
    }

    #[test]
    fn matches_past_nul_reversed() {
        assert_eq!(Some(0), memrchr(b'z', b"z\x00aaaa"));
    }

    #[test]
    fn no_match_empty_reversed() {
        assert_eq!(None, memrchr(b'a', b""));
    }

    #[test]
    fn no_match_reversed() {
        assert_eq!(None, memrchr(b'a', b"xyz"));
    }

    #[test]
    fn qc_never_fail_reversed() {
        fn prop(needle: u8, haystack: Vec<u8>) -> bool {
            memrchr(needle, &haystack); true
        }
        quickcheck::quickcheck(prop as fn(u8, Vec<u8>) -> bool);
    }

    #[test]
    fn qc_correct_memchr() {
        fn prop(v: Vec<u8>, offset: u8) -> bool {
            // test all pointer alignments
            let uoffset = (offset & 0xF) as usize;
            let data = if uoffset <= v.len() {
                &v[uoffset..]
            } else {
                &v[..]
            };
            for byte in 0..256u32 {
                let byte = byte as u8;
                if memchr(byte, &data) != data.iter().position(|elt| *elt == byte) {
                    return false;
                }
            }
            true
        }
        quickcheck::quickcheck(prop as fn(Vec<u8>, u8) -> bool);
    }

    #[test]
    fn qc_correct_memrchr() {
        fn prop(v: Vec<u8>, offset: u8) -> bool {
            // test all pointer alignments
            let uoffset = (offset & 0xF) as usize;
            let data = if uoffset <= v.len() {
                &v[uoffset..]
            } else {
                &v[..]
            };
            for byte in 0..256u32 {
                let byte = byte as u8;
                if memrchr(byte, &data) != data.iter().rposition(|elt| *elt == byte) {
                    return false;
                }
            }
            true
        }
        quickcheck::quickcheck(prop as fn(Vec<u8>, u8) -> bool);
    }
}
