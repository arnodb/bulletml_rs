use crate::errors::{ParseError, ParseErrorKind, ParseErrorPos};
use failure::{Fail, ResultExt};
use indextree::{Arena, NodeId};
use meval::Expr;
use roxmltree::TextPos;
use std::collections::HashMap;
use std::fs;
use std::io::prelude::*;
use std::path;

use crate::tree::{BulletML, BulletMLNode, BulletMLType, DirectionType, HVType, SpeedType};

pub struct BulletMLParser {
    arena: Arena<BulletMLNode>,
    bullet_refs: HashMap<String, NodeId>,
    action_refs: HashMap<String, NodeId>,
    fire_refs: HashMap<String, NodeId>,
}

impl BulletMLParser {
    pub fn parse(s: &str) -> Result<BulletML, ParseError> {
        let doc = roxmltree::Document::parse(s).context(ParseErrorKind::Xml)?;
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
            name => Err(ParseError::from(ParseErrorKind::UnexpectedElement {
                element: name.to_string(),
                pos: BulletMLParser::node_pos(&root),
            })),
        }
    }

    pub fn parse_file(path: &path::Path) -> Result<BulletML, ParseError> {
        let mut file = fs::File::open(&path).context(ParseErrorKind::FileOpen)?;
        let mut text = String::new();
        file.read_to_string(&mut text)
            .context(ParseErrorKind::FileRead)?;
        BulletMLParser::parse(&text)
    }

    fn parse_bulletml(&mut self, bulletml: roxmltree::Node) -> Result<NodeId, ParseError> {
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
                _ => Err(ParseErrorKind::UnrecognizedBmlType {
                    bml_type: type_att.to_string(),
                    pos: BulletMLParser::attribute_value_pos(&bulletml, "type"),
                })?,
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
                name => Err(ParseError::from(ParseErrorKind::UnexpectedElement {
                    element: name.to_string(),
                    pos: BulletMLParser::node_pos(&child),
                }))?,
            };
            id.append(child_id, &mut self.arena)
                .context(ParseErrorKind::Internal)?;
        }
        Ok(id)
    }

    fn parse_bullet(&mut self, bullet: roxmltree::Node) -> Result<NodeId, ParseError> {
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
                name => Err(ParseError::from(ParseErrorKind::UnexpectedElement {
                    element: name.to_string(),
                    pos: BulletMLParser::node_pos(&child),
                }))?,
            };
            id.append(child_id, &mut self.arena)
                .context(ParseErrorKind::Internal)?;
        }
        Ok(id)
    }

    fn parse_action(&mut self, action: roxmltree::Node) -> Result<NodeId, ParseError> {
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
                name => Err(ParseError::from(ParseErrorKind::UnexpectedElement {
                    element: name.to_string(),
                    pos: BulletMLParser::node_pos(&child),
                }))?,
            };
            id.append(child_id, &mut self.arena)
                .context(ParseErrorKind::Internal)?;
        }
        Ok(id)
    }

    fn parse_fire(&mut self, fire: roxmltree::Node) -> Result<NodeId, ParseError> {
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
                name => Err(ParseError::from(ParseErrorKind::UnexpectedElement {
                    element: name.to_string(),
                    pos: BulletMLParser::node_pos(&child),
                }))?,
            };
            id.append(child_id, &mut self.arena)
                .context(ParseErrorKind::Internal)?;
        }
        Ok(id)
    }

    fn parse_change_direction(
        &mut self,
        change_direction: roxmltree::Node,
    ) -> Result<NodeId, ParseError> {
        let id = self.arena.new_node(BulletMLNode::ChangeDirection);
        for child in change_direction.children().filter(|n| n.is_element()) {
            let child_name = child.tag_name();
            let child_id = match child_name.name() {
                "direction" => self.parse_direction(child)?,
                "term" => self.parse_term(child)?,
                name => Err(ParseError::from(ParseErrorKind::UnexpectedElement {
                    element: name.to_string(),
                    pos: BulletMLParser::node_pos(&child),
                }))?,
            };
            id.append(child_id, &mut self.arena)
                .context(ParseErrorKind::Internal)?;
        }
        Ok(id)
    }

    fn parse_change_speed(&mut self, change_speed: roxmltree::Node) -> Result<NodeId, ParseError> {
        let id = self.arena.new_node(BulletMLNode::ChangeSpeed);
        for child in change_speed.children().filter(|n| n.is_element()) {
            let child_name = child.tag_name();
            let child_id = match child_name.name() {
                "speed" => self.parse_speed(child)?,
                "term" => self.parse_term(child)?,
                name => Err(ParseError::from(ParseErrorKind::UnexpectedElement {
                    element: name.to_string(),
                    pos: BulletMLParser::node_pos(&child),
                }))?,
            };
            id.append(child_id, &mut self.arena)
                .context(ParseErrorKind::Internal)?;
        }
        Ok(id)
    }

    fn parse_accel(&mut self, accel: roxmltree::Node) -> Result<NodeId, ParseError> {
        let id = self.arena.new_node(BulletMLNode::Accel);
        for child in accel.children().filter(|n| n.is_element()) {
            let child_name = child.tag_name();
            let child_id = match child_name.name() {
                "horizontal" => self.parse_horizontal(child)?,
                "vertical" => self.parse_vertical(child)?,
                "term" => self.parse_term(child)?,
                name => Err(ParseError::from(ParseErrorKind::UnexpectedElement {
                    element: name.to_string(),
                    pos: BulletMLParser::node_pos(&child),
                }))?,
            };
            id.append(child_id, &mut self.arena)
                .context(ParseErrorKind::Internal)?;
        }
        Ok(id)
    }

    fn parse_wait(&mut self, wait: roxmltree::Node) -> Result<NodeId, ParseError> {
        let expr = self.parse_expression(wait)?;
        let id = self.arena.new_node(BulletMLNode::Wait(expr));
        Ok(id)
    }

    fn parse_vanish(&mut self, _vanish: roxmltree::Node) -> Result<NodeId, ParseError> {
        let id = self.arena.new_node(BulletMLNode::Vanish);
        Ok(id)
    }

    fn parse_repeat(&mut self, repeat: roxmltree::Node) -> Result<NodeId, ParseError> {
        let id = self.arena.new_node(BulletMLNode::Repeat);
        for child in repeat.children().filter(|n| n.is_element()) {
            let child_name = child.tag_name();
            let child_id = match child_name.name() {
                "times" => self.parse_times(child)?,
                "action" => self.parse_action(child)?,
                "actionRef" => self.parse_action_ref(child)?,
                name => Err(ParseError::from(ParseErrorKind::UnexpectedElement {
                    element: name.to_string(),
                    pos: BulletMLParser::node_pos(&child),
                }))?,
            };
            id.append(child_id, &mut self.arena)
                .context(ParseErrorKind::Internal)?;
        }
        Ok(id)
    }

    fn parse_direction(&mut self, direction: roxmltree::Node) -> Result<NodeId, ParseError> {
        let type_att = direction.attribute("type");
        let dir_type = match type_att {
            Some("aim") => Some(DirectionType::Aim),
            Some("absolute") => Some(DirectionType::Absolute),
            Some("relative") => Some(DirectionType::Relative),
            Some("sequence") => Some(DirectionType::Sequence),
            None => None,
            Some(type_att) => Err(ParseErrorKind::UnrecognizedDirectionType {
                dir_type: type_att.to_string(),
                pos: BulletMLParser::attribute_value_pos(&direction, "type"),
            })?,
        };
        let expr = self.parse_expression(direction)?;
        let id = self.arena.new_node(BulletMLNode::Direction {
            dir_type,
            dir: expr,
        });
        Ok(id)
    }

    fn parse_speed(&mut self, speed: roxmltree::Node) -> Result<NodeId, ParseError> {
        let type_att = speed.attribute("type");
        let spd_type = match type_att {
            Some("absolute") => Some(SpeedType::Absolute),
            Some("relative") => Some(SpeedType::Relative),
            Some("sequence") => Some(SpeedType::Sequence),
            None => None,
            Some(type_att) => Err(ParseErrorKind::UnrecognizedSpeedType {
                speed_type: type_att.to_string(),
                pos: BulletMLParser::attribute_value_pos(&speed, "type"),
            })?,
        };
        let expr = self.parse_expression(speed)?;
        let id = self.arena.new_node(BulletMLNode::Speed {
            spd_type,
            spd: expr,
        });
        Ok(id)
    }

    fn parse_horizontal(&mut self, horizontal: roxmltree::Node) -> Result<NodeId, ParseError> {
        let type_att = horizontal.attribute("type");
        let h_type = match type_att {
            Some("absolute") | None => HVType::Absolute,
            Some("relative") => HVType::Relative,
            Some("sequence") => HVType::Sequence,
            Some(type_att) => Err(ParseErrorKind::UnrecognizedAccelDirType {
                accel_dir_type: type_att.to_string(),
                pos: BulletMLParser::attribute_value_pos(&horizontal, "type"),
            })?,
        };
        let expr = self.parse_expression(horizontal)?;
        let id = self
            .arena
            .new_node(BulletMLNode::Horizontal { h_type, h: expr });
        Ok(id)
    }

    fn parse_vertical(&mut self, vertical: roxmltree::Node) -> Result<NodeId, ParseError> {
        let type_att = vertical.attribute("type");
        let v_type = match type_att {
            Some("absolute") | None => HVType::Absolute,
            Some("relative") => HVType::Relative,
            Some("sequence") => HVType::Sequence,
            Some(type_att) => Err(ParseErrorKind::UnrecognizedAccelDirType {
                accel_dir_type: type_att.to_string(),
                pos: BulletMLParser::attribute_value_pos(&vertical, "type"),
            })?,
        };
        let expr = self.parse_expression(vertical)?;
        let id = self
            .arena
            .new_node(BulletMLNode::Vertical { v_type, v: expr });
        Ok(id)
    }

    fn parse_term(&mut self, term: roxmltree::Node) -> Result<NodeId, ParseError> {
        let expr = self.parse_expression(term)?;
        let id = self.arena.new_node(BulletMLNode::Term(expr));
        Ok(id)
    }

    fn parse_times(&mut self, times: roxmltree::Node) -> Result<NodeId, ParseError> {
        let expr = self.parse_expression(times)?;
        let id = self.arena.new_node(BulletMLNode::Times(expr));
        Ok(id)
    }

    fn parse_bullet_ref(&mut self, bullet_ref: roxmltree::Node) -> Result<NodeId, ParseError> {
        let label = bullet_ref.attribute("label");
        let label = if let Some(label) = label {
            label
        } else {
            Err(ParseError::from(ParseErrorKind::MissingAttribute {
                attribute: "label".to_string(),
                element: bullet_ref.tag_name().name().to_string(),
                pos: BulletMLParser::node_pos(&bullet_ref),
            }))?
        };
        let id = self
            .arena
            .new_node(BulletMLNode::BulletRef(label.to_string()));
        for child in bullet_ref.children().filter(|n| n.is_element()) {
            let child_name = child.tag_name();
            let child_id = match child_name.name() {
                "param" => self.parse_param(child)?,
                name => Err(ParseError::from(ParseErrorKind::UnexpectedElement {
                    element: name.to_string(),
                    pos: BulletMLParser::node_pos(&child),
                }))?,
            };
            id.append(child_id, &mut self.arena)
                .context(ParseErrorKind::Internal)?;
        }
        Ok(id)
    }

    fn parse_action_ref(&mut self, action_ref: roxmltree::Node) -> Result<NodeId, ParseError> {
        let label = action_ref.attribute("label");
        let label = if let Some(label) = label {
            label
        } else {
            Err(ParseError::from(ParseErrorKind::MissingAttribute {
                attribute: "label".to_string(),
                element: action_ref.tag_name().name().to_string(),
                pos: BulletMLParser::node_pos(&action_ref),
            }))?
        };
        let id = self
            .arena
            .new_node(BulletMLNode::ActionRef(label.to_string()));
        for child in action_ref.children().filter(|n| n.is_element()) {
            let child_name = child.tag_name();
            let child_id = match child_name.name() {
                "param" => self.parse_param(child)?,
                name => Err(ParseError::from(ParseErrorKind::UnexpectedElement {
                    element: name.to_string(),
                    pos: BulletMLParser::node_pos(&child),
                }))?,
            };
            id.append(child_id, &mut self.arena)
                .context(ParseErrorKind::Internal)?;
        }
        Ok(id)
    }

    fn parse_fire_ref(&mut self, fire_ref: roxmltree::Node) -> Result<NodeId, ParseError> {
        let label = fire_ref.attribute("label");
        let label = if let Some(label) = label {
            label
        } else {
            Err(ParseError::from(ParseErrorKind::MissingAttribute {
                attribute: "label".to_string(),
                element: fire_ref.tag_name().name().to_string(),
                pos: BulletMLParser::node_pos(&fire_ref),
            }))?
        };
        let id = self
            .arena
            .new_node(BulletMLNode::FireRef(label.to_string()));
        for child in fire_ref.children().filter(|n| n.is_element()) {
            let child_name = child.tag_name();
            let child_id = match child_name.name() {
                "param" => self.parse_param(child)?,
                name => Err(ParseError::from(ParseErrorKind::UnexpectedElement {
                    element: name.to_string(),
                    pos: BulletMLParser::node_pos(&child),
                }))?,
            };
            id.append(child_id, &mut self.arena)
                .context(ParseErrorKind::Internal)?;
        }
        Ok(id)
    }

    fn parse_param(&mut self, param: roxmltree::Node) -> Result<NodeId, ParseError> {
        let expr = self.parse_expression(param)?;
        let id = self.arena.new_node(BulletMLNode::Param(expr));
        Ok(id)
    }

    fn parse_expression(&self, parent: roxmltree::Node) -> Result<Expr, ParseError> {
        let mut str: String = String::new();
        for child in parent.children() {
            let node_type = child.node_type();
            match node_type {
                roxmltree::NodeType::Text => {
                    str.push_str(child.text().unwrap());
                }
                roxmltree::NodeType::Root | roxmltree::NodeType::Element => {
                    Err(ParseError::from(ParseErrorKind::UnexpectedNodeType {
                        node_type: format!("{:?}", node_type),
                        pos: BulletMLParser::node_pos(&child),
                    }))?
                }
                roxmltree::NodeType::Comment | roxmltree::NodeType::PI => {}
            }
        }
        str = str.replace("$rand", "rand(0)");
        str = str.replace("$rank", "rank");
        str = str.replace("$", "v");
        let expr = str.parse::<Expr>().map_err(|err| {
            ParseError::from(err.context(ParseErrorKind::Expression {
                pos: BulletMLParser::node_pos(parent.first_child().as_ref().unwrap_or(&parent)),
            }))
        })?;
        Ok(expr)
    }

    #[inline]
    fn node_pos(node: &roxmltree::Node) -> ParseErrorPos {
        node.node_pos().into()
    }

    #[inline]
    fn attribute_value_pos(node: &roxmltree::Node, name: &str) -> ParseErrorPos {
        node.attribute_value_pos(name)
            .unwrap_or_else(|| TextPos { row: 0, col: 0 })
            .into()
    }
}

