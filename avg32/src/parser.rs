use nom::error::{ParseError, ErrorKind};
use nom::IResult;
use nom::number::streaming::{le_u8, le_u32};
use encoding_rs::SHIFT_JIS;

#[derive(Debug, PartialEq)]
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

#[derive(Debug, PartialEq)]
pub struct AVG32Scene {
    header: Header,
    opcodes: Vec<Opcode>
}

#[derive(Debug, PartialEq)]
pub struct Header {
    label_count: u32,
    labels: Vec<u32>,
    counter_start: u32,
    menu_count: u32,
    menus: Vec<Menu>,
    menu_strings: Vec<String>
}

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

named!(pub header<&[u8], Header, CustomError<&[u8]>>,
  do_parse!(
    tag!("TPC32") >>
    take!(0x13) >>
    label_count: le_u32 >>
    counter_start: le_u32 >>
    labels: count!(le_u32, label_count as usize) >>
    take!(0x30) >>
    menu_count: le_u32 >>
    menus: count!(menu, (menu_count) as usize) >>
    menu_strings: call!(menu_strings, &menus) >>
    take!(5) >>
    (Header {
        label_count: label_count,
        labels: labels,
        counter_start: counter_start,
        menu_count: menu_count,
        menus: menus,
        menu_strings: menu_strings
    })
  )
);

#[derive(Debug, PartialEq)]
pub struct Menu {
    id: u8,
    submenu_count: u8,
    submenus: Vec<Submenu>
}

named!(pub menu<&[u8], Menu, CustomError<&[u8]>>,
    do_parse!(
        id: le_u8 >>
        submenu_count: le_u8 >>
        take!(2) >>
        submenus: count!(submenu, submenu_count as usize) >>
        (Menu {
            id: id,
            submenu_count: submenu_count,
            submenus: submenus
        })
    )
);

#[derive(Debug, PartialEq)]
pub struct Submenu {
    id: u8,
    flag_count: u8,
    flags: Vec<Flag>
}

named!(pub submenu<&[u8], Submenu, CustomError<&[u8]>>,
    do_parse!(
        id: le_u8 >>
        flag_count: le_u8 >>
        take!(2) >>
        flags: count!(flag, flag_count as usize) >>
        (Submenu {
            id: id,
            flag_count: flag_count,
            flags: flags
        })
    )
);

#[derive(Debug, PartialEq)]
pub struct Flag {
    flag_count: u8,
    flags: Vec<u32>
}

named!(pub flag<&[u8], Flag, CustomError<&[u8]>>,
    do_parse!(
        flag_count: le_u8 >>
        take!(1) >>
        flags: count!(le_u32, flag_count as usize) >>
        (Flag {
            flag_count: flag_count,
            flags: flags
        })
    )
);

/// Byte position (jump, if, etc.)
pub type Pos = u32;

/// Literal value or variable index
pub type Val = u32;

fn scene_value(input: &[u8]) -> ParseResult<Val> {
    let num = input[0];
    let l = ((num >> 4) & 7) as usize;
    let mut ret: u32 = 0;
    for i in (0..l).rev() {
        ret <<= 4;
        ret |= input[i] as u32;
    }
    Ok((&input[l..], ret))
}

#[derive(Debug, PartialEq)]
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

#[derive(Debug, PartialEq)]
pub enum FormattedTextCmd {
    TextPointer(Val) // 0x03
}

named!(pub formatted_text_cmd<&[u8], FormattedTextCmd, CustomError<&[u8]>>,
    switch!(le_u8,
        0x03 => do_parse!(a: scene_value >> (FormattedTextCmd::TextPointer(a)))
    )
);

#[derive(Debug, PartialEq)]
pub enum SceneFormattedTextEntry {
    TextHankaku(String), // 0xfe
    TextZenkaku(String), // 0xff
    TextPointer(Val), // 0xfd
    Condition(Vec<Condition>), // 0x28
    Command(FormattedTextCmd) // 0x10
}

