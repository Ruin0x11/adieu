use nom::error::{ParseError, ErrorKind};
use nom::IResult;
use nom::number::streaming::{le_u8, le_u32};
use encoding_rs::SHIFT_JIS;

#[derive(Debug, PartialEq, Eq, Clone)]
pub enum CustomError<I> {
  MyError(String),
  Nom(I, ErrorKind),
}

impl<I> ParseError<I> for CustomError<I> {
  fn from_error_kind(input: I, kind: ErrorKind) -> Self {
    CustomError::Nom(input, kind)
  }

  fn append(_: I, _: ErrorKind, other: Self) -> Self {
    other
  }
}

type ParseResult<'a, I> = IResult<&'a [u8], I, CustomError<&'a [u8]>>;

// TODO
const SYS_VERSION: u32 = 1714;

fn sys_version_geq(min_ver: u32) -> bool {
    SYS_VERSION >= min_ver
}

#[derive(Serialize, Deserialize, Debug, PartialEq, Eq, Clone)]
pub struct AVG32Scene {
    pub header: Header,
    pub opcodes: Vec<Opcode>
}

#[derive(Serialize, Deserialize, Debug, PartialEq, Eq, Clone)]
pub struct Header {
    pub unk1: Vec<u8>,
    pub labels: Vec<u32>,
    pub unk2: Vec<u8>,
    pub counter_start: u32,
    pub menus: Vec<Menu>,
    pub menu_strings: Vec<String>,
    pub unk3: Vec<u8>,
}

named!(pub header<&[u8], Header, CustomError<&[u8]>>,
  do_parse!(
    tag!("TPC32") >>
    unk1: count!(le_u8, 0x13) >>
    label_count: le_u32 >>
    counter_start: le_u32 >>
    labels: count!(le_u32, label_count as usize) >>
    unk2: count!(le_u8, 0x30) >>
    menu_count: le_u32 >>
    menus: count!(menu, (menu_count) as usize) >>
    menu_strings: call!(menu_strings, &menus) >>
    unk3: count!(le_u8, 0x05) >>
    (Header {
        unk1: unk1,
        labels: labels,
        unk2: unk2,
        counter_start: counter_start,
        menus: menus,
        menu_strings: menu_strings,
        unk3: unk3
    })
  )
);

fn decode_sjis(input: &[u8]) -> Result<String, CustomError<&[u8]>> {
    let (res, _, errors) = SHIFT_JIS.decode(&input);
    if errors {
        Err(CustomError::MyError(String::from("Invalid SHIFT_JIS")))
    } else {
        Ok(res.to_string())
    }
}

named!(c_string<&[u8], String, CustomError<&[u8]>>,
    do_parse!(
        s: map_res!(take_until!("\0"), decode_sjis) >>
        tag!("\0") >>
        (s)
    )
);

fn menu_strings<'a, 'b>(input: &'a [u8], menus: &'b [Menu]) -> ParseResult<'a, Vec<String>> {
    let mut str_count = 0;
    for menu in menus {
        str_count = str_count + 1;
        for _ in menu.submenus.iter() {
            str_count = str_count + 1;
        }
    }

    nom::multi::count(c_string, str_count)(input)
}

#[derive(Serialize, Deserialize, Debug, PartialEq, Eq, Clone)]
pub struct Menu {
    pub id: u8,
    pub unk1: u8,
    pub unk2: u8,
    pub submenus: Vec<Submenu>
}

named!(pub menu<&[u8], Menu, CustomError<&[u8]>>,
    do_parse!(
        id: le_u8 >>
        submenu_count: le_u8 >>
            unk1: le_u8 >>
            unk2: le_u8 >>
        submenus: count!(submenu, submenu_count as usize) >>
        (Menu {
            id: id,
            unk1: unk1,
            unk2: unk2,
            submenus: submenus
        })
    )
);

#[derive(Serialize, Deserialize, Debug, PartialEq, Eq, Clone)]
pub struct Submenu {
    pub id: u8,
    pub unk1: u8,
    pub unk2: u8,
    pub flags: Vec<Flag>
}

named!(pub submenu<&[u8], Submenu, CustomError<&[u8]>>,
    do_parse!(
        id: le_u8 >>
        flag_count: le_u8 >>
            unk1: le_u8 >>
            unk2: le_u8 >>
        flags: count!(flag, flag_count as usize) >>
        (Submenu {
            id: id,
            unk1: unk1,
            unk2: unk2,
            flags: flags
        })
    )
);

#[derive(Serialize, Deserialize, Debug, PartialEq, Eq, Clone)]
pub struct Flag {
    pub unk1: u8,
    pub flags: Vec<u32>
}

named!(pub flag<&[u8], Flag, CustomError<&[u8]>>,
    do_parse!(
        flag_count: le_u8 >>
            unk1: le_u8 >>
        flags: count!(le_u32, flag_count as usize) >>
        (Flag {
            unk1: unk1,
            flags: flags
        })
    )
);

/// Byte position (jump, if, etc.)
#[derive(Serialize, Deserialize, Debug, PartialEq, Eq, PartialOrd, Ord, Clone)]
pub enum Pos {
    Offset(u32),
    Label(String)
}

impl From<u32> for Pos {
    fn from(i: u32) -> Self {
        Pos::Offset(i)
    }
}

named!(pub scene_pos<&[u8], Pos, CustomError<&[u8]>>,
       map!(le_u32, Pos::from)
);

#[derive(Serialize, Deserialize, Debug, PartialEq, Eq, PartialOrd, Ord, Clone, Copy)]
pub enum ValType {
    Const,
    Var
}

/// Literal value or variable index
#[derive(Serialize, Deserialize, Debug, PartialEq, Eq, PartialOrd, Ord, Clone, Copy)]
pub struct Val(pub u32, pub ValType);

pub fn scene_value(input: &[u8]) -> ParseResult<Val> {
    let num = input[0];
    let len = ((num >> 4) & 7) as usize;
    let is_var = num & 0x80 == 0x80;
    let kind = if is_var {
        ValType::Var
    } else {
        ValType::Const
    };
    let mut ret: u32 = 0;

    for i in (0..len-1).rev() {
        ret <<= 8;
        ret |= input[i+1] as u32;
    }

    ret <<= 4;
    ret |= (num & 0x0f) as u32;

    Ok((&input[len..], Val(ret, kind)))
}

#[derive(Serialize, Deserialize, Debug, PartialEq, Eq, Clone)]
pub enum SceneText {
    Pointer(Val),
    Literal(String)
}

fn scene_text(input: &[u8]) -> ParseResult<SceneText> {
    if input[0] == 0x40 {
        let (inp, val) = scene_value(input)?;
        Ok((inp, SceneText::Pointer(val)))
    } else {
        let (inp, val) = c_string(input)?;
        Ok((inp, SceneText::Literal(String::from(val))))
    }
}

#[derive(Serialize, Deserialize, Debug, PartialEq, Eq, Clone)]
pub enum FormattedTextCmd {
    Integer(Val), // 0x01
    IntegerZeroPadded(Val, Val), // 0x02
    TextPointer(Val), // 0x03
    Unknown1(Val), // 0x11
    Unknown2 // 0x13
}

named!(pub formatted_text_cmd<&[u8], FormattedTextCmd, CustomError<&[u8]>>,
    switch!(le_u8,
        0x01 => do_parse!(val: scene_value >> (FormattedTextCmd::Integer(val))) |
        0x02 => do_parse!(val: scene_value >> zeros: scene_value >> (FormattedTextCmd::IntegerZeroPadded(val, zeros))) |
        0x03 => do_parse!(val: scene_value >> (FormattedTextCmd::TextPointer(val))) |
        0x11 => do_parse!(val: scene_value >> (FormattedTextCmd::Unknown1(val))) |
        0x13 => value!(FormattedTextCmd::Unknown2)
    )
);

#[derive(Serialize, Deserialize, Debug, PartialEq, Eq, Clone)]
pub enum SceneFormattedTextEntry {
    Command(FormattedTextCmd), // 0x10
    Unknown, // 0x12
    Condition(Vec<Condition>), // 0x28
    TextPointer(Val), // 0xfd
    TextHankaku(String), // 0xfe
    TextZenkaku(String), // 0xff
}

named!(pub scene_formatted_text_entry<&[u8], SceneFormattedTextEntry, CustomError<&[u8]>>,
       switch!(le_u8,
               0x10 => do_parse!(a: formatted_text_cmd >> (SceneFormattedTextEntry::Command(a))) |
               0x12 => value!(SceneFormattedTextEntry::Unknown) |
               0x28 => do_parse!(a: scene_conditions >> (SceneFormattedTextEntry::Condition(a))) |
               0xfd => do_parse!(a: scene_value >> (SceneFormattedTextEntry::TextPointer(a))) |
               0xfe => do_parse!(a: c_string >> (SceneFormattedTextEntry::TextHankaku(a))) |
               0xff => do_parse!(a: c_string >> (SceneFormattedTextEntry::TextZenkaku(a)))
       )
);

#[derive(Serialize, Deserialize, Debug, PartialEq, Eq, Clone)]
pub struct SceneFormattedText(pub Vec<SceneFormattedTextEntry>);

named!(pub scene_formatted_text<&[u8], SceneFormattedText, CustomError<&[u8]>>,
    do_parse!(
        res: many_till!(scene_formatted_text_entry, tag!("\0")) >>
        (SceneFormattedText(res.0))
    )
);

//
// Opcode data
//

#[derive(Serialize, Deserialize, Debug, PartialEq, Eq, Clone)]
pub enum JumpToSceneCmd {
    Jump(Val), // 0x01
    Call(Val), // 0x02
}

named!(pub jump_to_scene_cmd<&[u8], JumpToSceneCmd, CustomError<&[u8]>>,
       switch!(le_u8,
               0x01 => do_parse!(a: scene_value >> (JumpToSceneCmd::Jump(a))) |
               0x02 => do_parse!(a: scene_value >> (JumpToSceneCmd::Call(a)))
        )
);

#[derive(Serialize, Deserialize, Debug, PartialEq, Eq, Clone)]
pub enum TextWinCmd {
    Hide, // 0x01
    HideEffect, // 0x02
    HideRedraw, // 0x03
    MouseWait, // 0x04
    ClearText // 0x05
}

named!(pub text_win_cmd<&[u8], TextWinCmd, CustomError<&[u8]>>,
    switch!(le_u8,
        0x01 => value!(TextWinCmd::Hide) |
        0x02 => value!(TextWinCmd::HideEffect) |
        0x03 => value!(TextWinCmd::HideRedraw) |
        0x04 => value!(TextWinCmd::MouseWait) |
        0x05 => value!(TextWinCmd::ClearText)
    )
);

#[derive(Serialize, Deserialize, Debug, PartialEq, Eq, Clone)]
pub enum FadeCmd {
    Fade(Val), // 0x01
    FadeTimed(Val, Val), // 0x02
    FadeColor(Val, Val, Val), // 0x03
    FadeTimedColor(Val, Val, Val, Val), // 0x04
    FillScreen(Val), // 0x10
    FillScreenColor(Val, Val, Val), // 0x11
}

named!(pub fade_cmd<&[u8], FadeCmd, CustomError<&[u8]>>,
    switch!(le_u8,
        0x01 => do_parse!(
            idx: scene_value >>
            (FadeCmd::Fade(idx))
        ) |
        0x02 => do_parse!(
            idx: scene_value >>
            fadestep: scene_value >>
            (FadeCmd::FadeTimed(idx, fadestep))
        ) |
        0x03 => do_parse!(
            r: scene_value >>
            g: scene_value >>
            b: scene_value >>
            (FadeCmd::FadeColor(r, g, b))
        ) |
        0x04 => do_parse!(
            r: scene_value >>
            g: scene_value >>
            b: scene_value >>
            fadestep: scene_value >>
            (FadeCmd::FadeTimedColor(r, g, b, fadestep))
        ) |
        0x10 => do_parse!(
            idx: scene_value >>
            (FadeCmd::FillScreen(idx))
        ) |
        0x11 => do_parse!(
            r: scene_value >>
            g: scene_value >>
            b: scene_value >>
            (FadeCmd::FillScreenColor(r, g, b))
        )
    )
);

#[derive(Serialize, Deserialize, Debug, PartialEq, Eq, Clone)]
pub struct GrpEffect {
    pub file: SceneText,
    pub sx1: Val,
    pub sy1: Val,
    pub sx2: Val,
    pub sy2: Val,
    pub dx: Val,
    pub dy: Val,
    pub steptime: Val,
    pub cmd: Val,
    pub mask: Val,
    pub arg1: Val,
    pub arg2: Val,
    pub arg3: Val,
    pub step: Val,
    pub arg5: Val,
    pub arg6: Val,
}

named!(pub grp_effect<&[u8], GrpEffect, CustomError<&[u8]>>,
       do_parse!(
           file: scene_text >>
               sx1: scene_value >>
               sy1: scene_value >>
               sx2: scene_value >>
               sy2: scene_value >>
               dx: scene_value >>
               dy: scene_value >>
               steptime: scene_value >>
               cmd: scene_value >>
               mask: scene_value >>
               arg1: scene_value >>
               arg2: scene_value >>
               arg3: scene_value >>
               step: scene_value >>
               arg5: scene_value >>
               arg6: scene_value >>
               (GrpEffect {
                   file: file,
                   sx1: sx1,
                   sy1: sy1,
                   sx2: sx2,
                   sy2: sy2,
                   dx: dx,
                   dy: dy,
                   steptime: steptime,
                   cmd: cmd,
                   mask: mask,
                   arg1: arg1,
                   arg2: arg2,
                   arg3: arg3,
                   step: step,
                   arg5: arg5,
                   arg6: arg6,
               })
       )
);

#[derive(Serialize, Deserialize, Debug, PartialEq, Eq, Clone)]
pub enum GrpCompositeMethod {
    Corner, // 0x01
    Copy(Val), // 0x02
    Move1(Val, Val, Val, Val, Val, Val), // 0x03
    Move2(Val, Val, Val, Val, Val, Val, Val) // 0x04
}

