use std::mem;
use std::io::{self, Write};
use byteorder::{LittleEndian, WriteBytesExt};
use encoding_rs::SHIFT_JIS;

use crate::parser::*;

pub trait Writeable {
    fn byte_size(&self) -> usize;
    fn write<W: Write>(&self, writer: &mut W) -> Result<(), io::Error>;
}

impl Writeable for u8 {
    fn byte_size(&self) -> usize {
        mem::size_of::<u8>()
    }

    fn write<W: Write>(&self, writer: &mut W) -> Result<(), io::Error> {
        writer.write_u8(*self)
    }
}

impl Writeable for u32 {
    fn byte_size(&self) -> usize {
        mem::size_of::<u32>()
    }

    fn write<W: Write>(&self, writer: &mut W) -> Result<(), io::Error> {
        writer.write_u32::<LittleEndian>(*self)
    }
}

// Assumes SHIFT_JIS encoding
impl Writeable for &str {
    fn byte_size(&self) -> usize {
        let (bytes, _, errors) = SHIFT_JIS.encode(self);
        assert!(!errors, "Cannot encode as SHIFT_JIS");
        bytes.len() + 1 // Null byte
    }

    fn write<W: Write>(&self, writer: &mut W) -> Result<(), io::Error> {
        let (bytes, _, errors) = SHIFT_JIS.encode(self);
        if errors {
            return Err(io::Error::new(io::ErrorKind::Other, "Cannot encode as SHIFT_JIS"));
        }
        writer.write_all(&bytes)?;
        writer.write_all(&[0x00])
    }
}

// Assumes SHIFT_JIS encoding
impl Writeable for String {
    fn byte_size(&self) -> usize {
        let s: &str = &self;
        s.byte_size()
    }

    fn write<W: Write>(&self, writer: &mut W) -> Result<(), io::Error> {
        let s: &str = &self;
        s.write(writer)
    }
}

impl<T: Writeable> Writeable for Option<T> {
    fn byte_size(&self) -> usize {
        match self {
            Some(v) => v.byte_size(),
            None => 0
        }
    }

    fn write<W: Write>(&self, writer: &mut W) -> Result<(), io::Error> {
        match self {
            Some(v) => v.write(writer),
            None => Ok(())
        }
    }
}

impl<T: Writeable> Writeable for Vec<T> {
    fn byte_size(&self) -> usize {
        self.iter().map(|x| x.byte_size()).sum()
    }

    fn write<W: Write>(&self, writer: &mut W) -> Result<(), io::Error> {
        for v in self.iter() {
            v.write(writer)?;
        }
        Ok(())
    }
}

impl Writeable for Header {
    fn byte_size(&self) -> usize {
        b"TPC32".len()
            + self.unk1.byte_size()
            + mem::size_of::<u32>()
            + self.counter_start.byte_size()
            + self.labels.byte_size()
            + self.unk2.byte_size()
            + mem::size_of::<u32>()
            + self.menus.byte_size()
            + self.menu_strings.byte_size()
            + self.unk3.byte_size()
    }

    fn write<W: Write>(&self, writer: &mut W) -> Result<(), io::Error> {
        writer.write_all(b"TPC32")?;
        self.unk1.write(writer)?;
        (self.labels.len() as u32).write(writer)?;
        self.counter_start.write(writer)?;
        self.labels.write(writer)?;
        self.unk2.write(writer)?;
        (self.menus.len() as u32).write(writer)?;
        self.menus.write(writer)?;
        self.menu_strings.write(writer)?;
        self.unk3.write(writer)
    }
}

impl Writeable for Menu {
    fn byte_size(&self) -> usize {
        self.id.byte_size()
            + mem::size_of::<u8>()
            + self.unk1.byte_size()
            + self.unk2.byte_size()
            + self.submenus.byte_size()
    }

    fn write<W: Write>(&self, writer: &mut W) -> Result<(), io::Error> {
        self.id.write(writer)?;
        (self.submenus.len() as u8).write(writer)?;
        self.unk1.write(writer)?;
        self.unk2.write(writer)?;
        self.submenus.write(writer)
    }
}

impl Writeable for Submenu {
    fn byte_size(&self) -> usize {
        self.id.byte_size()
            + mem::size_of::<u8>()
            + self.unk1.byte_size()
            + self.unk2.byte_size()
            + self.flags.byte_size()
    }

    fn write<W: Write>(&self, writer: &mut W) -> Result<(), io::Error> {
        self.id.write(writer)?;
        (self.flags.len() as u8).write(writer)?;
        self.unk1.write(writer)?;
        self.unk2.write(writer)?;
        self.flags.write(writer)
    }
}

impl Writeable for Flag {
    fn byte_size(&self) -> usize {
        mem::size_of::<u8>()
            + self.unk1.byte_size()
            + self.flags.byte_size()
    }

    fn write<W: Write>(&self, writer: &mut W) -> Result<(), io::Error> {
        (self.flags.len() as u8).write(writer)?;
        self.unk1.write(writer)?;
        self.flags.write(writer)
    }
}

impl Writeable for Pos {
    fn byte_size(&self) -> usize {
        mem::size_of::<u32>()
    }

    fn write<W: Write>(&self, writer: &mut W) -> Result<(), io::Error> {
        if let Pos::Byte(pos) = *self {
            pos.write(writer)
        } else {
            return Err(io::Error::new(io::ErrorKind::Other, "Cannot write uncompiled label"));
        }
    }
}

impl Writeable for Val {
    fn byte_size(&self) -> usize {
        match self.0 {
            0x00..=0x0F => 0,
            0x10..=0xFFF => 1,
            0x1000..=0xFFFFF => 2,
            0x100000..=0xFFFFFFF => 3,
            0x10000000..=0xFFFFFFFF => 4
        }
    }

    fn write<W: Write>(&self, writer: &mut W) -> Result<(), io::Error> {
        let len = self.byte_size() as u8;
        let mut v = self.0;

        let mut len_byte = ((len + 1) << 4) | (v as u8) & 0x0F;

        if let ValType::Var = self.1 {
            len_byte |= 0x80;
        }

        v >>= 4;
        let mut bytes = vec![len_byte];

        for _ in (0..len).rev() {
            let byte = v as u8;
            bytes.push(byte);
            v >>= 8;
        }

        writer.write_all(&bytes)
    }
}

impl Writeable for SceneText {
    fn byte_size(&self) -> usize {
        match self {
            SceneText::Pointer(val) => 1 + val.byte_size(), // '@'
            SceneText::Literal(s) => s.byte_size()
        }
    }

    fn write<W: Write>(&self, writer: &mut W) -> Result<(), io::Error> {
        match self {
            SceneText::Pointer(val) => {
                (0x40u8).write(writer)?;
                val.write(writer)
            }
            SceneText::Literal(s) => s.write(writer)
        }
    }
}

impl Writeable for FormattedTextCmd {
    fn byte_size(&self) -> usize {
        match self {
            FormattedTextCmd::Integer(idx) => 1 + idx.byte_size(),
            FormattedTextCmd::IntegerZeroPadded(idx, zeros) => 1 + idx.byte_size() + zeros.byte_size(),
            FormattedTextCmd::TextPointer(idx) => 1 + idx.byte_size(),
            FormattedTextCmd::Unknown1(idx) => 1 + idx.byte_size(),
            FormattedTextCmd::Unknown2 => 1
        }
    }

    fn write<W: Write>(&self, writer: &mut W) -> Result<(), io::Error> {
        match self {
            FormattedTextCmd::Integer(idx) => {
                (0x01u8).write(writer)?;
                idx.write(writer)
            },
            FormattedTextCmd::IntegerZeroPadded(idx, zeros) => {
                (0x02u8).write(writer)?;
                idx.write(writer)?;
                zeros.write(writer)
            },
            FormattedTextCmd::TextPointer(idx) => {
                (0x03u8).write(writer)?;
                idx.write(writer)
            },
            FormattedTextCmd::Unknown1(idx) => {
                (0x11u8).write(writer)?;
                idx.write(writer)
            },
            FormattedTextCmd::Unknown2 => (0x13u8).write(writer)
        }
    }
}

impl Writeable for Ret {
    fn byte_size(&self) -> usize {
        match self {
            Ret::Color(idx) => 1 + idx.byte_size(),
            Ret::Choice => 1,
            Ret::DisabledChoice(idx) => 1 + idx.byte_size()
        }
    }

    fn write<W: Write>(&self, writer: &mut W) -> Result<(), io::Error> {
        match self {
            Ret::Color(idx) => {
                (0x20u8).write(writer)?;
                idx.write(writer)
            },
            Ret::Choice => (0x21u8).write(writer),
            Ret::DisabledChoice(idx) => {
                (0x22u8).write(writer)?;
                idx.write(writer)
            },
        }
    }
}

impl Writeable for Condition {
    fn byte_size(&self) -> usize {
        match self {
            Condition::IncDepth => 1,
            Condition::DecDepth => 1,
            Condition::And => 1,
            Condition::Or => 1,
            Condition::Ret(ret) => 1 + ret.byte_size(),
            Condition::BitNotEq(a, b) => 1 + a.byte_size() + b.byte_size(),
            Condition::BitEq(a, b) => 1 + a.byte_size() + b.byte_size(),
            Condition::NotEq(a, b) => 1 + a.byte_size() + b.byte_size(),
            Condition::Eq(a, b) => 1 + a.byte_size() + b.byte_size(),
            Condition::FlagNotEqConst(a, b) => 1 + a.byte_size() + b.byte_size(),
            Condition::FlagEqConst(a, b) => 1 + a.byte_size() + b.byte_size(),
            Condition::FlagAndConst(a, b) => 1 + a.byte_size() + b.byte_size(),
            Condition::FlagAndConst2(a, b) => 1 + a.byte_size() + b.byte_size(),
            Condition::FlagXorConst(a, b) => 1 + a.byte_size() + b.byte_size(),
            Condition::FlagGtConst(a, b) => 1 + a.byte_size() + b.byte_size(),
            Condition::FlagLtConst(a, b) => 1 + a.byte_size() + b.byte_size(),
            Condition::FlagGeqConst(a, b) => 1 + a.byte_size() + b.byte_size(),
            Condition::FlagLeqConst(a, b) => 1 + a.byte_size() + b.byte_size(),
            Condition::FlagNotEq(a, b) => 1 + a.byte_size() + b.byte_size(),
            Condition::FlagEq(a, b) => 1 + a.byte_size() + b.byte_size(),
            Condition::FlagAnd(a, b) => 1 + a.byte_size() + b.byte_size(),
            Condition::FlagAnd2(a, b) => 1 + a.byte_size() + b.byte_size(),
            Condition::FlagXor(a, b) => 1 + a.byte_size() + b.byte_size(),
            Condition::FlagGt(a, b) => 1 + a.byte_size() + b.byte_size(),
            Condition::FlagLt(a, b) => 1 + a.byte_size() + b.byte_size(),
            Condition::FlagGeq(a, b) => 1 + a.byte_size() + b.byte_size(),
            Condition::FlagLeq(a, b) => 1 + a.byte_size() + b.byte_size()
        }
    }

    fn write<W: Write>(&self, writer: &mut W) -> Result<(), io::Error> {
        match self {
            Condition::And => (0x26u8).write(writer),
            Condition::Or => (0x27u8).write(writer),
            Condition::IncDepth => (0x28u8).write(writer),
            Condition::DecDepth => (0x29u8).write(writer),
            Condition::BitNotEq(a, b) => {
                (0x36u8).write(writer)?;
                a.write(writer)?;
                b.write(writer)
            },
            Condition::BitEq(a, b) => {
                (0x37u8).write(writer)?;
                a.write(writer)?;
                b.write(writer)
            },
            Condition::NotEq(a, b) => {
                (0x38u8).write(writer)?;
                a.write(writer)?;
                b.write(writer)
            },
            Condition::Eq(a, b) => {
                (0x39u8).write(writer)?;
                a.write(writer)?;
                b.write(writer)
            },
            Condition::FlagNotEqConst(a, b) => {
                (0x3Au8).write(writer)?;
                a.write(writer)?;
                b.write(writer)
            },
            Condition::FlagEqConst(a, b) => {
                (0x3Bu8).write(writer)?;
                a.write(writer)?;
                b.write(writer)
            },
            Condition::FlagAndConst(a, b) => {
                (0x41u8).write(writer)?;
                a.write(writer)?;
                b.write(writer)
            },
            Condition::FlagAndConst2(a, b) => {
                (0x42u8).write(writer)?;
                a.write(writer)?;
                b.write(writer)
            },
            Condition::FlagXorConst(a, b) => {
                (0x43u8).write(writer)?;
                a.write(writer)?;
                b.write(writer)
            },
            Condition::FlagGtConst(a, b) => {
                (0x44u8).write(writer)?;
                a.write(writer)?;
                b.write(writer)
            },
            Condition::FlagLtConst(a, b) => {
                (0x45u8).write(writer)?;
                a.write(writer)?;
                b.write(writer)
            },
            Condition::FlagGeqConst(a, b) => {
                (0x46u8).write(writer)?;
                a.write(writer)?;
                b.write(writer)
            },
            Condition::FlagLeqConst(a, b) => {
                (0x47u8).write(writer)?;
                a.write(writer)?;
                b.write(writer)
            },
            Condition::FlagNotEq(a, b) => {
                (0x48u8).write(writer)?;
                a.write(writer)?;
                b.write(writer)
            },
            Condition::FlagEq(a, b) => {
                (0x49u8).write(writer)?;
                a.write(writer)?;
                b.write(writer)
            },
            Condition::FlagAnd(a, b) => {
                (0x4Fu8).write(writer)?;
                a.write(writer)?;
                b.write(writer)
            },
            Condition::FlagAnd2(a, b) => {
                (0x50u8).write(writer)?;
                a.write(writer)?;
                b.write(writer)
            },
            Condition::FlagXor(a, b) => {
                (0x51u8).write(writer)?;
                a.write(writer)?;
                b.write(writer)
            },
            Condition::FlagGt(a, b) => {
                (0x52u8).write(writer)?;
                a.write(writer)?;
                b.write(writer)
            },
            Condition::FlagLt(a, b) => {
                (0x53u8).write(writer)?;
                a.write(writer)?;
                b.write(writer)
            },
            Condition::FlagGeq(a, b) => {
                (0x54u8).write(writer)?;
                a.write(writer)?;
                b.write(writer)
            },
            Condition::FlagLeq(a, b) => {
                (0x55u8).write(writer)?;
                a.write(writer)?;
                b.write(writer)
            },
            Condition::Ret(ret) => {
                (0x58u8).write(writer)?;
                ret.write(writer)
            },
        }
    }
}

