//! Kiểm tra bước đầu chữ ký mã của nhà phát triển khi nộp gói (spec developer-portal, "Nộp gói
//! phiên bản"): gói Windows phải có chữ ký Authenticode nhúng trong tệp chạy, gói macOS phải được
//! ký (không phải ký ad-hoc) và kèm vé notarize.
//!
//! Đây là kiểm tra **cấu trúc**: tệp có mang chữ ký hay không. Việc xác minh mật mã (hash của
//! tệp khớp chữ ký, chuỗi chứng thư, dấu thời gian, thu hồi) thuộc khâu kiểm duyệt (nhóm 9), chạy
//! bằng công cụ của hệ điều hành trong máy ảo quét; ở đây không tự viết phần mật mã.

/// OID `signedData` (1.2.840.113549.1.7.2) trong PKCS#7.
const SIGNED_DATA_OID: [u8; 11] = [
    0x06, 0x09, 0x2a, 0x86, 0x48, 0x86, 0xf7, 0x0d, 0x01, 0x07, 0x02,
];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Signature {
    Signed,
    Unsigned,
    /// Không phải định dạng tệp chạy mong đợi.
    NotExecutable,
}

fn u16le(b: &[u8], at: usize) -> Option<u16> {
    Some(u16::from_le_bytes(b.get(at..at + 2)?.try_into().ok()?))
}

fn u32le(b: &[u8], at: usize) -> Option<u32> {
    Some(u32::from_le_bytes(b.get(at..at + 4)?.try_into().ok()?))
}

fn u32be(b: &[u8], at: usize) -> Option<u32> {
    Some(u32::from_be_bytes(b.get(at..at + 4)?.try_into().ok()?))
}

/// Tệp PE (`.exe`) có chữ ký Authenticode nhúng: thư mục Security trỏ tới một `WIN_CERTIFICATE`
/// kiểu PKCS#7 `SignedData`.
pub fn authenticode(pe: &[u8]) -> Signature {
    let parse = || -> Option<Signature> {
        if pe.get(..2)? != b"MZ" {
            return None;
        }
        let nt = u32le(pe, 0x3c)? as usize;
        if pe.get(nt..nt + 4)? != b"PE\0\0" {
            return None;
        }
        let optional = nt + 24;
        // PE32: thư mục dữ liệu ở +96; PE32+: +112. Mục thứ 4 là Security.
        let directories = match u16le(pe, optional)? {
            0x10b => optional + 96,
            0x20b => optional + 112,
            _ => return None,
        };
        let count = u32le(pe, directories - 4)?;
        if count <= 4 {
            return Some(Signature::Unsigned);
        }
        let offset = u32le(pe, directories + 4 * 8)? as usize;
        let size = u32le(pe, directories + 4 * 8 + 4)? as usize;
        if offset == 0 || size < 8 {
            return Some(Signature::Unsigned);
        }
        let table = pe.get(offset..offset.checked_add(size)?)?;
        let length = u32le(table, 0)? as usize;
        let kind = u16le(table, 6)?;
        let der = table.get(8..length.min(table.len()))?;
        // WIN_CERT_TYPE_PKCS_SIGNED_DATA, nội dung là SEQUENCE chứa OID signedData.
        let pkcs7 = kind == 0x0002
            && der.first() == Some(&0x30)
            && der
                .get(..32.min(der.len()))?
                .windows(SIGNED_DATA_OID.len())
                .any(|w| w == SIGNED_DATA_OID);
        Some(if pkcs7 {
            Signature::Signed
        } else {
            Signature::Unsigned
        })
    };
    parse().unwrap_or(Signature::NotExecutable)
}