#[test]
fn test_bulletml() {
    let bml = BulletMLParser::parse(
        r##"<?xml version="1.0" ?>
<bulletml />"##,
    )
    .unwrap();
    assert_matches!(
        &bml.arena[bml.root].data,
        &BulletMLNode::BulletML { bml_type: None }
    );
}

#[test]
fn test_bulletml_type_none() {
    let bml = BulletMLParser::parse(
        r##"<?xml version="1.0" ?>
<bulletml type="none" />"##,
    )
    .unwrap();
    assert_matches!(
        &bml.arena[bml.root].data,
        &BulletMLNode::BulletML { bml_type: None }
    );
}

#[test]
fn test_bulletml_type_vertical() {
    let bml = BulletMLParser::parse(
        r##"<?xml version="1.0" ?>
<bulletml type="vertical" />"##,
    )
    .unwrap();
    assert_matches!(
        &bml.arena[bml.root].data,
        &BulletMLNode::BulletML {
            bml_type: Some(BulletMLType::Vertical)
        }
    );
}

#[test]
fn test_bulletml_type_horizontal() {
    let bml = BulletMLParser::parse(
        r##"<?xml version="1.0" ?>
<bulletml type="horizontal" />"##,
    )
    .unwrap();
    assert_matches!(
        &bml.arena[bml.root].data,
        &BulletMLNode::BulletML {
            bml_type: Some(BulletMLType::Horizontal)
        }
    );
}

