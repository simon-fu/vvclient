
use std::{fmt, io::{self, Write as _}};
use serde::Serialize;

const BUF_SIZE: usize = 4096;

struct FmtWriter<'a, 'b> {
    f: &'a mut fmt::Formatter<'b>,
    buf: [u8; BUF_SIZE],
    len: usize,
}

impl<'a, 'b> FmtWriter<'a, 'b> {
    fn new(f: &'a mut fmt::Formatter<'b>) -> Self {
        Self {
            f,
            buf: [0; BUF_SIZE],
            len: 0,
        }
    }

    // 将 buf 中能组成完整 UTF-8 的前缀写入 Formatter
    fn flush_buf(&mut self) -> io::Result<()> {
        let mut start = 0;
        while start < self.len {
            match std::str::from_utf8(&self.buf[start..self.len]) {
                Ok(s) => {
                    // 全部是有效 UTF-8
                    self.f.write_str(s).map_err(|_| io::Error::new(io::ErrorKind::Other, "fmt error"))?;
                    start = self.len;
                }
                Err(e) => {
                    let valid = e.valid_up_to();
                    if valid == 0 {
                        // 还没有完整的 UTF-8 字符，保留缓冲区等下一次写入
                        break;
                    }
                    let s = unsafe { std::str::from_utf8_unchecked(&self.buf[start..start + valid]) };
                    self.f.write_str(s).map_err(|_| io::Error::new(io::ErrorKind::Other, "fmt error"))?;
                    start += valid;
                }
            }
        }

        if start > 0 {
            // 将剩余未写出的字节移到 buf 开头
            let rem = self.len - start;
            if rem > 0 {
                self.buf.copy_within(start..self.len, 0);
            }
            self.len = rem;
        }
        Ok(())
    }
}

impl io::Write for FmtWriter<'_, '_> {
    fn write(&mut self, mut bytes: &[u8]) -> io::Result<usize> {
        let mut wrote = 0;
        while !bytes.is_empty() {
            let space = BUF_SIZE - self.len;
            if space == 0 {
                self.flush_buf()?;
            }
            let to_copy = space.min(bytes.len());
            self.buf[self.len..self.len + to_copy].copy_from_slice(&bytes[..to_copy]);
            self.len += to_copy;
            wrote += to_copy;
            bytes = &bytes[to_copy..];
            // 可选：当 buf 已经半满时尝试 flush（避免过多拷贝）
            if self.len >= BUF_SIZE / 2 {
                self.flush_buf()?;
            }
        }
        Ok(wrote)
    }

    fn flush(&mut self) -> io::Result<()> {
        self.flush_buf()
    }
}

/// 使用示例：给任意 Serialize 类型实现 Display（newtype）
pub struct JsonDisplay<T>(pub T);

impl<T> fmt::Display for JsonDisplay<T>
where
    T: Serialize,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut writer = FmtWriter::new(f);
        serde_json::to_writer(&mut writer, &self.0).map_err(|_| fmt::Error)?;
        writer.flush().map_err(|_| fmt::Error)?;
        Ok(())
    }
}

impl<T> serde::Serialize for JsonDisplay<T> 
where 
    T: Serialize,
{
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer 
    {
        serializer.collect_str(self)
    }
}