#[derive(Serialize, Deserialize, Debug, PartialEq, Eq, Clone)]
pub struct GrpCompositeChild {
    pub file: SceneText,
    pub method: GrpCompositeMethod
}

fn grp_composite_child(input: &[u8]) -> ParseResult<GrpCompositeChild> {
    let mut inp = input;
    let (i, idx) = le_u8(inp)?;
    inp = i;
    let (i, file) = scene_text(inp)?;
    inp = i;

    let method = match idx {
        0x01 => GrpCompositeMethod::Corner,
        0x02 => {
            let (i, val) = scene_value(inp)?;
            inp = i;

            GrpCompositeMethod::Copy(val)
        },
        0x03 => {
            let (i, srcx1) = scene_value(inp)?;
            let (i, srcy1) = scene_value(i)?;
            let (i, srcx2) = scene_value(i)?;
            let (i, srcy2) = scene_value(i)?;
            let (i, dstx1) = scene_value(i)?;
            let (i, dsty1) = scene_value(i)?;
            inp = i;

            GrpCompositeMethod::Move1(srcx1, srcy1, srcx2, srcy2, dstx1, dsty1)
        },
        0x04 => {
            let (i, srcx1) = scene_value(inp)?;
            let (i, srcy1) = scene_value(i)?;
            let (i, srcx2) = scene_value(i)?;
            let (i, srcy2) = scene_value(i)?;
            let (i, dstx1) = scene_value(i)?;
            let (i, dsty1) = scene_value(i)?;
            let (i, arg) = scene_value(i)?;
            inp = i;

            GrpCompositeMethod::Move2(srcx1, srcy1, srcx2, srcy2, dstx1, dsty1, arg)
        },
        _ => return Err(nom::Err::Error(CustomError::MyError(format!("Unknown {}", idx))))
    };

    let child = GrpCompositeChild {
        file: file,
        method: method
    };

    Ok((inp, child))
}

#[derive(Serialize, Deserialize, Debug, PartialEq, Eq, Clone)]
pub struct GrpComposite {
    pub base_file: SceneText,
    pub idx: Val,
    pub children: Vec<GrpCompositeChild>
}

named!(pub grp_composite<&[u8], GrpComposite, CustomError<&[u8]>>,
       do_parse!(
           count: le_u8 >>
               base_file: scene_text >>
               idx: scene_value >>
               children: count!(grp_composite_child, count as usize) >>
               (GrpComposite {
                   base_file: base_file,
                   idx: idx,
                   children: children
               })
       )
);

#[derive(Serialize, Deserialize, Debug, PartialEq, Eq, Clone)]
pub struct GrpCompositeIndexed {
    pub base_file: Val,
    pub idx: Val,
    pub children: Vec<GrpCompositeChild>
}

named!(pub grp_composite_indexed<&[u8], GrpCompositeIndexed, CustomError<&[u8]>>,
       do_parse!(
           count: le_u8 >>
           base_file: scene_value >>
           idx: scene_value >>
               children: count!(grp_composite_child, count as usize) >>
               (GrpCompositeIndexed {
                   base_file: base_file,
                   idx: idx,
                   children: children
               })
       )
);

#[derive(Serialize, Deserialize, Debug, PartialEq, Eq, Clone)]
pub enum GrpCmd {
    Load(SceneText, Val), // 0x01
    LoadEffect(GrpEffect), // 0x02
    Load2(SceneText, Val), // 0x03
    LoadEffect2(GrpEffect), // 0x04
    Load3(SceneText, Val), // 0x05
    LoadEffect3(GrpEffect), // 0x06
    Unknown1, // 0x08
    LoadToBuf(SceneText, Val), // 0x09
    LoadToBuf2(SceneText, Val), // 0x10
    LoadCaching(SceneText), // 0x11
    GrpCmd0x13, // 0x13
    LoadComposite(GrpComposite), // 0x22
    LoadCompositeIndexed(GrpCompositeIndexed), // 0x24
    MacroBufferClear, // 0x30
    MacroBufferDelete(Val), // 0x31
    MacroBufferRead(Val), // 0x32
    MacroBufferSet(Val), // 0x33
    BackupScreenCopy, // 0x50
    BackupScreenDisplay(Val), // 0x52
    LoadToBuf3(SceneText, Val), // 0x54
}

named!(pub grp_cmd<&[u8], GrpCmd, CustomError<&[u8]>>,
       switch!(le_u8,
               0x01 => do_parse!(
                   a: scene_text >>
                   b: scene_value >>
                   (GrpCmd::Load(a, b))
               ) |
               0x02 => do_parse!(
                   a: grp_effect >>
                   (GrpCmd::LoadEffect(a))
               ) |
               0x03 => do_parse!(
                   a: scene_text >>
                   b: scene_value >>
                   (GrpCmd::Load2(a, b))
               ) |
               0x04 => do_parse!(
                   a: grp_effect >>
                   (GrpCmd::LoadEffect2(a))
               ) |
               0x05 => do_parse!(
                   a: scene_text >>
                   b: scene_value >>
                   (GrpCmd::Load3(a, b))
               ) |
               0x06 => do_parse!(
                   a: grp_effect >>
                   (GrpCmd::LoadEffect3(a))
               ) |
               0x08 => value!(GrpCmd::Unknown1) |
               0x09 => do_parse!(
                   a: scene_text >>
                   b: scene_value >>
                   (GrpCmd::LoadToBuf(a, b))
               ) |
               0x10 => do_parse!(
                   a: scene_text >>
                   b: scene_value >>
                   (GrpCmd::LoadToBuf2(a, b))
               ) |
               0x11 => do_parse!(
                   a: scene_text >>
                   (GrpCmd::LoadCaching(a))
               ) |
               0x13 => value!(GrpCmd::GrpCmd0x13) |
               0x22 => do_parse!(
                   a: grp_composite >>
                   (GrpCmd::LoadComposite(a))
               ) |
               0x24 => do_parse!(
                   a: grp_composite_indexed >>
                   (GrpCmd::LoadCompositeIndexed(a))
               ) |
               0x30 => value!(GrpCmd::MacroBufferClear) |
               0x31 => do_parse!(
                   a: scene_value >>
                   (GrpCmd::MacroBufferDelete(a))
               ) |
               0x32 => do_parse!(
                   a: scene_value >>
                   (GrpCmd::MacroBufferRead(a))
               ) |
               0x33 => do_parse!(
                   a: scene_value >>
                   (GrpCmd::MacroBufferSet(a))
               ) |
               0x50 => value!(GrpCmd::BackupScreenCopy) |
               0x52 => do_parse!(
                   a: scene_value >>
                   (GrpCmd::BackupScreenDisplay(a))
               ) |
               0x54 => do_parse!(
                   a: scene_text >>
                   b: scene_value >>
                   (GrpCmd::LoadToBuf3(a, b))
               )

       )
);

#[derive(Serialize, Deserialize, Debug, PartialEq, Eq, Clone)]
pub enum SndCmd {
    BgmLoop(SceneText), // 0x01
    BgmWait(SceneText), // 0x02
    BgmOnce(SceneText), // 0x03
    BgmFadeInLoop(SceneText, Val), // 0x05
    BgmFadeInWait(SceneText, Val), // 0x06
    BgmFadeInOnce(SceneText, Val), // 0x07
    BgmFadeOut(Val), // 0x10
    BgmStop, // 0x11
    BgmRewind, // 0x12
    BgmUnknown1, // 0x16
    KoePlayWait(Val), // 0x20
    KoePlay(Val), // 0x21
    KoePlay2(Val, Val), // 0x22
    WavPlay(SceneText), // 0x30
    WavPlay2(SceneText, Val), // 0x31
    WavLoop(SceneText), // 0x32
    WavLoop2(SceneText, Val), // 0x33
    WavPlayWait(SceneText), // 0x34
    WavPlayWait2(SceneText, Val), // 0x35
    WavStop, // 0x36
    WavStop2(Val), // 0x37
    WavStop3, // 0x38
    WavUnknown0x39(Val), // 0x39
    SePlay(Val), // 0x44
    MoviePlay(SceneText, Val, Val, Val, Val), // 0x50
    MovieLoop(SceneText, Val, Val, Val, Val), // 0x51
    MovieWait(SceneText, Val, Val, Val, Val), // 0x52
    MovieWaitCancelable(SceneText, Val, Val, Val, Val), // 0x53
    MovieWait2(SceneText, SceneText, Val, Val, Val, Val), // 0x54
    MovieWaitCancelable2(SceneText, SceneText, Val, Val, Val, Val), // 0x55
    Unknown1, // 0x60
}

named!(pub snd_cmd<&[u8], SndCmd, CustomError<&[u8]>>,
       switch!(le_u8,
               0x01 => do_parse!(
                   a: scene_text >>
                   (SndCmd::BgmLoop(a))
               ) |
               0x02 => do_parse!(
                   a: scene_text >>
                   (SndCmd::BgmWait(a))
               ) |
               0x03 => do_parse!(
                   a: scene_text >>
                   (SndCmd::BgmOnce(a))
               ) |
               0x05 => do_parse!(
                   a: scene_text >>
                   b: scene_value >>
                   (SndCmd::BgmFadeInLoop(a, b))
               ) |
               0x06 => do_parse!(
                   a: scene_text >>
                   b: scene_value >>
                   (SndCmd::BgmFadeInWait(a, b))
               ) |
               0x07 => do_parse!(
                   a: scene_text >>
                   b: scene_value >>
                   (SndCmd::BgmFadeInOnce(a, b))
               ) |
               0x10 => do_parse!(
                   a: scene_value >>
                   (SndCmd::BgmFadeOut(a))
               ) |
               0x11 => value!(SndCmd::BgmStop) |
               0x12 => value!(SndCmd::BgmRewind) |
               0x16 => value!(SndCmd::BgmUnknown1) |
               0x20 => do_parse!(
                   a: scene_value >>
                   (SndCmd::KoePlayWait(a))
               ) |
               0x21 => do_parse!(
                   a: scene_value >>
                   (SndCmd::KoePlay(a))
               ) |
               0x22 => do_parse!(
                   a: scene_value >>
                   b: scene_value >>
                   (SndCmd::KoePlay2(a, b))
               ) |
               0x30 => do_parse!(
                   a: scene_text >>
                   (SndCmd::WavPlay(a))
               ) |
               0x31 => do_parse!(
                   a: scene_text >>
                   b: scene_value >>
                   (SndCmd::WavPlay2(a, b))
               ) |
               0x32 => do_parse!(
                   a: scene_text >>
                   (SndCmd::WavLoop(a))
               ) |
               0x33 => do_parse!(
                   a: scene_text >>
                   b: scene_value >>
                   (SndCmd::WavLoop2(a, b))
               ) |
               0x34 => do_parse!(
                   a: scene_text >>
                   (SndCmd::WavPlayWait(a))
               ) |
               0x35 => do_parse!(
                   a: scene_text >>
                   b: scene_value >>
                   (SndCmd::WavPlayWait2(a, b))
               ) |
               0x36 => value!(SndCmd::WavStop) |
               0x37 => do_parse!(
                   a: scene_value >>
                   (SndCmd::WavStop2(a))
               ) |
               0x38 => value!(SndCmd::WavStop) |
               0x39 => do_parse!(
                   a: scene_value >>
                   (SndCmd::WavUnknown0x39(a))
               ) |
               0x40 => do_parse!(
                   a: scene_value >>
                   (SndCmd::SePlay(a))
               ) |
               0x50 => do_parse!(
                   a: scene_text >>
                   b: scene_value >>
                   c: scene_value >>
                   d: scene_value >>
                   e: scene_value >>
                   (SndCmd::MoviePlay(a, b, c, d, e))
               ) |
               0x51 => do_parse!(
                   a: scene_text >>
                   b: scene_value >>
                   c: scene_value >>
                   d: scene_value >>
                   e: scene_value >>
                   (SndCmd::MovieLoop(a, b, c, d, e))
               ) |
               0x52 => do_parse!(
                   a: scene_text >>
                   b: scene_value >>
                   c: scene_value >>
                   d: scene_value >>
                   e: scene_value >>
                   (SndCmd::MovieWait(a, b, c, d, e))
               ) |
               0x53 => do_parse!(
                   a: scene_text >>
                   b: scene_value >>
                   c: scene_value >>
                   d: scene_value >>
                   e: scene_value >>
                   (SndCmd::MovieWaitCancelable(a, b, c, d, e))
               ) |
               0x50 => do_parse!(
                   a: scene_text >>
                   b: scene_text >>
                   c: scene_value >>
                   d: scene_value >>
                   e: scene_value >>
                   f: scene_value >>
                   (SndCmd::MovieWait2(a, b, c, d, e, f))
               ) |
               0x50 => do_parse!(
                   a: scene_text >>
                   b: scene_text >>
                   c: scene_value >>
                   d: scene_value >>
                   e: scene_value >>
                   f: scene_value >>
                   (SndCmd::MovieWaitCancelable2(a, b, c, d, e, f))
               ) |
               0x60 => value!(SndCmd::Unknown1)
       )
);

#[derive(Serialize, Deserialize, Debug, PartialEq, Eq, Clone)]
pub enum Ret {
    Color(Val), // 0x20
    Choice, // 0x21
    DisabledChoice(Val) // 0x22
}

#[derive(Serialize, Deserialize, Debug, PartialEq, Eq, Clone)]
pub enum Condition {
    And, // 0x26
    Or, // 0x27
    IncDepth, // 0x28
    DecDepth, // 0x29
    BitNotEq(Val, Val), // 0x36
    BitEq(Val, Val), // 0x37
    NotEq(Val, Val), // 0x38
    Eq(Val, Val), // 0x39
    FlagNotEqConst(Val, Val), // 0x3A
    FlagEqConst(Val, Val), // 0x3B
    FlagAndConst(Val, Val), // 0x41
    FlagAndConst2(Val, Val), // 0x42
    FlagXorConst(Val, Val), // 0x43
    FlagGtConst(Val, Val), // 0x44
    FlagLtConst(Val, Val), // 0x45
    FlagGeqConst(Val, Val), // 0x46
    FlagLeqConst(Val, Val), // 0x47
    FlagNotEq(Val, Val), // 0x48
    FlagEq(Val, Val), // 0x49
    FlagAnd(Val, Val), // 0x4F
    FlagAnd2(Val, Val), // 0x50
    FlagXor(Val, Val), // 0x51
    FlagGt(Val, Val), // 0x52
    FlagLt(Val, Val), // 0x53
    FlagGeq(Val, Val), // 0x54
    FlagLeq(Val, Val), // 0x55
    Ret(Ret), // 0x58
}