#[test]
fn test_full_bulletml() {
    // This covers all the good branches of the parser.
    BulletMLParser::parse(
        r##"<?xml version="1.0" ?>
<bulletml>
    <bullet label="b1">
        <direction>0</direction>
        <direction type="aim">0</direction>
        <direction type="absolute">0</direction>
        <direction type="relative">0</direction>
        <direction type="sequence">0</direction>
        <speed>0</speed>
        <speed type="absolute">0</speed>
        <speed type="relative">0</speed>
        <speed type="sequence">0</speed>
        <action label="a1">
            <repeat>
                <times>0</times>
                <action />
                <actionRef label="a1">
                    <param>0</param>
                </actionRef>
            </repeat>
            <fire label="f1">
                <direction>0</direction>
                <speed>0</speed>
                <bullet />
                <bulletRef label="b1">
                    <param>0</param>
                </bulletRef>
            </fire>
            <fireRef label="f1">
                <param>0</param>
            </fireRef>
            <changeSpeed>
                <speed>0</speed>
                <term>0</term>
            </changeSpeed>
            <changeDirection>
                <direction>0</direction>
                <term>0</term>
            </changeDirection>
            <accel>
                <horizontal>0</horizontal>
                <horizontal type="absolute">0</horizontal>
                <horizontal type="relative">0</horizontal>
                <horizontal type="sequence">0</horizontal>
                <vertical>0</vertical>
                <vertical type="absolute">0</vertical>
                <vertical type="relative">0</vertical>
                <vertical type="sequence">0</vertical>
                <term>0</term>
            </accel>
            <wait>0</wait>
            <vanish />
            <action />
            <actionRef label="a1" />
        </action>
        <actionRef label="a1" />
    </bullet>
</bulletml>"##,
    )
    .unwrap();
}