/// Tệp Mach-O có chữ ký mã với khối CMS (ký bằng chứng thư, không phải ad-hoc).
pub fn macho(file: &[u8]) -> Signature {
    const MH_MAGIC_64: u32 = 0xfeed_facf;
    const FAT_MAGIC: u32 = 0xcafe_babe;
    const LC_CODE_SIGNATURE: u32 = 0x1d;
    const SUPERBLOB: u32 = 0xfade_0cc0;
    const CMS_SLOT: u32 = 0x1_0000;

    fn thin(file: &[u8]) -> Option<Signature> {
        if u32le(file, 0)? != MH_MAGIC_64 {
            return None;
        }
        let commands = u32le(file, 16)?;
        let mut at = 32;
        for _ in 0..commands {
            let cmd = u32le(file, at)?;
            let size = u32le(file, at + 4)? as usize;
            if cmd == LC_CODE_SIGNATURE {
                let offset = u32le(file, at + 8)? as usize;
                let blob = file.get(offset..)?;
                if u32be(blob, 0)? != SUPERBLOB {
                    return Some(Signature::Unsigned);
                }
                let count = u32be(blob, 8)? as usize;
                for i in 0..count {
                    let slot = u32be(blob, 12 + i * 8)?;
                    let start = u32be(blob, 16 + i * 8)? as usize;
                    // Khối CMS dài hơn 8 byte đầu nghĩa là có chữ ký bằng chứng thư.
                    if slot == CMS_SLOT && u32be(blob, start + 4)? > 8 {
                        return Some(Signature::Signed);
                    }
                }
                return Some(Signature::Unsigned);
            }
            if size < 8 {
                return None;
            }
            at += size;
        }
        Some(Signature::Unsigned)
    }

    if u32be(file, 0) == Some(FAT_MAGIC) {
        // Tệp universal: mọi kiến trúc đều phải được ký.
        let count = u32be(file, 4).unwrap_or(0) as usize;
        let mut result = Signature::NotExecutable;
        for i in 0..count.min(16) {
            let Some(offset) = u32be(file, 8 + i * 20 + 8) else {
                return Signature::NotExecutable;
            };
            match file.get(offset as usize..).and_then(thin) {
                Some(Signature::Signed) => result = Signature::Signed,
                Some(other) => return other,
                None => return Signature::NotExecutable,
            }
        }
        return result;
    }
    thin(file).unwrap_or(Signature::NotExecutable)
}

/// Dựng tệp thử cho test (dùng ở cả test đơn vị lẫn E2E).
pub mod fixtures {
    use super::SIGNED_DATA_OID;

    /// Tệp PE32+ x64 tối thiểu; `signed` thì kèm bảng chứng thư Authenticode.
    pub fn pe(signed: bool) -> Vec<u8> {
        pe_for(0x8664, signed, &[])
    }

    /// Tệp PE32+ với kiến trúc `machine` (0x8664 x64, 0xaa64 arm64) và tối đa hai section.
    pub fn pe_for(machine: u16, signed: bool, sections: &[&str]) -> Vec<u8> {
        let mut f = vec![0u8; 0x200];
        f[..2].copy_from_slice(b"MZ");
        f[0x3c..0x40].copy_from_slice(&0x80u32.to_le_bytes());
        f[0x80..0x84].copy_from_slice(b"PE\0\0");
        f[0x84..0x86].copy_from_slice(&machine.to_le_bytes());
        let count = sections.len().min(2) as u16;
        f[0x86..0x88].copy_from_slice(&count.to_le_bytes());
        f[0x94..0x96].copy_from_slice(&240u16.to_le_bytes());
        let optional = 0x80 + 24;
        f[optional..optional + 2].copy_from_slice(&0x20bu16.to_le_bytes());
        f[optional + 108..optional + 112].copy_from_slice(&16u32.to_le_bytes());
        for (i, name) in sections.iter().enumerate().take(2) {
            let at = optional + 240 + i * 40;
            let n = name.len().min(8);
            f[at..at + n].copy_from_slice(&name.as_bytes()[..n]);
        }
        if signed {
            let mut der = vec![0x30, 0x82, 0x01, 0x00];
            der.extend_from_slice(&SIGNED_DATA_OID);
            der.resize(64, 0);
            let length = (8 + der.len()) as u32;
            let security = optional + 112 + 4 * 8;
            f[security..security + 4].copy_from_slice(&0x200u32.to_le_bytes());
            f[security + 4..security + 8].copy_from_slice(&length.to_le_bytes());
            f.extend_from_slice(&length.to_le_bytes());
            f.extend_from_slice(&0x0200u16.to_le_bytes());
            f.extend_from_slice(&0x0002u16.to_le_bytes());
            f.extend_from_slice(&der);
        }
        f
    }

