use crate::errors::{ParseError, ParseErrorPos};
use crate::tree::{
    BulletML, BulletMLExpression, BulletMLNode, BulletMLType, DirectionType, HVType, SpeedType,
};
use indextree::{Arena, NodeId};
use roxmltree::TextPos;
use std::collections::HashMap;
use std::fs;
use std::io::prelude::*;
use std::path;

/// BulletML parser.
pub struct BulletMLParser {
    arena: Arena<BulletMLNode>,
    bullet_refs: HashMap<String, NodeId>,
    action_refs: HashMap<String, NodeId>,
    fire_refs: HashMap<String, NodeId>,
    expr_parser: fasteval::Parser,
    expr_slab: fasteval::Slab,
}

impl BulletMLParser {
    /// Creates a new parser with default capacities.
    ///
    /// Pay attention to the fact that the capacity of the expression parser cannot grow due to
    /// `fasteval::Slab` implementation. If you need a higher capacity, refer to the
    /// [with_capacities](#method.with_capacities) constructor.
    pub fn new() -> Self {
        BulletMLParser {
            arena: Arena::new(),
            bullet_refs: HashMap::new(),
            action_refs: HashMap::new(),
            fire_refs: HashMap::new(),
            expr_parser: fasteval::Parser::new(),
            expr_slab: fasteval::Slab::new(),
        }
    }

    /// Creates a new parser with custom capacities.
    ///
    /// `refs_capacity` is the initial capacity of references containers which can grow on demand.
    ///
    /// `expr_capacity` is the capacity of the expression parser which cannot grow. In order to
    /// mitigate that limitation, the internal of this crate handle float literals without the help
    /// of the expression parser.
    pub fn with_capacities(refs_capacity: usize, expr_capacity: usize) -> Self {
        BulletMLParser {
            arena: Arena::new(),
            bullet_refs: HashMap::with_capacity(refs_capacity),
            action_refs: HashMap::with_capacity(refs_capacity),
            fire_refs: HashMap::with_capacity(refs_capacity),
            expr_parser: fasteval::Parser::new(),
            expr_slab: fasteval::Slab::with_capacity(expr_capacity),
        }
    }

    /// Parses an input XML document and transforms it into a [BulletML](../struct.BulletML.html)
    /// structure to be used by a [Runner](../struct.Runner.html).
    pub fn parse(mut self, s: &str) -> Result<BulletML, ParseError> {
        let doc = roxmltree::Document::parse(s)?;
        let root = doc.root_element();
        let root_name = root.tag_name();
        match root_name.name() {
            "bulletml" => {
                let root_id = self.parse_bulletml(root)?;
                Ok(BulletML {
                    arena: self.arena,
                    root: root_id,
                    bullet_refs: self.bullet_refs,
                    action_refs: self.action_refs,
                    fire_refs: self.fire_refs,
                    expr_slab: self.expr_slab,
                })
            }
            name => Err(ParseError::new_unexpected_element(
                name.to_string(),
                BulletMLParser::node_pos(&root),
            )),
        }
    }

