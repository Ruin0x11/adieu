use nom::IResult;
use nom::number::streaming::{le_u8, le_u32};

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

named!(c_string<&str>,
    do_parse!(
        s: map_res!(take_until!("\0"), std::str::from_utf8) >>
        tag!("\0") >>
        (s)
    )
);

fn menu_strings<'a, 'b>(input: &'a [u8], menus: &'b [Menu]) -> IResult<&'a [u8], Vec<String>> {
    let mut str_count = 0;
    for menu in menus {
        str_count = str_count + 1;
        for _ in menu.submenus.iter() {
            str_count = str_count + 1;
        }
    }

    nom::multi::count(c_string, str_count)(input).map(|(i, s)| (i, s.iter().map(|s| String::from(*s)).collect()))
}

named!(pub header<Header>,
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

named!(pub menu<Menu>,
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

named!(pub submenu<Submenu>,
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

named!(pub flag<Flag>,
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

pub type Val = u32;

fn scene_value(input: &[u8]) -> IResult<&[u8], Val> {
    let num = input[0];
    let l = ((num >> 4) & 7) as usize;
    let mut ret: u32 = 0;
    for i in (0..l).rev() {
        println!("{}", input[i]);
        ret <<= 4;
        ret |= input[i] as u32;
    }
    println!("val {} {} {}", num, l, ret);
    Ok((&input[l..], ret))
}

#[derive(Debug, PartialEq)]
pub enum SceneText {
    Pointer(Val),
    Literal(String)
}

fn scene_text(input: &[u8]) -> IResult<&[u8], SceneText> {
    if input[0] == 0x40 {
        let (inp, val) = scene_value(input)?;
        Ok((inp, SceneText::Pointer(val)))
    } else {
        let (inp, val) = c_string(input)?;
        Ok((inp, SceneText::Literal(String::from(val))))
    }
}

#[derive(Debug, PartialEq)]
pub enum Opcode {
    Op0x01,
    Op0x02,
    Op0x03,
    Op0x04,
    Op0x05,
    Op0x06,
    Op0x08,
    Op0x0b(GrpCmd),
    Op0x0c,
    Op0x0e(SndCmd),
    Op0x10,
    Op0x13,
    Op0x15(Vec<Condition>, u32),
    Op0x16,
    Op0x17,
    Op0x18,
    Op0x19(WaitCmd),
    Op0x1a,
    Op0x1b,
    Op0x1c(u32),
    Op0x1d,
    Op0x1e,
    Op0x20,
    Op0x22,
    Op0x23,
    Op0x24,
    Op0x25,
    Op0x26,
    Op0x27,
    Op0x28,
    Op0x29,
    Op0x2c,
    Op0x2d,
    Op0x2e,
    Op0x2f,
    Op0x30,
    Op0x31,
    Op0x37(Val, Val),
    Op0x39,
    Op0x3b,
    Op0x3c,
    Op0x3d,
    Op0x3e,
    Op0x3f,
    Op0x40,
    Op0x41,
    Op0x42,
    Op0x43,
    Op0x49,
    Op0x4a,
    Op0x4b,
    Op0x4c,
    Op0x4d,
    Op0x4e,
    Op0x4f,
    Op0x50,
    Op0x51,
    Op0x56,
    Op0x57,
    Op0x58,
    Op0x59,
    Op0x5b,
    Op0x5c,
    Op0x5d,
    Op0x5e,
    Op0x5f,
    Op0x60,
    Op0x61,
    Op0x63,
    Op0x64,
    Op0x65,
    Op0x67,
    Op0x68,
    Op0x69,
    Op0x6a,
    Op0x66,
    Op0x6c,
    Op0x6d,
    Op0x6e,
    Op0x6f,
    Op0x70,
    Op0x72,
    Op0x73,
    Op0x74,
    Op0x75,
    Op0x76,
    Op0x7f,
    Op0xfe,
    Op0xff,
}

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

named!(pub grp_effect<GrpEffect>,
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

fn grp_composite_child(input: &[u8]) -> IResult<&[u8], GrpCompositeChild> {
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
        _ => unreachable!()
    };

    let child = GrpCompositeChild {
        file: file,
        method: method
    };

    Ok((inp, child))
}

named!(pub grp_composite<GrpComposite>,
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

named!(pub grp_composite_indexed<GrpCompositeIndexed>,
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

named!(pub grp_cmd<GrpCmd>,
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

named!(pub opcode_0x0b<Opcode>,
       do_parse!(
           a: grp_cmd >>
           (Opcode::Op0x0b(a))
       )
);

#[derive(Debug, PartialEq)]
pub enum SndCmd {
    BgmLoop(SceneText), // 0x01
    BgmWait(SceneText), // 0x02
    BgmOnce(SceneText), // 0x03
    BgmFadeInLoop(SceneText), // 0x05
    BgmFadeInWait(SceneText), // 0x06
    BgmFadeInOnce(SceneText), // 0x07
    BgmFadeOut(SceneText), // 0x10
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

named!(pub snd_cmd<SndCmd>,
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
                   (SndCmd::BgmFadeInLoop(a))
               ) |
               0x06 => do_parse!(
                   a: scene_text >>
                   (SndCmd::BgmFadeInWait(a))
               ) |
               0x07 => do_parse!(
                   a: scene_text >>
                   (SndCmd::BgmFadeInOnce(a))
               ) |
               0x10 => do_parse!(
                   a: scene_text >>
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

named!(pub opcode_0x0e<Opcode>,
       do_parse!(
           a: snd_cmd >>
           (Opcode::Op0x0e(a))
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

fn scene_conditions(input: &[u8]) -> IResult<&[u8], Vec<Condition>> {
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
            _ => unreachable!()
        };

        conditions.push(cond);
    }

    Ok((inp, conditions))
}

named!(pub opcode_0x15<Opcode>,
       do_parse!(
           a: scene_conditions >>
               b: le_u32 >>
               (Opcode::Op0x15(a, b))
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

named!(pub wait_cmd<WaitCmd>,
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

named!(pub opcode_0x19<Opcode>,
       do_parse!(
           a: wait_cmd >>
           (Opcode::Op0x19(a))
       )
);

named!(pub opcode_0x1c<Opcode>,
       do_parse!(
           a: le_u32 >>
           (Opcode::Op0x1c(a))
       )
);

named!(pub opcode_0x37<Opcode>,
       do_parse!(
           a: scene_value >>
               b: scene_value >>
               (Opcode::Op0x37(a, b))
       )
);

named!(pub opcode<Opcode>,
       switch!(le_u8,
               0x01 => value!(Opcode::Op0x01) |
               0x02 => value!(Opcode::Op0x02) |
               0x03 => value!(Opcode::Op0x03) |
               0x04 => value!(Opcode::Op0x04) |
               0x05 => value!(Opcode::Op0x05) |
               0x06 => value!(Opcode::Op0x06) |
               0x08 => value!(Opcode::Op0x08) |
               0x0b => call!(opcode_0x0b) |
               0x0c => value!(Opcode::Op0x0c) |
               0x0e => call!(opcode_0x0e) |
               0x10 => value!(Opcode::Op0x10) |
               0x13 => value!(Opcode::Op0x13) |
               0x15 => call!(opcode_0x15) |
               0x16 => value!(Opcode::Op0x16) |
               0x17 => value!(Opcode::Op0x17) |
               0x18 => value!(Opcode::Op0x18) |
               0x19 => call!(opcode_0x19) |
               0x1a => value!(Opcode::Op0x1a) |
               0x1b => value!(Opcode::Op0x1b) |
               0x1c => call!(opcode_0x1c) |
               0x1d => value!(Opcode::Op0x1d) |
               0x1e => value!(Opcode::Op0x1e) |
               0x20 => value!(Opcode::Op0x20) |
               0x22 => value!(Opcode::Op0x22) |
               0x23 => value!(Opcode::Op0x23) |
               0x24 => value!(Opcode::Op0x24) |
               0x25 => value!(Opcode::Op0x25) |
               0x26 => value!(Opcode::Op0x26) |
               0x27 => value!(Opcode::Op0x27) |
               0x28 => value!(Opcode::Op0x28) |
               0x29 => value!(Opcode::Op0x29) |
               0x2c => value!(Opcode::Op0x2c) |
               0x2d => value!(Opcode::Op0x2d) |
               0x2e => value!(Opcode::Op0x2e) |
               0x2f => value!(Opcode::Op0x2f) |
               0x30 => value!(Opcode::Op0x30) |
               0x31 => value!(Opcode::Op0x31) |
               0x37 => call!(opcode_0x37) |
               0x39 => value!(Opcode::Op0x39) |
               0x3b => value!(Opcode::Op0x3b) |
               0x3c => value!(Opcode::Op0x3c) |
               0x3d => value!(Opcode::Op0x3d) |
               0x3e => value!(Opcode::Op0x3e) |
               0x3f => value!(Opcode::Op0x3f) |
               0x40 => value!(Opcode::Op0x40) |
               0x41 => value!(Opcode::Op0x41) |
               0x42 => value!(Opcode::Op0x42) |
               0x43 => value!(Opcode::Op0x43) |
               0x49 => value!(Opcode::Op0x49) |
               0x4a => value!(Opcode::Op0x4a) |
               0x4b => value!(Opcode::Op0x4b) |
               0x4c => value!(Opcode::Op0x4c) |
               0x4d => value!(Opcode::Op0x4d) |
               0x4e => value!(Opcode::Op0x4e) |
               0x4f => value!(Opcode::Op0x4f) |
               0x50 => value!(Opcode::Op0x50) |
               0x51 => value!(Opcode::Op0x51) |
               0x56 => value!(Opcode::Op0x56) |
               0x57 => value!(Opcode::Op0x57) |
               0x58 => value!(Opcode::Op0x58) |
               0x59 => value!(Opcode::Op0x59) |
               0x5b => value!(Opcode::Op0x5b) |
               0x5c => value!(Opcode::Op0x5c) |
               0x5d => value!(Opcode::Op0x5d) |
               0x5e => value!(Opcode::Op0x5e) |
               0x5f => value!(Opcode::Op0x5f) |
               0x60 => value!(Opcode::Op0x60) |
               0x61 => value!(Opcode::Op0x61) |
               0x63 => value!(Opcode::Op0x63) |
               0x64 => value!(Opcode::Op0x64) |
               0x65 => value!(Opcode::Op0x65) |
               0x67 => value!(Opcode::Op0x67) |
               0x68 => value!(Opcode::Op0x68) |
               0x69 => value!(Opcode::Op0x69) |
               0x6a => value!(Opcode::Op0x6a) |
               0x66 => value!(Opcode::Op0x66) |
               0x6c => value!(Opcode::Op0x6c) |
               0x6d => value!(Opcode::Op0x6d) |
               0x6e => value!(Opcode::Op0x6e) |
               0x6f => value!(Opcode::Op0x6f) |
               0x70 => value!(Opcode::Op0x70) |
               0x72 => value!(Opcode::Op0x72) |
               0x73 => value!(Opcode::Op0x73) |
               0x74 => value!(Opcode::Op0x74) |
               0x75 => value!(Opcode::Op0x75) |
               0x76 => value!(Opcode::Op0x76) |
               0x7f => value!(Opcode::Op0x7f) |
               0xfe => value!(Opcode::Op0xfe) |
               0xff => value!(Opcode::Op0xff)
       )
);

named!(pub avg32_scene<AVG32Scene>,
       do_parse!(
           header: header >>
           // opcodes: many0!(opcode) >>
               opcodes: dbg_dmp!(many1!(opcode)) >>
               (AVG32Scene {
                   header: header,
                   opcodes: opcodes
               })
       )
);
