//! The ELF reader of the size check: section sizes and symbol names of a
//! linked image.

/// What one image contains.
pub(super) struct Image {
    /// Code and constant data: allocated sections with contents.
    pub(super) flash: u64,
    /// Static memory: writable allocated sections, with or without
    /// contents.
    pub(super) ram: u64,
    /// Every symbol name.
    pub(super) symbols: Vec<String>,
}

/// Reads the section sizes and symbol names of a little-endian ELF image,
/// 32 or 64-bit (the ELF specification's section header and symbol table
/// layouts).
pub(super) fn read_elf(bytes: &[u8]) -> Option<Image> {
    const MAGIC: &[u8] = b"\x7fELF";
    const CLASS_32: u8 = 1;
    const CLASS_64: u8 = 2;
    const LITTLE_ENDIAN: u8 = 1;
    const SHT_SYMTAB: u32 = 2;
    const SHT_NOBITS: u32 = 8;
    const SHF_WRITE: u64 = 0x1;
    const SHF_ALLOC: u64 = 0x2;
    if bytes.get(..4)? != MAGIC || *bytes.get(5)? != LITTLE_ENDIAN {
        return None;
    }
    let wide = match *bytes.get(4)? {
        CLASS_32 => false,
        CLASS_64 => true,
        _ => return None,
    };
    let read = |offset: usize, size: usize| -> Option<u64> {
        let field = bytes.get(offset..offset.checked_add(size)?)?;
        Some(field.iter().rev().fold(0u64, |value, &byte| (value << 8) | u64::from(byte)))
    };
    let word = if wide { 8 } else { 4 };
    // e_shoff, e_shentsize and e_shnum.
    let (shoff, shentsize, shnum) = if wide {
        (read(0x28, 8)?, read(0x3A, 2)?, read(0x3C, 2)?)
    } else {
        (read(0x20, 4)?, read(0x2E, 2)?, read(0x30, 2)?)
    };
    let header = |index: u64| usize::try_from(shoff + index * shentsize).ok();
    // Fields of a section header: sh_type, sh_flags, sh_offset, sh_size,
    // sh_link, sh_entsize.
    let section = |index: u64| -> Option<(u32, u64, u64, u64, u64, u64)> {
        let base = header(index)?;
        let kind = u32::try_from(read(base + 4, 4)?).ok()?;
        let flags = read(base + 8, word)?;
        let offset = read(base + 8 + 2 * word, word)?;
        let size = read(base + 8 + 3 * word, word)?;
        let link = read(base + 8 + 4 * word, 4)?;
        let entsize = read(base + 16 + 5 * word, word)?;
        Some((kind, flags, offset, size, link, entsize))
    };
    let mut image = Image { flash: 0, ram: 0, symbols: Vec::new() };
    for index in 0..shnum {
        let (kind, flags, offset, size, link, entsize) = section(index)?;
        if flags & SHF_ALLOC != 0 {
            if kind != SHT_NOBITS {
                image.flash += size;
            }
            if flags & SHF_WRITE != 0 {
                image.ram += size;
            }
        }
        if kind == SHT_SYMTAB && entsize > 0 {
            let (_, _, strings, _, _, _) = section(link)?;
            for entry in 0..size / entsize {
                // st_name is the first word of a symbol in both classes.
                let name = usize::try_from(read(usize::try_from(offset + entry * entsize).ok()?, 4)?).ok()?;
                let start = usize::try_from(strings).ok()?.checked_add(name)?;
                let text = bytes.get(start..)?;
                let end = text.iter().position(|&byte| byte == 0)?;
                if end > 0 {
                    image.symbols.push(String::from_utf8_lossy(text.get(..end)?).into_owned());
                }
            }
        }
    }
    Some(image)
}