    /// Parses an input XML file and transforms it into a [BulletML](../struct.BulletML.html)
    /// structure to be used by a [Runner](../struct.Runner.html).
    pub fn parse_file<P: AsRef<path::Path>>(self, path: P) -> Result<BulletML, ParseError> {
        let mut file = fs::File::open(&path)?;
        let mut text = String::new();
        file.read_to_string(&mut text)?;
        self.parse(&text)
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
                _ => {
                    return Err(ParseError::new_unrecognized_bml_type(
                        type_att.to_string(),
                        BulletMLParser::attribute_value_pos(&bulletml, "type"),
                    ));
                }
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
                name => {
                    return Err(ParseError::new_unexpected_element(
                        name.to_string(),
                        BulletMLParser::node_pos(&child),
                    ));
                }
            };
            id.append(child_id, &mut self.arena);
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
                name => {
                    return Err(ParseError::new_unexpected_element(
                        name.to_string(),
                        BulletMLParser::node_pos(&child),
                    ));
                }
            };
            id.append(child_id, &mut self.arena);
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
                name => {
                    return Err(ParseError::new_unexpected_element(
                        name.to_string(),
                        BulletMLParser::node_pos(&child),
                    ));
                }
            };
            id.append(child_id, &mut self.arena);
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
                name => {
                    return Err(ParseError::new_unexpected_element(
                        name.to_string(),
                        BulletMLParser::node_pos(&child),
                    ));
                }
            };
            id.append(child_id, &mut self.arena);
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
                name => {
                    return Err(ParseError::new_unexpected_element(
                        name.to_string(),
                        BulletMLParser::node_pos(&child),
                    ));
                }
            };
            id.append(child_id, &mut self.arena);
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
                name => {
                    return Err(ParseError::new_unexpected_element(
                        name.to_string(),
                        BulletMLParser::node_pos(&child),
                    ));
                }
            };
            id.append(child_id, &mut self.arena);
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
                name => {
                    return Err(ParseError::new_unexpected_element(
                        name.to_string(),
                        BulletMLParser::node_pos(&child),
                    ));
                }
            };
            id.append(child_id, &mut self.arena);
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
                name => {
                    return Err(ParseError::new_unexpected_element(
                        name.to_string(),
                        BulletMLParser::node_pos(&child),
                    ));
                }
            };
            id.append(child_id, &mut self.arena);
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
            Some(type_att) => {
                return Err(ParseError::new_unrecognized_direction_type(
                    type_att.to_string(),
                    BulletMLParser::attribute_value_pos(&direction, "type"),
                ));
            }
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
            Some(type_att) => {
                return Err(ParseError::new_unrecognized_speed_type(
                    type_att.to_string(),
                    BulletMLParser::attribute_value_pos(&speed, "type"),
                ));
            }
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
            Some(type_att) => {
                return Err(ParseError::new_unrecognized_accel_dir_type(
                    type_att.to_string(),
                    BulletMLParser::attribute_value_pos(&horizontal, "type"),
                ));
            }
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
            Some(type_att) => {
                return Err(ParseError::new_unrecognized_accel_dir_type(
                    type_att.to_string(),
                    BulletMLParser::attribute_value_pos(&vertical, "type"),
                ));
            }
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
            return Err(ParseError::new_missing_attribute(
                "label".to_string(),
                bullet_ref.tag_name().name().to_string(),
                BulletMLParser::node_pos(&bullet_ref),
            ));
        };
        let id = self
            .arena
            .new_node(BulletMLNode::BulletRef(label.to_string()));
        for child in bullet_ref.children().filter(|n| n.is_element()) {
            let child_name = child.tag_name();
            let child_id = match child_name.name() {
                "param" => self.parse_param(child)?,
                name => {
                    return Err(ParseError::new_unexpected_element(
                        name.to_string(),
                        BulletMLParser::node_pos(&child),
                    ));
                }
            };
            id.append(child_id, &mut self.arena);
        }
        Ok(id)
    }

    fn parse_action_ref(&mut self, action_ref: roxmltree::Node) -> Result<NodeId, ParseError> {
        let label = action_ref.attribute("label");
        let label = if let Some(label) = label {
            label
        } else {
            return Err(ParseError::new_missing_attribute(
                "label".to_string(),
                action_ref.tag_name().name().to_string(),
                BulletMLParser::node_pos(&action_ref),
            ));
        };
        let id = self
            .arena
            .new_node(BulletMLNode::ActionRef(label.to_string()));
        for child in action_ref.children().filter(|n| n.is_element()) {
            let child_name = child.tag_name();
            let child_id = match child_name.name() {
                "param" => self.parse_param(child)?,
                name => {
                    return Err(ParseError::new_unexpected_element(
                        name.to_string(),
                        BulletMLParser::node_pos(&child),
                    ));
                }
            };
            id.append(child_id, &mut self.arena);
        }
        Ok(id)
    }

    fn parse_fire_ref(&mut self, fire_ref: roxmltree::Node) -> Result<NodeId, ParseError> {
        let label = fire_ref.attribute("label");
        let label = if let Some(label) = label {
            label
        } else {
            return Err(ParseError::new_missing_attribute(
                "label".to_string(),
                fire_ref.tag_name().name().to_string(),
                BulletMLParser::node_pos(&fire_ref),
            ));
        };
        let id = self
            .arena
            .new_node(BulletMLNode::FireRef(label.to_string()));
        for child in fire_ref.children().filter(|n| n.is_element()) {
            let child_name = child.tag_name();
            let child_id = match child_name.name() {
                "param" => self.parse_param(child)?,
                name => {
                    return Err(ParseError::new_unexpected_element(
                        name.to_string(),
                        BulletMLParser::node_pos(&child),
                    ));
                }
            };
            id.append(child_id, &mut self.arena);
        }
        Ok(id)
    }

    fn parse_param(&mut self, param: roxmltree::Node) -> Result<NodeId, ParseError> {
        let expr = self.parse_expression(param)?;
        let id = self.arena.new_node(BulletMLNode::Param(expr));
        Ok(id)
    }

    fn parse_expression(
        &mut self,
        parent: roxmltree::Node,
    ) -> Result<BulletMLExpression, ParseError> {
        let mut str: String = String::new();
        for child in parent.children() {
            let node_type = child.node_type();
            match node_type {
                roxmltree::NodeType::Text => {
                    str.push_str(child.text().unwrap());
                }
                roxmltree::NodeType::Root | roxmltree::NodeType::Element => {
                    return Err(ParseError::new_unexpected_node_type(
                        format!("{:?}", node_type),
                        BulletMLParser::node_pos(&child),
                    ));
                }
                roxmltree::NodeType::Comment | roxmltree::NodeType::PI => {}
            }
        }

        let constant = str.parse();
        if let Ok(constant) = constant {
            return Ok(BulletMLExpression::Const(constant));
        }

        let re = regex::Regex::new("\\$([0-9]+|rank|rand)").unwrap();
        let str = re.replace_all(&str, |captures: &regex::Captures| match &captures[1] {
            "rank" => "rank".to_string(),
            "rand" => "rand()".to_string(),
            v => {
                let maybe_num = v.parse::<u8>();
                match maybe_num {
                    Ok(num) => format!("v({})", num),
                    Err(..) => {
                        panic!("Unrecognized variable pattern ${}", v);
                    }
                }
            }
        });
        let expr_ref = self
            .expr_parser
            .parse_noclear(&str, &mut self.expr_slab.ps)
            .map_err(|err| {
                ParseError::new_expression(
                    err,
                    BulletMLParser::node_pos(parent.first_child().as_ref().unwrap_or(&parent)),
                )
            })?;
        Ok(BulletMLExpression::Expr(expr_ref))
    }

    #[inline]
    fn node_pos(node: &roxmltree::Node) -> ParseErrorPos {
        node.document().text_pos_at(node.range().start).into()
    }

    #[inline]
    fn attribute_value_pos(node: &roxmltree::Node, name: &str) -> ParseErrorPos {
        node.attribute_node(name)
            .map(|attr| node.document().text_pos_at(attr.value_range().start))
            .unwrap_or_else(|| TextPos { row: 0, col: 0 })
            .into()
    }
}