fn scene_conditions(input: &[u8]) -> ParseResult<Vec<Condition>> {
    let mut depth = 0;
    let mut conditions = vec![];
    let mut finish = false;
    let mut inp = input;

    while !finish {
        let (i, num) = le_u8(inp)?;
        inp = i;

        let cond = match num {
            0x26 => Condition::And,
            0x27 => Condition::Or,
            0x28 => {
                depth = depth + 1;
                Condition::IncDepth
            },
            0x29 => {
                depth = depth - 1;
                if depth <= 0 {
                    finish = true;
                }
                Condition::DecDepth
            },
            0x36..=0x55 => {
                let (i, val1) = scene_value(inp)?;
                inp = i;
                let (i, val2) = scene_value(inp)?;
                inp = i;

                match num {
                    0x36 => Condition::BitNotEq(val1, val2),
                    0x37 => Condition::BitEq(val1, val2),
                    0x38 => Condition::NotEq(val1, val2),
                    0x39 => Condition::Eq(val1, val2),
                    0x3A => Condition::FlagNotEqConst(val1, val2),
                    0x3B => Condition::FlagEqConst(val1, val2),
                    0x41 => Condition::FlagAndConst(val1, val2),
                    0x42 => Condition::FlagAndConst2(val1, val2),
                    0x43 => Condition::FlagXorConst(val1, val2),
                    0x44 => Condition::FlagGtConst(val1, val2),
                    0x45 => Condition::FlagLtConst(val1, val2),
                    0x46 => Condition::FlagGeqConst(val1, val2),
                    0x47 => Condition::FlagLeqConst(val1, val2),
                    0x48 => Condition::FlagNotEq(val1, val2),
                    0x49 => Condition::FlagEq(val1, val2),
                    0x4F => Condition::FlagAnd(val1, val2),
                    0x50 => Condition::FlagAnd2(val1, val2),
                    0x51 => Condition::FlagXor(val1, val2),
                    0x52 => Condition::FlagGt(val1, val2),
                    0x53 => Condition::FlagLt(val1, val2),
                    0x54 => Condition::FlagGeq(val1, val2),
                    0x55 => Condition::FlagLeq(val1, val2),
                    _ => unreachable!()
                }
            }
            0x58 => {
                let (i, attr) = le_u8(inp)?;
                inp = i;

                let ret = match attr {
                    0x20 => {
                        let (i, val) = scene_value(inp)?;
                        inp = i;
                        Ret::Color(val)
                    },
                    0x21 => Ret::Choice,
                    0x22 => {
                        let (i, val) = scene_value(inp)?;
                        inp = i;
                        Ret::DisabledChoice(val)
                    },
                    _ => unreachable!()
                };
                Condition::Ret(ret)
            },
            _ => return Err(nom::Err::Error(CustomError::MyError(format!("Unknown {}", num))))
        };

        conditions.push(cond);
    }

    Ok((inp, conditions))
}

#[derive(Serialize, Deserialize, Debug, PartialEq, Eq, Clone)]
pub enum ScreenShakeCmd {
    ScreenShake(Val), // 0x01
}

named!(pub screen_shake_cmd<&[u8], ScreenShakeCmd, CustomError<&[u8]>>,
    switch!(le_u8,
        0x01 => do_parse!(
            val: scene_value >>
            (ScreenShakeCmd::ScreenShake(val))
        )
    )
);

#[derive(Serialize, Deserialize, Debug, PartialEq, Eq, Clone)]
pub enum WaitCmd {
    Wait(Val), // 0x01
    WaitMouse(Val, Val), // 0x02
    SetToBase, // 0x03
    WaitFromBase(Val), // 0x04
    WaitFromBaseMouse(Val), // 0x05
    SetToBaseVal(Val), // 0x06
    Wait0x10, // 0x10
    Wait0x11, // 0x11
    Wait0x12, // 0x12
    Wait0x13 // 0x13
}

named!(pub wait_cmd<&[u8], WaitCmd, CustomError<&[u8]>>,
    switch!(le_u8,
        0x01 => do_parse!(
            val: scene_value >>
            (WaitCmd::Wait(val))
        ) |
        0x02 => do_parse!(
            val: scene_value >>
            cancel_index: scene_value >>
            (WaitCmd::WaitMouse(val, cancel_index))
        ) |
        0x03 => value!(WaitCmd::SetToBase) |
        0x04 => do_parse!(
            val: scene_value >>
            (WaitCmd::WaitFromBase(val))
        ) |
        0x05 => do_parse!(
            val: scene_value >>
            (WaitCmd::WaitFromBaseMouse(val))
        ) |
        0x06 => do_parse!(
            val: scene_value >>
            (WaitCmd::SetToBaseVal(val))
        ) |
        0x10 => value!(WaitCmd::Wait0x10) |
        0x11 => value!(WaitCmd::Wait0x11) |
        0x12 => value!(WaitCmd::Wait0x12) |
        0x13 => value!(WaitCmd::Wait0x13)
    )
);

#[derive(Serialize, Deserialize, Debug, PartialEq, Eq, Clone)]
pub enum RetCmd {
    SameScene, // 0x01
    OtherScene, // 0x02
    PopStack, // 0x03
    ClearStack // 0x06
}

named!(pub ret_cmd<&[u8], RetCmd, CustomError<&[u8]>>,
    switch!(le_u8,
        0x01 => value!(RetCmd::SameScene) |
        0x02 => value!(RetCmd::OtherScene) |
        0x03 => value!(RetCmd::PopStack) |
        0x06 => value!(RetCmd::ClearStack)
    )
);

#[derive(Serialize, Deserialize, Debug, PartialEq, Eq, Clone)]
pub enum ScenarioMenuCmd {
    SetBit(Val), // 0x01
    SetBit2(Val, Val) // 0x02
}

named!(pub scenario_menu_cmd<&[u8], ScenarioMenuCmd, CustomError<&[u8]>>,
    switch!(le_u8,
        0x01 => do_parse!(index: scene_value >> (ScenarioMenuCmd::SetBit(index))) |
        0x02 => do_parse!(index: scene_value >> value: scene_value >> (ScenarioMenuCmd::SetBit2(index, value)))
    )
);

#[derive(Serialize, Deserialize, Debug, PartialEq, Eq, Clone)]
pub enum TextRankCmd {
    Set(Val), // 0x01
    Clear, // 0x02
}

named!(pub text_rank_cmd<&[u8], TextRankCmd, CustomError<&[u8]>>,
    switch!(le_u8,
        0x01 => do_parse!(val: scene_value >> (TextRankCmd::Set(val))) |
        0x02 => value!(TextRankCmd::Clear)
    )
);

#[derive(Serialize, Deserialize, Debug, PartialEq, Eq, Clone)]
pub enum Choice {
    Choice, // 0x22
    End // 0x23
}

#[derive(Serialize, Deserialize, Debug, PartialEq, Eq, Clone)]
pub struct ChoiceText {
    pub pad: Option<u8>,
    pub texts: Vec<SceneFormattedText>
}

#[derive(Serialize, Deserialize, Debug, PartialEq, Eq, Clone)]
pub enum ChoiceCmd {
    Choice(Val, u8, Option<ChoiceText>), // 0x01
    Choice2(Val, u8, Option<ChoiceText>), // 0x02
    LoadMenu(Val) // 0x04
}

named!(pub choice_cmd<&[u8], ChoiceCmd, CustomError<&[u8]>>,
    switch!(le_u8,
            0x01 => do_parse!(
                index: scene_value >>
                    flag: le_u8 >>
                    texts: cond!(flag == 0x22,
                          do_parse!(
                              pad: opt!(le_u8) >>
                                  texts: many_till!(
                                      scene_formatted_text,
                                      tag!([0x23])
                                  ) >>
                                  (ChoiceText { pad: pad, texts: texts.0 })
                        )
                    ) >>
                    (ChoiceCmd::Choice(index, flag, texts))
            ) |
            0x02 => do_parse!(
                index: scene_value >>
                    flag: le_u8 >>
                    texts: cond!(flag == 0x22,
                          do_parse!(
                              pad: opt!(le_u8) >>
                                  texts: many_till!(
                                      scene_formatted_text,
                                      tag!([0x23])
                                  ) >>
                                  (ChoiceText { pad: pad, texts: texts.0 })
                        )
                    ) >>
                    (ChoiceCmd::Choice2(index, flag, texts))
            ) |
        0x04 => do_parse!(index: scene_value >> (ChoiceCmd::LoadMenu(index)))
    )
);

#[derive(Serialize, Deserialize, Debug, PartialEq, Eq, Clone)]
pub enum StringCmd {
    StrcpyLiteral(Val, SceneText), // 0x01
    Strlen(Val, Val), // 0x02
    Strcmp(Val, Val, Val), // 0x03
    Strcat(Val, Val), // 0x04
    Strcpy(Val, Val), // 0x05
    Itoa(Val, Val, Val), // 0x06
    HanToZen(Val), // 0x07
    Atoi(Val, Val), // 0x08
}

named!(pub string_cmd<&[u8], StringCmd, CustomError<&[u8]>>,
    switch!(le_u8,
        0x01 => do_parse!(dest: scene_value >> text: scene_text >> (StringCmd::StrcpyLiteral(dest, text))) |
        0x02 => do_parse!(dest: scene_value >> src: scene_value >> (StringCmd::Strlen(dest, src))) |
        0x03 => do_parse!(dest: scene_value >> text1: scene_value >> text2: scene_value >> (StringCmd::Strcmp(dest, text1, text2))) |
        0x04 => do_parse!(dest: scene_value >> text: scene_value >> (StringCmd::Strcat(dest, text))) |
        0x05 => do_parse!(dest: scene_value >> src: scene_value >> (StringCmd::Strcpy(dest, src))) |
        0x06 => do_parse!(dest: scene_value >> src: scene_value >> ordinal: scene_value >> (StringCmd::Itoa(dest, src, ordinal))) |
        0x07 => do_parse!(dest: scene_value >> (StringCmd::HanToZen(dest))) |
        0x08 => do_parse!(dest: scene_value >> src: scene_value >> (StringCmd::Atoi(dest, src)))
    )
);

#[derive(Serialize, Deserialize, Debug, PartialEq, Eq, Clone)]
pub enum SetMultiCmd {
    Val(Val, Val, Val), // 0x01
    Bit(Val, Val, Val), // 0x02
}

named!(pub set_multi_cmd<&[u8], SetMultiCmd, CustomError<&[u8]>>,
       switch!(le_u8,
                    0x01 => do_parse!(
                        start_idx: scene_value >>
                            end_idx: scene_value >>
                            value: scene_value >>
                            (SetMultiCmd::Val(start_idx, end_idx, value))
                    ) |
                    0x02 => do_parse!(
                        start_idx: scene_value >>
                            end_idx: scene_value >>
                            value: scene_value >>
                            (SetMultiCmd::Bit(start_idx, end_idx, value))
                    )
       )
);

#[derive(Serialize, Deserialize, Debug, PartialEq, Eq, Clone)]
pub struct BRGRectColor {
    pub srcx1: Val,
    pub srcy1: Val,
    pub srcx2: Val,
    pub srcy2: Val,
    pub srcpdt: Val,
    pub r: Val,
    pub g: Val,
    pub b: Val,
}

named!(pub brg_rect_color<&[u8], BRGRectColor, CustomError<&[u8]>>,
       do_parse!(
            srcx1: scene_value >>
               srcy1: scene_value >>
               srcx2: scene_value >>
               srcy2: scene_value >>
               srcpdt: scene_value >>
               r: scene_value >>
               g: scene_value >>
               b: scene_value >>
               (BRGRectColor {
                   srcx1: srcx1,
                   srcy1: srcy1,
                   srcx2: srcx2,
                   srcy2: srcy2,
                   srcpdt: srcpdt,
                   r: r,
                   g: g,
                   b: b
               })
       )
);

#[derive(Serialize, Deserialize, Debug, PartialEq, Eq, Clone)]
pub struct BRGRect {
    pub srcx1: Val,
    pub srcy1: Val,
    pub srcx2: Val,
    pub srcy2: Val,
    pub srcpdt: Val,
}

named!(pub brg_rect<&[u8], BRGRect, CustomError<&[u8]>>,
       do_parse!(
            srcx1: scene_value >>
               srcy1: scene_value >>
               srcx2: scene_value >>
               srcy2: scene_value >>
               srcpdt: scene_value >>
               (BRGRect {
                   srcx1: srcx1,
                   srcy1: srcy1,
                   srcx2: srcx2,
                   srcy2: srcy2,
                   srcpdt: srcpdt,
               })
       )
);

#[derive(Serialize, Deserialize, Debug, PartialEq, Eq, Clone)]
pub struct BRGFadeOutColor {
    pub srcx1: Val,
    pub srcy1: Val,
    pub srcx2: Val,
    pub srcy2: Val,
    pub srcpdt: Val,
    pub r: Val,
    pub g: Val,
    pub b: Val,
    pub count: Val,
}

named!(pub brg_fade_out_color<&[u8], BRGFadeOutColor, CustomError<&[u8]>>,
       do_parse!(
            srcx1: scene_value >>
               srcy1: scene_value >>
               srcx2: scene_value >>
               srcy2: scene_value >>
               srcpdt: scene_value >>
               r: scene_value >>
               g: scene_value >>
               b: scene_value >>
               count: scene_value >>
               (BRGFadeOutColor {
                   srcx1: srcx1,
                   srcy1: srcy1,
                   srcx2: srcx2,
                   srcy2: srcy2,
                   srcpdt: srcpdt,
                   r: r,
                   g: g,
                   b: b,
                   count: count
               })
       )
);

