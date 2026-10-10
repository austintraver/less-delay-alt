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

/// One byte of a signature: the game byte must equal `value` on the bits set in `mask`.
#[derive(Clone, Copy)]
pub struct Byte {
    value: u8,
    mask: u8,
}

/// A byte that must match exactly.
const fn b(value: u8) -> Byte {
    Byte { value, mask: 0xff }
}

/// A byte that must match only on the bits set in `mask`.
const fn m(value: u8, mask: u8) -> Byte {
    Byte { value: value & mask, mask }
}

/// A byte that is ignored entirely.
const W: Byte = Byte { value: 0, mask: 0 };

///
/// Searches for a masked byte pattern in the text region of the process memory.
/// Returns the start offset of the match only when the pattern occurs exactly once;
/// zero matches means the game changed, and several matches means the pattern is
/// not distinctive enough to trust.
///
fn byte_search(needle: &[Byte]) -> Option<usize> {
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
                .all(|(byte, expected)| byte & expected.mask == expected.value)
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
    ///
    signature: &'a [Byte],

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

// Every recompile of the game may move the PC-relative operands of BL, ADRP, CBZ and CBNZ even
// when the surrounding code is unchanged (between 13.0.3 and 13.0.4 only two ADRP operands moved,
// upstream commit 9ccb428). The masks below keep each such instruction's opcode and register bits
// and ignore every immediate bit, in little-endian byte order:
//   bl    imm26 = bits 0-25            -> bytes 0-2 ignored, byte 3 keeps bits 26-31 (mask 0xfc)
//   adrp  immlo = bits 29-30, immhi = bits 5-23, Rd = bits 0-4
//                                      -> byte 0 keeps Rd (0x1f), bytes 1-2 ignored, byte 3 keeps bits 24-28 and 31 (0x9f)
//   cbz/cbnz  imm19 = bits 5-23, Rt = bits 0-4
//                                      -> byte 0 keeps Rt (0x1f), bytes 1-2 ignored, byte 3 exact

const BL: [Byte; 4] = [W, W, W, m(0x94, 0xfc)];

pub static LOC_VSYNC_COUNT_THREAD: SSBUMemoryLocation = SSBUMemoryLocation {
    signature: &[
        b(0xa8), b(0xfe), b(0x5f), b(0xc8), // ldaxr x8, [x21]
        b(0x08), b(0x05), b(0x00), b(0x91), // add   x8, x8, #1
        b(0xa8), b(0xfe), b(0x09), b(0xc8), // stlxr w9, x8, [x21]
        m(0x09, 0x1f), W, W, b(0x35),       // cbnz  w9, <retry>
        b(0x88), b(0xc2), b(0x4e), b(0x39), // ldrb  w8, [x20, #0x3b0]
        m(0x08, 0x1f), W, W, b(0x34),       // cbz   w8, <loop>
        b(0xfd), b(0x7b), b(0x42), b(0xa9), // ldp   x29, x30, [sp, #0x20]
        b(0xf4), b(0x4f), b(0x41), b(0xa9), // ldp   x20, x19, [sp, #0x10]
        b(0xf5), b(0x07), b(0x43), b(0xf8), // ldr   x21, [sp], #0x30
        b(0xc0), b(0x03), b(0x5f), b(0xd6), // ret
    ],
    start_offset: 0,
    location_name: "vsync_count_thread",
    cached_offset: OnceCell::new(),
};

pub static LOC_RUN_SCENE_UPDATE: SSBUMemoryLocation = SSBUMemoryLocation {
    signature: &[
        BL[0], BL[1], BL[2], BL[3],               // bl    nn::os::GetSystemTick
        m(0x08, 0x1f), W, W, m(0xf0, 0x9f),       // adrp  x8, <start tick page>
        b(0x08), b(0x75), b(0x40), b(0xf9),       // ldr   x8, [x8, #0xe8]
        b(0x00), b(0x00), b(0x08), b(0xcb),       // sub   x0, x0, x8
        BL[0], BL[1], BL[2], BL[3],               // bl    nn::os::ConvertToTimeSpan
        b(0xe8), b(0x37), b(0x41), b(0xf9),       // ldr   x8, [sp, #0x268]
        b(0xff), b(0x3b), b(0x01), b(0xf9),       // str   xzr, [sp, #0x270]
        b(0x08), b(0x05), b(0x40), b(0xf9),       // ldr   x8, [x8, #8]
        b(0x01), b(0x01), b(0x40), b(0xf9),       // ldr   x1, [x8]
        b(0xe0), b(0x0b), b(0x40), b(0x91),       // add   x0, sp, #0x2, lsl #12
        b(0x00), b(0xc0), b(0x0f), b(0x91),       // add   x0, x0, #0x3f0
        b(0xe2), b(0xc3), b(0x09), b(0x91),       // add   x2, sp, #0x270
        BL[0], BL[1], BL[2], BL[3],               // bl    <present / sync step>
        m(0x08, 0x1f), W, W, m(0xb0, 0x9f),       // adrp  x8, <frame event page>   <- hook target (start_offset 52)
        b(0x08), b(0x69), b(0x43), b(0xf9),       // ldr   x8, [x8, #0x6d0]
        b(0x08), b(0x05), b(0x40), b(0xf9),       // ldr   x8, [x8, #8]
        b(0x00), b(0x01), b(0x40), b(0xf9),       // ldr   x0, [x8]
        m(0x00, 0x1f), W, W, b(0xb4),             // cbz   x0, <skip>
        BL[0], BL[1], BL[2], BL[3],               // bl    nn::os::SignalEvent
    ],
    start_offset: 52,
    location_name: "run_scene_update",
    cached_offset: OnceCell::new(),
};