named!(pub scene_formatted_text_entry<&[u8], SceneFormattedTextEntry, CustomError<&[u8]>>,
       switch!(le_u8,
               0xfe => do_parse!(a: c_string >> (SceneFormattedTextEntry::TextHankaku(a))) |
               0xff => do_parse!(a: c_string >> (SceneFormattedTextEntry::TextZenkaku(a))) |
               0xfd => do_parse!(a: scene_value >> (SceneFormattedTextEntry::TextPointer(a))) |
               0x28 => do_parse!(a: scene_conditions >> (SceneFormattedTextEntry::Condition(a))) |
               0x10 => do_parse!(a: formatted_text_cmd >> (SceneFormattedTextEntry::Command(a)))
        )
);

pub type SceneFormattedText = Vec<SceneFormattedTextEntry>;

named!(pub scene_formatted_text<&[u8], SceneFormattedText, CustomError<&[u8]>>,
    do_parse!(
        res: many_till!(scene_formatted_text_entry, tag!("\0")) >>
        tag!("\0") >>
        (res.0)
    )
);

//
// Opcode data
//

#[derive(Debug, PartialEq)]
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

#[derive(Debug, PartialEq)]
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

#[derive(Debug, PartialEq)]
pub struct GrpEffect {
    file: SceneText,
    sx1: Val,
    sy1: Val,
    sx2: Val,
    sy2: Val,
    dx: Val,
    dy: Val,
    steptime: Val,
    cmd: Val,
    mask: Val,
    arg1: Val,
    arg2: Val,
    arg3: Val,
    step: Val,
    arg5: Val,
    arg6: Val,
}

#[derive(Debug, PartialEq)]
pub enum GrpCompositeMethod {
    Corner, // 0x01
    Copy(Val), // 0x02
    Move1(Val, Val, Val, Val, Val, Val), // 0x03
    Move2(Val, Val, Val, Val, Val, Val, Val) // 0x04
}

#[derive(Debug, PartialEq)]
pub struct GrpCompositeChild {
    file: SceneText,
    method: GrpCompositeMethod
}

#[derive(Debug, PartialEq)]
pub struct GrpComposite {
    count: u8,
    base_file: SceneText,
    idx: Val,
    children: Vec<GrpCompositeChild>
}

#[derive(Debug, PartialEq)]
pub struct GrpCompositeIndexed {
    count: u8,
    base_file: Val,
    idx: Val,
    children: Vec<GrpCompositeChild>
}

#[derive(Debug, PartialEq)]
pub enum GrpCmd {
    Load(SceneText, Val), // 0x01
    LoadEffect(GrpEffect), // 0x02
    Load2(SceneText, Val), // 0x03
    LoadEffect2(GrpEffect), // 0x04
    Load3(SceneText, Val), // 0x05
    LoadEffect3(GrpEffect), // 0x06
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

named!(pub grp_composite<&[u8], GrpComposite, CustomError<&[u8]>>,
       do_parse!(
           count: le_u8 >>
           base_file: scene_text >>
           idx: scene_value >>
               children: count!(grp_composite_child, count as usize) >>
               (GrpComposite {
                   count: count,
                   base_file: base_file,
                   idx: idx,
                   children: children
               })
       )
);

named!(pub grp_composite_indexed<&[u8], GrpCompositeIndexed, CustomError<&[u8]>>,
       do_parse!(
           count: le_u8 >>
           base_file: scene_value >>
           idx: scene_value >>
               children: count!(grp_composite_child, count as usize) >>
               (GrpCompositeIndexed {
                   count: count,
                   base_file: base_file,
                   idx: idx,
                   children: children
               })
       )
);

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

named!(pub opcode_0x0b<&[u8], Opcode, CustomError<&[u8]>>,
       do_parse!(
           a: grp_cmd >>
           (Opcode::Graphics(a))
       )
);

#[derive(Debug, PartialEq)]
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
               )
       )
);

named!(pub opcode_0x0e<&[u8], Opcode, CustomError<&[u8]>>,
       do_parse!(
           a: snd_cmd >>
           (Opcode::Sound(a))
       )
);

#[derive(Debug, PartialEq)]
pub enum Ret {
    Color(Val),
    Choice,
    DisabledChoice,
    Unknown(u8)
}