#[derive(Serialize, Deserialize, Debug, PartialEq, Eq, Clone)]
pub struct BRGStretchBlit {
    pub srcx1: Val,
    pub srcy1: Val,
    pub srcx2: Val,
    pub srcy2: Val,
    pub srcpdt: Val,
    pub dstx1: Val,
    pub dstx2: Val,
    pub dsty1: Val,
    pub dsty2: Val,
    pub dstpdt: Val,
}

named!(pub brg_stretch_blit<&[u8], BRGStretchBlit, CustomError<&[u8]>>,
       do_parse!(
            srcx1: scene_value >>
               srcy1: scene_value >>
               srcx2: scene_value >>
               srcy2: scene_value >>
               srcpdt: scene_value >>
               dstx1: scene_value >>
               dsty1: scene_value >>
               dstx2: scene_value >>
               dsty2: scene_value >>
               dstpdt: scene_value >>
               (BRGStretchBlit {
                   srcx1: srcx1,
                   srcy1: srcy1,
                   srcx2: srcx2,
                   srcy2: srcy2,
                   srcpdt: srcpdt,
                   dstx1: dstx1,
                   dsty1: dsty1,
                   dstx2: dstx2,
                   dsty2: dsty2,
                   dstpdt: dstpdt,
               })
       )
);

#[derive(Serialize, Deserialize, Debug, PartialEq, Eq, Clone)]
pub struct BRGStretchBlitEffect {
    pub sx1: Val,
    pub sy1: Val,
    pub sx2: Val,
    pub sy2: Val,
    pub ex1: Val,
    pub ey1: Val,
    pub ex2: Val,
    pub ey2: Val,
    pub srcpdt: Val,
    pub dx1: Val,
    pub dy1: Val,
    pub dx2: Val,
    pub dy2: Val,
    pub dstpdt: Val,
    pub step: Val,
    pub steptime: Val
}

named!(pub brg_stretch_blit_effect<&[u8], BRGStretchBlitEffect, CustomError<&[u8]>>,
       do_parse!(
           sx1: scene_value >>
               sy1: scene_value >>
               sx2: scene_value >>
               sy2: scene_value >>
               ex1: scene_value >>
               ey1: scene_value >>
               ex2: scene_value >>
               ey2: scene_value >>
               srcpdt: scene_value >>
               dx1: scene_value >>
               dy1: scene_value >>
               dx2: scene_value >>
               dy2: scene_value >>
               dstpdt: scene_value >>
               step: scene_value >>
               steptime: scene_value >>
               (BRGStretchBlitEffect {
                   sx1: sx1,
                   sy1: sy1,
                   sx2: sx2,
                   sy2: sy2,
                   ex1: ex1,
                   ey1: ey1,
                   ex2: ex2,
                   ey2: ey2,
                   srcpdt: srcpdt,
                   dx1: dx1,
                   dy1: dy1,
                   dx2: dx2,
                   dy2: dy2,
                   dstpdt: dstpdt,
                   step: step,
                   steptime: steptime
               })
       )
);

#[derive(Serialize, Deserialize, Debug, PartialEq, Eq, Clone)]
pub enum BufferRegionGrpCmd {
    ClearRect(BRGRectColor), // 0x02
    DrawRectLine(BRGRectColor), // 0x04
    InvertColor(BRGRect), // 0x07
    ColorMask(BRGRectColor), // 0x10
    FadeOutColor(BRGRect), // 0x11
    FadeOutColor2(BRGRect), // 0x12
    FadeOutColor3(BRGFadeOutColor), // 0x15
    MakeMonoImage(BRGRect), // 0x20
    StretchBlit(BRGStretchBlit), // 0x30
    StretchBlitEffect(BRGStretchBlitEffect), // 0x32
}

named!(pub buffer_region_grp_cmd<&[u8], BufferRegionGrpCmd, CustomError<&[u8]>>,
       switch!(le_u8,
               0x02 => do_parse!(a: brg_rect_color >> (BufferRegionGrpCmd::ClearRect(a))) |
               0x04 => do_parse!(a: brg_rect_color >> (BufferRegionGrpCmd::DrawRectLine(a))) |
               0x07 => do_parse!(a: brg_rect >> (BufferRegionGrpCmd::InvertColor(a))) |
               0x10 => do_parse!(a: brg_rect_color >> (BufferRegionGrpCmd::ColorMask(a))) |
               0x11 => do_parse!(a: brg_rect >> (BufferRegionGrpCmd::FadeOutColor(a))) |
               0x12 => do_parse!(a: brg_rect >> (BufferRegionGrpCmd::FadeOutColor2(a))) |
               0x15 => do_parse!(a: brg_fade_out_color >> (BufferRegionGrpCmd::FadeOutColor3(a))) |
               0x20 => do_parse!(a: brg_rect >> (BufferRegionGrpCmd::MakeMonoImage(a))) |
               0x30 => do_parse!(a: brg_stretch_blit >> (BufferRegionGrpCmd::StretchBlit(a))) |
               0x32 => do_parse!(a: brg_stretch_blit_effect >> (BufferRegionGrpCmd::StretchBlitEffect(a)))
       )
);

#[derive(Serialize, Deserialize, Debug, PartialEq, Eq, Clone)]
pub struct BGCopySamePos {
    pub srcx1: Val,
    pub srcy1: Val,
    pub srcx2: Val,
    pub srcy2: Val,
    pub srcpdt: Val,
    pub flag: Val,
}

named!(pub bg_copy_same_pos<&[u8], BGCopySamePos, CustomError<&[u8]>>,
       do_parse!(
           srcx1: scene_value >>
               srcy1: scene_value >>
               srcx2: scene_value >>
               srcy2: scene_value >>
               srcpdt: scene_value >>
               flag: scene_value >>
               (BGCopySamePos {
                   srcx1: srcx1,
                   srcy1: srcy1,
                   srcx2: srcx2,
                   srcy2: srcy2,
                   srcpdt: srcpdt,
                   flag: flag,
               })
       )
);

#[derive(Serialize, Deserialize, Debug, PartialEq, Eq, Clone)]
pub struct BGCopyNewPos {
    pub srcx1: Val,
    pub srcy1: Val,
    pub srcx2: Val,
    pub srcy2: Val,
    pub srcpdt: Val,
    pub dstx1: Val,
    pub dsty1: Val,
    pub dstpdt: Val,
    pub flag: Option<Val>
}

named!(pub bg_copy_new_pos<&[u8], BGCopyNewPos, CustomError<&[u8]>>,
       do_parse!(
           srcx1: scene_value >>
           srcy1: scene_value >>
           srcx2: scene_value >>
           srcy2: scene_value >>
           srcpdt: scene_value >>
           dstx1: scene_value >>
           dsty1: scene_value >>
           dstpdt: scene_value >>
           flag: cond!(sys_version_geq(1704), scene_value) >> // AVG32 New Version (>17D) Only
           (BGCopyNewPos {
               srcx1: srcx1,
               srcy1: srcy1,
               srcx2: srcx2,
               srcy2: srcy2,
               srcpdt: srcpdt,
               dstx1: dstx1,
               dsty1: dsty1,
               dstpdt: dstpdt,
               flag: flag
           })
       )
);

named!(pub bg_copy_new_pos_mask<&[u8], BGCopyNewPos, CustomError<&[u8]>>,
       do_parse!(
           srcx1: scene_value >>
               srcy1: scene_value >>
               srcx2: scene_value >>
               srcy2: scene_value >>
               srcpdt: scene_value >>
               dstx1: scene_value >>
               dsty1: scene_value >>
               dstpdt: scene_value >>
               flag: cond!(sys_version_geq(1613), scene_value) >> // AVG32 New Version (>16M) Only??
               (BGCopyNewPos {
                   srcx1: srcx1,
                   srcy1: srcy1,
                   srcx2: srcx2,
                   srcy2: srcy2,
                   srcpdt: srcpdt,
                   dstx1: dstx1,
                   dsty1: dsty1,
                   dstpdt: dstpdt,
                   flag: flag,
               })
       )
);

#[derive(Serialize, Deserialize, Debug, PartialEq, Eq, Clone)]
pub struct BGCopyColor {
    pub srcx1: Val,
    pub srcy1: Val,
    pub srcx2: Val,
    pub srcy2: Val,
    pub srcpdt: Val,
    pub dstx1: Val,
    pub dsty1: Val,
    pub dstpdt: Val,
    pub r: Val,
    pub g: Val,
    pub b: Val
}

named!(pub bg_copy_color<&[u8], BGCopyColor, CustomError<&[u8]>>,
       do_parse!(
           srcx1: scene_value >>
               srcy1: scene_value >>
               srcx2: scene_value >>
               srcy2: scene_value >>
               srcpdt: scene_value >>
               dstx1: scene_value >>
               dsty1: scene_value >>
               dstpdt: scene_value >>
               r: scene_value >>
               g: scene_value >>
               b: scene_value >>
               (BGCopyColor {
                   srcx1: srcx1,
                   srcy1: srcy1,
                   srcx2: srcx2,
                   srcy2: srcy2,
                   srcpdt: srcpdt,
                   dstx1: dstx1,
                   dsty1: dsty1,
                   dstpdt: dstpdt,
                   r: r,
                   g: g,
                   b: b
               })
       )
);

#[derive(Serialize, Deserialize, Debug, PartialEq, Eq, Clone)]
pub struct BGSwap {
    pub srcx1: Val,
    pub srcy1: Val,
    pub srcx2: Val,
    pub srcy2: Val,
    pub srcpdt: Val,
    pub dstx1: Val,
    pub dsty1: Val,
    pub dstpdt: Val,
}

named!(pub bg_swap<&[u8], BGSwap, CustomError<&[u8]>>,
       do_parse!(
           srcx1: scene_value >>
               srcy1: scene_value >>
               srcx2: scene_value >>
               srcy2: scene_value >>
               srcpdt: scene_value >>
               dstx1: scene_value >>
               dsty1: scene_value >>
               dstpdt: scene_value >>
               (BGSwap {
                   srcx1: srcx1,
                   srcy1: srcy1,
                   srcx2: srcx2,
                   srcy2: srcy2,
                   srcpdt: srcpdt,
                   dstx1: dstx1,
                   dsty1: dsty1,
                   dstpdt: dstpdt,
               })
       )
);

#[derive(Serialize, Deserialize, Debug, PartialEq, Eq, Clone)]
pub struct BGCopyWithMask {
    pub srcx1: Val,
    pub srcy1: Val,
    pub srcx2: Val,
    pub srcy2: Val,
    pub srcpdt: Val,
    pub dstx1: Val,
    pub dsty1: Val,
    pub dstpdt: Val,
    pub flag: Val
}

named!(pub bg_copy_with_mask<&[u8], BGCopyWithMask, CustomError<&[u8]>>,
       do_parse!(
           srcx1: scene_value >>
               srcy1: scene_value >>
               srcx2: scene_value >>
               srcy2: scene_value >>
               srcpdt: scene_value >>
               dstx1: scene_value >>
               dsty1: scene_value >>
               dstpdt: scene_value >>
               flag: scene_value >>
               (BGCopyWithMask {
                   srcx1: srcx1,
                   srcy1: srcy1,
                   srcx2: srcx2,
                   srcy2: srcy2,
                   srcpdt: srcpdt,
                   dstx1: dstx1,
                   dsty1: dsty1,
                   dstpdt: dstpdt,
                   flag: flag,
               })
       )
);

#[derive(Serialize, Deserialize, Debug, PartialEq, Eq, Clone)]
pub struct BGCopyWholeScreen {
    pub srcpdt: Val,
    pub dstpdt: Val,
    pub flag: Option<Val>
}

named!(pub bg_copy_whole_screen<&[u8], BGCopyWholeScreen, CustomError<&[u8]>>,
       do_parse!(
               srcpdt: scene_value >>
               dstpdt: scene_value >>
               flag: cond!(sys_version_geq(1704), scene_value) >> // AVG32 New Version (>17D) Only
               (BGCopyWholeScreen {
                   srcpdt: srcpdt,
                   dstpdt: dstpdt,
                   flag: flag,
               })
       )
);

named!(pub bg_copy_whole_screen_mask<&[u8], BGCopyWholeScreen, CustomError<&[u8]>>,
       do_parse!(
               srcpdt: scene_value >>
               dstpdt: scene_value >>
               flag: cond!(sys_version_geq(1613), scene_value) >> // AVG32 New Version (>16M) Only
               (BGCopyWholeScreen {
                   srcpdt: srcpdt,
                   dstpdt: dstpdt,
                   flag: flag,
               })
       )
);

#[derive(Serialize, Deserialize, Debug, PartialEq, Eq, Clone)]
pub struct BGDisplayStrings {
    pub n: Val,
    pub srcx1: Val,
    pub srcy1: Val,
    pub srcx2: Val,
    pub srcy2: Val,
    pub srcdx: Val,
    pub srcdy: Val,
    pub srcpdt: Val,
    pub dstx1: Val,
    pub dsty1: Val,
    pub dstx2: Val,
    pub dsty2: Val,
    pub count: Val,
    pub zero: Val,
    pub dstpdt: Val,
}

named!(pub bg_display_strings<&[u8], BGDisplayStrings, CustomError<&[u8]>>,
       do_parse!(
           n: scene_value >>
               srcx1: scene_value >>
               srcy1: scene_value >>
               srcx2: scene_value >>
               srcy2: scene_value >>
               srcdx: scene_value >>
               srcdy: scene_value >>
               srcpdt: scene_value >>
               dstx1: scene_value >>
               dsty1: scene_value >>
               dstx2: scene_value >>
               dsty2: scene_value >>
               count: scene_value >>
               zero: scene_value >>
               dstpdt: scene_value >>
               (BGDisplayStrings {
                   n: n,
                   srcx1: srcx1,
                   srcy1: srcy1,
                   srcx2: srcx2,
                   srcy2: srcy2,
                   srcdx: srcdx,
                   srcdy: srcdy,
                   srcpdt: srcpdt,
                   dstx1: dstx1,
                   dsty1: dsty1,
                   dstx2: dstx2,
                   dsty2: dsty2,
                   count: count,
                   zero: zero,
                   dstpdt: dstpdt,
               })
       )
);

