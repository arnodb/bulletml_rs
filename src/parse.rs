use failure::Backtrace;
use indextree::{Arena, NodeId};
use meval::Expr;
use std::collections::HashMap;
use std::fs;
use std::io::prelude::*;
use std::path;

use crate::tree::{BulletML, BulletMLNode, BulletMLType, DirectionType, HVType, SpeedType};

#[derive(Fail, Debug)]
pub enum Error {
    #[fail(display = "Failure error")]
    Failure(#[cause] failure::Error, Backtrace),
    #[fail(display = "Xml error")]
    Xml(#[cause] roxmltree::Error, Backtrace),
    #[fail(display = "I/O error")]
    InputOutput(#[cause] std::io::Error, Backtrace),
    #[fail(display = "BulletML error")]
    BulletML(String, Backtrace),
    #[fail(display = "Expression error")]
    Expression(meval::Error, Backtrace),
}

impl From<failure::Error> for Error {
    fn from(inner: failure::Error) -> Self {
        Error::Failure(inner, Backtrace::new())
    }
}

impl From<roxmltree::Error> for Error {
    fn from(inner: roxmltree::Error) -> Self {
        Error::Xml(inner, Backtrace::new())
    }
}

impl From<std::io::Error> for Error {
    fn from(inner: std::io::Error) -> Self {
        Error::InputOutput(inner, Backtrace::new())
    }
}

impl From<meval::Error> for Error {
    fn from(inner: meval::Error) -> Self {
        Error::Expression(inner, Backtrace::new())
    }
}

pub struct BulletMLParser {
    arena: Arena<BulletMLNode>,
    bullet_refs: HashMap<String, NodeId>,
    action_refs: HashMap<String, NodeId>,
    fire_refs: HashMap<String, NodeId>,
}

impl BulletMLParser {
    pub fn parse(s: &str) -> Result<BulletML, Error> {
        let doc = roxmltree::Document::parse(s)?;
        let mut parser = BulletMLParser {
            arena: Arena::new(),
            bullet_refs: HashMap::new(),
            action_refs: HashMap::new(),
            fire_refs: HashMap::new(),
        };
        let root = doc.root_element();
        let root_name = root.tag_name();
        match root_name.name() {
            "bulletml" => {
                let root_id = parser.parse_bulletml(root)?;
                Ok(BulletML {
                    arena: parser.arena,
                    root: root_id,
                    bullet_refs: parser.bullet_refs,
                    action_refs: parser.action_refs,
                    fire_refs: parser.fire_refs,
                })
            }
            _ => Err(Error::BulletML(
                format!("Expected bulletml but got {}", root.tag_name().name()),
                Backtrace::new(),
            )),
        }
    }

    pub fn parse_file(path: &path::Path) -> Result<BulletML, Error> {
        let mut file = fs::File::open(&path)?;
        let mut text = String::new();
        file.read_to_string(&mut text)?;
        BulletMLParser::parse(&text)
    }

    fn parse_bulletml(&mut self, bulletml: roxmltree::Node) -> Result<NodeId, Error> {
        let type_att = bulletml.attribute("type");
        let id = match type_att {
            Some(type_att) => match type_att {
                "none" => self
                    .arena
                    .new_node(BulletMLNode::BulletML { bml_type: None }),
                "vertical" => self.arena.new_node(BulletMLNode::BulletML {
                    bml_type: Some(BulletMLType::Vertical),
                }),
                "horizontal" => self.arena.new_node(BulletMLNode::BulletML {
                    bml_type: Some(BulletMLType::Horizontal),
                }),
                _ => Err(Error::BulletML(
                    format!("Unrecognized type {}", type_att),
                    Backtrace::new(),
                ))?,
            },
            None => self
                .arena
                .new_node(BulletMLNode::BulletML { bml_type: None }),
        };
        for child in bulletml.children().filter(|n| n.is_element()) {
            let child_name = child.tag_name();
            let child_id = match child_name.name() {
                "bullet" => self.parse_bullet(child)?,
                "action" => self.parse_action(child)?,
                "fire" => self.parse_fire(child)?,
                name => Err(Error::BulletML(
                    format!("Expected bullet or action or fire but got {}", name),
                    Backtrace::new(),
                ))?,
            };
            id.append(child_id, &mut self.arena)?;
        }
        Ok(id)
    }

    fn parse_bullet(&mut self, bullet: roxmltree::Node) -> Result<NodeId, Error> {
        let label = bullet.attribute("label");
        let id = if let Some(label) = label {
            let id = self
                .arena
                .new_node(BulletMLNode::Bullet(Some(label.to_string())));
            self.bullet_refs.insert(label.to_string(), id);
            id
        } else {
            self.arena.new_node(BulletMLNode::Bullet(None))
        };
        for child in bullet.children().filter(|n| n.is_element()) {
            let child_name = child.tag_name();
            let child_id = match child_name.name() {
                "direction" => self.parse_direction(child)?,
                "speed" => self.parse_speed(child)?,
                "action" => self.parse_action(child)?,
                "actionRef" => self.parse_action_ref(child)?,
                name => Err(Error::BulletML(
                    format!(
                        "Expected direction or speed or action or actionRef but got {}",
                        name
                    ),
                    Backtrace::new(),
                ))?,
            };
            id.append(child_id, &mut self.arena)?;
        }
        Ok(id)
    }

    fn parse_action(&mut self, action: roxmltree::Node) -> Result<NodeId, Error> {
        let label = action.attribute("label");
        let id = if let Some(label) = label {
            let id = self
                .arena
                .new_node(BulletMLNode::Action(Some(label.to_string())));
            self.action_refs.insert(label.to_string(), id);
            id
        } else {
            self.arena.new_node(BulletMLNode::Action(None))
        };
        for child in action.children().filter(|n| n.is_element()) {
            let child_name = child.tag_name();
            let child_id = match child_name.name() {
                "repeat" => self.parse_repeat(child)?,
                "fire" => self.parse_fire(child)?,
                "fireRef" => self.parse_fire_ref(child)?,
                "changeSpeed" => self.parse_change_speed(child)?,
                "changeDirection" => self.parse_change_direction(child)?,
                "accel" => self.parse_accel(child)?,
                "wait" => self.parse_wait(child)?,
                "vanish" => self.parse_vanish(child)?,
                "action" => self.parse_action(child)?,
                "actionRef" => self.parse_action_ref(child)?,
                name => Err(Error::BulletML(
                    format!(
                        "Expected repeat or fire or fireRef or changeSpeed or changeDirection or accel or wait or vanish or action or actionRef but got {}",
                        name
                    ),
                    Backtrace::new(),
                ))?,
            };
            id.append(child_id, &mut self.arena)?;
        }
        Ok(id)
    }

    fn parse_fire(&mut self, fire: roxmltree::Node) -> Result<NodeId, Error> {
        let label = fire.attribute("label");
        let id = if let Some(label) = label {
            let id = self
                .arena
                .new_node(BulletMLNode::Fire(Some(label.to_string())));
            self.fire_refs.insert(label.to_string(), id);
            id
        } else {
            self.arena.new_node(BulletMLNode::Fire(None))
        };
        for child in fire.children().filter(|n| n.is_element()) {
            let child_name = child.tag_name();
            let child_id = match child_name.name() {
                "direction" => self.parse_direction(child)?,
                "speed" => self.parse_speed(child)?,
                "bullet" => self.parse_bullet(child)?,
                "bulletRef" => self.parse_bullet_ref(child)?,
                name => Err(Error::BulletML(
                    format!(
                        "Expected direction or speed or bullet or bulletRef but got {}",
                        name
                    ),
                    Backtrace::new(),
                ))?,
            };
            id.append(child_id, &mut self.arena)?;
        }
        Ok(id)
    }

    fn parse_change_direction(
        &mut self,
        change_direction: roxmltree::Node,
    ) -> Result<NodeId, Error> {
        let id = self.arena.new_node(BulletMLNode::ChangeDirection);
        for child in change_direction.children().filter(|n| n.is_element()) {
            let child_name = child.tag_name();
            let child_id = match child_name.name() {
                "direction" => self.parse_direction(child)?,
                "term" => self.parse_term(child)?,
                name => Err(Error::BulletML(
                    format!("Expected direction or term but got {}", name),
                    Backtrace::new(),
                ))?,
            };
            id.append(child_id, &mut self.arena)?;
        }
        Ok(id)
    }

    fn parse_change_speed(&mut self, change_speed: roxmltree::Node) -> Result<NodeId, Error> {
        let id = self.arena.new_node(BulletMLNode::ChangeSpeed);
        for child in change_speed.children().filter(|n| n.is_element()) {
            let child_name = child.tag_name();
            let child_id = match child_name.name() {
                "speed" => self.parse_speed(child)?,
                "term" => self.parse_term(child)?,
                name => Err(Error::BulletML(
                    format!("Expected speed or term but got {}", name),
                    Backtrace::new(),
                ))?,
            };
            id.append(child_id, &mut self.arena)?;
        }
        Ok(id)
    }

    fn parse_accel(&mut self, accel: roxmltree::Node) -> Result<NodeId, Error> {
        let id = self.arena.new_node(BulletMLNode::Accel);
        for child in accel.children().filter(|n| n.is_element()) {
            let child_name = child.tag_name();
            let child_id = match child_name.name() {
                "horizontal" => self.parse_horizontal(child)?,
                "vertical" => self.parse_vertical(child)?,
                "term" => self.parse_term(child)?,
                name => Err(Error::BulletML(
                    format!("Expected horizontal or vertical or term but got {}", name),
                    Backtrace::new(),
                ))?,
            };
            id.append(child_id, &mut self.arena)?;
        }
        Ok(id)
    }

    fn parse_wait(&mut self, wait: roxmltree::Node) -> Result<NodeId, Error> {
        let expr = self.parse_expression(wait)?;
        let id = self.arena.new_node(BulletMLNode::Wait(expr));
        Ok(id)
    }

    fn parse_vanish(&mut self, _vanish: roxmltree::Node) -> Result<NodeId, Error> {
        let id = self.arena.new_node(BulletMLNode::Vanish);
        Ok(id)
    }

    fn parse_repeat(&mut self, repeat: roxmltree::Node) -> Result<NodeId, Error> {
        let id = self.arena.new_node(BulletMLNode::Repeat);
        for child in repeat.children().filter(|n| n.is_element()) {
            let child_name = child.tag_name();
            let child_id = match child_name.name() {
                "times" => self.parse_times(child)?,
                "action" => self.parse_action(child)?,
                "actionRef" => self.parse_action_ref(child)?,
                name => Err(Error::BulletML(
                    format!("Expected times or action or actionRef but got {}", name),
                    Backtrace::new(),
                ))?,
            };
            id.append(child_id, &mut self.arena)?;
        }
        Ok(id)
    }

    fn parse_direction(&mut self, direction: roxmltree::Node) -> Result<NodeId, Error> {
        let type_att = direction.attribute("type");
        let expr = self.parse_expression(direction)?;
        let id = match type_att {
            Some(type_att) => match type_att {
                "aim" => self.arena.new_node(BulletMLNode::Direction {
                    dir_type: Some(DirectionType::Aim),
                    dir: expr,
                }),
                "absolute" => self.arena.new_node(BulletMLNode::Direction {
                    dir_type: Some(DirectionType::Absolute),
                    dir: expr,
                }),
                "relative" => self.arena.new_node(BulletMLNode::Direction {
                    dir_type: Some(DirectionType::Relative),
                    dir: expr,
                }),
                "sequence" => self.arena.new_node(BulletMLNode::Direction {
                    dir_type: Some(DirectionType::Sequence),
                    dir: expr,
                }),
                _ => Err(Error::BulletML(
                    format!("Unrecognized type {}", type_att),
                    Backtrace::new(),
                ))?,
            },
            None => self.arena.new_node(BulletMLNode::Direction {
                dir_type: None,
                dir: expr,
            }),
        };
        Ok(id)
    }

    fn parse_speed(&mut self, speed: roxmltree::Node) -> Result<NodeId, Error> {
        let type_att = speed.attribute("type");
        let expr = self.parse_expression(speed)?;
        let id = match type_att {
            Some(type_att) => match type_att {
                "absolute" => self.arena.new_node(BulletMLNode::Speed {
                    spd_type: Some(SpeedType::Absolute),
                    spd: expr,
                }),
                "relative" => self.arena.new_node(BulletMLNode::Speed {
                    spd_type: Some(SpeedType::Relative),
                    spd: expr,
                }),
                "sequence" => self.arena.new_node(BulletMLNode::Speed {
                    spd_type: Some(SpeedType::Sequence),
                    spd: expr,
                }),
                _ => Err(Error::BulletML(
                    format!("Unrecognized type {}", type_att),
                    Backtrace::new(),
                ))?,
            },
            None => self.arena.new_node(BulletMLNode::Speed {
                spd_type: None,
                spd: expr,
            }),
        };
        Ok(id)
    }

    fn parse_horizontal(&mut self, horizontal: roxmltree::Node) -> Result<NodeId, Error> {
        let type_att = horizontal.attribute("type");
        let expr = self.parse_expression(horizontal)?;
        let id = match type_att {
            Some(type_att) => match type_att {
                "absolute" => self.arena.new_node(BulletMLNode::Horizontal {
                    h_type: HVType::Absolute,
                    h: expr,
                }),
                "relative" => self.arena.new_node(BulletMLNode::Horizontal {
                    h_type: HVType::Relative,
                    h: expr,
                }),
                "sequence" => self.arena.new_node(BulletMLNode::Horizontal {
                    h_type: HVType::Sequence,
                    h: expr,
                }),
                _ => Err(Error::BulletML(
                    format!("Unrecognized type {}", type_att),
                    Backtrace::new(),
                ))?,
            },
            None => self.arena.new_node(BulletMLNode::Horizontal {
                h_type: HVType::Absolute,
                h: expr,
            }),
        };
        Ok(id)
    }

    fn parse_vertical(&mut self, vertical: roxmltree::Node) -> Result<NodeId, Error> {
        let type_att = vertical.attribute("type");
        let expr = self.parse_expression(vertical)?;
        let id = match type_att {
            Some(type_att) => match type_att {
                "absolute" => self.arena.new_node(BulletMLNode::Vertical {
                    v_type: HVType::Absolute,
                    v: expr,
                }),
                "relative" => self.arena.new_node(BulletMLNode::Vertical {
                    v_type: HVType::Relative,
                    v: expr,
                }),
                "sequence" => self.arena.new_node(BulletMLNode::Vertical {
                    v_type: HVType::Sequence,
                    v: expr,
                }),
                _ => Err(Error::BulletML(
                    format!("Unrecognized type {}", type_att),
                    Backtrace::new(),
                ))?,
            },
            None => self.arena.new_node(BulletMLNode::Vertical {
                v_type: HVType::Absolute,
                v: expr,
            }),
        };
        Ok(id)
    }

    fn parse_term(&mut self, term: roxmltree::Node) -> Result<NodeId, Error> {
        let expr = self.parse_expression(term)?;
        let id = self.arena.new_node(BulletMLNode::Term(expr));
        Ok(id)
    }

    fn parse_times(&mut self, times: roxmltree::Node) -> Result<NodeId, Error> {
        let expr = self.parse_expression(times)?;
        let id = self.arena.new_node(BulletMLNode::Times(expr));
        Ok(id)
    }

    fn parse_bullet_ref(&mut self, bullet_ref: roxmltree::Node) -> Result<NodeId, Error> {
        let label = bullet_ref.attribute("label");
        let label = if let Some(label) = label {
            label
        } else {
            return Err(Error::BulletML(
                "missing label".to_string(),
                Backtrace::new(),
            ));
        };
        let id = self
            .arena
            .new_node(BulletMLNode::BulletRef(label.to_string()));
        for child in bullet_ref.children().filter(|n| n.is_element()) {
            let child_name = child.tag_name();
            let child_id = match child_name.name() {
                "param" => self.parse_param(child)?,
                name => Err(Error::BulletML(
                    format!("Expected param but got {}", name),
                    Backtrace::new(),
                ))?,
            };
            id.append(child_id, &mut self.arena)?;
        }
        Ok(id)
    }

    fn parse_action_ref(&mut self, action_ref: roxmltree::Node) -> Result<NodeId, Error> {
        let label = action_ref.attribute("label");
        let label = if let Some(label) = label {
            label
        } else {
            return Err(Error::BulletML(
                "missing label".to_string(),
                Backtrace::new(),
            ));
        };
        let id = self
            .arena
            .new_node(BulletMLNode::ActionRef(label.to_string()));
        for child in action_ref.children().filter(|n| n.is_element()) {
            let child_name = child.tag_name();
            let child_id = match child_name.name() {
                "param" => self.parse_param(child)?,
                name => Err(Error::BulletML(
                    format!("Expected param but got {}", name),
                    Backtrace::new(),
                ))?,
            };
            id.append(child_id, &mut self.arena)?;
        }
        Ok(id)
    }

    fn parse_fire_ref(&mut self, fire_ref: roxmltree::Node) -> Result<NodeId, Error> {
        let label = fire_ref.attribute("label");
        let label = if let Some(label) = label {
            label
        } else {
            return Err(Error::BulletML(
                "missing label".to_string(),
                Backtrace::new(),
            ));
        };
        let id = self
            .arena
            .new_node(BulletMLNode::FireRef(label.to_string()));
        for child in fire_ref.children().filter(|n| n.is_element()) {
            let child_name = child.tag_name();
            let child_id = match child_name.name() {
                "param" => self.parse_param(child)?,
                name => Err(Error::BulletML(
                    format!("Expected param but got {}", name),
                    Backtrace::new(),
                ))?,
            };
            id.append(child_id, &mut self.arena)?;
        }
        Ok(id)
    }

    fn parse_param(&mut self, param: roxmltree::Node) -> Result<NodeId, Error> {
        let expr = self.parse_expression(param)?;
        let id = self.arena.new_node(BulletMLNode::Param(expr));
        Ok(id)
    }

    fn parse_expression(&self, parent: roxmltree::Node) -> Result<Expr, Error> {
        let mut str: String = String::new();
        for child in parent.children() {
            let node_type = child.node_type();
            match node_type {
                roxmltree::NodeType::Text => {
                    str.push_str(child.text().unwrap());
                }
                roxmltree::NodeType::Root | roxmltree::NodeType::Element => Err(Error::BulletML(
                    format!("Expected Text but got {:?}", node_type),
                    Backtrace::new(),
                ))?,
                roxmltree::NodeType::Comment | roxmltree::NodeType::PI => {}
            }
        }
        str = str.replace("$rand", "rand(0)");
        str = str.replace("$rank", "rank");
        str = str.replace("$", "v");
        Ok(str.parse()?)
    }
}