impl Writeable for SceneFormattedTextEntry {
    fn byte_size(&self) -> usize {
        match self {
            SceneFormattedTextEntry::Command(idx) => 1 + idx.byte_size(),
            SceneFormattedTextEntry::Unknown => 1,
            SceneFormattedTextEntry::Condition(conds) => conds.byte_size(),
            SceneFormattedTextEntry::TextPointer(idx) => 1 + idx.byte_size(),
            SceneFormattedTextEntry::TextHankaku(text) => 1 + text.byte_size(),
            SceneFormattedTextEntry::TextZenkaku(text) => 1 + text.byte_size(),
        }
    }

    fn write<W: Write>(&self, writer: &mut W) -> Result<(), io::Error> {
        match self {
            SceneFormattedTextEntry::Command(idx) => {
                (0x10u8).write(writer)?;
                idx.write(writer)
            },
            SceneFormattedTextEntry::Unknown => (0x12u8).write(writer),
            SceneFormattedTextEntry::Condition(conds) => {
                (0x28u8).write(writer)?;
                conds.write(writer)
            },
            SceneFormattedTextEntry::TextPointer(idx) => {
                (0xFDu8).write(writer)?;
                idx.write(writer)
            },
            SceneFormattedTextEntry::TextHankaku(text) => {
                (0xFEu8).write(writer)?;
                text.write(writer)
            },
            SceneFormattedTextEntry::TextZenkaku(text) => {
                (0xFFu8).write(writer)?;
                text.write(writer)
            },
        }
    }
}

impl Writeable for SceneFormattedText {
    fn byte_size(&self) -> usize {
        self.0.byte_size() + 1 // \0
    }

    fn write<W: Write>(&self, writer: &mut W) -> Result<(), io::Error> {
        self.0.write(writer)?;
        (0x00u8).write(writer)
    }
}

impl Writeable for JumpToSceneCmd {
    fn byte_size(&self) -> usize {
        match self {
            JumpToSceneCmd::Jump(idx) => 1 + idx.byte_size(),
            JumpToSceneCmd::Call(idx) => 1 + idx.byte_size(),
        }
    }

    fn write<W: Write>(&self, writer: &mut W) -> Result<(), io::Error> {
        match self {
            JumpToSceneCmd::Jump(idx) => {
                (0x01u8).write(writer)?;
                idx.write(writer)
            },
            JumpToSceneCmd::Call(idx) => {
                (0x02u8).write(writer)?;
                idx.write(writer)
            },
        }
    }
}

impl Writeable for TextWinCmd {
    fn byte_size(&self) -> usize {
        1
    }

    fn write<W: Write>(&self, writer: &mut W) -> Result<(), io::Error> {
        match self {
            TextWinCmd::Hide => (0x01u8).write(writer),
            TextWinCmd::HideEffect => (0x02u8).write(writer),
            TextWinCmd::HideRedraw => (0x03u8).write(writer),
            TextWinCmd::MouseWait => (0x04u8).write(writer),
            TextWinCmd::ClearText => (0x05u8).write(writer)
        }
    }
}

impl Writeable for FadeCmd {
    fn byte_size(&self) -> usize {
        match self {
            FadeCmd::Fade(idx) => 1 + idx.byte_size(),
            FadeCmd::FadeTimed(idx, fadestep) => 1 + idx.byte_size() + fadestep.byte_size(),
            FadeCmd::FadeColor(r, g, b) => 1 + r.byte_size() + g.byte_size() + b.byte_size(),
            FadeCmd::FadeTimedColor(r, g, b, fadestep) => 1 + r.byte_size() + g.byte_size() + b.byte_size() + fadestep.byte_size(),
            FadeCmd::FillScreen(idx) => 1 + idx.byte_size(),
            FadeCmd::FillScreenColor(r, g, b) => 1 + r.byte_size() + g.byte_size() + b.byte_size()
        }
    }

    fn write<W: Write>(&self, writer: &mut W) -> Result<(), io::Error> {
        match self {
            FadeCmd::Fade(idx) => {
                (0x01u8).write(writer)?;
                idx.write(writer)
            },
            FadeCmd::FadeTimed(idx, fadestep) => {
                (0x02u8).write(writer)?;
                idx.write(writer)?;
                fadestep.write(writer)
            },
            FadeCmd::FadeColor(r, g, b) => {
                (0x03u8).write(writer)?;
                r.write(writer)?;
                g.write(writer)?;
                b.write(writer)
            },
            FadeCmd::FadeTimedColor(r, g, b, fadestep) => {
                (0x04u8).write(writer)?;
                r.write(writer)?;
                g.write(writer)?;
                b.write(writer)?;
                fadestep.write(writer)
            },
            FadeCmd::FillScreen(idx) => {
                (0x10u8).write(writer)?;
                idx.write(writer)
            },
            FadeCmd::FillScreenColor(r, g, b) => {
                (0x11u8).write(writer)?;
                r.write(writer)?;
                g.write(writer)?;
                b.write(writer)
            },
        }
    }
}

impl Writeable for GrpEffect {
    fn byte_size(&self) -> usize {
        self.file.byte_size()
            + self.sx1.byte_size()
            + self.sy1.byte_size()
            + self.sx2.byte_size()
            + self.sy2.byte_size()
            + self.dx.byte_size()
            + self.dy.byte_size()
            + self.steptime.byte_size()
            + self.cmd.byte_size()
            + self.mask.byte_size()
            + self.arg1.byte_size()
            + self.arg2.byte_size()
            + self.arg3.byte_size()
            + self.step.byte_size()
            + self.arg5.byte_size()
            + self.arg6.byte_size()
    }

    fn write<W: Write>(&self, writer: &mut W) -> Result<(), io::Error> {
        self.file.write(writer)?;
        self.sx1.write(writer)?;
        self.sy1.write(writer)?;
        self.sx2.write(writer)?;
        self.sy2.write(writer)?;
        self.dx.write(writer)?;
        self.dy.write(writer)?;
        self.steptime.write(writer)?;
        self.cmd.write(writer)?;
        self.mask.write(writer)?;
        self.arg1.write(writer)?;
        self.arg2.write(writer)?;
        self.arg3.write(writer)?;
        self.step.write(writer)?;
        self.arg5.write(writer)?;
        self.arg6.write(writer)
    }
}

impl Writeable for GrpCompositeChild {
    fn byte_size(&self) -> usize {
        let method_size = match self.method {
            GrpCompositeMethod::Corner => 1,
            GrpCompositeMethod::Copy(val) => 1 + val.byte_size(),
            GrpCompositeMethod::Move1(srcx1, srcy1, srcx2, srcy2, dstx1, dstx2) => 1 + srcx1.byte_size() + srcy1.byte_size() + srcx2.byte_size() + srcy2.byte_size() + dstx1.byte_size() + dstx2.byte_size(),
            GrpCompositeMethod::Move2(srcx1, srcy1, srcx2, srcy2, dstx1, dstx2, arg) => 1 + srcx1.byte_size() + srcy1.byte_size() + srcx2.byte_size() + srcy2.byte_size() + dstx1.byte_size() + dstx2.byte_size() + arg.byte_size(),
        };
        1 + self.file.byte_size()
            + method_size
    }

    fn write<W: Write>(&self, writer: &mut W) -> Result<(), io::Error> {
        let code: u8 = match self.method {
            GrpCompositeMethod::Corner => 0x01,
            GrpCompositeMethod::Copy(_) => 0x02,
            GrpCompositeMethod::Move1(_, _, _, _, _, _) => 0x03,
            GrpCompositeMethod::Move2(_, _, _, _, _, _, _) => 0x04
        };

        code.write(writer)?;
        self.file.write(writer)?;

        match self.method {
            GrpCompositeMethod::Corner => Ok(()),
            GrpCompositeMethod::Copy(val) => val.write(writer),
            GrpCompositeMethod::Move1(srcx1, srcy1, srcx2, srcy2, dstx1, dstx2) => {
                srcx1.write(writer)?;
                srcy1.write(writer)?;
                srcx2.write(writer)?;
                srcy2.write(writer)?;
                dstx1.write(writer)?;
                dstx2.write(writer)
            },
            GrpCompositeMethod::Move2(srcx1, srcy1, srcx2, srcy2, dstx1, dstx2, arg) => {
                srcx1.write(writer)?;
                srcy1.write(writer)?;
                srcx2.write(writer)?;
                srcy2.write(writer)?;
                dstx1.write(writer)?;
                dstx2.write(writer)?;
                arg.write(writer)
            }
        }
    }
}

impl Writeable for GrpComposite {
    fn byte_size(&self) -> usize {
        mem::size_of::<u8>()
            + self.base_file.byte_size()
            + self.idx.byte_size()
            + self.children.byte_size()
    }

    fn write<W: Write>(&self, writer: &mut W) -> Result<(), io::Error> {
        (self.children.len() as u8).write(writer)?;
        self.base_file.write(writer)?;
        self.idx.write(writer)?;
        self.children.write(writer)
    }
}

impl Writeable for GrpCompositeIndexed {
    fn byte_size(&self) -> usize {
        mem::size_of::<u8>()
            + self.base_file.byte_size()
            + self.idx.byte_size()
            + self.children.byte_size()
    }

    fn write<W: Write>(&self, writer: &mut W) -> Result<(), io::Error> {
        (self.children.len() as u8).write(writer)?;
        self.base_file.write(writer)?;
        self.idx.write(writer)?;
        self.children.write(writer)
    }
}

impl Writeable for GrpCmd {
    fn byte_size(&self) -> usize {
        match self {
            GrpCmd::Load(a, b) => 1 + a.byte_size() + b.byte_size(),
            GrpCmd::LoadEffect(a) => 1 + a.byte_size(),
            GrpCmd::Load2(a, b) => 1 + a.byte_size() + b.byte_size(),
            GrpCmd::LoadEffect2(a) => 1 + a.byte_size(),
            GrpCmd::Load3(a, b) => 1 + a.byte_size() + b.byte_size(),
            GrpCmd::LoadEffect3(a) => 1 + a.byte_size(),
            GrpCmd::Unknown1 => 1,
            GrpCmd::LoadToBuf(a, b) => 1 + a.byte_size() + b.byte_size(),
            GrpCmd::LoadToBuf2(a, b) => 1 + a.byte_size() + b.byte_size(),
            GrpCmd::LoadCaching(a) => 1 + a.byte_size(),
            GrpCmd::GrpCmd0x13 => 1,
            GrpCmd::LoadComposite(a) => 1 + a.byte_size(),
            GrpCmd::LoadCompositeIndexed(a) => 1 + a.byte_size(),
            GrpCmd::MacroBufferClear => 1,
            GrpCmd::MacroBufferDelete(a) => 1 + a.byte_size(),
            GrpCmd::MacroBufferRead(a) => 1 + a.byte_size(),
            GrpCmd::MacroBufferSet(a) => 1 + a.byte_size(),
            GrpCmd::BackupScreenCopy => 1,
            GrpCmd::BackupScreenDisplay(a) => 1 + a.byte_size(),
            GrpCmd::LoadToBuf3(a, b) => 1 + a.byte_size() + b.byte_size(),
        }
    }

    fn write<W: Write>(&self, writer: &mut W) -> Result<(), io::Error> {
        match self {
            GrpCmd::Load(a, b) => {
                (0x01u8).write(writer)?;
                a.write(writer)?;
                b.write(writer)
            },
            GrpCmd::LoadEffect(a) => {
                (0x02u8).write(writer)?;
                a.write(writer)
            },
            GrpCmd::Load2(a, b) => {
                (0x03u8).write(writer)?;
                a.write(writer)?;
                b.write(writer)
            },
            GrpCmd::LoadEffect2(a) => {
                (0x04u8).write(writer)?;
                a.write(writer)
            },
            GrpCmd::Load3(a, b) => {
                (0x05u8).write(writer)?;
                a.write(writer)?;
                b.write(writer)
            },
            GrpCmd::LoadEffect3(a) => {
                (0x06u8).write(writer)?;
                a.write(writer)
            },
            GrpCmd::Unknown1 => (0x08u8).write(writer),
            GrpCmd::LoadToBuf(a, b) => {
                (0x09u8).write(writer)?;
                a.write(writer)?;
                b.write(writer)
            },
            GrpCmd::LoadToBuf2(a, b) => {
                (0x10u8).write(writer)?;
                a.write(writer)?;
                b.write(writer)
            },
            GrpCmd::LoadCaching(a) => {
                (0x11u8).write(writer)?;
                a.write(writer)
            },
            GrpCmd::GrpCmd0x13 => (0x13u8).write(writer),
            GrpCmd::LoadComposite(a) => {
                (0x22u8).write(writer)?;
                a.write(writer)
            },
            GrpCmd::LoadCompositeIndexed(a) => {
                (0x24u8).write(writer)?;
                a.write(writer)
            },
            GrpCmd::MacroBufferClear => (0x30u8).write(writer),
            GrpCmd::MacroBufferDelete(a) => {
                (0x31u8).write(writer)?;
                a.write(writer)
            },
            GrpCmd::MacroBufferRead(a) => {
                (0x32u8).write(writer)?;
                a.write(writer)
            },
            GrpCmd::MacroBufferSet(a) => {
                (0x33u8).write(writer)?;
                a.write(writer)
            },
            GrpCmd::BackupScreenCopy => (0x50u8).write(writer),
            GrpCmd::BackupScreenDisplay(a) => {
                (0x52u8).write(writer)?;
                a.write(writer)
            },
            GrpCmd::LoadToBuf3(a, b) => {
                (0x54u8).write(writer)?;
                a.write(writer)?;
                b.write(writer)
            },
        }
    }
}

impl Writeable for ScreenShakeCmd {
    fn byte_size(&self) -> usize {
        match self {
            ScreenShakeCmd::ScreenShake(idx) => 1 + idx.byte_size(),
        }
    }