#[derive(Debug, PartialEq)]
pub enum Condition {
    IncDepth,
    DecDepth,
    And,
    Or,
    Ret(Ret),
    BitNotEq(Val, Val),
    BitEq(Val, Val),
    NotEq(Val, Val),
    Eq(Val, Val),
    FlagNotEqConst(Val, Val),
    FlagEqConst(Val, Val),
    FlagAndConst(Val, Val),
    FlagAndConst2(Val, Val),
    FlagXorConst(Val, Val),
    FlagGtConst(Val, Val),
    FlagLtConst(Val, Val),
    FlagGeqConst(Val, Val),
    FlagLeqConst(Val, Val),
    FlagNotEq(Val, Val),
    FlagEq(Val, Val),
    FlagAnd(Val, Val),
    FlagAnd2(Val, Val),
    FlagXor(Val, Val),
    FlagGt(Val, Val),
    FlagLt(Val, Val),
    FlagGeq(Val, Val),
    FlagLeq(Val, Val)
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
                        Ret::Color(val)
                    },
                    i => Ret::Unknown(i)
                };
                Condition::Ret(ret)
            },
            _ => return Err(nom::Err::Error(CustomError::MyError(format!("Unknown {}", num))))
        };

        conditions.push(cond);
    }

    Ok((inp, conditions))
}

#[derive(Debug, PartialEq)]
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

#[derive(Debug, PartialEq)]
pub enum WaitCmd {
    Wait(Val),
    WaitMouse(Val),
    SetToBase,
    WaitFromBase(Val),
    WaitFromBaseMouse(Val),
    SetToBaseVal(Val),
    Wait0x10,
    Wait0x11,
    Wait0x12,
    Wait0x13
}

