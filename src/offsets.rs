use once_cell::sync::OnceCell;

#[macro_export]
macro_rules! ensure_hooks {
    ($($f:expr),*) => {{
        let mut is_successful = true;
        $(
            if $f.get_offset_in_memory().is_none() && is_successful {
                skyline::error::show_error(
                    420,
                    "Less Delay failed to load.\0",
                    format!("Error: Failed to find {} in memory.\n\n{}\n\n{}\n\n{}\0",
                    $f.location_name,
                    "This may be the result of an incompatible mod being loaded, or SSBU being updated.",
                    "If you are unsure, head over to the issues page at\nhttps://github.com/austintraver/less-delay-alt/issues",
                    "Less Delay will NOT be enabled now, however you can continue playing normally."
                    ).as_str()
                );

                is_successful = false;
            }
        )*

        is_successful
    }};
}

/// A byte that must match exactly.
const fn b(value: u8) -> Option<u8> {
    Some(value)
}

/// A wildcard byte: the operand of a PC-relative instruction (`BL`, `ADRP`, `CBZ`, ...),
/// which every recompile of the game may shift even when the surrounding code is unchanged.
const W: Option<u8> = None;

///
/// Searches for a masked byte pattern in the text region of the process memory.
/// Returns the start offset of the match only when the pattern occurs exactly once;
/// zero matches means the game changed, and several matches means the pattern is
/// not distinctive enough to trust.
///
fn byte_search(needle: &[Option<u8>]) -> Option<usize> {
    let search_space = unsafe {
        let start = skyline::hooks::getRegionAddress(skyline::hooks::Region::Text) as *const u8;
        let end = skyline::hooks::getRegionAddress(skyline::hooks::Region::Rodata) as *const u8;
        let length = end.offset_from(start) as usize;

        std::slice::from_raw_parts(start, length)
    };

    let mut matches = search_space
        .windows(needle.len())
        .enumerate()
        .filter(|(_, window)| {
            window
                .iter()
                .zip(needle)
                .all(|(byte, expected)| expected.is_none_or(|e| e == *byte))
        })
        .map(|(offset, _)| offset);

    let first = matches.next()?;
    if let Some(second) = matches.next() {
        println!("[less-delay] Pattern is ambiguous: matches at {first:#x} and {second:#x}");
        return None;
    }

    Some(first)
}

pub struct SSBUMemoryLocation<'a> {
    ///
    /// A memory signature used to search for the given location in memory.
    /// `None` entries are wildcards.
    ///
    signature: &'a [Option<u8>],

    ///
    /// Offset to the actual location (in bytes) that we are interested in hooking,
    /// relative to the start of the `signature`.
    /// If the first byte of `signature` is the exact location we want, this will be `0`.
    ///
    start_offset: isize,

    ///
    /// A human-readable function / location name, used to display an error message to
    /// the user if the location cannot be found for hooking.
    ///
    pub location_name: &'a str,

    cached_offset: OnceCell<Option<usize>>,
}

impl SSBUMemoryLocation<'_> {
    pub fn get_offset_in_memory(&self) -> Option<usize> {
        *self.cached_offset.get_or_init(|| unsafe {
            let r = Some(
                ((byte_search(self.signature)? as *const u8).offset(self.start_offset)) as usize,
            );

            if let Some(r) = r {
                println!("[less-delay] Found {} at {r:#09x?}", self.location_name);
            }

            r
        })
    }
}

// Both signatures keep every position-independent instruction byte-exact and wildcard the
// three operand bytes of each PC-relative instruction, leaving its opcode byte in place.
// Between 13.0.3 and 13.0.4 the only bytes that moved were ADRP operands (upstream commit
// 9ccb428); the BL displacements and the vsync loop were byte-identical.

pub static LOC_VSYNC_COUNT_THREAD: SSBUMemoryLocation = SSBUMemoryLocation {
    signature: &[
        b(0xa8), b(0xfe), b(0x5f), b(0xc8), // ldaxr
        b(0x08), b(0x05), b(0x00), b(0x91), // add
        b(0xa8), b(0xfe), b(0x09), b(0xc8), // stlxr
        W, W, W, b(0x35),                   // cbnz
        b(0x88), b(0xc2), b(0x4e), b(0x39), // ldrb
        W, W, W, b(0x34),                   // cbz
        b(0xfd), b(0x7b), b(0x42), b(0xa9), // ldp
        b(0xf4), b(0x4f), b(0x41), b(0xa9), // ldp
        b(0xf5), b(0x07), b(0x43), b(0xf8), // ldr
        b(0xc0), b(0x03), b(0x5f), b(0xd6), // ret
    ],
    start_offset: 0,
    location_name: "vsync_count_thread",
    cached_offset: OnceCell::new(),
};

pub static LOC_RUN_SCENE_UPDATE: SSBUMemoryLocation = SSBUMemoryLocation {
    signature: &[
        W, W, W, b(0x94),                   // bl
        W, W, W, b(0xf0),                   // adrp
        b(0x08), b(0x75), b(0x40), b(0xf9), // ldr
        b(0x00), b(0x00), b(0x08), b(0xcb), // sub
        W, W, W, b(0x94),                   // bl
        b(0xe8), b(0x37), b(0x41), b(0xf9), // ldr
        b(0xff), b(0x3b), b(0x01), b(0xf9), // str
        b(0x08), b(0x05), b(0x40), b(0xf9), // ldr
        b(0x01), b(0x01), b(0x40), b(0xf9), // ldr
        b(0xe0), b(0x0b), b(0x40), b(0x91), // add
        b(0x00), b(0xc0), b(0x0f), b(0x91), // add
        b(0xe2), b(0xc3), b(0x09), b(0x91), // add
        W, W, W, b(0x94),                   // bl
        W, W, W, b(0xb0),                   // adrp  <- hook target (start_offset 52)
        b(0x08), b(0x69), b(0x43), b(0xf9), // ldr
        b(0x08), b(0x05), b(0x40), b(0xf9), // ldr
        b(0x00), b(0x01), b(0x40), b(0xf9), // ldr
        W, W, W, b(0xb4),                   // cbz
        W, W, W, b(0x94),                   // bl
    ],
    start_offset: 52,
    location_name: "run_scene_update",
    cached_offset: OnceCell::new(),
};
