use avg32::parser::{AVG32Scene, Val, Pos, Opcode};

#[derive(Debug, PartialEq)]
enum LabelKind {
    Condition,
    Call,
    Jump,
    TableCall,
    TableJump,
}

#[derive(Debug, PartialEq)]
struct LabelPos {
    kind: LabelKind,
    offset: Pos
}

impl LabelPos {
    fn new(kind: LabelKind, offset: Pos) -> Self {
        LabelPos {
            kind: kind,
            offset: offset
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

fn extract_labels(opcodes: &[Opcode]) -> Vec<LabelPos> {
    opcodes.iter().map(extract_label).filter(|x| x.is_some()).map(|x| x.unwrap()).flatten().collect()
}

pub fn disassemble(scene: &AVG32Scene) -> String {
    let opts = serde_lexpr::print::Options::elisp();

    println!("{:#02x?}", extract_labels(&scene.opcodes));

    serde_lexpr::to_string_custom(scene, opts).unwrap()
}

pub fn reassemble(sexp: &str) -> AVG32Scene {
    serde_lexpr::from_str(sexp).unwrap()
}