#[test]
fn test_unexpected_root() {
    let bml = BulletMLParser::parse(
        r##"<?xml version="1.0" ?>
<foo />"##,
    );
    assert_eq!(
        bml.unwrap_err().kind(),
        &ParseErrorKind::UnexpectedElement {
            element: "foo".to_string(),
            pos: ParseErrorPos { row: 2, col: 1 }
        }
    );
}

#[test]
fn test_unrecognized_bml_type() {
    let bml = BulletMLParser::parse(
        r##"<?xml version="1.0" ?>
<bulletml type="foo" />"##,
    );
    assert_eq!(
        bml.unwrap_err().kind(),
        &ParseErrorKind::UnrecognizedBmlType {
            bml_type: "foo".to_string(),
            pos: ParseErrorPos { row: 2, col: 17 }
        }
    );
}

#[test]
fn test_unexpected_bulletml_child() {
    let bml = BulletMLParser::parse(
        r##"<?xml version="1.0" ?>
<bulletml>
    <foo />
</bulletml>"##,
    );
    assert_eq!(
        bml.unwrap_err().kind(),
        &ParseErrorKind::UnexpectedElement {
            element: "foo".to_string(),
            pos: ParseErrorPos { row: 3, col: 5 }
        }
    );
}

#[test]
fn test_unexpected_bullet_child() {
    let bml = BulletMLParser::parse(
        r##"<?xml version="1.0" ?>
<bulletml>
    <bullet>
        <foo />
    </bullet>
</bulletml>"##,
    );
    assert_eq!(
        bml.unwrap_err().kind(),
        &ParseErrorKind::UnexpectedElement {
            element: "foo".to_string(),
            pos: ParseErrorPos { row: 4, col: 9 }
        }
    );
}