impl Default for BulletMLParser {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
#[cfg_attr(coverage_nightly, coverage(off))]
mod tests {
    use super::*;

    #[test]
    fn test_bulletml() {
        let bml = BulletMLParser::new()
            .parse(
                r##"<?xml version="1.0" ?>
<bulletml />"##,
            )
            .unwrap();
        assert_matches!(
            bml.arena[bml.root].get(),
            &BulletMLNode::BulletML { bml_type: None }
        );
    }

    #[test]
    fn test_bulletml_type_none() {
        let bml = BulletMLParser::new()
            .parse(
                r##"<?xml version="1.0" ?>
<bulletml type="none" />"##,
            )
            .unwrap();
        assert_matches!(
            bml.arena[bml.root].get(),
            &BulletMLNode::BulletML { bml_type: None }
        );
    }

    #[test]
    fn test_bulletml_type_vertical() {
        let bml = BulletMLParser::new()
            .parse(
                r##"<?xml version="1.0" ?>
<bulletml type="vertical" />"##,
            )
            .unwrap();
        assert_matches!(
            bml.arena[bml.root].get(),
            &BulletMLNode::BulletML {
                bml_type: Some(BulletMLType::Vertical)
            }
        );
    }

    #[test]
    fn test_bulletml_type_horizontal() {
        let bml = BulletMLParser::new()
            .parse(
                r##"<?xml version="1.0" ?>
<bulletml type="horizontal" />"##,
            )
            .unwrap();
        assert_matches!(
            bml.arena[bml.root].get(),
            &BulletMLNode::BulletML {
                bml_type: Some(BulletMLType::Horizontal)
            }
        );
    }