#[derive(Serialize, Deserialize, Debug, PartialEq, Eq, Clone)]
pub struct BGDisplayStringsMask {
    pub n: Val,
    pub srcx1: Val,
    pub srcy1: Val,
    pub srcx2: Val,
    pub srcy2: Val,
    pub srcdx: Val,
    pub srcdy: Val,
    pub srcpdt: Val,
    pub dstx1: Val,
    pub dsty1: Val,
    pub dstx2: Val,
    pub dsty2: Val,
    pub count: Val,
    pub zero: Val,
    pub dstpdt: Val,
    pub flag: Val,
}

named!(pub bg_display_strings_mask<&[u8], BGDisplayStringsMask, CustomError<&[u8]>>,
       do_parse!(
           n: scene_value >>
               srcx1: scene_value >>
               srcy1: scene_value >>
               srcx2: scene_value >>
               srcy2: scene_value >>
               srcdx: scene_value >>
               srcdy: scene_value >>
               srcpdt: scene_value >>
               dstx1: scene_value >>
               dsty1: scene_value >>
               dstx2: scene_value >>
               dsty2: scene_value >>
               count: scene_value >>
               zero: scene_value >>
               dstpdt: scene_value >>
               flag: scene_value >>
               (BGDisplayStringsMask {
                   n: n,
                   srcx1: srcx1,
                   srcy1: srcy1,
                   srcx2: srcx2,
                   srcy2: srcy2,
                   srcdx: srcdx,
                   srcdy: srcdy,
                   srcpdt: srcpdt,
                   dstx1: dstx1,
                   dsty1: dsty1,
                   dstx2: dstx2,
                   dsty2: dsty2,
                   count: count,
                   zero: zero,
                   dstpdt: dstpdt,
                   flag: flag,
               })
       )
);

#[derive(Serialize, Deserialize, Debug, PartialEq, Eq, Clone)]
pub struct BGDisplayStringsColor {
    pub n: Val,
    pub srcx1: Val,
    pub srcy1: Val,
    pub srcx2: Val,
    pub srcy2: Val,
    pub srcdx: Val,
    pub srcdy: Val,
    pub srcpdt: Val,
    pub dstx1: Val,
    pub dsty1: Val,
    pub dstx2: Val,
    pub dsty2: Val,
    pub count: Val,
    pub zero: Val,
    pub dstpdt: Val,
    pub r: Val,
    pub g: Val,
    pub b: Val
}

named!(pub bg_display_strings_color<&[u8], BGDisplayStringsColor, CustomError<&[u8]>>,
       do_parse!(
           n: scene_value >>
               srcx1: scene_value >>
               srcy1: scene_value >>
               srcx2: scene_value >>
               srcy2: scene_value >>
               srcdx: scene_value >>
               srcdy: scene_value >>
               srcpdt: scene_value >>
               dstx1: scene_value >>
               dsty1: scene_value >>
               dstx2: scene_value >>
               dsty2: scene_value >>
               count: scene_value >>
               zero: scene_value >>
               dstpdt: scene_value >>
               r: scene_value >>
               g: scene_value >>
               b: scene_value >>
               (BGDisplayStringsColor {
                   n: n,
                   srcx1: srcx1,
                   srcy1: srcy1,
                   srcx2: srcx2,
                   srcy2: srcy2,
                   srcdx: srcdx,
                   srcdy: srcdy,
                   srcpdt: srcpdt,
                   dstx1: dstx1,
                   dsty1: dsty1,
                   dstx2: dstx2,
                   dsty2: dsty2,
                   count: count,
                   zero: zero,
                   dstpdt: dstpdt,
                   r: r,
                   g: g,
                   b: b
               })
       )
);

#[derive(Serialize, Deserialize, Debug, PartialEq, Eq, Clone)]
pub enum BufferGrpCmd {
    CopySamePos(BGCopySamePos), // 0x00
    CopyNewPos(BGCopyNewPos), // 0x01
    CopyNewPosMask(BGCopyNewPos), //0x02
    CopyColor(BGCopyColor), // 0x03
    Swap(BGSwap), // 0x05
    CopyWithMask(BGCopyWithMask), // 0x08
    CopyWholeScreen(BGCopyWholeScreen), // 0x11
    CopyWholeScreenMask(BGCopyWholeScreen), // 0x12
    DisplayStrings(BGDisplayStrings), // 0x20
    DisplayStringsMask(BGDisplayStringsMask), // 0x21
    DisplayStringsColor(BGDisplayStringsColor), // 0x22
}

named!(pub buffer_grp_cmd<&[u8], BufferGrpCmd, CustomError<&[u8]>>,
       switch!(le_u8,
               0x00 => do_parse!(a: bg_copy_same_pos >> (BufferGrpCmd::CopySamePos(a))) |
               0x01 => do_parse!(a: bg_copy_new_pos >> (BufferGrpCmd::CopyNewPos(a))) |
               0x02 => do_parse!(a: bg_copy_new_pos_mask >> (BufferGrpCmd::CopyNewPosMask(a))) |
               0x03 => do_parse!(a: bg_copy_color >> (BufferGrpCmd::CopyColor(a))) |
               0x05 => do_parse!(a: bg_swap >> (BufferGrpCmd::Swap(a))) |
               0x08 => do_parse!(a: bg_copy_with_mask >> (BufferGrpCmd::CopyWithMask(a))) |
               0x11 => do_parse!(a: bg_copy_whole_screen >> (BufferGrpCmd::CopyWholeScreen(a))) |
               0x12 => do_parse!(a: bg_copy_whole_screen_mask >> (BufferGrpCmd::CopyWholeScreenMask(a))) |
               0x20 => do_parse!(a: bg_display_strings >> (BufferGrpCmd::DisplayStrings(a))) |
               0x21 => do_parse!(a: bg_display_strings_mask >> (BufferGrpCmd::DisplayStringsMask(a))) |
               0x22 => do_parse!(a: bg_display_strings_color >> (BufferGrpCmd::DisplayStringsColor(a)))
       )
);

#[derive(Serialize, Deserialize, Debug, PartialEq, Eq, Clone)]
pub enum FlashGrpCmd {
    FillColor(Val, Val, Val, Val), // 0x01
    FlashScreen(Val, Val, Val, Val, Val), // 0x10
}

named!(pub flash_grp_cmd<&[u8], FlashGrpCmd, CustomError<&[u8]>>,
       switch!(le_u8,
               0x01 => do_parse!(
                   dstpdt: scene_value >>
                   r: scene_value >>
                   g: scene_value >>
                   b: scene_value >>
                       (FlashGrpCmd::FillColor(dstpdt, r, g, b))
               ) |
               0x10 => do_parse!(
                   r: scene_value >>
                   g: scene_value >>
                   b: scene_value >>
                   time: scene_value >>
                   count: scene_value >>
                       (FlashGrpCmd::FlashScreen(r, g, b, time, count))
               )
       )
);

#[derive(Serialize, Deserialize, Debug, PartialEq, Eq, Clone)]
pub struct MultiPdtEntry {
    pub text: SceneText,
    pub data: Val
}

named!(pub multi_pdt_entry<&[u8], MultiPdtEntry, CustomError<&[u8]>>,
       do_parse!(
           text: scene_text >>
               data: scene_value >>
               (MultiPdtEntry {
                   text: text,
                   data: data
               })
       )
);

#[derive(Serialize, Deserialize, Debug, PartialEq, Eq, Clone)]
pub enum MultiPdtCmd {
    Slideshow(Val, Val, Vec<MultiPdtEntry>), // 0x03
    SlideshowLoop(Val, Val, Vec<MultiPdtEntry>), // 0x04
    StopSlideshowLoop, // 0x05
    Scroll(u8, Val, Val, Val, Vec<MultiPdtEntry>), // 0x10
    Scroll2(u8, Val, Val, Val, Vec<MultiPdtEntry>), // 0x20
    ScrollWithCancel(u8, Val, Val, Val, Val, Vec<MultiPdtEntry>), // 0x30
}

named!(pub multi_pdt_cmd<&[u8], MultiPdtCmd, CustomError<&[u8]>>,
       switch!(le_u8,
               0x03 => do_parse!(
                   count: le_u8 >>
                       pos: scene_value >>
                       wait: scene_value >>
                       entries: count!(multi_pdt_entry, count as usize) >>
                       (MultiPdtCmd::Slideshow(pos, wait, entries))
               ) |
               0x04 => do_parse!(
                   count: le_u8 >>
                       pos: scene_value >>
                       wait: scene_value >>
                       entries: count!(multi_pdt_entry, count as usize) >>
                       (MultiPdtCmd::SlideshowLoop(pos, wait, entries))
               ) |
               0x05 => value!(MultiPdtCmd::StopSlideshowLoop) |
               0x10 => do_parse!(
                   poscmd: le_u8 >>
                       count: le_u8 >>
                       pos: scene_value >>
                       wait: scene_value >>
                       pixel: scene_value >>
                       entries: count!(multi_pdt_entry, count as usize) >>
                       (MultiPdtCmd::Scroll(poscmd, pos, wait, pixel, entries))
               ) |
               0x20 => do_parse!(
                   poscmd: le_u8 >>
                       count: le_u8 >>
                       pos: scene_value >>
                       wait: scene_value >>
                       pixel: scene_value >>
                       entries: count!(multi_pdt_entry, count as usize) >>
                       (MultiPdtCmd::Scroll2(poscmd, pos, wait, pixel, entries))
               ) |
               0x30 => do_parse!(
                   poscmd: le_u8 >>
                       count: le_u8 >>
                       pos: scene_value >>
                       wait: scene_value >>
                       pixel: scene_value >>
                       cancel_index: scene_value >>
                       entries: count!(multi_pdt_entry, count as usize) >>
                       (MultiPdtCmd::ScrollWithCancel(poscmd, pos, wait, pixel, cancel_index, entries))
               ))
);

#[derive(Serialize, Deserialize, Debug, PartialEq, Eq, Clone)]
pub enum SystemCmd {
    LoadGame(Val), // 0x02
    SaveGame(Val), // 0x03
    SetTitle(SceneFormattedText), // 0x04
    MakePopup, // 0x05
    GameEnd, // 0x20
    GetSaveTitle(Val, Val), // 0x30
    CheckSaveData(Val, Val), // 0x31
    Unknown1(Val, Val), // 0x35
    Unknown2(Val, Val), // 0x36
    Unknown3(Val, Val), // 0x37
}

named!(pub system_cmd<&[u8], SystemCmd, CustomError<&[u8]>>,
       switch!(le_u8,
               0x02 => do_parse!(a: scene_value >> (SystemCmd::LoadGame(a))) |
               0x03 => do_parse!(a: scene_value >> (SystemCmd::SaveGame(a))) |
               0x04 => do_parse!(a: scene_formatted_text >> (SystemCmd::SetTitle(a))) |
               0x05 => value!(SystemCmd::MakePopup) |
               0x20 => value!(SystemCmd::GameEnd) |
               0x30 => do_parse!(a: scene_value >> b: scene_value >> (SystemCmd::GetSaveTitle(a, b))) |
               0x31 => do_parse!(a: scene_value >> b: scene_value >> (SystemCmd::CheckSaveData(a, b))) |
               0x35 => do_parse!(a: scene_value >> b: scene_value >> (SystemCmd::Unknown1(a, b))) |
               0x36 => do_parse!(a: scene_value >> b: scene_value >> (SystemCmd::Unknown2(a, b))) |
               0x37 => do_parse!(a: scene_value >> b: scene_value >> (SystemCmd::Unknown3(a, b)))
       )
);

#[derive(Serialize, Deserialize, Debug, PartialEq, Eq, Clone)]
pub struct NameInputItem {
    pub idx: Val,
    pub text: SceneFormattedText
}

named!(pub name_input_item<&[u8], NameInputItem, CustomError<&[u8]>>,
       do_parse!(
           idx: scene_value >>
               text: scene_formatted_text >>
               (NameInputItem {
                   idx: idx,
                   text: text
               })
       )
);

#[derive(Serialize, Deserialize, Debug, PartialEq, Eq, Clone)]
pub enum NameCmd {
    InputBox(Val, Val, Val, Val, Val, Val, Val, Val, Val, Val), // 0x01
    InputBoxFinish(Val), // 0x02
    InputBoxStart(Val), // 0x03
    InputBoxClose(Val), // 0x04
    GetName(Val, Val), // 0x10
    SetName(Val, Val), // 0x11
    GetName2(Val, Val), // 0x12
    NameInputDialog(Val), // 0x20
    Unknown1(Val, SceneText, Val,  Val, Val, Val, Val, Val, Val, Val, Val), // 0x21
    NameInputDialogMulti(Vec<NameInputItem>), // 0x24
    Unknown2, // 0x30
    Unknown3, // 0x31
}

named!(pub name_cmd<&[u8], NameCmd, CustomError<&[u8]>>,
       switch!(le_u8,
               0x01 => do_parse!(
                   x: scene_value >>
                   y: scene_value >>
                   ex: scene_value >>
                   ey: scene_value >>
                   r: scene_value >>
                   g: scene_value >>
                   b: scene_value >>
                   br: scene_value >>
                   bg: scene_value >>
                   bb: scene_value >>
                       (NameCmd::InputBox(x, y, ex, ey, r, g, b, br, bg, bb))
               ) |
               0x02 => do_parse!(
                   idx: scene_value >>
                       (NameCmd::InputBoxFinish(idx))
               ) |
               0x03 => do_parse!(
                   idx: scene_value >>
                       (NameCmd::InputBoxStart(idx))
               ) |
               0x04 => do_parse!(
                   idx: scene_value >>
                       (NameCmd::InputBoxClose(idx))
               ) |
               0x10 => do_parse!(
                   idx: scene_value >>
                   text: scene_value >>
                       (NameCmd::GetName(idx, text))
               ) |
               0x11 => do_parse!(
                   idx: scene_value >>
                   text: scene_value >>
                       (NameCmd::SetName(idx, text))
               ) |
               0x12 => do_parse!(
                   idx: scene_value >>
                   text: scene_value >>
                       (NameCmd::GetName2(idx, text))
               ) |
               0x20 => do_parse!(
                   idx: scene_value >>
                       (NameCmd::NameInputDialog(idx))
               ) |
               0x21 => do_parse!(
                   idx: scene_value >>
                   text: scene_text >>
                   a: scene_value >>
                   b: scene_value >>
                   c: scene_value >>
                   d: scene_value >>
                   e: scene_value >>
                   f: scene_value >>
                   g: scene_value >>
                   h: scene_value >>
                   i: scene_value >>
                       (NameCmd::Unknown1(idx, text, a, b, c, d, e, f, g, h, i))
               ) |
               0x24 => do_parse!(
                   count: le_u8 >>
                   items: count!(name_input_item, count as usize) >>
                       (NameCmd::NameInputDialogMulti(items))
               ) |
               0x30 => value!(NameCmd::Unknown2) |
               0x31 => value!(NameCmd::Unknown3)
       )
);