#[test]
fn test_unexpected_action_child() {
    let bml = BulletMLParser::parse(
        r##"<?xml version="1.0" ?>
<bulletml>
    <action>
        <foo />
    </action>
</bulletml>"##,
    );
    assert_eq!(
        bml.unwrap_err().kind(),
        &ParseErrorKind::UnexpectedElement {
            element: "foo".to_string(),
            pos: ParseErrorPos { row: 4, col: 9 }
        }
    );
}

#[test]
fn test_unexpected_fire_child() {
    let bml = BulletMLParser::parse(
        r##"<?xml version="1.0" ?>
<bulletml>
    <fire>
        <foo />
    </fire>
</bulletml>"##,
    );
    assert_eq!(
        bml.unwrap_err().kind(),
        &ParseErrorKind::UnexpectedElement {
            element: "foo".to_string(),
            pos: ParseErrorPos { row: 4, col: 9 }
        }
    );
}

#[test]
fn test_unexpected_change_direction_child() {
    let bml = BulletMLParser::parse(
        r##"<?xml version="1.0" ?>
<bulletml>
    <action>
        <changeDirection>
            <foo />
        </changeDirection>
    </action>
</bulletml>"##,
    );
    assert_eq!(
        bml.unwrap_err().kind(),
        &ParseErrorKind::UnexpectedElement {
            element: "foo".to_string(),
            pos: ParseErrorPos { row: 5, col: 13 }
        }
    );
}