    #[test]
    fn test_full_bulletml() {
        // This covers all the good branches of the parser.
        BulletMLParser::new()
            .parse(
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
        let bml = BulletMLParser::new().parse(
            r##"<?xml version="1.0" ?>
<foo />"##,
        );
        let err = bml.unwrap_err();
        let (element, pos) = assert_matches!(
            err,
            ParseError::UnexpectedElement {
                ref element,
                pos,
                #[cfg(feature = "backtrace")]
                backtrace: _,
            } => (element, pos)
        );
        assert_eq!(element, "foo");
        assert_eq!((pos.row(), pos.col()), (2, 1));
        assert_eq!(
            format!("{}", &err),
            "Unexpected element foo at position 2:1"
        );
    }

    #[test]
    fn test_unrecognized_bml_type() {
        let bml = BulletMLParser::new().parse(
            r##"<?xml version="1.0" ?>
<bulletml type="foo" />"##,
        );
        let err = bml.unwrap_err();
        let (bml_type, pos) = assert_matches!(
            err,
            ParseError::UnrecognizedBmlType {
                ref bml_type,
                pos,
                #[cfg(feature = "backtrace")]
                backtrace: _,
            } => (bml_type, pos)
        );
        assert_eq!(bml_type, "foo");
        assert_eq!((pos.row(), pos.col()), (2, 17));
        assert_eq!(
            format!("{}", &err),
            "Unrecognized BulletML type foo at position 2:17"
        );
    }

    #[test]
    fn test_unexpected_bulletml_child() {
        let bml = BulletMLParser::new().parse(
            r##"<?xml version="1.0" ?>
<bulletml>
    <foo />
</bulletml>"##,
        );
        let err = bml.unwrap_err();
        let (element, pos) = assert_matches!(
            err,
            ParseError::UnexpectedElement {
                ref element,
                pos,
                #[cfg(feature = "backtrace")]
                backtrace: _,
            } => (element, pos)
        );
        assert_eq!(element, "foo");
        assert_eq!((pos.row(), pos.col()), (3, 5));
        assert_eq!(
            format!("{}", &err),
            "Unexpected element foo at position 3:5"
        );
    }

    #[test]
    fn test_unexpected_bullet_child() {
        let bml = BulletMLParser::new().parse(
            r##"<?xml version="1.0" ?>
<bulletml>
    <bullet>
        <foo />
    </bullet>
</bulletml>"##,
        );
        let err = bml.unwrap_err();
        let (element, pos) = assert_matches!(
            err,
            ParseError::UnexpectedElement {
                ref element,
                pos,
                #[cfg(feature = "backtrace")]
                backtrace: _,
            } => (element, pos)
        );
        assert_eq!(element, "foo");
        assert_eq!((pos.row(), pos.col()), (4, 9));
        assert_eq!(
            format!("{}", &err),
            "Unexpected element foo at position 4:9"
        );
    }

    #[test]
    fn test_unexpected_action_child() {
        let bml = BulletMLParser::new().parse(
            r##"<?xml version="1.0" ?>
<bulletml>
    <action>
        <foo />
    </action>
</bulletml>"##,
        );
        let err = bml.unwrap_err();
        let (element, pos) = assert_matches!(
            err,
            ParseError::UnexpectedElement {
                ref element,
                pos,
                #[cfg(feature = "backtrace")]
                backtrace: _,
            } => (element, pos)
        );
        assert_eq!(element, "foo");
        assert_eq!((pos.row(), pos.col()), (4, 9));
        assert_eq!(
            format!("{}", &err),
            "Unexpected element foo at position 4:9"
        );
    }

    #[test]
    fn test_unexpected_fire_child() {
        let bml = BulletMLParser::new().parse(
            r##"<?xml version="1.0" ?>
<bulletml>
    <fire>
        <foo />
    </fire>
</bulletml>"##,
        );
        let err = bml.unwrap_err();
        let (element, pos) = assert_matches!(
            err,
            ParseError::UnexpectedElement {
                ref element,
                pos,
                #[cfg(feature = "backtrace")]
                backtrace: _,
            } => (element, pos)
        );
        assert_eq!(element, "foo");
        assert_eq!((pos.row(), pos.col()), (4, 9));
        assert_eq!(
            format!("{}", &err),
            "Unexpected element foo at position 4:9"
        );
    }

    #[test]
    fn test_unexpected_change_direction_child() {
        let bml = BulletMLParser::new().parse(
            r##"<?xml version="1.0" ?>
<bulletml>
    <action>
        <changeDirection>
            <foo />
        </changeDirection>
    </action>
</bulletml>"##,
        );
        let err = bml.unwrap_err();
        let (element, pos) = assert_matches!(
            err,
            ParseError::UnexpectedElement {
                ref element,
                pos,
                #[cfg(feature = "backtrace")]
                backtrace: _,
            } => (element, pos)
        );
        assert_eq!(element, "foo");
        assert_eq!((pos.row(), pos.col()), (5, 13));
        assert_eq!(
            format!("{}", &err),
            "Unexpected element foo at position 5:13"
        );
    }

    #[test]
    fn test_unexpected_change_speed_child() {
        let bml = BulletMLParser::new().parse(
            r##"<?xml version="1.0" ?>
<bulletml>
    <action>
        <changeSpeed>
            <foo />
        </changeSpeed>
    </action>
</bulletml>"##,
        );
        let err = bml.unwrap_err();
        let (element, pos) = assert_matches!(
            err,
            ParseError::UnexpectedElement {
                ref element,
                pos,
                #[cfg(feature = "backtrace")]
                backtrace: _,
            } => (element, pos)
        );
        assert_eq!(element, "foo");
        assert_eq!((pos.row(), pos.col()), (5, 13));
        assert_eq!(
            format!("{}", &err),
            "Unexpected element foo at position 5:13"
        );
    }

    #[test]
    fn test_unexpected_accel_child() {
        let bml = BulletMLParser::new().parse(
            r##"<?xml version="1.0" ?>
<bulletml>
    <action>
        <accel>
            <foo />
        </accel>
    </action>
</bulletml>"##,
        );
        let err = bml.unwrap_err();
        let (element, pos) = assert_matches!(
            err,
            ParseError::UnexpectedElement {
                ref element,
                pos,
                #[cfg(feature = "backtrace")]
                backtrace: _,
            } => (element, pos)
        );
        assert_eq!(element, "foo");
        assert_eq!((pos.row(), pos.col()), (5, 13));
        assert_eq!(
            format!("{}", &err),
            "Unexpected element foo at position 5:13"
        );
    }

    #[test]
    fn test_unexpected_repeat_child() {
        let bml = BulletMLParser::new().parse(
            r##"<?xml version="1.0" ?>
<bulletml>
    <action>
        <repeat>
            <foo />
        </repeat>
    </action>
</bulletml>"##,
        );
        let err = bml.unwrap_err();
        let (element, pos) = assert_matches!(
            err,
            ParseError::UnexpectedElement {
                ref element,
                pos,
                #[cfg(feature = "backtrace")]
                backtrace: _,
            } => (element, pos)
        );
        assert_eq!(element, "foo");
        assert_eq!((pos.row(), pos.col()), (5, 13));
        assert_eq!(
            format!("{}", &err),
            "Unexpected element foo at position 5:13"
        );
    }

    #[test]
    fn test_unrecognized_direction_type() {
        let bml = BulletMLParser::new().parse(
            r##"<?xml version="1.0" ?>
<bulletml>
    <bullet>
        <direction type="foo" />
    </bullet>
</bulletml>"##,
        );
        let err = bml.unwrap_err();
        let (dir_type, pos) = assert_matches!(
            err,
            ParseError::UnrecognizedDirectionType {
                ref dir_type,
                pos,
                #[cfg(feature = "backtrace")]
                backtrace: _,
            } => (dir_type, pos)
        );
        assert_eq!(dir_type, "foo");
        assert_eq!((pos.row(), pos.col()), (4, 26));
        assert_eq!(
            format!("{}", &err),
            "Unrecognized direction type foo at position 4:26"
        );
    }

    #[test]
    fn test_unrecognized_speed_type() {
        let bml = BulletMLParser::new().parse(
            r##"<?xml version="1.0" ?>
<bulletml>
    <bullet>
        <speed type="foo" />
    </bullet>
</bulletml>"##,
        );
        let err = bml.unwrap_err();
        let (speed_type, pos) = assert_matches!(
            err,
            ParseError::UnrecognizedSpeedType {
                ref speed_type,
                pos,
                #[cfg(feature = "backtrace")]
                backtrace: _,
            } => (speed_type, pos)
        );
        assert_eq!(speed_type, "foo");
        assert_eq!((pos.row(), pos.col()), (4, 22));
        assert_eq!(
            format!("{}", &err),
            "Unrecognized speed type foo at position 4:22"
        );
    }

    #[test]
    fn test_unrecognized_accel_horizontal_type() {
        let bml = BulletMLParser::new().parse(
            r##"<?xml version="1.0" ?>
<bulletml>
    <action>
        <accel>
            <horizontal type="foo" />
        </accel>
    </action>
</bulletml>"##,
        );
        let err = bml.unwrap_err();
        let (accel_dir_type, pos) = assert_matches!(
            err,
            ParseError::UnrecognizedAccelDirType {
                ref accel_dir_type,
                pos,
                #[cfg(feature = "backtrace")]
                backtrace: _,
            } => (accel_dir_type, pos)
        );
        assert_eq!(accel_dir_type, "foo");
        assert_eq!((pos.row(), pos.col()), (5, 31));
        assert_eq!(
            format!("{}", &err),
            "Unrecognized acceleration direction type foo at position 5:31"
        );
    }

    #[test]
    fn test_unrecognized_accel_vertical_type() {
        let bml = BulletMLParser::new().parse(
            r##"<?xml version="1.0" ?>
<bulletml>
    <action>
        <accel>
            <vertical type="foo" />
        </accel>
    </action>
</bulletml>"##,
        );
        let err = bml.unwrap_err();
        let (accel_dir_type, pos) = assert_matches!(
            err,
            ParseError::UnrecognizedAccelDirType {
                ref accel_dir_type,
                pos,
                #[cfg(feature = "backtrace")]
                backtrace: _,
            } => (accel_dir_type, pos)
        );
        assert_eq!(accel_dir_type, "foo");
        assert_eq!((pos.row(), pos.col()), (5, 29));
        assert_eq!(
            format!("{}", &err),
            "Unrecognized acceleration direction type foo at position 5:29"
        );
    }

    #[test]
    fn test_missing_bullet_ref_label() {
        let bml = BulletMLParser::new().parse(
            r##"<?xml version="1.0" ?>
<bulletml>
    <fire>
        <bulletRef />
    </fire>
</bulletml>"##,
        );
        let err = bml.unwrap_err();
        let (attribute, element, pos) = assert_matches!(
            err,
            ParseError::MissingAttribute {
                ref attribute,
                ref element,
                pos,
                #[cfg(feature = "backtrace")]
                backtrace: _,
            } => (attribute, element, pos)
        );
        assert_eq!(attribute, "label");
        assert_eq!(element, "bulletRef");
        assert_eq!((pos.row(), pos.col()), (4, 9));
        assert_eq!(
            format!("{}", &err),
            "Missing attribute label in element bulletRef at position 4:9"
        );
    }

    #[test]
    fn test_unexpected_bullet_ref_child() {
        let bml = BulletMLParser::new().parse(
            r##"<?xml version="1.0" ?>
<bulletml>
    <fire>
        <bulletRef label="bar">
            <foo />
        </bulletRef>
    </fire>
</bulletml>"##,
        );
        let err = bml.unwrap_err();
        let (element, pos) = assert_matches!(
            err,
            ParseError::UnexpectedElement {
                ref element,
                pos,
                #[cfg(feature = "backtrace")]
                backtrace: _,
            } => (element, pos)
        );
        assert_eq!(element, "foo");
        assert_eq!((pos.row(), pos.col()), (5, 13));
        assert_eq!(
            format!("{}", &err),
            "Unexpected element foo at position 5:13"
        );
    }

    #[test]
    fn test_missing_action_ref_label() {
        let bml = BulletMLParser::new().parse(
            r##"<?xml version="1.0" ?>
<bulletml>
    <bullet>
        <actionRef />
    </bullet>
</bulletml>"##,
        );
        let err = bml.unwrap_err();
        let (attribute, element, pos) = assert_matches!(
            err,
            ParseError::MissingAttribute {
                ref attribute,
                ref element,
                pos,
                #[cfg(feature = "backtrace")]
                backtrace: _,
            } => (attribute, element, pos)
        );
        assert_eq!(attribute, "label");
        assert_eq!(element, "actionRef");
        assert_eq!((pos.row(), pos.col()), (4, 9));
        assert_eq!(
            format!("{}", &err),
            "Missing attribute label in element actionRef at position 4:9"
        );
    }

    #[test]
    fn test_unexpected_action_ref_child() {
        let bml = BulletMLParser::new().parse(
            r##"<?xml version="1.0" ?>
<bulletml>
    <bullet>
        <actionRef label="bar">
            <foo />
        </actionRef>
    </bullet>
</bulletml>"##,
        );
        let err = bml.unwrap_err();
        let (element, pos) = assert_matches!(
            err,
            ParseError::UnexpectedElement {
                ref element,
                pos,
                #[cfg(feature = "backtrace")]
                backtrace: _,
            } => (element, pos)
        );
        assert_eq!(element, "foo");
        assert_eq!((pos.row(), pos.col()), (5, 13));
        assert_eq!(
            format!("{}", &err),
            "Unexpected element foo at position 5:13"
        );
    }

    #[test]
    fn test_missing_fire_ref_label() {
        let bml = BulletMLParser::new().parse(
            r##"<?xml version="1.0" ?>
<bulletml>
    <action>
        <fireRef />
    </action>
</bulletml>"##,
        );
        let err = bml.unwrap_err();
        let (attribute, element, pos) = assert_matches!(
            err,
            ParseError::MissingAttribute {
                ref attribute,
                ref element,
                pos,
                #[cfg(feature = "backtrace")]
                backtrace: _,
            } => (attribute, element, pos)
        );
        assert_eq!(attribute, "label");
        assert_eq!(element, "fireRef");
        assert_eq!((pos.row(), pos.col()), (4, 9));
        assert_eq!(
            format!("{}", &err),
            "Missing attribute label in element fireRef at position 4:9"
        );
    }

    #[test]
    fn test_unexpected_fire_ref_child() {
        let bml = BulletMLParser::new().parse(
            r##"<?xml version="1.0" ?>
<bulletml>
    <action>
        <fireRef label="bar">
            <foo />
        </fireRef>
    </action>
</bulletml>"##,
        );
        let err = bml.unwrap_err();
        let (element, pos) = assert_matches!(
            err,
            ParseError::UnexpectedElement {
                ref element,
                pos,
                #[cfg(feature = "backtrace")]
                backtrace: _,
            } => (element, pos)
        );
        assert_eq!(element, "foo");
        assert_eq!((pos.row(), pos.col()), (5, 13));
        assert_eq!(
            format!("{}", &err),
            "Unexpected element foo at position 5:13"
        );
    }

    #[test]
    fn test_unexpected_node_type_in_expression() {
        let bml = BulletMLParser::new().parse(
            r##"<?xml version="1.0" ?>
<bulletml>
    <bullet>
        <direction><foo /></direction>
    </bullet>
</bulletml>"##,
        );
        let err = bml.unwrap_err();
        let (node_type, pos) = assert_matches!(
            err,
            ParseError::UnexpectedNodeType {
                ref node_type,
                pos,
                #[cfg(feature = "backtrace")]
                backtrace: _,
            } => (node_type, pos)
        );
        assert_eq!(node_type, "Element");
        assert_eq!((pos.row(), pos.col()), (4, 20));
        assert_eq!(
            format!("{}", &err),
            "Unexpected node of type Element at position 4:20"
        );
    }

    #[test]
    fn test_expression_error() {
        use std::error::Error;

        let bml = BulletMLParser::new().parse(
            r##"<?xml version="1.0" ?>
<bulletml>
    <bullet>
        <direction>-</direction>
    </bullet>
</bulletml>"##,
        );
        let err = bml.unwrap_err();
        let pos = assert_matches!(
            err,
            ParseError::Expression {
                source: _,
                pos,
                #[cfg(feature = "backtrace")]
                backtrace: _,
            } => pos
        );
        assert_eq!((pos.row(), pos.col()), (4, 20));
        let cause = err.source().unwrap().downcast_ref::<fasteval::Error>();
        assert_matches!(
            cause,
            Some(&fasteval::Error::EofWhileParsing(ref s)) if s.as_str() == "value"
        );
        assert_eq!(format!("{}", &err), "Expression error at position 4:20");
    }
}