    fn write<W: Write>(&self, writer: &mut W) -> Result<(), io::Error> {
        match self {
            ScreenShakeCmd::ScreenShake(idx) => {
                (0x01u8).write(writer)?;
                idx.write(writer)
            },
        }
    }
}

impl Writeable for SndCmd {
    fn byte_size(&self) -> usize {
        match self {
            SndCmd::BgmLoop(a) => 1 + a.byte_size(),
            SndCmd::BgmWait(a) => 1 + a.byte_size(),
            SndCmd::BgmOnce(a) => 1 + a.byte_size(),
            SndCmd::BgmFadeInLoop(a, b) => 1 + a.byte_size() + b.byte_size(),
            SndCmd::BgmFadeInWait(a, b) => 1 + a.byte_size() + b.byte_size(),
            SndCmd::BgmFadeInOnce(a, b) => 1 + a.byte_size() + b.byte_size(),
            SndCmd::BgmFadeOut(a) => 1 + a.byte_size(),
            SndCmd::BgmStop => 1,
            SndCmd::BgmRewind => 1,
            SndCmd::BgmUnknown1 => 1,
            SndCmd::KoePlayWait(a) => 1 + a.byte_size(),
            SndCmd::KoePlay(a) => 1 + a.byte_size(),
            SndCmd::KoePlay2(a, b) => 1 + a.byte_size() + b.byte_size(),
            SndCmd::WavPlay(a) => 1 + a.byte_size(),
            SndCmd::WavPlay2(a, b) => 1 + a.byte_size() + b.byte_size(),
            SndCmd::WavLoop(a) => 1 + a.byte_size(),
            SndCmd::WavLoop2(a, b) => 1 + a.byte_size() + b.byte_size(),
            SndCmd::WavPlayWait(a) => 1 + a.byte_size(),
            SndCmd::WavPlayWait2(a, b) => 1 + a.byte_size() + b.byte_size(),
            SndCmd::WavStop => 1,
            SndCmd::WavStop2(a) => 1 + a.byte_size(),
            SndCmd::WavStop3 => 1,
            SndCmd::WavUnknown0x39(a) => 1 + a.byte_size(),
            SndCmd::SePlay(a) => 1 + a.byte_size(),
            SndCmd::MoviePlay(a, b, c, d, e) => 1 + a.byte_size() + b.byte_size() + c.byte_size() + d.byte_size() + e.byte_size(),
            SndCmd::MovieLoop(a, b, c, d, e) => 1 + a.byte_size() + b.byte_size() + c.byte_size() + d.byte_size() + e.byte_size(),
            SndCmd::MovieWait(a, b, c, d, e) => 1 + a.byte_size() + b.byte_size() + c.byte_size() + d.byte_size() + e.byte_size(),
            SndCmd::MovieWaitCancelable(a, b, c, d, e) => 1 + a.byte_size() + b.byte_size() + c.byte_size() + d.byte_size() + e.byte_size(),
            SndCmd::MovieWait2(a, b, c, d, e, f) => 1 + a.byte_size() + b.byte_size() + c.byte_size() + d.byte_size() + e.byte_size() + f.byte_size(),
            SndCmd::MovieWaitCancelable2(a, b, c, d, e, f) => 1 + a.byte_size() + b.byte_size() + c.byte_size() + d.byte_size() + e.byte_size() + f.byte_size(),
            SndCmd::Unknown1 => 1,
        }
    }

    fn write<W: Write>(&self, writer: &mut W) -> Result<(), io::Error> {
        match self {
            SndCmd::BgmLoop(a) => {
                (0x01u8).write(writer)?;
                a.write(writer)
            },
            SndCmd::BgmWait(a) => {
                (0x02u8).write(writer)?;
                a.write(writer)
            },
            SndCmd::BgmOnce(a) => {
                (0x03u8).write(writer)?;
                a.write(writer)
            },
            SndCmd::BgmFadeInLoop(a, b) => {
                (0x05u8).write(writer)?;
                a.write(writer)?;
                b.write(writer)
            },
            SndCmd::BgmFadeInWait(a, b) => {
                (0x06u8).write(writer)?;
                a.write(writer)?;
                b.write(writer)
            },
            SndCmd::BgmFadeInOnce(a, b) => {
                (0x07u8).write(writer)?;
                a.write(writer)?;
                b.write(writer)
            },
            SndCmd::BgmFadeOut(a) => {
                (0x10u8).write(writer)?;
                a.write(writer)
            },
            SndCmd::BgmStop => (0x11u8).write(writer),
            SndCmd::BgmRewind => (0x12u8).write(writer),
            SndCmd::BgmUnknown1 => (0x16u8).write(writer),
            SndCmd::KoePlayWait(a) => {
                (0x20u8).write(writer)?;
                a.write(writer)
            },
            SndCmd::KoePlay(a) => {
                (0x21u8).write(writer)?;
                a.write(writer)
            },
            SndCmd::KoePlay2(a, b) => {
                (0x22u8).write(writer)?;
                a.write(writer)?;
                b.write(writer)
            },
            SndCmd::WavPlay(a) => {
                (0x30u8).write(writer)?;
                a.write(writer)
            },
            SndCmd::WavPlay2(a, b) => {
                (0x31u8).write(writer)?;
                a.write(writer)?;
                b.write(writer)
            },
            SndCmd::WavLoop(a) => {
                (0x32u8).write(writer)?;
                a.write(writer)
            },
            SndCmd::WavLoop2(a, b) => {
                (0x33u8).write(writer)?;
                a.write(writer)?;
                b.write(writer)
            },
            SndCmd::WavPlayWait(a) => {
                (0x34u8).write(writer)?;
                a.write(writer)
            },
            SndCmd::WavPlayWait2(a, b) => {
                (0x35u8).write(writer)?;
                a.write(writer)?;
                b.write(writer)
            },
            SndCmd::WavStop => (0x36u8).write(writer),
            SndCmd::WavStop2(a) => {
                (0x37u8).write(writer)?;
                a.write(writer)
            },
            SndCmd::WavStop3 => (0x38u8).write(writer),
            SndCmd::WavUnknown0x39(a) => {
                (0x39u8).write(writer)?;
                a.write(writer)
            },
            SndCmd::SePlay(a) => {
                (0x44u8).write(writer)?;
                a.write(writer)
            },
            SndCmd::MoviePlay(a, b, c, d, e) => {
                (0x50u8).write(writer)?;
                a.write(writer)?;
                b.write(writer)?;
                c.write(writer)?;
                d.write(writer)?;
                e.write(writer)
            },
            SndCmd::MovieLoop(a, b, c, d, e) => {
                (0x51u8).write(writer)?;
                a.write(writer)?;
                b.write(writer)?;
                c.write(writer)?;
                d.write(writer)?;
                e.write(writer)
            },
            SndCmd::MovieWait(a, b, c, d, e) => {
                (0x52u8).write(writer)?;
                a.write(writer)?;
                b.write(writer)?;
                c.write(writer)?;
                d.write(writer)?;
                e.write(writer)
            },
            SndCmd::MovieWaitCancelable(a, b, c, d, e) => {
                (0x53u8).write(writer)?;
                a.write(writer)?;
                b.write(writer)?;
                c.write(writer)?;
                d.write(writer)?;
                e.write(writer)
            },
            SndCmd::MovieWait2(a, b, c, d, e, f) => {
                (0x54u8).write(writer)?;
                a.write(writer)?;
                b.write(writer)?;
                c.write(writer)?;
                d.write(writer)?;
                e.write(writer)?;
                f.write(writer)
            },
            SndCmd::MovieWaitCancelable2(a, b, c, d, e, f) => {
                (0x55u8).write(writer)?;
                a.write(writer)?;
                b.write(writer)?;
                c.write(writer)?;
                d.write(writer)?;
                e.write(writer)?;
                f.write(writer)
            },
            SndCmd::Unknown1 => (0x60u8).write(writer),
        }
    }
}

impl Writeable for WaitCmd {
    fn byte_size(&self) -> usize {
        match self {
            WaitCmd::Wait(idx) => 1 + idx.byte_size(),
            WaitCmd::WaitMouse(a, b) => 1 + a.byte_size() + b.byte_size(),
            WaitCmd::SetToBase => 1,
            WaitCmd::WaitFromBase(idx) => 1 + idx.byte_size(),
            WaitCmd::WaitFromBaseMouse(idx) => 1 + idx.byte_size(),
            WaitCmd::SetToBaseVal(idx) => 1 + idx.byte_size(),
            WaitCmd::Wait0x10 => 1,
            WaitCmd::Wait0x11 => 1,
            WaitCmd::Wait0x12 => 1,
            WaitCmd::Wait0x13 => 1
        }
    }

    fn write<W: Write>(&self, writer: &mut W) -> Result<(), io::Error> {
        match self {
            WaitCmd::Wait(idx) => {
                (0x01u8).write(writer)?;
                idx.write(writer)
            },
            WaitCmd::WaitMouse(a, b) => {
                (0x02u8).write(writer)?;
                a.write(writer)?;
                b.write(writer)
            },
            WaitCmd::SetToBase => (0x03u8).write(writer),
            WaitCmd::WaitFromBase(idx) => {
                (0x04u8).write(writer)?;
                idx.write(writer)
            },
            WaitCmd::WaitFromBaseMouse(idx) => {
                (0x05u8).write(writer)?;
                idx.write(writer)
            },
            WaitCmd::SetToBaseVal(idx) => {
                (0x06u8).write(writer)?;
                idx.write(writer)
            },
            WaitCmd::Wait0x10 => (0x10u8).write(writer),
            WaitCmd::Wait0x11 => (0x11u8).write(writer),
            WaitCmd::Wait0x12 => (0x12u8).write(writer),
            WaitCmd::Wait0x13 => (0x13u8).write(writer)
        }
    }
}

impl Writeable for RetCmd {
    fn byte_size(&self) -> usize {
        match self {
            RetCmd::SameScene => 1,
            RetCmd::OtherScene => 1,
            RetCmd::PopStack => 1,
            RetCmd::ClearStack => 1
        }
    }

    fn write<W: Write>(&self, writer: &mut W) -> Result<(), io::Error> {
        match self {
            RetCmd::SameScene => (0x01u8).write(writer),
            RetCmd::OtherScene => (0x02u8).write(writer),
            RetCmd::PopStack => (0x03u8).write(writer),
            RetCmd::ClearStack => (0x06u8).write(writer)
        }
    }
}

impl Writeable for ScenarioMenuCmd {
    fn byte_size(&self) -> usize {
        match self {
            ScenarioMenuCmd::SetBit(idx) => 1 + idx.byte_size(),
            ScenarioMenuCmd::SetBit2(a, b) => 1 + a.byte_size() + b.byte_size(),
        }
    }

    fn write<W: Write>(&self, writer: &mut W) -> Result<(), io::Error> {
        match self {
            ScenarioMenuCmd::SetBit(idx) => {
                (0x01u8).write(writer)?;
                idx.write(writer)
            },
            ScenarioMenuCmd::SetBit2(a, b) => {
                (0x02u8).write(writer)?;
                a.write(writer)?;
                b.write(writer)
            }
        }
    }
}

impl Writeable for TextRankCmd {
    fn byte_size(&self) -> usize {
        match self {
            TextRankCmd::Set(idx) => 1 + idx.byte_size(),
            TextRankCmd::Clear => 1
        }
    }

    fn write<W: Write>(&self, writer: &mut W) -> Result<(), io::Error> {
        match self {
            TextRankCmd::Set(idx) => {
                (0x01u8).write(writer)?;
                idx.write(writer)
            },
            TextRankCmd::Clear => (0x02u8).write(writer)
        }
    }
}

impl Writeable for Choice {
    fn byte_size(&self) -> usize {
        1
    }

    fn write<W: Write>(&self, writer: &mut W) -> Result<(), io::Error> {
        match self {
            Choice::Choice => (0x22u8).write(writer),
            Choice::End => (0x23u8).write(writer)
        }
    }
}

impl Writeable for ChoiceText {
    fn byte_size(&self) -> usize {
        self.pad.byte_size() + self.texts.byte_size() + 1
    }

    fn write<W: Write>(&self, writer: &mut W) -> Result<(), io::Error> {
        self.pad.write(writer)?;
        self.texts.write(writer)?;
        (0x23u8).write(writer)
    }
}

impl Writeable for ChoiceCmd {
    fn byte_size(&self) -> usize {
        match self {
            ChoiceCmd::Choice(idx, flag, texts) => 1 + idx.byte_size() + flag.byte_size() + texts.byte_size(),
            ChoiceCmd::Choice2(idx, flag, texts) => 1 + idx.byte_size() + flag.byte_size() + texts.byte_size(),
            ChoiceCmd::LoadMenu(idx) => 1 + idx.byte_size()
        }
    }

    fn write<W: Write>(&self, writer: &mut W) -> Result<(), io::Error> {
        match self {
            ChoiceCmd::Choice(idx, flag, texts) => {
                (0x01u8).write(writer)?;
                idx.write(writer)?;
                flag.write(writer)?;
                texts.write(writer)
            },
            ChoiceCmd::Choice2(idx, flag, texts) => {
                (0x02u8).write(writer)?;
                idx.write(writer)?;
                flag.write(writer)?;
                texts.write(writer)
            },
            ChoiceCmd::LoadMenu(idx) => {
                (0x04u8).write(writer)?;
                idx.write(writer)
            }
        }
    }
}

impl Writeable for StringCmd {
    fn byte_size(&self) -> usize {
        match self {
            StringCmd::StrcpyLiteral(dest, text) => 1 + dest.byte_size() + text.byte_size(),
            StringCmd::Strlen(dest, src) => 1 + dest.byte_size() + src.byte_size(),
            StringCmd::Strcmp(dest, text1, text2) => 1 + dest.byte_size() + text1.byte_size() + text2.byte_size(),
            StringCmd::Strcat(dest, src) => 1 + dest.byte_size() + src.byte_size(),
            StringCmd::Strcpy(dest, src) => 1 + dest.byte_size() + src.byte_size(),
            StringCmd::Itoa(dest, src, ordinal) => 1 + dest.byte_size() + src.byte_size() + ordinal.byte_size(),
            StringCmd::HanToZen(dest) => 1 + dest.byte_size(),
            StringCmd::Atoi(dest, src) => 1 + dest.byte_size() + src.byte_size(),
        }
    }