#[test]
fn test_unexpected_change_speed_child() {
    let bml = BulletMLParser::parse(
        r##"<?xml version="1.0" ?>
<bulletml>
    <action>
        <changeSpeed>
            <foo />
        </changeSpeed>
    </action>
</bulletml>"##,
    );
    assert_eq!(
        bml.unwrap_err().kind(),
        &ParseErrorKind::UnexpectedElement {
            element: "foo".to_string(),
            pos: ParseErrorPos { row: 5, col: 13 }
        }
    );
}

#[test]
fn test_unexpected_accel_child() {
    let bml = BulletMLParser::parse(
        r##"<?xml version="1.0" ?>
<bulletml>
    <action>
        <accel>
            <foo />
        </accel>
    </action>
</bulletml>"##,
    );
    assert_eq!(
        bml.unwrap_err().kind(),
        &ParseErrorKind::UnexpectedElement {
            element: "foo".to_string(),
            pos: ParseErrorPos { row: 5, col: 13 }
        }
    );
}

#[test]
fn test_unexpected_repeat_child() {
    let bml = BulletMLParser::parse(
        r##"<?xml version="1.0" ?>
<bulletml>
    <action>
        <repeat>
            <foo />
        </repeat>
    </action>
</bulletml>"##,
    );
    assert_eq!(
        bml.unwrap_err().kind(),
        &ParseErrorKind::UnexpectedElement {
            element: "foo".to_string(),
            pos: ParseErrorPos { row: 5, col: 13 }
        }
    );
}

#[test]
fn test_unrecognized_direction_type() {
    let bml = BulletMLParser::parse(
        r##"<?xml version="1.0" ?>
<bulletml>
    <bullet>
        <direction type="foo" />
    </bullet>
</bulletml>"##,
    );
    assert_eq!(
        bml.unwrap_err().kind(),
        &ParseErrorKind::UnrecognizedDirectionType {
            dir_type: "foo".to_string(),
            pos: ParseErrorPos { row: 4, col: 26 }
        }
    );
}

#[test]
fn test_unrecognized_speed_type() {
    let bml = BulletMLParser::parse(
        r##"<?xml version="1.0" ?>
<bulletml>
    <bullet>
        <speed type="foo" />
    </bullet>
</bulletml>"##,
    );
    assert_eq!(
        bml.unwrap_err().kind(),
        &ParseErrorKind::UnrecognizedSpeedType {
            speed_type: "foo".to_string(),
            pos: ParseErrorPos { row: 4, col: 22 }
        }
    );
}

#[test]
fn test_unrecognized_accel_horizontal_type() {
    let bml = BulletMLParser::parse(
        r##"<?xml version="1.0" ?>
<bulletml>
    <action>
        <accel>
            <horizontal type="foo" />
        </accel>
    </action>
</bulletml>"##,
    );
    assert_eq!(
        bml.unwrap_err().kind(),
        &ParseErrorKind::UnrecognizedAccelDirType {
            accel_dir_type: "foo".to_string(),
            pos: ParseErrorPos { row: 5, col: 31 }
        }
    );
}

#[test]
fn test_unrecognized_accel_vertical_type() {
    let bml = BulletMLParser::parse(
        r##"<?xml version="1.0" ?>
<bulletml>
    <action>
        <accel>
            <vertical type="foo" />
        </accel>
    </action>
</bulletml>"##,
    );
    assert_eq!(
        bml.unwrap_err().kind(),
        &ParseErrorKind::UnrecognizedAccelDirType {
            accel_dir_type: "foo".to_string(),
            pos: ParseErrorPos { row: 5, col: 29 }
        }
    );
}