#[derive(Serialize, Deserialize, Debug, PartialEq, Eq, Clone)]
pub enum AreaBufferCmd {
    ReadCurArd(SceneText, SceneText), // 0x02
    Init, // 0x03
    GetClickedArea(Val, Val), // 0x04
    GetClickedArea2(Val, Val), // 0x05
    DisableArea(Val), // 0x10
    EnableArea(Val), // 0x11
    GetArea(Val, Val, Val), // 0x15
    AssignArea(Val, Val), // 0x20
}

named!(pub area_buffer_cmd<&[u8], AreaBufferCmd, CustomError<&[u8]>>,
       switch!(le_u8,
               0x02 => do_parse!(
                   cur: scene_text >>
                   ard: scene_text >>
                   (AreaBufferCmd::ReadCurArd(cur, ard))
               ) |
               0x03 => value!(AreaBufferCmd::Init) |
               0x04 => do_parse!(
                   val: scene_value >>
                   click: scene_value >>
                   (AreaBufferCmd::GetClickedArea(val, click))
               ) |
               0x05 => do_parse!(
                   val: scene_value >>
                   click: scene_value >>
                   (AreaBufferCmd::GetClickedArea2(val, click))
               ) |
               0x10 => do_parse!(
                   area: scene_value >>
                   (AreaBufferCmd::DisableArea(area))
               ) |
               0x11 => do_parse!(
                   area: scene_value >>
                   (AreaBufferCmd::EnableArea(area))
               ) |
               0x15 => do_parse!(
                   x: scene_value >>
                   y: scene_value >>
                   area: scene_value >>
                   (AreaBufferCmd::GetArea(area, x, y))
               ) |
               0x20 => do_parse!(
                   area_from: scene_value >>
                   area_to: scene_value >>
                   (AreaBufferCmd::AssignArea(area_from, area_to))
               )
       )
);

#[derive(Serialize, Deserialize, Debug, PartialEq, Eq, Clone)]
pub enum MouseCtrlCmd {
    WaitForClick, // 0x01
    SetPos(Val, Val, Val), // 0x02
    FlushClickData, // 0x03
    CursorOff, // 0x20
    CursorOn // 0x21
}

named!(pub mouse_ctrl_cmd<&[u8], MouseCtrlCmd, CustomError<&[u8]>>,
       switch!(le_u8,
               0x01 => value!(MouseCtrlCmd::WaitForClick) |
               0x02 => do_parse!(
                   a: scene_value >>
                   b: scene_value >>
                   c: scene_value >>
                   (MouseCtrlCmd::SetPos(a, b, c))
               ) |
               0x03 => value!(MouseCtrlCmd::FlushClickData) |
               0x20 => value!(MouseCtrlCmd::CursorOff) |
               0x21 => value!(MouseCtrlCmd::CursorOn)
       )
);

#[derive(Serialize, Deserialize, Debug, PartialEq, Eq, Clone)]
pub enum VolumeCmd {
    GetBgmVolume(Val), // 0x01
    GetWavVolume(Val), // 0x02
    GetKoeVolume(Val), // 0x03
    GetSeVolume(Val), // 0x04
    SetBgmVolume(Val), // 0x11
    SetWavVolume(Val), // 0x12
    SetKoeVolume(Val), // 0x13
    SetSeVolume(Val), // 0x14
    MuteBgm(Val), // 0x21
    MuteWav(Val), // 0x22
    MuteKoe(Val), // 0x23
    MuteSe(Val), // 0x24
}

named!(pub set_vol_cmd<&[u8], VolumeCmd, CustomError<&[u8]>>,
       switch!(le_u8,
               0x01 => do_parse!(a: scene_value >> (VolumeCmd::GetBgmVolume(a))) |
               0x02 => do_parse!(a: scene_value >> (VolumeCmd::GetWavVolume(a))) |
               0x03 => do_parse!(a: scene_value >> (VolumeCmd::GetKoeVolume(a))) |
               0x04 => do_parse!(a: scene_value >> (VolumeCmd::GetSeVolume(a))) |
               0x11 => do_parse!(a: scene_value >> (VolumeCmd::SetBgmVolume(a))) |
               0x12 => do_parse!(a: scene_value >> (VolumeCmd::SetWavVolume(a))) |
               0x13 => do_parse!(a: scene_value >> (VolumeCmd::SetKoeVolume(a))) |
               0x14 => do_parse!(a: scene_value >> (VolumeCmd::SetSeVolume(a))) |
               0x21 => do_parse!(a: scene_value >> (VolumeCmd::MuteBgm(a))) |
               0x22 => do_parse!(a: scene_value >> (VolumeCmd::MuteWav(a))) |
               0x23 => do_parse!(a: scene_value >> (VolumeCmd::MuteKoe(a))) |
               0x24 => do_parse!(a: scene_value >> (VolumeCmd::MuteSe(a)))
       )
);

#[derive(Serialize, Deserialize, Debug, PartialEq, Eq, Clone)]
pub enum NovelModeCmd {
    SetEnabled(Val), // 0x01
    Unknown1(Val), // 0x02
    Unknown2, // 0x03
    Unknown3, // 0x04
    Unknown4, // 0x05
}

named!(pub novel_mode_cmd<&[u8], NovelModeCmd, CustomError<&[u8]>>,
       switch!(le_u8,
               0x01 => do_parse!(a: scene_value >> (NovelModeCmd::SetEnabled(a))) |
               0x02 => do_parse!(a: scene_value >> (NovelModeCmd::Unknown1(a))) |
               0x03 => value!(NovelModeCmd::Unknown2) |
               0x04 => value!(NovelModeCmd::Unknown3) |
               0x05 => value!(NovelModeCmd::Unknown4)
       )
);

#[derive(Serialize, Deserialize, Debug, PartialEq, Eq, Clone)]
pub enum WindowVarCmd {
    GetBgFlagColor(Val, Val, Val, Val), // 0x01
    SetBgFlagColor(Val, Val, Val, Val), // 0x02
    GetWindowMove(Val), // 0x03
    SetWindowMove(Val), // 0x04
    GetWindowClearBox(Val), // 0x05
    SetWindowClearBox(Val), // 0x06
    GetWindowWaku(Val), // 0x10
    SetWindowWaku(Val), // 0x11
}

named!(pub window_var_cmd<&[u8], WindowVarCmd, CustomError<&[u8]>>,
       switch!(le_u8,
               0x01 => do_parse!(
                   idx_attr: scene_value >>
                   idx_r: scene_value >>
                   idx_g: scene_value >>
                   idx_b: scene_value >>
                   (WindowVarCmd::GetBgFlagColor(idx_attr, idx_r, idx_g, idx_b))
               ) |
               0x02 => do_parse!(
                   idx_attr: scene_value >>
                   idx_r: scene_value >>
                   idx_g: scene_value >>
                   idx_b: scene_value >>
                   (WindowVarCmd::SetBgFlagColor(idx_attr, idx_r, idx_g, idx_b))
               ) |
               0x03 => do_parse!(a: scene_value >> (WindowVarCmd::GetWindowMove(a))) |
               0x04 => do_parse!(a: scene_value >> (WindowVarCmd::SetWindowMove(a))) |
               0x05 => do_parse!(a: scene_value >> (WindowVarCmd::GetWindowClearBox(a))) |
               0x06 => do_parse!(a: scene_value >> (WindowVarCmd::SetWindowClearBox(a))) |
               0x10 => do_parse!(a: scene_value >> (WindowVarCmd::GetWindowWaku(a))) |
               0x11 => do_parse!(a: scene_value >> (WindowVarCmd::SetWindowWaku(a)))
       )
);

#[derive(Serialize, Deserialize, Debug, PartialEq, Eq, Clone)]
pub enum MessageWinCmd {
    GetWindowMsgPos(Val, Val), // 0x01
    GetWindowComPos(Val, Val), // 0x02
    GetWindowSysPos(Val, Val), // 0x03
    GetWindowSubPos(Val, Val), // 0x04
    GetWindowGrpPos(Val, Val), // 0x05
    SetWindowMsgPos(Val, Val), // 0x11
    SetWindowComPos(Val, Val), // 0x12
    SetWindowSysPos(Val, Val), // 0x13
    SetWindowSubPos(Val, Val), // 0x14
    SetWindowGrpPos(Val, Val), // 0x15
}

named!(pub message_win_cmd<&[u8], MessageWinCmd, CustomError<&[u8]>>,
       switch!(le_u8,
               0x01 => do_parse!(x: scene_value >> y: scene_value >> (MessageWinCmd::GetWindowMsgPos(x, y))) |
               0x02 => do_parse!(x: scene_value >> y: scene_value >> (MessageWinCmd::GetWindowComPos(x, y))) |
               0x03 => do_parse!(x: scene_value >> y: scene_value >> (MessageWinCmd::GetWindowSysPos(x, y))) |
               0x04 => do_parse!(x: scene_value >> y: scene_value >> (MessageWinCmd::GetWindowSubPos(x, y))) |
               0x05 => do_parse!(x: scene_value >> y: scene_value >> (MessageWinCmd::GetWindowGrpPos(x, y))) |
               0x11 => do_parse!(x: scene_value >> y: scene_value >> (MessageWinCmd::SetWindowMsgPos(x, y))) |
               0x12 => do_parse!(x: scene_value >> y: scene_value >> (MessageWinCmd::SetWindowComPos(x, y))) |
               0x13 => do_parse!(x: scene_value >> y: scene_value >> (MessageWinCmd::SetWindowSysPos(x, y))) |
               0x14 => do_parse!(x: scene_value >> y: scene_value >> (MessageWinCmd::SetWindowSubPos(x, y))) |
               0x15 => do_parse!(x: scene_value >> y: scene_value >> (MessageWinCmd::SetWindowGrpPos(x, y)))
       )
);

#[derive(Serialize, Deserialize, Debug, PartialEq, Eq, Clone)]
pub enum SystemVarCmd {
    GetMessageSize(Val, Val), // 0x01
    SetMessageSize(Val, Val), // 0x02
    GetMsgMojiSize(Val, Val), // 0x05
    SetMsgMojiSize(Val, Val), // 0x06
    GetMojiColor(Val), // 0x10
    SetMojiColor(Val), // 0x11
    GetMsgCancel(Val), // 0x12
    SetMsgCancel(Val), // 0x13
    GetMojiKage(Val), // 0x16
    SetMojiKage(Val), // 0x17
    GetKageColor(Val), // 0x18
    SetKageColor(Val), // 0x19
    GetSelCancel(Val), // 0x1a
    SetSelCancel(Val), // 0x1b
    GetCtrlKey(Val), // 0x1c
    SetCtrlKey(Val), // 0x1d
    GetSaveStart(Val), // 0x1e
    SetSaveStart(Val), // 0x1f
    GetDisableNvlTextFlag(Val), // 0x20
    SetDisableNvlTextFlag(Val), // 0x21
    GetFadeTime(Val), // 0x22
    SetFadeTime(Val), // 0x23
    GetCursorMono(Val), // 0x24
    SetCursorMono(Val), // 0x25
    GetCopyWindSw(Val), // 0x26
    SetCopyWindSw(Val), // 0x27
    GetMsgSpeed(Val), // 0x28
    SetMsgSpeed(Val), // 0x29
    GetMsgSpeed2(Val), // 0x2a
    SetMsgSpeed2(Val), // 0x2b
    GetReturnKeyWait(Val), // 0x2c
    SetReturnKeyWait(Val), // 0x2d
    GetKoeTextType(Val), // 0x2e
    SetKoeTextType(Val), // 0x2f
    GetGameSpeckInit(Val), // 0x30
    SetCursorPosition(Val, Val), // 0x31
    SetDisableKeyMouseFlag(Val), // 0x32
    GetGameSpeckInit2(Val), // 0x33
    SetGameSpeckInit(Val), // 0x34
}