    /// Tệp Mach-O 64-bit tối thiểu; `cms` là độ dài khối CMS (0: không có chữ ký, 8: ad-hoc).
    pub fn macho(cms: Option<u32>) -> Vec<u8> {
        let mut f = vec![0u8; 32];
        f[..4].copy_from_slice(&0xfeed_facfu32.to_le_bytes());
        // CPU_TYPE_ARM64.
        f[4..8].copy_from_slice(&0x0100_000cu32.to_le_bytes());
        let Some(cms) = cms else {
            return f;
        };
        f[16..20].copy_from_slice(&1u32.to_le_bytes());
        f.extend_from_slice(&0x1du32.to_le_bytes());
        f.extend_from_slice(&16u32.to_le_bytes());
        f.extend_from_slice(&48u32.to_le_bytes());
        f.extend_from_slice(&64u32.to_le_bytes());
        // SuperBlob: magic, length, count = 1, (slot CMS, offset 20), rồi khối CMS.
        f.extend_from_slice(&0xfade_0cc0u32.to_be_bytes());
        f.extend_from_slice(&64u32.to_be_bytes());
        f.extend_from_slice(&1u32.to_be_bytes());
        f.extend_from_slice(&0x1_0000u32.to_be_bytes());
        f.extend_from_slice(&20u32.to_be_bytes());
        f.extend_from_slice(&0xfade_0b01u32.to_be_bytes());
        f.extend_from_slice(&cms.to_be_bytes());
        f.resize(48 + 64, 0);
        f
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pe_with_and_without_authenticode() {
        assert_eq!(authenticode(&fixtures::pe(true)), Signature::Signed);
        assert_eq!(authenticode(&fixtures::pe(false)), Signature::Unsigned);
        assert_eq!(
            authenticode(b"#!/bin/sh\necho hi"),
            Signature::NotExecutable
        );
        assert_eq!(authenticode(b"MZ"), Signature::NotExecutable);
        // Bảng chứng thư không phải PKCS#7 SignedData thì không tính là đã ký.
        let mut garbage = fixtures::pe(true);
        let n = garbage.len();
        garbage[n - 60..n - 49].fill(0xff);
        assert_eq!(authenticode(&garbage), Signature::Unsigned);
    }

    #[test]
    fn macho_signed_adhoc_and_unsigned() {
        assert_eq!(macho(&fixtures::macho(Some(4096))), Signature::Signed);
        assert_eq!(
            macho(&fixtures::macho(Some(8))),
            Signature::Unsigned,
            "ad-hoc"
        );
        assert_eq!(macho(&fixtures::macho(None)), Signature::Unsigned);
        assert_eq!(macho(b"MZ not macho"), Signature::NotExecutable);
    }

    /// Với tệp thật trên Windows: git.exe có chữ ký nhúng, cargo.exe thì không.
    #[cfg(windows)]
    #[test]
    fn real_windows_binaries() {
        let git = std::path::Path::new("C:\\Program Files\\Git\\cmd\\git.exe");
        if let Ok(bytes) = std::fs::read(git) {
            assert_eq!(authenticode(&bytes), Signature::Signed, "git.exe");
        }
        let test_exe = std::fs::read(std::env::current_exe().unwrap()).unwrap();
        assert_eq!(
            authenticode(&test_exe),
            Signature::Unsigned,
            "tệp test tự build"
        );
    }
}
