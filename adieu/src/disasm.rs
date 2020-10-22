use avg32::parser::{AVG32Scene, Header, Pos, Opcode};
use avg32::write::Writeable;
use std::collections::HashMap;
use anyhow::{anyhow, Result};

#[derive(Debug, PartialEq, Eq, PartialOrd, Ord)]
enum LabelKind {
    Condition,
    Call,
    Jump,
    TableCall,
    TableJump,
}

#[derive(Debug, PartialEq, Eq, PartialOrd, Ord)]
struct LabelPos {
    kind: LabelKind,
    pos: Pos
}

impl LabelPos {
    fn new(kind: LabelKind, pos: Pos) -> Self {
        LabelPos {
            kind: kind,
            pos: pos
        }
    }
}

fn extract_label(opcode: &Opcode) -> Option<Vec<LabelPos>> {
    match opcode {
        Opcode::Condition(_, pos) => Some(vec![LabelPos::new(LabelKind::Condition, pos.clone())]),
        Opcode::Call(pos) => Some(vec![LabelPos::new(LabelKind::Call, pos.clone())]),
        Opcode::Jump(pos) => Some(vec![LabelPos::new(LabelKind::Jump, pos.clone())]),
        Opcode::TableCall(_, poss) => {
            let mut res = vec![];
            for pos in poss.iter() {
                res.push(LabelPos::new(LabelKind::TableCall, pos.clone()))
            }
            Some(res)
        },
        Opcode::TableJump(_, poss) => {
            let mut res = vec![];
            for pos in poss.iter() {
                res.push(LabelPos::new(LabelKind::TableJump, pos.clone()))
            }
            Some(res)
        },
        _ => None
    }
}

#[derive(Serialize, Deserialize, Debug, PartialEq)]
struct Label {
    name: String,
    opcodes: Vec<Opcode>
}

#[derive(Serialize, Deserialize, Debug, PartialEq)]
struct LabelResolvedScene {
    header: Header,
    labels: Vec<Label>
}

fn extract_labels(opcodes: &[Opcode]) -> Vec<LabelPos> {
    opcodes.iter().map(extract_label).filter(|x| x.is_some()).map(|x| x.unwrap()).flatten().collect()
}

fn resolve_labels(scene: &AVG32Scene) -> Result<LabelResolvedScene> {
    let mut labels = extract_labels(&scene.opcodes);
    labels.sort();

    let mut positions: HashMap<u32, Label> = HashMap::new();

    positions.insert(0, Label {
        name: String::from("start"),
        opcodes: Vec::new()
    });

    for label in labels.into_iter() {
        if let Pos::Byte(pos) = label.pos {
            if !positions.contains_key(&pos) {
                positions.insert(pos, Label {
                    name: format!("{:?}_0x{:x?}", label.kind, pos).to_lowercase(),
                    opcodes: Vec::new()
                });
            }
        } else {
            return Err(anyhow!("Labels were already resolved"));
        }
    }

    let mut offsets: Vec<u32> = positions.keys().cloned().collect();
    offsets.sort();
    let mut offset_iter = offsets.iter();
    let mut offset = offset_iter.next();
    let mut next_offset = offset_iter.next();
    let mut cur_pos = 0;
    let mut cur_label = positions.get_mut(&cur_pos).unwrap();

    let start_pos = scene.header.byte_size() as u32;

    for opcode in scene.opcodes.iter() {
        match next_offset {
            Some(noff) => {
                if cur_pos < *noff {
                    debug!("{:04x?}-{:04x}: 0x{:04x?} (0x{:04x?}) + 0x{:02x?} - {:x?}", offset.unwrap() + start_pos, *next_offset.unwrap_or(&0) + start_pos, cur_pos + start_pos, cur_pos, opcode.byte_size(), opcode);
                    cur_label.opcodes.push(opcode.clone());
                    cur_pos += opcode.byte_size() as u32;
                } else if cur_pos == *noff {
                    cur_label = positions.get_mut(noff).unwrap();
                    debug!("    {}:", cur_label.name);
                    debug!("{:04x?}-{:04x}: 0x{:04x?} (0x{:04x?}) + 0x{:02x?} - {:x?}", offset.unwrap() + start_pos, *next_offset.unwrap_or(&0) + start_pos, cur_pos + start_pos, cur_pos, opcode.byte_size(), opcode);
                    cur_label.opcodes.push(opcode.clone());
                    offset = next_offset;
                    next_offset = offset_iter.next();
                    cur_pos += opcode.byte_size() as u32;
                } else {
                    return Err(anyhow!("Misaligned opcode at pos 0x{:04x?}: offset 0x{:04x?} opcode {:x?}", cur_pos, offset, opcode));
                }
            },
            None => {
                debug!("{:04x?}-{:04x}: 0x{:04x?} (0x{:04x?}) + 0x{:02x?} - {:x?}", offset.unwrap() + start_pos, *next_offset.unwrap_or(&0) + start_pos, cur_pos + start_pos, cur_pos, opcode.byte_size(), opcode);
                cur_label.opcodes.push(opcode.clone());
                cur_pos += opcode.byte_size() as u32;
            }
        }
    }

    Ok(LabelResolvedScene {
        header: scene.header.clone(),
        labels: positions.into_values().collect()
    })
}

pub fn disassemble(scene: &AVG32Scene) -> Result<String> {
    let opts = serde_lexpr::print::Options::elisp();

    let resolved = resolve_labels(&scene)?;

    let sexp = serde_lexpr::to_string_custom(scene, opts).unwrap();
    Ok(sexp)
}

pub fn assemble(sexp: &str) -> AVG32Scene {
    serde_lexpr::from_str(sexp).unwrap()
}