    fn write<W: Write>(&self, writer: &mut W) -> Result<(), io::Error> {
        match self {
            StringCmd::StrcpyLiteral(dest, text) => {
                (0x01u8).write(writer)?;
                dest.write(writer)?;
                text.write(writer)
            },
            StringCmd::Strlen(dest, src) => {
                (0x02u8).write(writer)?;
                dest.write(writer)?;
                src.write(writer)
            },
            StringCmd::Strcmp(dest, text1, text2) => {
                (0x03u8).write(writer)?;
                dest.write(writer)?;
                text1.write(writer)?;
                text2.write(writer)
            },
            StringCmd::Strcat(dest, src) => {
                (0x04u8).write(writer)?;
                dest.write(writer)?;
                src.write(writer)
            },
            StringCmd::Strcpy(dest, src) => {
                (0x05u8).write(writer)?;
                dest.write(writer)?;
                src.write(writer)
            },
            StringCmd::Itoa(dest, src, ordinal) => {
                (0x06u8).write(writer)?;
                dest.write(writer)?;
                src.write(writer)?;
                ordinal.write(writer)
            },
            StringCmd::HanToZen(dest) => {
                (0x07u8).write(writer)?;
                dest.write(writer)
            },
            StringCmd::Atoi(dest, src) => {
                (0x08u8).write(writer)?;
                dest.write(writer)?;
                src.write(writer)
            },
        }
    }
}

impl Writeable for SetMultiCmd {
    fn byte_size(&self) -> usize {
        match self {
            SetMultiCmd::Val(start_idx, end_idx, value) => 1 + start_idx.byte_size() + end_idx.byte_size() + value.byte_size(),
            SetMultiCmd::Bit(start_idx, end_idx, value) => 1 + start_idx.byte_size() + end_idx.byte_size() + value.byte_size(),
        }
    }

    fn write<W: Write>(&self, writer: &mut W) -> Result<(), io::Error> {
        match self {
            SetMultiCmd::Val(start_idx, end_idx, value) => {
                (0x01u8).write(writer)?;
                start_idx.write(writer)?;
                end_idx.write(writer)?;
                value.write(writer)
            },
            SetMultiCmd::Bit(start_idx, end_idx, value) => {
                (0x02u8).write(writer)?;
                start_idx.write(writer)?;
                end_idx.write(writer)?;
                value.write(writer)
            },
        }
    }
}

impl Writeable for BRGRectColor {
    fn byte_size(&self) -> usize {
        self.srcx1.byte_size()
            + self.srcy1.byte_size()
            + self.srcx2.byte_size()
            + self.srcy2.byte_size()
            + self.srcpdt.byte_size()
            + self.r.byte_size()
            + self.g.byte_size()
            + self.b.byte_size()
    }

    fn write<W: Write>(&self, writer: &mut W) -> Result<(), io::Error> {
        self.srcx1.write(writer)?;
        self.srcy1.write(writer)?;
        self.srcx2.write(writer)?;
        self.srcy2.write(writer)?;
        self.srcpdt.write(writer)?;
        self.r.write(writer)?;
        self.g.write(writer)?;
        self.b.write(writer)
    }
}

impl Writeable for BRGRect {
    fn byte_size(&self) -> usize {
        self.srcx1.byte_size()
            + self.srcy1.byte_size()
            + self.srcx2.byte_size()
            + self.srcy2.byte_size()
            + self.srcpdt.byte_size()
    }

    fn write<W: Write>(&self, writer: &mut W) -> Result<(), io::Error> {
        self.srcx1.write(writer)?;
        self.srcy1.write(writer)?;
        self.srcx2.write(writer)?;
        self.srcy2.write(writer)?;
        self.srcpdt.write(writer)
    }
}

impl Writeable for BRGFadeOutColor {
    fn byte_size(&self) -> usize {
        self.srcx1.byte_size()
            + self.srcy1.byte_size()
            + self.srcx2.byte_size()
            + self.srcy2.byte_size()
            + self.srcpdt.byte_size()
            + self.r.byte_size()
            + self.g.byte_size()
            + self.b.byte_size()
            + self.count.byte_size()
    }

    fn write<W: Write>(&self, writer: &mut W) -> Result<(), io::Error> {
        self.srcx1.write(writer)?;
        self.srcy1.write(writer)?;
        self.srcx2.write(writer)?;
        self.srcy2.write(writer)?;
        self.srcpdt.write(writer)?;
        self.r.write(writer)?;
        self.g.write(writer)?;
        self.b.write(writer)?;
        self.count.write(writer)
    }
}

impl Writeable for BRGStretchBlit {
    fn byte_size(&self) -> usize {
        self.srcx1.byte_size()
            + self.srcy1.byte_size()
            + self.srcx2.byte_size()
            + self.srcy2.byte_size()
            + self.srcpdt.byte_size()
            + self.dstx1.byte_size()
            + self.dstx2.byte_size()
            + self.dsty1.byte_size()
            + self.dsty2.byte_size()
            + self.dstpdt.byte_size()
    }

    fn write<W: Write>(&self, writer: &mut W) -> Result<(), io::Error> {
        self.srcx1.write(writer)?;
        self.srcy1.write(writer)?;
        self.srcx2.write(writer)?;
        self.srcy2.write(writer)?;
        self.srcpdt.write(writer)?;
        self.dstx1.write(writer)?;
        self.dstx2.write(writer)?;
        self.dsty1.write(writer)?;
        self.dsty2.write(writer)?;
        self.dstpdt.write(writer)
    }
}

impl Writeable for BRGStretchBlitEffect {
    fn byte_size(&self) -> usize {
        self.sx1.byte_size()
            + self.sy1.byte_size()
            + self.sx2.byte_size()
            + self.sy2.byte_size()
            + self.ex1.byte_size()
            + self.ey1.byte_size()
            + self.ex2.byte_size()
            + self.ey2.byte_size()
            + self.srcpdt.byte_size()
            + self.dx1.byte_size()
            + self.dy1.byte_size()
            + self.dx2.byte_size()
            + self.dy2.byte_size()
            + self.dstpdt.byte_size()
            + self.step.byte_size()
            + self.steptime.byte_size()
    }

    fn write<W: Write>(&self, writer: &mut W) -> Result<(), io::Error> {
        self.sx1.write(writer)?;
        self.sy1.write(writer)?;
        self.sx2.write(writer)?;
        self.sy2.write(writer)?;
        self.ex1.write(writer)?;
        self.ey1.write(writer)?;
        self.ex2.write(writer)?;
        self.ey2.write(writer)?;
        self.srcpdt.write(writer)?;
        self.dx1.write(writer)?;
        self.dy1.write(writer)?;
        self.dx2.write(writer)?;
        self.dy2.write(writer)?;
        self.dstpdt.write(writer)?;
        self.step.write(writer)?;
        self.steptime.write(writer)
    }
}

impl Writeable for BufferRegionGrpCmd {
    fn byte_size(&self) -> usize {
        match self {
            BufferRegionGrpCmd::ClearRect(a) => 1 + a.byte_size(),
            BufferRegionGrpCmd::DrawRectLine(a) => 1 + a.byte_size(),
            BufferRegionGrpCmd::InvertColor(a) => 1 + a.byte_size(),
            BufferRegionGrpCmd::ColorMask(a) => 1 + a.byte_size(),
            BufferRegionGrpCmd::FadeOutColor(a) => 1 + a.byte_size(),
            BufferRegionGrpCmd::FadeOutColor2(a) => 1 + a.byte_size(),
            BufferRegionGrpCmd::FadeOutColor3(a) => 1 + a.byte_size(),
            BufferRegionGrpCmd::MakeMonoImage(a) => 1 + a.byte_size(),
            BufferRegionGrpCmd::StretchBlit(a) => 1 + a.byte_size(),
            BufferRegionGrpCmd::StretchBlitEffect(a) => 1 + a.byte_size(),
        }
    }

    fn write<W: Write>(&self, writer: &mut W) -> Result<(), io::Error> {
        match self {
            BufferRegionGrpCmd::ClearRect(a) => {
                (0x02u8).write(writer)?;
                a.write(writer)
            },
            BufferRegionGrpCmd::DrawRectLine(a) => {
                (0x04u8).write(writer)?;
                a.write(writer)
            },
            BufferRegionGrpCmd::InvertColor(a) => {
                (0x07u8).write(writer)?;
                a.write(writer)
            },
            BufferRegionGrpCmd::ColorMask(a) => {
                (0x10u8).write(writer)?;
                a.write(writer)
            },
            BufferRegionGrpCmd::FadeOutColor(a) => {
                (0x11u8).write(writer)?;
                a.write(writer)
            },
            BufferRegionGrpCmd::FadeOutColor2(a) => {
                (0x12u8).write(writer)?;
                a.write(writer)
            },
            BufferRegionGrpCmd::FadeOutColor3(a) => {
                (0x15u8).write(writer)?;
                a.write(writer)
            },
            BufferRegionGrpCmd::MakeMonoImage(a) => {
                (0x20u8).write(writer)?;
                a.write(writer)
            },
            BufferRegionGrpCmd::StretchBlit(a) => {
                (0x30u8).write(writer)?;
                a.write(writer)
            },
            BufferRegionGrpCmd::StretchBlitEffect(a) => {
                (0x32u8).write(writer)?;
                a.write(writer)
            },
        }
    }
}

impl Writeable for BGCopySamePos {
    fn byte_size(&self) -> usize {
        self.srcx1.byte_size()
            + self.srcy1.byte_size()
            + self.srcx2.byte_size()
            + self.srcy2.byte_size()
            + self.srcpdt.byte_size()
            + self.flag.byte_size()
    }

    fn write<W: Write>(&self, writer: &mut W) -> Result<(), io::Error> {
        self.srcx1.write(writer)?;
        self.srcy1.write(writer)?;
        self.srcx2.write(writer)?;
        self.srcy2.write(writer)?;
        self.srcpdt.write(writer)?;
        self.flag.write(writer)
    }
}

impl Writeable for BGCopyNewPos {
    fn byte_size(&self) -> usize {
        self.srcx1.byte_size()
            + self.srcy1.byte_size()
            + self.srcx2.byte_size()
            + self.srcy2.byte_size()
            + self.srcpdt.byte_size()
            + self.dstx1.byte_size()
            + self.dsty1.byte_size()
            + self.dstpdt.byte_size()
            + self.flag.byte_size()
    }

    fn write<W: Write>(&self, writer: &mut W) -> Result<(), io::Error> {
        self.srcx1.write(writer)?;
        self.srcy1.write(writer)?;
        self.srcx2.write(writer)?;
        self.srcy2.write(writer)?;
        self.srcpdt.write(writer)?;
        self.dstx1.write(writer)?;
        self.dsty1.write(writer)?;
        self.dstpdt.write(writer)?;
        self.flag.write(writer)
    }
}

impl Writeable for BGCopyColor {
    fn byte_size(&self) -> usize {
        self.srcx1.byte_size()
            + self.srcy1.byte_size()
            + self.srcx2.byte_size()
            + self.srcy2.byte_size()
            + self.srcpdt.byte_size()
            + self.dstx1.byte_size()
            + self.dsty1.byte_size()
            + self.dstpdt.byte_size()
            + self.r.byte_size()
            + self.g.byte_size()
            + self.b.byte_size()
    }

    fn write<W: Write>(&self, writer: &mut W) -> Result<(), io::Error> {
        self.srcx1.write(writer)?;
        self.srcy1.write(writer)?;
        self.srcx2.write(writer)?;
        self.srcy2.write(writer)?;
        self.srcpdt.write(writer)?;
        self.dstx1.write(writer)?;
        self.dsty1.write(writer)?;
        self.dstpdt.write(writer)?;
        self.r.write(writer)?;
        self.g.write(writer)?;
        self.b.write(writer)
    }
}

impl Writeable for BGSwap {
    fn byte_size(&self) -> usize {
        self.srcx1.byte_size()
            + self.srcy1.byte_size()
            + self.srcx2.byte_size()
            + self.srcy2.byte_size()
            + self.srcpdt.byte_size()
            + self.dstx1.byte_size()
            + self.dsty1.byte_size()
            + self.dstpdt.byte_size()
    }

    fn write<W: Write>(&self, writer: &mut W) -> Result<(), io::Error> {
        self.srcx1.write(writer)?;
        self.srcy1.write(writer)?;
        self.srcx2.write(writer)?;
        self.srcy2.write(writer)?;
        self.srcpdt.write(writer)?;
        self.dstx1.write(writer)?;
        self.dsty1.write(writer)?;
        self.dstpdt.write(writer)
    }
}

impl Writeable for BGCopyWithMask {
    fn byte_size(&self) -> usize {
        self.srcx1.byte_size()
            + self.srcy1.byte_size()
            + self.srcx2.byte_size()
            + self.srcy2.byte_size()
            + self.srcpdt.byte_size()
            + self.dstx1.byte_size()
            + self.dsty1.byte_size()
            + self.dstpdt.byte_size()
            + self.flag.byte_size()
    }

    fn write<W: Write>(&self, writer: &mut W) -> Result<(), io::Error> {
        self.srcx1.write(writer)?;
        self.srcy1.write(writer)?;
        self.srcx2.write(writer)?;
        self.srcy2.write(writer)?;
        self.srcpdt.write(writer)?;
        self.dstx1.write(writer)?;
        self.dsty1.write(writer)?;
        self.dstpdt.write(writer)?;
        self.flag.write(writer)
    }
}

impl Writeable for BGCopyWholeScreen {
    fn byte_size(&self) -> usize {
        self.srcpdt.byte_size()
            + self.dstpdt.byte_size()
            + self.flag.byte_size()
    }

    fn write<W: Write>(&self, writer: &mut W) -> Result<(), io::Error> {
        self.srcpdt.write(writer)?;
        self.dstpdt.write(writer)?;
        self.flag.write(writer)
    }
}

impl Writeable for BGDisplayStrings {
    fn byte_size(&self) -> usize {
        self.n.byte_size()
            + self.srcx1.byte_size()
            + self.srcy1.byte_size()
            + self.srcx2.byte_size()
            + self.srcy2.byte_size()
            + self.srcdx.byte_size()
            + self.srcdy.byte_size()
            + self.srcpdt.byte_size()
            + self.dstx1.byte_size()
            + self.dsty1.byte_size()
            + self.dstx2.byte_size()
            + self.dsty2.byte_size()
            + self.count.byte_size()
            + self.zero.byte_size()
            + self.dstpdt.byte_size()
    }