named!(pub system_var_cmd<&[u8], SystemVarCmd, CustomError<&[u8]>>,
       switch!(le_u8,
               0x01 => do_parse!(a: scene_value >> b: scene_value >> (SystemVarCmd::GetMessageSize(a, b))) |
               0x02 => do_parse!(a: scene_value >> b: scene_value >> (SystemVarCmd::SetMessageSize(a, b))) |
               0x05 => do_parse!(a: scene_value >> b: scene_value >> (SystemVarCmd::GetMsgMojiSize(a, b))) |
               0x06 => do_parse!(a: scene_value >> b: scene_value >> (SystemVarCmd::SetMsgMojiSize(a, b))) |
               0x10 => do_parse!(a: scene_value >> (SystemVarCmd::GetMojiColor(a))) |
               0x11 => do_parse!(a: scene_value >> (SystemVarCmd::SetMojiColor(a))) |
               0x12 => do_parse!(a: scene_value >> (SystemVarCmd::GetMsgCancel(a))) |
               0x13 => do_parse!(a: scene_value >> (SystemVarCmd::SetMsgCancel(a))) |
               0x16 => do_parse!(a: scene_value >> (SystemVarCmd::GetMojiKage(a))) |
               0x17 => do_parse!(a: scene_value >> (SystemVarCmd::SetMojiKage(a))) |
               0x18 => do_parse!(a: scene_value >> (SystemVarCmd::GetKageColor(a))) |
               0x19 => do_parse!(a: scene_value >> (SystemVarCmd::SetKageColor(a))) |
               0x1a => do_parse!(a: scene_value >> (SystemVarCmd::GetSelCancel(a))) |
               0x1b => do_parse!(a: scene_value >> (SystemVarCmd::SetSelCancel(a))) |
               0x1c => do_parse!(a: scene_value >> (SystemVarCmd::GetCtrlKey(a))) |
               0x1d => do_parse!(a: scene_value >> (SystemVarCmd::SetCtrlKey(a))) |
               0x1e => do_parse!(a: scene_value >> (SystemVarCmd::GetSaveStart(a))) |
               0x1f => do_parse!(a: scene_value >> (SystemVarCmd::SetSaveStart(a))) |
               0x20 => do_parse!(a: scene_value >> (SystemVarCmd::GetDisableNvlTextFlag(a))) |
               0x21 => do_parse!(a: scene_value >> (SystemVarCmd::SetDisableNvlTextFlag(a))) |
               0x22 => do_parse!(a: scene_value >> (SystemVarCmd::GetFadeTime(a))) |
               0x23 => do_parse!(a: scene_value >> (SystemVarCmd::SetFadeTime(a))) |
               0x24 => do_parse!(a: scene_value >> (SystemVarCmd::GetCursorMono(a))) |
               0x25 => do_parse!(a: scene_value >> (SystemVarCmd::SetCursorMono(a))) |
               0x26 => do_parse!(a: scene_value >> (SystemVarCmd::GetCopyWindSw(a))) |
               0x27 => do_parse!(a: scene_value >> (SystemVarCmd::SetCopyWindSw(a))) |
               0x28 => do_parse!(a: scene_value >> (SystemVarCmd::GetMsgSpeed(a))) |
               0x29 => do_parse!(a: scene_value >> (SystemVarCmd::SetMsgSpeed(a))) |
               0x2a => do_parse!(a: scene_value >> (SystemVarCmd::GetMsgSpeed2(a))) |
               0x2b => do_parse!(a: scene_value >> (SystemVarCmd::SetMsgSpeed2(a))) |
               0x2c => do_parse!(a: scene_value >> (SystemVarCmd::GetReturnKeyWait(a))) |
               0x2d => do_parse!(a: scene_value >> (SystemVarCmd::SetReturnKeyWait(a))) |
               0x2e => do_parse!(a: scene_value >> (SystemVarCmd::GetKoeTextType(a))) |
               0x2f => do_parse!(a: scene_value >> (SystemVarCmd::SetKoeTextType(a))) |
               0x30 => do_parse!(a: scene_value >> (SystemVarCmd::GetGameSpeckInit(a))) |
               0x31 => do_parse!(a: scene_value >> b: scene_value >> (SystemVarCmd::SetCursorPosition(a, b))) |
               0x32 => do_parse!(a: scene_value >> (SystemVarCmd::SetDisableKeyMouseFlag(a))) |
               0x33 => do_parse!(a: scene_value >> (SystemVarCmd::GetGameSpeckInit2(a))) |
               0x34 => do_parse!(a: scene_value >> (SystemVarCmd::SetGameSpeckInit(a)))
       )
);

#[derive(Serialize, Deserialize, Debug, PartialEq, Eq, Clone)]
pub enum PopupMenuCmd {
    GetMenuDisabled(Val), // 0x01
    SetMenuDisabled(Val), // 0x02
    GetItemDisabled(Val, Val), // 0x03
    SetItemDisabled(Val, Val), // 0x04
}

named!(pub popup_menu_cmd<&[u8], PopupMenuCmd, CustomError<&[u8]>>,
       switch!(le_u8,
               0x01 => do_parse!(val: scene_value >> (PopupMenuCmd::GetMenuDisabled(val))) |
               0x02 => do_parse!(val: scene_value >> (PopupMenuCmd::SetMenuDisabled(val))) |
               0x03 => do_parse!(item_idx: scene_value >> val: scene_value >> (PopupMenuCmd::GetItemDisabled(item_idx, val))) |
               0x04 => do_parse!(item_idx: scene_value >> val: scene_value >> (PopupMenuCmd::SetItemDisabled(item_idx, val)))
       )
);

#[derive(Serialize, Deserialize, Debug, PartialEq, Eq, Clone)]
pub enum Opcode {
    WaitMouse, // 0x01
    Newline, // 0x02
    WaitMouseText, // 0x03
    TextWin(TextWinCmd), // 0x04
    Op0x05,
    Op0x06,
    Op0x08,
    Graphics(GrpCmd), // 0x0b
    Op0x0c,
    Sound(SndCmd), // 0x0e
    DrawValText(FormattedTextCmd), // 0x10
    Fade(FadeCmd), // 0x13
    Condition(Vec<Condition>, Pos), // 0x15
    JumpToScene(JumpToSceneCmd), // 0x16
    ScreenShake(ScreenShakeCmd), // 0x17
    Op0x18,
    Wait(WaitCmd), // 0x19
    Op0x1a,
    Call(Pos), // 0x1b
    Jump(Pos), // 0x1c
    TableCall(Val, Vec<Pos>), // 0x1d
    TableJump(Val, Vec<Pos>), // 0x1e
    Return(RetCmd), // 0x20
    Unknown0x22, // 0x22
    Unknown0x23, // 0x23
    Unknown0x24, // 0x24
    Unknown0x25, // 0x25
    Unknown0x26, // 0x26
    Unknown0x27, // 0x27
    Unknown0x28, // 0x28
    Unknown0x29, // 0x29
    Op0x2c,
    Op0x2d,
    ScenarioMenu(ScenarioMenuCmd), // 0x2e
    Op0x2f,
    Op0x30,
    TextRank(TextRankCmd), // 0x31
    SetFlag(Val, Val), // 0x37
    CopyFlag(Val, Val), // 0x39
    SetValLiteral(Val, Val), // 0x3b
    AddVal(Val, Val), // 0x3c
    SubVal(Val, Val), // 0x3d
    MulVal(Val, Val), // 0x3e
    DivVal(Val, Val), // 0x3f
    ModVal(Val, Val), // 0x40
    AndVal(Val, Val), // 0x41
    OrVal(Val, Val), // 0x42
    XorVal(Val, Val), // 0x43
    SetVal(Val, Val), // 0x49
    AddValSelf(Val, Val), // 0x4a
    SubValSelf(Val, Val), // 0x4b
    MulValSelf(Val, Val), // 0x4c
    DivValSelf(Val, Val), // 0x4d
    ModValSelf(Val, Val), // 0x4e
    AndValSelf(Val, Val), // 0x4f
    OrValSelf(Val, Val), // 0x50
    XorValSelf(Val, Val), // 0x51
    SetFlagRandom(Val), // 0x56
    SetValRandom(Val, Val), // 0x57
    Choice(ChoiceCmd), // 0x58
    String(StringCmd), // 0x59
    Op0x5b,
    SetMulti(SetMultiCmd), // 0x5c
    Op0x5d,
    Op0x5e,
    Op0x5f,
    System(SystemCmd), // 0x60
    Name(NameCmd), // 0x61
    Op0x63,
    BufferRegion(BufferRegionGrpCmd), // 0x64
    Unknown0x65, // 0x65
    Buffer(BufferGrpCmd), // 0x67
    Flash(FlashGrpCmd), // 0x68
    Op0x69,
    MultiPdt(MultiPdtCmd), // 0x6a
    Op0x66,
    AreaBuffer(AreaBufferCmd), // 0x6c
    MouseCtrl(MouseCtrlCmd), // 0x6d
    Op0x6e,
    Op0x6f,
    WindowVar(WindowVarCmd), // 0x70
    MessageWin(MessageWinCmd), // 0x72
    SystemVar(SystemVarCmd), // 0x73
    PopupMenu(PopupMenuCmd), // 0x74
    Volume(VolumeCmd), // 0x75
    NovelMode(NovelModeCmd), // 0x76
    Op0x7f,
    Unknown0xea(Val), // 0xea
    TextHankaku(Option<u32>, SceneText), // 0xfe
    TextZenkaku(Option<u32>, SceneText), // 0xff
}

named!(pub opcode_0x01<&[u8], Opcode, CustomError<&[u8]>>,
       value!(Opcode::WaitMouse)
);

named!(pub opcode_0x02<&[u8], Opcode, CustomError<&[u8]>>,
       value!(Opcode::Newline)
);

named!(pub opcode_0x03<&[u8], Opcode, CustomError<&[u8]>>,
       value!(Opcode::WaitMouseText)
);

named!(pub opcode_0x04<&[u8], Opcode, CustomError<&[u8]>>,
       do_parse!(
           a: text_win_cmd >>
           (Opcode::TextWin(a))
       )
);

named!(pub opcode_0x0b<&[u8], Opcode, CustomError<&[u8]>>,
       do_parse!(
           a: grp_cmd >>
           (Opcode::Graphics(a))
       )
);

named!(pub opcode_0x0e<&[u8], Opcode, CustomError<&[u8]>>,
       do_parse!(
           a: snd_cmd >>
           (Opcode::Sound(a))
       )
);

named!(pub opcode_0x10<&[u8], Opcode, CustomError<&[u8]>>,
       do_parse!(
           a: formatted_text_cmd >>
           (Opcode::DrawValText(a))
       )
);

named!(pub opcode_0x13<&[u8], Opcode, CustomError<&[u8]>>,
       do_parse!(
           a: fade_cmd >>
           (Opcode::Fade(a))
       )
);

named!(pub opcode_0x15<&[u8], Opcode, CustomError<&[u8]>>,
       do_parse!(
           a: scene_conditions >>
               b: scene_pos >>
               (Opcode::Condition(a, b))
       )
);

named!(pub opcode_0x16<&[u8], Opcode, CustomError<&[u8]>>,
       do_parse!(
        a: jump_to_scene_cmd >>
               (Opcode::JumpToScene(a))
       )
);

named!(pub opcode_0x17<&[u8], Opcode, CustomError<&[u8]>>,
       do_parse!(
           a: screen_shake_cmd >>
               (Opcode::ScreenShake(a))
       )
);

named!(pub opcode_0x19<&[u8], Opcode, CustomError<&[u8]>>,
       do_parse!(
           a: wait_cmd >>
           (Opcode::Wait(a))
       )
);

named!(pub opcode_0x1b<&[u8], Opcode, CustomError<&[u8]>>,
    do_parse!(
       a: scene_pos >>
       (Opcode::Call(a))
    )
);

named!(pub opcode_0x1c<&[u8], Opcode, CustomError<&[u8]>>,
       do_parse!(
           a: scene_pos >>
           (Opcode::Jump(a))
       )
);

named!(pub opcode_0x1d<&[u8], Opcode, CustomError<&[u8]>>,
       do_parse!(
           a: le_u8 >>
           b: scene_value >>
           c: count!(scene_pos, a as usize) >>
           (Opcode::TableCall(b, c))
       )
);

named!(pub opcode_0x1e<&[u8], Opcode, CustomError<&[u8]>>,
       do_parse!(
           a: le_u8 >>
           b: scene_value >>
           c: count!(scene_pos, a as usize) >>
           (Opcode::TableJump(b, c))
       )
);

named!(pub opcode_0x20<&[u8], Opcode, CustomError<&[u8]>>,
       do_parse!(
           a: ret_cmd >>
           (Opcode::Return(a))
       )
);

named!(pub opcode_0x22<&[u8], Opcode, CustomError<&[u8]>>,
       value!(Opcode::Unknown0x22)
);

named!(pub opcode_0x23<&[u8], Opcode, CustomError<&[u8]>>,
       value!(Opcode::Unknown0x23)
);

named!(pub opcode_0x24<&[u8], Opcode, CustomError<&[u8]>>,
       value!(Opcode::Unknown0x24)
);

named!(pub opcode_0x25<&[u8], Opcode, CustomError<&[u8]>>,
       value!(Opcode::Unknown0x25)
);

named!(pub opcode_0x26<&[u8], Opcode, CustomError<&[u8]>>,
       value!(Opcode::Unknown0x26)
);

named!(pub opcode_0x27<&[u8], Opcode, CustomError<&[u8]>>,
       value!(Opcode::Unknown0x27)
);

named!(pub opcode_0x28<&[u8], Opcode, CustomError<&[u8]>>,
       value!(Opcode::Unknown0x28)
);

named!(pub opcode_0x29<&[u8], Opcode, CustomError<&[u8]>>,
       value!(Opcode::Unknown0x29)
);

named!(pub opcode_0x2e<&[u8], Opcode, CustomError<&[u8]>>,
       do_parse!(
           a: scenario_menu_cmd >>
               (Opcode::ScenarioMenu(a))
       )
);

named!(pub opcode_0x2f<&[u8], Opcode, CustomError<&[u8]>>,
       do_parse!(
           a: scenario_menu_cmd >>
               (Opcode::ScenarioMenu(a))
       )
);

named!(pub opcode_0x31<&[u8], Opcode, CustomError<&[u8]>>,
       do_parse!(
           a: text_rank_cmd >>
               (Opcode::TextRank(a))
       )
);

named!(pub opcode_0x37<&[u8], Opcode, CustomError<&[u8]>>,
       do_parse!(
           a: scene_value >>
               b: scene_value >>
               (Opcode::SetFlag(a, b))
       )
);

named!(pub opcode_0x39<&[u8], Opcode, CustomError<&[u8]>>,
       do_parse!(
           a: scene_value >>
               b: scene_value >>
               (Opcode::CopyFlag(a, b))
       )
);

named!(pub opcode_0x3b<&[u8], Opcode, CustomError<&[u8]>>,
       do_parse!(
           a: scene_value >>
               b: scene_value >>
               (Opcode::SetValLiteral(a, b))
       )
);

named!(pub opcode_0x3c<&[u8], Opcode, CustomError<&[u8]>>,
       do_parse!(
           a: scene_value >>
               b: scene_value >>
               (Opcode::AddVal(a, b))
       )
);

named!(pub opcode_0x3d<&[u8], Opcode, CustomError<&[u8]>>,
       do_parse!(
           a: scene_value >>
               b: scene_value >>
               (Opcode::SubVal(a, b))
       )
);