named!(pub wait_cmd<&[u8], WaitCmd, CustomError<&[u8]>>,
    switch!(le_u8,
        0x01 => do_parse!(
            val: scene_value >>
            (WaitCmd::Wait(val))
        ) |
        0x02 => do_parse!(
            val: scene_value >>
            (WaitCmd::WaitMouse(val))
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

#[derive(Debug, PartialEq)]
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

#[derive(Debug, PartialEq)]
pub struct BRGRectColor {
    srcx1: Val,
    srcy1: Val,
    srcx2: Val,
    srcy2: Val,
    srcpdt: Val,
    r: Val,
    g: Val,
    b: Val,
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

#[derive(Debug, PartialEq)]
pub struct BRGRect {
    srcx1: Val,
    srcy1: Val,
    srcx2: Val,
    srcy2: Val,
    srcpdt: Val,
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

#[derive(Debug, PartialEq)]
pub struct BRGFadeOutColor {
    srcx1: Val,
    srcy1: Val,
    srcx2: Val,
    srcy2: Val,
    srcpdt: Val,
    r: Val,
    g: Val,
    b: Val,
    count: Val,
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

#[derive(Debug, PartialEq)]
pub struct BRGStretchBlit {
    srcx1: Val,
    srcy1: Val,
    srcx2: Val,
    srcy2: Val,
    srcpdt: Val,
    dstx1: Val,
    dstx2: Val,
    dsty1: Val,
    dsty2: Val,
    dstpdt: Val,
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

#[derive(Debug, PartialEq)]
pub struct BRGStretchBlitEffect {
    sx1: Val,
    sy1: Val,
    sx2: Val,
    sy2: Val,
    ex1: Val,
    ey1: Val,
    ex2: Val,
    ey2: Val,
    srcpdt: Val,
    dx1: Val,
    dy1: Val,
    dx2: Val,
    dy2: Val,
    dstpdt: Val,
    step: Val,
    steptime: Val
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

#[derive(Debug, PartialEq)]
pub enum BufferRegionGrpCmd {
    ClearRect(BRGRectColor), // 0x02
    DrawRectLine(BRGRectColor), // 0x04
    InvertColor(BRGRect), // 0x07
    ColorMask(BRGRectColor), // 0x10
    FadeOutColor(BRGRect), // 0x11
    FadeOutColor2(BRGRect), // 0x12
    FadeOutColor3(BRGFadeOutColor), // 0x12
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
               0x12 => do_parse!(a: brg_fade_out_color >> (BufferRegionGrpCmd::FadeOutColor3(a))) |
               0x20 => do_parse!(a: brg_rect >> (BufferRegionGrpCmd::MakeMonoImage(a))) |
               0x30 => do_parse!(a: brg_stretch_blit >> (BufferRegionGrpCmd::StretchBlit(a))) |
               0x32 => do_parse!(a: brg_stretch_blit_effect >> (BufferRegionGrpCmd::StretchBlitEffect(a)))
       )
);

#[derive(Debug, PartialEq)]
pub struct BGCopySamePos {
    srcx1: Val,
    srcy1: Val,
    srcx2: Val,
    srcy2: Val,
    srcpdt: Val,
    flag: Val,
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

#[derive(Debug, PartialEq)]
pub struct BGCopyNewPos {
    srcx1: Val,
    srcy1: Val,
    srcx2: Val,
    srcy2: Val,
    srcpdt: Val,
    dstx1: Val,
    dsty1: Val,
    dstpdt: Val,
    flag: Option<Val>
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

#[derive(Debug, PartialEq)]
pub struct BGCopyColor {
    srcx1: Val,
    srcy1: Val,
    srcx2: Val,
    srcy2: Val,
    srcpdt: Val,
    dstx1: Val,
    dsty1: Val,
    dstpdt: Val,
    r: Val,
    g: Val,
    b: Val
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

#[derive(Debug, PartialEq)]
pub struct BGSwap {
    srcx1: Val,
    srcy1: Val,
    srcx2: Val,
    srcy2: Val,
    srcpdt: Val,
    dstx1: Val,
    dsty1: Val,
    dstpdt: Val,
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

#[derive(Debug, PartialEq)]
pub struct BGCopyWithMask {
    srcx1: Val,
    srcy1: Val,
    srcx2: Val,
    srcy2: Val,
    srcpdt: Val,
    dstx1: Val,
    dsty1: Val,
    dstpdt: Val,
    flag: Val
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

#[derive(Debug, PartialEq)]
pub struct BGCopyWholeScreen {
    srcpdt: Val,
    dstpdt: Val,
    flag: Option<Val>
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

#[derive(Debug, PartialEq)]
pub struct BGDisplayStrings {
    n: Val,
    srcx1: Val,
    srcy1: Val,
    srcx2: Val,
    srcy2: Val,
    srcdx: Val,
    srcdy: Val,
    srcpdt: Val,
    dstx1: Val,
    dsty1: Val,
    dstx2: Val,
    dsty2: Val,
    count: Val,
    zero: Val,
    dstpdt: Val,
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

#[derive(Debug, PartialEq)]
pub struct BGDisplayStringsMask {
    n: Val,
    srcx1: Val,
    srcy1: Val,
    srcx2: Val,
    srcy2: Val,
    srcdx: Val,
    srcdy: Val,
    srcpdt: Val,
    dstx1: Val,
    dsty1: Val,
    dstx2: Val,
    dsty2: Val,
    count: Val,
    zero: Val,
    dstpdt: Val,
    flag: Val,
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

#[derive(Debug, PartialEq)]
pub struct BGDisplayStringsColor {
    n: Val,
    srcx1: Val,
    srcy1: Val,
    srcx2: Val,
    srcy2: Val,
    srcdx: Val,
    srcdy: Val,
    srcpdt: Val,
    dstx1: Val,
    dsty1: Val,
    dstx2: Val,
    dsty2: Val,
    count: Val,
    zero: Val,
    dstpdt: Val,
    r: Val,
    g: Val,
    b: Val
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

#[derive(Debug, PartialEq)]
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

#[derive(Debug, PartialEq)]
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

#[derive(Debug, PartialEq)]
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

#[derive(Debug, PartialEq)]
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

#[derive(Debug, PartialEq)]
pub enum MouseCtrlCmd {
    WaitForClick, // 0x01
    SetPos(Val, Val, Val), // 0x02
    FlushData, // 0x03
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
               0x03 => value!(MouseCtrlCmd::FlushData) |
               0x20 => value!(MouseCtrlCmd::CursorOff) |
               0x21 => value!(MouseCtrlCmd::CursorOn)
       )
);

#[derive(Debug, PartialEq)]
pub enum SetVolCmd {
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

named!(pub set_vol_cmd<&[u8], SetVolCmd, CustomError<&[u8]>>,
       switch!(le_u8,
               0x01 => do_parse!(a: scene_value >> (SetVolCmd::GetBgmVolume(a))) |
               0x02 => do_parse!(a: scene_value >> (SetVolCmd::GetWavVolume(a))) |
               0x03 => do_parse!(a: scene_value >> (SetVolCmd::GetKoeVolume(a))) |
               0x04 => do_parse!(a: scene_value >> (SetVolCmd::GetSeVolume(a))) |
               0x11 => do_parse!(a: scene_value >> (SetVolCmd::SetBgmVolume(a))) |
               0x12 => do_parse!(a: scene_value >> (SetVolCmd::SetWavVolume(a))) |
               0x13 => do_parse!(a: scene_value >> (SetVolCmd::SetKoeVolume(a))) |
               0x14 => do_parse!(a: scene_value >> (SetVolCmd::SetSeVolume(a))) |
               0x21 => do_parse!(a: scene_value >> (SetVolCmd::MuteBgm(a))) |
               0x22 => do_parse!(a: scene_value >> (SetVolCmd::MuteWav(a))) |
               0x23 => do_parse!(a: scene_value >> (SetVolCmd::MuteKoe(a))) |
               0x24 => do_parse!(a: scene_value >> (SetVolCmd::MuteSe(a)))
       )
);

#[derive(Debug, PartialEq)]
pub enum NovelModeCmd {
    SetEnabled(Val), // 0x01
    Unknown1(Val), // 0x02
    Unknown2(Val), // 0x03
    Unknown3, // 0x04
    Unknown4, // 0x05
}

named!(pub novel_mode_cmd<&[u8], NovelModeCmd, CustomError<&[u8]>>,
       switch!(le_u8,
               0x01 => do_parse!(a: scene_value >> (NovelModeCmd::SetEnabled(a))) |
               0x02 => do_parse!(a: scene_value >> (NovelModeCmd::Unknown1(a))) |
               0x03 => do_parse!(a: scene_value >> (NovelModeCmd::Unknown2(a))) |
               0x04 => value!(NovelModeCmd::Unknown3) |
               0x05 => value!(NovelModeCmd::Unknown4)
       )
);

#[derive(Debug, PartialEq)]
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

#[derive(Debug, PartialEq)]
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

#[derive(Debug, PartialEq)]
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

#[derive(Debug, PartialEq)]
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

#[derive(Debug, PartialEq)]
pub enum Opcode {
    /// Wait For Mouse
    WaitMouse, // 0x01
    Newline, // 0x02
    Op0x03,
    /// Text Window Command
    TextWin(TextWinCmd), // 0x04
    Op0x05,
    Op0x06,
    Op0x08,
    Graphics(GrpCmd), // 0x0b
    Op0x0c,
    Sound(SndCmd), // 0x0e
    Op0x10,
    /// Fade In/Out
    Fade(FadeCmd), // 0x13
    /// Conditional Jump
    Condition(Vec<Condition>, Pos), // 0x15
    JumpToScene(Val), // 0x16
    ScreenShake(ScreenShakeCmd), // 0x17
    Op0x18,
    Wait(WaitCmd), // 0x19
    Op0x1a,
    Call(Pos), // 0x1b
    Jump(Pos), // 0x1c
    TableCall(u8, Val), // 0x1d
    TableJump(u8, Val), // 0x1e
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
    Op0x2e,
    Op0x2f,
    Op0x30,
    Op0x31,
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
    SetFlagRandom(Val, Val), // 0x56
    SetValRandom(Val, Val), // 0x57
    Op0x58,
    Op0x59,
    Op0x5b,
    Op0x5c,
    Op0x5d,
    Op0x5e,
    Op0x5f,
    System(SystemCmd), // 0x60
    Op0x61,
    Op0x63,
    BufferRegion(BufferRegionGrpCmd), // 0x64
    /// ???
    Unknown0x65, // 0x65
    /// Buffer Copy/Display
    Buffer(BufferGrpCmd), // 0x67
    Flash(FlashGrpCmd), // 0x68
    Op0x69,
    Op0x6a,
    Op0x66,
    AreaBuffer(AreaBufferCmd), // 0x6c
    MouseCtrl(MouseCtrlCmd), // 0x6d
    Op0x6e,
    Op0x6f,
    WindowVar(WindowVarCmd), // 0x70
    MessageWin(MessageWinCmd), // 0x72
    SystemVar(SystemVarCmd), // 0x73
    PopupMenu(PopupMenuCmd), // 0x74
    SetVol(SetVolCmd), // 0x75
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

named!(pub opcode_0x04<&[u8], Opcode, CustomError<&[u8]>>,
       do_parse!(
           a: text_win_cmd >>
           (Opcode::TextWin(a))
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
               b: le_u32 >>
               (Opcode::Condition(a, b))
       )
);

named!(pub opcode_0x16<&[u8], Opcode, CustomError<&[u8]>>,
       do_parse!(
        a: scene_value >>
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
       // TODO
       // a: le_u32 >>
       // (Opcode::Call(a))
       value!(Opcode::Call(0))
);

named!(pub opcode_0x1c<&[u8], Opcode, CustomError<&[u8]>>,
       do_parse!(
           a: le_u32 >>
           (Opcode::Jump(a))
       )
);

named!(pub opcode_0x1d<&[u8], Opcode, CustomError<&[u8]>>,
       do_parse!(
           a: le_u8 >>
           b: scene_value >>
           (Opcode::TableCall(a, b))
       )
);

named!(pub opcode_0x1e<&[u8], Opcode, CustomError<&[u8]>>,
       do_parse!(
           a: le_u8 >>
           b: scene_value >>
           (Opcode::TableJump(a, b))
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
               b: scene_value >>
               (Opcode::SetFlagRandom(a, b))
       )
);

named!(pub opcode_0x57<&[u8], Opcode, CustomError<&[u8]>>,
       do_parse!(
           a: scene_value >>
               b: scene_value >>
               (Opcode::SetValRandom(a, b))
       )
);

named!(pub opcode_0x60<&[u8], Opcode, CustomError<&[u8]>>,
       do_parse!(
           a: system_cmd >>
               (Opcode::System(a))
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
           (Opcode::SetVol(a))
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
               // 0x03 => value!(Opcode::Op0x03) |
               0x04 => call!(opcode_0x04) |
               // 0x05 => value!(Opcode::Op0x05) |
               // 0x06 => value!(Opcode::Op0x06) |
               // 0x08 => value!(Opcode::Op0x08) |
               0x0b => call!(opcode_0x0b) |
               // 0x0c => value!(Opcode::Op0x0c) |
               0x0e => call!(opcode_0x0e) |
               // 0x10 => value!(Opcode::Op0x10) |
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
               // 0x2e => value!(Opcode::Op0x2e) |
               // 0x2f => value!(Opcode::Op0x2f) |
               // 0x30 => value!(Opcode::Op0x30) |
               // 0x31 => value!(Opcode::Op0x31) |
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
               // 0x58 => value!(Opcode::Op0x58) |
               // 0x59 => value!(Opcode::Op0x59) |
               // 0x5b => value!(Opcode::Op0x5b) |
               // 0x5c => value!(Opcode::Op0x5c) |
               // 0x5d => value!(Opcode::Op0x5d) |
               // 0x5e => value!(Opcode::Op0x5e) |
               // 0x5f => value!(Opcode::Op0x5f) |
               0x60 => call!(opcode_0x60) |
               // 0x61 => value!(Opcode::Op0x61) |
               // 0x63 => value!(Opcode::Op0x63) |
               0x64 => call!(opcode_0x64) |
               0x64 => call!(opcode_0x65) |
               0x67 => call!(opcode_0x67) |
               0x68 => call!(opcode_0x68) |
               // 0x69 => value!(Opcode::Op0x69) |
               // 0x6a => value!(Opcode::Op0x6a) |
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
               // dbg_dmp!(tag!("\0")) >>
               // eof!() >>
               (AVG32Scene {
                   header: header,
                   opcodes: opcodes
               })
       )
);