    fn write<W: Write>(&self, writer: &mut W) -> Result<(), io::Error> {
        self.n.write(writer)?;
        self.srcx1.write(writer)?;
        self.srcy1.write(writer)?;
        self.srcx2.write(writer)?;
        self.srcy2.write(writer)?;
        self.srcdx.write(writer)?;
        self.srcdy.write(writer)?;
        self.srcpdt.write(writer)?;
        self.dstx1.write(writer)?;
        self.dsty1.write(writer)?;
        self.dstx2.write(writer)?;
        self.dsty2.write(writer)?;
        self.count.write(writer)?;
        self.zero.write(writer)?;
        self.dstpdt.write(writer)
    }
}

impl Writeable for BGDisplayStringsMask {
    fn byte_size(&self) -> usize {
        self.n.byte_size()
            + self.srcx1.byte_size()
            + self.srcy1.byte_size()
            + self.srcx2.byte_size()
            + self.srcy2.byte_size()
            + self.srcdx.byte_size()
            + self.srcdy.byte_size()
            + self.srcpdt.byte_size()
            + self.dstx1.byte_size()
            + self.dsty1.byte_size()
            + self.dstx2.byte_size()
            + self.dsty2.byte_size()
            + self.count.byte_size()
            + self.zero.byte_size()
            + self.dstpdt.byte_size()
            + self.flag.byte_size()
    }

    fn write<W: Write>(&self, writer: &mut W) -> Result<(), io::Error> {
        self.n.write(writer)?;
        self.srcx1.write(writer)?;
        self.srcy1.write(writer)?;
        self.srcx2.write(writer)?;
        self.srcy2.write(writer)?;
        self.srcdx.write(writer)?;
        self.srcdy.write(writer)?;
        self.srcpdt.write(writer)?;
        self.dstx1.write(writer)?;
        self.dsty1.write(writer)?;
        self.dstx2.write(writer)?;
        self.dsty2.write(writer)?;
        self.count.write(writer)?;
        self.zero.write(writer)?;
        self.dstpdt.write(writer)?;
        self.flag.write(writer)
    }
}

impl Writeable for BGDisplayStringsColor {
    fn byte_size(&self) -> usize {
        self.n.byte_size()
            + self.srcx1.byte_size()
            + self.srcy1.byte_size()
            + self.srcx2.byte_size()
            + self.srcy2.byte_size()
            + self.srcdx.byte_size()
            + self.srcdy.byte_size()
            + self.srcpdt.byte_size()
            + self.dstx1.byte_size()
            + self.dsty1.byte_size()
            + self.dstx2.byte_size()
            + self.dsty2.byte_size()
            + self.count.byte_size()
            + self.zero.byte_size()
            + self.dstpdt.byte_size()
            + self.r.byte_size()
            + self.g.byte_size()
            + self.b.byte_size()
    }

    fn write<W: Write>(&self, writer: &mut W) -> Result<(), io::Error> {
        self.n.write(writer)?;
        self.srcx1.write(writer)?;
        self.srcy1.write(writer)?;
        self.srcx2.write(writer)?;
        self.srcy2.write(writer)?;
        self.srcdx.write(writer)?;
        self.srcdy.write(writer)?;
        self.srcpdt.write(writer)?;
        self.dstx1.write(writer)?;
        self.dsty1.write(writer)?;
        self.dstx2.write(writer)?;
        self.dsty2.write(writer)?;
        self.count.write(writer)?;
        self.zero.write(writer)?;
        self.dstpdt.write(writer)?;
        self.r.write(writer)?;
        self.g.write(writer)?;
        self.b.write(writer)
    }
}

impl Writeable for BufferGrpCmd {
    fn byte_size(&self) -> usize {
        match self {
            BufferGrpCmd::CopySamePos(a) => 1 + a.byte_size(),
            BufferGrpCmd::CopyNewPos(a) => 1 + a.byte_size(),
            BufferGrpCmd::CopyNewPosMask(a) => 1 + a.byte_size(),
            BufferGrpCmd::CopyColor(a) => 1 + a.byte_size(),
            BufferGrpCmd::Swap(a) => 1 + a.byte_size(),
            BufferGrpCmd::CopyWithMask(a) => 1 + a.byte_size(),
            BufferGrpCmd::CopyWholeScreen(a) => 1 + a.byte_size(),
            BufferGrpCmd::CopyWholeScreenMask(a) => 1 + a.byte_size(),
            BufferGrpCmd::DisplayStrings(a) => 1 + a.byte_size(),
            BufferGrpCmd::DisplayStringsMask(a) => 1 + a.byte_size(),
            BufferGrpCmd::DisplayStringsColor(a) => 1 + a.byte_size(),
        }
    }

    fn write<W: Write>(&self, writer: &mut W) -> Result<(), io::Error> {
        match self {
            BufferGrpCmd::CopySamePos(a) => {
                (0x00u8).write(writer)?;
                a.write(writer)
            }
            BufferGrpCmd::CopyNewPos(a) => {
                (0x01u8).write(writer)?;
                a.write(writer)
            }
            BufferGrpCmd::CopyNewPosMask(a) => {
                (0x02u8).write(writer)?;
                a.write(writer)
            }
            BufferGrpCmd::CopyColor(a) => {
                (0x03u8).write(writer)?;
                a.write(writer)
            }
            BufferGrpCmd::Swap(a) => {
                (0x05u8).write(writer)?;
                a.write(writer)
            }
            BufferGrpCmd::CopyWithMask(a) => {
                (0x08u8).write(writer)?;
                a.write(writer)
            }
            BufferGrpCmd::CopyWholeScreen(a) => {
                (0x11u8).write(writer)?;
                a.write(writer)
            }
            BufferGrpCmd::CopyWholeScreenMask(a) => {
                (0x12u8).write(writer)?;
                a.write(writer)
            }
            BufferGrpCmd::DisplayStrings(a) => {
                (0x20u8).write(writer)?;
                a.write(writer)
            }
            BufferGrpCmd::DisplayStringsMask(a) => {
                (0x21u8).write(writer)?;
                a.write(writer)
            }
            BufferGrpCmd::DisplayStringsColor(a) => {
                (0x22u8).write(writer)?;
                a.write(writer)
            }
        }
    }
}

impl Writeable for FlashGrpCmd {
    fn byte_size(&self) -> usize {
        match self {
            FlashGrpCmd::FillColor(dstpdt, r, g, b) => 1 + dstpdt.byte_size() + r.byte_size() + g.byte_size() + b.byte_size(),
            FlashGrpCmd::FlashScreen(r, g, b, time, count) => 1 + r.byte_size() + g.byte_size() + b.byte_size() + time.byte_size() + count.byte_size(),
        }
    }

    fn write<W: Write>(&self, writer: &mut W) -> Result<(), io::Error> {
        match self {
            FlashGrpCmd::FillColor(dstpdt, r, g, b) => {
                (0x01u8).write(writer)?;
                dstpdt.write(writer)?;
                r.write(writer)?;
                g.write(writer)?;
                b.write(writer)
            },
            FlashGrpCmd::FlashScreen(r, g, b, time, count) => {
                (0x10u8).write(writer)?;
                r.write(writer)?;
                g.write(writer)?;
                b.write(writer)?;
                time.write(writer)?;
                count.write(writer)
            }
        }
    }
}

impl Writeable for MultiPdtEntry {
    fn byte_size(&self) -> usize {
        self.text.byte_size() + self.data.byte_size()
    }

    fn write<W: Write>(&self, writer: &mut W) -> Result<(), io::Error> {
        self.text.write(writer)?;
        self.data.write(writer)
    }
}

impl Writeable for MultiPdtCmd {
    fn byte_size(&self) -> usize {
        match self {
            MultiPdtCmd::Slideshow(pos, wait, entries) => 1 + mem::size_of::<u8>() + pos.byte_size() + wait.byte_size() + entries.byte_size(),
            MultiPdtCmd::SlideshowLoop(pos, wait, entries) => 1 + mem::size_of::<u8>() + pos.byte_size() + wait.byte_size() + entries.byte_size(),
            MultiPdtCmd::StopSlideshowLoop => 1,
            MultiPdtCmd::Scroll(poscmd, pos, wait, pixel, entries) => 1 + poscmd.byte_size() + mem::size_of::<u8>() + pos.byte_size() + wait.byte_size() + pixel.byte_size() + entries.byte_size(),
            MultiPdtCmd::Scroll2(poscmd, pos, wait, pixel, entries) => 1 + poscmd.byte_size() + mem::size_of::<u8>() + pos.byte_size() + wait.byte_size() + pixel.byte_size() + entries.byte_size(),
            MultiPdtCmd::ScrollWithCancel(poscmd, pos, wait, pixel, cancel_index, entries) => 1 + poscmd.byte_size() + mem::size_of::<u8>() + pos.byte_size() + wait.byte_size() + pixel.byte_size() + cancel_index.byte_size() + entries.byte_size(),
        }
    }

    fn write<W: Write>(&self, writer: &mut W) -> Result<(), io::Error> {
        match self {
            MultiPdtCmd::Slideshow(pos, wait, entries) => {
                (0x03u8).write(writer)?;
                (entries.len() as u8).write(writer)?;
                pos.write(writer)?;
                wait.write(writer)?;
                entries.write(writer)
            },
            MultiPdtCmd::SlideshowLoop(pos, wait, entries) => {
                (0x04u8).write(writer)?;
                (entries.len() as u8).write(writer)?;
                pos.write(writer)?;
                wait.write(writer)?;
                entries.write(writer)
            },
            MultiPdtCmd::StopSlideshowLoop => (0x05u8).write(writer),
            MultiPdtCmd::Scroll(poscmd, pos, wait, pixel, entries) => {
                (0x10u8).write(writer)?;
                poscmd.write(writer)?;
                (entries.len() as u8).write(writer)?;
                pos.write(writer)?;
                wait.write(writer)?;
                pixel.write(writer)?;
                entries.write(writer)
            },
            MultiPdtCmd::Scroll2(poscmd, pos, wait, pixel, entries) => {
                (0x20u8).write(writer)?;
                poscmd.write(writer)?;
                (entries.len() as u8).write(writer)?;
                pos.write(writer)?;
                wait.write(writer)?;
                pixel.write(writer)?;
                entries.write(writer)
            },
            MultiPdtCmd::ScrollWithCancel(poscmd, pos, wait, pixel, cancel_index, entries) => {
                (0x30u8).write(writer)?;
                poscmd.write(writer)?;
                (entries.len() as u8).write(writer)?;
                pos.write(writer)?;
                wait.write(writer)?;
                pixel.write(writer)?;
                cancel_index.write(writer)?;
                entries.write(writer)
            },
        }
    }
}

impl Writeable for SystemCmd {
    fn byte_size(&self) -> usize {
        match self {
            SystemCmd::LoadGame(a) => 1 + a.byte_size(),
            SystemCmd::SaveGame(a) => 1 + a.byte_size(),
            SystemCmd::SetTitle(a) => 1 + a.byte_size(),
            SystemCmd::MakePopup => 1,
            SystemCmd::GameEnd => 1,
            SystemCmd::GetSaveTitle(a, b) => 1 + a.byte_size() + b.byte_size(),
            SystemCmd::CheckSaveData(a, b) => 1 + a.byte_size() + b.byte_size(),
            SystemCmd::Unknown1(a, b) => 1 + a.byte_size() + b.byte_size(),
            SystemCmd::Unknown2(a, b) => 1 + a.byte_size() + b.byte_size(),
            SystemCmd::Unknown3(a, b) => 1 + a.byte_size() + b.byte_size(),
        }
    }

    fn write<W: Write>(&self, writer: &mut W) -> Result<(), io::Error> {
        match self {
            SystemCmd::LoadGame(a) => {
                (0x02u8).write(writer)?;
                a.write(writer)
            },
            SystemCmd::SaveGame(a) => {
                (0x03u8).write(writer)?;
                a.write(writer)
            },
            SystemCmd::SetTitle(a) => {
                (0x04u8).write(writer)?;
                a.write(writer)
            },
            SystemCmd::MakePopup => (0x05u8).write(writer),
            SystemCmd::GameEnd => (0x20u8).write(writer),
            SystemCmd::GetSaveTitle(a, b) => {
                (0x30u8).write(writer)?;
                a.write(writer)?;
                b.write(writer)
            },
            SystemCmd::CheckSaveData(a, b) => {
                (0x31u8).write(writer)?;
                a.write(writer)?;
                b.write(writer)
            },
            SystemCmd::Unknown1(a, b) => {
                (0x35u8).write(writer)?;
                a.write(writer)?;
                b.write(writer)
            },
            SystemCmd::Unknown2(a, b) => {
                (0x36u8).write(writer)?;
                a.write(writer)?;
                b.write(writer)
            },
            SystemCmd::Unknown3(a, b) => {
                (0x37u8).write(writer)?;
                a.write(writer)?;
                b.write(writer)
            },
        }
    }
}

impl Writeable for NameInputItem {
    fn byte_size(&self) -> usize {
        self.idx.byte_size() + self.text.byte_size()
    }

    fn write<W: Write>(&self, writer: &mut W) -> Result<(), io::Error> {
        self.idx.write(writer)?;
        self.text.write(writer)
    }
}

impl Writeable for NameCmd {
    fn byte_size(&self) -> usize {
        match self {
            NameCmd::InputBox(x, y, ex, ey, r, g, b, br, bg, bb) => 1 + x.byte_size() + y.byte_size() + ex.byte_size() + ey.byte_size() + r.byte_size() + g.byte_size() + b.byte_size() + br.byte_size() + bg.byte_size() + bb.byte_size(),
            NameCmd::InputBoxFinish(idx) => 1 + idx.byte_size(),
            NameCmd::InputBoxStart(idx) => 1 + idx.byte_size(),
            NameCmd::InputBoxClose(idx) => 1 + idx.byte_size(),
            NameCmd::GetName(idx, text) => 1 + idx.byte_size() + text.byte_size(),
            NameCmd::SetName(idx, text) => 1 + idx.byte_size() + text.byte_size(),
            NameCmd::GetName2(idx, text) => 1 + idx.byte_size() + text.byte_size(),
            NameCmd::NameInputDialog(idx) => 1 + idx.byte_size(),
            NameCmd::Unknown1(idx, text, a, b, c, d, e, f, g, h, i) => idx.byte_size() + text.byte_size() + a.byte_size() + b.byte_size() + c.byte_size() + d.byte_size() + e.byte_size() + f.byte_size() + g.byte_size() + h.byte_size() + i.byte_size(),
            NameCmd::NameInputDialogMulti(items) => 1 + mem::size_of::<u8>() + items.byte_size(),
            NameCmd::Unknown2 => 1,
            NameCmd::Unknown3 => 1
        }
    }