named!(pub opcode_0x3e<&[u8], Opcode, CustomError<&[u8]>>,
       do_parse!(
           a: scene_value >>
               b: scene_value >>
               (Opcode::MulVal(a, b))
       )
);

named!(pub opcode_0x3f<&[u8], Opcode, CustomError<&[u8]>>,
       do_parse!(
           a: scene_value >>
               b: scene_value >>
               (Opcode::DivVal(a, b))
       )
);

named!(pub opcode_0x40<&[u8], Opcode, CustomError<&[u8]>>,
       do_parse!(
           a: scene_value >>
               b: scene_value >>
               (Opcode::ModVal(a, b))
       )
);

named!(pub opcode_0x41<&[u8], Opcode, CustomError<&[u8]>>,
       do_parse!(
           a: scene_value >>
               b: scene_value >>
               (Opcode::AndVal(a, b))
       )
);

named!(pub opcode_0x42<&[u8], Opcode, CustomError<&[u8]>>,
       do_parse!(
           a: scene_value >>
               b: scene_value >>
               (Opcode::OrVal(a, b))
       )
);

named!(pub opcode_0x43<&[u8], Opcode, CustomError<&[u8]>>,
       do_parse!(
           a: scene_value >>
               b: scene_value >>
               (Opcode::XorVal(a, b))
       )
);

named!(pub opcode_0x49<&[u8], Opcode, CustomError<&[u8]>>,
       do_parse!(
           a: scene_value >>
               b: scene_value >>
               (Opcode::SetVal(a, b))
       )
);

named!(pub opcode_0x4a<&[u8], Opcode, CustomError<&[u8]>>,
       do_parse!(
           a: scene_value >>
               b: scene_value >>
               (Opcode::AddValSelf(a, b))
       )
);

named!(pub opcode_0x4b<&[u8], Opcode, CustomError<&[u8]>>,
       do_parse!(
           a: scene_value >>
               b: scene_value >>
               (Opcode::SubValSelf(a, b))
       )
);

named!(pub opcode_0x4c<&[u8], Opcode, CustomError<&[u8]>>,
       do_parse!(
           a: scene_value >>
               b: scene_value >>
               (Opcode::MulValSelf(a, b))
       )
);

named!(pub opcode_0x4d<&[u8], Opcode, CustomError<&[u8]>>,
       do_parse!(
           a: scene_value >>
               b: scene_value >>
               (Opcode::DivValSelf(a, b))
       )
);

named!(pub opcode_0x4e<&[u8], Opcode, CustomError<&[u8]>>,
       do_parse!(
           a: scene_value >>
               b: scene_value >>
               (Opcode::ModValSelf(a, b))
       )
);

named!(pub opcode_0x4f<&[u8], Opcode, CustomError<&[u8]>>,
       do_parse!(
           a: scene_value >>
               b: scene_value >>
               (Opcode::AndValSelf(a, b))
       )
);

named!(pub opcode_0x50<&[u8], Opcode, CustomError<&[u8]>>,
       do_parse!(
           a: scene_value >>
               b: scene_value >>
               (Opcode::OrValSelf(a, b))
       )
);

named!(pub opcode_0x51<&[u8], Opcode, CustomError<&[u8]>>,
       do_parse!(
           a: scene_value >>
               b: scene_value >>
               (Opcode::XorValSelf(a, b))
       )
);

named!(pub opcode_0x56<&[u8], Opcode, CustomError<&[u8]>>,
       do_parse!(
           a: scene_value >>
               (Opcode::SetFlagRandom(a))
       )
);

named!(pub opcode_0x57<&[u8], Opcode, CustomError<&[u8]>>,
       do_parse!(
           a: scene_value >>
               b: scene_value >>
               (Opcode::SetValRandom(a, b))
       )
);

named!(pub opcode_0x58<&[u8], Opcode, CustomError<&[u8]>>,
       do_parse!(
           a: choice_cmd >>
               (Opcode::Choice(a))
       )
);

named!(pub opcode_0x59<&[u8], Opcode, CustomError<&[u8]>>,
       do_parse!(
           a: string_cmd >>
               (Opcode::String(a))
       )
);

named!(pub opcode_0x5c<&[u8], Opcode, CustomError<&[u8]>>,
       do_parse!(
           a: set_multi_cmd >>
               (Opcode::SetMulti(a))
       )
);

named!(pub opcode_0x60<&[u8], Opcode, CustomError<&[u8]>>,
       do_parse!(
           a: system_cmd >>
               (Opcode::System(a))
       )
);

named!(pub opcode_0x61<&[u8], Opcode, CustomError<&[u8]>>,
       do_parse!(
           a: name_cmd >>
               (Opcode::Name(a))
       )
);

named!(pub opcode_0x64<&[u8], Opcode, CustomError<&[u8]>>,
       do_parse!(
           a: buffer_region_grp_cmd >>
               (Opcode::BufferRegion(a))
       )
);

named!(pub opcode_0x65<&[u8], Opcode, CustomError<&[u8]>>,
       value!(Opcode::Unknown0x65)
);

named!(pub opcode_0x67<&[u8], Opcode, CustomError<&[u8]>>,
       do_parse!(
           a: buffer_grp_cmd >>
               (Opcode::Buffer(a))
       )
);

named!(pub opcode_0x68<&[u8], Opcode, CustomError<&[u8]>>,
       do_parse!(
           a: flash_grp_cmd >>
               (Opcode::Flash(a))
       )
);

named!(pub opcode_0x6a<&[u8], Opcode, CustomError<&[u8]>>,
       do_parse!(
           a: multi_pdt_cmd >>
               (Opcode::MultiPdt(a))
       )
);

named!(pub opcode_0x6c<&[u8], Opcode, CustomError<&[u8]>>,
       do_parse!(
           a: area_buffer_cmd >>
           (Opcode::AreaBuffer(a))
       )
);

named!(pub opcode_0x6d<&[u8], Opcode, CustomError<&[u8]>>,
       do_parse!(
           a: mouse_ctrl_cmd >>
           (Opcode::MouseCtrl(a))
       )
);

named!(pub opcode_0x70<&[u8], Opcode, CustomError<&[u8]>>,
       do_parse!(
           a: window_var_cmd >>
           (Opcode::WindowVar(a))
       )
);

named!(pub opcode_0x72<&[u8], Opcode, CustomError<&[u8]>>,
       do_parse!(
           a: message_win_cmd >>
           (Opcode::MessageWin(a))
       )
);

named!(pub opcode_0x73<&[u8], Opcode, CustomError<&[u8]>>,
       do_parse!(
           a: system_var_cmd >>
           (Opcode::SystemVar(a))
       )
);

named!(pub opcode_0x74<&[u8], Opcode, CustomError<&[u8]>>,
       do_parse!(
           a: popup_menu_cmd >>
           (Opcode::PopupMenu(a))
       )
);

named!(pub opcode_0x75<&[u8], Opcode, CustomError<&[u8]>>,
       do_parse!(
           a: set_vol_cmd >>
           (Opcode::Volume(a))
       )
);

named!(pub opcode_0x76<&[u8], Opcode, CustomError<&[u8]>>,
       do_parse!(
           a: novel_mode_cmd >>
           (Opcode::NovelMode(a))
       )
);

named!(pub opcode_0xea<&[u8], Opcode, CustomError<&[u8]>>,
       do_parse!(
           a: scene_value >>
           (Opcode::Unknown0xea(a))
       )
);

named!(pub opcode_0xfe<&[u8], Opcode, CustomError<&[u8]>>,
       do_parse!(
           index: cond!(sys_version_geq(1714), le_u32) >>
           text: scene_text >>
           (Opcode::TextHankaku(index, text))
       )
);

named!(pub opcode_0xff<&[u8], Opcode, CustomError<&[u8]>>,
       do_parse!(
           index: cond!(sys_version_geq(1714), le_u32) >>
           text: scene_text >>
           (Opcode::TextZenkaku(index, text))
       )
);

named!(pub opcode<&[u8], Opcode, CustomError<&[u8]>>,
       switch!(le_u8,
               0x01 => call!(opcode_0x01) |
               0x02 => call!(opcode_0x02) |
               0x03 => call!(opcode_0x03) |
               0x04 => call!(opcode_0x04) |
               // 0x05 => value!(Opcode::Op0x05) |
               // 0x06 => value!(Opcode::Op0x06) |
               // 0x08 => value!(Opcode::Op0x08) |
               0x0b => call!(opcode_0x0b) |
               // 0x0c => value!(Opcode::Op0x0c) |
               0x0e => call!(opcode_0x0e) |
               0x10 => call!(opcode_0x10) |
               0x13 => call!(opcode_0x13) |
               0x15 => call!(opcode_0x15) |
               0x16 => call!(opcode_0x16) |
               0x17 => call!(opcode_0x17) |
               // 0x18 => value!(Opcode::Op0x18) |
               0x19 => call!(opcode_0x19) |
               // 0x1a => value!(Opcode::Op0x1a) |
               0x1b => call!(opcode_0x1b) |
               0x1c => call!(opcode_0x1c) |
               0x1d => call!(opcode_0x1d) |
               0x1e => call!(opcode_0x1e) |
               0x20 => call!(opcode_0x20) |
               0x22 => call!(opcode_0x22) |
               0x23 => call!(opcode_0x23) |
               0x24 => call!(opcode_0x24) |
               0x25 => call!(opcode_0x25) |
               0x26 => call!(opcode_0x26) |
               0x27 => call!(opcode_0x27) |
               0x28 => call!(opcode_0x28) |
               0x29 => call!(opcode_0x29) |
               // 0x2c => value!(Opcode::Op0x2c) |
               // 0x2d => value!(Opcode::Op0x2d) |
               0x2e => call!(opcode_0x2e) |
               0x2f => call!(opcode_0x2f) |
               // 0x30 => value!(Opcode::Op0x30) |
               0x31 => call!(opcode_0x31) |
               0x37 => call!(opcode_0x37) |
               0x39 => call!(opcode_0x39) |
               0x3b => call!(opcode_0x3b) |
               0x3c => call!(opcode_0x3c) |
               0x3d => call!(opcode_0x3d) |
               0x3e => call!(opcode_0x3e) |
               0x3f => call!(opcode_0x3f) |
               0x40 => call!(opcode_0x40) |
               0x41 => call!(opcode_0x41) |
               0x42 => call!(opcode_0x42) |
               0x43 => call!(opcode_0x43) |
               0x49 => call!(opcode_0x49) |
               0x4a => call!(opcode_0x4a) |
               0x4b => call!(opcode_0x4b) |
               0x4c => call!(opcode_0x4c) |
               0x4d => call!(opcode_0x4d) |
               0x4e => call!(opcode_0x4e) |
               0x4f => call!(opcode_0x4f) |
               0x50 => call!(opcode_0x50) |
               0x51 => call!(opcode_0x51) |
               0x56 => call!(opcode_0x56) |
               0x57 => call!(opcode_0x57) |
               0x58 => call!(opcode_0x58) |
               0x59 => call!(opcode_0x59) |
               // 0x5b => value!(Opcode::Op0x5b) |
               0x5c => call!(opcode_0x5c) |
               // 0x5d => value!(Opcode::Op0x5d) |
               // 0x5e => value!(Opcode::Op0x5e) |
               // 0x5f => value!(Opcode::Op0x5f) |
               0x60 => call!(opcode_0x60) |
               0x61 => call!(opcode_0x61) |
               // 0x63 => value!(Opcode::Op0x63) |
               0x64 => call!(opcode_0x64) |
               0x64 => call!(opcode_0x65) |
               0x67 => call!(opcode_0x67) |
               0x68 => call!(opcode_0x68) |
               // 0x69 => value!(Opcode::Op0x69) |
               0x6a => call!(opcode_0x6a) |
               // 0x66 => value!(Opcode::Op0x66) |
               0x6c => call!(opcode_0x6c) |
               0x6d => call!(opcode_0x6d) |
               // 0x6e => value!(Opcode::Op0x6e) |
               // 0x6f => value!(Opcode::Op0x6f) |
               0x70 => call!(opcode_0x70) |
               0x72 => call!(opcode_0x72) |
               0x73 => call!(opcode_0x73) |
               0x74 => call!(opcode_0x74) |
               0x75 => call!(opcode_0x75) |
               0x76 => call!(opcode_0x76) |
               // 0x7f => value!(Opcode::Op0x7f) |
               0xea => call!(opcode_0xea) |
               0xfe => call!(opcode_0xfe) |
               0xff => call!(opcode_0xff)
       )
);

named!(pub avg32_scene<&[u8], AVG32Scene, CustomError<&[u8]>>,
       do_parse!(
           header: header >>
               opcodes: dbg_dmp!(many1!(opcode)) >>
               dbg_dmp!(tag!("\0")) >>
               eof!() >>
               (AVG32Scene {
                   header: header,
                   opcodes: opcodes
               })
       )
);

named!(pub opcodes<&[u8], Vec<Opcode>, CustomError<&[u8]>>,
               dbg_dmp!(many1!(opcode))
);


#[cfg(test)]
mod tests {
    use crate::parser::*;

    #[test]
    fn parse_value() {
        assert_eq!(Val(0x00, ValType::Const), scene_value(&[0x10]).unwrap().1);
        assert_eq!(Val(0x0F, ValType::Const), scene_value(&[0x1F]).unwrap().1);
        assert_eq!(Val(0x01, ValType::Var), scene_value(&[0x91]).unwrap().1);
        assert_eq!(Val(0x800, ValType::Const), scene_value(&[0x20, 0x80]).unwrap().1);
        assert_eq!(Val(0x40804, ValType::Const), scene_value(&[0x34, 0x80, 0x40]).unwrap().1);
        assert_eq!(Val(0xFFFFF, ValType::Const), scene_value(&[0x3F, 0xFF, 0xFF]).unwrap().1);
        assert_eq!(Val(0x0A7D9F8, ValType::Const), scene_value(&[0x48, 0x9F, 0x7D, 0x0A]).unwrap().1);
        assert_eq!(Val(0xFFFFFFF, ValType::Const), scene_value(&[0x4F, 0xFF, 0xFF, 0xFF]).unwrap().1);
    }
}
