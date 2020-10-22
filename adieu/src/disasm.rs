use avg32::parser::{AVG32Scene, Header, Pos, Opcode};
use avg32::write::Writeable;
use std::collections::HashMap;
use anyhow::{anyhow, Result};

#[derive(Debug, PartialEq, Eq, PartialOrd, Ord, Clone)]
enum LabelKind {
    Condition,
    Call,
    Jump,
    TableCall,
    TableJump,
}

#[derive(Debug, PartialEq, Eq, PartialOrd, Ord, Clone)]
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

#[derive(Serialize, Deserialize, Debug, PartialEq, Clone)]
struct Label {
    name: String,
    opcodes: Vec<Opcode>
}

#[derive(Serialize, Deserialize, Debug, PartialEq)]
struct LabelResolvedScene {
    header: Header,
    labels: Vec<Label>
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
        if let Pos::Offset(pos) = label.pos {
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

    let mut resolved_labels = Vec::new();
    for offset in offsets.iter() {
        resolved_labels.push(positions.get(offset).unwrap().clone());
    }

    for label in resolved_labels.iter_mut() {
        convert_byte_to_label_positions(&mut label.opcodes, &positions);
    }

    Ok(LabelResolvedScene {
        header: scene.header.clone(),
        labels: resolved_labels
    })
}

fn convert_byte_to_label_positions(opcodes: &mut [Opcode], positions: &HashMap<u32, Label>) {
    for opcode in opcodes.iter_mut() {
        match opcode {
            Opcode::Condition(_, ref mut pos) => {
                if let Pos::Offset(b) = pos {
                    let label = positions.get(b).unwrap();
                    *pos = Pos::Label(label.name.clone());
                } else {
                    unreachable!()
                }
            },
            Opcode::Call(ref mut pos) => {
                if let Pos::Offset(b) = pos {
                    let label = positions.get(b).unwrap();
                    *pos = Pos::Label(label.name.clone());
                } else {
                    unreachable!()
                }
            },
            Opcode::Jump(ref mut pos) => {
                if let Pos::Offset(b) = pos {
                    let label = positions.get(b).unwrap();
                    *pos = Pos::Label(label.name.clone());
                } else {
                    unreachable!()
                }
            },
            Opcode::TableCall(_, poss) => {
                for pos in poss.iter_mut() {
                    if let Pos::Offset(b) = pos {
                        let label = positions.get(b).unwrap();
                        *pos = Pos::Label(label.name.clone());
                    } else {
                        unreachable!()
                    }
                }
            },
            Opcode::TableJump(_, poss) => {
                for pos in poss.iter_mut() {
                    if let Pos::Offset(b) = pos {
                        let label = positions.get(b).unwrap();
                        *pos = Pos::Label(label.name.clone());
                    } else {
                        unreachable!()
                    }
                }
            },
            _ => ()
        }
    }
}

fn compile_labels(resolved: &LabelResolvedScene) -> Result<AVG32Scene> {
    let mut opcodes = Vec::new();
    let mut positions: HashMap<String, u32> = HashMap::new();
    let mut cur_pos = 0;

    for label in resolved.labels.iter() {
        positions.insert(label.name.clone(), cur_pos);
        for opcode in label.opcodes.iter() {
            opcodes.push(opcode.clone());
            cur_pos += opcode.byte_size() as u32;
        }
    }

    convert_label_to_byte_positions(&mut opcodes, &positions);

    Ok(AVG32Scene {
        header: resolved.header.clone(),
        opcodes: opcodes
    })
}

fn convert_label_to_byte_positions(opcodes: &mut [Opcode], positions: &HashMap<String, u32>) {
    for opcode in opcodes.iter_mut() {
        match opcode {
            Opcode::Condition(_, ref mut pos) => {
                if let Pos::Label(name) = pos {
                    let b = positions.get(name).unwrap();
                    *pos = Pos::Offset(*b);
                } else {
                    unreachable!()
                }
            },
            Opcode::Call(ref mut pos) => {
                if let Pos::Label(name) = pos {
                    let b = positions.get(name).unwrap();
                    *pos = Pos::Offset(*b);
                } else {
                    unreachable!()
                }
            },
            Opcode::Jump(ref mut pos) => {
                if let Pos::Label(name) = pos {
                    let b = positions.get(name).unwrap();
                    *pos = Pos::Offset(*b);
                } else {
                    unreachable!()
                }
            },
            Opcode::TableCall(_, poss) => {
                for pos in poss.iter_mut() {
                    if let Pos::Label(name) = pos {
                        let b = positions.get(name).unwrap();
                        *pos = Pos::Offset(*b);
                    } else {
                        unreachable!()
                    }
                }
            },
            Opcode::TableJump(_, poss) => {
                for pos in poss.iter_mut() {
                    if let Pos::Label(name) = pos {
                        let b = positions.get(name).unwrap();
                        *pos = Pos::Offset(*b);
                    } else {
                        unreachable!()
                    }
                }
            },
            _ => ()
        }
    }
}

pub fn disassemble(scene: &AVG32Scene) -> Result<String> {
    let resolved = resolve_labels(&scene)?;

    let sexp = serde_lexpr::to_string(&resolved).unwrap();
    Ok(sexp)
}

pub fn assemble(sexp: &str) -> Result<AVG32Scene> {
    let resolved = serde_lexpr::from_str(sexp).unwrap();

    let scene = compile_labels(&resolved)?;

    Ok(scene)
}

#[cfg(test)]
mod tests {
    use avg32;
    use super::*;
    use pretty_assertions::assert_eq;

    #[test]
    fn test_roundtrip_scene() {
        use std::fs;
        for entry in fs::read_dir("../SEEN").unwrap() {
            let entry = entry.unwrap();
            let path = entry.path();
            println!("{:?}", path);

            let metadata = fs::metadata(&path).unwrap();
            if metadata.is_file() {
                let scene = avg32::load(&path.to_str().unwrap()).unwrap();

                let disasm = disassemble(&scene).unwrap();
                assert_eq!(scene, assemble(&disasm).unwrap());
            }
        }
    }
}