    fn write<W: Write>(&self, writer: &mut W) -> Result<(), io::Error> {
        match self {
            NameCmd::InputBox(x, y, ex, ey, r, g, b, br, bg, bb) => {
                (0x01u8).write(writer)?;
                x.write(writer)?;
                y.write(writer)?;
                ex.write(writer)?;
                ey.write(writer)?;
                r.write(writer)?;
                g.write(writer)?;
                b.write(writer)?;
                br.write(writer)?;
                bg.write(writer)?;
                bb.write(writer)
            },
            NameCmd::InputBoxFinish(idx) => {
                (0x02u8).write(writer)?;
                idx.write(writer)
            },
            NameCmd::InputBoxStart(idx) => {
                (0x03u8).write(writer)?;
                idx.write(writer)
            },
            NameCmd::InputBoxClose(idx) => {
                (0x04u8).write(writer)?;
                idx.write(writer)
            },
            NameCmd::GetName(idx, text) => {
                (0x10u8).write(writer)?;
                idx.write(writer)?;
                text.write(writer)
            },
            NameCmd::SetName(idx, text) => {
                (0x11u8).write(writer)?;
                idx.write(writer)?;
                text.write(writer)
            },
            NameCmd::GetName2(idx, text) => {
                (0x12u8).write(writer)?;
                idx.write(writer)?;
                text.write(writer)
            },
            NameCmd::NameInputDialog(idx) => {
                (0x20u8).write(writer)?;
                idx.write(writer)
            },
            NameCmd::Unknown1(idx, text, a, b, c, d, e, f, g, h, i) => {
                (0x21u8).write(writer)?;
                idx.write(writer)?;
                text.write(writer)?;
                a.write(writer)?;
                b.write(writer)?;
                c.write(writer)?;
                d.write(writer)?;
                e.write(writer)?;
                f.write(writer)?;
                g.write(writer)?;
                h.write(writer)?;
                i.write(writer)
            },
            NameCmd::NameInputDialogMulti(items) => {
                (0x24u8).write(writer)?;
                (items.len() as u8).write(writer)?;
                items.write(writer)
            },
            NameCmd::Unknown2 => (0x30u8).write(writer),
            NameCmd::Unknown3 => (0x31u8).write(writer)
        }
    }
}

impl Writeable for AreaBufferCmd {
    fn byte_size(&self) -> usize {
        match self {
            AreaBufferCmd::ReadCurArd(cur, ard) => 1 + cur.byte_size() + ard.byte_size(),
            AreaBufferCmd::Init => 1,
            AreaBufferCmd::GetClickedArea(val, click) => 1 + val.byte_size() + click.byte_size(),
            AreaBufferCmd::GetClickedArea2(val, click) => 1 + val.byte_size() + click.byte_size(),
            AreaBufferCmd::DisableArea(area) => 1 + area.byte_size(),
            AreaBufferCmd::EnableArea(area) => 1 + area.byte_size(),
            AreaBufferCmd::GetArea(x, y, area) => 1 + x.byte_size() + y.byte_size() + area.byte_size(),
            AreaBufferCmd::AssignArea(area_from, area_to) => 1 + area_from.byte_size() + area_to.byte_size()
        }
    }

    fn write<W: Write>(&self, writer: &mut W) -> Result<(), io::Error> {
        match self {
            AreaBufferCmd::ReadCurArd(cur, ard) => {
                (0x02u8).write(writer)?;
                cur.write(writer)?;
                ard.write(writer)
            },
            AreaBufferCmd::Init => (0x03u8).write(writer),
            AreaBufferCmd::GetClickedArea(val, click) => {
                (0x04u8).write(writer)?;
                val.write(writer)?;
                click.write(writer)
            },
            AreaBufferCmd::GetClickedArea2(val, click) => {
                (0x05u8).write(writer)?;
                val.write(writer)?;
                click.write(writer)
            },
            AreaBufferCmd::DisableArea(area) => {
                (0x10u8).write(writer)?;
                area.write(writer)
            },
            AreaBufferCmd::EnableArea(area) => {
                (0x11u8).write(writer)?;
                area.write(writer)
            },
            AreaBufferCmd::GetArea(x, y, area) => {
                (0x15u8).write(writer)?;
                x.write(writer)?;
                y.write(writer)?;
                area.write(writer)
            },
            AreaBufferCmd::AssignArea(area_from, area_to) => {
                (0x20u8).write(writer)?;
                area_from.write(writer)?;
                area_to.write(writer)
            }
        }
    }
}

impl Writeable for MouseCtrlCmd {
    fn byte_size(&self) -> usize {
        match self {
            MouseCtrlCmd::WaitForClick => 1,
            MouseCtrlCmd::SetPos(a, b, c) => 1 + a.byte_size() + b.byte_size() + c.byte_size(),
            MouseCtrlCmd::FlushClickData => 1,
            MouseCtrlCmd::CursorOff => 1,
            MouseCtrlCmd::CursorOn => 1
        }
    }

    fn write<W: Write>(&self, writer: &mut W) -> Result<(), io::Error> {
        match self {
            MouseCtrlCmd::WaitForClick => (0x01u8).write(writer),
            MouseCtrlCmd::SetPos(a, b, c) => {
                (0x02u8).write(writer)?;
                a.write(writer)?;
                b.write(writer)?;
                c.write(writer)
            },
            MouseCtrlCmd::FlushClickData => (0x03u8).write(writer),
            MouseCtrlCmd::CursorOff => (0x20u8).write(writer),
            MouseCtrlCmd::CursorOn => (0x21u8).write(writer)
        }
    }
}

impl Writeable for VolumeCmd {
    fn byte_size(&self) -> usize {
        match self {
            VolumeCmd::GetBgmVolume(a) => 1 + a.byte_size(),
            VolumeCmd::GetWavVolume(a) => 1 + a.byte_size(),
            VolumeCmd::GetKoeVolume(a) => 1 + a.byte_size(),
            VolumeCmd::GetSeVolume(a) => 1 + a.byte_size(),
            VolumeCmd::SetBgmVolume(a) => 1 + a.byte_size(),
            VolumeCmd::SetWavVolume(a) => 1 + a.byte_size(),
            VolumeCmd::SetKoeVolume(a) => 1 + a.byte_size(),
            VolumeCmd::SetSeVolume(a) => 1 + a.byte_size(),
            VolumeCmd::MuteBgm(a) => 1 + a.byte_size(),
            VolumeCmd::MuteWav(a) => 1 + a.byte_size(),
            VolumeCmd::MuteKoe(a) => 1 + a.byte_size(),
            VolumeCmd::MuteSe(a) => 1 + a.byte_size(),
        }
    }

    fn write<W: Write>(&self, writer: &mut W) -> Result<(), io::Error> {
        match self {
            VolumeCmd::GetBgmVolume(a) => {
                (0x01u8).write(writer)?;
                a.write(writer)
            },
            VolumeCmd::GetWavVolume(a) => {
                (0x02u8).write(writer)?;
                a.write(writer)
            },
            VolumeCmd::GetKoeVolume(a) => {
                (0x03u8).write(writer)?;
                a.write(writer)
            },
            VolumeCmd::GetSeVolume(a) => {
                (0x04u8).write(writer)?;
                a.write(writer)
            },
            VolumeCmd::SetBgmVolume(a) => {
                (0x11u8).write(writer)?;
                a.write(writer)
            },
            VolumeCmd::SetWavVolume(a) => {
                (0x12u8).write(writer)?;
                a.write(writer)
            },
            VolumeCmd::SetKoeVolume(a) => {
                (0x13u8).write(writer)?;
                a.write(writer)
            },
            VolumeCmd::SetSeVolume(a) => {
                (0x14u8).write(writer)?;
                a.write(writer)
            },
            VolumeCmd::MuteBgm(a) => {
                (0x21u8).write(writer)?;
                a.write(writer)
            },
            VolumeCmd::MuteWav(a) => {
                (0x22u8).write(writer)?;
                a.write(writer)
            },
            VolumeCmd::MuteKoe(a) => {
                (0x23u8).write(writer)?;
                a.write(writer)
            },
            VolumeCmd::MuteSe(a) => {
                (0x24u8).write(writer)?;
                a.write(writer)
            },
        }
    }
}

impl Writeable for NovelModeCmd {
    fn byte_size(&self) -> usize {
        match self {
            NovelModeCmd::SetEnabled(a) => 1 + a.byte_size(),
            NovelModeCmd::Unknown1(a) => 1 + a.byte_size(),
            NovelModeCmd::Unknown2 => 1,
            NovelModeCmd::Unknown3 => 1,
            NovelModeCmd::Unknown4 => 1
        }
    }

    fn write<W: Write>(&self, writer: &mut W) -> Result<(), io::Error> {
        match self {
            NovelModeCmd::SetEnabled(a) => {
                (0x01u8).write(writer)?;
                a.write(writer)
            },
            NovelModeCmd::Unknown1(a) => {
                (0x02u8).write(writer)?;
                a.write(writer)
            },
            NovelModeCmd::Unknown2 => (0x03u8).write(writer),
            NovelModeCmd::Unknown3 => (0x04u8).write(writer),
            NovelModeCmd::Unknown4 => (0x05u8).write(writer)
        }
    }
}

impl Writeable for WindowVarCmd {
    fn byte_size(&self) -> usize {
        match self {
            WindowVarCmd::GetBgFlagColor(attr, r, g, b) => 1 + attr.byte_size() + r.byte_size() + g.byte_size() + b.byte_size(),
            WindowVarCmd::SetBgFlagColor(attr, r, g, b) => 1 + attr.byte_size() + r.byte_size() + g.byte_size() + b.byte_size(),
            WindowVarCmd::GetWindowMove(a) => 1 + a.byte_size(),
            WindowVarCmd::SetWindowMove(a) => 1 + a.byte_size(),
            WindowVarCmd::GetWindowClearBox(a) => 1 + a.byte_size(),
            WindowVarCmd::SetWindowClearBox(a) => 1 + a.byte_size(),
            WindowVarCmd::GetWindowWaku(a) => 1 + a.byte_size(),
            WindowVarCmd::SetWindowWaku(a) => 1 + a.byte_size(),
        }
    }

    fn write<W: Write>(&self, writer: &mut W) -> Result<(), io::Error> {
        match self {
            WindowVarCmd::GetBgFlagColor(attr, r, g, b) => {
                (0x01u8).write(writer)?;
                attr.write(writer)?;
                r.write(writer)?;
                g.write(writer)?;
                b.write(writer)
            },
            WindowVarCmd::SetBgFlagColor(attr, r, g, b) => {
                (0x02u8).write(writer)?;
                attr.write(writer)?;
                r.write(writer)?;
                g.write(writer)?;
                b.write(writer)
            },
            WindowVarCmd::GetWindowMove(a) => {
                (0x03u8).write(writer)?;
                a.write(writer)
            },
            WindowVarCmd::SetWindowMove(a) => {
                (0x04u8).write(writer)?;
                a.write(writer)
            },
            WindowVarCmd::GetWindowClearBox(a) => {
                (0x05u8).write(writer)?;
                a.write(writer)
            },
            WindowVarCmd::SetWindowClearBox(a) => {
                (0x06u8).write(writer)?;
                a.write(writer)
            },
            WindowVarCmd::GetWindowWaku(a) => {
                (0x10u8).write(writer)?;
                a.write(writer)
            },
            WindowVarCmd::SetWindowWaku(a) => {
                (0x11u8).write(writer)?;
                a.write(writer)
            },
        }
    }
}

impl Writeable for MessageWinCmd {
    fn byte_size(&self) -> usize {
        match self {
            MessageWinCmd::GetWindowMsgPos(x, y) => 1 + x.byte_size() + y.byte_size(),
            MessageWinCmd::GetWindowComPos(x, y) => 1 + x.byte_size() + y.byte_size(),
            MessageWinCmd::GetWindowSysPos(x, y) => 1 + x.byte_size() + y.byte_size(),
            MessageWinCmd::GetWindowSubPos(x, y) => 1 + x.byte_size() + y.byte_size(),
            MessageWinCmd::GetWindowGrpPos(x, y) => 1 + x.byte_size() + y.byte_size(),
            MessageWinCmd::SetWindowMsgPos(x, y) => 1 + x.byte_size() + y.byte_size(),
            MessageWinCmd::SetWindowComPos(x, y) => 1 + x.byte_size() + y.byte_size(),
            MessageWinCmd::SetWindowSysPos(x, y) => 1 + x.byte_size() + y.byte_size(),
            MessageWinCmd::SetWindowSubPos(x, y) => 1 + x.byte_size() + y.byte_size(),
            MessageWinCmd::SetWindowGrpPos(x, y) => 1 + x.byte_size() + y.byte_size(),
        }
    }

    fn write<W: Write>(&self, writer: &mut W) -> Result<(), io::Error> {
        match self {
            MessageWinCmd::GetWindowMsgPos(x, y) => {
                (0x01u8).write(writer)?;
                x.write(writer)?;
                y.write(writer)
            },
            MessageWinCmd::GetWindowComPos(x, y) => {
                (0x02u8).write(writer)?;
                x.write(writer)?;
                y.write(writer)
            },
            MessageWinCmd::GetWindowSysPos(x, y) => {
                (0x03u8).write(writer)?;
                x.write(writer)?;
                y.write(writer)
            },
            MessageWinCmd::GetWindowSubPos(x, y) => {
                (0x04u8).write(writer)?;
                x.write(writer)?;
                y.write(writer)
            },
            MessageWinCmd::GetWindowGrpPos(x, y) => {
                (0x05u8).write(writer)?;
                x.write(writer)?;
                y.write(writer)
            },
            MessageWinCmd::SetWindowMsgPos(x, y) => {
                (0x11u8).write(writer)?;
                x.write(writer)?;
                y.write(writer)
            },
            MessageWinCmd::SetWindowComPos(x, y) => {
                (0x12u8).write(writer)?;
                x.write(writer)?;
                y.write(writer)
            },
            MessageWinCmd::SetWindowSysPos(x, y) => {
                (0x13u8).write(writer)?;
                x.write(writer)?;
                y.write(writer)
            },
            MessageWinCmd::SetWindowSubPos(x, y) => {
                (0x14u8).write(writer)?;
                x.write(writer)?;
                y.write(writer)
            },
            MessageWinCmd::SetWindowGrpPos(x, y) => {
                (0x15u8).write(writer)?;
                x.write(writer)?;
                y.write(writer)
            },
        }
    }
}