#[test]
fn test_missing_bullet_ref_label() {
    let bml = BulletMLParser::parse(
        r##"<?xml version="1.0" ?>
<bulletml>
    <fire>
        <bulletRef />
    </fire>
</bulletml>"##,
    );
    assert_eq!(
        bml.unwrap_err().kind(),
        &ParseErrorKind::MissingAttribute {
            attribute: "label".to_string(),
            element: "bulletRef".to_string(),
            pos: ParseErrorPos { row: 4, col: 9 }
        }
    );
}

#[test]
fn test_unexpected_bullet_ref_child() {
    let bml = BulletMLParser::parse(
        r##"<?xml version="1.0" ?>
<bulletml>
    <fire>
        <bulletRef label="bar">
            <foo />
        </bulletRef>
    </fire>
</bulletml>"##,
    );
    assert_eq!(
        bml.unwrap_err().kind(),
        &ParseErrorKind::UnexpectedElement {
            element: "foo".to_string(),
            pos: ParseErrorPos { row: 5, col: 13 }
        }
    );
}

#[test]
fn test_missing_action_ref_label() {
    let bml = BulletMLParser::parse(
        r##"<?xml version="1.0" ?>
<bulletml>
    <bullet>
        <actionRef />
    </bullet>
</bulletml>"##,
    );
    assert_eq!(
        bml.unwrap_err().kind(),
        &ParseErrorKind::MissingAttribute {
            attribute: "label".to_string(),
            element: "actionRef".to_string(),
            pos: ParseErrorPos { row: 4, col: 9 }
        }
    );
}

#[test]
fn test_unexpected_action_ref_child() {
    let bml = BulletMLParser::parse(
        r##"<?xml version="1.0" ?>
<bulletml>
    <bullet>
        <actionRef label="bar">
            <foo />
        </actionRef>
    </bullet>
</bulletml>"##,
    );
    assert_eq!(
        bml.unwrap_err().kind(),
        &ParseErrorKind::UnexpectedElement {
            element: "foo".to_string(),
            pos: ParseErrorPos { row: 5, col: 13 }
        }
    );
}

#[test]
fn test_missing_fire_ref_label() {
    let bml = BulletMLParser::parse(
        r##"<?xml version="1.0" ?>
<bulletml>
    <action>
        <fireRef />
    </action>
</bulletml>"##,
    );
    assert_eq!(
        bml.unwrap_err().kind(),
        &ParseErrorKind::MissingAttribute {
            attribute: "label".to_string(),
            element: "fireRef".to_string(),
            pos: ParseErrorPos { row: 4, col: 9 }
        }
    );
}

#[test]
fn test_unexpected_fire_ref_child() {
    let bml = BulletMLParser::parse(
        r##"<?xml version="1.0" ?>
<bulletml>
    <action>
        <fireRef label="bar">
            <foo />
        </fireRef>
    </action>
</bulletml>"##,
    );
    assert_eq!(
        bml.unwrap_err().kind(),
        &ParseErrorKind::UnexpectedElement {
            element: "foo".to_string(),
            pos: ParseErrorPos { row: 5, col: 13 }
        }
    );
}

#[test]
fn test_unexpected_node_type_in_expression() {
    let bml = BulletMLParser::parse(
        r##"<?xml version="1.0" ?>
<bulletml>
    <bullet>
        <direction><foo /></direction>
    </bullet>
</bulletml>"##,
    );
    assert_eq!(
        bml.unwrap_err().kind(),
        &ParseErrorKind::UnexpectedNodeType {
            node_type: "Element".to_string(),
            pos: ParseErrorPos { row: 4, col: 20 }
        }
    );
}

#[test]
fn test_expression_error() {
    let bml = BulletMLParser::parse(
        r##"<?xml version="1.0" ?>
<bulletml>
    <bullet>
        <direction>-</direction>
    </bullet>
</bulletml>"##,
    );
    let err = bml.unwrap_err();
    assert_eq!(
        err.kind(),
        &ParseErrorKind::Expression {
            pos: ParseErrorPos { row: 4, col: 20 }
        }
    );
    let cause = err.cause().unwrap().downcast_ref::<meval::Error>();
    assert_eq!(
        cause,
        Some(&meval::Error::ParseError(
            meval::ParseError::MissingArgument
        ))
    );
}