impl Writeable for SystemVarCmd {
    fn byte_size(&self) -> usize {
        match self {
            SystemVarCmd::GetMessageSize(a, b) => 1 + a.byte_size() + b.byte_size(),
            SystemVarCmd::SetMessageSize(a, b) => 1 + a.byte_size() + b.byte_size(),
            SystemVarCmd::GetMsgMojiSize(a, b) => 1 + a.byte_size() + b.byte_size(),
            SystemVarCmd::SetMsgMojiSize(a, b) => 1 + a.byte_size() + b.byte_size(),
            SystemVarCmd::GetMojiColor(a) => 1 + a.byte_size(),
            SystemVarCmd::SetMojiColor(a) => 1 + a.byte_size(),
            SystemVarCmd::GetMsgCancel(a) => 1 + a.byte_size(),
            SystemVarCmd::SetMsgCancel(a) => 1 + a.byte_size(),
            SystemVarCmd::GetMojiKage(a) => 1 + a.byte_size(),
            SystemVarCmd::SetMojiKage(a) => 1 + a.byte_size(),
            SystemVarCmd::GetKageColor(a) => 1 + a.byte_size(),
            SystemVarCmd::SetKageColor(a) => 1 + a.byte_size(),
            SystemVarCmd::GetSelCancel(a) => 1 + a.byte_size(),
            SystemVarCmd::SetSelCancel(a) => 1 + a.byte_size(),
            SystemVarCmd::GetCtrlKey(a) => 1 + a.byte_size(),
            SystemVarCmd::SetCtrlKey(a) => 1 + a.byte_size(),
            SystemVarCmd::GetSaveStart(a) => 1 + a.byte_size(),
            SystemVarCmd::SetSaveStart(a) => 1 + a.byte_size(),
            SystemVarCmd::GetDisableNvlTextFlag(a) => 1 + a.byte_size(),
            SystemVarCmd::SetDisableNvlTextFlag(a) => 1 + a.byte_size(),
            SystemVarCmd::GetFadeTime(a) => 1 + a.byte_size(),
            SystemVarCmd::SetFadeTime(a) => 1 + a.byte_size(),
            SystemVarCmd::GetCursorMono(a) => 1 + a.byte_size(),
            SystemVarCmd::SetCursorMono(a) => 1 + a.byte_size(),
            SystemVarCmd::GetCopyWindSw(a) => 1 + a.byte_size(),
            SystemVarCmd::SetCopyWindSw(a) => 1 + a.byte_size(),
            SystemVarCmd::GetMsgSpeed(a) => 1 + a.byte_size(),
            SystemVarCmd::SetMsgSpeed(a) => 1 + a.byte_size(),
            SystemVarCmd::GetMsgSpeed2(a) => 1 + a.byte_size(),
            SystemVarCmd::SetMsgSpeed2(a) => 1 + a.byte_size(),
            SystemVarCmd::GetReturnKeyWait(a) => 1 + a.byte_size(),
            SystemVarCmd::SetReturnKeyWait(a) => 1 + a.byte_size(),
            SystemVarCmd::GetKoeTextType(a) => 1 + a.byte_size(),
            SystemVarCmd::SetKoeTextType(a) => 1 + a.byte_size(),
            SystemVarCmd::GetGameSpeckInit(a) => 1 + a.byte_size(),
            SystemVarCmd::SetCursorPosition(a, b) => 1 + a.byte_size() + b.byte_size(),
            SystemVarCmd::SetDisableKeyMouseFlag(a) => 1 + a.byte_size(),
            SystemVarCmd::GetGameSpeckInit2(a) => 1 + a.byte_size(),
            SystemVarCmd::SetGameSpeckInit(a) => 1 + a.byte_size(),
        }
    }

    fn write<W: Write>(&self, writer: &mut W) -> Result<(), io::Error> {
        match self {
            SystemVarCmd::GetMessageSize(a, b) => {
                (0x01u8).write(writer)?;
                a.write(writer)?;
                b.write(writer)
            },
            SystemVarCmd::SetMessageSize(a, b) => {
                (0x02u8).write(writer)?;
                a.write(writer)?;
                b.write(writer)
            },
            SystemVarCmd::GetMsgMojiSize(a, b) => {
                (0x04u8).write(writer)?;
                a.write(writer)?;
                b.write(writer)
            },
            SystemVarCmd::SetMsgMojiSize(a, b) => {
                (0x06u8).write(writer)?;
                a.write(writer)?;
                b.write(writer)
            },
            SystemVarCmd::GetMojiColor(a) => {
                (0x10u8).write(writer)?;
                a.write(writer)
            },
            SystemVarCmd::SetMojiColor(a) => {
                (0x11u8).write(writer)?;
                a.write(writer)
            },
            SystemVarCmd::GetMsgCancel(a) => {
                (0x12u8).write(writer)?;
                a.write(writer)
            },
            SystemVarCmd::SetMsgCancel(a) => {
                (0x13u8).write(writer)?;
                a.write(writer)
            },
            SystemVarCmd::GetMojiKage(a) => {
                (0x16u8).write(writer)?;
                a.write(writer)
            },
            SystemVarCmd::SetMojiKage(a) => {
                (0x17u8).write(writer)?;
                a.write(writer)
            },
            SystemVarCmd::GetKageColor(a) => {
                (0x18u8).write(writer)?;
                a.write(writer)
            },
            SystemVarCmd::SetKageColor(a) => {
                (0x19u8).write(writer)?;
                a.write(writer)
            },
            SystemVarCmd::GetSelCancel(a) => {
                (0x1au8).write(writer)?;
                a.write(writer)
            },
            SystemVarCmd::SetSelCancel(a) => {
                (0x1bu8).write(writer)?;
                a.write(writer)
            },
            SystemVarCmd::GetCtrlKey(a) => {
                (0x1cu8).write(writer)?;
                a.write(writer)
            },
            SystemVarCmd::SetCtrlKey(a) => {
                (0x1du8).write(writer)?;
                a.write(writer)
            },
            SystemVarCmd::GetSaveStart(a) => {
                (0x1eu8).write(writer)?;
                a.write(writer)
            },
            SystemVarCmd::SetSaveStart(a) => {
                (0x1fu8).write(writer)?;
                a.write(writer)
            },
            SystemVarCmd::GetDisableNvlTextFlag(a) => {
                (0x20u8).write(writer)?;
                a.write(writer)
            },
            SystemVarCmd::SetDisableNvlTextFlag(a) => {
                (0x21u8).write(writer)?;
                a.write(writer)
            },
            SystemVarCmd::GetFadeTime(a) => {
                (0x22u8).write(writer)?;
                a.write(writer)
            },
            SystemVarCmd::SetFadeTime(a) => {
                (0x23u8).write(writer)?;
                a.write(writer)
            },
            SystemVarCmd::GetCursorMono(a) => {
                (0x24u8).write(writer)?;
                a.write(writer)
            },
            SystemVarCmd::SetCursorMono(a) => {
                (0x25u8).write(writer)?;
                a.write(writer)
            },
            SystemVarCmd::GetCopyWindSw(a) => {
                (0x26u8).write(writer)?;
                a.write(writer)
            },
            SystemVarCmd::SetCopyWindSw(a) => {
                (0x27u8).write(writer)?;
                a.write(writer)
            },
            SystemVarCmd::GetMsgSpeed(a) => {
                (0x28u8).write(writer)?;
                a.write(writer)
            },
            SystemVarCmd::SetMsgSpeed(a) => {
                (0x29u8).write(writer)?;
                a.write(writer)
            },
            SystemVarCmd::GetMsgSpeed2(a) => {
                (0x2au8).write(writer)?;
                a.write(writer)
            },
            SystemVarCmd::SetMsgSpeed2(a) => {
                (0x2bu8).write(writer)?;
                a.write(writer)
            },
            SystemVarCmd::GetReturnKeyWait(a) => {
                (0x2cu8).write(writer)?;
                a.write(writer)
            },
            SystemVarCmd::SetReturnKeyWait(a) => {
                (0x2du8).write(writer)?;
                a.write(writer)
            },
            SystemVarCmd::GetKoeTextType(a) => {
                (0x2eu8).write(writer)?;
                a.write(writer)
            },
            SystemVarCmd::SetKoeTextType(a) => {
                (0x2fu8).write(writer)?;
                a.write(writer)
            },
            SystemVarCmd::GetGameSpeckInit(a) => {
                (0x30u8).write(writer)?;
                a.write(writer)
            },
            SystemVarCmd::SetCursorPosition(a, b) => {
                (0x31u8).write(writer)?;
                a.write(writer)?;
                b.write(writer)
            },
            SystemVarCmd::SetDisableKeyMouseFlag(a) => {
                (0x32u8).write(writer)?;
                a.write(writer)
            },
            SystemVarCmd::GetGameSpeckInit2(a) => {
                (0x33u8).write(writer)?;
                a.write(writer)
            },
            SystemVarCmd::SetGameSpeckInit(a) => {
                (0x34u8).write(writer)?;
                a.write(writer)
            },
        }
    }
}

impl Writeable for PopupMenuCmd {
    fn byte_size(&self) -> usize {
        match self {
            PopupMenuCmd::GetMenuDisabled(val) => 1 + val.byte_size(),
            PopupMenuCmd::SetMenuDisabled(val) => 1 + val.byte_size(),
            PopupMenuCmd::GetItemDisabled(item_idx, val) => 1 + item_idx.byte_size() + val.byte_size(),
            PopupMenuCmd::SetItemDisabled(item_idx, val) => 1 + item_idx.byte_size() + val.byte_size(),
        }
    }

    fn write<W: Write>(&self, writer: &mut W) -> Result<(), io::Error> {
        match self {
            PopupMenuCmd::GetMenuDisabled(val) => {
                (0x01u8).write(writer)?;
                val.write(writer)
            },
            PopupMenuCmd::SetMenuDisabled(val) => {
                (0x02u8).write(writer)?;
                val.write(writer)
            },
            PopupMenuCmd::GetItemDisabled(item_idx, val) => {
                (0x03u8).write(writer)?;
                item_idx.write(writer)?;
                val.write(writer)
            },
            PopupMenuCmd::SetItemDisabled(item_idx, val) => {
                (0x04u8).write(writer)?;
                item_idx.write(writer)?;
                val.write(writer)
            },
        }
    }
}

impl Writeable for Opcode {
    fn byte_size(&self) -> usize {
        match self {
            Opcode::WaitMouse => 1,
            Opcode::Newline => 1,
            Opcode::WaitMouseText => 1,
            Opcode::TextWin(a) => 1 + a.byte_size(),
            Opcode::Op0x05 => 1,
            Opcode::Op0x06 => 1,
            Opcode::Op0x08 => 1,
            Opcode::Graphics(a) => 1 + a.byte_size(),
            Opcode::Op0x0c => 1,
            Opcode::Sound(a) => 1 + a.byte_size(),
            Opcode::DrawValText(a) => 1 + a.byte_size(),
            Opcode::Fade(a) => 1 + a.byte_size(),
            Opcode::Condition(a, b) => 1 + a.byte_size() + b.byte_size(),
            Opcode::JumpToScene(a) => 1 + a.byte_size(),
            Opcode::ScreenShake(a) => 1 + a.byte_size(),
            Opcode::Op0x18 => 1,
            Opcode::Wait(a) => 1 + a.byte_size(),
            Opcode::Op0x1a => 1,
            Opcode::Call(a) => 1 + a.byte_size(),
            Opcode::Jump(a) => 1 + a.byte_size(),
            Opcode::TableCall(a, b) => 1 + mem::size_of::<u8>() + a.byte_size() + b.byte_size(),
            Opcode::TableJump(a, b) => 1 + mem::size_of::<u8>() + a.byte_size() + b.byte_size(),
            Opcode::Return(a) => 1 + a.byte_size(),
            Opcode::Unknown0x22 => 1,
            Opcode::Unknown0x23 => 1,
            Opcode::Unknown0x24 => 1,
            Opcode::Unknown0x25 => 1,
            Opcode::Unknown0x26 => 1,
            Opcode::Unknown0x27 => 1,
            Opcode::Unknown0x28 => 1,
            Opcode::Unknown0x29 => 1,
            Opcode::Op0x2c => 1,
            Opcode::Op0x2d => 1,
            Opcode::ScenarioMenu(a) => 1 + a.byte_size(),
            Opcode::Op0x2f => 1,
            Opcode::Op0x30 => 1,
            Opcode::TextRank(a) => 1 + a.byte_size(),
            Opcode::SetFlag(a, b) => 1 + a.byte_size() + b.byte_size(),
            Opcode::CopyFlag(a, b) => 1 + a.byte_size() + b.byte_size(),
            Opcode::SetValLiteral(a, b) => 1 + a.byte_size() + b.byte_size(),
            Opcode::AddVal(a, b) => 1 + a.byte_size() + b.byte_size(),
            Opcode::SubVal(a, b) => 1 + a.byte_size() + b.byte_size(),
            Opcode::MulVal(a, b) => 1 + a.byte_size() + b.byte_size(),
            Opcode::DivVal(a, b) => 1 + a.byte_size() + b.byte_size(),
            Opcode::ModVal(a, b) => 1 + a.byte_size() + b.byte_size(),
            Opcode::AndVal(a, b) => 1 + a.byte_size() + b.byte_size(),
            Opcode::OrVal(a, b) => 1 + a.byte_size() + b.byte_size(),
            Opcode::XorVal(a, b) => 1 + a.byte_size() + b.byte_size(),
            Opcode::SetVal(a, b) => 1 + a.byte_size() + b.byte_size(),
            Opcode::AddValSelf(a, b) => 1 + a.byte_size() + b.byte_size(),
            Opcode::SubValSelf(a, b) => 1 + a.byte_size() + b.byte_size(),
            Opcode::MulValSelf(a, b) => 1 + a.byte_size() + b.byte_size(),
            Opcode::DivValSelf(a, b) => 1 + a.byte_size() + b.byte_size(),
            Opcode::ModValSelf(a, b) => 1 + a.byte_size() + b.byte_size(),
            Opcode::AndValSelf(a, b) => 1 + a.byte_size() + b.byte_size(),
            Opcode::OrValSelf(a, b) => 1 + a.byte_size() + b.byte_size(),
            Opcode::XorValSelf(a, b) => 1 + a.byte_size() + b.byte_size(),
            Opcode::SetFlagRandom(a) => 1 + a.byte_size(),
            Opcode::SetValRandom(a, b) => 1 + a.byte_size() + b.byte_size(),
            Opcode::Choice(a) => 1 + a.byte_size(),
            Opcode::String(a) => 1 + a.byte_size(),
            Opcode::Op0x5b => 1,
            Opcode::SetMulti(a) => 1 + a.byte_size(),
            Opcode::Op0x5d => 1,
            Opcode::Op0x5e => 1,
            Opcode::Op0x5f => 1,
            Opcode::System(a) => 1 + a.byte_size(),
            Opcode::Name(a) => 1 + a.byte_size(),
            Opcode::Op0x63 => 1,
            Opcode::BufferRegion(a) => 1 + a.byte_size(),
            Opcode::Unknown0x65 => 1,
            Opcode::Buffer(a) => 1 + a.byte_size(),
            Opcode::Flash(a) => 1 + a.byte_size(),
            Opcode::Op0x69 => 1,
            Opcode::MultiPdt(a) => 1 + a.byte_size(),
            Opcode::Op0x66 => 1,
            Opcode::AreaBuffer(a) => 1 + a.byte_size(),
            Opcode::MouseCtrl(a) => 1 + a.byte_size(),
            Opcode::Op0x6e => 1,
            Opcode::Op0x6f => 1,
            Opcode::WindowVar(a) => 1 + a.byte_size(),
            Opcode::MessageWin(a) => 1 + a.byte_size(),
            Opcode::SystemVar(a) => 1 + a.byte_size(),
            Opcode::PopupMenu(a) => 1 + a.byte_size(),
            Opcode::Volume(a) => 1 + a.byte_size(),
            Opcode::NovelMode(a) => 1 + a.byte_size(),
            Opcode::Op0x7f => 1,
            Opcode::Unknown0xea(a) => 1 + a.byte_size(),
            Opcode::TextHankaku(a, b) => 1 + a.byte_size() + b.byte_size(),
            Opcode::TextZenkaku(a, b) => 1 + a.byte_size() + b.byte_size(),
        }
    }

    fn write<W: Write>(&self, writer: &mut W) -> Result<(), io::Error> {
        match self {
            Opcode::WaitMouse => (0x01u8).write(writer),
            Opcode::Newline => (0x02u8).write(writer),
            Opcode::WaitMouseText => (0x03u8).write(writer),
            Opcode::TextWin(a) => {
                (0x04u8).write(writer)?;
                a.write(writer)
            },
            Opcode::Op0x05 => (0x05u8).write(writer),
            Opcode::Op0x06 => (0x06u8).write(writer),
            Opcode::Op0x08 => (0x08u8).write(writer),
            Opcode::Graphics(a) => {
                (0x0bu8).write(writer)?;
                a.write(writer)
            },
            Opcode::Op0x0c => (0x0cu8).write(writer),
            Opcode::Sound(a) => {
                (0x0eu8).write(writer)?;
                a.write(writer)
            },
            Opcode::DrawValText(a) => {
                (0x10u8).write(writer)?;
                a.write(writer)
            },
            Opcode::Fade(a) => {
                (0x13u8).write(writer)?;
                a.write(writer)
            },
            Opcode::Condition(a, b) => {
                (0x15u8).write(writer)?;
                a.write(writer)?;
                b.write(writer)
            },
            Opcode::JumpToScene(a) => {
                (0x16u8).write(writer)?;
                a.write(writer)
            },
            Opcode::ScreenShake(a) => {
                (0x17u8).write(writer)?;
                a.write(writer)
            },
            Opcode::Op0x18 => (0x18u8).write(writer),
            Opcode::Wait(a) => {
                (0x19u8).write(writer)?;
                a.write(writer)
            },
            Opcode::Op0x1a => (0x1au8).write(writer),
            Opcode::Call(a) => {
                (0x1bu8).write(writer)?;
                a.write(writer)
            },
            Opcode::Jump(a) => {
                (0x1cu8).write(writer)?;
                a.write(writer)
            },
            Opcode::TableCall(a, b) => {
                (0x1du8).write(writer)?;
                (b.len() as u8).write(writer)?;
                a.write(writer)?;
                b.write(writer)
            },
            Opcode::TableJump(a, b) => {
                (0x1eu8).write(writer)?;
                (b.len() as u8).write(writer)?;
                a.write(writer)?;
                b.write(writer)
            },
            Opcode::Return(a) => {
                (0x20u8).write(writer)?;
                a.write(writer)
            },
            Opcode::Unknown0x22 => (0x22u8).write(writer),
            Opcode::Unknown0x23 => (0x23u8).write(writer),
            Opcode::Unknown0x24 => (0x24u8).write(writer),
            Opcode::Unknown0x25 => (0x25u8).write(writer),
            Opcode::Unknown0x26 => (0x26u8).write(writer),
            Opcode::Unknown0x27 => (0x27u8).write(writer),
            Opcode::Unknown0x28 => (0x28u8).write(writer),
            Opcode::Unknown0x29 => (0x29u8).write(writer),
            Opcode::Op0x2c => (0x2cu8).write(writer),
            Opcode::Op0x2d => (0x2du8).write(writer),
            Opcode::ScenarioMenu(a) => {
                (0x2eu8).write(writer)?;
                a.write(writer)
            },
            Opcode::Op0x2f => (0x2fu8).write(writer),
            Opcode::Op0x30 => (0x30u8).write(writer),
            Opcode::TextRank(a) => {
                (0x31u8).write(writer)?;
                a.write(writer)
            },
            Opcode::SetFlag(a, b) => {
                (0x37u8).write(writer)?;
                a.write(writer)?;
                b.write(writer)
            },
            Opcode::CopyFlag(a, b) => {
                (0x39u8).write(writer)?;
                a.write(writer)?;
                b.write(writer)
            },
            Opcode::SetValLiteral(a, b) => {
                (0x3bu8).write(writer)?;
                a.write(writer)?;
                b.write(writer)
            },
            Opcode::AddVal(a, b) => {
                (0x3cu8).write(writer)?;
                a.write(writer)?;
                b.write(writer)
            },
            Opcode::SubVal(a, b) => {
                (0x3du8).write(writer)?;
                a.write(writer)?;
                b.write(writer)
            },
            Opcode::MulVal(a, b) => {
                (0x3eu8).write(writer)?;
                a.write(writer)?;
                b.write(writer)
            },
            Opcode::DivVal(a, b) => {
                (0x3fu8).write(writer)?;
                a.write(writer)?;
                b.write(writer)
            },
            Opcode::ModVal(a, b) => {
                (0x40u8).write(writer)?;
                a.write(writer)?;
                b.write(writer)
            },
            Opcode::AndVal(a, b) => {
                (0x41u8).write(writer)?;
                a.write(writer)?;
                b.write(writer)
            },
            Opcode::OrVal(a, b) => {
                (0x42u8).write(writer)?;
                a.write(writer)?;
                b.write(writer)
            },
            Opcode::XorVal(a, b) => {
                (0x43u8).write(writer)?;
                a.write(writer)?;
                b.write(writer)
            },
            Opcode::SetVal(a, b) => {
                (0x49u8).write(writer)?;
                a.write(writer)?;
                b.write(writer)
            },
            Opcode::AddValSelf(a, b) => {
                (0x4au8).write(writer)?;
                a.write(writer)?;
                b.write(writer)
            },
            Opcode::SubValSelf(a, b) => {
                (0x4bu8).write(writer)?;
                a.write(writer)?;
                b.write(writer)
            },
            Opcode::MulValSelf(a, b) => {
                (0x4cu8).write(writer)?;
                a.write(writer)?;
                b.write(writer)
            },
            Opcode::DivValSelf(a, b) => {
                (0x4du8).write(writer)?;
                a.write(writer)?;
                b.write(writer)
            },
            Opcode::ModValSelf(a, b) => {
                (0x4eu8).write(writer)?;
                a.write(writer)?;
                b.write(writer)
            },
            Opcode::AndValSelf(a, b) => {
                (0x4fu8).write(writer)?;
                a.write(writer)?;
                b.write(writer)
            },
            Opcode::OrValSelf(a, b) => {
                (0x50u8).write(writer)?;
                a.write(writer)?;
                b.write(writer)
            },
            Opcode::XorValSelf(a, b) => {
                (0x51u8).write(writer)?;
                a.write(writer)?;
                b.write(writer)
            },
            Opcode::SetFlagRandom(a) => {
                (0x56u8).write(writer)?;
                a.write(writer)
            },
            Opcode::SetValRandom(a, b) => {
                (0x57u8).write(writer)?;
                a.write(writer)?;
                b.write(writer)
            },
            Opcode::Choice(a) => {
                (0x58u8).write(writer)?;
                a.write(writer)
            },
            Opcode::String(a) => {
                (0x59u8).write(writer)?;
                a.write(writer)
            },
            Opcode::Op0x5b => (0x5bu8).write(writer),
            Opcode::SetMulti(a) => {
                (0x5cu8).write(writer)?;
                a.write(writer)
            },
            Opcode::Op0x5d => (0x5du8).write(writer),
            Opcode::Op0x5e => (0x5eu8).write(writer),
            Opcode::Op0x5f => (0x5fu8).write(writer),
            Opcode::System(a) => {
                (0x60u8).write(writer)?;
                a.write(writer)
            },
            Opcode::Name(a) => {
                (0x61u8).write(writer)?;
                a.write(writer)
            },
            Opcode::Op0x63 => (0x63u8).write(writer),
            Opcode::BufferRegion(a) => {
                (0x64u8).write(writer)?;
                a.write(writer)
            },
            Opcode::Unknown0x65 => (0x65u8).write(writer),
            Opcode::Buffer(a) => {
                (0x67u8).write(writer)?;
                a.write(writer)
            },
            Opcode::Flash(a) => {
                (0x68u8).write(writer)?;
                a.write(writer)
            },
            Opcode::Op0x69 => (0x69u8).write(writer),
            Opcode::MultiPdt(a) => {
                (0x6au8).write(writer)?;
                a.write(writer)
            },
            Opcode::Op0x66 => (0x66u8).write(writer),
            Opcode::AreaBuffer(a) => {
                (0x6cu8).write(writer)?;
                a.write(writer)
            },
            Opcode::MouseCtrl(a) => {
                (0x6du8).write(writer)?;
                a.write(writer)
            },
            Opcode::Op0x6e => (0x6eu8).write(writer),
            Opcode::Op0x6f => (0x6fu8).write(writer),
            Opcode::WindowVar(a) => {
                (0x70u8).write(writer)?;
                a.write(writer)
            },
            Opcode::MessageWin(a) => {
                (0x72u8).write(writer)?;
                a.write(writer)
            },
            Opcode::SystemVar(a) => {
                (0x73u8).write(writer)?;
                a.write(writer)
            },
            Opcode::PopupMenu(a) => {
                (0x74u8).write(writer)?;
                a.write(writer)
            },
            Opcode::Volume(a) => {
                (0x75u8).write(writer)?;
                a.write(writer)
            },
            Opcode::NovelMode(a) => {
                (0x76u8).write(writer)?;
                a.write(writer)
            },
            Opcode::Op0x7f => (0x7fu8).write(writer),
            Opcode::Unknown0xea(a) => {
                (0xeau8).write(writer)?;
                a.write(writer)
            },
            Opcode::TextHankaku(a, b) => {
                (0xfeu8).write(writer)?;
                a.write(writer)?;
                b.write(writer)
            },
            Opcode::TextZenkaku(a, b) => {
                (0xffu8).write(writer)?;
                a.write(writer)?;
                b.write(writer)
            },
        }
    }
}

impl Writeable for AVG32Scene {
    fn byte_size(&self) -> usize {
        self.header.byte_size() + self.opcodes.byte_size() + 1 // \0
    }

    fn write<W: Write>(&self, writer: &mut W) -> Result<(), io::Error> {
        self.header.write(writer)?;
        self.opcodes.write(writer)?;
        writer.write_all(&[0x00])
    }
}

#[cfg(test)]
mod tests {
    use crate::parser;
    use super::*;
    use pretty_assertions::assert_eq;

    #[test]
    fn test_roundtrip_value() {
        let test = |bytes: &[u8]| {
            let mut writer = Vec::new();
            let val = parser::scene_value(bytes).unwrap().1;
            println!("{:?}", val);

            val.write(&mut writer).unwrap();

            assert_eq!(&bytes[..], &writer);
        };

        test(&[0x10]);
        test(&[0x1F]);
        test(&[0x91]);
        test(&[0x20, 0x80]);
        test(&[0x3F, 0x80, 0x40]);
        test(&[0x3F, 0xFF, 0xFF]);
        test(&[0x48, 0x9F, 0x7D, 0x0A]);
        test(&[0x4F, 0xFF, 0xFF, 0xFF]);
    }

    #[test]
    fn test_roundtrip_scene() {
        use std::fs;
        for entry in fs::read_dir("../SEEN").unwrap() {
            let entry = entry.unwrap();
            let path = entry.path();
            println!("{:?}", path);

            let metadata = fs::metadata(&path).unwrap();
            if metadata.is_file() {
                let mut out = Vec::new();
                let bytes = fs::read(&path.to_str().unwrap()).unwrap();
                let scene = parser::avg32_scene(&bytes).unwrap().1;

                scene.write(&mut out).unwrap();

                assert_eq!(&bytes[..], &out);
            }
        }
    }

    #[test]
    fn test_string_size() {
        assert_eq!(11, "あいうえお".byte_size());
    }
}
